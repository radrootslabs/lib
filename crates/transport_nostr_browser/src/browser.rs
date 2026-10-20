use crate::state::{MAX_QUEUE_BYTES, SendRequest, Shared, Status, payload_len};
use futures::{
    channel::oneshot,
    future::{Either, poll_fn, select},
    lock::Mutex,
};
use radroots_transport::host::websocket::{Admission, Error, Frame, IoFuture, Socket};
use std::{num::NonZeroUsize, pin::pin, sync::Arc, task::Poll, time::Duration};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{BinaryType, Event, MessageEvent, WebSocket};
use web_time::Instant;

pub(crate) async fn connect(
    endpoint: &str,
    timeout: Duration,
    limit: NonZeroUsize,
    admission: Arc<dyn Admission>,
    scope: &crate::scope::Scope,
) -> Result<Arc<dyn Socket>, Error> {
    poll_fn(|context| {
        Poll::Ready(
            if admission.admitted_read(context) && admission.admitted_write(context) {
                Ok(())
            } else {
                Err(Error::Policy)
            },
        )
    })
    .await?;
    let shared = Arc::new(Shared::new(admission, limit.get()));
    scope.register(&shared)?;
    let (sender, receiver) = oneshot::channel();
    let mut cancellation = CancelConnect(Some(shared.clone()));
    start_owner(
        endpoint.to_owned(),
        Instant::now() + timeout,
        shared,
        sender,
    );
    let result = receiver.await.unwrap_or(Err(Error::Closed));
    if result.is_ok() {
        cancellation.0.take();
    }
    result
}

struct CancelConnect(Option<Arc<Shared>>);
impl Drop for CancelConnect {
    fn drop(&mut self) {
        if let Some(shared) = &self.0 {
            shared.stop();
        }
    }
}

// This synchronous handoff is the only place the Send future touches the local
// executor. The spawned future, DOM values and callbacks never leave it.
fn start_owner(
    endpoint: String,
    deadline: Instant,
    shared: Arc<Shared>,
    ready: oneshot::Sender<Result<Arc<dyn Socket>, Error>>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let result = poll_fn(|context| {
            // The local owner may start after the original caller yielded.
            // Recheck the original grant before constructing the DOM socket.
            if !shared.admission.admitted_read(context) || !shared.admission.admitted_write(context)
            {
                return Poll::Ready(Err(Error::Policy));
            }
            Poll::Ready(Owner::open(&endpoint, shared.clone(), deadline))
        })
        .await;
        let mut owner = match result {
            Ok(owner) => owner,
            Err(error) => {
                shared.fail(error);
                shared.finish();
                let _ = ready.send(Err(error));
                return;
            }
        };
        let opening = poll_fn(|context| {
            shared.actor.register(context.waker());
            if !shared.admission.admitted_read(context) || !shared.admission.admitted_write(context)
            {
                return Poll::Ready(Err(Error::Policy));
            }
            let state = shared.lock()?;
            Poll::Ready(match state.status {
                Status::Open if !state.stopping => Ok(()),
                Status::Failed(error) => Err(error),
                Status::Ended => Err(Error::Closed),
                _ if state.stopping => Err(Error::Closed),
                _ => return Poll::Pending,
            })
        });
        let opened = match select(pin!(opening), pin!(sleep_until(deadline))).await {
            Either::Left((result, _)) => result,
            Either::Right(_) => Err(Error::Timeout),
        };
        if let Err(error) = opened {
            shared.fail(error);
            let _ = ready.send(Err(error));
            return;
        }
        let socket: Arc<dyn Socket> = Arc::new(BrowserSocket {
            shared: shared.clone(),
            sending: Mutex::new(()),
            receiving: Mutex::new(()),
            closing: Mutex::new(()),
        });
        if ready.send(Ok(socket)).is_err() {
            return;
        }
        owner.drive().await;
    });
}

async fn sleep_until(deadline: Instant) {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        let millis = remaining
            .as_nanos()
            .div_ceil(1_000_000)
            .min(i32::MAX as u128) as u64;
        async_utility::time::sleep(Duration::from_millis(millis)).await;
    }
}

struct BrowserSocket {
    shared: Arc<Shared>,
    sending: Mutex<()>,
    receiving: Mutex<()>,
    closing: Mutex<()>,
}

impl Drop for BrowserSocket {
    fn drop(&mut self) {
        self.shared.stop();
    }
}

impl Socket for BrowserSocket {
    fn send(&self, frame: Frame) -> IoFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            let bytes = payload_len(&frame)?;
            if bytes > self.shared.limit {
                return Err(Error::Protocol);
            }
            let _guard = self.sending.lock().await;
            let (sender, receiver) = oneshot::channel();
            {
                let mut state = self.shared.lock()?;
                if state.stopping || state.status != Status::Open {
                    return Err(Error::Closed);
                }
                // Cancellation of a send observer does not erase admitted I/O.
                if state.outgoing.is_some() {
                    return Err(Error::Policy);
                }
                state.outgoing = Some(SendRequest {
                    frame,
                    result: sender,
                });
            }
            self.shared.actor.wake();
            receiver.await.unwrap_or(Err(Error::Closed))
        })
    }

    fn receive(&self) -> IoFuture<'_, Result<Option<Frame>, Error>> {
        Box::pin(async move {
            let _guard = self.receiving.lock().await;
            poll_fn(|context| {
                self.shared.reader.register(context.waker());
                if !self.shared.admission.admitted_read(context) {
                    self.shared.fail(Error::Policy);
                    return Poll::Ready(Err(Error::Policy));
                }
                if let Some(frame) = self.shared.pop()? {
                    return Poll::Ready(Ok(Some(frame)));
                }
                let state = self.shared.lock()?;
                if state.finished || state.stopping || state.status == Status::Ended {
                    Poll::Ready(Ok(None))
                } else {
                    Poll::Pending
                }
            })
            .await
        })
    }

    fn close(&self) -> IoFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            let _guard = self.closing.lock().await;
            self.shared.stop();
            poll_fn(|context| {
                self.shared.closer.register(context.waker());
                let finished = self.shared.lock()?.finished;
                if finished {
                    self.shared.closer.take();
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
            .await
        })
    }
}

struct Owner {
    socket: WebSocket,
    shared: Arc<Shared>,
    _open: Closure<dyn FnMut(Event)>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _error: Closure<dyn FnMut(Event)>,
    _close: Closure<dyn FnMut(Event)>,
}

impl Owner {
    fn open(endpoint: &str, shared: Arc<Shared>, deadline: Instant) -> Result<Self, Error> {
        if shared.lock()?.stopping {
            return Err(Error::Closed);
        }
        if Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
        let socket = WebSocket::new(endpoint).map_err(|_| Error::Unavailable)?;
        socket.set_binary_type(BinaryType::Arraybuffer);
        let state = shared.clone();
        let open = Closure::new(move |_: Event| {
            if let Ok(mut inner) = state.lock()
                && inner.status == Status::Connecting
                && !inner.stopping
            {
                inner.status = Status::Open;
            }
            state.wake();
        });
        let state = shared.clone();
        let message = Closure::new(move |event: MessageEvent| {
            let frame = decode_message(event, state.limit).and_then(|frame| state.push(frame));
            if let Err(error) = frame {
                state.fail(error);
            }
        });
        let state = shared.clone();
        let error = Closure::new(move |_: Event| state.fail(Error::Unavailable));
        let state = shared.clone();
        let close = Closure::new(move |_: Event| {
            if let Ok(mut inner) = state.lock()
                && !matches!(inner.status, Status::Failed(_))
            {
                inner.status = Status::Ended;
            }
            state.wake();
        });
        socket.set_onopen(Some(open.as_ref().unchecked_ref()));
        socket.set_onmessage(Some(message.as_ref().unchecked_ref()));
        socket.set_onerror(Some(error.as_ref().unchecked_ref()));
        socket.set_onclose(Some(close.as_ref().unchecked_ref()));
        Ok(Self {
            socket,
            shared,
            _open: open,
            _message: message,
            _error: error,
            _close: close,
        })
    }

    async fn drive(&mut self) {
        poll_fn(|context| {
            self.shared.actor.register(context.waker());
            if !self.shared.admission.admitted_read(context)
                || !self.shared.admission.admitted_write(context)
            {
                self.shared.fail(Error::Policy);
            }
            let request = {
                let mut state = match self.shared.lock() {
                    Ok(state) => state,
                    Err(_) => return Poll::Ready(()),
                };
                if state.stopping || state.status != Status::Open {
                    return Poll::Ready(());
                }
                state.outgoing.take()
            };
            if let Some(request) = request {
                let result = self.send(request.frame);
                if let Err(error) = result {
                    self.shared.fail(error);
                }
                let _ = request.result.send(result);
                if result.is_err() {
                    return Poll::Ready(());
                }
            }
            Poll::Pending
        })
        .await;
    }

    fn send(&self, frame: Frame) -> Result<(), Error> {
        let bytes = payload_len(&frame)?;
        if bytes > self.shared.limit {
            return Err(Error::Protocol);
        }
        if u64::from(self.socket.buffered_amount()) + bytes as u64 > MAX_QUEUE_BYTES as u64 {
            return Err(Error::Policy);
        }
        match frame {
            Frame::Text(text) => self.socket.send_with_str(&text),
            Frame::Binary(bytes) => self.socket.send_with_u8_array(&bytes),
            _ => return Err(Error::Unsupported),
        }
        .map_err(|_| Error::Unavailable)
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.socket.set_onopen(None);
        self.socket.set_onmessage(None);
        self.socket.set_onerror(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
        self.shared.finish();
    }
}

fn decode_message(event: MessageEvent, limit: usize) -> Result<Frame, Error> {
    let data = event.data();
    if data.is_string() {
        let value: js_sys::JsString = data.unchecked_into();
        if value.length() as usize > limit {
            return Err(Error::Protocol);
        }
        let text: String = value.into();
        if text.len() > limit {
            return Err(Error::Protocol);
        }
        Ok(Frame::Text(text))
    } else if data.is_instance_of::<js_sys::ArrayBuffer>() {
        let buffer: js_sys::ArrayBuffer = data.unchecked_into();
        if buffer.byte_length() as usize > limit {
            return Err(Error::Protocol);
        }
        Ok(Frame::Binary(js_sys::Uint8Array::new(&buffer).to_vec()))
    } else {
        Err(Error::Protocol)
    }
}

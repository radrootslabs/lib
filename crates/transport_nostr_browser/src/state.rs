//! Bounded owned data only. No JavaScript values cross this boundary.

use futures::{channel::oneshot, task::AtomicWaker};
use radroots_transport::host::websocket::{Admission, Error, Frame};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};

pub(crate) const MAX_MESSAGE: usize = 512 * 1024;
pub(crate) const MAX_QUEUE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_QUEUE_MESSAGES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Status {
    Connecting,
    Open,
    Ended,
    Failed(Error),
}

pub(crate) struct SendRequest {
    pub(crate) frame: Frame,
    pub(crate) result: oneshot::Sender<Result<(), Error>>,
}

pub(crate) struct State {
    pub(crate) status: Status,
    pub(crate) stopping: bool,
    pub(crate) finished: bool,
    pub(crate) outgoing: Option<SendRequest>,
    incoming: VecDeque<Frame>,
    bytes: usize,
}

impl State {
    fn check_enqueue(&self, bytes: usize, limit: usize) -> Result<(), Error> {
        if self.stopping || self.status != Status::Open {
            return Err(Error::Closed);
        }
        if bytes > limit
            || self.incoming.len() >= MAX_QUEUE_MESSAGES
            || bytes > MAX_QUEUE_BYTES.saturating_sub(self.bytes)
        {
            return Err(Error::Policy);
        }
        Ok(())
    }
}

pub(crate) struct Shared {
    state: Mutex<State>,
    pub(crate) admission: Arc<dyn Admission>,
    pub(crate) limit: usize,
    pub(crate) actor: AtomicWaker,
    pub(crate) reader: AtomicWaker,
    pub(crate) closer: AtomicWaker,
}

impl Shared {
    pub(crate) fn new(admission: Arc<dyn Admission>, limit: usize) -> Self {
        Self {
            state: Mutex::new(State {
                status: Status::Connecting,
                stopping: false,
                finished: false,
                outgoing: None,
                incoming: VecDeque::new(),
                bytes: 0,
            }),
            admission,
            limit: limit.min(MAX_MESSAGE),
            actor: AtomicWaker::new(),
            reader: AtomicWaker::new(),
            closer: AtomicWaker::new(),
        }
    }

    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, State>, Error> {
        self.state.lock().map_err(|_| Error::Closed)
    }

    pub(crate) fn wake(&self) {
        self.actor.wake();
        self.reader.wake();
        self.closer.wake();
    }

    pub(crate) fn stop(&self) {
        if let Ok(mut state) = self.lock() {
            state.stopping = true;
        }
        self.wake();
    }

    pub(crate) fn fail(&self, error: Error) {
        if let Ok(mut state) = self.lock() {
            // The first refusal remains authoritative through later callbacks.
            if !matches!(state.status, Status::Failed(_)) {
                state.status = Status::Failed(error);
            }
            state.incoming.clear();
            state.bytes = 0;
        }
        self.wake();
    }

    pub(crate) fn push(&self, frame: Frame) -> Result<(), Error> {
        let bytes = payload_len(&frame)?;
        self.lock()?.check_enqueue(bytes, self.limit)?;
        // Admission may wake or cancel an observer. Call it without our lock,
        // then revalidate closure and queue credit before exposing the frame.
        if !self.admission.charge(bytes, 1, 1) {
            return Err(Error::Policy);
        }
        let mut state = self.lock()?;
        state.check_enqueue(bytes, self.limit)?;
        state.bytes += bytes;
        state.incoming.push_back(frame);
        drop(state);
        self.wake();
        Ok(())
    }

    pub(crate) fn pop(&self) -> Result<Option<Frame>, Error> {
        let mut state = self.lock()?;
        if let Status::Failed(error) = state.status {
            return Err(error);
        }
        let frame = state.incoming.pop_front();
        if let Some(frame) = &frame {
            state.bytes -= payload_len(frame)?;
        }
        Ok(frame)
    }

    pub(crate) fn finish(&self) {
        let outgoing = if let Ok(mut state) = self.lock() {
            state.finished = true;
            if !matches!(state.status, Status::Failed(_)) {
                state.status = Status::Ended;
            }
            state.outgoing.take()
        } else {
            None
        };
        if let Some(request) = outgoing {
            let _ = request.result.send(Err(Error::Closed));
        }
        self.wake();
    }
}

pub(crate) fn payload_len(frame: &Frame) -> Result<usize, Error> {
    match frame {
        Frame::Text(text) => Ok(text.len()),
        Frame::Binary(bytes) => Ok(bytes.len()),
        _ => Err(Error::Unsupported),
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
pub(crate) mod tests;

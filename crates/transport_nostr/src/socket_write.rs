//! One serialized writer for SDK messages and exact signed-event publication.

use async_wsocket::Message;
use core::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use futures::{Sink, SinkExt, lock::Mutex};
use nostr_relay_pool::transport::{error::TransportError, websocket::WebSocketSink};
use radroots_transport::BoxFuture;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex as RegistryMutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::relay::policy_error;

pub(crate) struct SocketWriter {
    sink: Mutex<WebSocketSink>,
    open: AtomicBool,
}

impl SocketWriter {
    pub(crate) fn new(sink: WebSocketSink) -> Arc<Self> {
        Arc::new(Self {
            sink: Mutex::new(sink),
            open: AtomicBool::new(true),
        })
    }

    pub(crate) async fn send(&self, message: Message) -> Result<(), TransportError> {
        let mut sink = self.sink.lock().await;
        if !self.open.load(Ordering::Acquire) {
            return Err(policy_error("relay writer is closed"));
        }
        let result = sink.send(message).await;
        if result.is_err() {
            self.invalidate();
        }
        result
    }

    fn invalidate(&self) {
        self.open.store(false, Ordering::Release);
    }

    async fn close(&self) -> Result<(), TransportError> {
        self.invalidate();
        self.sink.lock().await.close().await
    }
}

/// Configured keys only; the registry never owns a connection or event bytes.
#[derive(Clone)]
pub(crate) struct WriterRegistry(Arc<RegistryMutex<BTreeMap<String, Weak<SocketWriter>>>>);

impl WriterRegistry {
    pub(crate) fn new(keys: impl Iterator<Item = String>) -> Self {
        Self(Arc::new(RegistryMutex::new(
            keys.map(|key| (key, Weak::new())).collect(),
        )))
    }

    pub(crate) fn install(
        &self,
        key: &str,
        writer: &Arc<SocketWriter>,
    ) -> Result<(), TransportError> {
        let mut entries = self
            .0
            .lock()
            .map_err(|_| policy_error("relay writer registry unavailable"))?;
        let slot = entries
            .get_mut(key)
            .ok_or_else(|| policy_error("relay writer is not configured"))?;
        if let Some(previous) = slot.upgrade() {
            previous.invalidate();
        }
        *slot = Arc::downgrade(writer);
        Ok(())
    }

    pub(crate) fn get(&self, key: &str) -> Result<Arc<SocketWriter>, TransportError> {
        self.0
            .lock()
            .map_err(|_| policy_error("relay writer registry unavailable"))?
            .get(key)
            .and_then(Weak::upgrade)
            .filter(|writer| writer.open.load(Ordering::Acquire))
            .ok_or_else(|| policy_error("relay writer is unavailable"))
    }
}

impl fmt::Debug for WriterRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WriterRegistry([redacted])")
    }
}

/// The SDK retains connection ownership; dropping its sink revokes raw writes.
pub(crate) struct SharedSocketSink {
    writer: Arc<SocketWriter>,
    pending: Option<BoxFuture<'static, Result<(), TransportError>>>,
    closing: bool,
}

impl SharedSocketSink {
    pub(crate) fn new(writer: Arc<SocketWriter>) -> Self {
        Self {
            writer,
            pending: None,
            closing: false,
        }
    }

    fn poll_pending(&mut self, context: &mut Context<'_>) -> Poll<Result<(), TransportError>> {
        if let Some(pending) = &mut self.pending {
            let result = futures::ready!(pending.as_mut().poll(context));
            self.pending = None;
            return Poll::Ready(result);
        }
        Poll::Ready(Ok(()))
    }
}

impl Sink<Message> for SharedSocketSink {
    type Error = TransportError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        if self.closing {
            return Poll::Ready(Err(policy_error("relay writer is closing")));
        }
        self.poll_pending(context)
    }

    fn start_send(mut self: Pin<&mut Self>, message: Message) -> Result<(), Self::Error> {
        if self.closing || self.pending.is_some() {
            return Err(policy_error("relay writer is not ready"));
        }
        let writer = Arc::clone(&self.writer);
        self.pending = Some(Box::pin(async move { writer.send(message).await }));
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.poll_pending(context)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        futures::ready!(self.poll_pending(context))?;
        if !self.closing {
            self.closing = true;
            self.writer.invalidate();
            let writer = Arc::clone(&self.writer);
            self.pending = Some(Box::pin(async move { writer.close().await }));
        }
        self.poll_pending(context)
    }
}

impl Drop for SharedSocketSink {
    fn drop(&mut self) {
        self.writer.invalidate();
    }
}

#[cfg(test)]
#[path = "socket_write_tests.rs"]
mod tests;

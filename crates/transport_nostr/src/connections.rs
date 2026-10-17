//! Bounded connection registration and synchronous completion cleanup.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

pub(crate) struct Connections {
    closed: AtomicBool,
    active: AtomicUsize,
    maximum: usize,
    relays: Mutex<BTreeMap<String, nostr_sdk::relay::Relay>>,
}

impl Connections {
    pub(crate) fn new(maximum: usize) -> Self {
        Self {
            closed: AtomicBool::new(false),
            active: AtomicUsize::new(0),
            maximum,
            relays: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub(crate) fn has_active_calls(&self) -> bool {
        self.active.load(Ordering::Acquire) != 0
    }

    pub(crate) fn begin(self: &Arc<Self>) -> Result<Operation, radroots_transport::Error> {
        if self.is_closed() {
            return Err(radroots_transport::Error::TransportClosed);
        }
        self.active
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count.checked_add(1)
            })
            .map_err(|_| radroots_transport::Error::TransportUnavailable)?;
        let operation = Operation(Arc::clone(self));
        if self.is_closed() {
            return Err(radroots_transport::Error::TransportClosed);
        }
        Ok(operation)
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if !self.has_active_calls() {
            self.disconnect();
        }
    }

    pub(crate) fn register(
        &self,
        relay: nostr_sdk::relay::Relay,
    ) -> Result<(), nostr_sdk::error::Error> {
        let mut relays = self.relays.lock().map_err(|_| {
            nostr_sdk::error::Error::with_static_message(
                nostr_sdk::error::ErrorKind::State,
                "connection registration is unavailable",
            )
        })?;
        let url = relay.url().to_string();
        if !relays.contains_key(&url) && relays.len() >= self.maximum {
            return Err(nostr_sdk::error::Error::with_static_message(
                nostr_sdk::error::ErrorKind::LimitExceeded,
                "connection registration limit reached",
            ));
        }
        relays.insert(url, relay);
        Ok(())
    }

    fn disconnect(&self) {
        // Disposal alone may recover poisoned state; it never authorizes work.
        let relays = self
            .relays
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for relay in relays.values() {
            relay.disconnect();
        }
    }
}

impl fmt::Debug for Connections {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Connections")
            .field("admission_closed", &self.is_closed())
            .field("active_calls", &self.active.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

pub(crate) struct Operation(Arc<Connections>);

impl Drop for Operation {
    fn drop(&mut self) {
        if self.0.active.fetch_sub(1, Ordering::AcqRel) == 1 && self.0.is_closed() {
            self.0.disconnect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_closes_without_discarding_active_calls() {
        let connections = Arc::new(Connections::new(2));
        assert!(!connections.has_active_calls());
        let first = connections.begin().unwrap();
        let second = connections.begin().unwrap();
        connections.close();
        assert!(connections.is_closed());
        assert!(matches!(
            connections.begin(),
            Err(radroots_transport::Error::TransportClosed)
        ));
        drop(first);
        assert!(connections.has_active_calls());
        drop(second);
        assert!(!connections.has_active_calls());
        connections.close();
    }

    #[test]
    fn an_open_completed_call_does_not_close_admission_and_overflow_is_explicit() {
        let connections = Arc::new(Connections::new(1));
        drop(connections.begin().unwrap());
        assert!(!connections.is_closed());
        assert!(!connections.has_active_calls());
        connections.active.store(usize::MAX, Ordering::Release);
        assert!(matches!(
            connections.begin(),
            Err(radroots_transport::Error::TransportUnavailable)
        ));
        assert_eq!(connections.active.load(Ordering::Acquire), usize::MAX);
        connections.active.store(0, Ordering::Release);
        connections.close();
    }

    #[tokio::test]
    async fn registration_is_deduplicated_bounded_and_safe_after_poison() {
        let client = nostr_sdk::client::Client::default();
        client.add_relay("ws://127.0.0.1:19001").await.unwrap();
        client.add_relay("ws://127.0.0.1:19002").await.unwrap();
        let first = client.relay("ws://127.0.0.1:19001").await.unwrap().unwrap();
        let second = client.relay("ws://127.0.0.1:19002").await.unwrap().unwrap();
        let connections = Arc::new(Connections::new(1));
        connections.register(first.clone()).unwrap();
        connections.register(first.clone()).unwrap();
        assert_eq!(connections.relays.lock().unwrap().len(), 1);
        assert_eq!(
            connections.register(second).unwrap_err().kind(),
            nostr_sdk::error::ErrorKind::LimitExceeded
        );
        let debug = format!("{connections:?}");
        assert!(!debug.contains("19001"));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = connections.relays.lock().unwrap();
            panic!("poison connection registration");
        }));
        assert_eq!(
            connections.register(first).unwrap_err().kind(),
            nostr_sdk::error::ErrorKind::State
        );
        connections.close();
        assert!(connections.is_closed());
        client.shutdown().await;
    }

    #[test]
    fn dropping_a_pending_call_releases_its_registration_without_an_await() {
        use std::{
            future::Future,
            task::{Context, Poll, Waker},
        };
        let connections = Arc::new(Connections::new(1));
        let copy = Arc::clone(&connections);
        let mut call = Box::pin(async move {
            let _operation = copy.begin().unwrap();
            std::future::pending::<()>().await;
        });
        assert_eq!(
            call.as_mut().poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        );
        assert!(connections.has_active_calls());
        connections.close();
        drop(call);
        assert!(!connections.has_active_calls());
    }
}

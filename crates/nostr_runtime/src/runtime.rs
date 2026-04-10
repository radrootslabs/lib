use crate::error::RadrootsNostrRuntimeError;
use crate::store::RadrootsNostrEventStore;
use crate::types::{
    RadrootsNostrConnectionSnapshot, RadrootsNostrRuntimeEvent, RadrootsNostrSubscriptionHandle,
    RadrootsNostrSubscriptionPolicy, RadrootsNostrSubscriptionSpec, RadrootsNostrTrafficLight,
};
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use core::time::Duration;
use futures::StreamExt;
use radroots_nostr::prelude::{
    RadrootsNostrClient, RadrootsNostrKeys, RadrootsNostrMonitor, RadrootsNostrMonitorNotification,
    RadrootsNostrRelayStatus, RadrootsNostrRelayUrl, RadrootsNostrTimestamp,
};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct RadrootsNostrRuntimeBuilder {
    keys: Option<RadrootsNostrKeys>,
    relays: Vec<String>,
    queue_capacity: usize,
    monitor_capacity: usize,
    event_store: Option<Arc<dyn RadrootsNostrEventStore>>,
}

impl RadrootsNostrRuntimeBuilder {
    pub const DEFAULT_QUEUE_CAPACITY: usize = 2_048;
    pub const DEFAULT_MONITOR_CAPACITY: usize = 2_048;

    pub fn new() -> Self {
        Self {
            keys: None,
            relays: Vec::new(),
            queue_capacity: Self::DEFAULT_QUEUE_CAPACITY,
            monitor_capacity: Self::DEFAULT_MONITOR_CAPACITY,
            event_store: None,
        }
    }

    pub fn keys(mut self, keys: RadrootsNostrKeys) -> Self {
        self.keys = Some(keys);
        self
    }

    pub fn relays(mut self, relays: Vec<String>) -> Self {
        self.relays = relays;
        self
    }

    pub fn add_relay(mut self, relay: impl Into<String>) -> Self {
        self.relays.push(relay.into());
        self
    }

    pub fn queue_capacity(mut self, capacity: usize) -> Self {
        self.queue_capacity = capacity;
        self
    }

    pub fn monitor_capacity(mut self, capacity: usize) -> Self {
        self.monitor_capacity = capacity;
        self
    }

    pub fn event_store(mut self, store: Arc<dyn RadrootsNostrEventStore>) -> Self {
        self.event_store = Some(store);
        self
    }

    pub fn build(self) -> Result<RadrootsNostrRuntime, RadrootsNostrRuntimeError> {
        let keys = self
            .keys
            .ok_or(RadrootsNostrRuntimeError::MissingConfig("keys"))?;
        if self.relays.is_empty() {
            return Err(RadrootsNostrRuntimeError::MissingConfig("relays"));
        }
        if self.queue_capacity == 0 {
            return Err(RadrootsNostrRuntimeError::InvalidConfig("queue_capacity"));
        }
        if self.monitor_capacity == 0 {
            return Err(RadrootsNostrRuntimeError::InvalidConfig("monitor_capacity"));
        }

        let monitor = RadrootsNostrMonitor::new(self.monitor_capacity);
        let client = RadrootsNostrClient::new_with_monitor(keys, monitor);
        let (queue_tx, queue_rx) = mpsc::channel(self.queue_capacity);

        let inner = Arc::new(RadrootsNostrRuntimeInner {
            client,
            relays: Mutex::new(self.relays),
            queue_tx,
            queue_rx: Mutex::new(queue_rx),
            statuses: Mutex::new(HashMap::new()),
            last_error: Mutex::new(None),
            monitor_task: Mutex::new(None),
            subscription_tasks: Mutex::new(HashMap::new()),
            started: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
            next_subscription_id: AtomicU64::new(1),
            event_store: self.event_store,
        });

        Ok(RadrootsNostrRuntime { inner })
    }
}

impl Default for RadrootsNostrRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct RadrootsNostrRuntime {
    inner: Arc<RadrootsNostrRuntimeInner>,
}

struct RadrootsNostrRuntimeInner {
    client: RadrootsNostrClient,
    relays: Mutex<Vec<String>>,
    queue_tx: mpsc::Sender<RadrootsNostrRuntimeEvent>,
    queue_rx: Mutex<mpsc::Receiver<RadrootsNostrRuntimeEvent>>,
    statuses: Mutex<HashMap<RadrootsNostrRelayUrl, RadrootsNostrRelayStatus>>,
    last_error: Mutex<Option<String>>,
    monitor_task: Mutex<Option<JoinHandle<()>>>,
    subscription_tasks: Mutex<HashMap<String, JoinHandle<()>>>,
    started: AtomicBool,
    shutting_down: AtomicBool,
    next_subscription_id: AtomicU64,
    event_store: Option<Arc<dyn RadrootsNostrEventStore>>,
}

impl RadrootsNostrRuntime {
    pub async fn start(&self) -> Result<(), RadrootsNostrRuntimeError> {
        if self.inner.started.swap(true, Ordering::SeqCst) {
            return Err(RadrootsNostrRuntimeError::RuntimeAlreadyStarted);
        }
        self.inner.shutting_down.store(false, Ordering::SeqCst);

        let relays = self.relays();
        for relay in relays {
            if let Err(source) = self.inner.client.add_relay(relay.as_str()).await {
                let message = source.to_string();
                self.record_error(message.clone());
                let _ = self
                    .inner
                    .queue_tx
                    .send(RadrootsNostrRuntimeEvent::Error { message })
                    .await;
            }
        }

        self.spawn_monitor_watcher();
        self.inner.client.connect().await;
        let _ = self
            .inner
            .queue_tx
            .send(RadrootsNostrRuntimeEvent::RuntimeStarted)
            .await;

        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), RadrootsNostrRuntimeError> {
        if !self.inner.started.swap(false, Ordering::SeqCst) {
            return Err(RadrootsNostrRuntimeError::RuntimeNotStarted);
        }
        self.inner.shutting_down.store(true, Ordering::SeqCst);

        if let Ok(mut guard) = self.inner.subscription_tasks.lock() {
            for (_, handle) in guard.drain() {
                handle.abort();
            }
        }

        if let Ok(mut guard) = self.inner.monitor_task.lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }

        let _ = self
            .inner
            .queue_tx
            .send(RadrootsNostrRuntimeEvent::RuntimeStopped)
            .await;

        Ok(())
    }

    pub async fn subscribe(
        &self,
        spec: RadrootsNostrSubscriptionSpec,
    ) -> Result<RadrootsNostrSubscriptionHandle, RadrootsNostrRuntimeError> {
        if !self.inner.started.load(Ordering::SeqCst) {
            return Err(RadrootsNostrRuntimeError::RuntimeNotStarted);
        }

        let sequence = self
            .inner
            .next_subscription_id
            .fetch_add(1, Ordering::SeqCst);
        let id = alloc::format!("sub-{sequence}");
        let handle = RadrootsNostrSubscriptionHandle {
            id: id.clone(),
            name: spec.name.clone(),
        };

        let worker = spawn_subscription_worker(self.inner.clone(), id.clone(), spec);
        self.inner
            .subscription_tasks
            .lock()
            .map_err(|_| RadrootsNostrRuntimeError::Runtime("subscription lock poisoned".into()))?
            .insert(id, worker);

        Ok(handle)
    }

    pub async fn unsubscribe(
        &self,
        handle: &RadrootsNostrSubscriptionHandle,
    ) -> Result<(), RadrootsNostrRuntimeError> {
        let removed = self
            .inner
            .subscription_tasks
            .lock()
            .map_err(|_| RadrootsNostrRuntimeError::Runtime("subscription lock poisoned".into()))?
            .remove(handle.id.as_str());

        let task = removed.ok_or_else(|| {
            RadrootsNostrRuntimeError::SubscriptionNotFound(handle.id.to_string())
        })?;
        task.abort();
        let _ = self
            .inner
            .queue_tx
            .send(RadrootsNostrRuntimeEvent::SubscriptionClosed {
                id: handle.id.clone(),
            })
            .await;

        Ok(())
    }

    pub fn set_relays(&self, relays: Vec<String>) -> Result<(), RadrootsNostrRuntimeError> {
        if relays.is_empty() {
            return Err(RadrootsNostrRuntimeError::InvalidConfig("relays"));
        }
        self.inner
            .relays
            .lock()
            .map_err(|_| RadrootsNostrRuntimeError::Runtime("relays lock poisoned".into()))
            .map(|mut guard| {
                *guard = relays;
            })
    }

    pub fn relays(&self) -> Vec<String> {
        self.inner
            .relays
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn drain_events(&self, max: usize) -> Vec<RadrootsNostrRuntimeEvent> {
        if max == 0 {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(max);
        let mut guard = match self.inner.queue_rx.lock() {
            Ok(guard) => guard,
            Err(_) => return out,
        };

        for _ in 0..max {
            match guard.try_recv() {
                Ok(event) => out.push(event),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        }

        out
    }

    pub fn snapshot(&self) -> RadrootsNostrConnectionSnapshot {
        let statuses = self
            .inner
            .statuses
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let last_error = self
            .inner
            .last_error
            .lock()
            .ok()
            .and_then(|guard| guard.clone());

        let mut connected = 0usize;
        let mut connecting = 0usize;
        for (_, status) in statuses.iter() {
            match status {
                RadrootsNostrRelayStatus::Connected => connected += 1,
                RadrootsNostrRelayStatus::Connecting => connecting += 1,
                _ => {}
            }
        }

        let light = if connected > 0 {
            RadrootsNostrTrafficLight::Green
        } else if connecting > 0 {
            RadrootsNostrTrafficLight::Yellow
        } else {
            RadrootsNostrTrafficLight::Red
        };

        RadrootsNostrConnectionSnapshot {
            light,
            connected,
            connecting,
            last_error,
        }
    }

    fn spawn_monitor_watcher(&self) {
        let inner = self.inner.clone();
        let handle = tokio::spawn(async move {
            if let Some(monitor) = inner.client.monitor() {
                let mut rx = monitor.subscribe();
                while let Ok(notification) = rx.recv().await {
                    match notification {
                        RadrootsNostrMonitorNotification::StatusChanged { relay_url, status } => {
                            if let Ok(mut map) = inner.statuses.lock() {
                                map.insert(relay_url, status);
                            }
                        }
                    }
                }
            }
        });

        if let Ok(mut guard) = self.inner.monitor_task.lock() {
            if let Some(existing) = guard.replace(handle) {
                existing.abort();
            }
        }
    }

    fn record_error(&self, message: String) {
        if let Ok(mut guard) = self.inner.last_error.lock() {
            *guard = Some(message);
        }
    }
}

fn spawn_subscription_worker(
    inner: Arc<RadrootsNostrRuntimeInner>,
    id: String,
    spec: RadrootsNostrSubscriptionSpec,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let _ = inner
            .queue_tx
            .send(RadrootsNostrRuntimeEvent::SubscriptionOpened { id: id.clone() })
            .await;

        let timeout = Duration::from_secs(spec.stream_timeout_secs.max(1));
        let reconnect_delay = Duration::from_millis(spec.reconnect_delay_millis.max(1));
        let mut since_unix: Option<u64> = None;

        loop {
            if inner.shutting_down.load(Ordering::SeqCst) {
                break;
            }

            let mut filter = spec.filter.clone();
            if let Some(since) = since_unix {
                filter = filter.since(RadrootsNostrTimestamp::from(since));
            }

            let mut stream = match inner.client.stream_events(filter, timeout).await {
                Ok(stream) => stream,
                Err(source) => {
                    let message = source.to_string();
                    if let Ok(mut guard) = inner.last_error.lock() {
                        *guard = Some(message.clone());
                    }
                    let _ = inner
                        .queue_tx
                        .send(RadrootsNostrRuntimeEvent::Error { message })
                        .await;

                    if matches!(spec.policy, RadrootsNostrSubscriptionPolicy::OneShotOnEose) {
                        break;
                    }

                    tokio::time::sleep(reconnect_delay).await;
                    continue;
                }
            };

            while let Some(event) = stream.next().await {
                let event_id = event.id.to_hex();
                let author = event.pubkey.to_hex();
                let kind = event.kind.as_u16();
                since_unix = Some(event.created_at.as_secs().saturating_add(1));

                if let Some(store) = inner.event_store.as_ref() {
                    if let Err(message) = store.ingest_event(&event) {
                        if let Ok(mut guard) = inner.last_error.lock() {
                            *guard = Some(message.clone());
                        }
                        let _ = inner
                            .queue_tx
                            .send(RadrootsNostrRuntimeEvent::Error { message })
                            .await;
                    }
                }

                let _ = inner
                    .queue_tx
                    .send(RadrootsNostrRuntimeEvent::Note {
                        subscription_id: id.clone(),
                        id: event_id,
                        author,
                        kind,
                        relay: None,
                    })
                    .await;

                if inner.shutting_down.load(Ordering::SeqCst) {
                    break;
                }
            }

            if matches!(spec.policy, RadrootsNostrSubscriptionPolicy::OneShotOnEose) {
                break;
            }

            tokio::time::sleep(reconnect_delay).await;
        }

        let _ = inner
            .queue_tx
            .send(RadrootsNostrRuntimeEvent::SubscriptionClosed { id })
            .await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::RadrootsNostrInMemoryEventStore;
    use alloc::sync::Arc;
    use radroots_nostr::prelude::RadrootsNostrFilter;

    fn sample_runtime() -> RadrootsNostrRuntime {
        RadrootsNostrRuntimeBuilder::new()
            .keys(RadrootsNostrKeys::generate())
            .add_relay("wss://relay.example.com")
            .build()
            .expect("runtime should build")
    }

    #[test]
    fn build_requires_keys() {
        let result = RadrootsNostrRuntimeBuilder::new()
            .add_relay("wss://relay.example.com")
            .build();
        assert!(matches!(
            result,
            Err(RadrootsNostrRuntimeError::MissingConfig("keys"))
        ));
    }

    #[test]
    fn build_requires_relays() {
        let result = RadrootsNostrRuntimeBuilder::new()
            .keys(RadrootsNostrKeys::generate())
            .build();
        assert!(matches!(
            result,
            Err(RadrootsNostrRuntimeError::MissingConfig("relays"))
        ));
    }

    #[test]
    fn queue_capacity_must_be_positive() {
        let result = RadrootsNostrRuntimeBuilder::new()
            .keys(RadrootsNostrKeys::generate())
            .add_relay("wss://relay.example.com")
            .queue_capacity(0)
            .build();
        assert!(matches!(
            result,
            Err(RadrootsNostrRuntimeError::InvalidConfig("queue_capacity"))
        ));
    }

    #[test]
    fn monitor_capacity_must_be_positive() {
        let result = RadrootsNostrRuntimeBuilder::new()
            .keys(RadrootsNostrKeys::generate())
            .add_relay("wss://relay.example.com")
            .monitor_capacity(0)
            .build();
        assert!(matches!(
            result,
            Err(RadrootsNostrRuntimeError::InvalidConfig("monitor_capacity"))
        ));
    }

    #[test]
    fn build_accepts_event_store() {
        let store = Arc::new(RadrootsNostrInMemoryEventStore::new());
        let result = RadrootsNostrRuntimeBuilder::new()
            .keys(RadrootsNostrKeys::generate())
            .add_relay("wss://relay.example.com")
            .event_store(store)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn set_relays_rejects_empty_input() {
        let runtime = sample_runtime();
        let result = runtime.set_relays(Vec::new());
        assert!(matches!(
            result,
            Err(RadrootsNostrRuntimeError::InvalidConfig("relays"))
        ));
    }

    #[test]
    fn drain_events_zero_returns_empty() {
        let runtime = sample_runtime();
        assert!(runtime.drain_events(0).is_empty());
    }

    #[tokio::test]
    async fn subscribe_requires_started_runtime() {
        let runtime = sample_runtime();
        let spec = RadrootsNostrSubscriptionSpec::streaming(RadrootsNostrFilter::new());
        let result = runtime.subscribe(spec).await;
        assert!(matches!(
            result,
            Err(RadrootsNostrRuntimeError::RuntimeNotStarted)
        ));
    }

    #[tokio::test]
    async fn unsubscribe_requires_existing_subscription() {
        let runtime = sample_runtime();
        let handle = RadrootsNostrSubscriptionHandle {
            id: "sub-999".into(),
            name: None,
        };
        let result = runtime.unsubscribe(&handle).await;
        assert!(matches!(
            result,
            Err(RadrootsNostrRuntimeError::SubscriptionNotFound(_))
        ));
    }
}

#[cfg(feature = "nostr-client")]
use std::collections::HashMap;
#[cfg(feature = "nostr-client")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "nostr-client")]
use nostr_sdk::prelude::*;
#[cfg(feature = "nostr-client")]
use tokio::runtime::Handle;
#[cfg(feature = "nostr-client")]
use tracing::{error, info};

#[cfg(feature = "nostr-client")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    Red,
    Yellow,
    Green,
}

#[cfg(feature = "nostr-client")]
#[derive(Debug, Clone)]
pub struct NostrConnectionSnapshot {
    pub light: Light,
    pub connected: usize,
    pub connecting: usize,
    pub last_error: Option<String>,
}

#[cfg(feature = "nostr-client")]
#[derive(Clone)]
pub struct NostrClientManager {
    inner: Arc<Inner>,
}

#[cfg(feature = "nostr-client")]
struct Inner {
    client: Client,
    relays: Arc<Mutex<Vec<String>>>,
    statuses: Arc<Mutex<HashMap<RelayUrl, RelayStatus>>>,
    last_error: Arc<Mutex<Option<String>>>,
    rt: Handle,
}

#[cfg(feature = "nostr-client")]
impl NostrClientManager {
    pub fn new(keys: nostr::Keys, rt: Handle) -> Self {
        let monitor = Monitor::new(2048);
        let client = Client::builder().signer(keys).monitor(monitor).build();

        let inner = Arc::new(Inner {
            client,
            relays: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(HashMap::new())),
            last_error: Arc::new(Mutex::new(None)),
            rt,
        });

        let this = Self {
            inner: inner.clone(),
        };
        this.spawn_status_watcher();
        this
    }

    pub fn set_relays(&self, urls: &[String]) {
        if let Ok(mut guard) = self.inner.relays.lock() {
            *guard = urls.to_vec();
        }
    }

    pub fn connect(&self) {
        let inner = self.inner.clone();
        let rt = inner.rt.clone();
        let inner_for_task = inner.clone();
        rt.spawn(async move {
            let urls = {
                let g = inner_for_task.relays.lock().ok();
                g.map(|v| v.clone()).unwrap_or_default()
            };
            if urls.is_empty() {
                info!("no relays configured; using default wss://relay.damus.io");
            }
            let effective = if urls.is_empty() {
                vec!["wss://relay.damus.io".to_string()]
            } else {
                urls
            };

            for u in &effective {
                match inner_for_task.client.add_relay(u.as_str()).await {
                    Ok(_) => {}
                    Err(e) => {
                        *inner_for_task.last_error.lock().unwrap() =
                            Some(format!("add_relay {u}: {e}"));
                        error!("add_relay failed for {u}: {e}");
                    }
                }
            }

            inner_for_task.client.connect().await;
        });
    }

    pub fn snapshot(&self) -> NostrConnectionSnapshot {
        let map = self
            .inner
            .statuses
            .lock()
            .ok()
            .map(|g| g.clone())
            .unwrap_or_default();
        let mut connected = 0usize;
        let mut connecting = 0usize;
        for (_url, st) in map.iter() {
            match st {
                RelayStatus::Connected => connected += 1,
                RelayStatus::Connecting => connecting += 1,
                _ => {}
            }
        }
        let light = if connected > 0 {
            Light::Green
        } else if connecting > 0 {
            Light::Yellow
        } else {
            Light::Red
        };
        let last_error = self.inner.last_error.lock().ok().and_then(|e| e.clone());
        NostrConnectionSnapshot {
            light,
            connected,
            connecting,
            last_error,
        }
    }

    fn spawn_status_watcher(&self) {
        let inner = self.inner.clone();
        let rt = inner.rt.clone();
        let inner_for_task = inner.clone();
        rt.spawn(async move {
            if let Some(m) = inner_for_task.client.monitor() {
                let mut rx = m.subscribe();
                while let Ok(notification) = rx.recv().await {
                    let MonitorNotification::StatusChanged { relay_url, status } = notification;
                    {
                        let mut map = inner_for_task.statuses.lock().unwrap();
                        map.insert(relay_url.clone(), status);
                    }
                    info!("relay status changed {} -> {:?}", relay_url, status);
                }
            }
        });
    }
}

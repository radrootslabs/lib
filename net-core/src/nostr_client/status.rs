use radroots_nostr::prelude::{
    RadrootsNostrMonitorNotification,
    RadrootsNostrRelayStatus,
};
use tracing::{info, warn};

use super::manager::NostrClientManager;
use super::types::{Light, NostrConnectionSnapshot};

impl NostrClientManager {
    pub(super) fn spawn_status_watcher(&self) {
        let inner = self.inner.clone();
        let rt = inner.rt.clone();
        let inner_for_task = inner.clone();

        rt.spawn(async move {
            if let Some(monitor) = inner_for_task.client.monitor() {
                let mut rx = monitor.subscribe();
                while let Ok(notification) = rx.recv().await {
                    match notification {
                        RadrootsNostrMonitorNotification::StatusChanged { relay_url, status } => {
                            if let Ok(mut map) = inner_for_task.statuses.lock() {
                                map.insert(relay_url.clone(), status);
                            } else if let Ok(mut last) = inner_for_task.last_error.lock() {
                                *last = Some("status watcher: statuses mutex poisoned".to_string());
                                warn!(
                                    "status watcher: statuses mutex poisoned; dropping update for {}",
                                    relay_url
                                );
                                continue;
                            }

                            info!("relay status changed {} -> {:?}", relay_url, status);
                        }
                    }
                }
            }
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
                RadrootsNostrRelayStatus::Connected => connected += 1,
                RadrootsNostrRelayStatus::Connecting => connecting += 1,
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
}

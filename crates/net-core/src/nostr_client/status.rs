use nostr_sdk::prelude::MonitorNotification;
use tracing::info;

use super::manager::NostrClientManager;
use super::types::{Light, NostrConnectionSnapshot};

impl NostrClientManager {
    pub(super) fn spawn_status_watcher(&self) {
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
                nostr_sdk::prelude::RelayStatus::Connected => connected += 1,
                nostr_sdk::prelude::RelayStatus::Connecting => connecting += 1,
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

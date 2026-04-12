use crate::error::{NetError, Result};
use tracing::error;

use super::manager::NostrClientManager;

impl NostrClientManager {
    pub fn set_relays(&self, urls: &[String]) {
        if let Ok(mut guard) = self.inner.relays.lock() {
            *guard = urls.to_vec();
        }
    }

    pub fn connect(&self) -> Result<()> {
        let inner = self.inner.clone();
        let urls = {
            let g = inner.relays.lock().ok();
            g.map(|v| v.clone()).unwrap_or_default()
        };

        if urls.is_empty() {
            if let Ok(mut e) = inner.last_error.lock() {
                *e = Some("no relays configured".to_string());
            }
            return Err(NetError::Msg("no relays configured".into()));
        }

        let inner_for_task = inner.clone();
        let rt = inner.rt.clone();
        rt.spawn(async move {
            for u in &urls {
                match inner_for_task.client.add_relay(u.as_str()).await {
                    Ok(_) => {}
                    Err(e) => {
                        if let Ok(mut last) = inner_for_task.last_error.lock() {
                            *last = Some(format!("add_relay {}: {}", u, e));
                        }
                        error!("add_relay failed for {}: {}", u, e);
                    }
                }
            }
            inner_for_task.client.connect().await;
        });

        Ok(())
    }
}

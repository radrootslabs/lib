#![cfg(feature = "nostr-client")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nostr_sdk::prelude::*;
use radroots_events::profile::models::RadrootsProfile;
use tokio::runtime::Handle;
use tracing::{error, info};

use crate::error::{NetError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    Red,
    Yellow,
    Green,
}

#[derive(Debug, Clone)]
pub struct NostrConnectionSnapshot {
    pub light: Light,
    pub connected: usize,
    pub connecting: usize,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct NostrClientManager {
    inner: Arc<Inner>,
}

struct Inner {
    client: Client,
    relays: Arc<Mutex<Vec<String>>>,
    statuses: Arc<Mutex<HashMap<RelayUrl, RelayStatus>>>,
    last_error: Arc<Mutex<Option<String>>>,
    rt: Handle,
}

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

    pub async fn fetch_profile_kind0(
        &self,
        author: nostr::PublicKey,
    ) -> Result<Option<RadrootsProfile>> {
        let filter = Filter::new()
            .authors(vec![author])
            .kind(Kind::Metadata)
            .limit(1);

        let events = self
            .inner
            .client
            .fetch_events(filter, std::time::Duration::from_secs(5))
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;

        if let Some(ev) = events.into_iter().next() {
            if let Ok(p) = serde_json::from_str::<RadrootsProfile>(&ev.content) {
                return Ok(Some(p));
            }
            if let Ok(md) = serde_json::from_str::<nostr::Metadata>(&ev.content) {
                let p = RadrootsProfile {
                    name: md.name.unwrap_or_default(),
                    display_name: md.display_name,
                    nip05: md.nip05,
                    about: md.about,
                    website: md.website.map(|u| u.to_string()),
                    picture: md.picture.map(|u| u.to_string()),
                    banner: md.banner.map(|u| u.to_string()),
                    lud06: md.lud06,
                    lud16: md.lud16,
                    bot: None,
                };
                return Ok(Some(p));
            }
            return Err(NetError::Msg(
                "failed to parse kind:0 metadata content".to_string(),
            ));
        }

        Ok(None)
    }

    pub fn fetch_profile_kind0_blocking(
        &self,
        author: nostr::PublicKey,
    ) -> Result<Option<RadrootsProfile>> {
        let rt = self.inner.rt.clone();
        let inner_for_task = self.inner.clone();
        rt.block_on(async move {
            let filter = Filter::new()
                .authors(vec![author])
                .kind(Kind::Metadata)
                .limit(1);
            let events = inner_for_task
                .client
                .fetch_events(filter, std::time::Duration::from_secs(5))
                .await
                .map_err(|e| NetError::Msg(e.to_string()))?;
            if let Some(ev) = events.into_iter().next() {
                if let Ok(p) = serde_json::from_str::<RadrootsProfile>(&ev.content) {
                    return Ok(Some(p));
                }
                if let Ok(md) = serde_json::from_str::<nostr::Metadata>(&ev.content) {
                    let p = RadrootsProfile {
                        name: md.name.unwrap_or_default(),
                        display_name: md.display_name,
                        nip05: md.nip05,
                        about: md.about,
                        website: md.website.map(|u| u.to_string()),
                        picture: md.picture.map(|u| u.to_string()),
                        banner: md.banner.map(|u| u.to_string()),
                        lud06: md.lud06,
                        lud16: md.lud16,
                        bot: None,
                    };
                    return Ok(Some(p));
                }
                return Err(NetError::Msg(
                    "failed to parse kind:0 metadata content".to_string(),
                ));
            }
            Ok(None)
        })
    }

    pub fn set_profile_kind0_blocking(
        &self,
        name: Option<String>,
        display_name: Option<String>,
        nip05: Option<String>,
        about: Option<String>,
    ) -> Result<String> {
        let rt = self.inner.rt.clone();
        let inner_for_task = self.inner.clone();
        rt.block_on(async move {
            let mut md = nostr::Metadata::new();
            if let Some(v) = name {
                md = md.name(v);
            }
            if let Some(v) = display_name {
                md = md.display_name(v);
            }
            if let Some(v) = nip05 {
                md = md.nip05(v);
            }
            if let Some(v) = about {
                md = md.about(v);
            }
            inner_for_task
                .client
                .set_metadata(&md)
                .await
                .map_err(|e| NetError::Msg(e.to_string()))?;
            Ok("ok".to_string())
        })
    }

    pub async fn publish_text_note(&self, content: String) -> Result<String> {
        let out = self
            .inner
            .client
            .send_event_builder(EventBuilder::text_note(content))
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;
        Ok(out.val.to_string())
    }

    pub fn publish_text_note_blocking(&self, content: String) -> Result<String> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.publish_text_note(content).await })
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

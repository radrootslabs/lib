use std::sync::Arc;
use tokio::runtime::Handle;

use super::inner::Inner;
use radroots_nostr::prelude::{
    RadrootsNostrFilter,
    RadrootsNostrKind,
    RadrootsNostrKeys,
    RadrootsNostrTimestamp,
};

#[derive(Clone)]
pub struct NostrClientManager {
    pub(super) inner: Arc<Inner>,
}

impl NostrClientManager {
    pub fn new(keys: RadrootsNostrKeys, rt: Handle) -> Self {
        let inner = Inner::new(keys, rt);
        let this = Self {
            inner: inner.clone(),
        };
        this.spawn_status_watcher();
        this
    }
}

impl NostrClientManager {
    pub fn start_post_event_stream(&self, since_unix: Option<u64>) {
        if self
            .inner
            .post_events_stream
            .lock()
            .ok()
            .is_some_and(|h| h.is_some())
        {
            return;
        }

        let inner = self.inner.clone();
        let rt = inner.rt.clone();
        let handle = rt.spawn({
            let inner = inner.clone();
            async move {
                use futures::StreamExt;

                let mut since = since_unix.unwrap_or_else(|| RadrootsNostrTimestamp::now().as_u64());
                loop {
                    let filter = RadrootsNostrFilter::new()
                        .kind(RadrootsNostrKind::TextNote)
                        .since(RadrootsNostrTimestamp::from(since));

                    let mut stream = match inner
                        .client
                        .stream_events(filter, core::time::Duration::from_secs(30))
                        .await
                    {
                        Ok(s) => s,
                        Err(_) => {
                            tokio::time::sleep(core::time::Duration::from_secs(2)).await;
                            continue;
                        }
                    };

                    while let Some((_, event)) = stream.next().await {
                        let event = match event {
                            Ok(ev) => ev,
                            Err(_) => continue,
                        };
                        let meta = radroots_nostr::event_adapters::to_post_event_metadata(&event);
                        let ts = event.created_at.as_u64();
                        since = ts.saturating_add(1);
                        let _ = inner.post_events_tx.send(meta);
                    }
                }
            }
        });

        if let Ok(mut g) = self.inner.post_events_stream.lock() {
            *g = Some(handle);
        }
    }

    pub fn stop_post_event_stream(&self) {
        if let Ok(mut g) = self.inner.post_events_stream.lock() {
            if let Some(h) = g.take() {
                h.abort();
            }
        }
    }

    pub fn subscribe_post_events(
        &self,
    ) -> tokio::sync::broadcast::Receiver<radroots_events::post::RadrootsPostEventMetadata>
    {
        self.inner.post_events_tx.subscribe()
    }
}

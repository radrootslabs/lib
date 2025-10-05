use std::time::Duration;

use crate::error::{NetError, Result};
use radroots_events::post::models::{RadrootsPost, RadrootsPostEventMetadata};

use super::manager::NostrClientManager;

impl NostrClientManager {
    pub async fn publish_text_note(&self, content: String) -> Result<String> {
        let out = self
            .inner
            .client
            .send_event_builder(nostr_sdk::prelude::EventBuilder::text_note(content))
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;
        Ok(out.val.to_string())
    }

    pub fn publish_text_note_blocking(&self, content: String) -> Result<String> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.publish_text_note(content).await })
    }

    pub async fn fetch_text_notes(
        &self,
        limit: u16,
        since_unix: Option<u64>,
    ) -> Result<Vec<RadrootsPostEventMetadata>> {
        let mut filter = nostr_sdk::prelude::Filter::new()
            .kind(nostr_sdk::prelude::Kind::TextNote)
            .limit(limit.into());
        if let Some(s) = since_unix {
            filter = filter.since(nostr_sdk::prelude::Timestamp::from(s));
        }
        let events = self
            .inner
            .client
            .fetch_events(filter, Duration::from_secs(10))
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;
        let out = events
            .into_iter()
            .map(|ev| RadrootsPostEventMetadata {
                id: ev.id.to_string(),
                author: ev.pubkey.to_string(),
                published_at: ev.created_at.as_u64() as u32,
                post: RadrootsPost {
                    content: ev.content,
                },
            })
            .collect();
        Ok(out)
    }

    pub fn fetch_text_notes_blocking(
        &self,
        limit: u16,
        since_unix: Option<u64>,
    ) -> Result<Vec<RadrootsPostEventMetadata>> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.fetch_text_notes(limit, since_unix).await })
    }
}

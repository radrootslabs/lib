use std::time::Duration;

use crate::error::{NetError, Result};
use radroots_events::post::models::{RadrootsPost, RadrootsPostEventMetadata};

use super::manager::NostrClientManager;
use nostr_sdk::prelude::*;

impl NostrClientManager {
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

    pub async fn publish_reply_to_event(
        &self,
        parent_event_id_hex: String,
        parent_author_hex: String,
        content: String,
        root_event_id_hex: Option<String>,
    ) -> Result<String> {
        let parent_id =
            EventId::from_hex(&parent_event_id_hex).map_err(|_| NetError::InvalidHex32)?;
        let parent_pubkey =
            PublicKey::from_hex(&parent_author_hex).map_err(|_| NetError::InvalidHex32)?;

        let mut tags: Vec<Tag> = Vec::new();

        if let Some(root_hex) = root_event_id_hex {
            if !root_hex.is_empty() {
                if let Ok(root_id) = EventId::from_hex(&root_hex) {
                    tags.push(Tag::event(root_id));
                }
            }
        }

        tags.push(Tag::event(parent_id));
        tags.push(Tag::public_key(parent_pubkey));

        let builder = EventBuilder::text_note(content).tags(tags);
        let out = self
            .inner
            .client
            .send_event_builder(builder)
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;

        Ok(out.val.to_string())
    }

    pub fn publish_reply_to_event_blocking(
        &self,
        parent_event_id_hex: String,
        parent_author_hex: String,
        content: String,
        root_event_id_hex: Option<String>,
    ) -> Result<String> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move {
            this.publish_reply_to_event(
                parent_event_id_hex,
                parent_author_hex,
                content,
                root_event_id_hex,
            )
            .await
        })
    }

    pub async fn fetch_text_notes(
        &self,
        limit: u16,
        since_unix: Option<u64>,
    ) -> Result<Vec<RadrootsPostEventMetadata>> {
        let mut filter = Filter::new().kind(Kind::TextNote).limit(limit.into());
        if let Some(s) = since_unix {
            filter = filter.since(Timestamp::from(s));
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

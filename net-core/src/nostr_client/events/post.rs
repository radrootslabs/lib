use crate::error::{NetError, Result};
use radroots_events::post::RadrootsPostEventMetadata;
use radroots_nostr::prelude::{
    radroots_nostr_build_post_event,
    radroots_nostr_build_post_reply_event,
    radroots_nostr_fetch_post_events,
};

use crate::nostr_client::manager::NostrClientManager;

impl NostrClientManager {
    pub async fn publish_post_event(&self, content: String) -> Result<String> {
        let builder = radroots_nostr_build_post_event(content);
        let out = self
            .inner
            .client
            .send_event_builder(builder)
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;
        Ok(out.val.to_string())
    }

    pub fn publish_post_event_blocking(&self, content: String) -> Result<String> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.publish_post_event(content).await })
    }

    pub async fn publish_post_reply_event(
        &self,
        parent_event_id_hex: String,
        parent_author_hex: String,
        content: String,
        root_event_id_hex: Option<String>,
    ) -> Result<String> {
        let builder = radroots_nostr_build_post_reply_event(
            &parent_event_id_hex,
            &parent_author_hex,
            content,
            root_event_id_hex.as_deref(),
        )
        .map_err(|e| NetError::Msg(e.to_string()))?;

        let out = self
            .inner
            .client
            .send_event_builder(builder)
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;

        Ok(out.val.to_string())
    }

    pub fn publish_post_reply_event_blocking(
        &self,
        parent_event_id_hex: String,
        parent_author_hex: String,
        content: String,
        root_event_id_hex: Option<String>,
    ) -> Result<String> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move {
            this.publish_post_reply_event(
                parent_event_id_hex,
                parent_author_hex,
                content,
                root_event_id_hex,
            )
            .await
        })
    }

    pub async fn fetch_post_events(
        &self,
        limit: u16,
        since_unix: Option<u64>,
    ) -> Result<Vec<RadrootsPostEventMetadata>> {
        let items = radroots_nostr_fetch_post_events(&self.inner.client, limit, since_unix)
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;
        Ok(items)
    }

    pub fn fetch_post_events_blocking(
        &self,
        limit: u16,
        since_unix: Option<u64>,
    ) -> Result<Vec<RadrootsPostEventMetadata>> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.fetch_post_events(limit, since_unix).await })
    }
}

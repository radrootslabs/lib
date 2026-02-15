#![forbid(unsafe_code)]

use crate::error::{NetError, Result};
use radroots_nostr::prelude::{
    RadrootsNostrEvent, RadrootsNostrFilter, radroots_nostr_build_event, radroots_nostr_send_event,
};

use crate::nostr_client::manager::NostrClientManager;

impl NostrClientManager {
    pub async fn send_custom_event(
        &self,
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
    ) -> Result<String> {
        let builder = radroots_nostr_build_event(kind, content, tags)
            .map_err(|e| NetError::Msg(e.to_string()))?;
        let out = radroots_nostr_send_event(&self.inner.client, builder)
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;
        Ok(out.val.to_string())
    }

    pub fn send_custom_event_blocking(
        &self,
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
    ) -> Result<String> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.send_custom_event(kind, content, tags).await })
    }

    pub async fn fetch_events(
        &self,
        filter: RadrootsNostrFilter,
        timeout: core::time::Duration,
    ) -> Result<Vec<RadrootsNostrEvent>> {
        self.inner
            .client
            .fetch_events(filter, timeout)
            .await
            .map_err(|e| NetError::Msg(e.to_string()))
    }

    pub fn fetch_events_blocking(
        &self,
        filter: RadrootsNostrFilter,
        timeout: core::time::Duration,
    ) -> Result<Vec<RadrootsNostrEvent>> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.fetch_events(filter, timeout).await })
    }
}

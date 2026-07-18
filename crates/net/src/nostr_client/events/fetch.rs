#![forbid(unsafe_code)]

use crate::error::{NetError, Result};
use radroots_nostr::prelude::{RadrootsNostrEvent, RadrootsNostrFilter};

use crate::nostr_client::manager::NostrClientManager;

impl NostrClientManager {
    pub async fn fetch_events(
        &self,
        filter: RadrootsNostrFilter,
        timeout: core::time::Duration,
    ) -> Result<Vec<RadrootsNostrEvent>> {
        self.inner
            .client
            .fetch_events(filter, timeout)
            .await
            .map_err(|error| NetError::Msg(error.to_string()))
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

use crate::error::{NetError, Result};
use radroots_event::{
    post::{RadrootsAuthoredUpdate, RadrootsPost},
    reply::RadrootsAuthoredNip10Reply,
};
use radroots_event_codec::parsed::RadrootsParsedData;
use radroots_nostr::prelude::{
    radroots_nostr_build_nip10_reply_event, radroots_nostr_build_update_event,
    radroots_nostr_fetch_post_events, radroots_nostr_send_nip10_reply_event,
    radroots_nostr_send_post_event,
};

use crate::nostr_client::manager::NostrClientManager;

impl NostrClientManager {
    pub async fn publish_update_event(&self, update: &RadrootsAuthoredUpdate) -> Result<String> {
        let builder =
            radroots_nostr_build_update_event(update).map_err(|e| NetError::Msg(e.to_string()))?;
        let out = radroots_nostr_send_post_event(&self.inner.client, builder)
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;
        Ok(out.val.to_string())
    }

    pub fn publish_update_event_blocking(&self, update: RadrootsAuthoredUpdate) -> Result<String> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.publish_update_event(&update).await })
    }

    pub async fn publish_nip10_reply_event(
        &self,
        reply: &RadrootsAuthoredNip10Reply,
    ) -> Result<String> {
        let builder = radroots_nostr_build_nip10_reply_event(reply)
            .map_err(|e| NetError::Msg(e.to_string()))?;
        let out = radroots_nostr_send_nip10_reply_event(&self.inner.client, builder)
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;

        Ok(out.val.to_string())
    }

    pub fn publish_nip10_reply_event_blocking(
        &self,
        reply: RadrootsAuthoredNip10Reply,
    ) -> Result<String> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.publish_nip10_reply_event(&reply).await })
    }

    /// Fetches generic kind-1 compatibility projections without claiming
    /// Radroots product-profile admission.
    pub async fn fetch_post_events(
        &self,
        limit: u16,
        since_unix: Option<u64>,
    ) -> Result<Vec<RadrootsParsedData<RadrootsPost>>> {
        let items = radroots_nostr_fetch_post_events(&self.inner.client, limit, since_unix)
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;
        Ok(items)
    }

    pub fn fetch_post_events_blocking(
        &self,
        limit: u16,
        since_unix: Option<u64>,
    ) -> Result<Vec<RadrootsParsedData<RadrootsPost>>> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.fetch_post_events(limit, since_unix).await })
    }
}

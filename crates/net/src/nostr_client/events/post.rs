use crate::error::{NetError, Result};
use radroots_event::{post::AuthoredUpdate, post::reply::AuthoredNip10Reply};
use radroots_event_codec::{parsed::RadrootsParsedData, post::decode::LegacyPost};
use radroots_nostr::events::post::radroots_nostr_build_update_event;
use radroots_nostr::events::post::radroots_nostr_post_events_filter;
use radroots_nostr::events::reply::radroots_nostr_build_nip10_reply_event;

use crate::nostr_client::manager::NostrClientManager;

impl NostrClientManager {
    pub async fn publish_update_event(&self, update: &AuthoredUpdate) -> Result<String> {
        let builder =
            radroots_nostr_build_update_event(update).map_err(|e| NetError::Msg(e.to_string()))?;
        let event = builder
            .sign_with_keys(&self.inner.keys)
            .map_err(|error| NetError::Msg(error.to_string()))?;
        let out = self
            .inner
            .client
            .send_event(&event)
            .await
            .map_err(|error| NetError::Msg(error.to_string()))?;
        Ok(out.val.to_string())
    }

    pub fn publish_update_event_blocking(&self, update: AuthoredUpdate) -> Result<String> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.publish_update_event(&update).await })
    }

    pub async fn publish_nip10_reply_event(&self, reply: &AuthoredNip10Reply) -> Result<String> {
        let builder = radroots_nostr_build_nip10_reply_event(reply)
            .map_err(|e| NetError::Msg(e.to_string()))?;
        let event = builder
            .sign_with_keys(&self.inner.keys)
            .map_err(|error| NetError::Msg(error.to_string()))?;
        let out = self
            .inner
            .client
            .send_event(&event)
            .await
            .map_err(|error| NetError::Msg(error.to_string()))?;

        Ok(out.val.to_string())
    }

    pub fn publish_nip10_reply_event_blocking(&self, reply: AuthoredNip10Reply) -> Result<String> {
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
    ) -> Result<Vec<RadrootsParsedData<LegacyPost>>> {
        let filter = radroots_nostr_post_events_filter(Some(limit), since_unix);
        let events = self
            .inner
            .client
            .fetch_events(filter, core::time::Duration::from_secs(10))
            .await
            .map_err(|error| NetError::Msg(error.to_string()))?;
        Ok(events
            .into_iter()
            .map(|event| radroots_nostr::event_adapters::to_post_event_metadata(&event))
            .collect())
    }

    pub fn fetch_post_events_blocking(
        &self,
        limit: u16,
        since_unix: Option<u64>,
    ) -> Result<Vec<RadrootsParsedData<LegacyPost>>> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.fetch_post_events(limit, since_unix).await })
    }
}

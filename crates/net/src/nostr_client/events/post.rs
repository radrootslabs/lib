use crate::error::{NetError, Result};
use radroots_event::post::{RadrootsAuthoredUpdate, RadrootsPost};
use radroots_event_codec::parsed::RadrootsParsedData;
use radroots_nostr::prelude::{
    radroots_nostr_build_post_reply_event, radroots_nostr_build_update_event,
    radroots_nostr_fetch_post_events, radroots_nostr_send_event, radroots_nostr_send_post_event,
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

        let out = radroots_nostr_send_event(&self.inner.client, builder)
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

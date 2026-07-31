use crate::error::{NetError, Result};
use radroots_event_codec::parsed::RadrootsParsedData;
use radroots_event_codec::profile::RadrootsProfileData;
use radroots_nostr::event::Kind as RadrootsNostrKind;
use radroots_nostr::filter::Filter as RadrootsNostrFilter;
use radroots_nostr::types::RadrootsNostrPublicKey;

use crate::nostr_client::manager::NostrClientManager;

impl NostrClientManager {
    pub async fn fetch_profile_event(
        &self,
        author: RadrootsNostrPublicKey,
    ) -> Result<Option<RadrootsParsedData<RadrootsProfileData>>> {
        let filter = RadrootsNostrFilter::new()
            .authors(vec![author])
            .kind(RadrootsNostrKind::Metadata);
        let stored = self
            .inner
            .client
            .query_database(filter.clone())
            .await
            .map_err(|error| NetError::Msg(error.to_string()))?;
        let fetched = self
            .inner
            .client
            .fetch_events(filter, core::time::Duration::from_secs(5))
            .await
            .map_err(|error| NetError::Msg(error.to_string()))?;
        let ev = stored
            .into_iter()
            .chain(fetched)
            .filter(|event| event.kind == RadrootsNostrKind::Metadata)
            .max_by_key(|event| event.created_at);
        if let Some(e) = ev {
            if let Some(meta) = radroots_nostr::event_adapters::to_profile_event_metadata(&e) {
                return Ok(Some(meta));
            }
            return Err(NetError::Msg(
                "failed to parse kind:0 metadata content".to_string(),
            ));
        }
        Ok(None)
    }

    pub fn fetch_profile_event_blocking(
        &self,
        author: RadrootsNostrPublicKey,
    ) -> Result<Option<RadrootsParsedData<RadrootsProfileData>>> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.fetch_profile_event(author).await })
    }
}

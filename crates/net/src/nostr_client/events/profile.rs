use crate::error::{NetError, Result};
use radroots_event_codec::parsed::RadrootsParsedData;
use radroots_event_codec::profile::RadrootsProfileData;
use radroots_nostr::prelude::{RadrootsNostrPublicKey, radroots_nostr_fetch_metadata_for_author};

use crate::nostr_client::manager::NostrClientManager;

impl NostrClientManager {
    pub async fn fetch_profile_event(
        &self,
        author: RadrootsNostrPublicKey,
    ) -> Result<Option<RadrootsParsedData<RadrootsProfileData>>> {
        let ev = radroots_nostr_fetch_metadata_for_author(
            &self.inner.client,
            author,
            core::time::Duration::from_secs(5),
        )
        .await
        .map_err(|e| NetError::Msg(e.to_string()))?;
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

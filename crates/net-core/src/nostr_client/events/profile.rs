use crate::error::{NetError, Result};
use radroots_events::profile::models::RadrootsProfileEventMetadata;

use crate::nostr_client::manager::NostrClientManager;

impl NostrClientManager {
    pub async fn fetch_profile_event(
        &self,
        author: nostr::PublicKey,
    ) -> Result<Option<RadrootsProfileEventMetadata>> {
        let ev = radroots_nostr::events::metadata::fetch_metadata_for_author(
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
        author: nostr::PublicKey,
    ) -> Result<Option<RadrootsProfileEventMetadata>> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.fetch_profile_event(author).await })
    }

    pub fn publish_profile_event_blocking(
        &self,
        name: Option<String>,
        display_name: Option<String>,
        nip05: Option<String>,
        about: Option<String>,
    ) -> Result<String> {
        let rt = self.inner.rt.clone();
        let inner_for_task = self.inner.clone();
        rt.block_on(async move {
            let mut md = nostr::Metadata::new();
            if let Some(v) = name {
                md = md.name(v);
            }
            if let Some(v) = display_name {
                md = md.display_name(v);
            }
            if let Some(v) = nip05 {
                md = md.nip05(v);
            }
            if let Some(v) = about {
                md = md.about(v);
            }
            let _ =
                radroots_nostr::events::metadata::post_metadata_event(&inner_for_task.client, &md)
                    .await
                    .map_err(|e| NetError::Msg(e.to_string()))?;
            Ok::<(), NetError>(())
        })?;
        Ok("ok".to_string())
    }
}

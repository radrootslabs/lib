use std::time::Duration;

use crate::error::{NetError, Result};
use radroots_events::profile::models::{RadrootsProfile, RadrootsProfileEventMetadata};

use super::manager::NostrClientManager;

impl NostrClientManager {
    pub async fn fetch_profile_kind0(
        &self,
        author: nostr::PublicKey,
    ) -> Result<Option<RadrootsProfileEventMetadata>> {
        let filter = nostr_sdk::prelude::Filter::new()
            .authors(vec![author])
            .kind(nostr_sdk::prelude::Kind::Metadata)
            .limit(1);

        let events = self
            .inner
            .client
            .fetch_events(filter, Duration::from_secs(5))
            .await
            .map_err(|e| NetError::Msg(e.to_string()))?;

        if let Some(ev) = events.into_iter().next() {
            if let Ok(p) = serde_json::from_str::<RadrootsProfile>(&ev.content) {
                let out = RadrootsProfileEventMetadata {
                    id: ev.id.to_string(),
                    author: ev.pubkey.to_string(),
                    published_at: ev.created_at.as_u64() as u32,
                    profile: p,
                };
                return Ok(Some(out));
            }
            if let Ok(md) = serde_json::from_str::<nostr::Metadata>(&ev.content) {
                let p = RadrootsProfile {
                    name: md.name.unwrap_or_default(),
                    display_name: md.display_name,
                    nip05: md.nip05,
                    about: md.about,
                    website: md.website.map(|u| u.to_string()),
                    picture: md.picture.map(|u| u.to_string()),
                    banner: md.banner.map(|u| u.to_string()),
                    lud06: md.lud06,
                    lud16: md.lud16,
                    bot: None,
                };
                let out = RadrootsProfileEventMetadata {
                    id: ev.id.to_string(),
                    author: ev.pubkey.to_string(),
                    published_at: ev.created_at.as_u64() as u32,
                    profile: p,
                };
                return Ok(Some(out));
            }
            return Err(NetError::Msg(
                "failed to parse kind:0 metadata content".to_string(),
            ));
        }

        Ok(None)
    }

    pub fn fetch_profile_kind0_blocking(
        &self,
        author: nostr::PublicKey,
    ) -> Result<Option<RadrootsProfileEventMetadata>> {
        let rt = self.inner.rt.clone();
        let this = self.clone();
        rt.block_on(async move { this.fetch_profile_kind0(author).await })
    }

    pub fn set_profile_kind0_blocking(
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
            inner_for_task
                .client
                .set_metadata(&md)
                .await
                .map_err(|e| NetError::Msg(e.to_string()))?;
            Ok::<(), NetError>(())
        })?;
        Ok("ok".to_string())
    }
}

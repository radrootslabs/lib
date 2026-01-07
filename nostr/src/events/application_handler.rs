#![forbid(unsafe_code)]

extern crate alloc;

use alloc::{string::String, vec::Vec};

use crate::error::RadrootsNostrError;
use crate::events::radroots_nostr_build_event;
#[cfg(feature = "client")]
use crate::filter::radroots_nostr_filter_tag;
#[cfg(feature = "client")]
use crate::tags::radroots_nostr_tag_first_value;
#[cfg(feature = "client")]
use crate::types::{RadrootsNostrEvent, RadrootsNostrFilter, RadrootsNostrKind};
use crate::types::{RadrootsNostrEventBuilder, RadrootsNostrMetadata};
use radroots_events::kinds::KIND_APPLICATION_HANDLER;
#[cfg(feature = "client")]
use core::time::Duration;

#[derive(Debug, Clone)]
pub struct RadrootsNostrApplicationHandlerSpec {
    pub kinds: Vec<u32>,
    pub identifier: Option<String>,
    pub metadata: Option<RadrootsNostrMetadata>,
    pub extra_tags: Vec<Vec<String>>,
    pub relays: Vec<String>,
    pub nostrconnect_url: Option<String>,
}

impl RadrootsNostrApplicationHandlerSpec {
    pub fn new(kinds: Vec<u32>) -> Self {
        Self {
            kinds,
            identifier: None,
            metadata: None,
            extra_tags: Vec::new(),
            relays: Vec::new(),
            nostrconnect_url: None,
        }
    }
}

pub fn radroots_nostr_build_application_handler_event(
    spec: &RadrootsNostrApplicationHandlerSpec,
) -> Result<RadrootsNostrEventBuilder, RadrootsNostrError> {
    if spec.kinds.is_empty() {
        return Err(RadrootsNostrError::FilterTagError(
            "application handler kinds are empty".to_string(),
        ));
    }

    let identifier = spec
        .identifier
        .clone()
        .unwrap_or_else(|| spec.kinds[0].to_string());

    let mut content = String::new();
    if let Some(md) = spec.metadata.as_ref() {
        if metadata_has_fields(md) {
            content = serde_json::to_string(md).unwrap_or_default();
        }
    }

    let mut tags = Vec::new();
    tags.push(vec!["d".to_string(), identifier]);
    for kind in &spec.kinds {
        tags.push(vec!["k".to_string(), kind.to_string()]);
    }
    for relay in &spec.relays {
        let relay = relay.trim();
        if relay.is_empty() {
            continue;
        }
        tags.push(vec!["relay".to_string(), relay.to_string()]);
    }
    if let Some(url) = spec.nostrconnect_url.as_ref() {
        let url = url.trim();
        if !url.is_empty() {
            tags.push(vec!["nostrconnect_url".to_string(), url.to_string()]);
        }
    }
    for tag in &spec.extra_tags {
        if tag.is_empty() {
            continue;
        }
        tags.push(tag.clone());
    }

    radroots_nostr_build_event(KIND_APPLICATION_HANDLER, content, tags)
}

fn metadata_has_fields(md: &RadrootsNostrMetadata) -> bool {
    md.name.is_some()
        || md.display_name.is_some()
        || md.about.is_some()
        || md.website.is_some()
        || md.picture.is_some()
        || md.banner.is_some()
        || md.nip05.is_some()
        || md.lud06.is_some()
        || md.lud16.is_some()
        || !md.custom.is_empty()
}

#[cfg(feature = "client")]
pub async fn radroots_nostr_publish_application_handler(
    client: &crate::client::RadrootsNostrClient,
    spec: &RadrootsNostrApplicationHandlerSpec,
) -> Result<crate::types::RadrootsNostrOutput<crate::types::RadrootsNostrEventId>, RadrootsNostrError>
{
    let mut spec = spec.clone();
    if spec.identifier.is_none() {
        if let Some(existing) = fetch_existing_identifier(client, &spec).await? {
            spec.identifier = Some(existing);
        }
    }
    let builder = radroots_nostr_build_application_handler_event(&spec)?;
    crate::client::radroots_nostr_send_event(client, builder).await
}

#[cfg(feature = "client")]
async fn fetch_existing_identifier(
    client: &crate::client::RadrootsNostrClient,
    spec: &RadrootsNostrApplicationHandlerSpec,
) -> Result<Option<String>, RadrootsNostrError> {
    let first_kind = spec
        .kinds
        .first()
        .ok_or_else(|| RadrootsNostrError::FilterTagError("kinds are empty".to_string()))?;
    let author = client.public_key().await?;
    let filter = RadrootsNostrFilter::new()
        .author(author)
        .kind(RadrootsNostrKind::Custom(KIND_APPLICATION_HANDLER as u16));
    let filter = radroots_nostr_filter_tag(filter, "k", vec![first_kind.to_string()])?;
    let mut events = client.fetch_events(filter, Duration::from_secs(5)).await?;
    events.sort_by_key(|event| event.created_at.as_secs());
    let event = events.pop();
    Ok(event.and_then(|event| tag_value(&event, "d")))
}

#[cfg(feature = "client")]
fn tag_value(event: &RadrootsNostrEvent, key: &str) -> Option<String> {
    event
        .tags
        .iter()
        .find_map(|tag| radroots_nostr_tag_first_value(tag, key))
}

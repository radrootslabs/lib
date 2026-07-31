//! Typed authoring helpers for NIP-89 application-handler announcements.

#![forbid(unsafe_code)]

extern crate alloc;

use alloc::{string::String, vec::Vec};

use crate::error::Error;
use crate::events::build_event_unchecked;
use crate::types::{GenericBuilder, RadrootsNostrMetadata};
use radroots_event::envelope::kind::KIND_APPLICATION_HANDLER;

#[derive(Debug, Clone)]
pub struct ApplicationHandlerSpec {
    kinds: Vec<u32>,
    identifier: Option<String>,
    metadata: Option<RadrootsNostrMetadata>,
    extra_tags: Vec<Vec<String>>,
    relays: Vec<String>,
    nostrconnect_url: Option<String>,
}

impl ApplicationHandlerSpec {
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

    #[must_use]
    pub fn with_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.identifier = Some(identifier.into());
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: RadrootsNostrMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    #[must_use]
    pub fn with_extra_tags(mut self, extra_tags: Vec<Vec<String>>) -> Self {
        self.extra_tags = extra_tags;
        self
    }

    #[must_use]
    pub fn with_relays(mut self, relays: Vec<String>) -> Self {
        self.relays = relays;
        self
    }

    #[must_use]
    pub fn with_nostr_connect_url(mut self, url: impl Into<String>) -> Self {
        self.nostrconnect_url = Some(url.into());
        self
    }

    pub fn kinds(&self) -> &[u32] {
        &self.kinds
    }

    pub fn identifier(&self) -> Option<&str> {
        self.identifier.as_deref()
    }

    pub fn metadata(&self) -> Option<&RadrootsNostrMetadata> {
        self.metadata.as_ref()
    }

    pub fn extra_tags(&self) -> &[Vec<String>] {
        &self.extra_tags
    }

    pub fn relays(&self) -> &[String] {
        &self.relays
    }

    pub fn nostr_connect_url(&self) -> Option<&str> {
        self.nostrconnect_url.as_deref()
    }
}

pub fn build_application_handler_event(
    spec: &ApplicationHandlerSpec,
) -> Result<GenericBuilder, Error> {
    if spec.kinds.is_empty() {
        return Err(Error::FilterTagError(
            "application handler kinds are empty".to_string(),
        ));
    }

    let identifier = spec
        .identifier
        .clone()
        .unwrap_or_else(|| spec.kinds[0].to_string());

    let mut content = String::new();
    if let Some(md) = spec.metadata.as_ref()
        && metadata_has_fields(md)
    {
        content = serde_json::to_string(md).unwrap_or_default();
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

    let builder = build_event_unchecked(KIND_APPLICATION_HANDLER, content, tags)?;
    Ok(GenericBuilder::from_unchecked(builder))
}

pub fn metadata_has_fields(md: &RadrootsNostrMetadata) -> bool {
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

#[cfg(test)]
mod tests {
    use super::metadata_has_fields;
    use crate::types::RadrootsNostrMetadata;

    #[test]
    fn metadata_has_fields_false_when_empty() {
        assert!(!metadata_has_fields(&RadrootsNostrMetadata::default()));
    }

    #[test]
    fn metadata_has_fields_true_when_about_is_set() {
        let metadata = RadrootsNostrMetadata {
            about: Some("ready".to_string()),
            ..Default::default()
        };
        assert!(metadata_has_fields(&metadata));
    }
}

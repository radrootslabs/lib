#![forbid(unsafe_code)]

use radroots_events_codec::job::traits::{JobEventBorrow, JobEventLike};
use crate::types::{RadrootsNostrEvent, RadrootsNostrKind};

#[derive(Clone, Debug)]
pub struct RadrootsNostrEventAdapter<'a> {
    evt: &'a RadrootsNostrEvent,
    id_hex: String,
    author_hex: String,
}

impl<'a> RadrootsNostrEventAdapter<'a> {
    #[inline]
    pub fn new(evt: &'a RadrootsNostrEvent) -> Self {
        Self {
            evt,
            id_hex: evt.id.to_hex(),
            author_hex: evt.pubkey.to_string(),
        }
    }

    #[inline]
    fn tags_as_slices(&self) -> Vec<Vec<String>> {
        self.evt
            .tags
            .iter()
            .map(|t| t.as_slice().to_vec())
            .collect()
    }
}

impl<'a> JobEventBorrow<'a> for RadrootsNostrEventAdapter<'a> {
    #[inline]
    fn raw_id(&'a self) -> &'a str {
        &self.id_hex
    }
    #[inline]
    fn raw_author(&'a self) -> &'a str {
        &self.author_hex
    }
    #[inline]
    fn raw_content(&'a self) -> &'a str {
        &self.evt.content
    }
    #[inline]
    fn raw_kind(&'a self) -> u32 {
        match self.evt.kind {
            RadrootsNostrKind::Custom(v) => v as u32,
            _ => 0,
        }
    }
}

impl JobEventLike for RadrootsNostrEventAdapter<'_> {
    fn raw_id(&self) -> String {
        self.id_hex.clone()
    }
    fn raw_author(&self) -> String {
        self.author_hex.clone()
    }
    fn raw_published_at(&self) -> u32 {
        self.evt.created_at.as_u64() as u32
    }
    fn raw_kind(&self) -> u32 {
        match self.evt.kind {
            RadrootsNostrKind::Custom(v) => v as u32,
            _ => 0,
        }
    }
    fn raw_content(&self) -> String {
        self.evt.content.clone()
    }
    fn raw_tags(&self) -> Vec<Vec<String>> {
        self.tags_as_slices()
    }
    fn raw_sig(&self) -> String {
        self.evt.sig.to_string()
    }
}

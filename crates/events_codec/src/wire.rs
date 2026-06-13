#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::draft::{RadrootsDraftError, RadrootsFrozenEventDraft};

#[derive(Debug, Clone)]
pub struct WireEventParts {
    pub kind: u32,
    pub content: String,
    pub tags: Vec<Vec<String>>,
}

pub fn to_frozen_draft(
    parts: WireEventParts,
    contract_id: impl Into<String>,
    expected_pubkey: impl AsRef<str>,
    created_at: u32,
) -> Result<RadrootsFrozenEventDraft, RadrootsDraftError> {
    RadrootsFrozenEventDraft::new(
        contract_id,
        parts.kind,
        created_at,
        parts.tags,
        parts.content,
        expected_pubkey,
    )
}

pub fn canonicalize_tags(tags: &mut Vec<Vec<String>>) {
    tags.retain(|t| t.first().map(|s| !s.trim().is_empty()).unwrap_or(false));
    for t in tags.iter_mut() {
        for s in t.iter_mut() {
            let trimmed = s.trim();
            if trimmed.len() != s.len() {
                *s = trimmed.to_string();
            }
        }
    }
    tags.sort_by(|a, b| a.first().cmp(&b.first()).then_with(|| a.cmp(b)));
    tags.dedup();
}

pub fn empty_content() -> String {
    String::new()
}

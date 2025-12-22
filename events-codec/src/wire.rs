#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

#[derive(Debug, Clone)]
pub struct WireEventParts {
    pub kind: u32,
    pub content: String,
    pub tags: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct EventDraft {
    pub kind: u32,
    pub created_at: u32,
    pub author: String,
    pub content: String,
    pub tags: Vec<Vec<String>>,
}

pub fn to_draft(parts: WireEventParts, author: impl Into<String>, created_at: u32) -> EventDraft {
    EventDraft {
        kind: parts.kind,
        created_at,
        author: author.into(),
        content: parts.content,
        tags: parts.tags,
    }
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

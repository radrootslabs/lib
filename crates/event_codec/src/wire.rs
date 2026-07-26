#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

#[cfg(feature = "serde_json")]
pub mod publication;

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

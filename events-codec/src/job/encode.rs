use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec, vec::Vec};

pub use crate::wire::{canonicalize_tags, empty_content, to_draft, EventDraft, WireEventParts};

#[derive(Debug)]
pub enum JobEncodeError {
    MissingProvidersForEncrypted,
    InvalidKind(u32),
    EmptyRequiredField(&'static str),
}

impl fmt::Display for JobEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobEncodeError::MissingProvidersForEncrypted => {
                write!(f, "encrypted=true requires at least one provider ('p') tag")
            }
            JobEncodeError::InvalidKind(k) => write!(f, "invalid job event kind: {}", k),
            JobEncodeError::EmptyRequiredField(n) => write!(f, "empty required field: {}", n),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for JobEncodeError {}

#[cfg(feature = "serde_json")]
pub fn json_content<T: serde::Serialize>(value: &T) -> Result<String, JobEncodeError> {
    serde_json::to_string(value).map_err(|_| JobEncodeError::EmptyRequiredField("content-json"))
}

pub fn push_status_tag(tags: &mut Vec<Vec<String>>, status: &str, extra: Option<&str>) {
    let mut v = vec!["status".into(), status.into()];
    if let Some(e) = extra {
        v.push(e.into());
    }
    tags.push(v);
}

pub fn push_provider_tag(tags: &mut Vec<Vec<String>>, p: &str) {
    tags.push(vec!["p".into(), p.into()]);
}

pub fn push_relay_tag(tags: &mut Vec<Vec<String>>, r: &str) {
    tags.push(vec!["relays".into(), r.into()]);
}

pub fn assert_no_inputs_when_encrypted(tags: &[Vec<String>]) -> bool {
    !tags
        .iter()
        .any(|t| t.get(0).map(|s| s == "i").unwrap_or(false))
}

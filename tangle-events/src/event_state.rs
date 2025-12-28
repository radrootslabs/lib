#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use sha2::{Digest, Sha256};

use crate::error::RadrootsTangleEventsError;

pub fn event_state_key(kind: u32, pubkey: &str, d_tag: &str) -> String {
    format!("{kind}:{pubkey}:{d_tag}")
}

pub fn event_content_hash(
    content: &str,
    tags: &[Vec<String>],
) -> Result<String, RadrootsTangleEventsError> {
    let tags_json = serde_json::to_string(tags)
        .map_err(|_| RadrootsTangleEventsError::InvalidData("tags serialization failed".to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hasher.update(tags_json.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

pub fn tag_value<'a>(tags: &'a [Vec<String>], key: &str) -> Option<&'a str> {
    tags.iter()
        .find(|tag| tag.get(0).map(|v| v.as_str()) == Some(key))
        .and_then(|tag| tag.get(1))
        .map(|value| value.as_str())
}

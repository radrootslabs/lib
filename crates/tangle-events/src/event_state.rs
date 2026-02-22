#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::RadrootsTangleEventsError;

pub fn event_state_key(kind: u32, pubkey: &str, d_tag: &str) -> String {
    format!("{kind}:{pubkey}:{d_tag}")
}

pub fn event_content_hash(
    content: &str,
    tags: &[Vec<String>],
) -> Result<String, RadrootsTangleEventsError> {
    let tags_json = Value::Array(
        tags.iter()
            .map(|tag| Value::Array(tag.iter().cloned().map(Value::String).collect()))
            .collect(),
    )
    .to_string();
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

#[cfg(test)]
mod tests {
    use super::{event_content_hash, event_state_key, tag_value};

    #[test]
    fn event_state_key_formats_consistently() {
        let key = event_state_key(30000, "author", "d-tag");
        assert_eq!(key, "30000:author:d-tag");
    }

    #[test]
    fn event_content_hash_is_stable_for_same_inputs() {
        let tags = vec![vec!["d".to_string(), "tag".to_string()]];
        let first = event_content_hash("content", &tags).expect("hash first");
        let second = event_content_hash("content", &tags).expect("hash second");
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn tag_value_finds_and_misses_keys() {
        let tags = vec![
            vec!["p".to_string(), "member".to_string()],
            vec!["d".to_string(), "farm".to_string()],
            vec!["x".to_string()],
        ];
        assert_eq!(tag_value(&tags, "p"), Some("member"));
        assert_eq!(tag_value(&tags, "d"), Some("farm"));
        assert_eq!(tag_value(&tags, "x"), None);
        assert_eq!(tag_value(&tags, "missing"), None);
    }
}

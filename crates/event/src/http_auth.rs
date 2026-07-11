#![forbid(unsafe_code)]

use crate::kinds::KIND_HTTP_AUTH as KIND_HTTP_AUTH_EVENT;

#[cfg(not(feature = "std"))]
use alloc::string::String;

pub const KIND_HTTP_AUTH: u32 = KIND_HTTP_AUTH_EVENT;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsHttpAuth {
    pub url: String,
    pub method: String,
    pub payload_sha256: Option<String>,
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn http_auth_kind_matches_nip98() {
        assert_eq!(KIND_HTTP_AUTH, 27235);
    }

    #[test]
    fn http_auth_serializes_optional_payload_hash() {
        let value = serde_json::to_value(RadrootsHttpAuth {
            url: "https://media.example.invalid/upload".to_string(),
            method: "POST".to_string(),
            payload_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
        })
        .unwrap();

        assert_eq!(value["url"], "https://media.example.invalid/upload");
        assert_eq!(value["method"], "POST");
        assert_eq!(
            value["payload_sha256"],
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn http_auth_allows_absent_payload_hash() {
        let auth = RadrootsHttpAuth {
            url: "https://media.example.invalid/download".to_string(),
            method: "GET".to_string(),
            payload_sha256: None,
        };

        assert_eq!(auth.payload_sha256, None);
    }
}

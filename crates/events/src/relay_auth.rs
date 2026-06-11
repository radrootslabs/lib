#![forbid(unsafe_code)]

use crate::kinds::KIND_RELAY_AUTH as KIND_RELAY_AUTH_EVENT;

#[cfg(not(feature = "std"))]
use alloc::string::String;

pub const KIND_RELAY_AUTH: u32 = KIND_RELAY_AUTH_EVENT;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayAuth {
    pub relay: String,
    pub challenge: String,
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn relay_auth_kind_matches_nip42() {
        assert_eq!(KIND_RELAY_AUTH, 22242);
    }

    #[test]
    fn relay_auth_serializes_nip42_tags() {
        let value = serde_json::to_value(RadrootsRelayAuth {
            relay: "wss://relay.example.invalid/farm/ABCDEFGHIJKLMNOPQRSTUV".to_string(),
            challenge: "relay-provided-challenge".to_string(),
        })
        .unwrap();

        assert_eq!(
            value["relay"],
            "wss://relay.example.invalid/farm/ABCDEFGHIJKLMNOPQRSTUV"
        );
        assert_eq!(value["challenge"], "relay-provided-challenge");
    }
}

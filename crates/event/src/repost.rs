#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::social::SocialTarget;

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct Repost {
    pub target: SocialTarget,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub content: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct GenericRepost {
    pub target: SocialTarget,
    pub target_kind: u32,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub content: Option<String>,
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn repost_models_represent_note_and_generic_targets() {
        let note_target = SocialTarget::Event {
            id: "a".repeat(64),
            author: Some(crate::test_valid_hex_64('b')),
            event_kind: Some(1),
            relays: None,
        };
        let article_target = SocialTarget::Address {
            address: "30023:pubkey:article".to_string(),
            author: Some(crate::test_valid_hex_64('b')),
            event_kind: Some(30023),
            relays: Some(vec!["wss://relay.example".to_string()]),
        };

        let repost = Repost {
            target: note_target,
            content: None,
        };
        let generic = GenericRepost {
            target: article_target,
            target_kind: 30023,
            content: Some("long-form share".to_string()),
        };

        assert!(matches!(repost.target, SocialTarget::Event { .. }));
        assert_eq!(generic.target_kind, 30023);
        assert!(matches!(generic.target, SocialTarget::Address { .. }));
    }
}

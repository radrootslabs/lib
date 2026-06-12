#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::social::RadrootsSocialTarget;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsRepost {
    pub target: RadrootsSocialTarget,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub content: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGenericRepost {
    pub target: RadrootsSocialTarget,
    pub target_kind: u32,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repost_models_represent_note_and_generic_targets() {
        let note_target = RadrootsSocialTarget::Event {
            id: "a".repeat(64),
            author: Some("b".repeat(64)),
            event_kind: Some(1),
            relays: None,
        };
        let article_target = RadrootsSocialTarget::Address {
            address: "30023:pubkey:article".to_string(),
            author: Some("b".repeat(64)),
            event_kind: Some(30023),
            relays: Some(vec!["wss://relay.example".to_string()]),
        };

        let repost = RadrootsRepost {
            target: note_target,
            content: None,
        };
        let generic = RadrootsGenericRepost {
            target: article_target,
            target_kind: 30023,
            content: Some("long-form share".to_string()),
        };

        assert!(matches!(repost.target, RadrootsSocialTarget::Event { .. }));
        assert_eq!(generic.target_kind, 30023);
        assert!(matches!(
            generic.target,
            RadrootsSocialTarget::Address { .. }
        ));
    }
}

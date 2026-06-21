#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    borrow::ToOwned,
    string::{String, ToString},
    vec::Vec,
};

#[cfg(feature = "std")]
use std::{
    borrow::ToOwned,
    string::{String, ToString},
    vec::Vec,
};

use crate::RadrootsNostrEvent;
use crate::contract::{RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION, event_contract};
use crate::ids::{
    RadrootsEventId, RadrootsEventSignature, RadrootsIdParseError, RadrootsPublicKey,
};
use core::fmt;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsDraftError {
    UnknownContract(String),
    ContractKindMismatch {
        contract_id: String,
        expected_kind: u32,
        actual_kind: u32,
    },
    SignedEventPubkeyMismatch {
        expected_pubkey: String,
        actual_pubkey: String,
    },
    SignedEventIdMismatch {
        expected_event_id: String,
        actual_event_id: String,
    },
    SignedEventCreatedAtMismatch {
        expected_created_at: u32,
        actual_created_at: u32,
    },
    SignedEventKindMismatch {
        expected_kind: u32,
        actual_kind: u32,
    },
    SignedEventTagsMismatch {
        expected_len: usize,
        actual_len: usize,
    },
    SignedEventContentMismatch {
        expected_len: usize,
        actual_len: usize,
    },
    SignedEventComputedIdMismatch {
        expected_event_id: String,
        computed_event_id: String,
    },
    IdParse(RadrootsIdParseError),
    JsonString(String),
}

impl fmt::Display for RadrootsDraftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownContract(contract_id) => {
                write!(f, "unknown event contract `{contract_id}`")
            }
            Self::ContractKindMismatch {
                contract_id,
                expected_kind,
                actual_kind,
            } => write!(
                f,
                "event contract `{contract_id}` expects kind {expected_kind}, got {actual_kind}"
            ),
            Self::SignedEventPubkeyMismatch {
                expected_pubkey,
                actual_pubkey,
            } => write!(
                f,
                "signed event pubkey mismatch: expected {expected_pubkey}, got {actual_pubkey}"
            ),
            Self::SignedEventIdMismatch {
                expected_event_id,
                actual_event_id,
            } => write!(
                f,
                "signed event id mismatch: expected {expected_event_id}, got {actual_event_id}"
            ),
            Self::SignedEventCreatedAtMismatch {
                expected_created_at,
                actual_created_at,
            } => write!(
                f,
                "signed event created_at mismatch: expected {expected_created_at}, got {actual_created_at}"
            ),
            Self::SignedEventKindMismatch {
                expected_kind,
                actual_kind,
            } => write!(
                f,
                "signed event kind mismatch: expected {expected_kind}, got {actual_kind}"
            ),
            Self::SignedEventTagsMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "signed event tags mismatch: expected {expected_len} tags, got {actual_len} tags"
            ),
            Self::SignedEventContentMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "signed event content mismatch: expected {expected_len} bytes, got {actual_len} bytes"
            ),
            Self::SignedEventComputedIdMismatch {
                expected_event_id,
                computed_event_id,
            } => write!(
                f,
                "signed event computed id mismatch: expected {expected_event_id}, computed {computed_event_id}"
            ),
            Self::IdParse(error) => write!(f, "{error}"),
            Self::JsonString(error) => write!(f, "json string serialization failed: {error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsDraftError {}

impl From<RadrootsIdParseError> for RadrootsDraftError {
    fn from(value: RadrootsIdParseError) -> Self {
        Self::IdParse(value)
    }
}

impl From<serde_json::Error> for RadrootsDraftError {
    fn from(value: serde_json::Error) -> Self {
        Self::JsonString(value.to_string())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsFrozenEventDraft {
    pub contract_id: String,
    pub contract_registry_version: u32,
    pub kind: u32,
    pub created_at: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub expected_pubkey: String,
    pub expected_event_id: String,
}

impl RadrootsFrozenEventDraft {
    pub fn new(
        contract_id: impl Into<String>,
        kind: u32,
        created_at: u32,
        tags: Vec<Vec<String>>,
        content: impl Into<String>,
        expected_pubkey: impl AsRef<str>,
    ) -> Result<Self, RadrootsDraftError> {
        let contract_id = contract_id.into();
        let contract = event_contract(&contract_id)
            .ok_or_else(|| RadrootsDraftError::UnknownContract(contract_id.clone()))?;
        if contract.kind != kind {
            return Err(RadrootsDraftError::ContractKindMismatch {
                contract_id,
                expected_kind: contract.kind,
                actual_kind: kind,
            });
        }
        let expected_pubkey = RadrootsPublicKey::parse(expected_pubkey.as_ref())?.into_string();
        let content = content.into();
        let expected_event_id =
            compute_nip01_event_id(expected_pubkey.as_str(), created_at, kind, &tags, &content)?
                .into_string();
        Ok(Self {
            contract_id: contract.id.to_owned(),
            contract_registry_version: RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION,
            kind,
            created_at,
            tags,
            content,
            expected_pubkey,
            expected_event_id,
        })
    }

    pub fn nip01_preimage(&self) -> Result<String, RadrootsDraftError> {
        nip01_event_id_preimage(
            self.expected_pubkey.as_str(),
            self.created_at,
            self.kind,
            &self.tags,
            self.content.as_str(),
        )
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSignedNostrEventParts {
    pub id: String,
    pub pubkey: String,
    pub created_at: u32,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
    pub raw_json: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSignedNostrEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: u32,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
    pub raw_json: String,
}

impl RadrootsSignedNostrEvent {
    pub fn new(parts: RadrootsSignedNostrEventParts) -> Result<Self, RadrootsDraftError> {
        let id = RadrootsEventId::parse(parts.id)?.into_string();
        let pubkey = RadrootsPublicKey::parse(parts.pubkey)?.into_string();
        let sig = RadrootsEventSignature::parse(parts.sig)?.into_string();
        Ok(Self {
            id,
            pubkey,
            created_at: parts.created_at,
            kind: parts.kind,
            tags: parts.tags,
            content: parts.content,
            sig,
            raw_json: parts.raw_json,
        })
    }

    pub fn from_event(
        event: RadrootsNostrEvent,
        raw_json: impl Into<String>,
    ) -> Result<Self, RadrootsDraftError> {
        Self::new(RadrootsSignedNostrEventParts {
            id: event.id,
            pubkey: event.author,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags,
            content: event.content,
            sig: event.sig,
            raw_json: raw_json.into(),
        })
    }
}

pub fn validate_signed_nostr_event_matches_draft(
    signed_event: &RadrootsSignedNostrEvent,
    draft: &RadrootsFrozenEventDraft,
) -> Result<(), RadrootsDraftError> {
    if signed_event.pubkey.as_str() != draft.expected_pubkey.as_str() {
        return Err(RadrootsDraftError::SignedEventPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey.clone(),
            actual_pubkey: signed_event.pubkey.clone(),
        });
    }
    if signed_event.id.as_str() != draft.expected_event_id.as_str() {
        return Err(RadrootsDraftError::SignedEventIdMismatch {
            expected_event_id: draft.expected_event_id.clone(),
            actual_event_id: signed_event.id.clone(),
        });
    }
    if signed_event.created_at != draft.created_at {
        return Err(RadrootsDraftError::SignedEventCreatedAtMismatch {
            expected_created_at: draft.created_at,
            actual_created_at: signed_event.created_at,
        });
    }
    if signed_event.kind != draft.kind {
        return Err(RadrootsDraftError::SignedEventKindMismatch {
            expected_kind: draft.kind,
            actual_kind: signed_event.kind,
        });
    }
    if signed_event.tags != draft.tags {
        return Err(RadrootsDraftError::SignedEventTagsMismatch {
            expected_len: draft.tags.len(),
            actual_len: signed_event.tags.len(),
        });
    }
    if signed_event.content != draft.content {
        return Err(RadrootsDraftError::SignedEventContentMismatch {
            expected_len: draft.content.len(),
            actual_len: signed_event.content.len(),
        });
    }
    let computed_event_id = compute_nip01_event_id(
        signed_event.pubkey.as_str(),
        signed_event.created_at,
        signed_event.kind,
        &signed_event.tags,
        signed_event.content.as_str(),
    )?
    .into_string();
    if computed_event_id.as_str() != signed_event.id.as_str() {
        return Err(RadrootsDraftError::SignedEventComputedIdMismatch {
            expected_event_id: signed_event.id.clone(),
            computed_event_id,
        });
    }
    Ok(())
}

pub fn compute_nip01_event_id(
    pubkey: &str,
    created_at: u32,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsEventId, RadrootsDraftError> {
    let pubkey = RadrootsPublicKey::parse(pubkey)?;
    let preimage = nip01_event_id_preimage(pubkey.as_str(), created_at, kind, tags, content)?;
    let digest = Sha256::digest(preimage.as_bytes());
    let event_id = hex::encode(digest);
    Ok(RadrootsEventId::parse(event_id)?)
}

pub fn nip01_event_id_preimage(
    pubkey: &str,
    created_at: u32,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<String, RadrootsDraftError> {
    let mut preimage = String::new();
    preimage.push_str("[0,");
    push_json_string(&mut preimage, pubkey)?;
    preimage.push(',');
    preimage.push_str(created_at.to_string().as_str());
    preimage.push(',');
    preimage.push_str(kind.to_string().as_str());
    preimage.push_str(",[");
    for (tag_index, tag) in tags.iter().enumerate() {
        if tag_index > 0 {
            preimage.push(',');
        }
        preimage.push('[');
        for (value_index, value) in tag.iter().enumerate() {
            if value_index > 0 {
                preimage.push(',');
            }
            push_json_string(&mut preimage, value)?;
        }
        preimage.push(']');
    }
    preimage.push_str("],");
    push_json_string(&mut preimage, content)?;
    preimage.push(']');
    Ok(preimage)
}

fn push_json_string(target: &mut String, value: &str) -> Result<(), RadrootsDraftError> {
    target.push_str(serde_json::to_string(value)?.as_str());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{KIND_POST, KIND_PROFILE};

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn signed_event_for_draft(draft: &RadrootsFrozenEventDraft) -> RadrootsSignedNostrEvent {
        RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: draft.expected_event_id.clone(),
            pubkey: draft.expected_pubkey.clone(),
            created_at: draft.created_at,
            kind: draft.kind,
            tags: draft.tags.clone(),
            content: draft.content.clone(),
            sig: "b".repeat(128),
            raw_json: "{}".to_owned(),
        })
        .expect("signed event")
    }

    fn post_draft() -> RadrootsFrozenEventDraft {
        RadrootsFrozenEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1_700_000_000,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "hello",
            "a".repeat(64),
        )
        .expect("draft")
    }

    #[test]
    fn frozen_draft_computes_expected_event_id() {
        let draft = RadrootsFrozenEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1_700_000_000,
            vec![
                vec!["t".to_owned(), "soil".to_owned()],
                vec!["p".to_owned(), hex_64('b')],
            ],
            "hello",
            hex_64('a'),
        )
        .expect("draft");

        assert_eq!(
            draft.nip01_preimage().expect("preimage"),
            "[0,\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",1700000000,1,[[\"t\",\"soil\"],[\"p\",\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"]],\"hello\"]"
        );
        assert_eq!(
            draft.expected_event_id,
            "59d2486ef5557e0e317127de55005f2863361ad4041277ae523a869f2294cf9c"
        );
    }

    #[test]
    fn deterministic_event_id_changes_when_preimage_changes() {
        let tags = vec![vec!["t".to_owned(), "soil".to_owned()]];
        let base = compute_nip01_event_id(hex_64('a').as_str(), 1, KIND_POST, &tags, "hello")
            .expect("base");
        let pubkey_changed =
            compute_nip01_event_id(hex_64('b').as_str(), 1, KIND_POST, &tags, "hello")
                .expect("pubkey");
        let time_changed =
            compute_nip01_event_id(hex_64('a').as_str(), 2, KIND_POST, &tags, "hello")
                .expect("time");
        let kind_changed =
            compute_nip01_event_id(hex_64('a').as_str(), 1, KIND_PROFILE, &tags, "hello")
                .expect("kind");
        let tag_order_changed = compute_nip01_event_id(
            hex_64('a').as_str(),
            1,
            KIND_POST,
            &[
                vec!["p".to_owned(), hex_64('c')],
                vec!["t".to_owned(), "soil".to_owned()],
            ],
            "hello",
        )
        .expect("tag order");
        let content_changed =
            compute_nip01_event_id(hex_64('a').as_str(), 1, KIND_POST, &tags, "hello!")
                .expect("content");

        assert_ne!(base, pubkey_changed);
        assert_ne!(base, time_changed);
        assert_ne!(base, kind_changed);
        assert_ne!(base, tag_order_changed);
        assert_ne!(base, content_changed);
    }

    #[test]
    fn profile_golden_event_id_is_stable() {
        let event_id = compute_nip01_event_id(hex_64('c').as_str(), 1_700_000_001, 0, &[], "{}")
            .expect("event id");

        assert_eq!(
            event_id.as_str(),
            "2a15e33622a155ae231b28bebe390869e67a0e228f77ecfcd652b1ce180a9dde"
        );
    }

    #[test]
    fn draft_constructor_rejects_unknown_contract_and_kind_mismatch() {
        let unknown =
            RadrootsFrozenEventDraft::new("missing", KIND_POST, 1, Vec::new(), "", hex_64('a'))
                .expect_err("unknown contract");
        assert!(matches!(unknown, RadrootsDraftError::UnknownContract(_)));

        let mismatch = RadrootsFrozenEventDraft::new(
            "radroots.social.post.v1",
            KIND_PROFILE,
            1,
            Vec::new(),
            "",
            hex_64('a'),
        )
        .expect_err("kind mismatch");
        assert!(matches!(
            mismatch,
            RadrootsDraftError::ContractKindMismatch { .. }
        ));

        let invalid_pubkey = RadrootsFrozenEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1,
            Vec::new(),
            "",
            "not-hex",
        )
        .expect_err("invalid pubkey");
        assert!(matches!(invalid_pubkey, RadrootsDraftError::IdParse(_)));
    }

    #[test]
    fn signed_event_validates_ids_and_roundtrips_with_serde() {
        let signed = RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: hex_64('d'),
            pubkey: hex_64('e'),
            created_at: 10,
            kind: KIND_POST,
            tags: Vec::new(),
            content: "hello".to_owned(),
            sig: "f".repeat(128),
            raw_json: "{\"id\":\"fixture\"}".to_owned(),
        })
        .expect("signed event");
        let json = serde_json::to_string(&signed).expect("serialize");
        let decoded: RadrootsSignedNostrEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, signed);
        assert_eq!(decoded.pubkey, hex_64('e'));
    }

    #[test]
    fn signed_event_from_nostr_event_validates_parts() {
        let event = RadrootsNostrEvent {
            id: hex_64('1'),
            author: hex_64('2'),
            created_at: 42,
            kind: KIND_POST,
            tags: vec![vec!["t".to_owned(), "soil".to_owned()]],
            content: "hello".to_owned(),
            sig: "3".repeat(128),
        };
        let signed = RadrootsSignedNostrEvent::from_event(event, "{\"id\":\"fixture\"}")
            .expect("signed event");

        assert_eq!(signed.id, hex_64('1'));
        assert_eq!(signed.pubkey, hex_64('2'));
        assert_eq!(signed.sig, "3".repeat(128));
        assert_eq!(signed.raw_json, "{\"id\":\"fixture\"}");

        let invalid = RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: "not-hex".to_owned(),
            pubkey: hex_64('e'),
            created_at: 10,
            kind: KIND_POST,
            tags: Vec::new(),
            content: String::new(),
            sig: "f".repeat(128),
            raw_json: "{}".to_owned(),
        })
        .expect_err("invalid id");
        assert!(matches!(invalid, RadrootsDraftError::IdParse(_)));

        let invalid = RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: hex_64('d'),
            pubkey: "not-hex".to_owned(),
            created_at: 10,
            kind: KIND_POST,
            tags: Vec::new(),
            content: String::new(),
            sig: "f".repeat(128),
            raw_json: "{}".to_owned(),
        })
        .expect_err("invalid pubkey");
        assert!(matches!(invalid, RadrootsDraftError::IdParse(_)));

        let invalid = RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: hex_64('d'),
            pubkey: hex_64('e'),
            created_at: 10,
            kind: KIND_POST,
            tags: Vec::new(),
            content: String::new(),
            sig: "not-hex".to_owned(),
            raw_json: "{}".to_owned(),
        })
        .expect_err("invalid sig");
        assert!(matches!(invalid, RadrootsDraftError::IdParse(_)));
    }

    #[test]
    fn signed_event_validation_accepts_exact_draft_match() {
        let draft = post_draft();
        let signed = signed_event_for_draft(&draft);

        validate_signed_nostr_event_matches_draft(&signed, &draft).expect("valid signed event");
    }

    #[test]
    fn signed_event_validation_rejects_draft_mismatches() {
        let draft = post_draft();

        let mut signed = signed_event_for_draft(&draft);
        signed.pubkey = hex_64('c');
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(
            error,
            RadrootsDraftError::SignedEventPubkeyMismatch { .. }
        ));

        let mut signed = signed_event_for_draft(&draft);
        signed.id = hex_64('d');
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(
            error,
            RadrootsDraftError::SignedEventIdMismatch { .. }
        ));

        let mut signed = signed_event_for_draft(&draft);
        signed.created_at += 1;
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(
            error,
            RadrootsDraftError::SignedEventCreatedAtMismatch { .. }
        ));

        let mut signed = signed_event_for_draft(&draft);
        signed.kind = KIND_PROFILE;
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(
            error,
            RadrootsDraftError::SignedEventKindMismatch { .. }
        ));

        let mut signed = signed_event_for_draft(&draft);
        signed.tags.push(vec!["p".to_owned(), hex_64('e')]);
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(
            error,
            RadrootsDraftError::SignedEventTagsMismatch { .. }
        ));

        let mut signed = signed_event_for_draft(&draft);
        signed.content = "changed".to_owned();
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(
            error,
            RadrootsDraftError::SignedEventContentMismatch { .. }
        ));

        let mut draft = post_draft();
        draft.expected_event_id = hex_64('f');
        let signed = signed_event_for_draft(&draft);
        let error =
            validate_signed_nostr_event_matches_draft(&signed, &draft).expect_err("mismatch");
        assert!(matches!(
            error,
            RadrootsDraftError::SignedEventComputedIdMismatch { .. }
        ));
    }

    #[test]
    fn draft_errors_format_all_variants() {
        let errors = [
            RadrootsDraftError::UnknownContract("missing".to_owned()),
            RadrootsDraftError::ContractKindMismatch {
                contract_id: "radroots.social.post.v1".to_owned(),
                expected_kind: KIND_POST,
                actual_kind: KIND_PROFILE,
            },
            RadrootsDraftError::SignedEventPubkeyMismatch {
                expected_pubkey: hex_64('a'),
                actual_pubkey: hex_64('b'),
            },
            RadrootsDraftError::SignedEventIdMismatch {
                expected_event_id: hex_64('c'),
                actual_event_id: hex_64('d'),
            },
            RadrootsDraftError::SignedEventCreatedAtMismatch {
                expected_created_at: 1,
                actual_created_at: 2,
            },
            RadrootsDraftError::SignedEventKindMismatch {
                expected_kind: KIND_POST,
                actual_kind: KIND_PROFILE,
            },
            RadrootsDraftError::SignedEventTagsMismatch {
                expected_len: 1,
                actual_len: 2,
            },
            RadrootsDraftError::SignedEventContentMismatch {
                expected_len: 5,
                actual_len: 7,
            },
            RadrootsDraftError::SignedEventComputedIdMismatch {
                expected_event_id: hex_64('e'),
                computed_event_id: hex_64('f'),
            },
            RadrootsDraftError::from(RadrootsIdParseError::Empty),
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }

        let json_error = serde_json::from_str::<String>("{").expect_err("json error");
        let error = RadrootsDraftError::from(json_error);
        assert!(
            error
                .to_string()
                .contains("json string serialization failed")
        );
    }

    #[test]
    fn event_id_computation_rejects_invalid_pubkeys() {
        let error =
            compute_nip01_event_id("not-hex", 1, KIND_POST, &[], "").expect_err("invalid pubkey");
        assert!(matches!(error, RadrootsDraftError::IdParse(_)));
    }
}

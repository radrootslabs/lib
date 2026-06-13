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
    pub fn new(
        id: impl AsRef<str>,
        pubkey: impl AsRef<str>,
        created_at: u32,
        kind: u32,
        tags: Vec<Vec<String>>,
        content: impl Into<String>,
        sig: impl AsRef<str>,
        raw_json: impl Into<String>,
    ) -> Result<Self, RadrootsDraftError> {
        let id = RadrootsEventId::parse(id.as_ref())?.into_string();
        let pubkey = RadrootsPublicKey::parse(pubkey.as_ref())?.into_string();
        let sig = RadrootsEventSignature::parse(sig.as_ref())?.into_string();
        Ok(Self {
            id,
            pubkey,
            created_at,
            kind,
            tags,
            content: content.into(),
            sig,
            raw_json: raw_json.into(),
        })
    }

    pub fn from_event(
        event: RadrootsNostrEvent,
        raw_json: impl Into<String>,
    ) -> Result<Self, RadrootsDraftError> {
        Self::new(
            event.id,
            event.author,
            event.created_at,
            event.kind,
            event.tags,
            event.content,
            event.sig,
            raw_json,
        )
    }
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
    }

    #[test]
    fn signed_event_validates_ids_and_roundtrips_with_serde() {
        let signed = RadrootsSignedNostrEvent::new(
            hex_64('d'),
            hex_64('e'),
            10,
            KIND_POST,
            Vec::new(),
            "hello",
            "f".repeat(128),
            "{\"id\":\"fixture\"}",
        )
        .expect("signed event");
        let json = serde_json::to_string(&signed).expect("serialize");
        let decoded: RadrootsSignedNostrEvent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, signed);
        assert_eq!(decoded.pubkey, hex_64('e'));
    }
}

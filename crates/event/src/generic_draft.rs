//! Generic-only authored event input.

#[cfg(all(not(feature = "std"), not(test)))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};
#[cfg(any(feature = "std", test))]
use std::{borrow::ToOwned, string::String, vec::Vec};

use radroots_identity::PublicKey;

use crate::{
    contract::{
        ContractKey, EventAuthoringPolicy, RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION,
        validate_event_contract_parts,
    },
    draft::DraftError,
    envelope::{EventKind, EventTags, EventTimestamp},
    id::{EventId, parse_public_key},
    wire::{canonical_nip01_event_id_preimage, compute_canonical_nip01_event_id},
};

/// A validated generic-only authored event input.
///
/// Typed-only and read-only contracts cannot cross this boundary. Typed
/// authoring is owned by explicit codec conversions instead.
///
/// ```compile_fail
/// use radroots_event::{GenericEventDraft, post::AuthoredUpdate};
///
/// let update = AuthoredUpdate::new("harvest").unwrap();
/// let _ = GenericEventDraft::from_authored_update(&update, 1, "00");
/// ```
#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericEventDraft {
    contract: ContractKey,
    kind: EventKind,
    created_at: EventTimestamp,
    tags: EventTags,
    content: String,
    expected_pubkey: PublicKey,
    expected_event_id: EventId,
}

impl GenericEventDraft {
    pub fn new(
        contract_id: impl Into<String>,
        kind: u32,
        created_at: u64,
        tags: Vec<Vec<String>>,
        content: impl Into<String>,
        expected_pubkey: impl AsRef<str>,
    ) -> Result<Self, DraftError> {
        let raw_contract_id = contract_id.into();
        let contract = ContractKey::current(raw_contract_id.clone())
            .map_err(|_| DraftError::UnknownContract(raw_contract_id))?;
        let definition = contract.contract();
        crate::require_invariant(definition.kind == kind, &|| {
            DraftError::ContractKindMismatch {
                contract_id: definition.id.to_owned(),
                expected_kind: definition.kind,
                actual_kind: kind,
            }
        })?;
        crate::require_invariant(
            definition.authoring_policy() == EventAuthoringPolicy::GenericDraft,
            &|| DraftError::ContractNotDraftAuthorable {
                contract_id: definition.id.to_owned(),
            },
        )?;
        let content = content.into();
        validate_event_contract_parts(kind, &tags, &content, definition.id).map_err(|error| {
            DraftError::ContractShape {
                contract_id: definition.id.to_owned(),
                error,
            }
        })?;
        let tags = EventTags::new(tags)?;
        let expected_pubkey = parse_public_key(expected_pubkey.as_ref())?;
        let expected_event_id = compute_canonical_nip01_event_id(
            &expected_pubkey.to_hex(),
            created_at,
            kind,
            &tags.to_vec(),
            &content,
        )?;
        Ok(Self {
            contract,
            kind: EventKind::new(kind),
            created_at: EventTimestamp::new(created_at),
            tags,
            content,
            expected_pubkey,
            expected_event_id,
        })
    }

    pub fn validate_for_authoring(&self) -> Result<(), DraftError> {
        let reconstructed = Self::new(
            self.contract.contract_id().as_str(),
            self.kind_u32(),
            self.created_at_u64(),
            self.tags_as_vec(),
            self.content.clone(),
            self.expected_pubkey.to_hex(),
        )?;
        crate::require_invariant(reconstructed.contract == self.contract, &|| {
            DraftError::ContractRegistryVersionMismatch {
                expected: RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION,
                actual: self.contract.registry_version().get(),
            }
        })?;
        crate::require_invariant(
            reconstructed.expected_event_id == self.expected_event_id,
            &|| DraftError::DraftExpectedEventIdMismatch {
                expected_event_id: reconstructed.expected_event_id.to_hex(),
                actual_event_id: self.expected_event_id.to_hex(),
            },
        )
    }

    pub fn nip01_preimage(&self) -> Result<String, DraftError> {
        Ok(canonical_nip01_event_id_preimage(
            &self.expected_pubkey.to_hex(),
            self.created_at_u64(),
            self.kind_u32(),
            &self.tags_as_vec(),
            self.content(),
        )?)
    }

    #[must_use]
    pub const fn contract(&self) -> &ContractKey {
        &self.contract
    }

    #[must_use]
    pub fn contract_id(&self) -> &str {
        self.contract.contract_id().as_str()
    }

    #[must_use]
    pub const fn kind(&self) -> EventKind {
        self.kind
    }

    #[must_use]
    pub const fn kind_u32(&self) -> u32 {
        self.kind.as_u32()
    }

    #[must_use]
    pub const fn created_at(&self) -> EventTimestamp {
        self.created_at
    }

    #[must_use]
    pub const fn created_at_u64(&self) -> u64 {
        self.created_at.as_u64()
    }

    #[must_use]
    pub const fn tags(&self) -> &EventTags {
        &self.tags
    }

    #[must_use]
    pub fn tags_as_vec(&self) -> Vec<Vec<String>> {
        self.tags.to_vec()
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub const fn expected_pubkey(&self) -> &PublicKey {
        &self.expected_pubkey
    }

    #[must_use]
    pub const fn expected_event_id(&self) -> &EventId {
        &self.expected_event_id
    }

    #[must_use]
    pub fn expected_event_id_hex(&self) -> String {
        self.expected_event_id.to_hex()
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for GenericEventDraft {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct GenericDraftSerde {
            contract: ContractKey,
            kind: EventKind,
            created_at: EventTimestamp,
            tags: EventTags,
            content: String,
            expected_pubkey: PublicKey,
            expected_event_id: EventId,
        }

        let value = GenericDraftSerde::deserialize(deserializer)?;
        let draft = Self::new(
            value.contract.contract_id().as_str(),
            value.kind.as_u32(),
            value.created_at.as_u64(),
            value.tags.to_vec(),
            value.content,
            value.expected_pubkey.to_hex(),
        )
        .map_err(serde::de::Error::custom)?;
        if draft.contract != value.contract {
            return Err(serde::de::Error::custom(
                "generic draft contract key mismatch",
            ));
        }
        if draft.expected_event_id != value.expected_event_id {
            return Err(serde::de::Error::custom(
                DraftError::DraftExpectedEventIdMismatch {
                    expected_event_id: draft.expected_event_id.to_hex(),
                    actual_event_id: value.expected_event_id.to_hex(),
                },
            ));
        }
        Ok(draft)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contract::{EventAuthoringPolicy, RegistryVersion, all_event_contracts},
        envelope::kind::KIND_GEOCHAT,
    };

    const PUBLIC_KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn draft() -> GenericEventDraft {
        GenericEventDraft::new(
            "radroots.social.geochat.v1",
            KIND_GEOCHAT,
            1_700_000_000,
            Vec::new(),
            "hello",
            PUBLIC_KEY,
        )
        .expect("generic draft")
    }

    #[test]
    fn generic_input_binds_exact_contract_author_timestamp_and_wire() {
        let draft = draft();
        assert_eq!(draft.contract_id(), "radroots.social.geochat.v1");
        assert_eq!(
            draft.contract().registry_version(),
            RegistryVersion::CURRENT
        );
        assert_eq!(draft.kind_u32(), KIND_GEOCHAT);
        assert_eq!(draft.created_at_u64(), 1_700_000_000);
        assert_eq!(draft.expected_pubkey().to_hex(), PUBLIC_KEY);
        assert_eq!(
            draft.nip01_preimage().expect("preimage"),
            "[0,\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",1700000000,20000,[],\"hello\"]"
        );
        draft.validate_for_authoring().expect("valid draft");
    }

    #[test]
    fn generic_input_rejects_every_non_generic_registry_contract() {
        for contract in all_event_contracts()
            .iter()
            .filter(|contract| contract.authoring_policy() != EventAuthoringPolicy::GenericDraft)
        {
            let error = GenericEventDraft::new(
                contract.id,
                contract.kind,
                1,
                Vec::new(),
                "hello",
                PUBLIC_KEY,
            )
            .expect_err("typed-only and read-only contracts must fail before shape validation");
            assert_eq!(
                error,
                DraftError::ContractNotDraftAuthorable {
                    contract_id: contract.id.to_owned(),
                },
                "{}",
                contract.id
            );
        }
    }

    #[test]
    fn serde_reconstruction_revalidates_all_private_fields() {
        let draft = draft();
        let value = serde_json::to_value(&draft).expect("draft JSON");
        assert_eq!(
            serde_json::from_value::<GenericEventDraft>(value.clone())
                .expect("decoded generic draft"),
            draft
        );

        for (field, replacement) in [
            ("kind", serde_json::json!(1)),
            ("content", serde_json::json!("")),
            ("expected_event_id", serde_json::json!("f".repeat(64))),
        ] {
            let mut invalid = value.clone();
            invalid[field] = replacement;
            assert!(serde_json::from_value::<GenericEventDraft>(invalid).is_err());
        }
        let mut invalid = value.clone();
        invalid["contract"]["contract_id"] = serde_json::json!("radroots.social.update.v1");
        assert!(serde_json::from_value::<GenericEventDraft>(invalid).is_err());
        let mut invalid = value;
        invalid["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<GenericEventDraft>(invalid).is_err());
    }
}

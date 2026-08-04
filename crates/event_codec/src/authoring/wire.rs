//! Bounded, proof-carrying storage wire for authored event plans.

#[cfg(all(not(feature = "std"), feature = "json"))]
use alloc::string::ToString;
#[cfg(not(feature = "std"))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{borrow::ToOwned, string::String, vec::Vec};

use core::fmt;
#[cfg(feature = "json")]
use radroots_event::{
    contract::{ContractId, EventAuthoringPolicy, validate_event_contract_parts},
    envelope::EventTags,
    wire::compute_canonical_nip01_event_id,
};
use radroots_event::{
    contract::{ContractIdentityError, ContractKey, RegistryVersion},
    id::EventId,
    wire::v1::DEFAULT_RAW_JSON_MAX_BYTES,
};
use radroots_identity::PublicKey;

#[cfg(feature = "json")]
use super::{AuthoredEventBody, typed::validate_historical_typed_profile};
use super::{AuthoredEventPlan, PLAN_WIRE_VERSION_V1, PlanDigest, PlanDigestError};

/// Hard byte limit applied before JSON parsing or field allocation.
pub const PLAN_WIRE_MAX_BYTES: usize = DEFAULT_RAW_JSON_MAX_BYTES;

/// Exact version-one durable representation of an authored event plan.
///
/// Construction is limited to validated plans. Deserialization is intentionally
/// implemented only through [`Self::from_json`], which validates the embedded
/// historical registry profile and both cryptographic commitments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanWireV1 {
    schema_version: u32,
    contract: ContractKey,
    expected_author: PublicKey,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    expected_event_id: EventId,
    plan_digest: PlanDigest,
}

impl PlanWireV1 {
    #[must_use]
    pub fn from_plan(plan: &AuthoredEventPlan) -> Self {
        Self {
            schema_version: PLAN_WIRE_VERSION_V1,
            contract: plan.body().contract().clone(),
            expected_author: *plan.author(),
            created_at: plan.created_at(),
            kind: plan.body().kind(),
            tags: plan.body().tags().to_vec(),
            content: plan.body().content().to_owned(),
            expected_event_id: *plan.expected_event_id(),
            plan_digest: plan.digest(),
        }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub const fn contract(&self) -> &ContractKey {
        &self.contract
    }

    #[must_use]
    pub const fn expected_author(&self) -> &PublicKey {
        &self.expected_author
    }

    #[must_use]
    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    #[must_use]
    pub const fn kind(&self) -> u32 {
        self.kind
    }

    #[must_use]
    pub fn tags(&self) -> &[Vec<String>] {
        &self.tags
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub const fn expected_event_id(&self) -> &EventId {
        &self.expected_event_id
    }

    #[must_use]
    pub const fn plan_digest(&self) -> PlanDigest {
        self.plan_digest
    }

    #[cfg(feature = "json")]
    pub fn to_json(&self) -> Result<Vec<u8>, PlanDecodeError> {
        serde_json::to_vec(self).map_err(|error| PlanDecodeError::Json(error.to_string()))
    }

    #[cfg(feature = "json")]
    pub fn from_json(bytes: &[u8]) -> Result<HistoricalPlanIntegrity, PlanDecodeError> {
        if bytes.len() > PLAN_WIRE_MAX_BYTES {
            return Err(PlanDecodeError::RawJsonTooLarge {
                max: PLAN_WIRE_MAX_BYTES,
                actual: bytes.len(),
            });
        }
        let wire = serde_json::from_slice::<RawPlanWireV1>(bytes)
            .map_err(|error| PlanDecodeError::Json(error.to_string()))?;
        Self::validate_raw(wire)
    }

    #[cfg(feature = "json")]
    fn validate_raw(wire: RawPlanWireV1) -> Result<HistoricalPlanIntegrity, PlanDecodeError> {
        if wire.schema_version != PLAN_WIRE_VERSION_V1 {
            return Err(PlanDecodeError::UnsupportedSchemaVersion {
                actual: wire.schema_version,
            });
        }

        let registry_version = RegistryVersion::new(wire.contract_registry_version)
            .map_err(PlanDecodeError::ContractIdentity)?;
        let contract_id =
            ContractId::parse(wire.contract_id).map_err(PlanDecodeError::ContractIdentity)?;
        let contract = ContractKey::new(registry_version, contract_id)
            .map_err(PlanDecodeError::ContractIdentity)?;
        let definition = contract.contract();
        if definition.kind != wire.kind {
            return Err(PlanDecodeError::ContractKindMismatch {
                expected: definition.kind,
                actual: wire.kind,
            });
        }
        match definition.authoring_policy() {
            EventAuthoringPolicy::GenericDraft => {
                validate_event_contract_parts(wire.kind, &wire.tags, &wire.content, definition.id)
                    .map_err(|error| PlanDecodeError::HistoricalShape(error.code().to_owned()))?;
            }
            EventAuthoringPolicy::TypedOnly => validate_historical_typed_profile(
                definition.id,
                wire.created_at,
                wire.kind,
                &wire.tags,
                &wire.content,
            )
            .map_err(PlanDecodeError::HistoricalShape)?,
            EventAuthoringPolicy::ReadOnly => {
                return Err(PlanDecodeError::HistoricalProfileUnavailable {
                    contract_id: definition.id.to_owned(),
                });
            }
        }
        EventTags::new(wire.tags.clone())
            .map_err(|error| PlanDecodeError::HistoricalShape(error.to_string()))?;
        let expected_author = PublicKey::from_hex(&wire.expected_author)
            .map_err(|error| PlanDecodeError::ExpectedAuthor(error.to_string()))?;
        if expected_author.to_hex() != wire.expected_author {
            return Err(PlanDecodeError::NonCanonicalExpectedAuthor);
        }
        let expected_event_id = EventId::parse(&wire.expected_event_id)
            .map_err(|error| PlanDecodeError::ExpectedEventId(error.to_string()))?;
        if expected_event_id.to_hex() != wire.expected_event_id {
            return Err(PlanDecodeError::NonCanonicalExpectedEventId);
        }
        let recomputed_event_id = compute_canonical_nip01_event_id(
            &wire.expected_author,
            wire.created_at,
            wire.kind,
            &wire.tags,
            &wire.content,
        )
        .map_err(|error| PlanDecodeError::ExpectedEventId(error.to_string()))?;
        if recomputed_event_id != expected_event_id {
            return Err(PlanDecodeError::EventIdMismatch {
                declared: expected_event_id.to_hex(),
                computed: recomputed_event_id.to_hex(),
            });
        }

        let declared_digest =
            PlanDigest::parse_hex(&wire.plan_digest).map_err(PlanDecodeError::PlanDigest)?;
        let body = AuthoredEventBody {
            contract: contract.clone(),
            kind: wire.kind,
            tags: wire.tags,
            content: wire.content,
        };
        let plan = AuthoredEventPlan::from_validated_parts(
            body,
            expected_author,
            wire.created_at,
            expected_event_id,
        );
        if plan.digest() != declared_digest {
            return Err(PlanDecodeError::PlanDigestMismatch {
                declared: declared_digest.to_hex(),
                computed: plan.digest().to_hex(),
            });
        }

        Ok(HistoricalPlanIntegrity { plan })
    }
}

/// Proof that a plan reconstructed successfully under its embedded registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoricalPlanIntegrity {
    plan: AuthoredEventPlan,
}

impl HistoricalPlanIntegrity {
    #[must_use]
    pub const fn plan(&self) -> &AuthoredEventPlan {
        &self.plan
    }

    #[must_use]
    pub fn into_plan(self) -> AuthoredEventPlan {
        self.plan
    }

    /// Compares registry versions for policy reporting only.
    ///
    /// This relation is not current signing authorization. Signing must obtain
    /// an explicit authorization decision from `radroots_signing`.
    #[must_use]
    pub fn registry_relation(&self, current: RegistryVersion) -> PlanRegistryRelation {
        if self.plan.body().contract().registry_version() == current {
            PlanRegistryRelation::Current
        } else {
            PlanRegistryRelation::Historical
        }
    }
}

/// Informational relation between an intact historical plan and current policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanRegistryRelation {
    Current,
    Historical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanDecodeError {
    RawJsonTooLarge { max: usize, actual: usize },
    Json(String),
    UnsupportedSchemaVersion { actual: u32 },
    ContractIdentity(ContractIdentityError),
    ContractKindMismatch { expected: u32, actual: u32 },
    HistoricalProfileUnavailable { contract_id: String },
    HistoricalShape(String),
    ExpectedAuthor(String),
    NonCanonicalExpectedAuthor,
    ExpectedEventId(String),
    NonCanonicalExpectedEventId,
    EventIdMismatch { declared: String, computed: String },
    PlanDigest(PlanDigestError),
    PlanDigestMismatch { declared: String, computed: String },
}

impl fmt::Display for PlanDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawJsonTooLarge { max, actual } => {
                write!(formatter, "plan wire is {actual} bytes; max is {max}")
            }
            Self::Json(error) => write!(formatter, "plan wire JSON is invalid: {error}"),
            Self::UnsupportedSchemaVersion { actual } => {
                write!(formatter, "unsupported plan wire schema version {actual}")
            }
            Self::ContractIdentity(error) => write!(formatter, "invalid contract key: {error}"),
            Self::ContractKindMismatch { expected, actual } => write!(
                formatter,
                "plan kind {actual} does not match historical contract kind {expected}"
            ),
            Self::HistoricalProfileUnavailable { contract_id } => write!(
                formatter,
                "historical authoring profile unavailable for `{contract_id}`"
            ),
            Self::HistoricalShape(error) => {
                write!(formatter, "historical plan shape is invalid: {error}")
            }
            Self::ExpectedAuthor(error) => write!(formatter, "invalid expected author: {error}"),
            Self::NonCanonicalExpectedAuthor => {
                formatter.write_str("expected author is not canonical lowercase hexadecimal")
            }
            Self::ExpectedEventId(error) => write!(formatter, "invalid expected event ID: {error}"),
            Self::NonCanonicalExpectedEventId => {
                formatter.write_str("expected event ID is not canonical lowercase hexadecimal")
            }
            Self::EventIdMismatch { declared, computed } => write!(
                formatter,
                "plan event ID mismatch: declared {declared}, computed {computed}"
            ),
            Self::PlanDigest(error) => write!(formatter, "invalid plan digest: {error}"),
            Self::PlanDigestMismatch { declared, computed } => write!(
                formatter,
                "plan digest mismatch: declared {declared}, computed {computed}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PlanDecodeError {}

#[cfg(feature = "json")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlanWireV1 {
    schema_version: u32,
    contract_registry_version: u32,
    contract_id: String,
    expected_author: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    expected_event_id: String,
    plan_digest: String,
}

#[cfg(feature = "serde")]
impl serde::Serialize for PlanWireV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("PlanWireV1", 10)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field(
            "contract_registry_version",
            &self.contract.registry_version().get(),
        )?;
        state.serialize_field("contract_id", self.contract.contract_id().as_str())?;
        state.serialize_field("expected_author", &self.expected_author.to_hex())?;
        state.serialize_field("created_at", &self.created_at)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("tags", &self.tags)?;
        state.serialize_field("content", &self.content)?;
        state.serialize_field("expected_event_id", &self.expected_event_id.to_hex())?;
        state.serialize_field("plan_digest", &self.plan_digest.to_hex())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::{GenericEventDraft, envelope::kind::KIND_GEOCHAT};
    use serde_json::{Value, json};

    const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn plan() -> AuthoredEventPlan {
        AuthoredEventPlan::from_generic(
            GenericEventDraft::new(
                "radroots.social.geochat.v1",
                KIND_GEOCHAT,
                1_700_000_000,
                vec![vec!["g".to_owned(), "u4pru".to_owned()]],
                "hello 🍓",
                ALICE,
            )
            .expect("generic draft"),
        )
        .expect("authored plan")
    }

    fn wire_json() -> Vec<u8> {
        PlanWireV1::from_plan(&plan()).to_json().expect("wire JSON")
    }

    fn mutate(mutator: impl FnOnce(&mut Value)) -> Vec<u8> {
        let mut value = serde_json::from_slice::<Value>(&wire_json()).expect("wire value");
        mutator(&mut value);
        serde_json::to_vec(&value).expect("mutated wire")
    }

    #[test]
    fn valid_plan_round_trips_with_exact_ordered_storage_wire() {
        let plan = plan();
        let wire = PlanWireV1::from_plan(&plan);
        let json = wire.to_json().expect("wire JSON");
        let expected = format!(
            concat!(
                "{{\"schema_version\":1,",
                "\"contract_registry_version\":7,",
                "\"contract_id\":\"radroots.social.geochat.v1\",",
                "\"expected_author\":\"{}\",",
                "\"created_at\":1700000000,",
                "\"kind\":20000,",
                "\"tags\":[[\"g\",\"u4pru\"]],",
                "\"content\":\"hello 🍓\",",
                "\"expected_event_id\":\"{}\",",
                "\"plan_digest\":\"{}\"}}"
            ),
            ALICE,
            plan.expected_event_id().to_hex(),
            plan.digest().to_hex(),
        );
        assert_eq!(String::from_utf8(json.clone()).expect("UTF-8"), expected);

        let integrity = PlanWireV1::from_json(&json).expect("historically valid plan");
        assert_eq!(integrity.plan(), &plan);
        assert_eq!(
            integrity.registry_relation(RegistryVersion::CURRENT),
            PlanRegistryRelation::Current
        );
        assert_eq!(
            integrity.registry_relation(RegistryVersion::new(8).expect("simulated advance")),
            PlanRegistryRelation::Historical
        );
        assert_eq!(PlanWireV1::from_plan(integrity.plan()), wire);
    }

    #[test]
    fn unknown_duplicate_oversize_and_unsupported_versions_fail_closed() {
        let unknown = mutate(|value| value["unknown"] = json!(true));
        assert!(matches!(
            PlanWireV1::from_json(&unknown),
            Err(PlanDecodeError::Json(_))
        ));

        let json = String::from_utf8(wire_json()).expect("UTF-8");
        let duplicate = json.replacen(
            "{\"schema_version\":1,",
            "{\"schema_version\":1,\"schema_version\":1,",
            1,
        );
        assert!(matches!(
            PlanWireV1::from_json(duplicate.as_bytes()),
            Err(PlanDecodeError::Json(_))
        ));
        assert_eq!(
            PlanWireV1::from_json(&vec![b' '; PLAN_WIRE_MAX_BYTES + 1]),
            Err(PlanDecodeError::RawJsonTooLarge {
                max: PLAN_WIRE_MAX_BYTES,
                actual: PLAN_WIRE_MAX_BYTES + 1,
            })
        );

        let schema = mutate(|value| value["schema_version"] = json!(2));
        assert_eq!(
            PlanWireV1::from_json(&schema),
            Err(PlanDecodeError::UnsupportedSchemaVersion { actual: 2 })
        );
        let registry = mutate(|value| value["contract_registry_version"] = json!(8));
        assert!(matches!(
            PlanWireV1::from_json(&registry),
            Err(PlanDecodeError::ContractIdentity(
                ContractIdentityError::UnsupportedRegistryVersion { actual: 8 }
            ))
        ));
    }

    #[test]
    fn stale_or_tampered_commitments_and_noncanonical_identifiers_fail_closed() {
        let content = mutate(|value| value["content"] = json!("tampered"));
        assert!(matches!(
            PlanWireV1::from_json(&content),
            Err(PlanDecodeError::EventIdMismatch { .. })
        ));
        let id = mutate(|value| {
            value["expected_event_id"] =
                json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        });
        assert!(matches!(
            PlanWireV1::from_json(&id),
            Err(PlanDecodeError::EventIdMismatch { .. })
        ));
        let digest = mutate(|value| {
            value["plan_digest"] =
                json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        });
        assert!(matches!(
            PlanWireV1::from_json(&digest),
            Err(PlanDecodeError::PlanDigestMismatch { .. })
        ));
        let uppercase_digest = mutate(|value| {
            value["plan_digest"] = json!(plan().digest().to_hex().to_uppercase());
        });
        assert_eq!(
            PlanWireV1::from_json(&uppercase_digest),
            Err(PlanDecodeError::PlanDigest(PlanDigestError))
        );
        let kind = mutate(|value| value["kind"] = json!(1));
        assert_eq!(
            PlanWireV1::from_json(&kind),
            Err(PlanDecodeError::ContractKindMismatch {
                expected: KIND_GEOCHAT,
                actual: 1,
            })
        );
    }

    #[test]
    fn typed_contract_rejects_a_shape_from_the_wrong_historical_profile() {
        let typed = mutate(|value| {
            value["contract_id"] = json!("radroots.social.update.v1");
            value["kind"] = json!(1);
            value["tags"] = json!([["g", "u4pru"]]);
            value["content"] = json!("hello");
        });
        assert_eq!(
            PlanWireV1::from_json(&typed),
            Err(PlanDecodeError::HistoricalShape(
                "update_tags_forbidden".to_owned()
            ))
        );
    }

    #[test]
    fn adversarial_truncation_and_single_byte_mutation_never_panic() {
        let wire = wire_json();
        for end in 0..wire.len() {
            let _ = PlanWireV1::from_json(&wire[..end]);
        }
        for index in 0..wire.len() {
            let mut mutated = wire.clone();
            mutated[index] = b'?';
            let _ = PlanWireV1::from_json(&mutated);
        }
    }
}

#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};

use core::fmt;

use crate::ids::{
    RadrootsAddressableCoordinate, RadrootsDTag, RadrootsEventId, RadrootsIdParseError,
    RadrootsInventoryBinId, RadrootsPublicKey, RadrootsTradeCandidateId, RadrootsTradeId,
    RadrootsTradeMutationId,
};
use crate::kinds::{
    KIND_TRADE_CANCELLATION, KIND_TRADE_DECISION, KIND_TRADE_PROPOSAL,
    KIND_TRADE_REVISION_DECISION, KIND_TRADE_REVISION_PROPOSAL,
};
#[cfg(feature = "serde")]
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

pub const RADROOTS_TRADE_SCHEMA_VERSION: u16 = 1;
pub const RADROOTS_TRADE_MUTATION_DOMAIN: &[u8] = b"radroots:trade-mutation:v1\0";
pub const RADROOTS_TRADE_CANDIDATE_DOMAIN: &[u8] = b"radroots:trade-candidate:v1\0";
pub const RADROOTS_TRADE_PROPOSAL_CONTRACT_ID: &str = "radroots.trade.proposal.v1";
pub const RADROOTS_TRADE_DECISION_CONTRACT_ID: &str = "radroots.trade.decision.v1";
pub const RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID: &str =
    "radroots.trade.revision_proposal.v1";
pub const RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID: &str =
    "radroots.trade.revision_decision.v1";
pub const RADROOTS_TRADE_CANCELLATION_CONTRACT_ID: &str = "radroots.trade.cancellation.v1";
pub const RADROOTS_TRADE_SELLER_RESERVATION_ASSERTION_CONTRACT_ID: &str =
    "radroots.trade.seller_reservation_assertion.v1";
pub const RADROOTS_TRADE_MUTATION_CONTRACT_IDS: [&str; 5] = [
    RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
    RADROOTS_TRADE_DECISION_CONTRACT_ID,
    RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID,
    RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID,
    RADROOTS_TRADE_CANCELLATION_CONTRACT_ID,
];
pub const RADROOTS_TRADE_MAX_PUBLIC_CONTENT_BYTES: usize = 64 * 1024;
pub const RADROOTS_TRADE_MAX_ACTIVE_LINES: usize = 64;
pub const RADROOTS_TRADE_MAX_ADJUSTMENTS: usize = 128;
pub const RADROOTS_TRADE_MAX_PARENT_MUTATIONS: usize = 4;
pub const RADROOTS_TRADE_MAX_PRIVATE_ARTIFACT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeMutationKindV1 {
    Proposal,
    Decision,
    RevisionProposal,
    RevisionDecision,
    Cancellation,
}

impl RadrootsTradeMutationKindV1 {
    pub const fn contract_id(self) -> &'static str {
        match self {
            Self::Proposal => RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
            Self::Decision => RADROOTS_TRADE_DECISION_CONTRACT_ID,
            Self::RevisionProposal => RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID,
            Self::RevisionDecision => RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID,
            Self::Cancellation => RADROOTS_TRADE_CANCELLATION_CONTRACT_ID,
        }
    }

    pub const fn nostr_kind(self) -> u32 {
        match self {
            Self::Proposal => KIND_TRADE_PROPOSAL,
            Self::Decision => KIND_TRADE_DECISION,
            Self::RevisionProposal => KIND_TRADE_REVISION_PROPOSAL,
            Self::RevisionDecision => KIND_TRADE_REVISION_DECISION,
            Self::Cancellation => KIND_TRADE_CANCELLATION,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsTradeMutationEnvelopeV1 {
    pub mutation_id: Option<RadrootsTradeMutationId>,
    pub contract_id: String,
    pub schema_version: u16,
    pub trade_id: RadrootsTradeId,
    pub root_mutation_id: Option<RadrootsTradeMutationId>,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub farm_id: RadrootsDTag,
    pub parent_mutation_ids: Vec<RadrootsTradeMutationId>,
    pub author_pubkey: RadrootsPublicKey,
    pub counterparty_pubkey: RadrootsPublicKey,
    pub authored_at_unix_s: u64,
    pub body: RadrootsTradeMutationBodyV1,
}

impl RadrootsTradeMutationEnvelopeV1 {
    pub fn mutation_kind(&self) -> RadrootsTradeMutationKindV1 {
        self.body.mutation_kind()
    }

    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        if self.schema_version != RADROOTS_TRADE_SCHEMA_VERSION {
            return Err(RadrootsTradeProtocolError::InvalidSchemaVersion {
                expected: RADROOTS_TRADE_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        let expected = self.mutation_kind().contract_id();
        if self.contract_id != expected {
            return Err(RadrootsTradeProtocolError::ContractMismatch {
                expected,
                actual: self.contract_id.clone(),
            });
        }
        validate_parent_mutation_ids(self.mutation_id.as_ref(), &self.parent_mutation_ids)?;
        match self.mutation_kind() {
            RadrootsTradeMutationKindV1::Proposal => {
                if self.root_mutation_id.is_some() || !self.parent_mutation_ids.is_empty() {
                    return Err(RadrootsTradeProtocolError::InvalidInitialParents);
                }
            }
            _ => {
                if self.root_mutation_id.is_none() || self.parent_mutation_ids.is_empty() {
                    return Err(RadrootsTradeProtocolError::MissingParentMutation);
                }
            }
        }
        self.body.validate()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "mutation_type")
)]
pub enum RadrootsTradeMutationBodyV1 {
    Proposal {
        candidate: RadrootsTradeCandidateTermsV1,
    },
    Decision {
        proposal_mutation_id: RadrootsTradeMutationId,
        candidate_id: RadrootsTradeCandidateId,
        decision: RadrootsTradeDecisionV1,
    },
    RevisionProposal {
        candidate: RadrootsTradeCandidateTermsV1,
    },
    RevisionDecision {
        proposal_mutation_id: RadrootsTradeMutationId,
        candidate_id: RadrootsTradeCandidateId,
        decision: RadrootsTradeDecisionV1,
    },
    Cancellation {
        target_candidate_id: Option<RadrootsTradeCandidateId>,
        target_claim_mutation_id: Option<RadrootsTradeMutationId>,
        reason: String,
    },
}

impl RadrootsTradeMutationBodyV1 {
    pub const fn mutation_kind(&self) -> RadrootsTradeMutationKindV1 {
        match self {
            Self::Proposal { .. } => RadrootsTradeMutationKindV1::Proposal,
            Self::Decision { .. } => RadrootsTradeMutationKindV1::Decision,
            Self::RevisionProposal { .. } => RadrootsTradeMutationKindV1::RevisionProposal,
            Self::RevisionDecision { .. } => RadrootsTradeMutationKindV1::RevisionDecision,
            Self::Cancellation { .. } => RadrootsTradeMutationKindV1::Cancellation,
        }
    }

    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        match self {
            Self::Proposal { candidate } | Self::RevisionProposal { candidate } => {
                candidate.validate()
            }
            Self::Decision {
                proposal_mutation_id: _,
                candidate_id: _,
                decision,
            }
            | Self::RevisionDecision {
                proposal_mutation_id: _,
                candidate_id: _,
                decision,
            } => decision.validate(),
            Self::Cancellation {
                target_candidate_id,
                target_claim_mutation_id,
                reason,
            } => {
                if target_candidate_id.is_none() && target_claim_mutation_id.is_none() {
                    return Err(RadrootsTradeProtocolError::MissingCancellationTarget);
                }
                validate_non_empty(reason, "reason")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsTradeCandidateTermsV1 {
    pub candidate_id: Option<RadrootsTradeCandidateId>,
    pub schema_version: u16,
    pub base_candidate_id: Option<RadrootsTradeCandidateId>,
    pub supersession_intent: Option<String>,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub farm_id: RadrootsDTag,
    pub lines: Vec<RadrootsTradeCandidateLineV1>,
    pub line_tombstones: Vec<RadrootsTradeLineTombstoneV1>,
    pub economics: RadrootsTradeEconomicsProfileV1,
    pub fulfillment: RadrootsFulfillmentProfileV1,
    pub cancellation: RadrootsTradeCancellationProfileV1,
    pub private_terms: Option<RadrootsTradePrivateTermsRefV1>,
    pub proposal_expires_at_unix_s: u64,
}

impl RadrootsTradeCandidateTermsV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        if self.schema_version != RADROOTS_TRADE_SCHEMA_VERSION {
            return Err(RadrootsTradeProtocolError::InvalidSchemaVersion {
                expected: RADROOTS_TRADE_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if self.lines.is_empty() {
            return Err(RadrootsTradeProtocolError::MissingLines);
        }
        if self.lines.len() > RADROOTS_TRADE_MAX_ACTIVE_LINES {
            return Err(RadrootsTradeProtocolError::TooManyLines {
                max: RADROOTS_TRADE_MAX_ACTIVE_LINES,
                actual: self.lines.len(),
            });
        }
        validate_sorted_unique_by(&self.lines, |line| line.line_id.as_str(), "lines")?;
        validate_sorted_unique_by(
            &self.line_tombstones,
            |line| line.line_id.as_str(),
            "line_tombstones",
        )?;
        for line in &self.lines {
            line.validate()?;
        }
        for tombstone in &self.line_tombstones {
            tombstone.validate()?;
        }
        self.economics.validate()?;
        self.fulfillment.validate()?;
        self.cancellation.validate()?;
        if let Some(private_terms) = &self.private_terms {
            private_terms.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsTradeCandidateLineV1 {
    pub line_id: RadrootsDTag,
    pub listing_addr: RadrootsAddressableCoordinate,
    pub listing_event_id: RadrootsEventId,
    pub listing_snapshot_sha256: String,
    pub product_id: String,
    pub option_id: Option<String>,
    pub bin_id: RadrootsInventoryBinId,
    pub quantity_mantissa: String,
    pub quantity_scale: u8,
    pub unit_code: String,
    pub unit_profile: String,
    pub unit_price_mantissa: String,
    pub currency_code: String,
    pub line_subtotal_mantissa: String,
    pub replaces_line_id: Option<RadrootsDTag>,
}

impl RadrootsTradeCandidateLineV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        validate_sha256_hex(&self.listing_snapshot_sha256, "listing_snapshot_sha256")?;
        validate_non_empty(&self.product_id, "product_id")?;
        if let Some(option_id) = &self.option_id {
            validate_non_empty(option_id, "option_id")?;
        }
        validate_decimal_string(&self.quantity_mantissa, "quantity_mantissa")?;
        validate_non_empty(&self.unit_code, "unit_code")?;
        validate_non_empty(&self.unit_profile, "unit_profile")?;
        validate_decimal_string(&self.unit_price_mantissa, "unit_price_mantissa")?;
        validate_non_empty(&self.currency_code, "currency_code")?;
        validate_decimal_string(&self.line_subtotal_mantissa, "line_subtotal_mantissa")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsTradeLineTombstoneV1 {
    pub line_id: RadrootsDTag,
    pub reason: String,
}

impl RadrootsTradeLineTombstoneV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        validate_non_empty(&self.reason, "line_tombstone.reason")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsTradeEconomicsProfileV1 {
    pub profile_id: String,
    pub currency_code: String,
    pub currency_exponent: u8,
    pub rounding_profile: String,
    pub subtotal_mantissa: String,
    pub discount_total_mantissa: String,
    pub adjustment_total_mantissa: String,
    pub total_mantissa: String,
    pub adjustments: Vec<RadrootsTradeEconomicAdjustmentV1>,
}

impl RadrootsTradeEconomicsProfileV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        validate_non_empty(&self.profile_id, "economics.profile_id")?;
        validate_non_empty(&self.currency_code, "economics.currency_code")?;
        validate_non_empty(&self.rounding_profile, "economics.rounding_profile")?;
        validate_decimal_string(&self.subtotal_mantissa, "economics.subtotal_mantissa")?;
        validate_decimal_string(
            &self.discount_total_mantissa,
            "economics.discount_total_mantissa",
        )?;
        validate_decimal_string(
            &self.adjustment_total_mantissa,
            "economics.adjustment_total_mantissa",
        )?;
        validate_decimal_string(&self.total_mantissa, "economics.total_mantissa")?;
        if self.adjustments.len() > RADROOTS_TRADE_MAX_ADJUSTMENTS {
            return Err(RadrootsTradeProtocolError::TooManyAdjustments {
                max: RADROOTS_TRADE_MAX_ADJUSTMENTS,
                actual: self.adjustments.len(),
            });
        }
        validate_sorted_unique_by(
            &self.adjustments,
            |line| line.adjustment_id.as_str(),
            "adjustments",
        )?;
        for adjustment in &self.adjustments {
            adjustment.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsTradeEconomicAdjustmentV1 {
    pub adjustment_id: RadrootsDTag,
    pub actor: String,
    pub effect: String,
    pub amount_mantissa: String,
    pub reason: String,
}

impl RadrootsTradeEconomicAdjustmentV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        validate_non_empty(&self.actor, "adjustment.actor")?;
        validate_non_empty(&self.effect, "adjustment.effect")?;
        validate_decimal_string(&self.amount_mantissa, "adjustment.amount_mantissa")?;
        validate_non_empty(&self.reason, "adjustment.reason")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsFulfillmentProfileV1 {
    pub profile_id: String,
    pub method: String,
    pub starts_at_unix_s: u64,
    pub ends_at_unix_s: u64,
    pub timezone: String,
    pub utc_offset_seconds: i32,
    pub fold: u8,
    pub location_class: String,
    pub requires_private_terms: bool,
}

impl RadrootsFulfillmentProfileV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        validate_non_empty(&self.profile_id, "fulfillment.profile_id")?;
        validate_non_empty(&self.method, "fulfillment.method")?;
        validate_non_empty(&self.timezone, "fulfillment.timezone")?;
        validate_non_empty(&self.location_class, "fulfillment.location_class")?;
        if self.ends_at_unix_s <= self.starts_at_unix_s {
            return Err(RadrootsTradeProtocolError::InvalidTimeRange);
        }
        if self.fold > 1 {
            return Err(RadrootsTradeProtocolError::InvalidField("fulfillment.fold"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsTradeCancellationProfileV1 {
    pub profile_id: String,
    pub buyer_pre_agreement: bool,
    pub post_agreement_cutoff_unix_s: Option<u64>,
}

impl RadrootsTradeCancellationProfileV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        validate_non_empty(&self.profile_id, "cancellation.profile_id")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsTradePrivateTermsRefV1 {
    pub artifact_id: String,
    pub schema_id: String,
    pub ciphertext_commitment: String,
    pub required_acknowledgement: bool,
}

impl RadrootsTradePrivateTermsRefV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        validate_non_empty(&self.artifact_id, "private_terms.artifact_id")?;
        validate_non_empty(&self.schema_id, "private_terms.schema_id")?;
        validate_sha256_hex(
            &self.ciphertext_commitment,
            "private_terms.ciphertext_commitment",
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsSellerReservationAssertionV1 {
    pub reservation_id: RadrootsDTag,
    pub inventory_authority_id: RadrootsPublicKey,
    pub inventory_epoch: u64,
    pub candidate_id: RadrootsTradeCandidateId,
    pub commitments: Vec<RadrootsSellerReservationLineV1>,
    pub reservation_expires_at_unix_s: u64,
    pub assertion_commitment: String,
}

impl RadrootsSellerReservationAssertionV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        if self.commitments.is_empty() {
            return Err(RadrootsTradeProtocolError::MissingReservationCommitments);
        }
        validate_sorted_unique_by(
            &self.commitments,
            |line| line.line_id.as_str(),
            "reservation.commitments",
        )?;
        for commitment in &self.commitments {
            commitment.validate()?;
        }
        validate_sha256_hex(
            &self.assertion_commitment,
            "reservation.assertion_commitment",
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RadrootsSellerReservationLineV1 {
    pub line_id: RadrootsDTag,
    pub bin_id: RadrootsInventoryBinId,
    pub quantity_mantissa: String,
    pub quantity_scale: u8,
    pub unit_code: String,
}

impl RadrootsSellerReservationLineV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        validate_decimal_string(
            &self.quantity_mantissa,
            "reservation.line.quantity_mantissa",
        )?;
        validate_non_empty(&self.unit_code, "reservation.line.unit_code")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "decision"))]
pub enum RadrootsTradeDecisionV1 {
    Accepted {
        reservation_assertion: Option<RadrootsSellerReservationAssertionV1>,
    },
    Declined {
        reason: String,
    },
}

impl RadrootsTradeDecisionV1 {
    pub fn validate(&self) -> Result<(), RadrootsTradeProtocolError> {
        match self {
            Self::Accepted {
                reservation_assertion,
            } => {
                if let Some(assertion) = reservation_assertion {
                    assertion.validate()?;
                }
                Ok(())
            }
            Self::Declined { reason } => validate_non_empty(reason, "decision.reason"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeCanonicalMutationV1 {
    pub mutation_id: RadrootsTradeMutationId,
    pub envelope: RadrootsTradeMutationEnvelopeV1,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeProtocolError {
    InvalidSchemaVersion {
        expected: u16,
        actual: u16,
    },
    ContractMismatch {
        expected: &'static str,
        actual: String,
    },
    InvalidInitialParents,
    MissingParentMutation,
    TooManyParents {
        max: usize,
        actual: usize,
    },
    UnsortedParents,
    DuplicateParent,
    SelfParent,
    MissingLines,
    TooManyLines {
        max: usize,
        actual: usize,
    },
    TooManyAdjustments {
        max: usize,
        actual: usize,
    },
    DuplicateKey(String),
    InvalidJson(String),
    NonCanonicalJson,
    UnsupportedNumber,
    ContentTooLarge {
        max: usize,
        actual: usize,
    },
    EmptyField(&'static str),
    InvalidField(&'static str),
    InvalidIdentifier {
        field: &'static str,
        error: RadrootsIdParseError,
    },
    InvalidTimeRange,
    MissingReservationCommitments,
    MissingCancellationTarget,
    CandidateIdMismatch {
        declared: String,
        computed: String,
    },
    MutationIdMismatch {
        declared: String,
        computed: String,
    },
}

impl fmt::Display for RadrootsTradeProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchemaVersion { expected, actual } => {
                write!(f, "trade schema version {actual} must be {expected}")
            }
            Self::ContractMismatch { expected, actual } => {
                write!(f, "trade contract {actual} must be {expected}")
            }
            Self::InvalidInitialParents => {
                write!(f, "initial trade proposal must not have parents")
            }
            Self::MissingParentMutation => write!(f, "trade mutation requires a parent mutation"),
            Self::TooManyParents { max, actual } => {
                write!(f, "trade mutation has {actual} parents; max is {max}")
            }
            Self::UnsortedParents => write!(f, "trade mutation parents must be sorted"),
            Self::DuplicateParent => write!(f, "trade mutation parents must be unique"),
            Self::SelfParent => write!(f, "trade mutation cannot reference itself as a parent"),
            Self::MissingLines => write!(f, "trade candidate requires at least one active line"),
            Self::TooManyLines { max, actual } => {
                write!(f, "trade candidate has {actual} active lines; max is {max}")
            }
            Self::TooManyAdjustments { max, actual } => {
                write!(f, "trade economics has {actual} adjustments; max is {max}")
            }
            Self::DuplicateKey(key) => write!(f, "trade canonical JSON has duplicate key {key}"),
            Self::InvalidJson(error) => write!(f, "trade canonical JSON is invalid: {error}"),
            Self::NonCanonicalJson => write!(f, "trade JSON is not canonical JCS"),
            Self::UnsupportedNumber => write!(f, "trade canonical JSON number is unsupported"),
            Self::ContentTooLarge { max, actual } => {
                write!(f, "trade content is {actual} bytes; max is {max}")
            }
            Self::EmptyField(field) => write!(f, "trade field {field} cannot be empty"),
            Self::InvalidField(field) => write!(f, "trade field {field} is invalid"),
            Self::InvalidIdentifier { field, error } => {
                write!(f, "trade field {field} is invalid: {error}")
            }
            Self::InvalidTimeRange => write!(f, "trade fulfillment time range is invalid"),
            Self::MissingReservationCommitments => {
                write!(f, "seller reservation assertion requires commitments")
            }
            Self::MissingCancellationTarget => write!(f, "trade cancellation requires a target"),
            Self::CandidateIdMismatch { declared, computed } => write!(
                f,
                "trade candidate id mismatch: declared {declared}, computed {computed}"
            ),
            Self::MutationIdMismatch { declared, computed } => write!(
                f,
                "trade mutation id mismatch: declared {declared}, computed {computed}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTradeProtocolError {}

#[cfg(feature = "serde")]
pub fn canonical_trade_candidate_id(
    candidate: &RadrootsTradeCandidateTermsV1,
) -> Result<RadrootsTradeCandidateId, RadrootsTradeProtocolError> {
    let mut value = serde_json::to_value(candidate)
        .map_err(|error| RadrootsTradeProtocolError::InvalidJson(error.to_string()))?;
    remove_object_field(&mut value, "candidate_id")?;
    let canonical = canonical_jcs_value(&value)?;
    digest_prefixed(RADROOTS_TRADE_CANDIDATE_DOMAIN, canonical.as_bytes())
        .parse()
        .map_err(|error| RadrootsTradeProtocolError::InvalidIdentifier {
            field: "candidate_id",
            error,
        })
}

#[cfg(feature = "serde")]
pub fn canonical_trade_mutation_id(
    envelope: &RadrootsTradeMutationEnvelopeV1,
) -> Result<RadrootsTradeMutationId, RadrootsTradeProtocolError> {
    let mut value = serde_json::to_value(envelope)
        .map_err(|error| RadrootsTradeProtocolError::InvalidJson(error.to_string()))?;
    remove_object_field(&mut value, "mutation_id")?;
    let canonical = canonical_jcs_value(&value)?;
    digest_prefixed(RADROOTS_TRADE_MUTATION_DOMAIN, canonical.as_bytes())
        .parse()
        .map_err(|error| RadrootsTradeProtocolError::InvalidIdentifier {
            field: "mutation_id",
            error,
        })
}

#[cfg(feature = "serde")]
pub fn canonical_trade_mutation_content(
    mut envelope: RadrootsTradeMutationEnvelopeV1,
) -> Result<RadrootsTradeCanonicalMutationV1, RadrootsTradeProtocolError> {
    finalize_body_candidate_ids(&mut envelope.body)?;
    envelope.validate()?;
    let mutation_id = canonical_trade_mutation_id(&envelope)?;
    envelope.mutation_id = Some(mutation_id.clone());
    let value = serde_json::to_value(&envelope)
        .map_err(|error| RadrootsTradeProtocolError::InvalidJson(error.to_string()))?;
    let content = canonical_jcs_value(&value)?;
    if content.len() > RADROOTS_TRADE_MAX_PUBLIC_CONTENT_BYTES {
        return Err(RadrootsTradeProtocolError::ContentTooLarge {
            max: RADROOTS_TRADE_MAX_PUBLIC_CONTENT_BYTES,
            actual: content.len(),
        });
    }
    Ok(RadrootsTradeCanonicalMutationV1 {
        mutation_id,
        envelope,
        content,
    })
}

#[cfg(feature = "serde")]
pub fn trade_mutation_from_canonical_content(
    content: &str,
) -> Result<RadrootsTradeMutationEnvelopeV1, RadrootsTradeProtocolError> {
    if content.len() > RADROOTS_TRADE_MAX_PUBLIC_CONTENT_BYTES {
        return Err(RadrootsTradeProtocolError::ContentTooLarge {
            max: RADROOTS_TRADE_MAX_PUBLIC_CONTENT_BYTES,
            actual: content.len(),
        });
    }
    let value = parse_json_without_duplicate_keys(content)?;
    if canonical_jcs_value(&value)? != content {
        return Err(RadrootsTradeProtocolError::NonCanonicalJson);
    }
    let envelope: RadrootsTradeMutationEnvelopeV1 = serde_json::from_value(value)
        .map_err(|error| RadrootsTradeProtocolError::InvalidJson(error.to_string()))?;
    envelope.validate()?;
    verify_candidate_ids(&envelope.body)?;
    if let Some(declared) = &envelope.mutation_id {
        let computed = canonical_trade_mutation_id(&envelope)?;
        if declared != &computed {
            return Err(RadrootsTradeProtocolError::MutationIdMismatch {
                declared: declared.to_string(),
                computed: computed.to_string(),
            });
        }
    }
    Ok(envelope)
}

#[cfg(feature = "serde")]
pub fn canonical_jcs_from_str(content: &str) -> Result<String, RadrootsTradeProtocolError> {
    let value = parse_json_without_duplicate_keys(content)?;
    canonical_jcs_value(&value)
}

#[cfg(feature = "serde")]
pub fn canonical_jcs_value(value: &Value) -> Result<String, RadrootsTradeProtocolError> {
    let mut output = String::new();
    write_canonical_jcs(value, &mut output)?;
    Ok(output)
}

#[cfg(feature = "serde")]
fn parse_json_without_duplicate_keys(content: &str) -> Result<Value, RadrootsTradeProtocolError> {
    let mut deserializer = serde_json::Deserializer::from_str(content);
    let value = NoDuplicateJsonValue::deserialize(&mut deserializer).map_err(map_json_error)?;
    deserializer.end().map_err(map_json_error)?;
    Ok(value.0)
}

#[cfg(feature = "serde")]
fn map_json_error(error: serde_json::Error) -> RadrootsTradeProtocolError {
    let message = error.to_string();
    if let Some(key) = message.strip_prefix("duplicate key: ") {
        let key = key.split(" at line ").next().unwrap_or(key).to_string();
        return RadrootsTradeProtocolError::DuplicateKey(key);
    }
    RadrootsTradeProtocolError::InvalidJson(message)
}

#[cfg(feature = "serde")]
fn write_canonical_jcs(
    value: &Value,
    output: &mut String,
) -> Result<(), RadrootsTradeProtocolError> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(number) => output.push_str(&canonical_number(number)?),
        Value::String(value) => {
            let encoded = serde_json::to_string(value)
                .map_err(|error| RadrootsTradeProtocolError::InvalidJson(error.to_string()))?;
            output.push_str(&encoded);
        }
        Value::Array(values) => {
            output.push('[');
            for (index, item) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical_jcs(item, output)?;
            }
            output.push(']');
        }
        Value::Object(map) => {
            output.push('{');
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                let encoded = serde_json::to_string(key.as_str())
                    .map_err(|error| RadrootsTradeProtocolError::InvalidJson(error.to_string()))?;
                output.push_str(&encoded);
                output.push(':');
                let value =
                    map.get(key.as_str())
                        .ok_or(RadrootsTradeProtocolError::InvalidJson(
                            "missing key".to_string(),
                        ))?;
                write_canonical_jcs(value, output)?;
            }
            output.push('}');
        }
    }
    Ok(())
}

fn canonical_number(number: &Number) -> Result<String, RadrootsTradeProtocolError> {
    if number.is_i64() || number.is_u64() {
        Ok(number.to_string())
    } else {
        Err(RadrootsTradeProtocolError::UnsupportedNumber)
    }
}

fn digest_prefixed(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(feature = "serde")]
fn remove_object_field(
    value: &mut Value,
    field: &'static str,
) -> Result<(), RadrootsTradeProtocolError> {
    let Value::Object(map) = value else {
        return Err(RadrootsTradeProtocolError::InvalidJson(
            "root must be object".to_string(),
        ));
    };
    map.remove(field);
    Ok(())
}

#[cfg(feature = "serde")]
fn finalize_body_candidate_ids(
    body: &mut RadrootsTradeMutationBodyV1,
) -> Result<(), RadrootsTradeProtocolError> {
    match body {
        RadrootsTradeMutationBodyV1::Proposal { candidate }
        | RadrootsTradeMutationBodyV1::RevisionProposal { candidate } => {
            let candidate_id = canonical_trade_candidate_id(candidate)?;
            candidate.candidate_id = Some(candidate_id);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(feature = "serde")]
fn verify_candidate_ids(
    body: &RadrootsTradeMutationBodyV1,
) -> Result<(), RadrootsTradeProtocolError> {
    match body {
        RadrootsTradeMutationBodyV1::Proposal { candidate }
        | RadrootsTradeMutationBodyV1::RevisionProposal { candidate } => {
            if let Some(declared) = &candidate.candidate_id {
                let computed = canonical_trade_candidate_id(candidate)?;
                if declared != &computed {
                    return Err(RadrootsTradeProtocolError::CandidateIdMismatch {
                        declared: declared.to_string(),
                        computed: computed.to_string(),
                    });
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_parent_mutation_ids(
    mutation_id: Option<&RadrootsTradeMutationId>,
    parents: &[RadrootsTradeMutationId],
) -> Result<(), RadrootsTradeProtocolError> {
    if parents.len() > RADROOTS_TRADE_MAX_PARENT_MUTATIONS {
        return Err(RadrootsTradeProtocolError::TooManyParents {
            max: RADROOTS_TRADE_MAX_PARENT_MUTATIONS,
            actual: parents.len(),
        });
    }
    let mut previous: Option<&str> = None;
    for parent in parents {
        if Some(parent) == mutation_id {
            return Err(RadrootsTradeProtocolError::SelfParent);
        }
        if let Some(previous) = previous {
            match previous.cmp(parent.as_str()) {
                core::cmp::Ordering::Greater => {
                    return Err(RadrootsTradeProtocolError::UnsortedParents);
                }
                core::cmp::Ordering::Equal => {
                    return Err(RadrootsTradeProtocolError::DuplicateParent);
                }
                core::cmp::Ordering::Less => {}
            }
        }
        previous = Some(parent.as_str());
    }
    Ok(())
}

fn validate_sorted_unique_by<T, F>(
    items: &[T],
    mut key: F,
    field: &'static str,
) -> Result<(), RadrootsTradeProtocolError>
where
    F: FnMut(&T) -> &str,
{
    let mut previous: Option<&str> = None;
    for item in items {
        let item_key = key(item);
        validate_non_empty(item_key, field)?;
        if let Some(previous) = previous {
            if previous >= item_key {
                return Err(RadrootsTradeProtocolError::InvalidField(field));
            }
        }
        previous = Some(item_key);
    }
    Ok(())
}

fn validate_non_empty(value: &str, field: &'static str) -> Result<(), RadrootsTradeProtocolError> {
    if value.trim().is_empty() {
        Err(RadrootsTradeProtocolError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn validate_decimal_string(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsTradeProtocolError> {
    validate_non_empty(value, field)?;
    let mut chars = value.chars();
    if matches!(chars.clone().next(), Some('-')) {
        chars.next();
    }
    let mut saw_digit = false;
    for character in chars {
        match character {
            '0'..='9' => saw_digit = true,
            _ => return Err(RadrootsTradeProtocolError::InvalidField(field)),
        }
    }
    if saw_digit {
        Ok(())
    } else {
        Err(RadrootsTradeProtocolError::InvalidField(field))
    }
}

fn validate_sha256_hex(value: &str, field: &'static str) -> Result<(), RadrootsTradeProtocolError> {
    RadrootsTradeMutationId::parse(value)
        .map(|_| ())
        .map_err(|error| RadrootsTradeProtocolError::InvalidIdentifier { field, error })
}

#[cfg(feature = "serde")]
struct NoDuplicateJsonValue(Value);

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for NoDuplicateJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(NoDuplicateJsonValueVisitor)
    }
}

#[cfg(feature = "serde")]
struct NoDuplicateJsonValueVisitor;

#[cfg(feature = "serde")]
impl<'de> Visitor<'de> for NoDuplicateJsonValueVisitor {
    type Value = NoDuplicateJsonValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Err(E::custom("unsupported number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(Value::String(value.to_string())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<NoDuplicateJsonValue>()? {
            values.push(value.0);
        }
        Ok(NoDuplicateJsonValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(A::Error::custom(format!("duplicate key: {key}")));
            }
            let value = map.next_value::<NoDuplicateJsonValue>()?;
            values.insert(key, value.0);
        }
        let mut object = Map::new();
        for (key, value) in values {
            object.insert(key, value);
        }
        Ok(NoDuplicateJsonValue(Value::Object(object)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn hex_32(character: char) -> String {
        core::iter::repeat_n(character, 32).collect()
    }

    fn pubkey(character: char) -> RadrootsPublicKey {
        RadrootsPublicKey::parse(hex_64(character)).unwrap()
    }

    fn event_id(character: char) -> RadrootsEventId {
        RadrootsEventId::parse(hex_64(character)).unwrap()
    }

    fn trade_id() -> RadrootsTradeId {
        RadrootsTradeId::parse(hex_32('1')).unwrap()
    }

    fn candidate() -> RadrootsTradeCandidateTermsV1 {
        RadrootsTradeCandidateTermsV1 {
            candidate_id: None,
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            base_candidate_id: None,
            supersession_intent: None,
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: RadrootsDTag::parse("farm-1").unwrap(),
            lines: vec![RadrootsTradeCandidateLineV1 {
                line_id: RadrootsDTag::parse("line-1").unwrap(),
                listing_addr: RadrootsAddressableCoordinate::parse(format!(
                    "30402:{}:listing-1",
                    hex_64('b')
                ))
                .unwrap(),
                listing_event_id: event_id('c'),
                listing_snapshot_sha256: hex_64('d'),
                product_id: "carrots".to_string(),
                option_id: None,
                bin_id: RadrootsInventoryBinId::parse("bin-1").unwrap(),
                quantity_mantissa: "2".to_string(),
                quantity_scale: 0,
                unit_code: "count".to_string(),
                unit_profile: "mvp-count".to_string(),
                unit_price_mantissa: "500".to_string(),
                currency_code: "USD".to_string(),
                line_subtotal_mantissa: "1000".to_string(),
                replaces_line_id: None,
            }],
            line_tombstones: Vec::new(),
            economics: RadrootsTradeEconomicsProfileV1 {
                profile_id: "mvp-fixed".to_string(),
                currency_code: "USD".to_string(),
                currency_exponent: 2,
                rounding_profile: "half-even".to_string(),
                subtotal_mantissa: "1000".to_string(),
                discount_total_mantissa: "0".to_string(),
                adjustment_total_mantissa: "0".to_string(),
                total_mantissa: "1000".to_string(),
                adjustments: Vec::new(),
            },
            fulfillment: RadrootsFulfillmentProfileV1 {
                profile_id: "market-pickup".to_string(),
                method: "pickup".to_string(),
                starts_at_unix_s: 1_800_000_000,
                ends_at_unix_s: 1_800_003_600,
                timezone: "America/New_York".to_string(),
                utc_offset_seconds: -18_000,
                fold: 0,
                location_class: "farmstand".to_string(),
                requires_private_terms: false,
            },
            cancellation: RadrootsTradeCancellationProfileV1 {
                profile_id: "buyer-pre-agreement".to_string(),
                buyer_pre_agreement: true,
                post_agreement_cutoff_unix_s: None,
            },
            private_terms: None,
            proposal_expires_at_unix_s: 1_799_999_000,
        }
    }

    fn proposal() -> RadrootsTradeMutationEnvelopeV1 {
        RadrootsTradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_PROPOSAL_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: None,
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: RadrootsDTag::parse("farm-1").unwrap(),
            parent_mutation_ids: Vec::new(),
            author_pubkey: pubkey('a'),
            counterparty_pubkey: pubkey('b'),
            authored_at_unix_s: 1_799_000_000,
            body: RadrootsTradeMutationBodyV1::Proposal {
                candidate: candidate(),
            },
        }
    }

    #[test]
    fn canonical_json_sorts_keys_and_rejects_duplicate_keys() {
        assert_eq!(
            canonical_jcs_from_str(r#"{"z":1,"a":["b",true,null]}"#).unwrap(),
            r#"{"a":["b",true,null],"z":1}"#
        );
        assert!(matches!(
            canonical_jcs_from_str(r#"{"a":1,"a":2}"#),
            Err(RadrootsTradeProtocolError::DuplicateKey(_))
        ));
    }

    #[test]
    fn canonical_trade_mutation_sets_candidate_and_mutation_ids() {
        let canonical = canonical_trade_mutation_content(proposal()).unwrap();
        assert!(canonical.envelope.mutation_id.is_some());
        assert_eq!(
            canonical.mutation_id,
            canonical.envelope.mutation_id.clone().unwrap()
        );
        let parsed = trade_mutation_from_canonical_content(&canonical.content).unwrap();
        assert_eq!(parsed.mutation_id, Some(canonical.mutation_id));
    }

    #[test]
    fn canonical_trade_mutation_rejects_parent_ordering() {
        let mut envelope = proposal();
        envelope.contract_id = RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID.to_string();
        envelope.root_mutation_id = Some(RadrootsTradeMutationId::parse(hex_64('a')).unwrap());
        envelope.parent_mutation_ids = vec![
            RadrootsTradeMutationId::parse(hex_64('b')).unwrap(),
            RadrootsTradeMutationId::parse(hex_64('a')).unwrap(),
        ];
        envelope.body = RadrootsTradeMutationBodyV1::RevisionProposal {
            candidate: candidate(),
        };
        assert!(matches!(
            canonical_trade_mutation_content(envelope),
            Err(RadrootsTradeProtocolError::UnsortedParents)
        ));
    }
}

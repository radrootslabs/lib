//! Validated, side-effect-free trade workflow plans.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec, vec::Vec};

use core::fmt;

use radroots_event::{
    id::{CandidateId, MutationId, TradeId},
    trade::{
        RADROOTS_TRADE_SCHEMA_VERSION, TradeMutationBodyV1, TradeMutationEnvelopeV1,
        TradeMutationKindV1,
    },
};

/// A host action required to execute a prepared trade plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum WorkflowAction {
    /// Authorize and sign the canonical mutation.
    Sign,
    /// Persist the signed mutation atomically.
    Persist,
    /// Deliver the signed event through an explicitly selected transport.
    Deliver,
    /// Verify required private terms before signing.
    VerifyPrivateTerms,
}

/// Private-term material a host must verify before executing a plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateTermsPlan {
    candidate_id: CandidateId,
    artifact_id: String,
    schema_id: String,
    ciphertext_commitment: String,
}

impl PrivateTermsPlan {
    /// Returns the candidate whose private terms must be verified.
    pub const fn candidate_id(&self) -> &CandidateId {
        &self.candidate_id
    }

    /// Returns the host-owned encrypted artifact identifier.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Returns the private-term schema identifier.
    pub fn schema_id(&self) -> &str {
        &self.schema_id
    }

    /// Returns the expected ciphertext commitment.
    pub fn ciphertext_commitment(&self) -> &str {
        &self.ciphertext_commitment
    }
}

/// A validated description of a trade operation and its required host actions.
///
/// Constructing a plan performs no signing, persistence, delivery, filesystem
/// access, scheduling, or private-artifact access.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "a workflow plan has no effect until a host explicitly executes its actions"]
pub struct WorkflowPlan {
    mutation: TradeMutationEnvelopeV1,
    mutation_id: MutationId,
    kind: TradeMutationKindV1,
    required_actions: Vec<WorkflowAction>,
    private_terms: Option<PrivateTermsPlan>,
}

impl WorkflowPlan {
    /// Validates a canonical trade mutation and prepares its host action plan.
    pub fn prepare(mutation: TradeMutationEnvelopeV1) -> Result<Self, Error> {
        if mutation.schema_version != RADROOTS_TRADE_SCHEMA_VERSION {
            return Err(Error::unsupported_schema(mutation.schema_version));
        }
        let mutation_id = mutation
            .mutation_id
            .ok_or_else(Error::missing_mutation_id)?;
        mutation.validate().map_err(Error::invalid_mutation)?;

        let kind = mutation.mutation_kind();
        let private_terms = private_terms_plan(&mutation.body)?;
        let mut required_actions = vec![
            WorkflowAction::Sign,
            WorkflowAction::Persist,
            WorkflowAction::Deliver,
        ];
        if private_terms.is_some() {
            required_actions.insert(0, WorkflowAction::VerifyPrivateTerms);
        }

        debug_assert_eq!(mutation.mutation_id, Some(mutation_id));
        Ok(Self {
            mutation,
            mutation_id,
            kind,
            required_actions,
            private_terms,
        })
    }

    /// Returns the operation kind represented by the plan.
    pub const fn kind(&self) -> TradeMutationKindV1 {
        self.kind
    }

    /// Returns the canonical mutation identifier.
    pub const fn mutation_id(&self) -> &MutationId {
        &self.mutation_id
    }

    /// Returns the canonical trade identifier.
    pub const fn trade_id(&self) -> &TradeId {
        &self.mutation.trade_id
    }

    /// Returns the validated canonical mutation without transferring ownership.
    pub const fn mutation(&self) -> &TradeMutationEnvelopeV1 {
        &self.mutation
    }

    /// Returns the ordered host actions required to execute the plan.
    pub fn required_actions(&self) -> &[WorkflowAction] {
        &self.required_actions
    }

    /// Returns required private-term verification material, when present.
    pub const fn private_terms(&self) -> Option<&PrivateTermsPlan> {
        self.private_terms.as_ref()
    }

    /// Consumes the plan and returns the validated canonical mutation.
    pub fn into_mutation(self) -> TradeMutationEnvelopeV1 {
        self.mutation
    }
}

/// Stable workflow-planning error classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The mutation uses an unsupported trade schema.
    UnsupportedSchema,
    /// The canonical mutation identifier is absent.
    MissingMutationId,
    /// The mutation violates its event-domain contract.
    InvalidMutation,
    /// Required private-term metadata is incomplete.
    InvalidPrivateTerms,
}

/// Workflow-plan validation failure with a redacted diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    source: Option<radroots_event::trade::TradeProtocolError>,
}

impl Error {
    fn unsupported_schema(found: u16) -> Self {
        Self {
            kind: ErrorKind::UnsupportedSchema,
            message: format!(
                "unsupported trade schema version {found}; expected {RADROOTS_TRADE_SCHEMA_VERSION}"
            ),
            source: None,
        }
    }

    fn missing_mutation_id() -> Self {
        Self {
            kind: ErrorKind::MissingMutationId,
            message: "canonical trade mutation identifier is missing".into(),
            source: None,
        }
    }

    fn invalid_mutation(source: radroots_event::trade::TradeProtocolError) -> Self {
        Self {
            kind: ErrorKind::InvalidMutation,
            message: format!("invalid canonical trade mutation: {source}"),
            source: Some(source),
        }
    }

    fn invalid_private_terms(message: &'static str) -> Self {
        Self {
            kind: ErrorKind::InvalidPrivateTerms,
            message: message.into(),
            source: None,
        }
    }

    /// Returns the stable error classification.
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the underlying event-domain validation failure, when present.
    pub const fn protocol_error(&self) -> Option<&radroots_event::trade::TradeProtocolError> {
        self.source.as_ref()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl core::error::Error for Error {
    #[cfg(feature = "std")]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn core::error::Error + 'static))
    }
}

fn private_terms_plan(body: &TradeMutationBodyV1) -> Result<Option<PrivateTermsPlan>, Error> {
    let candidate = match body {
        TradeMutationBodyV1::Proposal { candidate }
        | TradeMutationBodyV1::RevisionProposal { candidate } => candidate,
        TradeMutationBodyV1::Decision { .. }
        | TradeMutationBodyV1::RevisionDecision { .. }
        | TradeMutationBodyV1::Cancellation { .. } => return Ok(None),
    };
    let required =
        candidate.fulfillment.requires_private_terms || candidate.private_terms.is_some();
    if !required {
        return Ok(None);
    }
    let candidate_id = candidate.candidate_id.ok_or_else(|| {
        Error::invalid_private_terms("required private terms need a canonical candidate identifier")
    })?;
    let private_terms = candidate.private_terms.as_ref().ok_or_else(|| {
        Error::invalid_private_terms("required private terms need an encrypted artifact reference")
    })?;
    Ok(Some(PrivateTermsPlan {
        candidate_id,
        artifact_id: private_terms.artifact_id.clone(),
        schema_id: private_terms.schema_id.clone(),
        ciphertext_commitment: private_terms.ciphertext_commitment.clone(),
    }))
}

// Temporary migration reexports for the existing versioned reducer contract.
pub use crate::trade_contract_v1::{
    RADROOTS_TRADE_REDUCER_CONTRACT_ID, RADROOTS_TRADE_REDUCER_VERSION,
    RadrootsTradeAgreementClaimV1, RadrootsTradeAgreementStateV1, RadrootsTradeAttestationRecordV1,
    RadrootsTradeAttestationResultV1, RadrootsTradeAttestationStateV1,
    RadrootsTradeConflictStateV1, RadrootsTradeEvidenceStateV1, RadrootsTradeFulfillmentStateV1,
    RadrootsTradeMutationRecordV1, RadrootsTradeNegotiationStateV1, RadrootsTradePaymentStateV1,
    RadrootsTradePrivateTermsEvidenceV1, RadrootsTradePrivateTermsStateV1,
    RadrootsTradeProjectionV1, RadrootsTradeReducerIssueV1, RadrootsTradeReductionInputV1,
    reduce_trade_records,
};

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::{
        id::{ClassifiedListingAddress, DTag, EventId, InventoryBinId},
        trade::{
            FulfillmentProfileV1, RADROOTS_TRADE_CANCELLATION_CONTRACT_ID,
            RADROOTS_TRADE_DECISION_CONTRACT_ID, RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
            RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID,
            RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID, TradeCancellationProfileV1,
            TradeCandidateLineV1, TradeCandidateTermsV1, TradeDecisionV1,
            TradeEconomicAdjustmentV1, TradeEconomicsProfileV1, TradeLineTombstoneV1,
            TradePrivateTermsRefV1, canonical_trade_mutation_content,
        },
    };
    use radroots_identity::PublicKey;
    use radroots_test_fixtures::{FIXTURE_ALICE_PUBLIC_KEY_HEX, FIXTURE_BOB_PUBLIC_KEY_HEX};

    fn pubkey(value: &str) -> PublicKey {
        PublicKey::from_hex(value).expect("fixture public key")
    }

    fn mutation_id(marker: char) -> MutationId {
        MutationId::parse(core::iter::repeat_n(marker, 64).collect::<String>())
            .expect("fixture mutation id")
    }

    fn candidate(suffix: &str) -> TradeCandidateTermsV1 {
        TradeCandidateTermsV1 {
            candidate_id: None,
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            base_candidate_id: None,
            supersession_intent: None,
            buyer_pubkey: pubkey(FIXTURE_ALICE_PUBLIC_KEY_HEX),
            seller_pubkey: pubkey(FIXTURE_BOB_PUBLIC_KEY_HEX),
            farm_id: DTag::parse("farm-1").expect("farm id"),
            lines: vec![TradeCandidateLineV1 {
                line_id: DTag::parse(format!("line-{suffix}")).expect("line id"),
                listing_addr: ClassifiedListingAddress::parse(format!(
                    "30402:{FIXTURE_BOB_PUBLIC_KEY_HEX}:listing-{suffix}"
                ))
                .expect("listing address"),
                listing_event_id: EventId::parse("cc".repeat(32)).expect("event id"),
                listing_snapshot_sha256: "dd".repeat(32),
                product_id: format!("carrots-{suffix}"),
                option_id: None,
                bin_id: InventoryBinId::parse(format!("bin-{suffix}")).expect("bin id"),
                quantity_mantissa: "2".into(),
                quantity_scale: 0,
                unit_code: "count".into(),
                unit_profile: "mvp-count".into(),
                unit_price_mantissa: "500".into(),
                currency_code: "USD".into(),
                line_subtotal_mantissa: "1000".into(),
                replaces_line_id: None,
            }],
            line_tombstones: Vec::<TradeLineTombstoneV1>::new(),
            economics: TradeEconomicsProfileV1 {
                profile_id: "mvp-fixed".into(),
                currency_code: "USD".into(),
                currency_exponent: 2,
                rounding_profile: "half-even".into(),
                subtotal_mantissa: "1000".into(),
                discount_total_mantissa: "0".into(),
                adjustment_total_mantissa: "0".into(),
                total_mantissa: "1000".into(),
                adjustments: Vec::<TradeEconomicAdjustmentV1>::new(),
            },
            fulfillment: FulfillmentProfileV1 {
                profile_id: "market-pickup".into(),
                method: "pickup".into(),
                starts_at_unix_s: 1_800_000_000,
                ends_at_unix_s: 1_800_003_600,
                timezone: "America/New_York".into(),
                utc_offset_seconds: -18_000,
                fold: 0,
                location_class: "farmstand".into(),
                requires_private_terms: true,
            },
            cancellation: TradeCancellationProfileV1 {
                profile_id: "buyer-pre-agreement".into(),
                buyer_pre_agreement: true,
                post_agreement_cutoff_unix_s: None,
            },
            private_terms: Some(TradePrivateTermsRefV1 {
                artifact_id: "artifact-1".into(),
                schema_id: "radroots.private.fulfillment.v1".into(),
                ciphertext_commitment: "ee".repeat(32),
                required_acknowledgement: true,
            }),
            proposal_expires_at_unix_s: 1_799_999_000,
        }
    }

    fn envelope(contract_id: &str, body: TradeMutationBodyV1) -> TradeMutationEnvelopeV1 {
        let is_initial = body.mutation_kind() == TradeMutationKindV1::Proposal;
        canonical_trade_mutation_content(TradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: contract_id.into(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: TradeId::parse("11".repeat(16)).expect("trade id"),
            root_mutation_id: (!is_initial).then(|| mutation_id('1')),
            buyer_pubkey: pubkey(FIXTURE_ALICE_PUBLIC_KEY_HEX),
            seller_pubkey: pubkey(FIXTURE_BOB_PUBLIC_KEY_HEX),
            farm_id: DTag::parse("farm-1").expect("farm id"),
            parent_mutation_ids: if is_initial {
                Vec::new()
            } else {
                vec![mutation_id('1')]
            },
            author_pubkey: pubkey(FIXTURE_ALICE_PUBLIC_KEY_HEX),
            counterparty_pubkey: pubkey(FIXTURE_BOB_PUBLIC_KEY_HEX),
            authored_at_unix_s: 100,
            body,
        })
        .expect("canonical workflow mutation")
        .envelope
    }

    fn all_operation_mutations() -> Vec<TradeMutationEnvelopeV1> {
        let proposal = envelope(
            RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
            TradeMutationBodyV1::Proposal {
                candidate: candidate("1"),
            },
        );
        let proposal_id = proposal.mutation_id.expect("proposal id");
        let candidate_id = match &proposal.body {
            TradeMutationBodyV1::Proposal { candidate } => {
                candidate.candidate_id.expect("candidate id")
            }
            _ => unreachable!(),
        };
        vec![
            proposal,
            envelope(
                RADROOTS_TRADE_DECISION_CONTRACT_ID,
                TradeMutationBodyV1::Decision {
                    proposal_mutation_id: proposal_id,
                    candidate_id,
                    decision: TradeDecisionV1::Declined {
                        reason: "unavailable".into(),
                    },
                },
            ),
            envelope(
                RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID,
                TradeMutationBodyV1::RevisionProposal {
                    candidate: candidate("2"),
                },
            ),
            envelope(
                RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID,
                TradeMutationBodyV1::RevisionDecision {
                    proposal_mutation_id: mutation_id('2'),
                    candidate_id,
                    decision: TradeDecisionV1::Declined {
                        reason: "still unavailable".into(),
                    },
                },
            ),
            envelope(
                RADROOTS_TRADE_CANCELLATION_CONTRACT_ID,
                TradeMutationBodyV1::Cancellation {
                    target_candidate_id: Some(candidate_id),
                    target_claim_mutation_id: None,
                    reason: "cancelled".into(),
                },
            ),
        ]
    }

    #[test]
    fn plans_cover_every_trade_operation_without_host_side_effects() {
        let plans = all_operation_mutations()
            .into_iter()
            .map(WorkflowPlan::prepare)
            .collect::<Result<Vec<_>, _>>()
            .expect("valid workflow plans");

        assert_eq!(
            plans.iter().map(WorkflowPlan::kind).collect::<Vec<_>>(),
            vec![
                TradeMutationKindV1::Proposal,
                TradeMutationKindV1::Decision,
                TradeMutationKindV1::RevisionProposal,
                TradeMutationKindV1::RevisionDecision,
                TradeMutationKindV1::Cancellation,
            ]
        );
        assert_eq!(
            plans[0].required_actions(),
            [
                WorkflowAction::VerifyPrivateTerms,
                WorkflowAction::Sign,
                WorkflowAction::Persist,
                WorkflowAction::Deliver,
            ]
        );
        assert_eq!(
            plans[0].private_terms().unwrap().artifact_id(),
            "artifact-1"
        );
        for plan in &plans[1..] {
            let expected = if plan.kind() == TradeMutationKindV1::RevisionProposal {
                &[
                    WorkflowAction::VerifyPrivateTerms,
                    WorkflowAction::Sign,
                    WorkflowAction::Persist,
                    WorkflowAction::Deliver,
                ][..]
            } else {
                &[
                    WorkflowAction::Sign,
                    WorkflowAction::Persist,
                    WorkflowAction::Deliver,
                ][..]
            };
            assert_eq!(plan.required_actions(), expected);
        }
    }

    #[test]
    fn plan_validation_rejects_unsupported_missing_and_invalid_transitions() {
        let mut unsupported = all_operation_mutations().remove(0);
        unsupported.schema_version += 1;
        assert_eq!(
            WorkflowPlan::prepare(unsupported).unwrap_err().kind(),
            ErrorKind::UnsupportedSchema
        );

        let mut missing_id = all_operation_mutations().remove(0);
        missing_id.mutation_id = None;
        assert_eq!(
            WorkflowPlan::prepare(missing_id).unwrap_err().kind(),
            ErrorKind::MissingMutationId
        );

        let mut invalid = all_operation_mutations().remove(1);
        invalid.parent_mutation_ids.clear();
        assert_eq!(
            WorkflowPlan::prepare(invalid).unwrap_err().kind(),
            ErrorKind::InvalidMutation
        );
    }
}

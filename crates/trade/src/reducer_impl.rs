#![forbid(unsafe_code)]
//! Versioned reducer contract implementation behind the curated public modules.

#[cfg(all(not(feature = "std"), feature = "json"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec::Vec,
};

use radroots_event::{
    id::{CandidateId, DTag, EventId, MutationId, TradeId},
    trade::{
        RADROOTS_TRADE_SCHEMA_VERSION, SellerReservationAssertionV1, TradeCandidateTermsV1,
        TradeDecisionV1, TradeMutationBodyV1, TradeMutationEnvelopeV1,
    },
};
use radroots_identity::PublicKey;
#[cfg(feature = "json")]
use sha2::{Digest, Sha256};

pub const RADROOTS_TRADE_REDUCER_CONTRACT_ID: &str = "radroots.trade.reducer.v1";
pub const RADROOTS_TRADE_REDUCER_VERSION: u16 = 1;
#[cfg(feature = "json")]
const RADROOTS_TRADE_PROJECTION_DIGEST_DOMAIN: &[u8] = b"radroots:trade-projection:v1\0";

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeNegotiationStateV1 {
    #[default]
    None,
    Open,
    ClosedDeclined,
    ClosedExpired,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeAgreementStateV1 {
    #[default]
    None,
    Agreed,
    Contested,
    Cancelled,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeEvidenceStateV1 {
    Complete,
    #[default]
    Missing,
    QueryPartial,
    UnsupportedVersion,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeConflictStateV1 {
    #[default]
    None,
    ConcurrentCandidates,
    DoubleAcceptance,
    DecisionConflict,
    CancellationConflict,
    InvalidCausalChain,
    InventoryAuthorityConflict,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradePrivateTermsStateV1 {
    #[default]
    NotRequired,
    AvailableVerified,
    Missing,
    Undecryptable,
    CommitmentMismatch,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeAttestationStateV1 {
    #[default]
    None,
    PresentValid,
    PresentInvalid,
    Conflicting,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeFulfillmentStateV1 {
    #[default]
    NotStarted,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradePaymentStateV1 {
    #[default]
    NotTracked,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeReductionInputV1 {
    trade_id: TradeId,
    mutations: Vec<RadrootsTradeMutationRecordV1>,
    private_terms: Vec<RadrootsTradePrivateTermsEvidenceV1>,
    attestations: Vec<RadrootsTradeAttestationRecordV1>,
    evidence_state: RadrootsTradeEvidenceStateV1,
    observed_at_unix_s: Option<u64>,
}

impl RadrootsTradeReductionInputV1 {
    pub fn new(trade_id: TradeId) -> Self {
        Self {
            trade_id,
            mutations: Vec::new(),
            private_terms: Vec::new(),
            attestations: Vec::new(),
            evidence_state: RadrootsTradeEvidenceStateV1::Complete,
            observed_at_unix_s: None,
        }
    }

    #[must_use]
    pub fn with_mutations(mut self, mutations: Vec<RadrootsTradeMutationRecordV1>) -> Self {
        self.mutations = mutations;
        self
    }

    #[must_use]
    pub fn with_private_terms(
        mut self,
        private_terms: Vec<RadrootsTradePrivateTermsEvidenceV1>,
    ) -> Self {
        self.private_terms = private_terms;
        self
    }

    #[must_use]
    pub fn with_attestations(
        mut self,
        attestations: Vec<RadrootsTradeAttestationRecordV1>,
    ) -> Self {
        self.attestations = attestations;
        self
    }

    #[must_use]
    pub fn with_evidence_state(mut self, evidence_state: RadrootsTradeEvidenceStateV1) -> Self {
        self.evidence_state = evidence_state;
        self
    }

    #[must_use]
    pub fn with_observed_at_unix_s(mut self, observed_at_unix_s: Option<u64>) -> Self {
        self.observed_at_unix_s = observed_at_unix_s;
        self
    }

    pub const fn trade_id(&self) -> &TradeId {
        &self.trade_id
    }

    pub fn mutations(&self) -> &[RadrootsTradeMutationRecordV1] {
        &self.mutations
    }

    pub fn private_terms(&self) -> &[RadrootsTradePrivateTermsEvidenceV1] {
        &self.private_terms
    }

    pub fn attestations(&self) -> &[RadrootsTradeAttestationRecordV1] {
        &self.attestations
    }

    pub const fn evidence_state(&self) -> RadrootsTradeEvidenceStateV1 {
        self.evidence_state
    }

    pub const fn observed_at_unix_s(&self) -> Option<u64> {
        self.observed_at_unix_s
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeMutationRecordV1 {
    transport_event_id: Option<EventId>,
    mutation: TradeMutationEnvelopeV1,
}

impl RadrootsTradeMutationRecordV1 {
    pub const fn new(
        transport_event_id: Option<EventId>,
        mutation: TradeMutationEnvelopeV1,
    ) -> Self {
        Self {
            transport_event_id,
            mutation,
        }
    }

    pub const fn transport_event_id(&self) -> Option<&EventId> {
        self.transport_event_id.as_ref()
    }

    pub const fn mutation(&self) -> &TradeMutationEnvelopeV1 {
        &self.mutation
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradePrivateTermsEvidenceV1 {
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    candidate_id: CandidateId,
    state: RadrootsTradePrivateTermsStateV1,
}

impl RadrootsTradePrivateTermsEvidenceV1 {
    pub const fn new(candidate_id: CandidateId, state: RadrootsTradePrivateTermsStateV1) -> Self {
        Self {
            candidate_id,
            state,
        }
    }

    pub const fn candidate_id(&self) -> &CandidateId {
        &self.candidate_id
    }

    pub const fn state(&self) -> RadrootsTradePrivateTermsStateV1 {
        self.state
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeAttestationResultV1 {
    Valid,
    Invalid,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RadrootsTradeAttestationRecordV1 {
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    event_id: EventId,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    claim_mutation_id: MutationId,
    result: RadrootsTradeAttestationResultV1,
}

impl RadrootsTradeAttestationRecordV1 {
    pub const fn new(
        event_id: EventId,
        claim_mutation_id: MutationId,
        result: RadrootsTradeAttestationResultV1,
    ) -> Self {
        Self {
            event_id,
            claim_mutation_id,
            result,
        }
    }

    pub const fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub const fn claim_mutation_id(&self) -> &MutationId {
        &self.claim_mutation_id
    }

    pub const fn result(&self) -> RadrootsTradeAttestationResultV1 {
        self.result
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeProjectionV1 {
    reducer_contract_id: String,
    reducer_version: u16,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    trade_id: TradeId,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    root_mutation_id: Option<MutationId>,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    buyer_pubkey: Option<PublicKey>,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    seller_pubkey: Option<PublicKey>,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    farm_id: Option<DTag>,
    negotiation_state: RadrootsTradeNegotiationStateV1,
    agreement_state: RadrootsTradeAgreementStateV1,
    evidence_state: RadrootsTradeEvidenceStateV1,
    conflict_state: RadrootsTradeConflictStateV1,
    private_terms_state: RadrootsTradePrivateTermsStateV1,
    attestation_state: RadrootsTradeAttestationStateV1,
    fulfillment_state: RadrootsTradeFulfillmentStateV1,
    payment_state: RadrootsTradePaymentStateV1,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Array<string>")))]
    candidate_heads: Vec<MutationId>,
    agreement_claims: Vec<RadrootsTradeAgreementClaimV1>,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Array<string>")))]
    active_agreement_claim_ids: Vec<MutationId>,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Array<string>")))]
    contested_claim_ids: Vec<MutationId>,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Array<string>")))]
    cancelled_claim_ids: Vec<MutationId>,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Array<string>")))]
    declined_candidate_ids: Vec<CandidateId>,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Array<string>")))]
    missing_parent_ids: Vec<MutationId>,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Array<string>")))]
    missing_proposal_ids: Vec<MutationId>,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Array<string>")))]
    unsupported_mutation_ids: Vec<MutationId>,
    issues: Vec<RadrootsTradeReducerIssueV1>,
    attestations: Vec<RadrootsTradeAttestationRecordV1>,
    projection_digest: String,
}

impl RadrootsTradeProjectionV1 {
    fn empty(trade_id: TradeId) -> Self {
        Self {
            reducer_contract_id: RADROOTS_TRADE_REDUCER_CONTRACT_ID.to_string(),
            reducer_version: RADROOTS_TRADE_REDUCER_VERSION,
            trade_id,
            root_mutation_id: None,
            buyer_pubkey: None,
            seller_pubkey: None,
            farm_id: None,
            negotiation_state: RadrootsTradeNegotiationStateV1::None,
            agreement_state: RadrootsTradeAgreementStateV1::None,
            evidence_state: RadrootsTradeEvidenceStateV1::Missing,
            conflict_state: RadrootsTradeConflictStateV1::None,
            private_terms_state: RadrootsTradePrivateTermsStateV1::NotRequired,
            attestation_state: RadrootsTradeAttestationStateV1::None,
            fulfillment_state: RadrootsTradeFulfillmentStateV1::NotStarted,
            payment_state: RadrootsTradePaymentStateV1::NotTracked,
            candidate_heads: Vec::new(),
            agreement_claims: Vec::new(),
            active_agreement_claim_ids: Vec::new(),
            contested_claim_ids: Vec::new(),
            cancelled_claim_ids: Vec::new(),
            declined_candidate_ids: Vec::new(),
            missing_parent_ids: Vec::new(),
            missing_proposal_ids: Vec::new(),
            unsupported_mutation_ids: Vec::new(),
            issues: Vec::new(),
            attestations: Vec::new(),
            projection_digest: String::new(),
        }
    }

    fn finish(&mut self) {
        self.candidate_heads.sort();
        self.candidate_heads.dedup();
        self.active_agreement_claim_ids.sort();
        self.active_agreement_claim_ids.dedup();
        self.contested_claim_ids.sort();
        self.contested_claim_ids.dedup();
        self.cancelled_claim_ids.sort();
        self.cancelled_claim_ids.dedup();
        self.declined_candidate_ids.sort();
        self.declined_candidate_ids.dedup();
        self.missing_parent_ids.sort();
        self.missing_parent_ids.dedup();
        self.missing_proposal_ids.sort();
        self.missing_proposal_ids.dedup();
        self.unsupported_mutation_ids.sort();
        self.unsupported_mutation_ids.dedup();
        self.agreement_claims
            .sort_by_key(|left| left.claim_mutation_id);
        self.issues.sort();
        self.issues.dedup();
        self.attestations.sort();
        self.attestations.dedup();
        match projection_digest(self) {
            Ok(digest) => self.projection_digest = digest,
            Err(reason) => {
                self.projection_digest.clear();
                self.issues
                    .push(RadrootsTradeReducerIssueV1::ProjectionDigestUnavailable { reason });
                self.issues.sort();
                self.issues.dedup();
            }
        }
    }

    pub fn reducer_contract_id(&self) -> &str {
        &self.reducer_contract_id
    }

    pub const fn reducer_version(&self) -> u16 {
        self.reducer_version
    }

    pub const fn trade_id(&self) -> &TradeId {
        &self.trade_id
    }

    pub const fn root_mutation_id(&self) -> Option<&MutationId> {
        self.root_mutation_id.as_ref()
    }

    pub const fn buyer_pubkey(&self) -> Option<&PublicKey> {
        self.buyer_pubkey.as_ref()
    }

    pub const fn seller_pubkey(&self) -> Option<&PublicKey> {
        self.seller_pubkey.as_ref()
    }

    pub const fn farm_id(&self) -> Option<&DTag> {
        self.farm_id.as_ref()
    }

    pub const fn negotiation_state(&self) -> RadrootsTradeNegotiationStateV1 {
        self.negotiation_state
    }

    pub const fn agreement_state(&self) -> RadrootsTradeAgreementStateV1 {
        self.agreement_state
    }

    pub const fn evidence_state(&self) -> RadrootsTradeEvidenceStateV1 {
        self.evidence_state
    }

    pub const fn conflict_state(&self) -> RadrootsTradeConflictStateV1 {
        self.conflict_state
    }

    pub const fn private_terms_state(&self) -> RadrootsTradePrivateTermsStateV1 {
        self.private_terms_state
    }

    pub const fn attestation_state(&self) -> RadrootsTradeAttestationStateV1 {
        self.attestation_state
    }

    pub const fn fulfillment_state(&self) -> RadrootsTradeFulfillmentStateV1 {
        self.fulfillment_state
    }

    pub const fn payment_state(&self) -> RadrootsTradePaymentStateV1 {
        self.payment_state
    }

    pub fn candidate_heads(&self) -> &[MutationId] {
        &self.candidate_heads
    }

    pub fn agreement_claims(&self) -> &[RadrootsTradeAgreementClaimV1] {
        &self.agreement_claims
    }

    pub fn active_agreement_claim_ids(&self) -> &[MutationId] {
        &self.active_agreement_claim_ids
    }

    pub fn contested_claim_ids(&self) -> &[MutationId] {
        &self.contested_claim_ids
    }

    pub fn cancelled_claim_ids(&self) -> &[MutationId] {
        &self.cancelled_claim_ids
    }

    pub fn declined_candidate_ids(&self) -> &[CandidateId] {
        &self.declined_candidate_ids
    }

    pub fn missing_parent_ids(&self) -> &[MutationId] {
        &self.missing_parent_ids
    }

    pub fn missing_proposal_ids(&self) -> &[MutationId] {
        &self.missing_proposal_ids
    }

    pub fn unsupported_mutation_ids(&self) -> &[MutationId] {
        &self.unsupported_mutation_ids
    }

    pub fn issues(&self) -> &[RadrootsTradeReducerIssueV1] {
        &self.issues
    }

    pub fn attestations(&self) -> &[RadrootsTradeAttestationRecordV1] {
        &self.attestations
    }

    pub fn projection_digest(&self) -> &str {
        &self.projection_digest
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RadrootsTradeAgreementClaimV1 {
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    claim_mutation_id: MutationId,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    proposal_mutation_id: MutationId,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    candidate_id: CandidateId,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    candidate_author_pubkey: PublicKey,
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    accepted_by_pubkey: PublicKey,
    reservation_commitment: String,
}

impl RadrootsTradeAgreementClaimV1 {
    pub const fn claim_mutation_id(&self) -> &MutationId {
        &self.claim_mutation_id
    }

    pub const fn proposal_mutation_id(&self) -> &MutationId {
        &self.proposal_mutation_id
    }

    pub const fn candidate_id(&self) -> &CandidateId {
        &self.candidate_id
    }

    pub const fn candidate_author_pubkey(&self) -> &PublicKey {
        &self.candidate_author_pubkey
    }

    pub const fn accepted_by_pubkey(&self) -> &PublicKey {
        &self.accepted_by_pubkey
    }

    pub fn reservation_commitment(&self) -> &str {
        &self.reservation_commitment
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeReducerIssueV1 {
    MissingRootProposal,
    MultipleRootProposals,
    MissingMutationId,
    TradeIdentityMismatch {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        mutation_id: MutationId,
    },
    UnsupportedSchema {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        mutation_id: MutationId,
        schema_version: u16,
    },
    InvalidMutation {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        mutation_id: Option<MutationId>,
        reason: String,
    },
    MissingParent {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        mutation_id: MutationId,
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        parent_mutation_id: MutationId,
    },
    MissingProposal {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        decision_mutation_id: MutationId,
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        proposal_mutation_id: MutationId,
    },
    CandidateIdMismatch {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        decision_mutation_id: MutationId,
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        proposal_mutation_id: MutationId,
    },
    DecisionAuthorMismatch {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        decision_mutation_id: MutationId,
    },
    DecisionParentMissing {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        decision_mutation_id: MutationId,
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        proposal_mutation_id: MutationId,
    },
    MissingSellerReservation {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        decision_mutation_id: MutationId,
    },
    ReservationCandidateMismatch {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        decision_mutation_id: MutationId,
    },
    ReservationAuthorityMismatch {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        decision_mutation_id: MutationId,
    },
    ReservationLineMismatch {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        decision_mutation_id: MutationId,
    },
    DecisionConflict {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        proposal_mutation_id: MutationId,
    },
    DoubleAcceptance {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        proposal_mutation_id: MutationId,
    },
    CancellationConflict {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        cancellation_mutation_id: MutationId,
    },
    InvalidCausalChain {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        mutation_id: MutationId,
    },
    PrivateTermsUnavailable {
        #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
        candidate_id: CandidateId,
    },
    ProjectionDigestUnavailable {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CandidateRecord {
    proposal_mutation_id: MutationId,
    author_pubkey: PublicKey,
    candidate: TradeCandidateTermsV1,
}

struct DecisionApplication<'a> {
    mutation_id: &'a MutationId,
    mutation: &'a TradeMutationEnvelopeV1,
    proposal_mutation_id: &'a MutationId,
    candidate_id: &'a CandidateId,
    decision: &'a TradeDecisionV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CancellationRecord {
    mutation_id: MutationId,
    parent_mutation_ids: Vec<MutationId>,
    target_candidate_id: Option<CandidateId>,
    target_claim_mutation_id: Option<MutationId>,
}

pub fn reduce_trade_records(input: RadrootsTradeReductionInputV1) -> RadrootsTradeProjectionV1 {
    let mut projection = RadrootsTradeProjectionV1::empty(input.trade_id);
    let mut mutations = BTreeMap::<MutationId, TradeMutationEnvelopeV1>::new();

    for record in input.mutations {
        let mutation_id = match record.mutation.mutation_id {
            Some(mutation_id) => mutation_id,
            None => {
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::MissingMutationId);
                continue;
            }
        };
        if record.mutation.schema_version != RADROOTS_TRADE_SCHEMA_VERSION {
            projection.unsupported_mutation_ids.push(mutation_id);
            projection
                .issues
                .push(RadrootsTradeReducerIssueV1::UnsupportedSchema {
                    mutation_id,
                    schema_version: record.mutation.schema_version,
                });
            continue;
        }
        if let Err(error) = record.mutation.validate() {
            projection
                .issues
                .push(RadrootsTradeReducerIssueV1::InvalidMutation {
                    mutation_id: Some(mutation_id),
                    reason: error.to_string(),
                });
            continue;
        }
        if record.mutation.trade_id != input.trade_id {
            projection
                .issues
                .push(RadrootsTradeReducerIssueV1::TradeIdentityMismatch { mutation_id });
            continue;
        }
        mutations.entry(mutation_id).or_insert(record.mutation);
    }

    let mut root_proposals = Vec::<MutationId>::new();
    let mut candidates_by_proposal = BTreeMap::<MutationId, CandidateRecord>::new();
    let mut claims = BTreeMap::<MutationId, RadrootsTradeAgreementClaimV1>::new();
    let mut decisions_by_proposal = BTreeMap::<MutationId, Vec<MutationId>>::new();
    let mut cancellations = Vec::<CancellationRecord>::new();
    let mut referenced_parents = BTreeSet::<MutationId>::new();

    for (mutation_id, mutation) in &mutations {
        if matches!(mutation.body, TradeMutationBodyV1::Proposal { .. }) {
            root_proposals.push(*mutation_id);
        }
        for parent in &mutation.parent_mutation_ids {
            referenced_parents.insert(*parent);
            if !mutations.contains_key(parent) {
                projection.missing_parent_ids.push(*parent);
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::MissingParent {
                        mutation_id: *mutation_id,
                        parent_mutation_id: *parent,
                    });
            }
        }
        match &mutation.body {
            TradeMutationBodyV1::Proposal { candidate }
            | TradeMutationBodyV1::RevisionProposal { candidate } => {
                candidates_by_proposal.insert(
                    *mutation_id,
                    CandidateRecord {
                        proposal_mutation_id: *mutation_id,
                        author_pubkey: mutation.author_pubkey,
                        candidate: candidate.clone(),
                    },
                );
            }
            TradeMutationBodyV1::Decision { .. } | TradeMutationBodyV1::RevisionDecision { .. } => {
            }
            TradeMutationBodyV1::Cancellation {
                target_candidate_id,
                target_claim_mutation_id,
                reason: _,
            } => cancellations.push(CancellationRecord {
                mutation_id: *mutation_id,
                parent_mutation_ids: mutation.parent_mutation_ids.clone(),
                target_candidate_id: *target_candidate_id,
                target_claim_mutation_id: *target_claim_mutation_id,
            }),
        }
    }

    for (mutation_id, mutation) in &mutations {
        match &mutation.body {
            TradeMutationBodyV1::Decision {
                proposal_mutation_id,
                candidate_id,
                decision,
            }
            | TradeMutationBodyV1::RevisionDecision {
                proposal_mutation_id,
                candidate_id,
                decision,
            } => {
                decisions_by_proposal
                    .entry(*proposal_mutation_id)
                    .or_default()
                    .push(*mutation_id);
                apply_decision(
                    DecisionApplication {
                        mutation_id,
                        mutation,
                        proposal_mutation_id,
                        candidate_id,
                        decision,
                    },
                    &candidates_by_proposal,
                    &mut claims,
                    &mut projection,
                );
            }
            _ => {}
        }
    }

    if root_proposals.is_empty() {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::MissingRootProposal);
    } else if root_proposals.len() > 1 {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::MultipleRootProposals);
        set_conflict(
            &mut projection.conflict_state,
            RadrootsTradeConflictStateV1::ConcurrentCandidates,
        );
    } else {
        projection.root_mutation_id = root_proposals.first().cloned();
        let root = projection
            .root_mutation_id
            .as_ref()
            .and_then(|root_id| mutations.get(root_id))
            .expect("root proposal selected from the validated mutation map");
        projection.buyer_pubkey = Some(root.buyer_pubkey);
        projection.seller_pubkey = Some(root.seller_pubkey);
        projection.farm_id = Some(root.farm_id.clone());
    }

    for (proposal_mutation_id, decision_ids) in &decisions_by_proposal {
        let unique: BTreeSet<MutationId> = decision_ids.iter().cloned().collect();
        if unique.len() > 1 {
            let accept_count = unique
                .iter()
                .filter(|decision_id| {
                    mutations
                        .get(*decision_id)
                        .is_some_and(|mutation| is_acceptance(&mutation.body))
                })
                .count();
            if accept_count > 1 {
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::DoubleAcceptance {
                        proposal_mutation_id: *proposal_mutation_id,
                    });
                set_conflict(
                    &mut projection.conflict_state,
                    RadrootsTradeConflictStateV1::DoubleAcceptance,
                );
            }
            if accept_count > 0 && accept_count < unique.len() {
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::DecisionConflict {
                        proposal_mutation_id: *proposal_mutation_id,
                    });
                set_conflict(
                    &mut projection.conflict_state,
                    RadrootsTradeConflictStateV1::DecisionConflict,
                );
            }
        }
    }

    projection.candidate_heads = mutations
        .keys()
        .filter(|mutation_id| !referenced_parents.contains(*mutation_id))
        .cloned()
        .collect();
    projection.declined_candidate_ids = declined_candidate_ids(&mutations);
    projection.agreement_claims = claims.values().cloned().collect();
    apply_agreement_state(
        &mut projection,
        &claims,
        &mutations,
        &candidates_by_proposal,
        &cancellations,
    );
    apply_negotiation_state(
        &mut projection,
        &candidates_by_proposal,
        &claims,
        input.observed_at_unix_s,
    );
    projection.private_terms_state =
        reduce_private_terms_state(&projection, &candidates_by_proposal, &input.private_terms);
    if matches!(
        projection.private_terms_state,
        RadrootsTradePrivateTermsStateV1::Missing
            | RadrootsTradePrivateTermsStateV1::Undecryptable
            | RadrootsTradePrivateTermsStateV1::CommitmentMismatch
    ) {
        for claim_id in &projection.active_agreement_claim_ids {
            let claim = claims
                .get(claim_id)
                .expect("active agreement identifiers originate from indexed claims");
            projection
                .issues
                .push(RadrootsTradeReducerIssueV1::PrivateTermsUnavailable {
                    candidate_id: claim.candidate_id,
                });
        }
    }
    projection.attestations = input.attestations;
    projection.attestation_state = reduce_attestation_state(&projection.attestations);
    projection.evidence_state = reduce_evidence_state(&projection, input.evidence_state);
    projection.finish();
    projection
}

fn apply_decision(
    application: DecisionApplication<'_>,
    candidates_by_proposal: &BTreeMap<MutationId, CandidateRecord>,
    claims: &mut BTreeMap<MutationId, RadrootsTradeAgreementClaimV1>,
    projection: &mut RadrootsTradeProjectionV1,
) {
    let DecisionApplication {
        mutation_id,
        mutation,
        proposal_mutation_id,
        candidate_id,
        decision,
    } = application;
    let Some(candidate_record) = candidates_by_proposal.get(proposal_mutation_id) else {
        projection.missing_proposal_ids.push(*proposal_mutation_id);
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::MissingProposal {
                decision_mutation_id: *mutation_id,
                proposal_mutation_id: *proposal_mutation_id,
            });
        return;
    };
    if candidate_record.candidate.candidate_id.as_ref() != Some(candidate_id) {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::CandidateIdMismatch {
                decision_mutation_id: *mutation_id,
                proposal_mutation_id: *proposal_mutation_id,
            });
        set_conflict(
            &mut projection.conflict_state,
            RadrootsTradeConflictStateV1::DecisionConflict,
        );
        return;
    }
    if mutation.author_pubkey != candidate_record_author_counterparty(candidate_record, mutation) {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::DecisionAuthorMismatch {
                decision_mutation_id: *mutation_id,
            });
        return;
    }
    if !mutation.parent_mutation_ids.contains(proposal_mutation_id) {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::DecisionParentMissing {
                decision_mutation_id: *mutation_id,
                proposal_mutation_id: *proposal_mutation_id,
            });
        set_conflict(
            &mut projection.conflict_state,
            RadrootsTradeConflictStateV1::InvalidCausalChain,
        );
    }
    match decision {
        TradeDecisionV1::Accepted {
            reservation_assertion,
        } => {
            let Some(reservation) = reservation_assertion else {
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::MissingSellerReservation {
                        decision_mutation_id: *mutation_id,
                    });
                return;
            };
            if validate_reservation(
                mutation_id,
                candidate_id,
                &candidate_record.candidate,
                reservation,
                projection,
            ) {
                claims.insert(
                    *mutation_id,
                    RadrootsTradeAgreementClaimV1 {
                        claim_mutation_id: *mutation_id,
                        proposal_mutation_id: candidate_record.proposal_mutation_id,
                        candidate_id: *candidate_id,
                        candidate_author_pubkey: candidate_record.author_pubkey,
                        accepted_by_pubkey: mutation.author_pubkey,
                        reservation_commitment: reservation.assertion_commitment.clone(),
                    },
                );
            }
        }
        TradeDecisionV1::Declined { .. } => {}
    }
}

fn candidate_record_author_counterparty(
    candidate_record: &CandidateRecord,
    mutation: &TradeMutationEnvelopeV1,
) -> PublicKey {
    if candidate_record.author_pubkey == mutation.buyer_pubkey {
        mutation.seller_pubkey
    } else {
        mutation.buyer_pubkey
    }
}

fn validate_reservation(
    decision_mutation_id: &MutationId,
    candidate_id: &CandidateId,
    candidate: &TradeCandidateTermsV1,
    reservation: &SellerReservationAssertionV1,
    projection: &mut RadrootsTradeProjectionV1,
) -> bool {
    let mut valid = true;
    if &reservation.candidate_id != candidate_id {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::ReservationCandidateMismatch {
                decision_mutation_id: *decision_mutation_id,
            });
        valid = false;
    }
    if reservation.inventory_authority_id != candidate.seller_pubkey {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::ReservationAuthorityMismatch {
                decision_mutation_id: *decision_mutation_id,
            });
        set_conflict(
            &mut projection.conflict_state,
            RadrootsTradeConflictStateV1::InventoryAuthorityConflict,
        );
        valid = false;
    }
    if reservation.commitments.len() != candidate.lines.len() {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::ReservationLineMismatch {
                decision_mutation_id: *decision_mutation_id,
            });
        return false;
    }
    for (line, commitment) in candidate.lines.iter().zip(reservation.commitments.iter()) {
        if line.line_id != commitment.line_id
            || line.bin_id != commitment.bin_id
            || line.quantity_mantissa != commitment.quantity_mantissa
            || line.quantity_scale != commitment.quantity_scale
            || line.unit_code != commitment.unit_code
        {
            projection
                .issues
                .push(RadrootsTradeReducerIssueV1::ReservationLineMismatch {
                    decision_mutation_id: *decision_mutation_id,
                });
            valid = false;
            break;
        }
    }
    valid
}

fn apply_agreement_state(
    projection: &mut RadrootsTradeProjectionV1,
    claims: &BTreeMap<MutationId, RadrootsTradeAgreementClaimV1>,
    mutations: &BTreeMap<MutationId, TradeMutationEnvelopeV1>,
    candidates_by_proposal: &BTreeMap<MutationId, CandidateRecord>,
    cancellations: &[CancellationRecord],
) {
    if claims.is_empty() {
        if cancellation_without_claim(cancellations, candidates_by_proposal) {
            projection.agreement_state = RadrootsTradeAgreementStateV1::Cancelled;
        }
        return;
    }

    let active_claim_ids = non_dominated_claim_ids(claims, mutations);
    let active_claims = active_claim_ids
        .iter()
        .filter_map(|claim_id| claims.get(claim_id))
        .collect::<Vec<_>>();
    let compatible = compatible_claims(&active_claims);
    projection.active_agreement_claim_ids = active_claim_ids;

    if compatible {
        projection.agreement_state = RadrootsTradeAgreementStateV1::Agreed;
    } else {
        projection.agreement_state = RadrootsTradeAgreementStateV1::Contested;
        projection.contested_claim_ids = projection.active_agreement_claim_ids.clone();
        if projection.conflict_state == RadrootsTradeConflictStateV1::None {
            set_conflict(
                &mut projection.conflict_state,
                RadrootsTradeConflictStateV1::DecisionConflict,
            );
        }
    }

    for cancellation in cancellations {
        if let Some(target_claim_id) = &cancellation.target_claim_mutation_id
            && claims.contains_key(target_claim_id)
        {
            if cancellation.parent_mutation_ids.contains(target_claim_id) {
                projection.cancelled_claim_ids.push(*target_claim_id);
                if projection.active_agreement_claim_ids == [*target_claim_id] {
                    projection.agreement_state = RadrootsTradeAgreementStateV1::Cancelled;
                }
            } else {
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::CancellationConflict {
                        cancellation_mutation_id: cancellation.mutation_id,
                    });
                projection.agreement_state = RadrootsTradeAgreementStateV1::Contested;
                set_conflict(
                    &mut projection.conflict_state,
                    RadrootsTradeConflictStateV1::CancellationConflict,
                );
            }
        }
    }
}

fn cancellation_without_claim(
    cancellations: &[CancellationRecord],
    candidates_by_proposal: &BTreeMap<MutationId, CandidateRecord>,
) -> bool {
    cancellations.iter().any(|cancellation| {
        cancellation
            .target_candidate_id
            .as_ref()
            .is_some_and(|candidate_id| {
                candidates_by_proposal.values().any(|candidate| {
                    candidate.candidate.candidate_id.as_ref() == Some(candidate_id)
                        && candidate.candidate.cancellation.buyer_pre_agreement
                })
            })
    })
}

fn non_dominated_claim_ids(
    claims: &BTreeMap<MutationId, RadrootsTradeAgreementClaimV1>,
    mutations: &BTreeMap<MutationId, TradeMutationEnvelopeV1>,
) -> Vec<MutationId> {
    let mut memo = BTreeMap::<MutationId, BTreeSet<MutationId>>::new();
    claims
        .keys()
        .filter(|claim_id| {
            !claims.keys().any(|other_claim_id| {
                other_claim_id != *claim_id
                    && ancestors_of(other_claim_id, mutations, &mut memo).contains(*claim_id)
            })
        })
        .cloned()
        .collect()
}

fn compatible_claims(claims: &[&RadrootsTradeAgreementClaimV1]) -> bool {
    let first = claims
        .first()
        .expect("non-empty claims produce at least one non-dominated claim");
    claims.iter().all(|claim| {
        claim.candidate_id == first.candidate_id
            && claim.reservation_commitment == first.reservation_commitment
    })
}

fn ancestors_of(
    mutation_id: &MutationId,
    mutations: &BTreeMap<MutationId, TradeMutationEnvelopeV1>,
    memo: &mut BTreeMap<MutationId, BTreeSet<MutationId>>,
) -> BTreeSet<MutationId> {
    if let Some(cached) = memo.get(mutation_id) {
        return cached.clone();
    }
    let mut ancestors = BTreeSet::new();
    if let Some(mutation) = mutations.get(mutation_id) {
        for parent in &mutation.parent_mutation_ids {
            ancestors.insert(*parent);
            ancestors.extend(ancestors_of(parent, mutations, memo));
        }
    }
    memo.insert(*mutation_id, ancestors.clone());
    ancestors
}

fn apply_negotiation_state(
    projection: &mut RadrootsTradeProjectionV1,
    candidates_by_proposal: &BTreeMap<MutationId, CandidateRecord>,
    claims: &BTreeMap<MutationId, RadrootsTradeAgreementClaimV1>,
    observed_at_unix_s: Option<u64>,
) {
    if candidates_by_proposal.is_empty() {
        projection.negotiation_state = RadrootsTradeNegotiationStateV1::None;
    } else if claims.is_empty()
        && observed_at_unix_s.is_some_and(|observed_at| {
            candidates_by_proposal
                .values()
                .all(|candidate| candidate.candidate.proposal_expires_at_unix_s <= observed_at)
        })
    {
        projection.negotiation_state = RadrootsTradeNegotiationStateV1::ClosedExpired;
    } else if claims.is_empty() && !projection.declined_candidate_ids.is_empty() {
        projection.negotiation_state = RadrootsTradeNegotiationStateV1::ClosedDeclined;
    } else {
        projection.negotiation_state = RadrootsTradeNegotiationStateV1::Open;
    }
}

fn reduce_private_terms_state(
    projection: &RadrootsTradeProjectionV1,
    candidates_by_proposal: &BTreeMap<MutationId, CandidateRecord>,
    private_terms: &[RadrootsTradePrivateTermsEvidenceV1],
) -> RadrootsTradePrivateTermsStateV1 {
    let mut private_terms_by_candidate =
        BTreeMap::<CandidateId, RadrootsTradePrivateTermsStateV1>::new();
    for record in private_terms {
        private_terms_by_candidate
            .entry(record.candidate_id)
            .and_modify(|state| *state = (*state).max(record.state))
            .or_insert(record.state);
    }
    let mut required_states = Vec::new();
    for claim in &projection.agreement_claims {
        let candidate_record = candidates_by_proposal
            .get(&claim.proposal_mutation_id)
            .expect("agreement claims originate from indexed candidates");
        let requires_private_terms = candidate_record.candidate.private_terms.is_some()
            || candidate_record
                .candidate
                .fulfillment
                .requires_private_terms;
        if requires_private_terms {
            required_states.push(
                private_terms_by_candidate
                    .get(&claim.candidate_id)
                    .copied()
                    .unwrap_or(RadrootsTradePrivateTermsStateV1::Missing),
            );
        }
    }
    if required_states.is_empty() {
        RadrootsTradePrivateTermsStateV1::NotRequired
    } else if required_states.contains(&RadrootsTradePrivateTermsStateV1::CommitmentMismatch) {
        RadrootsTradePrivateTermsStateV1::CommitmentMismatch
    } else if required_states.contains(&RadrootsTradePrivateTermsStateV1::Undecryptable) {
        RadrootsTradePrivateTermsStateV1::Undecryptable
    } else if required_states.contains(&RadrootsTradePrivateTermsStateV1::Missing) {
        RadrootsTradePrivateTermsStateV1::Missing
    } else {
        RadrootsTradePrivateTermsStateV1::AvailableVerified
    }
}

fn reduce_attestation_state(
    attestations: &[RadrootsTradeAttestationRecordV1],
) -> RadrootsTradeAttestationStateV1 {
    let mut has_valid = false;
    let mut has_invalid = false;
    for attestation in attestations {
        match attestation.result {
            RadrootsTradeAttestationResultV1::Valid => has_valid = true,
            RadrootsTradeAttestationResultV1::Invalid => has_invalid = true,
        }
    }
    match (has_valid, has_invalid) {
        (false, false) => RadrootsTradeAttestationStateV1::None,
        (true, false) => RadrootsTradeAttestationStateV1::PresentValid,
        (false, true) => RadrootsTradeAttestationStateV1::PresentInvalid,
        (true, true) => RadrootsTradeAttestationStateV1::Conflicting,
    }
}

fn reduce_evidence_state(
    projection: &RadrootsTradeProjectionV1,
    requested_state: RadrootsTradeEvidenceStateV1,
) -> RadrootsTradeEvidenceStateV1 {
    if !projection.unsupported_mutation_ids.is_empty() {
        RadrootsTradeEvidenceStateV1::UnsupportedVersion
    } else if !projection.missing_parent_ids.is_empty()
        || !projection.missing_proposal_ids.is_empty()
        || projection.root_mutation_id.is_none()
    {
        RadrootsTradeEvidenceStateV1::Missing
    } else {
        requested_state
    }
}

fn declined_candidate_ids(
    mutations: &BTreeMap<MutationId, TradeMutationEnvelopeV1>,
) -> Vec<CandidateId> {
    mutations
        .values()
        .filter_map(|mutation| match &mutation.body {
            TradeMutationBodyV1::Decision {
                candidate_id,
                decision: TradeDecisionV1::Declined { .. },
                ..
            }
            | TradeMutationBodyV1::RevisionDecision {
                candidate_id,
                decision: TradeDecisionV1::Declined { .. },
                ..
            } => Some(*candidate_id),
            _ => None,
        })
        .collect()
}

fn is_acceptance(body: &TradeMutationBodyV1) -> bool {
    matches!(
        body,
        TradeMutationBodyV1::Decision {
            decision: TradeDecisionV1::Accepted { .. },
            ..
        } | TradeMutationBodyV1::RevisionDecision {
            decision: TradeDecisionV1::Accepted { .. },
            ..
        }
    )
}

fn set_conflict(
    current: &mut RadrootsTradeConflictStateV1,
    candidate: RadrootsTradeConflictStateV1,
) {
    if *current == RadrootsTradeConflictStateV1::None || candidate > *current {
        *current = candidate;
    }
}

#[cfg(feature = "json")]
fn projection_digest(projection: &RadrootsTradeProjectionV1) -> Result<String, String> {
    let mut digest_input = projection.clone();
    digest_input.projection_digest.clear();
    let value = serde_json::to_value(&digest_input)
        .map_err(|error| format!("projection serialization failed: {error}"))?;
    let canonical = radroots_event::trade::canonical_jcs_value(&value)
        .map_err(|error| format!("projection canonicalization failed: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(RADROOTS_TRADE_PROJECTION_DIGEST_DOMAIN);
    hasher.update(canonical.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(not(feature = "json"))]
fn projection_digest(_projection: &RadrootsTradeProjectionV1) -> Result<String, String> {
    Err("projection digest requires the serde_json feature".to_string())
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use radroots_event::{
        id::{ClassifiedListingAddress, InventoryBinId},
        trade::{
            FulfillmentProfileV1, RADROOTS_TRADE_DECISION_CONTRACT_ID,
            RADROOTS_TRADE_PROPOSAL_CONTRACT_ID, RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID,
            RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID, SellerReservationLineV1,
            TradeCancellationProfileV1, TradeCandidateLineV1, TradeEconomicAdjustmentV1,
            TradeEconomicsProfileV1, TradeLineTombstoneV1, TradePrivateTermsRefV1,
            canonical_trade_mutation_content,
        },
    };
    use radroots_test_fixtures::{FIXTURE_ALICE_PUBLIC_KEY_HEX, FIXTURE_BOB_PUBLIC_KEY_HEX};

    const CANONICAL_REDUCER_VECTORS: &str =
        include_str!("../../../contracts/conformance/vectors/trade/reduce_records.v1.json");
    const PACKAGED_REDUCER_VECTORS: &str = include_str!("../tests/fixtures/reduce_records.v1.json");

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn hex_32(character: char) -> String {
        core::iter::repeat_n(character, 32).collect()
    }

    fn pubkey(character: char) -> PublicKey {
        let public_key_hex = match character {
            'a' => FIXTURE_ALICE_PUBLIC_KEY_HEX,
            'b' => FIXTURE_BOB_PUBLIC_KEY_HEX,
            _ => panic!("unsupported fixture public key label: {character}"),
        };
        PublicKey::from_hex(public_key_hex).expect("fixture pubkey")
    }

    fn event_id(character: char) -> EventId {
        EventId::parse(hex_64(character)).unwrap()
    }

    fn trade_id() -> TradeId {
        TradeId::parse(hex_32('1')).unwrap()
    }

    fn dtag(value: &str) -> DTag {
        DTag::parse(value).unwrap()
    }

    fn bin_id(value: &str) -> InventoryBinId {
        InventoryBinId::parse(value).unwrap()
    }

    fn candidate(line_suffix: &str) -> TradeCandidateTermsV1 {
        TradeCandidateTermsV1 {
            candidate_id: None,
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            base_candidate_id: None,
            supersession_intent: None,
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            lines: vec![TradeCandidateLineV1 {
                line_id: dtag(&format!("line-{line_suffix}")),
                listing_addr: ClassifiedListingAddress::parse(format!(
                    "30402:{}:listing-{line_suffix}",
                    FIXTURE_BOB_PUBLIC_KEY_HEX
                ))
                .unwrap(),
                listing_event_id: event_id('c'),
                listing_snapshot_sha256: hex_64('d'),
                product_id: format!("carrots-{line_suffix}"),
                option_id: None,
                bin_id: bin_id(&format!("bin-{line_suffix}")),
                quantity_mantissa: "2".to_string(),
                quantity_scale: 0,
                unit_code: "count".to_string(),
                unit_profile: "mvp-count".to_string(),
                unit_price_mantissa: "500".to_string(),
                currency_code: "USD".to_string(),
                line_subtotal_mantissa: "1000".to_string(),
                replaces_line_id: None,
            }],
            line_tombstones: Vec::<TradeLineTombstoneV1>::new(),
            economics: TradeEconomicsProfileV1 {
                profile_id: "mvp-fixed".to_string(),
                currency_code: "USD".to_string(),
                currency_exponent: 2,
                rounding_profile: "half-even".to_string(),
                subtotal_mantissa: "1000".to_string(),
                discount_total_mantissa: "0".to_string(),
                adjustment_total_mantissa: "0".to_string(),
                total_mantissa: "1000".to_string(),
                adjustments: Vec::<TradeEconomicAdjustmentV1>::new(),
            },
            fulfillment: FulfillmentProfileV1 {
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
            cancellation: TradeCancellationProfileV1 {
                profile_id: "buyer-pre-agreement".to_string(),
                buyer_pre_agreement: true,
                post_agreement_cutoff_unix_s: Some(1_799_990_000),
            },
            private_terms: None,
            proposal_expires_at_unix_s: 1_799_999_000,
        }
    }

    fn proposal() -> TradeMutationEnvelopeV1 {
        canonical_trade_mutation_content(TradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_PROPOSAL_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: None,
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            parent_mutation_ids: Vec::new(),
            author_pubkey: pubkey('a'),
            counterparty_pubkey: pubkey('b'),
            authored_at_unix_s: 100,
            body: TradeMutationBodyV1::Proposal {
                candidate: candidate("1"),
            },
        })
        .unwrap()
        .envelope
    }

    fn reservation(
        candidate: &TradeCandidateTermsV1,
        marker: char,
    ) -> SellerReservationAssertionV1 {
        SellerReservationAssertionV1 {
            reservation_id: dtag(&format!("reservation-{marker}")),
            inventory_authority_id: candidate.seller_pubkey,
            inventory_epoch: 42,
            candidate_id: candidate.candidate_id.unwrap(),
            commitments: candidate
                .lines
                .iter()
                .map(|line| SellerReservationLineV1 {
                    line_id: line.line_id.clone(),
                    bin_id: line.bin_id.clone(),
                    quantity_mantissa: line.quantity_mantissa.clone(),
                    quantity_scale: line.quantity_scale,
                    unit_code: line.unit_code.clone(),
                })
                .collect(),
            reservation_expires_at_unix_s: 1_800_000_000,
            assertion_commitment: hex_64(marker),
        }
    }

    fn accepted_decision(
        proposal: &TradeMutationEnvelopeV1,
        marker: char,
    ) -> TradeMutationEnvelopeV1 {
        let proposal_id = proposal.mutation_id.unwrap();
        let candidate = match &proposal.body {
            TradeMutationBodyV1::Proposal { candidate }
            | TradeMutationBodyV1::RevisionProposal { candidate } => candidate.clone(),
            _ => unreachable!(),
        };
        canonical_trade_mutation_content(TradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_DECISION_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: Some(root_id(proposal)),
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            parent_mutation_ids: vec![proposal_id],
            author_pubkey: pubkey('b'),
            counterparty_pubkey: pubkey('a'),
            authored_at_unix_s: u64::from(marker),
            body: TradeMutationBodyV1::Decision {
                proposal_mutation_id: proposal_id,
                candidate_id: candidate.candidate_id.unwrap(),
                decision: TradeDecisionV1::Accepted {
                    reservation_assertion: Some(reservation(&candidate, marker)),
                },
            },
        })
        .unwrap()
        .envelope
    }

    fn declined_decision(proposal: &TradeMutationEnvelopeV1) -> TradeMutationEnvelopeV1 {
        let proposal_id = proposal.mutation_id.unwrap();
        let candidate = match &proposal.body {
            TradeMutationBodyV1::Proposal { candidate }
            | TradeMutationBodyV1::RevisionProposal { candidate } => candidate.clone(),
            _ => unreachable!(),
        };
        canonical_trade_mutation_content(TradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_DECISION_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: Some(root_id(proposal)),
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            parent_mutation_ids: vec![proposal_id],
            author_pubkey: pubkey('b'),
            counterparty_pubkey: pubkey('a'),
            authored_at_unix_s: 102,
            body: TradeMutationBodyV1::Decision {
                proposal_mutation_id: proposal_id,
                candidate_id: candidate.candidate_id.unwrap(),
                decision: TradeDecisionV1::Declined {
                    reason: "unavailable".to_string(),
                },
            },
        })
        .unwrap()
        .envelope
    }

    fn revision_proposal(
        root: &TradeMutationEnvelopeV1,
        parents: Vec<MutationId>,
    ) -> TradeMutationEnvelopeV1 {
        let mut parents = parents;
        parents.sort();
        canonical_trade_mutation_content(TradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: Some(root_id(root)),
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            parent_mutation_ids: parents,
            author_pubkey: pubkey('a'),
            counterparty_pubkey: pubkey('b'),
            authored_at_unix_s: 200,
            body: TradeMutationBodyV1::RevisionProposal {
                candidate: candidate("2"),
            },
        })
        .unwrap()
        .envelope
    }

    fn revision_acceptance(
        root: &TradeMutationEnvelopeV1,
        proposal: &TradeMutationEnvelopeV1,
    ) -> TradeMutationEnvelopeV1 {
        let proposal_id = proposal.mutation_id.unwrap();
        let candidate = match &proposal.body {
            TradeMutationBodyV1::RevisionProposal { candidate } => candidate.clone(),
            _ => unreachable!(),
        };
        canonical_trade_mutation_content(TradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: Some(root_id(root)),
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            parent_mutation_ids: vec![proposal_id],
            author_pubkey: pubkey('b'),
            counterparty_pubkey: pubkey('a'),
            authored_at_unix_s: 201,
            body: TradeMutationBodyV1::RevisionDecision {
                proposal_mutation_id: proposal_id,
                candidate_id: candidate.candidate_id.unwrap(),
                decision: TradeDecisionV1::Accepted {
                    reservation_assertion: Some(reservation(&candidate, '9')),
                },
            },
        })
        .unwrap()
        .envelope
    }

    fn cancellation(
        root: &TradeMutationEnvelopeV1,
        target_claim: MutationId,
        parent: MutationId,
    ) -> TradeMutationEnvelopeV1 {
        canonical_trade_mutation_content(TradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: radroots_event::trade::RADROOTS_TRADE_CANCELLATION_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: Some(root_id(root)),
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            parent_mutation_ids: vec![parent],
            author_pubkey: pubkey('a'),
            counterparty_pubkey: pubkey('b'),
            authored_at_unix_s: 300,
            body: TradeMutationBodyV1::Cancellation {
                target_candidate_id: None,
                target_claim_mutation_id: Some(target_claim),
                reason: "before cutoff".to_string(),
            },
        })
        .unwrap()
        .envelope
    }

    fn root_id(envelope: &TradeMutationEnvelopeV1) -> MutationId {
        envelope.mutation_id.unwrap()
    }

    fn record(mutation: TradeMutationEnvelopeV1) -> RadrootsTradeMutationRecordV1 {
        RadrootsTradeMutationRecordV1 {
            transport_event_id: None,
            mutation,
        }
    }

    fn reduce(mutations: Vec<TradeMutationEnvelopeV1>) -> RadrootsTradeProjectionV1 {
        let mut input = RadrootsTradeReductionInputV1::new(trade_id());
        input.mutations = mutations.into_iter().map(record).collect();
        reduce_trade_records(input)
    }

    fn recanonicalize(mut mutation: TradeMutationEnvelopeV1) -> TradeMutationEnvelopeV1 {
        mutation.mutation_id = None;
        canonical_trade_mutation_content(mutation)
            .expect("recanonicalized mutation")
            .envelope
    }

    fn candidate_cancellation(root: &TradeMutationEnvelopeV1) -> TradeMutationEnvelopeV1 {
        let candidate_id = match &root.body {
            TradeMutationBodyV1::Proposal { candidate } => {
                candidate.candidate_id.expect("candidate id")
            }
            _ => unreachable!(),
        };
        canonical_trade_mutation_content(TradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: radroots_event::trade::RADROOTS_TRADE_CANCELLATION_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: Some(root_id(root)),
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            parent_mutation_ids: vec![root_id(root)],
            author_pubkey: pubkey('a'),
            counterparty_pubkey: pubkey('b'),
            authored_at_unix_s: 301,
            body: TradeMutationBodyV1::Cancellation {
                target_candidate_id: Some(candidate_id),
                target_claim_mutation_id: None,
                reason: "cancel before agreement".to_string(),
            },
        })
        .expect("candidate cancellation")
        .envelope
    }

    #[test]
    fn reducer_digest_is_independent_of_input_order_and_duplicates() {
        let proposal = proposal();
        let decision = accepted_decision(&proposal, '1');
        let first = reduce(vec![proposal.clone(), decision.clone(), decision.clone()]);
        let second = reduce(vec![decision, proposal]);

        assert_eq!(first.agreement_state, RadrootsTradeAgreementStateV1::Agreed);
        assert_eq!(
            first.projection_digest,
            "35a5f555344febe675f4e5e6b15865356400b2b8f7791fc631590e0f1e1fd441"
        );
        assert_eq!(first.projection_digest, second.projection_digest);
        assert_eq!(first.active_agreement_claim_ids.len(), 1);
    }

    #[test]
    fn reducer_projection_is_identical_for_every_three_record_permutation() {
        let proposal = proposal();
        let decision = accepted_decision(&proposal, '1');
        let cancellation = cancellation(&proposal, root_id(&decision), root_id(&decision));
        let permutations = [
            vec![proposal.clone(), decision.clone(), cancellation.clone()],
            vec![proposal.clone(), cancellation.clone(), decision.clone()],
            vec![decision.clone(), proposal.clone(), cancellation.clone()],
            vec![decision.clone(), cancellation.clone(), proposal.clone()],
            vec![cancellation.clone(), proposal.clone(), decision.clone()],
            vec![cancellation, decision, proposal],
        ];
        let expected = reduce(permutations[0].clone());

        for permutation in permutations.into_iter().skip(1) {
            assert_eq!(reduce(permutation), expected);
        }
    }

    #[test]
    fn reducer_excludes_unsupported_versions_from_domain_semantics() {
        let mut unsupported = proposal();
        unsupported.schema_version += 1;
        let unsupported_id = root_id(&unsupported);

        let projection = reduce(vec![unsupported]);

        assert_eq!(
            projection.evidence_state,
            RadrootsTradeEvidenceStateV1::UnsupportedVersion
        );
        assert_eq!(
            projection.negotiation_state,
            RadrootsTradeNegotiationStateV1::None
        );
        assert_eq!(
            projection.agreement_state,
            RadrootsTradeAgreementStateV1::None
        );
        assert_eq!(projection.root_mutation_id, None);
        assert_eq!(projection.unsupported_mutation_ids, vec![unsupported_id]);
    }

    #[test]
    fn reducer_private_evidence_precedence_is_permutation_independent() {
        let mut root = proposal();
        if let TradeMutationBodyV1::Proposal { candidate } = &mut root.body {
            candidate.private_terms = Some(TradePrivateTermsRefV1 {
                artifact_id: "artifact-1".to_string(),
                schema_id: "radroots.private.fulfillment.v1".to_string(),
                ciphertext_commitment: hex_64('f'),
                required_acknowledgement: true,
            });
        }
        let root = recanonicalize(root);
        let candidate_id = match &root.body {
            TradeMutationBodyV1::Proposal { candidate } => candidate.candidate_id.unwrap(),
            _ => unreachable!(),
        };
        let decision = accepted_decision(&root, '1');
        let evidence = |state| RadrootsTradePrivateTermsEvidenceV1::new(candidate_id, state);
        let reduce_with = |private_terms| {
            RadrootsTradeReductionInputV1::new(trade_id())
                .with_mutations(vec![record(root.clone()), record(decision.clone())])
                .with_private_terms(private_terms)
        };

        let first = reduce_trade_records(reduce_with(vec![
            evidence(RadrootsTradePrivateTermsStateV1::AvailableVerified),
            evidence(RadrootsTradePrivateTermsStateV1::CommitmentMismatch),
        ]));
        let second = reduce_trade_records(reduce_with(vec![
            evidence(RadrootsTradePrivateTermsStateV1::CommitmentMismatch),
            evidence(RadrootsTradePrivateTermsStateV1::AvailableVerified),
        ]));

        assert_eq!(
            first.private_terms_state,
            RadrootsTradePrivateTermsStateV1::CommitmentMismatch
        );
        assert_eq!(first, second);
    }

    #[test]
    fn reducer_attestation_order_and_duplicates_do_not_change_digest() {
        let root = proposal();
        let decision = accepted_decision(&root, '1');
        let valid = RadrootsTradeAttestationRecordV1::new(
            event_id('8'),
            root_id(&decision),
            RadrootsTradeAttestationResultV1::Valid,
        );
        let invalid = RadrootsTradeAttestationRecordV1::new(
            event_id('9'),
            root_id(&decision),
            RadrootsTradeAttestationResultV1::Invalid,
        );
        let reduce_with = |attestations| {
            reduce_trade_records(
                RadrootsTradeReductionInputV1::new(trade_id())
                    .with_mutations(vec![record(root.clone()), record(decision.clone())])
                    .with_attestations(attestations),
            )
        };

        let first = reduce_with(vec![valid.clone(), invalid.clone(), valid.clone()]);
        let second = reduce_with(vec![invalid, valid]);

        assert_eq!(
            first.attestation_state,
            RadrootsTradeAttestationStateV1::Conflicting
        );
        assert_eq!(first.attestations.len(), 2);
        assert_eq!(first, second);
    }

    #[test]
    fn reducer_conformance_vectors_execute_deterministic_edge_cases() {
        assert_eq!(PACKAGED_REDUCER_VECTORS, CANONICAL_REDUCER_VECTORS);
        let document: serde_json::Value =
            serde_json::from_str(PACKAGED_REDUCER_VECTORS).expect("reducer vectors parse");
        assert_eq!(document["suite"], "trade");
        assert_eq!(document["contract_version"], "1.0.0");
        let vectors = document["vectors"]
            .as_array()
            .expect("reducer vector array");
        assert_eq!(vectors.len(), 7);
        let mut ids = BTreeSet::new();

        for vector in vectors {
            let id = vector["id"].as_str().expect("reducer vector id");
            assert!(ids.insert(id), "duplicate reducer vector {id}");
            assert_eq!(vector["kind"], "trade.reduce_records", "{id}");
            assert!(vector["input"].is_object(), "{id}: input must be object");
            assert!(
                vector["expected"].is_object(),
                "{id}: expected must be object"
            );

            match id {
                "trade_reduce_agreed_projection_digest_001" => {
                    reducer_digest_is_independent_of_input_order_and_duplicates();
                }
                "trade_reduce_contested_claims_002" => {
                    reducer_preserves_contested_incompatible_acceptances_without_timestamp_winner();
                }
                "trade_reduce_attestation_only_003" => {
                    reducer_attestation_never_commits_or_invalidates_agreement();
                }
                "trade_reduce_missing_parent_004" => {
                    reducer_keeps_missing_parents_as_incomplete_evidence();
                }
                "trade_reduce_unsupported_version_isolated_005" => {
                    reducer_excludes_unsupported_versions_from_domain_semantics();
                }
                "trade_reduce_private_evidence_precedence_006" => {
                    reducer_private_evidence_precedence_is_permutation_independent();
                }
                "trade_reduce_attestation_deduplication_007" => {
                    reducer_attestation_order_and_duplicates_do_not_change_digest();
                }
                _ => panic!("unsupported reducer vector {id}"),
            }
        }
    }

    #[test]
    fn reducer_preserves_contested_incompatible_acceptances_without_timestamp_winner() {
        let proposal = proposal();
        let first = accepted_decision(&proposal, '1');
        let second = accepted_decision(&proposal, '2');
        let projection = reduce(vec![second.clone(), proposal, first.clone()]);

        assert_eq!(
            projection.agreement_state,
            RadrootsTradeAgreementStateV1::Contested
        );
        assert_eq!(
            projection.conflict_state,
            RadrootsTradeConflictStateV1::DoubleAcceptance
        );
        assert_eq!(projection.contested_claim_ids, {
            let mut ids = vec![root_id(&first), root_id(&second)];
            ids.sort();
            ids
        });
    }

    #[test]
    fn reducer_resolves_contested_state_only_with_new_causal_accepted_candidate() {
        let proposal = proposal();
        let first = accepted_decision(&proposal, '1');
        let second = accepted_decision(&proposal, '2');
        let revision = revision_proposal(&proposal, vec![root_id(&first), root_id(&second)]);
        let revision_acceptance = revision_acceptance(&proposal, &revision);
        let projection = reduce(vec![
            second,
            revision_acceptance.clone(),
            proposal,
            first,
            revision,
        ]);

        assert_eq!(
            projection.agreement_state,
            RadrootsTradeAgreementStateV1::Agreed
        );
        assert_eq!(
            projection.active_agreement_claim_ids,
            vec![root_id(&revision_acceptance)]
        );
        assert!(projection.contested_claim_ids.is_empty());
    }

    #[test]
    fn reducer_keeps_missing_parents_as_incomplete_evidence() {
        let proposal = proposal();
        let mut decision = accepted_decision(&proposal, '1');
        let missing_parent = MutationId::parse(hex_64('e')).unwrap();
        decision.parent_mutation_ids = vec![missing_parent];
        let decision = canonical_trade_mutation_content(TradeMutationEnvelopeV1 {
            mutation_id: None,
            ..decision
        })
        .unwrap()
        .envelope;
        let projection = reduce(vec![proposal, decision]);

        assert_eq!(
            projection.evidence_state,
            RadrootsTradeEvidenceStateV1::Missing
        );
        assert_eq!(projection.missing_parent_ids, vec![missing_parent]);
        assert_eq!(
            projection.agreement_state,
            RadrootsTradeAgreementStateV1::Agreed
        );
    }

    #[test]
    fn reducer_attestation_never_commits_or_invalidates_agreement() {
        let proposal = proposal();
        let decision = accepted_decision(&proposal, '1');
        let claim_id = root_id(&decision);
        let mut input = RadrootsTradeReductionInputV1::new(trade_id());
        input.mutations = vec![record(proposal), record(decision)];
        input.attestations = vec![RadrootsTradeAttestationRecordV1 {
            event_id: event_id('9'),
            claim_mutation_id: claim_id,
            result: RadrootsTradeAttestationResultV1::Invalid,
        }];
        let projection = reduce_trade_records(input);

        assert_eq!(
            projection.agreement_state,
            RadrootsTradeAgreementStateV1::Agreed
        );
        assert_eq!(
            projection.attestation_state,
            RadrootsTradeAttestationStateV1::PresentInvalid
        );
    }

    #[test]
    fn reducer_reports_causally_unordered_cancellation_conflict() {
        let proposal = proposal();
        let decision = accepted_decision(&proposal, '1');
        let cancel = cancellation(&proposal, root_id(&decision), root_id(&proposal));
        let projection = reduce(vec![proposal, decision, cancel]);

        assert_eq!(
            projection.agreement_state,
            RadrootsTradeAgreementStateV1::Contested
        );
        assert_eq!(
            projection.conflict_state,
            RadrootsTradeConflictStateV1::CancellationConflict
        );
    }

    #[test]
    fn reducer_tracks_private_terms_without_hiding_claims() {
        let mut root = proposal();
        if let TradeMutationBodyV1::Proposal { candidate } = &mut root.body {
            candidate.private_terms = Some(TradePrivateTermsRefV1 {
                artifact_id: "artifact-1".to_string(),
                schema_id: "radroots.private.fulfillment.v1".to_string(),
                ciphertext_commitment: hex_64('f'),
                required_acknowledgement: true,
            });
            candidate.fulfillment.requires_private_terms = true;
        }
        let root = canonical_trade_mutation_content(TradeMutationEnvelopeV1 {
            mutation_id: None,
            ..root
        })
        .unwrap()
        .envelope;
        let decision = accepted_decision(&root, '1');
        let projection = reduce(vec![root, decision]);

        assert_eq!(
            projection.agreement_state,
            RadrootsTradeAgreementStateV1::Agreed
        );
        assert_eq!(
            projection.private_terms_state,
            RadrootsTradePrivateTermsStateV1::Missing
        );
        assert_eq!(projection.active_agreement_claim_ids.len(), 1);
    }

    #[test]
    fn reducer_decline_and_expiry_are_negotiation_state_not_agreement_authority() {
        let proposal = proposal();
        let declined = declined_decision(&proposal);
        let declined_projection = reduce(vec![proposal.clone(), declined]);
        assert_eq!(
            declined_projection.negotiation_state,
            RadrootsTradeNegotiationStateV1::ClosedDeclined
        );
        assert_eq!(
            declined_projection.agreement_state,
            RadrootsTradeAgreementStateV1::None
        );

        let mut input = RadrootsTradeReductionInputV1::new(trade_id());
        input.mutations = vec![record(proposal)];
        input.observed_at_unix_s = Some(1_900_000_000);
        let expired_projection = reduce_trade_records(input);
        assert_eq!(
            expired_projection.negotiation_state,
            RadrootsTradeNegotiationStateV1::ClosedExpired
        );
        assert_eq!(
            expired_projection.agreement_state,
            RadrootsTradeAgreementStateV1::None
        );
    }

    #[test]
    fn reducer_classifies_malformed_unsupported_and_foreign_records() {
        let empty = reduce(Vec::new());
        assert_eq!(
            empty.negotiation_state,
            RadrootsTradeNegotiationStateV1::None
        );
        assert_eq!(empty.evidence_state, RadrootsTradeEvidenceStateV1::Missing);
        assert!(
            empty
                .issues
                .contains(&RadrootsTradeReducerIssueV1::MissingRootProposal)
        );

        let mut missing_id = proposal();
        missing_id.mutation_id = None;
        let mut unsupported = proposal();
        unsupported.schema_version += 1;
        let unsupported_id = root_id(&unsupported);
        let mut invalid = proposal();
        invalid.contract_id = "invalid.contract".to_string();
        let mut second_root = proposal();
        second_root.authored_at_unix_s += 1;
        let second_root = recanonicalize(second_root);
        let projection = reduce(vec![
            missing_id.clone(),
            missing_id,
            unsupported,
            invalid,
            proposal(),
            second_root,
        ]);
        assert!(
            projection
                .issues
                .contains(&RadrootsTradeReducerIssueV1::MissingMutationId)
        );
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsTradeReducerIssueV1::UnsupportedSchema { mutation_id, .. }
                if mutation_id == &unsupported_id
        )));
        assert!(
            projection
                .issues
                .iter()
                .any(|issue| matches!(issue, RadrootsTradeReducerIssueV1::InvalidMutation { .. }))
        );
        assert!(
            projection
                .issues
                .contains(&RadrootsTradeReducerIssueV1::MultipleRootProposals)
        );
        assert_eq!(
            projection.conflict_state,
            RadrootsTradeConflictStateV1::ConcurrentCandidates
        );

        let foreign_trade = TradeId::parse(hex_32('2')).expect("foreign trade");
        let mut input = RadrootsTradeReductionInputV1::new(foreign_trade);
        input.mutations = vec![record(proposal())];
        let foreign = reduce_trade_records(input);
        assert!(foreign.issues.iter().any(|issue| matches!(
            issue,
            RadrootsTradeReducerIssueV1::TradeIdentityMismatch { .. }
        )));
    }

    #[test]
    fn reducer_rejects_invalid_decision_relationships() {
        let root = proposal();

        let missing_proposal = reduce(vec![accepted_decision(&root, '1')]);
        assert!(
            missing_proposal
                .issues
                .iter()
                .any(|issue| matches!(issue, RadrootsTradeReducerIssueV1::MissingProposal { .. }))
        );

        let mut wrong_candidate = accepted_decision(&root, '1');
        if let TradeMutationBodyV1::Decision { candidate_id, .. } = &mut wrong_candidate.body {
            *candidate_id = CandidateId::parse(hex_64('f')).expect("candidate id");
        }
        let wrong_candidate = recanonicalize(wrong_candidate);
        let projection = reduce(vec![root.clone(), wrong_candidate]);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsTradeReducerIssueV1::CandidateIdMismatch { .. }
        )));

        let mut wrong_author = accepted_decision(&root, '2');
        wrong_author.author_pubkey = wrong_author.buyer_pubkey;
        wrong_author.counterparty_pubkey = wrong_author.seller_pubkey;
        let wrong_author = recanonicalize(wrong_author);
        let projection = reduce(vec![root.clone(), wrong_author]);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsTradeReducerIssueV1::DecisionAuthorMismatch { .. }
        )));

        let mut no_reservation = accepted_decision(&root, '3');
        if let TradeMutationBodyV1::Decision { decision, .. } = &mut no_reservation.body {
            *decision = TradeDecisionV1::Accepted {
                reservation_assertion: None,
            };
        }
        let no_reservation = recanonicalize(no_reservation);
        let projection = reduce(vec![root, no_reservation]);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsTradeReducerIssueV1::MissingSellerReservation { .. }
        )));
    }

    #[test]
    fn reducer_rejects_every_reservation_mismatch() {
        let root = proposal();
        let mut mismatched = accepted_decision(&root, '4');
        if let TradeMutationBodyV1::Decision {
            decision:
                TradeDecisionV1::Accepted {
                    reservation_assertion: Some(reservation),
                },
            ..
        } = &mut mismatched.body
        {
            reservation.candidate_id = CandidateId::parse(hex_64('f')).expect("candidate id");
            reservation.inventory_authority_id = pubkey('a');
            reservation.commitments[0].unit_code = "kg".to_string();
        }
        let mismatched = recanonicalize(mismatched);
        let projection = reduce(vec![root.clone(), mismatched]);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsTradeReducerIssueV1::ReservationCandidateMismatch { .. }
        )));
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsTradeReducerIssueV1::ReservationAuthorityMismatch { .. }
        )));
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsTradeReducerIssueV1::ReservationLineMismatch { .. }
        )));
        assert_eq!(
            projection.conflict_state,
            RadrootsTradeConflictStateV1::InventoryAuthorityConflict
        );

        let mut wrong_count = accepted_decision(&root, '5');
        if let TradeMutationBodyV1::Decision {
            decision:
                TradeDecisionV1::Accepted {
                    reservation_assertion: Some(reservation),
                },
            ..
        } = &mut wrong_count.body
        {
            let mut extra = reservation.commitments[0].clone();
            extra.line_id = dtag("line-2");
            reservation.commitments.push(extra);
        }
        let wrong_count = recanonicalize(wrong_count);
        let projection = reduce(vec![root, wrong_count]);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsTradeReducerIssueV1::ReservationLineMismatch { .. }
        )));
    }

    #[test]
    fn reducer_covers_decision_conflict_and_ordered_cancellation() {
        let root = proposal();
        let accepted = accepted_decision(&root, '1');
        let declined = declined_decision(&root);
        let conflicted = reduce(vec![root.clone(), accepted.clone(), declined]);
        assert_eq!(
            conflicted.conflict_state,
            RadrootsTradeConflictStateV1::DecisionConflict
        );
        assert!(
            conflicted
                .issues
                .iter()
                .any(|issue| matches!(issue, RadrootsTradeReducerIssueV1::DecisionConflict { .. }))
        );

        let cancel = cancellation(&root, root_id(&accepted), root_id(&accepted));
        let cancelled = reduce(vec![root, accepted.clone(), cancel]);
        assert_eq!(
            cancelled.agreement_state,
            RadrootsTradeAgreementStateV1::Cancelled
        );
        assert_eq!(cancelled.cancelled_claim_ids, vec![root_id(&accepted)]);
    }

    #[test]
    fn reducer_covers_pre_agreement_cancellation_and_requested_evidence() {
        let root = proposal();
        let cancel = candidate_cancellation(&root);
        let cancelled = reduce(vec![root.clone(), cancel]);
        assert_eq!(
            cancelled.agreement_state,
            RadrootsTradeAgreementStateV1::Cancelled
        );

        let decision = accepted_decision(&root, '1');
        let mut input = RadrootsTradeReductionInputV1::new(trade_id());
        input.mutations = vec![record(root), record(decision)];
        input.evidence_state = RadrootsTradeEvidenceStateV1::QueryPartial;
        assert_eq!(
            reduce_trade_records(input).evidence_state,
            RadrootsTradeEvidenceStateV1::QueryPartial
        );
    }

    #[test]
    fn reducer_covers_private_terms_and_attestation_precedence() {
        let mut root = proposal();
        if let TradeMutationBodyV1::Proposal { candidate } = &mut root.body {
            candidate.private_terms = Some(TradePrivateTermsRefV1 {
                artifact_id: "artifact-1".to_string(),
                schema_id: "radroots.private.fulfillment.v1".to_string(),
                ciphertext_commitment: hex_64('f'),
                required_acknowledgement: true,
            });
        }
        let root = recanonicalize(root);
        let candidate_id = match &root.body {
            TradeMutationBodyV1::Proposal { candidate } => {
                candidate.candidate_id.expect("candidate id")
            }
            _ => unreachable!(),
        };
        let decision = accepted_decision(&root, '1');
        for (state, expected) in [
            (
                RadrootsTradePrivateTermsStateV1::AvailableVerified,
                RadrootsTradePrivateTermsStateV1::AvailableVerified,
            ),
            (
                RadrootsTradePrivateTermsStateV1::Undecryptable,
                RadrootsTradePrivateTermsStateV1::Undecryptable,
            ),
            (
                RadrootsTradePrivateTermsStateV1::CommitmentMismatch,
                RadrootsTradePrivateTermsStateV1::CommitmentMismatch,
            ),
        ] {
            let mut input = RadrootsTradeReductionInputV1::new(trade_id());
            input.mutations = vec![record(root.clone()), record(decision.clone())];
            input.private_terms = vec![RadrootsTradePrivateTermsEvidenceV1 {
                candidate_id,
                state,
            }];
            assert_eq!(reduce_trade_records(input).private_terms_state, expected);
        }

        for (results, expected) in [
            (
                vec![RadrootsTradeAttestationResultV1::Valid],
                RadrootsTradeAttestationStateV1::PresentValid,
            ),
            (
                vec![
                    RadrootsTradeAttestationResultV1::Valid,
                    RadrootsTradeAttestationResultV1::Invalid,
                ],
                RadrootsTradeAttestationStateV1::Conflicting,
            ),
        ] {
            let mut input = RadrootsTradeReductionInputV1::new(trade_id());
            input.mutations = vec![record(root.clone()), record(decision.clone())];
            input.attestations = results
                .into_iter()
                .enumerate()
                .map(|(index, result)| RadrootsTradeAttestationRecordV1 {
                    event_id: event_id(if index == 0 { '8' } else { '9' }),
                    claim_mutation_id: root_id(&decision),
                    result,
                })
                .collect();
            assert_eq!(reduce_trade_records(input).attestation_state, expected);
        }
    }

    #[test]
    fn reducer_private_helpers_cover_empty_graph_and_conflict_precedence() {
        let missing = MutationId::parse(hex_64('e')).expect("mutation id");
        assert!(ancestors_of(&missing, &BTreeMap::new(), &mut BTreeMap::new()).is_empty());

        let mut conflict = RadrootsTradeConflictStateV1::DecisionConflict;
        set_conflict(
            &mut conflict,
            RadrootsTradeConflictStateV1::ConcurrentCandidates,
        );
        assert_eq!(conflict, RadrootsTradeConflictStateV1::DecisionConflict);
        set_conflict(
            &mut conflict,
            RadrootsTradeConflictStateV1::InvalidCausalChain,
        );
        assert_eq!(conflict, RadrootsTradeConflictStateV1::InvalidCausalChain);
    }

    #[test]
    fn reducer_rejects_self_identified_decision_author_bypass() {
        let root = proposal();
        let mut decision = accepted_decision(&root, '7');
        decision.author_pubkey = decision.buyer_pubkey;
        decision.counterparty_pubkey = decision.buyer_pubkey;
        let decision = recanonicalize(decision);

        let projection = reduce(vec![root, decision]);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsTradeReducerIssueV1::DecisionAuthorMismatch { .. }
        )));
    }

    #[test]
    fn reducer_covers_all_decline_and_counterparty_shapes() {
        let root = proposal();
        let first = declined_decision(&root);
        let mut second = first.clone();
        second.authored_at_unix_s += 1;
        if let TradeMutationBodyV1::Decision {
            decision: TradeDecisionV1::Declined { reason },
            ..
        } = &mut second.body
        {
            *reason = "still unavailable".to_string();
        }
        let second = recanonicalize(second);
        let projection = reduce(vec![root.clone(), first, second]);
        assert_eq!(
            projection.negotiation_state,
            RadrootsTradeNegotiationStateV1::ClosedDeclined
        );

        let revision = revision_proposal(&root, vec![root_id(&root)]);
        let mut revision_decline = declined_decision(&revision);
        let TradeMutationBodyV1::Decision {
            proposal_mutation_id,
            candidate_id,
            decision,
        } = revision_decline.body
        else {
            unreachable!();
        };
        revision_decline.contract_id = RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID.to_string();
        revision_decline.root_mutation_id = Some(root_id(&root));
        revision_decline.body = TradeMutationBodyV1::RevisionDecision {
            proposal_mutation_id,
            candidate_id,
            decision,
        };
        let revision_decline = recanonicalize(revision_decline);
        assert_eq!(
            declined_candidate_ids(&BTreeMap::from([(
                root_id(&revision_decline),
                revision_decline,
            )])),
            vec![candidate_id]
        );

        let candidate = match &root.body {
            TradeMutationBodyV1::Proposal { candidate } => candidate.clone(),
            _ => unreachable!(),
        };
        let mut candidate_record = CandidateRecord {
            proposal_mutation_id: root_id(&root),
            author_pubkey: root.buyer_pubkey,
            candidate,
        };
        let decision = accepted_decision(&root, '8');
        assert_eq!(
            candidate_record_author_counterparty(&candidate_record, &decision),
            decision.seller_pubkey
        );
        candidate_record.author_pubkey = decision.seller_pubkey;
        assert_eq!(
            candidate_record_author_counterparty(&candidate_record, &decision),
            decision.buyer_pubkey
        );
    }

    #[test]
    fn reservation_line_validation_checks_each_field() {
        let root = proposal();
        let candidate = match &root.body {
            TradeMutationBodyV1::Proposal { candidate } => candidate.clone(),
            _ => unreachable!(),
        };
        let candidate_id = candidate.candidate_id.expect("candidate id");
        let decision_mutation_id = root_id(&accepted_decision(&root, '9'));

        for field in 0..5 {
            let mut reservation = reservation(&candidate, '9');
            match field {
                0 => reservation.commitments[0].line_id = dtag("line-other"),
                1 => reservation.commitments[0].bin_id = bin_id("bin-other"),
                2 => reservation.commitments[0].quantity_mantissa = "3".to_string(),
                3 => reservation.commitments[0].quantity_scale = 1,
                4 => reservation.commitments[0].unit_code = "kg".to_string(),
                _ => unreachable!(),
            }
            let mut projection = RadrootsTradeProjectionV1::empty(trade_id());
            assert!(!validate_reservation(
                &decision_mutation_id,
                &candidate_id,
                &candidate,
                &reservation,
                &mut projection,
            ));
            assert!(projection.issues.iter().any(|issue| matches!(
                issue,
                RadrootsTradeReducerIssueV1::ReservationLineMismatch { .. }
            )));
        }
    }

    #[test]
    fn agreement_and_cancellation_helpers_cover_nonterminal_shapes() {
        let root = proposal();
        let first_decision = accepted_decision(&root, '1');
        let second_decision = accepted_decision(&root, '2');
        let first_id = root_id(&first_decision);
        let second_id = root_id(&second_decision);
        let candidate = match &root.body {
            TradeMutationBodyV1::Proposal { candidate } => candidate.clone(),
            _ => unreachable!(),
        };
        let candidate_id = candidate.candidate_id.expect("candidate id");
        let claim = |claim_mutation_id: MutationId| RadrootsTradeAgreementClaimV1 {
            claim_mutation_id,
            proposal_mutation_id: root_id(&root),
            candidate_id,
            candidate_author_pubkey: root.buyer_pubkey,
            accepted_by_pubkey: root.seller_pubkey,
            reservation_commitment: hex_64('a'),
        };
        let claims = BTreeMap::from([(first_id, claim(first_id)), (second_id, claim(second_id))]);
        let mutations = BTreeMap::from([(first_id, first_decision), (second_id, second_decision)]);
        let missing_claim = MutationId::parse(hex_64('e')).expect("missing claim");
        let cancellations = vec![
            CancellationRecord {
                mutation_id: MutationId::parse(hex_64('3')).expect("cancellation"),
                parent_mutation_ids: Vec::new(),
                target_candidate_id: None,
                target_claim_mutation_id: None,
            },
            CancellationRecord {
                mutation_id: MutationId::parse(hex_64('4')).expect("cancellation"),
                parent_mutation_ids: Vec::new(),
                target_candidate_id: None,
                target_claim_mutation_id: Some(missing_claim),
            },
            CancellationRecord {
                mutation_id: MutationId::parse(hex_64('5')).expect("cancellation"),
                parent_mutation_ids: vec![first_id],
                target_candidate_id: None,
                target_claim_mutation_id: Some(first_id),
            },
        ];
        let mut projection = RadrootsTradeProjectionV1::empty(trade_id());
        apply_agreement_state(
            &mut projection,
            &claims,
            &mutations,
            &BTreeMap::new(),
            &cancellations,
        );
        assert_eq!(
            projection.agreement_state,
            RadrootsTradeAgreementStateV1::Agreed
        );
        assert_eq!(projection.cancelled_claim_ids, vec![first_id]);

        let mut incompatible_claims = claims.clone();
        incompatible_claims
            .get_mut(&second_id)
            .expect("second claim")
            .candidate_id = CandidateId::parse(hex_64('f')).expect("candidate");
        let mut incompatible = RadrootsTradeProjectionV1::empty(trade_id());
        apply_agreement_state(
            &mut incompatible,
            &incompatible_claims,
            &mutations,
            &BTreeMap::new(),
            &[],
        );
        assert_eq!(
            incompatible.conflict_state,
            RadrootsTradeConflictStateV1::DecisionConflict
        );

        let candidates = BTreeMap::from([(
            root_id(&root),
            CandidateRecord {
                proposal_mutation_id: root_id(&root),
                author_pubkey: root.author_pubkey,
                candidate: candidate.clone(),
            },
        )]);
        let unknown_candidate = CandidateId::parse(hex_64('f')).expect("candidate");
        assert!(!cancellation_without_claim(
            &[CancellationRecord {
                mutation_id: MutationId::parse(hex_64('6')).expect("cancellation"),
                parent_mutation_ids: Vec::new(),
                target_candidate_id: Some(unknown_candidate),
                target_claim_mutation_id: None,
            }],
            &candidates,
        ));
        let mut disabled_candidate = candidate;
        disabled_candidate.cancellation.buyer_pre_agreement = false;
        let disabled_candidates = BTreeMap::from([(
            root_id(&root),
            CandidateRecord {
                proposal_mutation_id: root_id(&root),
                author_pubkey: root.author_pubkey,
                candidate: disabled_candidate,
            },
        )]);
        assert!(!cancellation_without_claim(
            &[CancellationRecord {
                mutation_id: MutationId::parse(hex_64('7')).expect("cancellation"),
                parent_mutation_ids: Vec::new(),
                target_candidate_id: Some(candidate_id),
                target_claim_mutation_id: None,
            }],
            &disabled_candidates,
        ));
    }

    #[test]
    fn evidence_state_covers_missing_proposal_independently() {
        let root = proposal();
        let mut projection = RadrootsTradeProjectionV1::empty(trade_id());
        projection.root_mutation_id = Some(root_id(&root));
        projection
            .missing_proposal_ids
            .push(MutationId::parse(hex_64('e')).expect("proposal"));
        assert_eq!(
            reduce_evidence_state(&projection, RadrootsTradeEvidenceStateV1::Complete),
            RadrootsTradeEvidenceStateV1::Missing
        );
    }

    #[test]
    fn passive_reducer_types_expose_every_governed_accessor() {
        let root = proposal();
        let mutation_id = root.mutation_id.expect("mutation id");
        let candidate_id = match &root.body {
            TradeMutationBodyV1::Proposal { candidate } => {
                candidate.candidate_id.expect("candidate")
            }
            _ => unreachable!(),
        };
        let transport_event_id = event_id('e');
        let record = RadrootsTradeMutationRecordV1::new(Some(transport_event_id), root.clone());
        assert_eq!(record.transport_event_id(), Some(&transport_event_id));
        assert_eq!(record.mutation(), &root);

        let private = RadrootsTradePrivateTermsEvidenceV1::new(
            candidate_id,
            RadrootsTradePrivateTermsStateV1::AvailableVerified,
        );
        assert_eq!(private.candidate_id(), &candidate_id);
        assert_eq!(
            private.state(),
            RadrootsTradePrivateTermsStateV1::AvailableVerified
        );
        let attestation = RadrootsTradeAttestationRecordV1::new(
            event_id('f'),
            mutation_id,
            RadrootsTradeAttestationResultV1::Valid,
        );
        assert_eq!(attestation.event_id(), &event_id('f'));
        assert_eq!(attestation.claim_mutation_id(), &mutation_id);
        assert_eq!(
            attestation.result(),
            RadrootsTradeAttestationResultV1::Valid
        );

        let input = RadrootsTradeReductionInputV1::new(trade_id())
            .with_mutations(vec![record])
            .with_private_terms(vec![private])
            .with_attestations(vec![attestation])
            .with_evidence_state(RadrootsTradeEvidenceStateV1::QueryPartial)
            .with_observed_at_unix_s(Some(123));
        assert_eq!(input.trade_id(), &trade_id());
        assert_eq!(input.mutations().len(), 1);
        assert_eq!(input.private_terms().len(), 1);
        assert_eq!(input.attestations().len(), 1);
        assert_eq!(
            input.evidence_state(),
            RadrootsTradeEvidenceStateV1::QueryPartial
        );
        assert_eq!(input.observed_at_unix_s(), Some(123));

        let projection = reduce_trade_records(input);
        assert_eq!(
            projection.reducer_contract_id(),
            RADROOTS_TRADE_REDUCER_CONTRACT_ID
        );
        assert_eq!(projection.reducer_version(), RADROOTS_TRADE_REDUCER_VERSION);
        assert_eq!(projection.trade_id(), &trade_id());
        let _ = projection.root_mutation_id();
        let _ = projection.buyer_pubkey();
        let _ = projection.seller_pubkey();
        let _ = projection.farm_id();
        let _ = projection.negotiation_state();
        let _ = projection.agreement_state();
        let _ = projection.evidence_state();
        let _ = projection.conflict_state();
        let _ = projection.private_terms_state();
        let _ = projection.attestation_state();
        let _ = projection.fulfillment_state();
        let _ = projection.payment_state();
        let _ = projection.candidate_heads();
        let _ = projection.agreement_claims();
        let _ = projection.active_agreement_claim_ids();
        let _ = projection.contested_claim_ids();
        let _ = projection.cancelled_claim_ids();
        let _ = projection.declined_candidate_ids();
        let _ = projection.missing_parent_ids();
        let _ = projection.missing_proposal_ids();
        let _ = projection.unsupported_mutation_ids();
        let _ = projection.issues();
        let _ = projection.attestations();
        assert!(!projection.projection_digest().is_empty());
    }
}

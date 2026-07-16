#![forbid(unsafe_code)]

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
    ids::{
        RadrootsDTag, RadrootsEventId, RadrootsPublicKey, RadrootsTradeCandidateId,
        RadrootsTradeId, RadrootsTradeMutationId,
    },
    trade::{
        RADROOTS_TRADE_SCHEMA_VERSION, RadrootsSellerReservationAssertionV1,
        RadrootsTradeCandidateTermsV1, RadrootsTradeDecisionV1, RadrootsTradeMutationBodyV1,
        RadrootsTradeMutationEnvelopeV1,
    },
};
#[cfg(feature = "serde_json")]
use sha2::{Digest, Sha256};

pub const RADROOTS_TRADE_REDUCER_CONTRACT_ID: &str = "radroots.trade.reducer.v1";
pub const RADROOTS_TRADE_REDUCER_VERSION: u16 = 1;
#[cfg(feature = "serde_json")]
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
    pub trade_id: RadrootsTradeId,
    pub mutations: Vec<RadrootsTradeMutationRecordV1>,
    pub private_terms: Vec<RadrootsTradePrivateTermsEvidenceV1>,
    pub attestations: Vec<RadrootsTradeAttestationRecordV1>,
    pub evidence_state: RadrootsTradeEvidenceStateV1,
    pub observed_at_unix_s: Option<u64>,
}

impl RadrootsTradeReductionInputV1 {
    pub fn new(trade_id: RadrootsTradeId) -> Self {
        Self {
            trade_id,
            mutations: Vec::new(),
            private_terms: Vec::new(),
            attestations: Vec::new(),
            evidence_state: RadrootsTradeEvidenceStateV1::Complete,
            observed_at_unix_s: None,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeMutationRecordV1 {
    pub transport_event_id: Option<RadrootsEventId>,
    pub mutation: RadrootsTradeMutationEnvelopeV1,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradePrivateTermsEvidenceV1 {
    pub candidate_id: RadrootsTradeCandidateId,
    pub state: RadrootsTradePrivateTermsStateV1,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeAttestationRecordV1 {
    pub event_id: RadrootsEventId,
    pub claim_mutation_id: RadrootsTradeMutationId,
    pub result: RadrootsTradeAttestationResultV1,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeProjectionV1 {
    pub reducer_contract_id: String,
    pub reducer_version: u16,
    pub trade_id: RadrootsTradeId,
    pub root_mutation_id: Option<RadrootsTradeMutationId>,
    pub buyer_pubkey: Option<RadrootsPublicKey>,
    pub seller_pubkey: Option<RadrootsPublicKey>,
    pub farm_id: Option<RadrootsDTag>,
    pub negotiation_state: RadrootsTradeNegotiationStateV1,
    pub agreement_state: RadrootsTradeAgreementStateV1,
    pub evidence_state: RadrootsTradeEvidenceStateV1,
    pub conflict_state: RadrootsTradeConflictStateV1,
    pub private_terms_state: RadrootsTradePrivateTermsStateV1,
    pub attestation_state: RadrootsTradeAttestationStateV1,
    pub fulfillment_state: RadrootsTradeFulfillmentStateV1,
    pub payment_state: RadrootsTradePaymentStateV1,
    pub candidate_heads: Vec<RadrootsTradeMutationId>,
    pub agreement_claims: Vec<RadrootsTradeAgreementClaimV1>,
    pub active_agreement_claim_ids: Vec<RadrootsTradeMutationId>,
    pub contested_claim_ids: Vec<RadrootsTradeMutationId>,
    pub cancelled_claim_ids: Vec<RadrootsTradeMutationId>,
    pub declined_candidate_ids: Vec<RadrootsTradeCandidateId>,
    pub missing_parent_ids: Vec<RadrootsTradeMutationId>,
    pub missing_proposal_ids: Vec<RadrootsTradeMutationId>,
    pub unsupported_mutation_ids: Vec<RadrootsTradeMutationId>,
    pub issues: Vec<RadrootsTradeReducerIssueV1>,
    pub attestations: Vec<RadrootsTradeAttestationRecordV1>,
    pub projection_digest: String,
}

impl RadrootsTradeProjectionV1 {
    fn empty(trade_id: RadrootsTradeId) -> Self {
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
            .sort_by(|left, right| left.claim_mutation_id.cmp(&right.claim_mutation_id));
        self.issues.sort();
        self.issues.dedup();
        self.attestations
            .sort_by(|left, right| left.event_id.cmp(&right.event_id));
        self.projection_digest = projection_digest(self);
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RadrootsTradeAgreementClaimV1 {
    pub claim_mutation_id: RadrootsTradeMutationId,
    pub proposal_mutation_id: RadrootsTradeMutationId,
    pub candidate_id: RadrootsTradeCandidateId,
    pub candidate_author_pubkey: RadrootsPublicKey,
    pub accepted_by_pubkey: RadrootsPublicKey,
    pub reservation_commitment: String,
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
        mutation_id: RadrootsTradeMutationId,
    },
    UnsupportedSchema {
        mutation_id: RadrootsTradeMutationId,
        schema_version: u16,
    },
    InvalidMutation {
        mutation_id: Option<RadrootsTradeMutationId>,
        reason: String,
    },
    MissingParent {
        mutation_id: RadrootsTradeMutationId,
        parent_mutation_id: RadrootsTradeMutationId,
    },
    MissingProposal {
        decision_mutation_id: RadrootsTradeMutationId,
        proposal_mutation_id: RadrootsTradeMutationId,
    },
    CandidateIdMismatch {
        decision_mutation_id: RadrootsTradeMutationId,
        proposal_mutation_id: RadrootsTradeMutationId,
    },
    DecisionAuthorMismatch {
        decision_mutation_id: RadrootsTradeMutationId,
    },
    DecisionParentMissing {
        decision_mutation_id: RadrootsTradeMutationId,
        proposal_mutation_id: RadrootsTradeMutationId,
    },
    MissingSellerReservation {
        decision_mutation_id: RadrootsTradeMutationId,
    },
    ReservationCandidateMismatch {
        decision_mutation_id: RadrootsTradeMutationId,
    },
    ReservationAuthorityMismatch {
        decision_mutation_id: RadrootsTradeMutationId,
    },
    ReservationLineMismatch {
        decision_mutation_id: RadrootsTradeMutationId,
    },
    DecisionConflict {
        proposal_mutation_id: RadrootsTradeMutationId,
    },
    DoubleAcceptance {
        proposal_mutation_id: RadrootsTradeMutationId,
    },
    CancellationConflict {
        cancellation_mutation_id: RadrootsTradeMutationId,
    },
    InvalidCausalChain {
        mutation_id: RadrootsTradeMutationId,
    },
    PrivateTermsUnavailable {
        candidate_id: RadrootsTradeCandidateId,
    },
    ProjectionDigestUnavailable {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CandidateRecord {
    proposal_mutation_id: RadrootsTradeMutationId,
    author_pubkey: RadrootsPublicKey,
    candidate: RadrootsTradeCandidateTermsV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CancellationRecord {
    mutation_id: RadrootsTradeMutationId,
    parent_mutation_ids: Vec<RadrootsTradeMutationId>,
    target_candidate_id: Option<RadrootsTradeCandidateId>,
    target_claim_mutation_id: Option<RadrootsTradeMutationId>,
}

pub fn reduce_trade_records(input: RadrootsTradeReductionInputV1) -> RadrootsTradeProjectionV1 {
    let mut projection = RadrootsTradeProjectionV1::empty(input.trade_id.clone());
    let mut mutations = BTreeMap::<RadrootsTradeMutationId, RadrootsTradeMutationEnvelopeV1>::new();

    for record in input.mutations {
        let mutation_id = match record.mutation.mutation_id.clone() {
            Some(mutation_id) => mutation_id,
            None => {
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::MissingMutationId);
                continue;
            }
        };
        if record.mutation.schema_version != RADROOTS_TRADE_SCHEMA_VERSION {
            projection
                .unsupported_mutation_ids
                .push(mutation_id.clone());
            projection
                .issues
                .push(RadrootsTradeReducerIssueV1::UnsupportedSchema {
                    mutation_id: mutation_id.clone(),
                    schema_version: record.mutation.schema_version,
                });
            mutations.entry(mutation_id).or_insert(record.mutation);
            continue;
        }
        if let Err(error) = record.mutation.validate() {
            projection
                .issues
                .push(RadrootsTradeReducerIssueV1::InvalidMutation {
                    mutation_id: Some(mutation_id.clone()),
                    reason: error.to_string(),
                });
            continue;
        }
        if record.mutation.trade_id != input.trade_id {
            projection
                .issues
                .push(RadrootsTradeReducerIssueV1::TradeIdentityMismatch {
                    mutation_id: mutation_id.clone(),
                });
            continue;
        }
        mutations.entry(mutation_id).or_insert(record.mutation);
    }

    let mut root_proposals = Vec::<RadrootsTradeMutationId>::new();
    let mut candidates_by_proposal = BTreeMap::<RadrootsTradeMutationId, CandidateRecord>::new();
    let mut claims = BTreeMap::<RadrootsTradeMutationId, RadrootsTradeAgreementClaimV1>::new();
    let mut decisions_by_proposal =
        BTreeMap::<RadrootsTradeMutationId, Vec<RadrootsTradeMutationId>>::new();
    let mut decision_ids = Vec::<RadrootsTradeMutationId>::new();
    let mut cancellations = Vec::<CancellationRecord>::new();
    let mut referenced_parents = BTreeSet::<RadrootsTradeMutationId>::new();

    for (mutation_id, mutation) in &mutations {
        if mutation.parent_mutation_ids.is_empty()
            && matches!(mutation.body, RadrootsTradeMutationBodyV1::Proposal { .. })
        {
            root_proposals.push(mutation_id.clone());
        }
        for parent in &mutation.parent_mutation_ids {
            referenced_parents.insert(parent.clone());
            if !mutations.contains_key(parent) {
                projection.missing_parent_ids.push(parent.clone());
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::MissingParent {
                        mutation_id: mutation_id.clone(),
                        parent_mutation_id: parent.clone(),
                    });
            }
        }
        match &mutation.body {
            RadrootsTradeMutationBodyV1::Proposal { candidate }
            | RadrootsTradeMutationBodyV1::RevisionProposal { candidate } => {
                if let Some(candidate_id) = candidate.candidate_id.clone() {
                    candidates_by_proposal.insert(
                        mutation_id.clone(),
                        CandidateRecord {
                            proposal_mutation_id: mutation_id.clone(),
                            author_pubkey: mutation.author_pubkey.clone(),
                            candidate: candidate.clone(),
                        },
                    );
                    let _ = candidate_id;
                }
            }
            RadrootsTradeMutationBodyV1::Decision {
                proposal_mutation_id,
                candidate_id,
                decision,
            }
            | RadrootsTradeMutationBodyV1::RevisionDecision {
                proposal_mutation_id,
                candidate_id,
                decision,
            } => {
                let _ = (proposal_mutation_id, candidate_id, decision);
                decision_ids.push(mutation_id.clone());
            }
            RadrootsTradeMutationBodyV1::Cancellation {
                target_candidate_id,
                target_claim_mutation_id,
                reason: _,
            } => cancellations.push(CancellationRecord {
                mutation_id: mutation_id.clone(),
                parent_mutation_ids: mutation.parent_mutation_ids.clone(),
                target_candidate_id: target_candidate_id.clone(),
                target_claim_mutation_id: target_claim_mutation_id.clone(),
            }),
        }
    }

    for mutation_id in decision_ids {
        let Some(mutation) = mutations.get(&mutation_id) else {
            continue;
        };
        match &mutation.body {
            RadrootsTradeMutationBodyV1::Decision {
                proposal_mutation_id,
                candidate_id,
                decision,
            }
            | RadrootsTradeMutationBodyV1::RevisionDecision {
                proposal_mutation_id,
                candidate_id,
                decision,
            } => {
                decisions_by_proposal
                    .entry(proposal_mutation_id.clone())
                    .or_default()
                    .push(mutation_id.clone());
                apply_decision(
                    &mutation_id,
                    mutation,
                    proposal_mutation_id,
                    candidate_id,
                    decision,
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
    } else {
        projection.root_mutation_id = root_proposals.first().cloned();
        if let Some(root) = projection
            .root_mutation_id
            .as_ref()
            .and_then(|root_id| mutations.get(root_id))
        {
            projection.buyer_pubkey = Some(root.buyer_pubkey.clone());
            projection.seller_pubkey = Some(root.seller_pubkey.clone());
            projection.farm_id = Some(root.farm_id.clone());
        }
    }

    for (proposal_mutation_id, decision_ids) in &decisions_by_proposal {
        let unique: BTreeSet<RadrootsTradeMutationId> = decision_ids.iter().cloned().collect();
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
                        proposal_mutation_id: proposal_mutation_id.clone(),
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
                        proposal_mutation_id: proposal_mutation_id.clone(),
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
            if let Some(claim) = claims.get(claim_id) {
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::PrivateTermsUnavailable {
                        candidate_id: claim.candidate_id.clone(),
                    });
            }
        }
    }
    projection.attestations = input.attestations;
    projection.attestation_state = reduce_attestation_state(&projection.attestations);
    projection.evidence_state = reduce_evidence_state(&projection, input.evidence_state);
    projection.finish();
    projection
}

fn apply_decision(
    mutation_id: &RadrootsTradeMutationId,
    mutation: &RadrootsTradeMutationEnvelopeV1,
    proposal_mutation_id: &RadrootsTradeMutationId,
    candidate_id: &RadrootsTradeCandidateId,
    decision: &RadrootsTradeDecisionV1,
    candidates_by_proposal: &BTreeMap<RadrootsTradeMutationId, CandidateRecord>,
    claims: &mut BTreeMap<RadrootsTradeMutationId, RadrootsTradeAgreementClaimV1>,
    projection: &mut RadrootsTradeProjectionV1,
) {
    let Some(candidate_record) = candidates_by_proposal.get(proposal_mutation_id) else {
        projection
            .missing_proposal_ids
            .push(proposal_mutation_id.clone());
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::MissingProposal {
                decision_mutation_id: mutation_id.clone(),
                proposal_mutation_id: proposal_mutation_id.clone(),
            });
        return;
    };
    if candidate_record.candidate.candidate_id.as_ref() != Some(candidate_id) {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::CandidateIdMismatch {
                decision_mutation_id: mutation_id.clone(),
                proposal_mutation_id: proposal_mutation_id.clone(),
            });
        set_conflict(
            &mut projection.conflict_state,
            RadrootsTradeConflictStateV1::DecisionConflict,
        );
        return;
    }
    if mutation.author_pubkey != mutation.counterparty_pubkey
        && mutation.author_pubkey
            != candidate_record_author_counterparty(candidate_record, mutation)
    {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::DecisionAuthorMismatch {
                decision_mutation_id: mutation_id.clone(),
            });
        return;
    }
    if !mutation.parent_mutation_ids.contains(proposal_mutation_id) {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::DecisionParentMissing {
                decision_mutation_id: mutation_id.clone(),
                proposal_mutation_id: proposal_mutation_id.clone(),
            });
        set_conflict(
            &mut projection.conflict_state,
            RadrootsTradeConflictStateV1::InvalidCausalChain,
        );
    }
    match decision {
        RadrootsTradeDecisionV1::Accepted {
            reservation_assertion,
        } => {
            let Some(reservation) = reservation_assertion else {
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::MissingSellerReservation {
                        decision_mutation_id: mutation_id.clone(),
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
                    mutation_id.clone(),
                    RadrootsTradeAgreementClaimV1 {
                        claim_mutation_id: mutation_id.clone(),
                        proposal_mutation_id: candidate_record.proposal_mutation_id.clone(),
                        candidate_id: candidate_id.clone(),
                        candidate_author_pubkey: candidate_record.author_pubkey.clone(),
                        accepted_by_pubkey: mutation.author_pubkey.clone(),
                        reservation_commitment: reservation.assertion_commitment.clone(),
                    },
                );
            }
        }
        RadrootsTradeDecisionV1::Declined { .. } => {}
    }
}

fn candidate_record_author_counterparty(
    candidate_record: &CandidateRecord,
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> RadrootsPublicKey {
    if candidate_record.author_pubkey == mutation.buyer_pubkey {
        mutation.seller_pubkey.clone()
    } else {
        mutation.buyer_pubkey.clone()
    }
}

fn validate_reservation(
    decision_mutation_id: &RadrootsTradeMutationId,
    candidate_id: &RadrootsTradeCandidateId,
    candidate: &RadrootsTradeCandidateTermsV1,
    reservation: &RadrootsSellerReservationAssertionV1,
    projection: &mut RadrootsTradeProjectionV1,
) -> bool {
    let mut valid = true;
    if &reservation.candidate_id != candidate_id {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::ReservationCandidateMismatch {
                decision_mutation_id: decision_mutation_id.clone(),
            });
        valid = false;
    }
    if reservation.inventory_authority_id != candidate.seller_pubkey {
        projection
            .issues
            .push(RadrootsTradeReducerIssueV1::ReservationAuthorityMismatch {
                decision_mutation_id: decision_mutation_id.clone(),
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
                decision_mutation_id: decision_mutation_id.clone(),
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
                    decision_mutation_id: decision_mutation_id.clone(),
                });
            valid = false;
            break;
        }
    }
    valid
}

fn apply_agreement_state(
    projection: &mut RadrootsTradeProjectionV1,
    claims: &BTreeMap<RadrootsTradeMutationId, RadrootsTradeAgreementClaimV1>,
    mutations: &BTreeMap<RadrootsTradeMutationId, RadrootsTradeMutationEnvelopeV1>,
    candidates_by_proposal: &BTreeMap<RadrootsTradeMutationId, CandidateRecord>,
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
                projection.cancelled_claim_ids.push(target_claim_id.clone());
                if projection.active_agreement_claim_ids == [target_claim_id.clone()] {
                    projection.agreement_state = RadrootsTradeAgreementStateV1::Cancelled;
                }
            } else {
                projection
                    .issues
                    .push(RadrootsTradeReducerIssueV1::CancellationConflict {
                        cancellation_mutation_id: cancellation.mutation_id.clone(),
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
    candidates_by_proposal: &BTreeMap<RadrootsTradeMutationId, CandidateRecord>,
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
    claims: &BTreeMap<RadrootsTradeMutationId, RadrootsTradeAgreementClaimV1>,
    mutations: &BTreeMap<RadrootsTradeMutationId, RadrootsTradeMutationEnvelopeV1>,
) -> Vec<RadrootsTradeMutationId> {
    let mut memo = BTreeMap::<RadrootsTradeMutationId, BTreeSet<RadrootsTradeMutationId>>::new();
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
    let Some(first) = claims.first() else {
        return false;
    };
    claims.iter().all(|claim| {
        claim.candidate_id == first.candidate_id
            && claim.reservation_commitment == first.reservation_commitment
    })
}

fn ancestors_of(
    mutation_id: &RadrootsTradeMutationId,
    mutations: &BTreeMap<RadrootsTradeMutationId, RadrootsTradeMutationEnvelopeV1>,
    memo: &mut BTreeMap<RadrootsTradeMutationId, BTreeSet<RadrootsTradeMutationId>>,
) -> BTreeSet<RadrootsTradeMutationId> {
    if let Some(cached) = memo.get(mutation_id) {
        return cached.clone();
    }
    let mut ancestors = BTreeSet::new();
    if let Some(mutation) = mutations.get(mutation_id) {
        for parent in &mutation.parent_mutation_ids {
            ancestors.insert(parent.clone());
            ancestors.extend(ancestors_of(parent, mutations, memo));
        }
    }
    memo.insert(mutation_id.clone(), ancestors.clone());
    ancestors
}

fn apply_negotiation_state(
    projection: &mut RadrootsTradeProjectionV1,
    candidates_by_proposal: &BTreeMap<RadrootsTradeMutationId, CandidateRecord>,
    claims: &BTreeMap<RadrootsTradeMutationId, RadrootsTradeAgreementClaimV1>,
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
    candidates_by_proposal: &BTreeMap<RadrootsTradeMutationId, CandidateRecord>,
    private_terms: &[RadrootsTradePrivateTermsEvidenceV1],
) -> RadrootsTradePrivateTermsStateV1 {
    let private_terms_by_candidate = private_terms
        .iter()
        .map(|record| (record.candidate_id.clone(), record.state))
        .collect::<BTreeMap<_, _>>();
    let mut required_states = Vec::new();
    for claim in &projection.agreement_claims {
        let Some(candidate_record) = candidates_by_proposal.get(&claim.proposal_mutation_id) else {
            continue;
        };
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
    } else if required_states
        .iter()
        .any(|state| *state == RadrootsTradePrivateTermsStateV1::CommitmentMismatch)
    {
        RadrootsTradePrivateTermsStateV1::CommitmentMismatch
    } else if required_states
        .iter()
        .any(|state| *state == RadrootsTradePrivateTermsStateV1::Undecryptable)
    {
        RadrootsTradePrivateTermsStateV1::Undecryptable
    } else if required_states
        .iter()
        .any(|state| *state == RadrootsTradePrivateTermsStateV1::Missing)
    {
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
    mutations: &BTreeMap<RadrootsTradeMutationId, RadrootsTradeMutationEnvelopeV1>,
) -> Vec<RadrootsTradeCandidateId> {
    mutations
        .values()
        .filter_map(|mutation| match &mutation.body {
            RadrootsTradeMutationBodyV1::Decision {
                candidate_id,
                decision: RadrootsTradeDecisionV1::Declined { .. },
                ..
            }
            | RadrootsTradeMutationBodyV1::RevisionDecision {
                candidate_id,
                decision: RadrootsTradeDecisionV1::Declined { .. },
                ..
            } => Some(candidate_id.clone()),
            _ => None,
        })
        .collect()
}

fn is_acceptance(body: &RadrootsTradeMutationBodyV1) -> bool {
    matches!(
        body,
        RadrootsTradeMutationBodyV1::Decision {
            decision: RadrootsTradeDecisionV1::Accepted { .. },
            ..
        } | RadrootsTradeMutationBodyV1::RevisionDecision {
            decision: RadrootsTradeDecisionV1::Accepted { .. },
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

#[cfg(feature = "serde_json")]
fn projection_digest(projection: &RadrootsTradeProjectionV1) -> String {
    let mut digest_input = projection.clone();
    digest_input.projection_digest.clear();
    let value = match serde_json::to_value(&digest_input) {
        Ok(value) => value,
        Err(error) => {
            let mut hasher = Sha256::new();
            hasher.update(RADROOTS_TRADE_PROJECTION_DIGEST_DOMAIN);
            hasher.update(error.to_string().as_bytes());
            return hex::encode(hasher.finalize());
        }
    };
    let canonical = match radroots_event::trade::canonical_jcs_value(&value) {
        Ok(canonical) => canonical,
        Err(error) => {
            let mut hasher = Sha256::new();
            hasher.update(RADROOTS_TRADE_PROJECTION_DIGEST_DOMAIN);
            hasher.update(error.to_string().as_bytes());
            return hex::encode(hasher.finalize());
        }
    };
    let mut hasher = Sha256::new();
    hasher.update(RADROOTS_TRADE_PROJECTION_DIGEST_DOMAIN);
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(not(feature = "serde_json"))]
fn projection_digest(_projection: &RadrootsTradeProjectionV1) -> String {
    String::new()
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use radroots_event::{
        ids::{RadrootsAddressableCoordinate, RadrootsInventoryBinId},
        trade::{
            RADROOTS_TRADE_DECISION_CONTRACT_ID, RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
            RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID,
            RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID, RadrootsFulfillmentProfileV1,
            RadrootsSellerReservationLineV1, RadrootsTradeCancellationProfileV1,
            RadrootsTradeCandidateLineV1, RadrootsTradeEconomicAdjustmentV1,
            RadrootsTradeEconomicsProfileV1, RadrootsTradeLineTombstoneV1,
            RadrootsTradePrivateTermsRefV1, canonical_trade_mutation_content,
        },
    };

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

    fn dtag(value: &str) -> RadrootsDTag {
        RadrootsDTag::parse(value).unwrap()
    }

    fn bin_id(value: &str) -> RadrootsInventoryBinId {
        RadrootsInventoryBinId::parse(value).unwrap()
    }

    fn candidate(line_suffix: &str) -> RadrootsTradeCandidateTermsV1 {
        RadrootsTradeCandidateTermsV1 {
            candidate_id: None,
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            base_candidate_id: None,
            supersession_intent: None,
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            lines: vec![RadrootsTradeCandidateLineV1 {
                line_id: dtag(&format!("line-{line_suffix}")),
                listing_addr: RadrootsAddressableCoordinate::parse(format!(
                    "30402:{}:listing-{line_suffix}",
                    hex_64('b')
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
            line_tombstones: Vec::<RadrootsTradeLineTombstoneV1>::new(),
            economics: RadrootsTradeEconomicsProfileV1 {
                profile_id: "mvp-fixed".to_string(),
                currency_code: "USD".to_string(),
                currency_exponent: 2,
                rounding_profile: "half-even".to_string(),
                subtotal_mantissa: "1000".to_string(),
                discount_total_mantissa: "0".to_string(),
                adjustment_total_mantissa: "0".to_string(),
                total_mantissa: "1000".to_string(),
                adjustments: Vec::<RadrootsTradeEconomicAdjustmentV1>::new(),
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
                post_agreement_cutoff_unix_s: Some(1_799_990_000),
            },
            private_terms: None,
            proposal_expires_at_unix_s: 1_799_999_000,
        }
    }

    fn proposal() -> RadrootsTradeMutationEnvelopeV1 {
        canonical_trade_mutation_content(RadrootsTradeMutationEnvelopeV1 {
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
            body: RadrootsTradeMutationBodyV1::Proposal {
                candidate: candidate("1"),
            },
        })
        .unwrap()
        .envelope
    }

    fn reservation(
        candidate: &RadrootsTradeCandidateTermsV1,
        marker: char,
    ) -> RadrootsSellerReservationAssertionV1 {
        RadrootsSellerReservationAssertionV1 {
            reservation_id: dtag(&format!("reservation-{marker}")),
            inventory_authority_id: candidate.seller_pubkey.clone(),
            inventory_epoch: 42,
            candidate_id: candidate.candidate_id.clone().unwrap(),
            commitments: candidate
                .lines
                .iter()
                .map(|line| RadrootsSellerReservationLineV1 {
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
        proposal: &RadrootsTradeMutationEnvelopeV1,
        marker: char,
    ) -> RadrootsTradeMutationEnvelopeV1 {
        let proposal_id = proposal.mutation_id.clone().unwrap();
        let candidate = match &proposal.body {
            RadrootsTradeMutationBodyV1::Proposal { candidate }
            | RadrootsTradeMutationBodyV1::RevisionProposal { candidate } => candidate.clone(),
            _ => unreachable!(),
        };
        canonical_trade_mutation_content(RadrootsTradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_DECISION_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: Some(root_id(proposal)),
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            parent_mutation_ids: vec![proposal_id.clone()],
            author_pubkey: pubkey('b'),
            counterparty_pubkey: pubkey('a'),
            authored_at_unix_s: u64::from(marker),
            body: RadrootsTradeMutationBodyV1::Decision {
                proposal_mutation_id: proposal_id,
                candidate_id: candidate.candidate_id.clone().unwrap(),
                decision: RadrootsTradeDecisionV1::Accepted {
                    reservation_assertion: Some(reservation(&candidate, marker)),
                },
            },
        })
        .unwrap()
        .envelope
    }

    fn declined_decision(
        proposal: &RadrootsTradeMutationEnvelopeV1,
    ) -> RadrootsTradeMutationEnvelopeV1 {
        let proposal_id = proposal.mutation_id.clone().unwrap();
        let candidate = match &proposal.body {
            RadrootsTradeMutationBodyV1::Proposal { candidate } => candidate.clone(),
            _ => unreachable!(),
        };
        canonical_trade_mutation_content(RadrootsTradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_DECISION_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: Some(root_id(proposal)),
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            parent_mutation_ids: vec![proposal_id.clone()],
            author_pubkey: pubkey('b'),
            counterparty_pubkey: pubkey('a'),
            authored_at_unix_s: 102,
            body: RadrootsTradeMutationBodyV1::Decision {
                proposal_mutation_id: proposal_id,
                candidate_id: candidate.candidate_id.clone().unwrap(),
                decision: RadrootsTradeDecisionV1::Declined {
                    reason: "unavailable".to_string(),
                },
            },
        })
        .unwrap()
        .envelope
    }

    fn revision_proposal(
        root: &RadrootsTradeMutationEnvelopeV1,
        parents: Vec<RadrootsTradeMutationId>,
    ) -> RadrootsTradeMutationEnvelopeV1 {
        let mut parents = parents;
        parents.sort();
        canonical_trade_mutation_content(RadrootsTradeMutationEnvelopeV1 {
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
            body: RadrootsTradeMutationBodyV1::RevisionProposal {
                candidate: candidate("2"),
            },
        })
        .unwrap()
        .envelope
    }

    fn revision_acceptance(
        root: &RadrootsTradeMutationEnvelopeV1,
        proposal: &RadrootsTradeMutationEnvelopeV1,
    ) -> RadrootsTradeMutationEnvelopeV1 {
        let proposal_id = proposal.mutation_id.clone().unwrap();
        let candidate = match &proposal.body {
            RadrootsTradeMutationBodyV1::RevisionProposal { candidate } => candidate.clone(),
            _ => unreachable!(),
        };
        canonical_trade_mutation_content(RadrootsTradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID.to_string(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: Some(root_id(root)),
            buyer_pubkey: pubkey('a'),
            seller_pubkey: pubkey('b'),
            farm_id: dtag("farm-1"),
            parent_mutation_ids: vec![proposal_id.clone()],
            author_pubkey: pubkey('b'),
            counterparty_pubkey: pubkey('a'),
            authored_at_unix_s: 201,
            body: RadrootsTradeMutationBodyV1::RevisionDecision {
                proposal_mutation_id: proposal_id,
                candidate_id: candidate.candidate_id.clone().unwrap(),
                decision: RadrootsTradeDecisionV1::Accepted {
                    reservation_assertion: Some(reservation(&candidate, '9')),
                },
            },
        })
        .unwrap()
        .envelope
    }

    fn cancellation(
        root: &RadrootsTradeMutationEnvelopeV1,
        target_claim: RadrootsTradeMutationId,
        parent: RadrootsTradeMutationId,
    ) -> RadrootsTradeMutationEnvelopeV1 {
        canonical_trade_mutation_content(RadrootsTradeMutationEnvelopeV1 {
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
            body: RadrootsTradeMutationBodyV1::Cancellation {
                target_candidate_id: None,
                target_claim_mutation_id: Some(target_claim),
                reason: "before cutoff".to_string(),
            },
        })
        .unwrap()
        .envelope
    }

    fn root_id(envelope: &RadrootsTradeMutationEnvelopeV1) -> RadrootsTradeMutationId {
        envelope.mutation_id.clone().unwrap()
    }

    fn record(mutation: RadrootsTradeMutationEnvelopeV1) -> RadrootsTradeMutationRecordV1 {
        RadrootsTradeMutationRecordV1 {
            transport_event_id: None,
            mutation,
        }
    }

    fn reduce(mutations: Vec<RadrootsTradeMutationEnvelopeV1>) -> RadrootsTradeProjectionV1 {
        let mut input = RadrootsTradeReductionInputV1::new(trade_id());
        input.mutations = mutations.into_iter().map(record).collect();
        reduce_trade_records(input)
    }

    #[test]
    fn reducer_digest_is_independent_of_input_order_and_duplicates() {
        let proposal = proposal();
        let decision = accepted_decision(&proposal, '1');
        let first = reduce(vec![proposal.clone(), decision.clone(), decision.clone()]);
        let second = reduce(vec![decision, proposal]);

        assert_eq!(first.agreement_state, RadrootsTradeAgreementStateV1::Agreed);
        assert_eq!(first.projection_digest, second.projection_digest);
        assert_eq!(first.active_agreement_claim_ids.len(), 1);
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
        let missing_parent = RadrootsTradeMutationId::parse(hex_64('e')).unwrap();
        decision.parent_mutation_ids = vec![missing_parent.clone()];
        let decision = canonical_trade_mutation_content(RadrootsTradeMutationEnvelopeV1 {
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
        if let RadrootsTradeMutationBodyV1::Proposal { candidate } = &mut root.body {
            candidate.private_terms = Some(RadrootsTradePrivateTermsRefV1 {
                artifact_id: "artifact-1".to_string(),
                schema_id: "radroots.private.fulfillment.v1".to_string(),
                ciphertext_commitment: hex_64('f'),
                required_acknowledgement: true,
            });
            candidate.fulfillment.requires_private_terms = true;
        }
        let root = canonical_trade_mutation_content(RadrootsTradeMutationEnvelopeV1 {
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
}

#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

#[cfg(feature = "event_store")]
use radroots_event_store::{RadrootsEventStore, RadrootsEventStoreError, RadrootsStoredEvent};
#[cfg(feature = "serde_json")]
use radroots_events::RadrootsNostrEvent;
use radroots_events::ids::{
    RadrootsEventId, RadrootsIdParseError, RadrootsInventoryBinId, RadrootsListingAddress,
    RadrootsOrderId, RadrootsPublicKey,
};
#[cfg(feature = "serde_json")]
use radroots_events::kinds::{
    KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_REQUEST, KIND_ORDER_REVISION_DECISION,
    KIND_ORDER_REVISION_PROPOSAL,
};
#[cfg(feature = "serde_json")]
use radroots_events::order::RadrootsOrderEventType;
use radroots_events::order::{
    RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
    RadrootsOrderEconomics, RadrootsOrderInventoryCommitment, RadrootsOrderItem,
    RadrootsOrderRequest, RadrootsOrderRevisionDecision, RadrootsOrderRevisionOutcome,
    RadrootsOrderRevisionProposal,
};
#[cfg(feature = "event_store")]
use radroots_events::tags::TAG_D;
#[cfg(feature = "serde_json")]
use radroots_events_codec::order::{
    RadrootsOrderEnvelopeParseError, order_cancellation_from_event, order_decision_from_event,
    order_event_context_from_tags, order_request_from_event, order_revision_decision_from_event,
    order_revision_proposal_from_event,
};
#[cfg(feature = "serde_json")]
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::listing::{
    RadrootsPublicListingAddress, RadrootsPublicListingAddressError, parse_public_listing_address,
};

#[derive(Debug, Error)]
pub enum RadrootsOrderCanonicalizationError {
    #[error("{0} cannot be empty")]
    EmptyField(&'static str),
    #[error("invalid listing_addr: {0}")]
    InvalidListingAddress(String),
    #[error("listing_addr must reference a public NIP-99 listing")]
    InvalidListingKind,
    #[error("buyer_pubkey must match the requested signer identity")]
    InvalidBuyerSigner,
    #[error("seller_pubkey must match listing_addr seller")]
    InvalidSellerListing,
    #[error("items must contain at least one item")]
    MissingItems,
    #[error("items[{index}].bin_count must be greater than zero")]
    InvalidBinCount { index: usize },
    #[error("accepted decisions must contain at least one inventory commitment")]
    MissingInventoryCommitments,
    #[error("inventory_commitments[{index}].bin_count must be greater than zero")]
    InvalidInventoryCommitmentCount { index: usize },
}

pub const ORDER_EVENT_CONTRACT_IDS: [&str; 5] = [
    "radroots.order.request.v1",
    "radroots.order.decision.v1",
    "radroots.order.revision_proposal.v1",
    "radroots.order.revision_decision.v1",
    "radroots.order.cancellation.v1",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderRequestRecord {
    pub event_id: RadrootsEventId,
    pub author_pubkey: RadrootsPublicKey,
    pub payload: RadrootsOrderRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderDecisionRecord {
    pub event_id: RadrootsEventId,
    pub author_pubkey: RadrootsPublicKey,
    pub counterparty_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub prev_event_id: RadrootsEventId,
    pub payload: RadrootsOrderDecision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderRevisionProposalRecord {
    pub event_id: RadrootsEventId,
    pub author_pubkey: RadrootsPublicKey,
    pub counterparty_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub prev_event_id: RadrootsEventId,
    pub payload: RadrootsOrderRevisionProposal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderRevisionDecisionRecord {
    pub event_id: RadrootsEventId,
    pub author_pubkey: RadrootsPublicKey,
    pub counterparty_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub prev_event_id: RadrootsEventId,
    pub payload: RadrootsOrderRevisionDecision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderCancellationRecord {
    pub event_id: RadrootsEventId,
    pub author_pubkey: RadrootsPublicKey,
    pub counterparty_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub prev_event_id: RadrootsEventId,
    pub payload: RadrootsOrderCancellation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEventRecord {
    Request(RadrootsOrderRequestRecord),
    Decision(RadrootsOrderDecisionRecord),
    RevisionProposal(RadrootsOrderRevisionProposalRecord),
    RevisionDecision(RadrootsOrderRevisionDecisionRecord),
    Cancellation(RadrootsOrderCancellationRecord),
}

impl RadrootsOrderEventRecord {
    pub fn event_id(&self) -> &RadrootsEventId {
        match self {
            Self::Request(record) => &record.event_id,
            Self::Decision(record) => &record.event_id,
            Self::RevisionProposal(record) => &record.event_id,
            Self::RevisionDecision(record) => &record.event_id,
            Self::Cancellation(record) => &record.event_id,
        }
    }

    pub fn order_id(&self) -> &RadrootsOrderId {
        match self {
            Self::Request(record) => &record.payload.order_id,
            Self::Decision(record) => &record.payload.order_id,
            Self::RevisionProposal(record) => &record.payload.order_id,
            Self::RevisionDecision(record) => &record.payload.order_id,
            Self::Cancellation(record) => &record.payload.order_id,
        }
    }
}

#[cfg(feature = "serde_json")]
#[derive(Debug, Error)]
pub enum RadrootsOrderEventDecodeError {
    #[error("unsupported order event kind: {kind}")]
    UnsupportedKind { kind: u32 },
    #[error("invalid order event id: {0}")]
    InvalidEventId(RadrootsIdParseError),
    #[error("invalid order event author: {0}")]
    InvalidAuthor(RadrootsIdParseError),
    #[error("order event context is missing root event id")]
    MissingRootEventId,
    #[error("order event context is missing previous event id")]
    MissingPreviousEventId,
    #[error("{0}")]
    Envelope(#[from] RadrootsOrderEnvelopeParseError),
}

#[cfg(feature = "serde_json")]
pub fn order_event_record_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsOrderEventRecord, RadrootsOrderEventDecodeError> {
    let message_type = RadrootsOrderEventType::from_kind(event.kind)
        .ok_or(RadrootsOrderEventDecodeError::UnsupportedKind { kind: event.kind })?;
    let context = order_event_context_from_tags(message_type, &event.tags)?;
    let event_id =
        RadrootsEventId::parse(&event.id).map_err(RadrootsOrderEventDecodeError::InvalidEventId)?;
    let author_pubkey = RadrootsPublicKey::parse(&event.author)
        .map_err(RadrootsOrderEventDecodeError::InvalidAuthor)?;

    match event.kind {
        KIND_ORDER_REQUEST => {
            let envelope = order_request_from_event(event)?;
            Ok(RadrootsOrderEventRecord::Request(
                RadrootsOrderRequestRecord {
                    event_id,
                    author_pubkey,
                    payload: envelope.payload,
                },
            ))
        }
        KIND_ORDER_DECISION => {
            let envelope = order_decision_from_event(event)?;
            Ok(RadrootsOrderEventRecord::Decision(
                RadrootsOrderDecisionRecord {
                    event_id,
                    author_pubkey,
                    counterparty_pubkey: context.counterparty_pubkey.clone(),
                    root_event_id: require_context_root_event_id(&context)?,
                    prev_event_id: require_context_prev_event_id(&context)?,
                    payload: envelope.payload,
                },
            ))
        }
        KIND_ORDER_REVISION_PROPOSAL => {
            let envelope = order_revision_proposal_from_event(event)?;
            Ok(RadrootsOrderEventRecord::RevisionProposal(
                RadrootsOrderRevisionProposalRecord {
                    event_id,
                    author_pubkey,
                    counterparty_pubkey: context.counterparty_pubkey.clone(),
                    root_event_id: require_context_root_event_id(&context)?,
                    prev_event_id: require_context_prev_event_id(&context)?,
                    payload: envelope.payload,
                },
            ))
        }
        KIND_ORDER_REVISION_DECISION => {
            let envelope = order_revision_decision_from_event(event)?;
            Ok(RadrootsOrderEventRecord::RevisionDecision(
                RadrootsOrderRevisionDecisionRecord {
                    event_id,
                    author_pubkey,
                    counterparty_pubkey: context.counterparty_pubkey.clone(),
                    root_event_id: require_context_root_event_id(&context)?,
                    prev_event_id: require_context_prev_event_id(&context)?,
                    payload: envelope.payload,
                },
            ))
        }
        KIND_ORDER_CANCELLATION => {
            let envelope = order_cancellation_from_event(event)?;
            Ok(RadrootsOrderEventRecord::Cancellation(
                RadrootsOrderCancellationRecord {
                    event_id,
                    author_pubkey,
                    counterparty_pubkey: context.counterparty_pubkey.clone(),
                    root_event_id: require_context_root_event_id(&context)?,
                    prev_event_id: require_context_prev_event_id(&context)?,
                    payload: envelope.payload,
                },
            ))
        }
        _ => Err(RadrootsOrderEventDecodeError::UnsupportedKind { kind: event.kind }),
    }
}

#[cfg(feature = "event_store")]
#[derive(Debug, Error)]
pub enum RadrootsOrderStoreQueryError {
    #[error("{0}")]
    Store(#[from] RadrootsEventStoreError),
    #[error("stored order event {event_id} contains invalid tags_json: {source}")]
    InvalidStoredTagsJson {
        event_id: String,
        source: serde_json::Error,
    },
    #[error("stored order event {event_id} could not decode as an order record: {source}")]
    Decode {
        event_id: String,
        source: RadrootsOrderEventDecodeError,
    },
}

#[cfg(feature = "event_store")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderProjectionQueryResult {
    pub projection: RadrootsOrderProjection,
    pub event_count: usize,
    pub limit_applied: u32,
    pub event_ids: Vec<RadrootsEventId>,
}

#[cfg(feature = "event_store")]
pub async fn order_events_for_order_id(
    store: &RadrootsEventStore,
    order_id: &RadrootsOrderId,
    limit: u32,
) -> Result<Vec<RadrootsOrderEventRecord>, RadrootsOrderStoreQueryError> {
    let stored_events = store
        .events_by_contract_and_tag(&ORDER_EVENT_CONTRACT_IDS, TAG_D, order_id.as_str(), limit)
        .await?;
    let mut records = Vec::with_capacity(stored_events.len());
    for stored_event in stored_events {
        let event = stored_order_event_to_nostr_event(&stored_event)?;
        let record = order_event_record_from_event(&event).map_err(|source| {
            RadrootsOrderStoreQueryError::Decode {
                event_id: stored_event.event_id.clone(),
                source,
            }
        })?;
        if record.order_id() == order_id {
            records.push(record);
        }
    }
    Ok(records)
}

#[cfg(feature = "event_store")]
pub async fn order_projection_for_order_id(
    store: &RadrootsEventStore,
    order_id: &RadrootsOrderId,
    limit: u32,
) -> Result<RadrootsOrderProjection, RadrootsOrderStoreQueryError> {
    order_projection_query_for_order_id(store, order_id, limit)
        .await
        .map(|result| result.projection)
}

#[cfg(feature = "event_store")]
pub async fn order_projection_query_for_order_id(
    store: &RadrootsEventStore,
    order_id: &RadrootsOrderId,
    limit: u32,
) -> Result<RadrootsOrderProjectionQueryResult, RadrootsOrderStoreQueryError> {
    let records = order_events_for_order_id(store, order_id, limit).await?;
    let event_ids = records
        .iter()
        .map(|record| record.event_id().clone())
        .collect::<Vec<_>>();
    let event_count = records.len();
    Ok(RadrootsOrderProjectionQueryResult {
        projection: reduce_order_event_records(order_id, records),
        event_count,
        limit_applied: limit,
        event_ids,
    })
}

#[cfg(feature = "event_store")]
fn stored_order_event_to_nostr_event(
    stored_event: &RadrootsStoredEvent,
) -> Result<RadrootsNostrEvent, RadrootsOrderStoreQueryError> {
    let tags = serde_json::from_str(&stored_event.tags_json).map_err(|source| {
        RadrootsOrderStoreQueryError::InvalidStoredTagsJson {
            event_id: stored_event.event_id.clone(),
            source,
        }
    })?;
    Ok(RadrootsNostrEvent {
        id: stored_event.event_id.clone(),
        author: stored_event.pubkey.clone(),
        created_at: stored_event.created_at,
        kind: stored_event.kind,
        tags,
        content: stored_event.content.clone(),
        sig: stored_event.sig.clone(),
    })
}

#[cfg(feature = "serde_json")]
fn require_context_root_event_id(
    context: &radroots_events_codec::order::RadrootsOrderEventContext,
) -> Result<RadrootsEventId, RadrootsOrderEventDecodeError> {
    context
        .root_event_id
        .clone()
        .ok_or(RadrootsOrderEventDecodeError::MissingRootEventId)
}

#[cfg(feature = "serde_json")]
fn require_context_prev_event_id(
    context: &radroots_events_codec::order::RadrootsOrderEventContext,
) -> Result<RadrootsEventId, RadrootsOrderEventDecodeError> {
    context
        .prev_event_id
        .clone()
        .ok_or(RadrootsOrderEventDecodeError::MissingPreviousEventId)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOrderStatus {
    Missing,
    Requested,
    Accepted,
    Declined,
    Cancelled,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOrderIssue {
    MissingRequest,
    MultipleRequests { event_ids: Vec<RadrootsEventId> },
    RequestPayloadInvalid { event_id: RadrootsEventId },
    RequestOrderIdMismatch { event_id: RadrootsEventId },
    RequestAuthorMismatch { event_id: RadrootsEventId },
    RequestListingAddressInvalid { event_id: RadrootsEventId },
    RequestSellerListingMismatch { event_id: RadrootsEventId },
    DecisionPayloadInvalid { event_id: RadrootsEventId },
    DecisionOrderIdMismatch { event_id: RadrootsEventId },
    DecisionAuthorMismatch { event_id: RadrootsEventId },
    DecisionCounterpartyMismatch { event_id: RadrootsEventId },
    DecisionBuyerMismatch { event_id: RadrootsEventId },
    DecisionSellerMismatch { event_id: RadrootsEventId },
    DecisionListingAddressInvalid { event_id: RadrootsEventId },
    DecisionListingMismatch { event_id: RadrootsEventId },
    DecisionRootMismatch { event_id: RadrootsEventId },
    DecisionPreviousMismatch { event_id: RadrootsEventId },
    DecisionMissingInventoryCommitments { event_id: RadrootsEventId },
    DecisionInventoryCommitmentMismatch { event_id: RadrootsEventId },
    DecisionMissingReason { event_id: RadrootsEventId },
    ConflictingDecisions { event_ids: Vec<RadrootsEventId> },
    RevisionProposalPayloadInvalid { event_id: RadrootsEventId },
    RevisionProposalOrderIdMismatch { event_id: RadrootsEventId },
    RevisionProposalAuthorMismatch { event_id: RadrootsEventId },
    RevisionProposalCounterpartyMismatch { event_id: RadrootsEventId },
    RevisionProposalBuyerMismatch { event_id: RadrootsEventId },
    RevisionProposalSellerMismatch { event_id: RadrootsEventId },
    RevisionProposalListingAddressInvalid { event_id: RadrootsEventId },
    RevisionProposalListingMismatch { event_id: RadrootsEventId },
    RevisionProposalRootMismatch { event_id: RadrootsEventId },
    RevisionProposalPreviousMismatch { event_id: RadrootsEventId },
    RevisionDecisionWithoutProposal { event_id: RadrootsEventId },
    RevisionDecisionPayloadInvalid { event_id: RadrootsEventId },
    RevisionDecisionOrderIdMismatch { event_id: RadrootsEventId },
    RevisionDecisionAuthorMismatch { event_id: RadrootsEventId },
    RevisionDecisionCounterpartyMismatch { event_id: RadrootsEventId },
    RevisionDecisionBuyerMismatch { event_id: RadrootsEventId },
    RevisionDecisionSellerMismatch { event_id: RadrootsEventId },
    RevisionDecisionListingAddressInvalid { event_id: RadrootsEventId },
    RevisionDecisionListingMismatch { event_id: RadrootsEventId },
    RevisionDecisionRootMismatch { event_id: RadrootsEventId },
    RevisionDecisionPreviousMismatch { event_id: RadrootsEventId },
    RevisionDecisionRevisionIdMismatch { event_id: RadrootsEventId },
    CancellationWithoutCancellableOrder { event_id: RadrootsEventId },
    CancellationPayloadInvalid { event_id: RadrootsEventId },
    CancellationOrderIdMismatch { event_id: RadrootsEventId },
    CancellationAuthorMismatch { event_id: RadrootsEventId },
    CancellationCounterpartyMismatch { event_id: RadrootsEventId },
    CancellationBuyerMismatch { event_id: RadrootsEventId },
    CancellationSellerMismatch { event_id: RadrootsEventId },
    CancellationListingAddressInvalid { event_id: RadrootsEventId },
    CancellationListingMismatch { event_id: RadrootsEventId },
    CancellationRootMismatch { event_id: RadrootsEventId },
    CancellationPreviousMismatch { event_id: RadrootsEventId },
    ForkedLifecycle { event_ids: Vec<RadrootsEventId> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderProjection {
    pub order_id: RadrootsOrderId,
    pub status: RadrootsOrderStatus,
    pub request_event_id: Option<RadrootsEventId>,
    pub decision_event_id: Option<RadrootsEventId>,
    pub cancellation_event_id: Option<RadrootsEventId>,
    pub lifecycle_terminal: bool,
    pub economics: Option<RadrootsOrderEconomics>,
    pub agreement_event_id: Option<RadrootsEventId>,
    pub pending_revision_event_id: Option<RadrootsEventId>,
    pub listing_addr: Option<RadrootsListingAddress>,
    pub buyer_pubkey: Option<RadrootsPublicKey>,
    pub seller_pubkey: Option<RadrootsPublicKey>,
    pub last_event_id: Option<RadrootsEventId>,
    pub issues: Vec<RadrootsOrderIssue>,
}

#[cfg(feature = "serde_json")]
#[derive(Debug, Error)]
pub enum RadrootsOrderEconomicsDigestError {
    #[error("failed to serialize order economics for digest: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingInventoryBinAvailability {
    pub bin_id: RadrootsInventoryBinId,
    pub available_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingInventoryOrderReservation {
    pub order_id: RadrootsOrderId,
    pub agreement_event_id: RadrootsEventId,
    pub bin_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingInventoryBinAccounting {
    pub bin_id: RadrootsInventoryBinId,
    pub available_count: u64,
    pub accepted_reserved_count: u64,
    pub remaining_count: u64,
    pub over_reserved: bool,
    pub accepted_orders: Vec<RadrootsListingInventoryOrderReservation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsListingInventoryAccountingIssue {
    InvalidOrder {
        order_id: RadrootsOrderId,
        event_ids: Vec<RadrootsEventId>,
    },
    ArithmeticOverflow {
        bin_id: RadrootsInventoryBinId,
        event_ids: Vec<RadrootsEventId>,
    },
    UnknownInventoryBin {
        bin_id: RadrootsInventoryBinId,
        event_ids: Vec<RadrootsEventId>,
    },
    OverReserved {
        bin_id: RadrootsInventoryBinId,
        available_count: u64,
        reserved_count: u64,
        event_ids: Vec<RadrootsEventId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingInventoryAccountingProjection {
    pub listing_addr: RadrootsListingAddress,
    pub listing_event_id: RadrootsEventId,
    pub bins: Vec<RadrootsListingInventoryBinAccounting>,
    pub declined_order_ids: Vec<RadrootsOrderId>,
    pub cancelled_order_ids: Vec<RadrootsOrderId>,
    pub invalid_event_ids: Vec<RadrootsEventId>,
    pub issues: Vec<RadrootsListingInventoryAccountingIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderReductionInputs<I, J, K, L, M> {
    pub requests: I,
    pub decisions: J,
    pub revision_proposals: K,
    pub revision_decisions: L,
    pub cancellations: M,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsGroupedOrderEventRecords {
    pub requests: Vec<RadrootsOrderRequestRecord>,
    pub decisions: Vec<RadrootsOrderDecisionRecord>,
    pub revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
    pub revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
    pub cancellations: Vec<RadrootsOrderCancellationRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingInventoryAccountingInputs<I, J, K, L, M, N> {
    pub bins: I,
    pub requests: J,
    pub decisions: K,
    pub revision_proposals: L,
    pub revision_decisions: M,
    pub cancellations: N,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RadrootsListingInventoryAccountingRecords {
    bins: Vec<RadrootsListingInventoryBinAvailability>,
    requests: Vec<RadrootsOrderRequestRecord>,
    decisions: Vec<RadrootsOrderDecisionRecord>,
    revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
    revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
    cancellations: Vec<RadrootsOrderCancellationRecord>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn reduce_order_events<I, J, K, L, M>(
    order_id: &RadrootsOrderId,
    inputs: RadrootsOrderReductionInputs<I, J, K, L, M>,
) -> RadrootsOrderProjection
where
    I: IntoIterator<Item = RadrootsOrderRequestRecord>,
    J: IntoIterator<Item = RadrootsOrderDecisionRecord>,
    K: IntoIterator<Item = RadrootsOrderRevisionProposalRecord>,
    L: IntoIterator<Item = RadrootsOrderRevisionDecisionRecord>,
    M: IntoIterator<Item = RadrootsOrderCancellationRecord>,
{
    reduce_grouped_order_event_records(
        order_id,
        RadrootsGroupedOrderEventRecords {
            requests: inputs.requests.into_iter().collect(),
            decisions: inputs.decisions.into_iter().collect(),
            revision_proposals: inputs.revision_proposals.into_iter().collect(),
            revision_decisions: inputs.revision_decisions.into_iter().collect(),
            cancellations: inputs.cancellations.into_iter().collect(),
        },
    )
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn reduce_order_event_records<I>(
    order_id: &RadrootsOrderId,
    records: I,
) -> RadrootsOrderProjection
where
    I: IntoIterator<Item = RadrootsOrderEventRecord>,
{
    let mut seen_event_ids = Vec::new();
    let mut grouped = RadrootsGroupedOrderEventRecords::default();

    for record in records {
        let event_id = record.event_id().clone();
        if seen_event_ids.iter().any(|seen| seen == &event_id) {
            continue;
        }
        seen_event_ids.push(event_id);
        match record {
            RadrootsOrderEventRecord::Request(record) => grouped.requests.push(record),
            RadrootsOrderEventRecord::Decision(record) => grouped.decisions.push(record),
            RadrootsOrderEventRecord::RevisionProposal(record) => {
                grouped.revision_proposals.push(record);
            }
            RadrootsOrderEventRecord::RevisionDecision(record) => {
                grouped.revision_decisions.push(record);
            }
            RadrootsOrderEventRecord::Cancellation(record) => grouped.cancellations.push(record),
        }
    }

    reduce_grouped_order_event_records(order_id, grouped)
}

fn reduce_grouped_order_event_records(
    order_id: &RadrootsOrderId,
    records: RadrootsGroupedOrderEventRecords,
) -> RadrootsOrderProjection {
    let requests = unique_request_records(records.requests);
    let decisions = unique_decision_records(records.decisions);
    let revision_proposals = unique_revision_proposal_records(records.revision_proposals);
    let revision_decisions = unique_revision_decision_records(records.revision_decisions);
    let cancellations = unique_cancellation_records(records.cancellations);
    if requests.is_empty()
        && decisions.is_empty()
        && revision_proposals.is_empty()
        && revision_decisions.is_empty()
        && cancellations.is_empty()
    {
        return empty_projection(order_id, RadrootsOrderStatus::Missing, false);
    }

    let mut issues = Vec::new();
    let mut valid_requests = Vec::new();
    for request in requests {
        if validate_order_request_record(order_id, &request, &mut issues) {
            valid_requests.push(request);
        }
    }

    if valid_requests.len() > 1 {
        let mut event_ids = valid_requests
            .iter()
            .map(|request| request.event_id.clone())
            .collect::<Vec<_>>();
        sort_and_dedup_values(&mut event_ids);
        issues.push(RadrootsOrderIssue::MultipleRequests { event_ids });
    }

    let Some(request) = valid_requests.first() else {
        if !decisions.is_empty()
            || !revision_proposals.is_empty()
            || !revision_decisions.is_empty()
            || !cancellations.is_empty()
        {
            issues.push(RadrootsOrderIssue::MissingRequest);
        }
        return invalid_projection(order_id, None, issues);
    };

    if valid_requests.len() > 1 {
        return invalid_projection(order_id, Some(request), issues);
    }

    let mut valid_decisions = Vec::new();
    for decision in decisions {
        if validate_order_decision_record(request, &decision, &mut issues) {
            valid_decisions.push(decision);
        }
    }

    let mut valid_revision_proposals = Vec::new();
    for proposal in revision_proposals {
        if validate_order_revision_proposal_record(request, &proposal, &mut issues) {
            valid_revision_proposals.push(proposal);
        }
    }

    let mut valid_revision_decisions = Vec::new();
    for decision in revision_decisions {
        if validate_order_revision_decision_record(request, &decision, &mut issues) {
            valid_revision_decisions.push(decision);
        }
    }

    let mut valid_cancellations = Vec::new();
    for cancellation in cancellations {
        if validate_order_cancellation_record(request, &cancellation, &mut issues) {
            valid_cancellations.push(cancellation);
        }
    }

    if !issues.is_empty() {
        return invalid_projection(order_id, Some(request), issues);
    }

    if valid_cancellations.len() > 1 {
        let mut event_ids = valid_cancellations
            .iter()
            .map(|cancellation| cancellation.event_id.clone())
            .collect::<Vec<_>>();
        sort_and_dedup_values(&mut event_ids);
        return invalid_projection(
            order_id,
            Some(request),
            vec![RadrootsOrderIssue::ForkedLifecycle { event_ids }],
        );
    }

    if let Some(cancellation) = valid_cancellations.first() {
        return cancelled_projection(
            order_id,
            request,
            cancellation,
            &valid_decisions,
            &valid_revision_proposals,
            &valid_revision_decisions,
        );
    }

    match valid_decisions.len() {
        0 => negotiation_projection(
            order_id,
            request,
            &valid_revision_proposals,
            &valid_revision_decisions,
        ),
        1 => decided_projection(
            order_id,
            request,
            &valid_decisions[0],
            &valid_revision_proposals,
            &valid_revision_decisions,
        ),
        _ => {
            let mut event_ids = valid_decisions
                .iter()
                .map(|decision| decision.event_id.clone())
                .collect::<Vec<_>>();
            sort_and_dedup_values(&mut event_ids);
            invalid_projection(
                order_id,
                Some(request),
                vec![RadrootsOrderIssue::ConflictingDecisions { event_ids }],
            )
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn reduce_listing_inventory_accounting<I, J, K, L, M, N>(
    listing_addr: &RadrootsListingAddress,
    listing_event_id: &RadrootsEventId,
    inputs: RadrootsListingInventoryAccountingInputs<I, J, K, L, M, N>,
) -> RadrootsListingInventoryAccountingProjection
where
    I: IntoIterator<Item = RadrootsListingInventoryBinAvailability>,
    J: IntoIterator<Item = RadrootsOrderRequestRecord>,
    K: IntoIterator<Item = RadrootsOrderDecisionRecord>,
    L: IntoIterator<Item = RadrootsOrderRevisionProposalRecord>,
    M: IntoIterator<Item = RadrootsOrderRevisionDecisionRecord>,
    N: IntoIterator<Item = RadrootsOrderCancellationRecord>,
{
    reduce_listing_inventory_accounting_records(
        listing_addr,
        listing_event_id,
        RadrootsListingInventoryAccountingRecords {
            bins: inputs.bins.into_iter().collect(),
            requests: inputs.requests.into_iter().collect(),
            decisions: inputs.decisions.into_iter().collect(),
            revision_proposals: inputs.revision_proposals.into_iter().collect(),
            revision_decisions: inputs.revision_decisions.into_iter().collect(),
            cancellations: inputs.cancellations.into_iter().collect(),
        },
    )
}

fn reduce_listing_inventory_accounting_records(
    listing_addr: &RadrootsListingAddress,
    listing_event_id: &RadrootsEventId,
    records: RadrootsListingInventoryAccountingRecords,
) -> RadrootsListingInventoryAccountingProjection {
    let (mut bins, mut issues) = normalized_listing_inventory_bins(records.bins);
    let requests = unique_request_records(records.requests)
        .into_iter()
        .filter(|request| request.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let decisions = unique_decision_records(records.decisions)
        .into_iter()
        .filter(|decision| decision.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let revision_proposals = unique_revision_proposal_records(records.revision_proposals)
        .into_iter()
        .filter(|proposal| proposal.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let revision_decisions = unique_revision_decision_records(records.revision_decisions)
        .into_iter()
        .filter(|decision| decision.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let cancellations = unique_cancellation_records(records.cancellations)
        .into_iter()
        .filter(|cancellation| cancellation.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let mut order_ids = listing_order_ids(
        &requests,
        &decisions,
        &revision_proposals,
        &revision_decisions,
        &cancellations,
    );
    let mut declined_order_ids = Vec::new();
    let mut cancelled_order_ids = Vec::new();
    let mut invalid_event_ids = Vec::new();

    for order_id in order_ids.drain(..) {
        let order_requests = requests
            .iter()
            .filter(|request| request.payload.order_id == order_id)
            .cloned()
            .collect::<Vec<_>>();
        let order_decisions = decisions
            .iter()
            .filter(|decision| decision.payload.order_id == order_id)
            .cloned()
            .collect::<Vec<_>>();
        let order_revision_proposals = revision_proposals
            .iter()
            .filter(|proposal| proposal.payload.order_id == order_id)
            .cloned()
            .collect::<Vec<_>>();
        let order_revision_decisions = revision_decisions
            .iter()
            .filter(|decision| decision.payload.order_id == order_id)
            .cloned()
            .collect::<Vec<_>>();
        let order_cancellations = cancellations
            .iter()
            .filter(|cancellation| cancellation.payload.order_id == order_id)
            .cloned()
            .collect::<Vec<_>>();
        let projection = reduce_order_events(
            &order_id,
            RadrootsOrderReductionInputs {
                requests: order_requests.clone(),
                decisions: order_decisions.clone(),
                revision_proposals: order_revision_proposals.clone(),
                revision_decisions: order_revision_decisions.clone(),
                cancellations: order_cancellations.clone(),
            },
        );
        match projection.status {
            RadrootsOrderStatus::Accepted => {
                if let Some(agreement_event_id) = projection.agreement_event_id.as_ref()
                    && let Some(economics) = projection.economics.as_ref()
                {
                    add_accepted_inventory_reservations_from_economics(
                        &mut bins,
                        &order_id,
                        agreement_event_id,
                        economics,
                        &mut issues,
                    );
                }
            }
            RadrootsOrderStatus::Cancelled => cancelled_order_ids.push(order_id),
            RadrootsOrderStatus::Declined => declined_order_ids.push(order_id),
            RadrootsOrderStatus::Invalid => {
                let mut event_ids = projection_issue_event_ids(&projection.issues);
                if event_ids.is_empty() {
                    event_ids.extend(
                        order_requests
                            .iter()
                            .map(|request| request.event_id.clone()),
                    );
                    event_ids.extend(
                        order_decisions
                            .iter()
                            .map(|decision| decision.event_id.clone()),
                    );
                    event_ids.extend(
                        order_revision_proposals
                            .iter()
                            .map(|proposal| proposal.event_id.clone()),
                    );
                    event_ids.extend(
                        order_revision_decisions
                            .iter()
                            .map(|decision| decision.event_id.clone()),
                    );
                    event_ids.extend(
                        order_cancellations
                            .iter()
                            .map(|cancellation| cancellation.event_id.clone()),
                    );
                    sort_and_dedup_values(&mut event_ids);
                }
                invalid_event_ids.extend(event_ids.iter().cloned());
                issues.push(RadrootsListingInventoryAccountingIssue::InvalidOrder {
                    order_id,
                    event_ids,
                });
            }
            RadrootsOrderStatus::Missing | RadrootsOrderStatus::Requested => {}
        }
    }

    sort_and_dedup_values(&mut declined_order_ids);
    sort_and_dedup_values(&mut cancelled_order_ids);
    sort_and_dedup_values(&mut invalid_event_ids);
    finish_inventory_accounting_bins(&mut bins, &mut issues);
    issues.sort_by(inventory_issue_sort_key);
    RadrootsListingInventoryAccountingProjection {
        listing_addr: listing_addr.clone(),
        listing_event_id: listing_event_id.clone(),
        bins,
        declined_order_ids,
        cancelled_order_ids,
        invalid_event_ids,
        issues,
    }
}

pub fn canonicalize_order_request_for_signer(
    mut request: RadrootsOrderRequest,
    signer_pubkey: &str,
) -> Result<RadrootsOrderRequest, RadrootsOrderCanonicalizationError> {
    let order_id = request.order_id.clone();
    let listing_addr_raw = request.listing_addr.to_string();
    let listing_addr = parse_public_listing_addr(&listing_addr_raw)?;

    let buyer_pubkey = request.buyer_pubkey.clone();
    if buyer_pubkey.as_str() != signer_pubkey {
        return Err(RadrootsOrderCanonicalizationError::InvalidBuyerSigner);
    }

    let seller_pubkey = request.seller_pubkey.clone();
    if seller_pubkey != listing_addr.seller_pubkey {
        return Err(RadrootsOrderCanonicalizationError::InvalidSellerListing);
    }

    canonicalize_items(&mut request.items)?;
    request.economics.canonicalize();
    request.order_id = order_id;
    request.listing_addr = listing_addr.address;
    request.buyer_pubkey = buyer_pubkey;
    request.seller_pubkey = seller_pubkey;
    Ok(request)
}

pub fn canonicalize_order_decision_for_signer(
    mut decision_event: RadrootsOrderDecision,
    signer_pubkey: &str,
) -> Result<RadrootsOrderDecision, RadrootsOrderCanonicalizationError> {
    let order_id = decision_event.order_id.clone();
    let listing_addr_raw = decision_event.listing_addr.to_string();
    let listing_addr = parse_public_listing_addr(&listing_addr_raw)?;

    let seller_pubkey = decision_event.seller_pubkey.clone();
    if seller_pubkey.as_str() != signer_pubkey || seller_pubkey != listing_addr.seller_pubkey {
        return Err(RadrootsOrderCanonicalizationError::InvalidSellerListing);
    }

    let buyer_pubkey = decision_event.buyer_pubkey.clone();
    canonicalize_decision(&mut decision_event.decision)?;

    decision_event.order_id = order_id;
    decision_event.listing_addr = listing_addr.address;
    decision_event.buyer_pubkey = buyer_pubkey;
    decision_event.seller_pubkey = seller_pubkey;
    Ok(decision_event)
}

#[cfg(feature = "serde_json")]
pub fn radroots_order_economics_digest(
    economics: &RadrootsOrderEconomics,
) -> Result<String, RadrootsOrderEconomicsDigestError> {
    let encoded = serde_json::to_vec(economics)?;
    let digest = Sha256::digest(encoded);
    let mut value = String::from("sha256:");
    value.push_str(&hex::encode(digest));
    Ok(value)
}

fn cancelled_projection(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    cancellation: &RadrootsOrderCancellationRecord,
    decisions: &[RadrootsOrderDecisionRecord],
    revision_proposals: &[RadrootsOrderRevisionProposalRecord],
    revision_decisions: &[RadrootsOrderRevisionDecisionRecord],
) -> RadrootsOrderProjection {
    if !decisions.is_empty() || !revision_decisions.is_empty() {
        let mut event_ids = Vec::new();
        event_ids.extend(decisions.iter().map(|decision| decision.event_id.clone()));
        event_ids.extend(
            revision_decisions
                .iter()
                .map(|decision| decision.event_id.clone()),
        );
        event_ids.push(cancellation.event_id.clone());
        sort_and_dedup_values(&mut event_ids);
        return invalid_projection(
            order_id,
            Some(request),
            vec![RadrootsOrderIssue::ForkedLifecycle { event_ids }],
        );
    }
    if revision_proposals.len() > 1 {
        let mut event_ids = revision_proposals
            .iter()
            .map(|proposal| proposal.event_id.clone())
            .collect::<Vec<_>>();
        event_ids.push(cancellation.event_id.clone());
        sort_and_dedup_values(&mut event_ids);
        return invalid_projection(
            order_id,
            Some(request),
            vec![RadrootsOrderIssue::ForkedLifecycle { event_ids }],
        );
    }
    let expected_prev_event_id = revision_proposals
        .first()
        .map(|proposal| &proposal.event_id)
        .unwrap_or(&request.event_id);
    if &cancellation.prev_event_id != expected_prev_event_id {
        return invalid_projection(
            order_id,
            Some(request),
            vec![RadrootsOrderIssue::CancellationPreviousMismatch {
                event_id: cancellation.event_id.clone(),
            }],
        );
    }

    let mut projection = request_projection(order_id, request, RadrootsOrderStatus::Cancelled);
    projection.cancellation_event_id = Some(cancellation.event_id.clone());
    projection.lifecycle_terminal = true;
    projection.last_event_id = Some(cancellation.event_id.clone());
    projection
}

fn negotiation_projection(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    revision_proposals: &[RadrootsOrderRevisionProposalRecord],
    revision_decisions: &[RadrootsOrderRevisionDecisionRecord],
) -> RadrootsOrderProjection {
    match revision_proposals.len() {
        0 => {
            if revision_decisions.is_empty() {
                request_projection(order_id, request, RadrootsOrderStatus::Requested)
            } else {
                invalid_projection(
                    order_id,
                    Some(request),
                    revision_decisions
                        .iter()
                        .map(
                            |decision| RadrootsOrderIssue::RevisionDecisionWithoutProposal {
                                event_id: decision.event_id.clone(),
                            },
                        )
                        .collect(),
                )
            }
        }
        1 => {
            let proposal = &revision_proposals[0];
            if proposal.prev_event_id != request.event_id {
                return invalid_projection(
                    order_id,
                    Some(request),
                    vec![RadrootsOrderIssue::RevisionProposalPreviousMismatch {
                        event_id: proposal.event_id.clone(),
                    }],
                );
            }
            match revision_decisions.len() {
                0 => {
                    let mut projection =
                        request_projection(order_id, request, RadrootsOrderStatus::Requested);
                    projection.pending_revision_event_id = Some(proposal.event_id.clone());
                    projection.economics = Some(proposal.payload.economics.clone());
                    projection.last_event_id = Some(proposal.event_id.clone());
                    projection
                }
                1 => revision_decision_projection(
                    order_id,
                    request,
                    proposal,
                    &revision_decisions[0],
                ),
                _ => {
                    let mut event_ids = revision_decisions
                        .iter()
                        .map(|decision| decision.event_id.clone())
                        .collect::<Vec<_>>();
                    sort_and_dedup_values(&mut event_ids);
                    invalid_projection(
                        order_id,
                        Some(request),
                        vec![RadrootsOrderIssue::ForkedLifecycle { event_ids }],
                    )
                }
            }
        }
        _ => {
            let mut event_ids = revision_proposals
                .iter()
                .map(|proposal| proposal.event_id.clone())
                .collect::<Vec<_>>();
            sort_and_dedup_values(&mut event_ids);
            invalid_projection(
                order_id,
                Some(request),
                vec![RadrootsOrderIssue::ForkedLifecycle { event_ids }],
            )
        }
    }
}

fn decided_projection(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    decision: &RadrootsOrderDecisionRecord,
    revision_proposals: &[RadrootsOrderRevisionProposalRecord],
    revision_decisions: &[RadrootsOrderRevisionDecisionRecord],
) -> RadrootsOrderProjection {
    if !revision_proposals.is_empty() || !revision_decisions.is_empty() {
        let mut event_ids = Vec::new();
        event_ids.extend(
            revision_proposals
                .iter()
                .map(|proposal| proposal.event_id.clone()),
        );
        event_ids.extend(
            revision_decisions
                .iter()
                .map(|decision| decision.event_id.clone()),
        );
        event_ids.push(decision.event_id.clone());
        sort_and_dedup_values(&mut event_ids);
        return invalid_projection(
            order_id,
            Some(request),
            vec![RadrootsOrderIssue::ForkedLifecycle { event_ids }],
        );
    }

    match &decision.payload.decision {
        RadrootsOrderDecisionOutcome::Accepted { .. } => {
            let mut projection =
                request_projection(order_id, request, RadrootsOrderStatus::Accepted);
            projection.decision_event_id = Some(decision.event_id.clone());
            projection.lifecycle_terminal = true;
            projection.economics = Some(request.payload.economics.clone());
            projection.agreement_event_id = Some(decision.event_id.clone());
            projection.last_event_id = Some(decision.event_id.clone());
            projection
        }
        RadrootsOrderDecisionOutcome::Declined { .. } => {
            let mut projection =
                request_projection(order_id, request, RadrootsOrderStatus::Declined);
            projection.decision_event_id = Some(decision.event_id.clone());
            projection.lifecycle_terminal = true;
            projection.last_event_id = Some(decision.event_id.clone());
            projection
        }
    }
}

fn revision_decision_projection(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    proposal: &RadrootsOrderRevisionProposalRecord,
    decision: &RadrootsOrderRevisionDecisionRecord,
) -> RadrootsOrderProjection {
    if decision.prev_event_id != proposal.event_id {
        return invalid_projection(
            order_id,
            Some(request),
            vec![RadrootsOrderIssue::RevisionDecisionPreviousMismatch {
                event_id: decision.event_id.clone(),
            }],
        );
    }
    if decision.payload.revision_id != proposal.payload.revision_id {
        return invalid_projection(
            order_id,
            Some(request),
            vec![RadrootsOrderIssue::RevisionDecisionRevisionIdMismatch {
                event_id: decision.event_id.clone(),
            }],
        );
    }

    match &decision.payload.decision {
        RadrootsOrderRevisionOutcome::Accepted => {
            let mut projection =
                request_projection(order_id, request, RadrootsOrderStatus::Accepted);
            projection.economics = Some(proposal.payload.economics.clone());
            projection.agreement_event_id = Some(decision.event_id.clone());
            projection.lifecycle_terminal = true;
            projection.last_event_id = Some(decision.event_id.clone());
            projection
        }
        RadrootsOrderRevisionOutcome::Declined { .. } => {
            let mut projection =
                request_projection(order_id, request, RadrootsOrderStatus::Declined);
            projection.lifecycle_terminal = true;
            projection.pending_revision_event_id = Some(proposal.event_id.clone());
            projection.last_event_id = Some(decision.event_id.clone());
            projection
        }
    }
}

fn request_projection(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    status: RadrootsOrderStatus,
) -> RadrootsOrderProjection {
    RadrootsOrderProjection {
        order_id: order_id.clone(),
        status,
        request_event_id: Some(request.event_id.clone()),
        decision_event_id: None,
        cancellation_event_id: None,
        lifecycle_terminal: false,
        economics: Some(request.payload.economics.clone()),
        agreement_event_id: None,
        pending_revision_event_id: None,
        listing_addr: Some(request.payload.listing_addr.clone()),
        buyer_pubkey: Some(request.payload.buyer_pubkey.clone()),
        seller_pubkey: Some(request.payload.seller_pubkey.clone()),
        last_event_id: Some(request.event_id.clone()),
        issues: Vec::new(),
    }
}

fn invalid_projection(
    order_id: &RadrootsOrderId,
    request: Option<&RadrootsOrderRequestRecord>,
    mut issues: Vec<RadrootsOrderIssue>,
) -> RadrootsOrderProjection {
    issues.sort_by(order_issue_sort_key);
    let last_event_id = projection_issue_event_ids(&issues).into_iter().last();
    match request {
        Some(request) => {
            let mut projection =
                request_projection(order_id, request, RadrootsOrderStatus::Invalid);
            projection.lifecycle_terminal = true;
            projection.last_event_id = last_event_id.or_else(|| Some(request.event_id.clone()));
            projection.issues = issues;
            projection
        }
        None => {
            let mut projection = empty_projection(order_id, RadrootsOrderStatus::Invalid, true);
            projection.last_event_id = last_event_id;
            projection.issues = issues;
            projection
        }
    }
}

fn empty_projection(
    order_id: &RadrootsOrderId,
    status: RadrootsOrderStatus,
    lifecycle_terminal: bool,
) -> RadrootsOrderProjection {
    RadrootsOrderProjection {
        order_id: order_id.clone(),
        status,
        request_event_id: None,
        decision_event_id: None,
        cancellation_event_id: None,
        lifecycle_terminal,
        economics: None,
        agreement_event_id: None,
        pending_revision_event_id: None,
        listing_addr: None,
        buyer_pubkey: None,
        seller_pubkey: None,
        last_event_id: None,
        issues: Vec::new(),
    }
}

fn validate_order_request_record(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> bool {
    let mut valid = true;
    if request.payload.validate().is_err() {
        issues.push(RadrootsOrderIssue::RequestPayloadInvalid {
            event_id: request.event_id.clone(),
        });
        valid = false;
    }
    if request.payload.order_id.as_str() != order_id.as_str() {
        issues.push(RadrootsOrderIssue::RequestOrderIdMismatch {
            event_id: request.event_id.clone(),
        });
        valid = false;
    }
    if request.author_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::RequestAuthorMismatch {
            event_id: request.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&request.payload.listing_addr) {
        Ok(listing_addr) => {
            if listing_addr.seller_pubkey != request.payload.seller_pubkey {
                issues.push(RadrootsOrderIssue::RequestSellerListingMismatch {
                    event_id: request.event_id.clone(),
                });
                valid = false;
            }
        }
        Err(_) => {
            issues.push(RadrootsOrderIssue::RequestListingAddressInvalid {
                event_id: request.event_id.clone(),
            });
            valid = false;
        }
    }
    valid
}

fn validate_order_decision_record(
    request: &RadrootsOrderRequestRecord,
    decision: &RadrootsOrderDecisionRecord,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> bool {
    let mut valid = true;
    if decision_payload_issue(&decision.payload.decision, &decision.event_id, issues) {
        valid = false;
    }
    if decision.payload.validate().is_err() {
        issues.push(RadrootsOrderIssue::DecisionPayloadInvalid {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.payload.order_id != request.payload.order_id {
        issues.push(RadrootsOrderIssue::DecisionOrderIdMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.author_pubkey != decision.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::DecisionAuthorMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.counterparty_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::DecisionCounterpartyMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.payload.buyer_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::DecisionBuyerMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.payload.seller_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::DecisionSellerMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&decision.payload.listing_addr) {
        Ok(listing_addr) => {
            if decision.payload.listing_addr != request.payload.listing_addr
                || listing_addr.seller_pubkey != decision.payload.seller_pubkey
            {
                issues.push(RadrootsOrderIssue::DecisionListingMismatch {
                    event_id: decision.event_id.clone(),
                });
                valid = false;
            }
        }
        Err(_) => {
            issues.push(RadrootsOrderIssue::DecisionListingAddressInvalid {
                event_id: decision.event_id.clone(),
            });
            valid = false;
        }
    }
    if decision.root_event_id != request.event_id {
        issues.push(RadrootsOrderIssue::DecisionRootMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.prev_event_id != request.event_id {
        issues.push(RadrootsOrderIssue::DecisionPreviousMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if let RadrootsOrderDecisionOutcome::Accepted {
        inventory_commitments,
    } = &decision.payload.decision
        && decision.payload.validate().is_ok()
        && !inventory_commitments_match_request(&request.payload.items, inventory_commitments)
    {
        issues.push(RadrootsOrderIssue::DecisionInventoryCommitmentMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    valid
}

fn validate_order_revision_proposal_record(
    request: &RadrootsOrderRequestRecord,
    proposal: &RadrootsOrderRevisionProposalRecord,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> bool {
    let mut valid = true;
    if proposal.payload.validate().is_err() {
        issues.push(RadrootsOrderIssue::RevisionProposalPayloadInvalid {
            event_id: proposal.event_id.clone(),
        });
        valid = false;
    }
    if proposal.payload.order_id != request.payload.order_id {
        issues.push(RadrootsOrderIssue::RevisionProposalOrderIdMismatch {
            event_id: proposal.event_id.clone(),
        });
        valid = false;
    }
    if proposal.author_pubkey != proposal.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::RevisionProposalAuthorMismatch {
            event_id: proposal.event_id.clone(),
        });
        valid = false;
    }
    if proposal.counterparty_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::RevisionProposalCounterpartyMismatch {
            event_id: proposal.event_id.clone(),
        });
        valid = false;
    }
    if proposal.payload.buyer_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::RevisionProposalBuyerMismatch {
            event_id: proposal.event_id.clone(),
        });
        valid = false;
    }
    if proposal.payload.seller_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::RevisionProposalSellerMismatch {
            event_id: proposal.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&proposal.payload.listing_addr) {
        Ok(listing_addr) => {
            if proposal.payload.listing_addr != request.payload.listing_addr
                || listing_addr.seller_pubkey != proposal.payload.seller_pubkey
            {
                issues.push(RadrootsOrderIssue::RevisionProposalListingMismatch {
                    event_id: proposal.event_id.clone(),
                });
                valid = false;
            }
        }
        Err(_) => {
            issues.push(RadrootsOrderIssue::RevisionProposalListingAddressInvalid {
                event_id: proposal.event_id.clone(),
            });
            valid = false;
        }
    }
    if proposal.root_event_id != request.event_id
        || proposal.payload.root_event_id != request.event_id
    {
        issues.push(RadrootsOrderIssue::RevisionProposalRootMismatch {
            event_id: proposal.event_id.clone(),
        });
        valid = false;
    }
    if proposal.prev_event_id.trim().is_empty()
        || proposal.prev_event_id == proposal.event_id
        || proposal.payload.prev_event_id != proposal.prev_event_id
    {
        issues.push(RadrootsOrderIssue::RevisionProposalPreviousMismatch {
            event_id: proposal.event_id.clone(),
        });
        valid = false;
    }
    valid
}

fn validate_order_revision_decision_record(
    request: &RadrootsOrderRequestRecord,
    decision: &RadrootsOrderRevisionDecisionRecord,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> bool {
    let mut valid = true;
    if decision.payload.validate().is_err() {
        issues.push(RadrootsOrderIssue::RevisionDecisionPayloadInvalid {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.payload.order_id != request.payload.order_id {
        issues.push(RadrootsOrderIssue::RevisionDecisionOrderIdMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.author_pubkey != decision.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::RevisionDecisionAuthorMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.counterparty_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::RevisionDecisionCounterpartyMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.payload.buyer_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::RevisionDecisionBuyerMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.payload.seller_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::RevisionDecisionSellerMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&decision.payload.listing_addr) {
        Ok(listing_addr) => {
            if decision.payload.listing_addr != request.payload.listing_addr
                || listing_addr.seller_pubkey != decision.payload.seller_pubkey
            {
                issues.push(RadrootsOrderIssue::RevisionDecisionListingMismatch {
                    event_id: decision.event_id.clone(),
                });
                valid = false;
            }
        }
        Err(_) => {
            issues.push(RadrootsOrderIssue::RevisionDecisionListingAddressInvalid {
                event_id: decision.event_id.clone(),
            });
            valid = false;
        }
    }
    if decision.root_event_id != request.event_id
        || decision.payload.root_event_id != request.event_id
    {
        issues.push(RadrootsOrderIssue::RevisionDecisionRootMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.prev_event_id.trim().is_empty()
        || decision.prev_event_id == decision.event_id
        || decision.payload.prev_event_id != decision.prev_event_id
    {
        issues.push(RadrootsOrderIssue::RevisionDecisionPreviousMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    valid
}

fn validate_order_cancellation_record(
    request: &RadrootsOrderRequestRecord,
    cancellation: &RadrootsOrderCancellationRecord,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> bool {
    let mut valid = true;
    if cancellation.payload.validate().is_err() {
        issues.push(RadrootsOrderIssue::CancellationPayloadInvalid {
            event_id: cancellation.event_id.clone(),
        });
        valid = false;
    }
    if cancellation.payload.order_id != request.payload.order_id {
        issues.push(RadrootsOrderIssue::CancellationOrderIdMismatch {
            event_id: cancellation.event_id.clone(),
        });
        valid = false;
    }
    if cancellation.author_pubkey != cancellation.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::CancellationAuthorMismatch {
            event_id: cancellation.event_id.clone(),
        });
        valid = false;
    }
    if cancellation.counterparty_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::CancellationCounterpartyMismatch {
            event_id: cancellation.event_id.clone(),
        });
        valid = false;
    }
    if cancellation.payload.buyer_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::CancellationBuyerMismatch {
            event_id: cancellation.event_id.clone(),
        });
        valid = false;
    }
    if cancellation.payload.seller_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::CancellationSellerMismatch {
            event_id: cancellation.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&cancellation.payload.listing_addr) {
        Ok(listing_addr) => {
            if cancellation.payload.listing_addr != request.payload.listing_addr
                || listing_addr.seller_pubkey != cancellation.payload.seller_pubkey
            {
                issues.push(RadrootsOrderIssue::CancellationListingMismatch {
                    event_id: cancellation.event_id.clone(),
                });
                valid = false;
            }
        }
        Err(_) => {
            issues.push(RadrootsOrderIssue::CancellationListingAddressInvalid {
                event_id: cancellation.event_id.clone(),
            });
            valid = false;
        }
    }
    if cancellation.root_event_id != request.event_id {
        issues.push(RadrootsOrderIssue::CancellationRootMismatch {
            event_id: cancellation.event_id.clone(),
        });
        valid = false;
    }
    if cancellation.prev_event_id.trim().is_empty()
        || cancellation.prev_event_id == cancellation.event_id
    {
        issues.push(RadrootsOrderIssue::CancellationPreviousMismatch {
            event_id: cancellation.event_id.clone(),
        });
        valid = false;
    }
    valid
}

fn decision_payload_issue(
    decision: &RadrootsOrderDecisionOutcome,
    event_id: &RadrootsEventId,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> bool {
    match decision {
        RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } => {
            if inventory_commitments.is_empty() {
                issues.push(RadrootsOrderIssue::DecisionMissingInventoryCommitments {
                    event_id: event_id.clone(),
                });
                true
            } else {
                false
            }
        }
        RadrootsOrderDecisionOutcome::Declined { reason } => {
            if reason.trim().is_empty() {
                issues.push(RadrootsOrderIssue::DecisionMissingReason {
                    event_id: event_id.clone(),
                });
                true
            } else {
                false
            }
        }
    }
}

fn unique_request_records(
    requests: Vec<RadrootsOrderRequestRecord>,
) -> Vec<RadrootsOrderRequestRecord> {
    unique_records_by_event_id(requests, |record| &record.event_id)
}

fn unique_decision_records(
    decisions: Vec<RadrootsOrderDecisionRecord>,
) -> Vec<RadrootsOrderDecisionRecord> {
    unique_records_by_event_id(decisions, |record| &record.event_id)
}

fn unique_revision_proposal_records(
    revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
) -> Vec<RadrootsOrderRevisionProposalRecord> {
    unique_records_by_event_id(revision_proposals, |record| &record.event_id)
}

fn unique_revision_decision_records(
    revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
) -> Vec<RadrootsOrderRevisionDecisionRecord> {
    unique_records_by_event_id(revision_decisions, |record| &record.event_id)
}

fn unique_cancellation_records(
    cancellations: Vec<RadrootsOrderCancellationRecord>,
) -> Vec<RadrootsOrderCancellationRecord> {
    unique_records_by_event_id(cancellations, |record| &record.event_id)
}

fn unique_records_by_event_id<T>(
    mut records: Vec<T>,
    event_id: impl Fn(&T) -> &RadrootsEventId,
) -> Vec<T> {
    let mut unique = Vec::new();
    records.sort_by(|left, right| event_id(left).cmp(event_id(right)));
    for record in records {
        if unique
            .iter()
            .all(|existing: &T| event_id(existing) != event_id(&record))
        {
            unique.push(record);
        }
    }
    unique
}

fn normalized_listing_inventory_bins<I>(
    bins: I,
) -> (
    Vec<RadrootsListingInventoryBinAccounting>,
    Vec<RadrootsListingInventoryAccountingIssue>,
)
where
    I: IntoIterator<Item = RadrootsListingInventoryBinAvailability>,
{
    let mut normalized: Vec<RadrootsListingInventoryBinAccounting> = Vec::new();
    let mut issues = Vec::new();
    for bin in bins {
        let bin_id = bin.bin_id;
        if let Some(existing) = normalized
            .iter_mut()
            .find(|existing| existing.bin_id == bin_id)
        {
            if let Some(next_count) = existing.available_count.checked_add(bin.available_count) {
                existing.available_count = next_count;
                existing.remaining_count = next_count;
            } else {
                existing.available_count = u64::MAX;
                existing.remaining_count = u64::MAX;
                issues.push(
                    RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                        bin_id: existing.bin_id.clone(),
                        event_ids: Vec::new(),
                    },
                );
            }
        } else {
            normalized.push(RadrootsListingInventoryBinAccounting {
                bin_id,
                available_count: bin.available_count,
                accepted_reserved_count: 0,
                remaining_count: bin.available_count,
                over_reserved: false,
                accepted_orders: Vec::new(),
            });
        }
    }
    normalized.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
    (normalized, issues)
}

fn listing_order_ids(
    requests: &[RadrootsOrderRequestRecord],
    decisions: &[RadrootsOrderDecisionRecord],
    revision_proposals: &[RadrootsOrderRevisionProposalRecord],
    revision_decisions: &[RadrootsOrderRevisionDecisionRecord],
    cancellations: &[RadrootsOrderCancellationRecord],
) -> Vec<RadrootsOrderId> {
    let mut order_ids = Vec::new();
    order_ids.extend(
        requests
            .iter()
            .map(|request| request.payload.order_id.clone()),
    );
    order_ids.extend(
        decisions
            .iter()
            .map(|decision| decision.payload.order_id.clone()),
    );
    order_ids.extend(
        revision_proposals
            .iter()
            .map(|proposal| proposal.payload.order_id.clone()),
    );
    order_ids.extend(
        revision_decisions
            .iter()
            .map(|decision| decision.payload.order_id.clone()),
    );
    order_ids.extend(
        cancellations
            .iter()
            .map(|cancellation| cancellation.payload.order_id.clone()),
    );
    sort_and_dedup_values(&mut order_ids);
    order_ids
}

fn add_accepted_inventory_reservations_from_economics(
    bins: &mut [RadrootsListingInventoryBinAccounting],
    order_id: &RadrootsOrderId,
    agreement_event_id: &RadrootsEventId,
    economics: &RadrootsOrderEconomics,
    issues: &mut Vec<RadrootsListingInventoryAccountingIssue>,
) {
    for item in &economics.items {
        if let Some(bin) = bins.iter_mut().find(|bin| bin.bin_id == item.bin_id) {
            add_inventory_reservation_event(
                bin,
                order_id,
                agreement_event_id,
                u64::from(item.bin_count),
                issues,
            );
        } else {
            issues.push(
                RadrootsListingInventoryAccountingIssue::UnknownInventoryBin {
                    bin_id: item.bin_id.clone(),
                    event_ids: vec![agreement_event_id.clone()],
                },
            );
        }
    }
}

fn add_inventory_reservation_event(
    bin: &mut RadrootsListingInventoryBinAccounting,
    order_id: &RadrootsOrderId,
    event_id: &RadrootsEventId,
    bin_count: u64,
    issues: &mut Vec<RadrootsListingInventoryAccountingIssue>,
) {
    if let Some(next_count) = bin.accepted_reserved_count.checked_add(bin_count) {
        bin.accepted_reserved_count = next_count;
        bin.accepted_orders
            .push(RadrootsListingInventoryOrderReservation {
                order_id: order_id.clone(),
                agreement_event_id: event_id.clone(),
                bin_count,
            });
    } else {
        issues.push(
            RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                bin_id: bin.bin_id.clone(),
                event_ids: vec![event_id.clone()],
            },
        );
    }
}

fn finish_inventory_accounting_bins(
    bins: &mut [RadrootsListingInventoryBinAccounting],
    issues: &mut Vec<RadrootsListingInventoryAccountingIssue>,
) {
    for bin in bins.iter_mut() {
        bin.accepted_orders.sort_by(|left, right| {
            left.order_id
                .cmp(&right.order_id)
                .then_with(|| left.agreement_event_id.cmp(&right.agreement_event_id))
        });
        bin.remaining_count = bin
            .available_count
            .saturating_sub(bin.accepted_reserved_count);
        bin.over_reserved = bin.accepted_reserved_count > bin.available_count;
        if bin.over_reserved {
            let mut event_ids = bin
                .accepted_orders
                .iter()
                .map(|reservation| reservation.agreement_event_id.clone())
                .collect::<Vec<_>>();
            sort_and_dedup_values(&mut event_ids);
            issues.push(RadrootsListingInventoryAccountingIssue::OverReserved {
                bin_id: bin.bin_id.clone(),
                available_count: bin.available_count,
                reserved_count: bin.accepted_reserved_count,
                event_ids,
            });
        }
    }
    bins.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
}

fn projection_issue_event_ids(issues: &[RadrootsOrderIssue]) -> Vec<RadrootsEventId> {
    let mut event_ids = Vec::new();
    for issue in issues {
        match issue {
            RadrootsOrderIssue::MissingRequest => {}
            RadrootsOrderIssue::MultipleRequests { event_ids: ids }
            | RadrootsOrderIssue::ConflictingDecisions { event_ids: ids }
            | RadrootsOrderIssue::ForkedLifecycle { event_ids: ids } => {
                event_ids.extend(ids.iter().cloned());
            }
            RadrootsOrderIssue::RequestPayloadInvalid { event_id }
            | RadrootsOrderIssue::RequestOrderIdMismatch { event_id }
            | RadrootsOrderIssue::RequestAuthorMismatch { event_id }
            | RadrootsOrderIssue::RequestListingAddressInvalid { event_id }
            | RadrootsOrderIssue::RequestSellerListingMismatch { event_id }
            | RadrootsOrderIssue::DecisionPayloadInvalid { event_id }
            | RadrootsOrderIssue::DecisionOrderIdMismatch { event_id }
            | RadrootsOrderIssue::DecisionAuthorMismatch { event_id }
            | RadrootsOrderIssue::DecisionCounterpartyMismatch { event_id }
            | RadrootsOrderIssue::DecisionBuyerMismatch { event_id }
            | RadrootsOrderIssue::DecisionSellerMismatch { event_id }
            | RadrootsOrderIssue::DecisionListingAddressInvalid { event_id }
            | RadrootsOrderIssue::DecisionListingMismatch { event_id }
            | RadrootsOrderIssue::DecisionRootMismatch { event_id }
            | RadrootsOrderIssue::DecisionPreviousMismatch { event_id }
            | RadrootsOrderIssue::DecisionMissingInventoryCommitments { event_id }
            | RadrootsOrderIssue::DecisionInventoryCommitmentMismatch { event_id }
            | RadrootsOrderIssue::DecisionMissingReason { event_id }
            | RadrootsOrderIssue::RevisionProposalPayloadInvalid { event_id }
            | RadrootsOrderIssue::RevisionProposalOrderIdMismatch { event_id }
            | RadrootsOrderIssue::RevisionProposalAuthorMismatch { event_id }
            | RadrootsOrderIssue::RevisionProposalCounterpartyMismatch { event_id }
            | RadrootsOrderIssue::RevisionProposalBuyerMismatch { event_id }
            | RadrootsOrderIssue::RevisionProposalSellerMismatch { event_id }
            | RadrootsOrderIssue::RevisionProposalListingAddressInvalid { event_id }
            | RadrootsOrderIssue::RevisionProposalListingMismatch { event_id }
            | RadrootsOrderIssue::RevisionProposalRootMismatch { event_id }
            | RadrootsOrderIssue::RevisionProposalPreviousMismatch { event_id }
            | RadrootsOrderIssue::RevisionDecisionWithoutProposal { event_id }
            | RadrootsOrderIssue::RevisionDecisionPayloadInvalid { event_id }
            | RadrootsOrderIssue::RevisionDecisionOrderIdMismatch { event_id }
            | RadrootsOrderIssue::RevisionDecisionAuthorMismatch { event_id }
            | RadrootsOrderIssue::RevisionDecisionCounterpartyMismatch { event_id }
            | RadrootsOrderIssue::RevisionDecisionBuyerMismatch { event_id }
            | RadrootsOrderIssue::RevisionDecisionSellerMismatch { event_id }
            | RadrootsOrderIssue::RevisionDecisionListingAddressInvalid { event_id }
            | RadrootsOrderIssue::RevisionDecisionListingMismatch { event_id }
            | RadrootsOrderIssue::RevisionDecisionRootMismatch { event_id }
            | RadrootsOrderIssue::RevisionDecisionPreviousMismatch { event_id }
            | RadrootsOrderIssue::RevisionDecisionRevisionIdMismatch { event_id }
            | RadrootsOrderIssue::CancellationWithoutCancellableOrder { event_id }
            | RadrootsOrderIssue::CancellationPayloadInvalid { event_id }
            | RadrootsOrderIssue::CancellationOrderIdMismatch { event_id }
            | RadrootsOrderIssue::CancellationAuthorMismatch { event_id }
            | RadrootsOrderIssue::CancellationCounterpartyMismatch { event_id }
            | RadrootsOrderIssue::CancellationBuyerMismatch { event_id }
            | RadrootsOrderIssue::CancellationSellerMismatch { event_id }
            | RadrootsOrderIssue::CancellationListingAddressInvalid { event_id }
            | RadrootsOrderIssue::CancellationListingMismatch { event_id }
            | RadrootsOrderIssue::CancellationRootMismatch { event_id }
            | RadrootsOrderIssue::CancellationPreviousMismatch { event_id } => {
                event_ids.push(event_id.clone());
            }
        }
    }
    sort_and_dedup_values(&mut event_ids);
    event_ids
}

fn parse_public_listing_addr(
    value: impl AsRef<str>,
) -> Result<RadrootsPublicListingAddress, RadrootsOrderCanonicalizationError> {
    parse_public_listing_address(value).map_err(|error| match error {
        RadrootsPublicListingAddressError::InvalidAddress(error) => {
            RadrootsOrderCanonicalizationError::InvalidListingAddress(error.to_string())
        }
        RadrootsPublicListingAddressError::InvalidListingKind { .. } => {
            RadrootsOrderCanonicalizationError::InvalidListingKind
        }
        RadrootsPublicListingAddressError::InvalidKind { .. } => {
            RadrootsOrderCanonicalizationError::InvalidListingKind
        }
    })
}

fn canonicalize_items(
    items: &mut [RadrootsOrderItem],
) -> Result<(), RadrootsOrderCanonicalizationError> {
    if items.is_empty() {
        return Err(RadrootsOrderCanonicalizationError::MissingItems);
    }
    for (index, item) in items.iter().enumerate() {
        if item.bin_count == 0 {
            return Err(RadrootsOrderCanonicalizationError::InvalidBinCount { index });
        }
    }
    items.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
    Ok(())
}

fn canonicalize_decision(
    decision: &mut RadrootsOrderDecisionOutcome,
) -> Result<(), RadrootsOrderCanonicalizationError> {
    match decision {
        RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } => {
            if inventory_commitments.is_empty() {
                return Err(RadrootsOrderCanonicalizationError::MissingInventoryCommitments);
            }
            for (index, commitment) in inventory_commitments.iter().enumerate() {
                if commitment.bin_count == 0 {
                    return Err(
                        RadrootsOrderCanonicalizationError::InvalidInventoryCommitmentCount {
                            index,
                        },
                    );
                }
            }
            inventory_commitments.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
            Ok(())
        }
        RadrootsOrderDecisionOutcome::Declined { reason } => {
            if reason.trim().is_empty() {
                return Err(RadrootsOrderCanonicalizationError::EmptyField("reason"));
            }
            *reason = reason.trim().to_string();
            Ok(())
        }
    }
}

fn inventory_commitments_match_request(
    items: &[RadrootsOrderItem],
    commitments: &[RadrootsOrderInventoryCommitment],
) -> bool {
    if items.len() != commitments.len() {
        return false;
    }
    let mut expected = items.to_vec();
    expected.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
    let mut actual = commitments.to_vec();
    actual.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
    expected
        .iter()
        .zip(actual.iter())
        .all(|(item, commitment)| {
            item.bin_id == commitment.bin_id && item.bin_count == commitment.bin_count
        })
}

fn sort_and_dedup_values<T: Ord>(values: &mut Vec<T>) {
    values.sort();
    values.dedup();
}

fn inventory_issue_sort_key(
    left: &RadrootsListingInventoryAccountingIssue,
    right: &RadrootsListingInventoryAccountingIssue,
) -> core::cmp::Ordering {
    inventory_issue_rank(left)
        .cmp(&inventory_issue_rank(right))
        .then_with(|| inventory_issue_id(left).cmp(inventory_issue_id(right)))
        .then_with(|| inventory_issue_event_ids(left).cmp(inventory_issue_event_ids(right)))
}

fn inventory_issue_rank(issue: &RadrootsListingInventoryAccountingIssue) -> u8 {
    match issue {
        RadrootsListingInventoryAccountingIssue::InvalidOrder { .. } => 0,
        RadrootsListingInventoryAccountingIssue::ArithmeticOverflow { .. } => 1,
        RadrootsListingInventoryAccountingIssue::UnknownInventoryBin { .. } => 2,
        RadrootsListingInventoryAccountingIssue::OverReserved { .. } => 3,
    }
}

fn inventory_issue_id(issue: &RadrootsListingInventoryAccountingIssue) -> &str {
    match issue {
        RadrootsListingInventoryAccountingIssue::InvalidOrder { order_id, .. } => order_id,
        RadrootsListingInventoryAccountingIssue::ArithmeticOverflow { bin_id, .. }
        | RadrootsListingInventoryAccountingIssue::UnknownInventoryBin { bin_id, .. }
        | RadrootsListingInventoryAccountingIssue::OverReserved { bin_id, .. } => bin_id,
    }
}

fn inventory_issue_event_ids(
    issue: &RadrootsListingInventoryAccountingIssue,
) -> &[RadrootsEventId] {
    match issue {
        RadrootsListingInventoryAccountingIssue::InvalidOrder { event_ids, .. }
        | RadrootsListingInventoryAccountingIssue::ArithmeticOverflow { event_ids, .. }
        | RadrootsListingInventoryAccountingIssue::UnknownInventoryBin { event_ids, .. }
        | RadrootsListingInventoryAccountingIssue::OverReserved { event_ids, .. } => event_ids,
    }
}

fn order_issue_sort_key(
    left: &RadrootsOrderIssue,
    right: &RadrootsOrderIssue,
) -> core::cmp::Ordering {
    order_issue_rank(left)
        .cmp(&order_issue_rank(right))
        .then_with(|| {
            projection_issue_event_ids(core::slice::from_ref(left))
                .cmp(&projection_issue_event_ids(core::slice::from_ref(right)))
        })
}

fn order_issue_rank(issue: &RadrootsOrderIssue) -> u8 {
    match issue {
        RadrootsOrderIssue::MissingRequest => 0,
        RadrootsOrderIssue::MultipleRequests { .. } => 1,
        RadrootsOrderIssue::RequestPayloadInvalid { .. } => 2,
        RadrootsOrderIssue::RequestOrderIdMismatch { .. } => 3,
        RadrootsOrderIssue::RequestAuthorMismatch { .. } => 4,
        RadrootsOrderIssue::RequestListingAddressInvalid { .. } => 5,
        RadrootsOrderIssue::RequestSellerListingMismatch { .. } => 6,
        RadrootsOrderIssue::DecisionPayloadInvalid { .. } => 7,
        RadrootsOrderIssue::DecisionOrderIdMismatch { .. } => 8,
        RadrootsOrderIssue::DecisionAuthorMismatch { .. } => 9,
        RadrootsOrderIssue::DecisionCounterpartyMismatch { .. } => 10,
        RadrootsOrderIssue::DecisionBuyerMismatch { .. } => 11,
        RadrootsOrderIssue::DecisionSellerMismatch { .. } => 12,
        RadrootsOrderIssue::DecisionListingAddressInvalid { .. } => 13,
        RadrootsOrderIssue::DecisionListingMismatch { .. } => 14,
        RadrootsOrderIssue::DecisionRootMismatch { .. } => 15,
        RadrootsOrderIssue::DecisionPreviousMismatch { .. } => 16,
        RadrootsOrderIssue::DecisionMissingInventoryCommitments { .. } => 17,
        RadrootsOrderIssue::DecisionInventoryCommitmentMismatch { .. } => 18,
        RadrootsOrderIssue::DecisionMissingReason { .. } => 19,
        RadrootsOrderIssue::ConflictingDecisions { .. } => 20,
        RadrootsOrderIssue::RevisionProposalPayloadInvalid { .. } => 21,
        RadrootsOrderIssue::RevisionProposalOrderIdMismatch { .. } => 22,
        RadrootsOrderIssue::RevisionProposalAuthorMismatch { .. } => 23,
        RadrootsOrderIssue::RevisionProposalCounterpartyMismatch { .. } => 24,
        RadrootsOrderIssue::RevisionProposalBuyerMismatch { .. } => 25,
        RadrootsOrderIssue::RevisionProposalSellerMismatch { .. } => 26,
        RadrootsOrderIssue::RevisionProposalListingAddressInvalid { .. } => 27,
        RadrootsOrderIssue::RevisionProposalListingMismatch { .. } => 28,
        RadrootsOrderIssue::RevisionProposalRootMismatch { .. } => 29,
        RadrootsOrderIssue::RevisionProposalPreviousMismatch { .. } => 30,
        RadrootsOrderIssue::RevisionDecisionWithoutProposal { .. } => 31,
        RadrootsOrderIssue::RevisionDecisionPayloadInvalid { .. } => 32,
        RadrootsOrderIssue::RevisionDecisionOrderIdMismatch { .. } => 33,
        RadrootsOrderIssue::RevisionDecisionAuthorMismatch { .. } => 34,
        RadrootsOrderIssue::RevisionDecisionCounterpartyMismatch { .. } => 35,
        RadrootsOrderIssue::RevisionDecisionBuyerMismatch { .. } => 36,
        RadrootsOrderIssue::RevisionDecisionSellerMismatch { .. } => 37,
        RadrootsOrderIssue::RevisionDecisionListingAddressInvalid { .. } => 38,
        RadrootsOrderIssue::RevisionDecisionListingMismatch { .. } => 39,
        RadrootsOrderIssue::RevisionDecisionRootMismatch { .. } => 40,
        RadrootsOrderIssue::RevisionDecisionPreviousMismatch { .. } => 41,
        RadrootsOrderIssue::RevisionDecisionRevisionIdMismatch { .. } => 42,
        RadrootsOrderIssue::CancellationWithoutCancellableOrder { .. } => 43,
        RadrootsOrderIssue::CancellationPayloadInvalid { .. } => 44,
        RadrootsOrderIssue::CancellationOrderIdMismatch { .. } => 45,
        RadrootsOrderIssue::CancellationAuthorMismatch { .. } => 46,
        RadrootsOrderIssue::CancellationCounterpartyMismatch { .. } => 47,
        RadrootsOrderIssue::CancellationBuyerMismatch { .. } => 48,
        RadrootsOrderIssue::CancellationSellerMismatch { .. } => 49,
        RadrootsOrderIssue::CancellationListingAddressInvalid { .. } => 50,
        RadrootsOrderIssue::CancellationListingMismatch { .. } => 51,
        RadrootsOrderIssue::CancellationRootMismatch { .. } => 52,
        RadrootsOrderIssue::CancellationPreviousMismatch { .. } => 53,
        RadrootsOrderIssue::ForkedLifecycle { .. } => 54,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RadrootsListingInventoryAccountingInputs, RadrootsListingInventoryBinAvailability,
        RadrootsOrderCancellationRecord, RadrootsOrderDecisionRecord, RadrootsOrderEventRecord,
        RadrootsOrderReductionInputs, RadrootsOrderRequestRecord,
        RadrootsOrderRevisionDecisionRecord, RadrootsOrderRevisionProposalRecord,
        RadrootsOrderStatus, reduce_listing_inventory_accounting, reduce_order_event_records,
        reduce_order_events,
    };
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
    };
    use radroots_events::{
        ids::{
            RadrootsEventId, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsOrderId,
            RadrootsOrderQuoteId, RadrootsOrderRevisionId, RadrootsPublicKey,
        },
        kinds::KIND_LISTING,
        order::{
            RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
            RadrootsOrderEconomicItem, RadrootsOrderEconomics, RadrootsOrderInventoryCommitment,
            RadrootsOrderItem, RadrootsOrderPricingBasis, RadrootsOrderRequest,
            RadrootsOrderRevisionDecision, RadrootsOrderRevisionOutcome,
            RadrootsOrderRevisionProposal,
        },
    };

    const BUYER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn event_id(raw: u8) -> RadrootsEventId {
        RadrootsEventId::parse(format!("{raw:064x}")).expect("event id")
    }

    fn public_key(raw: &str) -> RadrootsPublicKey {
        RadrootsPublicKey::parse(raw).expect("public key")
    }

    fn order_id(raw: &str) -> RadrootsOrderId {
        RadrootsOrderId::parse(raw).expect("order id")
    }

    fn revision_id(raw: &str) -> RadrootsOrderRevisionId {
        RadrootsOrderRevisionId::parse(raw).expect("revision id")
    }

    fn quote_id(raw: &str) -> RadrootsOrderQuoteId {
        RadrootsOrderQuoteId::parse(raw).expect("quote id")
    }

    fn bin_id(raw: &str) -> RadrootsInventoryBinId {
        RadrootsInventoryBinId::parse(raw).expect("bin id")
    }

    fn listing_addr() -> RadrootsListingAddress {
        RadrootsListingAddress::parse(format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"))
            .expect("listing address")
    }

    fn economics(bin_count: u32) -> RadrootsOrderEconomics {
        let currency = RadrootsCoreCurrency::USD;
        let amount = RadrootsCoreDecimal::from(1200u32);
        RadrootsOrderEconomics {
            quote_id: quote_id("quote-1"),
            quote_version: 1,
            pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
            currency,
            items: vec![RadrootsOrderEconomicItem {
                bin_id: bin_id("bin-1"),
                bin_count,
                quantity_amount: RadrootsCoreDecimal::ONE,
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: amount,
                unit_price_currency: currency,
                line_subtotal: RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(u64::from(bin_count) * 1200),
                    currency,
                ),
            }],
            discounts: Vec::new(),
            adjustments: Vec::new(),
            subtotal: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(u64::from(bin_count) * 1200),
                currency,
            ),
            discount_total: RadrootsCoreMoney::zero(currency),
            adjustment_total: RadrootsCoreMoney::zero(currency),
            total: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(u64::from(bin_count) * 1200),
                currency,
            ),
        }
    }

    fn request_record() -> RadrootsOrderRequestRecord {
        RadrootsOrderRequestRecord {
            event_id: event_id(1),
            author_pubkey: public_key(BUYER),
            payload: RadrootsOrderRequest {
                order_id: order_id("order-1"),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                items: vec![RadrootsOrderItem {
                    bin_id: bin_id("bin-1"),
                    bin_count: 2,
                }],
                economics: economics(2),
            },
        }
    }

    fn accepted_decision() -> RadrootsOrderDecisionRecord {
        RadrootsOrderDecisionRecord {
            event_id: event_id(2),
            author_pubkey: public_key(SELLER),
            counterparty_pubkey: public_key(BUYER),
            root_event_id: event_id(1),
            prev_event_id: event_id(1),
            payload: RadrootsOrderDecision {
                order_id: order_id("order-1"),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                decision: RadrootsOrderDecisionOutcome::Accepted {
                    inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                        bin_id: bin_id("bin-1"),
                        bin_count: 2,
                    }],
                },
            },
        }
    }

    fn declined_decision() -> RadrootsOrderDecisionRecord {
        RadrootsOrderDecisionRecord {
            event_id: event_id(2),
            author_pubkey: public_key(SELLER),
            counterparty_pubkey: public_key(BUYER),
            root_event_id: event_id(1),
            prev_event_id: event_id(1),
            payload: RadrootsOrderDecision {
                order_id: order_id("order-1"),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                decision: RadrootsOrderDecisionOutcome::Declined {
                    reason: "not available".into(),
                },
            },
        }
    }

    fn revision_proposal() -> RadrootsOrderRevisionProposalRecord {
        RadrootsOrderRevisionProposalRecord {
            event_id: event_id(3),
            author_pubkey: public_key(SELLER),
            counterparty_pubkey: public_key(BUYER),
            root_event_id: event_id(1),
            prev_event_id: event_id(1),
            payload: RadrootsOrderRevisionProposal {
                revision_id: revision_id("revision-1"),
                order_id: order_id("order-1"),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                root_event_id: event_id(1),
                prev_event_id: event_id(1),
                items: vec![RadrootsOrderItem {
                    bin_id: bin_id("bin-1"),
                    bin_count: 1,
                }],
                economics: economics(1),
                reason: "one bin remains".into(),
            },
        }
    }

    fn accepted_revision_decision() -> RadrootsOrderRevisionDecisionRecord {
        RadrootsOrderRevisionDecisionRecord {
            event_id: event_id(4),
            author_pubkey: public_key(BUYER),
            counterparty_pubkey: public_key(SELLER),
            root_event_id: event_id(1),
            prev_event_id: event_id(3),
            payload: RadrootsOrderRevisionDecision {
                revision_id: revision_id("revision-1"),
                order_id: order_id("order-1"),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                root_event_id: event_id(1),
                prev_event_id: event_id(3),
                decision: RadrootsOrderRevisionOutcome::Accepted,
            },
        }
    }

    fn cancellation(prev_event_id: RadrootsEventId) -> RadrootsOrderCancellationRecord {
        RadrootsOrderCancellationRecord {
            event_id: event_id(5),
            author_pubkey: public_key(BUYER),
            counterparty_pubkey: public_key(SELLER),
            root_event_id: event_id(1),
            prev_event_id,
            payload: RadrootsOrderCancellation {
                order_id: order_id("order-1"),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                reason: "changed plans".into(),
            },
        }
    }

    fn reduce(
        decisions: Vec<RadrootsOrderDecisionRecord>,
        revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
        revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
        cancellations: Vec<RadrootsOrderCancellationRecord>,
    ) -> super::RadrootsOrderProjection {
        reduce_order_events(
            &order_id("order-1"),
            RadrootsOrderReductionInputs {
                requests: vec![request_record()],
                decisions,
                revision_proposals,
                revision_decisions,
                cancellations,
            },
        )
    }

    #[test]
    fn reducer_projects_requested_order() {
        let projection = reduce(Vec::new(), Vec::new(), Vec::new(), Vec::new());

        assert_eq!(projection.issues, Vec::new());
        assert_eq!(projection.status, RadrootsOrderStatus::Requested);
        assert_eq!(projection.request_event_id, Some(event_id(1)));
        assert!(!projection.lifecycle_terminal);
        assert!(projection.agreement_event_id.is_none());
    }

    #[test]
    fn reducer_projects_accepted_order_agreement() {
        let projection = reduce(
            vec![accepted_decision()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(projection.decision_event_id, Some(event_id(2)));
        assert_eq!(projection.agreement_event_id, Some(event_id(2)));
        assert!(projection.lifecycle_terminal);
    }

    #[test]
    fn reducer_projects_declined_order() {
        let projection = reduce(
            vec![declined_decision()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Declined);
        assert_eq!(projection.decision_event_id, Some(event_id(2)));
        assert!(projection.lifecycle_terminal);
    }

    #[test]
    fn reducer_projects_revision_acceptance_as_agreement() {
        let projection = reduce(
            Vec::new(),
            vec![revision_proposal()],
            vec![accepted_revision_decision()],
            Vec::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(projection.agreement_event_id, Some(event_id(4)));
        assert_eq!(
            projection.economics.expect("economics").items[0].bin_count,
            1
        );
        assert!(projection.lifecycle_terminal);
    }

    #[test]
    fn reducer_allows_pre_agreement_cancellation() {
        let projection = reduce(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![cancellation(event_id(1))],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Cancelled);
        assert_eq!(projection.cancellation_event_id, Some(event_id(5)));
        assert!(projection.lifecycle_terminal);
    }

    #[test]
    fn reducer_rejects_cancellation_after_agreement() {
        let projection = reduce(
            vec![accepted_decision()],
            Vec::new(),
            Vec::new(),
            vec![cancellation(event_id(2))],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(projection.lifecycle_terminal);
    }

    #[test]
    fn reducer_groups_event_records() {
        let projection = reduce_order_event_records(
            &order_id("order-1"),
            vec![
                RadrootsOrderEventRecord::Request(request_record()),
                RadrootsOrderEventRecord::Decision(accepted_decision()),
            ],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(projection.agreement_event_id, Some(event_id(2)));
    }

    #[test]
    fn inventory_accounting_reserves_only_accepted_agreements() {
        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            &event_id(9),
            RadrootsListingInventoryAccountingInputs {
                bins: vec![RadrootsListingInventoryBinAvailability {
                    bin_id: bin_id("bin-1"),
                    available_count: 3,
                }],
                requests: vec![request_record()],
                decisions: vec![accepted_decision()],
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );

        assert_eq!(projection.bins[0].accepted_reserved_count, 2);
        assert_eq!(projection.bins[0].remaining_count, 1);
        assert_eq!(
            projection.bins[0].accepted_orders[0].agreement_event_id,
            event_id(2)
        );
    }
}

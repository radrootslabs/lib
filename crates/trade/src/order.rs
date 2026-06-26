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
use crate::workflow::{RadrootsTradeWorkflowState, inventory_reservations_from_commitments};

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
    #[error("seller accepted decisions must contain at least one inventory commitment")]
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

    match message_type {
        RadrootsOrderEventType::OrderRequested => {
            let envelope = order_request_from_event(event)?;
            Ok(RadrootsOrderEventRecord::Request(
                RadrootsOrderRequestRecord {
                    event_id,
                    author_pubkey,
                    payload: envelope.payload,
                },
            ))
        }
        RadrootsOrderEventType::OrderDecision => {
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
        RadrootsOrderEventType::OrderRevisionProposed => {
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
        RadrootsOrderEventType::OrderRevisionDecision => {
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
        RadrootsOrderEventType::OrderCancelled => {
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
    }
}

#[cfg(feature = "event_store")]
#[derive(Debug, Error)]
pub enum RadrootsOrderStoreQueryError {
    #[error("{0}")]
    Store(#[from] RadrootsEventStoreError),
    #[error("{0}")]
    Projection(#[from] crate::projection::RadrootsTradeProjectionError),
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
    crate::projection::trade_projection_query_for_order_id(store, order_id, limit)
        .await
        .map_err(Into::into)
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
pub enum RadrootsOrderIssue {
    MissingRequest,
    MultipleRequests {
        event_ids: Vec<RadrootsEventId>,
    },
    RequestPayloadInvalid {
        event_id: RadrootsEventId,
    },
    RequestOrderIdMismatch {
        event_id: RadrootsEventId,
    },
    RequestAuthorMismatch {
        event_id: RadrootsEventId,
    },
    RequestListingAddressInvalid {
        event_id: RadrootsEventId,
    },
    RequestSellerListingMismatch {
        event_id: RadrootsEventId,
    },
    DecisionPayloadInvalid {
        event_id: RadrootsEventId,
    },
    DecisionOrderIdMismatch {
        event_id: RadrootsEventId,
    },
    DecisionAuthorMismatch {
        event_id: RadrootsEventId,
    },
    DecisionCounterpartyMismatch {
        event_id: RadrootsEventId,
    },
    DecisionBuyerMismatch {
        event_id: RadrootsEventId,
    },
    DecisionSellerMismatch {
        event_id: RadrootsEventId,
    },
    DecisionListingAddressInvalid {
        event_id: RadrootsEventId,
    },
    DecisionListingMismatch {
        event_id: RadrootsEventId,
    },
    DecisionRootMismatch {
        event_id: RadrootsEventId,
    },
    DecisionPreviousMismatch {
        event_id: RadrootsEventId,
    },
    DecisionMissingInventoryCommitments {
        event_id: RadrootsEventId,
    },
    DecisionInventoryCommitmentMismatch {
        event_id: RadrootsEventId,
    },
    DecisionMissingReason {
        event_id: RadrootsEventId,
    },
    ConflictingDecisions {
        event_ids: Vec<RadrootsEventId>,
    },
    RevisionProposalPayloadInvalid {
        event_id: RadrootsEventId,
    },
    RevisionProposalOrderIdMismatch {
        event_id: RadrootsEventId,
    },
    RevisionProposalAuthorMismatch {
        event_id: RadrootsEventId,
    },
    RevisionProposalCounterpartyMismatch {
        event_id: RadrootsEventId,
    },
    RevisionProposalBuyerMismatch {
        event_id: RadrootsEventId,
    },
    RevisionProposalSellerMismatch {
        event_id: RadrootsEventId,
    },
    RevisionProposalListingAddressInvalid {
        event_id: RadrootsEventId,
    },
    RevisionProposalListingMismatch {
        event_id: RadrootsEventId,
    },
    RevisionProposalRootMismatch {
        event_id: RadrootsEventId,
    },
    RevisionProposalPreviousMismatch {
        event_id: RadrootsEventId,
    },
    RevisionDecisionWithoutProposal {
        event_id: RadrootsEventId,
    },
    RevisionDecisionPayloadInvalid {
        event_id: RadrootsEventId,
    },
    RevisionDecisionOrderIdMismatch {
        event_id: RadrootsEventId,
    },
    RevisionDecisionAuthorMismatch {
        event_id: RadrootsEventId,
    },
    RevisionDecisionCounterpartyMismatch {
        event_id: RadrootsEventId,
    },
    RevisionDecisionBuyerMismatch {
        event_id: RadrootsEventId,
    },
    RevisionDecisionSellerMismatch {
        event_id: RadrootsEventId,
    },
    RevisionDecisionListingAddressInvalid {
        event_id: RadrootsEventId,
    },
    RevisionDecisionListingMismatch {
        event_id: RadrootsEventId,
    },
    RevisionDecisionRootMismatch {
        event_id: RadrootsEventId,
    },
    RevisionDecisionPreviousMismatch {
        event_id: RadrootsEventId,
    },
    RevisionDecisionRevisionIdMismatch {
        event_id: RadrootsEventId,
    },
    CancellationWithoutCancellableOrder {
        event_id: RadrootsEventId,
    },
    CancellationPayloadInvalid {
        event_id: RadrootsEventId,
    },
    CancellationOrderIdMismatch {
        event_id: RadrootsEventId,
    },
    CancellationAuthorMismatch {
        event_id: RadrootsEventId,
    },
    CancellationCounterpartyMismatch {
        event_id: RadrootsEventId,
    },
    CancellationBuyerMismatch {
        event_id: RadrootsEventId,
    },
    CancellationSellerMismatch {
        event_id: RadrootsEventId,
    },
    CancellationListingAddressInvalid {
        event_id: RadrootsEventId,
    },
    CancellationListingMismatch {
        event_id: RadrootsEventId,
    },
    CancellationRootMismatch {
        event_id: RadrootsEventId,
    },
    CancellationPreviousMismatch {
        event_id: RadrootsEventId,
    },
    ForkedLifecycle {
        event_ids: Vec<RadrootsEventId>,
    },
    ValidationReceiptWithoutPendingAgreement {
        event_id: RadrootsEventId,
    },
    ValidationReceiptOrderIdMismatch {
        event_id: RadrootsEventId,
    },
    ValidationReceiptTypeMismatch {
        event_id: RadrootsEventId,
    },
    ValidationReceiptRootMismatch {
        event_id: RadrootsEventId,
    },
    ValidationReceiptTargetMismatch {
        event_id: RadrootsEventId,
    },
    ValidationReceiptListingMismatch {
        event_id: RadrootsEventId,
    },
    ConflictingValidationReceipts {
        event_ids: Vec<RadrootsEventId>,
    },
    DeterministicValidationFailure {
        event_id: RadrootsEventId,
        reason: String,
    },
    StaleListingEvent {
        expected_event_id: RadrootsEventId,
        current_event_id: RadrootsEventId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderProjection {
    pub order_id: RadrootsOrderId,
    pub status: RadrootsTradeWorkflowState,
    pub request_event_id: Option<RadrootsEventId>,
    pub decision_event_id: Option<RadrootsEventId>,
    pub cancellation_event_id: Option<RadrootsEventId>,
    pub validation_receipt_event_id: Option<RadrootsEventId>,
    pub lifecycle_terminal: bool,
    pub economics: Option<RadrootsOrderEconomics>,
    pub agreement_event_id: Option<RadrootsEventId>,
    pub pending_revision_event_id: Option<RadrootsEventId>,
    pub pending_inventory_reservations: Vec<RadrootsOrderInventoryCommitment>,
    pub committed_inventory_reservations: Vec<RadrootsOrderInventoryCommitment>,
    pub listing_addr: Option<RadrootsListingAddress>,
    pub buyer_pubkey: Option<RadrootsPublicKey>,
    pub seller_pubkey: Option<RadrootsPublicKey>,
    pub last_event_id: Option<RadrootsEventId>,
    pub issues: Vec<RadrootsOrderIssue>,
}

impl RadrootsOrderProjection {
    pub(crate) fn finish_issue_state(&mut self) {
        self.issues.sort_by(order_issue_sort_key);
        if self.last_event_id.is_none() {
            self.last_event_id = projection_issue_event_ids(&self.issues).into_iter().last();
        }
    }
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
    pub pending_reserved_count: u64,
    pub committed_reserved_count: u64,
    pub remaining_count: u64,
    pub over_reserved: bool,
    pub pending_orders: Vec<RadrootsListingInventoryOrderReservation>,
    pub committed_orders: Vec<RadrootsListingInventoryOrderReservation>,
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

pub(crate) fn reduce_grouped_order_event_records(
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
        return empty_projection(order_id, RadrootsTradeWorkflowState::Missing, false);
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
            RadrootsTradeWorkflowState::AgreedPendingRhi => {
                for (agreement_event_id, economics) in projection
                    .agreement_event_id
                    .iter()
                    .zip(projection.economics.iter())
                {
                    add_pending_inventory_reservations_from_economics(
                        &mut bins,
                        &order_id,
                        agreement_event_id,
                        economics,
                        &mut issues,
                    );
                }
            }
            RadrootsTradeWorkflowState::Cancelled => cancelled_order_ids.push(order_id),
            RadrootsTradeWorkflowState::Declined => declined_order_ids.push(order_id),
            RadrootsTradeWorkflowState::Invalid => {
                let mut event_ids = projection_issue_event_ids(&projection.issues);
                if event_ids.is_empty() {
                    event_ids = fallback_order_event_ids(
                        &order_requests,
                        &order_decisions,
                        &order_revision_proposals,
                        &order_revision_decisions,
                        &order_cancellations,
                    );
                }
                invalid_event_ids.extend(event_ids.iter().cloned());
                issues.push(RadrootsListingInventoryAccountingIssue::InvalidOrder {
                    order_id,
                    event_ids,
                });
            }
            RadrootsTradeWorkflowState::Missing
            | RadrootsTradeWorkflowState::Requested
            | RadrootsTradeWorkflowState::RevisionProposed
            | RadrootsTradeWorkflowState::Committed => {}
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

fn fallback_order_event_ids(
    requests: &[RadrootsOrderRequestRecord],
    decisions: &[RadrootsOrderDecisionRecord],
    revision_proposals: &[RadrootsOrderRevisionProposalRecord],
    revision_decisions: &[RadrootsOrderRevisionDecisionRecord],
    cancellations: &[RadrootsOrderCancellationRecord],
) -> Vec<RadrootsEventId> {
    let mut event_ids = Vec::new();
    event_ids.extend(requests.iter().map(|request| request.event_id.clone()));
    event_ids.extend(decisions.iter().map(|decision| decision.event_id.clone()));
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
    event_ids.extend(
        cancellations
            .iter()
            .map(|cancellation| cancellation.event_id.clone()),
    );
    sort_and_dedup_values(&mut event_ids);
    event_ids
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

    let mut projection =
        request_projection(order_id, request, RadrootsTradeWorkflowState::Cancelled);
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
                request_projection(order_id, request, RadrootsTradeWorkflowState::Requested)
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
                    let mut projection = request_projection(
                        order_id,
                        request,
                        RadrootsTradeWorkflowState::RevisionProposed,
                    );
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
            let mut projection = request_projection(
                order_id,
                request,
                RadrootsTradeWorkflowState::AgreedPendingRhi,
            );
            projection.decision_event_id = Some(decision.event_id.clone());
            projection.economics = Some(request.payload.economics.clone());
            projection.agreement_event_id = Some(decision.event_id.clone());
            projection.pending_inventory_reservations =
                inventory_commitments_from_items(&request.payload.items);
            projection.last_event_id = Some(decision.event_id.clone());
            projection
        }
        RadrootsOrderDecisionOutcome::Declined { .. } => {
            let mut projection =
                request_projection(order_id, request, RadrootsTradeWorkflowState::Declined);
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
            let mut projection = request_projection(
                order_id,
                request,
                RadrootsTradeWorkflowState::AgreedPendingRhi,
            );
            projection.economics = Some(proposal.payload.economics.clone());
            projection.agreement_event_id = Some(decision.event_id.clone());
            projection.pending_inventory_reservations =
                inventory_commitments_from_items(&proposal.payload.items);
            projection.last_event_id = Some(decision.event_id.clone());
            projection
        }
        RadrootsOrderRevisionOutcome::Declined { .. } => {
            let mut projection =
                request_projection(order_id, request, RadrootsTradeWorkflowState::Declined);
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
    status: RadrootsTradeWorkflowState,
) -> RadrootsOrderProjection {
    RadrootsOrderProjection {
        order_id: order_id.clone(),
        status,
        request_event_id: Some(request.event_id.clone()),
        decision_event_id: None,
        cancellation_event_id: None,
        validation_receipt_event_id: None,
        lifecycle_terminal: false,
        economics: Some(request.payload.economics.clone()),
        agreement_event_id: None,
        pending_revision_event_id: None,
        pending_inventory_reservations: Vec::new(),
        committed_inventory_reservations: Vec::new(),
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
                request_projection(order_id, request, RadrootsTradeWorkflowState::Invalid);
            projection.lifecycle_terminal = true;
            projection.last_event_id = last_event_id.or_else(|| Some(request.event_id.clone()));
            projection.issues = issues;
            projection
        }
        None => {
            let mut projection =
                empty_projection(order_id, RadrootsTradeWorkflowState::Invalid, true);
            projection.last_event_id = last_event_id;
            projection.issues = issues;
            projection
        }
    }
}

fn empty_projection(
    order_id: &RadrootsOrderId,
    status: RadrootsTradeWorkflowState,
    lifecycle_terminal: bool,
) -> RadrootsOrderProjection {
    RadrootsOrderProjection {
        order_id: order_id.clone(),
        status,
        request_event_id: None,
        decision_event_id: None,
        cancellation_event_id: None,
        validation_receipt_event_id: None,
        lifecycle_terminal,
        economics: None,
        agreement_event_id: None,
        pending_revision_event_id: None,
        pending_inventory_reservations: Vec::new(),
        committed_inventory_reservations: Vec::new(),
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
    if proposal.prev_event_id == proposal.event_id
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
    if decision.prev_event_id == decision.event_id
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
    if cancellation.prev_event_id == cancellation.event_id {
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
                pending_reserved_count: 0,
                committed_reserved_count: 0,
                remaining_count: bin.available_count,
                over_reserved: false,
                pending_orders: Vec::new(),
                committed_orders: Vec::new(),
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

fn add_pending_inventory_reservations_from_economics(
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
    if let Some(next_count) = bin.pending_reserved_count.checked_add(bin_count) {
        bin.pending_reserved_count = next_count;
        bin.pending_orders
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
        bin.pending_orders.sort_by(|left, right| {
            left.order_id
                .cmp(&right.order_id)
                .then_with(|| left.agreement_event_id.cmp(&right.agreement_event_id))
        });
        bin.committed_orders.sort_by(|left, right| {
            left.order_id
                .cmp(&right.order_id)
                .then_with(|| left.agreement_event_id.cmp(&right.agreement_event_id))
        });
        let reserved_count = bin
            .pending_reserved_count
            .saturating_add(bin.committed_reserved_count);
        bin.remaining_count = bin.available_count.saturating_sub(reserved_count);
        bin.over_reserved = reserved_count > bin.available_count;
        if bin.over_reserved {
            let mut event_ids = bin
                .pending_orders
                .iter()
                .chain(bin.committed_orders.iter())
                .map(|reservation| reservation.agreement_event_id.clone())
                .collect::<Vec<_>>();
            sort_and_dedup_values(&mut event_ids);
            issues.push(RadrootsListingInventoryAccountingIssue::OverReserved {
                bin_id: bin.bin_id.clone(),
                available_count: bin.available_count,
                reserved_count,
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
            | RadrootsOrderIssue::ForkedLifecycle { event_ids: ids }
            | RadrootsOrderIssue::ConflictingValidationReceipts { event_ids: ids } => {
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
            | RadrootsOrderIssue::CancellationPreviousMismatch { event_id }
            | RadrootsOrderIssue::ValidationReceiptWithoutPendingAgreement { event_id }
            | RadrootsOrderIssue::ValidationReceiptOrderIdMismatch { event_id }
            | RadrootsOrderIssue::ValidationReceiptTypeMismatch { event_id }
            | RadrootsOrderIssue::ValidationReceiptRootMismatch { event_id }
            | RadrootsOrderIssue::ValidationReceiptTargetMismatch { event_id }
            | RadrootsOrderIssue::ValidationReceiptListingMismatch { event_id }
            | RadrootsOrderIssue::DeterministicValidationFailure { event_id, .. } => {
                event_ids.push(event_id.clone());
            }
            RadrootsOrderIssue::StaleListingEvent {
                expected_event_id,
                current_event_id,
            } => {
                event_ids.push(expected_event_id.clone());
                event_ids.push(current_event_id.clone());
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

fn inventory_commitments_from_items(
    items: &[RadrootsOrderItem],
) -> Vec<RadrootsOrderInventoryCommitment> {
    let commitments = items
        .iter()
        .map(|item| RadrootsOrderInventoryCommitment {
            bin_id: item.bin_id.clone(),
            bin_count: item.bin_count,
        })
        .collect::<Vec<_>>();
    inventory_reservations_from_commitments(&commitments)
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
        RadrootsOrderIssue::ValidationReceiptWithoutPendingAgreement { .. } => 55,
        RadrootsOrderIssue::ValidationReceiptOrderIdMismatch { .. } => 56,
        RadrootsOrderIssue::ValidationReceiptTypeMismatch { .. } => 57,
        RadrootsOrderIssue::ValidationReceiptRootMismatch { .. } => 58,
        RadrootsOrderIssue::ValidationReceiptTargetMismatch { .. } => 59,
        RadrootsOrderIssue::ValidationReceiptListingMismatch { .. } => 60,
        RadrootsOrderIssue::ConflictingValidationReceipts { .. } => 61,
        RadrootsOrderIssue::DeterministicValidationFailure { .. } => 62,
        RadrootsOrderIssue::StaleListingEvent { .. } => 63,
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::{
        RadrootsListingInventoryAccountingInputs, RadrootsListingInventoryAccountingIssue,
        RadrootsListingInventoryBinAvailability, RadrootsOrderCancellationRecord,
        RadrootsOrderDecisionRecord, RadrootsOrderEventRecord, RadrootsOrderIssue,
        RadrootsOrderReductionInputs, RadrootsOrderRequestRecord,
        RadrootsOrderRevisionDecisionRecord, RadrootsOrderRevisionProposalRecord,
        RadrootsTradeWorkflowState, reduce_listing_inventory_accounting,
        reduce_order_event_records, reduce_order_events,
    };
    use core::mem::discriminant;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
    };
    use radroots_events::{
        RadrootsNostrEvent, RadrootsNostrEventPtr,
        ids::{
            RadrootsEventId, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsOrderId,
            RadrootsOrderQuoteId, RadrootsOrderRevisionId, RadrootsPublicKey,
        },
        kinds::{KIND_LISTING, KIND_LISTING_DRAFT},
        order::{
            RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
            RadrootsOrderEconomicItem, RadrootsOrderEconomics, RadrootsOrderInventoryCommitment,
            RadrootsOrderItem, RadrootsOrderPricingBasis, RadrootsOrderRequest,
            RadrootsOrderRevisionDecision, RadrootsOrderRevisionOutcome,
            RadrootsOrderRevisionProposal,
        },
    };
    #[cfg(feature = "serde_json")]
    use radroots_events_codec::{
        order::{
            order_cancellation_event_build, order_decision_event_build, order_request_event_build,
            order_revision_decision_event_build, order_revision_proposal_event_build,
        },
        wire::WireEventParts,
    };

    const BUYER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

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

    fn draft_listing_addr() -> RadrootsListingAddress {
        RadrootsListingAddress::parse(format!(
            "{KIND_LISTING_DRAFT}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
        ))
        .expect("draft listing address")
    }

    fn other_seller_listing_addr() -> RadrootsListingAddress {
        RadrootsListingAddress::parse(format!("{KIND_LISTING}:{OTHER}:AAAAAAAAAAAAAAAAAAAAAg"))
            .expect("other seller listing address")
    }

    #[cfg(feature = "serde_json")]
    fn listing_event_ptr() -> RadrootsNostrEventPtr {
        RadrootsNostrEventPtr {
            id: event_id(80).into_string(),
            relays: Some("wss://relay.example.test".into()),
        }
    }

    #[cfg(feature = "serde_json")]
    fn event_from_parts(raw_id: u8, author: &str, parts: WireEventParts) -> RadrootsNostrEvent {
        RadrootsNostrEvent {
            id: event_id(raw_id).into_string(),
            author: author.into(),
            created_at: 1,
            kind: parts.kind,
            tags: parts.tags,
            content: parts.content,
            sig: "sig".into(),
        }
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

    fn assert_order_issue_kind(issues: &[RadrootsOrderIssue], expected: RadrootsOrderIssue) {
        let expected_kind = discriminant(&expected);
        assert!(
            issues
                .iter()
                .any(|issue| discriminant(issue) == expected_kind),
            "missing issue kind {expected:?} in {issues:?}"
        );
    }

    fn assert_inventory_issue_kind(
        issues: &[RadrootsListingInventoryAccountingIssue],
        expected: RadrootsListingInventoryAccountingIssue,
    ) {
        let expected_kind = discriminant(&expected);
        assert!(
            issues
                .iter()
                .any(|issue| discriminant(issue) == expected_kind),
            "missing inventory issue kind {expected:?} in {issues:?}"
        );
    }

    fn assert_request_issue(
        mutate: impl FnOnce(&mut RadrootsOrderRequestRecord),
        expected: RadrootsOrderIssue,
    ) {
        let mut request = request_record();
        mutate(&mut request);
        let mut issues = Vec::new();
        assert!(!super::validate_order_request_record(
            &order_id("order-1"),
            &request,
            &mut issues
        ));
        assert_order_issue_kind(&issues, expected);
    }

    fn assert_decision_issue(
        mutate: impl FnOnce(&mut RadrootsOrderDecisionRecord),
        expected: RadrootsOrderIssue,
    ) {
        let request = request_record();
        let mut decision = accepted_decision();
        mutate(&mut decision);
        let mut issues = Vec::new();
        assert!(!super::validate_order_decision_record(
            &request,
            &decision,
            &mut issues
        ));
        assert_order_issue_kind(&issues, expected);
    }

    fn assert_revision_proposal_issue(
        mutate: impl FnOnce(&mut RadrootsOrderRevisionProposalRecord),
        expected: RadrootsOrderIssue,
    ) {
        let request = request_record();
        let mut proposal = revision_proposal();
        mutate(&mut proposal);
        let mut issues = Vec::new();
        assert!(!super::validate_order_revision_proposal_record(
            &request,
            &proposal,
            &mut issues
        ));
        assert_order_issue_kind(&issues, expected);
    }

    fn assert_revision_decision_issue(
        mutate: impl FnOnce(&mut RadrootsOrderRevisionDecisionRecord),
        expected: RadrootsOrderIssue,
    ) {
        let request = request_record();
        let mut decision = accepted_revision_decision();
        mutate(&mut decision);
        let mut issues = Vec::new();
        assert!(!super::validate_order_revision_decision_record(
            &request,
            &decision,
            &mut issues
        ));
        assert_order_issue_kind(&issues, expected);
    }

    fn assert_cancellation_issue(
        mutate: impl FnOnce(&mut RadrootsOrderCancellationRecord),
        expected: RadrootsOrderIssue,
    ) {
        let request = request_record();
        let mut cancellation = cancellation(event_id(1));
        mutate(&mut cancellation);
        let mut issues = Vec::new();
        assert!(!super::validate_order_cancellation_record(
            &request,
            &cancellation,
            &mut issues
        ));
        assert_order_issue_kind(&issues, expected);
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
    fn order_event_record_accessors_cover_all_variants() {
        let records = vec![
            RadrootsOrderEventRecord::Request(request_record()),
            RadrootsOrderEventRecord::Decision(accepted_decision()),
            RadrootsOrderEventRecord::RevisionProposal(revision_proposal()),
            RadrootsOrderEventRecord::RevisionDecision(accepted_revision_decision()),
            RadrootsOrderEventRecord::Cancellation(cancellation(event_id(1))),
        ];

        let event_ids = records
            .iter()
            .map(RadrootsOrderEventRecord::event_id)
            .cloned()
            .collect::<Vec<_>>();
        let order_ids = records
            .iter()
            .map(RadrootsOrderEventRecord::order_id)
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(
            event_ids,
            vec![
                event_id(1),
                event_id(2),
                event_id(3),
                event_id(4),
                event_id(5)
            ]
        );
        assert_eq!(order_ids, vec![order_id("order-1"); 5]);
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn order_event_records_decode_wire_events_and_decode_errors() {
        let request = request_record();
        let request_parts =
            order_request_event_build(&listing_event_ptr(), &request.payload).unwrap();
        let request_record =
            super::order_event_record_from_event(&event_from_parts(11, BUYER, request_parts))
                .unwrap();
        assert!(matches!(
            request_record,
            RadrootsOrderEventRecord::Request(record)
                if record.event_id == event_id(11)
                    && record.author_pubkey == public_key(BUYER)
                    && record.payload.order_id == order_id("order-1")
        ));

        let decision = accepted_decision();
        let decision_parts = order_decision_event_build(
            &decision.root_event_id,
            &decision.prev_event_id,
            &decision.payload,
        )
        .unwrap();
        let decision_record =
            super::order_event_record_from_event(&event_from_parts(12, SELLER, decision_parts))
                .unwrap();
        assert!(matches!(
            decision_record,
            RadrootsOrderEventRecord::Decision(record)
                if record.event_id == event_id(12)
                    && record.counterparty_pubkey == public_key(BUYER)
                    && record.root_event_id == event_id(1)
                    && record.prev_event_id == event_id(1)
        ));

        let proposal = revision_proposal();
        let proposal_parts = order_revision_proposal_event_build(
            &proposal.root_event_id,
            &proposal.prev_event_id,
            &proposal.payload,
        )
        .unwrap();
        let proposal_record =
            super::order_event_record_from_event(&event_from_parts(13, SELLER, proposal_parts))
                .unwrap();
        assert!(matches!(
            proposal_record,
            RadrootsOrderEventRecord::RevisionProposal(record)
                if record.event_id == event_id(13)
                    && record.counterparty_pubkey == public_key(BUYER)
                    && record.payload.revision_id == revision_id("revision-1")
        ));

        let revision_decision = accepted_revision_decision();
        let revision_decision_parts = order_revision_decision_event_build(
            &revision_decision.root_event_id,
            &revision_decision.prev_event_id,
            &revision_decision.payload,
        )
        .unwrap();
        let revision_decision_record = super::order_event_record_from_event(&event_from_parts(
            14,
            BUYER,
            revision_decision_parts,
        ))
        .unwrap();
        assert!(matches!(
            revision_decision_record,
            RadrootsOrderEventRecord::RevisionDecision(record)
                if record.event_id == event_id(14)
                    && record.counterparty_pubkey == public_key(SELLER)
                    && record.payload.revision_id == revision_id("revision-1")
        ));

        let cancellation = cancellation(event_id(1));
        let cancellation_parts = order_cancellation_event_build(
            &cancellation.root_event_id,
            &cancellation.prev_event_id,
            &cancellation.payload,
        )
        .unwrap();
        let cancellation_record =
            super::order_event_record_from_event(&event_from_parts(15, BUYER, cancellation_parts))
                .unwrap();
        assert!(matches!(
            cancellation_record,
            RadrootsOrderEventRecord::Cancellation(record)
                if record.event_id == event_id(15)
                    && record.counterparty_pubkey == public_key(SELLER)
                    && record.payload.reason == "changed plans"
        ));

        let unsupported = RadrootsNostrEvent {
            id: event_id(16).into_string(),
            author: BUYER.into(),
            created_at: 1,
            kind: 1,
            tags: Vec::new(),
            content: "{}".into(),
            sig: "sig".into(),
        };
        assert!(matches!(
            super::order_event_record_from_event(&unsupported),
            Err(super::RadrootsOrderEventDecodeError::UnsupportedKind { kind: 1 })
        ));

        let request_parts =
            order_request_event_build(&listing_event_ptr(), &request.payload).unwrap();
        let mut invalid_id_event = event_from_parts(17, BUYER, request_parts);
        invalid_id_event.id = "not-an-event-id".into();
        assert!(matches!(
            super::order_event_record_from_event(&invalid_id_event),
            Err(super::RadrootsOrderEventDecodeError::InvalidEventId(_))
        ));

        let request_parts =
            order_request_event_build(&listing_event_ptr(), &request.payload).unwrap();
        let mut invalid_author_event = event_from_parts(18, BUYER, request_parts);
        invalid_author_event.author = "not-a-pubkey".into();
        assert!(matches!(
            super::order_event_record_from_event(&invalid_author_event),
            Err(super::RadrootsOrderEventDecodeError::InvalidAuthor(_))
        ));
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn order_event_context_requirements_report_missing_chain_ids() {
        let context = radroots_events_codec::order::RadrootsOrderEventContext {
            counterparty_pubkey: public_key(BUYER),
            listing_event: None,
            root_event_id: None,
            prev_event_id: None,
        };

        assert!(matches!(
            super::require_context_root_event_id(&context),
            Err(super::RadrootsOrderEventDecodeError::MissingRootEventId)
        ));
        assert!(matches!(
            super::require_context_prev_event_id(&context),
            Err(super::RadrootsOrderEventDecodeError::MissingPreviousEventId)
        ));
    }

    #[test]
    fn reducer_groups_all_record_variants_and_skips_duplicate_event_ids() {
        let mut duplicate_decision = declined_decision();
        duplicate_decision.event_id = event_id(2);
        let projection = reduce_order_event_records(
            &order_id("order-1"),
            vec![
                RadrootsOrderEventRecord::Cancellation(cancellation(event_id(1))),
                RadrootsOrderEventRecord::RevisionDecision(accepted_revision_decision()),
                RadrootsOrderEventRecord::RevisionProposal(revision_proposal()),
                RadrootsOrderEventRecord::Decision(accepted_decision()),
                RadrootsOrderEventRecord::Decision(duplicate_decision),
                RadrootsOrderEventRecord::Request(request_record()),
            ],
        );

        assert_eq!(projection.status, RadrootsTradeWorkflowState::Invalid);
        assert_order_issue_kind(
            &projection.issues,
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: Vec::new(),
            },
        );
        assert_eq!(
            super::projection_issue_event_ids(&projection.issues),
            vec![event_id(2), event_id(4), event_id(5)]
        );

        let mut duplicate_request = request_record();
        duplicate_request.payload.order_id = order_id("order-duplicate");
        let mut duplicate_request_later = duplicate_request.clone();
        duplicate_request_later.payload.buyer_pubkey = public_key(OTHER);
        let deduped =
            super::unique_request_records(vec![duplicate_request.clone(), duplicate_request_later]);
        assert_eq!(deduped.len(), 1);
        assert_eq!(
            deduped[0].payload.order_id,
            duplicate_request.payload.order_id
        );
        assert_eq!(
            deduped[0].payload.buyer_pubkey,
            duplicate_request.payload.buyer_pubkey
        );
    }

    #[test]
    fn canonicalize_order_request_reports_signer_listing_and_item_errors() {
        let canonical =
            super::canonicalize_order_request_for_signer(request_record().payload, BUYER).unwrap();
        assert_eq!(canonical.buyer_pubkey, public_key(BUYER));
        assert_eq!(canonical.seller_pubkey, public_key(SELLER));

        let mut unsorted_items = request_record().payload;
        unsorted_items.items.push(RadrootsOrderItem {
            bin_id: bin_id("bin-0"),
            bin_count: 1,
        });
        let canonical =
            super::canonicalize_order_request_for_signer(unsorted_items, BUYER).unwrap();
        assert_eq!(canonical.items[0].bin_id, bin_id("bin-0"));
        assert_eq!(canonical.items[1].bin_id, bin_id("bin-1"));

        assert!(matches!(
            super::parse_public_listing_addr("not-an-address"),
            Err(super::RadrootsOrderCanonicalizationError::InvalidListingAddress(_))
        ));
        assert!(matches!(
            super::parse_public_listing_addr(format!(
                "{KIND_LISTING_DRAFT}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
            )),
            Err(super::RadrootsOrderCanonicalizationError::InvalidListingKind)
        ));
        assert!(matches!(
            super::parse_public_listing_addr(format!("30023:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")),
            Err(super::RadrootsOrderCanonicalizationError::InvalidListingKind)
        ));

        assert!(matches!(
            super::canonicalize_order_request_for_signer(request_record().payload, SELLER),
            Err(super::RadrootsOrderCanonicalizationError::InvalidBuyerSigner)
        ));

        let mut seller_mismatch = request_record().payload;
        seller_mismatch.seller_pubkey = public_key(OTHER);
        assert!(matches!(
            super::canonicalize_order_request_for_signer(seller_mismatch, BUYER),
            Err(super::RadrootsOrderCanonicalizationError::InvalidSellerListing)
        ));

        let mut invalid_kind = request_record().payload;
        invalid_kind.listing_addr = draft_listing_addr();
        assert!(matches!(
            super::canonicalize_order_request_for_signer(invalid_kind, BUYER),
            Err(super::RadrootsOrderCanonicalizationError::InvalidListingKind)
        ));

        let mut missing_items = request_record().payload;
        missing_items.items.clear();
        assert!(matches!(
            super::canonicalize_order_request_for_signer(missing_items, BUYER),
            Err(super::RadrootsOrderCanonicalizationError::MissingItems)
        ));

        let mut zero_count = request_record().payload;
        zero_count.items[0].bin_count = 0;
        assert!(matches!(
            super::canonicalize_order_request_for_signer(zero_count, BUYER),
            Err(super::RadrootsOrderCanonicalizationError::InvalidBinCount { index: 0 })
        ));
    }

    #[test]
    fn canonicalize_order_decision_reports_signer_and_decision_errors() {
        let canonical =
            super::canonicalize_order_decision_for_signer(accepted_decision().payload, SELLER)
                .unwrap();
        assert_eq!(canonical.seller_pubkey, public_key(SELLER));

        let mut unsorted_commitments = accepted_decision().payload;
        if let RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } = &mut unsorted_commitments.decision
        {
            inventory_commitments.push(RadrootsOrderInventoryCommitment {
                bin_id: bin_id("bin-0"),
                bin_count: 1,
            });
        }
        let canonical =
            super::canonicalize_order_decision_for_signer(unsorted_commitments, SELLER).unwrap();
        if let RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } = canonical.decision
        {
            assert_eq!(inventory_commitments[0].bin_id, bin_id("bin-0"));
            assert_eq!(inventory_commitments[1].bin_id, bin_id("bin-1"));
        }

        let mut listing_seller_mismatch = accepted_decision().payload;
        listing_seller_mismatch.listing_addr = other_seller_listing_addr();
        assert!(matches!(
            super::canonicalize_order_decision_for_signer(listing_seller_mismatch, SELLER),
            Err(super::RadrootsOrderCanonicalizationError::InvalidSellerListing)
        ));

        assert!(matches!(
            super::canonicalize_order_decision_for_signer(accepted_decision().payload, BUYER),
            Err(super::RadrootsOrderCanonicalizationError::InvalidSellerListing)
        ));

        let mut missing_commitments = accepted_decision().payload;
        missing_commitments.decision = RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments: Vec::new(),
        };
        assert!(matches!(
            super::canonicalize_order_decision_for_signer(missing_commitments, SELLER),
            Err(super::RadrootsOrderCanonicalizationError::MissingInventoryCommitments)
        ));

        let mut zero_commitment = accepted_decision().payload;
        if let RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } = &mut zero_commitment.decision
        {
            inventory_commitments[0].bin_count = 0;
        }
        assert!(matches!(
            super::canonicalize_order_decision_for_signer(zero_commitment, SELLER),
            Err(
                super::RadrootsOrderCanonicalizationError::InvalidInventoryCommitmentCount {
                    index: 0
                }
            )
        ));

        let mut declined = declined_decision().payload;
        declined.decision = RadrootsOrderDecisionOutcome::Declined {
            reason: "  already sold  ".into(),
        };
        let declined = super::canonicalize_order_decision_for_signer(declined, SELLER).unwrap();
        assert_eq!(
            declined.decision,
            RadrootsOrderDecisionOutcome::Declined {
                reason: "already sold".into()
            }
        );

        let mut blank_reason = declined_decision().payload;
        blank_reason.decision = RadrootsOrderDecisionOutcome::Declined { reason: " ".into() };
        assert!(matches!(
            super::canonicalize_order_decision_for_signer(blank_reason, SELLER),
            Err(super::RadrootsOrderCanonicalizationError::EmptyField(
                "reason"
            ))
        ));
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn order_economics_digest_is_stable_sha256_hex() {
        let digest = super::radroots_order_economics_digest(&economics(2)).unwrap();
        assert_eq!(
            digest,
            super::radroots_order_economics_digest(&economics(2)).unwrap()
        );
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), "sha256:".len() + 64);
    }

    #[test]
    fn order_helper_sorting_and_matching_paths_are_deterministic() {
        let request_items = vec![
            RadrootsOrderItem {
                bin_id: bin_id("bin-2"),
                bin_count: 1,
            },
            RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            },
        ];
        let matching_commitments = vec![
            RadrootsOrderInventoryCommitment {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            },
            RadrootsOrderInventoryCommitment {
                bin_id: bin_id("bin-2"),
                bin_count: 1,
            },
        ];
        assert!(super::inventory_commitments_match_request(
            &request_items,
            &matching_commitments
        ));
        assert!(!super::inventory_commitments_match_request(
            &request_items,
            &matching_commitments[..1]
        ));
        let mut count_mismatch = matching_commitments.clone();
        count_mismatch[0].bin_count = 1;
        assert!(!super::inventory_commitments_match_request(
            &request_items,
            &count_mismatch
        ));
        let mut bin_mismatch = matching_commitments.clone();
        bin_mismatch[0].bin_id = bin_id("bin-3");
        assert!(!super::inventory_commitments_match_request(
            &request_items,
            &bin_mismatch
        ));

        let mut order_issues = vec![
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: vec![event_id(9), event_id(3)],
            },
            RadrootsOrderIssue::CancellationWithoutCancellableOrder {
                event_id: event_id(5),
            },
            RadrootsOrderIssue::DecisionPayloadInvalid {
                event_id: event_id(2),
            },
            RadrootsOrderIssue::MissingRequest,
        ];
        assert_eq!(
            super::projection_issue_event_ids(&order_issues),
            vec![event_id(2), event_id(3), event_id(5), event_id(9)]
        );
        order_issues.sort_by(super::order_issue_sort_key);
        assert!(matches!(
            order_issues[0],
            RadrootsOrderIssue::MissingRequest
        ));
        assert!(matches!(
            order_issues[1],
            RadrootsOrderIssue::DecisionPayloadInvalid { .. }
        ));
        assert!(matches!(
            order_issues[2],
            RadrootsOrderIssue::CancellationWithoutCancellableOrder { .. }
        ));
        assert!(matches!(
            order_issues[3],
            RadrootsOrderIssue::ForkedLifecycle { .. }
        ));

        let mut tied_order_issues = vec![
            RadrootsOrderIssue::DecisionPayloadInvalid {
                event_id: event_id(8),
            },
            RadrootsOrderIssue::DecisionPayloadInvalid {
                event_id: event_id(7),
            },
        ];
        tied_order_issues.sort_by(super::order_issue_sort_key);
        let RadrootsOrderIssue::DecisionPayloadInvalid {
            event_id: issue_event_id,
        } = &tied_order_issues[0]
        else {
            panic!("expected decision issue");
        };
        assert_eq!(issue_event_id, &event_id(7));

        let mut inventory_issues = vec![
            RadrootsListingInventoryAccountingIssue::OverReserved {
                bin_id: bin_id("bin-2"),
                available_count: 1,
                reserved_count: 2,
                event_ids: vec![event_id(8)],
            },
            RadrootsListingInventoryAccountingIssue::UnknownInventoryBin {
                bin_id: bin_id("bin-1"),
                event_ids: vec![event_id(7)],
            },
            RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                bin_id: bin_id("bin-3"),
                event_ids: vec![event_id(6)],
            },
            RadrootsListingInventoryAccountingIssue::InvalidOrder {
                order_id: order_id("order-1"),
                event_ids: vec![event_id(5)],
            },
        ];
        inventory_issues.sort_by(super::inventory_issue_sort_key);
        assert!(matches!(
            inventory_issues[0],
            RadrootsListingInventoryAccountingIssue::InvalidOrder { .. }
        ));
        assert!(matches!(
            inventory_issues[1],
            RadrootsListingInventoryAccountingIssue::ArithmeticOverflow { .. }
        ));
        assert!(matches!(
            inventory_issues[2],
            RadrootsListingInventoryAccountingIssue::UnknownInventoryBin { .. }
        ));
        assert!(matches!(
            inventory_issues[3],
            RadrootsListingInventoryAccountingIssue::OverReserved { .. }
        ));

        let mut tied_inventory_issues = vec![
            RadrootsListingInventoryAccountingIssue::UnknownInventoryBin {
                bin_id: bin_id("bin-2"),
                event_ids: vec![event_id(9)],
            },
            RadrootsListingInventoryAccountingIssue::UnknownInventoryBin {
                bin_id: bin_id("bin-1"),
                event_ids: vec![event_id(8)],
            },
            RadrootsListingInventoryAccountingIssue::UnknownInventoryBin {
                bin_id: bin_id("bin-1"),
                event_ids: vec![event_id(7)],
            },
        ];
        tied_inventory_issues.sort_by(super::inventory_issue_sort_key);
        assert_eq!(
            super::inventory_issue_id(&tied_inventory_issues[0]),
            "bin-1"
        );
        assert_eq!(
            super::inventory_issue_event_ids(&tied_inventory_issues[0]),
            &[event_id(7)]
        );

        let invalid = super::invalid_projection(
            &order_id("order-1"),
            Some(&request_record()),
            vec![RadrootsOrderIssue::MissingRequest],
        );
        assert_eq!(invalid.last_event_id, Some(event_id(1)));
    }

    #[test]
    fn order_issue_rank_and_event_id_helpers_cover_every_issue_variant() {
        let id = event_id(42);
        let event_ids = vec![id.clone()];
        let issues = vec![
            RadrootsOrderIssue::MissingRequest,
            RadrootsOrderIssue::MultipleRequests {
                event_ids: event_ids.clone(),
            },
            RadrootsOrderIssue::RequestPayloadInvalid {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RequestOrderIdMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RequestAuthorMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RequestListingAddressInvalid {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RequestSellerListingMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionPayloadInvalid {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionOrderIdMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionAuthorMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionCounterpartyMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionBuyerMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionSellerMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionListingAddressInvalid {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionListingMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionRootMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionPreviousMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionMissingInventoryCommitments {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionInventoryCommitmentMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::DecisionMissingReason {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::ConflictingDecisions {
                event_ids: event_ids.clone(),
            },
            RadrootsOrderIssue::RevisionProposalPayloadInvalid {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionProposalOrderIdMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionProposalAuthorMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionProposalCounterpartyMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionProposalBuyerMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionProposalSellerMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionProposalListingAddressInvalid {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionProposalListingMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionProposalRootMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionProposalPreviousMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionWithoutProposal {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionPayloadInvalid {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionOrderIdMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionAuthorMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionCounterpartyMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionBuyerMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionSellerMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionListingAddressInvalid {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionListingMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionRootMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionPreviousMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::RevisionDecisionRevisionIdMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationWithoutCancellableOrder {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationPayloadInvalid {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationOrderIdMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationAuthorMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationCounterpartyMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationBuyerMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationSellerMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationListingAddressInvalid {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationListingMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationRootMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::CancellationPreviousMismatch {
                event_id: id.clone(),
            },
            RadrootsOrderIssue::ForkedLifecycle { event_ids },
        ];

        for (rank, issue) in issues.iter().enumerate() {
            assert_eq!(super::order_issue_rank(issue), rank as u8);
        }
        assert_eq!(super::projection_issue_event_ids(&issues), vec![id]);
    }

    #[test]
    fn inventory_issue_helpers_cover_all_issue_variants() {
        let id = event_id(9);
        let issues = vec![
            RadrootsListingInventoryAccountingIssue::InvalidOrder {
                order_id: order_id("order-1"),
                event_ids: vec![id.clone()],
            },
            RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                bin_id: bin_id("bin-1"),
                event_ids: vec![id.clone()],
            },
            RadrootsListingInventoryAccountingIssue::UnknownInventoryBin {
                bin_id: bin_id("bin-2"),
                event_ids: vec![id.clone()],
            },
            RadrootsListingInventoryAccountingIssue::OverReserved {
                bin_id: bin_id("bin-3"),
                available_count: 1,
                reserved_count: 2,
                event_ids: vec![id.clone()],
            },
        ];

        assert_eq!(super::inventory_issue_rank(&issues[0]), 0);
        assert_eq!(super::inventory_issue_rank(&issues[1]), 1);
        assert_eq!(super::inventory_issue_rank(&issues[2]), 2);
        assert_eq!(super::inventory_issue_rank(&issues[3]), 3);
        assert_eq!(super::inventory_issue_id(&issues[0]), "order-1");
        assert_eq!(super::inventory_issue_id(&issues[1]), "bin-1");
        assert_eq!(super::inventory_issue_id(&issues[2]), "bin-2");
        assert_eq!(super::inventory_issue_id(&issues[3]), "bin-3");
        for issue in &issues {
            assert_eq!(super::inventory_issue_event_ids(issue), &[id.clone()]);
        }
    }

    #[test]
    fn reducer_reports_missing_request_for_each_non_request_input_family() {
        let decision_only = reduce_order_events(
            &order_id("order-1"),
            RadrootsOrderReductionInputs {
                requests: Vec::<RadrootsOrderRequestRecord>::new(),
                decisions: vec![accepted_decision()],
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );
        assert_order_issue_kind(&decision_only.issues, RadrootsOrderIssue::MissingRequest);

        let proposal_only = reduce_order_events(
            &order_id("order-1"),
            RadrootsOrderReductionInputs {
                requests: Vec::<RadrootsOrderRequestRecord>::new(),
                decisions: Vec::<RadrootsOrderDecisionRecord>::new(),
                revision_proposals: vec![revision_proposal()],
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );
        assert_order_issue_kind(&proposal_only.issues, RadrootsOrderIssue::MissingRequest);

        let revision_decision_only = reduce_order_events(
            &order_id("order-1"),
            RadrootsOrderReductionInputs {
                requests: Vec::<RadrootsOrderRequestRecord>::new(),
                decisions: Vec::<RadrootsOrderDecisionRecord>::new(),
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: vec![accepted_revision_decision()],
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );
        assert_order_issue_kind(
            &revision_decision_only.issues,
            RadrootsOrderIssue::MissingRequest,
        );

        let cancellation_only = reduce_order_events(
            &order_id("order-1"),
            RadrootsOrderReductionInputs {
                requests: Vec::<RadrootsOrderRequestRecord>::new(),
                decisions: Vec::<RadrootsOrderDecisionRecord>::new(),
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: vec![cancellation(event_id(1))],
            },
        );
        assert_order_issue_kind(
            &cancellation_only.issues,
            RadrootsOrderIssue::MissingRequest,
        );
    }

    #[test]
    fn reducer_reports_multiple_valid_cancellations_as_forked_lifecycle() {
        let mut second_cancellation = cancellation(event_id(1));
        second_cancellation.event_id = event_id(6);
        let projection = reduce(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![cancellation(event_id(1)), second_cancellation],
        );

        assert_order_issue_kind(
            &projection.issues,
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: Vec::new(),
            },
        );
        assert_eq!(projection.last_event_id, Some(event_id(6)));
    }

    #[test]
    fn inventory_accounting_private_helpers_cover_merge_sort_and_overflow_paths() {
        let (bins, issues) = super::normalized_listing_inventory_bins(vec![
            RadrootsListingInventoryBinAvailability {
                bin_id: bin_id("bin-1"),
                available_count: 1,
            },
            RadrootsListingInventoryBinAvailability {
                bin_id: bin_id("bin-1"),
                available_count: 2,
            },
        ]);
        assert_eq!(issues, Vec::new());
        assert_eq!(bins[0].available_count, 3);
        assert_eq!(bins[0].remaining_count, 3);

        let mut overflow_bin = super::RadrootsListingInventoryBinAccounting {
            bin_id: bin_id("bin-overflow"),
            available_count: u64::MAX,
            pending_reserved_count: u64::MAX,
            committed_reserved_count: 0,
            remaining_count: u64::MAX,
            over_reserved: false,
            pending_orders: Vec::new(),
            committed_orders: Vec::new(),
        };
        let mut overflow_issues = Vec::new();
        super::add_inventory_reservation_event(
            &mut overflow_bin,
            &order_id("order-overflow"),
            &event_id(90),
            1,
            &mut overflow_issues,
        );
        assert_inventory_issue_kind(
            &overflow_issues,
            RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                bin_id: bin_id("bin-overflow"),
                event_ids: Vec::new(),
            },
        );

        let mut sorting_bin = super::RadrootsListingInventoryBinAccounting {
            bin_id: bin_id("bin-sort"),
            available_count: 1,
            pending_reserved_count: 2,
            committed_reserved_count: 0,
            remaining_count: 1,
            over_reserved: false,
            pending_orders: vec![
                super::RadrootsListingInventoryOrderReservation {
                    order_id: order_id("order-2"),
                    agreement_event_id: event_id(92),
                    bin_count: 1,
                },
                super::RadrootsListingInventoryOrderReservation {
                    order_id: order_id("order-1"),
                    agreement_event_id: event_id(91),
                    bin_count: 1,
                },
                super::RadrootsListingInventoryOrderReservation {
                    order_id: order_id("order-1"),
                    agreement_event_id: event_id(90),
                    bin_count: 1,
                },
            ],
            committed_orders: Vec::new(),
        };
        let mut finish_issues = Vec::new();
        super::finish_inventory_accounting_bins(
            core::slice::from_mut(&mut sorting_bin),
            &mut finish_issues,
        );
        assert_eq!(sorting_bin.remaining_count, 0);
        assert!(sorting_bin.over_reserved);
        assert_eq!(sorting_bin.pending_orders[0].order_id, order_id("order-1"));
        assert_eq!(
            sorting_bin.pending_orders[0].agreement_event_id,
            event_id(90)
        );
        assert_inventory_issue_kind(
            &finish_issues,
            RadrootsListingInventoryAccountingIssue::OverReserved {
                bin_id: bin_id("bin-sort"),
                available_count: 1,
                reserved_count: 2,
                event_ids: Vec::new(),
            },
        );

        let mut fallback_request = request_record();
        fallback_request.event_id = event_id(95);
        let mut fallback_decision = accepted_decision();
        fallback_decision.event_id = event_id(93);
        let mut fallback_proposal = revision_proposal();
        fallback_proposal.event_id = event_id(94);
        let mut fallback_revision_decision = accepted_revision_decision();
        fallback_revision_decision.event_id = event_id(92);
        let mut fallback_cancellation = cancellation(event_id(3));
        fallback_cancellation.event_id = event_id(91);
        let fallback_ids = super::fallback_order_event_ids(
            &[fallback_request],
            &[fallback_decision],
            &[fallback_proposal],
            &[fallback_revision_decision],
            &[fallback_cancellation],
        );
        assert_eq!(
            fallback_ids,
            vec![
                event_id(91),
                event_id(92),
                event_id(93),
                event_id(94),
                event_id(95)
            ]
        );
    }

    #[test]
    fn reducer_reports_missing_duplicate_and_forked_lifecycles() {
        let missing = reduce_order_events(
            &order_id("order-1"),
            RadrootsOrderReductionInputs {
                requests: Vec::<RadrootsOrderRequestRecord>::new(),
                decisions: Vec::<RadrootsOrderDecisionRecord>::new(),
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );
        assert_eq!(missing.status, RadrootsTradeWorkflowState::Missing);

        let missing_request = reduce_order_events(
            &order_id("order-1"),
            RadrootsOrderReductionInputs {
                requests: Vec::<RadrootsOrderRequestRecord>::new(),
                decisions: vec![accepted_decision()],
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );
        assert_order_issue_kind(&missing_request.issues, RadrootsOrderIssue::MissingRequest);

        let mut duplicate_request = request_record();
        duplicate_request.event_id = event_id(6);
        let duplicate = reduce_order_events(
            &order_id("order-1"),
            RadrootsOrderReductionInputs {
                requests: vec![request_record(), duplicate_request],
                decisions: Vec::<RadrootsOrderDecisionRecord>::new(),
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );
        assert_order_issue_kind(
            &duplicate.issues,
            RadrootsOrderIssue::MultipleRequests {
                event_ids: Vec::new(),
            },
        );

        let mut second_decision = declined_decision();
        second_decision.event_id = event_id(6);
        let conflicting = reduce(
            vec![accepted_decision(), second_decision],
            vec![],
            vec![],
            vec![],
        );
        assert_order_issue_kind(
            &conflicting.issues,
            RadrootsOrderIssue::ConflictingDecisions {
                event_ids: Vec::new(),
            },
        );

        let without_proposal = reduce(
            Vec::new(),
            Vec::new(),
            vec![accepted_revision_decision()],
            Vec::new(),
        );
        assert_order_issue_kind(
            &without_proposal.issues,
            RadrootsOrderIssue::RevisionDecisionWithoutProposal {
                event_id: event_id(4),
            },
        );

        let mut second_proposal = revision_proposal();
        second_proposal.event_id = event_id(6);
        let multiple_proposals = reduce(
            Vec::new(),
            vec![revision_proposal(), second_proposal],
            vec![],
            vec![],
        );
        assert_order_issue_kind(
            &multiple_proposals.issues,
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: Vec::new(),
            },
        );

        let mut second_revision_decision = accepted_revision_decision();
        second_revision_decision.event_id = event_id(7);
        let multiple_revision_decisions = reduce(
            Vec::new(),
            vec![revision_proposal()],
            vec![accepted_revision_decision(), second_revision_decision],
            Vec::new(),
        );
        assert_order_issue_kind(
            &multiple_revision_decisions.issues,
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: Vec::new(),
            },
        );

        let decided_then_revised = reduce(
            vec![accepted_decision()],
            vec![revision_proposal()],
            Vec::new(),
            Vec::new(),
        );
        assert_order_issue_kind(
            &decided_then_revised.issues,
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: Vec::new(),
            },
        );

        let decided_then_revision_decision = reduce(
            vec![accepted_decision()],
            Vec::new(),
            vec![accepted_revision_decision()],
            Vec::new(),
        );
        assert_order_issue_kind(
            &decided_then_revision_decision.issues,
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: Vec::new(),
            },
        );
    }

    #[test]
    fn reducer_covers_revision_and_cancellation_edge_paths() {
        let pending_revision = reduce(
            Vec::new(),
            vec![revision_proposal()],
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            pending_revision.status,
            RadrootsTradeWorkflowState::RevisionProposed
        );
        assert_eq!(
            pending_revision.pending_revision_event_id,
            Some(event_id(3))
        );
        assert_eq!(
            pending_revision.economics.expect("pending economics").items[0].bin_count,
            1
        );

        let mut bad_proposal_previous = revision_proposal();
        bad_proposal_previous.prev_event_id = event_id(8);
        bad_proposal_previous.payload.prev_event_id = event_id(8);
        let bad_proposal = reduce(
            Vec::new(),
            vec![bad_proposal_previous],
            Vec::new(),
            Vec::new(),
        );
        assert_order_issue_kind(
            &bad_proposal.issues,
            RadrootsOrderIssue::RevisionProposalPreviousMismatch {
                event_id: event_id(3),
            },
        );

        let mut bad_decision_previous = accepted_revision_decision();
        bad_decision_previous.prev_event_id = event_id(8);
        bad_decision_previous.payload.prev_event_id = event_id(8);
        let bad_decision = reduce(
            Vec::new(),
            vec![revision_proposal()],
            vec![bad_decision_previous],
            Vec::new(),
        );
        assert_order_issue_kind(
            &bad_decision.issues,
            RadrootsOrderIssue::RevisionDecisionPreviousMismatch {
                event_id: event_id(4),
            },
        );

        let mut bad_revision_id = accepted_revision_decision();
        bad_revision_id.payload.revision_id = revision_id("revision-2");
        let bad_revision = reduce(
            Vec::new(),
            vec![revision_proposal()],
            vec![bad_revision_id],
            Vec::new(),
        );
        assert_order_issue_kind(
            &bad_revision.issues,
            RadrootsOrderIssue::RevisionDecisionRevisionIdMismatch {
                event_id: event_id(4),
            },
        );

        let mut declined_revision = accepted_revision_decision();
        declined_revision.payload.decision = RadrootsOrderRevisionOutcome::Declined {
            reason: "too late".into(),
        };
        let declined = reduce(
            Vec::new(),
            vec![revision_proposal()],
            vec![declined_revision],
            Vec::new(),
        );
        assert_eq!(declined.status, RadrootsTradeWorkflowState::Declined);
        assert_eq!(declined.pending_revision_event_id, Some(event_id(3)));

        let cancellation_after_decision = reduce(
            vec![declined_decision()],
            Vec::new(),
            Vec::new(),
            vec![cancellation(event_id(2))],
        );
        assert_order_issue_kind(
            &cancellation_after_decision.issues,
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: Vec::new(),
            },
        );

        let cancellation_after_revision_decision = reduce(
            Vec::new(),
            vec![revision_proposal()],
            vec![accepted_revision_decision()],
            vec![cancellation(event_id(4))],
        );
        assert_order_issue_kind(
            &cancellation_after_revision_decision.issues,
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: Vec::new(),
            },
        );

        let mut second_proposal = revision_proposal();
        second_proposal.event_id = event_id(6);
        let cancellation_after_multiple_proposals = reduce(
            Vec::new(),
            vec![revision_proposal(), second_proposal],
            Vec::new(),
            vec![cancellation(event_id(3))],
        );
        assert_order_issue_kind(
            &cancellation_after_multiple_proposals.issues,
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: Vec::new(),
            },
        );

        let cancellation_previous_mismatch = reduce(
            Vec::new(),
            vec![revision_proposal()],
            Vec::new(),
            vec![cancellation(event_id(1))],
        );
        assert_order_issue_kind(
            &cancellation_previous_mismatch.issues,
            RadrootsOrderIssue::CancellationPreviousMismatch {
                event_id: event_id(5),
            },
        );
    }

    #[test]
    fn reducer_validators_report_request_and_decision_issue_kinds() {
        assert_request_issue(
            |request| request.payload.items.clear(),
            RadrootsOrderIssue::RequestPayloadInvalid {
                event_id: event_id(1),
            },
        );
        assert_request_issue(
            |request| request.payload.order_id = order_id("order-2"),
            RadrootsOrderIssue::RequestOrderIdMismatch {
                event_id: event_id(1),
            },
        );
        assert_request_issue(
            |request| request.author_pubkey = public_key(SELLER),
            RadrootsOrderIssue::RequestAuthorMismatch {
                event_id: event_id(1),
            },
        );
        assert_request_issue(
            |request| request.payload.listing_addr = draft_listing_addr(),
            RadrootsOrderIssue::RequestListingAddressInvalid {
                event_id: event_id(1),
            },
        );
        assert_request_issue(
            |request| request.payload.seller_pubkey = public_key(OTHER),
            RadrootsOrderIssue::RequestSellerListingMismatch {
                event_id: event_id(1),
            },
        );

        assert_decision_issue(
            |decision| {
                decision.payload.decision = RadrootsOrderDecisionOutcome::Accepted {
                    inventory_commitments: Vec::new(),
                };
            },
            RadrootsOrderIssue::DecisionMissingInventoryCommitments {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| {
                decision.payload.decision =
                    RadrootsOrderDecisionOutcome::Declined { reason: " ".into() };
            },
            RadrootsOrderIssue::DecisionMissingReason {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| decision.payload.order_id = order_id("order-2"),
            RadrootsOrderIssue::DecisionOrderIdMismatch {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| decision.author_pubkey = public_key(BUYER),
            RadrootsOrderIssue::DecisionAuthorMismatch {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| decision.counterparty_pubkey = public_key(SELLER),
            RadrootsOrderIssue::DecisionCounterpartyMismatch {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| decision.payload.buyer_pubkey = public_key(SELLER),
            RadrootsOrderIssue::DecisionBuyerMismatch {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| decision.payload.seller_pubkey = public_key(BUYER),
            RadrootsOrderIssue::DecisionSellerMismatch {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| decision.payload.listing_addr = draft_listing_addr(),
            RadrootsOrderIssue::DecisionListingAddressInvalid {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| decision.payload.listing_addr = other_seller_listing_addr(),
            RadrootsOrderIssue::DecisionListingMismatch {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| decision.root_event_id = event_id(8),
            RadrootsOrderIssue::DecisionRootMismatch {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| decision.prev_event_id = event_id(8),
            RadrootsOrderIssue::DecisionPreviousMismatch {
                event_id: event_id(2),
            },
        );
        assert_decision_issue(
            |decision| {
                if let RadrootsOrderDecisionOutcome::Accepted {
                    inventory_commitments,
                } = &mut decision.payload.decision
                {
                    inventory_commitments[0].bin_count = 1;
                }
            },
            RadrootsOrderIssue::DecisionInventoryCommitmentMismatch {
                event_id: event_id(2),
            },
        );
    }

    #[test]
    fn reducer_validators_report_revision_and_cancellation_issue_kinds() {
        assert_revision_proposal_issue(
            |proposal| proposal.payload.items.clear(),
            RadrootsOrderIssue::RevisionProposalPayloadInvalid {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.payload.order_id = order_id("order-2"),
            RadrootsOrderIssue::RevisionProposalOrderIdMismatch {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.author_pubkey = public_key(BUYER),
            RadrootsOrderIssue::RevisionProposalAuthorMismatch {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.counterparty_pubkey = public_key(SELLER),
            RadrootsOrderIssue::RevisionProposalCounterpartyMismatch {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.payload.buyer_pubkey = public_key(SELLER),
            RadrootsOrderIssue::RevisionProposalBuyerMismatch {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.payload.seller_pubkey = public_key(BUYER),
            RadrootsOrderIssue::RevisionProposalSellerMismatch {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.payload.listing_addr = draft_listing_addr(),
            RadrootsOrderIssue::RevisionProposalListingAddressInvalid {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.payload.listing_addr = other_seller_listing_addr(),
            RadrootsOrderIssue::RevisionProposalListingMismatch {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.root_event_id = event_id(8),
            RadrootsOrderIssue::RevisionProposalRootMismatch {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.payload.root_event_id = event_id(8),
            RadrootsOrderIssue::RevisionProposalRootMismatch {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.prev_event_id = event_id(8),
            RadrootsOrderIssue::RevisionProposalPreviousMismatch {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.prev_event_id = event_id(3),
            RadrootsOrderIssue::RevisionProposalPreviousMismatch {
                event_id: event_id(3),
            },
        );
        assert_revision_proposal_issue(
            |proposal| proposal.payload.prev_event_id = event_id(8),
            RadrootsOrderIssue::RevisionProposalPreviousMismatch {
                event_id: event_id(3),
            },
        );

        assert_revision_decision_issue(
            |decision| {
                decision.payload.decision =
                    RadrootsOrderRevisionOutcome::Declined { reason: " ".into() };
            },
            RadrootsOrderIssue::RevisionDecisionPayloadInvalid {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.payload.order_id = order_id("order-2"),
            RadrootsOrderIssue::RevisionDecisionOrderIdMismatch {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.author_pubkey = public_key(SELLER),
            RadrootsOrderIssue::RevisionDecisionAuthorMismatch {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.counterparty_pubkey = public_key(BUYER),
            RadrootsOrderIssue::RevisionDecisionCounterpartyMismatch {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.payload.buyer_pubkey = public_key(SELLER),
            RadrootsOrderIssue::RevisionDecisionBuyerMismatch {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.payload.seller_pubkey = public_key(BUYER),
            RadrootsOrderIssue::RevisionDecisionSellerMismatch {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.payload.listing_addr = draft_listing_addr(),
            RadrootsOrderIssue::RevisionDecisionListingAddressInvalid {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.payload.listing_addr = other_seller_listing_addr(),
            RadrootsOrderIssue::RevisionDecisionListingMismatch {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.root_event_id = event_id(8),
            RadrootsOrderIssue::RevisionDecisionRootMismatch {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.payload.root_event_id = event_id(8),
            RadrootsOrderIssue::RevisionDecisionRootMismatch {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.prev_event_id = event_id(8),
            RadrootsOrderIssue::RevisionDecisionPreviousMismatch {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.prev_event_id = event_id(4),
            RadrootsOrderIssue::RevisionDecisionPreviousMismatch {
                event_id: event_id(4),
            },
        );
        assert_revision_decision_issue(
            |decision| decision.payload.prev_event_id = event_id(8),
            RadrootsOrderIssue::RevisionDecisionPreviousMismatch {
                event_id: event_id(4),
            },
        );

        assert_cancellation_issue(
            |cancellation| cancellation.payload.reason = " ".into(),
            RadrootsOrderIssue::CancellationPayloadInvalid {
                event_id: event_id(5),
            },
        );
        assert_cancellation_issue(
            |cancellation| cancellation.payload.order_id = order_id("order-2"),
            RadrootsOrderIssue::CancellationOrderIdMismatch {
                event_id: event_id(5),
            },
        );
        assert_cancellation_issue(
            |cancellation| cancellation.author_pubkey = public_key(SELLER),
            RadrootsOrderIssue::CancellationAuthorMismatch {
                event_id: event_id(5),
            },
        );
        assert_cancellation_issue(
            |cancellation| cancellation.counterparty_pubkey = public_key(BUYER),
            RadrootsOrderIssue::CancellationCounterpartyMismatch {
                event_id: event_id(5),
            },
        );
        assert_cancellation_issue(
            |cancellation| cancellation.payload.buyer_pubkey = public_key(SELLER),
            RadrootsOrderIssue::CancellationBuyerMismatch {
                event_id: event_id(5),
            },
        );
        assert_cancellation_issue(
            |cancellation| cancellation.payload.seller_pubkey = public_key(BUYER),
            RadrootsOrderIssue::CancellationSellerMismatch {
                event_id: event_id(5),
            },
        );
        assert_cancellation_issue(
            |cancellation| cancellation.payload.listing_addr = draft_listing_addr(),
            RadrootsOrderIssue::CancellationListingAddressInvalid {
                event_id: event_id(5),
            },
        );
        assert_cancellation_issue(
            |cancellation| cancellation.payload.listing_addr = other_seller_listing_addr(),
            RadrootsOrderIssue::CancellationListingMismatch {
                event_id: event_id(5),
            },
        );
        assert_cancellation_issue(
            |cancellation| cancellation.root_event_id = event_id(8),
            RadrootsOrderIssue::CancellationRootMismatch {
                event_id: event_id(5),
            },
        );
        assert_cancellation_issue(
            |cancellation| cancellation.prev_event_id = event_id(5),
            RadrootsOrderIssue::CancellationPreviousMismatch {
                event_id: event_id(5),
            },
        );
    }

    #[test]
    fn reducer_reports_invalid_records_from_all_non_request_families() {
        let mut bad_request = request_record();
        bad_request.payload.order_id = order_id("order-2");
        let invalid_request = reduce_order_events(
            &order_id("order-1"),
            RadrootsOrderReductionInputs {
                requests: vec![bad_request],
                decisions: Vec::<RadrootsOrderDecisionRecord>::new(),
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );
        assert_order_issue_kind(
            &invalid_request.issues,
            RadrootsOrderIssue::RequestOrderIdMismatch {
                event_id: event_id(1),
            },
        );

        let mut bad_decision = accepted_decision();
        bad_decision.payload.order_id = order_id("order-2");
        let mut bad_proposal = revision_proposal();
        bad_proposal.payload.order_id = order_id("order-2");
        let mut bad_revision_decision = accepted_revision_decision();
        bad_revision_decision.payload.order_id = order_id("order-2");
        let mut bad_cancellation = cancellation(event_id(1));
        bad_cancellation.payload.order_id = order_id("order-2");
        let invalid_non_requests = reduce_order_events(
            &order_id("order-1"),
            RadrootsOrderReductionInputs {
                requests: vec![request_record()],
                decisions: vec![bad_decision],
                revision_proposals: vec![bad_proposal],
                revision_decisions: vec![bad_revision_decision],
                cancellations: vec![bad_cancellation],
            },
        );

        assert_order_issue_kind(
            &invalid_non_requests.issues,
            RadrootsOrderIssue::DecisionOrderIdMismatch {
                event_id: event_id(2),
            },
        );
        assert_order_issue_kind(
            &invalid_non_requests.issues,
            RadrootsOrderIssue::RevisionProposalOrderIdMismatch {
                event_id: event_id(3),
            },
        );
        assert_order_issue_kind(
            &invalid_non_requests.issues,
            RadrootsOrderIssue::RevisionDecisionOrderIdMismatch {
                event_id: event_id(4),
            },
        );
        assert_order_issue_kind(
            &invalid_non_requests.issues,
            RadrootsOrderIssue::CancellationOrderIdMismatch {
                event_id: event_id(5),
            },
        );
    }

    #[test]
    fn inventory_accounting_reports_invalid_unknown_overreserved_and_terminal_orders() {
        let mut unknown_bin_request = request_record();
        unknown_bin_request.event_id = event_id(40);
        unknown_bin_request.payload.order_id = order_id("order-4");
        unknown_bin_request.payload.items[0].bin_id = bin_id("bin-missing");
        unknown_bin_request.payload.economics.items[0].bin_id = bin_id("bin-missing");

        let mut unknown_bin_decision = accepted_decision();
        unknown_bin_decision.event_id = event_id(41);
        unknown_bin_decision.root_event_id = event_id(40);
        unknown_bin_decision.prev_event_id = event_id(40);
        unknown_bin_decision.payload.order_id = order_id("order-4");
        if let RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } = &mut unknown_bin_decision.payload.decision
        {
            inventory_commitments[0].bin_id = bin_id("bin-missing");
        }

        let mut declined_request = request_record();
        declined_request.event_id = event_id(20);
        declined_request.payload.order_id = order_id("order-2");
        let mut terminal_decline = declined_decision();
        terminal_decline.event_id = event_id(21);
        terminal_decline.root_event_id = event_id(20);
        terminal_decline.prev_event_id = event_id(20);
        terminal_decline.payload.order_id = order_id("order-2");

        let mut cancelled_request = request_record();
        cancelled_request.event_id = event_id(30);
        cancelled_request.payload.order_id = order_id("order-3");
        let mut terminal_cancellation = cancellation(event_id(30));
        terminal_cancellation.event_id = event_id(31);
        terminal_cancellation.root_event_id = event_id(30);
        terminal_cancellation.payload.order_id = order_id("order-3");

        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            &event_id(9),
            RadrootsListingInventoryAccountingInputs {
                bins: vec![
                    RadrootsListingInventoryBinAvailability {
                        bin_id: bin_id("bin-1"),
                        available_count: 1,
                    },
                    RadrootsListingInventoryBinAvailability {
                        bin_id: bin_id("bin-overflow"),
                        available_count: u64::MAX,
                    },
                    RadrootsListingInventoryBinAvailability {
                        bin_id: bin_id("bin-overflow"),
                        available_count: 1,
                    },
                ],
                requests: vec![
                    request_record(),
                    unknown_bin_request,
                    declined_request,
                    cancelled_request,
                ],
                decisions: vec![accepted_decision(), unknown_bin_decision, terminal_decline],
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: vec![terminal_cancellation],
            },
        );

        assert_eq!(projection.declined_order_ids, vec![order_id("order-2")]);
        assert_eq!(projection.cancelled_order_ids, vec![order_id("order-3")]);
        assert_inventory_issue_kind(
            &projection.issues,
            RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                bin_id: bin_id("bin-overflow"),
                event_ids: Vec::new(),
            },
        );
        assert_inventory_issue_kind(
            &projection.issues,
            RadrootsListingInventoryAccountingIssue::UnknownInventoryBin {
                bin_id: bin_id("bin-missing"),
                event_ids: Vec::new(),
            },
        );
        assert_inventory_issue_kind(
            &projection.issues,
            RadrootsListingInventoryAccountingIssue::OverReserved {
                bin_id: bin_id("bin-1"),
                available_count: 0,
                reserved_count: 0,
                event_ids: Vec::new(),
            },
        );

        let invalid_without_request = reduce_listing_inventory_accounting(
            &listing_addr(),
            &event_id(9),
            RadrootsListingInventoryAccountingInputs {
                bins: Vec::<RadrootsListingInventoryBinAvailability>::new(),
                requests: Vec::<RadrootsOrderRequestRecord>::new(),
                decisions: vec![accepted_decision()],
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );
        assert_eq!(invalid_without_request.invalid_event_ids, vec![event_id(2)]);
        assert_inventory_issue_kind(
            &invalid_without_request.issues,
            RadrootsListingInventoryAccountingIssue::InvalidOrder {
                order_id: order_id("order-1"),
                event_ids: Vec::new(),
            },
        );

        let mut duplicate_request = request_record();
        duplicate_request.event_id = event_id(11);
        let invalid_duplicate_requests = reduce_listing_inventory_accounting(
            &listing_addr(),
            &event_id(9),
            RadrootsListingInventoryAccountingInputs {
                bins: Vec::<RadrootsListingInventoryBinAvailability>::new(),
                requests: vec![request_record(), duplicate_request],
                decisions: Vec::<RadrootsOrderDecisionRecord>::new(),
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );
        assert_eq!(
            invalid_duplicate_requests.invalid_event_ids,
            vec![event_id(1), event_id(11)]
        );
        assert_inventory_issue_kind(
            &invalid_duplicate_requests.issues,
            RadrootsListingInventoryAccountingIssue::InvalidOrder {
                order_id: order_id("order-1"),
                event_ids: Vec::new(),
            },
        );

        let invalid_revision_streams = reduce_listing_inventory_accounting(
            &listing_addr(),
            &event_id(9),
            RadrootsListingInventoryAccountingInputs {
                bins: Vec::<RadrootsListingInventoryBinAvailability>::new(),
                requests: Vec::<RadrootsOrderRequestRecord>::new(),
                decisions: vec![accepted_decision()],
                revision_proposals: vec![revision_proposal()],
                revision_decisions: vec![accepted_revision_decision()],
                cancellations: vec![cancellation(event_id(3))],
            },
        );
        assert_eq!(
            invalid_revision_streams.invalid_event_ids,
            vec![event_id(2), event_id(3), event_id(4), event_id(5)]
        );
        assert_inventory_issue_kind(
            &invalid_revision_streams.issues,
            RadrootsListingInventoryAccountingIssue::InvalidOrder {
                order_id: order_id("order-1"),
                event_ids: Vec::new(),
            },
        );
    }

    #[test]
    fn reducer_projects_requested_order() {
        let projection = reduce(Vec::new(), Vec::new(), Vec::new(), Vec::new());

        assert_eq!(projection.issues, Vec::new());
        assert_eq!(projection.status, RadrootsTradeWorkflowState::Requested);
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

        assert_eq!(
            projection.status,
            RadrootsTradeWorkflowState::AgreedPendingRhi
        );
        assert_eq!(projection.decision_event_id, Some(event_id(2)));
        assert_eq!(projection.agreement_event_id, Some(event_id(2)));
        assert!(!projection.lifecycle_terminal);
        assert_eq!(projection.pending_inventory_reservations.len(), 1);
        assert!(projection.committed_inventory_reservations.is_empty());
    }

    #[test]
    fn reducer_projects_declined_order() {
        let projection = reduce(
            vec![declined_decision()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(projection.status, RadrootsTradeWorkflowState::Declined);
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

        assert_eq!(
            projection.status,
            RadrootsTradeWorkflowState::AgreedPendingRhi
        );
        assert_eq!(projection.agreement_event_id, Some(event_id(4)));
        assert_eq!(
            projection.economics.expect("economics").items[0].bin_count,
            1
        );
        assert!(!projection.lifecycle_terminal);
        assert_eq!(projection.pending_inventory_reservations.len(), 1);
        assert!(projection.committed_inventory_reservations.is_empty());
    }

    #[test]
    fn reducer_allows_pre_agreement_cancellation() {
        let projection = reduce(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![cancellation(event_id(1))],
        );

        assert_eq!(projection.status, RadrootsTradeWorkflowState::Cancelled);
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

        assert_eq!(projection.status, RadrootsTradeWorkflowState::Invalid);
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

        assert_eq!(
            projection.status,
            RadrootsTradeWorkflowState::AgreedPendingRhi
        );
        assert_eq!(projection.agreement_event_id, Some(event_id(2)));
    }

    #[test]
    fn inventory_accounting_reserves_only_accepted_agreements() {
        let requested_projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            &event_id(8),
            RadrootsListingInventoryAccountingInputs {
                bins: vec![RadrootsListingInventoryBinAvailability {
                    bin_id: bin_id("bin-1"),
                    available_count: 3,
                }],
                requests: vec![request_record()],
                decisions: Vec::<RadrootsOrderDecisionRecord>::new(),
                revision_proposals: Vec::<RadrootsOrderRevisionProposalRecord>::new(),
                revision_decisions: Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
                cancellations: Vec::<RadrootsOrderCancellationRecord>::new(),
            },
        );

        assert_eq!(requested_projection.bins[0].pending_reserved_count, 0);
        assert_eq!(requested_projection.bins[0].remaining_count, 3);

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

        assert_eq!(projection.bins[0].pending_reserved_count, 2);
        assert_eq!(projection.bins[0].remaining_count, 1);
        assert_eq!(
            projection.bins[0].pending_orders[0].agreement_event_id,
            event_id(2)
        );
    }
}

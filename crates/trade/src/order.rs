#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_core::{RadrootsCoreCurrency, RadrootsCoreDecimal};
#[cfg(feature = "event_store")]
use radroots_event_store::{RadrootsEventStore, RadrootsEventStoreError, RadrootsStoredEvent};
#[cfg(feature = "serde_json")]
use radroots_events::RadrootsNostrEvent;
use radroots_events::ids::{
    RadrootsEconomicsDigest, RadrootsEventId, RadrootsIdParseError, RadrootsInventoryBinId,
    RadrootsListingAddress, RadrootsOrderId, RadrootsOrderQuoteId, RadrootsPublicKey,
};
#[cfg(feature = "serde_json")]
use radroots_events::kinds::{
    KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_FULFILLMENT_UPDATE,
    KIND_ORDER_PAYMENT_RECORD, KIND_ORDER_RECEIPT, KIND_ORDER_REQUEST,
    KIND_ORDER_REVISION_DECISION, KIND_ORDER_REVISION_PROPOSAL, KIND_ORDER_SETTLEMENT_DECISION,
};
#[cfg(feature = "serde_json")]
use radroots_events::order::RadrootsOrderEventType;
use radroots_events::order::{
    RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
    RadrootsOrderEconomics, RadrootsOrderFulfillmentState, RadrootsOrderFulfillmentUpdate,
    RadrootsOrderInventoryCommitment, RadrootsOrderItem, RadrootsOrderPaymentMethod,
    RadrootsOrderPaymentRecord as RadrootsOrderPaymentPayload, RadrootsOrderReceipt,
    RadrootsOrderRequest, RadrootsOrderRevisionDecision, RadrootsOrderRevisionOutcome,
    RadrootsOrderRevisionProposal, RadrootsOrderSettlementDecision, RadrootsOrderSettlementOutcome,
};
#[cfg(feature = "event_store")]
use radroots_events::tags::TAG_D;
#[cfg(feature = "serde_json")]
use radroots_events_codec::order::{
    RadrootsOrderEnvelopeParseError, order_cancellation_from_event, order_decision_from_event,
    order_event_context_from_tags, order_fulfillment_update_from_event,
    order_payment_record_from_event, order_receipt_from_event, order_request_from_event,
    order_revision_decision_from_event, order_revision_proposal_from_event,
    order_settlement_decision_from_event,
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

pub const ORDER_EVENT_CONTRACT_IDS: [&str; 9] = [
    "radroots.order.request.v1",
    "radroots.order.decision.v1",
    "radroots.order.revision_proposal.v1",
    "radroots.order.revision_decision.v1",
    "radroots.order.cancellation.v1",
    "radroots.order.fulfillment_update.v1",
    "radroots.order.receipt.v1",
    "radroots.order.payment_record.v1",
    "radroots.order.settlement_decision.v1",
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
pub struct RadrootsOrderFulfillmentRecord {
    pub event_id: RadrootsEventId,
    pub author_pubkey: RadrootsPublicKey,
    pub counterparty_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub prev_event_id: RadrootsEventId,
    pub payload: RadrootsOrderFulfillmentUpdate,
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
pub struct RadrootsOrderReceiptRecord {
    pub event_id: RadrootsEventId,
    pub author_pubkey: RadrootsPublicKey,
    pub counterparty_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub prev_event_id: RadrootsEventId,
    pub payload: RadrootsOrderReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderPaymentEventRecord {
    pub event_id: RadrootsEventId,
    pub author_pubkey: RadrootsPublicKey,
    pub counterparty_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub prev_event_id: RadrootsEventId,
    pub payload: RadrootsOrderPaymentPayload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderSettlementRecord {
    pub event_id: RadrootsEventId,
    pub author_pubkey: RadrootsPublicKey,
    pub counterparty_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub prev_event_id: RadrootsEventId,
    pub payload: RadrootsOrderSettlementDecision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOrderEventRecord {
    Request(RadrootsOrderRequestRecord),
    Decision(RadrootsOrderDecisionRecord),
    RevisionProposal(RadrootsOrderRevisionProposalRecord),
    RevisionDecision(RadrootsOrderRevisionDecisionRecord),
    Fulfillment(RadrootsOrderFulfillmentRecord),
    Cancellation(RadrootsOrderCancellationRecord),
    Receipt(RadrootsOrderReceiptRecord),
    Payment(RadrootsOrderPaymentEventRecord),
    Settlement(RadrootsOrderSettlementRecord),
}

impl RadrootsOrderEventRecord {
    pub fn event_id(&self) -> &RadrootsEventId {
        match self {
            Self::Request(record) => &record.event_id,
            Self::Decision(record) => &record.event_id,
            Self::RevisionProposal(record) => &record.event_id,
            Self::RevisionDecision(record) => &record.event_id,
            Self::Fulfillment(record) => &record.event_id,
            Self::Cancellation(record) => &record.event_id,
            Self::Receipt(record) => &record.event_id,
            Self::Payment(record) => &record.event_id,
            Self::Settlement(record) => &record.event_id,
        }
    }

    pub fn order_id(&self) -> &RadrootsOrderId {
        match self {
            Self::Request(record) => &record.payload.order_id,
            Self::Decision(record) => &record.payload.order_id,
            Self::RevisionProposal(record) => &record.payload.order_id,
            Self::RevisionDecision(record) => &record.payload.order_id,
            Self::Fulfillment(record) => &record.payload.order_id,
            Self::Cancellation(record) => &record.payload.order_id,
            Self::Receipt(record) => &record.payload.order_id,
            Self::Payment(record) => &record.payload.order_id,
            Self::Settlement(record) => &record.payload.order_id,
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
        KIND_ORDER_FULFILLMENT_UPDATE => {
            let envelope = order_fulfillment_update_from_event(event)?;
            Ok(RadrootsOrderEventRecord::Fulfillment(
                RadrootsOrderFulfillmentRecord {
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
        KIND_ORDER_RECEIPT => {
            let envelope = order_receipt_from_event(event)?;
            Ok(RadrootsOrderEventRecord::Receipt(
                RadrootsOrderReceiptRecord {
                    event_id,
                    author_pubkey,
                    counterparty_pubkey: context.counterparty_pubkey.clone(),
                    root_event_id: require_context_root_event_id(&context)?,
                    prev_event_id: require_context_prev_event_id(&context)?,
                    payload: envelope.payload,
                },
            ))
        }
        KIND_ORDER_PAYMENT_RECORD => {
            let envelope = order_payment_record_from_event(event)?;
            Ok(RadrootsOrderEventRecord::Payment(
                RadrootsOrderPaymentEventRecord {
                    event_id,
                    author_pubkey,
                    counterparty_pubkey: context.counterparty_pubkey.clone(),
                    root_event_id: require_context_root_event_id(&context)?,
                    prev_event_id: require_context_prev_event_id(&context)?,
                    payload: envelope.payload,
                },
            ))
        }
        KIND_ORDER_SETTLEMENT_DECISION => {
            let envelope = order_settlement_decision_from_event(event)?;
            Ok(RadrootsOrderEventRecord::Settlement(
                RadrootsOrderSettlementRecord {
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
    Completed,
    Disputed,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOrderPaymentState {
    NotRecorded,
    Recorded,
    Settled,
    Rejected,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOrderSettlementState {
    NotRequired,
    Pending,
    Accepted,
    Rejected,
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
    RevisionProposalWithoutAcceptedDecision { event_id: RadrootsEventId },
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
    FulfillmentWithoutAcceptedDecision { event_id: RadrootsEventId },
    FulfillmentPayloadInvalid { event_id: RadrootsEventId },
    FulfillmentOrderIdMismatch { event_id: RadrootsEventId },
    FulfillmentAuthorMismatch { event_id: RadrootsEventId },
    FulfillmentCounterpartyMismatch { event_id: RadrootsEventId },
    FulfillmentBuyerMismatch { event_id: RadrootsEventId },
    FulfillmentSellerMismatch { event_id: RadrootsEventId },
    FulfillmentListingAddressInvalid { event_id: RadrootsEventId },
    FulfillmentListingMismatch { event_id: RadrootsEventId },
    FulfillmentRootMismatch { event_id: RadrootsEventId },
    FulfillmentPreviousMismatch { event_id: RadrootsEventId },
    FulfillmentStatusNotPublishable { event_id: RadrootsEventId },
    FulfillmentUnsupportedTransition { event_id: RadrootsEventId },
    ForkedFulfillments { event_ids: Vec<RadrootsEventId> },
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
    CancellationAfterFulfillment { event_id: RadrootsEventId },
    ReceiptWithoutEligibleFulfillment { event_id: RadrootsEventId },
    ReceiptPayloadInvalid { event_id: RadrootsEventId },
    ReceiptOrderIdMismatch { event_id: RadrootsEventId },
    ReceiptAuthorMismatch { event_id: RadrootsEventId },
    ReceiptCounterpartyMismatch { event_id: RadrootsEventId },
    ReceiptBuyerMismatch { event_id: RadrootsEventId },
    ReceiptSellerMismatch { event_id: RadrootsEventId },
    ReceiptListingAddressInvalid { event_id: RadrootsEventId },
    ReceiptListingMismatch { event_id: RadrootsEventId },
    ReceiptRootMismatch { event_id: RadrootsEventId },
    ReceiptPreviousMismatch { event_id: RadrootsEventId },
    PaymentWithoutAcceptedAgreement { event_id: RadrootsEventId },
    PaymentPayloadInvalid { event_id: RadrootsEventId },
    PaymentOrderIdMismatch { event_id: RadrootsEventId },
    PaymentAuthorMismatch { event_id: RadrootsEventId },
    PaymentCounterpartyMismatch { event_id: RadrootsEventId },
    PaymentBuyerMismatch { event_id: RadrootsEventId },
    PaymentSellerMismatch { event_id: RadrootsEventId },
    PaymentListingAddressInvalid { event_id: RadrootsEventId },
    PaymentListingMismatch { event_id: RadrootsEventId },
    PaymentRootMismatch { event_id: RadrootsEventId },
    PaymentPreviousMismatch { event_id: RadrootsEventId },
    PaymentAgreementMismatch { event_id: RadrootsEventId },
    PaymentQuoteMismatch { event_id: RadrootsEventId },
    PaymentQuoteVersionMismatch { event_id: RadrootsEventId },
    PaymentEconomicsDigestMismatch { event_id: RadrootsEventId },
    PaymentAmountMismatch { event_id: RadrootsEventId },
    PaymentCurrencyMismatch { event_id: RadrootsEventId },
    PaymentAfterCancellation { event_id: RadrootsEventId },
    RevisionAfterPayment { event_id: RadrootsEventId },
    DuplicatePayments { event_ids: Vec<RadrootsEventId> },
    SettlementWithoutValidPayment { event_id: RadrootsEventId },
    SettlementPayloadInvalid { event_id: RadrootsEventId },
    SettlementOrderIdMismatch { event_id: RadrootsEventId },
    SettlementAuthorMismatch { event_id: RadrootsEventId },
    SettlementCounterpartyMismatch { event_id: RadrootsEventId },
    SettlementBuyerMismatch { event_id: RadrootsEventId },
    SettlementSellerMismatch { event_id: RadrootsEventId },
    SettlementListingAddressInvalid { event_id: RadrootsEventId },
    SettlementListingMismatch { event_id: RadrootsEventId },
    SettlementRootMismatch { event_id: RadrootsEventId },
    SettlementPreviousMismatch { event_id: RadrootsEventId },
    SettlementPaymentEventMismatch { event_id: RadrootsEventId },
    SettlementAgreementMismatch { event_id: RadrootsEventId },
    SettlementQuoteMismatch { event_id: RadrootsEventId },
    SettlementQuoteVersionMismatch { event_id: RadrootsEventId },
    SettlementEconomicsDigestMismatch { event_id: RadrootsEventId },
    SettlementAmountMismatch { event_id: RadrootsEventId },
    SettlementCurrencyMismatch { event_id: RadrootsEventId },
    DuplicateSettlements { event_ids: Vec<RadrootsEventId> },
    ForkedLifecycle { event_ids: Vec<RadrootsEventId> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderPaymentProjection {
    pub state: RadrootsOrderPaymentState,
    pub settlement_state: RadrootsOrderSettlementState,
    pub payment_event_id: Option<RadrootsEventId>,
    pub settlement_event_id: Option<RadrootsEventId>,
    pub agreement_event_id: Option<RadrootsEventId>,
    pub quote_id: Option<RadrootsOrderQuoteId>,
    pub quote_version: Option<u32>,
    pub economics_digest: Option<RadrootsEconomicsDigest>,
    pub amount: Option<RadrootsCoreDecimal>,
    pub currency: Option<RadrootsCoreCurrency>,
    pub method: Option<RadrootsOrderPaymentMethod>,
    pub reference: Option<String>,
    pub paid_at: Option<u64>,
    pub reason: Option<String>,
}

impl RadrootsOrderPaymentProjection {
    pub fn not_recorded() -> Self {
        Self {
            state: RadrootsOrderPaymentState::NotRecorded,
            settlement_state: RadrootsOrderSettlementState::NotRequired,
            payment_event_id: None,
            settlement_event_id: None,
            agreement_event_id: None,
            quote_id: None,
            quote_version: None,
            economics_digest: None,
            amount: None,
            currency: None,
            method: None,
            reference: None,
            paid_at: None,
            reason: None,
        }
    }

    pub fn invalid() -> Self {
        let mut projection = Self::not_recorded();
        projection.state = RadrootsOrderPaymentState::Invalid;
        projection.settlement_state = RadrootsOrderSettlementState::Invalid;
        projection
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOrderProjection {
    pub order_id: RadrootsOrderId,
    pub status: RadrootsOrderStatus,
    pub request_event_id: Option<RadrootsEventId>,
    pub decision_event_id: Option<RadrootsEventId>,
    pub fulfillment_event_id: Option<RadrootsEventId>,
    pub fulfillment_status: Option<RadrootsOrderFulfillmentState>,
    pub cancellation_event_id: Option<RadrootsEventId>,
    pub receipt_event_id: Option<RadrootsEventId>,
    pub receipt_received: Option<bool>,
    pub receipt_issue: Option<String>,
    pub receipt_received_at: Option<u64>,
    pub lifecycle_terminal: bool,
    pub payment: RadrootsOrderPaymentProjection,
    pub economics: Option<RadrootsOrderEconomics>,
    pub agreement_event_id: Option<RadrootsEventId>,
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
    pub decision_event_id: RadrootsEventId,
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
pub struct RadrootsOrderReductionInputs<I, J, K, L, M, N, O, P, Q> {
    pub requests: I,
    pub decisions: J,
    pub revision_proposals: K,
    pub revision_decisions: L,
    pub fulfillments: M,
    pub cancellations: N,
    pub receipts: O,
    pub payments: P,
    pub settlements: Q,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsGroupedOrderEventRecords {
    pub requests: Vec<RadrootsOrderRequestRecord>,
    pub decisions: Vec<RadrootsOrderDecisionRecord>,
    pub revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
    pub revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
    pub fulfillments: Vec<RadrootsOrderFulfillmentRecord>,
    pub cancellations: Vec<RadrootsOrderCancellationRecord>,
    pub receipts: Vec<RadrootsOrderReceiptRecord>,
    pub payments: Vec<RadrootsOrderPaymentEventRecord>,
    pub settlements: Vec<RadrootsOrderSettlementRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingInventoryAccountingInputs<I, J, K, L, M, N, O, P> {
    pub bins: I,
    pub requests: J,
    pub decisions: K,
    pub revision_proposals: L,
    pub revision_decisions: M,
    pub fulfillments: N,
    pub cancellations: O,
    pub receipts: P,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RadrootsListingInventoryAccountingRecords {
    bins: Vec<RadrootsListingInventoryBinAvailability>,
    requests: Vec<RadrootsOrderRequestRecord>,
    decisions: Vec<RadrootsOrderDecisionRecord>,
    revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
    revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
    fulfillments: Vec<RadrootsOrderFulfillmentRecord>,
    cancellations: Vec<RadrootsOrderCancellationRecord>,
    receipts: Vec<RadrootsOrderReceiptRecord>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RadrootsOrderDecisionProjectionRecords {
    revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
    revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
    fulfillments: Vec<RadrootsOrderFulfillmentRecord>,
    cancellations: Vec<RadrootsOrderCancellationRecord>,
    receipts: Vec<RadrootsOrderReceiptRecord>,
    payments: Vec<RadrootsOrderPaymentEventRecord>,
    settlements: Vec<RadrootsOrderSettlementRecord>,
}

struct RadrootsReceiptProjectionInput<'a> {
    order_id: &'a RadrootsOrderId,
    request: &'a RadrootsOrderRequestRecord,
    decision: &'a RadrootsOrderDecisionRecord,
    agreement_event_id: &'a RadrootsEventId,
    economics: &'a RadrootsOrderEconomics,
    latest_fulfillment: Option<&'a RadrootsOrderFulfillmentRecord>,
    fulfillments: &'a [RadrootsOrderFulfillmentRecord],
    receipts: Vec<RadrootsOrderReceiptRecord>,
    issues: &'a mut Vec<RadrootsOrderIssue>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn reduce_order_events<I, J, K, L, M, N, O, P, Q>(
    order_id: &RadrootsOrderId,
    inputs: RadrootsOrderReductionInputs<I, J, K, L, M, N, O, P, Q>,
) -> RadrootsOrderProjection
where
    I: IntoIterator<Item = RadrootsOrderRequestRecord>,
    J: IntoIterator<Item = RadrootsOrderDecisionRecord>,
    K: IntoIterator<Item = RadrootsOrderRevisionProposalRecord>,
    L: IntoIterator<Item = RadrootsOrderRevisionDecisionRecord>,
    M: IntoIterator<Item = RadrootsOrderFulfillmentRecord>,
    N: IntoIterator<Item = RadrootsOrderCancellationRecord>,
    O: IntoIterator<Item = RadrootsOrderReceiptRecord>,
    P: IntoIterator<Item = RadrootsOrderPaymentEventRecord>,
    Q: IntoIterator<Item = RadrootsOrderSettlementRecord>,
{
    reduce_grouped_order_event_records(
        order_id,
        RadrootsGroupedOrderEventRecords {
            requests: inputs.requests.into_iter().collect(),
            decisions: inputs.decisions.into_iter().collect(),
            revision_proposals: inputs.revision_proposals.into_iter().collect(),
            revision_decisions: inputs.revision_decisions.into_iter().collect(),
            fulfillments: inputs.fulfillments.into_iter().collect(),
            cancellations: inputs.cancellations.into_iter().collect(),
            receipts: inputs.receipts.into_iter().collect(),
            payments: inputs.payments.into_iter().collect(),
            settlements: inputs.settlements.into_iter().collect(),
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
    let mut requests = Vec::new();
    let mut decisions = Vec::new();
    let mut revision_proposals = Vec::new();
    let mut revision_decisions = Vec::new();
    let mut fulfillments = Vec::new();
    let mut cancellations = Vec::new();
    let mut receipts = Vec::new();
    let mut payments = Vec::new();
    let mut settlements = Vec::new();

    for record in records {
        let event_id = record.event_id().clone();
        if seen_event_ids.iter().any(|seen| seen == &event_id) {
            continue;
        }
        seen_event_ids.push(event_id);
        match record {
            RadrootsOrderEventRecord::Request(record) => requests.push(record),
            RadrootsOrderEventRecord::Decision(record) => decisions.push(record),
            RadrootsOrderEventRecord::RevisionProposal(record) => revision_proposals.push(record),
            RadrootsOrderEventRecord::RevisionDecision(record) => revision_decisions.push(record),
            RadrootsOrderEventRecord::Fulfillment(record) => fulfillments.push(record),
            RadrootsOrderEventRecord::Cancellation(record) => cancellations.push(record),
            RadrootsOrderEventRecord::Receipt(record) => receipts.push(record),
            RadrootsOrderEventRecord::Payment(record) => payments.push(record),
            RadrootsOrderEventRecord::Settlement(record) => settlements.push(record),
        }
    }

    reduce_grouped_order_event_records(
        order_id,
        RadrootsGroupedOrderEventRecords {
            requests,
            decisions,
            revision_proposals,
            revision_decisions,
            fulfillments,
            cancellations,
            receipts,
            payments,
            settlements,
        },
    )
}

fn reduce_grouped_order_event_records(
    order_id: &RadrootsOrderId,
    records: RadrootsGroupedOrderEventRecords,
) -> RadrootsOrderProjection {
    let requests = unique_request_records(records.requests);
    let decisions = unique_decision_records(records.decisions);
    let revision_proposals = unique_revision_proposal_records(records.revision_proposals);
    let revision_decisions = unique_revision_decision_records(records.revision_decisions);
    let fulfillments = unique_fulfillment_records(records.fulfillments);
    let cancellations = unique_cancellation_records(records.cancellations);
    let receipts = unique_receipt_records(records.receipts);
    let payments = unique_payment_records(records.payments);
    let settlements = unique_settlement_records(records.settlements);
    if requests.is_empty()
        && decisions.is_empty()
        && revision_proposals.is_empty()
        && revision_decisions.is_empty()
        && fulfillments.is_empty()
        && cancellations.is_empty()
        && receipts.is_empty()
        && payments.is_empty()
        && settlements.is_empty()
    {
        return RadrootsOrderProjection {
            order_id: order_id.clone(),
            status: RadrootsOrderStatus::Missing,
            request_event_id: None,
            decision_event_id: None,
            fulfillment_event_id: None,
            fulfillment_status: None,
            cancellation_event_id: None,
            receipt_event_id: None,
            receipt_received: None,
            receipt_issue: None,
            receipt_received_at: None,
            lifecycle_terminal: false,
            payment: RadrootsOrderPaymentProjection::not_recorded(),
            economics: None,
            agreement_event_id: None,
            listing_addr: None,
            buyer_pubkey: None,
            seller_pubkey: None,
            last_event_id: None,
            issues: Vec::new(),
        };
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
        event_ids.sort();
        issues.push(RadrootsOrderIssue::MultipleRequests { event_ids });
    }

    let Some(request) = valid_requests.first() else {
        if decisions.is_empty()
            && revision_proposals.is_empty()
            && revision_decisions.is_empty()
            && fulfillments.is_empty()
            && cancellations.is_empty()
            && receipts.is_empty()
            && payments.is_empty()
            && settlements.is_empty()
        {
            return invalid_projection(order_id, None, issues);
        }
        issues.push(RadrootsOrderIssue::MissingRequest);
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

    if !issues.is_empty() {
        return invalid_projection(order_id, Some(request), issues);
    }

    let mut valid_cancellations = Vec::new();
    for cancellation in cancellations {
        if validate_order_cancellation_record(request, &cancellation, &mut issues) {
            valid_cancellations.push(cancellation);
        }
    }
    let mut valid_receipts = Vec::new();
    for receipt in receipts {
        if validate_order_receipt_record(request, &receipt, &mut issues) {
            valid_receipts.push(receipt);
        }
    }
    if !issues.is_empty() {
        return invalid_projection(order_id, Some(request), issues);
    }

    let request_cancellations = valid_cancellations
        .iter()
        .filter(|cancellation| cancellation.prev_event_id == request.event_id)
        .collect::<Vec<_>>();
    if !request_cancellations.is_empty() && !valid_decisions.is_empty() {
        let mut event_ids = valid_decisions
            .iter()
            .map(|decision| decision.event_id.clone())
            .collect::<Vec<_>>();
        event_ids.extend(
            request_cancellations
                .iter()
                .map(|cancellation| cancellation.event_id.clone()),
        );
        sort_and_dedup_values(&mut event_ids);
        return invalid_projection(
            order_id,
            Some(request),
            vec![RadrootsOrderIssue::ForkedLifecycle { event_ids }],
        );
    }

    match valid_decisions.len() {
        0 => {
            record_revision_proposal_without_accepted_decision(
                &valid_revision_proposals,
                &mut issues,
            );
            record_revision_decision_without_proposal(&valid_revision_decisions, &mut issues);
            if !fulfillments.is_empty() {
                record_fulfillment_without_accepted_decision(&fulfillments, &mut issues);
            }
            if !valid_receipts.is_empty() {
                record_receipt_without_eligible_fulfillment(&valid_receipts, &mut issues);
            }
            record_payment_without_accepted_agreement(&payments, &mut issues);
            record_settlement_without_valid_payment(&settlements, &mut issues);
            if !issues.is_empty() {
                invalid_projection_with_payment(
                    order_id,
                    Some(request),
                    issues,
                    RadrootsOrderPaymentProjection::invalid(),
                )
            } else if valid_cancellations.is_empty() {
                requested_projection(order_id, request)
            } else {
                requested_cancellation_projection(order_id, request, valid_cancellations)
            }
        }
        1 => decided_projection(
            order_id,
            request,
            &valid_decisions[0],
            RadrootsOrderDecisionProjectionRecords {
                revision_proposals: valid_revision_proposals,
                revision_decisions: valid_revision_decisions,
                fulfillments,
                cancellations: valid_cancellations,
                receipts: valid_receipts,
                payments,
                settlements,
            },
        ),
        _ => {
            let mut event_ids = valid_decisions
                .iter()
                .map(|decision| decision.event_id.clone())
                .collect::<Vec<_>>();
            event_ids.sort();
            invalid_projection(
                order_id,
                Some(request),
                vec![RadrootsOrderIssue::ConflictingDecisions { event_ids }],
            )
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn reduce_listing_inventory_accounting<I, J, K, L, M, N, O, P>(
    listing_addr: &RadrootsListingAddress,
    listing_event_id: &RadrootsEventId,
    inputs: RadrootsListingInventoryAccountingInputs<I, J, K, L, M, N, O, P>,
) -> RadrootsListingInventoryAccountingProjection
where
    I: IntoIterator<Item = RadrootsListingInventoryBinAvailability>,
    J: IntoIterator<Item = RadrootsOrderRequestRecord>,
    K: IntoIterator<Item = RadrootsOrderDecisionRecord>,
    L: IntoIterator<Item = RadrootsOrderRevisionProposalRecord>,
    M: IntoIterator<Item = RadrootsOrderRevisionDecisionRecord>,
    N: IntoIterator<Item = RadrootsOrderFulfillmentRecord>,
    O: IntoIterator<Item = RadrootsOrderCancellationRecord>,
    P: IntoIterator<Item = RadrootsOrderReceiptRecord>,
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
            fulfillments: inputs.fulfillments.into_iter().collect(),
            cancellations: inputs.cancellations.into_iter().collect(),
            receipts: inputs.receipts.into_iter().collect(),
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
    let fulfillments = unique_fulfillment_records(records.fulfillments)
        .into_iter()
        .filter(|fulfillment| fulfillment.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let cancellations = unique_cancellation_records(records.cancellations)
        .into_iter()
        .filter(|cancellation| cancellation.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let receipts = unique_receipt_records(records.receipts)
        .into_iter()
        .filter(|receipt| receipt.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let mut order_ids = listing_order_ids(
        &requests,
        &decisions,
        &revision_proposals,
        &revision_decisions,
        &fulfillments,
        &cancellations,
        &receipts,
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
        let order_fulfillments = fulfillments
            .iter()
            .filter(|fulfillment| fulfillment.payload.order_id == order_id)
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
        let order_receipts = receipts
            .iter()
            .filter(|receipt| receipt.payload.order_id == order_id)
            .cloned()
            .collect::<Vec<_>>();
        let projection = reduce_order_events(
            &order_id,
            RadrootsOrderReductionInputs {
                requests: order_requests.clone(),
                decisions: order_decisions.clone(),
                revision_proposals: order_revision_proposals.clone(),
                revision_decisions: order_revision_decisions.clone(),
                fulfillments: order_fulfillments.clone(),
                cancellations: order_cancellations.clone(),
                receipts: order_receipts.clone(),
                payments: Vec::<RadrootsOrderPaymentEventRecord>::new(),
                settlements: Vec::<RadrootsOrderSettlementRecord>::new(),
            },
        );
        match projection.status {
            RadrootsOrderStatus::Accepted
            | RadrootsOrderStatus::Completed
            | RadrootsOrderStatus::Disputed => {
                if projection.fulfillment_status
                    == Some(RadrootsOrderFulfillmentState::SellerCancelled)
                {
                    continue;
                }
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
                        order_fulfillments
                            .iter()
                            .map(|fulfillment| fulfillment.event_id.clone()),
                    );
                    event_ids.extend(
                        order_cancellations
                            .iter()
                            .map(|cancellation| cancellation.event_id.clone()),
                    );
                    event_ids.extend(
                        order_receipts
                            .iter()
                            .map(|receipt| receipt.event_id.clone()),
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

fn unique_request_records(
    requests: Vec<RadrootsOrderRequestRecord>,
) -> Vec<RadrootsOrderRequestRecord> {
    let mut unique = Vec::new();
    let mut records = requests;
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for request in records {
        if unique
            .iter()
            .all(|existing: &RadrootsOrderRequestRecord| existing.event_id != request.event_id)
        {
            unique.push(request);
        }
    }
    unique
}

fn unique_decision_records(
    decisions: Vec<RadrootsOrderDecisionRecord>,
) -> Vec<RadrootsOrderDecisionRecord> {
    let mut unique = Vec::new();
    let mut records = decisions;
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for decision in records {
        if unique
            .iter()
            .all(|existing: &RadrootsOrderDecisionRecord| existing.event_id != decision.event_id)
        {
            unique.push(decision);
        }
    }
    unique
}

fn unique_revision_proposal_records(
    revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
) -> Vec<RadrootsOrderRevisionProposalRecord> {
    let mut unique = Vec::new();
    let mut records = revision_proposals;
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for proposal in records {
        if unique
            .iter()
            .all(|existing: &RadrootsOrderRevisionProposalRecord| {
                existing.event_id != proposal.event_id
            })
        {
            unique.push(proposal);
        }
    }
    unique
}

fn unique_revision_decision_records(
    revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
) -> Vec<RadrootsOrderRevisionDecisionRecord> {
    let mut unique = Vec::new();
    let mut records = revision_decisions;
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for decision in records {
        if unique
            .iter()
            .all(|existing: &RadrootsOrderRevisionDecisionRecord| {
                existing.event_id != decision.event_id
            })
        {
            unique.push(decision);
        }
    }
    unique
}

fn unique_fulfillment_records(
    fulfillments: Vec<RadrootsOrderFulfillmentRecord>,
) -> Vec<RadrootsOrderFulfillmentRecord> {
    let mut unique = Vec::new();
    let mut records = fulfillments;
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for fulfillment in records {
        if unique
            .iter()
            .all(|existing: &RadrootsOrderFulfillmentRecord| {
                existing.event_id != fulfillment.event_id
            })
        {
            unique.push(fulfillment);
        }
    }
    unique
}

fn unique_cancellation_records(
    cancellations: Vec<RadrootsOrderCancellationRecord>,
) -> Vec<RadrootsOrderCancellationRecord> {
    let mut unique = Vec::new();
    let mut records = cancellations;
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for cancellation in records {
        if unique
            .iter()
            .all(|existing: &RadrootsOrderCancellationRecord| {
                existing.event_id != cancellation.event_id
            })
        {
            unique.push(cancellation);
        }
    }
    unique
}

fn unique_receipt_records(
    receipts: Vec<RadrootsOrderReceiptRecord>,
) -> Vec<RadrootsOrderReceiptRecord> {
    let mut unique = Vec::new();
    let mut records = receipts;
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for receipt in records {
        if unique
            .iter()
            .all(|existing: &RadrootsOrderReceiptRecord| existing.event_id != receipt.event_id)
        {
            unique.push(receipt);
        }
    }
    unique
}

fn unique_payment_records(
    payments: Vec<RadrootsOrderPaymentEventRecord>,
) -> Vec<RadrootsOrderPaymentEventRecord> {
    let mut unique = Vec::new();
    let mut records = payments;
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for payment in records {
        if unique
            .iter()
            .all(|existing: &RadrootsOrderPaymentEventRecord| existing.event_id != payment.event_id)
        {
            unique.push(payment);
        }
    }
    unique
}

fn unique_settlement_records(
    settlements: Vec<RadrootsOrderSettlementRecord>,
) -> Vec<RadrootsOrderSettlementRecord> {
    let mut unique = Vec::new();
    let mut records = settlements;
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for settlement in records {
        if unique
            .iter()
            .all(|existing: &RadrootsOrderSettlementRecord| {
                existing.event_id != settlement.event_id
            })
        {
            unique.push(settlement);
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
    fulfillments: &[RadrootsOrderFulfillmentRecord],
    cancellations: &[RadrootsOrderCancellationRecord],
    receipts: &[RadrootsOrderReceiptRecord],
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
        fulfillments
            .iter()
            .map(|fulfillment| fulfillment.payload.order_id.clone()),
    );
    order_ids.extend(
        cancellations
            .iter()
            .map(|cancellation| cancellation.payload.order_id.clone()),
    );
    order_ids.extend(
        receipts
            .iter()
            .map(|receipt| receipt.payload.order_id.clone()),
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

#[cfg(test)]
fn add_inventory_reservation(
    bin: &mut RadrootsListingInventoryBinAccounting,
    order_id: &RadrootsOrderId,
    decision: &RadrootsOrderDecisionRecord,
    bin_count: u64,
    issues: &mut Vec<RadrootsListingInventoryAccountingIssue>,
) {
    add_inventory_reservation_event(bin, order_id, &decision.event_id, bin_count, issues);
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
                decision_event_id: event_id.clone(),
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
                .then_with(|| left.decision_event_id.cmp(&right.decision_event_id))
        });
        bin.remaining_count = bin
            .available_count
            .saturating_sub(bin.accepted_reserved_count);
        bin.over_reserved = bin.accepted_reserved_count > bin.available_count;
        if bin.over_reserved {
            let mut event_ids = bin
                .accepted_orders
                .iter()
                .map(|reservation| reservation.decision_event_id.clone())
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
            | RadrootsOrderIssue::DuplicatePayments { event_ids: ids }
            | RadrootsOrderIssue::DuplicateSettlements { event_ids: ids }
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
            | RadrootsOrderIssue::RevisionProposalWithoutAcceptedDecision { event_id }
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
            | RadrootsOrderIssue::FulfillmentWithoutAcceptedDecision { event_id }
            | RadrootsOrderIssue::FulfillmentPayloadInvalid { event_id }
            | RadrootsOrderIssue::FulfillmentOrderIdMismatch { event_id }
            | RadrootsOrderIssue::FulfillmentAuthorMismatch { event_id }
            | RadrootsOrderIssue::FulfillmentCounterpartyMismatch { event_id }
            | RadrootsOrderIssue::FulfillmentBuyerMismatch { event_id }
            | RadrootsOrderIssue::FulfillmentSellerMismatch { event_id }
            | RadrootsOrderIssue::FulfillmentListingAddressInvalid { event_id }
            | RadrootsOrderIssue::FulfillmentListingMismatch { event_id }
            | RadrootsOrderIssue::FulfillmentRootMismatch { event_id }
            | RadrootsOrderIssue::FulfillmentPreviousMismatch { event_id }
            | RadrootsOrderIssue::FulfillmentStatusNotPublishable { event_id }
            | RadrootsOrderIssue::FulfillmentUnsupportedTransition { event_id }
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
            | RadrootsOrderIssue::CancellationAfterFulfillment { event_id }
            | RadrootsOrderIssue::ReceiptWithoutEligibleFulfillment { event_id }
            | RadrootsOrderIssue::ReceiptPayloadInvalid { event_id }
            | RadrootsOrderIssue::ReceiptOrderIdMismatch { event_id }
            | RadrootsOrderIssue::ReceiptAuthorMismatch { event_id }
            | RadrootsOrderIssue::ReceiptCounterpartyMismatch { event_id }
            | RadrootsOrderIssue::ReceiptBuyerMismatch { event_id }
            | RadrootsOrderIssue::ReceiptSellerMismatch { event_id }
            | RadrootsOrderIssue::ReceiptListingAddressInvalid { event_id }
            | RadrootsOrderIssue::ReceiptListingMismatch { event_id }
            | RadrootsOrderIssue::ReceiptRootMismatch { event_id }
            | RadrootsOrderIssue::ReceiptPreviousMismatch { event_id }
            | RadrootsOrderIssue::PaymentWithoutAcceptedAgreement { event_id }
            | RadrootsOrderIssue::PaymentPayloadInvalid { event_id }
            | RadrootsOrderIssue::PaymentOrderIdMismatch { event_id }
            | RadrootsOrderIssue::PaymentAuthorMismatch { event_id }
            | RadrootsOrderIssue::PaymentCounterpartyMismatch { event_id }
            | RadrootsOrderIssue::PaymentBuyerMismatch { event_id }
            | RadrootsOrderIssue::PaymentSellerMismatch { event_id }
            | RadrootsOrderIssue::PaymentListingAddressInvalid { event_id }
            | RadrootsOrderIssue::PaymentListingMismatch { event_id }
            | RadrootsOrderIssue::PaymentRootMismatch { event_id }
            | RadrootsOrderIssue::PaymentPreviousMismatch { event_id }
            | RadrootsOrderIssue::PaymentAgreementMismatch { event_id }
            | RadrootsOrderIssue::PaymentQuoteMismatch { event_id }
            | RadrootsOrderIssue::PaymentQuoteVersionMismatch { event_id }
            | RadrootsOrderIssue::PaymentEconomicsDigestMismatch { event_id }
            | RadrootsOrderIssue::PaymentAmountMismatch { event_id }
            | RadrootsOrderIssue::PaymentCurrencyMismatch { event_id }
            | RadrootsOrderIssue::PaymentAfterCancellation { event_id }
            | RadrootsOrderIssue::RevisionAfterPayment { event_id }
            | RadrootsOrderIssue::SettlementWithoutValidPayment { event_id }
            | RadrootsOrderIssue::SettlementPayloadInvalid { event_id }
            | RadrootsOrderIssue::SettlementOrderIdMismatch { event_id }
            | RadrootsOrderIssue::SettlementAuthorMismatch { event_id }
            | RadrootsOrderIssue::SettlementCounterpartyMismatch { event_id }
            | RadrootsOrderIssue::SettlementBuyerMismatch { event_id }
            | RadrootsOrderIssue::SettlementSellerMismatch { event_id }
            | RadrootsOrderIssue::SettlementListingAddressInvalid { event_id }
            | RadrootsOrderIssue::SettlementListingMismatch { event_id }
            | RadrootsOrderIssue::SettlementRootMismatch { event_id }
            | RadrootsOrderIssue::SettlementPreviousMismatch { event_id }
            | RadrootsOrderIssue::SettlementPaymentEventMismatch { event_id }
            | RadrootsOrderIssue::SettlementAgreementMismatch { event_id }
            | RadrootsOrderIssue::SettlementQuoteMismatch { event_id }
            | RadrootsOrderIssue::SettlementQuoteVersionMismatch { event_id }
            | RadrootsOrderIssue::SettlementEconomicsDigestMismatch { event_id }
            | RadrootsOrderIssue::SettlementAmountMismatch { event_id }
            | RadrootsOrderIssue::SettlementCurrencyMismatch { event_id } => {
                event_ids.push(event_id.clone());
            }
            RadrootsOrderIssue::ForkedFulfillments { event_ids: ids } => {
                event_ids.extend(ids.iter().cloned());
            }
        }
    }
    sort_and_dedup_values(&mut event_ids);
    event_ids
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

fn validate_order_fulfillment_record(
    request: &RadrootsOrderRequestRecord,
    fulfillment: &RadrootsOrderFulfillmentRecord,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> bool {
    let mut valid = true;
    if !fulfillment.payload.status.is_publishable_update() {
        issues.push(RadrootsOrderIssue::FulfillmentStatusNotPublishable {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.payload.validate().is_err() {
        issues.push(RadrootsOrderIssue::FulfillmentPayloadInvalid {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.payload.order_id != request.payload.order_id {
        issues.push(RadrootsOrderIssue::FulfillmentOrderIdMismatch {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.author_pubkey != fulfillment.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::FulfillmentAuthorMismatch {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.counterparty_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::FulfillmentCounterpartyMismatch {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.payload.buyer_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::FulfillmentBuyerMismatch {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.payload.seller_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::FulfillmentSellerMismatch {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&fulfillment.payload.listing_addr) {
        Ok(listing_addr) => {
            if fulfillment.payload.listing_addr != request.payload.listing_addr
                || listing_addr.seller_pubkey != fulfillment.payload.seller_pubkey
            {
                issues.push(RadrootsOrderIssue::FulfillmentListingMismatch {
                    event_id: fulfillment.event_id.clone(),
                });
                valid = false;
            }
        }
        Err(_) => {
            issues.push(RadrootsOrderIssue::FulfillmentListingAddressInvalid {
                event_id: fulfillment.event_id.clone(),
            });
            valid = false;
        }
    }
    if fulfillment.root_event_id != request.event_id {
        issues.push(RadrootsOrderIssue::FulfillmentRootMismatch {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.prev_event_id.trim().is_empty()
        || fulfillment.prev_event_id == fulfillment.event_id
    {
        issues.push(RadrootsOrderIssue::FulfillmentPreviousMismatch {
            event_id: fulfillment.event_id.clone(),
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

fn validate_order_receipt_record(
    request: &RadrootsOrderRequestRecord,
    receipt: &RadrootsOrderReceiptRecord,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> bool {
    let mut valid = true;
    if receipt.payload.validate().is_err() {
        issues.push(RadrootsOrderIssue::ReceiptPayloadInvalid {
            event_id: receipt.event_id.clone(),
        });
        valid = false;
    }
    if receipt.payload.order_id != request.payload.order_id {
        issues.push(RadrootsOrderIssue::ReceiptOrderIdMismatch {
            event_id: receipt.event_id.clone(),
        });
        valid = false;
    }
    if receipt.author_pubkey != receipt.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::ReceiptAuthorMismatch {
            event_id: receipt.event_id.clone(),
        });
        valid = false;
    }
    if receipt.counterparty_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::ReceiptCounterpartyMismatch {
            event_id: receipt.event_id.clone(),
        });
        valid = false;
    }
    if receipt.payload.buyer_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::ReceiptBuyerMismatch {
            event_id: receipt.event_id.clone(),
        });
        valid = false;
    }
    if receipt.payload.seller_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::ReceiptSellerMismatch {
            event_id: receipt.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&receipt.payload.listing_addr) {
        Ok(listing_addr) => {
            if receipt.payload.listing_addr != request.payload.listing_addr
                || listing_addr.seller_pubkey != receipt.payload.seller_pubkey
            {
                issues.push(RadrootsOrderIssue::ReceiptListingMismatch {
                    event_id: receipt.event_id.clone(),
                });
                valid = false;
            }
        }
        Err(_) => {
            issues.push(RadrootsOrderIssue::ReceiptListingAddressInvalid {
                event_id: receipt.event_id.clone(),
            });
            valid = false;
        }
    }
    if receipt.root_event_id != request.event_id {
        issues.push(RadrootsOrderIssue::ReceiptRootMismatch {
            event_id: receipt.event_id.clone(),
        });
        valid = false;
    }
    if receipt.prev_event_id.trim().is_empty() || receipt.prev_event_id == receipt.event_id {
        issues.push(RadrootsOrderIssue::ReceiptPreviousMismatch {
            event_id: receipt.event_id.clone(),
        });
        valid = false;
    }
    valid
}

fn reduce_order_payment_settlement_records(
    request: &RadrootsOrderRequestRecord,
    agreement_event_id: &RadrootsEventId,
    economics: &RadrootsOrderEconomics,
    payments: Vec<RadrootsOrderPaymentEventRecord>,
    settlements: Vec<RadrootsOrderSettlementRecord>,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> RadrootsOrderPaymentProjection {
    let mut valid_payments = Vec::new();
    for payment in payments {
        if validate_order_payment_record(request, &payment, issues) {
            valid_payments.push(payment);
        }
    }
    let mut valid_settlements = Vec::new();
    for settlement in settlements {
        if validate_order_settlement_record(request, &settlement, issues) {
            valid_settlements.push(settlement);
        }
    }
    if !issues.is_empty() {
        return RadrootsOrderPaymentProjection::invalid();
    }
    if valid_payments.is_empty() {
        record_settlement_without_valid_payment(&valid_settlements, issues);
        return if issues.is_empty() {
            RadrootsOrderPaymentProjection::not_recorded()
        } else {
            RadrootsOrderPaymentProjection::invalid()
        };
    }

    let mut previous_payment_parent = agreement_event_id.clone();
    let mut used_payment_event_ids = Vec::new();
    let mut used_settlement_event_ids = Vec::new();
    let mut rejected_projection = None;

    loop {
        let payment_candidates = valid_payments
            .iter()
            .filter(|payment| {
                payment.prev_event_id == previous_payment_parent
                    && payment.payload.previous_event_id == previous_payment_parent
                    && !used_payment_event_ids.contains(&payment.event_id)
            })
            .collect::<Vec<_>>();
        if payment_candidates.is_empty() {
            for payment in valid_payments
                .iter()
                .filter(|payment| !used_payment_event_ids.contains(&payment.event_id))
            {
                issues.push(RadrootsOrderIssue::PaymentPreviousMismatch {
                    event_id: payment.event_id.clone(),
                });
            }
            for settlement in valid_settlements
                .iter()
                .filter(|settlement| !used_settlement_event_ids.contains(&settlement.event_id))
            {
                issues.push(RadrootsOrderIssue::SettlementWithoutValidPayment {
                    event_id: settlement.event_id.clone(),
                });
            }
            return if issues.is_empty() {
                rejected_projection.unwrap_or_else(RadrootsOrderPaymentProjection::not_recorded)
            } else {
                RadrootsOrderPaymentProjection::invalid()
            };
        }
        if payment_candidates.len() > 1 {
            let mut event_ids = payment_candidates
                .iter()
                .map(|payment| payment.event_id.clone())
                .collect::<Vec<_>>();
            event_ids.sort();
            issues.push(RadrootsOrderIssue::DuplicatePayments { event_ids });
            return RadrootsOrderPaymentProjection::invalid();
        }
        let payment = payment_candidates[0];
        validate_order_payment_agreement_record(payment, agreement_event_id, economics, issues);
        if !issues.is_empty() {
            return RadrootsOrderPaymentProjection::invalid();
        }
        used_payment_event_ids.push(payment.event_id.clone());

        let settlement_candidates = valid_settlements
            .iter()
            .filter(|settlement| {
                settlement.prev_event_id == payment.event_id
                    && settlement.payload.previous_event_id == payment.event_id
                    && settlement.payload.payment_event_id == payment.event_id
                    && !used_settlement_event_ids.contains(&settlement.event_id)
            })
            .collect::<Vec<_>>();
        if settlement_candidates.is_empty() {
            for settlement in valid_settlements
                .iter()
                .filter(|settlement| !used_settlement_event_ids.contains(&settlement.event_id))
            {
                issues.push(RadrootsOrderIssue::SettlementWithoutValidPayment {
                    event_id: settlement.event_id.clone(),
                });
            }
            return if issues.is_empty() {
                payment_projection_from_record(
                    payment,
                    RadrootsOrderPaymentState::Recorded,
                    RadrootsOrderSettlementState::Pending,
                    None,
                )
            } else {
                RadrootsOrderPaymentProjection::invalid()
            };
        }
        if settlement_candidates.len() > 1 {
            let mut event_ids = settlement_candidates
                .iter()
                .map(|settlement| settlement.event_id.clone())
                .collect::<Vec<_>>();
            event_ids.sort();
            issues.push(RadrootsOrderIssue::DuplicateSettlements { event_ids });
            return RadrootsOrderPaymentProjection::invalid();
        }
        let settlement = settlement_candidates[0];
        validate_order_settlement_payment_record(settlement, payment, issues);
        if !issues.is_empty() {
            return RadrootsOrderPaymentProjection::invalid();
        }
        used_settlement_event_ids.push(settlement.event_id.clone());
        match settlement.payload.decision {
            RadrootsOrderSettlementOutcome::Accepted => {
                for payment in valid_payments
                    .iter()
                    .filter(|payment| !used_payment_event_ids.contains(&payment.event_id))
                {
                    issues.push(RadrootsOrderIssue::PaymentPreviousMismatch {
                        event_id: payment.event_id.clone(),
                    });
                }
                for settlement in valid_settlements
                    .iter()
                    .filter(|settlement| !used_settlement_event_ids.contains(&settlement.event_id))
                {
                    issues.push(RadrootsOrderIssue::SettlementWithoutValidPayment {
                        event_id: settlement.event_id.clone(),
                    });
                }
                return if issues.is_empty() {
                    payment_projection_from_record(
                        payment,
                        RadrootsOrderPaymentState::Settled,
                        RadrootsOrderSettlementState::Accepted,
                        Some(settlement),
                    )
                } else {
                    RadrootsOrderPaymentProjection::invalid()
                };
            }
            RadrootsOrderSettlementOutcome::Rejected => {
                rejected_projection = Some(payment_projection_from_record(
                    payment,
                    RadrootsOrderPaymentState::Rejected,
                    RadrootsOrderSettlementState::Rejected,
                    Some(settlement),
                ));
                previous_payment_parent = settlement.event_id.clone();
            }
        }
    }
}

fn payment_projection_from_record(
    payment: &RadrootsOrderPaymentEventRecord,
    state: RadrootsOrderPaymentState,
    settlement_state: RadrootsOrderSettlementState,
    settlement: Option<&RadrootsOrderSettlementRecord>,
) -> RadrootsOrderPaymentProjection {
    RadrootsOrderPaymentProjection {
        state,
        settlement_state,
        payment_event_id: Some(payment.event_id.clone()),
        settlement_event_id: settlement.map(|settlement| settlement.event_id.clone()),
        agreement_event_id: Some(payment.payload.agreement_event_id.clone()),
        quote_id: Some(payment.payload.quote_id.clone()),
        quote_version: Some(payment.payload.quote_version),
        economics_digest: Some(payment.payload.economics_digest.clone()),
        amount: Some(payment.payload.amount),
        currency: Some(payment.payload.currency),
        method: Some(payment.payload.method),
        reference: payment.payload.reference.clone(),
        paid_at: payment.payload.paid_at,
        reason: settlement.and_then(|settlement| settlement.payload.reason.clone()),
    }
}

fn validate_order_payment_record(
    request: &RadrootsOrderRequestRecord,
    payment: &RadrootsOrderPaymentEventRecord,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> bool {
    let mut valid = true;
    if payment.payload.validate().is_err() {
        issues.push(RadrootsOrderIssue::PaymentPayloadInvalid {
            event_id: payment.event_id.clone(),
        });
        valid = false;
    }
    if payment.payload.order_id != request.payload.order_id {
        issues.push(RadrootsOrderIssue::PaymentOrderIdMismatch {
            event_id: payment.event_id.clone(),
        });
        valid = false;
    }
    if payment.author_pubkey != payment.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::PaymentAuthorMismatch {
            event_id: payment.event_id.clone(),
        });
        valid = false;
    }
    if payment.counterparty_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::PaymentCounterpartyMismatch {
            event_id: payment.event_id.clone(),
        });
        valid = false;
    }
    if payment.payload.buyer_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::PaymentBuyerMismatch {
            event_id: payment.event_id.clone(),
        });
        valid = false;
    }
    if payment.payload.seller_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::PaymentSellerMismatch {
            event_id: payment.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&payment.payload.listing_addr) {
        Ok(listing_addr) => {
            if payment.payload.listing_addr != request.payload.listing_addr
                || listing_addr.seller_pubkey != payment.payload.seller_pubkey
            {
                issues.push(RadrootsOrderIssue::PaymentListingMismatch {
                    event_id: payment.event_id.clone(),
                });
                valid = false;
            }
        }
        Err(_) => {
            issues.push(RadrootsOrderIssue::PaymentListingAddressInvalid {
                event_id: payment.event_id.clone(),
            });
            valid = false;
        }
    }
    if payment.root_event_id != request.event_id
        || payment.payload.root_event_id != request.event_id
    {
        issues.push(RadrootsOrderIssue::PaymentRootMismatch {
            event_id: payment.event_id.clone(),
        });
        valid = false;
    }
    if payment.prev_event_id.trim().is_empty()
        || payment.prev_event_id == payment.event_id
        || payment.payload.previous_event_id != payment.prev_event_id
    {
        issues.push(RadrootsOrderIssue::PaymentPreviousMismatch {
            event_id: payment.event_id.clone(),
        });
        valid = false;
    }
    valid
}

fn validate_order_payment_agreement_record(
    payment: &RadrootsOrderPaymentEventRecord,
    agreement_event_id: &RadrootsEventId,
    economics: &RadrootsOrderEconomics,
    issues: &mut Vec<RadrootsOrderIssue>,
) {
    if payment.payload.agreement_event_id.as_str() != agreement_event_id.as_str() {
        issues.push(RadrootsOrderIssue::PaymentAgreementMismatch {
            event_id: payment.event_id.clone(),
        });
    }
    if payment.payload.quote_id != economics.quote_id {
        issues.push(RadrootsOrderIssue::PaymentQuoteMismatch {
            event_id: payment.event_id.clone(),
        });
    }
    if payment.payload.quote_version != economics.quote_version {
        issues.push(RadrootsOrderIssue::PaymentQuoteVersionMismatch {
            event_id: payment.event_id.clone(),
        });
    }
    if payment.payload.amount != economics.total.amount {
        issues.push(RadrootsOrderIssue::PaymentAmountMismatch {
            event_id: payment.event_id.clone(),
        });
    }
    if payment.payload.currency != economics.total.currency
        || payment.payload.currency != economics.currency
    {
        issues.push(RadrootsOrderIssue::PaymentCurrencyMismatch {
            event_id: payment.event_id.clone(),
        });
    }
    #[cfg(feature = "serde_json")]
    match radroots_order_economics_digest(economics) {
        Ok(expected_digest) if payment.payload.economics_digest != expected_digest => {
            issues.push(RadrootsOrderIssue::PaymentEconomicsDigestMismatch {
                event_id: payment.event_id.clone(),
            });
        }
        Ok(_) => {}
        Err(_) => {
            issues.push(RadrootsOrderIssue::PaymentEconomicsDigestMismatch {
                event_id: payment.event_id.clone(),
            });
        }
    }
}

fn validate_order_settlement_record(
    request: &RadrootsOrderRequestRecord,
    settlement: &RadrootsOrderSettlementRecord,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> bool {
    let mut valid = true;
    if settlement.payload.validate().is_err() {
        issues.push(RadrootsOrderIssue::SettlementPayloadInvalid {
            event_id: settlement.event_id.clone(),
        });
        valid = false;
    }
    if settlement.payload.order_id != request.payload.order_id {
        issues.push(RadrootsOrderIssue::SettlementOrderIdMismatch {
            event_id: settlement.event_id.clone(),
        });
        valid = false;
    }
    if settlement.author_pubkey != settlement.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::SettlementAuthorMismatch {
            event_id: settlement.event_id.clone(),
        });
        valid = false;
    }
    if settlement.counterparty_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::SettlementCounterpartyMismatch {
            event_id: settlement.event_id.clone(),
        });
        valid = false;
    }
    if settlement.payload.buyer_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsOrderIssue::SettlementBuyerMismatch {
            event_id: settlement.event_id.clone(),
        });
        valid = false;
    }
    if settlement.payload.seller_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsOrderIssue::SettlementSellerMismatch {
            event_id: settlement.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&settlement.payload.listing_addr) {
        Ok(listing_addr) => {
            if settlement.payload.listing_addr != request.payload.listing_addr
                || listing_addr.seller_pubkey != settlement.payload.seller_pubkey
            {
                issues.push(RadrootsOrderIssue::SettlementListingMismatch {
                    event_id: settlement.event_id.clone(),
                });
                valid = false;
            }
        }
        Err(_) => {
            issues.push(RadrootsOrderIssue::SettlementListingAddressInvalid {
                event_id: settlement.event_id.clone(),
            });
            valid = false;
        }
    }
    if settlement.root_event_id != request.event_id
        || settlement.payload.root_event_id != request.event_id
    {
        issues.push(RadrootsOrderIssue::SettlementRootMismatch {
            event_id: settlement.event_id.clone(),
        });
        valid = false;
    }
    if settlement.prev_event_id.trim().is_empty()
        || settlement.prev_event_id == settlement.event_id
        || settlement.payload.previous_event_id != settlement.prev_event_id
    {
        issues.push(RadrootsOrderIssue::SettlementPreviousMismatch {
            event_id: settlement.event_id.clone(),
        });
        valid = false;
    }
    valid
}

fn validate_order_settlement_payment_record(
    settlement: &RadrootsOrderSettlementRecord,
    payment: &RadrootsOrderPaymentEventRecord,
    issues: &mut Vec<RadrootsOrderIssue>,
) {
    if settlement.payload.payment_event_id != payment.event_id {
        issues.push(RadrootsOrderIssue::SettlementPaymentEventMismatch {
            event_id: settlement.event_id.clone(),
        });
    }
    if settlement.payload.agreement_event_id != payment.payload.agreement_event_id {
        issues.push(RadrootsOrderIssue::SettlementAgreementMismatch {
            event_id: settlement.event_id.clone(),
        });
    }
    if settlement.payload.quote_id != payment.payload.quote_id {
        issues.push(RadrootsOrderIssue::SettlementQuoteMismatch {
            event_id: settlement.event_id.clone(),
        });
    }
    if settlement.payload.quote_version != payment.payload.quote_version {
        issues.push(RadrootsOrderIssue::SettlementQuoteVersionMismatch {
            event_id: settlement.event_id.clone(),
        });
    }
    if settlement.payload.economics_digest != payment.payload.economics_digest {
        issues.push(RadrootsOrderIssue::SettlementEconomicsDigestMismatch {
            event_id: settlement.event_id.clone(),
        });
    }
    if settlement.payload.amount != payment.payload.amount {
        issues.push(RadrootsOrderIssue::SettlementAmountMismatch {
            event_id: settlement.event_id.clone(),
        });
    }
    if settlement.payload.currency != payment.payload.currency {
        issues.push(RadrootsOrderIssue::SettlementCurrencyMismatch {
            event_id: settlement.event_id.clone(),
        });
    }
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

fn record_fulfillment_without_accepted_decision(
    fulfillments: &[RadrootsOrderFulfillmentRecord],
    issues: &mut Vec<RadrootsOrderIssue>,
) {
    for fulfillment in fulfillments {
        issues.push(RadrootsOrderIssue::FulfillmentWithoutAcceptedDecision {
            event_id: fulfillment.event_id.clone(),
        });
    }
}

fn record_revision_proposal_without_accepted_decision(
    revision_proposals: &[RadrootsOrderRevisionProposalRecord],
    issues: &mut Vec<RadrootsOrderIssue>,
) {
    for proposal in revision_proposals {
        issues.push(
            RadrootsOrderIssue::RevisionProposalWithoutAcceptedDecision {
                event_id: proposal.event_id.clone(),
            },
        );
    }
}

fn record_revision_decision_without_proposal(
    revision_decisions: &[RadrootsOrderRevisionDecisionRecord],
    issues: &mut Vec<RadrootsOrderIssue>,
) {
    for decision in revision_decisions {
        issues.push(RadrootsOrderIssue::RevisionDecisionWithoutProposal {
            event_id: decision.event_id.clone(),
        });
    }
}

fn record_cancellation_without_cancellable_order(
    cancellations: &[RadrootsOrderCancellationRecord],
    issues: &mut Vec<RadrootsOrderIssue>,
) {
    for cancellation in cancellations {
        issues.push(RadrootsOrderIssue::CancellationWithoutCancellableOrder {
            event_id: cancellation.event_id.clone(),
        });
    }
}

fn record_receipt_without_eligible_fulfillment(
    receipts: &[RadrootsOrderReceiptRecord],
    issues: &mut Vec<RadrootsOrderIssue>,
) {
    for receipt in receipts {
        issues.push(RadrootsOrderIssue::ReceiptWithoutEligibleFulfillment {
            event_id: receipt.event_id.clone(),
        });
    }
}

fn record_payment_without_accepted_agreement(
    payments: &[RadrootsOrderPaymentEventRecord],
    issues: &mut Vec<RadrootsOrderIssue>,
) {
    for payment in payments {
        issues.push(RadrootsOrderIssue::PaymentWithoutAcceptedAgreement {
            event_id: payment.event_id.clone(),
        });
    }
}

fn record_settlement_without_valid_payment(
    settlements: &[RadrootsOrderSettlementRecord],
    issues: &mut Vec<RadrootsOrderIssue>,
) {
    for settlement in settlements {
        issues.push(RadrootsOrderIssue::SettlementWithoutValidPayment {
            event_id: settlement.event_id.clone(),
        });
    }
}

fn record_payment_after_cancellation(
    payments: &[RadrootsOrderPaymentEventRecord],
    issues: &mut Vec<RadrootsOrderIssue>,
) {
    for payment in payments {
        issues.push(RadrootsOrderIssue::PaymentAfterCancellation {
            event_id: payment.event_id.clone(),
        });
    }
}

fn single_lifecycle_child<T>(
    records: &[T],
    event_id: impl Fn(&T) -> &RadrootsEventId,
) -> Result<Option<T>, RadrootsOrderIssue>
where
    T: Clone,
{
    match records {
        [] => Ok(None),
        [record] => Ok(Some(record.clone())),
        _ => {
            let mut event_ids = records.iter().map(event_id).cloned().collect::<Vec<_>>();
            sort_and_dedup_values(&mut event_ids);
            Err(RadrootsOrderIssue::ForkedLifecycle { event_ids })
        }
    }
}

fn validated_fulfillment_records(
    request: &RadrootsOrderRequestRecord,
    fulfillments: Vec<RadrootsOrderFulfillmentRecord>,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> Vec<RadrootsOrderFulfillmentRecord> {
    let mut valid_fulfillments = Vec::new();
    for fulfillment in fulfillments {
        if validate_order_fulfillment_record(request, &fulfillment, issues) {
            valid_fulfillments.push(fulfillment);
        }
    }
    valid_fulfillments
}

struct RadrootsOrderRevisionState {
    agreement_event_id: RadrootsEventId,
    lifecycle_parent_event_id: RadrootsEventId,
    economics: RadrootsOrderEconomics,
    pending_revision_event_id: Option<RadrootsEventId>,
}

fn order_revision_state(
    request: &RadrootsOrderRequestRecord,
    decision: &RadrootsOrderDecisionRecord,
    revision_proposals: &[RadrootsOrderRevisionProposalRecord],
    revision_decisions: &[RadrootsOrderRevisionDecisionRecord],
    issues: &mut Vec<RadrootsOrderIssue>,
) -> Option<RadrootsOrderRevisionState> {
    let mut state = RadrootsOrderRevisionState {
        agreement_event_id: decision.event_id.clone(),
        lifecycle_parent_event_id: decision.event_id.clone(),
        economics: request.payload.economics.clone(),
        pending_revision_event_id: None,
    };
    let mut used_proposal_event_ids = Vec::new();
    let mut used_decision_event_ids = Vec::new();

    loop {
        let matching_proposals = revision_proposals
            .iter()
            .filter(|proposal| {
                proposal.prev_event_id == state.lifecycle_parent_event_id
                    && !used_proposal_event_ids.contains(&proposal.event_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let proposal = match single_lifecycle_child(&matching_proposals, |record| &record.event_id)
        {
            Ok(Some(proposal)) => proposal,
            Ok(None) => break,
            Err(issue) => {
                issues.push(issue);
                return None;
            }
        };
        used_proposal_event_ids.push(proposal.event_id.clone());
        let matching_decisions = revision_decisions
            .iter()
            .filter(|decision| {
                decision.prev_event_id == proposal.event_id
                    && !used_decision_event_ids.contains(&decision.event_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let revision_decision =
            match single_lifecycle_child(&matching_decisions, |record| &record.event_id) {
                Ok(Some(decision)) => decision,
                Ok(None) => {
                    state.pending_revision_event_id = Some(proposal.event_id.clone());
                    state.lifecycle_parent_event_id = proposal.event_id;
                    break;
                }
                Err(issue) => {
                    issues.push(issue);
                    return None;
                }
            };
        if revision_decision.payload.revision_id != proposal.payload.revision_id {
            issues.push(RadrootsOrderIssue::RevisionDecisionRevisionIdMismatch {
                event_id: revision_decision.event_id.clone(),
            });
            return None;
        }
        used_decision_event_ids.push(revision_decision.event_id.clone());
        match revision_decision.payload.decision {
            RadrootsOrderRevisionOutcome::Accepted => {
                state.agreement_event_id = revision_decision.event_id.clone();
                state.economics = proposal.payload.economics;
            }
            RadrootsOrderRevisionOutcome::Declined { .. } => {}
        }
        state.lifecycle_parent_event_id = revision_decision.event_id;
    }

    for proposal in revision_proposals {
        if !used_proposal_event_ids.contains(&proposal.event_id) {
            issues.push(RadrootsOrderIssue::RevisionProposalPreviousMismatch {
                event_id: proposal.event_id.clone(),
            });
        }
    }
    for decision in revision_decisions {
        if used_decision_event_ids.contains(&decision.event_id) {
            continue;
        }
        if let Some(proposal) = revision_proposals
            .iter()
            .find(|proposal| proposal.event_id == decision.prev_event_id)
        {
            if proposal.payload.revision_id != decision.payload.revision_id {
                issues.push(RadrootsOrderIssue::RevisionDecisionRevisionIdMismatch {
                    event_id: decision.event_id.clone(),
                });
            } else {
                issues.push(RadrootsOrderIssue::RevisionDecisionPreviousMismatch {
                    event_id: decision.event_id.clone(),
                });
            }
        } else {
            issues.push(RadrootsOrderIssue::RevisionDecisionWithoutProposal {
                event_id: decision.event_id.clone(),
            });
        }
    }

    if issues.is_empty() { Some(state) } else { None }
}

fn latest_fulfillment_record(
    parent_event_id: &RadrootsEventId,
    valid_fulfillments: &[RadrootsOrderFulfillmentRecord],
    issues: &mut Vec<RadrootsOrderIssue>,
) -> Option<RadrootsOrderFulfillmentRecord> {
    if !issues.is_empty() {
        return None;
    }
    let mut used_event_ids = Vec::new();
    let mut previous_event_id = parent_event_id.clone();
    let mut previous_status = RadrootsOrderFulfillmentState::AcceptedNotFulfilled;
    let mut latest = None;

    loop {
        let mut children = valid_fulfillments
            .iter()
            .filter(|fulfillment| {
                fulfillment.prev_event_id == previous_event_id
                    && !used_event_ids.contains(&fulfillment.event_id)
            })
            .collect::<Vec<_>>();
        if children.is_empty() {
            break;
        }
        children.sort_by(|left, right| left.event_id.cmp(&right.event_id));
        if children.len() > 1 {
            let mut event_ids = children
                .iter()
                .map(|fulfillment| fulfillment.event_id.clone())
                .collect::<Vec<_>>();
            event_ids.sort();
            issues.push(RadrootsOrderIssue::ForkedFulfillments { event_ids });
            return None;
        }
        let child = children[0];
        if matches!(
            previous_status,
            RadrootsOrderFulfillmentState::Delivered
                | RadrootsOrderFulfillmentState::SellerCancelled
        ) {
            issues.push(RadrootsOrderIssue::FulfillmentUnsupportedTransition {
                event_id: child.event_id.clone(),
            });
            return None;
        }
        used_event_ids.push(child.event_id.clone());
        previous_event_id = child.event_id.clone();
        previous_status = child.payload.status;
        latest = Some((*child).clone());
    }

    for fulfillment in valid_fulfillments {
        if !used_event_ids.contains(&fulfillment.event_id) {
            issues.push(RadrootsOrderIssue::FulfillmentPreviousMismatch {
                event_id: fulfillment.event_id.clone(),
            });
        }
    }
    latest
}

fn requested_projection(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
) -> RadrootsOrderProjection {
    RadrootsOrderProjection {
        order_id: order_id.clone(),
        status: RadrootsOrderStatus::Requested,
        request_event_id: Some(request.event_id.clone()),
        decision_event_id: None,
        fulfillment_event_id: None,
        fulfillment_status: None,
        cancellation_event_id: None,
        receipt_event_id: None,
        receipt_received: None,
        receipt_issue: None,
        receipt_received_at: None,
        lifecycle_terminal: false,
        payment: RadrootsOrderPaymentProjection::not_recorded(),
        economics: Some(request.payload.economics.clone()),
        agreement_event_id: None,
        listing_addr: Some(request.payload.listing_addr.clone()),
        buyer_pubkey: Some(request.payload.buyer_pubkey.clone()),
        seller_pubkey: Some(request.payload.seller_pubkey.clone()),
        last_event_id: Some(request.event_id.clone()),
        issues: Vec::new(),
    }
}

fn requested_cancellation_projection(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    cancellations: Vec<RadrootsOrderCancellationRecord>,
) -> RadrootsOrderProjection {
    let mut issues = Vec::new();
    for cancellation in cancellations
        .iter()
        .filter(|cancellation| cancellation.prev_event_id != request.event_id)
    {
        issues.push(RadrootsOrderIssue::CancellationPreviousMismatch {
            event_id: cancellation.event_id.clone(),
        });
    }
    if !issues.is_empty() {
        return invalid_projection(order_id, Some(request), issues);
    }
    let matching = cancellations
        .into_iter()
        .filter(|cancellation| cancellation.prev_event_id == request.event_id)
        .collect::<Vec<_>>();
    match single_lifecycle_child(&matching, |record| &record.event_id) {
        Ok(Some(cancellation)) => cancelled_projection(
            order_id,
            request,
            None,
            None,
            request.payload.economics.clone(),
            cancellation,
        ),
        Ok(None) => requested_projection(order_id, request),
        Err(issue) => invalid_projection(order_id, Some(request), vec![issue]),
    }
}

fn decided_projection(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    decision: &RadrootsOrderDecisionRecord,
    records: RadrootsOrderDecisionProjectionRecords,
) -> RadrootsOrderProjection {
    let RadrootsOrderDecisionProjectionRecords {
        revision_proposals,
        revision_decisions,
        fulfillments,
        cancellations,
        receipts,
        payments,
        settlements,
    } = records;
    let status = match &decision.payload.decision {
        RadrootsOrderDecisionOutcome::Accepted { .. } => RadrootsOrderStatus::Accepted,
        RadrootsOrderDecisionOutcome::Declined { .. } => RadrootsOrderStatus::Declined,
    };
    let mut issues = Vec::new();
    let (
        fulfillment_event_id,
        fulfillment_status,
        last_event_id,
        agreement_event_id,
        economics,
        payment,
    ) = match status {
        RadrootsOrderStatus::Accepted => {
            let Some(revision_state) = order_revision_state(
                request,
                decision,
                &revision_proposals,
                &revision_decisions,
                &mut issues,
            ) else {
                return invalid_projection(order_id, Some(request), issues);
            };
            if let Some(pending_revision_event_id) =
                revision_state.pending_revision_event_id.as_ref()
                && (!fulfillments.is_empty()
                    || !cancellations.is_empty()
                    || !receipts.is_empty()
                    || !payments.is_empty()
                    || !settlements.is_empty())
            {
                let mut event_ids = vec![pending_revision_event_id.clone()];
                event_ids.extend(
                    fulfillments
                        .iter()
                        .map(|fulfillment| fulfillment.event_id.clone()),
                );
                event_ids.extend(
                    cancellations
                        .iter()
                        .map(|cancellation| cancellation.event_id.clone()),
                );
                event_ids.extend(receipts.iter().map(|receipt| receipt.event_id.clone()));
                event_ids.extend(payments.iter().map(|payment| payment.event_id.clone()));
                event_ids.extend(
                    settlements
                        .iter()
                        .map(|settlement| settlement.event_id.clone()),
                );
                sort_and_dedup_values(&mut event_ids);
                return invalid_projection(
                    order_id,
                    Some(request),
                    vec![RadrootsOrderIssue::ForkedLifecycle { event_ids }],
                );
            }
            let fulfillment_records =
                validated_fulfillment_records(request, fulfillments, &mut issues);
            let latest = latest_fulfillment_record(
                &revision_state.lifecycle_parent_event_id,
                &fulfillment_records,
                &mut issues,
            );
            if !issues.is_empty() {
                return invalid_projection(order_id, Some(request), issues);
            }
            let decision_cancellations = cancellations
                .iter()
                .filter(|cancellation| {
                    cancellation.prev_event_id == revision_state.lifecycle_parent_event_id
                })
                .cloned()
                .collect::<Vec<_>>();
            for cancellation in cancellations.iter().filter(|cancellation| {
                cancellation.prev_event_id != revision_state.lifecycle_parent_event_id
            }) {
                issues.push(RadrootsOrderIssue::CancellationPreviousMismatch {
                    event_id: cancellation.event_id.clone(),
                });
            }
            if !issues.is_empty() {
                return invalid_projection(order_id, Some(request), issues);
            }
            if !decision_cancellations.is_empty() {
                record_payment_after_cancellation(&payments, &mut issues);
                record_settlement_without_valid_payment(&settlements, &mut issues);
                if !issues.is_empty() {
                    return invalid_projection_with_payment(
                        order_id,
                        Some(request),
                        issues,
                        RadrootsOrderPaymentProjection::invalid(),
                    );
                }
            }
            if let Some(first_fulfillment) = fulfillment_records.iter().find(|fulfillment| {
                fulfillment.prev_event_id == revision_state.lifecycle_parent_event_id
            }) && !decision_cancellations.is_empty()
            {
                let mut event_ids = decision_cancellations
                    .iter()
                    .map(|cancellation| cancellation.event_id.clone())
                    .collect::<Vec<_>>();
                event_ids.push(first_fulfillment.event_id.clone());
                sort_and_dedup_values(&mut event_ids);
                return invalid_projection(
                    order_id,
                    Some(request),
                    vec![RadrootsOrderIssue::ForkedLifecycle { event_ids }],
                );
            }
            if latest.is_some() {
                for cancellation in decision_cancellations {
                    issues.push(RadrootsOrderIssue::CancellationAfterFulfillment {
                        event_id: cancellation.event_id,
                    });
                }
                if !issues.is_empty() {
                    return invalid_projection(order_id, Some(request), issues);
                }
            } else {
                match single_lifecycle_child(&decision_cancellations, |record| &record.event_id) {
                    Ok(Some(cancellation)) => {
                        return cancelled_projection(
                            order_id,
                            request,
                            Some(decision.event_id.clone()),
                            Some(revision_state.agreement_event_id.clone()),
                            revision_state.economics.clone(),
                            cancellation,
                        );
                    }
                    Ok(None) => {}
                    Err(issue) => {
                        return invalid_projection(order_id, Some(request), vec![issue]);
                    }
                }
            }
            let payment = reduce_order_payment_settlement_records(
                request,
                &revision_state.agreement_event_id,
                &revision_state.economics,
                payments,
                settlements,
                &mut issues,
            );
            if !issues.is_empty() {
                return invalid_projection_with_payment(
                    order_id,
                    Some(request),
                    issues,
                    RadrootsOrderPaymentProjection::invalid(),
                );
            }
            let receipt_result = receipt_projection(RadrootsReceiptProjectionInput {
                order_id,
                request,
                decision,
                agreement_event_id: &revision_state.agreement_event_id,
                economics: &revision_state.economics,
                latest_fulfillment: latest.as_ref(),
                fulfillments: &fulfillment_records,
                receipts,
                issues: &mut issues,
            });
            if let Some(mut projection) = receipt_result {
                projection.payment = payment;
                return projection;
            }
            if !issues.is_empty() {
                return invalid_projection(order_id, Some(request), issues);
            }
            let (fulfillment_event_id, fulfillment_status, last_event_id) = match latest {
                Some(fulfillment) => (
                    Some(fulfillment.event_id.clone()),
                    Some(fulfillment.payload.status),
                    Some(fulfillment.event_id),
                ),
                None => (
                    None,
                    Some(RadrootsOrderFulfillmentState::AcceptedNotFulfilled),
                    Some(revision_state.lifecycle_parent_event_id.clone()),
                ),
            };
            let mut projection_payment = payment;
            if projection_payment.state == RadrootsOrderPaymentState::NotRecorded {
                projection_payment.settlement_state = RadrootsOrderSettlementState::NotRequired;
            }
            (
                fulfillment_event_id,
                fulfillment_status,
                last_event_id,
                Some(revision_state.agreement_event_id),
                Some(revision_state.economics),
                projection_payment,
            )
        }
        RadrootsOrderStatus::Declined => {
            record_revision_proposal_without_accepted_decision(&revision_proposals, &mut issues);
            record_revision_decision_without_proposal(&revision_decisions, &mut issues);
            record_payment_without_accepted_agreement(&payments, &mut issues);
            record_settlement_without_valid_payment(&settlements, &mut issues);
            if fulfillments.is_empty()
                && cancellations.is_empty()
                && receipts.is_empty()
                && payments.is_empty()
                && settlements.is_empty()
                && issues.is_empty()
            {
                (
                    None,
                    None,
                    Some(decision.event_id.clone()),
                    None,
                    None,
                    RadrootsOrderPaymentProjection::not_recorded(),
                )
            } else {
                record_fulfillment_without_accepted_decision(&fulfillments, &mut issues);
                record_cancellation_without_cancellable_order(&cancellations, &mut issues);
                record_receipt_without_eligible_fulfillment(&receipts, &mut issues);
                return invalid_projection_with_payment(
                    order_id,
                    Some(request),
                    issues,
                    RadrootsOrderPaymentProjection::invalid(),
                );
            }
        }
        _ => (
            None,
            None,
            Some(decision.event_id.clone()),
            None,
            None,
            RadrootsOrderPaymentProjection::not_recorded(),
        ),
    };
    RadrootsOrderProjection {
        order_id: order_id.clone(),
        status,
        request_event_id: Some(request.event_id.clone()),
        decision_event_id: Some(decision.event_id.clone()),
        fulfillment_event_id,
        fulfillment_status,
        cancellation_event_id: None,
        receipt_event_id: None,
        receipt_received: None,
        receipt_issue: None,
        receipt_received_at: None,
        lifecycle_terminal: false,
        payment,
        economics,
        agreement_event_id,
        listing_addr: Some(request.payload.listing_addr.clone()),
        buyer_pubkey: Some(request.payload.buyer_pubkey.clone()),
        seller_pubkey: Some(request.payload.seller_pubkey.clone()),
        last_event_id,
        issues: Vec::new(),
    }
}

fn receipt_projection(
    input: RadrootsReceiptProjectionInput<'_>,
) -> Option<RadrootsOrderProjection> {
    let RadrootsReceiptProjectionInput {
        order_id,
        request,
        decision,
        agreement_event_id,
        economics,
        latest_fulfillment,
        fulfillments,
        receipts,
        issues,
    } = input;
    if receipts.is_empty() {
        return None;
    }
    let Some(fulfillment) = latest_fulfillment else {
        record_receipt_without_eligible_fulfillment(&receipts, issues);
        return None;
    };
    if !matches!(
        fulfillment.payload.status,
        RadrootsOrderFulfillmentState::ReadyForPickup | RadrootsOrderFulfillmentState::Delivered
    ) {
        record_receipt_without_eligible_fulfillment(&receipts, issues);
        return None;
    }
    let mut fork_event_ids = Vec::new();
    for receipt in &receipts {
        let Some(receipt_parent) = fulfillments
            .iter()
            .find(|candidate| candidate.event_id == receipt.prev_event_id)
        else {
            continue;
        };
        if !matches!(
            receipt_parent.payload.status,
            RadrootsOrderFulfillmentState::ReadyForPickup
                | RadrootsOrderFulfillmentState::Delivered
        ) {
            continue;
        }
        let sibling_fulfillment_event_ids = fulfillments
            .iter()
            .filter(|candidate| candidate.prev_event_id == receipt.prev_event_id)
            .map(|candidate| candidate.event_id.clone())
            .collect::<Vec<_>>();
        if !sibling_fulfillment_event_ids.is_empty() {
            fork_event_ids.push(receipt.event_id.clone());
            fork_event_ids.extend(sibling_fulfillment_event_ids);
        }
    }
    if !fork_event_ids.is_empty() {
        sort_and_dedup_values(&mut fork_event_ids);
        issues.push(RadrootsOrderIssue::ForkedLifecycle {
            event_ids: fork_event_ids,
        });
        return None;
    }
    let matching = receipts
        .iter()
        .filter(|receipt| receipt.prev_event_id == fulfillment.event_id)
        .cloned()
        .collect::<Vec<_>>();
    match single_lifecycle_child(&matching, |record| &record.event_id) {
        Ok(Some(receipt)) => Some(receipt_terminal_projection(
            order_id,
            request,
            decision,
            agreement_event_id,
            economics,
            fulfillment,
            receipt,
        )),
        Ok(None) => {
            for receipt in receipts {
                issues.push(RadrootsOrderIssue::ReceiptPreviousMismatch {
                    event_id: receipt.event_id,
                });
            }
            None
        }
        Err(issue) => {
            issues.push(issue);
            None
        }
    }
}

fn cancelled_projection(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    decision_event_id: Option<RadrootsEventId>,
    agreement_event_id: Option<RadrootsEventId>,
    economics: RadrootsOrderEconomics,
    cancellation: RadrootsOrderCancellationRecord,
) -> RadrootsOrderProjection {
    RadrootsOrderProjection {
        order_id: order_id.clone(),
        status: RadrootsOrderStatus::Cancelled,
        request_event_id: Some(request.event_id.clone()),
        decision_event_id,
        fulfillment_event_id: None,
        fulfillment_status: None,
        cancellation_event_id: Some(cancellation.event_id.clone()),
        receipt_event_id: None,
        receipt_received: None,
        receipt_issue: None,
        receipt_received_at: None,
        lifecycle_terminal: true,
        payment: RadrootsOrderPaymentProjection::not_recorded(),
        economics: Some(economics),
        agreement_event_id,
        listing_addr: Some(request.payload.listing_addr.clone()),
        buyer_pubkey: Some(request.payload.buyer_pubkey.clone()),
        seller_pubkey: Some(request.payload.seller_pubkey.clone()),
        last_event_id: Some(cancellation.event_id),
        issues: Vec::new(),
    }
}

fn receipt_terminal_projection(
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    decision: &RadrootsOrderDecisionRecord,
    agreement_event_id: &RadrootsEventId,
    economics: &RadrootsOrderEconomics,
    fulfillment: &RadrootsOrderFulfillmentRecord,
    receipt: RadrootsOrderReceiptRecord,
) -> RadrootsOrderProjection {
    let status = if receipt.payload.received {
        RadrootsOrderStatus::Completed
    } else {
        RadrootsOrderStatus::Disputed
    };
    RadrootsOrderProjection {
        order_id: order_id.clone(),
        status,
        request_event_id: Some(request.event_id.clone()),
        decision_event_id: Some(decision.event_id.clone()),
        fulfillment_event_id: Some(fulfillment.event_id.clone()),
        fulfillment_status: Some(fulfillment.payload.status),
        cancellation_event_id: None,
        receipt_event_id: Some(receipt.event_id.clone()),
        receipt_received: Some(receipt.payload.received),
        receipt_issue: receipt.payload.issue.clone(),
        receipt_received_at: Some(receipt.payload.received_at),
        lifecycle_terminal: true,
        payment: RadrootsOrderPaymentProjection::not_recorded(),
        economics: Some(economics.clone()),
        agreement_event_id: Some(agreement_event_id.clone()),
        listing_addr: Some(request.payload.listing_addr.clone()),
        buyer_pubkey: Some(request.payload.buyer_pubkey.clone()),
        seller_pubkey: Some(request.payload.seller_pubkey.clone()),
        last_event_id: Some(receipt.event_id),
        issues: Vec::new(),
    }
}

fn invalid_projection(
    order_id: &RadrootsOrderId,
    request: Option<&RadrootsOrderRequestRecord>,
    issues: Vec<RadrootsOrderIssue>,
) -> RadrootsOrderProjection {
    invalid_projection_with_payment(
        order_id,
        request,
        issues,
        RadrootsOrderPaymentProjection::not_recorded(),
    )
}

fn invalid_projection_with_payment(
    order_id: &RadrootsOrderId,
    request: Option<&RadrootsOrderRequestRecord>,
    issues: Vec<RadrootsOrderIssue>,
    payment: RadrootsOrderPaymentProjection,
) -> RadrootsOrderProjection {
    let economics = match request {
        Some(request) if request.payload.validate().is_ok() => {
            Some(request.payload.economics.clone())
        }
        _ => None,
    };
    RadrootsOrderProjection {
        order_id: order_id.clone(),
        status: RadrootsOrderStatus::Invalid,
        request_event_id: request.map(|request| request.event_id.clone()),
        decision_event_id: None,
        fulfillment_event_id: None,
        fulfillment_status: None,
        cancellation_event_id: None,
        receipt_event_id: None,
        receipt_received: None,
        receipt_issue: None,
        receipt_received_at: None,
        lifecycle_terminal: true,
        payment,
        economics,
        agreement_event_id: None,
        listing_addr: request.map(|request| request.payload.listing_addr.clone()),
        buyer_pubkey: request.map(|request| request.payload.buyer_pubkey.clone()),
        seller_pubkey: request.map(|request| request.payload.seller_pubkey.clone()),
        last_event_id: request.map(|request| request.event_id.clone()),
        issues,
    }
}

fn parse_public_listing_addr(
    listing_addr_raw: &str,
) -> Result<RadrootsPublicListingAddress, RadrootsOrderCanonicalizationError> {
    parse_public_listing_address(listing_addr_raw).map_err(|error| match error {
        RadrootsPublicListingAddressError::InvalidAddress(error) => {
            RadrootsOrderCanonicalizationError::InvalidListingAddress(error.to_string())
        }
        RadrootsPublicListingAddressError::InvalidListingKind { .. }
        | RadrootsPublicListingAddressError::InvalidKind { .. } => {
            RadrootsOrderCanonicalizationError::InvalidListingKind
        }
    })
}

fn canonicalize_items(
    items: &mut Vec<RadrootsOrderItem>,
) -> Result<(), RadrootsOrderCanonicalizationError> {
    if items.is_empty() {
        return Err(RadrootsOrderCanonicalizationError::MissingItems);
    }
    let mut canonical_items: Vec<RadrootsOrderItem> = Vec::new();
    for (index, item) in items.iter_mut().enumerate() {
        if item.bin_count == 0 {
            return Err(RadrootsOrderCanonicalizationError::InvalidBinCount { index });
        }
        if let Some(existing) = canonical_items
            .iter_mut()
            .find(|canonical| canonical.bin_id.as_str() == item.bin_id.as_str())
        {
            existing.bin_count = existing
                .bin_count
                .checked_add(item.bin_count)
                .ok_or(RadrootsOrderCanonicalizationError::InvalidBinCount { index })?;
        } else {
            canonical_items.push(RadrootsOrderItem {
                bin_id: item.bin_id.clone(),
                bin_count: item.bin_count,
            });
        }
    }
    canonical_items.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
    *items = canonical_items;
    Ok(())
}

fn canonicalize_decision(
    decision: &mut RadrootsOrderDecisionOutcome,
) -> Result<(), RadrootsOrderCanonicalizationError> {
    match decision {
        RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } => canonicalize_inventory_commitments(inventory_commitments),
        RadrootsOrderDecisionOutcome::Declined { reason } => {
            *reason = normalized_required_string(core::mem::take(reason), "reason")?;
            Ok(())
        }
    }
}

fn canonicalize_inventory_commitments(
    commitments: &mut [RadrootsOrderInventoryCommitment],
) -> Result<(), RadrootsOrderCanonicalizationError> {
    if commitments.is_empty() {
        return Err(RadrootsOrderCanonicalizationError::MissingInventoryCommitments);
    }
    for (index, commitment) in commitments.iter_mut().enumerate() {
        if commitment.bin_count == 0 {
            return Err(
                RadrootsOrderCanonicalizationError::InvalidInventoryCommitmentCount { index },
            );
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct NormalizedInventoryCount {
    bin_id: RadrootsInventoryBinId,
    bin_count: u64,
}

fn inventory_commitments_match_request(
    request_items: &[RadrootsOrderItem],
    inventory_commitments: &[RadrootsOrderInventoryCommitment],
) -> bool {
    normalized_request_item_counts(request_items)
        == normalized_inventory_commitment_counts(inventory_commitments)
}

fn normalized_request_item_counts(
    items: &[RadrootsOrderItem],
) -> Option<Vec<NormalizedInventoryCount>> {
    let mut counts = Vec::new();
    for item in items {
        push_normalized_inventory_count(&mut counts, &item.bin_id, item.bin_count)?;
    }
    counts.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
    Some(counts)
}

fn normalized_inventory_commitment_counts(
    commitments: &[RadrootsOrderInventoryCommitment],
) -> Option<Vec<NormalizedInventoryCount>> {
    let mut counts = Vec::new();
    for commitment in commitments {
        push_normalized_inventory_count(&mut counts, &commitment.bin_id, commitment.bin_count)?;
    }
    counts.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
    Some(counts)
}

fn push_normalized_inventory_count(
    counts: &mut Vec<NormalizedInventoryCount>,
    bin_id: &RadrootsInventoryBinId,
    bin_count: u32,
) -> Option<()> {
    if bin_count == 0 {
        return None;
    }
    if let Some(existing) = counts
        .iter_mut()
        .find(|count| count.bin_id.as_str() == bin_id.as_str())
    {
        existing.bin_count = existing.bin_count.checked_add(u64::from(bin_count))?;
    } else {
        counts.push(NormalizedInventoryCount {
            bin_id: bin_id.clone(),
            bin_count: u64::from(bin_count),
        });
    }
    Some(())
}

fn normalized_required_string(
    value: String,
    field: &'static str,
) -> Result<String, RadrootsOrderCanonicalizationError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(RadrootsOrderCanonicalizationError::EmptyField(field));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use core::fmt::Write as _;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
    };
    #[cfg(feature = "event_store")]
    use radroots_event_store::{RadrootsEventIngest, RadrootsEventStore};
    use radroots_events::ids::{
        RadrootsEconomicsDigest, RadrootsEventId, RadrootsInventoryBinId, RadrootsListingAddress,
        RadrootsOrderId, RadrootsOrderQuoteId, RadrootsOrderRevisionId, RadrootsPublicKey,
    };
    use radroots_events::kinds::{KIND_LISTING, KIND_LISTING_DRAFT};
    use radroots_events::order::{
        RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
        RadrootsOrderEconomicItem, RadrootsOrderEconomicLine, RadrootsOrderEconomics,
        RadrootsOrderFulfillmentState, RadrootsOrderFulfillmentUpdate,
        RadrootsOrderInventoryCommitment, RadrootsOrderItem, RadrootsOrderPaymentMethod,
        RadrootsOrderPaymentRecord as RadrootsOrderPaymentPayload, RadrootsOrderPricingBasis,
        RadrootsOrderReceipt, RadrootsOrderRequest, RadrootsOrderRevisionDecision,
        RadrootsOrderRevisionOutcome, RadrootsOrderRevisionProposal,
        RadrootsOrderSettlementDecision, RadrootsOrderSettlementOutcome,
    };
    use radroots_events::tags::{TAG_E_PREV, TAG_E_ROOT};
    use radroots_events::{RadrootsNostrEvent, RadrootsNostrEventPtr};
    use radroots_events_codec::order::{
        RadrootsOrderEnvelopeParseError, order_cancellation_event_build,
        order_decision_event_build, order_fulfillment_update_event_build,
        order_payment_record_event_build, order_receipt_event_build, order_request_event_build,
        order_revision_decision_event_build, order_revision_proposal_event_build,
        order_settlement_decision_event_build,
    };
    use radroots_events_codec::wire::WireEventParts;
    #[cfg(feature = "event_store")]
    use radroots_nostr::prelude::{
        RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp,
        radroots_event_from_nostr, radroots_nostr_build_event,
    };

    #[cfg(feature = "event_store")]
    use super::{
        ORDER_EVENT_CONTRACT_IDS, RadrootsOrderStoreQueryError, order_events_for_order_id,
        order_projection_for_order_id, order_projection_query_for_order_id,
    };
    use super::{
        RadrootsListingInventoryAccountingInputs, RadrootsListingInventoryAccountingIssue,
        RadrootsListingInventoryAccountingProjection, RadrootsListingInventoryBinAccounting,
        RadrootsListingInventoryBinAvailability, RadrootsListingInventoryOrderReservation,
        RadrootsOrderCancellationRecord, RadrootsOrderCanonicalizationError,
        RadrootsOrderDecisionRecord, RadrootsOrderEventDecodeError, RadrootsOrderEventRecord,
        RadrootsOrderFulfillmentRecord, RadrootsOrderIssue, RadrootsOrderPaymentEventRecord,
        RadrootsOrderPaymentProjection, RadrootsOrderPaymentState, RadrootsOrderProjection,
        RadrootsOrderReceiptRecord, RadrootsOrderReductionInputs, RadrootsOrderRequestRecord,
        RadrootsOrderRevisionDecisionRecord, RadrootsOrderRevisionProposalRecord,
        RadrootsOrderSettlementRecord, RadrootsOrderSettlementState, RadrootsOrderStatus,
        add_inventory_reservation, canonicalize_order_decision_for_signer,
        canonicalize_order_request_for_signer, inventory_issue_event_ids, inventory_issue_id,
        inventory_issue_rank, inventory_issue_sort_key, order_event_record_from_event,
        projection_issue_event_ids, radroots_order_economics_digest,
        reduce_listing_inventory_accounting as reduce_listing_inventory_accounting_with_revisions_inner,
        reduce_order_event_records,
        reduce_order_events as reduce_order_events_with_revisions_inner,
    };

    const SELLER: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const BUYER: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    #[cfg(feature = "event_store")]
    const STORE_BUYER_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    #[cfg(feature = "event_store")]
    const STORE_BUYER_PUBLIC_KEY_HEX: &str =
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    #[cfg(feature = "event_store")]
    const STORE_SELLER_SECRET_KEY_HEX: &str =
        "59392e9068f66431b12f70218fb61281cb6b433d7f27c55d61f1a63fe1a96ff8";
    #[cfg(feature = "event_store")]
    const STORE_SELLER_PUBLIC_KEY_HEX: &str =
        "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    fn order_id(raw: &str) -> RadrootsOrderId {
        RadrootsOrderId::parse(raw).expect("order id")
    }

    fn pubkey(raw: &str) -> RadrootsPublicKey {
        RadrootsPublicKey::parse(raw).expect("public key")
    }

    fn pubkey_or(default: &str, raw: &str) -> RadrootsPublicKey {
        if raw.is_empty() {
            pubkey(default)
        } else {
            pubkey(raw)
        }
    }

    fn test_event_id(raw: &str) -> RadrootsEventId {
        let mut bytes = [0u8; 32];
        for (index, byte) in raw.bytes().enumerate() {
            let primary = index % bytes.len();
            let secondary = (index * 7 + 13) % bytes.len();
            bytes[primary] = bytes[primary]
                .wrapping_add(byte)
                .wrapping_add((index as u8).wrapping_mul(31));
            bytes[secondary] ^= byte.rotate_left((index % 8) as u32);
        }
        let mut hex = String::with_capacity(64);
        for byte in bytes {
            write!(&mut hex, "{byte:02x}").unwrap();
        }
        RadrootsEventId::parse(hex).expect("event id")
    }

    fn order_revision_id(raw: &str) -> RadrootsOrderRevisionId {
        RadrootsOrderRevisionId::parse(raw).expect("revision id")
    }

    fn order_quote_id(raw: &str) -> RadrootsOrderQuoteId {
        RadrootsOrderQuoteId::parse(raw).expect("quote id")
    }

    fn bin_id(raw: &str) -> RadrootsInventoryBinId {
        RadrootsInventoryBinId::parse(raw).expect("bin id")
    }

    fn listing_address() -> RadrootsListingAddress {
        RadrootsListingAddress::parse(listing_addr()).expect("listing address")
    }

    fn economics_digest(raw: impl AsRef<str>) -> RadrootsEconomicsDigest {
        RadrootsEconomicsDigest::parse(raw.as_ref()).expect("economics digest")
    }

    fn sample_order_request(buyer_pubkey: &str, seller_pubkey: &str) -> RadrootsOrderRequest {
        RadrootsOrderRequest {
            order_id: order_id("order-1"),
            listing_addr: listing_address(),
            buyer_pubkey: pubkey_or(BUYER, buyer_pubkey),
            seller_pubkey: pubkey_or(SELLER, seller_pubkey),
            items: vec![RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            }],
            economics: request_economics("bin-1", 2, "10"),
        }
    }

    fn decimal(raw: &str) -> RadrootsCoreDecimal {
        raw.parse().unwrap()
    }

    fn usd(raw: &str) -> RadrootsCoreMoney {
        RadrootsCoreMoney::new(decimal(raw), RadrootsCoreCurrency::USD)
    }

    fn request_economics(
        raw_bin_id: &str,
        bin_count: u32,
        subtotal: &str,
    ) -> RadrootsOrderEconomics {
        RadrootsOrderEconomics {
            quote_id: order_quote_id("quote-1"),
            quote_version: 1,
            pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
            currency: RadrootsCoreCurrency::USD,
            items: vec![RadrootsOrderEconomicItem {
                bin_id: bin_id(raw_bin_id),
                bin_count,
                quantity_amount: decimal("1"),
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: decimal("5"),
                unit_price_currency: RadrootsCoreCurrency::USD,
                line_subtotal: usd(subtotal),
            }],
            discounts: Vec::<RadrootsOrderEconomicLine>::new(),
            adjustments: Vec::<RadrootsOrderEconomicLine>::new(),
            subtotal: usd(subtotal),
            discount_total: usd("0"),
            adjustment_total: usd("0"),
            total: usd(subtotal),
        }
    }

    fn sample_order_decision(seller_pubkey: &str) -> RadrootsOrderDecision {
        RadrootsOrderDecision {
            order_id: order_id("order-1"),
            listing_addr: listing_address(),
            buyer_pubkey: pubkey(BUYER),
            seller_pubkey: pubkey_or(SELLER, seller_pubkey),
            decision: RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                    bin_id: bin_id("bin-1"),
                    bin_count: 2,
                }],
            },
        }
    }

    fn listing_addr() -> String {
        format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
    }

    fn listing_event_ptr() -> RadrootsNostrEventPtr {
        RadrootsNostrEventPtr {
            id: test_event_id("listing-event").into_string(),
            relays: Some("wss://relay.radroots.test".to_string()),
        }
    }

    #[cfg(feature = "event_store")]
    fn store_fixture_keys(secret_key_hex: &str) -> RadrootsNostrKeys {
        let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    #[cfg(feature = "event_store")]
    fn signed_store_event_from_parts(
        secret_key_hex: &str,
        created_at: u32,
        parts: WireEventParts,
    ) -> RadrootsNostrEvent {
        let event = radroots_nostr_build_event(parts.kind, parts.content, parts.tags)
            .expect("event builder")
            .custom_created_at(RadrootsNostrTimestamp::from_secs(u64::from(created_at)))
            .sign_with_keys(&store_fixture_keys(secret_key_hex))
            .expect("signed event");
        radroots_event_from_nostr(&event)
    }

    #[cfg(feature = "event_store")]
    fn signed_store_event(
        secret_key_hex: &str,
        kind: u32,
        created_at: u32,
        tags: Vec<Vec<String>>,
        content: impl Into<String>,
    ) -> RadrootsNostrEvent {
        let event = radroots_nostr_build_event(kind, content, tags)
            .expect("event builder")
            .custom_created_at(RadrootsNostrTimestamp::from_secs(u64::from(created_at)))
            .sign_with_keys(&store_fixture_keys(secret_key_hex))
            .expect("signed event");
        radroots_event_from_nostr(&event)
    }

    #[cfg(feature = "event_store")]
    fn tamper_store_event_signature(event: &mut RadrootsNostrEvent) {
        let replacement = if event.sig.starts_with('0') { "1" } else { "0" };
        event.sig.replace_range(0..1, replacement);
    }

    #[cfg(feature = "event_store")]
    fn store_listing_address() -> RadrootsListingAddress {
        RadrootsListingAddress::parse(format!(
            "{KIND_LISTING}:{STORE_SELLER_PUBLIC_KEY_HEX}:AAAAAAAAAAAAAAAAAAAAAg"
        ))
        .expect("store listing address")
    }

    #[cfg(feature = "event_store")]
    fn store_listing_event_ptr() -> RadrootsNostrEventPtr {
        RadrootsNostrEventPtr {
            id: test_event_id("store-listing-event").into_string(),
            relays: Some("wss://relay.radroots.test".to_string()),
        }
    }

    #[cfg(feature = "event_store")]
    fn store_order_request(raw_order_id: &str) -> RadrootsOrderRequest {
        RadrootsOrderRequest {
            order_id: order_id(raw_order_id),
            listing_addr: store_listing_address(),
            buyer_pubkey: pubkey(STORE_BUYER_PUBLIC_KEY_HEX),
            seller_pubkey: pubkey(STORE_SELLER_PUBLIC_KEY_HEX),
            items: vec![RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            }],
            economics: request_economics("bin-1", 2, "10"),
        }
    }

    #[cfg(feature = "event_store")]
    fn store_order_decision(raw_order_id: &str) -> RadrootsOrderDecision {
        RadrootsOrderDecision {
            order_id: order_id(raw_order_id),
            listing_addr: store_listing_address(),
            buyer_pubkey: pubkey(STORE_BUYER_PUBLIC_KEY_HEX),
            seller_pubkey: pubkey(STORE_SELLER_PUBLIC_KEY_HEX),
            decision: RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                    bin_id: bin_id("bin-1"),
                    bin_count: 2,
                }],
            },
        }
    }

    #[cfg(feature = "event_store")]
    fn store_order_request_event(raw_order_id: &str, created_at: u32) -> RadrootsNostrEvent {
        let request = store_order_request(raw_order_id);
        signed_store_event_from_parts(
            STORE_BUYER_SECRET_KEY_HEX,
            created_at,
            order_request_event_build(&store_listing_event_ptr(), &request).unwrap(),
        )
    }

    #[cfg(feature = "event_store")]
    fn store_order_decision_event(
        raw_order_id: &str,
        root_event_id: &RadrootsEventId,
        created_at: u32,
    ) -> RadrootsNostrEvent {
        let decision = store_order_decision(raw_order_id);
        signed_store_event_from_parts(
            STORE_SELLER_SECRET_KEY_HEX,
            created_at,
            order_decision_event_build(root_event_id, root_event_id, &decision).unwrap(),
        )
    }

    fn event_from_parts(
        event_id: &RadrootsEventId,
        author_pubkey: &RadrootsPublicKey,
        parts: WireEventParts,
    ) -> RadrootsNostrEvent {
        RadrootsNostrEvent {
            id: event_id.clone().into_string(),
            author: author_pubkey.clone().into_string(),
            created_at: 1,
            kind: parts.kind,
            tags: parts.tags,
            content: parts.content,
            sig: "sig".to_string(),
        }
    }

    fn clean_request_payload() -> RadrootsOrderRequest {
        RadrootsOrderRequest {
            order_id: order_id("order-1"),
            listing_addr: listing_address(),
            buyer_pubkey: pubkey(BUYER),
            seller_pubkey: pubkey(SELLER),
            items: vec![RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            }],
            economics: request_economics("bin-1", 2, "10"),
        }
    }

    fn request_record_with_event_id(event_id: &str) -> RadrootsOrderRequestRecord {
        RadrootsOrderRequestRecord {
            event_id: test_event_id(event_id),
            author_pubkey: pubkey(BUYER),
            payload: clean_request_payload(),
        }
    }

    fn request_record() -> RadrootsOrderRequestRecord {
        request_record_with_event_id("request-1")
    }

    fn request_record_for(
        raw_order_id: &str,
        event_id: &str,
        bin_count: u32,
    ) -> RadrootsOrderRequestRecord {
        let mut request = request_record_with_event_id(event_id);
        request.payload.order_id = order_id(raw_order_id);
        request.payload.items[0].bin_count = bin_count;
        let subtotal =
            (RadrootsCoreDecimal::from(5u32) * RadrootsCoreDecimal::from(bin_count)).to_string();
        request.payload.economics = request_economics("bin-1", bin_count, &subtotal);
        request
    }

    fn decision_payload(decision: RadrootsOrderDecisionOutcome) -> RadrootsOrderDecision {
        RadrootsOrderDecision {
            order_id: order_id("order-1"),
            listing_addr: listing_address(),
            buyer_pubkey: pubkey(BUYER),
            seller_pubkey: pubkey(SELLER),
            decision,
        }
    }

    fn accepted_decision_record(event_id: &str) -> RadrootsOrderDecisionRecord {
        RadrootsOrderDecisionRecord {
            event_id: test_event_id(event_id),
            author_pubkey: pubkey(SELLER),
            counterparty_pubkey: pubkey(BUYER),
            root_event_id: test_event_id("request-1"),
            prev_event_id: test_event_id("request-1"),
            payload: decision_payload(RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                    bin_id: bin_id("bin-1"),
                    bin_count: 2,
                }],
            }),
        }
    }

    fn declined_decision_record(event_id: &str) -> RadrootsOrderDecisionRecord {
        RadrootsOrderDecisionRecord {
            event_id: test_event_id(event_id),
            author_pubkey: pubkey(SELLER),
            counterparty_pubkey: pubkey(BUYER),
            root_event_id: test_event_id("request-1"),
            prev_event_id: test_event_id("request-1"),
            payload: decision_payload(RadrootsOrderDecisionOutcome::Declined {
                reason: "out_of_stock".to_string(),
            }),
        }
    }

    fn fulfillment_record(
        event_id: &str,
        prev_event_id: &str,
        status: RadrootsOrderFulfillmentState,
    ) -> RadrootsOrderFulfillmentRecord {
        RadrootsOrderFulfillmentRecord {
            event_id: test_event_id(event_id),
            author_pubkey: pubkey(SELLER),
            counterparty_pubkey: pubkey(BUYER),
            root_event_id: test_event_id("request-1"),
            prev_event_id: test_event_id(prev_event_id),
            payload: RadrootsOrderFulfillmentUpdate {
                order_id: order_id("order-1"),
                listing_addr: listing_address(),
                buyer_pubkey: pubkey(BUYER),
                seller_pubkey: pubkey(SELLER),
                status,
            },
        }
    }

    fn cancellation_record(event_id: &str, prev_event_id: &str) -> RadrootsOrderCancellationRecord {
        RadrootsOrderCancellationRecord {
            event_id: test_event_id(event_id),
            author_pubkey: pubkey(BUYER),
            counterparty_pubkey: pubkey(SELLER),
            root_event_id: test_event_id("request-1"),
            prev_event_id: test_event_id(prev_event_id),
            payload: RadrootsOrderCancellation {
                order_id: order_id("order-1"),
                listing_addr: listing_address(),
                buyer_pubkey: pubkey(BUYER),
                seller_pubkey: pubkey(SELLER),
                reason: "changed plans".to_string(),
            },
        }
    }

    fn receipt_record(
        event_id: &str,
        prev_event_id: &str,
        received: bool,
    ) -> RadrootsOrderReceiptRecord {
        RadrootsOrderReceiptRecord {
            event_id: test_event_id(event_id),
            author_pubkey: pubkey(BUYER),
            counterparty_pubkey: pubkey(SELLER),
            root_event_id: test_event_id("request-1"),
            prev_event_id: test_event_id(prev_event_id),
            payload: RadrootsOrderReceipt {
                order_id: order_id("order-1"),
                listing_addr: listing_address(),
                buyer_pubkey: pubkey(BUYER),
                seller_pubkey: pubkey(SELLER),
                received,
                issue: (!received).then(|| "damaged items".to_string()),
                received_at: 1_777_665_600,
            },
        }
    }

    fn payment_record(event_id: &str, prev_event_id: &str) -> RadrootsOrderPaymentEventRecord {
        let economics = request_economics("bin-1", 2, "10");
        RadrootsOrderPaymentEventRecord {
            event_id: test_event_id(event_id),
            author_pubkey: pubkey(BUYER),
            counterparty_pubkey: pubkey(SELLER),
            root_event_id: test_event_id("request-1"),
            prev_event_id: test_event_id(prev_event_id),
            payload: RadrootsOrderPaymentPayload {
                order_id: order_id("order-1"),
                listing_addr: listing_address(),
                buyer_pubkey: pubkey(BUYER),
                seller_pubkey: pubkey(SELLER),
                root_event_id: test_event_id("request-1"),
                previous_event_id: test_event_id(prev_event_id),
                agreement_event_id: test_event_id("decision-1"),
                quote_id: economics.quote_id.clone(),
                quote_version: economics.quote_version,
                economics_digest: economics_digest(
                    radroots_order_economics_digest(&economics).unwrap(),
                ),
                amount: economics.total.amount,
                currency: economics.total.currency,
                method: RadrootsOrderPaymentMethod::ManualTransfer,
                reference: Some("manual reference".to_string()),
                paid_at: Some(1_777_666_000),
            },
        }
    }

    fn settlement_record(
        event_id: &str,
        payment_event_id: &str,
        decision: RadrootsOrderSettlementOutcome,
    ) -> RadrootsOrderSettlementRecord {
        let payment = payment_record(payment_event_id, "decision-1");
        RadrootsOrderSettlementRecord {
            event_id: test_event_id(event_id),
            author_pubkey: pubkey(SELLER),
            counterparty_pubkey: pubkey(BUYER),
            root_event_id: test_event_id("request-1"),
            prev_event_id: test_event_id(payment_event_id),
            payload: RadrootsOrderSettlementDecision {
                order_id: payment.payload.order_id,
                listing_addr: payment.payload.listing_addr,
                seller_pubkey: payment.payload.seller_pubkey,
                buyer_pubkey: payment.payload.buyer_pubkey,
                root_event_id: payment.payload.root_event_id.clone(),
                previous_event_id: test_event_id(payment_event_id),
                agreement_event_id: payment.payload.agreement_event_id,
                payment_event_id: test_event_id(payment_event_id),
                quote_id: payment.payload.quote_id,
                quote_version: payment.payload.quote_version,
                economics_digest: payment.payload.economics_digest,
                amount: payment.payload.amount,
                currency: payment.payload.currency,
                decision,
                reason: (decision == RadrootsOrderSettlementOutcome::Rejected)
                    .then(|| "reference mismatch".to_string()),
            },
        }
    }

    fn accepted_decision_record_for(
        raw_order_id: &str,
        event_id: &str,
        request_event_id: &str,
        bin_count: u32,
    ) -> RadrootsOrderDecisionRecord {
        let mut decision = accepted_decision_record(event_id);
        decision.root_event_id = test_event_id(request_event_id);
        decision.prev_event_id = test_event_id(request_event_id);
        decision.payload.order_id = order_id(raw_order_id);
        let RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } = &mut decision.payload.decision
        else {
            panic!("expected accepted decision")
        };
        inventory_commitments[0].bin_count = bin_count;
        decision
    }

    fn inventory_bin(available_count: u64) -> RadrootsListingInventoryBinAvailability {
        RadrootsListingInventoryBinAvailability {
            bin_id: bin_id("bin-1"),
            available_count,
        }
    }

    fn revision_proposal_record(
        event_id: &str,
        prev_event_id: &str,
        revision_id: &str,
        bin_count: u32,
    ) -> RadrootsOrderRevisionProposalRecord {
        let subtotal =
            (RadrootsCoreDecimal::from(5u32) * RadrootsCoreDecimal::from(bin_count)).to_string();
        RadrootsOrderRevisionProposalRecord {
            event_id: test_event_id(event_id),
            author_pubkey: pubkey(SELLER),
            counterparty_pubkey: pubkey(BUYER),
            root_event_id: test_event_id("request-1"),
            prev_event_id: test_event_id(prev_event_id),
            payload: RadrootsOrderRevisionProposal {
                revision_id: order_revision_id(revision_id),
                order_id: order_id("order-1"),
                listing_addr: listing_address(),
                buyer_pubkey: pubkey(BUYER),
                seller_pubkey: pubkey(SELLER),
                root_event_id: test_event_id("request-1"),
                prev_event_id: test_event_id(prev_event_id),
                items: vec![RadrootsOrderItem {
                    bin_id: bin_id("bin-1"),
                    bin_count,
                }],
                economics: request_economics("bin-1", bin_count, &subtotal),
                reason: "field yield changed".to_string(),
            },
        }
    }

    fn revision_decision_record(
        event_id: &str,
        prev_event_id: &str,
        revision_id: &str,
        decision: RadrootsOrderRevisionOutcome,
    ) -> RadrootsOrderRevisionDecisionRecord {
        RadrootsOrderRevisionDecisionRecord {
            event_id: test_event_id(event_id),
            author_pubkey: pubkey(BUYER),
            counterparty_pubkey: pubkey(SELLER),
            root_event_id: test_event_id("request-1"),
            prev_event_id: test_event_id(prev_event_id),
            payload: RadrootsOrderRevisionDecision {
                revision_id: order_revision_id(revision_id),
                order_id: order_id("order-1"),
                listing_addr: listing_address(),
                buyer_pubkey: pubkey(BUYER),
                seller_pubkey: pubkey(SELLER),
                root_event_id: test_event_id("request-1"),
                prev_event_id: test_event_id(prev_event_id),
                decision,
            },
        }
    }

    #[test]
    fn order_event_record_accessors_cover_all_variants() {
        let records = vec![
            (
                RadrootsOrderEventRecord::Request(request_record_with_event_id("record-request")),
                "record-request",
            ),
            (
                RadrootsOrderEventRecord::Decision(accepted_decision_record("record-decision")),
                "record-decision",
            ),
            (
                RadrootsOrderEventRecord::RevisionProposal(revision_proposal_record(
                    "record-revision-proposal",
                    "decision-1",
                    "revision-1",
                    3,
                )),
                "record-revision-proposal",
            ),
            (
                RadrootsOrderEventRecord::RevisionDecision(revision_decision_record(
                    "record-revision-decision",
                    "record-revision-proposal",
                    "revision-1",
                    RadrootsOrderRevisionOutcome::Accepted,
                )),
                "record-revision-decision",
            ),
            (
                RadrootsOrderEventRecord::Fulfillment(fulfillment_record(
                    "record-fulfillment",
                    "decision-1",
                    RadrootsOrderFulfillmentState::ReadyForPickup,
                )),
                "record-fulfillment",
            ),
            (
                RadrootsOrderEventRecord::Cancellation(cancellation_record(
                    "record-cancellation",
                    "request-1",
                )),
                "record-cancellation",
            ),
            (
                RadrootsOrderEventRecord::Receipt(receipt_record(
                    "record-receipt",
                    "record-fulfillment",
                    true,
                )),
                "record-receipt",
            ),
            (
                RadrootsOrderEventRecord::Payment(payment_record("record-payment", "decision-1")),
                "record-payment",
            ),
            (
                RadrootsOrderEventRecord::Settlement(settlement_record(
                    "record-settlement",
                    "record-payment",
                    RadrootsOrderSettlementOutcome::Accepted,
                )),
                "record-settlement",
            ),
        ];

        for (record, event_id_raw) in records {
            let expected_event_id = test_event_id(event_id_raw);
            assert_eq!(record.event_id(), &expected_event_id);
            assert_eq!(record.order_id(), &order_id("order-1"));
        }
    }

    #[test]
    fn order_event_record_from_event_decodes_all_variants() {
        let request = request_record_with_event_id("decode-request");
        let request_event = event_from_parts(
            &request.event_id,
            &request.author_pubkey,
            order_request_event_build(&listing_event_ptr(), &request.payload).unwrap(),
        );
        assert_eq!(
            order_event_record_from_event(&request_event).unwrap(),
            RadrootsOrderEventRecord::Request(request)
        );

        let decision = accepted_decision_record("decode-decision");
        let decision_event = event_from_parts(
            &decision.event_id,
            &decision.author_pubkey,
            order_decision_event_build(
                &decision.root_event_id,
                &decision.prev_event_id,
                &decision.payload,
            )
            .unwrap(),
        );
        assert_eq!(
            order_event_record_from_event(&decision_event).unwrap(),
            RadrootsOrderEventRecord::Decision(decision)
        );

        let proposal = revision_proposal_record(
            "decode-revision-proposal",
            "decode-decision",
            "revision-1",
            3,
        );
        let proposal_event = event_from_parts(
            &proposal.event_id,
            &proposal.author_pubkey,
            order_revision_proposal_event_build(
                &proposal.root_event_id,
                &proposal.prev_event_id,
                &proposal.payload,
            )
            .unwrap(),
        );
        assert_eq!(
            order_event_record_from_event(&proposal_event).unwrap(),
            RadrootsOrderEventRecord::RevisionProposal(proposal)
        );

        let revision_decision = revision_decision_record(
            "decode-revision-decision",
            "decode-revision-proposal",
            "revision-1",
            RadrootsOrderRevisionOutcome::Accepted,
        );
        let revision_decision_event = event_from_parts(
            &revision_decision.event_id,
            &revision_decision.author_pubkey,
            order_revision_decision_event_build(
                &revision_decision.root_event_id,
                &revision_decision.prev_event_id,
                &revision_decision.payload,
            )
            .unwrap(),
        );
        assert_eq!(
            order_event_record_from_event(&revision_decision_event).unwrap(),
            RadrootsOrderEventRecord::RevisionDecision(revision_decision)
        );

        let fulfillment = fulfillment_record(
            "decode-fulfillment",
            "decode-revision-decision",
            RadrootsOrderFulfillmentState::ReadyForPickup,
        );
        let fulfillment_event = event_from_parts(
            &fulfillment.event_id,
            &fulfillment.author_pubkey,
            order_fulfillment_update_event_build(
                &fulfillment.root_event_id,
                &fulfillment.prev_event_id,
                &fulfillment.payload,
            )
            .unwrap(),
        );
        assert_eq!(
            order_event_record_from_event(&fulfillment_event).unwrap(),
            RadrootsOrderEventRecord::Fulfillment(fulfillment)
        );

        let cancellation = cancellation_record("decode-cancellation", "decode-request");
        let cancellation_event = event_from_parts(
            &cancellation.event_id,
            &cancellation.author_pubkey,
            order_cancellation_event_build(
                &cancellation.root_event_id,
                &cancellation.prev_event_id,
                &cancellation.payload,
            )
            .unwrap(),
        );
        assert_eq!(
            order_event_record_from_event(&cancellation_event).unwrap(),
            RadrootsOrderEventRecord::Cancellation(cancellation)
        );

        let receipt = receipt_record("decode-receipt", "decode-fulfillment", true);
        let receipt_event = event_from_parts(
            &receipt.event_id,
            &receipt.author_pubkey,
            order_receipt_event_build(
                &receipt.root_event_id,
                &receipt.prev_event_id,
                &receipt.payload,
            )
            .unwrap(),
        );
        assert_eq!(
            order_event_record_from_event(&receipt_event).unwrap(),
            RadrootsOrderEventRecord::Receipt(receipt)
        );

        let payment = payment_record("decode-payment", "decode-decision");
        let payment_event = event_from_parts(
            &payment.event_id,
            &payment.author_pubkey,
            order_payment_record_event_build(
                &payment.root_event_id,
                &payment.prev_event_id,
                &payment.payload,
            )
            .unwrap(),
        );
        assert_eq!(
            order_event_record_from_event(&payment_event).unwrap(),
            RadrootsOrderEventRecord::Payment(payment)
        );

        let settlement = settlement_record(
            "decode-settlement",
            "decode-payment",
            RadrootsOrderSettlementOutcome::Accepted,
        );
        let settlement_event = event_from_parts(
            &settlement.event_id,
            &settlement.author_pubkey,
            order_settlement_decision_event_build(
                &settlement.root_event_id,
                &settlement.prev_event_id,
                &settlement.payload,
            )
            .unwrap(),
        );
        assert_eq!(
            order_event_record_from_event(&settlement_event).unwrap(),
            RadrootsOrderEventRecord::Settlement(settlement)
        );
    }

    #[test]
    fn order_event_record_from_event_rejects_wrong_kind() {
        let request = request_record_with_event_id("decode-wrong-kind");
        let mut event = event_from_parts(
            &request.event_id,
            &request.author_pubkey,
            order_request_event_build(&listing_event_ptr(), &request.payload).unwrap(),
        );
        event.kind = KIND_LISTING;
        assert!(matches!(
            order_event_record_from_event(&event),
            Err(RadrootsOrderEventDecodeError::UnsupportedKind { kind: KIND_LISTING })
        ));
    }

    #[test]
    fn order_event_record_from_event_rejects_wrong_envelope_type() {
        let request = request_record_with_event_id("decode-request-content");
        let request_parts =
            order_request_event_build(&listing_event_ptr(), &request.payload).unwrap();
        let decision = accepted_decision_record("decode-wrong-envelope");
        let mut event = event_from_parts(
            &decision.event_id,
            &decision.author_pubkey,
            order_decision_event_build(
                &decision.root_event_id,
                &decision.prev_event_id,
                &decision.payload,
            )
            .unwrap(),
        );
        event.content = request_parts.content;
        assert!(matches!(
            order_event_record_from_event(&event),
            Err(RadrootsOrderEventDecodeError::Envelope(_))
        ));
    }

    #[test]
    fn order_event_record_from_event_rejects_root_and_previous_mismatches() {
        let proposal =
            revision_proposal_record("decode-chain-mismatch", "decode-decision", "revision-1", 3);
        let parts = order_revision_proposal_event_build(
            &proposal.root_event_id,
            &proposal.prev_event_id,
            &proposal.payload,
        )
        .unwrap();
        let mut root_event = event_from_parts(&proposal.event_id, &proposal.author_pubkey, parts);
        root_event
            .tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(TAG_E_ROOT))
            .unwrap()[1] = test_event_id("wrong-root").into_string();
        assert!(matches!(
            order_event_record_from_event(&root_event),
            Err(RadrootsOrderEventDecodeError::Envelope(
                RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("root_event_id")
            ))
        ));

        let parts = order_revision_proposal_event_build(
            &proposal.root_event_id,
            &proposal.prev_event_id,
            &proposal.payload,
        )
        .unwrap();
        let mut prev_event = event_from_parts(&proposal.event_id, &proposal.author_pubkey, parts);
        prev_event
            .tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some(TAG_E_PREV))
            .unwrap()[1] = test_event_id("wrong-prev").into_string();
        assert!(matches!(
            order_event_record_from_event(&prev_event),
            Err(RadrootsOrderEventDecodeError::Envelope(
                RadrootsOrderEnvelopeParseError::PayloadBindingMismatch("prev_event_id")
            ))
        ));
    }

    #[test]
    fn order_event_record_from_event_rejects_counterparty_mismatch() {
        let decision = accepted_decision_record("decode-counterparty-mismatch");
        let mut event = event_from_parts(
            &decision.event_id,
            &decision.author_pubkey,
            order_decision_event_build(
                &decision.root_event_id,
                &decision.prev_event_id,
                &decision.payload,
            )
            .unwrap(),
        );
        event
            .tags
            .iter_mut()
            .find(|tag| tag.first().map(String::as_str) == Some("p"))
            .unwrap()[1] = SELLER.to_string();
        assert!(matches!(
            order_event_record_from_event(&event),
            Err(RadrootsOrderEventDecodeError::Envelope(
                RadrootsOrderEnvelopeParseError::CounterpartyTagMismatch
            ))
        ));
    }

    #[test]
    fn reduce_order_event_records_dedupes_duplicate_event_ids() {
        let request = request_record_with_event_id("unified-duplicate");
        let mut duplicate = request.clone();
        duplicate.payload.items[0].bin_count = 4;

        let projection = reduce_order_event_records(
            &order_id("order-1"),
            [
                RadrootsOrderEventRecord::Request(request.clone()),
                RadrootsOrderEventRecord::Request(duplicate),
            ],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Requested);
        assert_eq!(projection.request_event_id, Some(request.event_id));
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_order_event_records_is_stable_for_shuffled_input() {
        let request = request_record();
        let decision = accepted_decision_record("decision-1");
        let fulfillment = fulfillment_record(
            "fulfillment-1",
            "decision-1",
            RadrootsOrderFulfillmentState::ReadyForPickup,
        );
        let receipt = receipt_record("receipt-1", "fulfillment-1", true);

        let grouped = reduce_order_events(
            "order-1",
            [request.clone()],
            [decision.clone()],
            [fulfillment.clone()],
            [],
            [receipt.clone()],
        );
        let unified = reduce_order_event_records(
            &order_id("order-1"),
            [
                RadrootsOrderEventRecord::Receipt(receipt),
                RadrootsOrderEventRecord::Fulfillment(fulfillment),
                RadrootsOrderEventRecord::Decision(decision),
                RadrootsOrderEventRecord::Request(request),
            ],
        );

        assert_eq!(unified, grouped);
    }

    #[test]
    fn reduce_order_event_records_reports_missing_for_empty_stream() {
        let projection = reduce_order_event_records(
            &order_id("order-1"),
            Vec::<RadrootsOrderEventRecord>::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Missing);
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_order_event_records_reports_multiple_requests_deterministically() {
        let first = request_record_with_event_id("request-a");
        let second = request_record_with_event_id("request-b");
        let projection = reduce_order_event_records(
            &order_id("order-1"),
            [
                RadrootsOrderEventRecord::Request(second.clone()),
                RadrootsOrderEventRecord::Request(first.clone()),
            ],
        );
        let reversed = reduce_order_event_records(
            &order_id("order-1"),
            [
                RadrootsOrderEventRecord::Request(first),
                RadrootsOrderEventRecord::Request(second),
            ],
        );

        assert_eq!(projection, reversed);
        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(
            projection
                .issues
                .iter()
                .any(|issue| matches!(issue, RadrootsOrderIssue::MultipleRequests { .. }))
        );
    }

    #[cfg(feature = "event_store")]
    #[tokio::test]
    async fn order_events_for_order_id_queries_d_tag_and_filters_store_rows() {
        let store = RadrootsEventStore::open_memory().await.expect("store");
        let request_event = store_order_request_event("order-1", 10);
        let request_event_id = RadrootsEventId::parse(&request_event.id).expect("request id");
        let decision_event = store_order_decision_event("order-1", &request_event_id, 11);
        let wrong_order_event = store_order_request_event("order-2", 12);
        let wrong_contract_event = signed_store_event(
            STORE_BUYER_SECRET_KEY_HEX,
            KIND_LISTING,
            13,
            vec![vec!["d".to_string(), "order-1".to_string()]],
            "{}",
        );
        let mut unprojected_order_event = store_order_request_event("order-1", 14);
        tamper_store_event_signature(&mut unprojected_order_event);

        for (event, observed_at_ms) in [
            (wrong_contract_event, 1_000),
            (request_event.clone(), 1_100),
            (wrong_order_event, 1_200),
            (unprojected_order_event, 1_300),
            (decision_event.clone(), 1_400),
        ] {
            store
                .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
                .await
                .expect("ingest");
        }

        let records = order_events_for_order_id(&store, &order_id("order-1"), 10)
            .await
            .expect("order events");

        assert_eq!(ORDER_EVENT_CONTRACT_IDS.len(), 9);
        assert_eq!(records.len(), 2);
        assert!(matches!(records[0], RadrootsOrderEventRecord::Request(_)));
        assert!(matches!(records[1], RadrootsOrderEventRecord::Decision(_)));
        assert_eq!(records[0].event_id().as_str(), request_event.id.as_str());
        assert_eq!(records[1].event_id().as_str(), decision_event.id.as_str());
        assert!(
            records
                .iter()
                .all(|record| record.order_id().as_str() == "order-1")
        );
    }

    #[cfg(feature = "event_store")]
    #[tokio::test]
    async fn order_projection_for_order_id_reduces_store_events() {
        let store = RadrootsEventStore::open_memory().await.expect("store");
        let request_event = store_order_request_event("order-1", 20);
        let request_event_id = RadrootsEventId::parse(&request_event.id).expect("request id");
        let decision_event = store_order_decision_event("order-1", &request_event_id, 21);

        for (event, observed_at_ms) in [(request_event, 2_000), (decision_event.clone(), 2_100)] {
            store
                .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
                .await
                .expect("ingest");
        }

        let projection = order_projection_for_order_id(&store, &order_id("order-1"), 10)
            .await
            .expect("projection");

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(
            projection
                .decision_event_id
                .as_ref()
                .map(RadrootsEventId::as_str),
            Some(decision_event.id.as_str())
        );
        assert!(projection.issues.is_empty());

        let result = order_projection_query_for_order_id(&store, &order_id("order-1"), 10)
            .await
            .expect("projection result");

        assert_eq!(result.event_count, 2);
        assert_eq!(result.limit_applied, 10);
        assert_eq!(
            result
                .event_ids
                .iter()
                .map(RadrootsEventId::as_str)
                .collect::<Vec<_>>(),
            vec![request_event_id.as_str(), decision_event.id.as_str()]
        );
        assert_eq!(result.projection.status, RadrootsOrderStatus::Accepted);
    }

    #[cfg(feature = "event_store")]
    #[tokio::test]
    async fn order_events_for_order_id_reports_invalid_stored_tags_json() {
        let store = RadrootsEventStore::open_memory().await.expect("store");
        let request_event = store_order_request_event("order-1", 30);
        store
            .ingest_event(RadrootsEventIngest::new(request_event.clone(), 3_000))
            .await
            .expect("ingest");
        sqlx::query("UPDATE nostr_event SET tags_json = '[' WHERE event_id = ?")
            .bind(request_event.id.as_str())
            .execute(store.pool())
            .await
            .expect("corrupt tags_json");

        let error = order_events_for_order_id(&store, &order_id("order-1"), 10)
            .await
            .expect_err("invalid stored tags");

        assert!(matches!(
            error,
            RadrootsOrderStoreQueryError::InvalidStoredTagsJson { .. }
        ));
    }

    fn reduce_order_events<I, J, K, L, M>(
        order_id: &str,
        requests: I,
        decisions: J,
        fulfillments: K,
        cancellations: L,
        receipts: M,
    ) -> RadrootsOrderProjection
    where
        I: IntoIterator<Item = RadrootsOrderRequestRecord>,
        J: IntoIterator<Item = RadrootsOrderDecisionRecord>,
        K: IntoIterator<Item = RadrootsOrderFulfillmentRecord>,
        L: IntoIterator<Item = RadrootsOrderCancellationRecord>,
        M: IntoIterator<Item = RadrootsOrderReceiptRecord>,
    {
        let order_id = RadrootsOrderId::parse(order_id).expect("order id");
        reduce_order_events_with_revisions(
            &order_id,
            requests,
            decisions,
            Vec::<RadrootsOrderRevisionProposalRecord>::new(),
            Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
            fulfillments,
            cancellations,
            receipts,
            Vec::<RadrootsOrderPaymentEventRecord>::new(),
            Vec::<RadrootsOrderSettlementRecord>::new(),
        )
    }

    fn reduce_order_events_with_revisions<I, J, K, L, M, N, O, P, Q>(
        order_id: &RadrootsOrderId,
        requests: I,
        decisions: J,
        revision_proposals: K,
        revision_decisions: L,
        fulfillments: M,
        cancellations: N,
        receipts: O,
        payments: P,
        settlements: Q,
    ) -> RadrootsOrderProjection
    where
        I: IntoIterator<Item = RadrootsOrderRequestRecord>,
        J: IntoIterator<Item = RadrootsOrderDecisionRecord>,
        K: IntoIterator<Item = RadrootsOrderRevisionProposalRecord>,
        L: IntoIterator<Item = RadrootsOrderRevisionDecisionRecord>,
        M: IntoIterator<Item = RadrootsOrderFulfillmentRecord>,
        N: IntoIterator<Item = RadrootsOrderCancellationRecord>,
        O: IntoIterator<Item = RadrootsOrderReceiptRecord>,
        P: IntoIterator<Item = RadrootsOrderPaymentEventRecord>,
        Q: IntoIterator<Item = RadrootsOrderSettlementRecord>,
    {
        reduce_order_events_with_revisions_inner(
            order_id,
            RadrootsOrderReductionInputs {
                requests,
                decisions,
                revision_proposals,
                revision_decisions,
                fulfillments,
                cancellations,
                receipts,
                payments,
                settlements,
            },
        )
    }

    fn reduce_listing_inventory_accounting<I, J, K, L, M, N>(
        listing_addr: &str,
        listing_event_id: &str,
        bins: I,
        requests: J,
        decisions: K,
        fulfillments: L,
        cancellations: M,
        receipts: N,
    ) -> RadrootsListingInventoryAccountingProjection
    where
        I: IntoIterator<Item = RadrootsListingInventoryBinAvailability>,
        J: IntoIterator<Item = RadrootsOrderRequestRecord>,
        K: IntoIterator<Item = RadrootsOrderDecisionRecord>,
        L: IntoIterator<Item = RadrootsOrderFulfillmentRecord>,
        M: IntoIterator<Item = RadrootsOrderCancellationRecord>,
        N: IntoIterator<Item = RadrootsOrderReceiptRecord>,
    {
        let listing_addr = RadrootsListingAddress::parse(listing_addr).expect("listing address");
        let listing_event_id = test_event_id(listing_event_id);
        reduce_listing_inventory_accounting_with_revisions(
            &listing_addr,
            &listing_event_id,
            bins,
            requests,
            decisions,
            Vec::<RadrootsOrderRevisionProposalRecord>::new(),
            Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
            fulfillments,
            cancellations,
            receipts,
        )
    }

    fn reduce_listing_inventory_accounting_with_revisions<I, J, K, L, M, N, O, P>(
        listing_addr: &RadrootsListingAddress,
        listing_event_id: &RadrootsEventId,
        bins: I,
        requests: J,
        decisions: K,
        revision_proposals: L,
        revision_decisions: M,
        fulfillments: N,
        cancellations: O,
        receipts: P,
    ) -> RadrootsListingInventoryAccountingProjection
    where
        I: IntoIterator<Item = RadrootsListingInventoryBinAvailability>,
        J: IntoIterator<Item = RadrootsOrderRequestRecord>,
        K: IntoIterator<Item = RadrootsOrderDecisionRecord>,
        L: IntoIterator<Item = RadrootsOrderRevisionProposalRecord>,
        M: IntoIterator<Item = RadrootsOrderRevisionDecisionRecord>,
        N: IntoIterator<Item = RadrootsOrderFulfillmentRecord>,
        O: IntoIterator<Item = RadrootsOrderCancellationRecord>,
        P: IntoIterator<Item = RadrootsOrderReceiptRecord>,
    {
        reduce_listing_inventory_accounting_with_revisions_inner(
            listing_addr,
            listing_event_id,
            RadrootsListingInventoryAccountingInputs {
                bins,
                requests,
                decisions,
                revision_proposals,
                revision_decisions,
                fulfillments,
                cancellations,
                receipts,
            },
        )
    }

    #[test]
    fn canonicalize_order_request_sets_authority_and_trims_items() {
        let request =
            canonicalize_order_request_for_signer(sample_order_request("", ""), BUYER).unwrap();

        assert_eq!(request.order_id, "order-1");
        assert_eq!(
            request.listing_addr,
            format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(request.buyer_pubkey, BUYER);
        assert_eq!(request.seller_pubkey, SELLER);
        assert_eq!(request.items[0].bin_id, "bin-1");
    }

    #[test]
    fn canonicalize_order_request_merges_duplicate_items() {
        let mut request = sample_order_request("", "");
        request.economics.total = usd("12");
        request.items = vec![
            RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 1,
            },
            RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 1,
            },
        ];

        let request = canonicalize_order_request_for_signer(request, BUYER).unwrap();

        assert_eq!(
            request.items,
            vec![RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 2,
            }]
        );
        assert_eq!(request.economics.total, usd("10"));
    }

    #[test]
    fn canonicalize_order_request_rejects_wrong_buyer_signer() {
        let error = canonicalize_order_request_for_signer(sample_order_request(SELLER, ""), BUYER)
            .unwrap_err();

        assert!(matches!(
            error,
            RadrootsOrderCanonicalizationError::InvalidBuyerSigner
        ));
    }

    #[test]
    fn canonicalize_order_request_rejects_draft_listing_address() {
        let mut request = sample_order_request("", "");
        request.listing_addr = RadrootsListingAddress::parse(format!(
            "{KIND_LISTING_DRAFT}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
        ))
        .expect("draft listing address");

        let error = canonicalize_order_request_for_signer(request, BUYER).unwrap_err();

        assert!(matches!(
            error,
            RadrootsOrderCanonicalizationError::InvalidListingKind
        ));
    }

    #[test]
    fn canonicalize_order_decision_sets_seller_authority_and_commitments() {
        let decision =
            canonicalize_order_decision_for_signer(sample_order_decision(""), SELLER).unwrap();

        assert_eq!(decision.order_id, "order-1");
        assert_eq!(
            decision.listing_addr,
            format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(decision.buyer_pubkey, BUYER);
        assert_eq!(decision.seller_pubkey, SELLER);
        let RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } = decision.decision
        else {
            panic!("expected accepted decision")
        };
        assert_eq!(inventory_commitments[0].bin_id, "bin-1");
    }

    #[test]
    fn canonicalize_order_decision_rejects_wrong_seller_signer() {
        let error = canonicalize_order_decision_for_signer(sample_order_decision(BUYER), SELLER)
            .unwrap_err();

        assert!(matches!(
            error,
            RadrootsOrderCanonicalizationError::InvalidSellerListing
        ));
    }

    #[test]
    fn canonicalize_order_decision_rejects_invalid_commitments() {
        let mut decision = sample_order_decision("");
        let RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } = &mut decision.decision
        else {
            panic!("expected accepted decision")
        };
        inventory_commitments.clear();

        let error = canonicalize_order_decision_for_signer(decision, SELLER).unwrap_err();
        assert!(matches!(
            error,
            RadrootsOrderCanonicalizationError::MissingInventoryCommitments
        ));
    }

    #[test]
    fn canonicalize_order_decision_trims_decline_reason() {
        let mut decision = sample_order_decision("");
        decision.decision = RadrootsOrderDecisionOutcome::Declined {
            reason: " out_of_stock ".to_string(),
        };

        let decision = canonicalize_order_decision_for_signer(decision, SELLER).unwrap();
        let RadrootsOrderDecisionOutcome::Declined { reason } = decision.decision else {
            panic!("expected declined decision")
        };
        assert_eq!(reason, "out_of_stock");
    }

    #[test]
    fn reduce_order_events_reports_missing_without_events() {
        let projection = reduce_order_events("order-1", [], [], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Missing);
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_order_events_reports_requested_state() {
        let projection = reduce_order_events("order-1", [request_record()], [], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Requested);
        assert_eq!(
            projection.request_event_id.as_ref(),
            Some(&test_event_id("request-1"))
        );
        assert_eq!(
            projection.last_event_id.as_ref(),
            Some(&test_event_id("request-1"))
        );
        assert_eq!(
            projection.economics,
            Some(request_economics("bin-1", 2, "10"))
        );
    }

    #[test]
    fn reduce_order_events_reports_accepted_state() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [],
            [],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(
            projection.decision_event_id.as_ref(),
            Some(&test_event_id("decision-1"))
        );
        assert_eq!(
            projection.fulfillment_status,
            Some(RadrootsOrderFulfillmentState::AcceptedNotFulfilled)
        );
        assert_eq!(projection.fulfillment_event_id, None);
        assert_eq!(
            projection.last_event_id.as_ref(),
            Some(&test_event_id("decision-1"))
        );
        assert_eq!(
            projection.economics,
            Some(request_economics("bin-1", 2, "10"))
        );
    }

    #[test]
    fn reduce_order_events_reports_recorded_payment_state() {
        let projection = reduce_order_events_with_revisions(
            &order_id("order-1"),
            [request_record()],
            [accepted_decision_record("decision-1")],
            Vec::<RadrootsOrderRevisionProposalRecord>::new(),
            Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
            Vec::<RadrootsOrderFulfillmentRecord>::new(),
            Vec::<RadrootsOrderCancellationRecord>::new(),
            Vec::<RadrootsOrderReceiptRecord>::new(),
            [payment_record("payment-1", "decision-1")],
            Vec::<RadrootsOrderSettlementRecord>::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(
            projection.payment.state,
            RadrootsOrderPaymentState::Recorded
        );
        assert_eq!(
            projection.payment.settlement_state,
            RadrootsOrderSettlementState::Pending
        );
        assert_eq!(
            projection.payment.payment_event_id.as_ref(),
            Some(&test_event_id("payment-1"))
        );
        assert_eq!(
            projection.payment.agreement_event_id.as_ref(),
            Some(&test_event_id("decision-1"))
        );
        assert_eq!(projection.payment.amount, Some(decimal("10")));
        assert_eq!(projection.payment.currency, Some(RadrootsCoreCurrency::USD));
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_order_events_reports_accepted_settlement_state() {
        let projection = reduce_order_events_with_revisions(
            &order_id("order-1"),
            [request_record()],
            [accepted_decision_record("decision-1")],
            Vec::<RadrootsOrderRevisionProposalRecord>::new(),
            Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
            Vec::<RadrootsOrderFulfillmentRecord>::new(),
            Vec::<RadrootsOrderCancellationRecord>::new(),
            Vec::<RadrootsOrderReceiptRecord>::new(),
            [payment_record("payment-1", "decision-1")],
            [settlement_record(
                "settlement-1",
                "payment-1",
                RadrootsOrderSettlementOutcome::Accepted,
            )],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(projection.payment.state, RadrootsOrderPaymentState::Settled);
        assert_eq!(
            projection.payment.settlement_state,
            RadrootsOrderSettlementState::Accepted
        );
        assert_eq!(
            projection.payment.settlement_event_id.as_ref(),
            Some(&test_event_id("settlement-1"))
        );
        assert_eq!(projection.payment.reason, None);
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_order_events_rejects_stale_payment_amount() {
        let mut payment = payment_record("payment-1", "decision-1");
        payment.payload.amount = decimal("9");

        let projection = reduce_order_events_with_revisions(
            &order_id("order-1"),
            [request_record()],
            [accepted_decision_record("decision-1")],
            Vec::<RadrootsOrderRevisionProposalRecord>::new(),
            Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
            Vec::<RadrootsOrderFulfillmentRecord>::new(),
            Vec::<RadrootsOrderCancellationRecord>::new(),
            Vec::<RadrootsOrderReceiptRecord>::new(),
            [payment],
            Vec::<RadrootsOrderSettlementRecord>::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(projection.payment.state, RadrootsOrderPaymentState::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsOrderIssue::PaymentAmountMismatch { event_id }
                if event_id == &test_event_id("payment-1")
        )));
    }

    #[test]
    fn reduce_order_events_keeps_payment_separate_from_receipt() {
        let projection = reduce_order_events_with_revisions(
            &order_id("order-1"),
            [request_record()],
            [accepted_decision_record("decision-1")],
            Vec::<RadrootsOrderRevisionProposalRecord>::new(),
            Vec::<RadrootsOrderRevisionDecisionRecord>::new(),
            [fulfillment_record(
                "fulfillment-1",
                "decision-1",
                RadrootsOrderFulfillmentState::Delivered,
            )],
            Vec::<RadrootsOrderCancellationRecord>::new(),
            [receipt_record("receipt-1", "fulfillment-1", true)],
            [payment_record("payment-1", "decision-1")],
            Vec::<RadrootsOrderSettlementRecord>::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Completed);
        assert_eq!(projection.receipt_received, Some(true));
        assert_eq!(
            projection.payment.state,
            RadrootsOrderPaymentState::Recorded
        );
        assert_eq!(
            projection.payment.settlement_state,
            RadrootsOrderSettlementState::Pending
        );
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_order_events_applies_accepted_revision_agreement() {
        let projection = reduce_order_events_with_revisions(
            &order_id("order-1"),
            [request_record()],
            [accepted_decision_record("decision-1")],
            [revision_proposal_record(
                "revision-proposal-1",
                "decision-1",
                "revision-1",
                1,
            )],
            [revision_decision_record(
                "revision-decision-1",
                "revision-proposal-1",
                "revision-1",
                RadrootsOrderRevisionOutcome::Accepted,
            )],
            Vec::<RadrootsOrderFulfillmentRecord>::new(),
            Vec::<RadrootsOrderCancellationRecord>::new(),
            Vec::<RadrootsOrderReceiptRecord>::new(),
            Vec::<RadrootsOrderPaymentEventRecord>::new(),
            Vec::<RadrootsOrderSettlementRecord>::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(
            projection.agreement_event_id.as_ref(),
            Some(&test_event_id("revision-decision-1"))
        );
        assert_eq!(
            projection.last_event_id.as_ref(),
            Some(&test_event_id("revision-decision-1"))
        );
        assert_eq!(
            projection.economics,
            Some(request_economics("bin-1", 1, "5"))
        );
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_order_events_preserves_agreement_after_declined_revision() {
        let projection = reduce_order_events_with_revisions(
            &order_id("order-1"),
            [request_record()],
            [accepted_decision_record("decision-1")],
            [revision_proposal_record(
                "revision-proposal-1",
                "decision-1",
                "revision-1",
                1,
            )],
            [revision_decision_record(
                "revision-decision-1",
                "revision-proposal-1",
                "revision-1",
                RadrootsOrderRevisionOutcome::Declined {
                    reason: "keep original order".to_string(),
                },
            )],
            Vec::<RadrootsOrderFulfillmentRecord>::new(),
            Vec::<RadrootsOrderCancellationRecord>::new(),
            Vec::<RadrootsOrderReceiptRecord>::new(),
            Vec::<RadrootsOrderPaymentEventRecord>::new(),
            Vec::<RadrootsOrderSettlementRecord>::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(
            projection.agreement_event_id.as_ref(),
            Some(&test_event_id("decision-1"))
        );
        assert_eq!(
            projection.last_event_id.as_ref(),
            Some(&test_event_id("revision-decision-1"))
        );
        assert_eq!(
            projection.economics,
            Some(request_economics("bin-1", 2, "10"))
        );
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_order_events_rejects_wrong_actor_revision_decision() {
        let mut decision = revision_decision_record(
            "revision-decision-1",
            "revision-proposal-1",
            "revision-1",
            RadrootsOrderRevisionOutcome::Accepted,
        );
        decision.author_pubkey = pubkey(SELLER);

        let projection = reduce_order_events_with_revisions(
            &order_id("order-1"),
            [request_record()],
            [accepted_decision_record("decision-1")],
            [revision_proposal_record(
                "revision-proposal-1",
                "decision-1",
                "revision-1",
                1,
            )],
            [decision],
            Vec::<RadrootsOrderFulfillmentRecord>::new(),
            Vec::<RadrootsOrderCancellationRecord>::new(),
            Vec::<RadrootsOrderReceiptRecord>::new(),
            Vec::<RadrootsOrderPaymentEventRecord>::new(),
            Vec::<RadrootsOrderSettlementRecord>::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsOrderIssue::RevisionDecisionAuthorMismatch { event_id }
                if event_id == &test_event_id("revision-decision-1")
        )));
    }

    #[test]
    fn reduce_order_events_rejects_stale_revision_decision() {
        let projection = reduce_order_events_with_revisions(
            &order_id("order-1"),
            [request_record()],
            [accepted_decision_record("decision-1")],
            [revision_proposal_record(
                "revision-proposal-1",
                "decision-1",
                "revision-1",
                1,
            )],
            [revision_decision_record(
                "revision-decision-1",
                "unknown-proposal",
                "revision-1",
                RadrootsOrderRevisionOutcome::Accepted,
            )],
            Vec::<RadrootsOrderFulfillmentRecord>::new(),
            Vec::<RadrootsOrderCancellationRecord>::new(),
            Vec::<RadrootsOrderReceiptRecord>::new(),
            Vec::<RadrootsOrderPaymentEventRecord>::new(),
            Vec::<RadrootsOrderSettlementRecord>::new(),
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsOrderIssue::RevisionDecisionWithoutProposal { event_id }
                if event_id == &test_event_id("revision-decision-1")
        )));
    }

    #[test]
    fn reduce_order_events_rejects_invalid_request_economics() {
        let mut request = request_record();
        request.payload.economics.total = usd("12");

        let projection = reduce_order_events("order-1", [request], [], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(projection.economics, None);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::RequestPayloadInvalid {
                event_id: test_event_id("request-1")
            }]
        );
    }

    #[test]
    fn reduce_order_events_reports_latest_fulfillment_state() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [
                fulfillment_record(
                    "fulfillment-2",
                    "fulfillment-1",
                    RadrootsOrderFulfillmentState::ReadyForPickup,
                ),
                fulfillment_record(
                    "fulfillment-1",
                    "decision-1",
                    RadrootsOrderFulfillmentState::Preparing,
                ),
            ],
            [],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(
            projection.fulfillment_status,
            Some(RadrootsOrderFulfillmentState::ReadyForPickup)
        );
        assert_eq!(
            projection.fulfillment_event_id.as_ref(),
            Some(&test_event_id("fulfillment-2"))
        );
        assert_eq!(
            projection.last_event_id.as_ref(),
            Some(&test_event_id("fulfillment-2"))
        );
    }

    #[test]
    fn reduce_order_events_keeps_delivered_without_receipt_nonterminal() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [fulfillment_record(
                "fulfillment-1",
                "decision-1",
                RadrootsOrderFulfillmentState::Delivered,
            )],
            [],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert_eq!(
            projection.fulfillment_status,
            Some(RadrootsOrderFulfillmentState::Delivered)
        );
        assert!(!projection.lifecycle_terminal);
    }

    #[test]
    fn reduce_order_events_reports_requested_cancellation() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [],
            [],
            [cancellation_record("cancel-1", "request-1")],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Cancelled);
        assert_eq!(
            projection.request_event_id.as_ref(),
            Some(&test_event_id("request-1"))
        );
        assert_eq!(
            projection.cancellation_event_id.as_ref(),
            Some(&test_event_id("cancel-1"))
        );
        assert_eq!(
            projection.last_event_id.as_ref(),
            Some(&test_event_id("cancel-1"))
        );
        assert!(projection.lifecycle_terminal);
        assert_eq!(
            projection.payment,
            RadrootsOrderPaymentProjection::not_recorded()
        );
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_order_events_rejects_request_cancellation_decision_fork() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [],
            [cancellation_record("cancel-1", "request-1")],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::ForkedLifecycle {
                event_ids: vec![test_event_id("cancel-1"), test_event_id("decision-1")]
            }]
        );
    }

    #[test]
    fn reduce_order_events_reports_accepted_cancellation_before_fulfillment() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [],
            [cancellation_record("cancel-1", "decision-1")],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Cancelled);
        assert_eq!(
            projection.decision_event_id.as_ref(),
            Some(&test_event_id("decision-1"))
        );
        assert_eq!(
            projection.cancellation_event_id.as_ref(),
            Some(&test_event_id("cancel-1"))
        );
        assert_eq!(
            projection.last_event_id.as_ref(),
            Some(&test_event_id("cancel-1"))
        );
        assert!(projection.lifecycle_terminal);
    }

    #[test]
    fn reduce_order_events_rejects_cancellation_fulfillment_fork() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [fulfillment_record(
                "fulfillment-1",
                "decision-1",
                RadrootsOrderFulfillmentState::Preparing,
            )],
            [cancellation_record("cancel-1", "decision-1")],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::ForkedLifecycle {
                event_ids: vec![test_event_id("cancel-1"), test_event_id("fulfillment-1")]
            }]
        );
    }

    #[test]
    fn reduce_order_events_reports_completed_buyer_receipt() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [fulfillment_record(
                "fulfillment-1",
                "decision-1",
                RadrootsOrderFulfillmentState::ReadyForPickup,
            )],
            [],
            [receipt_record("receipt-1", "fulfillment-1", true)],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Completed);
        assert_eq!(
            projection.fulfillment_event_id.as_ref(),
            Some(&test_event_id("fulfillment-1"))
        );
        assert_eq!(
            projection.receipt_event_id.as_ref(),
            Some(&test_event_id("receipt-1"))
        );
        assert_eq!(projection.receipt_received, Some(true));
        assert_eq!(projection.receipt_issue, None);
        assert_eq!(projection.receipt_received_at, Some(1_777_665_600));
        assert!(projection.lifecycle_terminal);
        assert_eq!(
            projection.payment,
            RadrootsOrderPaymentProjection::not_recorded()
        );
    }

    #[test]
    fn reduce_order_events_rejects_receipt_fulfillment_fork() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [
                fulfillment_record(
                    "fulfillment-1",
                    "decision-1",
                    RadrootsOrderFulfillmentState::ReadyForPickup,
                ),
                fulfillment_record(
                    "fulfillment-2",
                    "fulfillment-1",
                    RadrootsOrderFulfillmentState::Delivered,
                ),
            ],
            [],
            [receipt_record("receipt-1", "fulfillment-1", true)],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::ForkedLifecycle {
                event_ids: vec![test_event_id("fulfillment-2"), test_event_id("receipt-1")]
            }]
        );
    }

    #[test]
    fn reduce_order_events_reports_disputed_buyer_receipt() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [fulfillment_record(
                "fulfillment-1",
                "decision-1",
                RadrootsOrderFulfillmentState::Delivered,
            )],
            [],
            [receipt_record("receipt-1", "fulfillment-1", false)],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Disputed);
        assert_eq!(
            projection.receipt_event_id.as_ref(),
            Some(&test_event_id("receipt-1"))
        );
        assert_eq!(projection.receipt_received, Some(false));
        assert_eq!(projection.receipt_issue.as_deref(), Some("damaged items"));
        assert!(projection.lifecycle_terminal);
        assert_eq!(
            projection.payment,
            RadrootsOrderPaymentProjection::not_recorded()
        );
    }

    #[test]
    fn reduce_order_events_rejects_receipt_without_eligible_fulfillment() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [fulfillment_record(
                "fulfillment-1",
                "decision-1",
                RadrootsOrderFulfillmentState::Preparing,
            )],
            [],
            [receipt_record("receipt-1", "fulfillment-1", true)],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::ReceiptWithoutEligibleFulfillment {
                event_id: test_event_id("receipt-1")
            }]
        );
    }

    #[test]
    fn reduce_order_events_rejects_fulfillment_before_acceptance() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [],
            [fulfillment_record(
                "fulfillment-1",
                "request-1",
                RadrootsOrderFulfillmentState::Preparing,
            )],
            [],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::FulfillmentWithoutAcceptedDecision {
                event_id: test_event_id("fulfillment-1")
            }]
        );
    }

    #[test]
    fn reduce_order_events_rejects_fulfillment_after_decline() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [declined_decision_record("decision-1")],
            [fulfillment_record(
                "fulfillment-1",
                "decision-1",
                RadrootsOrderFulfillmentState::Preparing,
            )],
            [],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::FulfillmentWithoutAcceptedDecision {
                event_id: test_event_id("fulfillment-1")
            }]
        );
    }

    #[test]
    fn reduce_order_events_rejects_wrong_actor_fulfillment() {
        let mut fulfillment = fulfillment_record(
            "fulfillment-1",
            "decision-1",
            RadrootsOrderFulfillmentState::Preparing,
        );
        fulfillment.author_pubkey = pubkey(BUYER);

        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [fulfillment],
            [],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsOrderIssue::FulfillmentAuthorMismatch { event_id }
                if event_id == &test_event_id("fulfillment-1")
        )));
    }

    #[test]
    fn reduce_order_events_rejects_forked_fulfillment_chain() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [
                fulfillment_record(
                    "fulfillment-2",
                    "decision-1",
                    RadrootsOrderFulfillmentState::Preparing,
                ),
                fulfillment_record(
                    "fulfillment-1",
                    "decision-1",
                    RadrootsOrderFulfillmentState::ReadyForPickup,
                ),
            ],
            [],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::ForkedFulfillments {
                event_ids: vec![
                    test_event_id("fulfillment-1"),
                    test_event_id("fulfillment-2")
                ]
            }]
        );
    }

    #[test]
    fn reduce_order_events_rejects_terminal_fulfillment_transition() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [
                fulfillment_record(
                    "fulfillment-1",
                    "decision-1",
                    RadrootsOrderFulfillmentState::Delivered,
                ),
                fulfillment_record(
                    "fulfillment-2",
                    "fulfillment-1",
                    RadrootsOrderFulfillmentState::ReadyForPickup,
                ),
            ],
            [],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::FulfillmentUnsupportedTransition {
                event_id: test_event_id("fulfillment-2")
            }]
        );
    }

    #[test]
    fn reduce_order_events_reports_declined_state() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [declined_decision_record("decision-1")],
            [],
            [],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Declined);
        assert_eq!(
            projection.decision_event_id.as_ref(),
            Some(&test_event_id("decision-1"))
        );
    }

    #[test]
    fn reduce_listing_inventory_accounting_reserves_accepted_inventory() {
        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [inventory_bin(5)],
            [request_record()],
            [accepted_decision_record("decision-1")],
            [],
            [],
            [],
        );

        assert_eq!(
            projection.listing_event_id,
            test_event_id("listing-event-1")
        );
        assert_eq!(projection.declined_order_ids, Vec::<RadrootsOrderId>::new());
        assert_eq!(
            projection.cancelled_order_ids,
            Vec::<RadrootsOrderId>::new()
        );
        assert_eq!(projection.invalid_event_ids, Vec::<RadrootsEventId>::new());
        assert!(projection.issues.is_empty());
        assert_eq!(
            projection.bins,
            vec![RadrootsListingInventoryBinAccounting {
                bin_id: bin_id("bin-1"),
                available_count: 5,
                accepted_reserved_count: 2,
                remaining_count: 3,
                over_reserved: false,
                accepted_orders: vec![RadrootsListingInventoryOrderReservation {
                    order_id: order_id("order-1"),
                    decision_event_id: test_event_id("decision-1"),
                    bin_count: 2,
                }],
            }]
        );
    }

    #[test]
    fn reduce_listing_inventory_accounting_reserves_accepted_revision_inventory() {
        let projection = reduce_listing_inventory_accounting_with_revisions(
            &listing_address(),
            &test_event_id("listing-event-1"),
            [inventory_bin(5)],
            [request_record()],
            [accepted_decision_record("decision-1")],
            [revision_proposal_record(
                "revision-proposal-1",
                "decision-1",
                "revision-1",
                1,
            )],
            [revision_decision_record(
                "revision-decision-1",
                "revision-proposal-1",
                "revision-1",
                RadrootsOrderRevisionOutcome::Accepted,
            )],
            Vec::<RadrootsOrderFulfillmentRecord>::new(),
            Vec::<RadrootsOrderCancellationRecord>::new(),
            Vec::<RadrootsOrderReceiptRecord>::new(),
        );

        assert!(projection.issues.is_empty());
        assert_eq!(projection.bins[0].accepted_reserved_count, 1);
        assert_eq!(projection.bins[0].remaining_count, 4);
        assert_eq!(
            projection.bins[0].accepted_orders,
            vec![RadrootsListingInventoryOrderReservation {
                order_id: order_id("order-1"),
                decision_event_id: test_event_id("revision-decision-1"),
                bin_count: 1,
            }]
        );
    }

    #[test]
    fn reduce_listing_inventory_accounting_releases_latest_seller_cancelled_order() {
        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [inventory_bin(5)],
            [request_record()],
            [accepted_decision_record("decision-1")],
            [fulfillment_record(
                "fulfillment-1",
                "decision-1",
                RadrootsOrderFulfillmentState::SellerCancelled,
            )],
            [],
            [],
        );

        assert!(projection.issues.is_empty());
        assert_eq!(projection.invalid_event_ids, Vec::<RadrootsEventId>::new());
        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(projection.bins[0].remaining_count, 5);
        assert!(projection.bins[0].accepted_orders.is_empty());
    }

    #[test]
    fn reduce_listing_inventory_accounting_releases_accepted_buyer_cancelled_order() {
        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [inventory_bin(5)],
            [request_record()],
            [accepted_decision_record("decision-1")],
            [],
            [cancellation_record("cancel-1", "decision-1")],
            [],
        );

        assert!(projection.issues.is_empty());
        assert_eq!(projection.cancelled_order_ids, vec![order_id("order-1")]);
        assert_eq!(projection.invalid_event_ids, Vec::<RadrootsEventId>::new());
        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(projection.bins[0].remaining_count, 5);
        assert!(projection.bins[0].accepted_orders.is_empty());
    }

    #[test]
    fn reduce_listing_inventory_accounting_keeps_receipted_order_reserved() {
        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [inventory_bin(5)],
            [request_record()],
            [accepted_decision_record("decision-1")],
            [fulfillment_record(
                "fulfillment-1",
                "decision-1",
                RadrootsOrderFulfillmentState::Delivered,
            )],
            [],
            [receipt_record("receipt-1", "fulfillment-1", true)],
        );

        assert!(projection.issues.is_empty());
        assert!(projection.cancelled_order_ids.is_empty());
        assert_eq!(projection.bins[0].accepted_reserved_count, 2);
        assert_eq!(projection.bins[0].remaining_count, 3);
    }

    #[test]
    fn reduce_listing_inventory_accounting_rejects_forked_cancel_release() {
        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [inventory_bin(5)],
            [request_record()],
            [accepted_decision_record("decision-1")],
            [
                fulfillment_record(
                    "fulfillment-2",
                    "decision-1",
                    RadrootsOrderFulfillmentState::SellerCancelled,
                ),
                fulfillment_record(
                    "fulfillment-1",
                    "decision-1",
                    RadrootsOrderFulfillmentState::Preparing,
                ),
            ],
            [],
            [],
        );

        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(
            projection.invalid_event_ids,
            vec![
                test_event_id("fulfillment-1"),
                test_event_id("fulfillment-2")
            ]
        );
        assert_eq!(
            projection.issues,
            vec![RadrootsListingInventoryAccountingIssue::InvalidOrder {
                order_id: order_id("order-1"),
                event_ids: vec![
                    test_event_id("fulfillment-1"),
                    test_event_id("fulfillment-2")
                ],
            }]
        );
    }

    #[test]
    fn reduce_listing_inventory_accounting_leaves_declined_inventory_available() {
        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [inventory_bin(5)],
            [request_record()],
            [declined_decision_record("decision-1")],
            [],
            [],
            [],
        );

        assert_eq!(projection.declined_order_ids, vec![order_id("order-1")]);
        assert!(projection.cancelled_order_ids.is_empty());
        assert!(projection.invalid_event_ids.is_empty());
        assert!(projection.issues.is_empty());
        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(projection.bins[0].remaining_count, 5);
        assert!(!projection.bins[0].over_reserved);
    }

    #[test]
    fn reduce_listing_inventory_accounting_reports_invalid_mismatched_commitment() {
        let decision = RadrootsOrderDecisionRecord {
            payload: decision_payload(RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                    bin_id: bin_id("bin-1"),
                    bin_count: 1,
                }],
            }),
            ..accepted_decision_record("decision-1")
        };

        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [inventory_bin(5)],
            [request_record()],
            [decision],
            [],
            [],
            [],
        );

        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(
            projection.invalid_event_ids,
            vec![test_event_id("decision-1")]
        );
        assert_eq!(
            projection.issues,
            vec![RadrootsListingInventoryAccountingIssue::InvalidOrder {
                order_id: order_id("order-1"),
                event_ids: vec![test_event_id("decision-1")],
            }]
        );
    }

    #[test]
    fn reduce_listing_inventory_accounting_reports_over_reserved_bins() {
        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [inventory_bin(3)],
            [
                request_record_for("order-2", "request-2", 2),
                request_record_for("order-1", "request-1", 2),
            ],
            [
                accepted_decision_record_for("order-2", "decision-2", "request-2", 2),
                accepted_decision_record_for("order-1", "decision-1", "request-1", 2),
            ],
            [],
            [],
            [],
        );

        assert_eq!(projection.bins[0].available_count, 3);
        assert_eq!(projection.bins[0].accepted_reserved_count, 4);
        assert_eq!(projection.bins[0].remaining_count, 0);
        assert!(projection.bins[0].over_reserved);
        assert_eq!(
            projection.issues,
            vec![RadrootsListingInventoryAccountingIssue::OverReserved {
                bin_id: bin_id("bin-1"),
                available_count: 3,
                reserved_count: 4,
                event_ids: vec![test_event_id("decision-1"), test_event_id("decision-2")],
            }]
        );
    }

    #[test]
    fn reduce_listing_inventory_accounting_reports_duplicate_availability_overflow() {
        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [
                RadrootsListingInventoryBinAvailability {
                    bin_id: bin_id("bin-1"),
                    available_count: u64::MAX,
                },
                inventory_bin(1),
            ],
            Vec::<RadrootsOrderRequestRecord>::new(),
            Vec::<RadrootsOrderDecisionRecord>::new(),
            Vec::<RadrootsOrderFulfillmentRecord>::new(),
            Vec::<RadrootsOrderCancellationRecord>::new(),
            Vec::<RadrootsOrderReceiptRecord>::new(),
        );

        assert_eq!(projection.bins[0].available_count, u64::MAX);
        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(projection.bins[0].remaining_count, u64::MAX);
        assert_eq!(
            projection.issues,
            vec![
                RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                    bin_id: bin_id("bin-1"),
                    event_ids: Vec::new(),
                }
            ]
        );
    }

    #[test]
    fn add_inventory_reservation_reports_reservation_overflow() {
        let mut bin = RadrootsListingInventoryBinAccounting {
            bin_id: bin_id("bin-1"),
            available_count: u64::MAX,
            accepted_reserved_count: u64::MAX,
            remaining_count: 0,
            over_reserved: false,
            accepted_orders: Vec::new(),
        };
        let decision = accepted_decision_record("decision-overflow");
        let mut issues = Vec::new();

        add_inventory_reservation(
            &mut bin,
            &order_id("order-overflow"),
            &decision,
            1,
            &mut issues,
        );

        assert_eq!(bin.accepted_reserved_count, u64::MAX);
        assert!(bin.accepted_orders.is_empty());
        assert_eq!(
            issues,
            vec![
                RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                    bin_id: bin_id("bin-1"),
                    event_ids: vec![test_event_id("decision-overflow")],
                }
            ]
        );
    }

    #[test]
    fn inventory_accounting_issues_sort_by_rank_id_and_event_ids() {
        let invalid = RadrootsListingInventoryAccountingIssue::InvalidOrder {
            order_id: order_id("order-1"),
            event_ids: vec![test_event_id("event-c")],
        };
        let overflow = RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
            bin_id: bin_id("bin-1"),
            event_ids: vec![test_event_id("event-b")],
        };
        let unknown = RadrootsListingInventoryAccountingIssue::UnknownInventoryBin {
            bin_id: bin_id("bin-1"),
            event_ids: vec![test_event_id("event-a")],
        };
        let over_reserved = RadrootsListingInventoryAccountingIssue::OverReserved {
            bin_id: bin_id("bin-1"),
            available_count: 1,
            reserved_count: 2,
            event_ids: vec![test_event_id("event-d")],
        };

        assert_eq!(inventory_issue_rank(&invalid), 0);
        assert_eq!(inventory_issue_rank(&overflow), 1);
        assert_eq!(inventory_issue_rank(&unknown), 2);
        assert_eq!(inventory_issue_rank(&over_reserved), 3);
        assert_eq!(inventory_issue_id(&invalid), "order-1");
        assert_eq!(inventory_issue_id(&overflow), "bin-1");
        assert_eq!(inventory_issue_id(&unknown), "bin-1");
        assert_eq!(inventory_issue_id(&over_reserved), "bin-1");
        assert_eq!(
            inventory_issue_event_ids(&invalid),
            [test_event_id("event-c")]
        );
        assert_eq!(
            inventory_issue_event_ids(&overflow),
            [test_event_id("event-b")]
        );
        assert_eq!(
            inventory_issue_event_ids(&unknown),
            [test_event_id("event-a")]
        );
        assert_eq!(
            inventory_issue_event_ids(&over_reserved),
            [test_event_id("event-d")]
        );

        let mut issues = vec![
            over_reserved.clone(),
            unknown.clone(),
            overflow.clone(),
            invalid.clone(),
        ];
        issues.sort_by(inventory_issue_sort_key);

        assert_eq!(issues, vec![invalid, overflow, unknown, over_reserved]);
    }

    #[test]
    fn reduce_order_events_rejects_invalid_decision_actor() {
        let mut decision = accepted_decision_record("decision-1");
        decision.author_pubkey = pubkey(BUYER);

        let projection = reduce_order_events("order-1", [request_record()], [decision], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsOrderIssue::DecisionAuthorMismatch { event_id }
                if event_id == &test_event_id("decision-1")
        )));
    }

    #[test]
    fn reduce_order_events_rejects_invalid_decision_counterparty() {
        let mut decision = accepted_decision_record("decision-1");
        decision.counterparty_pubkey = pubkey(SELLER);

        let projection = reduce_order_events("order-1", [request_record()], [decision], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsOrderIssue::DecisionCounterpartyMismatch { event_id }
                if event_id == &test_event_id("decision-1")
        )));
    }

    #[test]
    fn reduce_listing_inventory_accounting_ignores_wrong_counterparty_decision() {
        let mut decision = accepted_decision_record("decision-1");
        decision.counterparty_pubkey = pubkey(SELLER);

        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [inventory_bin(5)],
            [request_record()],
            [decision],
            [],
            [],
            [],
        );

        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(
            projection.invalid_event_ids,
            vec![test_event_id("decision-1")]
        );
        assert_eq!(
            projection.issues,
            vec![RadrootsListingInventoryAccountingIssue::InvalidOrder {
                order_id: order_id("order-1"),
                event_ids: vec![test_event_id("decision-1")],
            }]
        );
    }

    #[test]
    fn reduce_order_events_rejects_invalid_decision_chain() {
        let mut decision = accepted_decision_record("decision-1");
        decision.prev_event_id = test_event_id("request-2");

        let projection = reduce_order_events("order-1", [request_record()], [decision], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsOrderIssue::DecisionPreviousMismatch { event_id }
                if event_id == &test_event_id("decision-1")
        )));
    }

    #[test]
    fn reduce_order_events_rejects_missing_commitment() {
        let decision = RadrootsOrderDecisionRecord {
            payload: decision_payload(RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: Vec::new(),
            }),
            ..accepted_decision_record("decision-1")
        };

        let projection = reduce_order_events("order-1", [request_record()], [decision], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsOrderIssue::DecisionMissingInventoryCommitments { event_id }
                if event_id == &test_event_id("decision-1")
        )));
    }

    #[test]
    fn reduce_order_events_rejects_commitment_count_mismatch() {
        let decision = RadrootsOrderDecisionRecord {
            payload: decision_payload(RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                    bin_id: bin_id("bin-1"),
                    bin_count: 1,
                }],
            }),
            ..accepted_decision_record("decision-1")
        };

        let projection = reduce_order_events("order-1", [request_record()], [decision], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsOrderIssue::DecisionInventoryCommitmentMismatch { event_id }
                if event_id == &test_event_id("decision-1")
        )));
    }

    #[test]
    fn reduce_order_events_rejects_commitment_bin_mismatch() {
        let decision = RadrootsOrderDecisionRecord {
            payload: decision_payload(RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                    bin_id: bin_id("bin-2"),
                    bin_count: 2,
                }],
            }),
            ..accepted_decision_record("decision-1")
        };

        let projection = reduce_order_events("order-1", [request_record()], [decision], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::DecisionInventoryCommitmentMismatch {
                event_id: test_event_id("decision-1")
            }]
        );
    }

    #[test]
    fn reduce_order_events_matches_normalized_duplicate_bins() {
        let mut request = request_record();
        request.payload.items = vec![
            RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 1,
            },
            RadrootsOrderItem {
                bin_id: bin_id("bin-1"),
                bin_count: 1,
            },
        ];
        let decision = RadrootsOrderDecisionRecord {
            payload: decision_payload(RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                    bin_id: bin_id("bin-1"),
                    bin_count: 2,
                }],
            }),
            ..accepted_decision_record("decision-1")
        };

        let projection = reduce_order_events("order-1", [request], [decision], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Accepted);
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_order_events_rejects_missing_decline_reason() {
        let decision = RadrootsOrderDecisionRecord {
            payload: decision_payload(RadrootsOrderDecisionOutcome::Declined {
                reason: " ".to_string(),
            }),
            ..declined_decision_record("decision-1")
        };

        let projection = reduce_order_events("order-1", [request_record()], [decision], [], [], []);

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsOrderIssue::DecisionMissingReason { event_id }
                if event_id == &test_event_id("decision-1")
        )));
    }

    #[test]
    fn reduce_order_events_rejects_conflicting_decisions() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [
                accepted_decision_record("decision-2"),
                declined_decision_record("decision-1"),
            ],
            [],
            [],
            [],
        );

        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::ConflictingDecisions {
                event_ids: vec![test_event_id("decision-1"), test_event_id("decision-2")]
            }]
        );
    }

    #[test]
    fn reduce_order_events_reports_multiple_requests_deterministically() {
        let projection = reduce_order_events(
            "order-1",
            [
                request_record_with_event_id("request-2"),
                request_record_with_event_id("request-1"),
            ],
            [],
            [],
            [],
            [],
        );
        let reversed = reduce_order_events(
            "order-1",
            [
                request_record_with_event_id("request-1"),
                request_record_with_event_id("request-2"),
            ],
            [],
            [],
            [],
            [],
        );

        assert_eq!(projection, reversed);
        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.request_event_id.as_ref(),
            Some(&test_event_id("request-2"))
        );
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::MultipleRequests {
                event_ids: vec![test_event_id("request-2"), test_event_id("request-1")]
            }]
        );
    }

    #[test]
    fn reduce_order_events_reports_conflicting_decisions_deterministically() {
        let projection = reduce_order_events(
            "order-1",
            [request_record()],
            [
                accepted_decision_record("decision-2"),
                declined_decision_record("decision-1"),
            ],
            [],
            [],
            [],
        );
        let reversed = reduce_order_events(
            "order-1",
            [request_record()],
            [
                declined_decision_record("decision-1"),
                accepted_decision_record("decision-2"),
            ],
            [],
            [],
            [],
        );

        assert_eq!(projection, reversed);
        assert_eq!(projection.status, RadrootsOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsOrderIssue::ConflictingDecisions {
                event_ids: vec![test_event_id("decision-1"), test_event_id("decision-2")]
            }]
        );
    }

    #[test]
    fn projection_issue_event_ids_covers_all_issue_variants() {
        macro_rules! issue {
            ($variant:ident, $id:expr) => {
                RadrootsOrderIssue::$variant {
                    event_id: test_event_id($id),
                }
            };
        }

        let issues = vec![
            RadrootsOrderIssue::MissingRequest,
            RadrootsOrderIssue::MultipleRequests {
                event_ids: vec![test_event_id("multi-b"), test_event_id("multi-a")],
            },
            issue!(RequestPayloadInvalid, "request-payload"),
            issue!(RequestOrderIdMismatch, "request-order"),
            issue!(RequestAuthorMismatch, "request-author"),
            issue!(RequestListingAddressInvalid, "request-listing-address"),
            issue!(RequestSellerListingMismatch, "request-seller-listing"),
            issue!(DecisionPayloadInvalid, "decision-payload"),
            issue!(DecisionOrderIdMismatch, "decision-order"),
            issue!(DecisionAuthorMismatch, "decision-author"),
            issue!(DecisionCounterpartyMismatch, "decision-counterparty"),
            issue!(DecisionBuyerMismatch, "decision-buyer"),
            issue!(DecisionSellerMismatch, "decision-seller"),
            issue!(DecisionListingAddressInvalid, "decision-listing-address"),
            issue!(DecisionListingMismatch, "decision-listing"),
            issue!(DecisionRootMismatch, "decision-root"),
            issue!(DecisionPreviousMismatch, "decision-previous"),
            issue!(
                DecisionMissingInventoryCommitments,
                "decision-missing-commitments"
            ),
            issue!(
                DecisionInventoryCommitmentMismatch,
                "decision-commitment-mismatch"
            ),
            issue!(DecisionMissingReason, "decision-missing-reason"),
            RadrootsOrderIssue::ConflictingDecisions {
                event_ids: vec![test_event_id("conflict-b"), test_event_id("conflict-a")],
            },
            issue!(
                RevisionProposalWithoutAcceptedDecision,
                "proposal-without-accepted"
            ),
            issue!(RevisionProposalPayloadInvalid, "proposal-payload"),
            issue!(RevisionProposalOrderIdMismatch, "proposal-order"),
            issue!(RevisionProposalAuthorMismatch, "proposal-author"),
            issue!(
                RevisionProposalCounterpartyMismatch,
                "proposal-counterparty"
            ),
            issue!(RevisionProposalBuyerMismatch, "proposal-buyer"),
            issue!(RevisionProposalSellerMismatch, "proposal-seller"),
            issue!(
                RevisionProposalListingAddressInvalid,
                "proposal-listing-address"
            ),
            issue!(RevisionProposalListingMismatch, "proposal-listing"),
            issue!(RevisionProposalRootMismatch, "proposal-root"),
            issue!(RevisionProposalPreviousMismatch, "proposal-previous"),
            issue!(RevisionDecisionWithoutProposal, "revision-without-proposal"),
            issue!(RevisionDecisionPayloadInvalid, "revision-payload"),
            issue!(RevisionDecisionOrderIdMismatch, "revision-order"),
            issue!(RevisionDecisionAuthorMismatch, "revision-author"),
            issue!(
                RevisionDecisionCounterpartyMismatch,
                "revision-counterparty"
            ),
            issue!(RevisionDecisionBuyerMismatch, "revision-buyer"),
            issue!(RevisionDecisionSellerMismatch, "revision-seller"),
            issue!(
                RevisionDecisionListingAddressInvalid,
                "revision-listing-address"
            ),
            issue!(RevisionDecisionListingMismatch, "revision-listing"),
            issue!(RevisionDecisionRootMismatch, "revision-root"),
            issue!(RevisionDecisionPreviousMismatch, "revision-previous"),
            issue!(RevisionDecisionRevisionIdMismatch, "revision-id"),
            issue!(
                FulfillmentWithoutAcceptedDecision,
                "fulfillment-without-accepted"
            ),
            issue!(FulfillmentPayloadInvalid, "fulfillment-payload"),
            issue!(FulfillmentOrderIdMismatch, "fulfillment-order"),
            issue!(FulfillmentAuthorMismatch, "fulfillment-author"),
            issue!(FulfillmentCounterpartyMismatch, "fulfillment-counterparty"),
            issue!(FulfillmentBuyerMismatch, "fulfillment-buyer"),
            issue!(FulfillmentSellerMismatch, "fulfillment-seller"),
            issue!(
                FulfillmentListingAddressInvalid,
                "fulfillment-listing-address"
            ),
            issue!(FulfillmentListingMismatch, "fulfillment-listing"),
            issue!(FulfillmentRootMismatch, "fulfillment-root"),
            issue!(FulfillmentPreviousMismatch, "fulfillment-previous"),
            issue!(
                FulfillmentStatusNotPublishable,
                "fulfillment-not-publishable"
            ),
            issue!(
                FulfillmentUnsupportedTransition,
                "fulfillment-unsupported-transition"
            ),
            RadrootsOrderIssue::ForkedFulfillments {
                event_ids: vec![
                    test_event_id("fulfillment-fork-b"),
                    test_event_id("fulfillment-fork-a"),
                ],
            },
            issue!(
                CancellationWithoutCancellableOrder,
                "cancellation-without-cancellable"
            ),
            issue!(CancellationPayloadInvalid, "cancellation-payload"),
            issue!(CancellationOrderIdMismatch, "cancellation-order"),
            issue!(CancellationAuthorMismatch, "cancellation-author"),
            issue!(
                CancellationCounterpartyMismatch,
                "cancellation-counterparty"
            ),
            issue!(CancellationBuyerMismatch, "cancellation-buyer"),
            issue!(CancellationSellerMismatch, "cancellation-seller"),
            issue!(
                CancellationListingAddressInvalid,
                "cancellation-listing-address"
            ),
            issue!(CancellationListingMismatch, "cancellation-listing"),
            issue!(CancellationRootMismatch, "cancellation-root"),
            issue!(CancellationPreviousMismatch, "cancellation-previous"),
            issue!(
                CancellationAfterFulfillment,
                "cancellation-after-fulfillment"
            ),
            issue!(
                ReceiptWithoutEligibleFulfillment,
                "receipt-without-eligible"
            ),
            issue!(ReceiptPayloadInvalid, "receipt-payload"),
            issue!(ReceiptOrderIdMismatch, "receipt-order"),
            issue!(ReceiptAuthorMismatch, "receipt-author"),
            issue!(ReceiptCounterpartyMismatch, "receipt-counterparty"),
            issue!(ReceiptBuyerMismatch, "receipt-buyer"),
            issue!(ReceiptSellerMismatch, "receipt-seller"),
            issue!(ReceiptListingAddressInvalid, "receipt-listing-address"),
            issue!(ReceiptListingMismatch, "receipt-listing"),
            issue!(ReceiptRootMismatch, "receipt-root"),
            issue!(ReceiptPreviousMismatch, "receipt-previous"),
            issue!(PaymentWithoutAcceptedAgreement, "payment-without-agreement"),
            issue!(PaymentPayloadInvalid, "payment-payload"),
            issue!(PaymentOrderIdMismatch, "payment-order"),
            issue!(PaymentAuthorMismatch, "payment-author"),
            issue!(PaymentCounterpartyMismatch, "payment-counterparty"),
            issue!(PaymentBuyerMismatch, "payment-buyer"),
            issue!(PaymentSellerMismatch, "payment-seller"),
            issue!(PaymentListingAddressInvalid, "payment-listing-address"),
            issue!(PaymentListingMismatch, "payment-listing"),
            issue!(PaymentRootMismatch, "payment-root"),
            issue!(PaymentPreviousMismatch, "payment-previous"),
            issue!(PaymentAgreementMismatch, "payment-agreement"),
            issue!(PaymentQuoteMismatch, "payment-quote"),
            issue!(PaymentQuoteVersionMismatch, "payment-quote-version"),
            issue!(PaymentEconomicsDigestMismatch, "payment-digest"),
            issue!(PaymentAmountMismatch, "payment-amount"),
            issue!(PaymentCurrencyMismatch, "payment-currency"),
            issue!(PaymentAfterCancellation, "payment-after-cancellation"),
            issue!(RevisionAfterPayment, "revision-after-payment"),
            RadrootsOrderIssue::DuplicatePayments {
                event_ids: vec![
                    test_event_id("payment-duplicate-b"),
                    test_event_id("payment-duplicate-a"),
                ],
            },
            issue!(SettlementWithoutValidPayment, "settlement-without-payment"),
            issue!(SettlementPayloadInvalid, "settlement-payload"),
            issue!(SettlementOrderIdMismatch, "settlement-order"),
            issue!(SettlementAuthorMismatch, "settlement-author"),
            issue!(SettlementCounterpartyMismatch, "settlement-counterparty"),
            issue!(SettlementBuyerMismatch, "settlement-buyer"),
            issue!(SettlementSellerMismatch, "settlement-seller"),
            issue!(
                SettlementListingAddressInvalid,
                "settlement-listing-address"
            ),
            issue!(SettlementListingMismatch, "settlement-listing"),
            issue!(SettlementRootMismatch, "settlement-root"),
            issue!(SettlementPreviousMismatch, "settlement-previous"),
            issue!(SettlementPaymentEventMismatch, "settlement-payment-event"),
            issue!(SettlementAgreementMismatch, "settlement-agreement"),
            issue!(SettlementQuoteMismatch, "settlement-quote"),
            issue!(SettlementQuoteVersionMismatch, "settlement-quote-version"),
            issue!(SettlementEconomicsDigestMismatch, "settlement-digest"),
            issue!(SettlementAmountMismatch, "settlement-amount"),
            issue!(SettlementCurrencyMismatch, "settlement-currency"),
            RadrootsOrderIssue::DuplicateSettlements {
                event_ids: vec![
                    test_event_id("settlement-duplicate-b"),
                    test_event_id("settlement-duplicate-a"),
                ],
            },
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: vec![test_event_id("lifecycle-b"), test_event_id("lifecycle-a")],
            },
        ];

        let event_ids = projection_issue_event_ids(&issues);

        assert!(event_ids.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(event_ids.contains(&test_event_id("payment-digest")), true);
        assert_eq!(event_ids.contains(&test_event_id("multi-a")), true);
        assert_eq!(event_ids.contains(&test_event_id("multi-b")), true);
        assert_eq!(event_ids.contains(&test_event_id("missing-request")), false);
        assert_eq!(event_ids.len(), 126);
    }
}

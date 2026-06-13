#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_core::{RadrootsCoreCurrency, RadrootsCoreDecimal};
use radroots_events::ids::{
    RadrootsEconomicsDigest, RadrootsEventId, RadrootsInventoryBinId, RadrootsListingAddress,
    RadrootsOrderId, RadrootsOrderQuoteId, RadrootsPublicKey,
};
use radroots_events::kinds::KIND_LISTING;
use radroots_events::order::{
    RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
    RadrootsOrderEconomics, RadrootsOrderFulfillmentState, RadrootsOrderFulfillmentUpdate,
    RadrootsOrderInventoryCommitment, RadrootsOrderItem, RadrootsOrderPaymentMethod,
    RadrootsOrderPaymentRecord as RadrootsOrderPaymentPayload, RadrootsOrderReceipt,
    RadrootsOrderRequest, RadrootsOrderRevisionDecision, RadrootsOrderRevisionOutcome,
    RadrootsOrderRevisionProposal, RadrootsOrderSettlementDecision, RadrootsOrderSettlementOutcome,
};
use radroots_events_codec::order::RadrootsOrderListingAddress as OrderListingAddress;
#[cfg(feature = "serde_json")]
use sha2::{Digest, Sha256};
use thiserror::Error;

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

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn reduce_order_events<I, J, K, L, M, N, O, P, Q>(
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
    reduce_order_event_records(
        order_id,
        requests.into_iter().collect(),
        decisions.into_iter().collect(),
        revision_proposals.into_iter().collect(),
        revision_decisions.into_iter().collect(),
        fulfillments.into_iter().collect(),
        cancellations.into_iter().collect(),
        receipts.into_iter().collect(),
        payments.into_iter().collect(),
        settlements.into_iter().collect(),
    )
}

fn reduce_order_event_records(
    order_id: &RadrootsOrderId,
    requests: Vec<RadrootsOrderRequestRecord>,
    decisions: Vec<RadrootsOrderDecisionRecord>,
    revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
    revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
    fulfillments: Vec<RadrootsOrderFulfillmentRecord>,
    cancellations: Vec<RadrootsOrderCancellationRecord>,
    receipts: Vec<RadrootsOrderReceiptRecord>,
    payments: Vec<RadrootsOrderPaymentEventRecord>,
    settlements: Vec<RadrootsOrderSettlementRecord>,
) -> RadrootsOrderProjection {
    let requests = unique_request_records(requests);
    let decisions = unique_decision_records(decisions);
    let revision_proposals = unique_revision_proposal_records(revision_proposals);
    let revision_decisions = unique_revision_decision_records(revision_decisions);
    let fulfillments = unique_fulfillment_records(fulfillments);
    let cancellations = unique_cancellation_records(cancellations);
    let receipts = unique_receipt_records(receipts);
    let payments = unique_payment_records(payments);
    let settlements = unique_settlement_records(settlements);
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
            valid_revision_proposals,
            valid_revision_decisions,
            fulfillments,
            valid_cancellations,
            valid_receipts,
            payments,
            settlements,
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
    reduce_listing_inventory_accounting_records(
        listing_addr,
        listing_event_id,
        bins.into_iter().collect(),
        requests.into_iter().collect(),
        decisions.into_iter().collect(),
        revision_proposals.into_iter().collect(),
        revision_decisions.into_iter().collect(),
        fulfillments.into_iter().collect(),
        cancellations.into_iter().collect(),
        receipts.into_iter().collect(),
    )
}

fn reduce_listing_inventory_accounting_records(
    listing_addr: &RadrootsListingAddress,
    listing_event_id: &RadrootsEventId,
    bins: Vec<RadrootsListingInventoryBinAvailability>,
    requests: Vec<RadrootsOrderRequestRecord>,
    decisions: Vec<RadrootsOrderDecisionRecord>,
    revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
    revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
    fulfillments: Vec<RadrootsOrderFulfillmentRecord>,
    cancellations: Vec<RadrootsOrderCancellationRecord>,
    receipts: Vec<RadrootsOrderReceiptRecord>,
) -> RadrootsListingInventoryAccountingProjection {
    let (mut bins, mut issues) = normalized_listing_inventory_bins(bins);
    let requests = unique_request_records(requests)
        .into_iter()
        .filter(|request| request.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let decisions = unique_decision_records(decisions)
        .into_iter()
        .filter(|decision| decision.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let revision_proposals = unique_revision_proposal_records(revision_proposals)
        .into_iter()
        .filter(|proposal| proposal.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let revision_decisions = unique_revision_decision_records(revision_decisions)
        .into_iter()
        .filter(|decision| decision.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let fulfillments = unique_fulfillment_records(fulfillments)
        .into_iter()
        .filter(|fulfillment| fulfillment.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let cancellations = unique_cancellation_records(cancellations)
        .into_iter()
        .filter(|cancellation| cancellation.payload.listing_addr.as_str() == listing_addr.as_str())
        .collect::<Vec<_>>();
    let receipts = unique_receipt_records(receipts)
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
            order_requests.clone(),
            order_decisions.clone(),
            order_revision_proposals.clone(),
            order_revision_decisions.clone(),
            order_fulfillments.clone(),
            order_cancellations.clone(),
            order_receipts.clone(),
            Vec::<RadrootsOrderPaymentEventRecord>::new(),
            Vec::<RadrootsOrderSettlementRecord>::new(),
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
    if seller_pubkey.as_str() != listing_addr.seller_pubkey {
        return Err(RadrootsOrderCanonicalizationError::InvalidSellerListing);
    }

    canonicalize_items(&mut request.items)?;
    request.economics.canonicalize();
    request.order_id = order_id;
    request.listing_addr =
        RadrootsListingAddress::parse(listing_addr.as_str()).map_err(|error| {
            RadrootsOrderCanonicalizationError::InvalidListingAddress(error.to_string())
        })?;
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
    if seller_pubkey.as_str() != signer_pubkey
        || seller_pubkey.as_str() != listing_addr.seller_pubkey
    {
        return Err(RadrootsOrderCanonicalizationError::InvalidSellerListing);
    }

    let buyer_pubkey = decision_event.buyer_pubkey.clone();
    canonicalize_decision(&mut decision_event.decision)?;

    decision_event.order_id = order_id;
    decision_event.listing_addr =
        RadrootsListingAddress::parse(listing_addr.as_str()).map_err(|error| {
            RadrootsOrderCanonicalizationError::InvalidListingAddress(error.to_string())
        })?;
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
    revision_proposals: Vec<RadrootsOrderRevisionProposalRecord>,
    revision_decisions: Vec<RadrootsOrderRevisionDecisionRecord>,
    fulfillments: Vec<RadrootsOrderFulfillmentRecord>,
    cancellations: Vec<RadrootsOrderCancellationRecord>,
    receipts: Vec<RadrootsOrderReceiptRecord>,
    payments: Vec<RadrootsOrderPaymentEventRecord>,
    settlements: Vec<RadrootsOrderSettlementRecord>,
) -> RadrootsOrderProjection {
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
                .cloned()
                .filter(|cancellation| {
                    cancellation.prev_event_id == revision_state.lifecycle_parent_event_id
                })
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
            let receipt_result = receipt_projection(
                order_id,
                request,
                decision,
                &revision_state.agreement_event_id,
                &revision_state.economics,
                latest.as_ref(),
                &fulfillment_records,
                receipts,
                &mut issues,
            );
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
    order_id: &RadrootsOrderId,
    request: &RadrootsOrderRequestRecord,
    decision: &RadrootsOrderDecisionRecord,
    agreement_event_id: &RadrootsEventId,
    economics: &RadrootsOrderEconomics,
    latest_fulfillment: Option<&RadrootsOrderFulfillmentRecord>,
    fulfillments: &[RadrootsOrderFulfillmentRecord],
    receipts: Vec<RadrootsOrderReceiptRecord>,
    issues: &mut Vec<RadrootsOrderIssue>,
) -> Option<RadrootsOrderProjection> {
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
        .cloned()
        .filter(|receipt| receipt.prev_event_id == fulfillment.event_id)
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
) -> Result<OrderListingAddress, RadrootsOrderCanonicalizationError> {
    let listing_addr = OrderListingAddress::parse(listing_addr_raw).map_err(|error| {
        RadrootsOrderCanonicalizationError::InvalidListingAddress(error.to_string())
    })?;
    if u32::from(listing_addr.kind) != KIND_LISTING {
        return Err(RadrootsOrderCanonicalizationError::InvalidListingKind);
    }
    Ok(listing_addr)
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
    use radroots_events::ids::{
        RadrootsEconomicsDigest, RadrootsEventId, RadrootsInventoryBinId, RadrootsListingAddress,
        RadrootsOrderId, RadrootsOrderQuoteId, RadrootsOrderRevisionId, RadrootsPublicKey,
    };
    use radroots_events::kinds::KIND_LISTING;
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

    use super::{
        RadrootsListingInventoryAccountingIssue, RadrootsListingInventoryAccountingProjection,
        RadrootsListingInventoryBinAccounting, RadrootsListingInventoryBinAvailability,
        RadrootsListingInventoryOrderReservation, RadrootsOrderCancellationRecord,
        RadrootsOrderCanonicalizationError, RadrootsOrderDecisionRecord,
        RadrootsOrderFulfillmentRecord, RadrootsOrderIssue, RadrootsOrderPaymentEventRecord,
        RadrootsOrderPaymentProjection, RadrootsOrderPaymentState, RadrootsOrderProjection,
        RadrootsOrderReceiptRecord, RadrootsOrderRequestRecord,
        RadrootsOrderRevisionDecisionRecord, RadrootsOrderRevisionProposalRecord,
        RadrootsOrderSettlementRecord, RadrootsOrderSettlementState, RadrootsOrderStatus,
        add_inventory_reservation, canonicalize_order_decision_for_signer,
        canonicalize_order_request_for_signer, inventory_issue_event_ids, inventory_issue_id,
        inventory_issue_rank, inventory_issue_sort_key, projection_issue_event_ids,
        radroots_order_economics_digest,
        reduce_listing_inventory_accounting as reduce_listing_inventory_accounting_with_revisions,
        reduce_order_events as reduce_order_events_with_revisions,
    };

    const SELLER: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const BUYER: &str = "2222222222222222222222222222222222222222222222222222222222222222";

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

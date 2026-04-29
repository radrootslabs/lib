#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::kinds::KIND_LISTING;
use radroots_events::trade::{
    RadrootsActiveTradeFulfillmentState, RadrootsTradeFulfillmentUpdated,
    RadrootsTradeInventoryCommitment, RadrootsTradeOrder as TradeOrder, RadrootsTradeOrderDecision,
    RadrootsTradeOrderDecisionEvent, RadrootsTradeOrderItem, RadrootsTradeOrderRequested,
};
use radroots_events_codec::trade::RadrootsTradeListingAddress as TradeListingAddress;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsTradeOrderCanonicalizationError {
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
pub struct RadrootsActiveOrderRequestRecord {
    pub event_id: String,
    pub author_pubkey: String,
    pub payload: RadrootsTradeOrderRequested,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsActiveOrderDecisionRecord {
    pub event_id: String,
    pub author_pubkey: String,
    pub counterparty_pubkey: String,
    pub root_event_id: String,
    pub prev_event_id: String,
    pub payload: RadrootsTradeOrderDecisionEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsActiveOrderFulfillmentRecord {
    pub event_id: String,
    pub author_pubkey: String,
    pub counterparty_pubkey: String,
    pub root_event_id: String,
    pub prev_event_id: String,
    pub payload: RadrootsTradeFulfillmentUpdated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsActiveOrderStatus {
    Missing,
    Requested,
    Accepted,
    Declined,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsActiveOrderReducerIssue {
    MissingRequest,
    MultipleRequests { event_ids: Vec<String> },
    RequestPayloadInvalid { event_id: String },
    RequestOrderIdMismatch { event_id: String },
    RequestAuthorMismatch { event_id: String },
    RequestListingAddressInvalid { event_id: String },
    RequestSellerListingMismatch { event_id: String },
    DecisionPayloadInvalid { event_id: String },
    DecisionOrderIdMismatch { event_id: String },
    DecisionAuthorMismatch { event_id: String },
    DecisionCounterpartyMismatch { event_id: String },
    DecisionBuyerMismatch { event_id: String },
    DecisionSellerMismatch { event_id: String },
    DecisionListingAddressInvalid { event_id: String },
    DecisionListingMismatch { event_id: String },
    DecisionRootMismatch { event_id: String },
    DecisionPreviousMismatch { event_id: String },
    DecisionMissingInventoryCommitments { event_id: String },
    DecisionInventoryCommitmentMismatch { event_id: String },
    DecisionMissingReason { event_id: String },
    ConflictingDecisions { event_ids: Vec<String> },
    FulfillmentWithoutAcceptedDecision { event_id: String },
    FulfillmentPayloadInvalid { event_id: String },
    FulfillmentOrderIdMismatch { event_id: String },
    FulfillmentAuthorMismatch { event_id: String },
    FulfillmentCounterpartyMismatch { event_id: String },
    FulfillmentBuyerMismatch { event_id: String },
    FulfillmentSellerMismatch { event_id: String },
    FulfillmentListingAddressInvalid { event_id: String },
    FulfillmentListingMismatch { event_id: String },
    FulfillmentRootMismatch { event_id: String },
    FulfillmentPreviousMismatch { event_id: String },
    FulfillmentStatusNotPublishable { event_id: String },
    FulfillmentUnsupportedTransition { event_id: String },
    ForkedFulfillments { event_ids: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsActiveOrderProjection {
    pub order_id: String,
    pub status: RadrootsActiveOrderStatus,
    pub request_event_id: Option<String>,
    pub decision_event_id: Option<String>,
    pub fulfillment_event_id: Option<String>,
    pub fulfillment_status: Option<RadrootsActiveTradeFulfillmentState>,
    pub listing_addr: Option<String>,
    pub buyer_pubkey: Option<String>,
    pub seller_pubkey: Option<String>,
    pub last_event_id: Option<String>,
    pub issues: Vec<RadrootsActiveOrderReducerIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingInventoryBinAvailability {
    pub bin_id: String,
    pub available_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingInventoryOrderReservation {
    pub order_id: String,
    pub decision_event_id: String,
    pub bin_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingInventoryBinAccounting {
    pub bin_id: String,
    pub available_count: u64,
    pub accepted_reserved_count: u64,
    pub remaining_count: u64,
    pub over_reserved: bool,
    pub accepted_orders: Vec<RadrootsListingInventoryOrderReservation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsListingInventoryAccountingIssue {
    InvalidActiveOrder {
        order_id: String,
        event_ids: Vec<String>,
    },
    ArithmeticOverflow {
        bin_id: String,
        event_ids: Vec<String>,
    },
    UnknownInventoryBin {
        bin_id: String,
        event_ids: Vec<String>,
    },
    OverReserved {
        bin_id: String,
        available_count: u64,
        reserved_count: u64,
        event_ids: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingInventoryAccountingProjection {
    pub listing_addr: String,
    pub listing_event_id: String,
    pub bins: Vec<RadrootsListingInventoryBinAccounting>,
    pub declined_order_ids: Vec<String>,
    pub invalid_event_ids: Vec<String>,
    pub issues: Vec<RadrootsListingInventoryAccountingIssue>,
}

pub fn reduce_active_order_events<I, J>(
    order_id: &str,
    requests: I,
    decisions: J,
    fulfillments: impl IntoIterator<Item = RadrootsActiveOrderFulfillmentRecord>,
) -> RadrootsActiveOrderProjection
where
    I: IntoIterator<Item = RadrootsActiveOrderRequestRecord>,
    J: IntoIterator<Item = RadrootsActiveOrderDecisionRecord>,
{
    let requests = unique_request_records(requests);
    let decisions = unique_decision_records(decisions);
    let fulfillments = unique_fulfillment_records(fulfillments);
    if requests.is_empty() && decisions.is_empty() && fulfillments.is_empty() {
        return RadrootsActiveOrderProjection {
            order_id: order_id.to_string(),
            status: RadrootsActiveOrderStatus::Missing,
            request_event_id: None,
            decision_event_id: None,
            fulfillment_event_id: None,
            fulfillment_status: None,
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
        if validate_active_request_record(order_id, &request, &mut issues) {
            valid_requests.push(request);
        }
    }

    if valid_requests.len() > 1 {
        let mut event_ids = valid_requests
            .iter()
            .map(|request| request.event_id.clone())
            .collect::<Vec<_>>();
        event_ids.sort();
        issues.push(RadrootsActiveOrderReducerIssue::MultipleRequests { event_ids });
    }

    let Some(request) = valid_requests.first() else {
        if decisions.is_empty() && fulfillments.is_empty() {
            return invalid_projection(order_id, None, issues);
        }
        issues.push(RadrootsActiveOrderReducerIssue::MissingRequest);
        return invalid_projection(order_id, None, issues);
    };

    if valid_requests.len() > 1 {
        return invalid_projection(order_id, Some(request), issues);
    }

    let mut valid_decisions = Vec::new();
    for decision in decisions {
        if validate_active_decision_record(request, &decision, &mut issues) {
            valid_decisions.push(decision);
        }
    }

    if !issues.is_empty() {
        return invalid_projection(order_id, Some(request), issues);
    }

    match valid_decisions.len() {
        0 => {
            if fulfillments.is_empty() {
                requested_projection(order_id, request)
            } else {
                record_fulfillment_without_accepted_decision(&fulfillments, &mut issues);
                invalid_projection(order_id, Some(request), issues)
            }
        }
        1 => decided_projection(order_id, request, &valid_decisions[0], fulfillments),
        _ => {
            let mut event_ids = valid_decisions
                .iter()
                .map(|decision| decision.event_id.clone())
                .collect::<Vec<_>>();
            event_ids.sort();
            invalid_projection(
                order_id,
                Some(request),
                vec![RadrootsActiveOrderReducerIssue::ConflictingDecisions { event_ids }],
            )
        }
    }
}

pub fn reduce_listing_inventory_accounting<I, J, K, L>(
    listing_addr: &str,
    listing_event_id: &str,
    bins: I,
    requests: J,
    decisions: K,
    fulfillments: L,
) -> RadrootsListingInventoryAccountingProjection
where
    I: IntoIterator<Item = RadrootsListingInventoryBinAvailability>,
    J: IntoIterator<Item = RadrootsActiveOrderRequestRecord>,
    K: IntoIterator<Item = RadrootsActiveOrderDecisionRecord>,
    L: IntoIterator<Item = RadrootsActiveOrderFulfillmentRecord>,
{
    let (mut bins, mut issues) = normalized_listing_inventory_bins(bins);
    let requests = unique_request_records(requests)
        .into_iter()
        .filter(|request| request.payload.listing_addr.trim() == listing_addr)
        .collect::<Vec<_>>();
    let decisions = unique_decision_records(decisions)
        .into_iter()
        .filter(|decision| decision.payload.listing_addr.trim() == listing_addr)
        .collect::<Vec<_>>();
    let fulfillments = unique_fulfillment_records(fulfillments)
        .into_iter()
        .filter(|fulfillment| fulfillment.payload.listing_addr.trim() == listing_addr)
        .collect::<Vec<_>>();
    let mut order_ids = listing_order_ids(&requests, &decisions, &fulfillments);
    let mut declined_order_ids = Vec::new();
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
        let projection = reduce_active_order_events(
            &order_id,
            order_requests.clone(),
            order_decisions.clone(),
            order_fulfillments.clone(),
        );
        match projection.status {
            RadrootsActiveOrderStatus::Accepted => {
                if projection.fulfillment_status
                    == Some(RadrootsActiveTradeFulfillmentState::SellerCancelled)
                {
                    continue;
                }
                if let Some(decision_event_id) = projection.decision_event_id.as_deref()
                    && let Some(decision) = order_decisions
                        .iter()
                        .find(|decision| decision.event_id == decision_event_id)
                {
                    add_accepted_inventory_reservations(
                        &mut bins,
                        &order_id,
                        decision,
                        &mut issues,
                    );
                }
            }
            RadrootsActiveOrderStatus::Declined => declined_order_ids.push(order_id),
            RadrootsActiveOrderStatus::Invalid => {
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
                    sort_and_dedup_strings(&mut event_ids);
                }
                invalid_event_ids.extend(event_ids.iter().cloned());
                issues.push(
                    RadrootsListingInventoryAccountingIssue::InvalidActiveOrder {
                        order_id,
                        event_ids,
                    },
                );
            }
            RadrootsActiveOrderStatus::Missing | RadrootsActiveOrderStatus::Requested => {}
        }
    }

    sort_and_dedup_strings(&mut declined_order_ids);
    sort_and_dedup_strings(&mut invalid_event_ids);
    finish_inventory_accounting_bins(&mut bins, &mut issues);
    issues.sort_by(inventory_issue_sort_key);
    RadrootsListingInventoryAccountingProjection {
        listing_addr: listing_addr.to_string(),
        listing_event_id: listing_event_id.to_string(),
        bins,
        declined_order_ids,
        invalid_event_ids,
        issues,
    }
}

pub fn canonicalize_order_request_for_signer(
    mut order: TradeOrder,
    signer_pubkey: &str,
) -> Result<TradeOrder, RadrootsTradeOrderCanonicalizationError> {
    let order_id = normalized_required_string(core::mem::take(&mut order.order_id), "order_id")?;
    let listing_addr_raw =
        normalized_required_string(core::mem::take(&mut order.listing_addr), "listing_addr")?;
    let listing_addr = TradeListingAddress::parse(&listing_addr_raw).map_err(|error| {
        RadrootsTradeOrderCanonicalizationError::InvalidListingAddress(error.to_string())
    })?;
    if u32::from(listing_addr.kind) != KIND_LISTING {
        return Err(RadrootsTradeOrderCanonicalizationError::InvalidListingKind);
    }

    let buyer_pubkey = if order.buyer_pubkey.trim().is_empty() {
        signer_pubkey.to_string()
    } else {
        normalized_required_string(core::mem::take(&mut order.buyer_pubkey), "buyer_pubkey")?
    };
    if buyer_pubkey != signer_pubkey {
        return Err(RadrootsTradeOrderCanonicalizationError::InvalidBuyerSigner);
    }

    let seller_pubkey = if order.seller_pubkey.trim().is_empty() {
        listing_addr.seller_pubkey.clone()
    } else {
        normalized_required_string(core::mem::take(&mut order.seller_pubkey), "seller_pubkey")?
    };
    if seller_pubkey != listing_addr.seller_pubkey {
        return Err(RadrootsTradeOrderCanonicalizationError::InvalidSellerListing);
    }

    if order.items.is_empty() {
        return Err(RadrootsTradeOrderCanonicalizationError::MissingItems);
    }
    for (index, item) in order.items.iter_mut().enumerate() {
        item.bin_id = normalized_required_string(item.bin_id.clone(), "bin_id")?;
        if item.bin_count == 0 {
            return Err(RadrootsTradeOrderCanonicalizationError::InvalidBinCount { index });
        }
    }

    order.order_id = order_id;
    order.listing_addr = listing_addr.as_str();
    order.buyer_pubkey = buyer_pubkey;
    order.seller_pubkey = seller_pubkey;
    if order.discounts.as_ref().is_some_and(Vec::is_empty) {
        order.discounts = None;
    }
    Ok(order)
}

pub fn canonicalize_active_order_request_for_signer(
    mut request: RadrootsTradeOrderRequested,
    signer_pubkey: &str,
) -> Result<RadrootsTradeOrderRequested, RadrootsTradeOrderCanonicalizationError> {
    let order_id = normalized_required_string(core::mem::take(&mut request.order_id), "order_id")?;
    let listing_addr_raw =
        normalized_required_string(core::mem::take(&mut request.listing_addr), "listing_addr")?;
    let listing_addr = parse_public_listing_addr(&listing_addr_raw)?;

    let buyer_pubkey = if request.buyer_pubkey.trim().is_empty() {
        normalized_required_string(signer_pubkey.to_string(), "buyer_pubkey")?
    } else {
        normalized_required_string(core::mem::take(&mut request.buyer_pubkey), "buyer_pubkey")?
    };
    if buyer_pubkey != signer_pubkey {
        return Err(RadrootsTradeOrderCanonicalizationError::InvalidBuyerSigner);
    }

    let seller_pubkey = if request.seller_pubkey.trim().is_empty() {
        listing_addr.seller_pubkey.clone()
    } else {
        normalized_required_string(core::mem::take(&mut request.seller_pubkey), "seller_pubkey")?
    };
    if seller_pubkey != listing_addr.seller_pubkey {
        return Err(RadrootsTradeOrderCanonicalizationError::InvalidSellerListing);
    }

    canonicalize_items(&mut request.items)?;
    request.order_id = order_id;
    request.listing_addr = listing_addr.as_str();
    request.buyer_pubkey = buyer_pubkey;
    request.seller_pubkey = seller_pubkey;
    Ok(request)
}

pub fn canonicalize_active_order_decision_for_signer(
    mut decision_event: RadrootsTradeOrderDecisionEvent,
    signer_pubkey: &str,
) -> Result<RadrootsTradeOrderDecisionEvent, RadrootsTradeOrderCanonicalizationError> {
    let order_id =
        normalized_required_string(core::mem::take(&mut decision_event.order_id), "order_id")?;
    let listing_addr_raw = normalized_required_string(
        core::mem::take(&mut decision_event.listing_addr),
        "listing_addr",
    )?;
    let listing_addr = parse_public_listing_addr(&listing_addr_raw)?;

    let seller_pubkey = if decision_event.seller_pubkey.trim().is_empty() {
        normalized_required_string(signer_pubkey.to_string(), "seller_pubkey")?
    } else {
        normalized_required_string(
            core::mem::take(&mut decision_event.seller_pubkey),
            "seller_pubkey",
        )?
    };
    if seller_pubkey != signer_pubkey || seller_pubkey != listing_addr.seller_pubkey {
        return Err(RadrootsTradeOrderCanonicalizationError::InvalidSellerListing);
    }

    let buyer_pubkey = normalized_required_string(
        core::mem::take(&mut decision_event.buyer_pubkey),
        "buyer_pubkey",
    )?;
    canonicalize_decision(&mut decision_event.decision)?;

    decision_event.order_id = order_id;
    decision_event.listing_addr = listing_addr.as_str();
    decision_event.buyer_pubkey = buyer_pubkey;
    decision_event.seller_pubkey = seller_pubkey;
    Ok(decision_event)
}

fn unique_request_records<I>(requests: I) -> Vec<RadrootsActiveOrderRequestRecord>
where
    I: IntoIterator<Item = RadrootsActiveOrderRequestRecord>,
{
    let mut unique = Vec::new();
    let mut records = requests.into_iter().collect::<Vec<_>>();
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for request in records {
        if unique
            .iter()
            .all(|existing: &RadrootsActiveOrderRequestRecord| {
                existing.event_id != request.event_id
            })
        {
            unique.push(request);
        }
    }
    unique
}

fn unique_decision_records<I>(decisions: I) -> Vec<RadrootsActiveOrderDecisionRecord>
where
    I: IntoIterator<Item = RadrootsActiveOrderDecisionRecord>,
{
    let mut unique = Vec::new();
    let mut records = decisions.into_iter().collect::<Vec<_>>();
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for decision in records {
        if unique
            .iter()
            .all(|existing: &RadrootsActiveOrderDecisionRecord| {
                existing.event_id != decision.event_id
            })
        {
            unique.push(decision);
        }
    }
    unique
}

fn unique_fulfillment_records<I>(fulfillments: I) -> Vec<RadrootsActiveOrderFulfillmentRecord>
where
    I: IntoIterator<Item = RadrootsActiveOrderFulfillmentRecord>,
{
    let mut unique = Vec::new();
    let mut records = fulfillments.into_iter().collect::<Vec<_>>();
    records.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for fulfillment in records {
        if unique
            .iter()
            .all(|existing: &RadrootsActiveOrderFulfillmentRecord| {
                existing.event_id != fulfillment.event_id
            })
        {
            unique.push(fulfillment);
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
        let bin_id = bin.bin_id.trim();
        if bin_id.is_empty() {
            continue;
        }
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
                bin_id: bin_id.to_string(),
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
    requests: &[RadrootsActiveOrderRequestRecord],
    decisions: &[RadrootsActiveOrderDecisionRecord],
    fulfillments: &[RadrootsActiveOrderFulfillmentRecord],
) -> Vec<String> {
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
        fulfillments
            .iter()
            .map(|fulfillment| fulfillment.payload.order_id.clone()),
    );
    sort_and_dedup_strings(&mut order_ids);
    order_ids
}

fn add_accepted_inventory_reservations(
    bins: &mut [RadrootsListingInventoryBinAccounting],
    order_id: &str,
    decision: &RadrootsActiveOrderDecisionRecord,
    issues: &mut Vec<RadrootsListingInventoryAccountingIssue>,
) {
    let RadrootsTradeOrderDecision::Accepted {
        inventory_commitments,
    } = &decision.payload.decision
    else {
        return;
    };
    let Some(commitments) = normalized_inventory_commitment_counts(inventory_commitments) else {
        issues.push(
            RadrootsListingInventoryAccountingIssue::InvalidActiveOrder {
                order_id: order_id.to_string(),
                event_ids: vec![decision.event_id.clone()],
            },
        );
        return;
    };
    for commitment in commitments {
        if let Some(bin) = bins.iter_mut().find(|bin| bin.bin_id == commitment.bin_id) {
            add_inventory_reservation(bin, order_id, decision, commitment.bin_count, issues);
        } else {
            issues.push(
                RadrootsListingInventoryAccountingIssue::UnknownInventoryBin {
                    bin_id: commitment.bin_id,
                    event_ids: vec![decision.event_id.clone()],
                },
            );
        }
    }
}

fn add_inventory_reservation(
    bin: &mut RadrootsListingInventoryBinAccounting,
    order_id: &str,
    decision: &RadrootsActiveOrderDecisionRecord,
    bin_count: u64,
    issues: &mut Vec<RadrootsListingInventoryAccountingIssue>,
) {
    if let Some(next_count) = bin.accepted_reserved_count.checked_add(bin_count) {
        bin.accepted_reserved_count = next_count;
        bin.accepted_orders
            .push(RadrootsListingInventoryOrderReservation {
                order_id: order_id.to_string(),
                decision_event_id: decision.event_id.clone(),
                bin_count,
            });
    } else {
        issues.push(
            RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                bin_id: bin.bin_id.clone(),
                event_ids: vec![decision.event_id.clone()],
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
            sort_and_dedup_strings(&mut event_ids);
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

fn projection_issue_event_ids(issues: &[RadrootsActiveOrderReducerIssue]) -> Vec<String> {
    let mut event_ids = Vec::new();
    for issue in issues {
        match issue {
            RadrootsActiveOrderReducerIssue::MissingRequest => {}
            RadrootsActiveOrderReducerIssue::MultipleRequests { event_ids: ids }
            | RadrootsActiveOrderReducerIssue::ConflictingDecisions { event_ids: ids } => {
                event_ids.extend(ids.iter().cloned());
            }
            RadrootsActiveOrderReducerIssue::RequestPayloadInvalid { event_id }
            | RadrootsActiveOrderReducerIssue::RequestOrderIdMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::RequestAuthorMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::RequestListingAddressInvalid { event_id }
            | RadrootsActiveOrderReducerIssue::RequestSellerListingMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionPayloadInvalid { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionOrderIdMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionAuthorMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionCounterpartyMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionBuyerMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionSellerMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionListingAddressInvalid { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionListingMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionRootMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionPreviousMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionMissingInventoryCommitments { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionInventoryCommitmentMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::DecisionMissingReason { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentWithoutAcceptedDecision { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentPayloadInvalid { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentOrderIdMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentAuthorMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentCounterpartyMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentBuyerMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentSellerMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentListingAddressInvalid { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentListingMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentRootMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentPreviousMismatch { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentStatusNotPublishable { event_id }
            | RadrootsActiveOrderReducerIssue::FulfillmentUnsupportedTransition { event_id } => {
                event_ids.push(event_id.clone());
            }
            RadrootsActiveOrderReducerIssue::ForkedFulfillments { event_ids: ids } => {
                event_ids.extend(ids.iter().cloned());
            }
        }
    }
    sort_and_dedup_strings(&mut event_ids);
    event_ids
}

fn sort_and_dedup_strings(values: &mut Vec<String>) {
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
        RadrootsListingInventoryAccountingIssue::InvalidActiveOrder { .. } => 0,
        RadrootsListingInventoryAccountingIssue::ArithmeticOverflow { .. } => 1,
        RadrootsListingInventoryAccountingIssue::UnknownInventoryBin { .. } => 2,
        RadrootsListingInventoryAccountingIssue::OverReserved { .. } => 3,
    }
}

fn inventory_issue_id(issue: &RadrootsListingInventoryAccountingIssue) -> &str {
    match issue {
        RadrootsListingInventoryAccountingIssue::InvalidActiveOrder { order_id, .. } => order_id,
        RadrootsListingInventoryAccountingIssue::ArithmeticOverflow { bin_id, .. }
        | RadrootsListingInventoryAccountingIssue::UnknownInventoryBin { bin_id, .. }
        | RadrootsListingInventoryAccountingIssue::OverReserved { bin_id, .. } => bin_id,
    }
}

fn inventory_issue_event_ids(issue: &RadrootsListingInventoryAccountingIssue) -> &[String] {
    match issue {
        RadrootsListingInventoryAccountingIssue::InvalidActiveOrder { event_ids, .. }
        | RadrootsListingInventoryAccountingIssue::ArithmeticOverflow { event_ids, .. }
        | RadrootsListingInventoryAccountingIssue::UnknownInventoryBin { event_ids, .. }
        | RadrootsListingInventoryAccountingIssue::OverReserved { event_ids, .. } => event_ids,
    }
}

fn validate_active_request_record(
    order_id: &str,
    request: &RadrootsActiveOrderRequestRecord,
    issues: &mut Vec<RadrootsActiveOrderReducerIssue>,
) -> bool {
    let mut valid = true;
    if request.payload.validate().is_err() {
        issues.push(RadrootsActiveOrderReducerIssue::RequestPayloadInvalid {
            event_id: request.event_id.clone(),
        });
        valid = false;
    }
    if request.payload.order_id != order_id {
        issues.push(RadrootsActiveOrderReducerIssue::RequestOrderIdMismatch {
            event_id: request.event_id.clone(),
        });
        valid = false;
    }
    if request.author_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsActiveOrderReducerIssue::RequestAuthorMismatch {
            event_id: request.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&request.payload.listing_addr) {
        Ok(listing_addr) => {
            if listing_addr.seller_pubkey != request.payload.seller_pubkey {
                issues.push(
                    RadrootsActiveOrderReducerIssue::RequestSellerListingMismatch {
                        event_id: request.event_id.clone(),
                    },
                );
                valid = false;
            }
        }
        Err(_) => {
            issues.push(
                RadrootsActiveOrderReducerIssue::RequestListingAddressInvalid {
                    event_id: request.event_id.clone(),
                },
            );
            valid = false;
        }
    }
    valid
}

fn validate_active_decision_record(
    request: &RadrootsActiveOrderRequestRecord,
    decision: &RadrootsActiveOrderDecisionRecord,
    issues: &mut Vec<RadrootsActiveOrderReducerIssue>,
) -> bool {
    let mut valid = true;
    if decision_payload_issue(&decision.payload.decision, &decision.event_id, issues) {
        valid = false;
    }
    if decision.payload.validate().is_err() {
        issues.push(RadrootsActiveOrderReducerIssue::DecisionPayloadInvalid {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.payload.order_id != request.payload.order_id {
        issues.push(RadrootsActiveOrderReducerIssue::DecisionOrderIdMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.author_pubkey != decision.payload.seller_pubkey {
        issues.push(RadrootsActiveOrderReducerIssue::DecisionAuthorMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.counterparty_pubkey != request.payload.buyer_pubkey {
        issues.push(
            RadrootsActiveOrderReducerIssue::DecisionCounterpartyMismatch {
                event_id: decision.event_id.clone(),
            },
        );
        valid = false;
    }
    if decision.payload.buyer_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsActiveOrderReducerIssue::DecisionBuyerMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.payload.seller_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsActiveOrderReducerIssue::DecisionSellerMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&decision.payload.listing_addr) {
        Ok(listing_addr) => {
            if decision.payload.listing_addr != request.payload.listing_addr
                || listing_addr.seller_pubkey != decision.payload.seller_pubkey
            {
                issues.push(RadrootsActiveOrderReducerIssue::DecisionListingMismatch {
                    event_id: decision.event_id.clone(),
                });
                valid = false;
            }
        }
        Err(_) => {
            issues.push(
                RadrootsActiveOrderReducerIssue::DecisionListingAddressInvalid {
                    event_id: decision.event_id.clone(),
                },
            );
            valid = false;
        }
    }
    if decision.root_event_id != request.event_id {
        issues.push(RadrootsActiveOrderReducerIssue::DecisionRootMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if decision.prev_event_id != request.event_id {
        issues.push(RadrootsActiveOrderReducerIssue::DecisionPreviousMismatch {
            event_id: decision.event_id.clone(),
        });
        valid = false;
    }
    if let RadrootsTradeOrderDecision::Accepted {
        inventory_commitments,
    } = &decision.payload.decision
        && decision.payload.validate().is_ok()
        && !inventory_commitments_match_request(&request.payload.items, inventory_commitments)
    {
        issues.push(
            RadrootsActiveOrderReducerIssue::DecisionInventoryCommitmentMismatch {
                event_id: decision.event_id.clone(),
            },
        );
        valid = false;
    }
    valid
}

fn validate_active_fulfillment_record(
    request: &RadrootsActiveOrderRequestRecord,
    fulfillment: &RadrootsActiveOrderFulfillmentRecord,
    issues: &mut Vec<RadrootsActiveOrderReducerIssue>,
) -> bool {
    let mut valid = true;
    if !fulfillment.payload.status.is_publishable_update() {
        issues.push(
            RadrootsActiveOrderReducerIssue::FulfillmentStatusNotPublishable {
                event_id: fulfillment.event_id.clone(),
            },
        );
        valid = false;
    }
    if fulfillment.payload.validate().is_err() {
        issues.push(RadrootsActiveOrderReducerIssue::FulfillmentPayloadInvalid {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.payload.order_id != request.payload.order_id {
        issues.push(
            RadrootsActiveOrderReducerIssue::FulfillmentOrderIdMismatch {
                event_id: fulfillment.event_id.clone(),
            },
        );
        valid = false;
    }
    if fulfillment.author_pubkey != fulfillment.payload.seller_pubkey {
        issues.push(RadrootsActiveOrderReducerIssue::FulfillmentAuthorMismatch {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.counterparty_pubkey != request.payload.buyer_pubkey {
        issues.push(
            RadrootsActiveOrderReducerIssue::FulfillmentCounterpartyMismatch {
                event_id: fulfillment.event_id.clone(),
            },
        );
        valid = false;
    }
    if fulfillment.payload.buyer_pubkey != request.payload.buyer_pubkey {
        issues.push(RadrootsActiveOrderReducerIssue::FulfillmentBuyerMismatch {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.payload.seller_pubkey != request.payload.seller_pubkey {
        issues.push(RadrootsActiveOrderReducerIssue::FulfillmentSellerMismatch {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    match parse_public_listing_addr(&fulfillment.payload.listing_addr) {
        Ok(listing_addr) => {
            if fulfillment.payload.listing_addr != request.payload.listing_addr
                || listing_addr.seller_pubkey != fulfillment.payload.seller_pubkey
            {
                issues.push(
                    RadrootsActiveOrderReducerIssue::FulfillmentListingMismatch {
                        event_id: fulfillment.event_id.clone(),
                    },
                );
                valid = false;
            }
        }
        Err(_) => {
            issues.push(
                RadrootsActiveOrderReducerIssue::FulfillmentListingAddressInvalid {
                    event_id: fulfillment.event_id.clone(),
                },
            );
            valid = false;
        }
    }
    if fulfillment.root_event_id != request.event_id {
        issues.push(RadrootsActiveOrderReducerIssue::FulfillmentRootMismatch {
            event_id: fulfillment.event_id.clone(),
        });
        valid = false;
    }
    if fulfillment.prev_event_id.trim().is_empty()
        || fulfillment.prev_event_id == fulfillment.event_id
    {
        issues.push(
            RadrootsActiveOrderReducerIssue::FulfillmentPreviousMismatch {
                event_id: fulfillment.event_id.clone(),
            },
        );
        valid = false;
    }
    valid
}

fn decision_payload_issue(
    decision: &RadrootsTradeOrderDecision,
    event_id: &str,
    issues: &mut Vec<RadrootsActiveOrderReducerIssue>,
) -> bool {
    match decision {
        RadrootsTradeOrderDecision::Accepted {
            inventory_commitments,
        } => {
            if inventory_commitments.is_empty() {
                issues.push(
                    RadrootsActiveOrderReducerIssue::DecisionMissingInventoryCommitments {
                        event_id: event_id.to_string(),
                    },
                );
                true
            } else {
                false
            }
        }
        RadrootsTradeOrderDecision::Declined { reason } => {
            if reason.trim().is_empty() {
                issues.push(RadrootsActiveOrderReducerIssue::DecisionMissingReason {
                    event_id: event_id.to_string(),
                });
                true
            } else {
                false
            }
        }
    }
}

fn record_fulfillment_without_accepted_decision(
    fulfillments: &[RadrootsActiveOrderFulfillmentRecord],
    issues: &mut Vec<RadrootsActiveOrderReducerIssue>,
) {
    for fulfillment in fulfillments {
        issues.push(
            RadrootsActiveOrderReducerIssue::FulfillmentWithoutAcceptedDecision {
                event_id: fulfillment.event_id.clone(),
            },
        );
    }
}

fn latest_fulfillment_record(
    request: &RadrootsActiveOrderRequestRecord,
    decision: &RadrootsActiveOrderDecisionRecord,
    fulfillments: Vec<RadrootsActiveOrderFulfillmentRecord>,
    issues: &mut Vec<RadrootsActiveOrderReducerIssue>,
) -> Option<RadrootsActiveOrderFulfillmentRecord> {
    let mut valid_fulfillments = Vec::new();
    for fulfillment in fulfillments {
        if validate_active_fulfillment_record(request, &fulfillment, issues) {
            valid_fulfillments.push(fulfillment);
        }
    }
    if !issues.is_empty() {
        return None;
    }
    let mut used_event_ids = Vec::new();
    let mut previous_event_id = decision.event_id.clone();
    let mut previous_status = RadrootsActiveTradeFulfillmentState::AcceptedNotFulfilled;
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
            issues.push(RadrootsActiveOrderReducerIssue::ForkedFulfillments { event_ids });
            return None;
        }
        let child = children[0];
        if matches!(
            previous_status,
            RadrootsActiveTradeFulfillmentState::Delivered
                | RadrootsActiveTradeFulfillmentState::SellerCancelled
        ) {
            issues.push(
                RadrootsActiveOrderReducerIssue::FulfillmentUnsupportedTransition {
                    event_id: child.event_id.clone(),
                },
            );
            return None;
        }
        used_event_ids.push(child.event_id.clone());
        previous_event_id = child.event_id.clone();
        previous_status = child.payload.status;
        latest = Some((*child).clone());
    }

    for fulfillment in valid_fulfillments {
        if !used_event_ids.contains(&fulfillment.event_id) {
            issues.push(
                RadrootsActiveOrderReducerIssue::FulfillmentPreviousMismatch {
                    event_id: fulfillment.event_id,
                },
            );
        }
    }
    latest
}

fn requested_projection(
    order_id: &str,
    request: &RadrootsActiveOrderRequestRecord,
) -> RadrootsActiveOrderProjection {
    RadrootsActiveOrderProjection {
        order_id: order_id.to_string(),
        status: RadrootsActiveOrderStatus::Requested,
        request_event_id: Some(request.event_id.clone()),
        decision_event_id: None,
        fulfillment_event_id: None,
        fulfillment_status: None,
        listing_addr: Some(request.payload.listing_addr.clone()),
        buyer_pubkey: Some(request.payload.buyer_pubkey.clone()),
        seller_pubkey: Some(request.payload.seller_pubkey.clone()),
        last_event_id: Some(request.event_id.clone()),
        issues: Vec::new(),
    }
}

fn decided_projection(
    order_id: &str,
    request: &RadrootsActiveOrderRequestRecord,
    decision: &RadrootsActiveOrderDecisionRecord,
    fulfillments: Vec<RadrootsActiveOrderFulfillmentRecord>,
) -> RadrootsActiveOrderProjection {
    let status = match &decision.payload.decision {
        RadrootsTradeOrderDecision::Accepted { .. } => RadrootsActiveOrderStatus::Accepted,
        RadrootsTradeOrderDecision::Declined { .. } => RadrootsActiveOrderStatus::Declined,
    };
    let mut issues = Vec::new();
    let (fulfillment_event_id, fulfillment_status, last_event_id) = match status {
        RadrootsActiveOrderStatus::Accepted => {
            let latest = latest_fulfillment_record(request, decision, fulfillments, &mut issues);
            if !issues.is_empty() {
                return invalid_projection(order_id, Some(request), issues);
            }
            match latest {
                Some(fulfillment) => (
                    Some(fulfillment.event_id.clone()),
                    Some(fulfillment.payload.status),
                    Some(fulfillment.event_id),
                ),
                None => (
                    None,
                    Some(RadrootsActiveTradeFulfillmentState::AcceptedNotFulfilled),
                    Some(decision.event_id.clone()),
                ),
            }
        }
        RadrootsActiveOrderStatus::Declined => {
            if fulfillments.is_empty() {
                (None, None, Some(decision.event_id.clone()))
            } else {
                record_fulfillment_without_accepted_decision(&fulfillments, &mut issues);
                return invalid_projection(order_id, Some(request), issues);
            }
        }
        _ => (None, None, Some(decision.event_id.clone())),
    };
    RadrootsActiveOrderProjection {
        order_id: order_id.to_string(),
        status,
        request_event_id: Some(request.event_id.clone()),
        decision_event_id: Some(decision.event_id.clone()),
        fulfillment_event_id,
        fulfillment_status,
        listing_addr: Some(request.payload.listing_addr.clone()),
        buyer_pubkey: Some(request.payload.buyer_pubkey.clone()),
        seller_pubkey: Some(request.payload.seller_pubkey.clone()),
        last_event_id,
        issues: Vec::new(),
    }
}

fn invalid_projection(
    order_id: &str,
    request: Option<&RadrootsActiveOrderRequestRecord>,
    issues: Vec<RadrootsActiveOrderReducerIssue>,
) -> RadrootsActiveOrderProjection {
    RadrootsActiveOrderProjection {
        order_id: order_id.to_string(),
        status: RadrootsActiveOrderStatus::Invalid,
        request_event_id: request.map(|request| request.event_id.clone()),
        decision_event_id: None,
        fulfillment_event_id: None,
        fulfillment_status: None,
        listing_addr: request.map(|request| request.payload.listing_addr.clone()),
        buyer_pubkey: request.map(|request| request.payload.buyer_pubkey.clone()),
        seller_pubkey: request.map(|request| request.payload.seller_pubkey.clone()),
        last_event_id: request.map(|request| request.event_id.clone()),
        issues,
    }
}

fn parse_public_listing_addr(
    listing_addr_raw: &str,
) -> Result<TradeListingAddress, RadrootsTradeOrderCanonicalizationError> {
    let listing_addr = TradeListingAddress::parse(listing_addr_raw).map_err(|error| {
        RadrootsTradeOrderCanonicalizationError::InvalidListingAddress(error.to_string())
    })?;
    if u32::from(listing_addr.kind) != KIND_LISTING {
        return Err(RadrootsTradeOrderCanonicalizationError::InvalidListingKind);
    }
    Ok(listing_addr)
}

fn canonicalize_items(
    items: &mut [RadrootsTradeOrderItem],
) -> Result<(), RadrootsTradeOrderCanonicalizationError> {
    if items.is_empty() {
        return Err(RadrootsTradeOrderCanonicalizationError::MissingItems);
    }
    for (index, item) in items.iter_mut().enumerate() {
        item.bin_id = normalized_required_string(item.bin_id.clone(), "bin_id")?;
        if item.bin_count == 0 {
            return Err(RadrootsTradeOrderCanonicalizationError::InvalidBinCount { index });
        }
    }
    Ok(())
}

fn canonicalize_decision(
    decision: &mut RadrootsTradeOrderDecision,
) -> Result<(), RadrootsTradeOrderCanonicalizationError> {
    match decision {
        RadrootsTradeOrderDecision::Accepted {
            inventory_commitments,
        } => canonicalize_inventory_commitments(inventory_commitments),
        RadrootsTradeOrderDecision::Declined { reason } => {
            *reason = normalized_required_string(core::mem::take(reason), "reason")?;
            Ok(())
        }
    }
}

fn canonicalize_inventory_commitments(
    commitments: &mut [RadrootsTradeInventoryCommitment],
) -> Result<(), RadrootsTradeOrderCanonicalizationError> {
    if commitments.is_empty() {
        return Err(RadrootsTradeOrderCanonicalizationError::MissingInventoryCommitments);
    }
    for (index, commitment) in commitments.iter_mut().enumerate() {
        commitment.bin_id = normalized_required_string(commitment.bin_id.clone(), "bin_id")?;
        if commitment.bin_count == 0 {
            return Err(
                RadrootsTradeOrderCanonicalizationError::InvalidInventoryCommitmentCount { index },
            );
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct NormalizedInventoryCount {
    bin_id: String,
    bin_count: u64,
}

fn inventory_commitments_match_request(
    request_items: &[RadrootsTradeOrderItem],
    inventory_commitments: &[RadrootsTradeInventoryCommitment],
) -> bool {
    normalized_request_item_counts(request_items)
        == normalized_inventory_commitment_counts(inventory_commitments)
}

fn normalized_request_item_counts(
    items: &[RadrootsTradeOrderItem],
) -> Option<Vec<NormalizedInventoryCount>> {
    let mut counts = Vec::new();
    for item in items {
        push_normalized_inventory_count(&mut counts, &item.bin_id, item.bin_count)?;
    }
    counts.sort_by(|left, right| left.bin_id.cmp(&right.bin_id));
    Some(counts)
}

fn normalized_inventory_commitment_counts(
    commitments: &[RadrootsTradeInventoryCommitment],
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
    bin_id: &str,
    bin_count: u32,
) -> Option<()> {
    let bin_id = bin_id.trim();
    if bin_id.is_empty() || bin_count == 0 {
        return None;
    }
    if let Some(existing) = counts.iter_mut().find(|count| count.bin_id == bin_id) {
        existing.bin_count = existing.bin_count.checked_add(u64::from(bin_count))?;
    } else {
        counts.push(NormalizedInventoryCount {
            bin_id: bin_id.to_string(),
            bin_count: u64::from(bin_count),
        });
    }
    Some(())
}

fn normalized_required_string(
    value: String,
    field: &'static str,
) -> Result<String, RadrootsTradeOrderCanonicalizationError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(RadrootsTradeOrderCanonicalizationError::EmptyField(field));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use radroots_events::kinds::KIND_LISTING;
    use radroots_events::trade::{
        RadrootsActiveTradeFulfillmentState, RadrootsTradeFulfillmentUpdated,
        RadrootsTradeInventoryCommitment, RadrootsTradeOrder as TradeOrder,
        RadrootsTradeOrderDecision, RadrootsTradeOrderDecisionEvent, RadrootsTradeOrderItem,
        RadrootsTradeOrderRequested,
    };

    use super::{
        RadrootsActiveOrderDecisionRecord, RadrootsActiveOrderFulfillmentRecord,
        RadrootsActiveOrderReducerIssue, RadrootsActiveOrderRequestRecord,
        RadrootsActiveOrderStatus, RadrootsListingInventoryAccountingIssue,
        RadrootsListingInventoryBinAccounting, RadrootsListingInventoryBinAvailability,
        RadrootsListingInventoryOrderReservation, RadrootsTradeOrderCanonicalizationError,
        add_inventory_reservation, canonicalize_active_order_decision_for_signer,
        canonicalize_active_order_request_for_signer, canonicalize_order_request_for_signer,
        reduce_active_order_events, reduce_listing_inventory_accounting,
    };

    const SELLER: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const BUYER: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    fn base_order(buyer_pubkey: &str, seller_pubkey: &str) -> TradeOrder {
        TradeOrder {
            order_id: "order-1".to_string(),
            listing_addr: format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"),
            buyer_pubkey: buyer_pubkey.to_string(),
            seller_pubkey: seller_pubkey.to_string(),
            items: vec![RadrootsTradeOrderItem {
                bin_id: "bin-1".to_string(),
                bin_count: 1,
            }],
            discounts: None,
        }
    }

    fn active_request(buyer_pubkey: &str, seller_pubkey: &str) -> RadrootsTradeOrderRequested {
        RadrootsTradeOrderRequested {
            order_id: " order-1 ".to_string(),
            listing_addr: format!(" {KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg "),
            buyer_pubkey: buyer_pubkey.to_string(),
            seller_pubkey: seller_pubkey.to_string(),
            items: vec![RadrootsTradeOrderItem {
                bin_id: " bin-1 ".to_string(),
                bin_count: 2,
            }],
        }
    }

    fn active_decision(seller_pubkey: &str) -> RadrootsTradeOrderDecisionEvent {
        RadrootsTradeOrderDecisionEvent {
            order_id: " order-1 ".to_string(),
            listing_addr: format!(" {KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg "),
            buyer_pubkey: format!(" {BUYER} "),
            seller_pubkey: seller_pubkey.to_string(),
            decision: RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: vec![RadrootsTradeInventoryCommitment {
                    bin_id: " bin-1 ".to_string(),
                    bin_count: 2,
                }],
            },
        }
    }

    fn listing_addr() -> String {
        format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
    }

    fn clean_request_payload() -> RadrootsTradeOrderRequested {
        RadrootsTradeOrderRequested {
            order_id: "order-1".to_string(),
            listing_addr: listing_addr(),
            buyer_pubkey: BUYER.to_string(),
            seller_pubkey: SELLER.to_string(),
            items: vec![RadrootsTradeOrderItem {
                bin_id: "bin-1".to_string(),
                bin_count: 2,
            }],
        }
    }

    fn request_record_with_event_id(event_id: &str) -> RadrootsActiveOrderRequestRecord {
        RadrootsActiveOrderRequestRecord {
            event_id: event_id.to_string(),
            author_pubkey: BUYER.to_string(),
            payload: clean_request_payload(),
        }
    }

    fn request_record() -> RadrootsActiveOrderRequestRecord {
        request_record_with_event_id("request-1")
    }

    fn request_record_for(
        order_id: &str,
        event_id: &str,
        bin_count: u32,
    ) -> RadrootsActiveOrderRequestRecord {
        let mut request = request_record_with_event_id(event_id);
        request.payload.order_id = order_id.to_string();
        request.payload.items[0].bin_count = bin_count;
        request
    }

    fn decision_payload(decision: RadrootsTradeOrderDecision) -> RadrootsTradeOrderDecisionEvent {
        RadrootsTradeOrderDecisionEvent {
            order_id: "order-1".to_string(),
            listing_addr: listing_addr(),
            buyer_pubkey: BUYER.to_string(),
            seller_pubkey: SELLER.to_string(),
            decision,
        }
    }

    fn accepted_decision_record(event_id: &str) -> RadrootsActiveOrderDecisionRecord {
        RadrootsActiveOrderDecisionRecord {
            event_id: event_id.to_string(),
            author_pubkey: SELLER.to_string(),
            counterparty_pubkey: BUYER.to_string(),
            root_event_id: "request-1".to_string(),
            prev_event_id: "request-1".to_string(),
            payload: decision_payload(RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: vec![RadrootsTradeInventoryCommitment {
                    bin_id: "bin-1".to_string(),
                    bin_count: 2,
                }],
            }),
        }
    }

    fn declined_decision_record(event_id: &str) -> RadrootsActiveOrderDecisionRecord {
        RadrootsActiveOrderDecisionRecord {
            event_id: event_id.to_string(),
            author_pubkey: SELLER.to_string(),
            counterparty_pubkey: BUYER.to_string(),
            root_event_id: "request-1".to_string(),
            prev_event_id: "request-1".to_string(),
            payload: decision_payload(RadrootsTradeOrderDecision::Declined {
                reason: "out_of_stock".to_string(),
            }),
        }
    }

    fn fulfillment_record(
        event_id: &str,
        prev_event_id: &str,
        status: RadrootsActiveTradeFulfillmentState,
    ) -> RadrootsActiveOrderFulfillmentRecord {
        RadrootsActiveOrderFulfillmentRecord {
            event_id: event_id.to_string(),
            author_pubkey: SELLER.to_string(),
            counterparty_pubkey: BUYER.to_string(),
            root_event_id: "request-1".to_string(),
            prev_event_id: prev_event_id.to_string(),
            payload: RadrootsTradeFulfillmentUpdated {
                order_id: "order-1".to_string(),
                listing_addr: listing_addr(),
                buyer_pubkey: BUYER.to_string(),
                seller_pubkey: SELLER.to_string(),
                status,
            },
        }
    }

    fn accepted_decision_record_for(
        order_id: &str,
        event_id: &str,
        request_event_id: &str,
        bin_count: u32,
    ) -> RadrootsActiveOrderDecisionRecord {
        let mut decision = accepted_decision_record(event_id);
        decision.root_event_id = request_event_id.to_string();
        decision.prev_event_id = request_event_id.to_string();
        decision.payload.order_id = order_id.to_string();
        let RadrootsTradeOrderDecision::Accepted {
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
            bin_id: "bin-1".to_string(),
            available_count,
        }
    }

    #[test]
    fn canonicalize_order_request_sets_missing_pubkeys() {
        let order = canonicalize_order_request_for_signer(base_order("", ""), SELLER)
            .expect("canonical order");

        assert_eq!(order.buyer_pubkey, SELLER);
        assert_eq!(order.seller_pubkey, SELLER);
    }

    #[test]
    fn canonicalize_active_order_request_sets_authority_and_trims_items() {
        let request =
            canonicalize_active_order_request_for_signer(active_request("", ""), BUYER).unwrap();

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
    fn canonicalize_active_order_request_rejects_wrong_buyer_signer() {
        let error = canonicalize_active_order_request_for_signer(active_request(SELLER, ""), BUYER)
            .unwrap_err();

        assert!(matches!(
            error,
            RadrootsTradeOrderCanonicalizationError::InvalidBuyerSigner
        ));
    }

    #[test]
    fn canonicalize_active_order_decision_sets_seller_authority_and_commitments() {
        let decision =
            canonicalize_active_order_decision_for_signer(active_decision(""), SELLER).unwrap();

        assert_eq!(decision.order_id, "order-1");
        assert_eq!(
            decision.listing_addr,
            format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(decision.buyer_pubkey, BUYER);
        assert_eq!(decision.seller_pubkey, SELLER);
        let RadrootsTradeOrderDecision::Accepted {
            inventory_commitments,
        } = decision.decision
        else {
            panic!("expected accepted decision")
        };
        assert_eq!(inventory_commitments[0].bin_id, "bin-1");
    }

    #[test]
    fn canonicalize_active_order_decision_rejects_wrong_seller_signer() {
        let error = canonicalize_active_order_decision_for_signer(active_decision(BUYER), SELLER)
            .unwrap_err();

        assert!(matches!(
            error,
            RadrootsTradeOrderCanonicalizationError::InvalidSellerListing
        ));
    }

    #[test]
    fn canonicalize_active_order_decision_rejects_invalid_commitments() {
        let mut decision = active_decision("");
        let RadrootsTradeOrderDecision::Accepted {
            inventory_commitments,
        } = &mut decision.decision
        else {
            panic!("expected accepted decision")
        };
        inventory_commitments.clear();

        let error = canonicalize_active_order_decision_for_signer(decision, SELLER).unwrap_err();
        assert!(matches!(
            error,
            RadrootsTradeOrderCanonicalizationError::MissingInventoryCommitments
        ));
    }

    #[test]
    fn canonicalize_active_order_decision_trims_decline_reason() {
        let mut decision = active_decision("");
        decision.decision = RadrootsTradeOrderDecision::Declined {
            reason: " out_of_stock ".to_string(),
        };

        let decision = canonicalize_active_order_decision_for_signer(decision, SELLER).unwrap();
        let RadrootsTradeOrderDecision::Declined { reason } = decision.decision else {
            panic!("expected declined decision")
        };
        assert_eq!(reason, "out_of_stock");
    }

    #[test]
    fn reduce_active_order_events_reports_missing_without_events() {
        let projection = reduce_active_order_events("order-1", [], [], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Missing);
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_active_order_events_reports_requested_state() {
        let projection = reduce_active_order_events("order-1", [request_record()], [], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Requested);
        assert_eq!(projection.request_event_id.as_deref(), Some("request-1"));
        assert_eq!(projection.last_event_id.as_deref(), Some("request-1"));
    }

    #[test]
    fn reduce_active_order_events_reports_accepted_state() {
        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [],
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Accepted);
        assert_eq!(projection.decision_event_id.as_deref(), Some("decision-1"));
        assert_eq!(
            projection.fulfillment_status,
            Some(RadrootsActiveTradeFulfillmentState::AcceptedNotFulfilled)
        );
        assert_eq!(projection.fulfillment_event_id, None);
        assert_eq!(projection.last_event_id.as_deref(), Some("decision-1"));
    }

    #[test]
    fn reduce_active_order_events_reports_latest_fulfillment_state() {
        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [
                fulfillment_record(
                    "fulfillment-2",
                    "fulfillment-1",
                    RadrootsActiveTradeFulfillmentState::ReadyForPickup,
                ),
                fulfillment_record(
                    "fulfillment-1",
                    "decision-1",
                    RadrootsActiveTradeFulfillmentState::Preparing,
                ),
            ],
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Accepted);
        assert_eq!(
            projection.fulfillment_status,
            Some(RadrootsActiveTradeFulfillmentState::ReadyForPickup)
        );
        assert_eq!(
            projection.fulfillment_event_id.as_deref(),
            Some("fulfillment-2")
        );
        assert_eq!(projection.last_event_id.as_deref(), Some("fulfillment-2"));
    }

    #[test]
    fn reduce_active_order_events_rejects_fulfillment_before_acceptance() {
        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [],
            [fulfillment_record(
                "fulfillment-1",
                "request-1",
                RadrootsActiveTradeFulfillmentState::Preparing,
            )],
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![
                RadrootsActiveOrderReducerIssue::FulfillmentWithoutAcceptedDecision {
                    event_id: "fulfillment-1".to_string()
                }
            ]
        );
    }

    #[test]
    fn reduce_active_order_events_rejects_fulfillment_after_decline() {
        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [declined_decision_record("decision-1")],
            [fulfillment_record(
                "fulfillment-1",
                "decision-1",
                RadrootsActiveTradeFulfillmentState::Preparing,
            )],
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![
                RadrootsActiveOrderReducerIssue::FulfillmentWithoutAcceptedDecision {
                    event_id: "fulfillment-1".to_string()
                }
            ]
        );
    }

    #[test]
    fn reduce_active_order_events_rejects_wrong_actor_fulfillment() {
        let mut fulfillment = fulfillment_record(
            "fulfillment-1",
            "decision-1",
            RadrootsActiveTradeFulfillmentState::Preparing,
        );
        fulfillment.author_pubkey = BUYER.to_string();

        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [fulfillment],
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsActiveOrderReducerIssue::FulfillmentAuthorMismatch { event_id }
                if event_id == "fulfillment-1"
        )));
    }

    #[test]
    fn reduce_active_order_events_rejects_forked_fulfillment_chain() {
        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [
                fulfillment_record(
                    "fulfillment-2",
                    "decision-1",
                    RadrootsActiveTradeFulfillmentState::Preparing,
                ),
                fulfillment_record(
                    "fulfillment-1",
                    "decision-1",
                    RadrootsActiveTradeFulfillmentState::ReadyForPickup,
                ),
            ],
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsActiveOrderReducerIssue::ForkedFulfillments {
                event_ids: vec!["fulfillment-1".to_string(), "fulfillment-2".to_string()]
            }]
        );
    }

    #[test]
    fn reduce_active_order_events_rejects_terminal_fulfillment_transition() {
        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [accepted_decision_record("decision-1")],
            [
                fulfillment_record(
                    "fulfillment-1",
                    "decision-1",
                    RadrootsActiveTradeFulfillmentState::Delivered,
                ),
                fulfillment_record(
                    "fulfillment-2",
                    "fulfillment-1",
                    RadrootsActiveTradeFulfillmentState::ReadyForPickup,
                ),
            ],
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![
                RadrootsActiveOrderReducerIssue::FulfillmentUnsupportedTransition {
                    event_id: "fulfillment-2".to_string()
                }
            ]
        );
    }

    #[test]
    fn reduce_active_order_events_reports_declined_state() {
        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [declined_decision_record("decision-1")],
            [],
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Declined);
        assert_eq!(projection.decision_event_id.as_deref(), Some("decision-1"));
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
        );

        assert_eq!(projection.listing_event_id, "listing-event-1");
        assert_eq!(projection.declined_order_ids, Vec::<String>::new());
        assert_eq!(projection.invalid_event_ids, Vec::<String>::new());
        assert!(projection.issues.is_empty());
        assert_eq!(
            projection.bins,
            vec![RadrootsListingInventoryBinAccounting {
                bin_id: "bin-1".to_string(),
                available_count: 5,
                accepted_reserved_count: 2,
                remaining_count: 3,
                over_reserved: false,
                accepted_orders: vec![RadrootsListingInventoryOrderReservation {
                    order_id: "order-1".to_string(),
                    decision_event_id: "decision-1".to_string(),
                    bin_count: 2,
                }],
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
                RadrootsActiveTradeFulfillmentState::SellerCancelled,
            )],
        );

        assert!(projection.issues.is_empty());
        assert_eq!(projection.invalid_event_ids, Vec::<String>::new());
        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(projection.bins[0].remaining_count, 5);
        assert!(projection.bins[0].accepted_orders.is_empty());
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
                    RadrootsActiveTradeFulfillmentState::SellerCancelled,
                ),
                fulfillment_record(
                    "fulfillment-1",
                    "decision-1",
                    RadrootsActiveTradeFulfillmentState::Preparing,
                ),
            ],
        );

        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(
            projection.invalid_event_ids,
            vec!["fulfillment-1".to_string(), "fulfillment-2".to_string()]
        );
        assert_eq!(
            projection.issues,
            vec![
                RadrootsListingInventoryAccountingIssue::InvalidActiveOrder {
                    order_id: "order-1".to_string(),
                    event_ids: vec!["fulfillment-1".to_string(), "fulfillment-2".to_string()],
                }
            ]
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
        );

        assert_eq!(projection.declined_order_ids, vec!["order-1".to_string()]);
        assert!(projection.invalid_event_ids.is_empty());
        assert!(projection.issues.is_empty());
        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(projection.bins[0].remaining_count, 5);
        assert!(!projection.bins[0].over_reserved);
    }

    #[test]
    fn reduce_listing_inventory_accounting_reports_invalid_mismatched_commitment() {
        let decision = RadrootsActiveOrderDecisionRecord {
            payload: decision_payload(RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: vec![RadrootsTradeInventoryCommitment {
                    bin_id: "bin-1".to_string(),
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
        );

        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(projection.invalid_event_ids, vec!["decision-1".to_string()]);
        assert_eq!(
            projection.issues,
            vec![
                RadrootsListingInventoryAccountingIssue::InvalidActiveOrder {
                    order_id: "order-1".to_string(),
                    event_ids: vec!["decision-1".to_string()],
                }
            ]
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
        );

        assert_eq!(projection.bins[0].available_count, 3);
        assert_eq!(projection.bins[0].accepted_reserved_count, 4);
        assert_eq!(projection.bins[0].remaining_count, 0);
        assert!(projection.bins[0].over_reserved);
        assert_eq!(
            projection.issues,
            vec![RadrootsListingInventoryAccountingIssue::OverReserved {
                bin_id: "bin-1".to_string(),
                available_count: 3,
                reserved_count: 4,
                event_ids: vec!["decision-1".to_string(), "decision-2".to_string()],
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
                    bin_id: "bin-1".to_string(),
                    available_count: u64::MAX,
                },
                inventory_bin(1),
            ],
            Vec::<RadrootsActiveOrderRequestRecord>::new(),
            Vec::<RadrootsActiveOrderDecisionRecord>::new(),
            Vec::<RadrootsActiveOrderFulfillmentRecord>::new(),
        );

        assert_eq!(projection.bins[0].available_count, u64::MAX);
        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(projection.bins[0].remaining_count, u64::MAX);
        assert_eq!(
            projection.issues,
            vec![
                RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                    bin_id: "bin-1".to_string(),
                    event_ids: Vec::new(),
                }
            ]
        );
    }

    #[test]
    fn add_inventory_reservation_reports_reservation_overflow() {
        let mut bin = RadrootsListingInventoryBinAccounting {
            bin_id: "bin-1".to_string(),
            available_count: u64::MAX,
            accepted_reserved_count: u64::MAX,
            remaining_count: 0,
            over_reserved: false,
            accepted_orders: Vec::new(),
        };
        let decision = accepted_decision_record("decision-overflow");
        let mut issues = Vec::new();

        add_inventory_reservation(&mut bin, "order-overflow", &decision, 1, &mut issues);

        assert_eq!(bin.accepted_reserved_count, u64::MAX);
        assert!(bin.accepted_orders.is_empty());
        assert_eq!(
            issues,
            vec![
                RadrootsListingInventoryAccountingIssue::ArithmeticOverflow {
                    bin_id: "bin-1".to_string(),
                    event_ids: vec!["decision-overflow".to_string()],
                }
            ]
        );
    }

    #[test]
    fn reduce_active_order_events_rejects_invalid_decision_actor() {
        let mut decision = accepted_decision_record("decision-1");
        decision.author_pubkey = BUYER.to_string();

        let projection = reduce_active_order_events("order-1", [request_record()], [decision], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsActiveOrderReducerIssue::DecisionAuthorMismatch { event_id }
                if event_id == "decision-1"
        )));
    }

    #[test]
    fn reduce_active_order_events_rejects_invalid_decision_counterparty() {
        let mut decision = accepted_decision_record("decision-1");
        decision.counterparty_pubkey = SELLER.to_string();

        let projection = reduce_active_order_events("order-1", [request_record()], [decision], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsActiveOrderReducerIssue::DecisionCounterpartyMismatch { event_id }
                if event_id == "decision-1"
        )));
    }

    #[test]
    fn reduce_listing_inventory_accounting_ignores_wrong_counterparty_decision() {
        let mut decision = accepted_decision_record("decision-1");
        decision.counterparty_pubkey = SELLER.to_string();

        let projection = reduce_listing_inventory_accounting(
            &listing_addr(),
            "listing-event-1",
            [inventory_bin(5)],
            [request_record()],
            [decision],
            [],
        );

        assert_eq!(projection.bins[0].accepted_reserved_count, 0);
        assert_eq!(projection.invalid_event_ids, vec!["decision-1".to_string()]);
        assert_eq!(
            projection.issues,
            vec![
                RadrootsListingInventoryAccountingIssue::InvalidActiveOrder {
                    order_id: "order-1".to_string(),
                    event_ids: vec!["decision-1".to_string()],
                }
            ]
        );
    }

    #[test]
    fn reduce_active_order_events_rejects_invalid_decision_chain() {
        let mut decision = accepted_decision_record("decision-1");
        decision.prev_event_id = "request-2".to_string();

        let projection = reduce_active_order_events("order-1", [request_record()], [decision], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsActiveOrderReducerIssue::DecisionPreviousMismatch { event_id }
                if event_id == "decision-1"
        )));
    }

    #[test]
    fn reduce_active_order_events_rejects_missing_commitment() {
        let decision = RadrootsActiveOrderDecisionRecord {
            payload: decision_payload(RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: Vec::new(),
            }),
            ..accepted_decision_record("decision-1")
        };

        let projection = reduce_active_order_events("order-1", [request_record()], [decision], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsActiveOrderReducerIssue::DecisionMissingInventoryCommitments { event_id }
                if event_id == "decision-1"
        )));
    }

    #[test]
    fn reduce_active_order_events_rejects_commitment_count_mismatch() {
        let decision = RadrootsActiveOrderDecisionRecord {
            payload: decision_payload(RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: vec![RadrootsTradeInventoryCommitment {
                    bin_id: "bin-1".to_string(),
                    bin_count: 1,
                }],
            }),
            ..accepted_decision_record("decision-1")
        };

        let projection = reduce_active_order_events("order-1", [request_record()], [decision], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsActiveOrderReducerIssue::DecisionInventoryCommitmentMismatch { event_id }
                if event_id == "decision-1"
        )));
    }

    #[test]
    fn reduce_active_order_events_rejects_commitment_bin_mismatch() {
        let decision = RadrootsActiveOrderDecisionRecord {
            payload: decision_payload(RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: vec![RadrootsTradeInventoryCommitment {
                    bin_id: "bin-2".to_string(),
                    bin_count: 2,
                }],
            }),
            ..accepted_decision_record("decision-1")
        };

        let projection = reduce_active_order_events("order-1", [request_record()], [decision], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![
                RadrootsActiveOrderReducerIssue::DecisionInventoryCommitmentMismatch {
                    event_id: "decision-1".to_string()
                }
            ]
        );
    }

    #[test]
    fn reduce_active_order_events_matches_normalized_duplicate_bins() {
        let mut request = request_record();
        request.payload.items = vec![
            RadrootsTradeOrderItem {
                bin_id: " bin-1 ".to_string(),
                bin_count: 1,
            },
            RadrootsTradeOrderItem {
                bin_id: "bin-1".to_string(),
                bin_count: 1,
            },
        ];
        let decision = RadrootsActiveOrderDecisionRecord {
            payload: decision_payload(RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: vec![RadrootsTradeInventoryCommitment {
                    bin_id: "bin-1".to_string(),
                    bin_count: 2,
                }],
            }),
            ..accepted_decision_record("decision-1")
        };

        let projection = reduce_active_order_events("order-1", [request], [decision], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Accepted);
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_active_order_events_rejects_missing_decline_reason() {
        let decision = RadrootsActiveOrderDecisionRecord {
            payload: decision_payload(RadrootsTradeOrderDecision::Declined {
                reason: " ".to_string(),
            }),
            ..declined_decision_record("decision-1")
        };

        let projection = reduce_active_order_events("order-1", [request_record()], [decision], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsActiveOrderReducerIssue::DecisionMissingReason { event_id }
                if event_id == "decision-1"
        )));
    }

    #[test]
    fn reduce_active_order_events_rejects_conflicting_decisions() {
        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [
                accepted_decision_record("decision-2"),
                declined_decision_record("decision-1"),
            ],
            [],
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsActiveOrderReducerIssue::ConflictingDecisions {
                event_ids: vec!["decision-1".to_string(), "decision-2".to_string()]
            }]
        );
    }

    #[test]
    fn reduce_active_order_events_reports_multiple_requests_deterministically() {
        let projection = reduce_active_order_events(
            "order-1",
            [
                request_record_with_event_id("request-2"),
                request_record_with_event_id("request-1"),
            ],
            [],
            [],
        );
        let reversed = reduce_active_order_events(
            "order-1",
            [
                request_record_with_event_id("request-1"),
                request_record_with_event_id("request-2"),
            ],
            [],
            [],
        );

        assert_eq!(projection, reversed);
        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert_eq!(projection.request_event_id.as_deref(), Some("request-1"));
        assert_eq!(
            projection.issues,
            vec![RadrootsActiveOrderReducerIssue::MultipleRequests {
                event_ids: vec!["request-1".to_string(), "request-2".to_string()]
            }]
        );
    }

    #[test]
    fn reduce_active_order_events_reports_conflicting_decisions_deterministically() {
        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [
                accepted_decision_record("decision-2"),
                declined_decision_record("decision-1"),
            ],
            [],
        );
        let reversed = reduce_active_order_events(
            "order-1",
            [request_record()],
            [
                declined_decision_record("decision-1"),
                accepted_decision_record("decision-2"),
            ],
            [],
        );

        assert_eq!(projection, reversed);
        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert_eq!(
            projection.issues,
            vec![RadrootsActiveOrderReducerIssue::ConflictingDecisions {
                event_ids: vec!["decision-1".to_string(), "decision-2".to_string()]
            }]
        );
    }
}

#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::kinds::KIND_LISTING;
use radroots_events::trade::{
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
    pub root_event_id: String,
    pub prev_event_id: String,
    pub payload: RadrootsTradeOrderDecisionEvent,
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
    DecisionBuyerMismatch { event_id: String },
    DecisionSellerMismatch { event_id: String },
    DecisionListingAddressInvalid { event_id: String },
    DecisionListingMismatch { event_id: String },
    DecisionRootMismatch { event_id: String },
    DecisionPreviousMismatch { event_id: String },
    DecisionMissingInventoryCommitments { event_id: String },
    DecisionMissingReason { event_id: String },
    ConflictingDecisions { event_ids: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsActiveOrderProjection {
    pub order_id: String,
    pub status: RadrootsActiveOrderStatus,
    pub request_event_id: Option<String>,
    pub decision_event_id: Option<String>,
    pub listing_addr: Option<String>,
    pub buyer_pubkey: Option<String>,
    pub seller_pubkey: Option<String>,
    pub last_event_id: Option<String>,
    pub issues: Vec<RadrootsActiveOrderReducerIssue>,
}

pub fn reduce_active_order_events<I, J>(
    order_id: &str,
    requests: I,
    decisions: J,
) -> RadrootsActiveOrderProjection
where
    I: IntoIterator<Item = RadrootsActiveOrderRequestRecord>,
    J: IntoIterator<Item = RadrootsActiveOrderDecisionRecord>,
{
    let requests = unique_request_records(requests);
    let decisions = unique_decision_records(decisions);
    if requests.is_empty() && decisions.is_empty() {
        return RadrootsActiveOrderProjection {
            order_id: order_id.to_string(),
            status: RadrootsActiveOrderStatus::Missing,
            request_event_id: None,
            decision_event_id: None,
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
        if decisions.is_empty() {
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
        0 => requested_projection(order_id, request),
        1 => decided_projection(order_id, request, &valid_decisions[0]),
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

fn requested_projection(
    order_id: &str,
    request: &RadrootsActiveOrderRequestRecord,
) -> RadrootsActiveOrderProjection {
    RadrootsActiveOrderProjection {
        order_id: order_id.to_string(),
        status: RadrootsActiveOrderStatus::Requested,
        request_event_id: Some(request.event_id.clone()),
        decision_event_id: None,
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
) -> RadrootsActiveOrderProjection {
    let status = match &decision.payload.decision {
        RadrootsTradeOrderDecision::Accepted { .. } => RadrootsActiveOrderStatus::Accepted,
        RadrootsTradeOrderDecision::Declined { .. } => RadrootsActiveOrderStatus::Declined,
    };
    RadrootsActiveOrderProjection {
        order_id: order_id.to_string(),
        status,
        request_event_id: Some(request.event_id.clone()),
        decision_event_id: Some(decision.event_id.clone()),
        listing_addr: Some(request.payload.listing_addr.clone()),
        buyer_pubkey: Some(request.payload.buyer_pubkey.clone()),
        seller_pubkey: Some(request.payload.seller_pubkey.clone()),
        last_event_id: Some(decision.event_id.clone()),
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
        RadrootsTradeInventoryCommitment, RadrootsTradeOrder as TradeOrder,
        RadrootsTradeOrderDecision, RadrootsTradeOrderDecisionEvent, RadrootsTradeOrderItem,
        RadrootsTradeOrderRequested,
    };

    use super::{
        RadrootsActiveOrderDecisionRecord, RadrootsActiveOrderReducerIssue,
        RadrootsActiveOrderRequestRecord, RadrootsActiveOrderStatus,
        RadrootsTradeOrderCanonicalizationError, canonicalize_active_order_decision_for_signer,
        canonicalize_active_order_request_for_signer, canonicalize_order_request_for_signer,
        reduce_active_order_events,
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

    fn clean_request_payload() -> RadrootsTradeOrderRequested {
        RadrootsTradeOrderRequested {
            order_id: "order-1".to_string(),
            listing_addr: format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"),
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

    fn decision_payload(decision: RadrootsTradeOrderDecision) -> RadrootsTradeOrderDecisionEvent {
        RadrootsTradeOrderDecisionEvent {
            order_id: "order-1".to_string(),
            listing_addr: format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"),
            buyer_pubkey: BUYER.to_string(),
            seller_pubkey: SELLER.to_string(),
            decision,
        }
    }

    fn accepted_decision_record(event_id: &str) -> RadrootsActiveOrderDecisionRecord {
        RadrootsActiveOrderDecisionRecord {
            event_id: event_id.to_string(),
            author_pubkey: SELLER.to_string(),
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
            root_event_id: "request-1".to_string(),
            prev_event_id: "request-1".to_string(),
            payload: decision_payload(RadrootsTradeOrderDecision::Declined {
                reason: "out_of_stock".to_string(),
            }),
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
        let projection = reduce_active_order_events("order-1", [], []);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Missing);
        assert!(projection.issues.is_empty());
    }

    #[test]
    fn reduce_active_order_events_reports_requested_state() {
        let projection = reduce_active_order_events("order-1", [request_record()], []);

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
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Accepted);
        assert_eq!(projection.decision_event_id.as_deref(), Some("decision-1"));
        assert_eq!(projection.last_event_id.as_deref(), Some("decision-1"));
    }

    #[test]
    fn reduce_active_order_events_reports_declined_state() {
        let projection = reduce_active_order_events(
            "order-1",
            [request_record()],
            [declined_decision_record("decision-1")],
        );

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Declined);
        assert_eq!(projection.decision_event_id.as_deref(), Some("decision-1"));
    }

    #[test]
    fn reduce_active_order_events_rejects_invalid_decision_actor() {
        let mut decision = accepted_decision_record("decision-1");
        decision.author_pubkey = BUYER.to_string();

        let projection = reduce_active_order_events("order-1", [request_record()], [decision]);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsActiveOrderReducerIssue::DecisionAuthorMismatch { event_id }
                if event_id == "decision-1"
        )));
    }

    #[test]
    fn reduce_active_order_events_rejects_invalid_decision_chain() {
        let mut decision = accepted_decision_record("decision-1");
        decision.prev_event_id = "request-2".to_string();

        let projection = reduce_active_order_events("order-1", [request_record()], [decision]);

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

        let projection = reduce_active_order_events("order-1", [request_record()], [decision]);

        assert_eq!(projection.status, RadrootsActiveOrderStatus::Invalid);
        assert!(projection.issues.iter().any(|issue| matches!(
            issue,
            RadrootsActiveOrderReducerIssue::DecisionMissingInventoryCommitments { event_id }
                if event_id == "decision-1"
        )));
    }

    #[test]
    fn reduce_active_order_events_rejects_missing_decline_reason() {
        let decision = RadrootsActiveOrderDecisionRecord {
            payload: decision_payload(RadrootsTradeOrderDecision::Declined {
                reason: " ".to_string(),
            }),
            ..declined_decision_record("decision-1")
        };

        let projection = reduce_active_order_events("order-1", [request_record()], [decision]);

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
        );
        let reversed = reduce_active_order_events(
            "order-1",
            [
                request_record_with_event_id("request-1"),
                request_record_with_event_id("request-2"),
            ],
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
        );
        let reversed = reduce_active_order_events(
            "order-1",
            [request_record()],
            [
                declined_decision_record("decision-1"),
                accepted_decision_record("decision-2"),
            ],
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

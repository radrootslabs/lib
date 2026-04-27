#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

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
        RadrootsTradeOrderCanonicalizationError, canonicalize_active_order_decision_for_signer,
        canonicalize_active_order_request_for_signer, canonicalize_order_request_for_signer,
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
}

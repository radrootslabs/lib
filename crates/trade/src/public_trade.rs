#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use radroots_events::kinds::KIND_LISTING;
use radroots_events::trade::RadrootsTradeMessageType as TradeListingMessageType;
use radroots_events_codec::trade::RadrootsTradeListingAddress as TradeListingAddress;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalPublicTradeContext {
    pub listing_addr: String,
    pub order_id: String,
    pub counterparty_pubkey: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpectedPublicTradeAuthor {
    Buyer,
    Seller,
    Either,
}

#[derive(Debug, Error)]
pub enum RadrootsPublicTradeCanonicalizationError {
    #[error("{0} cannot be empty")]
    EmptyField(&'static str),
    #[error("invalid listing_addr: {0}")]
    InvalidListingAddress(String),
    #[error("listing_addr must reference a public NIP-99 listing")]
    InvalidListingKind,
    #[error("counterparty_pubkey must not match the requested signer identity")]
    DuplicateCounterparty,
    #[error("{0}")]
    InvalidAuthor(String),
}

pub fn canonicalize_public_trade_context(
    listing_addr: String,
    order_id: String,
    counterparty_pubkey: String,
    signer_pubkey: &str,
    message_type: TradeListingMessageType,
) -> Result<CanonicalPublicTradeContext, RadrootsPublicTradeCanonicalizationError> {
    let listing_addr = normalized_required_string(listing_addr, "listing_addr")?;
    let parsed_listing_addr = TradeListingAddress::parse(&listing_addr).map_err(|error| {
        RadrootsPublicTradeCanonicalizationError::InvalidListingAddress(error.to_string())
    })?;
    if u32::from(parsed_listing_addr.kind) != KIND_LISTING {
        return Err(RadrootsPublicTradeCanonicalizationError::InvalidListingKind);
    }

    let order_id = normalized_required_string(order_id, "order_id")?;
    let counterparty_pubkey =
        normalized_required_string(counterparty_pubkey, "counterparty_pubkey")?;
    if counterparty_pubkey == signer_pubkey {
        return Err(RadrootsPublicTradeCanonicalizationError::DuplicateCounterparty);
    }

    validate_expected_author(
        &parsed_listing_addr,
        message_type,
        signer_pubkey,
        &counterparty_pubkey,
    )?;

    Ok(CanonicalPublicTradeContext {
        listing_addr,
        order_id,
        counterparty_pubkey,
    })
}

fn validate_expected_author(
    listing_addr: &TradeListingAddress,
    message_type: TradeListingMessageType,
    signer_pubkey: &str,
    counterparty_pubkey: &str,
) -> Result<(), RadrootsPublicTradeCanonicalizationError> {
    match expected_author(message_type) {
        ExpectedPublicTradeAuthor::Seller => {
            if signer_pubkey != listing_addr.seller_pubkey {
                return Err(RadrootsPublicTradeCanonicalizationError::InvalidAuthor(
                    format!("{message_type:?} must be authored by the listing seller"),
                ));
            }
            if counterparty_pubkey == listing_addr.seller_pubkey {
                return Err(RadrootsPublicTradeCanonicalizationError::InvalidAuthor(
                    format!("{message_type:?} counterparty must not be the listing seller"),
                ));
            }
        }
        ExpectedPublicTradeAuthor::Buyer => {
            if signer_pubkey == listing_addr.seller_pubkey {
                return Err(RadrootsPublicTradeCanonicalizationError::InvalidAuthor(
                    format!("{message_type:?} must be authored by the listing buyer"),
                ));
            }
            if counterparty_pubkey != listing_addr.seller_pubkey {
                return Err(RadrootsPublicTradeCanonicalizationError::InvalidAuthor(
                    format!("{message_type:?} counterparty must be the listing seller"),
                ));
            }
        }
        ExpectedPublicTradeAuthor::Either => {}
    }
    Ok(())
}

fn expected_author(message_type: TradeListingMessageType) -> ExpectedPublicTradeAuthor {
    use TradeListingMessageType as MessageType;

    match message_type {
        MessageType::OrderResponse
        | MessageType::OrderRevision
        | MessageType::OrderRevisionAccept
        | MessageType::OrderRevisionDecline
        | MessageType::Answer
        | MessageType::DiscountOffer
        | MessageType::DiscountAccept
        | MessageType::DiscountDecline
        | MessageType::FulfillmentUpdate => ExpectedPublicTradeAuthor::Seller,
        MessageType::Question
        | MessageType::DiscountRequest
        | MessageType::Cancel
        | MessageType::Receipt => ExpectedPublicTradeAuthor::Buyer,
        MessageType::OrderRequest
        | MessageType::ListingValidateRequest
        | MessageType::ListingValidateResult => ExpectedPublicTradeAuthor::Either,
    }
}

fn normalized_required_string(
    value: String,
    field: &'static str,
) -> Result<String, RadrootsPublicTradeCanonicalizationError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(RadrootsPublicTradeCanonicalizationError::EmptyField(field));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use radroots_events::kinds::KIND_LISTING;

    use super::canonicalize_public_trade_context;

    #[test]
    fn canonicalize_public_trade_context_accepts_seller_authored_message() {
        let context = canonicalize_public_trade_context(
            format!(
                "{KIND_LISTING}:1111111111111111111111111111111111111111111111111111111111111111:AAAAAAAAAAAAAAAAAAAAAg"
            ),
            "order-1".to_string(),
            "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            "1111111111111111111111111111111111111111111111111111111111111111",
            super::TradeListingMessageType::OrderResponse,
        )
        .expect("canonical public trade context");

        assert_eq!(context.order_id, "order-1");
    }

    #[test]
    fn canonicalize_public_trade_context_rejects_wrong_seller_role() {
        let err = canonicalize_public_trade_context(
            format!(
                "{KIND_LISTING}:1111111111111111111111111111111111111111111111111111111111111111:AAAAAAAAAAAAAAAAAAAAAg"
            ),
            "order-1".to_string(),
            "3333333333333333333333333333333333333333333333333333333333333333".to_string(),
            "2222222222222222222222222222222222222222222222222222222222222222",
            super::TradeListingMessageType::OrderResponse,
        )
        .expect_err("invalid seller role");

        assert!(err.to_string().contains("listing seller"));
    }
}

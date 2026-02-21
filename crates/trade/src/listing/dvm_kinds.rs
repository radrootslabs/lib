#![forbid(unsafe_code)]

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

pub const KIND_TRADE_LISTING_VALIDATE_REQ: u16 = 5321;
pub const KIND_TRADE_LISTING_VALIDATE_RES: u16 = 6321;

pub const KIND_TRADE_LISTING_ORDER_REQ: u16 = 5322;
pub const KIND_TRADE_LISTING_ORDER_RES: u16 = 6322;

pub const KIND_TRADE_LISTING_ORDER_REVISION_REQ: u16 = 5323;
pub const KIND_TRADE_LISTING_ORDER_REVISION_RES: u16 = 6323;

pub const KIND_TRADE_LISTING_QUESTION_REQ: u16 = 5324;
pub const KIND_TRADE_LISTING_ANSWER_RES: u16 = 6324;

pub const KIND_TRADE_LISTING_DISCOUNT_REQ: u16 = 5325;
pub const KIND_TRADE_LISTING_DISCOUNT_OFFER_RES: u16 = 6325;

pub const KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ: u16 = 5326;
pub const KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ: u16 = 5327;

pub const KIND_TRADE_LISTING_CANCEL_REQ: u16 = 5328;
pub const KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ: u16 = 5329;
pub const KIND_TRADE_LISTING_RECEIPT_REQ: u16 = 5330;

pub const TRADE_LISTING_DVM_KINDS: [u16; 15] = [
    KIND_TRADE_LISTING_VALIDATE_REQ,
    KIND_TRADE_LISTING_VALIDATE_RES,
    KIND_TRADE_LISTING_ORDER_REQ,
    KIND_TRADE_LISTING_ORDER_RES,
    KIND_TRADE_LISTING_ORDER_REVISION_REQ,
    KIND_TRADE_LISTING_ORDER_REVISION_RES,
    KIND_TRADE_LISTING_QUESTION_REQ,
    KIND_TRADE_LISTING_ANSWER_RES,
    KIND_TRADE_LISTING_DISCOUNT_REQ,
    KIND_TRADE_LISTING_DISCOUNT_OFFER_RES,
    KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ,
    KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ,
    KIND_TRADE_LISTING_CANCEL_REQ,
    KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ,
    KIND_TRADE_LISTING_RECEIPT_REQ,
];

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        rename_all = "SCREAMING_SNAKE_CASE",
        repr(enum)
    )
)]
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TradeListingDvmKind {
    KindTradeListingValidateReq = KIND_TRADE_LISTING_VALIDATE_REQ,
    KindTradeListingValidateRes = KIND_TRADE_LISTING_VALIDATE_RES,
    KindTradeListingOrderReq = KIND_TRADE_LISTING_ORDER_REQ,
    KindTradeListingOrderRes = KIND_TRADE_LISTING_ORDER_RES,
    KindTradeListingOrderRevisionReq = KIND_TRADE_LISTING_ORDER_REVISION_REQ,
    KindTradeListingOrderRevisionRes = KIND_TRADE_LISTING_ORDER_REVISION_RES,
    KindTradeListingQuestionReq = KIND_TRADE_LISTING_QUESTION_REQ,
    KindTradeListingAnswerRes = KIND_TRADE_LISTING_ANSWER_RES,
    KindTradeListingDiscountReq = KIND_TRADE_LISTING_DISCOUNT_REQ,
    KindTradeListingDiscountOfferRes = KIND_TRADE_LISTING_DISCOUNT_OFFER_RES,
    KindTradeListingDiscountAcceptReq = KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ,
    KindTradeListingDiscountDeclineReq = KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ,
    KindTradeListingCancelReq = KIND_TRADE_LISTING_CANCEL_REQ,
    KindTradeListingFulfillmentUpdateReq = KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ,
    KindTradeListingReceiptReq = KIND_TRADE_LISTING_RECEIPT_REQ,
}

#[inline]
pub const fn is_trade_listing_dvm_request_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_TRADE_LISTING_VALIDATE_REQ
            | KIND_TRADE_LISTING_ORDER_REQ
            | KIND_TRADE_LISTING_ORDER_REVISION_REQ
            | KIND_TRADE_LISTING_QUESTION_REQ
            | KIND_TRADE_LISTING_DISCOUNT_REQ
            | KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ
            | KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ
            | KIND_TRADE_LISTING_CANCEL_REQ
            | KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ
            | KIND_TRADE_LISTING_RECEIPT_REQ
    )
}

#[inline]
pub const fn is_trade_listing_dvm_result_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_TRADE_LISTING_VALIDATE_RES
            | KIND_TRADE_LISTING_ORDER_RES
            | KIND_TRADE_LISTING_ORDER_REVISION_RES
            | KIND_TRADE_LISTING_ANSWER_RES
            | KIND_TRADE_LISTING_DISCOUNT_OFFER_RES
    )
}

#[inline]
pub const fn is_trade_listing_dvm_kind(kind: u16) -> bool {
    is_trade_listing_dvm_request_kind(kind) || is_trade_listing_dvm_result_kind(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_request_and_result_kinds() {
        for kind in [
            KIND_TRADE_LISTING_VALIDATE_REQ,
            KIND_TRADE_LISTING_ORDER_REQ,
            KIND_TRADE_LISTING_ORDER_REVISION_REQ,
            KIND_TRADE_LISTING_QUESTION_REQ,
            KIND_TRADE_LISTING_DISCOUNT_REQ,
            KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ,
            KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ,
            KIND_TRADE_LISTING_CANCEL_REQ,
            KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ,
            KIND_TRADE_LISTING_RECEIPT_REQ,
        ] {
            assert!(is_trade_listing_dvm_request_kind(kind));
            assert!(is_trade_listing_dvm_kind(kind));
            assert!(!is_trade_listing_dvm_result_kind(kind));
        }

        for kind in [
            KIND_TRADE_LISTING_VALIDATE_RES,
            KIND_TRADE_LISTING_ORDER_RES,
            KIND_TRADE_LISTING_ORDER_REVISION_RES,
            KIND_TRADE_LISTING_ANSWER_RES,
            KIND_TRADE_LISTING_DISCOUNT_OFFER_RES,
        ] {
            assert!(is_trade_listing_dvm_result_kind(kind));
            assert!(is_trade_listing_dvm_kind(kind));
            assert!(!is_trade_listing_dvm_request_kind(kind));
        }
    }

    #[test]
    fn rejects_non_trade_dvm_kind() {
        assert!(!is_trade_listing_dvm_kind(5000));
        assert!(!is_trade_listing_dvm_request_kind(5000));
        assert!(!is_trade_listing_dvm_result_kind(5000));
    }

    #[test]
    fn dvm_kind_array_contains_expected_kind_values() {
        assert_eq!(TRADE_LISTING_DVM_KINDS.len(), 15);
        assert!(TRADE_LISTING_DVM_KINDS.contains(&KIND_TRADE_LISTING_VALIDATE_REQ));
        assert!(TRADE_LISTING_DVM_KINDS.contains(&KIND_TRADE_LISTING_VALIDATE_RES));
        assert!(TRADE_LISTING_DVM_KINDS.contains(&KIND_TRADE_LISTING_ORDER_REQ));
        assert!(TRADE_LISTING_DVM_KINDS.contains(&KIND_TRADE_LISTING_ORDER_RES));
        assert!(TRADE_LISTING_DVM_KINDS.contains(&KIND_TRADE_LISTING_RECEIPT_REQ));
    }
}

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

pub const TRADE_LISTING_KINDS: [u16; 15] = [
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
pub enum TradeListingKind {
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
pub const fn is_trade_listing_request_kind(kind: u16) -> bool {
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
pub const fn is_trade_listing_result_kind(kind: u16) -> bool {
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
pub const fn is_trade_listing_kind(kind: u16) -> bool {
    is_trade_listing_request_kind(kind) || is_trade_listing_result_kind(kind)
}

#[inline]
pub const fn trade_listing_result_kind_for_request(kind: u16) -> Option<u16> {
    match kind {
        KIND_TRADE_LISTING_VALIDATE_REQ => Some(KIND_TRADE_LISTING_VALIDATE_RES),
        KIND_TRADE_LISTING_ORDER_REQ => Some(KIND_TRADE_LISTING_ORDER_RES),
        KIND_TRADE_LISTING_ORDER_REVISION_REQ => Some(KIND_TRADE_LISTING_ORDER_REVISION_RES),
        KIND_TRADE_LISTING_QUESTION_REQ => Some(KIND_TRADE_LISTING_ANSWER_RES),
        KIND_TRADE_LISTING_DISCOUNT_REQ => Some(KIND_TRADE_LISTING_DISCOUNT_OFFER_RES),
        _ => None,
    }
}

#[inline]
pub const fn trade_listing_request_kind_for_result(kind: u16) -> Option<u16> {
    match kind {
        KIND_TRADE_LISTING_VALIDATE_RES => Some(KIND_TRADE_LISTING_VALIDATE_REQ),
        KIND_TRADE_LISTING_ORDER_RES => Some(KIND_TRADE_LISTING_ORDER_REQ),
        KIND_TRADE_LISTING_ORDER_REVISION_RES => Some(KIND_TRADE_LISTING_ORDER_REVISION_REQ),
        KIND_TRADE_LISTING_ANSWER_RES => Some(KIND_TRADE_LISTING_QUESTION_REQ),
        KIND_TRADE_LISTING_DISCOUNT_OFFER_RES => Some(KIND_TRADE_LISTING_DISCOUNT_REQ),
        _ => None,
    }
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
            assert!(is_trade_listing_request_kind(kind));
            assert!(is_trade_listing_kind(kind));
            assert!(!is_trade_listing_result_kind(kind));
        }

        for kind in [
            KIND_TRADE_LISTING_VALIDATE_RES,
            KIND_TRADE_LISTING_ORDER_RES,
            KIND_TRADE_LISTING_ORDER_REVISION_RES,
            KIND_TRADE_LISTING_ANSWER_RES,
            KIND_TRADE_LISTING_DISCOUNT_OFFER_RES,
        ] {
            assert!(is_trade_listing_result_kind(kind));
            assert!(is_trade_listing_kind(kind));
            assert!(!is_trade_listing_request_kind(kind));
        }
    }

    #[test]
    fn request_to_result_roundtrip_is_defined_for_request_response_pairs() {
        let pairs = [
            (KIND_TRADE_LISTING_VALIDATE_REQ, KIND_TRADE_LISTING_VALIDATE_RES),
            (KIND_TRADE_LISTING_ORDER_REQ, KIND_TRADE_LISTING_ORDER_RES),
            (
                KIND_TRADE_LISTING_ORDER_REVISION_REQ,
                KIND_TRADE_LISTING_ORDER_REVISION_RES,
            ),
            (KIND_TRADE_LISTING_QUESTION_REQ, KIND_TRADE_LISTING_ANSWER_RES),
            (
                KIND_TRADE_LISTING_DISCOUNT_REQ,
                KIND_TRADE_LISTING_DISCOUNT_OFFER_RES,
            ),
        ];

        for (req, res) in pairs {
            assert_eq!(trade_listing_result_kind_for_request(req), Some(res));
            assert_eq!(trade_listing_request_kind_for_result(res), Some(req));
        }
    }

    #[test]
    fn request_to_result_rejects_non_roundtrip_kinds() {
        for kind in [
            KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ,
            KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ,
            KIND_TRADE_LISTING_CANCEL_REQ,
            KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ,
            KIND_TRADE_LISTING_RECEIPT_REQ,
        ] {
            assert_eq!(trade_listing_result_kind_for_request(kind), None);
        }
        assert_eq!(trade_listing_request_kind_for_result(5000), None);
        assert!(!is_trade_listing_kind(5000));
    }

    #[test]
    fn kind_array_contains_expected_kinds() {
        assert_eq!(TRADE_LISTING_KINDS.len(), 15);
        assert!(TRADE_LISTING_KINDS.contains(&KIND_TRADE_LISTING_VALIDATE_REQ));
        assert!(TRADE_LISTING_KINDS.contains(&KIND_TRADE_LISTING_VALIDATE_RES));
        assert!(TRADE_LISTING_KINDS.contains(&KIND_TRADE_LISTING_ORDER_REQ));
        assert!(TRADE_LISTING_KINDS.contains(&KIND_TRADE_LISTING_ORDER_RES));
        assert!(TRADE_LISTING_KINDS.contains(&KIND_TRADE_LISTING_RECEIPT_REQ));
    }
}

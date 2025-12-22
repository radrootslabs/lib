#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_ORDER_REQ: u16 = 5301;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_ORDER_RES: u16 = 6301;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_ACCEPT_REQ: u16 = 5302;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_ACCEPT_RES: u16 = 6302;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_CONVEYANCE_REQ: u16 = 5303;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_CONVEYANCE_RES: u16 = 6303;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_INVOICE_REQ: u16 = 5304;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_INVOICE_RES: u16 = 6304;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_PAYMENT_REQ: u16 = 5305;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_PAYMENT_RES: u16 = 6305;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_FULFILL_REQ: u16 = 5306;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_FULFILL_RES: u16 = 6306;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_RECEIPT_REQ: u16 = 5307;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_RECEIPT_RES: u16 = 6307;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_CANCEL_REQ: u16 = 5309;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_CANCEL_RES: u16 = 6309;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_REFUND_REQ: u16 = 5310;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_TRADE_LISTING_REFUND_RES: u16 = 6310;

#[inline]
pub const fn is_trade_listing_request_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_TRADE_LISTING_ORDER_REQ
            | KIND_TRADE_LISTING_ACCEPT_REQ
            | KIND_TRADE_LISTING_CONVEYANCE_REQ
            | KIND_TRADE_LISTING_INVOICE_REQ
            | KIND_TRADE_LISTING_PAYMENT_REQ
            | KIND_TRADE_LISTING_FULFILL_REQ
            | KIND_TRADE_LISTING_RECEIPT_REQ
            | KIND_TRADE_LISTING_CANCEL_REQ
            | KIND_TRADE_LISTING_REFUND_REQ
    )
}

#[inline]
pub const fn is_trade_listing_result_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_TRADE_LISTING_ORDER_RES
            | KIND_TRADE_LISTING_ACCEPT_RES
            | KIND_TRADE_LISTING_CONVEYANCE_RES
            | KIND_TRADE_LISTING_INVOICE_RES
            | KIND_TRADE_LISTING_PAYMENT_RES
            | KIND_TRADE_LISTING_FULFILL_RES
            | KIND_TRADE_LISTING_RECEIPT_RES
            | KIND_TRADE_LISTING_CANCEL_RES
            | KIND_TRADE_LISTING_REFUND_RES
    )
}

#[inline]
pub const fn trade_listing_result_kind_for_request(kind: u16) -> Option<u16> {
    match kind {
        KIND_TRADE_LISTING_ORDER_REQ => Some(KIND_TRADE_LISTING_ORDER_RES),
        KIND_TRADE_LISTING_ACCEPT_REQ => Some(KIND_TRADE_LISTING_ACCEPT_RES),
        KIND_TRADE_LISTING_CONVEYANCE_REQ => Some(KIND_TRADE_LISTING_CONVEYANCE_RES),
        KIND_TRADE_LISTING_INVOICE_REQ => Some(KIND_TRADE_LISTING_INVOICE_RES),
        KIND_TRADE_LISTING_PAYMENT_REQ => Some(KIND_TRADE_LISTING_PAYMENT_RES),
        KIND_TRADE_LISTING_FULFILL_REQ => Some(KIND_TRADE_LISTING_FULFILL_RES),
        KIND_TRADE_LISTING_RECEIPT_REQ => Some(KIND_TRADE_LISTING_RECEIPT_RES),
        KIND_TRADE_LISTING_CANCEL_REQ => Some(KIND_TRADE_LISTING_CANCEL_RES),
        KIND_TRADE_LISTING_REFUND_REQ => Some(KIND_TRADE_LISTING_REFUND_RES),
        _ => None,
    }
}

#[inline]
pub const fn trade_listing_request_kind_for_result(kind: u16) -> Option<u16> {
    match kind {
        KIND_TRADE_LISTING_ORDER_RES => Some(KIND_TRADE_LISTING_ORDER_REQ),
        KIND_TRADE_LISTING_ACCEPT_RES => Some(KIND_TRADE_LISTING_ACCEPT_REQ),
        KIND_TRADE_LISTING_CONVEYANCE_RES => Some(KIND_TRADE_LISTING_CONVEYANCE_REQ),
        KIND_TRADE_LISTING_INVOICE_RES => Some(KIND_TRADE_LISTING_INVOICE_REQ),
        KIND_TRADE_LISTING_PAYMENT_RES => Some(KIND_TRADE_LISTING_PAYMENT_REQ),
        KIND_TRADE_LISTING_FULFILL_RES => Some(KIND_TRADE_LISTING_FULFILL_REQ),
        KIND_TRADE_LISTING_RECEIPT_RES => Some(KIND_TRADE_LISTING_RECEIPT_REQ),
        KIND_TRADE_LISTING_CANCEL_RES => Some(KIND_TRADE_LISTING_CANCEL_REQ),
        KIND_TRADE_LISTING_REFUND_RES => Some(KIND_TRADE_LISTING_REFUND_REQ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_to_result_roundtrip() {
        let pairs = [
            (KIND_TRADE_LISTING_ORDER_REQ, KIND_TRADE_LISTING_ORDER_RES),
            (KIND_TRADE_LISTING_ACCEPT_REQ, KIND_TRADE_LISTING_ACCEPT_RES),
            (
                KIND_TRADE_LISTING_CONVEYANCE_REQ,
                KIND_TRADE_LISTING_CONVEYANCE_RES,
            ),
            (KIND_TRADE_LISTING_INVOICE_REQ, KIND_TRADE_LISTING_INVOICE_RES),
            (KIND_TRADE_LISTING_PAYMENT_REQ, KIND_TRADE_LISTING_PAYMENT_RES),
            (KIND_TRADE_LISTING_FULFILL_REQ, KIND_TRADE_LISTING_FULFILL_RES),
            (KIND_TRADE_LISTING_RECEIPT_REQ, KIND_TRADE_LISTING_RECEIPT_RES),
            (KIND_TRADE_LISTING_CANCEL_REQ, KIND_TRADE_LISTING_CANCEL_RES),
            (KIND_TRADE_LISTING_REFUND_REQ, KIND_TRADE_LISTING_REFUND_RES),
        ];

        for (req, res) in pairs {
            assert_eq!(trade_listing_result_kind_for_request(req), Some(res));
            assert_eq!(trade_listing_request_kind_for_result(res), Some(req));
            assert!(is_trade_listing_request_kind(req));
            assert!(is_trade_listing_result_kind(res));
        }
    }

    #[test]
    fn request_to_result_rejects_non_trade_kinds() {
        assert_eq!(trade_listing_result_kind_for_request(5000), None);
        assert_eq!(trade_listing_request_kind_for_result(6000), None);
        assert!(!is_trade_listing_request_kind(5000));
        assert!(!is_trade_listing_result_kind(6000));
    }
}

#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_ORDER_REQ: u16 = 5301;
#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_ORDER_RES: u16 = 6301;

#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_ACCEPT_REQ: u16 = 5302;
#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_ACCEPT_RES: u16 = 6302;

#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_CONVEYANCE_REQ: u16 = 5303;
#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_CONVEYANCE_RES: u16 = 6303;

#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_INVOICE_REQ: u16 = 5304;
#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_INVOICE_RES: u16 = 6304;

#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_PAYMENT_REQ: u16 = 5305;
#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_PAYMENT_RES: u16 = 6305;

#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_FULFILL_REQ: u16 = 5306;
#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_FULFILL_RES: u16 = 6306;

#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_RECEIPT_REQ: u16 = 5307;
#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_RECEIPT_RES: u16 = 6307;

#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_CANCEL_REQ: u16 = 5309;
#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_CANCEL_RES: u16 = 6309;

#[typeshare::typeshare]
pub const KIND_TRADE_LISTING_REFUND_REQ: u16 = 5310;
#[typeshare::typeshare]
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

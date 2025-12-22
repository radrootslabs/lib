import { TradeListingKind, TradeListingMarker } from "./types.js";

export { TradeListingKind, TradeListingMarker };

export const MARKER_LISTING = TradeListingMarker["listing"];
export const MARKER_PAYLOAD = TradeListingMarker["payload"];
export const MARKER_PREVIOUS = TradeListingMarker["previous"];
export const MARKER_ORDER_RESULT = TradeListingMarker["order_result"];
export const MARKER_ACCEPT_RESULT = TradeListingMarker["accept_result"];
export const MARKER_CONVEYANCE_RESULT = TradeListingMarker["conveyance_result"];
export const MARKER_INVOICE_RESULT = TradeListingMarker["invoice_result"];
export const MARKER_PAYMENT_RESULT = TradeListingMarker["payment_result"];
export const MARKER_FULFILLMENT_RESULT = TradeListingMarker["fulfillment_result"];
export const MARKER_RECEIPT_RESULT = TradeListingMarker["receipt_result"];
export const MARKER_CANCEL_RESULT = TradeListingMarker["cancel_result"];
export const MARKER_REFUND_RESULT = TradeListingMarker["refund_result"];
export const MARKER_PROOF = TradeListingMarker["proof"];

export const KIND_TRADE_LISTING_ORDER_REQ = TradeListingKind.KIND_TRADE_LISTING_ORDER_REQ;
export const KIND_TRADE_LISTING_ORDER_RES = TradeListingKind.KIND_TRADE_LISTING_ORDER_RES;
export const KIND_TRADE_LISTING_ACCEPT_REQ = TradeListingKind.KIND_TRADE_LISTING_ACCEPT_REQ;
export const KIND_TRADE_LISTING_ACCEPT_RES = TradeListingKind.KIND_TRADE_LISTING_ACCEPT_RES;
export const KIND_TRADE_LISTING_CONVEYANCE_REQ = TradeListingKind.KIND_TRADE_LISTING_CONVEYANCE_REQ;
export const KIND_TRADE_LISTING_CONVEYANCE_RES = TradeListingKind.KIND_TRADE_LISTING_CONVEYANCE_RES;
export const KIND_TRADE_LISTING_INVOICE_REQ = TradeListingKind.KIND_TRADE_LISTING_INVOICE_REQ;
export const KIND_TRADE_LISTING_INVOICE_RES = TradeListingKind.KIND_TRADE_LISTING_INVOICE_RES;
export const KIND_TRADE_LISTING_PAYMENT_REQ = TradeListingKind.KIND_TRADE_LISTING_PAYMENT_REQ;
export const KIND_TRADE_LISTING_PAYMENT_RES = TradeListingKind.KIND_TRADE_LISTING_PAYMENT_RES;
export const KIND_TRADE_LISTING_FULFILL_REQ = TradeListingKind.KIND_TRADE_LISTING_FULFILL_REQ;
export const KIND_TRADE_LISTING_FULFILL_RES = TradeListingKind.KIND_TRADE_LISTING_FULFILL_RES;
export const KIND_TRADE_LISTING_RECEIPT_REQ = TradeListingKind.KIND_TRADE_LISTING_RECEIPT_REQ;
export const KIND_TRADE_LISTING_RECEIPT_RES = TradeListingKind.KIND_TRADE_LISTING_RECEIPT_RES;
export const KIND_TRADE_LISTING_CANCEL_REQ = TradeListingKind.KIND_TRADE_LISTING_CANCEL_REQ;
export const KIND_TRADE_LISTING_CANCEL_RES = TradeListingKind.KIND_TRADE_LISTING_CANCEL_RES;
export const KIND_TRADE_LISTING_REFUND_REQ = TradeListingKind.KIND_TRADE_LISTING_REFUND_REQ;
export const KIND_TRADE_LISTING_REFUND_RES = TradeListingKind.KIND_TRADE_LISTING_REFUND_RES;

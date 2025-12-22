use core::fmt;
use core::str::FromStr;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(export, export_to = "types.ts", rename_all = "snake_case", repr(enum = name))
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TradeListingMarker {
    Listing,
    Payload,
    Previous,
    OrderResult,
    AcceptResult,
    ConveyanceResult,
    InvoiceResult,
    PaymentResult,
    FulfillmentResult,
    ReceiptResult,
    CancelResult,
    RefundResult,
    Proof,
}

impl TradeListingMarker {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            TradeListingMarker::Listing => "listing",
            TradeListingMarker::Payload => "payload",
            TradeListingMarker::Previous => "previous",
            TradeListingMarker::OrderResult => "order_result",
            TradeListingMarker::AcceptResult => "accept_result",
            TradeListingMarker::ConveyanceResult => "conveyance_result",
            TradeListingMarker::InvoiceResult => "invoice_result",
            TradeListingMarker::PaymentResult => "payment_result",
            TradeListingMarker::FulfillmentResult => "fulfillment_result",
            TradeListingMarker::ReceiptResult => "receipt_result",
            TradeListingMarker::CancelResult => "cancel_result",
            TradeListingMarker::RefundResult => "refund_result",
            TradeListingMarker::Proof => "proof",
        }
    }
}

pub const MARKER_LISTING: &str = TradeListingMarker::Listing.as_str();
pub const MARKER_PAYLOAD: &str = TradeListingMarker::Payload.as_str();
pub const MARKER_PREVIOUS: &str = TradeListingMarker::Previous.as_str();

pub const MARKER_ORDER_RESULT: &str = TradeListingMarker::OrderResult.as_str();
pub const MARKER_ACCEPT_RESULT: &str = TradeListingMarker::AcceptResult.as_str();
pub const MARKER_CONVEYANCE_RESULT: &str = TradeListingMarker::ConveyanceResult.as_str();
pub const MARKER_INVOICE_RESULT: &str = TradeListingMarker::InvoiceResult.as_str();
pub const MARKER_PAYMENT_RESULT: &str = TradeListingMarker::PaymentResult.as_str();
pub const MARKER_FULFILLMENT_RESULT: &str = TradeListingMarker::FulfillmentResult.as_str();
pub const MARKER_RECEIPT_RESULT: &str = TradeListingMarker::ReceiptResult.as_str();
pub const MARKER_CANCEL_RESULT: &str = TradeListingMarker::CancelResult.as_str();
pub const MARKER_REFUND_RESULT: &str = TradeListingMarker::RefundResult.as_str();
pub const MARKER_PROOF: &str = TradeListingMarker::Proof.as_str();

const MARKERS_ORDER_REQUEST: [&str; 2] = [MARKER_LISTING, MARKER_PAYLOAD];
const MARKERS_ACCEPT_REQUEST: [&str; 2] = [MARKER_ORDER_RESULT, MARKER_LISTING];
const MARKERS_CONVEYANCE_REQUEST: [&str; 2] = [MARKER_ACCEPT_RESULT, MARKER_PAYLOAD];
const MARKERS_INVOICE_REQUEST: [&str; 1] = [MARKER_ACCEPT_RESULT];
const MARKERS_PAYMENT_REQUEST: [&str; 2] = [MARKER_INVOICE_RESULT, MARKER_PROOF];
const MARKERS_FULFILLMENT_REQUEST: [&str; 1] = [MARKER_PAYMENT_RESULT];
const MARKERS_RECEIPT_REQUEST: [&str; 2] = [MARKER_FULFILLMENT_RESULT, MARKER_PAYLOAD];
const MARKERS_CANCEL_REQUEST: [&str; 2] = [MARKER_PREVIOUS, MARKER_PAYLOAD];
const MARKERS_REFUND_REQUEST: [&str; 2] = [MARKER_PAYMENT_RESULT, MARKER_PAYLOAD];

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
pub enum TradeListingStage {
    Order,
    Accept,
    Conveyance,
    Invoice,
    Payment,
    Fulfillment,
    Receipt,
    Cancel,
    Refund,
}

impl TradeListingStage {
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            TradeListingStage::Order => "order",
            TradeListingStage::Accept => "accept",
            TradeListingStage::Conveyance => "conveyance",
            TradeListingStage::Invoice => "invoice",
            TradeListingStage::Payment => "payment",
            TradeListingStage::Fulfillment => "fulfillment",
            TradeListingStage::Receipt => "receipt",
            TradeListingStage::Cancel => "cancel",
            TradeListingStage::Refund => "refund",
        }
    }

    #[inline]
    pub const fn request_kind(&self) -> u16 {
        match self {
            TradeListingStage::Order => crate::listing::kinds::KIND_TRADE_LISTING_ORDER_REQ,
            TradeListingStage::Accept => crate::listing::kinds::KIND_TRADE_LISTING_ACCEPT_REQ,
            TradeListingStage::Conveyance => {
                crate::listing::kinds::KIND_TRADE_LISTING_CONVEYANCE_REQ
            }
            TradeListingStage::Invoice => crate::listing::kinds::KIND_TRADE_LISTING_INVOICE_REQ,
            TradeListingStage::Payment => crate::listing::kinds::KIND_TRADE_LISTING_PAYMENT_REQ,
            TradeListingStage::Fulfillment => crate::listing::kinds::KIND_TRADE_LISTING_FULFILL_REQ,
            TradeListingStage::Receipt => crate::listing::kinds::KIND_TRADE_LISTING_RECEIPT_REQ,
            TradeListingStage::Cancel => crate::listing::kinds::KIND_TRADE_LISTING_CANCEL_REQ,
            TradeListingStage::Refund => crate::listing::kinds::KIND_TRADE_LISTING_REFUND_REQ,
        }
    }

    #[inline]
    pub const fn result_kind(&self) -> u16 {
        self.request_kind() + 1000
    }

    #[inline]
    pub const fn request_markers(&self) -> &'static [&'static str] {
        match self {
            TradeListingStage::Order => &MARKERS_ORDER_REQUEST,
            TradeListingStage::Accept => &MARKERS_ACCEPT_REQUEST,
            TradeListingStage::Conveyance => &MARKERS_CONVEYANCE_REQUEST,
            TradeListingStage::Invoice => &MARKERS_INVOICE_REQUEST,
            TradeListingStage::Payment => &MARKERS_PAYMENT_REQUEST,
            TradeListingStage::Fulfillment => &MARKERS_FULFILLMENT_REQUEST,
            TradeListingStage::Receipt => &MARKERS_RECEIPT_REQUEST,
            TradeListingStage::Cancel => &MARKERS_CANCEL_REQUEST,
            TradeListingStage::Refund => &MARKERS_REFUND_REQUEST,
        }
    }

    #[inline]
    pub const fn result_marker(&self) -> &'static str {
        match self {
            TradeListingStage::Order => MARKER_ORDER_RESULT,
            TradeListingStage::Accept => MARKER_ACCEPT_RESULT,
            TradeListingStage::Conveyance => MARKER_CONVEYANCE_RESULT,
            TradeListingStage::Invoice => MARKER_INVOICE_RESULT,
            TradeListingStage::Payment => MARKER_PAYMENT_RESULT,
            TradeListingStage::Fulfillment => MARKER_FULFILLMENT_RESULT,
            TradeListingStage::Receipt => MARKER_RECEIPT_RESULT,
            TradeListingStage::Cancel => MARKER_CANCEL_RESULT,
            TradeListingStage::Refund => MARKER_REFUND_RESULT,
        }
    }

    #[inline]
    pub const fn from_request_kind(kind: u16) -> Option<Self> {
        match kind {
            crate::listing::kinds::KIND_TRADE_LISTING_ORDER_REQ => Some(TradeListingStage::Order),
            crate::listing::kinds::KIND_TRADE_LISTING_ACCEPT_REQ => Some(TradeListingStage::Accept),
            crate::listing::kinds::KIND_TRADE_LISTING_CONVEYANCE_REQ => {
                Some(TradeListingStage::Conveyance)
            }
            crate::listing::kinds::KIND_TRADE_LISTING_INVOICE_REQ => {
                Some(TradeListingStage::Invoice)
            }
            crate::listing::kinds::KIND_TRADE_LISTING_PAYMENT_REQ => {
                Some(TradeListingStage::Payment)
            }
            crate::listing::kinds::KIND_TRADE_LISTING_FULFILL_REQ => {
                Some(TradeListingStage::Fulfillment)
            }
            crate::listing::kinds::KIND_TRADE_LISTING_RECEIPT_REQ => {
                Some(TradeListingStage::Receipt)
            }
            crate::listing::kinds::KIND_TRADE_LISTING_CANCEL_REQ => Some(TradeListingStage::Cancel),
            crate::listing::kinds::KIND_TRADE_LISTING_REFUND_REQ => Some(TradeListingStage::Refund),
            _ => None,
        }
    }

    #[inline]
    pub const fn from_result_kind(kind: u16) -> Option<Self> {
        match kind {
            crate::listing::kinds::KIND_TRADE_LISTING_ORDER_RES => Some(TradeListingStage::Order),
            crate::listing::kinds::KIND_TRADE_LISTING_ACCEPT_RES => Some(TradeListingStage::Accept),
            crate::listing::kinds::KIND_TRADE_LISTING_CONVEYANCE_RES => {
                Some(TradeListingStage::Conveyance)
            }
            crate::listing::kinds::KIND_TRADE_LISTING_INVOICE_RES => {
                Some(TradeListingStage::Invoice)
            }
            crate::listing::kinds::KIND_TRADE_LISTING_PAYMENT_RES => {
                Some(TradeListingStage::Payment)
            }
            crate::listing::kinds::KIND_TRADE_LISTING_FULFILL_RES => {
                Some(TradeListingStage::Fulfillment)
            }
            crate::listing::kinds::KIND_TRADE_LISTING_RECEIPT_RES => {
                Some(TradeListingStage::Receipt)
            }
            crate::listing::kinds::KIND_TRADE_LISTING_CANCEL_RES => Some(TradeListingStage::Cancel),
            crate::listing::kinds::KIND_TRADE_LISTING_REFUND_RES => Some(TradeListingStage::Refund),
            _ => None,
        }
    }
}

impl fmt::Display for TradeListingStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeListingStageParseError {
    UnknownStage,
}

impl fmt::Display for TradeListingStageParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradeListingStageParseError::UnknownStage => {
                write!(f, "unknown trade listing stage")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TradeListingStageParseError {}

impl FromStr for TradeListingStage {
    type Err = TradeListingStageParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "order" => Ok(Self::Order),
            "accept" => Ok(Self::Accept),
            "conveyance" => Ok(Self::Conveyance),
            "invoice" => Ok(Self::Invoice),
            "payment" => Ok(Self::Payment),
            "fulfillment" => Ok(Self::Fulfillment),
            "receipt" => Ok(Self::Receipt),
            "cancel" => Ok(Self::Cancel),
            "refund" => Ok(Self::Refund),
            _ => Err(TradeListingStageParseError::UnknownStage),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MARKER_CONVEYANCE_RESULT, MARKER_FULFILLMENT_RESULT, MARKER_INVOICE_RESULT, MARKER_LISTING,
        MARKER_ORDER_RESULT, MARKER_PAYLOAD, MARKER_PAYMENT_RESULT, MARKER_PROOF,
        MARKER_RECEIPT_RESULT, TradeListingStage, TradeListingStageParseError,
    };

    #[test]
    fn stage_roundtrip() {
        let cases = [
            (TradeListingStage::Order, "order"),
            (TradeListingStage::Accept, "accept"),
            (TradeListingStage::Conveyance, "conveyance"),
            (TradeListingStage::Invoice, "invoice"),
            (TradeListingStage::Payment, "payment"),
            (TradeListingStage::Fulfillment, "fulfillment"),
            (TradeListingStage::Receipt, "receipt"),
            (TradeListingStage::Cancel, "cancel"),
            (TradeListingStage::Refund, "refund"),
        ];

        for (stage, name) in cases {
            assert_eq!(stage.as_str(), name);
            assert_eq!(stage.to_string(), name);
            assert_eq!(name.parse::<TradeListingStage>().unwrap(), stage);
        }
    }

    #[test]
    fn stage_parse_rejects_unknown() {
        let err = "unknown".parse::<TradeListingStage>().unwrap_err();
        assert_eq!(err, TradeListingStageParseError::UnknownStage);
    }

    #[test]
    fn stage_kinds_follow_nip_90() {
        let cases = [
            TradeListingStage::Order,
            TradeListingStage::Accept,
            TradeListingStage::Conveyance,
            TradeListingStage::Invoice,
            TradeListingStage::Payment,
            TradeListingStage::Fulfillment,
            TradeListingStage::Receipt,
            TradeListingStage::Cancel,
            TradeListingStage::Refund,
        ];

        for stage in cases {
            assert_eq!(stage.result_kind(), stage.request_kind() + 1000);
            assert_eq!(
                TradeListingStage::from_request_kind(stage.request_kind()),
                Some(stage)
            );
            assert_eq!(
                TradeListingStage::from_result_kind(stage.result_kind()),
                Some(stage)
            );
        }
    }

    #[test]
    fn stage_markers_cover_expected_inputs() {
        assert_eq!(
            TradeListingStage::Order.request_markers(),
            &[MARKER_LISTING, MARKER_PAYLOAD]
        );
        assert_eq!(
            TradeListingStage::Accept.request_markers(),
            &[MARKER_ORDER_RESULT, MARKER_LISTING]
        );
        assert_eq!(
            TradeListingStage::Payment.request_markers(),
            &[MARKER_INVOICE_RESULT, MARKER_PROOF]
        );
        assert_eq!(
            TradeListingStage::Fulfillment.request_markers(),
            &[MARKER_PAYMENT_RESULT]
        );
        assert_eq!(
            TradeListingStage::Receipt.request_markers(),
            &[MARKER_FULFILLMENT_RESULT, MARKER_PAYLOAD]
        );
        assert_eq!(
            TradeListingStage::Refund.request_markers(),
            &[MARKER_PAYMENT_RESULT, MARKER_PAYLOAD]
        );
        assert_eq!(
            TradeListingStage::Order.result_marker(),
            MARKER_ORDER_RESULT
        );
        assert_eq!(
            TradeListingStage::Conveyance.result_marker(),
            MARKER_CONVEYANCE_RESULT
        );
        assert_eq!(
            TradeListingStage::Receipt.result_marker(),
            MARKER_RECEIPT_RESULT
        );
    }
}

#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_core::RadrootsCoreDiscountValue;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeOrderItem {
    pub bin_id: String,
    pub bin_count: u32,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind", content = "amount"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeOrderChange {
    BinCount {
        item_index: u32,
        bin_count: u32,
    },
    ItemAdd {
        item: TradeOrderItem,
    },
    ItemRemove {
        item_index: u32,
    },
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeOrderRevision {
    pub revision_id: String,
    pub order_id: String,
    pub changes: Vec<TradeOrderChange>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeOrder {
    pub order_id: String,
    pub listing_addr: String,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub items: Vec<TradeOrderItem>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsCoreDiscountValue[] | null")
    )]
    pub discounts: Option<Vec<RadrootsCoreDiscountValue>>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub notes: Option<String>,
    pub status: TradeOrderStatus,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeOrderStatus {
    Draft,
    Validated,
    Requested,
    Questioned,
    Revised,
    Accepted,
    Declined,
    Cancelled,
    Fulfilled,
    Completed,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeQuestion {
    pub question_id: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub order_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub listing_addr: Option<String>,
    pub question_text: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeAnswer {
    pub question_id: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub order_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub listing_addr: Option<String>,
    pub answer_text: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeDiscountRequest {
    pub discount_id: String,
    pub order_id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDiscountValue"))]
    pub value: RadrootsCoreDiscountValue,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub conditions: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeDiscountOffer {
    pub discount_id: String,
    pub order_id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDiscountValue"))]
    pub value: RadrootsCoreDiscountValue,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub conditions: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind", content = "amount"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeDiscountDecision {
    Accept {
        #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDiscountValue"))]
        value: RadrootsCoreDiscountValue,
    },
    Decline {
        #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
        reason: Option<String>,
    },
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind", content = "amount"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeFulfillmentStatus {
    Preparing,
    Shipped,
    ReadyForPickup,
    Delivered,
    Cancelled,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeFulfillmentUpdate {
    pub status: TradeFulfillmentStatus,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub tracking: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub eta: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub notes: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeReceipt {
    pub acknowledged: bool,
    pub at: u64,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub note: Option<String>,
}

#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use crate::listing::projection::{
    RadrootsTradeListingProjection, RadrootsTradeListingQuery, RadrootsTradeListingSort,
    RadrootsTradeMarketplaceListingSummary, RadrootsTradeMarketplaceOrderSummary,
    RadrootsTradeOrderQuery, RadrootsTradeOrderSort, RadrootsTradeOrderWorkflowProjection,
    RadrootsTradeReadIndex,
};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeReviewPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeReviewStatus {
    Queued,
    InProgress,
    Blocked,
    Resolved,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeReviewQueueEntry {
    pub queue: String,
    pub priority: RadrootsTradeReviewPriority,
    pub status: RadrootsTradeReviewStatus,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub assigned_operator: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

impl RadrootsTradeReviewQueueEntry {
    pub fn requires_review(&self) -> bool {
        !matches!(self.status, RadrootsTradeReviewStatus::Resolved)
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeModerationSeverity {
    Notice,
    Warning,
    Block,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeModerationStatus {
    Open,
    Snoozed,
    Resolved,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeModerationFlag {
    pub code: String,
    pub severity: RadrootsTradeModerationSeverity,
    pub status: RadrootsTradeModerationStatus,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub source: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

impl RadrootsTradeModerationFlag {
    pub fn is_open(&self) -> bool {
        !matches!(self.status, RadrootsTradeModerationStatus::Resolved)
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeFulfillmentExceptionSeverity {
    Notice,
    Warning,
    Blocking,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeFulfillmentExceptionStatus {
    Open,
    Monitoring,
    Resolved,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeFulfillmentException {
    pub code: String,
    pub severity: RadrootsTradeFulfillmentExceptionSeverity,
    pub status: RadrootsTradeFulfillmentExceptionStatus,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub source: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub notes: Option<String>,
}

impl RadrootsTradeFulfillmentException {
    pub fn is_open(&self) -> bool {
        !matches!(
            self.status,
            RadrootsTradeFulfillmentExceptionStatus::Resolved
        )
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeListingBackofficeOverlay {
    pub listing_addr: String,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsTradeReviewQueueEntry | null")
    )]
    pub review_queue: Option<RadrootsTradeReviewQueueEntry>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeModerationFlag[]"))]
    pub moderation_flags: Vec<RadrootsTradeModerationFlag>,
}

impl RadrootsTradeListingBackofficeOverlay {
    pub fn requires_review(&self) -> bool {
        self.review_queue
            .as_ref()
            .is_some_and(RadrootsTradeReviewQueueEntry::requires_review)
    }

    pub fn open_moderation_flag_count(&self) -> u32 {
        self.moderation_flags.iter().fold(0u32, |count, flag| {
            if flag.is_open() {
                count.saturating_add(1)
            } else {
                count
            }
        })
    }

    pub fn has_open_moderation_flags(&self) -> bool {
        self.open_moderation_flag_count() > 0
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderBackofficeOverlay {
    pub order_id: String,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsTradeReviewQueueEntry | null")
    )]
    pub review_queue: Option<RadrootsTradeReviewQueueEntry>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeModerationFlag[]"))]
    pub moderation_flags: Vec<RadrootsTradeModerationFlag>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFulfillmentException[]"))]
    pub fulfillment_exceptions: Vec<RadrootsTradeFulfillmentException>,
}

impl RadrootsTradeOrderBackofficeOverlay {
    pub fn requires_review(&self) -> bool {
        self.review_queue
            .as_ref()
            .is_some_and(RadrootsTradeReviewQueueEntry::requires_review)
    }

    pub fn open_moderation_flag_count(&self) -> u32 {
        self.moderation_flags.iter().fold(0u32, |count, flag| {
            if flag.is_open() {
                count.saturating_add(1)
            } else {
                count
            }
        })
    }

    pub fn has_open_moderation_flags(&self) -> bool {
        self.open_moderation_flag_count() > 0
    }

    pub fn open_fulfillment_exception_count(&self) -> u32 {
        self.fulfillment_exceptions
            .iter()
            .fold(0u32, |count, exception| {
                if exception.is_open() {
                    count.saturating_add(1)
                } else {
                    count
                }
            })
    }

    pub fn has_open_fulfillment_exceptions(&self) -> bool {
        self.open_fulfillment_exception_count() > 0
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsTradeListingBackofficeQuery {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeListingQuery"))]
    pub listing: RadrootsTradeListingQuery,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "boolean | null"))]
    pub requires_review: Option<bool>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "boolean | null"))]
    pub has_open_moderation_flags: Option<bool>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsTradeOrderBackofficeQuery {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeOrderQuery"))]
    pub order: RadrootsTradeOrderQuery,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "boolean | null"))]
    pub requires_review: Option<bool>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "boolean | null"))]
    pub has_open_moderation_flags: Option<bool>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "boolean | null"))]
    pub has_open_fulfillment_exceptions: Option<bool>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsTradeListingBackofficeView {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeListingProjection"))]
    pub listing: RadrootsTradeListingProjection,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsTradeMarketplaceListingSummary | null")
    )]
    pub marketplace: Option<RadrootsTradeMarketplaceListingSummary>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsTradeListingBackofficeOverlay | null")
    )]
    pub overlay: Option<RadrootsTradeListingBackofficeOverlay>,
    pub requires_review: bool,
    pub open_moderation_flag_count: u32,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsTradeOrderBackofficeView {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeOrderWorkflowProjection"))]
    pub order: RadrootsTradeOrderWorkflowProjection,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeMarketplaceOrderSummary"))]
    pub marketplace: RadrootsTradeMarketplaceOrderSummary,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsTradeOrderBackofficeOverlay | null")
    )]
    pub overlay: Option<RadrootsTradeOrderBackofficeOverlay>,
    pub requires_review: bool,
    pub open_moderation_flag_count: u32,
    pub open_fulfillment_exception_count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct RadrootsTradeBackofficeOverlayStore {
    listing_overlays: BTreeMap<String, RadrootsTradeListingBackofficeOverlay>,
    order_overlays: BTreeMap<String, RadrootsTradeOrderBackofficeOverlay>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeBackofficeOverlayError {
    MissingListingAddr,
    MissingOrderId,
}

impl core::fmt::Display for RadrootsTradeBackofficeOverlayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingListingAddr => write!(f, "missing listing address"),
            Self::MissingOrderId => write!(f, "missing order id"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTradeBackofficeOverlayError {}

impl RadrootsTradeBackofficeOverlayStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn listing_overlays(&self) -> &BTreeMap<String, RadrootsTradeListingBackofficeOverlay> {
        &self.listing_overlays
    }

    pub fn order_overlays(&self) -> &BTreeMap<String, RadrootsTradeOrderBackofficeOverlay> {
        &self.order_overlays
    }

    pub fn listing_overlay(
        &self,
        listing_addr: &str,
    ) -> Option<&RadrootsTradeListingBackofficeOverlay> {
        self.listing_overlays.get(listing_addr)
    }

    pub fn order_overlay(&self, order_id: &str) -> Option<&RadrootsTradeOrderBackofficeOverlay> {
        self.order_overlays.get(order_id)
    }

    pub fn upsert_listing_overlay(
        &mut self,
        overlay: RadrootsTradeListingBackofficeOverlay,
    ) -> Result<&RadrootsTradeListingBackofficeOverlay, RadrootsTradeBackofficeOverlayError> {
        if overlay.listing_addr.is_empty() {
            return Err(RadrootsTradeBackofficeOverlayError::MissingListingAddr);
        }
        let listing_addr = overlay.listing_addr.clone();
        self.listing_overlays.insert(listing_addr.clone(), overlay);
        Ok(self
            .listing_overlays
            .get(&listing_addr)
            .expect("listing overlay should exist after upsert"))
    }

    pub fn upsert_order_overlay(
        &mut self,
        overlay: RadrootsTradeOrderBackofficeOverlay,
    ) -> Result<&RadrootsTradeOrderBackofficeOverlay, RadrootsTradeBackofficeOverlayError> {
        if overlay.order_id.is_empty() {
            return Err(RadrootsTradeBackofficeOverlayError::MissingOrderId);
        }
        let order_id = overlay.order_id.clone();
        self.order_overlays.insert(order_id.clone(), overlay);
        Ok(self
            .order_overlays
            .get(&order_id)
            .expect("order overlay should exist after upsert"))
    }

    pub fn merge_listing_projection(
        &self,
        listing: &RadrootsTradeListingProjection,
    ) -> RadrootsTradeListingBackofficeView {
        let overlay = self.listing_overlay(&listing.listing_addr).cloned();
        let requires_review = overlay
            .as_ref()
            .is_some_and(RadrootsTradeListingBackofficeOverlay::requires_review);
        let open_moderation_flag_count = overlay.as_ref().map_or(
            0,
            RadrootsTradeListingBackofficeOverlay::open_moderation_flag_count,
        );

        RadrootsTradeListingBackofficeView {
            listing: listing.clone(),
            marketplace: listing.marketplace_summary(),
            overlay,
            requires_review,
            open_moderation_flag_count,
        }
    }

    pub fn merge_order_projection(
        &self,
        order: &RadrootsTradeOrderWorkflowProjection,
    ) -> RadrootsTradeOrderBackofficeView {
        let overlay = self.order_overlay(&order.order_id).cloned();
        let requires_review = overlay
            .as_ref()
            .is_some_and(RadrootsTradeOrderBackofficeOverlay::requires_review);
        let open_moderation_flag_count = overlay.as_ref().map_or(
            0,
            RadrootsTradeOrderBackofficeOverlay::open_moderation_flag_count,
        );
        let open_fulfillment_exception_count = overlay.as_ref().map_or(
            0,
            RadrootsTradeOrderBackofficeOverlay::open_fulfillment_exception_count,
        );

        RadrootsTradeOrderBackofficeView {
            order: order.clone(),
            marketplace: order.marketplace_summary(),
            overlay,
            requires_review,
            open_moderation_flag_count,
            open_fulfillment_exception_count,
        }
    }

    pub fn listing_backoffice_views(
        &self,
        read_index: &RadrootsTradeReadIndex,
        query: &RadrootsTradeListingBackofficeQuery,
        sort: RadrootsTradeListingSort,
    ) -> Vec<RadrootsTradeListingBackofficeView> {
        read_index
            .query_listings(&query.listing, sort)
            .into_iter()
            .map(|listing| self.merge_listing_projection(listing))
            .filter(|view| listing_backoffice_matches_query(view, query))
            .collect()
    }

    pub fn order_backoffice_views(
        &self,
        read_index: &RadrootsTradeReadIndex,
        query: &RadrootsTradeOrderBackofficeQuery,
        sort: RadrootsTradeOrderSort,
    ) -> Vec<RadrootsTradeOrderBackofficeView> {
        read_index
            .query_orders(&query.order, sort)
            .into_iter()
            .map(|order| self.merge_order_projection(order))
            .filter(|view| order_backoffice_matches_query(view, query))
            .collect()
    }
}

fn bool_filter_matches(value: bool, filter: Option<bool>) -> bool {
    match filter {
        Some(expected) => expected == value,
        None => true,
    }
}

fn listing_backoffice_matches_query(
    view: &RadrootsTradeListingBackofficeView,
    query: &RadrootsTradeListingBackofficeQuery,
) -> bool {
    bool_filter_matches(view.requires_review, query.requires_review)
        && bool_filter_matches(
            view.open_moderation_flag_count > 0,
            query.has_open_moderation_flags,
        )
}

fn order_backoffice_matches_query(
    view: &RadrootsTradeOrderBackofficeView,
    query: &RadrootsTradeOrderBackofficeQuery,
) -> bool {
    bool_filter_matches(view.requires_review, query.requires_review)
        && bool_filter_matches(
            view.open_moderation_flag_count > 0,
            query.has_open_moderation_flags,
        )
        && bool_filter_matches(
            view.open_fulfillment_exception_count > 0,
            query.has_open_fulfillment_exceptions,
        )
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{
        RadrootsTradeBackofficeOverlayError, RadrootsTradeBackofficeOverlayStore,
        RadrootsTradeFulfillmentException, RadrootsTradeFulfillmentExceptionSeverity,
        RadrootsTradeFulfillmentExceptionStatus, RadrootsTradeListingBackofficeOverlay,
        RadrootsTradeListingBackofficeQuery, RadrootsTradeModerationFlag,
        RadrootsTradeModerationSeverity, RadrootsTradeModerationStatus,
        RadrootsTradeOrderBackofficeOverlay, RadrootsTradeOrderBackofficeQuery,
        RadrootsTradeReviewPriority, RadrootsTradeReviewQueueEntry, RadrootsTradeReviewStatus,
    };
    use crate::listing::{
        dvm::{TradeListingCancel, TradeListingMessagePayload, TradeOrderResponse},
        projection::RadrootsTradeOrderWorkflowMessage,
    };
    use crate::listing::{
        order::{
            TradeFulfillmentStatus, TradeFulfillmentUpdate, TradeOrder, TradeOrderItem,
            TradeOrderStatus, TradeReceipt,
        },
        projection::{
            RadrootsTradeListingSort, RadrootsTradeListingSortField, RadrootsTradeOrderQuery,
            RadrootsTradeOrderSort, RadrootsTradeOrderSortField, RadrootsTradeReadIndex,
            RadrootsTradeSortDirection,
        },
    };
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCorePercent,
        RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::RadrootsNostrEventPtr;
    use radroots_events::farm::RadrootsFarmRef;
    use radroots_events::listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingLocation, RadrootsListingProduct,
        RadrootsListingStatus,
    };

    #[derive(Clone, Debug)]
    struct TestWorkflowChain {
        buyer_pubkey: String,
        seller_pubkey: String,
        root_event_id: String,
        last_event_id: String,
        next_sequence: u32,
    }

    thread_local! {
        static TEST_WORKFLOW_CHAINS: RefCell<std::collections::BTreeMap<String, TestWorkflowChain>> =
            RefCell::new(std::collections::BTreeMap::new());
    }

    fn listing_snapshot(listing_addr: &str) -> RadrootsNostrEventPtr {
        RadrootsNostrEventPtr {
            id: format!("snapshot:{listing_addr}"),
            relays: None,
        }
    }

    fn seller_pubkey_from_listing_addr(listing_addr: &str) -> String {
        listing_addr
            .split(':')
            .nth(1)
            .unwrap_or_default()
            .to_string()
    }

    fn workflow_refs(
        actor_pubkey: &str,
        listing_addr: &str,
        order_id: Option<&str>,
        payload: &TradeListingMessagePayload,
    ) -> (
        String,
        String,
        Option<RadrootsNostrEventPtr>,
        Option<String>,
        Option<String>,
    ) {
        let message_type = payload.message_type();
        let listing_event = message_type
            .requires_listing_snapshot()
            .then(|| listing_snapshot(listing_addr));
        let default_seller = seller_pubkey_from_listing_addr(listing_addr);

        match (payload, order_id) {
            (_, None) => (
                format!("event:no-order:{}:{actor_pubkey}", message_type.kind()),
                default_seller,
                listing_event,
                None,
                None,
            ),
            (TradeListingMessagePayload::OrderRequest(order), Some(order_id)) => {
                let event_id = format!("{order_id}:request");
                TEST_WORKFLOW_CHAINS.with(|chains| {
                    chains.borrow_mut().insert(
                        order_id.to_string(),
                        TestWorkflowChain {
                            buyer_pubkey: order.buyer_pubkey.clone(),
                            seller_pubkey: order.seller_pubkey.clone(),
                            root_event_id: event_id.clone(),
                            last_event_id: event_id.clone(),
                            next_sequence: 1,
                        },
                    );
                });
                (
                    event_id,
                    order.seller_pubkey.clone(),
                    listing_event,
                    None,
                    None,
                )
            }
            (_, Some(order_id)) => TEST_WORKFLOW_CHAINS.with(|chains| {
                let mut chains = chains.borrow_mut();
                let chain =
                    chains
                        .entry(order_id.to_string())
                        .or_insert_with(|| TestWorkflowChain {
                            buyer_pubkey: String::from("buyer-pubkey"),
                            seller_pubkey: default_seller.clone(),
                            root_event_id: format!("{order_id}:root"),
                            last_event_id: format!("{order_id}:root"),
                            next_sequence: 1,
                        });
                let event_id =
                    format!("{order_id}:{}:{}", message_type.kind(), chain.next_sequence);
                chain.next_sequence += 1;
                let counterparty_pubkey = if actor_pubkey == chain.seller_pubkey {
                    chain.buyer_pubkey.clone()
                } else {
                    chain.seller_pubkey.clone()
                };
                let prev_event_id = chain.last_event_id.clone();
                let root_event_id = chain.root_event_id.clone();
                chain.last_event_id = event_id.clone();
                (
                    event_id,
                    counterparty_pubkey,
                    listing_event,
                    Some(root_event_id),
                    Some(prev_event_id),
                )
            }),
        }
    }

    fn base_listing() -> RadrootsListing {
        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAg".into(),
            farm: RadrootsFarmRef {
                pubkey: "farm-pubkey".into(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
            },
            product: RadrootsListingProduct {
                key: "coffee".into(),
                title: "Coffee".into(),
                category: "coffee".into(),
                summary: Some("single origin".into()),
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: "bin-1".into(),
            bins: vec![RadrootsListingBin {
                bin_id: "bin-1".into(),
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(1000u32),
                    RadrootsCoreUnit::MassG,
                ),
                price_per_canonical_unit: RadrootsCoreQuantityPrice::new(
                    RadrootsCoreMoney::new(
                        RadrootsCoreDecimal::from(2u32),
                        RadrootsCoreCurrency::USD,
                    ),
                    RadrootsCoreQuantity::new(
                        RadrootsCoreDecimal::from(1u32),
                        RadrootsCoreUnit::MassG,
                    ),
                ),
                display_amount: None,
                display_unit: None,
                display_label: Some("1kg bag".into()),
                display_price: Some(RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(2000u32),
                    RadrootsCoreCurrency::USD,
                )),
                display_price_unit: Some(RadrootsCoreUnit::Each),
            }],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: Some(RadrootsCoreDecimal::from(10u32)),
            availability: Some(RadrootsListingAvailability::Status {
                status: RadrootsListingStatus::Active,
            }),
            delivery_method: Some(RadrootsListingDeliveryMethod::Pickup),
            location: Some(RadrootsListingLocation {
                primary: "farm".into(),
                city: Some("Nashville".into()),
                region: Some("TN".into()),
                country: Some("US".into()),
                lat: None,
                lng: None,
                geohash: None,
            }),
            images: None,
        }
    }

    fn alternate_listing() -> RadrootsListing {
        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".into(),
            farm: RadrootsFarmRef {
                pubkey: "farm-pubkey-2".into(),
                d_tag: "AAAAAAAAAAAAAAAAAAAABA".into(),
            },
            product: RadrootsListingProduct {
                key: "greens".into(),
                title: "Greens".into(),
                category: "vegetables".into(),
                summary: Some("washed bunches".into()),
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: "bin-1".into(),
            bins: vec![RadrootsListingBin {
                bin_id: "bin-1".into(),
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(500u32),
                    RadrootsCoreUnit::MassG,
                ),
                price_per_canonical_unit: RadrootsCoreQuantityPrice::new(
                    RadrootsCoreMoney::new(
                        RadrootsCoreDecimal::from(4u32),
                        RadrootsCoreCurrency::USD,
                    ),
                    RadrootsCoreQuantity::new(
                        RadrootsCoreDecimal::from(1u32),
                        RadrootsCoreUnit::MassG,
                    ),
                ),
                display_amount: None,
                display_unit: None,
                display_label: Some("500g bunch".into()),
                display_price: Some(RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(2000u32),
                    RadrootsCoreCurrency::USD,
                )),
                display_price_unit: Some(RadrootsCoreUnit::Each),
            }],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: Some(RadrootsCoreDecimal::from(4u32)),
            availability: Some(RadrootsListingAvailability::Status {
                status: RadrootsListingStatus::Sold,
            }),
            delivery_method: Some(RadrootsListingDeliveryMethod::Shipping),
            location: Some(RadrootsListingLocation {
                primary: "warehouse".into(),
                city: Some("Louisville".into()),
                region: Some("KY".into()),
                country: Some("US".into()),
                lat: None,
                lng: None,
                geohash: None,
            }),
            images: None,
        }
    }

    fn base_order() -> TradeOrder {
        TradeOrder {
            order_id: "order-1".into(),
            listing_addr: "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg".into(),
            buyer_pubkey: "buyer-pubkey".into(),
            seller_pubkey: "seller-pubkey".into(),
            items: vec![TradeOrderItem {
                bin_id: "bin-1".into(),
                bin_count: 2,
            }],
            discounts: Some(vec![radroots_core::RadrootsCoreDiscountValue::Percent(
                RadrootsCorePercent::new(RadrootsCoreDecimal::from(10u32)),
            )]),
        }
    }

    fn alternate_order() -> TradeOrder {
        TradeOrder {
            order_id: "order-2".into(),
            listing_addr: "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw".into(),
            buyer_pubkey: "buyer-pubkey-2".into(),
            seller_pubkey: "seller-pubkey".into(),
            items: vec![TradeOrderItem {
                bin_id: "bin-1".into(),
                bin_count: 3,
            }],
            discounts: None,
        }
    }

    fn message(
        actor_pubkey: &str,
        listing_addr: &str,
        order_id: Option<&str>,
        payload: TradeListingMessagePayload,
    ) -> RadrootsTradeOrderWorkflowMessage {
        let (event_id, counterparty_pubkey, listing_event, root_event_id, prev_event_id) =
            workflow_refs(actor_pubkey, listing_addr, order_id, &payload);
        RadrootsTradeOrderWorkflowMessage {
            event_id,
            actor_pubkey: actor_pubkey.into(),
            counterparty_pubkey,
            listing_addr: listing_addr.into(),
            order_id: order_id.map(str::to_string),
            listing_event,
            root_event_id,
            prev_event_id,
            payload,
        }
    }

    #[test]
    fn overlay_helpers_and_store_accessors_cover_flags_and_errors() {
        let review_entry = RadrootsTradeReviewQueueEntry {
            queue: "queue".into(),
            priority: RadrootsTradeReviewPriority::Normal,
            status: RadrootsTradeReviewStatus::Resolved,
            assigned_operator: None,
            reason: None,
        };
        assert!(!review_entry.requires_review());

        let listing_overlay = RadrootsTradeListingBackofficeOverlay {
            listing_addr: "listing-1".into(),
            review_queue: Some(review_entry),
            moderation_flags: vec![
                RadrootsTradeModerationFlag {
                    code: "resolved".into(),
                    severity: RadrootsTradeModerationSeverity::Notice,
                    status: RadrootsTradeModerationStatus::Resolved,
                    source: None,
                    reason: None,
                },
                RadrootsTradeModerationFlag {
                    code: "open".into(),
                    severity: RadrootsTradeModerationSeverity::Warning,
                    status: RadrootsTradeModerationStatus::Open,
                    source: None,
                    reason: None,
                },
            ],
        };
        assert!(!listing_overlay.requires_review());
        assert_eq!(listing_overlay.open_moderation_flag_count(), 1);
        assert!(listing_overlay.has_open_moderation_flags());

        let order_overlay = RadrootsTradeOrderBackofficeOverlay {
            order_id: "order-1".into(),
            review_queue: Some(RadrootsTradeReviewQueueEntry {
                queue: "queue".into(),
                priority: RadrootsTradeReviewPriority::Low,
                status: RadrootsTradeReviewStatus::Resolved,
                assigned_operator: None,
                reason: None,
            }),
            moderation_flags: vec![RadrootsTradeModerationFlag {
                code: "resolved".into(),
                severity: RadrootsTradeModerationSeverity::Notice,
                status: RadrootsTradeModerationStatus::Resolved,
                source: None,
                reason: None,
            }],
            fulfillment_exceptions: vec![
                RadrootsTradeFulfillmentException {
                    code: "resolved".into(),
                    severity: RadrootsTradeFulfillmentExceptionSeverity::Notice,
                    status: RadrootsTradeFulfillmentExceptionStatus::Resolved,
                    source: None,
                    notes: None,
                },
                RadrootsTradeFulfillmentException {
                    code: "open".into(),
                    severity: RadrootsTradeFulfillmentExceptionSeverity::Blocking,
                    status: RadrootsTradeFulfillmentExceptionStatus::Open,
                    source: None,
                    notes: None,
                },
            ],
        };
        assert!(!order_overlay.requires_review());
        assert_eq!(order_overlay.open_moderation_flag_count(), 0);
        assert!(!order_overlay.has_open_moderation_flags());
        assert_eq!(order_overlay.open_fulfillment_exception_count(), 1);
        assert!(order_overlay.has_open_fulfillment_exceptions());

        let mut store = RadrootsTradeBackofficeOverlayStore::new();
        assert!(store.listing_overlays().is_empty());
        assert!(store.order_overlays().is_empty());
        store
            .upsert_listing_overlay(listing_overlay)
            .expect("listing overlay");
        store
            .upsert_order_overlay(order_overlay)
            .expect("order overlay");
        assert_eq!(store.listing_overlays().len(), 1);
        assert_eq!(store.order_overlays().len(), 1);
        assert!(store.listing_overlay("listing-1").is_some());
        assert!(store.order_overlay("order-1").is_some());

        let missing_listing = RadrootsTradeBackofficeOverlayError::MissingListingAddr;
        let missing_order = RadrootsTradeBackofficeOverlayError::MissingOrderId;
        assert_eq!(missing_listing.to_string(), "missing listing address");
        assert_eq!(missing_order.to_string(), "missing order id");
        assert!(std::error::Error::source(&missing_listing).is_none());
    }

    #[test]
    fn message_helper_bootstraps_missing_chain_for_non_request_payload() {
        let orphan_message = message(
            "seller-pubkey",
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            Some("orphan-order"),
            TradeListingMessagePayload::Cancel(TradeListingCancel {
                reason: Some("operator-cancelled".into()),
            }),
        );

        assert_eq!(orphan_message.order_id.as_deref(), Some("orphan-order"));
        assert_eq!(orphan_message.counterparty_pubkey, "buyer-pubkey");
        assert_eq!(
            orphan_message.root_event_id.as_deref(),
            Some("orphan-order:root")
        );
        assert_eq!(
            orphan_message.prev_event_id.as_deref(),
            Some("orphan-order:root")
        );

        let no_order_message = message(
            "seller-pubkey",
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            None,
            TradeListingMessagePayload::Cancel(TradeListingCancel {
                reason: Some("operator-cancelled".into()),
            }),
        );
        assert!(no_order_message.order_id.is_none());
        assert!(no_order_message.root_event_id.is_none());
        assert!(no_order_message.prev_event_id.is_none());
    }

    #[test]
    fn listing_backoffice_views_merge_overlay_without_mutating_canonical_projection() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("base listing");
        index
            .upsert_listing("seller-pubkey", &alternate_listing())
            .expect("alternate listing");

        let mut overlays = RadrootsTradeBackofficeOverlayStore::new();
        overlays
            .upsert_listing_overlay(RadrootsTradeListingBackofficeOverlay {
                listing_addr: "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg".into(),
                review_queue: Some(RadrootsTradeReviewQueueEntry {
                    queue: "listing-review".into(),
                    priority: RadrootsTradeReviewPriority::High,
                    status: RadrootsTradeReviewStatus::Queued,
                    assigned_operator: Some("ops-1".into()),
                    reason: Some("verify organic claim".into()),
                }),
                moderation_flags: vec![RadrootsTradeModerationFlag {
                    code: "needs-copy-review".into(),
                    severity: RadrootsTradeModerationSeverity::Warning,
                    status: RadrootsTradeModerationStatus::Open,
                    source: Some("policy".into()),
                    reason: Some("contains superlative marketing copy".into()),
                }],
            })
            .expect("listing overlay");

        let views = overlays.listing_backoffice_views(
            &index,
            &RadrootsTradeListingBackofficeQuery {
                has_open_moderation_flags: Some(true),
                ..Default::default()
            },
            RadrootsTradeListingSort {
                field: RadrootsTradeListingSortField::ListingAddr,
                direction: RadrootsTradeSortDirection::Asc,
            },
        );

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].listing.listing_addr, base_order().listing_addr);
        assert!(views[0].requires_review);
        assert_eq!(views[0].open_moderation_flag_count, 1);
        assert!(!super::listing_backoffice_matches_query(
            &views[0],
            &RadrootsTradeListingBackofficeQuery {
                requires_review: Some(false),
                has_open_moderation_flags: Some(false),
                ..Default::default()
            }
        ));
        assert_eq!(
            views[0]
                .overlay
                .as_ref()
                .and_then(|overlay| overlay.review_queue.as_ref())
                .and_then(|entry| entry.assigned_operator.as_deref()),
            Some("ops-1")
        );

        let listing = index
            .listing("30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg")
            .expect("canonical listing");
        assert_eq!(listing.order_count, 0);
        assert_eq!(listing.open_order_count, 0);
        assert_eq!(listing.terminal_order_count, 0);
    }

    #[test]
    fn order_backoffice_views_filter_review_and_exception_overlays() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("base listing");
        index
            .upsert_listing("seller-pubkey", &alternate_listing())
            .expect("alternate listing");

        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("first order");
        index
            .apply_workflow_message(&message(
                "buyer-pubkey-2",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::OrderRequest(alternate_order()),
            ))
            .expect("second order");
        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                    accepted: true,
                    reason: Some("approved".into()),
                }),
            ))
            .expect("accepted");
        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::FulfillmentUpdate(TradeFulfillmentUpdate {
                    status: TradeFulfillmentStatus::Delivered,
                }),
            ))
            .expect("fulfilled");
        index
            .apply_workflow_message(&message(
                "buyer-pubkey-2",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::Receipt(TradeReceipt {
                    acknowledged: true,
                    at: 1_700_000_020,
                }),
            ))
            .expect("completed");

        let mut overlays = RadrootsTradeBackofficeOverlayStore::new();
        overlays
            .upsert_order_overlay(RadrootsTradeOrderBackofficeOverlay {
                order_id: "order-1".into(),
                review_queue: Some(RadrootsTradeReviewQueueEntry {
                    queue: "order-review".into(),
                    priority: RadrootsTradeReviewPriority::Critical,
                    status: RadrootsTradeReviewStatus::InProgress,
                    assigned_operator: Some("ops-2".into()),
                    reason: Some("buyer requested rush handling".into()),
                }),
                moderation_flags: vec![RadrootsTradeModerationFlag {
                    code: "buyer-note-review".into(),
                    severity: RadrootsTradeModerationSeverity::Notice,
                    status: RadrootsTradeModerationStatus::Snoozed,
                    source: Some("operator".into()),
                    reason: Some("monitor communication tone".into()),
                }],
                fulfillment_exceptions: vec![RadrootsTradeFulfillmentException {
                    code: "dock-delay".into(),
                    severity: RadrootsTradeFulfillmentExceptionSeverity::Blocking,
                    status: RadrootsTradeFulfillmentExceptionStatus::Open,
                    source: Some("fulfillment".into()),
                    notes: Some("carrier missed pickup window".into()),
                }],
            })
            .expect("order overlay");
        overlays
            .upsert_order_overlay(RadrootsTradeOrderBackofficeOverlay {
                order_id: "order-2".into(),
                review_queue: Some(RadrootsTradeReviewQueueEntry {
                    queue: "order-review".into(),
                    priority: RadrootsTradeReviewPriority::Low,
                    status: RadrootsTradeReviewStatus::Resolved,
                    assigned_operator: None,
                    reason: Some("completed successfully".into()),
                }),
                moderation_flags: Vec::new(),
                fulfillment_exceptions: vec![RadrootsTradeFulfillmentException {
                    code: "tracking-delay".into(),
                    severity: RadrootsTradeFulfillmentExceptionSeverity::Notice,
                    status: RadrootsTradeFulfillmentExceptionStatus::Resolved,
                    source: Some("fulfillment".into()),
                    notes: Some("carrier synced late".into()),
                }],
            })
            .expect("resolved order overlay");

        let views = overlays.order_backoffice_views(
            &index,
            &RadrootsTradeOrderBackofficeQuery {
                order: RadrootsTradeOrderQuery {
                    seller_pubkey: Some("seller-pubkey".into()),
                    ..Default::default()
                },
                requires_review: Some(true),
                has_open_fulfillment_exceptions: Some(true),
                ..Default::default()
            },
            RadrootsTradeOrderSort {
                field: RadrootsTradeOrderSortField::OrderId,
                direction: RadrootsTradeSortDirection::Asc,
            },
        );

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].order.order_id, "order-1");
        assert_eq!(views[0].open_moderation_flag_count, 1);
        assert_eq!(views[0].open_fulfillment_exception_count, 1);
        assert!(views[0].requires_review);
        assert_eq!(views[0].marketplace.status, TradeOrderStatus::Requested);
        assert!(!super::order_backoffice_matches_query(
            &views[0],
            &RadrootsTradeOrderBackofficeQuery {
                requires_review: Some(true),
                has_open_moderation_flags: Some(false),
                has_open_fulfillment_exceptions: Some(true),
                ..Default::default()
            }
        ));

        let completed_order = index.order("order-2").expect("canonical completed order");
        assert_eq!(completed_order.status, TradeOrderStatus::Completed);
        assert_eq!(completed_order.receipt_count, 1);
    }

    #[test]
    fn overlay_store_rejects_missing_identity_keys() {
        let mut overlays = RadrootsTradeBackofficeOverlayStore::new();

        let listing_err = overlays
            .upsert_listing_overlay(RadrootsTradeListingBackofficeOverlay {
                listing_addr: String::new(),
                review_queue: None,
                moderation_flags: Vec::new(),
            })
            .expect_err("missing listing addr should fail");
        assert_eq!(
            listing_err,
            RadrootsTradeBackofficeOverlayError::MissingListingAddr
        );

        let order_err = overlays
            .upsert_order_overlay(RadrootsTradeOrderBackofficeOverlay {
                order_id: String::new(),
                review_queue: None,
                moderation_flags: Vec::new(),
                fulfillment_exceptions: Vec::new(),
            })
            .expect_err("missing order id should fail");
        assert_eq!(
            order_err,
            RadrootsTradeBackofficeOverlayError::MissingOrderId
        );
    }
}

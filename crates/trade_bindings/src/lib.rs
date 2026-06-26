pub use radroots_trade as upstream;

pub mod dto;

pub use dto::dto_roots;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TradeTypeDisposition {
    SourceTradeRoot,
    EventsBindingImport,
    SdkLocalPackageShape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TradeTypeInventoryEntry {
    pub export_name: &'static str,
    pub disposition: TradeTypeDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TradeLargeIntegerPolicy {
    JsonNumberSafeCount,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TradeLargeIntegerPolicyEntry {
    pub type_name: &'static str,
    pub field_name: &'static str,
    pub policy: TradeLargeIntegerPolicy,
}

pub const TRADE_TYPE_INVENTORY: &[TradeTypeInventoryEntry] = &[
    event_import("RadrootsFarmRef"),
    event_import("RadrootsListing"),
    event_import("RadrootsListingAvailability"),
    event_import("RadrootsListingBin"),
    event_import("RadrootsListingDeliveryMethod"),
    event_import("RadrootsListingProduct"),
    event_import("RadrootsListingPublicLocation"),
    event_import("RadrootsListingStatus"),
    local_shape("RadrootsTradeFacetCount"),
    source_root("RadrootsTradeListing"),
    local_shape("RadrootsTradeListingBackofficeOverlay"),
    local_shape("RadrootsTradeListingBackofficeQuery"),
    local_shape("RadrootsTradeListingBackofficeView"),
    local_shape("RadrootsTradeListingBinProjection"),
    local_shape("RadrootsTradeListingFacets"),
    local_shape("RadrootsTradeListingMarketStatus"),
    local_shape("RadrootsTradeListingProjection"),
    local_shape("RadrootsTradeListingQuery"),
    local_shape("RadrootsTradeListingSort"),
    local_shape("RadrootsTradeListingSortField"),
    source_root("RadrootsTradeListingSubtotal"),
    source_root("RadrootsTradeListingTotal"),
    local_shape("RadrootsTradeMarketplaceListingSummary"),
    local_shape("RadrootsTradeMarketplaceOrderSummary"),
    event_import("RadrootsTradeMessageType"),
    local_shape("RadrootsTradeModerationFlag"),
    local_shape("RadrootsTradeModerationSeverity"),
    local_shape("RadrootsTradeModerationStatus"),
    local_shape("RadrootsTradeOrderBackofficeOverlay"),
    local_shape("RadrootsTradeOrderBackofficeQuery"),
    local_shape("RadrootsTradeOrderBackofficeView"),
    local_shape("RadrootsTradeOrderFacets"),
    local_shape("RadrootsTradeOrderQuery"),
    local_shape("RadrootsTradeOrderSort"),
    local_shape("RadrootsTradeOrderSortField"),
    event_import("RadrootsTradeOrderStatus"),
    local_shape("RadrootsTradeOrderWorkflowMessage"),
    local_shape("RadrootsTradeOrderWorkflowProjection"),
    local_shape("RadrootsTradeReviewPriority"),
    local_shape("RadrootsTradeReviewQueueEntry"),
    local_shape("RadrootsTradeReviewStatus"),
    local_shape("RadrootsTradeSortDirection"),
];

pub const TRADE_LARGE_INTEGER_POLICIES: &[TradeLargeIntegerPolicyEntry] = &[
    json_number_safe_count("RadrootsTradeFacetCount", "count"),
    json_number_safe_count(
        "RadrootsTradeListingBackofficeView",
        "open_moderation_flag_count",
    ),
    json_number_safe_count("RadrootsTradeListingProjection", "order_count"),
    json_number_safe_count("RadrootsTradeListingProjection", "open_order_count"),
    json_number_safe_count("RadrootsTradeListingProjection", "terminal_order_count"),
    json_number_safe_count("RadrootsTradeMarketplaceListingSummary", "order_count"),
    json_number_safe_count("RadrootsTradeMarketplaceListingSummary", "open_order_count"),
    json_number_safe_count(
        "RadrootsTradeMarketplaceListingSummary",
        "terminal_order_count",
    ),
    json_number_safe_count("RadrootsTradeMarketplaceOrderSummary", "item_count"),
    json_number_safe_count("RadrootsTradeMarketplaceOrderSummary", "total_bin_count"),
    json_number_safe_count(
        "RadrootsTradeOrderBackofficeView",
        "open_moderation_flag_count",
    ),
    json_number_safe_count("RadrootsTradeOrderWorkflowProjection", "question_count"),
    json_number_safe_count("RadrootsTradeOrderWorkflowProjection", "answer_count"),
    json_number_safe_count("RadrootsTradeOrderWorkflowProjection", "revision_count"),
    json_number_safe_count(
        "RadrootsTradeOrderWorkflowProjection",
        "discount_request_count",
    ),
    json_number_safe_count(
        "RadrootsTradeOrderWorkflowProjection",
        "discount_offer_count",
    ),
    json_number_safe_count(
        "RadrootsTradeOrderWorkflowProjection",
        "discount_accept_count",
    ),
    json_number_safe_count(
        "RadrootsTradeOrderWorkflowProjection",
        "discount_decline_count",
    ),
    json_number_safe_count("RadrootsTradeOrderWorkflowProjection", "cancellation_count"),
];

const fn source_root(export_name: &'static str) -> TradeTypeInventoryEntry {
    TradeTypeInventoryEntry {
        export_name,
        disposition: TradeTypeDisposition::SourceTradeRoot,
    }
}

const fn event_import(export_name: &'static str) -> TradeTypeInventoryEntry {
    TradeTypeInventoryEntry {
        export_name,
        disposition: TradeTypeDisposition::EventsBindingImport,
    }
}

const fn local_shape(export_name: &'static str) -> TradeTypeInventoryEntry {
    TradeTypeInventoryEntry {
        export_name,
        disposition: TradeTypeDisposition::SdkLocalPackageShape,
    }
}

const fn json_number_safe_count(
    type_name: &'static str,
    field_name: &'static str,
) -> TradeLargeIntegerPolicyEntry {
    TradeLargeIntegerPolicyEntry {
        type_name,
        field_name,
        policy: TradeLargeIntegerPolicy::JsonNumberSafeCount,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TRADE_LARGE_INTEGER_POLICIES, TRADE_TYPE_INVENTORY, TradeTypeDisposition, dto_roots,
    };

    #[test]
    fn trade_dto_roots_build_registry() {
        let registry = dto_bindgen_core::build_registry(dto_roots());

        assert!(
            !registry.has_errors(),
            "trade binding registry has diagnostics: {:?}",
            registry.diagnostics
        );
    }

    #[test]
    fn trade_type_inventory_is_deterministic() {
        let expected = TRADE_TYPE_INVENTORY
            .iter()
            .map(|entry| entry.export_name)
            .collect::<Vec<_>>();

        assert_eq!(
            expected,
            [
                "RadrootsFarmRef",
                "RadrootsListing",
                "RadrootsListingAvailability",
                "RadrootsListingBin",
                "RadrootsListingDeliveryMethod",
                "RadrootsListingProduct",
                "RadrootsListingPublicLocation",
                "RadrootsListingStatus",
                "RadrootsTradeFacetCount",
                "RadrootsTradeListing",
                "RadrootsTradeListingBackofficeOverlay",
                "RadrootsTradeListingBackofficeQuery",
                "RadrootsTradeListingBackofficeView",
                "RadrootsTradeListingBinProjection",
                "RadrootsTradeListingFacets",
                "RadrootsTradeListingMarketStatus",
                "RadrootsTradeListingProjection",
                "RadrootsTradeListingQuery",
                "RadrootsTradeListingSort",
                "RadrootsTradeListingSortField",
                "RadrootsTradeListingSubtotal",
                "RadrootsTradeListingTotal",
                "RadrootsTradeMarketplaceListingSummary",
                "RadrootsTradeMarketplaceOrderSummary",
                "RadrootsTradeMessageType",
                "RadrootsTradeModerationFlag",
                "RadrootsTradeModerationSeverity",
                "RadrootsTradeModerationStatus",
                "RadrootsTradeOrderBackofficeOverlay",
                "RadrootsTradeOrderBackofficeQuery",
                "RadrootsTradeOrderBackofficeView",
                "RadrootsTradeOrderFacets",
                "RadrootsTradeOrderQuery",
                "RadrootsTradeOrderSort",
                "RadrootsTradeOrderSortField",
                "RadrootsTradeOrderStatus",
                "RadrootsTradeOrderWorkflowMessage",
                "RadrootsTradeOrderWorkflowProjection",
                "RadrootsTradeReviewPriority",
                "RadrootsTradeReviewQueueEntry",
                "RadrootsTradeReviewStatus",
                "RadrootsTradeSortDirection"
            ]
        );
    }

    #[test]
    fn source_owned_trade_support_types_are_marked_for_event_import() {
        for export_name in [
            "RadrootsFarmRef",
            "RadrootsListing",
            "RadrootsListingAvailability",
            "RadrootsListingBin",
            "RadrootsListingDeliveryMethod",
            "RadrootsListingProduct",
            "RadrootsListingPublicLocation",
            "RadrootsListingStatus",
            "RadrootsTradeMessageType",
            "RadrootsTradeOrderStatus",
        ] {
            assert_eq!(
                disposition(export_name),
                TradeTypeDisposition::EventsBindingImport
            );
        }
    }

    #[test]
    fn trade_source_roots_are_marked_for_source_registry() {
        let source_roots = TRADE_TYPE_INVENTORY
            .iter()
            .filter(|entry| entry.disposition == TradeTypeDisposition::SourceTradeRoot)
            .map(|entry| entry.export_name)
            .collect::<Vec<_>>();

        assert_eq!(
            source_roots,
            [
                "RadrootsTradeListing",
                "RadrootsTradeListingSubtotal",
                "RadrootsTradeListingTotal"
            ]
        );
    }

    #[test]
    fn trade_large_integer_policy_covers_current_count_fields() {
        let actual = TRADE_LARGE_INTEGER_POLICIES
            .iter()
            .map(|entry| (entry.type_name, entry.field_name, entry.policy))
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            [
                (
                    "RadrootsTradeFacetCount",
                    "count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeListingBackofficeView",
                    "open_moderation_flag_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeListingProjection",
                    "order_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeListingProjection",
                    "open_order_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeListingProjection",
                    "terminal_order_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeMarketplaceListingSummary",
                    "order_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeMarketplaceListingSummary",
                    "open_order_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeMarketplaceListingSummary",
                    "terminal_order_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeMarketplaceOrderSummary",
                    "item_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeMarketplaceOrderSummary",
                    "total_bin_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeOrderBackofficeView",
                    "open_moderation_flag_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeOrderWorkflowProjection",
                    "question_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeOrderWorkflowProjection",
                    "answer_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeOrderWorkflowProjection",
                    "revision_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeOrderWorkflowProjection",
                    "discount_request_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeOrderWorkflowProjection",
                    "discount_offer_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeOrderWorkflowProjection",
                    "discount_accept_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeOrderWorkflowProjection",
                    "discount_decline_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeOrderWorkflowProjection",
                    "cancellation_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
            ]
        );
    }

    fn disposition(export_name: &str) -> TradeTypeDisposition {
        TRADE_TYPE_INVENTORY
            .iter()
            .find(|entry| entry.export_name == export_name)
            .map(|entry| entry.disposition)
            .expect("inventory entry")
    }
}

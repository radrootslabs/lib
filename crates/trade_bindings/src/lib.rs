//! Private inventory for canonical native trade bindings.
//!
//! The SDK generator authenticates the reviewed mapping against final
//! `radroots_trade`, `radroots_event`, and `radroots_core` owners. This crate
//! intentionally does not activate code generation in a public runtime crate.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TradeTypeDisposition {
    SourceTradeRoot,
    SourceTradeSupport,
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
    event_import("RadrootsOperationalListing"),
    event_import("RadrootsOperationalListingAvailability"),
    event_import("RadrootsOperationalListingBin"),
    event_import("RadrootsOperationalListingDeliveryMethod"),
    event_import("RadrootsOperationalListingProduct"),
    event_import("RadrootsOperationalListingPublicLocation"),
    event_import("RadrootsOperationalListingStatus"),
    local_shape("RadrootsTradeFacetCount"),
    source_root("RadrootsTradeAgreementStateV1"),
    source_root("RadrootsTradeAttestationStateV1"),
    source_root("RadrootsTradeConflictStateV1"),
    source_root("RadrootsTradeEvidenceStateV1"),
    source_root("RadrootsTradeFulfillmentStateV1"),
    source_root("RadrootsOperationalListingTradeProjection"),
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
    source_root("RadrootsOperationalListingSubtotal"),
    source_root("RadrootsOperationalListingTotal"),
    local_shape("RadrootsTradeMarketplaceListingSummary"),
    local_shape("RadrootsTradeModerationFlag"),
    local_shape("RadrootsTradeModerationSeverity"),
    local_shape("RadrootsTradeModerationStatus"),
    source_root("RadrootsTradeNegotiationStateV1"),
    source_root("RadrootsTradePaymentStateV1"),
    source_root("RadrootsTradePrivateTermsStateV1"),
    source_root("RadrootsTradeProjectionV1"),
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
    json_number_safe_count("RadrootsTradeListingProjection", "trade_count"),
    json_number_safe_count("RadrootsTradeListingProjection", "open_trade_count"),
    json_number_safe_count("RadrootsTradeListingProjection", "terminal_trade_count"),
    json_number_safe_count("RadrootsTradeMarketplaceListingSummary", "trade_count"),
    json_number_safe_count("RadrootsTradeMarketplaceListingSummary", "open_trade_count"),
    json_number_safe_count(
        "RadrootsTradeMarketplaceListingSummary",
        "terminal_trade_count",
    ),
];

#[cfg_attr(coverage_nightly, coverage(off))]
const fn source_root(export_name: &'static str) -> TradeTypeInventoryEntry {
    TradeTypeInventoryEntry {
        export_name,
        disposition: TradeTypeDisposition::SourceTradeRoot,
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
const fn event_import(export_name: &'static str) -> TradeTypeInventoryEntry {
    TradeTypeInventoryEntry {
        export_name,
        disposition: TradeTypeDisposition::EventsBindingImport,
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
const fn local_shape(export_name: &'static str) -> TradeTypeInventoryEntry {
    TradeTypeInventoryEntry {
        export_name,
        disposition: TradeTypeDisposition::SdkLocalPackageShape,
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

/// Validates that the checked-in trade binding inventory is internally coherent.
#[must_use]
pub fn inventory_is_valid() -> bool {
    inventory_entries_are_valid(TRADE_TYPE_INVENTORY, TRADE_LARGE_INTEGER_POLICIES)
}

fn inventory_entries_are_valid(
    types: &[TradeTypeInventoryEntry],
    policies: &[TradeLargeIntegerPolicyEntry],
) -> bool {
    !types.is_empty()
        && types.iter().enumerate().all(|(index, entry)| {
            !entry.export_name.is_empty()
                && !types[..index]
                    .iter()
                    .any(|prior| prior.export_name == entry.export_name)
        })
        && policies.iter().all(|policy| {
            !policy.field_name.is_empty()
                && types
                    .iter()
                    .any(|entry| entry.export_name == policy.type_name)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        TRADE_LARGE_INTEGER_POLICIES, TRADE_TYPE_INVENTORY, TradeLargeIntegerPolicy,
        TradeLargeIntegerPolicyEntry, TradeTypeDisposition, TradeTypeInventoryEntry,
        inventory_entries_are_valid, inventory_is_valid,
    };

    #[test]
    fn checked_in_inventory_is_coherent_and_invalid_shapes_fail_closed() {
        assert!(inventory_is_valid());
        assert!(!inventory_entries_are_valid(&[], &[]));

        let valid = TradeTypeInventoryEntry {
            export_name: "Valid",
            disposition: TradeTypeDisposition::SourceTradeRoot,
        };
        let empty = TradeTypeInventoryEntry {
            export_name: "",
            disposition: TradeTypeDisposition::SourceTradeRoot,
        };
        assert!(!inventory_entries_are_valid(&[empty], &[]));
        assert!(!inventory_entries_are_valid(&[valid, valid], &[]));

        let empty_field = TradeLargeIntegerPolicyEntry {
            type_name: "Valid",
            field_name: "",
            policy: TradeLargeIntegerPolicy::JsonNumberSafeCount,
        };
        let unknown_type = TradeLargeIntegerPolicyEntry {
            type_name: "Missing",
            field_name: "count",
            policy: TradeLargeIntegerPolicy::JsonNumberSafeCount,
        };
        assert!(!inventory_entries_are_valid(&[valid], &[empty_field]));
        assert!(!inventory_entries_are_valid(&[valid], &[unknown_type]));
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
                "RadrootsOperationalListing",
                "RadrootsOperationalListingAvailability",
                "RadrootsOperationalListingBin",
                "RadrootsOperationalListingDeliveryMethod",
                "RadrootsOperationalListingProduct",
                "RadrootsOperationalListingPublicLocation",
                "RadrootsOperationalListingStatus",
                "RadrootsTradeFacetCount",
                "RadrootsTradeAgreementStateV1",
                "RadrootsTradeAttestationStateV1",
                "RadrootsTradeConflictStateV1",
                "RadrootsTradeEvidenceStateV1",
                "RadrootsTradeFulfillmentStateV1",
                "RadrootsOperationalListingTradeProjection",
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
                "RadrootsOperationalListingSubtotal",
                "RadrootsOperationalListingTotal",
                "RadrootsTradeMarketplaceListingSummary",
                "RadrootsTradeModerationFlag",
                "RadrootsTradeModerationSeverity",
                "RadrootsTradeModerationStatus",
                "RadrootsTradeNegotiationStateV1",
                "RadrootsTradePaymentStateV1",
                "RadrootsTradePrivateTermsStateV1",
                "RadrootsTradeProjectionV1",
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
            "RadrootsOperationalListing",
            "RadrootsOperationalListingAvailability",
            "RadrootsOperationalListingBin",
            "RadrootsOperationalListingDeliveryMethod",
            "RadrootsOperationalListingProduct",
            "RadrootsOperationalListingPublicLocation",
            "RadrootsOperationalListingStatus",
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
                "RadrootsTradeAgreementStateV1",
                "RadrootsTradeAttestationStateV1",
                "RadrootsTradeConflictStateV1",
                "RadrootsTradeEvidenceStateV1",
                "RadrootsTradeFulfillmentStateV1",
                "RadrootsOperationalListingTradeProjection",
                "RadrootsOperationalListingSubtotal",
                "RadrootsOperationalListingTotal",
                "RadrootsTradeNegotiationStateV1",
                "RadrootsTradePaymentStateV1",
                "RadrootsTradePrivateTermsStateV1",
                "RadrootsTradeProjectionV1"
            ]
        );
    }

    #[test]
    fn trade_source_support_types_are_marked_for_source_registry() {
        assert!(
            TRADE_TYPE_INVENTORY
                .iter()
                .all(|entry| entry.disposition != TradeTypeDisposition::SourceTradeSupport)
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
                    "trade_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeListingProjection",
                    "open_trade_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeListingProjection",
                    "terminal_trade_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeMarketplaceListingSummary",
                    "trade_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeMarketplaceListingSummary",
                    "open_trade_count",
                    super::TradeLargeIntegerPolicy::JsonNumberSafeCount
                ),
                (
                    "RadrootsTradeMarketplaceListingSummary",
                    "terminal_trade_count",
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

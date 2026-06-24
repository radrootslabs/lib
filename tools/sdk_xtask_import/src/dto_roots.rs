use dto_bindgen_core::{Registry, RootDescriptor, RustTypeId, TypeId, build_registry};

use crate::dto_render::{DtoRegistryRenderOptions, DtoTypesModule, render_registry_types};

#[derive(Clone, Copy, Debug)]
pub struct DtoPackageRootSet {
    pub package_key: &'static str,
    roots: fn() -> Vec<RootDescriptor>,
}

impl DtoPackageRootSet {
    pub fn roots(&self) -> Vec<RootDescriptor> {
        (self.roots)()
    }

    pub fn registry(&self) -> Registry {
        build_registry(self.roots())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManualDescriptorFamily {
    pub package_key: &'static str,
    pub source_family: &'static str,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SdkLocalWrapperAllowance {
    pub package_key: &'static str,
    pub shape_family: &'static str,
    pub reason: &'static str,
}

pub const DTO_PACKAGE_ROOTS: &[DtoPackageRootSet] = &[
    DtoPackageRootSet {
        package_key: "core",
        roots: core_roots,
    },
    DtoPackageRootSet {
        package_key: "events",
        roots: events_roots,
    },
    DtoPackageRootSet {
        package_key: "events_indexed",
        roots: events_indexed_roots,
    },
    DtoPackageRootSet {
        package_key: "trade",
        roots: trade_roots,
    },
    DtoPackageRootSet {
        package_key: "types",
        roots: types_roots,
    },
];

pub const MANUAL_DESCRIPTOR_FAMILIES: &[ManualDescriptorFamily] = &[
    ManualDescriptorFamily {
        package_key: "core",
        source_family: "decimal, currency, money, quantity, percent, quantity price, unit, and discount value families",
        reason: "custom serde, string-backed newtypes, aliases, and tagged enum wire forms require source-owned manual descriptors",
    },
    ManualDescriptorFamily {
        package_key: "events",
        source_family: "event timestamps, counters, and optional metadata fields",
        reason: "large integers and source-specific optional/null policy must be explicit",
    },
    ManualDescriptorFamily {
        package_key: "events",
        source_family: "GeoJSON coordinate arrays",
        reason: "fixed-size Rust arrays must preserve tuple semantics in TypeScript",
    },
    ManualDescriptorFamily {
        package_key: "events_indexed",
        source_family: "checkpoint and index cursor fields",
        reason: "custom deserialization and large integers require manual descriptor policy",
    },
    ManualDescriptorFamily {
        package_key: "trade",
        source_family: "trade listing roots and package projection count fields",
        reason: "core aliases, source-owned event imports, and count-family numeric policy require explicit descriptors",
    },
    ManualDescriptorFamily {
        package_key: "replica_db_schema",
        source_family: "untagged query wrappers and serde_json value fields",
        reason: "schema query shapes are generated and not all source fields map to derive-supported DTOs",
    },
    ManualDescriptorFamily {
        package_key: "types",
        source_family: "generic result wrapper types",
        reason: "generic export instantiations must be explicit and package-scoped",
    },
];

pub const SDK_LOCAL_WRAPPER_ALLOWANCES: &[SdkLocalWrapperAllowance] = &[
    SdkLocalWrapperAllowance {
        package_key: "core",
        shape_family: "RadrootsCoreCurrency and RadrootsCoreDecimal package aliases",
        reason: "source descriptors correctly describe fields as strings, while package roots still need stable named TypeScript aliases",
    },
    SdkLocalWrapperAllowance {
        package_key: "replica_db_schema",
        shape_family: "generated query argument wrappers",
        reason: "schema operation inputs are generated package shapes rather than source-owned public DTO structs",
    },
    SdkLocalWrapperAllowance {
        package_key: "types",
        shape_family: "IResult, IResultList, and IResultPass generic envelopes",
        reason: "generic helper envelopes are SDK package contracts used across generated schema packages",
    },
    SdkLocalWrapperAllowance {
        package_key: "events_indexed",
        shape_family: "RadrootsEventsIndexedShardId package alias",
        reason: "source descriptors correctly describe the shard id newtype as a string, while package roots still need a stable named TypeScript alias",
    },
    SdkLocalWrapperAllowance {
        package_key: "trade",
        shape_family: "marketplace, query, projection, sort, review, and backoffice DTO shapes",
        reason: "these are SDK package contract shapes layered over source-owned trade, events, and core DTOs",
    },
];

pub fn package_root_set(package_key: &str) -> Option<&'static DtoPackageRootSet> {
    DTO_PACKAGE_ROOTS
        .iter()
        .find(|root_set| root_set.package_key == package_key)
}

pub fn core_types_module() -> Result<DtoTypesModule, String> {
    let root_set = package_root_set("core").ok_or_else(|| "missing core DTO roots".to_owned())?;
    let rendered =
        render_registry_types(&root_set.registry(), &DtoRegistryRenderOptions::default())?;
    Ok(DtoTypesModule::new(
        rendered.imports_ts().unwrap_or_default(),
        format!(
            "export type RadrootsCoreCurrency = string;\n\nexport type RadrootsCoreDecimal = string;\n\n{}",
            rendered.body_ts()
        ),
    ))
}

pub fn events_types_module() -> Result<DtoTypesModule, String> {
    let root_set =
        package_root_set("events").ok_or_else(|| "missing events DTO roots".to_owned())?;
    let registry = root_set.registry();
    let rendered = render_registry_types(
        &registry,
        &core_import_options(&registry, DtoRegistryRenderOptions::default()),
    )?;
    Ok(DtoTypesModule::new(
        rendered.imports_ts().unwrap_or_default(),
        with_events_sdk_wrappers(rendered.body_ts()),
    ))
}

pub fn events_indexed_types_module() -> Result<DtoTypesModule, String> {
    let root_set = package_root_set("events_indexed")
        .ok_or_else(|| "missing events-indexed DTO roots".to_owned())?;
    let rendered =
        render_registry_types(&root_set.registry(), &DtoRegistryRenderOptions::default())?;
    Ok(DtoTypesModule::new(
        rendered.imports_ts().unwrap_or_default(),
        with_events_indexed_sdk_wrappers(rendered.body_ts()),
    ))
}

pub fn trade_types_module() -> Result<DtoTypesModule, String> {
    let root_set = package_root_set("trade").ok_or_else(|| "missing trade DTO roots".to_owned())?;
    let registry = root_set.registry();
    render_registry_types(
        &registry,
        &trade_import_options(DtoRegistryRenderOptions::default()),
    )
}

pub fn types_types_module() -> Result<DtoTypesModule, String> {
    let root_set = package_root_set("types").ok_or_else(|| "missing types DTO roots".to_owned())?;
    render_registry_types(&root_set.registry(), &DtoRegistryRenderOptions::default())
}

fn core_roots() -> Vec<RootDescriptor> {
    radroots_core::dto::dto_roots().into_iter().collect()
}

fn events_roots() -> Vec<RootDescriptor> {
    radroots_events::dto::dto_roots().into_iter().collect()
}

fn events_indexed_roots() -> Vec<RootDescriptor> {
    radroots_events_indexed::dto::dto_roots()
        .into_iter()
        .collect()
}

fn trade_roots() -> Vec<RootDescriptor> {
    radroots_trade_bindings::dto_roots()
}

fn types_roots() -> Vec<RootDescriptor> {
    radroots_types_bindings::dto_roots()
}

fn core_import_options(
    registry: &Registry,
    mut options: DtoRegistryRenderOptions,
) -> DtoRegistryRenderOptions {
    for export_name in [
        "RadrootsCoreCurrency",
        "RadrootsCoreDecimal",
        "RadrootsCoreDiscount",
        "RadrootsCoreDiscountScope",
        "RadrootsCoreDiscountThreshold",
        "RadrootsCoreDiscountValue",
        "RadrootsCoreMoney",
        "RadrootsCorePercent",
        "RadrootsCoreQuantity",
        "RadrootsCoreQuantityPrice",
        "RadrootsCoreUnit",
        "RadrootsCoreUnitDimension",
    ] {
        if let Some(type_id) = core_type_id(registry, export_name) {
            options = options.with_external_type(type_id, export_name, "@radroots/core-bindings");
        }
    }
    options
}

fn core_type_id(registry: &Registry, rust_ident: &str) -> Option<TypeId> {
    registry
        .rust_id_to_type_id
        .get(&RustTypeId::new("radroots_core", rust_ident))
        .copied()
}

fn trade_import_options(mut options: DtoRegistryRenderOptions) -> DtoRegistryRenderOptions {
    for export_name in [
        "RadrootsCoreCurrency",
        "RadrootsCoreDecimal",
        "RadrootsCoreDiscount",
        "RadrootsCoreDiscountValue",
        "RadrootsCoreMoney",
        "RadrootsCoreQuantity",
        "RadrootsCoreQuantityPrice",
        "RadrootsCoreUnit",
    ] {
        options =
            options.with_external_override(export_name, export_name, "@radroots/core-bindings");
    }

    for export_name in [
        "RadrootsFarmRef",
        "RadrootsListing",
        "RadrootsListingAvailability",
        "RadrootsListingBin",
        "RadrootsListingDeliveryMethod",
        "RadrootsListingImage",
        "RadrootsListingLocation",
        "RadrootsListingProduct",
        "RadrootsListingStatus",
        "RadrootsNostrEventPtr",
        "RadrootsPlotRef",
        "RadrootsResourceAreaRef",
        "RadrootsTradeMessagePayload",
        "RadrootsTradeMessageType",
        "RadrootsTradeOrderEconomicLine",
        "RadrootsTradeOrderItem",
        "RadrootsTradeOrderStatus",
    ] {
        options =
            options.with_external_override(export_name, export_name, "@radroots/events-bindings");
    }

    options
}

fn with_events_sdk_wrappers(body: &str) -> String {
    let mut declarations = body
        .split("\n\n")
        .filter(|declaration| !declaration.trim().is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    declarations.push(
        "export type RadrootsListingProductTagKeys = readonly [\"key\", \"title\", \"category\", \"summary\", \"process\", \"lot\", \"location\", \"profile\", \"year\"];"
            .to_owned(),
    );
    declarations.sort_by(|left, right| declaration_name(left).cmp(declaration_name(right)));
    declarations.join("\n\n")
}

fn declaration_name(declaration: &str) -> &str {
    declaration
        .strip_prefix("export type ")
        .and_then(|rest| rest.split([' ', '<']).next())
        .unwrap_or(declaration)
}

fn with_events_indexed_sdk_wrappers(body: &str) -> String {
    let mut declarations = body
        .split("\n\n")
        .filter(|declaration| !declaration.trim().is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    declarations.push("export type RadrootsEventsIndexedShardId = string;".to_owned());
    let order = [
        "RadrootsEventsIndexedShardId",
        "RadrootsEventsIndexedIdRange",
        "RadrootsEventsIndexedShardMetadata",
        "RadrootsEventsIndexedManifest",
        "RadrootsEventsIndexedShardCheckpoint",
        "RadrootsEventsIndexedIndexCheckpoint",
    ];
    declarations.sort_by_key(|declaration| {
        order
            .iter()
            .position(|name| *name == declaration_name(declaration))
            .unwrap_or(order.len())
    });
    declarations.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::{
        DTO_PACKAGE_ROOTS, MANUAL_DESCRIPTOR_FAMILIES, SDK_LOCAL_WRAPPER_ALLOWANCES,
        package_root_set,
    };

    const EVENTS_BINDINGS_TYPES_TS: &str =
        include_str!("../../../packages/events-bindings/src/generated/types.ts");
    const EVENTS_INDEXED_BINDINGS_TYPES_TS: &str =
        include_str!("../../../packages/events-indexed-bindings/src/generated/types.ts");
    const TRADE_BINDINGS_TYPES_TS: &str =
        include_str!("../../../packages/trade-bindings/src/generated/types.ts");
    const EVENTS_TYPE_INVENTORY: &[&str] = &[
        "JobFeedbackStatus",
        "JobInputType",
        "JobPaymentRequest",
        "RadrootsAccountClaim",
        "RadrootsActiveTradeEnvelope",
        "RadrootsActiveTradeMessageType",
        "RadrootsAppData",
        "RadrootsComment",
        "RadrootsCoop",
        "RadrootsCoopLocation",
        "RadrootsCoopRef",
        "RadrootsDocument",
        "RadrootsDocumentSubject",
        "RadrootsFarm",
        "RadrootsFarmLocation",
        "RadrootsFarmRef",
        "RadrootsFollow",
        "RadrootsFollowProfile",
        "RadrootsGcsLocation",
        "RadrootsGeoChat",
        "RadrootsGeoJsonPoint",
        "RadrootsGeoJsonPolygon",
        "RadrootsGiftWrap",
        "RadrootsGiftWrapRecipient",
        "RadrootsJobFeedback",
        "RadrootsJobInput",
        "RadrootsJobParam",
        "RadrootsJobRequest",
        "RadrootsJobResult",
        "RadrootsList",
        "RadrootsListEntry",
        "RadrootsListSet",
        "RadrootsListing",
        "RadrootsListingAvailability",
        "RadrootsListingBin",
        "RadrootsListingDeliveryMethod",
        "RadrootsListingImage",
        "RadrootsListingImageSize",
        "RadrootsListingLocation",
        "RadrootsListingProduct",
        "RadrootsListingProductTagKeys",
        "RadrootsListingStatus",
        "RadrootsMessage",
        "RadrootsMessageFile",
        "RadrootsMessageFileDimensions",
        "RadrootsMessageRecipient",
        "RadrootsNostrEvent",
        "RadrootsNostrEventPtr",
        "RadrootsNostrEventRef",
        "RadrootsPlot",
        "RadrootsPlotLocation",
        "RadrootsPlotRef",
        "RadrootsPost",
        "RadrootsProfile",
        "RadrootsProfileType",
        "RadrootsReaction",
        "RadrootsRelayDocument",
        "RadrootsResourceArea",
        "RadrootsResourceAreaLocation",
        "RadrootsResourceAreaRef",
        "RadrootsResourceHarvestCap",
        "RadrootsResourceHarvestProduct",
        "RadrootsSeal",
        "RadrootsTradeAnswer",
        "RadrootsTradeDiscountDecision",
        "RadrootsTradeDiscountOffer",
        "RadrootsTradeDiscountRequest",
        "RadrootsTradeDomain",
        "RadrootsTradeEconomicActor",
        "RadrootsTradeEconomicEffect",
        "RadrootsTradeEconomicLineKind",
        "RadrootsTradeEnvelope",
        "RadrootsTradeInventoryCommitment",
        "RadrootsTradeListingCancel",
        "RadrootsTradeListingParseError",
        "RadrootsTradeListingValidateRequest",
        "RadrootsTradeListingValidateResult",
        "RadrootsTradeListingValidationError",
        "RadrootsTradeMessagePayload",
        "RadrootsTradeMessageType",
        "RadrootsTradeOrderCancelled",
        "RadrootsTradeOrderChange",
        "RadrootsTradeOrderDecision",
        "RadrootsTradeOrderDecisionEvent",
        "RadrootsTradeOrderEconomicItem",
        "RadrootsTradeOrderEconomicLine",
        "RadrootsTradeOrderEconomicTotals",
        "RadrootsTradeOrderEconomics",
        "RadrootsTradeOrderItem",
        "RadrootsTradeOrderRequested",
        "RadrootsTradeOrderResponse",
        "RadrootsTradeOrderRevision",
        "RadrootsTradeOrderRevisionDecision",
        "RadrootsTradeOrderRevisionDecisionEvent",
        "RadrootsTradeOrderRevisionProposed",
        "RadrootsTradeOrderRevisionResponse",
        "RadrootsTradeOrderStatus",
        "RadrootsTradePricingBasis",
        "RadrootsTradeQuestion",
        "RadrootsTradeTransportLane",
    ];
    const EVENTS_INDEXED_TYPE_INVENTORY: &[&str] = &[
        "RadrootsEventsIndexedShardId",
        "RadrootsEventsIndexedIdRange",
        "RadrootsEventsIndexedShardMetadata",
        "RadrootsEventsIndexedManifest",
        "RadrootsEventsIndexedShardCheckpoint",
        "RadrootsEventsIndexedIndexCheckpoint",
    ];
    const TRADE_TYPE_INVENTORY: &[&str] = &[
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
        "RadrootsTradeOrderWorkflowMessage",
        "RadrootsTradeOrderWorkflowProjection",
        "RadrootsTradeReviewPriority",
        "RadrootsTradeReviewQueueEntry",
        "RadrootsTradeReviewStatus",
        "RadrootsTradeSortDirection",
    ];

    #[test]
    fn approved_source_roots_build_registries() {
        for root_set in DTO_PACKAGE_ROOTS {
            let registry = root_set.registry();
            assert!(
                !registry.has_errors(),
                "registry for {} has diagnostics: {:?}",
                root_set.package_key,
                registry.diagnostics
            );
            assert!(!registry.roots.is_empty());
        }
    }

    #[test]
    fn package_roots_are_explicit_not_discovered() {
        assert!(package_root_set("core").is_some());
        assert!(package_root_set("events").is_some());
        assert!(package_root_set("events_indexed").is_some());
        assert!(package_root_set("trade").is_some());
        assert!(package_root_set("types").is_some());
    }

    #[test]
    fn manual_descriptor_catalog_covers_known_review_families() {
        assert!(
            MANUAL_DESCRIPTOR_FAMILIES
                .iter()
                .any(|family| family.source_family.contains("GeoJSON"))
        );
        assert!(
            MANUAL_DESCRIPTOR_FAMILIES
                .iter()
                .any(|family| family.source_family.contains("generic result"))
        );
        assert!(
            SDK_LOCAL_WRAPPER_ALLOWANCES
                .iter()
                .any(|allowance| allowance.shape_family.contains("RadrootsCoreDecimal"))
        );
        assert!(
            SDK_LOCAL_WRAPPER_ALLOWANCES
                .iter()
                .any(|allowance| allowance.shape_family.contains("IResult"))
        );
        assert!(SDK_LOCAL_WRAPPER_ALLOWANCES.iter().any(|allowance| {
            allowance
                .shape_family
                .contains("RadrootsEventsIndexedShardId")
        }));
        assert!(SDK_LOCAL_WRAPPER_ALLOWANCES.iter().any(|allowance| {
            allowance
                .shape_family
                .contains("marketplace, query, projection")
        }));
    }

    #[test]
    fn events_type_inventory_matches_current_package_surface() {
        let actual = type_inventory(EVENTS_BINDINGS_TYPES_TS);

        assert_eq!(actual, EVENTS_TYPE_INVENTORY);
    }

    #[test]
    fn events_indexed_type_inventory_matches_current_package_surface() {
        let actual = type_inventory(EVENTS_INDEXED_BINDINGS_TYPES_TS);

        assert_eq!(actual, EVENTS_INDEXED_TYPE_INVENTORY);
    }

    #[test]
    fn trade_type_inventory_matches_current_package_surface() {
        let actual = type_inventory(TRADE_BINDINGS_TYPES_TS);

        assert_eq!(actual, TRADE_TYPE_INVENTORY);
    }

    #[test]
    fn trade_package_imports_source_owned_support_types() {
        assert!(TRADE_BINDINGS_TYPES_TS.contains("from \"@radroots/core-bindings\""));
        assert!(TRADE_BINDINGS_TYPES_TS.contains("from \"@radroots/events-bindings\""));

        for duplicate in [
            "export type RadrootsListing = ",
            "export type RadrootsFarmRef = ",
            "export type RadrootsTradeMessageType = ",
            "export type RadrootsTradeOrderStatus = ",
        ] {
            assert!(!TRADE_BINDINGS_TYPES_TS.contains(duplicate));
        }
    }

    fn type_inventory(types_ts: &str) -> Vec<&str> {
        types_ts
            .lines()
            .filter_map(|line| line.strip_prefix("export type "))
            .map(|rest| rest.split([' ', '<']).next().expect("type name"))
            .collect()
    }
}

use std::collections::{BTreeMap, BTreeSet};

use dto_bindgen_backend_ts::{
    DtoRegistryRenderOptions, DtoTypesModule, TypeScriptDeclaration, TypeScriptModule,
    TypeScriptType, render_registry_types,
};
use dto_bindgen_core::{Registry, RootDescriptor, build_registry};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DtoExternalOverride {
    target_type: &'static str,
    import_name: &'static str,
    from_package_key: &'static str,
    from: &'static str,
}

const CORE_BINDINGS_PACKAGE_KEY: &str = "core";
const CORE_BINDINGS_PACKAGE_NAME: &str = "@radroots/core-bindings";
const EVENT_BINDINGS_PACKAGE_KEY: &str = "event";
const EVENT_BINDINGS_PACKAGE_NAME: &str = "@radroots/event-bindings";

pub const DTO_PACKAGE_ROOTS: &[DtoPackageRootSet] = &[
    DtoPackageRootSet {
        package_key: "core",
        roots: core_roots,
    },
    DtoPackageRootSet {
        package_key: "event",
        roots: event_roots,
    },
    DtoPackageRootSet {
        package_key: "event_index",
        roots: event_index_roots,
    },
    DtoPackageRootSet {
        package_key: "trade",
        roots: trade_roots,
    },
];

pub const MANUAL_DESCRIPTOR_FAMILIES: &[ManualDescriptorFamily] = &[
    ManualDescriptorFamily {
        package_key: "core",
        source_family: "decimal, currency, money, quantity, percent, quantity price, unit, and discount value families",
        reason: "custom serde, string-backed newtypes, aliases, and tagged enum wire forms require source-owned manual descriptors",
    },
    ManualDescriptorFamily {
        package_key: "event",
        source_family: "event timestamps, counters, and optional metadata fields",
        reason: "large integers and source-specific optional/null policy must be explicit",
    },
    ManualDescriptorFamily {
        package_key: "event",
        source_family: "GeoJSON coordinate arrays",
        reason: "fixed-size Rust arrays must preserve tuple semantics in TypeScript",
    },
    ManualDescriptorFamily {
        package_key: "event_index",
        source_family: "checkpoint and index cursor fields",
        reason: "custom deserialization and large integers require manual descriptor policy",
    },
    ManualDescriptorFamily {
        package_key: "trade",
        source_family: "trade listing roots and package projection count fields",
        reason: "core aliases, source-owned event imports, and count-family numeric policy require explicit descriptors",
    },
    ManualDescriptorFamily {
        package_key: "replica_schema",
        source_family: "untagged query wrappers and serde_json value fields",
        reason: "schema query shapes are generated and not all source fields map to derive-supported DTOs",
    },
];

pub const SDK_LOCAL_WRAPPER_ALLOWANCES: &[SdkLocalWrapperAllowance] = &[
    SdkLocalWrapperAllowance {
        package_key: "core",
        shape_family: "RadrootsCoreCurrency and RadrootsCoreDecimal package aliases",
        reason: "source descriptors correctly describe fields as strings, while package roots still need stable named TypeScript aliases",
    },
    SdkLocalWrapperAllowance {
        package_key: "replica_schema",
        shape_family: "generated query argument wrappers",
        reason: "schema operation inputs are generated package shapes rather than source-owned public DTO structs",
    },
    SdkLocalWrapperAllowance {
        package_key: "event_index",
        shape_family: "RadrootsEventIndexShardId package alias",
        reason: "source descriptors correctly describe the shard id newtype as a string, while package roots still need a stable named TypeScript alias",
    },
    SdkLocalWrapperAllowance {
        package_key: "trade",
        shape_family: "marketplace, query, projection, sort, review, and backoffice DTO shapes",
        reason: "these are SDK package contract shapes layered over source-owned trade, events, and core DTOs",
    },
];

const TRADE_EXTERNAL_OVERRIDES: &[DtoExternalOverride] = &[
    core_override("RadrootsCoreCurrency"),
    core_override("RadrootsCoreDecimal"),
    core_override("RadrootsCoreDiscount"),
    core_override("RadrootsCoreDiscountValue"),
    core_override("RadrootsCoreMoney"),
    core_override("RadrootsCoreQuantity"),
    core_override("RadrootsCoreQuantityPrice"),
    core_override("RadrootsCoreUnit"),
    event_override("RadrootsFarmRef"),
    event_override("RadrootsListing"),
    event_override("RadrootsListingAvailability"),
    event_override("RadrootsListingBin"),
    event_override("RadrootsListingDeliveryMethod"),
    event_override("RadrootsListingImage"),
    event_override("RadrootsListingProduct"),
    event_override("RadrootsListingPublicLocation"),
    event_override("RadrootsListingStatus"),
    event_override("RadrootsEventPtr"),
    event_override("RadrootsPlotRef"),
    event_override("RadrootsResourceAreaRef"),
    event_override("RadrootsOrderEconomicActor"),
    event_override("RadrootsOrderEconomicEffect"),
    event_override("RadrootsOrderEconomicItem"),
    event_override("RadrootsOrderEconomicLine"),
    event_override("RadrootsOrderEconomicLineKind"),
    event_override("RadrootsOrderEconomics"),
    event_override("RadrootsOrderEventType"),
    event_override("RadrootsOrderInventoryCommitment"),
    event_override("RadrootsOrderItem"),
    event_override("RadrootsOrderPricingBasis"),
];

const TRADE_REQUIRED_EXTERNAL_PACKAGE_IMPORTS: &[&str] =
    &[CORE_BINDINGS_PACKAGE_NAME, EVENT_BINDINGS_PACKAGE_NAME];

pub fn package_root_set(package_key: &str) -> Option<&'static DtoPackageRootSet> {
    DTO_PACKAGE_ROOTS
        .iter()
        .find(|root_set| root_set.package_key == package_key)
}

pub fn core_types_module() -> Result<DtoTypesModule, String> {
    let root_set = package_root_set("core").ok_or_else(|| "missing core DTO roots".to_owned())?;
    let rendered =
        render_registry_types(&root_set.registry(), &DtoRegistryRenderOptions::default())?;
    Ok(with_type_aliases_sorted(
        rendered,
        [
            type_alias("RadrootsCoreCurrency", TypeScriptType::String),
            type_alias("RadrootsCoreDecimal", TypeScriptType::String),
        ],
    ))
}

pub fn event_types_module() -> Result<DtoTypesModule, String> {
    let root_set = package_root_set("event").ok_or_else(|| "missing event DTO roots".to_owned())?;
    let registry = root_set.registry();
    let options = core_import_options(&registry, DtoRegistryRenderOptions::default())?;
    let rendered = render_registry_types(&registry, &options)?;
    Ok(with_event_sdk_wrappers(rendered))
}

pub fn event_index_types_module() -> Result<DtoTypesModule, String> {
    let root_set = package_root_set("event_index")
        .ok_or_else(|| "missing event-index DTO roots".to_owned())?;
    let rendered =
        render_registry_types(&root_set.registry(), &DtoRegistryRenderOptions::default())?;
    Ok(with_event_index_sdk_wrappers(rendered))
}

pub fn replica_schema_types_module() -> Result<DtoTypesModule, String> {
    render_registry_types(
        &radroots_replica_schema_bindings::dto_registry(),
        &DtoRegistryRenderOptions::default(),
    )
}

pub fn trade_types_module() -> Result<DtoTypesModule, String> {
    let root_set = package_root_set("trade").ok_or_else(|| "missing trade DTO roots".to_owned())?;
    let registry = root_set.registry();
    let options = trade_import_options(&registry, DtoRegistryRenderOptions::default())?;
    let rendered = render_registry_types(&registry, &options)?;
    validate_external_override_usage(&rendered, TRADE_EXTERNAL_OVERRIDES)?;
    Ok(rendered)
}

fn core_roots() -> Vec<RootDescriptor> {
    radroots_core::dto::dto_roots().into_iter().collect()
}

fn event_roots() -> Vec<RootDescriptor> {
    radroots_event::dto::dto_roots().into_iter().collect()
}

fn event_index_roots() -> Vec<RootDescriptor> {
    radroots_event_index::dto::dto_roots().into_iter().collect()
}

fn trade_roots() -> Vec<RootDescriptor> {
    radroots_trade_bindings::dto_roots()
}

fn core_import_options(
    registry: &Registry,
    mut options: DtoRegistryRenderOptions,
) -> Result<DtoRegistryRenderOptions, String> {
    for export_name in [
        "RadrootsCoreDiscount",
        "RadrootsCoreDiscountScope",
        "RadrootsCoreDiscountThreshold",
        "RadrootsCoreDiscountValue",
        "RadrootsCoreMoney",
        "RadrootsCorePercent",
        "RadrootsCoreQuantity",
        "RadrootsCoreQuantityPrice",
        "RadrootsCoreUnit",
    ] {
        options =
            with_checked_external_type(registry, options, export_name, CORE_BINDINGS_PACKAGE_NAME)?;
    }
    Ok(options)
}

fn with_checked_external_type(
    registry: &Registry,
    options: DtoRegistryRenderOptions,
    export_name: &str,
    from: &str,
) -> Result<DtoRegistryRenderOptions, String> {
    let type_id = registry
        .type_id_by_export_name(export_name)
        .map_err(|error| {
            format!("failed to resolve external DTO type `{export_name}` from `{from}`: {error}")
        })?;
    Ok(options.with_external_type(type_id, export_name, from))
}

fn trade_import_options(
    registry: &Registry,
    mut options: DtoRegistryRenderOptions,
) -> Result<DtoRegistryRenderOptions, String> {
    let package_exports = generated_external_package_exports()?;
    for override_target in TRADE_EXTERNAL_OVERRIDES {
        validate_external_override_target(*override_target, &package_exports)?;
        if let Ok(type_id) = registry.type_id_by_export_name(override_target.target_type) {
            options = options.with_external_type(
                type_id,
                override_target.import_name,
                override_target.from,
            );
        }
        options = options.with_external_override(
            override_target.target_type,
            override_target.import_name,
            override_target.from,
        );
    }

    Ok(options)
}

fn generated_external_package_exports() -> Result<BTreeMap<&'static str, BTreeSet<String>>, String>
{
    Ok(BTreeMap::from([
        (
            CORE_BINDINGS_PACKAGE_KEY,
            type_exports(core_types_module()?.body_ts()),
        ),
        (
            EVENT_BINDINGS_PACKAGE_KEY,
            type_exports(event_types_module()?.body_ts()),
        ),
    ]))
}

fn validate_external_override_target(
    override_target: DtoExternalOverride,
    package_exports: &BTreeMap<&'static str, BTreeSet<String>>,
) -> Result<(), String> {
    let exports = package_exports
        .get(override_target.from_package_key)
        .ok_or_else(|| {
            format!(
                "external DTO override `{}` references unknown package key `{}`",
                override_target.target_type, override_target.from_package_key
            )
        })?;
    if exports.contains(override_target.import_name) {
        return Ok(());
    }
    Err(format!(
        "external DTO override `{}` imports `{}` from `{}`, but package `{}` does not export it",
        override_target.target_type,
        override_target.import_name,
        override_target.from,
        override_target.from_package_key
    ))
}

fn validate_external_override_usage(
    module: &DtoTypesModule,
    overrides: &[DtoExternalOverride],
) -> Result<(), String> {
    let imports = imported_type_inventory(module.imports_ts().unwrap_or_default());
    let package_exports = generated_external_package_exports()?;
    for package_name in TRADE_REQUIRED_EXTERNAL_PACKAGE_IMPORTS {
        if !imports.contains_key(*package_name) {
            return Err(format!(
                "expected generated DTO imports from `{package_name}` but none were emitted"
            ));
        }
    }

    for override_target in overrides {
        if type_exports(module.body_ts()).contains(override_target.target_type) {
            return Err(format!(
                "external DTO override `{}` from `{}` was emitted locally instead of imported",
                override_target.target_type, override_target.from
            ));
        }
        if imports
            .get(override_target.from)
            .is_some_and(|names| names.contains(override_target.import_name))
        {
            validate_external_override_target(*override_target, &package_exports)?;
        }
    }
    Ok(())
}

fn imported_type_inventory(imports_ts: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut imports = BTreeMap::new();
    let mut pending_names: Option<Vec<String>> = None;

    for line in imports_ts.lines() {
        let line = line.trim();
        if let Some(single) = line.strip_prefix("import type { ") {
            if let Some((name, from)) = single.split_once(" } from ") {
                insert_import_names(&mut imports, from, name);
            }
        } else if line == "import type {" {
            pending_names = Some(Vec::new());
        } else if let Some(from) = line.strip_prefix("} from ") {
            if let Some(names) = pending_names.take() {
                for name in names {
                    insert_import(&mut imports, from, &name);
                }
            }
        } else if let Some(names) = pending_names.as_mut()
            && let Some(name) = line.strip_suffix(',')
        {
            names.push(name.to_owned());
        }
    }

    imports
}

fn insert_import_names(imports: &mut BTreeMap<String, BTreeSet<String>>, from: &str, names: &str) {
    for name in names.split(',') {
        insert_import(imports, from, name);
    }
}

fn insert_import(imports: &mut BTreeMap<String, BTreeSet<String>>, from: &str, name: &str) {
    let from = from.trim_end_matches(';').trim_matches('"');
    imports
        .entry(from.to_owned())
        .or_default()
        .insert(name.trim().to_owned());
}

fn type_exports(types_ts: &str) -> BTreeSet<String> {
    types_ts
        .lines()
        .filter_map(|line| line.strip_prefix("export type "))
        .filter_map(|rest| rest.split([' ', '<']).next())
        .map(str::to_owned)
        .collect()
}

const fn core_override(export_name: &'static str) -> DtoExternalOverride {
    DtoExternalOverride {
        target_type: export_name,
        import_name: export_name,
        from_package_key: CORE_BINDINGS_PACKAGE_KEY,
        from: CORE_BINDINGS_PACKAGE_NAME,
    }
}

const fn event_override(export_name: &'static str) -> DtoExternalOverride {
    DtoExternalOverride {
        target_type: export_name,
        import_name: export_name,
        from_package_key: EVENT_BINDINGS_PACKAGE_KEY,
        from: EVENT_BINDINGS_PACKAGE_NAME,
    }
}

fn with_event_sdk_wrappers(module: DtoTypesModule) -> DtoTypesModule {
    let mut declarations = module
        .body_ts()
        .split("\n\n")
        .filter(|declaration| !declaration.trim().is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    declarations.push(type_alias(
        "RadrootsListingProductTagKeys",
        TypeScriptType::readonly_tuple(
            [
                "key", "title", "category", "summary", "process", "lot", "location", "profile",
                "year",
            ]
            .into_iter()
            .map(TypeScriptType::literal_string)
            .collect::<Vec<_>>(),
        ),
    ));
    declarations.sort_by(|left, right| declaration_name(left).cmp(declaration_name(right)));
    DtoTypesModule::new(
        module.imports_ts().unwrap_or_default(),
        declarations.join("\n\n"),
    )
}

fn declaration_name(declaration: &str) -> &str {
    declaration
        .strip_prefix("export type ")
        .and_then(|rest| rest.split([' ', '<']).next())
        .unwrap_or(declaration)
}

fn with_event_index_sdk_wrappers(module: DtoTypesModule) -> DtoTypesModule {
    let mut declarations = module
        .body_ts()
        .split("\n\n")
        .filter(|declaration| !declaration.trim().is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    declarations.push(type_alias(
        "RadrootsEventIndexShardId",
        TypeScriptType::String,
    ));
    let order = [
        "RadrootsEventIndexShardId",
        "RadrootsEventIndexIdRange",
        "RadrootsEventIndexShardMetadata",
        "RadrootsEventIndexManifest",
        "RadrootsEventIndexShardCheckpoint",
        "RadrootsEventIndexCheckpoint",
    ];
    declarations.sort_by_key(|declaration| {
        order
            .iter()
            .position(|name| *name == declaration_name(declaration))
            .unwrap_or(order.len())
    });
    DtoTypesModule::new(
        module.imports_ts().unwrap_or_default(),
        declarations.join("\n\n"),
    )
}

fn with_type_aliases_sorted(
    module: DtoTypesModule,
    aliases: impl IntoIterator<Item = String>,
) -> DtoTypesModule {
    let mut declarations = module
        .body_ts()
        .split("\n\n")
        .filter(|declaration| !declaration.trim().is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    declarations.extend(aliases);
    declarations.sort_by(|left, right| declaration_name(left).cmp(declaration_name(right)));
    DtoTypesModule::new(
        module.imports_ts().unwrap_or_default(),
        declarations.join("\n\n"),
    )
}

fn type_alias(name: impl Into<String>, type_expr: TypeScriptType) -> String {
    TypeScriptModule::new("types.ts")
        .with_declaration(TypeScriptDeclaration::type_alias(name, type_expr))
        .render_source()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use dto_bindgen_core::{Registry, RustTypeId, SourceSpan, StructDef, TypeDef};

    use super::{
        CORE_BINDINGS_PACKAGE_KEY, CORE_BINDINGS_PACKAGE_NAME, DTO_PACKAGE_ROOTS,
        DtoExternalOverride, MANUAL_DESCRIPTOR_FAMILIES, SDK_LOCAL_WRAPPER_ALLOWANCES,
        event_override, imported_type_inventory, package_root_set, trade_import_options,
        type_exports, validate_external_override_target, validate_external_override_usage,
        with_checked_external_type,
    };
    use dto_bindgen_backend_ts::{DtoRegistryRenderOptions, DtoTypesModule};

    const EVENT_BINDINGS_TYPES_TS: &str =
        include_str!("../../../packages/event-bindings/src/generated/types.ts");
    const EVENT_INDEX_BINDINGS_TYPES_TS: &str =
        include_str!("../../../packages/event-index-bindings/src/generated/types.ts");
    const REPLICA_SCHEMA_BINDINGS_TYPES_TS: &str =
        include_str!("../../../packages/replica-schema-bindings/src/generated/types.ts");
    const TRADE_BINDINGS_TYPES_TS: &str =
        include_str!("../../../packages/trade-bindings/src/generated/types.ts");
    const REPLICA_SCHEMA_MODEL_SOURCES: &[&str] = &[
        include_str!("../../../../lib/crates/replica_schema/src/models/farm.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/farm_gcs_location.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/farm_member.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/farm_member_claim.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/farm_tag.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/gcs_location.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/log_error.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/media_image.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/nostr_event_head.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/nostr_profile.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/nostr_profile_relay.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/nostr_relay.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/plot.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/plot_gcs_location.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/plot_tag.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/trade_product.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/trade_product_location.rs"),
        include_str!("../../../../lib/crates/replica_schema/src/models/trade_product_media.rs"),
    ];
    const EVENT_TYPE_INVENTORY: &[&str] = &[
        "JobFeedbackStatus",
        "JobInputType",
        "JobPaymentRequest",
        "RadrootsAccountClaim",
        "RadrootsAddressableRef",
        "RadrootsAppData",
        "RadrootsComment",
        "RadrootsCommercialDomain",
        "RadrootsContributionAttestation",
        "RadrootsCoop",
        "RadrootsCoopLocation",
        "RadrootsCoopRef",
        "RadrootsDocument",
        "RadrootsDocumentSubject",
        "RadrootsEventEnvelopeDto",
        "RadrootsEventPtr",
        "RadrootsEventRef",
        "RadrootsEvidenceBounty",
        "RadrootsFarm",
        "RadrootsFarmPublicLocation",
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
        "RadrootsKnowledgeChangeProposal",
        "RadrootsKnowledgeCitationSpan",
        "RadrootsKnowledgeClaim",
        "RadrootsKnowledgeFieldContext",
        "RadrootsKnowledgeFieldReport",
        "RadrootsKnowledgeLocation",
        "RadrootsKnowledgeLocationPrecision",
        "RadrootsKnowledgeNodeRef",
        "RadrootsKnowledgeObservation",
        "RadrootsKnowledgeObservationValue",
        "RadrootsKnowledgeRelation",
        "RadrootsKnowledgeReview",
        "RadrootsKnowledgeReviewScope",
        "RadrootsKnowledgeReviewScore",
        "RadrootsKnowledgeReviewTarget",
        "RadrootsKnowledgeSource",
        "RadrootsList",
        "RadrootsListEntry",
        "RadrootsListSet",
        "RadrootsListing",
        "RadrootsListingAvailability",
        "RadrootsListingBin",
        "RadrootsListingDeliveryMethod",
        "RadrootsListingImage",
        "RadrootsListingImageSize",
        "RadrootsListingParseError",
        "RadrootsListingProduct",
        "RadrootsListingProductTagKeys",
        "RadrootsListingPublicLocation",
        "RadrootsListingStatus",
        "RadrootsMessage",
        "RadrootsMessageFile",
        "RadrootsMessageFileDimensions",
        "RadrootsMessageRecipient",
        "RadrootsNip01EventWireDto",
        "RadrootsOrderCancellation",
        "RadrootsOrderDecision",
        "RadrootsOrderDecisionOutcome",
        "RadrootsOrderEconomicActor",
        "RadrootsOrderEconomicEffect",
        "RadrootsOrderEconomicItem",
        "RadrootsOrderEconomicLine",
        "RadrootsOrderEconomicLineKind",
        "RadrootsOrderEconomicTotals",
        "RadrootsOrderEconomics",
        "RadrootsOrderEventType",
        "RadrootsOrderInventoryCommitment",
        "RadrootsOrderItem",
        "RadrootsOrderPricingBasis",
        "RadrootsOrderRequest",
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
        "RadrootsRightsAssertion",
        "RadrootsSeal",
        "RadrootsSignedEventDto",
        "RadrootsSignedEventVerificationStateDto",
        "RadrootsSocialFarmAnchor",
        "RadrootsSocialLocation",
        "RadrootsSocialMediaDimensions",
        "RadrootsSocialMediaMetadata",
        "RadrootsSocialMediaThumbnail",
        "RadrootsSocialTarget",
        "RadrootsTradeListingValidateRequest",
        "RadrootsTradeListingValidateResult",
        "RadrootsTradeValidationListingError",
        "RadrootsVerifiedSignedEventDto",
        "RadrootsVerifiedSignedEventVerificationStateDto",
        "RadrootsWikiArticle",
        "RadrootsWikiArticleVersionRef",
        "RadrootsWikiMergeRequest",
        "RadrootsWikiRedirect",
    ];
    const EVENT_INDEX_TYPE_INVENTORY: &[&str] = &[
        "RadrootsEventIndexShardId",
        "RadrootsEventIndexIdRange",
        "RadrootsEventIndexShardMetadata",
        "RadrootsEventIndexManifest",
        "RadrootsEventIndexShardCheckpoint",
        "RadrootsEventIndexCheckpoint",
    ];
    const TRADE_TYPE_INVENTORY: &[&str] = &[
        "RadrootsOrderIssue",
        "RadrootsOrderWorkflowProjection",
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
        "RadrootsTradeReviewPriority",
        "RadrootsTradeReviewQueueEntry",
        "RadrootsTradeReviewStatus",
        "RadrootsTradeSortDirection",
        "RadrootsTradeWorkflowState",
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
        assert!(package_root_set("event").is_some());
        assert!(package_root_set("event_index").is_some());
        assert!(package_root_set("trade").is_some());
    }

    #[test]
    fn manual_descriptor_catalog_covers_known_review_families() {
        assert!(
            MANUAL_DESCRIPTOR_FAMILIES
                .iter()
                .any(|family| family.source_family.contains("GeoJSON"))
        );
        assert!(
            SDK_LOCAL_WRAPPER_ALLOWANCES
                .iter()
                .any(|allowance| allowance.shape_family.contains("RadrootsCoreDecimal"))
        );
        assert!(
            SDK_LOCAL_WRAPPER_ALLOWANCES
                .iter()
                .any(|allowance| { allowance.shape_family.contains("RadrootsEventIndexShardId") })
        );
        assert!(SDK_LOCAL_WRAPPER_ALLOWANCES.iter().any(|allowance| {
            allowance
                .shape_family
                .contains("marketplace, query, projection")
        }));
    }

    #[test]
    fn checked_external_type_lookup_rejects_missing_export() {
        let registry = Registry::new();
        let error = with_checked_external_type(
            &registry,
            DtoRegistryRenderOptions::default(),
            "MissingType",
            CORE_BINDINGS_PACKAGE_NAME,
        )
        .expect_err("missing export must fail");

        assert_eq!(
            error,
            "failed to resolve external DTO type `MissingType` from `@radroots/core-bindings`: no DTO type exports `MissingType`"
        );
    }

    #[test]
    fn checked_external_type_lookup_rejects_duplicate_export() {
        let mut registry = Registry::new();
        registry.register_type(
            RustTypeId::new("sdk", "sdk", "FirstDuplicate"),
            TypeDef::Struct(StructDef::new("FirstDuplicate", "DuplicateType", span())),
        );
        registry.register_type(
            RustTypeId::new("sdk", "sdk", "SecondDuplicate"),
            TypeDef::Struct(StructDef::new("SecondDuplicate", "DuplicateType", span())),
        );

        let error = with_checked_external_type(
            &registry,
            DtoRegistryRenderOptions::default(),
            "DuplicateType",
            CORE_BINDINGS_PACKAGE_NAME,
        )
        .expect_err("duplicate export must fail");

        assert_eq!(
            error,
            "failed to resolve external DTO type `DuplicateType` from `@radroots/core-bindings`: DTO export name `DuplicateType` is ambiguous across 2 types"
        );
    }

    #[test]
    fn trade_external_override_targets_resolve_to_generated_exports() {
        let registry = package_root_set("trade").expect("trade roots").registry();

        trade_import_options(&registry, DtoRegistryRenderOptions::default())
            .expect("trade external overrides validate");
    }

    #[test]
    fn external_override_target_validation_rejects_missing_generated_export() {
        let package_exports = BTreeMap::from([(
            CORE_BINDINGS_PACKAGE_KEY,
            BTreeSet::from(["ExistingType".to_owned()]),
        )]);
        let error = validate_external_override_target(
            DtoExternalOverride {
                target_type: "MissingType",
                import_name: "MissingType",
                from_package_key: CORE_BINDINGS_PACKAGE_KEY,
                from: CORE_BINDINGS_PACKAGE_NAME,
            },
            &package_exports,
        )
        .expect_err("missing target package export must fail");

        assert_eq!(
            error,
            "external DTO override `MissingType` imports `MissingType` from `@radroots/core-bindings`, but package `core` does not export it"
        );
    }

    #[test]
    fn external_override_usage_rejects_absent_required_package_import() {
        let module = DtoTypesModule::new("", "export type RadrootsTradeListing = string;");
        let error = validate_external_override_usage(&module, &[])
            .expect_err("missing required package import must fail");

        assert_eq!(
            error,
            "expected generated DTO imports from `@radroots/core-bindings` but none were emitted"
        );
    }

    #[test]
    fn external_override_usage_rejects_local_emission_for_imported_type() {
        let module = DtoTypesModule::new(
            "import type { RadrootsCoreDecimal } from \"@radroots/core-bindings\";\n\nimport type { RadrootsFarmRef } from \"@radroots/event-bindings\";\n\n",
            "export type RadrootsFarmRef = { pubkey: string, d_tag: string, };",
        );
        let error = validate_external_override_usage(&module, &[event_override("RadrootsFarmRef")])
            .expect_err("local emission of imported type must fail");

        assert_eq!(
            error,
            "external DTO override `RadrootsFarmRef` from `@radroots/event-bindings` was emitted locally instead of imported"
        );
    }

    #[test]
    fn generated_import_inventory_parses_single_and_multiline_imports() {
        let imports = imported_type_inventory(
            "import type { One, Four } from \"@radroots/one\";\nimport type {\n    Two,\n    Three,\n} from \"@radroots/many\";\n\n",
        );

        assert_eq!(
            imports.get("@radroots/one").expect("single import package"),
            &BTreeSet::from(["Four".to_owned(), "One".to_owned()])
        );
        assert_eq!(
            imports.get("@radroots/many").expect("multi import package"),
            &BTreeSet::from(["Three".to_owned(), "Two".to_owned()])
        );
    }

    #[test]
    fn generated_type_export_inventory_parses_type_aliases() {
        assert_eq!(
            type_exports("export type Alpha = string;\nexport type Beta<T> = T;\n"),
            BTreeSet::from(["Alpha".to_owned(), "Beta".to_owned()])
        );
    }

    #[test]
    fn event_type_inventory_matches_current_package_surface() {
        let actual = type_inventory(EVENT_BINDINGS_TYPES_TS);

        assert_eq!(actual, EVENT_TYPE_INVENTORY);
    }

    #[test]
    fn event_generated_types_expose_current_signed_event_dto_shapes() {
        let wire = type_declaration(EVENT_BINDINGS_TYPES_TS, "RadrootsNip01EventWireDto");
        let envelope = type_declaration(EVENT_BINDINGS_TYPES_TS, "RadrootsEventEnvelopeDto");
        let signed = type_declaration(EVENT_BINDINGS_TYPES_TS, "RadrootsSignedEventDto");
        let verified = type_declaration(EVENT_BINDINGS_TYPES_TS, "RadrootsVerifiedSignedEventDto");

        assert!(wire.contains("pubkey: string"));
        assert!(wire.contains("extra: { [key: string]: unknown }"));
        assert!(envelope.contains("author: string"));
        assert!(!envelope.contains("pubkey"));
        assert!(!envelope.contains("extra"));
        assert!(signed.contains("state: RadrootsSignedEventVerificationStateDto"));
        assert!(signed.contains("envelope: RadrootsEventEnvelopeDto"));
        assert!(signed.contains("wire: RadrootsNip01EventWireDto"));
        assert!(signed.contains("raw_json: string"));
        assert!(verified.contains("state: RadrootsVerifiedSignedEventVerificationStateDto"));
        assert!(verified.contains("signed_event: RadrootsSignedEventDto"));
        assert!(
            !EVENT_BINDINGS_TYPES_TS.contains("export type RadrootsEventEnvelope ="),
            "event package must not export the retired raw envelope type name"
        );
    }

    #[test]
    fn event_index_type_inventory_matches_current_package_surface() {
        let actual = type_inventory(EVENT_INDEX_BINDINGS_TYPES_TS);

        assert_eq!(actual, EVENT_INDEX_TYPE_INVENTORY);
    }

    #[test]
    fn trade_type_inventory_matches_current_package_surface() {
        let actual = type_inventory(TRADE_BINDINGS_TYPES_TS);

        assert_eq!(actual, TRADE_TYPE_INVENTORY);
    }

    #[test]
    fn replica_schema_generated_types_preserve_source_schema_contracts() {
        let actual = type_inventory(REPLICA_SCHEMA_BINDINGS_TYPES_TS);
        let trade_product_filter = type_declaration(
            REPLICA_SCHEMA_BINDINGS_TYPES_TS,
            "ITradeProductFieldsFilter",
        );
        let trade_product_partial = type_declaration(
            REPLICA_SCHEMA_BINDINGS_TYPES_TS,
            "ITradeProductFieldsPartial",
        );

        assert!(actual.contains(&"Farm"));
        assert!(actual.contains(&"GcsLocation"));
        assert!(actual.contains(&"NostrEventHead"));
        assert!(actual.contains(&"ReplicaStoreJsonValue"));
        assert!(actual.contains(&"ITradeProductFieldsPartial"));
        assert!(!actual.contains(&"NostrEventState"));
        assert!(REPLICA_SCHEMA_BINDINGS_TYPES_TS.contains(
            "export type ReplicaStoreJsonValue = null | boolean | number | string | Array<ReplicaStoreJsonValue> | { [key: string]: ReplicaStoreJsonValue };"
        ));
        assert!(
            REPLICA_SCHEMA_BINDINGS_TYPES_TS
                .contains("export type IFarmFindOneResolve = ReplicaSchemaResult<Farm | null>;")
        );
        assert!(actual.contains(&"ReplicaSchemaResult"));
        assert!(actual.contains(&"ReplicaSchemaResultList"));
        assert!(actual.contains(&"ReplicaSchemaResultPass"));
        assert!(actual.contains(&"ReplicaSchemaError"));
        assert!(trade_product_filter.contains("year?: bigint"));
        assert!(trade_product_filter.contains("qty_avail?: bigint"));
        assert!(trade_product_partial.contains("year?: ReplicaStoreJsonValue | null"));
        assert!(trade_product_partial.contains("qty_avail?: ReplicaStoreJsonValue | null"));
    }

    #[test]
    fn replica_schema_generated_types_match_source_public_inventory() {
        let actual = type_inventory(REPLICA_SCHEMA_BINDINGS_TYPES_TS)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let missing = source_public_schema_type_inventory()
            .into_iter()
            .filter(|name| !actual.contains(name))
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "missing generated replica schema exports: {missing:?}"
        );
    }

    #[test]
    fn trade_package_imports_source_owned_support_types() {
        assert!(TRADE_BINDINGS_TYPES_TS.contains("from \"@radroots/core-bindings\""));
        assert!(TRADE_BINDINGS_TYPES_TS.contains("from \"@radroots/event-bindings\""));

        let imports = imported_type_inventory(TRADE_BINDINGS_TYPES_TS);
        assert!(
            imports
                .get("@radroots/event-bindings")
                .is_some_and(|names| names.contains("RadrootsOrderEventType"))
        );
        assert!(
            imports
                .get("@radroots/event-bindings")
                .is_some_and(|names| names.contains("RadrootsOrderEconomics"))
        );
        assert!(
            imports
                .get("@radroots/event-bindings")
                .is_some_and(|names| names.contains("RadrootsOrderInventoryCommitment"))
        );

        for duplicate in [
            "export type RadrootsListing = ",
            "export type RadrootsFarmRef = ",
            "export type RadrootsOrderEconomics = ",
            "export type RadrootsOrderInventoryCommitment = ",
            "export type RadrootsCommercialMessagePayload = ",
            "export type RadrootsCommercialMessageType = ",
            "export type RadrootsOrderStatus = ",
            "export type RadrootsTradeOrderWorkflowMessage = ",
        ] {
            assert!(!TRADE_BINDINGS_TYPES_TS.contains(duplicate));
        }

        let marketplace_order_summary = type_declaration(
            TRADE_BINDINGS_TYPES_TS,
            "RadrootsTradeMarketplaceOrderSummary",
        );
        assert!(marketplace_order_summary.contains("status: RadrootsTradeWorkflowState"));
        assert!(marketplace_order_summary.contains("last_message_type: RadrootsOrderEventType"));
        assert!(!marketplace_order_summary.contains("has_requested_discounts"));

        let order_query = type_declaration(TRADE_BINDINGS_TYPES_TS, "RadrootsTradeOrderQuery");
        assert!(order_query.contains("status?: RadrootsTradeWorkflowState | null"));

        let workflow_projection =
            type_declaration(TRADE_BINDINGS_TYPES_TS, "RadrootsOrderWorkflowProjection");
        assert!(workflow_projection.contains("status: RadrootsTradeWorkflowState"));
        assert!(workflow_projection.contains("request_event_id?: string | null"));
        assert!(workflow_projection.contains("last_event_id?: string | null"));
        assert!(workflow_projection.contains("listing_addr?: string | null"));
        for stale_counter in [
            "root_event_id",
            "last_message_type",
            "last_discount_request",
            "last_discount_offer",
            "accepted_discount",
            "last_discount_decline_reason",
            "has_requested_discounts",
            "question_count",
            "answer_count",
            "discount_request_count",
            "discount_offer_count",
            "discount_accept_count",
            "discount_decline_count",
        ] {
            assert!(!workflow_projection.contains(stale_counter));
        }
    }

    fn type_inventory(types_ts: &str) -> Vec<&str> {
        types_ts
            .lines()
            .filter_map(|line| line.strip_prefix("export type "))
            .map(|rest| rest.split([' ', '<']).next().expect("type name"))
            .collect()
    }

    fn source_public_schema_type_inventory() -> Vec<&'static str> {
        let mut names = BTreeSet::new();

        for source in REPLICA_SCHEMA_MODEL_SOURCES {
            for line in source.lines() {
                if let Some(name) = public_rust_type_name(line)
                    && !name.ends_with("Ts")
                {
                    names.insert(name);
                }
            }
        }

        names.into_iter().collect()
    }

    fn public_rust_type_name(line: &'static str) -> Option<&'static str> {
        let line = line.trim_start();

        ["pub struct ", "pub enum ", "pub type "]
            .into_iter()
            .find_map(|prefix| {
                line.strip_prefix(prefix).map(|rest| {
                    rest.split(|char: char| !(char == '_' || char.is_ascii_alphanumeric()))
                        .next()
                        .expect("type name")
                })
            })
    }

    fn type_declaration<'a>(types_ts: &'a str, name: &str) -> &'a str {
        types_ts
            .lines()
            .find(|line| line.starts_with(&format!("export type {name} = ")))
            .unwrap_or_else(|| panic!("missing type declaration for {name}"))
    }

    fn span() -> SourceSpan {
        SourceSpan::new("tools/xtask/src/dto_roots.rs", 1, 1)
    }
}

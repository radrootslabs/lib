use dto_bindgen_core::{
    BackendId, DescribeCtx, Dto, EnumDef, EnumRepr, FieldDef, IdentName, RootDescriptor,
    RustTypeId, SourceSpan, TargetFieldNames, TargetOverride, TypeDef, TypeRef, VariantDef,
    VariantShape, WireFieldNames,
};
use radroots_trade::operational_listing::model::RadrootsOperationalListingTotal;

pub fn dto_roots() -> Vec<RootDescriptor> {
    let mut roots = radroots_trade::dto::dto_roots()
        .into_iter()
        .collect::<Vec<_>>();
    roots.extend([
        RootDescriptor::new::<RadrootsTradeFacetCount>(),
        RootDescriptor::new::<RadrootsTradeListingBackofficeOverlay>(),
        RootDescriptor::new::<RadrootsTradeListingBackofficeQuery>(),
        RootDescriptor::new::<RadrootsTradeListingBackofficeView>(),
        RootDescriptor::new::<RadrootsTradeListingBinProjection>(),
        RootDescriptor::new::<RadrootsTradeListingFacets>(),
        RootDescriptor::new::<RadrootsTradeListingMarketStatus>(),
        RootDescriptor::new::<RadrootsTradeListingProjection>(),
        RootDescriptor::new::<RadrootsTradeListingQuery>(),
        RootDescriptor::new::<RadrootsTradeListingSort>(),
        RootDescriptor::new::<RadrootsTradeListingSortField>(),
        RootDescriptor::new::<RadrootsTradeMarketplaceListingSummary>(),
        RootDescriptor::new::<RadrootsTradeModerationFlag>(),
        RootDescriptor::new::<RadrootsTradeModerationSeverity>(),
        RootDescriptor::new::<RadrootsTradeModerationStatus>(),
        RootDescriptor::new::<RadrootsTradeReviewPriority>(),
        RootDescriptor::new::<RadrootsTradeReviewQueueEntry>(),
        RootDescriptor::new::<RadrootsTradeReviewStatus>(),
        RootDescriptor::new::<RadrootsTradeSortDirection>(),
    ]);
    roots
}

macro_rules! imported_ts_type {
    ($ty:ident, $target:literal) => {
        pub struct $ty;

        impl Dto for $ty {
            fn describe(_ctx: &mut DescribeCtx) -> TypeRef {
                TypeRef::Override(TargetOverride::new(BackendId::TypeScript, $target))
            }
        }
    };
}

imported_ts_type!(RadrootsCoreDecimalImport, "RadrootsCoreDecimal");
imported_ts_type!(RadrootsCoreDiscountImport, "RadrootsCoreDiscount");
imported_ts_type!(RadrootsCoreMoneyImport, "RadrootsCoreMoney");
imported_ts_type!(RadrootsCoreQuantityImport, "RadrootsCoreQuantity");
imported_ts_type!(RadrootsCoreQuantityPriceImport, "RadrootsCoreQuantityPrice");
imported_ts_type!(RadrootsCoreUnitImport, "RadrootsCoreUnit");
imported_ts_type!(RadrootsFarmRefImport, "RadrootsFarmRef");
imported_ts_type!(
    RadrootsOperationalListingAvailabilityImport,
    "RadrootsOperationalListingAvailability"
);
imported_ts_type!(
    RadrootsOperationalListingBinImport,
    "RadrootsOperationalListingBin"
);
imported_ts_type!(
    RadrootsOperationalListingDeliveryMethodImport,
    "RadrootsOperationalListingDeliveryMethod"
);
imported_ts_type!(
    RadrootsOperationalListingImageImport,
    "RadrootsOperationalListingImage"
);
imported_ts_type!(
    RadrootsOperationalListingPublicLocationImport,
    "RadrootsOperationalListingPublicLocation"
);
imported_ts_type!(
    RadrootsOperationalListingProductImport,
    "RadrootsOperationalListingProduct"
);
imported_ts_type!(RadrootsPlotRefImport, "RadrootsPlotRef");
imported_ts_type!(RadrootsResourceAreaRefImport, "RadrootsResourceAreaRef");

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeFacetCount {
    pub key: String,
    pub count: u32,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeListingBackofficeOverlay {
    pub listing_addr: String,
    pub review_queue: Option<RadrootsTradeReviewQueueEntry>,
    pub moderation_flags: Vec<RadrootsTradeModerationFlag>,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeListingBackofficeQuery {
    pub listing: RadrootsTradeListingQuery,
    pub requires_review: Option<bool>,
    pub has_open_moderation_flags: Option<bool>,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeListingBackofficeView {
    pub listing: RadrootsTradeListingProjection,
    pub marketplace: Option<RadrootsTradeMarketplaceListingSummary>,
    pub overlay: Option<RadrootsTradeListingBackofficeOverlay>,
    pub requires_review: bool,
    pub open_moderation_flag_count: u32,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeListingBinProjection {
    pub bin: RadrootsOperationalListingBinImport,
    pub one_bin_total: RadrootsOperationalListingTotal,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeListingFacets {
    pub seller_pubkeys: Vec<RadrootsTradeFacetCount>,
    pub farm_pubkeys: Vec<RadrootsTradeFacetCount>,
    pub farm_ids: Vec<RadrootsTradeFacetCount>,
    pub product_keys: Vec<RadrootsTradeFacetCount>,
    pub product_categories: Vec<RadrootsTradeFacetCount>,
    pub listing_statuses: Vec<RadrootsTradeFacetCount>,
}

pub enum RadrootsTradeListingMarketStatus {
    Unknown,
    Window,
    Active,
    Sold,
    Other { value: String },
}

impl Dto for RadrootsTradeListingMarketStatus {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsTradeListingMarketStatus",
            "RadrootsTradeListingMarketStatus",
            EnumRepr::External,
            span("crates/trade_bindings/src/dto.rs", 140),
        )
        .with_variant(unit_variant(
            "Unknown",
            "unknown",
            "crates/trade_bindings/src/dto.rs",
            141,
        ))
        .with_variant(unit_variant(
            "Window",
            "window",
            "crates/trade_bindings/src/dto.rs",
            142,
        ))
        .with_variant(unit_variant(
            "Active",
            "active",
            "crates/trade_bindings/src/dto.rs",
            143,
        ))
        .with_variant(unit_variant(
            "Sold",
            "sold",
            "crates/trade_bindings/src/dto.rs",
            144,
        ))
        .with_variant(VariantDef::new(
            "Other",
            "other",
            VariantShape::Struct(vec![field(
                "value",
                "value",
                String::describe(ctx),
                "crates/trade_bindings/src/dto.rs",
                145,
            )]),
            span("crates/trade_bindings/src/dto.rs", 145),
        ));
        register(ctx, "RadrootsTradeListingMarketStatus", TypeDef::Enum(def))
    }
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeListingProjection {
    pub listing_addr: String,
    pub seller_pubkey: String,
    pub listing_id: String,
    pub farm: RadrootsFarmRefImport,
    pub product: RadrootsOperationalListingProductImport,
    pub primary_bin_id: String,
    pub bins: Vec<RadrootsTradeListingBinProjection>,
    pub resource_area: Option<RadrootsResourceAreaRefImport>,
    pub plot: Option<RadrootsPlotRefImport>,
    pub discounts: Option<Vec<RadrootsCoreDiscountImport>>,
    pub inventory_available: Option<RadrootsCoreDecimalImport>,
    pub availability: Option<RadrootsOperationalListingAvailabilityImport>,
    pub delivery_method: Option<RadrootsOperationalListingDeliveryMethodImport>,
    pub location: Option<RadrootsOperationalListingPublicLocationImport>,
    pub images: Option<Vec<RadrootsOperationalListingImageImport>>,
    pub trade_count: u32,
    pub open_trade_count: u32,
    pub terminal_trade_count: u32,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeListingQuery {
    pub seller_pubkey: Option<String>,
    pub farm_pubkey: Option<String>,
    pub farm_id: Option<String>,
    pub product_key: Option<String>,
    pub product_category: Option<String>,
    pub listing_status: Option<RadrootsTradeListingMarketStatus>,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeListingSort {
    pub field: RadrootsTradeListingSortField,
    pub direction: RadrootsTradeSortDirection,
}

#[derive(dto_bindgen::Dto)]
pub enum RadrootsTradeListingSortField {
    #[serde(rename = "listing_addr")]
    ListingAddr,
    #[serde(rename = "product_title")]
    ProductTitle,
    #[serde(rename = "product_category")]
    ProductCategory,
    #[serde(rename = "seller_pubkey")]
    SellerPubkey,
    #[serde(rename = "inventory_available")]
    InventoryAvailable,
    #[serde(rename = "open_trade_count")]
    OpenTradeCount,
    #[serde(rename = "total_trade_count")]
    TotalTradeCount,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeMarketplaceListingSummary {
    pub listing_addr: String,
    pub seller_pubkey: String,
    pub farm_pubkey: String,
    pub farm_id: String,
    pub product_key: String,
    pub product_title: String,
    pub product_category: String,
    pub product_summary: Option<String>,
    pub listing_status: RadrootsTradeListingMarketStatus,
    pub location_primary: Option<String>,
    pub inventory_available: Option<RadrootsCoreDecimalImport>,
    pub primary_bin_id: String,
    pub primary_bin_label: Option<String>,
    pub primary_bin_total: RadrootsOperationalListingTotal,
    pub trade_count: u32,
    pub open_trade_count: u32,
    pub terminal_trade_count: u32,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeModerationFlag {
    pub code: String,
    pub severity: RadrootsTradeModerationSeverity,
    pub status: RadrootsTradeModerationStatus,
    pub source: Option<String>,
    pub reason: Option<String>,
}

#[derive(dto_bindgen::Dto)]
pub enum RadrootsTradeModerationSeverity {
    #[serde(rename = "notice")]
    Notice,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "block")]
    Block,
}

#[derive(dto_bindgen::Dto)]
pub enum RadrootsTradeModerationStatus {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "snoozed")]
    Snoozed,
    #[serde(rename = "resolved")]
    Resolved,
}

#[derive(dto_bindgen::Dto)]
pub enum RadrootsTradeReviewPriority {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "critical")]
    Critical,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeReviewQueueEntry {
    pub queue: String,
    pub priority: RadrootsTradeReviewPriority,
    pub status: RadrootsTradeReviewStatus,
    pub assigned_operator: Option<String>,
    pub reason: Option<String>,
}

#[derive(dto_bindgen::Dto)]
pub enum RadrootsTradeReviewStatus {
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "blocked")]
    Blocked,
    #[serde(rename = "resolved")]
    Resolved,
}

#[derive(dto_bindgen::Dto)]
pub enum RadrootsTradeSortDirection {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

fn register(ctx: &mut DescribeCtx, rust_ident: &str, type_def: TypeDef) -> TypeRef {
    ctx.register_type(
        RustTypeId::new(
            "radroots_trade_bindings",
            "radroots_trade_bindings",
            rust_ident,
        ),
        type_def,
    )
}

fn unit_variant(rust_name: &str, wire_name: &str, file: &str, line: u32) -> VariantDef {
    VariantDef::new(rust_name, wire_name, VariantShape::Unit, span(file, line))
}

fn field(rust_name: &str, wire_name: &str, ty: TypeRef, file: &str, line: u32) -> FieldDef {
    FieldDef::new(
        IdentName::new(rust_name),
        WireFieldNames::same(wire_name),
        TargetFieldNames::new(wire_name, rust_name),
        ty,
        span(file, line),
    )
}

fn span(file: &str, line: u32) -> SourceSpan {
    SourceSpan::new(file, line, 1)
}

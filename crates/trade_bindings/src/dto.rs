use dto_bindgen_core::{
    BackendId, DescribeCtx, Dto, EnumDef, EnumRepr, FieldDef, IdentName, RootDescriptor,
    RustTypeId, SourceSpan, TargetFieldNames, TargetOverride, TypeDef, TypeRef, VariantDef,
    VariantShape, WireFieldNames,
};
use radroots_trade::listing::model::RadrootsTradeListingTotal;

pub fn dto_roots() -> Vec<RootDescriptor> {
    let mut roots = radroots_trade::dto::dto_roots()
        .into_iter()
        .collect::<Vec<_>>();
    roots.extend([
        RootDescriptor::new::<RadrootsCommercialMessagePayload>(),
        RootDescriptor::new::<RadrootsCommercialMessageType>(),
        RootDescriptor::new::<RadrootsOrderStatus>(),
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
        RootDescriptor::new::<RadrootsTradeMarketplaceOrderSummary>(),
        RootDescriptor::new::<RadrootsTradeModerationFlag>(),
        RootDescriptor::new::<RadrootsTradeModerationSeverity>(),
        RootDescriptor::new::<RadrootsTradeModerationStatus>(),
        RootDescriptor::new::<RadrootsTradeOrderBackofficeOverlay>(),
        RootDescriptor::new::<RadrootsTradeOrderBackofficeQuery>(),
        RootDescriptor::new::<RadrootsTradeOrderBackofficeView>(),
        RootDescriptor::new::<RadrootsTradeOrderFacets>(),
        RootDescriptor::new::<RadrootsTradeOrderQuery>(),
        RootDescriptor::new::<RadrootsTradeOrderSort>(),
        RootDescriptor::new::<RadrootsTradeOrderSortField>(),
        RootDescriptor::new::<RadrootsTradeOrderWorkflowMessage>(),
        RootDescriptor::new::<RadrootsTradeOrderWorkflowProjection>(),
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
imported_ts_type!(RadrootsCoreDiscountValueImport, "RadrootsCoreDiscountValue");
imported_ts_type!(RadrootsCoreMoneyImport, "RadrootsCoreMoney");
imported_ts_type!(RadrootsCoreQuantityImport, "RadrootsCoreQuantity");
imported_ts_type!(RadrootsCoreQuantityPriceImport, "RadrootsCoreQuantityPrice");
imported_ts_type!(RadrootsCoreUnitImport, "RadrootsCoreUnit");
imported_ts_type!(RadrootsFarmRefImport, "RadrootsFarmRef");
imported_ts_type!(RadrootsListingImport, "RadrootsListing");
imported_ts_type!(
    RadrootsListingAvailabilityImport,
    "RadrootsListingAvailability"
);
imported_ts_type!(RadrootsListingBinImport, "RadrootsListingBin");
imported_ts_type!(
    RadrootsListingDeliveryMethodImport,
    "RadrootsListingDeliveryMethod"
);
imported_ts_type!(RadrootsListingImageImport, "RadrootsListingImage");
imported_ts_type!(
    RadrootsListingPublicLocationImport,
    "RadrootsListingPublicLocation"
);
imported_ts_type!(RadrootsListingProductImport, "RadrootsListingProduct");
imported_ts_type!(RadrootsNostrEventPtrImport, "RadrootsNostrEventPtr");
imported_ts_type!(RadrootsPlotRefImport, "RadrootsPlotRef");
imported_ts_type!(RadrootsResourceAreaRefImport, "RadrootsResourceAreaRef");
imported_ts_type!(RadrootsOrderCancellationImport, "RadrootsOrderCancellation");
imported_ts_type!(RadrootsOrderDecisionImport, "RadrootsOrderDecision");
imported_ts_type!(RadrootsOrderEconomicLineImport, "RadrootsOrderEconomicLine");
imported_ts_type!(RadrootsOrderItemImport, "RadrootsOrderItem");
imported_ts_type!(RadrootsOrderRequestImport, "RadrootsOrderRequest");
imported_ts_type!(
    RadrootsOrderRevisionDecisionImport,
    "RadrootsOrderRevisionDecision"
);
imported_ts_type!(
    RadrootsOrderRevisionProposalImport,
    "RadrootsOrderRevisionProposal"
);
imported_ts_type!(
    RadrootsTradeListingValidateRequestImport,
    "RadrootsTradeListingValidateRequest"
);
imported_ts_type!(
    RadrootsTradeListingValidateResultImport,
    "RadrootsTradeListingValidateResult"
);

pub enum RadrootsCommercialMessagePayload {
    ListingValidateRequest,
    ListingValidateResult,
    TradeOrderRequested,
    OrderDecision,
    OrderRevisionProposal,
    OrderRevisionDecisionAccepted,
    OrderRevisionDecisionDeclined,
    Cancel,
}

impl Dto for RadrootsCommercialMessagePayload {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsCommercialMessagePayload",
            "RadrootsCommercialMessagePayload",
            EnumRepr::External,
            span("crates/trade_bindings/src/dto.rs", 97),
        )
        .with_variant(payload_variant(
            "ListingValidateRequest",
            "listing_validate_request",
            RadrootsTradeListingValidateRequestImport::describe(ctx),
            "crates/trade_bindings/src/dto.rs",
            99,
        ))
        .with_variant(payload_variant(
            "ListingValidateResult",
            "listing_validate_result",
            RadrootsTradeListingValidateResultImport::describe(ctx),
            "crates/trade_bindings/src/dto.rs",
            100,
        ))
        .with_variant(payload_variant(
            "TradeOrderRequested",
            "trade_order_requested",
            RadrootsOrderRequestImport::describe(ctx),
            "crates/trade_bindings/src/dto.rs",
            101,
        ))
        .with_variant(payload_variant(
            "OrderDecision",
            "order_response",
            RadrootsOrderDecisionImport::describe(ctx),
            "crates/trade_bindings/src/dto.rs",
            102,
        ))
        .with_variant(payload_variant(
            "OrderRevisionProposal",
            "order_revision",
            RadrootsOrderRevisionProposalImport::describe(ctx),
            "crates/trade_bindings/src/dto.rs",
            103,
        ))
        .with_variant(payload_variant(
            "OrderRevisionDecisionAccepted",
            "order_revision_accept",
            RadrootsOrderRevisionDecisionImport::describe(ctx),
            "crates/trade_bindings/src/dto.rs",
            104,
        ))
        .with_variant(payload_variant(
            "OrderRevisionDecisionDeclined",
            "order_revision_decline",
            RadrootsOrderRevisionDecisionImport::describe(ctx),
            "crates/trade_bindings/src/dto.rs",
            105,
        ))
        .with_variant(payload_variant(
            "Cancel",
            "cancel",
            RadrootsOrderCancellationImport::describe(ctx),
            "crates/trade_bindings/src/dto.rs",
            106,
        ));
        register(ctx, "RadrootsCommercialMessagePayload", TypeDef::Enum(def))
    }
}

#[derive(dto_bindgen::Dto)]
pub enum RadrootsCommercialMessageType {
    #[serde(rename = "listing_validate_request")]
    ListingValidateRequest,
    #[serde(rename = "listing_validate_result")]
    ListingValidateResult,
    #[serde(rename = "order_request")]
    OrderRequest,
    #[serde(rename = "order_response")]
    OrderResponse,
    #[serde(rename = "order_revision")]
    OrderRevision,
    #[serde(rename = "order_revision_accept")]
    OrderRevisionAccept,
    #[serde(rename = "order_revision_decline")]
    OrderRevisionDecline,
    #[serde(rename = "question")]
    Question,
    #[serde(rename = "answer")]
    Answer,
    #[serde(rename = "discount_request")]
    DiscountRequest,
    #[serde(rename = "discount_offer")]
    DiscountOffer,
    #[serde(rename = "discount_accept")]
    DiscountAccept,
    #[serde(rename = "discount_decline")]
    DiscountDecline,
    #[serde(rename = "cancel")]
    Cancel,
}

#[derive(dto_bindgen::Dto)]
pub enum RadrootsOrderStatus {
    #[serde(rename = "draft")]
    Draft,
    #[serde(rename = "validated")]
    Validated,
    #[serde(rename = "requested")]
    Requested,
    #[serde(rename = "questioned")]
    Questioned,
    #[serde(rename = "revised")]
    Revised,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "declined")]
    Declined,
    #[serde(rename = "cancelled")]
    Cancelled,
}

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
    pub bin: RadrootsListingBinImport,
    pub one_bin_total: RadrootsTradeListingTotal,
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
    pub product: RadrootsListingProductImport,
    pub primary_bin_id: String,
    pub bins: Vec<RadrootsTradeListingBinProjection>,
    pub resource_area: Option<RadrootsResourceAreaRefImport>,
    pub plot: Option<RadrootsPlotRefImport>,
    pub discounts: Option<Vec<RadrootsCoreDiscountImport>>,
    pub inventory_available: Option<RadrootsCoreDecimalImport>,
    pub availability: Option<RadrootsListingAvailabilityImport>,
    pub delivery_method: Option<RadrootsListingDeliveryMethodImport>,
    pub location: Option<RadrootsListingPublicLocationImport>,
    pub images: Option<Vec<RadrootsListingImageImport>>,
    pub order_count: u32,
    pub open_order_count: u32,
    pub terminal_order_count: u32,
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
    #[serde(rename = "open_order_count")]
    OpenOrderCount,
    #[serde(rename = "total_order_count")]
    TotalOrderCount,
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
    pub primary_bin_total: RadrootsTradeListingTotal,
    pub order_count: u32,
    pub open_order_count: u32,
    pub terminal_order_count: u32,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeMarketplaceOrderSummary {
    pub order_id: String,
    pub listing_addr: String,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub status: RadrootsOrderStatus,
    pub last_message_type: RadrootsCommercialMessageType,
    pub item_count: u32,
    pub total_bin_count: u32,
    pub has_requested_discounts: bool,
    pub last_reason: Option<String>,
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
pub struct RadrootsTradeOrderBackofficeOverlay {
    pub order_id: String,
    pub review_queue: Option<RadrootsTradeReviewQueueEntry>,
    pub moderation_flags: Vec<RadrootsTradeModerationFlag>,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeOrderBackofficeQuery {
    pub order: RadrootsTradeOrderQuery,
    pub requires_review: Option<bool>,
    pub has_open_moderation_flags: Option<bool>,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeOrderBackofficeView {
    pub order: RadrootsTradeOrderWorkflowProjection,
    pub marketplace: RadrootsTradeMarketplaceOrderSummary,
    pub overlay: Option<RadrootsTradeOrderBackofficeOverlay>,
    pub requires_review: bool,
    pub open_moderation_flag_count: u32,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeOrderFacets {
    pub buyer_pubkeys: Vec<RadrootsTradeFacetCount>,
    pub seller_pubkeys: Vec<RadrootsTradeFacetCount>,
    pub listing_addrs: Vec<RadrootsTradeFacetCount>,
    pub statuses: Vec<RadrootsTradeFacetCount>,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeOrderQuery {
    pub listing_addr: Option<String>,
    pub buyer_pubkey: Option<String>,
    pub seller_pubkey: Option<String>,
    pub status: Option<RadrootsOrderStatus>,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeOrderSort {
    pub field: RadrootsTradeOrderSortField,
    pub direction: RadrootsTradeSortDirection,
}

#[derive(dto_bindgen::Dto)]
pub enum RadrootsTradeOrderSortField {
    #[serde(rename = "order_id")]
    OrderId,
    #[serde(rename = "listing_addr")]
    ListingAddr,
    #[serde(rename = "buyer_pubkey")]
    BuyerPubkey,
    #[serde(rename = "seller_pubkey")]
    SellerPubkey,
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "last_message_type")]
    LastMessageType,
    #[serde(rename = "total_bin_count")]
    TotalBinCount,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeOrderWorkflowMessage {
    pub event_id: String,
    pub actor_pubkey: String,
    pub counterparty_pubkey: String,
    pub listing_addr: String,
    pub order_id: Option<String>,
    pub listing_event: Option<RadrootsNostrEventPtrImport>,
    pub root_event_id: Option<String>,
    pub prev_event_id: Option<String>,
    pub payload: RadrootsCommercialMessagePayload,
}

#[derive(dto_bindgen::Dto)]
pub struct RadrootsTradeOrderWorkflowProjection {
    pub order_id: String,
    pub listing_addr: String,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub items: Vec<RadrootsOrderItemImport>,
    pub requested_discounts: Option<Vec<RadrootsOrderEconomicLineImport>>,
    pub status: RadrootsOrderStatus,
    pub listing_snapshot: Option<RadrootsNostrEventPtrImport>,
    pub root_event_id: String,
    pub last_event_id: String,
    pub last_discount_request: Option<RadrootsCoreDiscountValueImport>,
    pub last_discount_offer: Option<RadrootsCoreDiscountValueImport>,
    pub accepted_discount: Option<RadrootsCoreDiscountValueImport>,
    pub last_reason: Option<String>,
    pub last_discount_decline_reason: Option<String>,
    pub question_count: u32,
    pub answer_count: u32,
    pub revision_count: u32,
    pub discount_request_count: u32,
    pub discount_offer_count: u32,
    pub discount_accept_count: u32,
    pub discount_decline_count: u32,
    pub cancellation_count: u32,
    pub last_message_type: RadrootsCommercialMessageType,
    pub last_actor_pubkey: String,
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

fn payload_variant(
    rust_name: &str,
    wire_name: &str,
    ty: TypeRef,
    file: &str,
    line: u32,
) -> VariantDef {
    VariantDef::new(
        rust_name,
        wire_name,
        VariantShape::Newtype(ty),
        span(file, line),
    )
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

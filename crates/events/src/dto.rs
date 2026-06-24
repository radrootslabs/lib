use dto_bindgen_core::{
    DescribeCtx, Dto, EnumDef, EnumRepr, FieldDef, FieldPresence, GenericParam, IdentName, IntRepr,
    RootDescriptor, RustTypeId, SourceSpan, StructDef, TargetFieldNames, TypeDef, TypeRef,
    VariantDef, VariantShape, WireFieldNames,
};
use radroots_core::RadrootsCoreDiscountValue;

use crate::{
    RadrootsNostrEvent, RadrootsNostrEventPtr, RadrootsNostrEventRef,
    account::RadrootsAccountClaim,
    app_data::RadrootsAppData,
    comment::RadrootsComment,
    coop::{RadrootsCoop, RadrootsCoopLocation, RadrootsCoopRef},
    document::{RadrootsDocument, RadrootsDocumentSubject},
    farm::{
        RadrootsFarm, RadrootsFarmLocation, RadrootsFarmRef, RadrootsGcsLocation,
        RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon,
    },
    follow::{RadrootsFollow, RadrootsFollowProfile},
    geochat::RadrootsGeoChat,
    gift_wrap::{RadrootsGiftWrap, RadrootsGiftWrapRecipient},
    ids::{
        RadrootsDTag, RadrootsEventId, RadrootsInventoryBinId, RadrootsListingAddress,
        RadrootsOrderId, RadrootsOrderQuoteId, RadrootsOrderRevisionId, RadrootsPublicKey,
    },
    job::{JobFeedbackStatus, JobInputType, JobPaymentRequest},
    job_feedback::RadrootsJobFeedback,
    job_request::{RadrootsJobInput, RadrootsJobParam, RadrootsJobRequest},
    job_result::RadrootsJobResult,
    list::{RadrootsList, RadrootsListEntry},
    list_set::RadrootsListSet,
    listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingImage, RadrootsListingImageSize,
        RadrootsListingLocation, RadrootsListingProduct, RadrootsListingStatus,
    },
    message::{RadrootsMessage, RadrootsMessageRecipient},
    message_file::{RadrootsMessageFile, RadrootsMessageFileDimensions},
    order::{
        RadrootsCommercialDomain, RadrootsListingParseError, RadrootsOrderCancellation,
        RadrootsOrderDecision, RadrootsOrderDecisionOutcome, RadrootsOrderEventType,
        RadrootsOrderInventoryCommitment, RadrootsOrderRequest, RadrootsOrderRevisionDecision,
        RadrootsOrderRevisionOutcome, RadrootsOrderRevisionProposal,
    },
    order_economics::{
        RadrootsOrderEconomicActor, RadrootsOrderEconomicEffect, RadrootsOrderEconomicItem,
        RadrootsOrderEconomicLine, RadrootsOrderEconomicLineKind, RadrootsOrderEconomicTotals,
        RadrootsOrderEconomics, RadrootsOrderItem, RadrootsOrderPricingBasis,
    },
    plot::{RadrootsPlot, RadrootsPlotLocation, RadrootsPlotRef},
    post::RadrootsPost,
    profile::{RadrootsProfile, RadrootsProfileType},
    reaction::RadrootsReaction,
    relay_document::RadrootsRelayDocument,
    resource_area::{RadrootsResourceArea, RadrootsResourceAreaLocation, RadrootsResourceAreaRef},
    resource_cap::{RadrootsResourceHarvestCap, RadrootsResourceHarvestProduct},
    seal::RadrootsSeal,
    trade_validation::{
        RadrootsTradeValidationListingError, RadrootsTradeValidationListingRequest,
        RadrootsTradeValidationListingResult,
    },
};

pub fn dto_roots() -> Vec<RootDescriptor> {
    vec![
        RootDescriptor::new::<JobFeedbackStatus>(),
        RootDescriptor::new::<JobInputType>(),
        RootDescriptor::new::<JobPaymentRequest>(),
        RootDescriptor::new::<RadrootsAccountClaim>(),
        RootDescriptor::new::<RadrootsOrderEnvelopeDto>(),
        RootDescriptor::new::<RadrootsAppData>(),
        RootDescriptor::new::<RadrootsComment>(),
        RootDescriptor::new::<RadrootsCoop>(),
        RootDescriptor::new::<RadrootsCoopLocation>(),
        RootDescriptor::new::<RadrootsCoopRef>(),
        RootDescriptor::new::<RadrootsDocument>(),
        RootDescriptor::new::<RadrootsDocumentSubject>(),
        RootDescriptor::new::<RadrootsFarm>(),
        RootDescriptor::new::<RadrootsFarmLocation>(),
        RootDescriptor::new::<RadrootsFarmRef>(),
        RootDescriptor::new::<RadrootsFollow>(),
        RootDescriptor::new::<RadrootsFollowProfile>(),
        RootDescriptor::new::<RadrootsGcsLocation>(),
        RootDescriptor::new::<RadrootsGeoChat>(),
        RootDescriptor::new::<RadrootsGeoJsonPoint>(),
        RootDescriptor::new::<RadrootsGeoJsonPolygon>(),
        RootDescriptor::new::<RadrootsGiftWrap>(),
        RootDescriptor::new::<RadrootsGiftWrapRecipient>(),
        RootDescriptor::new::<RadrootsJobFeedback>(),
        RootDescriptor::new::<RadrootsJobInput>(),
        RootDescriptor::new::<RadrootsJobParam>(),
        RootDescriptor::new::<RadrootsJobRequest>(),
        RootDescriptor::new::<RadrootsJobResult>(),
        RootDescriptor::new::<RadrootsList>(),
        RootDescriptor::new::<RadrootsListEntry>(),
        RootDescriptor::new::<RadrootsListSet>(),
        RootDescriptor::new::<RadrootsListing>(),
        RootDescriptor::new::<RadrootsListingAvailability>(),
        RootDescriptor::new::<RadrootsListingBin>(),
        RootDescriptor::new::<RadrootsListingDeliveryMethod>(),
        RootDescriptor::new::<RadrootsNostrEvent>(),
        RootDescriptor::new::<RadrootsNostrEventRef>(),
        RootDescriptor::new::<RadrootsNostrEventPtr>(),
        RootDescriptor::new::<RadrootsListingLocation>(),
        RootDescriptor::new::<RadrootsListingProduct>(),
        RootDescriptor::new::<RadrootsListingStatus>(),
        RootDescriptor::new::<RadrootsListingImage>(),
        RootDescriptor::new::<RadrootsMessage>(),
        RootDescriptor::new::<RadrootsMessageFile>(),
        RootDescriptor::new::<RadrootsMessageFileDimensions>(),
        RootDescriptor::new::<RadrootsMessageRecipient>(),
        RootDescriptor::new::<RadrootsPlot>(),
        RootDescriptor::new::<RadrootsPlotLocation>(),
        RootDescriptor::new::<RadrootsPlotRef>(),
        RootDescriptor::new::<RadrootsPost>(),
        RootDescriptor::new::<RadrootsProfile>(),
        RootDescriptor::new::<RadrootsProfileType>(),
        RootDescriptor::new::<RadrootsReaction>(),
        RootDescriptor::new::<RadrootsRelayDocument>(),
        RootDescriptor::new::<RadrootsResourceArea>(),
        RootDescriptor::new::<RadrootsResourceAreaLocation>(),
        RootDescriptor::new::<RadrootsResourceAreaRef>(),
        RootDescriptor::new::<RadrootsResourceHarvestCap>(),
        RootDescriptor::new::<RadrootsResourceHarvestProduct>(),
        RootDescriptor::new::<RadrootsSeal>(),
        RootDescriptor::new::<RadrootsListingAnswerDto>(),
        RootDescriptor::new::<RadrootsListingDiscountDecisionDto>(),
        RootDescriptor::new::<RadrootsListingDiscountOfferDto>(),
        RootDescriptor::new::<RadrootsListingDiscountRequestDto>(),
        RootDescriptor::new::<RadrootsCommercialDomain>(),
        RootDescriptor::new::<RadrootsOrderEconomicActor>(),
        RootDescriptor::new::<RadrootsOrderEconomicEffect>(),
        RootDescriptor::new::<RadrootsOrderEconomicLineKind>(),
        RootDescriptor::new::<RadrootsCommercialEnvelopeDto>(),
        RootDescriptor::new::<RadrootsOrderInventoryCommitment>(),
        RootDescriptor::new::<RadrootsListingCancelDto>(),
        RootDescriptor::new::<RadrootsListingParseError>(),
        RootDescriptor::new::<RadrootsTradeValidationListingRequest>(),
        RootDescriptor::new::<RadrootsTradeValidationListingResult>(),
        RootDescriptor::new::<RadrootsTradeValidationListingError>(),
        RootDescriptor::new::<RadrootsCommercialMessagePayloadDto>(),
        RootDescriptor::new::<RadrootsCommercialMessageTypeDto>(),
        RootDescriptor::new::<RadrootsOrderCancellation>(),
        RootDescriptor::new::<RadrootsOrderChangeDto>(),
        RootDescriptor::new::<RadrootsOrderDecisionOutcome>(),
        RootDescriptor::new::<RadrootsOrderDecision>(),
        RootDescriptor::new::<RadrootsOrderEconomicItem>(),
        RootDescriptor::new::<RadrootsOrderEconomicLine>(),
        RootDescriptor::new::<RadrootsOrderEconomicTotals>(),
        RootDescriptor::new::<RadrootsOrderEconomics>(),
        RootDescriptor::new::<RadrootsOrderItem>(),
        RootDescriptor::new::<RadrootsOrderRequest>(),
        RootDescriptor::new::<RadrootsOrderResponseDto>(),
        RootDescriptor::new::<RadrootsOrderRevisionDto>(),
        RootDescriptor::new::<RadrootsOrderRevisionOutcome>(),
        RootDescriptor::new::<RadrootsOrderRevisionDecision>(),
        RootDescriptor::new::<RadrootsOrderRevisionProposal>(),
        RootDescriptor::new::<RadrootsOrderRevisionResponseDto>(),
        RootDescriptor::new::<RadrootsOrderStatusDto>(),
        RootDescriptor::new::<RadrootsOrderPricingBasis>(),
        RootDescriptor::new::<RadrootsListingQuestionDto>(),
        RootDescriptor::new::<RadrootsCommercialTransportLaneDto>(),
    ]
}

pub struct RadrootsOrderEnvelopeDto;
pub struct RadrootsListingAnswerDto;
pub struct RadrootsListingDiscountDecisionDto;
pub struct RadrootsListingDiscountOfferDto;
pub struct RadrootsListingDiscountRequestDto;
pub struct RadrootsCommercialEnvelopeDto;
pub struct RadrootsListingCancelDto;
pub struct RadrootsCommercialMessagePayloadDto;
pub struct RadrootsCommercialMessageTypeDto;
pub struct RadrootsOrderChangeDto;
pub struct RadrootsOrderResponseDto;
pub struct RadrootsOrderRevisionDto;
pub struct RadrootsOrderRevisionResponseDto;
pub struct RadrootsOrderStatusDto;
pub struct RadrootsListingQuestionDto;
pub struct RadrootsCommercialTransportLaneDto;

macro_rules! string_dto {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Dto for $ty {
                fn describe(_ctx: &mut DescribeCtx) -> TypeRef {
                    TypeRef::String
                }
            }
        )+
    };
}

string_dto!(
    RadrootsDTag,
    RadrootsEventId,
    RadrootsInventoryBinId,
    RadrootsListingAddress,
    RadrootsOrderId,
    RadrootsOrderQuoteId,
    RadrootsOrderRevisionId,
    RadrootsPublicKey,
);

impl Dto for RadrootsNostrEvent {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsNostrEvent",
            "RadrootsNostrEvent",
            span("crates/events/src/lib.rs", 52),
        )
        .with_field(field(
            "id",
            "id",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            53,
        ))
        .with_field(field(
            "author",
            "author",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            54,
        ))
        .with_field(field(
            "created_at",
            "created_at",
            u32::describe(ctx),
            "crates/events/src/lib.rs",
            55,
        ))
        .with_field(field(
            "kind",
            "kind",
            u32::describe(ctx),
            "crates/events/src/lib.rs",
            56,
        ))
        .with_field(field(
            "tags",
            "tags",
            <Vec<Vec<String>> as Dto>::describe(ctx),
            "crates/events/src/lib.rs",
            57,
        ))
        .with_field(field(
            "content",
            "content",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            58,
        ))
        .with_field(field(
            "sig",
            "sig",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            59,
        ));
        register(ctx, "RadrootsNostrEvent", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsNostrEventRef {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsNostrEventRef",
            "RadrootsNostrEventRef",
            span("crates/events/src/lib.rs", 64),
        )
        .with_field(field(
            "id",
            "id",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            65,
        ))
        .with_field(field(
            "author",
            "author",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            66,
        ))
        .with_field(field(
            "kind",
            "kind",
            u32::describe(ctx),
            "crates/events/src/lib.rs",
            67,
        ))
        .with_field(optional_nullable_field(
            "d_tag",
            "d_tag",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/lib.rs",
            68,
        ))
        .with_field(optional_nullable_field(
            "relays",
            "relays",
            <Option<Vec<String>> as Dto>::describe(ctx),
            "crates/events/src/lib.rs",
            69,
        ));
        register(ctx, "RadrootsNostrEventRef", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsNostrEventPtr {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsNostrEventPtr",
            "RadrootsNostrEventPtr",
            span("crates/events/src/lib.rs", 74),
        )
        .with_field(field(
            "id",
            "id",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            75,
        ))
        .with_field(optional_nullable_field(
            "relays",
            "relays",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/lib.rs",
            76,
        ));
        register(ctx, "RadrootsNostrEventPtr", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingProduct {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListingProduct",
            "RadrootsListingProduct",
            span("crates/events/src/listing.rs", 81),
        )
        .with_field(field(
            "key",
            "key",
            String::describe(ctx),
            "crates/events/src/listing.rs",
            82,
        ))
        .with_field(field(
            "title",
            "title",
            String::describe(ctx),
            "crates/events/src/listing.rs",
            83,
        ))
        .with_field(field(
            "category",
            "category",
            String::describe(ctx),
            "crates/events/src/listing.rs",
            84,
        ))
        .with_field(optional_nullable_field(
            "summary",
            "summary",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            85,
        ))
        .with_field(optional_nullable_field(
            "process",
            "process",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            86,
        ))
        .with_field(optional_nullable_field(
            "lot",
            "lot",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            87,
        ))
        .with_field(optional_nullable_field(
            "location",
            "location",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            88,
        ))
        .with_field(optional_nullable_field(
            "profile",
            "profile",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            89,
        ))
        .with_field(optional_nullable_field(
            "year",
            "year",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            90,
        ));
        register(ctx, "RadrootsListingProduct", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingBin {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListingBin",
            "RadrootsListingBin",
            span("crates/events/src/listing.rs", 101),
        )
        .with_field(field(
            "bin_id",
            "bin_id",
            RadrootsInventoryBinId::describe(ctx),
            "crates/events/src/listing.rs",
            102,
        ))
        .with_field(field(
            "quantity",
            "quantity",
            radroots_core::RadrootsCoreQuantity::describe(ctx),
            "crates/events/src/listing.rs",
            103,
        ))
        .with_field(field(
            "price_per_canonical_unit",
            "price_per_canonical_unit",
            radroots_core::RadrootsCoreQuantityPrice::describe(ctx),
            "crates/events/src/listing.rs",
            104,
        ))
        .with_field(optional_nullable_field(
            "display_amount",
            "display_amount",
            TypeRef::option(core_decimal(ctx)),
            "crates/events/src/listing.rs",
            105,
        ))
        .with_field(optional_nullable_field(
            "display_unit",
            "display_unit",
            <Option<radroots_core::RadrootsCoreUnit> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            106,
        ))
        .with_field(optional_nullable_field(
            "display_label",
            "display_label",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            107,
        ))
        .with_field(optional_nullable_field(
            "display_price",
            "display_price",
            <Option<radroots_core::RadrootsCoreMoney> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            108,
        ))
        .with_field(optional_nullable_field(
            "display_price_unit",
            "display_price_unit",
            <Option<radroots_core::RadrootsCoreUnit> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            109,
        ));
        register(ctx, "RadrootsListingBin", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingImageSize {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListingImageSize",
            "RadrootsListingImageSize",
            span("crates/events/src/listing.rs", 133),
        )
        .with_field(field(
            "w",
            "w",
            u32::describe(ctx),
            "crates/events/src/listing.rs",
            134,
        ))
        .with_field(field(
            "h",
            "h",
            u32::describe(ctx),
            "crates/events/src/listing.rs",
            135,
        ));
        register(ctx, "RadrootsListingImageSize", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingImage {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let size = RadrootsListingImageSize::describe(ctx);
        let def = StructDef::new(
            "RadrootsListingImage",
            "RadrootsListingImage",
            span("crates/events/src/listing.rs", 126),
        )
        .with_field(field(
            "url",
            "url",
            String::describe(ctx),
            "crates/events/src/listing.rs",
            127,
        ))
        .with_field(optional_nullable_field(
            "size",
            "size",
            TypeRef::option(size),
            "crates/events/src/listing.rs",
            128,
        ));
        register(ctx, "RadrootsListingImage", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingAvailability {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsListingAvailability",
            "RadrootsListingAvailability",
            EnumRepr::Adjacent {
                tag: "kind".to_owned(),
                content: "amount".to_owned(),
            },
            span("crates/events/src/listing.rs", 20),
        )
        .with_variant(VariantDef::new(
            "Window",
            "window",
            VariantShape::Struct(vec![
                optional_nullable_field(
                    "start",
                    "start",
                    <Option<u64> as Dto>::describe(ctx),
                    "crates/events/src/listing.rs",
                    21,
                )
                .with_int_repr(IntRepr::JsonNumberUnsafe),
                optional_nullable_field(
                    "end",
                    "end",
                    <Option<u64> as Dto>::describe(ctx),
                    "crates/events/src/listing.rs",
                    22,
                )
                .with_int_repr(IntRepr::JsonNumberUnsafe),
            ]),
            span("crates/events/src/listing.rs", 21),
        ))
        .with_variant(VariantDef::new(
            "Status",
            "status",
            VariantShape::Struct(vec![field(
                "status",
                "status",
                RadrootsListingStatus::describe(ctx),
                "crates/events/src/listing.rs",
                24,
            )]),
            span("crates/events/src/listing.rs", 24),
        ));
        register(ctx, "RadrootsListingAvailability", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsListingStatus {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsListingStatus",
            "RadrootsListingStatus",
            EnumRepr::Adjacent {
                tag: "kind".to_owned(),
                content: "amount".to_owned(),
            },
            span("crates/events/src/listing.rs", 36),
        )
        .with_variant(VariantDef::new(
            "Active",
            "active",
            VariantShape::Unit,
            span("crates/events/src/listing.rs", 37),
        ))
        .with_variant(VariantDef::new(
            "Sold",
            "sold",
            VariantShape::Unit,
            span("crates/events/src/listing.rs", 38),
        ))
        .with_variant(VariantDef::new(
            "Other",
            "other",
            VariantShape::Struct(vec![field(
                "value",
                "value",
                String::describe(ctx),
                "crates/events/src/listing.rs",
                39,
            )]),
            span("crates/events/src/listing.rs", 39),
        ));
        register(ctx, "RadrootsListingStatus", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsListingDeliveryMethod {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsListingDeliveryMethod",
            "RadrootsListingDeliveryMethod",
            EnumRepr::Adjacent {
                tag: "kind".to_owned(),
                content: "amount".to_owned(),
            },
            span("crates/events/src/listing.rs", 48),
        )
        .with_variant(VariantDef::new(
            "Pickup",
            "pickup",
            VariantShape::Unit,
            span("crates/events/src/listing.rs", 49),
        ))
        .with_variant(VariantDef::new(
            "LocalDelivery",
            "local_delivery",
            VariantShape::Unit,
            span("crates/events/src/listing.rs", 50),
        ))
        .with_variant(VariantDef::new(
            "Shipping",
            "shipping",
            VariantShape::Unit,
            span("crates/events/src/listing.rs", 51),
        ))
        .with_variant(VariantDef::new(
            "Other",
            "other",
            VariantShape::Struct(vec![field(
                "method",
                "method",
                String::describe(ctx),
                "crates/events/src/listing.rs",
                52,
            )]),
            span("crates/events/src/listing.rs", 52),
        ));
        register(ctx, "RadrootsListingDeliveryMethod", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsListing {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListing",
            "RadrootsListing",
            span("crates/events/src/listing.rs", 57),
        )
        .with_field(field(
            "d_tag",
            "d_tag",
            RadrootsDTag::describe(ctx),
            "crates/events/src/listing.rs",
            58,
        ))
        .with_field(
            optional_nullable_field(
                "published_at",
                "published_at",
                <Option<u64> as Dto>::describe(ctx),
                "crates/events/src/listing.rs",
                63,
            )
            .with_presence(FieldPresence::optional_nullable_skip_if_none())
            .with_int_repr(IntRepr::JsonNumberUnsafe),
        )
        .with_field(field(
            "farm",
            "farm",
            RadrootsFarmRef::describe(ctx),
            "crates/events/src/listing.rs",
            68,
        ))
        .with_field(field(
            "product",
            "product",
            RadrootsListingProduct::describe(ctx),
            "crates/events/src/listing.rs",
            69,
        ))
        .with_field(field(
            "primary_bin_id",
            "primary_bin_id",
            RadrootsInventoryBinId::describe(ctx),
            "crates/events/src/listing.rs",
            70,
        ))
        .with_field(field(
            "bins",
            "bins",
            <Vec<RadrootsListingBin> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            71,
        ))
        .with_field(optional_nullable_field(
            "resource_area",
            "resource_area",
            <Option<RadrootsResourceAreaRef> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            72,
        ))
        .with_field(optional_nullable_field(
            "plot",
            "plot",
            <Option<RadrootsPlotRef> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            73,
        ))
        .with_field(optional_nullable_field(
            "discounts",
            "discounts",
            <Option<Vec<radroots_core::RadrootsCoreDiscount>> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            74,
        ))
        .with_field(optional_nullable_field(
            "inventory_available",
            "inventory_available",
            TypeRef::option(core_decimal(ctx)),
            "crates/events/src/listing.rs",
            75,
        ))
        .with_field(optional_nullable_field(
            "availability",
            "availability",
            <Option<RadrootsListingAvailability> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            76,
        ))
        .with_field(optional_nullable_field(
            "delivery_method",
            "delivery_method",
            <Option<RadrootsListingDeliveryMethod> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            77,
        ))
        .with_field(optional_nullable_field(
            "location",
            "location",
            <Option<RadrootsListingLocation> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            78,
        ))
        .with_field(optional_nullable_field(
            "images",
            "images",
            <Option<Vec<RadrootsListingImage>> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            79,
        ));
        register(ctx, "RadrootsListing", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsResourceHarvestCap {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsResourceHarvestCap",
            "RadrootsResourceHarvestCap",
            span("crates/events/src/resource_cap.rs", 20),
        )
        .with_field(field(
            "d_tag",
            "d_tag",
            String::describe(ctx),
            "crates/events/src/resource_cap.rs",
            21,
        ))
        .with_field(field(
            "resource_area",
            "resource_area",
            RadrootsResourceAreaRef::describe(ctx),
            "crates/events/src/resource_cap.rs",
            22,
        ))
        .with_field(field(
            "product",
            "product",
            RadrootsResourceHarvestProduct::describe(ctx),
            "crates/events/src/resource_cap.rs",
            23,
        ))
        .with_field(
            field(
                "start",
                "start",
                u64::describe(ctx),
                "crates/events/src/resource_cap.rs",
                24,
            )
            .with_int_repr(IntRepr::NonJsonBigint),
        )
        .with_field(
            field(
                "end",
                "end",
                u64::describe(ctx),
                "crates/events/src/resource_cap.rs",
                25,
            )
            .with_int_repr(IntRepr::NonJsonBigint),
        )
        .with_field(field(
            "cap_quantity",
            "cap_quantity",
            radroots_core::RadrootsCoreQuantity::describe(ctx),
            "crates/events/src/resource_cap.rs",
            26,
        ))
        .with_field(optional_nullable_field(
            "display_amount",
            "display_amount",
            TypeRef::option(core_decimal(ctx)),
            "crates/events/src/resource_cap.rs",
            27,
        ))
        .with_field(optional_nullable_field(
            "display_unit",
            "display_unit",
            <Option<radroots_core::RadrootsCoreUnit> as Dto>::describe(ctx),
            "crates/events/src/resource_cap.rs",
            28,
        ))
        .with_field(optional_nullable_field(
            "display_label",
            "display_label",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/resource_cap.rs",
            29,
        ))
        .with_field(optional_nullable_field(
            "tags",
            "tags",
            <Option<Vec<String>> as Dto>::describe(ctx),
            "crates/events/src/resource_cap.rs",
            30,
        ));
        register(ctx, "RadrootsResourceHarvestCap", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsComment {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let event_ref = RadrootsNostrEventRef::describe(ctx);
        let def = StructDef::new(
            "RadrootsComment",
            "RadrootsComment",
            span("crates/events/src/comment.rs", 8),
        )
        .with_field(field(
            "root",
            "root",
            event_ref,
            "crates/events/src/comment.rs",
            9,
        ))
        .with_field(field(
            "parent",
            "parent",
            RadrootsNostrEventRef::describe(ctx),
            "crates/events/src/comment.rs",
            10,
        ))
        .with_field(field(
            "content",
            "content",
            String::describe(ctx),
            "crates/events/src/comment.rs",
            11,
        ));
        register(ctx, "RadrootsComment", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsReaction {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsReaction",
            "RadrootsReaction",
            span("crates/events/src/reaction.rs", 8),
        )
        .with_field(field(
            "root",
            "root",
            RadrootsNostrEventRef::describe(ctx),
            "crates/events/src/reaction.rs",
            9,
        ))
        .with_field(field(
            "content",
            "content",
            String::describe(ctx),
            "crates/events/src/reaction.rs",
            10,
        ));
        register(ctx, "RadrootsReaction", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsPost {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsPost",
            "RadrootsPost",
            span("crates/events/src/post.rs", 11),
        )
        .with_field(field(
            "content",
            "content",
            String::describe(ctx),
            "crates/events/src/post.rs",
            12,
        ));
        register(ctx, "RadrootsPost", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingParseError {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsListingParseError",
            "RadrootsListingParseError",
            EnumRepr::External,
            span("crates/events/src/order.rs", 26),
        )
        .with_variant(VariantDef::new(
            "InvalidKind",
            "InvalidKind",
            VariantShape::Newtype(u32::describe(ctx)),
            span("crates/events/src/order.rs", 27),
        ))
        .with_variant(VariantDef::new(
            "MissingTag",
            "MissingTag",
            VariantShape::Newtype(String::describe(ctx)),
            span("crates/events/src/order.rs", 28),
        ))
        .with_variant(VariantDef::new(
            "InvalidTag",
            "InvalidTag",
            VariantShape::Newtype(String::describe(ctx)),
            span("crates/events/src/order.rs", 29),
        ))
        .with_variant(VariantDef::new(
            "InvalidNumber",
            "InvalidNumber",
            VariantShape::Newtype(String::describe(ctx)),
            span("crates/events/src/order.rs", 30),
        ))
        .with_variant(VariantDef::new(
            "InvalidUnit",
            "InvalidUnit",
            VariantShape::Unit,
            span("crates/events/src/order.rs", 31),
        ))
        .with_variant(VariantDef::new(
            "InvalidCurrency",
            "InvalidCurrency",
            VariantShape::Unit,
            span("crates/events/src/order.rs", 32),
        ))
        .with_variant(VariantDef::new(
            "InvalidJson",
            "InvalidJson",
            VariantShape::Newtype(String::describe(ctx)),
            span("crates/events/src/order.rs", 33),
        ))
        .with_variant(VariantDef::new(
            "InvalidDiscount",
            "InvalidDiscount",
            VariantShape::Newtype(String::describe(ctx)),
            span("crates/events/src/order.rs", 34),
        ));
        register(ctx, "RadrootsListingParseError", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsTradeValidationListingError {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let parse_error = RadrootsListingParseError::describe(ctx);
        let def = EnumDef::new(
            "RadrootsTradeValidationListingError",
            "RadrootsTradeListingValidationError",
            EnumRepr::Adjacent {
                tag: "kind".to_owned(),
                content: "amount".to_owned(),
            },
            span("crates/events/src/trade_validation.rs", 14),
        )
        .with_variant(VariantDef::new(
            "InvalidKind",
            "invalid_kind",
            VariantShape::Struct(vec![field(
                "kind",
                "kind",
                u32::describe(ctx),
                "crates/events/src/trade_validation.rs",
                15,
            )]),
            span("crates/events/src/trade_validation.rs", 15),
        ))
        .with_variant(unit_variant(
            "MissingListingId",
            "missing_listing_id",
            "crates/events/src/trade_validation.rs",
            16,
        ))
        .with_variant(VariantDef::new(
            "ListingEventNotFound",
            "listing_event_not_found",
            VariantShape::Struct(vec![field(
                "listing_addr",
                "listing_addr",
                String::describe(ctx),
                "crates/events/src/trade_validation.rs",
                17,
            )]),
            span("crates/events/src/trade_validation.rs", 17),
        ))
        .with_variant(VariantDef::new(
            "ListingEventFetchFailed",
            "listing_event_fetch_failed",
            VariantShape::Struct(vec![field(
                "listing_addr",
                "listing_addr",
                String::describe(ctx),
                "crates/events/src/trade_validation.rs",
                18,
            )]),
            span("crates/events/src/trade_validation.rs", 18),
        ))
        .with_variant(VariantDef::new(
            "ParseError",
            "parse_error",
            VariantShape::Struct(vec![field(
                "error",
                "error",
                parse_error,
                "crates/events/src/trade_validation.rs",
                19,
            )]),
            span("crates/events/src/trade_validation.rs", 19),
        ))
        .with_variant(unit_variant(
            "InvalidSeller",
            "invalid_seller",
            "crates/events/src/trade_validation.rs",
            20,
        ))
        .with_variant(unit_variant(
            "MissingFarmProfile",
            "missing_farm_profile",
            "crates/events/src/trade_validation.rs",
            21,
        ))
        .with_variant(unit_variant(
            "MissingFarmRecord",
            "missing_farm_record",
            "crates/events/src/trade_validation.rs",
            22,
        ))
        .with_variant(unit_variant(
            "MissingTitle",
            "missing_title",
            "crates/events/src/trade_validation.rs",
            23,
        ))
        .with_variant(unit_variant(
            "MissingDescription",
            "missing_description",
            "crates/events/src/trade_validation.rs",
            24,
        ))
        .with_variant(unit_variant(
            "MissingProductType",
            "missing_product_type",
            "crates/events/src/trade_validation.rs",
            25,
        ))
        .with_variant(unit_variant(
            "MissingBins",
            "missing_bins",
            "crates/events/src/trade_validation.rs",
            26,
        ))
        .with_variant(unit_variant(
            "MissingPrimaryBin",
            "missing_primary_bin",
            "crates/events/src/trade_validation.rs",
            27,
        ))
        .with_variant(unit_variant(
            "InvalidBin",
            "invalid_bin",
            "crates/events/src/trade_validation.rs",
            28,
        ))
        .with_variant(unit_variant(
            "MissingPrice",
            "missing_price",
            "crates/events/src/trade_validation.rs",
            29,
        ))
        .with_variant(unit_variant(
            "InvalidPrice",
            "invalid_price",
            "crates/events/src/trade_validation.rs",
            30,
        ))
        .with_variant(unit_variant(
            "MissingInventory",
            "missing_inventory",
            "crates/events/src/trade_validation.rs",
            31,
        ))
        .with_variant(unit_variant(
            "InvalidInventory",
            "invalid_inventory",
            "crates/events/src/trade_validation.rs",
            32,
        ))
        .with_variant(unit_variant(
            "MissingAvailability",
            "missing_availability",
            "crates/events/src/trade_validation.rs",
            33,
        ))
        .with_variant(unit_variant(
            "MissingLocation",
            "missing_location",
            "crates/events/src/trade_validation.rs",
            34,
        ))
        .with_variant(unit_variant(
            "MissingDeliveryMethod",
            "missing_delivery_method",
            "crates/events/src/trade_validation.rs",
            35,
        ));
        register(
            ctx,
            "RadrootsTradeValidationListingError",
            TypeDef::Enum(def),
        )
    }
}

impl Dto for RadrootsOrderEconomicItem {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsOrderEconomicItem",
            "RadrootsOrderEconomicItem",
            span("crates/events/src/order_economics.rs", 70),
        )
        .with_field(field(
            "bin_id",
            "bin_id",
            RadrootsInventoryBinId::describe(ctx),
            "crates/events/src/order_economics.rs",
            71,
        ))
        .with_field(field(
            "bin_count",
            "bin_count",
            u32::describe(ctx),
            "crates/events/src/order_economics.rs",
            72,
        ))
        .with_field(field(
            "quantity_amount",
            "quantity_amount",
            core_decimal(ctx),
            "crates/events/src/order_economics.rs",
            73,
        ))
        .with_field(field(
            "quantity_unit",
            "quantity_unit",
            radroots_core::RadrootsCoreUnit::describe(ctx),
            "crates/events/src/order_economics.rs",
            74,
        ))
        .with_field(field(
            "unit_price_amount",
            "unit_price_amount",
            core_decimal(ctx),
            "crates/events/src/order_economics.rs",
            75,
        ))
        .with_field(field(
            "unit_price_currency",
            "unit_price_currency",
            core_currency(ctx),
            "crates/events/src/order_economics.rs",
            76,
        ))
        .with_field(field(
            "line_subtotal",
            "line_subtotal",
            radroots_core::RadrootsCoreMoney::describe(ctx),
            "crates/events/src/order_economics.rs",
            77,
        ));
        register(ctx, "RadrootsOrderEconomicItem", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsOrderEconomics {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsOrderEconomics",
            "RadrootsOrderEconomics",
            span("crates/events/src/order_economics.rs", 119),
        )
        .with_field(field(
            "quote_id",
            "quote_id",
            RadrootsOrderQuoteId::describe(ctx),
            "crates/events/src/order_economics.rs",
            120,
        ))
        .with_field(field(
            "quote_version",
            "quote_version",
            u32::describe(ctx),
            "crates/events/src/order_economics.rs",
            121,
        ))
        .with_field(field(
            "pricing_basis",
            "pricing_basis",
            RadrootsOrderPricingBasis::describe(ctx),
            "crates/events/src/order_economics.rs",
            122,
        ))
        .with_field(field(
            "currency",
            "currency",
            core_currency(ctx),
            "crates/events/src/order_economics.rs",
            123,
        ))
        .with_field(field(
            "items",
            "items",
            <Vec<RadrootsOrderEconomicItem> as Dto>::describe(ctx),
            "crates/events/src/order_economics.rs",
            124,
        ))
        .with_field(field(
            "discounts",
            "discounts",
            <Vec<RadrootsOrderEconomicLine> as Dto>::describe(ctx),
            "crates/events/src/order_economics.rs",
            125,
        ))
        .with_field(field(
            "adjustments",
            "adjustments",
            <Vec<RadrootsOrderEconomicLine> as Dto>::describe(ctx),
            "crates/events/src/order_economics.rs",
            126,
        ))
        .with_field(field(
            "subtotal",
            "subtotal",
            radroots_core::RadrootsCoreMoney::describe(ctx),
            "crates/events/src/order_economics.rs",
            127,
        ))
        .with_field(field(
            "discount_total",
            "discount_total",
            radroots_core::RadrootsCoreMoney::describe(ctx),
            "crates/events/src/order_economics.rs",
            128,
        ))
        .with_field(field(
            "adjustment_total",
            "adjustment_total",
            radroots_core::RadrootsCoreMoney::describe(ctx),
            "crates/events/src/order_economics.rs",
            129,
        ))
        .with_field(field(
            "total",
            "total",
            radroots_core::RadrootsCoreMoney::describe(ctx),
            "crates/events/src/order_economics.rs",
            130,
        ));
        register(ctx, "RadrootsOrderEconomics", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsOrderRevisionOutcome {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsOrderRevisionOutcome",
            "RadrootsOrderRevisionDecision",
            EnumRepr::Internal {
                tag: "decision".to_owned(),
            },
            span("crates/events/src/order.rs", 221),
        )
        .with_variant(VariantDef::new(
            "Accepted",
            "accepted",
            VariantShape::Unit,
            span("crates/events/src/order.rs", 222),
        ))
        .with_variant(VariantDef::new(
            "Declined",
            "declined",
            VariantShape::Struct(vec![field(
                "reason",
                "reason",
                String::describe(ctx),
                "crates/events/src/order.rs",
                223,
            )]),
            span("crates/events/src/order.rs", 223),
        ));
        register(ctx, "RadrootsOrderRevisionOutcome", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsOrderEnvelopeDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef {
            generics: vec![GenericParam::new("T")],
            ..StructDef::new(
                "RadrootsOrderEnvelopeDto",
                "RadrootsOrderEnvelope",
                span("crates/events/src/order.rs", 408),
            )
        }
        .with_field(field(
            "version",
            "version",
            u16::describe(ctx),
            "crates/events/src/order.rs",
            409,
        ))
        .with_field(field(
            "domain",
            "domain",
            RadrootsCommercialDomain::describe(ctx),
            "crates/events/src/order.rs",
            410,
        ))
        .with_field(field(
            "message_type",
            "type",
            RadrootsOrderEventType::describe(ctx),
            "crates/events/src/order.rs",
            412,
        ))
        .with_field(field(
            "order_id",
            "order_id",
            String::describe(ctx),
            "crates/events/src/order.rs",
            413,
        ))
        .with_field(field(
            "listing_addr",
            "listing_addr",
            String::describe(ctx),
            "crates/events/src/order.rs",
            414,
        ))
        .with_field(field(
            "payload",
            "payload",
            TypeRef::GenericParam("T".to_owned()),
            "crates/events/src/order.rs",
            415,
        ));
        register(ctx, "RadrootsOrderEnvelopeDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsCommercialEnvelopeDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef {
            generics: vec![GenericParam::new("T")],
            ..StructDef::new(
                "RadrootsCommercialEnvelopeDto",
                "RadrootsCommercialEnvelope",
                span("crates/events/src/order.rs", 408),
            )
        }
        .with_field(field(
            "version",
            "version",
            u16::describe(ctx),
            "crates/events/src/order.rs",
            409,
        ))
        .with_field(field(
            "domain",
            "domain",
            RadrootsCommercialDomain::describe(ctx),
            "crates/events/src/order.rs",
            410,
        ))
        .with_field(field(
            "message_type",
            "type",
            RadrootsCommercialMessageTypeDto::describe(ctx),
            "crates/events/src/order.rs",
            412,
        ))
        .with_field(optional_nullable_field(
            "order_id",
            "order_id",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/order.rs",
            413,
        ))
        .with_field(field(
            "listing_addr",
            "listing_addr",
            String::describe(ctx),
            "crates/events/src/order.rs",
            414,
        ))
        .with_field(field(
            "payload",
            "payload",
            TypeRef::GenericParam("T".to_owned()),
            "crates/events/src/order.rs",
            415,
        ));
        register(ctx, "RadrootsCommercialEnvelopeDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsCommercialMessageTypeDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsCommercialMessageTypeDto",
            "RadrootsCommercialMessageType",
            EnumRepr::External,
            span("crates/events/src/order.rs", 408),
        )
        .with_variant(unit_variant(
            "ListingValidateRequest",
            "listing_validate_request",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "ListingValidateResult",
            "listing_validate_result",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "OrderRequest",
            "order_request",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "OrderResponse",
            "order_response",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "OrderRevision",
            "order_revision",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "OrderRevisionAccept",
            "order_revision_accept",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "OrderRevisionDecline",
            "order_revision_decline",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Question",
            "question",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Answer",
            "answer",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "DiscountRequest",
            "discount_request",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "DiscountOffer",
            "discount_offer",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "DiscountAccept",
            "discount_accept",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "DiscountDecline",
            "discount_decline",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Cancel",
            "cancel",
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsCommercialMessageTypeDto", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsCommercialTransportLaneDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsCommercialTransportLaneDto",
            "RadrootsCommercialTransportLane",
            EnumRepr::External,
            span("crates/events/src/order.rs", 408),
        )
        .with_variant(unit_variant(
            "Service",
            "service",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Public",
            "public",
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsCommercialTransportLaneDto", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsOrderStatusDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsOrderStatusDto",
            "RadrootsOrderStatus",
            EnumRepr::External,
            span("crates/events/src/order.rs", 408),
        )
        .with_variant(unit_variant(
            "Draft",
            "draft",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Validated",
            "validated",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Requested",
            "requested",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Questioned",
            "questioned",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Revised",
            "revised",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Accepted",
            "accepted",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Declined",
            "declined",
            "crates/events/src/order.rs",
            408,
        ))
        .with_variant(unit_variant(
            "Cancelled",
            "cancelled",
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsOrderStatusDto", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsListingQuestionDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListingQuestionDto",
            "RadrootsListingQuestion",
            span("crates/events/src/order.rs", 408),
        )
        .with_field(field(
            "question_id",
            "question_id",
            String::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsListingQuestionDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingAnswerDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListingAnswerDto",
            "RadrootsListingAnswer",
            span("crates/events/src/order.rs", 408),
        )
        .with_field(field(
            "question_id",
            "question_id",
            String::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsListingAnswerDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingDiscountRequestDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListingDiscountRequestDto",
            "RadrootsListingDiscountRequest",
            span("crates/events/src/order.rs", 408),
        )
        .with_field(field(
            "discount_id",
            "discount_id",
            String::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ))
        .with_field(field(
            "value",
            "value",
            RadrootsCoreDiscountValue::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsListingDiscountRequestDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingDiscountOfferDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListingDiscountOfferDto",
            "RadrootsListingDiscountOffer",
            span("crates/events/src/order.rs", 408),
        )
        .with_field(field(
            "discount_id",
            "discount_id",
            String::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ))
        .with_field(field(
            "value",
            "value",
            RadrootsCoreDiscountValue::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsListingDiscountOfferDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingDiscountDecisionDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsListingDiscountDecisionDto",
            "RadrootsListingDiscountDecision",
            EnumRepr::Adjacent {
                tag: "kind".to_owned(),
                content: "amount".to_owned(),
            },
            span("crates/events/src/order.rs", 408),
        )
        .with_variant(VariantDef::new(
            "Accept",
            "accept",
            VariantShape::Struct(vec![field(
                "value",
                "value",
                RadrootsCoreDiscountValue::describe(ctx),
                "crates/events/src/order.rs",
                408,
            )]),
            span("crates/events/src/order.rs", 408),
        ))
        .with_variant(VariantDef::new(
            "Decline",
            "decline",
            VariantShape::Struct(vec![optional_nullable_field(
                "reason",
                "reason",
                <Option<String> as Dto>::describe(ctx),
                "crates/events/src/order.rs",
                408,
            )]),
            span("crates/events/src/order.rs", 408),
        ));
        register(ctx, "RadrootsListingDiscountDecisionDto", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsListingCancelDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListingCancelDto",
            "RadrootsListingCancel",
            span("crates/events/src/order.rs", 408),
        )
        .with_field(optional_nullable_field(
            "reason",
            "reason",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsListingCancelDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsOrderResponseDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        accepted_reason_struct(
            ctx,
            "RadrootsOrderResponseDto",
            "RadrootsOrderResponse",
        )
    }
}

impl Dto for RadrootsOrderRevisionResponseDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        accepted_reason_struct(
            ctx,
            "RadrootsOrderRevisionResponseDto",
            "RadrootsOrderRevisionResponse",
        )
    }
}

impl Dto for RadrootsOrderRevisionDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsOrderRevisionDto",
            "RadrootsOrderRevision",
            span("crates/events/src/order.rs", 408),
        )
        .with_field(field(
            "revision_id",
            "revision_id",
            String::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ))
        .with_field(field(
            "changes",
            "changes",
            <Vec<RadrootsOrderChangeDto> as Dto>::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsOrderRevisionDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsOrderChangeDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsOrderChangeDto",
            "RadrootsOrderChange",
            EnumRepr::Adjacent {
                tag: "kind".to_owned(),
                content: "amount".to_owned(),
            },
            span("crates/events/src/order.rs", 408),
        )
        .with_variant(VariantDef::new(
            "BinCount",
            "bin_count",
            VariantShape::Struct(vec![
                field(
                    "item_index",
                    "item_index",
                    u32::describe(ctx),
                    "crates/events/src/order.rs",
                    408,
                ),
                field(
                    "bin_count",
                    "bin_count",
                    u32::describe(ctx),
                    "crates/events/src/order.rs",
                    408,
                ),
            ]),
            span("crates/events/src/order.rs", 408),
        ))
        .with_variant(VariantDef::new(
            "ItemAdd",
            "item_add",
            VariantShape::Struct(vec![field(
                "item",
                "item",
                RadrootsOrderItem::describe(ctx),
                "crates/events/src/order.rs",
                408,
            )]),
            span("crates/events/src/order.rs", 408),
        ))
        .with_variant(VariantDef::new(
            "ItemRemove",
            "item_remove",
            VariantShape::Struct(vec![field(
                "item_index",
                "item_index",
                u32::describe(ctx),
                "crates/events/src/order.rs",
                408,
            )]),
            span("crates/events/src/order.rs", 408),
        ));
        register(ctx, "RadrootsOrderChangeDto", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsCommercialMessagePayloadDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsCommercialMessagePayloadDto",
            "RadrootsCommercialMessagePayload",
            EnumRepr::Adjacent {
                tag: "kind".to_owned(),
                content: "amount".to_owned(),
            },
            span("crates/events/src/order.rs", 408),
        )
        .with_variant(newtype_variant(
            "ListingValidateRequest",
            "listing_validate_request",
            RadrootsTradeValidationListingRequest::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "ListingValidateResult",
            "listing_validate_result",
            RadrootsTradeValidationListingResult::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "TradeOrderRequested",
            "trade_order_requested",
            RadrootsOrderRequest::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "OrderResponse",
            "order_response",
            RadrootsOrderResponseDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "OrderRevision",
            "order_revision",
            RadrootsOrderRevisionDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "OrderRevisionAccept",
            "order_revision_accept",
            RadrootsOrderRevisionResponseDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "OrderRevisionDecline",
            "order_revision_decline",
            RadrootsOrderRevisionResponseDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "Question",
            "question",
            RadrootsListingQuestionDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "Answer",
            "answer",
            RadrootsListingAnswerDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "DiscountRequest",
            "discount_request",
            RadrootsListingDiscountRequestDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "DiscountOffer",
            "discount_offer",
            RadrootsListingDiscountOfferDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "DiscountAccept",
            "discount_accept",
            RadrootsListingDiscountDecisionDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "DiscountDecline",
            "discount_decline",
            RadrootsListingDiscountDecisionDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "Cancel",
            "cancel",
            RadrootsListingCancelDto::describe(ctx),
        ));
        register(ctx, "RadrootsCommercialMessagePayloadDto", TypeDef::Enum(def))
    }
}

fn register(ctx: &mut DescribeCtx, rust_ident: &str, type_def: TypeDef) -> TypeRef {
    ctx.register_type(RustTypeId::new("radroots_events", rust_ident), type_def)
}

fn core_decimal(ctx: &mut DescribeCtx) -> TypeRef {
    external_core_alias(ctx, "RadrootsCoreDecimal")
}

fn core_currency(ctx: &mut DescribeCtx) -> TypeRef {
    external_core_alias(ctx, "RadrootsCoreCurrency")
}

fn external_core_alias(ctx: &mut DescribeCtx, rust_ident: &str) -> TypeRef {
    ctx.register_type(
        RustTypeId::new("radroots_core", rust_ident),
        TypeDef::Struct(StructDef::new(
            rust_ident,
            rust_ident,
            span("crates/core/src/dto.rs", 1),
        )),
    )
}

fn accepted_reason_struct(ctx: &mut DescribeCtx, rust_ident: &str, export_name: &str) -> TypeRef {
    let def = StructDef::new(
        rust_ident,
        export_name,
        span("crates/events/src/order.rs", 408),
    )
    .with_field(field(
        "accepted",
        "accepted",
        bool::describe(ctx),
        "crates/events/src/order.rs",
        408,
    ))
    .with_field(optional_nullable_field(
        "reason",
        "reason",
        <Option<String> as Dto>::describe(ctx),
        "crates/events/src/order.rs",
        408,
    ));
    register(ctx, rust_ident, TypeDef::Struct(def))
}

fn unit_variant(rust_name: &str, wire_name: &str, file: &str, line: u32) -> VariantDef {
    VariantDef::new(rust_name, wire_name, VariantShape::Unit, span(file, line))
}

fn newtype_variant(rust_name: &str, wire_name: &str, ty: TypeRef) -> VariantDef {
    VariantDef::new(
        rust_name,
        wire_name,
        VariantShape::Newtype(ty),
        span("crates/events/src/order.rs", 408),
    )
}

fn optional_nullable_field(
    rust_name: &str,
    wire_name: &str,
    ty: TypeRef,
    file: &str,
    line: u32,
) -> FieldDef {
    field(rust_name, wire_name, ty, file, line).with_presence(FieldPresence::optional_nullable())
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

#[cfg(test)]
mod tests {
    use dto_bindgen_core::{TypeDef, build_registry};

    use super::dto_roots;

    #[test]
    fn event_descriptor_roots_build_registry() {
        let registry = build_registry(dto_roots());

        assert!(!registry.has_errors());
        assert_eq!(registry.roots.len(), dto_roots().len());
        assert!(registry.types_by_id.values().any(
            |def| matches!(def, TypeDef::Struct(def) if def.export_name == "RadrootsNostrEvent")
        ));
        assert!(
            registry
                .types_by_id
                .values()
                .any(|def| matches!(def, TypeDef::Struct(def) if def.export_name == "RadrootsListingImageSize"))
        );
    }

    #[test]
    fn option_fields_are_optional_nullable() {
        let registry = build_registry(dto_roots());

        let product = registry
            .types_by_id
            .values()
            .find_map(|def| match def {
                TypeDef::Struct(def) if def.export_name == "RadrootsListingProduct" => Some(def),
                _ => None,
            })
            .expect("listing product descriptor exists");
        let summary = product
            .fields
            .iter()
            .find(|field| field.rust_name.as_str() == "summary")
            .expect("summary field exists");
        assert!(!summary.presence.required_on_deserialize);
        assert!(summary.presence.nullable);

        let event_ref = registry
            .types_by_id
            .values()
            .find_map(|def| match def {
                TypeDef::Struct(def) if def.export_name == "RadrootsNostrEventRef" => Some(def),
                _ => None,
            })
            .expect("event ref descriptor exists");
        let d_tag = event_ref
            .fields
            .iter()
            .find(|field| field.rust_name.as_str() == "d_tag")
            .expect("d_tag field exists");
        assert!(!d_tag.presence.required_on_deserialize);
        assert!(d_tag.presence.nullable);
    }
}

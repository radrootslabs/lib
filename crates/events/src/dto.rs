use dto_bindgen_core::{
    DescribeCtx, Dto, EnumDef, EnumRepr, FieldDef, FieldPresence, GenericParam, IdentName,
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
    farm::{RadrootsFarm, RadrootsFarmPublicLocation, RadrootsFarmRef},
    follow::{RadrootsFollow, RadrootsFollowProfile},
    gcs::{RadrootsGcsLocation, RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon},
    geochat::RadrootsGeoChat,
    gift_wrap::{RadrootsGiftWrap, RadrootsGiftWrapRecipient},
    job::{JobFeedbackStatus, JobInputType, JobPaymentRequest},
    job_feedback::RadrootsJobFeedback,
    job_request::{RadrootsJobInput, RadrootsJobParam, RadrootsJobRequest},
    job_result::RadrootsJobResult,
    list::{RadrootsList, RadrootsListEntry},
    list_set::RadrootsListSet,
    listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingImage, RadrootsListingProduct,
        RadrootsListingPublicLocation, RadrootsListingStatus,
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
        RootDescriptor::new::<RadrootsFarmPublicLocation>(),
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
        RootDescriptor::new::<RadrootsListingPublicLocation>(),
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
        RootDescriptor::new::<RadrootsOrderAnswerDto>(),
        RootDescriptor::new::<RadrootsOrderDiscountDecisionDto>(),
        RootDescriptor::new::<RadrootsOrderDiscountOfferDto>(),
        RootDescriptor::new::<RadrootsOrderDiscountRequestDto>(),
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
        RootDescriptor::new::<RadrootsOrderQuestionDto>(),
        RootDescriptor::new::<RadrootsCommercialTransportLaneDto>(),
    ]
}

pub struct RadrootsOrderEnvelopeDto;
pub struct RadrootsOrderAnswerDto;
pub struct RadrootsOrderDiscountDecisionDto;
pub struct RadrootsOrderDiscountOfferDto;
pub struct RadrootsOrderDiscountRequestDto;
pub struct RadrootsCommercialEnvelopeDto;
pub struct RadrootsListingCancelDto;
pub struct RadrootsCommercialMessagePayloadDto;
pub struct RadrootsCommercialMessageTypeDto;
pub struct RadrootsOrderChangeDto;
pub struct RadrootsOrderResponseDto;
pub struct RadrootsOrderRevisionDto;
pub struct RadrootsOrderRevisionResponseDto;
pub struct RadrootsOrderStatusDto;
pub struct RadrootsOrderQuestionDto;
pub struct RadrootsCommercialTransportLaneDto;

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
        register(
            ctx,
            "RadrootsCommercialTransportLaneDto",
            TypeDef::Enum(def),
        )
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

impl Dto for RadrootsOrderQuestionDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsOrderQuestionDto",
            "RadrootsOrderQuestion",
            span("crates/events/src/order.rs", 408),
        )
        .with_field(field(
            "question_id",
            "question_id",
            String::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsOrderQuestionDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsOrderAnswerDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsOrderAnswerDto",
            "RadrootsOrderAnswer",
            span("crates/events/src/order.rs", 408),
        )
        .with_field(field(
            "question_id",
            "question_id",
            String::describe(ctx),
            "crates/events/src/order.rs",
            408,
        ));
        register(ctx, "RadrootsOrderAnswerDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsOrderDiscountRequestDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsOrderDiscountRequestDto",
            "RadrootsOrderDiscountRequest",
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
        register(ctx, "RadrootsOrderDiscountRequestDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsOrderDiscountOfferDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsOrderDiscountOfferDto",
            "RadrootsOrderDiscountOffer",
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
        register(ctx, "RadrootsOrderDiscountOfferDto", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsOrderDiscountDecisionDto {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsOrderDiscountDecisionDto",
            "RadrootsOrderDiscountDecision",
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
        register(ctx, "RadrootsOrderDiscountDecisionDto", TypeDef::Enum(def))
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
        accepted_reason_struct(ctx, "RadrootsOrderResponseDto", "RadrootsOrderResponse")
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
            RadrootsOrderQuestionDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "Answer",
            "answer",
            RadrootsOrderAnswerDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "DiscountRequest",
            "discount_request",
            RadrootsOrderDiscountRequestDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "DiscountOffer",
            "discount_offer",
            RadrootsOrderDiscountOfferDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "DiscountAccept",
            "discount_accept",
            RadrootsOrderDiscountDecisionDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "DiscountDecline",
            "discount_decline",
            RadrootsOrderDiscountDecisionDto::describe(ctx),
        ))
        .with_variant(newtype_variant(
            "Cancel",
            "cancel",
            RadrootsListingCancelDto::describe(ctx),
        ));
        register(
            ctx,
            "RadrootsCommercialMessagePayloadDto",
            TypeDef::Enum(def),
        )
    }
}

fn register(ctx: &mut DescribeCtx, rust_ident: &str, type_def: TypeDef) -> TypeRef {
    ctx.register_type(
        RustTypeId::new("radroots_events", "radroots_events", rust_ident),
        type_def,
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

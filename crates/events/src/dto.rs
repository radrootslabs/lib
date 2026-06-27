use dto_bindgen_core::RootDescriptor;

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
        RootDescriptor::new::<RadrootsCommercialDomain>(),
        RootDescriptor::new::<RadrootsOrderEventType>(),
        RootDescriptor::new::<RadrootsOrderEconomicActor>(),
        RootDescriptor::new::<RadrootsOrderEconomicEffect>(),
        RootDescriptor::new::<RadrootsOrderEconomicLineKind>(),
        RootDescriptor::new::<RadrootsOrderInventoryCommitment>(),
        RootDescriptor::new::<RadrootsListingParseError>(),
        RootDescriptor::new::<RadrootsTradeValidationListingRequest>(),
        RootDescriptor::new::<RadrootsTradeValidationListingResult>(),
        RootDescriptor::new::<RadrootsTradeValidationListingError>(),
        RootDescriptor::new::<RadrootsOrderCancellation>(),
        RootDescriptor::new::<RadrootsOrderDecisionOutcome>(),
        RootDescriptor::new::<RadrootsOrderDecision>(),
        RootDescriptor::new::<RadrootsOrderEconomicItem>(),
        RootDescriptor::new::<RadrootsOrderEconomicLine>(),
        RootDescriptor::new::<RadrootsOrderEconomicTotals>(),
        RootDescriptor::new::<RadrootsOrderEconomics>(),
        RootDescriptor::new::<RadrootsOrderItem>(),
        RootDescriptor::new::<RadrootsOrderRequest>(),
        RootDescriptor::new::<RadrootsOrderRevisionOutcome>(),
        RootDescriptor::new::<RadrootsOrderRevisionDecision>(),
        RootDescriptor::new::<RadrootsOrderRevisionProposal>(),
        RootDescriptor::new::<RadrootsOrderPricingBasis>(),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

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

    #[test]
    fn order_descriptor_roots_are_source_owned() {
        let registry = build_registry(dto_roots());
        let export_names = registry_export_names(&registry);
        let rust_names = registry_rust_names(&registry);

        for obsolete_export in [
            "RadrootsOrderEnvelope",
            "RadrootsCommercialEnvelope",
            "RadrootsCommercialMessagePayload",
            "RadrootsCommercialMessageType",
            "RadrootsCommercialTransportLane",
            "RadrootsOrderStatus",
            "RadrootsOrderQuestion",
            "RadrootsOrderAnswer",
            "RadrootsOrderDiscountRequest",
            "RadrootsOrderDiscountOffer",
            "RadrootsOrderDiscountDecision",
            "RadrootsListingCancel",
            "RadrootsOrderChange",
            "RadrootsOrderResponse",
            "RadrootsOrderRevision",
            "RadrootsOrderRevisionResponse",
        ] {
            assert!(
                !export_names.contains(obsolete_export),
                "{obsolete_export} should not remain as a binding-only descriptor root"
            );
        }

        for source_root in [
            "RadrootsCommercialDomain",
            "RadrootsOrderEventType",
            "RadrootsOrderRequest",
            "RadrootsOrderDecision",
            "RadrootsOrderDecisionOutcome",
            "RadrootsOrderRevisionProposal",
            "RadrootsOrderRevisionDecision",
            "RadrootsOrderRevisionOutcome",
            "RadrootsOrderCancellation",
            "RadrootsListingParseError",
        ] {
            assert!(
                rust_names.contains(source_root),
                "{source_root} should be registered from source"
            );
        }
    }

    fn registry_export_names(registry: &dto_bindgen_core::Registry) -> BTreeSet<&str> {
        registry
            .types_by_id
            .values()
            .map(|def| match def {
                TypeDef::Struct(def) => def.export_name.as_str(),
                TypeDef::Enum(def) => def.export_name.as_str(),
            })
            .collect()
    }

    fn registry_rust_names(registry: &dto_bindgen_core::Registry) -> BTreeSet<&str> {
        registry
            .types_by_id
            .values()
            .map(|def| match def {
                TypeDef::Struct(def) => def.rust_name.as_str(),
                TypeDef::Enum(def) => def.rust_name.as_str(),
            })
            .collect()
    }
}

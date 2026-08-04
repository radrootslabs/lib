#![allow(dead_code, unused_imports)]

use crate::envelope::EventEnvelope;

#[derive(dto_bindgen::Dto)]
#[dto(export)]
pub struct Nip01EventWireDto {
    pub id: String,
    pub pubkey: String,
    #[dto(int = "json_number")]
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
    pub extra: Nip01EventExtraDto,
}

#[derive(dto_bindgen::Dto)]
#[dto(export)]
#[dto(as = "string_enum")]
pub enum SignedEventVerificationStateDto {
    #[dto(rename = "id_verified")]
    IdVerified,
}

#[derive(dto_bindgen::Dto)]
#[dto(export)]
#[dto(as = "string_enum")]
pub enum VerifiedSignedEventVerificationStateDto {
    #[dto(rename = "signature_verified")]
    SignatureVerified,
}

#[derive(dto_bindgen::Dto)]
#[dto(export)]
pub struct SignedEventDto {
    pub state: SignedEventVerificationStateDto,
    pub envelope: EventEnvelope,
    pub wire: Nip01EventWireDto,
    pub raw_json: String,
}

#[derive(dto_bindgen::Dto)]
#[dto(export)]
pub struct VerifiedSignedEventDto {
    pub state: VerifiedSignedEventVerificationStateDto,
    pub signed_event: SignedEventDto,
}

pub struct Nip01EventExtraDto;

impl dto_bindgen::Dto for Nip01EventExtraDto {
    fn describe(_ctx: &mut dto_bindgen::__private::DescribeCtx) -> dto_bindgen::__private::TypeRef {
        dto_bindgen::__private::TypeRef::Override(dto_bindgen::__private::TargetOverride::new(
            dto_bindgen::__private::BackendId::TypeScript,
            "{ [key: string]: unknown }",
        ))
    }
}

#[path = "generated/dto_roots.rs"]
mod generated_roots;

pub use generated_roots::dto_bindgen_roots as dto_roots;

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::collections::BTreeSet;

    use dto_bindgen::export::{Registry, build_registry};

    use super::dto_roots;

    #[test]
    fn event_descriptor_roots_build_registry() {
        let registry = build_registry(dto_roots());

        assert!(!registry.has_errors());
        assert_eq!(registry.roots.len(), dto_roots().len());
        let export_names = registry_export_names(&registry);

        assert!(export_names.contains("EventEnvelope"));
        assert!(export_names.contains("Nip01EventWireDto"));
        assert!(export_names.contains("SignedEventDto"));
        assert!(export_names.contains("VerifiedSignedEventDto"));
        assert!(export_names.contains("OperationalListingImageSize"));
    }

    #[test]
    fn option_fields_are_optional_nullable() {
        let registry = build_registry(dto_roots());

        let summary = registry
            .struct_field_presence("OperationalListingProduct", "summary")
            .expect("summary field exists");
        assert!(!summary.required_on_deserialize);
        assert!(summary.nullable);

        let d_tag = registry
            .struct_field_presence("EventRef", "d_tag")
            .expect("d_tag field exists");
        assert!(!d_tag.required_on_deserialize);
        assert!(d_tag.nullable);
    }

    #[test]
    fn order_descriptor_roots_are_source_owned() {
        let registry = build_registry(dto_roots());
        let export_names = registry_export_names(&registry);
        let rust_names = registry_rust_names(&registry);

        for obsolete_export in [
            "OrderEnvelope",
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
            "RadrootsOperationalListingCancel",
            "RadrootsOrderChange",
            "RadrootsOrderResponse",
            "RadrootsOrderRevision",
            "RadrootsOrderRevisionResponse",
            "RadrootsOrderRevisionProposal",
            "RadrootsOrderRevisionDecision",
            "RadrootsOrderRevisionOutcome",
            "RadrootsTradeValidationListingRequest",
            "RadrootsTradeValidationListingResult",
        ] {
            assert!(
                !export_names.contains(obsolete_export),
                "{obsolete_export} should not remain as a binding-only descriptor root"
            );
        }

        for source_root in [
            "CommercialDomain",
            "OrderEventType",
            "OrderRequest",
            "OrderDecision",
            "OrderDecisionOutcome",
            "OrderCancellation",
            "OperationalListingParseError",
        ] {
            assert!(
                rust_names.contains(source_root),
                "{source_root} should be registered from source"
            );
        }
    }

    fn registry_export_names(registry: &Registry) -> BTreeSet<&str> {
        registry.type_export_names().collect()
    }

    fn registry_rust_names(registry: &Registry) -> BTreeSet<&str> {
        registry.type_rust_names().collect()
    }
}

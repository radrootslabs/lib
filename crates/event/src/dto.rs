use crate::RadrootsEventEnvelope;

#[derive(dto_bindgen::Dto)]
#[dto(export)]
pub struct RadrootsNip01EventWireDto {
    pub id: String,
    pub pubkey: String,
    #[dto(int = "json_number")]
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
    pub extra: RadrootsNip01EventExtraDto,
}

#[derive(dto_bindgen::Dto)]
#[dto(export)]
#[dto(as = "string_enum")]
pub enum RadrootsSignedEventVerificationStateDto {
    #[dto(rename = "id_verified")]
    IdVerified,
}

#[derive(dto_bindgen::Dto)]
#[dto(export)]
#[dto(as = "string_enum")]
pub enum RadrootsVerifiedSignedEventVerificationStateDto {
    #[dto(rename = "signature_verified")]
    SignatureVerified,
}

#[derive(dto_bindgen::Dto)]
#[dto(export)]
pub struct RadrootsSignedEventDto {
    pub state: RadrootsSignedEventVerificationStateDto,
    pub envelope: RadrootsEventEnvelope,
    pub wire: RadrootsNip01EventWireDto,
    pub raw_json: String,
}

#[derive(dto_bindgen::Dto)]
#[dto(export)]
pub struct RadrootsVerifiedSignedEventDto {
    pub state: RadrootsVerifiedSignedEventVerificationStateDto,
    pub signed_event: RadrootsSignedEventDto,
}

pub struct RadrootsNip01EventExtraDto;

impl dto_bindgen::Dto for RadrootsNip01EventExtraDto {
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

        assert!(export_names.contains("RadrootsEventEnvelope"));
        assert!(export_names.contains("RadrootsNip01EventWireDto"));
        assert!(export_names.contains("RadrootsSignedEventDto"));
        assert!(export_names.contains("RadrootsVerifiedSignedEventDto"));
        assert!(export_names.contains("RadrootsListingImageSize"));
    }

    #[test]
    fn option_fields_are_optional_nullable() {
        let registry = build_registry(dto_roots());

        let summary = registry
            .struct_field_presence("RadrootsListingProduct", "summary")
            .expect("summary field exists");
        assert!(!summary.required_on_deserialize);
        assert!(summary.nullable);

        let d_tag = registry
            .struct_field_presence("RadrootsEventRef", "d_tag")
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

    fn registry_export_names(registry: &Registry) -> BTreeSet<&str> {
        registry.type_export_names().collect()
    }

    fn registry_rust_names(registry: &Registry) -> BTreeSet<&str> {
        registry.type_rust_names().collect()
    }
}

#[path = "generated/dto_roots.rs"]
mod generated_roots;

pub use generated_roots::dto_bindgen_roots as dto_roots;

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

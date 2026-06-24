use dto_bindgen_core::RootDescriptor;

use crate::listing::{
    model::{RadrootsTradeListingSubtotal, RadrootsTradeListingTotal},
    validation::RadrootsTradeListing,
};

pub fn dto_roots() -> [RootDescriptor; 3] {
    [
        RootDescriptor::new::<RadrootsTradeListing>(),
        RootDescriptor::new::<RadrootsTradeListingSubtotal>(),
        RootDescriptor::new::<RadrootsTradeListingTotal>(),
    ]
}

#[cfg(test)]
mod tests {
    use dto_bindgen_core::{Registry, TypeDef, build_registry};

    use super::dto_roots;

    const TRADE_SOURCE_ROOTS: &[&str] = &[
        "RadrootsTradeListing",
        "RadrootsTradeListingSubtotal",
        "RadrootsTradeListingTotal",
    ];
    const TRADE_IMPORTED_SOURCE_DEPENDENCIES: &[&str] = &[
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
        "RadrootsFarmRef",
        "RadrootsListing",
        "RadrootsListingAvailability",
        "RadrootsListingBin",
        "RadrootsListingDeliveryMethod",
        "RadrootsListingImage",
        "RadrootsListingImageSize",
        "RadrootsListingLocation",
        "RadrootsListingProduct",
        "RadrootsListingStatus",
        "RadrootsPlotRef",
        "RadrootsResourceAreaRef",
    ];

    #[test]
    fn trade_descriptor_roots_build_registry() {
        let registry = build_registry(dto_roots());

        assert!(
            !registry.has_errors(),
            "trade registry has diagnostics: {:?}",
            registry.diagnostics
        );
        assert_eq!(registry.roots.len(), dto_roots().len());
    }

    #[test]
    fn trade_source_roots_are_deterministic() {
        let registry = build_registry(dto_roots());

        assert_eq!(root_export_names(&registry), TRADE_SOURCE_ROOTS);
    }

    #[test]
    fn trade_source_dependencies_are_explicit() {
        let registry = build_registry(dto_roots());
        let mut dependencies = type_export_names(&registry)
            .into_iter()
            .filter(|name| !TRADE_SOURCE_ROOTS.contains(name))
            .collect::<Vec<_>>();
        dependencies.sort();

        assert_eq!(dependencies, TRADE_IMPORTED_SOURCE_DEPENDENCIES);
    }

    fn root_export_names(registry: &Registry) -> Vec<&str> {
        registry
            .roots
            .iter()
            .map(|type_id| {
                registry
                    .type_def(*type_id)
                    .map(type_export_name)
                    .expect("root type")
            })
            .collect()
    }

    fn type_export_names(registry: &Registry) -> Vec<&str> {
        registry
            .types_by_id
            .values()
            .map(type_export_name)
            .collect::<Vec<_>>()
    }

    fn type_export_name(def: &TypeDef) -> &str {
        match def {
            TypeDef::Struct(def) => def.export_name.as_str(),
            TypeDef::Enum(def) => def.export_name.as_str(),
        }
    }
}

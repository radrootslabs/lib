use dto_bindgen_core::{
    BackendId, DescribeCtx, Dto, FieldDef, IdentName, RootDescriptor, RustTypeId, SourceSpan,
    StructDef, TargetFieldNames, TargetOverride, TypeDef, TypeRef, WireFieldNames,
};

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

impl Dto for RadrootsTradeListing {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsTradeListing",
            "RadrootsTradeListing",
            span("crates/trade/src/listing/validation.rs", 25),
        )
        .with_field(field(
            "listing_id",
            "listing_id",
            String::describe(ctx),
            "crates/trade/src/listing/validation.rs",
            26,
        ))
        .with_field(field(
            "listing_addr",
            "listing_addr",
            String::describe(ctx),
            "crates/trade/src/listing/validation.rs",
            27,
        ))
        .with_field(field(
            "seller_pubkey",
            "seller_pubkey",
            String::describe(ctx),
            "crates/trade/src/listing/validation.rs",
            28,
        ))
        .with_field(field(
            "title",
            "title",
            String::describe(ctx),
            "crates/trade/src/listing/validation.rs",
            29,
        ))
        .with_field(field(
            "description",
            "description",
            String::describe(ctx),
            "crates/trade/src/listing/validation.rs",
            30,
        ))
        .with_field(field(
            "product_type",
            "product_type",
            String::describe(ctx),
            "crates/trade/src/listing/validation.rs",
            31,
        ))
        .with_field(field(
            "primary_bin_id",
            "primary_bin_id",
            String::describe(ctx),
            "crates/trade/src/listing/validation.rs",
            32,
        ))
        .with_field(field(
            "bin_quantity",
            "bin_quantity",
            ts_ref("RadrootsCoreQuantity"),
            "crates/trade/src/listing/validation.rs",
            33,
        ))
        .with_field(field(
            "unit",
            "unit",
            ts_ref("RadrootsCoreUnit"),
            "crates/trade/src/listing/validation.rs",
            34,
        ))
        .with_field(field(
            "unit_price",
            "unit_price",
            ts_ref("RadrootsCoreMoney"),
            "crates/trade/src/listing/validation.rs",
            35,
        ))
        .with_field(field(
            "inventory_available",
            "inventory_available",
            ts_ref("RadrootsCoreDecimal"),
            "crates/trade/src/listing/validation.rs",
            36,
        ))
        .with_field(field(
            "availability",
            "availability",
            ts_ref("RadrootsListingAvailability"),
            "crates/trade/src/listing/validation.rs",
            37,
        ))
        .with_field(field(
            "location",
            "location",
            ts_ref("RadrootsListingPublicLocation"),
            "crates/trade/src/listing/validation.rs",
            38,
        ))
        .with_field(field(
            "delivery_method",
            "delivery_method",
            ts_ref("RadrootsListingDeliveryMethod"),
            "crates/trade/src/listing/validation.rs",
            39,
        ))
        .with_field(field(
            "listing",
            "listing",
            ts_ref("RadrootsListing"),
            "crates/trade/src/listing/validation.rs",
            40,
        ));
        register(ctx, "RadrootsTradeListing", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsTradeListingSubtotal {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        trade_listing_total_like(
            ctx,
            "RadrootsTradeListingSubtotal",
            "crates/trade/src/listing/model.rs",
            3,
        )
    }
}

impl Dto for RadrootsTradeListingTotal {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        trade_listing_total_like(
            ctx,
            "RadrootsTradeListingTotal",
            "crates/trade/src/listing/model.rs",
            12,
        )
    }
}

fn trade_listing_total_like(
    ctx: &mut DescribeCtx,
    rust_ident: &str,
    file: &str,
    line: u32,
) -> TypeRef {
    let def = StructDef::new(rust_ident, rust_ident, span(file, line))
        .with_field(field(
            "price_amount",
            "price_amount",
            ts_ref("RadrootsCoreMoney"),
            file,
            line + 1,
        ))
        .with_field(field(
            "price_currency",
            "price_currency",
            ts_ref("RadrootsCoreCurrency"),
            file,
            line + 2,
        ))
        .with_field(field(
            "quantity_amount",
            "quantity_amount",
            ts_ref("RadrootsCoreDecimal"),
            file,
            line + 3,
        ))
        .with_field(field(
            "quantity_unit",
            "quantity_unit",
            ts_ref("RadrootsCoreUnit"),
            file,
            line + 4,
        ));
    register(ctx, rust_ident, TypeDef::Struct(def))
}

fn register(ctx: &mut DescribeCtx, rust_ident: &str, type_def: TypeDef) -> TypeRef {
    ctx.register_type(RustTypeId::new("radroots_trade", rust_ident), type_def)
}

fn ts_ref(target_type: &str) -> TypeRef {
    TypeRef::Override(TargetOverride::new(BackendId::TypeScript, target_type))
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
    use dto_bindgen_core::{BackendId, Registry, StructDef, TypeDef, TypeRef, build_registry};

    use super::dto_roots;

    const TRADE_SOURCE_ROOTS: &[&str] = &[
        "RadrootsTradeListing",
        "RadrootsTradeListingSubtotal",
        "RadrootsTradeListingTotal",
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
    fn trade_source_fields_use_package_aliases_for_import_boundaries() {
        let registry = build_registry(dto_roots());
        let listing = find_struct(&registry, "RadrootsTradeListing");
        let subtotal = find_struct(&registry, "RadrootsTradeListingSubtotal");

        assert_eq!(
            typescript_override_target(field_ty(listing, "inventory_available")),
            Some("RadrootsCoreDecimal")
        );
        assert_eq!(
            typescript_override_target(field_ty(listing, "listing")),
            Some("RadrootsListing")
        );
        assert_eq!(
            typescript_override_target(field_ty(subtotal, "price_currency")),
            Some("RadrootsCoreCurrency")
        );
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

    fn type_export_name(def: &TypeDef) -> &str {
        match def {
            TypeDef::Struct(def) => def.export_name.as_str(),
            TypeDef::Enum(def) => def.export_name.as_str(),
        }
    }

    fn find_struct<'a>(registry: &'a Registry, export_name: &str) -> &'a StructDef {
        registry
            .types_by_id
            .values()
            .find_map(|def| match def {
                TypeDef::Struct(def) if def.export_name == export_name => Some(def),
                _ => None,
            })
            .expect("descriptor struct")
    }

    fn field_ty<'a>(def: &'a StructDef, field_name: &str) -> &'a TypeRef {
        &def.fields
            .iter()
            .find(|field| field.target.typescript == field_name)
            .expect("descriptor field")
            .ty
    }

    fn typescript_override_target(ty: &TypeRef) -> Option<&str> {
        match ty {
            TypeRef::Override(target) if target.backend == BackendId::TypeScript => {
                Some(target.target_type.as_str())
            }
            _ => None,
        }
    }
}

use dto_bindgen_core::{
    DescribeCtx, Dto, EnumDef, EnumRepr, FieldDef, FieldPresence, IdentName, RootDescriptor,
    RustTypeId, SourceSpan, StructDef, TargetFieldNames, TypeDef, TypeRef, VariantDef,
    VariantShape, WireFieldNames,
};

use crate::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreDiscount, RadrootsCoreDiscountScope,
    RadrootsCoreDiscountThreshold, RadrootsCoreDiscountValue, RadrootsCoreMoney,
    RadrootsCorePercent, RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    RadrootsCoreUnitDimension,
};

pub fn dto_roots() -> [RootDescriptor; 9] {
    [
        RootDescriptor::new::<RadrootsCoreDiscount>(),
        RootDescriptor::new::<RadrootsCoreDiscountScope>(),
        RootDescriptor::new::<RadrootsCoreDiscountThreshold>(),
        RootDescriptor::new::<RadrootsCoreDiscountValue>(),
        RootDescriptor::new::<RadrootsCoreMoney>(),
        RootDescriptor::new::<RadrootsCorePercent>(),
        RootDescriptor::new::<RadrootsCoreQuantity>(),
        RootDescriptor::new::<RadrootsCoreQuantityPrice>(),
        RootDescriptor::new::<RadrootsCoreUnit>(),
    ]
}

impl Dto for RadrootsCoreCurrency {
    fn describe(_ctx: &mut DescribeCtx) -> TypeRef {
        TypeRef::String
    }
}

impl Dto for RadrootsCoreDecimal {
    fn describe(_ctx: &mut DescribeCtx) -> TypeRef {
        TypeRef::String
    }
}

impl Dto for RadrootsCoreUnitDimension {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsCoreUnitDimension",
            "RadrootsCoreUnitDimension",
            EnumRepr::External,
            span("crates/core/src/unit.rs", 17),
        )
        .with_variant(unit_variant(
            "Count",
            "count",
            "crates/core/src/unit.rs",
            18,
        ))
        .with_variant(unit_variant("Mass", "mass", "crates/core/src/unit.rs", 19))
        .with_variant(unit_variant(
            "Volume",
            "volume",
            "crates/core/src/unit.rs",
            20,
        ));
        register(ctx, "RadrootsCoreUnitDimension", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsCoreUnit {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsCoreUnit",
            "RadrootsCoreUnit",
            EnumRepr::External,
            span("crates/core/src/unit.rs", 24),
        )
        .with_variant(unit_variant("Each", "each", "crates/core/src/unit.rs", 25))
        .with_variant(unit_variant("MassKg", "kg", "crates/core/src/unit.rs", 26))
        .with_variant(unit_variant("MassG", "g", "crates/core/src/unit.rs", 27))
        .with_variant(unit_variant("MassOz", "oz", "crates/core/src/unit.rs", 28))
        .with_variant(unit_variant("MassLb", "lb", "crates/core/src/unit.rs", 29))
        .with_variant(unit_variant("VolumeL", "l", "crates/core/src/unit.rs", 30))
        .with_variant(unit_variant(
            "VolumeMl",
            "ml",
            "crates/core/src/unit.rs",
            31,
        ));
        register(ctx, "RadrootsCoreUnit", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsCoreMoney {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsCoreMoney",
            "RadrootsCoreMoney",
            span("crates/core/src/money.rs", 8),
        )
        .with_field(field(
            "amount",
            "amount",
            RadrootsCoreDecimal::describe(ctx),
            "crates/core/src/money.rs",
            9,
        ))
        .with_field(field(
            "currency",
            "currency",
            RadrootsCoreCurrency::describe(ctx),
            "crates/core/src/money.rs",
            10,
        ));
        register(ctx, "RadrootsCoreMoney", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsCorePercent {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsCorePercent",
            "RadrootsCorePercent",
            span("crates/core/src/percent.rs", 9),
        )
        .with_field(field(
            "value",
            "value",
            RadrootsCoreDecimal::describe(ctx),
            "crates/core/src/percent.rs",
            10,
        ));
        register(ctx, "RadrootsCorePercent", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsCoreQuantity {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let unit = RadrootsCoreUnit::describe(ctx);
        let def = StructDef::new(
            "RadrootsCoreQuantity",
            "RadrootsCoreQuantity",
            span("crates/core/src/quantity.rs", 13),
        )
        .with_field(field(
            "amount",
            "amount",
            RadrootsCoreDecimal::describe(ctx),
            "crates/core/src/quantity.rs",
            14,
        ))
        .with_field(field(
            "unit",
            "unit",
            unit,
            "crates/core/src/quantity.rs",
            16,
        ))
        .with_field(
            field(
                "label",
                "label",
                <Option<String> as Dto>::describe(ctx),
                "crates/core/src/quantity.rs",
                18,
            )
            .with_presence(FieldPresence::optional_nullable_skip_if_none()),
        );
        register(ctx, "RadrootsCoreQuantity", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsCoreQuantityPrice {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let amount = RadrootsCoreMoney::describe(ctx);
        let quantity = RadrootsCoreQuantity::describe(ctx);
        let def = StructDef::new(
            "RadrootsCoreQuantityPrice",
            "RadrootsCoreQuantityPrice",
            span("crates/core/src/quantity_price.rs", 5),
        )
        .with_field(field(
            "amount",
            "amount",
            amount,
            "crates/core/src/quantity_price.rs",
            7,
        ))
        .with_field(field(
            "quantity",
            "quantity",
            quantity,
            "crates/core/src/quantity_price.rs",
            9,
        ));
        register(ctx, "RadrootsCoreQuantityPrice", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsCoreDiscountScope {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = EnumDef::new(
            "RadrootsCoreDiscountScope",
            "RadrootsCoreDiscountScope",
            EnumRepr::External,
            span("crates/core/src/discount.rs", 9),
        )
        .with_variant(unit_variant(
            "Bin",
            "bin",
            "crates/core/src/discount.rs",
            10,
        ))
        .with_variant(unit_variant(
            "OrderTotal",
            "order_total",
            "crates/core/src/discount.rs",
            11,
        ));
        register(ctx, "RadrootsCoreDiscountScope", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsCoreDiscountThreshold {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let quantity = RadrootsCoreQuantity::describe(ctx);
        let def = EnumDef::new(
            "RadrootsCoreDiscountThreshold",
            "RadrootsCoreDiscountThreshold",
            EnumRepr::Adjacent {
                tag: "kind".to_owned(),
                content: "amount".to_owned(),
            },
            span("crates/core/src/discount.rs", 20),
        )
        .with_variant(VariantDef::new(
            "BinCount",
            "bin_count",
            VariantShape::Struct(vec![
                field(
                    "bin_id",
                    "bin_id",
                    String::describe(ctx),
                    "crates/core/src/discount.rs",
                    21,
                ),
                field(
                    "min",
                    "min",
                    u32::describe(ctx),
                    "crates/core/src/discount.rs",
                    21,
                ),
            ]),
            span("crates/core/src/discount.rs", 21),
        ))
        .with_variant(VariantDef::new(
            "OrderQuantity",
            "order_quantity",
            VariantShape::Struct(vec![field(
                "min",
                "min",
                quantity,
                "crates/core/src/discount.rs",
                22,
            )]),
            span("crates/core/src/discount.rs", 22),
        ));
        register(ctx, "RadrootsCoreDiscountThreshold", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsCoreDiscountValue {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let money = RadrootsCoreMoney::describe(ctx);
        let percent = RadrootsCorePercent::describe(ctx);
        let def = EnumDef::new(
            "RadrootsCoreDiscountValue",
            "RadrootsCoreDiscountValue",
            EnumRepr::Adjacent {
                tag: "kind".to_owned(),
                content: "amount".to_owned(),
            },
            span("crates/core/src/discount.rs", 31),
        )
        .with_variant(VariantDef::new(
            "MoneyPerBin",
            "money_per_bin",
            VariantShape::Newtype(money),
            span("crates/core/src/discount.rs", 32),
        ))
        .with_variant(VariantDef::new(
            "Percent",
            "percent",
            VariantShape::Newtype(percent),
            span("crates/core/src/discount.rs", 33),
        ));
        register(ctx, "RadrootsCoreDiscountValue", TypeDef::Enum(def))
    }
}

impl Dto for RadrootsCoreDiscount {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let scope = RadrootsCoreDiscountScope::describe(ctx);
        let threshold = RadrootsCoreDiscountThreshold::describe(ctx);
        let value = RadrootsCoreDiscountValue::describe(ctx);
        let def = StructDef::new(
            "RadrootsCoreDiscount",
            "RadrootsCoreDiscount",
            span("crates/core/src/discount.rs", 39),
        )
        .with_field(field(
            "scope",
            "scope",
            scope,
            "crates/core/src/discount.rs",
            40,
        ))
        .with_field(field(
            "threshold",
            "threshold",
            threshold,
            "crates/core/src/discount.rs",
            41,
        ))
        .with_field(field(
            "value",
            "value",
            value,
            "crates/core/src/discount.rs",
            42,
        ));
        register(ctx, "RadrootsCoreDiscount", TypeDef::Struct(def))
    }
}

fn register(ctx: &mut DescribeCtx, rust_ident: &str, type_def: TypeDef) -> TypeRef {
    ctx.register_type(RustTypeId::new("radroots_core", rust_ident), type_def)
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

#[cfg(test)]
mod tests {
    use dto_bindgen_core::{TypeDef, build_registry};

    use super::dto_roots;

    #[test]
    fn core_descriptor_roots_build_registry() {
        let registry = build_registry(dto_roots());

        assert!(!registry.has_errors());
        assert_eq!(registry.roots.len(), dto_roots().len());
        assert!(registry.types_by_id.values().any(
            |def| matches!(def, TypeDef::Struct(def) if def.export_name == "RadrootsCoreMoney")
        ));
        assert!(
            registry.types_by_id.values().any(
                |def| matches!(def, TypeDef::Enum(def) if def.export_name == "RadrootsCoreUnit")
            )
        );
    }
}

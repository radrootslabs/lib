#![allow(dead_code)]

use dto_bindgen_core::RootDescriptor;

pub fn dto_roots() -> Vec<RootDescriptor> {
    vec![
        RootDescriptor::new::<DiscountDescriptor>(),
        RootDescriptor::new::<DiscountScopeDescriptor>(),
        RootDescriptor::new::<DiscountThresholdDescriptor>(),
        RootDescriptor::new::<DiscountValueDescriptor>(),
        RootDescriptor::new::<MoneyDescriptor>(),
        RootDescriptor::new::<PercentDescriptor>(),
        RootDescriptor::new::<QuantityDescriptor>(),
        RootDescriptor::new::<QuantityPriceDescriptor>(),
        RootDescriptor::new::<UnitDescriptor>(),
        RootDescriptor::new::<UnitDimensionDescriptor>(),
    ]
}

#[derive(dto_bindgen::Dto)]
#[dto(as = "string")]
#[dto(ts(name = "Currency"))]
struct CurrencyDescriptor(String);

#[derive(dto_bindgen::Dto)]
#[dto(as = "string")]
#[dto(ts(name = "Decimal"))]
struct DecimalDescriptor(String);

#[derive(dto_bindgen::Dto)]
#[dto(ts(name = "Money"))]
struct MoneyDescriptor {
    amount: DecimalDescriptor,
    currency: CurrencyDescriptor,
}

#[derive(dto_bindgen::Dto)]
#[dto(ts(name = "Percent"))]
struct PercentDescriptor {
    value: DecimalDescriptor,
}

#[derive(dto_bindgen::Dto)]
#[dto(as = "string_enum")]
#[dto(ts(name = "Unit"))]
enum UnitDescriptor {
    #[dto(rename = "each")]
    Each,
    #[dto(rename = "kg")]
    MassKg,
    #[dto(rename = "g")]
    MassG,
    #[dto(rename = "oz")]
    MassOz,
    #[dto(rename = "lb")]
    MassLb,
    #[dto(rename = "l")]
    VolumeL,
    #[dto(rename = "ml")]
    VolumeMl,
}

#[derive(dto_bindgen::Dto)]
#[dto(as = "string_enum")]
#[dto(ts(name = "UnitDimension"))]
enum UnitDimensionDescriptor {
    #[dto(rename = "count")]
    Count,
    #[dto(rename = "mass")]
    Mass,
    #[dto(rename = "volume")]
    Volume,
}

#[derive(dto_bindgen::Dto)]
#[dto(ts(name = "Quantity"))]
struct QuantityDescriptor {
    amount: DecimalDescriptor,
    unit: UnitDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

#[derive(dto_bindgen::Dto)]
#[dto(ts(name = "QuantityPrice"))]
struct QuantityPriceDescriptor {
    amount: MoneyDescriptor,
    quantity: QuantityDescriptor,
}

#[derive(dto_bindgen::Dto)]
#[dto(as = "string_enum")]
#[dto(ts(name = "DiscountScope"))]
enum DiscountScopeDescriptor {
    #[dto(rename = "bin")]
    Bin,
    #[dto(rename = "order_total")]
    OrderTotal,
}

#[derive(dto_bindgen::Dto)]
#[dto(ts(name = "DiscountThreshold"))]
#[serde(rename_all = "snake_case", tag = "kind", content = "amount")]
enum DiscountThresholdDescriptor {
    BinCount { bin_id: String, min: u32 },
    OrderQuantity { min: QuantityDescriptor },
}

#[derive(dto_bindgen::Dto)]
#[dto(ts(name = "DiscountValue"))]
#[serde(rename_all = "snake_case", tag = "kind", content = "amount")]
enum DiscountValueDescriptor {
    MoneyPerBin(MoneyDescriptor),
    Percent(PercentDescriptor),
}

#[derive(dto_bindgen::Dto)]
#[dto(ts(name = "Discount"))]
struct DiscountDescriptor {
    scope: DiscountScopeDescriptor,
    threshold: DiscountThresholdDescriptor,
    value: DiscountValueDescriptor,
}

#[cfg(test)]
mod tests {
    use dto_bindgen_core::build_registry;

    use super::dto_roots;

    #[test]
    fn canonical_core_descriptor_roots_build_without_diagnostics() {
        let roots = dto_roots();
        let registry = build_registry(roots.clone());

        assert!(!registry.has_errors(), "{:?}", registry.diagnostics);
        assert_eq!(registry.roots.len(), roots.len());
    }
}

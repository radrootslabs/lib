use radroots_domain::{
    Decimal, ListingOptionId, PoolId, PositiveDecimal, PublicKey, Quantity, QuantityError,
    Sequence,
    listing::{ListingCoordinate, ListingId, MeasurementUnit},
    stock::{MappingError, OptionMappingKey, OptionPoolMapping, PoolUnit, StockError},
};
fn key() -> OptionMappingKey {
    OptionMappingKey::new(
        ListingCoordinate::new(
            PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
                .unwrap(),
            ListingId::from_bytes([1; 16]),
        ),
        ListingOptionId::from_bytes([2; 32]),
    )
}
fn quantity(amount: &str, unit: MeasurementUnit) -> Quantity {
    Quantity::new(amount.parse::<Decimal>().unwrap(), unit).unwrap()
}
fn mapping(
    amount: &str,
    unit: MeasurementUnit,
    pool_unit: PoolUnit,
    version: u64,
) -> Result<OptionPoolMapping, MappingError> {
    OptionPoolMapping::new(
        key(),
        PoolId::from_bytes([3; 32]),
        pool_unit,
        quantity(amount, unit),
        Sequence::new(version).unwrap(),
    )
}
#[test]
fn exact_mapping_preserves_the_declared_si_basis_and_multiplies_without_tick_rounding() {
    let value = mapping(
        "250",
        MeasurementUnit::Gram,
        PoolUnit::new(MeasurementUnit::Kilogram, 3).unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(value.key(), key());
    assert_eq!(value.pool_id(), PoolId::from_bytes([3; 32]));
    assert_eq!(
        value.per_sale_unit(),
        quantity("250", MeasurementUnit::Gram)
    );
    assert_eq!(value.per_sale_ticks().get(), 250);
    assert_eq!(
        value
            .draw_for_sales("2".parse::<PositiveDecimal>().unwrap())
            .unwrap()
            .get(),
        500
    );
    assert_eq!(
        value.draw_for_sales("1.5".parse().unwrap()).unwrap().get(),
        375
    );
    let volume = mapping(
        "250",
        MeasurementUnit::Millilitre,
        PoolUnit::new(MeasurementUnit::Litre, 3).unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(volume.per_sale_ticks().get(), 250);
    let one_tick = mapping(
        "1",
        MeasurementUnit::Each,
        PoolUnit::new(MeasurementUnit::Each, 0).unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(
        one_tick.draw_for_sales("0.5".parse().unwrap()),
        Err(MappingError::Stock(StockError::PrecisionLoss))
    );
    assert_eq!(
        format!("{value:?}"),
        "OptionPoolMapping(<private metadata>)"
    );
}
#[test]
fn zero_absent_versions_unjustified_dimensions_precision_loss_and_capacity_are_refused() {
    let kg = PoolUnit::new(MeasurementUnit::Kilogram, 3).unwrap();
    assert_eq!(
        mapping("0", MeasurementUnit::Gram, kg, 1),
        Err(MappingError::ZeroDraw)
    );
    assert_eq!(
        mapping("250", MeasurementUnit::Gram, kg, 0),
        Err(MappingError::InvalidVersion)
    );
    for unit in [
        MeasurementUnit::Litre,
        MeasurementUnit::Millilitre,
        MeasurementUnit::Each,
    ] {
        assert_eq!(
            mapping("1", unit, kg, 1),
            Err(MappingError::Quantity(QuantityError::IncompatibleDimension))
        );
    }
    assert_eq!(
        mapping(
            "1",
            MeasurementUnit::Gram,
            PoolUnit::new(MeasurementUnit::Kilogram, 0).unwrap(),
            1
        ),
        Err(MappingError::Stock(StockError::PrecisionLoss))
    );
    assert_eq!(
        mapping("0.000001", MeasurementUnit::Gram, kg, 1),
        Err(MappingError::Quantity(QuantityError::PrecisionLoss))
    );
    let large = mapping(
        "999999999999",
        MeasurementUnit::Kilogram,
        PoolUnit::new(MeasurementUnit::Kilogram, 6).unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(
        large.draw_for_sales("2".parse().unwrap()),
        Err(MappingError::Stock(StockError::OutOfRange))
    );
    assert_eq!(
        large.draw_for_sales("999999999999".parse().unwrap()),
        Err(MappingError::Stock(StockError::OutOfRange))
    );
    assert_eq!(
        large
            .draw_for_sales("0.000001".parse().unwrap())
            .unwrap()
            .get(),
        999999999999
    );
}
#[cfg(feature = "serde")]
#[test]
fn closed_mapping_codecs_recompute_derived_ticks_and_refuse_coordinate_or_value_aliases() {
    use serde_json::json;
    let value = mapping(
        "250",
        MeasurementUnit::Gram,
        PoolUnit::new(MeasurementUnit::Kilogram, 3).unwrap(),
        1,
    )
    .unwrap();
    let wire = serde_json::to_value(value).unwrap();
    assert_eq!(wire["per_sale_ticks"], json!("250"));
    assert_eq!(
        serde_json::from_value::<OptionPoolMapping>(wire.clone()).unwrap(),
        value
    );
    for (path, bad) in [
        ("/per_sale_ticks", json!("251")),
        ("/per_sale_ticks", json!(250)),
        ("/version", json!("0")),
        ("/version", json!(1)),
        ("/per_sale_unit/amount", json!("0")),
        ("/pool_unit/scale", json!("2")),
        ("/listing/kind", json!(1)),
        ("/listing/kind", json!(30402.0)),
        ("/listing/kind", json!("30402")),
        ("/listing", json!([30402, "00".repeat(32), "00".repeat(16)])),
        ("/pool_unit", json!(["kg", "3"])),
    ] {
        let mut changed = wire.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        assert!(
            serde_json::from_value::<OptionPoolMapping>(changed).is_err(),
            "{path}"
        );
    }
    for field in [
        "listing",
        "option_id",
        "pool_id",
        "pool_unit",
        "per_sale_unit",
        "per_sale_ticks",
        "version",
    ] {
        let mut changed = wire.clone();
        changed.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<OptionPoolMapping>(changed).is_err(),
            "{field}"
        );
    }
    let mut changed = wire.clone();
    changed["available"] = json!("999");
    assert!(serde_json::from_value::<OptionPoolMapping>(changed).is_err());
    assert!(serde_json::from_value::<OptionPoolMapping>(json!([wire])).is_err());
}

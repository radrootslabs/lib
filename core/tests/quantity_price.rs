mod common;

use radroots_core::{
    RadrootsCoreQuantityPrice, RadrootsCoreQuantityPriceError, RadrootsCoreQuantityPriceOps,
    RadrootsCoreUnit,
};

#[test]
fn cost_for_scales_by_ratio() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("10", "USD"),
        common::qty("1", RadrootsCoreUnit::MassKg),
    );
    let cost = price.cost_for(&common::qty("2", RadrootsCoreUnit::MassKg));
    assert_eq!(cost.amount, common::dec("20"));
}

#[test]
fn cost_for_returns_zero_on_unit_mismatch() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("10", "USD"),
        common::qty("1", RadrootsCoreUnit::MassKg),
    );
    let cost = price.cost_for(&common::qty("1", RadrootsCoreUnit::Each));
    assert!(cost.amount.is_zero());
}

#[test]
fn cost_for_rounded_and_quantized_price_differ() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("1.005", "USD"),
        common::qty("1", RadrootsCoreUnit::Each),
    );
    let qty = common::qty("2", RadrootsCoreUnit::Each);
    let rounded = price.cost_for_rounded(&qty);
    let quantized = price.cost_for_with_quantized_price(&qty);

    assert_eq!(rounded.amount, common::dec("2.01"));
    assert_eq!(quantized.amount, common::dec("2.02"));
}

#[test]
fn try_cost_for_validates_quantity_and_units() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("10", "USD"),
        common::qty("1", RadrootsCoreUnit::Each),
    );
    let zero_price = RadrootsCoreQuantityPrice::new(
        common::money("10", "USD"),
        common::qty("0", RadrootsCoreUnit::Each),
    );

    assert_eq!(
        zero_price.try_cost_for(&common::qty("1", RadrootsCoreUnit::Each)),
        Err(RadrootsCoreQuantityPriceError::PerQuantityZero)
    );
    assert_eq!(
        price.try_cost_for(&common::qty("1", RadrootsCoreUnit::MassKg)),
        Err(RadrootsCoreQuantityPriceError::UnitMismatch {
            have: RadrootsCoreUnit::MassKg,
            want: RadrootsCoreUnit::Each
        })
    );
}

#[test]
fn try_cost_for_amount_in_converts_mass_units() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("10", "USD"),
        common::qty("1", RadrootsCoreUnit::MassKg),
    );
    let cost = price
        .try_cost_for_amount_in(common::dec("500"), RadrootsCoreUnit::MassG)
        .unwrap();
    assert_eq!(cost.amount, common::dec("5"));
}

#[test]
fn try_cost_for_amount_in_converts_volume_units() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("10", "USD"),
        common::qty("1", RadrootsCoreUnit::VolumeL),
    );
    let cost = price
        .try_cost_for_amount_in(common::dec("500"), RadrootsCoreUnit::VolumeMl)
        .unwrap();
    assert_eq!(cost.amount, common::dec("5"));
}

#[test]
fn try_cost_for_amount_in_rejects_non_convertible_units() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("10", "USD"),
        common::qty("1", RadrootsCoreUnit::MassKg),
    );
    assert_eq!(
        price.try_cost_for_amount_in(common::dec("1"), RadrootsCoreUnit::Each),
        Err(RadrootsCoreQuantityPriceError::NonConvertibleUnits {
            from: RadrootsCoreUnit::Each,
            to: RadrootsCoreUnit::MassKg
        })
    );
}

#[test]
fn try_to_canonical_unit_price_converts_units() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("6.99", "USD"),
        common::qty("1", RadrootsCoreUnit::MassLb),
    );
    let canonical = price.try_to_canonical_unit_price().unwrap();
    assert_eq!(canonical.quantity.unit, RadrootsCoreUnit::MassG);
    assert_eq!(canonical.quantity.amount, common::dec("1"));
    let expected = common::dec("6.99") / common::dec("453.59237");
    assert_eq!(canonical.amount.amount, expected);
}

#[test]
fn is_price_per_canonical_unit_detects_canonical() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("1.00", "USD"),
        common::qty("1", RadrootsCoreUnit::MassG),
    );
    assert!(price.is_price_per_canonical_unit());

    let price = RadrootsCoreQuantityPrice::new(
        common::money("1.00", "USD"),
        common::qty("1", RadrootsCoreUnit::MassKg),
    );
    assert!(!price.is_price_per_canonical_unit());
}

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
fn try_cost_for_amount_in_same_unit_path_is_exercised() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("4", "USD"),
        common::qty("1", RadrootsCoreUnit::Each),
    );
    let out = price
        .try_cost_for_amount_in(common::dec("3"), RadrootsCoreUnit::Each)
        .unwrap();
    assert_eq!(out.amount, common::dec("12"));
}

#[test]
fn try_cost_for_quantity_in_path_is_exercised() {
    let price = RadrootsCoreQuantityPrice::new(
        common::money("10", "USD"),
        common::qty("1", RadrootsCoreUnit::MassKg),
    );
    let qty = common::qty("250", RadrootsCoreUnit::MassG);
    let out = price.try_cost_for_quantity_in(&qty).unwrap();
    assert_eq!(out.amount, common::dec("2.5"));
}

#[test]
fn try_to_unit_price_error_and_same_unit_paths_are_exercised() {
    let zero = RadrootsCoreQuantityPrice::new(
        common::money("10", "USD"),
        common::qty("0", RadrootsCoreUnit::MassKg),
    );
    assert_eq!(
        zero.try_to_unit_price(RadrootsCoreUnit::MassG),
        Err(RadrootsCoreQuantityPriceError::PerQuantityZero)
    );

    let base = RadrootsCoreQuantityPrice::new(
        common::money("5", "USD"),
        common::qty("2", RadrootsCoreUnit::MassKg),
    );
    let same = base.try_to_unit_price(RadrootsCoreUnit::MassKg).unwrap();
    assert_eq!(same.quantity.unit, RadrootsCoreUnit::MassKg);
    assert_eq!(same.quantity.amount, common::dec("1"));
    assert_eq!(same.amount.amount, common::dec("2.5"));

    let err = base.try_to_unit_price(RadrootsCoreUnit::VolumeMl).unwrap_err();
    assert_eq!(
        err,
        RadrootsCoreQuantityPriceError::NonConvertibleUnits {
            from: RadrootsCoreUnit::MassKg,
            to: RadrootsCoreUnit::VolumeMl
        }
    );
}

#[test]
fn cost_for_and_quantized_price_zero_paths_are_exercised() {
    let p = RadrootsCoreQuantityPrice::new(
        common::money("3.33", "USD"),
        common::qty("1", RadrootsCoreUnit::Each),
    );
    let zero_qty = common::qty("0", RadrootsCoreUnit::Each);
    assert!(p.cost_for(&zero_qty).amount.is_zero());
    assert!(p.cost_for_with_quantized_price(&zero_qty).amount.is_zero());

    let zero_per = RadrootsCoreQuantityPrice::new(
        common::money("3.33", "USD"),
        common::qty("0", RadrootsCoreUnit::Each),
    );
    assert!(zero_per.cost_for(&common::qty("1", RadrootsCoreUnit::Each)).amount.is_zero());
    assert!(
        zero_per
            .cost_for_with_quantized_price(&common::qty("1", RadrootsCoreUnit::Each))
            .amount
            .is_zero()
    );

    let mismatch_qty = common::qty("1", RadrootsCoreUnit::MassG);
    assert!(p.cost_for_with_quantized_price(&mismatch_qty).amount.is_zero());
}

#[test]
fn try_to_unit_price_detects_underflow_to_zero_normalized_amount() {
    let tiny = RadrootsCoreQuantityPrice::new(
        common::money("1", "USD"),
        common::qty("0.0000000000000000000000000001", RadrootsCoreUnit::VolumeMl),
    );
    let err = tiny.try_to_unit_price(RadrootsCoreUnit::VolumeL).unwrap_err();
    assert_eq!(err, RadrootsCoreQuantityPriceError::PerQuantityZero);
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

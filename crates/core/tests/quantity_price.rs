mod common;

use radroots_core::{
    QuantityPrice, RadrootsCoreQuantityPriceError, RadrootsCoreQuantityPriceOps, Unit,
};

#[test]
fn cost_for_scales_by_ratio() {
    let price = QuantityPrice::new(common::money("10", "USD"), common::qty("1", Unit::MassKg));
    let cost = price.cost_for(&common::qty("2", Unit::MassKg));
    assert_eq!(cost.amount, common::dec("20"));
}

#[test]
fn cost_for_returns_zero_on_unit_mismatch() {
    let price = QuantityPrice::new(common::money("10", "USD"), common::qty("1", Unit::MassKg));
    let cost = price.cost_for(&common::qty("1", Unit::Each));
    assert!(cost.amount.is_zero());
}

#[test]
fn cost_for_rounded_and_quantized_price_differ() {
    let price = QuantityPrice::new(common::money("1.005", "USD"), common::qty("1", Unit::Each));
    let qty = common::qty("2", Unit::Each);
    let rounded = price.cost_for_rounded(&qty);
    let quantized = price.cost_for_with_quantized_price(&qty);

    assert_eq!(rounded.amount, common::dec("2.01"));
    assert_eq!(quantized.amount, common::dec("2.02"));
}

#[test]
fn try_cost_for_validates_quantity_and_units() {
    let price = QuantityPrice::new(common::money("10", "USD"), common::qty("1", Unit::Each));
    let zero_price = QuantityPrice::new(common::money("10", "USD"), common::qty("0", Unit::Each));

    assert_eq!(
        zero_price.try_cost_for(&common::qty("1", Unit::Each)),
        Err(RadrootsCoreQuantityPriceError::PerQuantityZero)
    );
    assert_eq!(
        price.try_cost_for(&common::qty("1", Unit::MassKg)),
        Err(RadrootsCoreQuantityPriceError::UnitMismatch {
            have: Unit::MassKg,
            want: Unit::Each
        })
    );
}

#[test]
fn try_cost_for_rounded_error_path_is_exercised() {
    let price = QuantityPrice::new(common::money("10", "USD"), common::qty("1", Unit::Each));
    assert_eq!(
        price.try_cost_for_rounded(&common::qty("1", Unit::MassKg)),
        Err(RadrootsCoreQuantityPriceError::UnitMismatch {
            have: Unit::MassKg,
            want: Unit::Each
        })
    );
}

#[test]
fn try_cost_for_amount_in_converts_mass_units() {
    let price = QuantityPrice::new(common::money("10", "USD"), common::qty("1", Unit::MassKg));
    let cost = price
        .try_cost_for_amount_in(common::dec("500"), Unit::MassG)
        .unwrap();
    assert_eq!(cost.amount, common::dec("5"));
}

#[test]
fn try_cost_for_amount_in_converts_volume_units() {
    let price = QuantityPrice::new(common::money("10", "USD"), common::qty("1", Unit::VolumeL));
    let cost = price
        .try_cost_for_amount_in(common::dec("500"), Unit::VolumeMl)
        .unwrap();
    assert_eq!(cost.amount, common::dec("5"));
}

#[test]
fn try_cost_for_amount_in_rejects_non_convertible_units() {
    let price = QuantityPrice::new(common::money("10", "USD"), common::qty("1", Unit::MassKg));
    assert_eq!(
        price.try_cost_for_amount_in(common::dec("1"), Unit::Each),
        Err(RadrootsCoreQuantityPriceError::NonConvertibleUnits {
            from: Unit::Each,
            to: Unit::MassKg
        })
    );
}

#[test]
fn try_cost_for_amount_in_same_unit_path_is_exercised() {
    let price = QuantityPrice::new(common::money("4", "USD"), common::qty("1", Unit::Each));
    let out = price
        .try_cost_for_amount_in(common::dec("3"), Unit::Each)
        .unwrap();
    assert_eq!(out.amount, common::dec("12"));
}

#[test]
fn try_cost_for_quantity_in_path_is_exercised() {
    let price = QuantityPrice::new(common::money("10", "USD"), common::qty("1", Unit::MassKg));
    let qty = common::qty("250", Unit::MassG);
    let out = price.try_cost_for_quantity_in(&qty).unwrap();
    assert_eq!(out.amount, common::dec("2.5"));
}

#[test]
fn try_to_unit_price_error_and_same_unit_paths_are_exercised() {
    let zero = QuantityPrice::new(common::money("10", "USD"), common::qty("0", Unit::MassKg));
    assert_eq!(
        zero.try_to_unit_price(Unit::MassG),
        Err(RadrootsCoreQuantityPriceError::PerQuantityZero)
    );

    let base = QuantityPrice::new(common::money("5", "USD"), common::qty("2", Unit::MassKg));
    let same = base.try_to_unit_price(Unit::MassKg).unwrap();
    assert_eq!(same.quantity.unit, Unit::MassKg);
    assert_eq!(same.quantity.amount, common::dec("1"));
    assert_eq!(same.amount.amount, common::dec("2.5"));

    let err = base.try_to_unit_price(Unit::VolumeMl).unwrap_err();
    assert_eq!(
        err,
        RadrootsCoreQuantityPriceError::NonConvertibleUnits {
            from: Unit::MassKg,
            to: Unit::VolumeMl
        }
    );
}

#[test]
fn cost_for_and_quantized_price_zero_paths_are_exercised() {
    let p = QuantityPrice::new(common::money("3.33", "USD"), common::qty("1", Unit::Each));
    let zero_qty = common::qty("0", Unit::Each);
    assert!(p.cost_for(&zero_qty).amount.is_zero());
    assert!(p.cost_for_with_quantized_price(&zero_qty).amount.is_zero());

    let zero_per = QuantityPrice::new(common::money("3.33", "USD"), common::qty("0", Unit::Each));
    assert!(
        zero_per
            .cost_for(&common::qty("1", Unit::Each))
            .amount
            .is_zero()
    );
    assert!(
        zero_per
            .cost_for_with_quantized_price(&common::qty("1", Unit::Each))
            .amount
            .is_zero()
    );

    let mismatch_qty = common::qty("1", Unit::MassG);
    assert!(
        p.cost_for_with_quantized_price(&mismatch_qty)
            .amount
            .is_zero()
    );
}

#[test]
fn try_to_unit_price_detects_underflow_to_zero_normalized_amount() {
    let tiny = QuantityPrice::new(
        common::money("1", "USD"),
        common::qty("0.0000000000000000000000000001", Unit::VolumeMl),
    );
    let err = tiny.try_to_unit_price(Unit::VolumeL).unwrap_err();
    assert_eq!(err, RadrootsCoreQuantityPriceError::PerQuantityZero);
}

#[test]
fn try_to_canonical_unit_price_converts_units() {
    let price = QuantityPrice::new(common::money("6.99", "USD"), common::qty("1", Unit::MassLb));
    let canonical = price.try_to_canonical_unit_price().unwrap();
    assert_eq!(canonical.quantity.unit, Unit::MassG);
    assert_eq!(canonical.quantity.amount, common::dec("1"));
    let expected = common::dec("6.99") / common::dec("453.59237");
    assert_eq!(canonical.amount.amount, expected);
}

#[test]
fn is_price_per_canonical_unit_detects_canonical() {
    let price = QuantityPrice::new(common::money("1.00", "USD"), common::qty("1", Unit::MassG));
    assert!(price.is_price_per_canonical_unit());

    let price = QuantityPrice::new(common::money("1.00", "USD"), common::qty("1", Unit::MassKg));
    assert!(!price.is_price_per_canonical_unit());
}

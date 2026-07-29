mod common;

use radroots_core::{
    Currency, Decimal, Money, Quantity, QuantityPrice, Unit,
    pricing::{Error, QuantityPriceOps},
};

fn unit_price(amount: &str, per: &str, unit: Unit) -> QuantityPrice {
    QuantityPrice::try_new(common::money(amount, "USD"), common::qty(per, unit))
        .expect("valid price")
}

#[test]
fn checked_cost_scales_by_ratio_and_rounds_deterministically() {
    let price = unit_price("10", "1", Unit::MassKg);
    let cost = price.try_cost_for(&common::qty("2", Unit::MassKg)).unwrap();
    assert_eq!(cost.amount(), common::dec("20"));

    let rounded = unit_price("1.005", "1", Unit::Each)
        .try_cost_for_rounded(&common::qty("2", Unit::Each))
        .unwrap();
    assert_eq!(rounded.amount(), common::dec("2.01"));
}

#[test]
fn checked_constructor_rejects_zero_per_quantity() {
    assert_eq!(
        QuantityPrice::try_new(common::money("1", "USD"), Quantity::zero(Unit::Each)),
        Err(Error::PerQuantityZero)
    );
}

#[test]
fn checked_cost_never_silently_converts_failures_to_zero() {
    let price = unit_price("10", "1", Unit::Each);
    assert_eq!(
        price.try_cost_for(&common::qty("1", Unit::MassKg)),
        Err(Error::UnitMismatch {
            have: Unit::MassKg,
            want: Unit::Each,
        })
    );
    assert_eq!(
        price.try_cost_for_amount_in(common::dec("-1"), Unit::Each),
        Err(Error::NegativeRequestedQuantity)
    );
    assert_eq!(
        price.try_cost_for_rounded(&common::qty("1", Unit::MassKg)),
        Err(Error::UnitMismatch {
            have: Unit::MassKg,
            want: Unit::Each,
        })
    );
}

#[test]
fn checked_cost_reports_arithmetic_overflow() {
    let price = QuantityPrice::try_new(
        Money::try_new(Decimal::MAX, Currency::USD).unwrap(),
        Quantity::try_new(Decimal::ONE, Unit::Each).unwrap(),
    )
    .unwrap();
    assert_eq!(
        price.try_cost_for(&Quantity::try_new(Decimal::from(2u32), Unit::Each).unwrap()),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn amount_and_quantity_conversion_support_mass_and_volume() {
    let mass = unit_price("10", "1", Unit::MassKg);
    assert_eq!(
        mass.try_cost_for_amount_in(common::dec("500"), Unit::MassG)
            .unwrap()
            .amount(),
        common::dec("5")
    );
    assert_eq!(
        mass.try_cost_for_quantity_in(&common::qty("250", Unit::MassG))
            .unwrap()
            .amount(),
        common::dec("2.5")
    );
    assert_eq!(
        mass.try_cost_for_amount_in(common::dec("1"), Unit::Each),
        Err(Error::NonConvertibleUnits {
            from: Unit::Each,
            to: Unit::MassKg,
        })
    );

    let volume = unit_price("10", "1", Unit::VolumeL);
    assert_eq!(
        volume
            .try_cost_for_amount_in(common::dec("500"), Unit::VolumeMl)
            .unwrap()
            .amount(),
        common::dec("5")
    );
}

#[test]
fn unit_price_conversion_is_checked_and_uses_accessors() {
    let base = unit_price("5", "2", Unit::MassKg);
    let same = base.try_to_unit_price(Unit::MassKg).unwrap();
    assert_eq!(same.quantity().unit(), Unit::MassKg);
    assert_eq!(same.quantity().amount(), Decimal::ONE);
    assert_eq!(same.amount().amount(), common::dec("2.5"));
    assert_eq!(
        base.try_to_unit_price(Unit::VolumeMl),
        Err(Error::NonConvertibleUnits {
            from: Unit::MassKg,
            to: Unit::VolumeMl,
        })
    );

    let canonical = unit_price("6.99", "1", Unit::MassLb)
        .try_to_canonical_unit_price()
        .unwrap();
    assert_eq!(canonical.quantity().unit(), Unit::MassG);
    assert_eq!(canonical.quantity().amount(), Decimal::ONE);
    assert!(canonical.is_price_per_canonical_unit());
}

#[test]
fn unit_price_conversion_detects_precision_underflow() {
    let tiny = unit_price("1", "0.0000000000000000000000000001", Unit::VolumeMl);
    assert_eq!(
        tiny.try_to_unit_price(Unit::VolumeL),
        Err(Error::PerQuantityZero)
    );
}

#[test]
fn pricing_error_messages_are_stable() {
    assert_eq!(
        Error::PerQuantityZero.to_string(),
        "price quantity must be greater than zero"
    );
    assert_eq!(
        Error::PerQuantityNegative.to_string(),
        "price quantity must not be negative"
    );
    assert_eq!(
        Error::NegativePrice.to_string(),
        "price amount must not be negative"
    );
    assert_eq!(
        Error::NegativeRequestedQuantity.to_string(),
        "requested quantity must not be negative"
    );
    assert_eq!(
        Error::ArithmeticOverflow.to_string(),
        "price arithmetic overflow"
    );
}

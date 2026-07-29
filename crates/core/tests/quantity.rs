mod common;

use radroots_core::{Decimal, Quantity, Unit, quantity::Error};

#[test]
fn zero_and_checked_constructor_preserve_invariants() {
    let zero = Quantity::zero(Unit::MassKg);
    assert!(zero.is_zero());
    assert_eq!(zero.amount(), Decimal::ZERO);
    assert_eq!(zero.unit(), Unit::MassKg);
    assert_eq!(zero.canonical_unit(), Unit::MassG);
    assert!(!zero.is_canonical());

    assert_eq!(
        Quantity::try_new(common::dec("-1"), Unit::Each),
        Err(Error::NegativeAmount)
    );
}

#[test]
fn label_helpers_set_replace_and_clear() {
    let quantity = common::qty("1", Unit::Each).with_label("box");
    assert_eq!(quantity.label(), Some("box"));
    let quantity = quantity.with_optional_label(Some("case"));
    assert_eq!(quantity.label(), Some("case"));
    let quantity = quantity.with_optional_label::<&str>(None);
    assert_eq!(quantity.label(), None);
    assert_eq!(quantity.with_label("bag").clear_label().label(), None);
}

#[test]
fn checked_addition_and_subtraction_require_matching_units() {
    let first = common::qty("5", Unit::Each).with_label("lhs");
    let second = common::qty("2", Unit::Each);
    let kilograms = common::qty("1", Unit::MassKg);

    let sum = first.try_add(&second).unwrap();
    assert_eq!(sum.amount(), common::dec("7"));
    assert_eq!(sum.label(), Some("lhs"));
    let difference = first.try_sub(&second).unwrap();
    assert_eq!(difference.amount(), common::dec("3"));
    assert_eq!(difference.label(), Some("lhs"));

    assert_eq!(first.try_add(&kilograms), Err(Error::UnitMismatch));
    assert_eq!(first.try_sub(&kilograms), Err(Error::UnitMismatch));
    assert!(first.checked_add(&kilograms).is_none());
    assert!(first.checked_sub(&kilograms).is_none());
    assert_eq!(second.try_sub(&first), Err(Error::NegativeAmount));
}

#[test]
fn checked_scaling_reports_negative_overflow_and_zero_division() {
    let quantity = common::qty("2", Unit::Each).with_label("unit");
    let scaled = quantity.checked_mul_decimal(common::dec("2.5")).unwrap();
    assert_eq!(scaled.amount(), common::dec("5"));
    assert_eq!(scaled.unit(), Unit::Each);
    assert_eq!(scaled.label(), Some("unit"));

    let divided = quantity.checked_div_decimal(common::dec("2")).unwrap();
    assert_eq!(divided.amount(), common::dec("1"));
    assert_eq!(
        quantity.checked_mul_decimal(common::dec("-1")),
        Err(Error::NegativeAmount)
    );
    assert_eq!(
        quantity.checked_div_decimal(Decimal::ZERO),
        Err(Error::DivisionByZero)
    );

    let max = Quantity::try_new(Decimal::MAX, Unit::Each).unwrap();
    assert_eq!(max.try_add(&quantity), Err(Error::ArithmeticOverflow));
}

#[test]
fn conversion_is_checked_and_preserves_labels() {
    let kilograms = common::qty("1", Unit::MassKg).with_label("bag");
    let grams = kilograms.try_convert_to(Unit::MassG).unwrap();
    assert_eq!(grams.amount(), common::dec("1000"));
    assert_eq!(grams.unit(), Unit::MassG);
    assert_eq!(grams.label(), Some("bag"));
    assert!(grams.is_canonical());

    let canonical = common::qty("2", Unit::VolumeL).to_canonical().unwrap();
    assert_eq!(canonical.unit(), Unit::VolumeMl);
    assert_eq!(canonical.amount(), common::dec("2000"));

    assert_eq!(
        common::qty("1", Unit::Each).try_convert_to(Unit::MassG),
        Err(radroots_core::unit::ConvertError::NotConvertibleUnits {
            from: Unit::Each,
            to: Unit::MassG,
        })
    );
}

#[test]
fn display_and_error_messages_are_stable() {
    assert_eq!(
        common::qty("1.5", Unit::Each).with_label("bag").to_string(),
        "1.5 each (bag)"
    );
    assert_eq!(common::qty("1.5", Unit::Each).to_string(), "1.5 each");
    assert_eq!(
        Error::NegativeAmount.to_string(),
        "quantity amount must be ≥ 0"
    );
    assert_eq!(Error::UnitMismatch.to_string(), "quantity unit mismatch");
    assert_eq!(
        Error::ArithmeticOverflow.to_string(),
        "quantity arithmetic overflow"
    );
    assert_eq!(
        Error::DivisionByZero.to_string(),
        "quantity division by zero"
    );
}

#[test]
fn display_propagates_formatter_errors() {
    use core::fmt::{self, Write};

    struct FailWriter;
    impl Write for FailWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    let quantity = common::qty("1.5", Unit::Each).with_label("bag");
    assert!(fmt::write(&mut FailWriter, format_args!("{quantity}")).is_err());
}

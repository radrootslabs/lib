mod common;

use radroots_core::{Currency, Decimal, Money, Quantity, Unit};

const VALUES: [&str; 8] = ["-100", "-1.25", "-0.01", "0", "0.01", "1.25", "42", "100"];
const NON_NEGATIVE_VALUES: [&str; 6] = ["0", "0.01", "1", "1.25", "42", "100"];

#[test]
fn checked_decimal_arithmetic_obeys_ring_laws_on_representative_values() {
    for left in VALUES.map(common::dec) {
        assert_eq!(left.checked_add(Decimal::ZERO).unwrap(), left);
        assert_eq!(left.checked_mul(Decimal::ONE).unwrap(), left);
        for right in VALUES.map(common::dec) {
            assert_eq!(
                left.checked_add(right).unwrap(),
                right.checked_add(left).unwrap()
            );
            assert_eq!(
                left.checked_mul(right).unwrap(),
                right.checked_mul(left).unwrap()
            );
            assert_eq!(
                left.checked_add(right).unwrap().checked_sub(right).unwrap(),
                left
            );
        }
    }
}

#[test]
fn checked_decimal_multiplication_distributes_over_addition() {
    for left in VALUES.map(common::dec) {
        for middle in VALUES.map(common::dec) {
            for right in VALUES.map(common::dec) {
                let combined = left
                    .checked_add(middle)
                    .unwrap()
                    .checked_mul(right)
                    .unwrap();
                let distributed = left
                    .checked_mul(right)
                    .unwrap()
                    .checked_add(middle.checked_mul(right).unwrap())
                    .unwrap();
                assert_eq!(combined, distributed);
            }
        }
    }
}

#[test]
fn checked_money_addition_is_commutative_and_associative() {
    for first in NON_NEGATIVE_VALUES.map(common::dec) {
        for second in NON_NEGATIVE_VALUES.map(common::dec) {
            for third in NON_NEGATIVE_VALUES.map(common::dec) {
                let first = Money::try_new(first, Currency::USD).unwrap();
                let second = Money::try_new(second, Currency::USD).unwrap();
                let third = Money::try_new(third, Currency::USD).unwrap();
                assert_eq!(first.checked_add(&second), second.checked_add(&first));
                assert_eq!(
                    first.checked_add(&second).unwrap().checked_add(&third),
                    first.checked_add(&second.checked_add(&third).unwrap())
                );
            }
        }
    }
}

#[test]
fn checked_quantity_addition_preserves_unit_and_is_associative() {
    for unit in [Unit::Each, Unit::MassG, Unit::VolumeMl] {
        for first in NON_NEGATIVE_VALUES.map(common::dec) {
            for second in NON_NEGATIVE_VALUES.map(common::dec) {
                let first = Quantity::try_new(first, unit).unwrap();
                let second = Quantity::try_new(second, unit).unwrap();
                let sum = first.try_add(&second).unwrap();
                assert_eq!(sum.unit(), unit);
                assert_eq!(
                    sum.amount(),
                    first.amount().checked_add(second.amount()).unwrap()
                );
                assert_eq!(first.try_add(&second), second.try_add(&first));
            }
        }
    }
}

#[test]
fn canonical_unit_conversion_is_idempotent() {
    for (unit, canonical) in [
        (Unit::Each, Unit::Each),
        (Unit::MassKg, Unit::MassG),
        (Unit::MassG, Unit::MassG),
        (Unit::MassOz, Unit::MassG),
        (Unit::MassLb, Unit::MassG),
        (Unit::VolumeL, Unit::VolumeMl),
        (Unit::VolumeMl, Unit::VolumeMl),
    ] {
        for amount in NON_NEGATIVE_VALUES.map(common::dec) {
            let quantity = Quantity::try_new(amount, unit).unwrap();
            let once = quantity.to_canonical().unwrap();
            let twice = once.to_canonical().unwrap();
            assert_eq!(once, twice);
            assert_eq!(once.unit(), canonical);
        }
    }
}

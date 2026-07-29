mod common;

use radroots_core::{Currency, Decimal, Money, money::Error};

#[test]
fn zero_and_checked_constructor_preserve_invariants() {
    let zero = Money::zero(Currency::USD);
    assert!(zero.is_zero());
    assert_eq!(zero.amount(), Decimal::ZERO);
    assert_eq!(zero.currency(), Currency::USD);

    assert_eq!(
        Money::try_new(common::dec("-0.01"), Currency::USD),
        Err(Error::NegativeAmount)
    );
    let signed_zero = Money::try_new(common::dec("-0.00"), Currency::USD).unwrap();
    assert_eq!(signed_zero.amount(), Decimal::ZERO);
}

#[test]
fn quantization_is_deterministic_midpoint_away_from_zero() {
    let a = common::money("1.234", "USD").quantize_to_currency();
    let b = common::money("1.235", "USD").quantize_to_currency();
    assert_eq!(a.amount(), common::dec("1.23"));
    assert_eq!(b.amount(), common::dec("1.24"));
}

#[test]
fn checked_arithmetic_reports_domain_failures() {
    let one = common::money("1.00", "USD");
    let two = common::money("2.00", "USD");
    let euros = common::money("3.00", "EUR");

    assert_eq!(one.checked_add(&two).unwrap().amount(), common::dec("3"));
    assert_eq!(one.checked_add(&euros), Err(Error::CurrencyMismatch));
    assert_eq!(two.checked_sub(&one).unwrap().amount(), common::dec("1"));
    assert_eq!(one.checked_sub(&two), Err(Error::NegativeAmount));
    assert_eq!(
        one.checked_mul_decimal(common::dec("-1")),
        Err(Error::NegativeAmount)
    );
    assert_eq!(
        one.checked_div_decimal(Decimal::ZERO),
        Err(Error::DivisionByZero)
    );

    let max = Money::try_new(Decimal::MAX, Currency::USD).unwrap();
    assert_eq!(max.checked_add(&one), Err(Error::ArithmeticOverflow));
}

#[test]
fn minor_units_exact_and_rounded_cover_currency_exponents() {
    let exact = common::money("1.23", "USD");
    let fractional = common::money("1.234", "USD");
    let rounded = common::money("1.235", "USD");
    assert_eq!(exact.to_minor_units_u64_exact(), Ok(123));
    assert_eq!(
        fractional.to_minor_units_u64_exact(),
        Err(Error::NotWholeMinorUnits)
    );
    assert_eq!(rounded.to_minor_units_u64_rounded(), Ok(124));

    assert_eq!(
        common::money("123", "JPY").to_minor_units_u64_exact(),
        Ok(123)
    );
    assert_eq!(
        common::money("1.234", "KWD").to_minor_units_u64_exact(),
        Ok(1234)
    );
}

#[test]
fn minor_unit_constructors_roundtrip_and_detect_u32_overflow() {
    for currency in [Currency::JPY, Currency::USD, common::currency("KWD")] {
        for minor in [0, 1, 99, 12_345, u32::MAX as u64] {
            let money = Money::from_minor_units_u64(minor, currency);
            assert_eq!(money.to_minor_units_u64_exact(), Ok(minor));
        }
    }

    let from_u32 = Money::from_minor_units_u32(505, Currency::USD);
    assert_eq!(from_u32.amount(), common::dec("5.05"));
    assert_eq!(from_u32.to_minor_units_u32_rounded(), Ok(505));

    let too_large = Money::from_minor_units_u64(u64::from(u32::MAX) + 1, Currency::USD);
    assert_eq!(
        too_large.to_minor_units_u32_exact(),
        Err(Error::AmountOverflow)
    );
    assert_eq!(
        too_large.to_minor_units_u32_rounded(),
        Err(Error::AmountOverflow)
    );
}

#[test]
fn exact_scale_change_never_rounds() {
    let money = common::money("1.2300", "USD");
    assert_eq!(money.try_with_scale_exact(2).unwrap().amount().scale(), 2);
    assert_eq!(
        common::money("1.235", "USD").try_with_scale_exact(2),
        Err(Error::PrecisionLoss)
    );
}

#[test]
fn display_and_error_messages_are_stable() {
    assert_eq!(common::money("10", "USD").to_string(), "10 USD");
    assert_eq!(
        Error::NegativeAmount.to_string(),
        "money amount must be ≥ 0"
    );
    assert_eq!(
        Error::NotWholeMinorUnits.to_string(),
        "money not a whole number of minor units"
    );
    assert_eq!(
        Error::AmountOverflow.to_string(),
        "money minor-unit conversion overflow"
    );
    assert_eq!(
        Error::CurrencyMismatch.to_string(),
        "money currency mismatch"
    );
    assert_eq!(
        Error::ArithmeticOverflow.to_string(),
        "money arithmetic overflow"
    );
    assert_eq!(Error::DivisionByZero.to_string(), "money division by zero");
    assert_eq!(
        Error::ScaleOutOfRange.to_string(),
        "money scale is outside the supported range"
    );
    assert_eq!(
        Error::PrecisionLoss.to_string(),
        "money operation would lose precision"
    );
}

mod common;

use radroots_core::{Currency, Decimal, Money, money::Error};
use rust_decimal::RoundingStrategy;

#[test]
fn zero_and_is_zero() {
    let usd = Currency::USD;
    let zero = Money::zero(usd);
    assert!(zero.is_zero());
    assert_eq!(zero.currency, usd);
}

#[test]
fn ensure_non_negative_rejects_negative_amount() {
    let money = Money::new(common::dec("-1"), Currency::USD);
    assert_eq!(money.ensure_non_negative(), Err(Error::NegativeAmount));
}

#[test]
fn ensure_non_negative_accepts_zero_and_positive() {
    let zero = Money::new(common::dec("0"), Currency::USD);
    let pos = Money::new(common::dec("1"), Currency::USD);
    assert_eq!(zero.ensure_non_negative(), Ok(()));
    assert_eq!(pos.ensure_non_negative(), Ok(()));
}

#[test]
fn checked_constructor_rejects_negative_and_canonicalizes_zero() {
    assert_eq!(
        Money::try_new(common::dec("-0.01"), Currency::USD),
        Err(Error::NegativeAmount)
    );
    let zero = Money::try_new(common::dec("-0.00"), Currency::USD).unwrap();
    assert_eq!(zero.amount(), Decimal::ZERO);
    assert_eq!(zero.currency(), Currency::USD);
}

#[test]
fn quantize_to_currency_rounds_midpoint_away_from_zero() {
    let usd = Currency::USD;
    let a = Money::new(common::dec("1.234"), usd).quantize_to_currency();
    let b = Money::new(common::dec("1.235"), usd).quantize_to_currency();
    let c = Money::new(common::dec("-1.235"), usd).quantize_to_currency();

    assert_eq!(a.amount, common::dec("1.23"));
    assert_eq!(b.amount, common::dec("1.24"));
    assert_eq!(c.amount, common::dec("-1.24"));
}

#[test]
fn quantize_to_currency_with_strategy_uses_strategy() {
    let usd = Currency::USD;
    let a = Money::new(common::dec("1.235"), usd)
        .quantize_to_currency_with_strategy(RoundingStrategy::MidpointTowardZero);
    let b = Money::new(common::dec("-1.235"), usd)
        .quantize_to_currency_with_strategy(RoundingStrategy::MidpointTowardZero);
    assert_eq!(a.amount, common::dec("1.23"));
    assert_eq!(b.amount, common::dec("-1.23"));
}

#[test]
fn checked_add_and_sub_require_currency_match() {
    let usd = Currency::USD;
    let eur = Currency::EUR;
    let a = Money::new(common::dec("1.00"), usd);
    let b = Money::new(common::dec("2.00"), usd);
    let c = Money::new(common::dec("3.00"), eur);

    assert_eq!(a.checked_add(&b).unwrap().amount, common::dec("3.00"));
    assert_eq!(a.checked_add(&c), Err(Error::CurrencyMismatch));
    assert_eq!(b.checked_sub(&a).unwrap().amount, common::dec("1.00"));
}

#[test]
fn checked_sub_mismatch_returns_currency_error() {
    let usd = Currency::USD;
    let eur = Currency::EUR;
    let a = Money::new(common::dec("1.00"), usd);
    let b = Money::new(common::dec("2.00"), eur);
    assert_eq!(a.checked_sub(&b), Err(Error::CurrencyMismatch));
}

#[test]
fn checked_arithmetic_rejects_invalid_results() {
    let usd = Currency::USD;
    let one = Money::try_new(Decimal::ONE, usd).unwrap();
    let two = Money::try_new(Decimal::from(2u32), usd).unwrap();
    assert_eq!(one.checked_sub(&two), Err(Error::NegativeAmount));

    let max = Money::try_new(Decimal::MAX, usd).unwrap();
    assert_eq!(max.checked_add(&one), Err(Error::ArithmeticOverflow));
    assert_eq!(
        one.checked_div_decimal(Decimal::ZERO),
        Err(Error::DivisionByZero)
    );
    assert_eq!(
        one.checked_mul_decimal(common::dec("-1")),
        Err(Error::NegativeAmount)
    );
}

#[test]
fn minor_units_exact_and_rounded() {
    let usd = Currency::USD;
    let exact = Money::new(common::dec("1.23"), usd);
    let frac = Money::new(common::dec("1.234"), usd);
    let rounded = Money::new(common::dec("1.235"), usd);

    assert_eq!(exact.to_minor_units_u64_exact().unwrap(), 123);
    assert_eq!(
        frac.to_minor_units_u64_exact(),
        Err(Error::NotWholeMinorUnits)
    );
    assert_eq!(
        rounded
            .to_minor_units_u64_rounded(RoundingStrategy::MidpointAwayFromZero)
            .unwrap(),
        124
    );
}

#[test]
fn minor_units_cover_additional_currency_exponents() {
    let jpy = Money::new(common::dec("123"), common::currency("JPY"));
    assert_eq!(jpy.to_minor_units_u64_exact().unwrap(), 123);
    assert_eq!(
        jpy.to_minor_units_u64_rounded(RoundingStrategy::MidpointAwayFromZero)
            .unwrap(),
        123
    );

    let kwd = Money::new(common::dec("1.234"), common::currency("KWD"));
    assert_eq!(kwd.to_minor_units_u64_exact().unwrap(), 1234);
}

#[test]
fn minor_units_u32_overflow_is_detected() {
    let usd = Currency::USD;
    let too_large = Money::from_minor_units_u64(u64::from(u32::MAX) + 1, usd);
    assert_eq!(
        too_large.to_minor_units_u32_exact(),
        Err(Error::AmountOverflow)
    );
}

#[test]
fn minor_units_u32_exact_success_path_is_exercised() {
    let usd = Currency::USD;
    let m = Money::new(common::dec("42.01"), usd);
    assert_eq!(m.to_minor_units_u32_exact().unwrap(), 4201);
    let fractional = Money::new(common::dec("1.001"), usd);
    assert_eq!(
        fractional.to_minor_units_u32_exact(),
        Err(Error::NotWholeMinorUnits)
    );
}

#[test]
fn from_minor_units_u32_and_u32_rounded_paths_are_exercised() {
    let usd = Currency::USD;
    let from_u32 = Money::from_minor_units_u32(505, usd);
    assert_eq!(from_u32.amount, common::dec("5.05"));
    assert_eq!(
        from_u32
            .to_minor_units_u32_rounded(RoundingStrategy::MidpointAwayFromZero)
            .unwrap(),
        505
    );
}

#[test]
fn minor_units_u32_rounded_overflow_is_detected() {
    let usd = Currency::USD;
    let too_large = Money::from_minor_units_u64(u64::from(u32::MAX) + 1, usd);
    assert_eq!(
        too_large.to_minor_units_u32_rounded(RoundingStrategy::MidpointAwayFromZero),
        Err(Error::AmountOverflow)
    );
    let negative = Money::new(common::dec("-1.00"), usd);
    assert_eq!(
        negative.to_minor_units_u32_rounded(RoundingStrategy::MidpointAwayFromZero),
        Err(Error::NegativeAmount)
    );
}

#[test]
fn with_scale_path_is_exercised() {
    let usd = Currency::USD;
    let m = Money::new(common::dec("1.2300"), usd).with_scale(1);
    assert_eq!(m.amount, common::dec("1.2"));
}

#[test]
fn from_minor_units_roundtrips() {
    let usd = Currency::USD;
    let money = Money::from_minor_units_u64(12345, usd);
    assert_eq!(money.to_minor_units_u64_exact().unwrap(), 12345);
}

#[test]
fn minor_units_roundtrip_across_supported_exponents() {
    for currency in [Currency::JPY, Currency::USD, common::currency("KWD")] {
        for minor in [0, 1, 99, 12_345, u32::MAX as u64] {
            let money = Money::from_minor_units_u64(minor, currency);
            assert_eq!(money.to_minor_units_u64_exact().unwrap(), minor);
        }
    }
}

#[test]
fn exact_scale_change_never_rounds() {
    let money = Money::try_new(common::dec("1.2300"), Currency::USD).unwrap();
    assert_eq!(money.try_with_scale_exact(2).unwrap().amount().scale(), 2);
    assert_eq!(
        Money::try_new(common::dec("1.235"), Currency::USD)
            .unwrap()
            .try_with_scale_exact(2),
        Err(Error::PrecisionLoss)
    );
}

#[test]
fn display_and_operator_impl_paths_are_exercised() {
    let usd = Currency::USD;
    let m = Money::new(common::dec("10"), usd);
    assert_eq!(m.to_string(), "10 USD");

    let times = m.clone() * common::dec("2");
    assert_eq!(times.amount, common::dec("20"));
    let divided = m / common::dec("4");
    assert_eq!(divided.amount, common::dec("2.5"));
}

#[test]
fn invariant_error_display_variants_are_exercised() {
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

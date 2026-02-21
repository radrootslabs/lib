mod common;

use radroots_core::{RadrootsCoreCurrency, RadrootsCoreMoney, RadrootsCoreMoneyInvariantError};
use rust_decimal::RoundingStrategy;

#[test]
fn zero_and_is_zero() {
    let usd = RadrootsCoreCurrency::USD;
    let zero = RadrootsCoreMoney::zero(usd);
    assert!(zero.is_zero());
    assert_eq!(zero.currency, usd);
}

#[test]
fn ensure_non_negative_rejects_negative_amount() {
    let money = RadrootsCoreMoney::new(common::dec("-1"), RadrootsCoreCurrency::USD);
    assert_eq!(
        money.ensure_non_negative(),
        Err(RadrootsCoreMoneyInvariantError::NegativeAmount)
    );
}

#[test]
fn ensure_non_negative_accepts_zero_and_positive() {
    let zero = RadrootsCoreMoney::new(common::dec("0"), RadrootsCoreCurrency::USD);
    let pos = RadrootsCoreMoney::new(common::dec("1"), RadrootsCoreCurrency::USD);
    assert_eq!(zero.ensure_non_negative(), Ok(()));
    assert_eq!(pos.ensure_non_negative(), Ok(()));
}

#[test]
fn quantize_to_currency_rounds_midpoint_away_from_zero() {
    let usd = RadrootsCoreCurrency::USD;
    let a = RadrootsCoreMoney::new(common::dec("1.234"), usd).quantize_to_currency();
    let b = RadrootsCoreMoney::new(common::dec("1.235"), usd).quantize_to_currency();
    let c = RadrootsCoreMoney::new(common::dec("-1.235"), usd).quantize_to_currency();

    assert_eq!(a.amount, common::dec("1.23"));
    assert_eq!(b.amount, common::dec("1.24"));
    assert_eq!(c.amount, common::dec("-1.24"));
}

#[test]
fn quantize_to_currency_with_strategy_uses_strategy() {
    let usd = RadrootsCoreCurrency::USD;
    let a = RadrootsCoreMoney::new(common::dec("1.235"), usd)
        .quantize_to_currency_with_strategy(RoundingStrategy::MidpointTowardZero);
    let b = RadrootsCoreMoney::new(common::dec("-1.235"), usd)
        .quantize_to_currency_with_strategy(RoundingStrategy::MidpointTowardZero);
    assert_eq!(a.amount, common::dec("1.23"));
    assert_eq!(b.amount, common::dec("-1.23"));
}

#[test]
fn checked_add_and_sub_require_currency_match() {
    let usd = RadrootsCoreCurrency::USD;
    let eur = RadrootsCoreCurrency::EUR;
    let a = RadrootsCoreMoney::new(common::dec("1.00"), usd);
    let b = RadrootsCoreMoney::new(common::dec("2.00"), usd);
    let c = RadrootsCoreMoney::new(common::dec("3.00"), eur);

    assert_eq!(a.checked_add(&b).unwrap().amount, common::dec("3.00"));
    assert_eq!(
        a.checked_add(&c),
        Err(RadrootsCoreMoneyInvariantError::CurrencyMismatch)
    );
    assert_eq!(b.checked_sub(&a).unwrap().amount, common::dec("1.00"));
}

#[test]
fn checked_sub_mismatch_returns_currency_error() {
    let usd = RadrootsCoreCurrency::USD;
    let eur = RadrootsCoreCurrency::EUR;
    let a = RadrootsCoreMoney::new(common::dec("1.00"), usd);
    let b = RadrootsCoreMoney::new(common::dec("2.00"), eur);
    assert_eq!(
        a.checked_sub(&b),
        Err(RadrootsCoreMoneyInvariantError::CurrencyMismatch)
    );
}

#[test]
fn minor_units_exact_and_rounded() {
    let usd = RadrootsCoreCurrency::USD;
    let exact = RadrootsCoreMoney::new(common::dec("1.23"), usd);
    let frac = RadrootsCoreMoney::new(common::dec("1.234"), usd);
    let rounded = RadrootsCoreMoney::new(common::dec("1.235"), usd);

    assert_eq!(exact.to_minor_units_u64_exact().unwrap(), 123);
    assert_eq!(
        frac.to_minor_units_u64_exact(),
        Err(RadrootsCoreMoneyInvariantError::NotWholeMinorUnits)
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
    let jpy = RadrootsCoreMoney::new(common::dec("123"), common::currency("JPY"));
    assert_eq!(jpy.to_minor_units_u64_exact().unwrap(), 123);
    assert_eq!(
        jpy.to_minor_units_u64_rounded(RoundingStrategy::MidpointAwayFromZero)
            .unwrap(),
        123
    );

    let kwd = RadrootsCoreMoney::new(common::dec("1.234"), common::currency("KWD"));
    assert_eq!(kwd.to_minor_units_u64_exact().unwrap(), 1234);
}

#[test]
fn minor_units_u32_overflow_is_detected() {
    let usd = RadrootsCoreCurrency::USD;
    let too_large = RadrootsCoreMoney::from_minor_units_u64(u64::from(u32::MAX) + 1, usd);
    assert_eq!(
        too_large.to_minor_units_u32_exact(),
        Err(RadrootsCoreMoneyInvariantError::AmountOverflow)
    );
}

#[test]
fn from_minor_units_u32_and_u32_rounded_paths_are_exercised() {
    let usd = RadrootsCoreCurrency::USD;
    let from_u32 = RadrootsCoreMoney::from_minor_units_u32(505, usd);
    assert_eq!(from_u32.amount, common::dec("5.05"));
    assert_eq!(
        from_u32
            .to_minor_units_u32_rounded(RoundingStrategy::MidpointAwayFromZero)
            .unwrap(),
        505
    );
}

#[test]
fn from_minor_units_roundtrips() {
    let usd = RadrootsCoreCurrency::USD;
    let money = RadrootsCoreMoney::from_minor_units_u64(12345, usd);
    assert_eq!(money.to_minor_units_u64_exact().unwrap(), 12345);
}

#[test]
fn display_and_operator_impl_paths_are_exercised() {
    let usd = RadrootsCoreCurrency::USD;
    let m = RadrootsCoreMoney::new(common::dec("10"), usd);
    assert_eq!(m.to_string(), "10 USD");

    let times = m.clone() * common::dec("2");
    assert_eq!(times.amount, common::dec("20"));
    let divided = m / common::dec("4");
    assert_eq!(divided.amount, common::dec("2.5"));
}

#[test]
fn invariant_error_display_variants_are_exercised() {
    assert_eq!(
        RadrootsCoreMoneyInvariantError::NegativeAmount.to_string(),
        "money amount must be ≥ 0"
    );
    assert_eq!(
        RadrootsCoreMoneyInvariantError::NotWholeMinorUnits.to_string(),
        "money not a whole number of minor units"
    );
    assert_eq!(
        RadrootsCoreMoneyInvariantError::AmountOverflow.to_string(),
        "money minor-unit conversion overflow"
    );
    assert_eq!(
        RadrootsCoreMoneyInvariantError::CurrencyMismatch.to_string(),
        "money currency mismatch"
    );
}

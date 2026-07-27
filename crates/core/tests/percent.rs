mod common;

use core::str::FromStr;

use radroots_core::{Decimal, Percent, decimal, money, percent::ParseError};

#[test]
fn ratio_roundtrip() {
    let pct = Percent::from_ratio(common::dec("0.125"));
    assert_eq!(pct.value, common::dec("12.5"));
    assert_eq!(pct.to_ratio(), common::dec("0.125"));
}

#[test]
fn checked_ratio_paths_report_overflow_and_roundtrip() {
    assert_eq!(
        Percent::try_from_ratio(Decimal::MAX),
        Err(decimal::Error::ArithmeticOverflow)
    );
    for ratio in ["-0.125", "0", "0.5", "1", "2.75"] {
        let ratio = common::dec(ratio);
        let percent = Percent::try_from_ratio(ratio).unwrap();
        assert_eq!(percent.try_to_ratio().unwrap(), ratio);
    }
}

#[test]
fn parses_percent_strings() {
    let pct = Percent::from_str("12.5%").unwrap();
    assert_eq!(pct.value, common::dec("12.5"));

    let pct = Percent::from_str(" 12.5 ").unwrap();
    assert_eq!(pct.value, common::dec("12.5"));

    assert_eq!(Percent::from_str("nope"), Err(ParseError::InvalidNumber));
}

#[test]
fn of_money_and_quantized() {
    let base = common::money("20.00", "USD");
    let pct = Percent::from_str("10").unwrap();
    let out = pct.of_money(&base);
    assert_eq!(out.amount, common::dec("2.00"));

    let tiny = common::money("0.05", "USD");
    let pct = Percent::from_str("10").unwrap();
    let rounded = pct.of_money_quantized(&tiny);
    assert_eq!(rounded.amount, common::dec("0.01"));
}

#[test]
fn checked_money_calculation_rejects_invalid_base_and_overflow() {
    let negative = common::money("-10", "USD");
    let pct = Percent::new(common::dec("10"));
    assert_eq!(
        pct.try_of_money(&negative),
        Err(money::Error::NegativeAmount)
    );

    let max = radroots_core::Money::try_new(Decimal::MAX, radroots_core::Currency::USD).unwrap();
    let two_hundred = Percent::new(common::dec("200"));
    assert_eq!(
        two_hundred.try_of_money(&max),
        Err(money::Error::ArithmeticOverflow)
    );
}

#[test]
fn display_and_parse_error_display_paths_are_exercised() {
    let pct = Percent::from_str("12.5%").unwrap();
    assert_eq!(pct.to_string(), "12.5%");
    assert_eq!(
        ParseError::InvalidNumber.to_string(),
        "invalid percent string"
    );
}

mod common;

use core::str::FromStr;

use radroots_core::{Decimal, Percent, decimal, money, percent::ParseError};

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
fn parsing_and_accessors_preserve_signed_values() {
    let percent = Percent::from_str("12.5%").unwrap();
    assert_eq!(percent.value(), common::dec("12.5"));
    assert_eq!(Percent::from_str(" 12.5 ").unwrap(), percent);
    assert_eq!(Percent::from_str("nope"), Err(ParseError::InvalidNumber));
}

#[test]
fn checked_money_calculation_and_quantization_are_deterministic() {
    let base = common::money("20.00", "USD");
    let percent = Percent::from_str("10").unwrap();
    assert_eq!(
        percent.try_of_money(&base).unwrap().amount(),
        common::dec("2")
    );

    let tiny = common::money("0.05", "USD");
    assert_eq!(
        percent.try_of_money_quantized(&tiny).unwrap().amount(),
        common::dec("0.01")
    );

    let max = radroots_core::Money::try_new(Decimal::MAX, radroots_core::Currency::USD).unwrap();
    let two_hundred = Percent::new(common::dec("200"));
    assert_eq!(
        two_hundred.try_of_money(&max),
        Err(money::Error::ArithmeticOverflow)
    );
}

#[test]
fn display_and_parse_error_messages_are_stable() {
    assert_eq!(Percent::from_str("12.5%").unwrap().to_string(), "12.5%");
    assert_eq!(
        ParseError::InvalidNumber.to_string(),
        "invalid percent string"
    );
}

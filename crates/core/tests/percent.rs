mod common;

use core::str::FromStr;

use radroots_core::{RadrootsCorePercent, RadrootsCorePercentParseError};

#[test]
fn ratio_roundtrip() {
    let pct = RadrootsCorePercent::from_ratio(common::dec("0.125"));
    assert_eq!(pct.value, common::dec("12.5"));
    assert_eq!(pct.to_ratio(), common::dec("0.125"));
}

#[test]
fn parses_percent_strings() {
    let pct = RadrootsCorePercent::from_str("12.5%").unwrap();
    assert_eq!(pct.value, common::dec("12.5"));

    let pct = RadrootsCorePercent::from_str(" 12.5 ").unwrap();
    assert_eq!(pct.value, common::dec("12.5"));

    assert_eq!(
        RadrootsCorePercent::from_str("nope"),
        Err(RadrootsCorePercentParseError::InvalidNumber)
    );
}

#[test]
fn of_money_and_quantized() {
    let base = common::money("20.00", "USD");
    let pct = RadrootsCorePercent::from_str("10").unwrap();
    let out = pct.of_money(&base);
    assert_eq!(out.amount, common::dec("2.00"));

    let tiny = common::money("0.05", "USD");
    let pct = RadrootsCorePercent::from_str("10").unwrap();
    let rounded = pct.of_money_quantized(&tiny);
    assert_eq!(rounded.amount, common::dec("0.01"));
}

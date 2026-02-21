mod common;

use core::str::FromStr;

use radroots_core::RadrootsCoreDecimal;

#[test]
fn display_normalizes_trailing_zeros() {
    let d = RadrootsCoreDecimal::from_str("1.2300").unwrap();
    assert_eq!(d.to_string(), "1.23");
}

#[test]
fn scale_reflects_input_precision() {
    let d = RadrootsCoreDecimal::from_str("1.2300").unwrap();
    assert_eq!(d.scale(), 4);
}

#[test]
fn to_u64_exact_requires_whole_number() {
    let whole = common::dec("42.0");
    let frac = common::dec("42.5");
    assert_eq!(whole.to_u64_exact(), Some(42));
    assert_eq!(frac.to_u64_exact(), None);
}

#[test]
fn from_f64_display_roundtrips_reasonably() {
    let d = RadrootsCoreDecimal::from_f64_display(1.25).unwrap();
    let v = d.to_f64_lossy().expect("f64 conversion");
    assert!((v - 1.25).abs() < 1e-12);
}

#[test]
fn from_str_exact_and_conversion_impl_paths_are_exercised() {
    let exact = RadrootsCoreDecimal::from_str_exact("42.000").unwrap();
    assert_eq!(exact, common::dec("42"));

    let from_decimal = RadrootsCoreDecimal::from(rust_decimal::Decimal::from(5u32));
    assert_eq!(from_decimal, common::dec("5"));
    let back: rust_decimal::Decimal = from_decimal.into();
    assert_eq!(back, rust_decimal::Decimal::from(5u32));

    let from_u32 = RadrootsCoreDecimal::from(7u32);
    let from_i32 = RadrootsCoreDecimal::from(-2i32);
    let from_u64 = RadrootsCoreDecimal::from(11u64);
    let from_i64 = RadrootsCoreDecimal::from(-9i64);
    assert_eq!(from_u32, common::dec("7"));
    assert_eq!(from_i32, common::dec("-2"));
    assert_eq!(from_u64, common::dec("11"));
    assert_eq!(from_i64, common::dec("-9"));
}

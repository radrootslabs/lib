mod common;

use core::str::FromStr;

use radroots_core::{Decimal, decimal::Error};

#[test]
fn display_normalizes_trailing_zeros() {
    let d = Decimal::from_str("1.2300").unwrap();
    assert_eq!(d.to_string(), "1.23");
}

#[test]
fn scale_reflects_input_precision() {
    let d = Decimal::from_str("1.2300").unwrap();
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
    let d = Decimal::from_f64_display(1.25).unwrap();
    let v = d.to_f64_lossy().expect("f64 conversion");
    assert!((v - 1.25).abs() < 1e-12);
}

#[test]
fn parsing_normalizes_zero_and_owns_parse_errors() {
    let zero = Decimal::from_str("-0.000").unwrap();
    assert_eq!(zero, Decimal::ZERO);
    assert_eq!(zero.scale(), 0);
    assert_eq!(
        Decimal::from_str("not-a-decimal"),
        Err(Error::InvalidFormat)
    );
    assert_eq!(
        Decimal::from_str_exact("0.00000000000000000000000000001"),
        Err(Error::OutOfRange)
    );
}

#[test]
fn checked_arithmetic_reports_overflow_and_zero_division() {
    assert_eq!(
        Decimal::MAX.checked_add(Decimal::ONE),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(
        Decimal::MIN.checked_sub(Decimal::ONE),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(
        Decimal::MAX.checked_mul(Decimal::from(2u32)),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(
        Decimal::ONE.checked_div(Decimal::ZERO),
        Err(Error::DivisionByZero)
    );
}

#[test]
fn exact_rescale_rejects_rounding_and_unrepresentable_scales() {
    let mut exact = Decimal::from_str("1.2300").unwrap();
    exact.try_rescale_exact(2).unwrap();
    assert_eq!(exact.scale(), 2);
    assert_eq!(exact, common::dec("1.23"));

    let mut would_round = common::dec("1.25");
    assert_eq!(would_round.try_rescale_exact(1), Err(Error::PrecisionLoss));
    assert_eq!(would_round, common::dec("1.25"));

    assert_eq!(
        would_round.try_rescale_exact(Decimal::MAX_SCALE + 1),
        Err(Error::ScaleOutOfRange)
    );
}

#[test]
fn display_parse_roundtrips_normalized_values() {
    for input in ["0", "-42.1250", "999999.000001", "0.00000001"] {
        let value = Decimal::from_str(input).unwrap();
        let reparsed = Decimal::from_str(&value.to_string()).unwrap();
        assert_eq!(reparsed, value);
    }
}

#[test]
fn float_conversion_rejects_non_finite_values() {
    assert_eq!(
        Decimal::from_f64_display(f64::NAN),
        Err(Error::InvalidFormat)
    );
    assert_eq!(
        Decimal::from_f64_display(f64::INFINITY),
        Err(Error::InvalidFormat)
    );
}

#[test]
fn from_str_exact_and_primitive_conversion_paths_are_exercised() {
    let exact = Decimal::from_str_exact("42.000").unwrap();
    assert_eq!(exact, common::dec("42"));

    let from_u32 = Decimal::from(7u32);
    let from_i32 = Decimal::from(-2i32);
    let from_u64 = Decimal::from(11u64);
    let from_i64 = Decimal::from(-9i64);
    assert_eq!(from_u32, common::dec("7"));
    assert_eq!(from_i32, common::dec("-2"));
    assert_eq!(from_u64, common::dec("11"));
    assert_eq!(from_i64, common::dec("-9"));
}

#[cfg(feature = "serde")]
#[test]
fn serde_deserialize_error_paths_are_exercised() {
    let parse_err = serde_json::from_str::<Decimal>("\"not-a-decimal\"").unwrap_err();
    assert!(!parse_err.to_string().is_empty());
    let non_string_err = serde_json::from_str::<Decimal>("123").unwrap_err();
    assert!(non_string_err.to_string().contains("invalid type"));
}

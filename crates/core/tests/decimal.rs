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

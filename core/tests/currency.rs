use core::str::FromStr;

use radroots_core::{RadrootsCoreCurrency, RadrootsCoreCurrencyParseError};

#[test]
fn from_str_upper_accepts_valid() {
    let usd = RadrootsCoreCurrency::from_str_upper("USD").unwrap();
    assert_eq!(usd.as_str(), "USD");
}

#[test]
fn from_str_upper_rejects_invalid() {
    assert_eq!(
        RadrootsCoreCurrency::from_str_upper("Usd"),
        Err(RadrootsCoreCurrencyParseError::InvalidFormat)
    );
    assert_eq!(
        RadrootsCoreCurrency::from_str_upper("US"),
        Err(RadrootsCoreCurrencyParseError::InvalidFormat)
    );
}

#[test]
fn from_str_trims_and_uppercases() {
    let usd = RadrootsCoreCurrency::from_str(" usd ").unwrap();
    assert_eq!(usd.as_str(), "USD");
}

#[test]
fn from_str_rejects_non_alpha() {
    assert_eq!(
        RadrootsCoreCurrency::from_str("US1"),
        Err(RadrootsCoreCurrencyParseError::InvalidFormat)
    );
}

#[test]
fn from_const_validates_bytes() {
    assert!(RadrootsCoreCurrency::from_const(*b"USD").is_ok());
    assert_eq!(
        RadrootsCoreCurrency::from_const(*b"usd"),
        Err(RadrootsCoreCurrencyParseError::InvalidFormat)
    );
}

#[test]
fn minor_unit_exponent_matches_known_currencies() {
    let jpy = RadrootsCoreCurrency::from_str_upper("JPY").unwrap();
    let kwd = RadrootsCoreCurrency::from_str_upper("KWD").unwrap();
    let usd = RadrootsCoreCurrency::from_str_upper("USD").unwrap();

    assert_eq!(jpy.minor_unit_exponent(), 0);
    assert_eq!(kwd.minor_unit_exponent(), 3);
    assert_eq!(usd.minor_unit_exponent(), 2);
}

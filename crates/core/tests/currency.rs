use core::str::FromStr;

use radroots_core::{Currency, currency::ParseError};

#[test]
fn from_str_upper_accepts_valid() {
    let usd = Currency::from_str_upper("USD").unwrap();
    assert_eq!(usd.as_str(), "USD");
}

#[test]
fn from_str_upper_rejects_invalid() {
    assert_eq!(
        Currency::from_str_upper("Usd"),
        Err(ParseError::InvalidFormat)
    );
    assert_eq!(
        Currency::from_str_upper("US"),
        Err(ParseError::InvalidFormat)
    );
}

#[test]
fn from_str_trims_and_uppercases() {
    let usd = Currency::from_str(" usd ").unwrap();
    assert_eq!(usd.as_str(), "USD");
}

#[test]
fn from_str_rejects_non_alpha() {
    assert_eq!(Currency::from_str("US1"), Err(ParseError::InvalidFormat));
    assert_eq!(Currency::from_str("US"), Err(ParseError::InvalidFormat));
}

#[test]
fn from_const_validates_bytes() {
    assert!(Currency::from_const(*b"USD").is_ok());
    assert_eq!(
        Currency::from_const(*b"1SD"),
        Err(ParseError::InvalidFormat)
    );
    assert_eq!(
        Currency::from_const(*b"Usd"),
        Err(ParseError::InvalidFormat)
    );
    assert_eq!(
        Currency::from_const(*b"USd"),
        Err(ParseError::InvalidFormat)
    );
    assert_eq!(
        Currency::from_const(*b"usd"),
        Err(ParseError::InvalidFormat)
    );
}

#[test]
fn minor_unit_exponent_matches_known_currencies() {
    let jpy = Currency::from_str_upper("JPY").unwrap();
    let kwd = Currency::from_str_upper("KWD").unwrap();
    let usd = Currency::from_str_upper("USD").unwrap();

    assert_eq!(jpy.minor_unit_exponent(), 0);
    assert_eq!(kwd.minor_unit_exponent(), 3);
    assert_eq!(usd.minor_unit_exponent(), 2);
}

#[test]
fn display_debug_tryfrom_and_error_display_paths_are_exercised() {
    let usd = Currency::from_str("usd").unwrap();
    assert_eq!(usd.to_string(), "USD");
    assert_eq!(format!("{usd:?}"), "Currency(\"USD\")");
    let via_try_from = Currency::try_from("usd").unwrap();
    assert_eq!(via_try_from, usd);
    assert_eq!(
        ParseError::InvalidFormat.to_string(),
        "currency must be a 3-letter code"
    );
}

#[cfg(feature = "serde")]
#[test]
fn serde_deserialize_paths_are_exercised() {
    let parsed: Currency = serde_json::from_str("\"USD\"").unwrap();
    assert_eq!(parsed.as_str(), "USD");
    let err = serde_json::from_str::<Currency>("\"US1\"").unwrap_err();
    assert!(err.to_string().contains("currency must be a 3-letter code"));
    let non_string_err = serde_json::from_str::<Currency>("123").unwrap_err();
    assert!(non_string_err.to_string().contains("invalid type"));
}

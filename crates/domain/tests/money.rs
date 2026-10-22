use radroots_domain::{
    CURRENCY_REGISTRY_ID, CURRENCY_REGISTRY_SHA256, Currency, CurrencyError, MinorAmount, Money,
    MoneyError, SUPPORTED_CURRENCY_COUNT,
};
use std::{collections::BTreeSet, error::Error};

fn money(code: &str, value: &str, exponent: u8) -> Money {
    Money::from_parts(code, value, exponent).unwrap()
}

#[test]
fn complete_pinned_registry_preserves_each_code_and_exponent_without_defaults() {
    let expected = include_str!("../data/currency_registry_v1.csv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let (code, exponent) = line.split_once(',').unwrap();
            (code, exponent.parse::<u8>().unwrap())
        })
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 165);
    assert_eq!(SUPPORTED_CURRENCY_COUNT, expected.len());
    assert_eq!(CURRENCY_REGISTRY_ID, "radroots.currency_registry.v1");
    assert_eq!(CURRENCY_REGISTRY_SHA256.len(), 64);
    let actual = Currency::supported()
        .map(|currency| (currency.code(), currency.exponent()))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(
        actual
            .iter()
            .map(|row| row.0)
            .collect::<BTreeSet<_>>()
            .len(),
        165
    );
    for (code, exponent) in actual {
        let currency: Currency = code.parse().unwrap();
        assert_eq!(currency.code(), code);
        assert_eq!(currency.exponent(), exponent);
        let value = money(code, "1", exponent);
        assert_eq!(value.currency(), currency);
        for other in 0..=4 {
            if other != exponent {
                assert_eq!(
                    Money::from_parts(code, "1", other),
                    Err(MoneyError::ExponentMismatch)
                );
            }
        }
    }
}

#[test]
fn independent_iso_minor_units_are_distinct_from_display_rounding() {
    for (code, exponent) in [
        ("CAD", 2),
        ("JPY", 0),
        ("KWD", 3),
        ("CLF", 4),
        ("UYW", 4),
        ("AFN", 2),
        ("ALL", 2),
        ("COP", 2),
        ("HUF", 2),
        ("XAD", 2),
    ] {
        assert_eq!(Currency::from_code(code).unwrap().exponent(), exponent);
    }
}

#[test]
fn currency_admission_is_ascii_canonical_bounded_and_supported_only() {
    for input in [
        "",
        "US",
        "USDD",
        "usd",
        "Usd",
        " USD",
        "USD ",
        "12A",
        "éA",
        "ＵＳＤ",
        "A\0B",
    ] {
        assert_eq!(
            Currency::from_code(input),
            Err(CurrencyError::InvalidFormat)
        );
    }
    assert_eq!(
        Currency::from_code(&"A".repeat(100_000)),
        Err(CurrencyError::InvalidFormat)
    );
    for input in [
        "ZZZ", "XAG", "XAU", "XBA", "XBB", "XBC", "XBD", "XDR", "XPD", "XPT", "XSU", "XTS", "XUA",
        "XXX", "BGN", "ANG",
    ] {
        assert_eq!(
            Currency::from_code(input),
            Err(CurrencyError::UnsupportedCode)
        );
    }
}

#[test]
fn minor_amount_strings_and_numeric_construction_are_exact_at_both_bounds() {
    for (text, expected) in [
        ("0", 0),
        ("1", 1),
        ("900719925474099", 900_719_925_474_099),
        ("999999999999999", 999_999_999_999_999),
    ] {
        let amount: MinorAmount = text.parse().unwrap();
        assert_eq!(amount.get(), expected);
        assert_eq!(amount.to_string(), text);
        assert_eq!(MinorAmount::try_from(expected).unwrap(), amount);
    }
    assert_eq!(MinorAmount::ZERO.get(), 0);
    assert_eq!(MinorAmount::MAX.get(), MinorAmount::MAX_VALUE);
    assert_eq!(
        MinorAmount::new(1_000_000_000_000_000),
        Err(MoneyError::MinorOutOfRange)
    );
    assert_eq!(MinorAmount::new(u64::MAX), Err(MoneyError::MinorOutOfRange));
    for input in [
        "", "-0", "-1", "+1", "1.0", "1e2", " 1", "1 ", "1\n", "1,000", "1_000", "１", "١", "NaN",
    ] {
        assert_eq!(
            input.parse::<MinorAmount>(),
            Err(MoneyError::InvalidMinorFormat)
        );
    }
    for input in ["00", "01", "000000000000000"] {
        assert_eq!(
            input.parse::<MinorAmount>(),
            Err(MoneyError::NonCanonicalMinor)
        );
    }
    for input in ["1000000000000000", "9999999999999999", &"9".repeat(100_000)] {
        assert_eq!(
            input.parse::<MinorAmount>(),
            Err(MoneyError::MinorOutOfRange)
        );
    }
}

#[test]
fn money_checks_explicit_exponents_and_never_changes_currency_or_price_mode() {
    for (code, exponent) in [("JPY", 0), ("CAD", 2), ("KWD", 3), ("CLF", 4)] {
        let value = money(code, "325", exponent);
        assert_eq!(value.currency().code(), code);
        assert_eq!(value.exponent(), exponent);
        assert_eq!(value.minor().get(), 325);
        assert_eq!(Money::new(value.currency(), value.minor()), value);
        assert_eq!(
            Money::from_parts(code, "325", 5),
            Err(MoneyError::InvalidExponent)
        );
        assert_eq!(
            Money::from_parts(code, "325", u8::MAX),
            Err(MoneyError::InvalidExponent)
        );
    }
    assert_eq!(money("CAD", "0", 2).minor(), MinorAmount::ZERO);
    assert_eq!(
        Money::from_parts("XXX", "0", 0),
        Err(MoneyError::Currency(CurrencyError::UnsupportedCode))
    );
    assert_eq!(
        Money::from_parts("cad", "0", 2),
        Err(MoneyError::Currency(CurrencyError::InvalidFormat))
    );
    assert_eq!(
        Money::from_parts("CAD", "0.1", 2),
        Err(MoneyError::InvalidMinorFormat)
    );
}

#[test]
fn checked_money_arithmetic_preserves_bounds_and_refuses_cross_currency_operations() {
    let zero = money("CAD", "0", 2);
    let one = money("CAD", "1", 2);
    let maximum = money("CAD", "999999999999999", 2);
    assert_eq!(maximum.checked_add(zero).unwrap(), maximum);
    assert_eq!(maximum.checked_sub(zero).unwrap(), maximum);
    assert_eq!(maximum.checked_sub(maximum).unwrap(), zero);
    assert_eq!(
        money("CAD", "998", 2).checked_add(one).unwrap(),
        money("CAD", "999", 2)
    );
    assert_eq!(maximum.checked_add(one), Err(MoneyError::MinorOutOfRange));
    assert_eq!(
        maximum.checked_add(maximum),
        Err(MoneyError::MinorOutOfRange)
    );
    assert_eq!(zero.checked_sub(one), Err(MoneyError::NegativeResult));
    for other in [
        money("USD", "1", 2),
        money("JPY", "1", 0),
        money("CLF", "1", 4),
    ] {
        assert_eq!(one.checked_add(other), Err(MoneyError::CurrencyMismatch));
        assert_eq!(one.checked_sub(other), Err(MoneyError::CurrencyMismatch));
    }
}

#[test]
fn diagnostics_are_fixed_and_money_values_are_redacted() {
    let value = money("CAD", "928174615", 2);
    for debug in [
        format!("{value:?}"),
        format!("{:?}", value.minor()),
        format!("{:?}", value.currency()),
    ] {
        assert!(!debug.contains("928174615"));
        assert!(!debug.contains("CAD"));
    }
    for error in [
        MoneyError::InvalidMinorFormat,
        MoneyError::NonCanonicalMinor,
        MoneyError::MinorOutOfRange,
        MoneyError::InvalidExponent,
        MoneyError::ExponentMismatch,
        MoneyError::CurrencyMismatch,
        MoneyError::NegativeResult,
        CurrencyError::InvalidFormat.into(),
        CurrencyError::UnsupportedCode.into(),
    ] {
        assert!(!error.to_string().is_empty());
        assert!(!error.to_string().contains("928174615"));
        assert!(!error.to_string().contains("CAD"));
    }
    assert!(
        MoneyError::Currency(CurrencyError::UnsupportedCode)
            .source()
            .is_some()
    );
    assert!(MoneyError::ExponentMismatch.source().is_none());
}

#[cfg(feature = "serde")]
#[test]
fn money_serde_is_closed_duplicate_safe_and_validates_every_explicit_field() {
    let value = money("CAD", "999999999999999", 2);
    let encoded = serde_json::to_string(&value).unwrap();
    assert_eq!(
        encoded,
        r#"{"currency":"CAD","minor":"999999999999999","exponent":2}"#
    );
    assert_eq!(serde_json::from_str::<Money>(&encoded).unwrap(), value);
    assert_eq!(
        serde_json::to_string(&value.currency()).unwrap(),
        r#""CAD""#
    );
    assert_eq!(
        serde_json::to_string(&value.minor()).unwrap(),
        r#""999999999999999""#
    );
    for input in [
        r#"{"currency":"CAD","minor":325,"exponent":2}"#,
        r#"{"currency":"CAD","minor":"0325","exponent":2}"#,
        r#"{"currency":"CAD","minor":"1000000000000000","exponent":2}"#,
        r#"{"currency":"CAD","minor":"325","exponent":0}"#,
        r#"{"currency":"CAD","minor":"325","exponent":5}"#,
        r#"{"currency":"CAD","minor":"325","exponent":256}"#,
        r#"{"currency":"CAD","minor":"325","exponent":-1}"#,
        r#"{"currency":"CAD","minor":"325","exponent":"2"}"#,
        r#"{"currency":"CAD","minor":"325","exponent":2.0}"#,
        r#"{"currency":"XXX","minor":"0","exponent":0}"#,
        r#"{"currency":"CAD","minor":"0","exponent":2,"extra":null}"#,
        r#"{"currency":"CAD","currency":"USD","minor":"0","exponent":2}"#,
        r#"{"currency":"CAD","minor":"0","minor":"1","exponent":2}"#,
        r#"{"currency":"CAD","minor":"0","exponent":2,"exponent":2}"#,
        r#"{"currency":"CAD","minor":"0"}"#,
        r#"{"currency":"CAD","exponent":2}"#,
        r#"{"minor":"0","exponent":2}"#,
        r#"[]"#,
        r#"["CAD","325",2]"#,
        r#"null"#,
    ] {
        assert!(serde_json::from_str::<Money>(input).is_err(), "{input}");
    }
    for input in ["325", "325.0", "null", r#""1.5""#, r#""-1""#] {
        assert!(serde_json::from_str::<MinorAmount>(input).is_err());
    }
    for input in ["325", "null", r#""cad""#, r#""ZZZ""#] {
        assert!(serde_json::from_str::<Currency>(input).is_err());
    }
}

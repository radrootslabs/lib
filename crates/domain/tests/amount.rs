use radroots_domain::{AmountError, Decimal, PositiveDecimal};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

fn decimal(input: &str) -> Decimal {
    input.parse().unwrap()
}

#[test]
fn canonical_hand_vectors_round_trip_without_float_conversion() {
    for (text, coefficient, scale) in [
        ("0", 0, 0),
        ("1", 1, 0),
        ("0.000001", 1, 6),
        ("999999999999", 999_999_999_999, 0),
        ("999999.999999", 999_999_999_999, 6),
        ("12.345", 12_345, 3),
        ("0.01", 1, 2),
    ] {
        let value = decimal(text);
        assert_eq!(value.coefficient(), coefficient, "{text}");
        assert_eq!(value.scale(), scale, "{text}");
        assert_eq!(value.to_string(), text);
        assert_eq!(Decimal::from_parts(coefficient, scale).unwrap(), value);
    }
    assert_eq!(Decimal::MAX, decimal("999999999999"));
    assert_eq!(Decimal::ONE, decimal("1"));
}

#[test]
fn construction_normalizes_fractional_zeros_and_zero() {
    for scale in 0..=Decimal::MAX_SCALE {
        assert_eq!(Decimal::from_parts(0, scale).unwrap(), Decimal::ZERO);
    }
    assert_eq!(Decimal::from_parts(123_000, 4).unwrap(), decimal("12.3"));
    assert_eq!(Decimal::from_parts(100_000, 5).unwrap(), Decimal::ONE);
    assert_eq!(Decimal::from_parts(120, 0).unwrap().coefficient(), 120);
}

#[test]
fn constructor_checks_both_input_bounds_before_normalization() {
    assert_eq!(
        Decimal::from_parts(1_000_000_000_000, 1),
        Err(AmountError::CoefficientOutOfRange)
    );
    assert_eq!(
        Decimal::from_parts(u64::MAX, 0),
        Err(AmountError::CoefficientOutOfRange)
    );
    assert_eq!(Decimal::from_parts(0, 7), Err(AmountError::ScaleOutOfRange));
    assert_eq!(
        Decimal::from_parts(1, u8::MAX),
        Err(AmountError::ScaleOutOfRange)
    );
}

#[test]
fn strings_require_canonical_unsigned_ascii_spelling() {
    for input in [
        "", "-0", "-1", "+1", ".1", "1.", "1..2", "1e2", "1E2", " 1", "1 ", "1\n", "1\t", "1,000",
        "1_000", "１", "١", "NaN", "Infinity",
    ] {
        assert!(input.parse::<Decimal>().is_err(), "{input:?}");
    }
    for input in ["00", "01", "00.1", "0.0", "1.0", "1.2300", "0.000000"] {
        assert_eq!(
            input.parse::<Decimal>(),
            Err(AmountError::NonCanonical),
            "{input}"
        );
    }
}

#[test]
fn combined_coefficient_and_scale_are_semantic_admission_bounds() {
    // These fit the syntax's integer/fraction widths but exceed the combined bound.
    for input in ["999999999999.1", "9999999.999999", "1000000000000"] {
        assert_eq!(
            input.parse::<Decimal>(),
            Err(AmountError::CoefficientOutOfRange),
            "{input}"
        );
    }
    assert_eq!(
        "0.0000001".parse::<Decimal>(),
        Err(AmountError::ScaleOutOfRange)
    );
}

#[test]
fn overlong_inputs_fail_with_constant_redacted_diagnostics() {
    let input = "sensitive_fixture_".repeat(4096);
    let error = input.parse::<Decimal>().unwrap_err();
    assert_eq!(error, AmountError::InvalidFormat);
    assert_eq!(error.to_string(), "invalid decimal format");
    assert!(!format!("{error:?}").contains("sensitive_fixture"));
    assert_eq!(
        "9".repeat(Decimal::MAX_ENCODED_LEN + 1).parse::<Decimal>(),
        Err(AmountError::InvalidFormat)
    );
}

#[test]
fn positivity_cannot_be_bypassed_by_construction_or_parsing() {
    assert_eq!(
        PositiveDecimal::new(Decimal::ZERO),
        Err(AmountError::MustBePositive)
    );
    assert_eq!(
        PositiveDecimal::try_from(Decimal::ZERO),
        Err(AmountError::MustBePositive)
    );
    assert_eq!(
        "0".parse::<PositiveDecimal>(),
        Err(AmountError::MustBePositive)
    );
    assert!("-1".parse::<PositiveDecimal>().is_err());
    for text in ["0.000001", "1", "999999999999"] {
        let positive: PositiveDecimal = text.parse().unwrap();
        assert_eq!(positive.amount(), decimal(text));
        assert_eq!(Decimal::from(positive), decimal(text));
        assert_eq!(positive.to_string(), text);
    }
}

#[test]
fn ordering_is_numeric_across_scales_and_hashing_uses_canonical_parts() {
    let ascending = [
        "0",
        "0.000001",
        "0.1",
        "0.99",
        "1",
        "1.01",
        "2",
        "10",
        "999999999999",
    ];
    for pair in ascending.windows(2) {
        assert!(decimal(pair[0]) < decimal(pair[1]));
        assert!(decimal(pair[1]) > decimal(pair[0]));
    }
    let canonical = decimal("1.23");
    let normalized = Decimal::from_parts(123_000, 5).unwrap();
    assert_eq!(canonical, normalized);
    assert_eq!(
        canonical.partial_cmp(&normalized),
        Some(core::cmp::Ordering::Equal)
    );
    let hash = |v: Decimal| {
        let mut h = DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(canonical), hash(normalized));
    assert!("0.1".parse::<PositiveDecimal>().unwrap() < "1".parse().unwrap());
}

#[test]
fn addition_and_subtraction_preserve_exact_results_across_scales() {
    for (left, right, sum) in [
        ("0.1", "0.2", "0.3"),
        ("1", "0.000001", "1.000001"),
        ("999999999998", "1", "999999999999"),
        ("0.999999", "0.000001", "1"),
        ("50000000000", "50000000000", "100000000000"),
        ("0", "1.23", "1.23"),
    ] {
        let (left, right, sum) = (decimal(left), decimal(right), decimal(sum));
        assert_eq!(left.checked_add(right).unwrap(), sum);
        assert_eq!(right.checked_add(left).unwrap(), sum);
        assert_eq!(sum.checked_sub(left).unwrap(), right);
        assert_eq!(sum.checked_sub(right).unwrap(), left);
    }
    assert_eq!(Decimal::MAX.checked_sub(Decimal::MAX), Ok(Decimal::ZERO));
}

#[test]
fn arithmetic_rejects_negative_or_unrepresentable_results_without_rounding() {
    assert_eq!(
        Decimal::ZERO.checked_sub(Decimal::ONE),
        Err(AmountError::NegativeResult)
    );
    assert_eq!(
        Decimal::MAX.checked_add(Decimal::ONE),
        Err(AmountError::CoefficientOutOfRange)
    );
    assert_eq!(
        Decimal::MAX.checked_add(decimal("0.000001")),
        Err(AmountError::CoefficientOutOfRange)
    );
    assert_eq!(
        Decimal::MAX.checked_sub(decimal("0.000001")),
        Err(AmountError::CoefficientOutOfRange)
    );
    assert_eq!(
        decimal("999999999998").checked_sub(Decimal::MAX),
        Err(AmountError::NegativeResult)
    );
}

#[test]
fn bounded_small_grid_matches_independent_integer_micro_units() {
    // Independent integer oracle in millionths; no production parsing/arithmetic helpers.
    for left in 0..=20_u64 {
        for right in 0..=20_u64 {
            for scale in 0..=6_u8 {
                let a = Decimal::from_parts(left, scale).unwrap();
                let b = Decimal::from_parts(right, 6 - scale).unwrap();
                let expected =
                    left * 10_u64.pow(u32::from(6 - scale)) + right * 10_u64.pow(u32::from(scale));
                let sum = a.checked_add(b).unwrap();
                assert_eq!(
                    sum.coefficient() * 10_u64.pow(u32::from(6 - sum.scale())),
                    expected
                );
                assert_eq!(sum.checked_sub(a).unwrap(), b);
            }
        }
    }
}

#[test]
fn all_error_messages_are_static_bounded_and_distinct() {
    let errors = [
        AmountError::InvalidFormat,
        AmountError::NonCanonical,
        AmountError::CoefficientOutOfRange,
        AmountError::ScaleOutOfRange,
        AmountError::MustBePositive,
        AmountError::NegativeResult,
    ];
    let messages = errors.map(|e| e.to_string());
    for (i, message) in messages.iter().enumerate() {
        assert!(!message.is_empty() && message.len() <= 64);
        assert!(!messages[..i].contains(message));
    }
}

#[cfg(feature = "serde")]
mod json {
    use super::*;

    #[test]
    fn decimal_and_positive_values_round_trip_only_as_canonical_strings() {
        for text in ["0", "0.000001", "999999999999", "999999.999999"] {
            let value = decimal(text);
            let encoded = serde_json::to_string(&value).unwrap();
            assert_eq!(encoded, format!("\"{text}\""));
            assert_eq!(serde_json::from_str::<Decimal>(&encoded).unwrap(), value);
            if !value.is_zero() {
                let positive = PositiveDecimal::new(value).unwrap();
                assert_eq!(serde_json::to_string(&positive).unwrap(), encoded);
                assert_eq!(
                    serde_json::from_str::<PositiveDecimal>(&encoded).unwrap(),
                    positive
                );
            }
        }
    }

    #[test]
    fn numbers_objects_and_noncanonical_values_cannot_bypass_admission() {
        for input in [
            "1",
            "1.5",
            "null",
            "true",
            "[]",
            "{}",
            "{\"coefficient\":1,\"scale\":0}",
            "\"-0\"",
            "\"01\"",
            "\"1.0\"",
            "\"1e2\"",
            "\"999999999999.1\"",
            "\"0.0000001\"",
        ] {
            assert!(serde_json::from_str::<Decimal>(input).is_err(), "{input}");
            assert!(
                serde_json::from_str::<PositiveDecimal>(input).is_err(),
                "{input}"
            );
        }
        assert!(serde_json::from_str::<PositiveDecimal>("\"0\"").is_err());
    }
}

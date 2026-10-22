use radroots_domain::listing::MeasurementUnit::{self, Each, Gram, Kilogram, Litre, Millilitre};
use radroots_domain::{AmountError, ContentMeasure, Decimal, Quantity, QuantityError};
use std::error::Error;

fn quantity(amount: &str, unit: MeasurementUnit) -> Quantity {
    Quantity::new(amount.parse().unwrap(), unit).unwrap()
}

#[test]
fn explicit_si_conversions_match_independent_canonical_examples_in_both_dimensions() {
    for (source, target) in [(Gram, Kilogram), (Millilitre, Litre)] {
        for (small, large) in [
            ("0", "0"),
            ("1", "0.001"),
            ("1000", "1"),
            ("1250", "1.25"),
            ("0.001", "0.000001"),
            ("999999999999", "999999999.999"),
        ] {
            let small = quantity(small, source);
            let large = quantity(large, target);
            assert_eq!(small.convert(target).unwrap(), large);
            assert_eq!(large.convert(source).unwrap(), small);
            assert_eq!(small.convert(source).unwrap(), small);
            assert_eq!(large.convert(target).unwrap(), large);
        }
    }
    assert_eq!(
        quantity("0.000001", Kilogram).convert(Gram).unwrap(),
        quantity("0.001", Gram)
    );
    assert_eq!(
        quantity("0.000001", Litre).convert(Millilitre).unwrap(),
        quantity("0.001", Millilitre)
    );
}

#[test]
fn exact_conversions_refuse_rounding_and_normalized_amount_overflow() {
    for (source, target) in [(Gram, Kilogram), (Millilitre, Litre)] {
        for amount in ["0.000001", "0.00001", "0.0001", "1.000001"] {
            assert_eq!(
                quantity(amount, source).convert(target),
                Err(QuantityError::PrecisionLoss)
            );
        }
        assert_eq!(
            quantity("999999999999", target).convert(source),
            Err(QuantityError::Amount(AmountError::CoefficientOutOfRange))
        );
        // The exact largest output remains representable after conversion.
        assert_eq!(
            quantity("999999999.999", target)
                .convert(source)
                .unwrap()
                .amount(),
            Decimal::MAX
        );
    }
}

#[test]
fn every_unit_pair_preserves_explicit_dimensions_even_for_zero() {
    let units = [Gram, Kilogram, Millilitre, Litre, Each];
    for source in units {
        for target in units {
            let compatible = source == target
                || matches!(
                    (source, target),
                    (Gram, Kilogram) | (Kilogram, Gram) | (Millilitre, Litre) | (Litre, Millilitre)
                );
            for amount in ["0", "1"] {
                let result = quantity(amount, source).convert(target);
                if compatible {
                    let converted = result.unwrap();
                    assert_eq!(converted.unit(), target);
                    assert_eq!(converted.convert(source).unwrap(), quantity(amount, source));
                } else {
                    assert_eq!(result, Err(QuantityError::IncompatibleDimension));
                }
            }
        }
    }
}

#[test]
fn each_counts_use_semantic_normalization_but_cannot_be_fractional() {
    for amount in ["0", "1", "999999999999"] {
        let value = quantity(amount, Each);
        assert_eq!(value.amount().to_string(), amount);
        assert_eq!(value.unit(), Each);
    }
    assert_eq!(
        Quantity::new(Decimal::from_parts(10, 1).unwrap(), Each).unwrap(),
        quantity("1", Each)
    );
    for amount in ["0.000001", "0.5", "1.5"] {
        assert_eq!(
            Quantity::new(amount.parse().unwrap(), Each),
            Err(QuantityError::FractionalCount)
        );
        for unit in [Gram, Kilogram, Millilitre, Litre] {
            assert!(Quantity::new(amount.parse().unwrap(), unit).is_ok());
        }
    }
}

#[test]
fn arithmetic_requires_matching_units_and_keeps_decimal_bounds_exact() {
    let one = quantity("1", Kilogram);
    let quarter = quantity("250", Gram);
    assert_eq!(one.checked_add(quarter), Err(QuantityError::UnitMismatch));
    assert_eq!(one.checked_sub(quarter), Err(QuantityError::UnitMismatch));
    let quarter = quarter.convert(Kilogram).unwrap();
    assert_eq!(
        one.checked_add(quarter).unwrap(),
        quantity("1.25", Kilogram)
    );
    assert_eq!(
        one.checked_sub(quarter).unwrap(),
        quantity("0.75", Kilogram)
    );
    assert_eq!(one.checked_sub(one).unwrap(), quantity("0", Kilogram));
    assert_eq!(
        quarter.checked_sub(one),
        Err(QuantityError::Amount(AmountError::NegativeResult))
    );
    assert_eq!(
        quantity("999999999999", Each).checked_add(quantity("1", Each)),
        Err(QuantityError::Amount(AmountError::CoefficientOutOfRange))
    );
}

#[test]
fn fixed_estimated_and_unspecified_contents_never_collapse_into_one_claim() {
    let amount = quantity("1.25", Kilogram);
    let fixed = ContentMeasure::Fixed { quantity: amount };
    let estimated = ContentMeasure::Estimated { quantity: amount };
    assert_ne!(fixed, estimated);
    assert_eq!(fixed.fixed_quantity(), Some(amount));
    assert_eq!(estimated.fixed_quantity(), None);
    assert_eq!(ContentMeasure::Unspecified.fixed_quantity(), None);
    let explicit_zero = ContentMeasure::Fixed {
        quantity: quantity("0", Gram),
    };
    assert_ne!(explicit_zero, ContentMeasure::Unspecified);
}

#[test]
fn diagnostics_redact_physical_values_and_preserve_typed_fixed_failures() {
    let value = quantity("928174615", Kilogram);
    for debug in [
        format!("{value:?}"),
        format!("{:?}", ContentMeasure::Fixed { quantity: value }),
        format!("{:?}", ContentMeasure::Estimated { quantity: value }),
    ] {
        assert!(!debug.contains("928174615"));
        assert!(!debug.contains("Kilogram"));
    }
    for error in [
        QuantityError::FractionalCount,
        QuantityError::UnitMismatch,
        QuantityError::IncompatibleDimension,
        QuantityError::PrecisionLoss,
        QuantityError::ArithmeticOutOfRange,
        AmountError::CoefficientOutOfRange.into(),
    ] {
        assert!(!error.to_string().is_empty());
        assert!(!error.to_string().contains("928174615"));
    }
    assert!(QuantityError::PrecisionLoss.source().is_none());
    assert!(
        QuantityError::Amount(AmountError::NegativeResult)
            .source()
            .is_some()
    );
}

#[cfg(feature = "serde")]
#[test]
fn serialization_preserves_exact_strings_units_and_explicit_content_alternatives() {
    let value = quantity("0.000001", Kilogram);
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        serde_json::json!({"amount":"0.000001","unit":"kg"})
    );
    assert_eq!(
        serde_json::to_value(ContentMeasure::Fixed { quantity: value }).unwrap(),
        serde_json::json!({"type":"fixed","quantity":{"amount":"0.000001","unit":"kg"}})
    );
    assert_eq!(
        serde_json::to_value(ContentMeasure::Estimated { quantity: value }).unwrap(),
        serde_json::json!({"type":"estimated","quantity":{"amount":"0.000001","unit":"kg"}})
    );
    assert_eq!(
        serde_json::to_value(ContentMeasure::Unspecified).unwrap(),
        serde_json::json!({"type":"unspecified"})
    );
}

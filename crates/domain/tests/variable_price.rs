use radroots_domain::listing::MeasurementUnit::{self, Each, Gram, Kilogram, Litre};
use radroots_domain::{Money, MoneyError, PricingError, Quantity, VariableWeight};
use std::error::Error;

fn money(value: &str) -> Money {
    Money::from_parts("CAD", value, 2).unwrap()
}
fn quantity(value: &str, unit: MeasurementUnit) -> Quantity {
    Quantity::new(value.parse().unwrap(), unit).unwrap()
}
fn bounds(minimum: &str, maximum: &str, rate: &str, basis: &str, cap: &str) -> VariableWeight {
    VariableWeight::new(
        Kilogram,
        minimum.parse().unwrap(),
        maximum.parse().unwrap(),
        money(rate),
        basis.parse().unwrap(),
        money(cap),
    )
    .unwrap()
}

#[test]
fn inclusive_measurement_bounds_recompute_exact_minor_prices_with_the_shared_rule() {
    let value = bounds("0.8", "1.2", "325", "1", "400");
    assert_eq!(value.measurement_unit(), Kilogram);
    assert_eq!(value.minimum().to_string(), "0.8");
    assert_eq!(value.maximum().to_string(), "1.2");
    assert_eq!(value.rate(), money("325"));
    assert_eq!(value.basis().to_string(), "1");
    assert_eq!(value.maximum_total(), money("400"));
    for (measurement, expected) in [
        ("0.8", "260"),
        ("0.9", "292"),
        ("0.98", "318"),
        ("1.02", "332"),
        ("1.2", "390"),
    ] {
        assert_eq!(
            value.total_at(quantity(measurement, Kilogram)).unwrap(),
            money(expected)
        );
    }
}

#[test]
fn final_measurements_cannot_silently_convert_units_or_extend_the_range() {
    let value = bounds("0.8", "1.2", "325", "1", "400");
    for measurement in ["0", "0.799999", "1.200001", "999999999999"] {
        assert_eq!(
            value.total_at(quantity(measurement, Kilogram)),
            Err(PricingError::MeasurementOutOfRange)
        );
    }
    for measurement in [
        quantity("800", Gram),
        quantity("1", Litre),
        quantity("1", Each),
    ] {
        assert_eq!(
            value.total_at(measurement),
            Err(PricingError::MeasurementUnitMismatch)
        );
    }
    assert_eq!(
        value
            .total_at(quantity("800", Gram).convert(Kilogram).unwrap())
            .unwrap(),
        money("260")
    );
}

#[test]
fn cap_and_physical_range_are_joint_constraints_without_implicit_amendment() {
    let value = bounds("0.8", "1.2", "325", "1", "300");
    assert_eq!(
        value.total_at(quantity("0.8", Kilogram)).unwrap(),
        money("260")
    );
    assert_eq!(
        value.total_at(quantity("0.9", Kilogram)).unwrap(),
        money("292")
    );
    assert_eq!(
        value.total_at(quantity("1", Kilogram)),
        Err(PricingError::CapExceeded)
    );
    assert_eq!(
        value.total_at(quantity("1.2", Kilogram)),
        Err(PricingError::CapExceeded)
    );
    assert_eq!(value.maximum().to_string(), "1.2");
    assert_eq!(value.maximum_total(), money("300"));
    assert_eq!(
        VariableWeight::new(
            Kilogram,
            "0.8".parse().unwrap(),
            "1.2".parse().unwrap(),
            money("325"),
            "1".parse().unwrap(),
            money("259")
        ),
        Err(PricingError::CapExceeded)
    );
    assert!(
        VariableWeight::new(
            Kilogram,
            "0.8".parse().unwrap(),
            "1.2".parse().unwrap(),
            money("325"),
            "1".parse().unwrap(),
            money("260")
        )
        .is_ok()
    );
}

#[test]
fn constructors_validate_order_count_units_and_exact_currency_before_capture() {
    assert_eq!(
        VariableWeight::new(
            Kilogram,
            "1.2".parse().unwrap(),
            "0.8".parse().unwrap(),
            money("325"),
            "1".parse().unwrap(),
            money("400")
        ),
        Err(PricingError::InvalidMeasurementRange)
    );
    for (minimum, maximum) in [("1.5", "3"), ("1", "3.5")] {
        assert_eq!(
            VariableWeight::new(
                Each,
                minimum.parse().unwrap(),
                maximum.parse().unwrap(),
                money("100"),
                "1".parse().unwrap(),
                money("400")
            ),
            Err(PricingError::InvalidMeasurementRange)
        );
    }
    let value = VariableWeight::new(
        Each,
        "2".parse().unwrap(),
        "4".parse().unwrap(),
        money("101"),
        "2".parse().unwrap(),
        money("202"),
    )
    .unwrap();
    assert_eq!(value.total_at(quantity("3", Each)).unwrap(), money("152"));
    assert_eq!(value.total_at(quantity("4", Each)).unwrap(), money("202"));
    for cap in [
        Money::from_parts("USD", "400", 2).unwrap(),
        Money::from_parts("JPY", "400", 0).unwrap(),
    ] {
        assert_eq!(
            VariableWeight::new(
                Kilogram,
                "0.8".parse().unwrap(),
                "1.2".parse().unwrap(),
                money("325"),
                "1".parse().unwrap(),
                cap
            ),
            Err(PricingError::Money(MoneyError::CurrencyMismatch))
        );
    }
    let exact = bounds("1", "1", "325", "1", "325");
    assert_eq!(
        exact.total_at(quantity("1", Kilogram)).unwrap(),
        money("325")
    );
}

#[test]
fn free_and_large_exact_inputs_remain_bounded_without_float_or_zero_fallbacks() {
    let free = bounds("0.000001", "999999999999", "0", "0.000001", "0");
    for amount in ["0.000001", "1", "999999999999"] {
        assert_eq!(
            free.total_at(quantity(amount, Kilogram)).unwrap(),
            money("0")
        );
    }
    let maximum = bounds(
        "1",
        "999999999999",
        "999999999999999",
        "999999999999",
        "999999999999999",
    );
    assert_eq!(
        maximum.total_at(quantity("1", Kilogram)).unwrap(),
        money("1000")
    );
    assert_eq!(
        maximum
            .total_at(quantity("999999999999", Kilogram))
            .unwrap(),
        money("999999999999999")
    );
    let over = bounds(
        "0.000001",
        "999999999999",
        "999999999999999",
        "1",
        "999999999999999",
    );
    assert_eq!(
        over.total_at(quantity("999999999999", Kilogram)),
        Err(PricingError::Money(MoneyError::MinorOutOfRange))
    );
    assert_eq!(
        VariableWeight::new(
            Kilogram,
            "1".parse().unwrap(),
            "2".parse().unwrap(),
            money("999999999999999"),
            "0.000001".parse().unwrap(),
            money("999999999999999")
        ),
        Err(PricingError::Money(MoneyError::MinorOutOfRange))
    );
}

#[test]
fn currency_exponents_do_not_change_the_minor_unit_rounding_rule() {
    for (code, exponent) in [("JPY", 0), ("CAD", 2), ("KWD", 3), ("CLF", 4)] {
        let rate = Money::from_parts(code, "101", exponent).unwrap();
        let value = VariableWeight::new(
            Gram,
            "0.5".parse().unwrap(),
            "1".parse().unwrap(),
            rate,
            "1".parse().unwrap(),
            rate,
        )
        .unwrap();
        assert_eq!(
            value.total_at(quantity("0.5", Gram)).unwrap(),
            Money::from_parts(code, "50", exponent).unwrap()
        );
    }
}

#[test]
fn measurement_diagnostics_redact_terms_and_keep_fixed_failure_messages() {
    let value = bounds("0.8", "1.2", "928174615", "1", "999999999999999");
    let debug = format!("{value:?}");
    assert!(!debug.contains("928174615"));
    assert!(!debug.contains("CAD"));
    for error in [
        PricingError::InvalidMeasurementRange,
        PricingError::MeasurementUnitMismatch,
        PricingError::MeasurementOutOfRange,
        PricingError::CapExceeded,
    ] {
        assert!(!error.to_string().is_empty());
        assert!(!error.to_string().contains("928174615"));
        assert!(error.source().is_none());
    }
}

#[cfg(feature = "serde")]
#[test]
fn serialization_retains_complete_exact_bounded_measurement_terms() {
    let value = bounds("0.8", "1.2", "325", "1", "400");
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        serde_json::json!({"measurement_unit":"kg","minimum":"0.8","maximum":"1.2","rate":{"currency":"CAD","minor":"325","exponent":2},"basis":"1","maximum_total":{"currency":"CAD","minor":"400","exponent":2}})
    );
}

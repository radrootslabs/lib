use radroots_domain::{
    ContentMeasure, DiscountBps, FixedPrice, ListingOptionId, Money, PriceTier, Quantity,
    VariableWeight, listing::*,
};

fn money(minor: &str) -> Money {
    Money::from_parts("CAD", minor, 2).unwrap()
}
fn option(price: Price, unit: SaleUnit) -> ListingOption {
    ListingOption::new(
        ListingOptionId::from_bytes([1; 32]),
        "Carrot bunch",
        unit,
        "1".parse().unwrap(),
        "2".parse().unwrap(),
        price,
        Supply::Available {
            amount: "0".parse().unwrap(),
        },
        LeadSeconds::new(0).unwrap(),
    )
    .unwrap()
}
fn fixed() -> FixedPrice {
    FixedPrice::new(
        money("325"),
        "1".parse().unwrap(),
        DiscountBps::new(1000).unwrap(),
        vec![PriceTier::new("3".parse().unwrap(), money("299"))],
    )
    .unwrap()
}
fn variable() -> VariableWeight {
    VariableWeight::new(
        MeasurementUnit::Kilogram,
        "0.8".parse().unwrap(),
        "1.2".parse().unwrap(),
        money("325"),
        "1".parse().unwrap(),
        money("400"),
    )
    .unwrap()
}

#[test]
fn fixed_options_admit_purchase_rules_before_resolving_tiers_and_rounding() {
    let value = option(
        Price::Fixed(fixed()),
        SaleUnit::package_unspecified("bunch").unwrap(),
    );
    assert_eq!(
        value.fixed_line("2".parse().unwrap()).unwrap().total(),
        money("585")
    );
    let selected = value.fixed_line("3".parse().unwrap()).unwrap();
    assert_eq!(selected.rate(), money("299"));
    assert_eq!(selected.selected_tier_from().unwrap().to_string(), "3");
    assert_eq!(selected.total(), money("807"));
    for amount in ["1", "2.5"] {
        assert_eq!(
            value.fixed_line(amount.parse().unwrap()),
            Err(ListingError::InvalidQuantity)
        );
    }
    // Explicit advertised zero is not a purchase ceiling or physical claim.
    assert_eq!(
        value.fixed_line("10".parse().unwrap()).unwrap().total(),
        money("2691")
    );
    let free = option(
        Price::Fixed(
            FixedPrice::new(money("0"), "1".parse().unwrap(), DiscountBps::NONE, vec![]).unwrap(),
        ),
        SaleUnit::measured(MeasurementUnit::Each),
    );
    assert_eq!(
        free.fixed_line("2".parse().unwrap()).unwrap().total(),
        money("0")
    );
}

#[test]
fn quote_and_per_sale_unit_variable_modes_never_invent_fixed_line_totals() {
    for price in [Price::QuoteRequired, Price::VariableWeight(variable())] {
        let value = option(price, SaleUnit::package_unspecified("bag").unwrap());
        assert_eq!(
            value.fixed_line("2".parse().unwrap()),
            Err(ListingError::NotFixedPrice)
        );
        assert_eq!(
            value.fixed_line("1".parse().unwrap()),
            Err(ListingError::InvalidQuantity)
        );
    }
    let value = variable();
    assert_eq!(
        value
            .total_at(Quantity::new("1.2".parse().unwrap(), MeasurementUnit::Kilogram).unwrap())
            .unwrap(),
        money("390")
    );
    // Selecting two bags does not silently double a per-sale-unit value here.
    assert_eq!(value.maximum().to_string(), "1.2");
    assert_eq!(value.maximum_total(), money("400"));
}

#[test]
fn package_content_modes_preserve_counts_and_do_not_turn_estimates_into_conversions() {
    let quantity = Quantity::new("0.5".parse().unwrap(), MeasurementUnit::Kilogram).unwrap();
    for contents in [
        ContentMeasure::Fixed { quantity },
        ContentMeasure::Estimated { quantity },
        ContentMeasure::Unspecified,
    ] {
        let unit = SaleUnit::package("bag", contents).unwrap();
        assert_eq!(unit.contents(), Some(contents));
        assert!(unit.integral());
        assert_eq!(unit.label(), "bag");
        assert!(!format!("{unit:?}").contains("bag"));
        let value = option(Price::Fixed(fixed()), unit);
        assert!(value.validate_amount("2".parse().unwrap()).is_ok());
        assert_eq!(
            value.validate_amount("2.5".parse().unwrap()),
            Err(ListingError::InvalidQuantity)
        );
        assert_eq!(
            contents.fixed_quantity(),
            match contents {
                ContentMeasure::Fixed { quantity } => Some(quantity),
                _ => None,
            }
        );
    }
    assert_eq!(
        SaleUnit::measured(MeasurementUnit::Kilogram).contents(),
        None
    );
    assert!(SaleUnit::package("", ContentMeasure::Unspecified).is_err());
    assert!(SaleUnit::package(&"x".repeat(81), ContentMeasure::Unspecified).is_err());
}

#[test]
fn failed_fixed_calculations_remain_typed_and_redacted() {
    use std::error::Error;
    let value = option(
        Price::Fixed(
            FixedPrice::new(
                money("999999999999999"),
                "0.000001".parse().unwrap(),
                DiscountBps::NONE,
                vec![],
            )
            .unwrap(),
        ),
        SaleUnit::package_unspecified("private label").unwrap(),
    );
    let error = value.fixed_line("2".parse().unwrap()).unwrap_err();
    assert!(matches!(error, ListingError::Pricing(_)));
    assert!(error.source().is_some());
    assert!(ListingError::NotFixedPrice.source().is_none());
    assert!(!format!("{value:?} {error}").contains("999999999999999"));
    assert!(!format!("{:?}", value.price()).contains("999999999999999"));
}

#[cfg(feature = "serde")]
#[test]
fn all_price_and_package_modes_serialize_the_exact_closed_public_shapes() {
    use serde_json::json;
    let fixed = serde_json::to_value(Price::Fixed(fixed())).unwrap();
    assert_eq!(
        fixed,
        json!({"type":"fixed","rate":{"currency":"CAD","minor":"325","exponent":2},
        "basis":"1","discount_bps":1000,"tiers":[{"from":"3","rate":{"currency":"CAD","minor":"299","exponent":2}}]})
    );
    assert_eq!(
        serde_json::to_value(Price::QuoteRequired).unwrap(),
        json!({"type":"quote_required"})
    );
    assert_eq!(
        serde_json::to_value(Price::VariableWeight(variable())).unwrap(),
        json!({"type":"variable_weight","measurement_unit":"kg","minimum_per_sale_unit":"0.8",
            "maximum_per_sale_unit":"1.2","rate":{"currency":"CAD","minor":"325","exponent":2},
            "basis":"1","maximum_per_sale_unit_total":{"currency":"CAD","minor":"400","exponent":2}})
    );
    let quantity = Quantity::new("0.5".parse().unwrap(), MeasurementUnit::Kilogram).unwrap();
    for (name, contents) in [
        ("fixed", ContentMeasure::Fixed { quantity }),
        ("estimated", ContentMeasure::Estimated { quantity }),
    ] {
        let unit = SaleUnit::package("bag", contents).unwrap();
        assert_eq!(
            serde_json::to_value(unit).unwrap(),
            json!({"type":"package","label":"bag",
            "contents":{"type":name,"quantity":{"amount":"0.5","unit":"kg"}}})
        );
    }
}

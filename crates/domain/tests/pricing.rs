use radroots_domain::{
    Decimal, DiscountBps, FixedPrice, MAX_PRICED_LINES, Money, MoneyError, PRICING_PROFILE,
    PriceTier, PricingError, checked_order_total,
};
use std::error::Error;

fn money(code: &str, value: &str, exponent: u8) -> Money {
    Money::from_parts(code, value, exponent).unwrap()
}
fn cad(value: &str) -> Money {
    money("CAD", value, 2)
}
fn price(rate: &str, basis: &str, discount: u16, tiers: Vec<PriceTier>) -> FixedPrice {
    FixedPrice::new(
        cad(rate),
        basis.parse().unwrap(),
        DiscountBps::new(discount).unwrap(),
        tiers,
    )
    .unwrap()
}
fn tier(from: &str, rate: &str) -> PriceTier {
    PriceTier::new(from.parse().unwrap(), cad(rate))
}

#[test]
fn independent_fraction_decimal_and_bigint_vectors_execute_the_rust_engine() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/net_line_v1.json")).unwrap();
    assert_eq!(fixture["schema_version"], 1);
    assert_eq!(fixture["profile"], PRICING_PROFILE);
    let vectors = fixture["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 26);
    for row in vectors {
        let input = price(
            row["rate_minor"].as_str().unwrap(),
            row["basis"].as_str().unwrap(),
            u16::try_from(row["discount_bps"].as_u64().unwrap()).unwrap(),
            vec![],
        );
        let result = input.line(row["quantity"].as_str().unwrap().parse().unwrap());
        if row["expected_error"].is_null() {
            let line = result.unwrap();
            assert_eq!(
                line.total(),
                cad(row["expected_minor"].as_str().unwrap()),
                "{}",
                row["id"]
            );
            assert_eq!(line.rate(), input.rate());
            assert_eq!(line.basis(), input.basis());
            assert_eq!(line.discount_bps(), input.discount_bps());
            assert_eq!(line.selected_tier_from(), None);
        } else {
            assert_eq!(row["expected_error"], "minor_out_of_range");
            assert_eq!(
                result,
                Err(PricingError::Money(MoneyError::MinorOutOfRange)),
                "{}",
                row["id"]
            );
        }
    }
}

#[test]
fn discounts_are_bounded_and_explicit_free_prices_are_not_quotes() {
    for value in [0, 1, 9999, 10_000] {
        assert_eq!(DiscountBps::new(value).unwrap().get(), value);
    }
    for value in [10_001, u16::MAX] {
        assert_eq!(DiscountBps::new(value), Err(PricingError::InvalidDiscount));
    }
    assert_eq!(DiscountBps::NONE.get(), 0);
    assert_eq!(DiscountBps::FULL.get(), 10_000);
    for input in [
        price("0", "1", 0, vec![]),
        price("325", "1", 10_000, vec![]),
    ] {
        assert_eq!(input.line("1".parse().unwrap()).unwrap().total(), cad("0"));
    }
}

#[test]
fn one_inclusive_tier_is_selected_without_sorting_or_combining_discounts() {
    let input = price("500", "1", 1000, vec![tier("2", "400"), tier("5", "300")]);
    for (amount, rate, selected, total) in [
        ("1", "500", None, "450"),
        ("1.999999", "500", None, "900"),
        ("2", "400", Some("2"), "720"),
        ("4.999999", "400", Some("2"), "1800"),
        ("5", "300", Some("5"), "1350"),
        ("7.5", "300", Some("5"), "2025"),
    ] {
        let line = input.line(amount.parse().unwrap()).unwrap();
        assert_eq!(line.rate(), cad(rate));
        assert_eq!(
            line.selected_tier_from(),
            selected.map(|v| v.parse().unwrap())
        );
        assert_eq!(line.total(), cad(total));
        assert_eq!(line.basis().amount(), Decimal::ONE);
        assert_eq!(line.discount_bps().get(), 1000);
    }
    let explicit_zero = price("500", "1", 0, vec![tier("0", "100")]);
    let line = explicit_zero.line("0.000001".parse().unwrap()).unwrap();
    assert_eq!(line.selected_tier_from(), Some(Decimal::ZERO));
    assert_eq!(line.rate(), cad("100"));
}

#[test]
fn tier_admission_checks_numeric_order_currency_and_count_without_guessing() {
    for tiers in [
        vec![tier("2", "100"), tier("1", "90")],
        vec![tier("1", "100"), tier("1", "90")],
        vec![tier("0", "100"), tier("0", "90")],
    ] {
        assert_eq!(
            FixedPrice::new(cad("200"), "1".parse().unwrap(), DiscountBps::NONE, tiers),
            Err(PricingError::InvalidTierOrder)
        );
    }
    for rate in [money("USD", "100", 2), money("JPY", "100", 0)] {
        assert_eq!(
            FixedPrice::new(
                cad("200"),
                "1".parse().unwrap(),
                DiscountBps::NONE,
                vec![PriceTier::new(Decimal::ONE, rate)]
            ),
            Err(PricingError::Money(MoneyError::CurrencyMismatch))
        );
    }
    let tiers = (0..=8).map(|n| tier(&n.to_string(), "100")).collect();
    assert_eq!(
        FixedPrice::new(cad("200"), "1".parse().unwrap(), DiscountBps::NONE, tiers),
        Err(PricingError::TooManyTiers)
    );
    assert_eq!(FixedPrice::MAX_TIERS, 8);
    let mut tiers = Vec::with_capacity(100_000);
    tiers.extend((0..8).map(|n| tier(&n.to_string(), "100")));
    let input = price("200", "100", 0, tiers);
    assert_eq!(input.tiers().len(), 8);
    assert_eq!(input.tiers()[7].from().to_string(), "7");
    assert_eq!(input.tiers()[7].rate(), cad("100"));
    assert_eq!(
        input.line("100".parse().unwrap()).unwrap().total(),
        cad("100")
    );
    let across_scales = price("200", "1", 0, vec![tier("0.5", "150"), tier("2", "100")]);
    assert_eq!(
        across_scales.line("2".parse().unwrap()).unwrap().total(),
        cad("200")
    );
}

#[test]
fn line_rounding_precedes_the_checked_aggregate_and_fulfillment_charge() {
    let one_half = price("1", "1", 0, vec![])
        .line("0.5".parse().unwrap())
        .unwrap()
        .total();
    assert_eq!(one_half, cad("0"));
    assert_eq!(
        checked_order_total(&[one_half, one_half], cad("5")).unwrap(),
        cad("5")
    );
    let combined = price("1", "1", 0, vec![])
        .line("1".parse().unwrap())
        .unwrap()
        .total();
    assert_eq!(combined, cad("1"));
    assert_eq!(
        checked_order_total(&[combined], cad("5")).unwrap(),
        cad("6")
    );
    let zero = cad("0");
    assert_eq!(
        checked_order_total(&[], zero),
        Err(PricingError::InvalidLineCount)
    );
    assert_eq!(
        checked_order_total(&[zero; 33], zero),
        Err(PricingError::InvalidLineCount)
    );
    assert_eq!(MAX_PRICED_LINES, 32);
    assert_eq!(
        checked_order_total(&[cad("1"); 32], cad("7")).unwrap(),
        cad("39")
    );
    let maximum = cad("999999999999999");
    assert_eq!(checked_order_total(&[maximum], zero).unwrap(), maximum);
    assert_eq!(
        checked_order_total(&[maximum], cad("1")),
        Err(PricingError::Money(MoneyError::MinorOutOfRange))
    );
    assert_eq!(
        checked_order_total(&[maximum, maximum], zero),
        Err(PricingError::Money(MoneyError::MinorOutOfRange))
    );
    for other in [money("USD", "0", 2), money("JPY", "0", 0)] {
        assert_eq!(
            checked_order_total(&[zero], other),
            Err(PricingError::Money(MoneyError::CurrencyMismatch))
        );
        assert_eq!(
            checked_order_total(&[other], zero),
            Err(PricingError::Money(MoneyError::CurrencyMismatch))
        );
    }
}

#[test]
fn private_price_diagnostics_are_redacted_and_failures_are_input_free() {
    let input = price("928174615", "1", 1234, vec![tier("3", "928174614")]);
    let line = input.line("3".parse().unwrap()).unwrap();
    for debug in [
        format!("{input:?}"),
        format!("{line:?}"),
        format!("{:?}", input.discount_bps()),
        format!("{:?}", input.tiers()[0]),
    ] {
        assert!(!debug.contains("92817461"));
        assert!(!debug.contains("CAD"));
        assert!(!debug.contains("1234"));
    }
    for error in [
        PricingError::InvalidDiscount,
        PricingError::TooManyTiers,
        PricingError::InvalidTierOrder,
        PricingError::InvalidLineCount,
        PricingError::ArithmeticOutOfRange,
        MoneyError::CurrencyMismatch.into(),
    ] {
        let text = error.to_string();
        assert!(!text.is_empty());
        assert!(!text.contains("92817461"));
        assert!(!text.contains("CAD"));
    }
    assert!(PricingError::InvalidDiscount.source().is_none());
    assert!(
        PricingError::Money(MoneyError::CurrencyMismatch)
            .source()
            .is_some()
    );
}

#[cfg(feature = "serde")]
#[test]
fn explicit_serialization_retains_exact_basis_discount_and_tier_fields() {
    let input = price("325", "0.5", 125, vec![tier("2.5", "299")]);
    let expected = serde_json::json!({"rate":{"currency":"CAD","minor":"325","exponent":2},"basis":"0.5","discount_bps":125,"tiers":[{"from":"2.5","rate":{"currency":"CAD","minor":"299","exponent":2}}]});
    assert_eq!(serde_json::to_value(input).unwrap(), expected);
}

use radroots_domain::{ListingOptionId, listing::*};

#[path = "support/listing_media.rs"]
mod media;

#[path = "support/listing_time.rs"]
mod time;

#[path = "support/agreement.rs"]
mod agreement;

#[test]
fn public_text_has_shared_unicode_bounds_and_revision_data_rejects_inverted_time() {
    use radroots_domain::{EnvelopeEventId, PublicKey, UnixSeconds};
    let title = "🥕".repeat(160);
    let summary = "é".repeat(280);
    let body = " ".repeat(16_384);
    let value = ListingText::new(&title, &summary, &body).unwrap();
    #[cfg(feature = "serde")]
    assert_eq!(
        serde_json::to_value(&value).unwrap(),
        serde_json::json!({"title":title,"summary":summary,"description_markdown":body})
    );
    assert!(!format!("{value:?}").contains('🥕'));
    for (title, summary, body) in [
        ("🥕".repeat(161), "summary".into(), String::new()),
        ("title".into(), "é".repeat(281), String::new()),
        ("title".into(), "summary".into(), " ".repeat(16_385)),
    ] {
        assert_eq!(
            ListingText::new(&title, &summary, &body).unwrap_err(),
            ListingError::InvalidText
        );
    }
    let terms = ListingTerms::new(
        ListingState::Active,
        product(),
        ListingOptionId::from_bytes([1; 32]),
        vec![option(1, SaleUnit::measured(MeasurementUnit::Each), "1", "1", "10").unwrap()],
        fulfillment(),
        None,
        vec![],
    )
    .unwrap();
    let key =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .unwrap();
    // These are structural public data, without a signed/installed claim.
    assert_eq!(
        Listing::new(
            ListingId::from_bytes([1; 16]),
            key,
            EnvelopeEventId::from_bytes([2; 32]),
            UnixSeconds::new(99).unwrap(),
            UnixSeconds::new(100).unwrap(),
            value,
            terms
        )
        .unwrap_err(),
        ListingError::InvalidTime
    );
}

fn product() -> Product {
    Product::new(
        "Carrots",
        "Public description",
        None,
        Some("Example region"),
    )
    .unwrap()
}
fn fulfillment() -> PublicFulfillment {
    PublicFulfillment::new(
        vec![FulfillmentMode::Pickup],
        "Example region",
        LeadSeconds::new(0).unwrap(),
    )
    .unwrap()
}
fn option(
    n: u8,
    unit: SaleUnit,
    increment: &str,
    minimum: &str,
    amount: &str,
) -> Result<ListingOption, ListingError> {
    ListingOption::quote_required(
        ListingOptionId::from_bytes([n; 32]),
        "One bunch",
        unit,
        increment.parse().unwrap(),
        minimum.parse().unwrap(),
        Supply::Available {
            amount: amount.parse().unwrap(),
        },
        LeadSeconds::new(0).unwrap(),
    )
}

#[test]
fn spare_caller_capacity_preserves_exact_terms_and_default_selection() {
    let mut modes = Vec::with_capacity(4096);
    modes.push(FulfillmentMode::Pickup);
    let mut options = Vec::with_capacity(4096);
    options.push(
        option(
            1,
            SaleUnit::package_unspecified("bunch").unwrap(),
            "1",
            "1",
            "10",
        )
        .unwrap(),
    );
    options.push(
        option(
            2,
            SaleUnit::measured(MeasurementUnit::Kilogram),
            "0.25",
            "0.5",
            "10",
        )
        .unwrap(),
    );
    let compact = options.clone();
    let expanded = ListingTerms::new(
        ListingState::Active,
        product(),
        ListingOptionId::from_bytes([2; 32]),
        options,
        PublicFulfillment::new(modes, "Example region", LeadSeconds::new(0).unwrap()).unwrap(),
        None,
        vec![],
    )
    .unwrap();
    let expected = ListingTerms::new(
        ListingState::Active,
        product(),
        ListingOptionId::from_bytes([2; 32]),
        compact,
        fulfillment(),
        None,
        vec![],
    )
    .unwrap();
    assert_eq!(expanded, expected);
    assert_eq!(
        expanded.default_option().id(),
        ListingOptionId::from_bytes([2; 32])
    );
    #[cfg(feature = "serde")]
    assert_eq!(
        serde_json::to_vec(&expanded).unwrap(),
        serde_json::to_vec(&expected).unwrap()
    );
    assert_eq!(expanded.clone(), expected);
}

#[test]
fn identifiers_durations_and_public_data_preserve_exact_cross_host_values() {
    let id = ListingId::from_bytes([0xAB; 16]);
    assert_eq!(id.to_string(), "ab".repeat(16));
    assert_eq!(
        id.to_string().parse::<ListingId>().unwrap().as_bytes(),
        &[0xAB; 16]
    );
    for bad in [
        "ab".repeat(15),
        "ab".repeat(17),
        "AB".repeat(16),
        "0g".repeat(16),
        "é".repeat(16),
    ] {
        assert_eq!(
            bad.parse::<ListingId>().unwrap_err(),
            ListingError::InvalidIdentifier
        );
    }
    assert_eq!(
        LeadSeconds::new(i64::MAX as u64).unwrap().get(),
        i64::MAX as u64
    );
    assert_eq!(
        LeadSeconds::new(i64::MAX as u64 + 1).unwrap_err(),
        ListingError::InvalidDuration
    );
    let name = "🥕".repeat(160);
    let value = Product::new(&name, "\u{0001}", None, None).unwrap();
    assert_eq!(value.name(), name);
    assert!(!format!("{value:?}").contains('🥕'));
    assert!(Product::new(&(name + "x"), "", None, None).is_err());
    assert!(Product::new("", "", None, None).is_err());
    assert!(Product::new("x", &"x".repeat(4097), None, None).is_err());
    assert!(Product::new("x", "", None, Some(&"x".repeat(161))).is_err());
    assert!(SaleUnit::package_unspecified("").is_err());
    assert!(SaleUnit::package_unspecified(&"x".repeat(81)).is_err());
    assert!(PublicFulfillment::new(vec![], "", LeadSeconds::new(0).unwrap()).is_err());
    assert!(
        PublicFulfillment::new(
            vec![FulfillmentMode::Pickup; 2],
            "",
            LeadSeconds::new(0).unwrap()
        )
        .is_err()
    );
    assert!(
        PublicFulfillment::new(
            vec![FulfillmentMode::Pickup; 3],
            "",
            LeadSeconds::new(0).unwrap()
        )
        .is_err()
    );
    assert!(
        PublicFulfillment::new(
            vec![FulfillmentMode::Pickup],
            &"x".repeat(161),
            LeadSeconds::new(0).unwrap()
        )
        .is_err()
    );
    assert!(
        PublicFulfillment::new(
            vec![FulfillmentMode::Pickup, FulfillmentMode::Delivery],
            "",
            LeadSeconds::new(0).unwrap()
        )
        .is_ok()
    );
}

#[test]
fn exact_increments_and_units_never_invent_fractional_packages_or_conversion() {
    for unit in [
        MeasurementUnit::Gram,
        MeasurementUnit::Kilogram,
        MeasurementUnit::Millilitre,
        MeasurementUnit::Litre,
    ] {
        let value = option(1, SaleUnit::measured(unit), "0.25", "0.5", "1.5").unwrap();
        assert_eq!(value.unit().label(), unit.symbol());
        assert!(!value.unit().integral());
        assert_eq!(value.price(), &Price::QuoteRequired);
        assert!(option(1, SaleUnit::measured(unit), "0.25", "0.3", "1.5").is_err());
    }
    for unit in [
        SaleUnit::measured(MeasurementUnit::Each),
        SaleUnit::package_unspecified("bunch").unwrap(),
    ] {
        assert!(unit.integral());
        for (inc, min, amount) in [
            ("0.5", "1", "10"),
            ("1", "1.5", "10"),
            ("1", "1", "0.5"),
            ("2", "1", "10"),
        ] {
            assert_eq!(
                option(1, unit.clone(), inc, min, amount).unwrap_err(),
                ListingError::InvalidQuantity
            );
        }
        let available = option(1, unit, "1", "2", "0").unwrap();
        assert_eq!(available.id(), ListingOptionId::from_bytes([1; 32]));
    }
    let unknown = ListingOption::quote_required(
        ListingOptionId::from_bytes([2; 32]),
        "Measured",
        SaleUnit::measured(MeasurementUnit::Kilogram),
        "0.000001".parse().unwrap(),
        "0.000002".parse().unwrap(),
        Supply::Unknown,
        LeadSeconds::new(0).unwrap(),
    )
    .unwrap();
    assert_eq!(unknown.label(), "Measured");
}

#[test]
fn selected_amounts_validate_minimum_and_increment_without_using_advertised_stock() {
    let measured = option(
        1,
        SaleUnit::measured(MeasurementUnit::Kilogram),
        "0.25",
        "0.5",
        "0",
    )
    .unwrap();
    assert_eq!(measured.minimum().to_string(), "0.5");
    assert_eq!(measured.increment().to_string(), "0.25");
    for amount in ["0.5", "0.75", "10.25", "999999999999"] {
        measured.validate_amount(amount.parse().unwrap()).unwrap();
    }
    for amount in ["0.25", "0.6", "0.500001"] {
        assert_eq!(
            measured.validate_amount(amount.parse().unwrap()),
            Err(ListingError::InvalidQuantity)
        );
    }
    for unit in [
        SaleUnit::measured(MeasurementUnit::Each),
        SaleUnit::package_unspecified("bag").unwrap(),
    ] {
        let value = option(2, unit, "2", "4", "0").unwrap();
        for amount in ["4", "6", "100"] {
            value.validate_amount(amount.parse().unwrap()).unwrap();
        }
        for amount in ["1", "3", "4.5", "5"] {
            assert_eq!(
                value.validate_amount(amount.parse().unwrap()),
                Err(ListingError::InvalidQuantity)
            );
        }
    }
}

#[test]
fn selected_amounts_preserve_extreme_exact_scales_and_shared_constructor_rules() {
    let tiny = option(
        1,
        SaleUnit::measured(MeasurementUnit::Gram),
        "0.000001",
        "0.000002",
        "0",
    )
    .unwrap();
    for amount in ["0.000002", "1", "999999999999"] {
        tiny.validate_amount(amount.parse().unwrap()).unwrap();
    }
    assert_eq!(
        tiny.validate_amount("0.000001".parse().unwrap()),
        Err(ListingError::InvalidQuantity)
    );
    let large = option(
        2,
        SaleUnit::measured(MeasurementUnit::Litre),
        "999999999999",
        "999999999999",
        "0",
    )
    .unwrap();
    large
        .validate_amount("999999999999".parse().unwrap())
        .unwrap();
    assert_eq!(
        large.validate_amount("1".parse().unwrap()),
        Err(ListingError::InvalidQuantity)
    );
}

#[test]
fn bounded_unique_options_preserve_exact_default_without_fallback() {
    let make = |n| {
        option(
            n,
            SaleUnit::package_unspecified("bunch").unwrap(),
            "1",
            "1",
            "10",
        )
        .unwrap()
    };
    let terms = |choices, default| {
        ListingTerms::new(
            ListingState::Active,
            product(),
            ListingOptionId::from_bytes([default; 32]),
            choices,
            fulfillment(),
            None,
            vec![],
        )
    };
    assert!(terms(vec![], 1).is_err());
    assert!(terms(vec![make(1), make(1)], 1).is_err());
    assert!(terms(vec![make(1)], 2).is_err());
    assert!(terms((1..=17).map(make).collect(), 1).is_err());
    let valid = terms((1..=16).map(make).collect(), 16).unwrap();
    assert_eq!(
        valid.default_option().id(),
        ListingOptionId::from_bytes([16; 32])
    );
    assert_eq!(valid.options().len(), 16);
    assert_eq!(valid.fulfillment().area(), "Example region");
    assert_eq!(valid.state(), ListingState::Active);
    assert!(!format!("{valid:?}").contains("Carrots"));
}

#[cfg(feature = "serde")]
#[test]
fn serialized_values_have_exact_string_numbers_and_no_private_or_derived_fields() {
    let id = ListingId::from_bytes([0xAB; 16]);
    let raw = serde_json::to_string(&id).unwrap();
    assert_eq!(serde_json::from_str::<ListingId>(&raw).unwrap(), id);
    assert!(serde_json::from_str::<ListingId>("null").is_err());
    assert!(serde_json::from_str::<ListingId>(&format!("\"{}\"", "AB".repeat(16))).is_err());
    assert_eq!(
        serde_json::to_string(&LeadSeconds::new(0).unwrap()).unwrap(),
        "\"0\""
    );
    for bad in ["0", "\"00\"", "\"-1\"", "null"] {
        assert!(serde_json::from_str::<LeadSeconds>(bad).is_err());
    }
    let choice = option(
        1,
        SaleUnit::package_unspecified("bunch").unwrap(),
        "1",
        "1",
        "10",
    )
    .unwrap();
    let terms = ListingTerms::new(
        ListingState::Active,
        product(),
        choice.id(),
        vec![choice],
        fulfillment(),
        None,
        vec![],
    )
    .unwrap();
    let value = serde_json::to_value(terms).unwrap();
    assert_eq!(
        value["options"][0]["unit"],
        serde_json::json!({"type":"package","label":"bunch","contents":{"type":"unspecified"}})
    );
    assert_eq!(value["options"][0]["increment"], "1");
    assert_eq!(
        value["options"][0]["price"],
        serde_json::json!({"type":"quote_required"})
    );
    assert_eq!(
        value["options"][0]["supply"],
        serde_json::json!({"type":"available","amount":"10"})
    );
    assert!(value.get("default_index").is_none());
    assert_eq!(value["media"], serde_json::json!([]));
    assert_eq!(value.as_object().unwrap().len(), 7);
}

use radroots_domain::{
    FulfillmentError, ListingOptionId, PrivateFulfillment, TIME_ZONE_DATABASE_VERSION, TimeError,
    TimeZoneId, UnixSeconds, Window, listing::*,
};

fn window() -> Window {
    Window::new(
        UnixSeconds::new(100).unwrap(),
        UnixSeconds::new(101).unwrap(),
        TimeZoneId::from_name("America/Toronto").unwrap(),
    )
    .unwrap()
}

#[test]
fn pinned_zone_names_are_exact_and_all_dataset_entries_are_admitted() {
    assert_eq!(jiff_tzdb::VERSION, Some(TIME_ZONE_DATABASE_VERSION));
    for name in jiff_tzdb::available() {
        assert_eq!(TimeZoneId::from_name(name).unwrap().name(), name);
    }
    for name in [
        "",
        "america/toronto",
        " America/Toronto",
        "America/Toronto ",
        "Invented/Zone",
        "+05:00",
        "America/Toronto\0",
        &"a".repeat(81),
        &"🥕".repeat(21),
    ] {
        assert_eq!(TimeZoneId::from_name(name), Err(TimeError::InvalidTimeZone));
    }
    let zone = TimeZoneId::from_name("Etc/UTC").unwrap();
    assert_eq!(zone.name(), "Etc/UTC");
    assert!(!format!("{zone:?}").contains("UTC"));
}

#[test]
fn windows_preserve_ordered_absolute_bounds_without_civil_time_conversion() {
    let zone = TimeZoneId::from_name("America/Toronto").unwrap();
    let start = UnixSeconds::new(0).unwrap();
    let end = UnixSeconds::new(253_402_300_799).unwrap();
    let value = Window::new(start, end, zone).unwrap();
    assert_eq!(
        (value.start(), value.end(), value.timezone()),
        (start, end, zone)
    );
    for (start, end) in [(end, start), (start, start), (end, end)] {
        assert_eq!(Window::new(start, end, zone), Err(TimeError::InvalidWindow));
    }
    // Absolute instants remain exact; no local fold/offset is inferred.
    let transition = Window::new(
        UnixSeconds::new(1_793_480_400).unwrap(),
        UnixSeconds::new(1_793_487_600).unwrap(),
        zone,
    )
    .unwrap();
    assert_eq!(transition.end().get() - transition.start().get(), 7200);
    assert_eq!(format!("{transition:?}"), "Window(<redacted>)");
    assert_eq!(
        TimeError::InvalidWindow.to_string(),
        "invalid absolute window"
    );
    assert_eq!(TimeError::InvalidTimeZone.to_string(), "invalid time zone");
    assert_eq!(
        TimeError::UnsupportedDatabase.to_string(),
        "unsupported time zone database"
    );
    #[cfg(feature = "serde")]
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        serde_json::json!({
        "start":"0","end":"253402300799","timezone":"America/Toronto"})
    );
}

#[test]
fn private_fulfillment_bounds_unicode_and_redacts_without_public_terms_or_effects() {
    let address = "🥕".repeat(2048);
    let instructions = "é".repeat(2048);
    let contact = "☎".repeat(256);
    for mode in [FulfillmentMode::Pickup, FulfillmentMode::Delivery] {
        let value =
            PrivateFulfillment::new(mode, window(), &address, &instructions, Some(&contact))
                .unwrap();
        assert_eq!(value.mode(), mode);
        assert_eq!(value.window(), window());
        assert_eq!(value.address(), address);
        assert_eq!(value.instructions(), instructions);
        assert_eq!(value.contact(), Some(contact.as_str()));
        assert_eq!(value, value.clone());
        assert_eq!(format!("{value:?}"), "PrivateFulfillment(<redacted>)");
        let empty = PrivateFulfillment::new(mode, window(), "", "", None).unwrap();
        assert_eq!(empty.contact(), None);
        assert_eq!(empty.address(), "");
        assert_eq!(empty.instructions(), "");
        assert_eq!(
            PrivateFulfillment::new(mode, window(), "", "", Some(""))
                .unwrap()
                .contact(),
            Some("")
        );
    }
    for (address, instructions, contact) in [
        ("a".repeat(2049), String::new(), None),
        ("a".repeat(8193), String::new(), None),
        (String::new(), "é".repeat(2049), None),
        (String::new(), String::new(), Some("☎".repeat(257))),
        (String::new(), String::new(), Some("a".repeat(1025))),
    ] {
        assert_eq!(
            PrivateFulfillment::new(
                FulfillmentMode::Pickup,
                window(),
                &address,
                &instructions,
                contact.as_deref()
            ),
            Err(FulfillmentError::InvalidText)
        );
    }
    assert_eq!(
        FulfillmentError::InvalidText.to_string(),
        "invalid private fulfillment text"
    );
}

#[test]
fn estimates_allow_zero_and_measured_fractions_but_keep_count_units_integral() {
    for unit in [
        SaleUnit::package_unspecified("bunch").unwrap(),
        SaleUnit::measured(MeasurementUnit::Each),
        SaleUnit::measured(MeasurementUnit::Kilogram),
    ] {
        for amount in ["0", "2", "0.5"] {
            for window in [None, Some(window())] {
                let result = ListingOption::quote_required(
                    ListingOptionId::from_bytes([9; 32]),
                    "expected",
                    unit.clone(),
                    "1".parse().unwrap(),
                    "1".parse().unwrap(),
                    Supply::Estimated {
                        amount: amount.parse().unwrap(),
                        window,
                    },
                    LeadSeconds::new(0).unwrap(),
                );
                if amount == "0.5" && unit.integral() {
                    assert_eq!(result.unwrap_err(), ListingError::InvalidQuantity);
                } else {
                    let value = result.unwrap();
                    assert_eq!(
                        value.supply(),
                        Supply::Estimated {
                            amount: amount.parse().unwrap(),
                            window
                        }
                    );
                    // Advertised estimate is neither a ceiling nor reservation.
                    assert!(value.validate_amount("100".parse().unwrap()).is_ok());
                    #[cfg(feature = "serde")]
                    assert_eq!(
                        serde_json::to_value(value.supply()).unwrap(),
                        serde_json::json!({
                        "type":"estimated","amount":amount,"window":window})
                    );
                }
            }
        }
    }
}

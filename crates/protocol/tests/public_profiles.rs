use radroots_protocol::{
    advertisement::{self, Feature, Profile},
    command::CommandError,
    listing,
    nip01::EventFields,
};
use serde_json::Value;
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/public_profiles.v1.json")).unwrap()
}

fn decoded_wire(wire: &Value) -> Result<listing::DecodedListing, CommandError> {
    let tags: Vec<Vec<String>> = serde_json::from_value(wire["tags"].clone()).unwrap();
    let fields = EventFields {
        author: wire["pubkey"].as_str().unwrap().parse().unwrap(),
        created_at: wire["created_at"].as_u64().unwrap(),
        kind: wire["kind"].as_u64().unwrap() as u32,
        tags: &tags,
        content: wire["content"].as_str().unwrap(),
    };
    listing::decode_observed(&fields, [0; 32])
}

#[test]
fn exact_standard_projection_handles_supported_exponents_and_free_prices() {
    let data = fixture();
    let base = &data["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "listing_fixed")
        .unwrap()["raw"];
    for (currency, exponent, minor, amount) in [
        ("JPY", 0, "325", "325.000"),
        ("KWD", 3, "325", "0.3250"),
        ("CLF", 4, "325", "0.032500"),
        ("CAD", 2, "0", "0"),
        ("CAD", 2, "0", "0.0000"),
    ] {
        let mut wire: Value = serde_json::from_str(base.as_str().unwrap()).unwrap();
        let tags = wire["tags"].as_array_mut().unwrap();
        let profile_tag = tags
            .iter_mut()
            .find(|t| t[0] == "radroots:listing")
            .unwrap();
        let mut profile: Value = serde_json::from_str(profile_tag[1].as_str().unwrap()).unwrap();
        profile["options"][0]["price"]["rate"]["currency"] = serde_json::json!(currency);
        profile["options"][0]["price"]["rate"]["exponent"] = serde_json::json!(exponent);
        profile["options"][0]["price"]["rate"]["minor"] = serde_json::json!(minor);
        profile_tag[1] = serde_json::json!(profile.to_string());
        *tags.iter_mut().find(|t| t[0] == "price").unwrap() =
            serde_json::json!(["price", amount, currency]);
        *tags.iter_mut().find(|t| t[0] == "summary").unwrap() =
            serde_json::json!(["summary", "One bunch, exact published rate."]);
        assert!(decoded_wire(&wire).is_ok(), "{currency} {amount}");
    }
}

#[test]
fn complete_public_projections_refuse_missing_duplicate_and_contradictory_fields() {
    let data = fixture();
    let base: Value = serde_json::from_str(
        data["vectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == "listing_fixed")
            .unwrap()["raw"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for name in [
        "d",
        "title",
        "summary",
        "published_at",
        "location",
        "radroots:price_unit",
        "radroots:listing",
        "status",
        "price",
    ] {
        let mut wire = base.clone();
        wire["tags"]
            .as_array_mut()
            .unwrap()
            .retain(|tag| tag[0] != name);
        assert!(decoded_wire(&wire).is_err(), "missing {name}");
        wire = base.clone();
        let tags = wire["tags"].as_array_mut().unwrap();
        let duplicate = tags.iter().find(|tag| tag[0] == name).unwrap().clone();
        tags.push(duplicate);
        assert!(decoded_wire(&wire).is_err(), "duplicate {name}");
    }
    for (name, value) in [
        ("d", "noncanonical"),
        ("published_at", "099"),
        ("published_at", "101"),
        ("title", ""),
        ("location", "Other area"),
        ("radroots:price_unit", "kg"),
        ("status", "sold"),
    ] {
        let mut wire = base.clone();
        wire["tags"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|tag| tag[0] == name)
            .unwrap()[1] = serde_json::json!(value);
        assert!(decoded_wire(&wire).is_err(), "{name}={value}");
    }
    let mut wire = base.clone();
    wire["created_at"] = serde_json::json!(u64::MAX);
    assert!(decoded_wire(&wire).is_err());
    wire = base.clone();
    wire["kind"] = serde_json::json!(1);
    assert!(decoded_wire(&wire).is_err());
    wire = base.clone();
    wire["tags"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|tag| tag[0] == "price")
        .unwrap()[2] = serde_json::json!("USD");
    assert!(decoded_wire(&wire).is_err());
    for name in ["listing_paused", "listing_quote", "listing_variable"] {
        let mut wire: Value = serde_json::from_str(
            data["vectors"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["name"] == name)
                .unwrap()["raw"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        wire["tags"]
            .as_array_mut()
            .unwrap()
            .push(if name == "listing_paused" {
                serde_json::json!(["status", "active"])
            } else {
                serde_json::json!(["price", "0", "CAD"])
            });
        assert!(decoded_wire(&wire).is_err(), "{name}");
    }
}
#[test]
fn independently_signed_profile_projections_decode_without_claiming_authenticity() {
    for row in fixture()["vectors"].as_array().unwrap() {
        let wire: Value = serde_json::from_str(row["raw"].as_str().unwrap()).unwrap();
        let tags: Vec<Vec<String>> = serde_json::from_value(wire["tags"].clone()).unwrap();
        let kind = wire["kind"].as_u64().unwrap() as u32;
        let result = if kind == 30402 {
            let fields = EventFields {
                author: wire["pubkey"].as_str().unwrap().parse().unwrap(),
                created_at: wire["created_at"].as_u64().unwrap(),
                kind,
                tags: &tags,
                content: wire["content"].as_str().unwrap(),
            };
            listing::decode_observed(&fields, [0; 32]).map(|v| {
                let terms = serde_json::to_value(v.listing().terms()).unwrap();
                assert_eq!(terms["product"]["name"], "Carrots 🥕");
                "listing"
            })
        } else {
            advertisement::decode(
                wire["content"].as_str().unwrap().as_bytes(),
                wire["created_at"].as_u64().unwrap(),
            )
            .map(|v| {
                assert!(!v.claims().is_empty());
                "capabilities"
            })
        };
        // Nonempty tags are an outer carrier rule, checked by Nostr after auth.
        if row["name"] == "capability_tag" {
            assert!(result.is_ok());
            continue;
        }
        match result {
            Ok(value) => assert_eq!(value, row["expected"].as_str().unwrap(), "{}", row["name"]),
            Err(CommandError::UnsupportedProfile) => {
                assert_eq!(row["expected"], "unsupported", "{}", row["name"])
            }
            Err(_) => assert_eq!(row["expected"], "invalid", "{}", row["name"]),
        }
    }
}
#[test]
fn capability_claims_enforce_closed_arrays_numbers_and_calendar_bounds() {
    let mut cap = serde_json::json!({"profile":"radroots.capabilities","version":1,"supported_profiles":[{"profile":"radroots.commerce","versions":[1,65535],"features":["base-order-v1","bounded-variable-weight-v1"]}],"valid_until":"200"});
    let decoded = advertisement::decode(cap.to_string().as_bytes(), 100).unwrap();
    assert_eq!(decoded.valid_until().get(), 200);
    assert_eq!(decoded.claims()[0].profile(), Profile::Commerce);
    assert_eq!(decoded.claims()[0].versions(), [1, 65535]);
    assert_eq!(
        decoded.claims()[0].features(),
        [Feature::BaseOrder, Feature::BoundedVariableWeight]
    );
    let original = cap.clone();
    for value in [
        serde_json::json!([]),
        serde_json::json!([0]),
        serde_json::json!([65536]),
        serde_json::json!([1.0]),
        serde_json::json!(["1"]),
        serde_json::json!([1, 2, 3, 4, 5, 6, 7, 8, 9]),
    ] {
        cap = original.clone();
        cap["supported_profiles"][0]["versions"] = value;
        assert!(advertisement::decode(cap.to_string().as_bytes(), 100).is_err());
    }
    for value in [
        serde_json::json!([]),
        serde_json::json!(["base-order-v1", "base-order-v1"]),
        serde_json::json!([false]),
    ] {
        cap = original.clone();
        cap["supported_profiles"][0]["features"] = value;
        assert!(advertisement::decode(cap.to_string().as_bytes(), 100).is_err());
    }
    for time in ["0", "99", "100", "2592101", "0200", "253402300800"] {
        cap = original.clone();
        cap["valid_until"] = serde_json::json!(time);
        assert!(advertisement::decode(cap.to_string().as_bytes(), 100).is_err());
    }
    assert!(advertisement::decode(original.to_string().as_bytes(), u64::MAX).is_err());
    assert!(advertisement::decode(&vec![b' '; 65537], 100).is_err());
    for raw in [
        "[]",
        "{}",
        r#"{"profile":"radroots.capabilities","profile":"radroots.capabilities","version":1}"#,
        r#"{"profile":"radroots.capabilities","version":1,"supported_profiles":[],"valid_until":"200","extra":true}"#,
    ] {
        assert!(advertisement::decode(raw.as_bytes(), 100).is_err());
    }
}

#[test]
fn direct_listing_entry_enforces_complete_event_budget_before_domain_values() {
    let data = fixture();
    let row = data["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "listing_fixed")
        .unwrap();
    let mut wire: Value = serde_json::from_str(row["raw"].as_str().unwrap()).unwrap();
    wire["content"] = serde_json::json!("");
    // Independent physical JSON framing, rather than the production counter,
    // determines the exact boundary; this test makes no signature claim.
    let available = 65536 - wire.to_string().len();
    let exact = "🥬".repeat(available / 4) + &"x".repeat(available % 4);
    assert!(exact.chars().count() <= 16384);
    wire["content"] = serde_json::json!(&exact);
    assert_eq!(wire.to_string().len(), 65536);
    let tags: Vec<Vec<String>> = serde_json::from_value(wire["tags"].clone()).unwrap();
    let mut fields = EventFields {
        author: wire["pubkey"].as_str().unwrap().parse().unwrap(),
        created_at: 100,
        kind: 30402,
        tags: &tags,
        content: &exact,
    };
    assert!(listing::decode_observed(&fields, [0; 32]).is_ok());
    let over = exact.clone() + "x";
    fields.content = &over;
    assert_eq!(
        listing::decode_observed(&fields, [0; 32]).unwrap_err(),
        CommandError::LimitExceeded
    );
}

#[test]
fn standard_price_projection_compares_exact_value_without_a_builder_format_requirement() {
    let data = fixture();
    let row = data["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "listing_fixed")
        .unwrap();
    let wire: Value = serde_json::from_str(row["raw"].as_str().unwrap()).unwrap();
    let original: Vec<Vec<String>> = serde_json::from_value(wire["tags"].clone()).unwrap();
    for (amount, accepted) in [
        ("3.25", true),
        ("3.250", true),
        ("3.2500", true),
        ("3.250000000000000000000000000000000", true),
        ("3.251", false),
        ("3.2501", false),
        ("3.24", false),
        ("-3.25", false),
        ("+3.25", false),
        (" 3.25", false),
        ("3.25 ", false),
        ("3.25e0", false),
        ("NaN", false),
        ("", false),
        (".325", false),
        ("3.", false),
        ("3..25", false),
        ("99999999999999999999999999999999999999999", false),
    ] {
        let mut tags = original.clone();
        tags.iter_mut().find(|tag| tag[0] == "price").unwrap()[1] = amount.into();
        let fields = EventFields {
            author: wire["pubkey"].as_str().unwrap().parse().unwrap(),
            created_at: 100,
            kind: 30402,
            tags: &tags,
            content: wire["content"].as_str().unwrap(),
        };
        assert_eq!(
            listing::decode_observed(&fields, [0; 32]).is_ok(),
            accepted,
            "{amount}"
        );
    }
}

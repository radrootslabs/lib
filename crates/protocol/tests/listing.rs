#![cfg(feature = "listing")]
use radroots_domain::{ListingOptionId, PublicKey, UnixSeconds, listing::*};
use radroots_protocol::{
    listing::*,
    nip01::{self, WireError},
};

fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn input(state: ListingState, body: &str) -> PublishInput {
    let option = ListingOption::quote_required(
        ListingOptionId::from_bytes([2; 32]),
        "One bunch",
        SaleUnit::package_unspecified("bunch").unwrap(),
        "1".parse().unwrap(),
        "1".parse().unwrap(),
        Supply::Available {
            amount: "10".parse().unwrap(),
        },
        LeadSeconds::new(0).unwrap(),
    )
    .unwrap();
    let terms = ListingTerms::new(
        state,
        Product::new(
            "Carrots",
            "Public description",
            None,
            Some("Example region"),
        )
        .unwrap(),
        option.id(),
        vec![option],
        PublicFulfillment::new(
            vec![FulfillmentMode::Pickup],
            "Example region",
            LeadSeconds::new(0).unwrap(),
        )
        .unwrap(),
        None,
    )
    .unwrap();
    PublishInput::new(
        ListingId::from_bytes([1; 16]),
        "Carrot bunch",
        "One bunch; quote required",
        body,
        UnixSeconds::new(100).unwrap(),
        terms,
    )
    .unwrap()
}
fn event(input: &PublishInput) -> ListingEvent {
    ListingEvent::new(input, key(), UnixSeconds::new(101).unwrap()).unwrap()
}
fn tag<'a>(event: &'a ListingEvent, name: &str) -> &'a [String] {
    let fields = event.fields();
    let mut selected = fields.tags.iter().filter(|t| t[0] == name);
    let value = selected.next().unwrap();
    assert!(selected.next().is_none());
    value
}

#[test]
fn assigned_listing_profile_is_canonical_and_quote_required_has_no_fabricated_price() {
    let captured = input(ListingState::Active, "Ask for terms.");
    let value = event(&captured);
    assert_eq!(value.fields().kind, 30402);
    assert_eq!(tag(&value, "d"), ["d", &"01".repeat(16)]);
    assert_eq!(
        tag(&value, "radroots:price_unit"),
        ["radroots:price_unit", "bunch"]
    );
    assert_eq!(tag(&value, "status"), ["status", "active"]);
    assert_eq!(tag(&value, "published_at"), ["published_at", "100"]);
    assert!(!value.fields().tags.iter().any(|t| t[0] == "price"));
    // Independently written golden profile fixes ordering and exact scalar
    // representation; this does not claim independent signature qualification.
    let golden = concat!(
        "{\"authority\":null,\"default_option\":\"",
        "0202020202020202020202020202020202020202020202020202020202020202",
        "\",\"fulfillment\":{\"area\":\"Example region\",\"lead_seconds\":\"0\",\"modes\":[\"pickup\"]},\"media\":[],\"options\":[{\"id\":\"",
        "0202020202020202020202020202020202020202020202020202020202020202",
        "\",\"increment\":\"1\",\"label\":\"One bunch\",\"lead_seconds\":\"0\",\"minimum\":\"1\",\"price\":{\"type\":\"quote_required\"},\"supply\":{\"amount\":\"10\",\"type\":\"available\"},\"unit\":{\"contents\":{\"type\":\"unspecified\"},\"label\":\"bunch\",\"type\":\"package\"}}],\"product\":{\"description\":\"Public description\",\"name\":\"Carrots\",\"origin_summary\":\"Example region\",\"producer\":null},\"profile\":\"radroots.listing\",\"state\":\"active\",\"version\":1}"
    );
    assert_eq!(tag(&value, "radroots:listing")[1], golden);
    let serialized = serde_json::to_value(&captured).unwrap();
    assert_eq!(
        serialized["profile"],
        serde_json::from_str::<serde_json::Value>(golden).unwrap()
    );
    assert!(serialized.get("terms").is_none());
    assert_eq!(serialized.as_object().unwrap().len(), 6);
    assert_eq!(captured.id().to_string(), "01".repeat(16));
    assert_eq!(captured.published_at().get(), 100);
    assert!(!format!("{captured:?} {value:?}").contains("Ask for terms"));
}

#[test]
fn inactive_revisions_keep_the_original_coordinate_without_a_false_sold_status() {
    let active = event(&input(ListingState::Active, "Original terms."));
    for state in [ListingState::Paused, ListingState::Withdrawn] {
        let revised = event(&input(state, "Original terms."));
        assert_eq!(revised.fields().kind, active.fields().kind);
        assert_eq!(tag(&revised, "d"), tag(&active, "d"));
        assert_ne!(revised.event_id(), active.event_id());
        assert!(revised.fields().content.contains("unavailable"));
        assert!(revised.fields().content.ends_with("Original terms."));
        assert!(tag(&revised, "summary")[1].contains("unavailable"));
        assert!(
            !revised
                .fields()
                .tags
                .iter()
                .any(|t| t[0] == "status" || t[0] == "price")
        );
    }
}

#[test]
fn measured_choice_and_opaque_metadata_supply_no_pool_or_endpoint_authority() {
    use radroots_domain::{AuthorityRef, ExecutorId, Generation};
    let choice = ListingOption::quote_required(
        ListingOptionId::from_bytes([3; 32]),
        "Measured herbs",
        SaleUnit::measured(MeasurementUnit::Kilogram),
        "0.25".parse().unwrap(),
        "0.5".parse().unwrap(),
        Supply::Unknown,
        LeadSeconds::new(60).unwrap(),
    )
    .unwrap();
    let terms = ListingTerms::new(
        ListingState::Active,
        Product::new("Herbs", "Public description", Some(key()), None).unwrap(),
        choice.id(),
        vec![choice],
        PublicFulfillment::new(
            vec![FulfillmentMode::Pickup, FulfillmentMode::Delivery],
            "Example region",
            LeadSeconds::new(60).unwrap(),
        )
        .unwrap(),
        Some(AuthorityRef::new(
            ExecutorId::from_bytes([4; 32]),
            Generation::new(2).unwrap(),
        )),
    )
    .unwrap();
    let captured = PublishInput::new(
        ListingId::from_bytes([1; 16]),
        "Herbs",
        "Half kilogram; quote required",
        "",
        UnixSeconds::new(100).unwrap(),
        terms,
    )
    .unwrap();
    let value = event(&captured);
    assert_eq!(
        tag(&value, "radroots:price_unit"),
        ["radroots:price_unit", "kg"]
    );
    let profile: serde_json::Value =
        serde_json::from_str(&tag(&value, "radroots:listing")[1]).unwrap();
    assert_eq!(
        profile["authority"],
        serde_json::json!({"executor_id":"04".repeat(32),"generation":"2"})
    );
    assert_eq!(profile["product"]["producer"], key().to_hex());
    assert_eq!(
        profile["options"][0]["unit"],
        serde_json::json!({"type":"measured","unit":"kg"})
    );
    assert_eq!(
        profile["options"][0]["supply"],
        serde_json::json!({"type":"unknown"})
    );
    assert_eq!(profile["options"][0]["lead_seconds"], "60");
    assert_eq!(profile["authority"].as_object().unwrap().len(), 2);
}

#[test]
fn complete_signed_event_limit_counts_escaping_and_fixed_signature_framing() {
    let empty = event(&input(ListingState::Active, ""));
    let overhead = nip01::signed_json_len(&empty.fields(), LISTING_EVENT_MAX_BYTES).unwrap();
    let free = LISTING_EVENT_MAX_BYTES - overhead;
    let body = "\u{0001}".repeat(free / 6) + &"a".repeat(free % 6);
    assert!(body.chars().count() <= 16_384);
    let exact = event(&input(ListingState::Active, &body));
    assert_eq!(
        nip01::signed_json_len(&exact.fields(), LISTING_EVENT_MAX_BYTES).unwrap(),
        LISTING_EVENT_MAX_BYTES
    );
    assert!(matches!(
        ListingEvent::new(
            &input(ListingState::Active, &(body + "a")),
            key(),
            UnixSeconds::new(101).unwrap()
        ),
        Err(WireError::LimitExceeded)
    ));
    assert!(matches!(
        ListingEvent::new(
            &input(ListingState::Active, ""),
            key(),
            UnixSeconds::new(99).unwrap()
        ),
        Err(WireError::InvalidInput)
    ));
}

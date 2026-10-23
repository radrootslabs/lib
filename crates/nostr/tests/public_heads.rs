use radroots_event::envelope::event_head::{EventHeadCoordinate, EventHeadDecision};
use radroots_nostr::public_ingress::{Error, Interpretation, ObservedHead};
use radroots_protocol::advertisement::UnixSeconds;
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/public_heads.v1.json")).unwrap()
}
fn scope(value: &Value, kind: u32) -> EventHeadCoordinate {
    let pubkey = value["author"].as_str().unwrap().parse().unwrap();
    if kind == 30402 {
        EventHeadCoordinate::Addressable {
            kind,
            pubkey,
            d_tag: value["d"].as_str().unwrap().into(),
        }
    } else {
        EventHeadCoordinate::Replaceable { kind, pubkey }
    }
}
fn raw<'a>(value: &'a Value, name: &str) -> &'a str {
    value["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == name)
        .unwrap()["raw"]
        .as_str()
        .unwrap()
}
fn classify(value: Interpretation) -> &'static str {
    match value {
        Interpretation::Listing(_) => "listing",
        Interpretation::Capabilities(_) => "capabilities",
        Interpretation::Unsupported => "unsupported",
        Interpretation::Invalid => "invalid",
        Interpretation::Expired => "expired",
        Interpretation::ClockAhead => "clock_ahead",
    }
}

#[test]
fn independently_signed_public_vectors_authenticate_before_interpretation() {
    let data = fixture();
    let now = UnixSeconds::new(data["observed_at"].as_u64().unwrap()).unwrap();
    for row in data["vectors"].as_array().unwrap() {
        let wire: Value = serde_json::from_str(row["raw"].as_str().unwrap()).unwrap();
        let mut head =
            ObservedHead::new(scope(&data, wire["kind"].as_u64().unwrap() as u32)).unwrap();
        let result = head.observe(row["raw"].as_str().unwrap());
        if row["expected"] == "unauthenticated" {
            assert_eq!(result.unwrap_err(), Error::Unauthenticated);
            assert!(head.raw_json().is_none());
        } else {
            assert!(
                matches!(result, Ok(EventHeadDecision::Applied(_))),
                "{}: {result:?}",
                row["name"]
            );
            assert_eq!(
                classify(head.interpret(now).unwrap()),
                row["expected"].as_str().unwrap(),
                "{}",
                row["name"]
            );
            assert_eq!(head.raw_json(), row["raw"].as_str());
        }
    }
}
#[test]
fn newer_nonactionable_heads_never_restore_an_older_supported_revision() {
    let data = fixture();
    let now = UnixSeconds::new(150).unwrap();
    for (older, newer, expected, kind) in [
        ("listing_fixed", "listing_unsupported", "unsupported", 30402),
        ("listing_fixed", "listing_unknown_field", "invalid", 30402),
        ("listing_fixed", "listing_duplicate_d", "invalid", 30402),
        ("listing_fixed", "listing_future", "clock_ahead", 30402),
        ("capability", "capability_unsupported", "unsupported", 18890),
        ("capability", "capability_expired", "expired", 18890),
        ("capability", "capability_future", "clock_ahead", 18890),
    ] {
        for reverse in [false, true] {
            let mut head = ObservedHead::new(scope(&data, kind)).unwrap();
            let pair = if reverse {
                [newer, older]
            } else {
                [older, newer]
            };
            for name in pair {
                head.observe(raw(&data, name)).unwrap();
            }
            assert_eq!(head.raw_json(), Some(raw(&data, newer)));
            assert_eq!(classify(head.interpret(now).unwrap()), expected);
        }
    }
}
#[test]
fn same_timestamp_lowest_identity_wins_and_invalid_signatures_cannot_replace() {
    let data = fixture();
    let mut pair = [raw(&data, "listing_fixed"), raw(&data, "listing_shuffled")];
    pair.sort_by_key(|raw| {
        serde_json::from_str::<Value>(raw).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    });
    for reverse in [false, true] {
        let mut head = ObservedHead::new(scope(&data, 30402)).unwrap();
        for value in if reverse { [pair[1], pair[0]] } else { pair } {
            head.observe(value).unwrap();
        }
        assert_eq!(head.raw_json(), Some(pair[0]));
        assert_eq!(
            head.observe(raw(&data, "bad_signature")),
            Err(Error::Unauthenticated)
        );
        assert_eq!(
            head.observe(raw(&data, "bad_id")),
            Err(Error::Unauthenticated)
        );
        assert_eq!(head.raw_json(), Some(pair[0]));
        assert_eq!(
            head.observe(pair[0]).unwrap(),
            EventHeadDecision::SkippedDuplicate
        );
    }
}
#[test]
fn explicit_scopes_and_absence_never_fabricate_observations() {
    let data = fixture();
    let mut head = ObservedHead::new(scope(&data, 30402)).unwrap();
    assert!(head.raw_json().is_none());
    assert!(head.selected().is_none());
    assert!(head.interpret(UnixSeconds::new(0).unwrap()).is_none());
    assert_eq!(
        head.observe(raw(&data, "capability")),
        Err(Error::ScopeMismatch)
    );
    assert!(ObservedHead::new(scope(&data, 0)).is_err());
    assert!(head.raw_json().is_none());
}

#![cfg(feature = "terms")]

use radroots_domain::{
    AgreedFixedPrice, AgreementLine, AgreementLineId, AgreementPrice, ContentMeasure, DiscountBps,
    EnvelopeEventId, FixedPrice, FulfillmentGroup, FulfillmentGroupId, FuturePromise,
    ListingOptionId, Money, PositiveDecimal, PriceTier, PrivateFulfillment, PublicKey, Quantity,
    SupplyCommitment, Terms, TimeZoneId, UnixSeconds, VariableWeight, Window, listing::*,
};
use radroots_protocol::terms::{EncodedTerms, TERMS_MAX_BYTES, TermsWireError};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    vectors: Vec<Vector>,
    invalid: Vec<Invalid>,
}
#[derive(Deserialize)]
struct Vector {
    name: String,
    config: Config,
    wire: String,
    utf8_bytes: usize,
    terms_digest: String,
    raw_sha256: String,
    record_domain_digest: String,
}
#[derive(Deserialize)]
struct Invalid {
    config: Config,
    utf8_bytes: usize,
}
#[derive(Deserialize)]
struct Config {
    price: String,
    currency: String,
    exponent: u8,
    contents: String,
    public_window: bool,
    mode: String,
    address: String,
    instructions: String,
    contact: Option<String>,
    lot: Option<String>,
    future: bool,
    instructions_by_group: Option<Vec<String>>,
}
fn fixtures() -> Fixture {
    serde_json::from_str(include_str!("fixtures/terms_wire.v1.json")).unwrap()
}
fn seller() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn buyer() -> PublicKey {
    PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5").unwrap()
}
fn window(start: u64, end: u64) -> Window {
    Window::new(
        UnixSeconds::new(start).unwrap(),
        UnixSeconds::new(end).unwrap(),
        TimeZoneId::from_name("Etc/UTC").unwrap(),
    )
    .unwrap()
}
fn build(config: &Config) -> Terms {
    let money = |minor| Money::from_parts(&config.currency, minor, config.exponent).unwrap();
    let quantity = Quantity::new("500".parse().unwrap(), MeasurementUnit::Gram).unwrap();
    let contents = match config.contents.as_str() {
        "fixed" => ContentMeasure::Fixed { quantity },
        "estimated" => ContentMeasure::Estimated { quantity },
        "unspecified" => ContentMeasure::Unspecified,
        _ => panic!("unknown fixture contents"),
    };
    let quoted = config.price == "negotiated";
    let variable = config.price == "variable";
    let price = if quoted {
        Price::QuoteRequired
    } else if variable {
        Price::VariableWeight(
            VariableWeight::new(
                MeasurementUnit::Gram,
                "250".parse().unwrap(),
                "500".parse().unwrap(),
                money("17"),
                "100".parse().unwrap(),
                money("85"),
            )
            .unwrap(),
        )
    } else {
        Price::Fixed(
            FixedPrice::new(
                money("200"),
                "1".parse().unwrap(),
                DiscountBps::new(250).unwrap(),
                vec![PriceTier::new("2".parse().unwrap(), money("150"))],
            )
            .unwrap(),
        )
    };
    let option = ListingOption::new(
        ListingOptionId::from_bytes([2; 32]),
        "Bag",
        SaleUnit::package("bag", contents).unwrap(),
        "1".parse().unwrap(),
        "1".parse().unwrap(),
        price,
        if quoted {
            Supply::Unknown
        } else {
            Supply::Estimated {
                amount: "0".parse().unwrap(),
                window: config.public_window.then(|| window(100, 200)),
            }
        },
        LeadSeconds::new(0).unwrap(),
    )
    .unwrap();
    let amount: PositiveDecimal = if quoted || variable { "2" } else { "3" }.parse().unwrap();
    let agreed = if quoted {
        AgreementPrice::Fixed(AgreedFixedPrice::new(
            money("325"),
            "2".parse().unwrap(),
            DiscountBps::NONE,
            None,
        ))
    } else if variable {
        AgreementPrice::VariableWeight(
            VariableWeight::new(
                MeasurementUnit::Gram,
                "500".parse().unwrap(),
                "1000".parse().unwrap(),
                money("17"),
                "100".parse().unwrap(),
                money("170"),
            )
            .unwrap(),
        )
    } else {
        AgreementPrice::Fixed(AgreedFixedPrice::from_line(
            option.fixed_line(amount).unwrap(),
        ))
    };
    let total = if quoted {
        "325"
    } else if variable {
        "170"
    } else {
        "439"
    };
    let fee = if quoted { "0" } else { "25" };
    let line = AgreementLine::new(
        AgreementLineId::from_bytes([5; 32]),
        ListingRevision::new(
            ListingCoordinate::new(seller(), ListingId::from_bytes([1; 16])),
            EnvelopeEventId::from_bytes([3; 32]),
        ),
        option.id(),
        Product::new(
            "Carrots 🥕",
            "Public crop",
            Some(seller()),
            Some("Example area"),
        )
        .unwrap(),
        option,
        amount,
        FulfillmentGroupId::from_bytes([4; 32]),
        config.lot.as_deref(),
        agreed,
        money(total),
    )
    .unwrap();
    let instructions = config
        .instructions_by_group
        .as_deref()
        .unwrap_or_else(|| std::slice::from_ref(&config.instructions));
    let groups = instructions
        .iter()
        .enumerate()
        .map(|(index, instruction)| {
            FulfillmentGroup::new(
                FulfillmentGroupId::from_bytes([u8::try_from(4 + index).unwrap(); 32]),
                PrivateFulfillment::new(
                    match config.mode.as_str() {
                        "pickup" => FulfillmentMode::Pickup,
                        "delivery" => FulfillmentMode::Delivery,
                        _ => panic!("unknown fixture mode"),
                    },
                    window(300, 400),
                    &config.address,
                    instruction,
                    config.contact.as_deref(),
                )
                .unwrap(),
            )
        })
        .collect();
    Terms::new(
        buyer(),
        seller(),
        vec![line],
        groups,
        money(fee),
        money(total).checked_add(money(fee)).unwrap(),
        if config.future {
            SupplyCommitment::FuturePromise(
                FuturePromise::new(window(100, 200), "Weather change 🌧").unwrap(),
            )
        } else {
            SupplyCommitment::StockBacked
        },
    )
    .unwrap()
}

#[test]
fn independent_ecmascript_terms_bytes_and_all_hash_domains_match() {
    let fixture = fixtures();
    assert_eq!(fixture.vectors.len(), 9);
    for vector in fixture.vectors {
        let terms = build(&vector.config);
        let encoded = EncodedTerms::capture(&terms).unwrap();
        assert_eq!(
            encoded.protected_wire(),
            vector.wire.as_bytes(),
            "{}",
            vector.name
        );
        assert_eq!(
            encoded.protected_wire().len(),
            vector.utf8_bytes,
            "{}",
            vector.name
        );
        assert_eq!(
            encoded.digest().to_string(),
            vector.terms_digest,
            "{}",
            vector.name
        );
        assert_ne!(encoded.digest().to_string(), vector.raw_sha256);
        assert_ne!(encoded.digest().to_string(), vector.record_domain_digest);
        assert_eq!(
            encoded.retained_bytes(),
            std::mem::size_of::<EncodedTerms>() + vector.utf8_bytes
        );
        assert_eq!(format!("{encoded:?}"), "EncodedTerms(<redacted>)");
        let mut value: serde_json::Value =
            serde_json::from_slice(encoded.protected_wire()).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 9);
        assert!(value.as_object_mut().unwrap().remove("record_id").is_none());
    }
}

#[test]
fn unicode_spelling_and_explicit_optional_values_remain_distinct() {
    let fixture = fixtures();
    let get = |name| {
        fixture
            .vectors
            .iter()
            .find(|value| value.name == name)
            .unwrap()
    };
    for (left, right) in [
        ("unicode_composed", "unicode_decomposed"),
        ("contact_null", "contact_empty"),
    ] {
        let first = EncodedTerms::capture(&build(&get(left).config)).unwrap();
        let second = EncodedTerms::capture(&build(&get(right).config)).unwrap();
        assert_ne!(first.protected_wire(), second.protected_wire());
        assert_ne!(first.digest(), second.digest());
    }
}

#[test]
fn complete_escaped_content_accepts_exact_limit_and_refuses_one_more_byte() {
    let fixture = fixtures();
    let exact = fixture
        .vectors
        .iter()
        .find(|value| value.name == "exact_content_limit")
        .unwrap();
    assert_eq!(
        EncodedTerms::capture(&build(&exact.config))
            .unwrap()
            .protected_wire()
            .len(),
        TERMS_MAX_BYTES
    );
    assert_eq!(fixture.invalid.len(), 1);
    for invalid in fixture.invalid {
        assert_eq!(invalid.utf8_bytes, TERMS_MAX_BYTES + 1);
        let terms = build(&invalid.config);
        assert!(matches!(
            EncodedTerms::capture(&terms),
            Err(TermsWireError::LimitExceeded)
        ));
    }
}

#[test]
fn complete_line_and_fulfillment_array_order_is_part_of_the_terms_identity() {
    let mut config = fixtures().vectors.remove(0).config;
    config.instructions_by_group = Some(vec!["First group".into(), "Second group".into()]);
    let original = build(&config);
    let line = &original.lines()[0];
    let second = AgreementLine::new(
        AgreementLineId::from_bytes([6; 32]),
        line.source(),
        line.option_id(),
        line.product().clone(),
        line.option().clone(),
        line.amount(),
        FulfillmentGroupId::from_bytes([5; 32]),
        line.source_lot_label(),
        line.price(),
        line.maximum_line_total(),
    )
    .unwrap();
    let total = original
        .maximum_total()
        .checked_add(second.maximum_line_total())
        .unwrap();
    let lines = vec![line.clone(), second];
    let groups = original.fulfillment().to_vec();
    let create = |lines, groups| {
        Terms::new(
            original.buyer(),
            original.seller(),
            lines,
            groups,
            original.fulfillment_charge(),
            total,
            original.supply().clone(),
        )
        .unwrap()
    };
    let normal = EncodedTerms::capture(&create(lines.clone(), groups.clone())).unwrap();
    let reversed_lines = EncodedTerms::capture(&create(
        lines.iter().rev().cloned().collect(),
        groups.clone(),
    ))
    .unwrap();
    let reversed_groups =
        EncodedTerms::capture(&create(lines, groups.into_iter().rev().collect())).unwrap();
    assert_ne!(normal.protected_wire(), reversed_lines.protected_wire());
    assert_ne!(normal.digest(), reversed_lines.digest());
    assert_ne!(normal.protected_wire(), reversed_groups.protected_wire());
    assert_ne!(normal.digest(), reversed_groups.digest());
}

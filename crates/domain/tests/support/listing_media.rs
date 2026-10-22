use super::*;

fn reference() -> Media {
    Media::new(
        "https://media.fixture.invalid/carrot?version=1",
        [0xab; 32],
        "image/jpeg",
        "Carrot 🥕\nExact alternative text",
    )
    .unwrap()
}

#[test]
fn media_preserves_bounded_claims_and_exact_digests_without_normalization_or_trust() {
    let value = reference();
    assert_eq!(
        value.url(),
        "https://media.fixture.invalid/carrot?version=1"
    );
    assert_eq!(value.sha256(), &[0xab; 32]);
    assert_eq!(value.mime(), "image/jpeg");
    assert_eq!(value.alt(), "Carrot 🥕\nExact alternative text");
    assert_eq!(
        Media::from_hex_digest(value.url(), &"ab".repeat(32), value.mime(), value.alt()).unwrap(),
        value
    );
    let debug = format!("{value:?}");
    assert!(!debug.contains("fixture"));
    assert!(!debug.contains("image/jpeg"));
    assert!(!debug.contains("Carrot"));
    assert_eq!(value.clone(), value);
    for bad in [
        "ab".repeat(31),
        "ab".repeat(33),
        "AB".repeat(32),
        "g0".repeat(32),
        "0g".repeat(32),
        "é".repeat(32),
    ] {
        assert_eq!(
            Media::from_hex_digest(value.url(), &bad, value.mime(), value.alt()).unwrap_err(),
            ListingError::InvalidMediaDigest
        );
    }
    assert!(
        !ListingError::InvalidMediaDigest
            .to_string()
            .contains(value.url())
    );
}

#[test]
fn media_text_and_collection_bounds_reject_before_owned_capture() {
    let prefix = "https://media.fixture.invalid/";
    let url = prefix.to_owned() + &"a".repeat(2048 - prefix.len());
    let mime = "application/".to_owned() + &"x".repeat(68);
    let alt = "🥕".repeat(240);
    assert_eq!(mime.len(), 80);
    let value = Media::new(&url, [0; 32], &mime, &alt).unwrap();
    assert_eq!(value.alt(), alt);
    for (url, mime, alt) in [
        (String::new(), mime.clone(), alt.clone()),
        (url.clone() + "a", mime.clone(), alt.clone()),
        (url.clone(), String::new(), alt.clone()),
        (url.clone(), mime.clone() + "x", alt.clone()),
        (url.clone(), mime.clone(), alt + "🥕"),
    ] {
        assert_eq!(
            Media::new(&url, [0; 32], &mime, &alt).unwrap_err(),
            ListingError::InvalidText
        );
    }
    assert!(
        Media::new(
            "urn:sha256:declared-object",
            [0; 32],
            "application/octet-stream",
            ""
        )
        .is_ok()
    );
    let make = |media| {
        ListingTerms::new(
            ListingState::Active,
            product(),
            ListingOptionId::from_bytes([1; 32]),
            vec![option(1, SaleUnit::measured(MeasurementUnit::Each), "1", "1", "0").unwrap()],
            fulfillment(),
            None,
            media,
        )
    };
    assert_eq!(ListingTerms::MAX_MEDIA, 8);
    assert!(make(vec![]).unwrap().media().is_empty());
    let mut spare = Vec::with_capacity(4096);
    for _ in 0..8 {
        spare.push(reference());
    }
    let terms = make(spare).unwrap();
    assert_eq!(terms.media(), vec![reference(); 8]);
    assert_eq!(
        make(vec![reference(); 9]).unwrap_err(),
        ListingError::TooManyMedia
    );
    assert!(!ListingError::TooManyMedia.to_string().contains("fixture"));
}

#[cfg(feature = "serde")]
#[test]
fn media_serializes_closed_exact_strings_and_preserves_full_profile_order() {
    use serde_json::json;
    let value = reference();
    assert_eq!(
        serde_json::to_value(&value).unwrap(),
        json!({
        "url":"https://media.fixture.invalid/carrot?version=1","sha256":"ab".repeat(32),
        "mime":"image/jpeg","alt":"Carrot 🥕\nExact alternative text"})
    );
    let second = Media::new(
        "ipfs://declared-object",
        [0; 32],
        "application/octet-stream",
        "",
    )
    .unwrap();
    let terms = ListingTerms::new(
        ListingState::Active,
        product(),
        ListingOptionId::from_bytes([1; 32]),
        vec![option(1, SaleUnit::measured(MeasurementUnit::Each), "1", "1", "0").unwrap()],
        fulfillment(),
        None,
        vec![value.clone(), second.clone()],
    )
    .unwrap();
    let wire = serde_json::to_value(&terms).unwrap();
    assert_eq!(wire["media"], json!([value, second]));
    assert_eq!(wire.as_object().unwrap().len(), 7);
    assert!(!format!("{terms:?}").contains("fixture"));
}

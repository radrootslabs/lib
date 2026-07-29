use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};
use radroots_event::food::availability::{
    FoodAvailabilityDetails, FoodAvailabilityDetailsParts, FoodAvailabilityImage,
    FoodAvailabilityStatus, FoodContent, FoodCurrency, FoodIdentifier, FoodImageDimensions,
    FoodPrice, FoodPublishedAt, FoodQuantity, FoodText, FoodUnit,
};
use radroots_event::media::AuthoredImage;
use radroots_event_codec::food_availability::admission::{
    RadrootsFoodAvailabilityAdmissionOutcome, verify_and_admit_food_availability_event,
};
use radroots_nostr::prelude::{
    RadrootsNostrError, RadrootsNostrGenericEventBuilder, RadrootsNostrKeys, RadrootsNostrKind,
    RadrootsNostrSecretKey, RadrootsNostrTag, RadrootsNostrTimestamp, radroots_event_from_nostr,
    radroots_nostr_build_food_availability_event,
};
use radroots_test_fixtures::FIXTURE_ALICE_SECRET_KEY_HEX;

#[cfg(feature = "client")]
use radroots_nostr::prelude::RadrootsNostrClient;

const CREATED_AT: u64 = 1_784_347_200;

#[test]
fn typed_food_builder_signs_the_exact_strict_profile() {
    let keys = fixture_keys();
    let created_at = RadrootsNostrTimestamp::from_secs(CREATED_AT);
    let event = radroots_nostr_build_food_availability_event(&details(), created_at)
        .expect("typed FoodAvailability builder")
        .sign_with_keys(&keys)
        .expect("signed FoodAvailability event");

    assert_eq!(
        event.kind,
        RadrootsNostrKind::Custom(
            u16::try_from(radroots_event::envelope::kind::KIND_CLASSIFIED_LISTING)
                .expect("classified listing kind")
        )
    );
    assert_eq!(event.created_at, created_at);
    assert_eq!(
        event
            .tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect::<Vec<_>>(),
        vec![
            vec!["d", "nantes-carrots"],
            vec!["title", "Nantes Carrots"],
            vec!["summary", "Fresh bunches"],
            vec!["published_at", "1784347100"],
            vec!["location", "Central Saanich, BC"],
            vec!["price", "3", "CAD"],
            vec!["radroots:price_unit", "lb"],
            vec!["radroots:quantity", "24", "lb"],
            vec!["status", "active"],
        ]
    );
    event.verify().expect("valid NIP-01 event");

    let envelope = radroots_event_from_nostr(&event).expect("Radroots event adapter");
    assert!(matches!(
        verify_and_admit_food_availability_event(envelope)
            .expect("verified FoodAvailability admission"),
        RadrootsFoodAvailabilityAdmissionOutcome::Admitted(_)
    ));
}

#[test]
fn typed_food_builder_keeps_timestamp_validation_inside_construction() {
    let error = radroots_nostr_build_food_availability_event(
        &details(),
        RadrootsNostrTimestamp::from_secs(1_784_347_000),
    )
    .err()
    .expect("created_at before published_at must fail");

    assert!(matches!(
        error,
        RadrootsNostrError::FoodAvailabilityEncode(_)
    ));
}

#[test]
fn typed_food_builder_preserves_a_byte_verified_blossom_image_tuple() {
    let image = blossom_image();
    let image_url = image.url().to_owned();
    let event = radroots_nostr_build_food_availability_event(
        &details_with_images(vec![image]),
        RadrootsNostrTimestamp::from_secs(CREATED_AT),
    )
    .expect("typed media FoodAvailability builder")
    .sign_with_keys(&fixture_keys())
    .expect("signed media FoodAvailability event");

    let image_tags = event
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().is_some_and(|name| name == "image"))
        .map(|tag| tag.as_slice().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(
        image_tags,
        vec![vec!["image".to_owned(), image_url, "640x480".to_owned(),]]
    );
}

#[test]
fn generic_builder_reserves_focused_and_ambiguous_listing_profiles() {
    let keys = fixture_keys();
    let kind = RadrootsNostrKind::Custom(
        u16::try_from(radroots_event::envelope::kind::KIND_CLASSIFIED_LISTING)
            .expect("classified listing kind"),
    );

    for tags in [
        vec![
            RadrootsNostrTag::parse(["d", "focused"]).expect("d tag"),
            RadrootsNostrTag::parse(["radroots:price_unit", "lb"]).expect("focused marker"),
        ],
        vec![
            RadrootsNostrTag::parse(["d", "ambiguous"]).expect("d tag"),
            RadrootsNostrTag::parse(["radroots:price_unit", "lb"]).expect("focused marker"),
            RadrootsNostrTag::parse(["radroots:primary_bin", "bin-1"]).expect("operational marker"),
        ],
    ] {
        assert!(matches!(
            RadrootsNostrGenericEventBuilder::new(kind, "reserved")
                .tags(tags)
                .sign_with_keys(&keys),
            Err(RadrootsNostrError::TypedAuthoringRequired { kind: 30_402 })
        ));
    }
}

#[test]
fn generic_builder_retains_marker_free_and_operational_nip99_compatibility() {
    let keys = fixture_keys();
    let kind = RadrootsNostrKind::Custom(
        u16::try_from(radroots_event::envelope::kind::KIND_CLASSIFIED_LISTING)
            .expect("classified listing kind"),
    );

    for tags in [
        vec![RadrootsNostrTag::parse(["d", "generic"]).expect("d tag")],
        vec![
            RadrootsNostrTag::parse(["d", "operational"]).expect("d tag"),
            RadrootsNostrTag::parse(["radroots:primary_bin", "bin-1"]).expect("operational marker"),
        ],
    ] {
        let event = RadrootsNostrGenericEventBuilder::new(kind, "compatible")
            .tags(tags)
            .custom_created_at(RadrootsNostrTimestamp::from_secs(CREATED_AT))
            .sign_with_keys(&keys)
            .expect("non-focused NIP-99 remains generic-authorable");
        event.verify().expect("valid generic NIP-99 event");
    }
}

#[cfg(feature = "client")]
#[tokio::test]
async fn typed_food_builder_reaches_client_publication() {
    let client = RadrootsNostrClient::new(fixture_keys());
    let builder = radroots_nostr_build_food_availability_event(
        &details(),
        RadrootsNostrTimestamp::from_secs(CREATED_AT),
    )
    .expect("typed FoodAvailability builder");

    let error = client
        .send_food_availability_event_builder(builder)
        .await
        .expect_err("no relay is configured");

    assert!(matches!(error, RadrootsNostrError::ClientError(_)));
}

fn fixture_keys() -> RadrootsNostrKeys {
    RadrootsNostrKeys::new(
        RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("fixture secret key"),
    )
}

fn details() -> FoodAvailabilityDetails {
    details_with_images(Vec::new())
}

fn details_with_images(images: Vec<FoodAvailabilityImage>) -> FoodAvailabilityDetails {
    FoodAvailabilityDetails::new(FoodAvailabilityDetailsParts {
        content: FoodContent::new("Carrots available this week.").expect("content"),
        identifier: FoodIdentifier::parse("nantes-carrots").expect("identifier"),
        title: FoodText::new("Nantes Carrots").expect("title"),
        summary: FoodText::new("Fresh bunches").expect("summary"),
        published_at: FoodPublishedAt::new(1_784_347_100).expect("published_at"),
        location: FoodText::new("Central Saanich, BC").expect("location"),
        price: FoodPrice::new(
            "3",
            FoodCurrency::parse("CAD").expect("currency"),
            FoodUnit::Pound,
        )
        .expect("price"),
        quantity: Some(FoodQuantity::new("24", FoodUnit::Pound).expect("quantity")),
        status: FoodAvailabilityStatus::Active,
        images,
    })
    .expect("FoodAvailability details")
}

fn blossom_image() -> FoodAvailabilityImage {
    let bytes = b"victoria-carrots-image-fixture";
    let hash = Sha256::digest(bytes);
    let media_type = MediaType::parse("image/webp").expect("image media type");
    let verified = BlobDescriptor::new(
        BlobUrl::parse(&format!("https://media.example/{hash}.webp")).expect("Blossom URL"),
        hash,
        bytes.len() as u64,
        media_type.clone(),
        CREATED_AT,
    )
    .expect("Blossom descriptor")
    .approve_reference()
    .expect("approved Blossom reference")
    .verify_bytes(bytes, &media_type)
    .expect("byte-verified Blossom descriptor");
    let image = AuthoredImage::try_from_verified_descriptor(verified).expect("authored image");
    let dimensions = FoodImageDimensions::new(640, 480).expect("image dimensions");
    FoodAvailabilityImage::new(image, dimensions)
}

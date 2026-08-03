#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
};

use radroots_core::{Decimal, Money, Quantity, Unit};
use radroots_event::{
    envelope::kind::{KIND_CLASSIFIED_LISTING, is_classified_listing_kind},
    farm::location::{has_textual_locality, is_public_geohash5},
    id::ClassifiedListingAddress,
    listing::classified::{ClassifiedListingPartition, classify_classified_listing_tags},
    listing::operational::{
        OperationalListing, OperationalListingAvailability, OperationalListingBin,
        OperationalListingDeliveryMethod, OperationalListingPublicLocation,
    },
    trade::validation::OperationalListingValidationError,
};
use radroots_identity::PublicKey;

use radroots_event_codec::{
    operational_listing::decode::operational_listing_from_nostr_event,
    verification::RadrootsSignatureVerifiedEvent,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsOperationalListingTradeProjection {
    pub listing_id: String,
    pub listing_addr: ClassifiedListingAddress,
    pub seller_pubkey: String,
    pub title: String,
    pub description: String,
    pub product_type: String,
    pub primary_bin_id: String,
    pub bin_quantity: Quantity,
    pub unit: Unit,
    pub unit_price: Money,
    pub inventory_available: Decimal,
    pub availability: OperationalListingAvailability,
    pub location: OperationalListingPublicLocation,
    pub delivery_method: OperationalListingDeliveryMethod,
    pub listing: OperationalListing,
}

/// Validates a signature-verified Operational Listing event.
///
/// A plain envelope cannot cross this boundary:
///
/// ```compile_fail
/// use radroots_event::envelope::EventEnvelope;
/// use radroots_trade::operational_listing::validation::validate_operational_listing_event;
///
/// fn validate_unverified(event: &EventEnvelope) {
///     let _ = validate_operational_listing_event(event);
/// }
/// ```
pub fn validate_operational_listing_event(
    verified_event: &RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsOperationalListingTradeProjection, OperationalListingValidationError> {
    let event = verified_event.event();
    if !is_classified_listing_kind(event.kind_u32()) {
        return Err(OperationalListingValidationError::InvalidKind {
            kind: event.kind_u32(),
        });
    }
    if classify_classified_listing_tags(event.tags())
        != ClassifiedListingPartition::OperationalListing
    {
        return Err(OperationalListingValidationError::InvalidProfile);
    }

    let listing = operational_listing_from_nostr_event(event)
        .map_err(|error| OperationalListingValidationError::ParseError { error })?;
    validate_operational_listing_model(listing, event.author())
}

/// Validates the trade semantics of an unsigned Operational Listing model.
///
/// The seller is typed independently from the model because it is the
/// authority against which the listing's farm identity is checked. This
/// function does not perform event-kind, profile, decoding, or signature
/// checks; callers handling Nostr events must use
/// [`validate_operational_listing_event`] instead.
pub fn validate_operational_listing_model(
    listing: OperationalListing,
    seller_pubkey: &PublicKey,
) -> Result<RadrootsOperationalListingTradeProjection, OperationalListingValidationError> {
    let listing_id = listing.d_tag.as_str().trim().to_string();

    if listing.farm.pubkey != seller_pubkey.to_hex() {
        return Err(OperationalListingValidationError::InvalidSeller);
    }
    let listing_addr_raw = format!("{KIND_CLASSIFIED_LISTING}:{}:{listing_id}", seller_pubkey);
    let listing_addr = ClassifiedListingAddress::parse(&listing_addr_raw)
        .expect("validated listing identity must form a listing address");

    let title = listing.product.title.trim().to_string();
    if title.is_empty() {
        return Err(OperationalListingValidationError::MissingTitle);
    }

    let description = listing
        .product
        .summary
        .as_ref()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if description.is_empty() {
        return Err(OperationalListingValidationError::MissingDescription);
    }

    let product_type = if !listing.product.category.trim().is_empty() {
        listing.product.category.trim().to_string()
    } else {
        listing.product.key.trim().to_string()
    };
    if product_type.is_empty() {
        return Err(OperationalListingValidationError::MissingProductType);
    }

    if listing.bins.is_empty() {
        return Err(OperationalListingValidationError::MissingBins);
    }
    let primary_bin_id = listing.primary_bin_id.as_str().trim().to_string();
    let primary_bin_index = listing
        .bins
        .iter()
        .position(|bin| bin.bin_id.as_str() == primary_bin_id)
        .ok_or(OperationalListingValidationError::MissingPrimaryBin)?;
    for (index, bin) in listing.bins.iter().enumerate() {
        if listing.bins[..index]
            .iter()
            .any(|seen| seen.bin_id == bin.bin_id)
        {
            return Err(OperationalListingValidationError::InvalidBin);
        }
    }
    let primary_bin = &listing.bins[primary_bin_index];

    validate_listing_bin(primary_bin)?;
    for (index, bin) in listing.bins.iter().enumerate() {
        if index != primary_bin_index {
            validate_listing_bin(bin)?;
        }
    }

    let inventory_available = listing
        .inventory_available
        .ok_or(OperationalListingValidationError::MissingInventory)?;
    if inventory_available.is_sign_negative() {
        return Err(OperationalListingValidationError::InvalidInventory);
    }

    let availability = listing
        .availability
        .clone()
        .ok_or(OperationalListingValidationError::MissingAvailability)?;
    let location = listing
        .location
        .clone()
        .ok_or(OperationalListingValidationError::MissingLocation)?;
    if !has_textual_locality(
        &location.primary,
        location.city.as_deref(),
        location.region.as_deref(),
        location.country.as_deref(),
    ) {
        return Err(OperationalListingValidationError::MissingLocationLocality);
    }
    validate_listing_location_geohash(&location.geohash)?;
    let delivery_method = listing
        .delivery_method
        .clone()
        .ok_or(OperationalListingValidationError::MissingDeliveryMethod)?;

    Ok(RadrootsOperationalListingTradeProjection {
        listing_id,
        listing_addr,
        seller_pubkey: seller_pubkey.to_hex(),
        title,
        description,
        product_type,
        primary_bin_id: primary_bin_id.clone(),
        bin_quantity: primary_bin.quantity.clone(),
        unit: primary_bin.quantity.unit(),
        unit_price: primary_bin.price_per_canonical_unit.amount().clone(),
        inventory_available,
        availability,
        location,
        delivery_method,
        listing,
    })
}

fn validate_listing_bin(
    bin: &OperationalListingBin,
) -> Result<(), OperationalListingValidationError> {
    if bin.quantity.amount().is_sign_negative() || !bin.quantity.is_canonical() {
        return Err(OperationalListingValidationError::InvalidBin);
    }
    if !bin.price_per_canonical_unit.is_price_per_canonical_unit()
        || bin
            .price_per_canonical_unit
            .amount()
            .amount()
            .is_sign_negative()
        || bin.price_per_canonical_unit.quantity().unit() != bin.quantity.unit()
    {
        return Err(OperationalListingValidationError::InvalidPrice);
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn validate_listing_location_geohash(
    geohash: &str,
) -> Result<(), OperationalListingValidationError> {
    if geohash.trim().is_empty() {
        return Err(OperationalListingValidationError::MissingLocationGeohash);
    }
    if !is_public_geohash5(geohash) {
        return Err(OperationalListingValidationError::InvalidLocationGeohash);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        OperationalListingValidationError, validate_operational_listing_event,
        validate_operational_listing_model,
    };
    use nostr::{EventBuilder, JsonUtil, Keys, Kind, Tag, Timestamp};
    use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
    use radroots_event::{
        envelope::EventEnvelope,
        envelope::EventEnvelopeParts,
        envelope::kind::KIND_CLASSIFIED_LISTING,
        farm::FarmRef,
        id::{DTag, InventoryBinId},
        listing::operational::{
            OperationalListing, OperationalListingAvailability, OperationalListingBin,
            OperationalListingDeliveryMethod, OperationalListingProduct,
            OperationalListingPublicLocation,
        },
        wire::Nip01EventWire,
    };
    use radroots_event_codec::verification::{RadrootsSignatureVerifiedEvent, verify_nip01_event};
    use radroots_identity::PublicKey;
    use radroots_test_fixtures::{
        FIXTURE_ALICE_PUBLIC_KEY_HEX, FIXTURE_ALICE_SECRET_KEY_HEX, FIXTURE_BOB_PUBLIC_KEY_HEX,
        FIXTURE_BOB_SECRET_KEY_HEX,
    };

    const SELLER: &str = FIXTURE_ALICE_PUBLIC_KEY_HEX;
    const OTHER_SELLER: &str = FIXTURE_BOB_PUBLIC_KEY_HEX;

    fn d_tag(raw: &str) -> DTag {
        DTag::parse(raw).expect("d tag")
    }

    fn bin_id(raw: &str) -> InventoryBinId {
        InventoryBinId::parse(raw).expect("bin id")
    }

    fn seller_pubkey() -> PublicKey {
        PublicKey::from_hex(SELLER).expect("seller pubkey")
    }

    fn other_seller_pubkey() -> PublicKey {
        PublicKey::from_hex(OTHER_SELLER).expect("other seller pubkey")
    }

    fn base_listing() -> OperationalListing {
        OperationalListing {
            d_tag: d_tag("AAAAAAAAAAAAAAAAAAAAAg"),
            published_at: None,
            farm: FarmRef {
                pubkey: SELLER.into(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
            },
            product: OperationalListingProduct {
                key: "coffee".into(),
                title: "Coffee".into(),
                category: "coffee".into(),
                summary: Some("Single origin coffee".into()),
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: bin_id("bin-1"),
            bins: vec![OperationalListingBin {
                bin_id: bin_id("bin-1"),
                quantity: Quantity::try_new(Decimal::from(1000u32), Unit::MassG).unwrap(),
                price_per_canonical_unit: QuantityPrice::try_new(
                    Money::try_new(Decimal::from(20u32), Currency::USD).unwrap(),
                    Quantity::try_new(Decimal::from(1u32), Unit::MassG).unwrap(),
                )
                .unwrap(),
                display_amount: None,
                display_unit: None,
                display_label: None,
                display_price: None,
                display_price_unit: None,
            }],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: Some(Decimal::from(5u32)),
            availability: Some(OperationalListingAvailability::Status {
                status: radroots_event::listing::operational::OperationalListingStatus::Active,
            }),
            delivery_method: Some(OperationalListingDeliveryMethod::Pickup),
            location: Some(OperationalListingPublicLocation {
                primary: "Farm".into(),
                city: Some("Town".into()),
                region: Some("Region".into()),
                country: Some("US".into()),
                geohash: "9q8yy".into(),
            }),
            images: None,
        }
    }

    fn base_event(listing: &OperationalListing) -> RadrootsSignatureVerifiedEvent {
        let mut tags = vec![
            vec!["d".into(), listing.d_tag.to_string()],
            vec!["p".into(), listing.farm.pubkey.clone()],
            vec![
                "a".into(),
                format!("30340:{}:{}", listing.farm.pubkey, listing.farm.d_tag),
            ],
            vec!["key".into(), listing.product.key.clone()],
            vec!["title".into(), listing.product.title.clone()],
            vec!["category".into(), listing.product.category.clone()],
            vec![
                "summary".into(),
                listing.product.summary.clone().unwrap_or_default(),
            ],
            vec![
                "radroots:primary_bin".into(),
                listing.primary_bin_id.to_string(),
            ],
        ];
        for bin in &listing.bins {
            tags.push(vec![
                "radroots:bin".into(),
                bin.bin_id.to_string(),
                bin.quantity.amount().to_string(),
                bin.quantity.unit().code().to_string(),
            ]);
            tags.push(vec![
                "radroots:price".into(),
                bin.bin_id.to_string(),
                bin.price_per_canonical_unit.amount().amount().to_string(),
                bin.price_per_canonical_unit
                    .amount()
                    .currency()
                    .as_str()
                    .to_string(),
                bin.price_per_canonical_unit.quantity().amount().to_string(),
                bin.price_per_canonical_unit
                    .quantity()
                    .unit()
                    .code()
                    .to_string(),
            ]);
        }
        if let Some(inventory) = listing.inventory_available {
            tags.push(vec!["inventory".into(), inventory.to_string()]);
        }
        if let Some(availability) = &listing.availability {
            match availability {
                OperationalListingAvailability::Status { status } => tags.push(vec![
                    "status".into(),
                    match status {
                        radroots_event::listing::operational::OperationalListingStatus::Active => {
                            "active".into()
                        }
                        radroots_event::listing::operational::OperationalListingStatus::Sold => {
                            "sold".into()
                        }
                        radroots_event::listing::operational::OperationalListingStatus::Other {
                            value,
                        } => value.clone(),
                    },
                ]),
                OperationalListingAvailability::Window { start, end } => {
                    if let Some(start) = start {
                        tags.push(vec![
                            "radroots:availability_start".into(),
                            start.to_string(),
                        ]);
                    }
                    if let Some(end) = end {
                        tags.push(vec!["expires_at".into(), end.to_string()]);
                    }
                }
            }
        }
        if let Some(delivery) = &listing.delivery_method {
            let mut tag = vec!["delivery".into()];
            match delivery {
                OperationalListingDeliveryMethod::Pickup => tag.push("pickup".into()),
                OperationalListingDeliveryMethod::LocalDelivery => {
                    tag.push("local_delivery".into())
                }
                OperationalListingDeliveryMethod::Shipping => tag.push("shipping".into()),
                OperationalListingDeliveryMethod::Other { method } => {
                    tag.push("other".into());
                    tag.push(method.clone());
                }
            }
            tags.push(tag);
        }
        if let Some(location) = &listing.location {
            tags.push(vec![
                "location".into(),
                location.primary.clone(),
                location.city.clone().unwrap_or_default(),
                location.region.clone().unwrap_or_default(),
                location.country.clone().unwrap_or_default(),
            ]);
            tags.push(vec!["g".into(), location.geohash.clone()]);
        }

        event_with_parts(SELLER, KIND_CLASSIFIED_LISTING, tags, String::new())
    }

    fn event_with_parts(
        author: &str,
        kind: u32,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> RadrootsSignatureVerifiedEvent {
        let secret = match author {
            SELLER => FIXTURE_ALICE_SECRET_KEY_HEX,
            OTHER_SELLER => FIXTURE_BOB_SECRET_KEY_HEX,
            _ => panic!("test author must be an approved fixture identity"),
        };
        let keys = Keys::parse(secret).expect("fixture signing key");
        let tags = tags
            .into_iter()
            .map(|tag| Tag::parse(tag).expect("test tag"))
            .collect::<Vec<_>>();
        let event = EventBuilder::new(
            Kind::Custom(u16::try_from(kind).expect("test kind")),
            content,
        )
        .tags(tags)
        .allow_self_tagging()
        .custom_created_at(Timestamp::from_secs(1))
        .sign_with_keys(&keys)
        .expect("signed test event");
        let raw_json = event.as_json();
        let envelope = Nip01EventWire::parse_json(raw_json.as_str())
            .expect("canonical event wire")
            .into_envelope()
            .expect("event envelope");
        verify_nip01_event(envelope).expect("verified test event")
    }

    fn assert_validation_err(
        listing: OperationalListing,
        expected: OperationalListingValidationError,
    ) {
        let event = base_event(&listing);
        let err = validate_operational_listing_event(&event).unwrap_err();
        assert_eq!(format!("{err}"), format!("{expected}"));
    }

    fn assert_secondary_bin_model_error(
        update: impl FnOnce(&mut OperationalListingBin),
        expected: OperationalListingValidationError,
    ) {
        let mut listing = listing_with_secondary_bin();
        update(&mut listing.bins[1]);

        assert_eq!(
            validate_operational_listing_model(listing, &seller_pubkey())
                .expect_err("invalid secondary bin"),
            expected
        );
    }

    fn listing_with_secondary_bin() -> OperationalListing {
        let mut listing = base_listing();
        let mut secondary_bin = listing.bins[0].clone();
        secondary_bin.bin_id = bin_id("bin-2");
        listing.bins.push(secondary_bin);
        listing
    }

    #[test]
    fn validate_listing_ok() {
        let listing = base_listing();
        let event = base_event(&listing);
        assert!(validate_operational_listing_event(&event).is_ok());
    }

    #[test]
    #[cfg(feature = "json")]
    fn model_and_verified_event_validation_return_the_same_projection() {
        let listing = listing_with_secondary_bin();
        let event_projection =
            validate_operational_listing_event(&base_event(&listing)).expect("event projection");
        let model_projection = validate_operational_listing_model(listing, &seller_pubkey())
            .expect("model projection");

        assert_eq!(
            serde_json::to_value(model_projection).expect("model projection JSON"),
            serde_json::to_value(event_projection).expect("event projection JSON")
        );
    }

    #[test]
    fn model_and_verified_event_validation_return_the_same_semantic_errors() {
        let mut listing = base_listing();
        listing.inventory_available = None;
        let event_error = validate_operational_listing_event(&base_event(&listing))
            .expect_err("event inventory error");
        let model_error = validate_operational_listing_model(listing, &seller_pubkey())
            .expect_err("model inventory error");
        assert_eq!(model_error, event_error);

        let listing = base_listing();
        let event = event_with_parts(
            OTHER_SELLER,
            KIND_CLASSIFIED_LISTING,
            base_event(&listing).event().tags_as_vec(),
            String::new(),
        );
        let event_error =
            validate_operational_listing_event(&event).expect_err("event seller error");
        let model_error = validate_operational_listing_model(listing, &other_seller_pubkey())
            .expect_err("model seller error");
        assert_eq!(model_error, event_error);
    }

    #[test]
    fn model_validation_reports_errors_before_event_encoding() {
        let mut listing = base_listing();
        listing.bins.clear();
        assert_eq!(
            validate_operational_listing_model(listing, &seller_pubkey())
                .expect_err("missing bins"),
            OperationalListingValidationError::MissingBins
        );

        let mut listing = base_listing();
        listing.primary_bin_id = bin_id("missing");
        assert_eq!(
            validate_operational_listing_model(listing, &seller_pubkey())
                .expect_err("missing primary bin"),
            OperationalListingValidationError::MissingPrimaryBin
        );

        let mut listing = base_listing();
        listing.bins.push(listing.bins[0].clone());
        assert_eq!(
            validate_operational_listing_model(listing, &seller_pubkey())
                .expect_err("duplicate bin ID"),
            OperationalListingValidationError::InvalidBin
        );

        let mut listing = base_listing();
        listing.location.as_mut().expect("location").geohash = " ".into();
        assert_eq!(
            validate_operational_listing_model(listing, &seller_pubkey())
                .expect_err("missing geohash"),
            OperationalListingValidationError::MissingLocationGeohash
        );

        let mut listing = base_listing();
        listing.location.as_mut().expect("location").geohash = "9q8yyz".into();
        assert_eq!(
            validate_operational_listing_model(listing, &seller_pubkey())
                .expect_err("invalid geohash"),
            OperationalListingValidationError::InvalidLocationGeohash
        );
    }

    #[test]
    fn model_validation_rejects_invalid_secondary_bin_quantities() {
        assert_secondary_bin_model_error(
            |bin| bin.quantity = Quantity::try_new(Decimal::ONE, Unit::MassKg).unwrap(),
            OperationalListingValidationError::InvalidBin,
        );
    }

    #[test]
    fn model_validation_rejects_invalid_secondary_bin_prices() {
        assert_secondary_bin_model_error(
            |bin| {
                bin.price_per_canonical_unit = QuantityPrice::try_new(
                    bin.price_per_canonical_unit.amount().clone(),
                    Quantity::try_new(Decimal::from(2u32), Unit::MassG).unwrap(),
                )
                .unwrap();
            },
            OperationalListingValidationError::InvalidPrice,
        );
        assert_secondary_bin_model_error(
            |bin| {
                bin.price_per_canonical_unit = QuantityPrice::try_new(
                    bin.price_per_canonical_unit.amount().clone(),
                    Quantity::try_new(Decimal::ONE, Unit::MassKg).unwrap(),
                )
                .unwrap();
            },
            OperationalListingValidationError::InvalidPrice,
        );
        assert_secondary_bin_model_error(
            |bin| {
                bin.price_per_canonical_unit = QuantityPrice::try_new(
                    bin.price_per_canonical_unit.amount().clone(),
                    Quantity::try_new(Decimal::ONE, Unit::Each).unwrap(),
                )
                .unwrap();
            },
            OperationalListingValidationError::InvalidPrice,
        );
    }

    #[test]
    fn validate_listing_rejects_retired_kind() {
        let listing = base_listing();
        let event = event_with_parts(
            SELLER,
            30403,
            base_event(&listing).event().tags_as_vec(),
            String::new(),
        );
        let err = validate_operational_listing_event(&event).unwrap_err();
        assert_eq!(
            err,
            OperationalListingValidationError::InvalidKind { kind: 30403 }
        );
    }

    #[test]
    fn validate_listing_rejects_missing_d_tag() {
        let event = event_with_parts(SELLER, KIND_CLASSIFIED_LISTING, Vec::new(), String::new());
        let err = validate_operational_listing_event(&event).unwrap_err();
        assert_eq!(err, OperationalListingValidationError::InvalidProfile);
    }

    #[test]
    fn validate_listing_rejects_invalid_currency() {
        let event = event_with_parts(
            SELLER,
            KIND_CLASSIFIED_LISTING,
            vec![
                vec!["d".into(), "AAAAAAAAAAAAAAAAAAAAAg".into()],
                vec!["p".into(), SELLER.into()],
                vec!["a".into(), format!("30340:{SELLER}:AAAAAAAAAAAAAAAAAAAAAA")],
                vec!["key".into(), "coffee".into()],
                vec!["title".into(), "Coffee".into()],
                vec!["category".into(), "coffee".into()],
                vec!["summary".into(), "Single origin".into()],
                vec!["radroots:primary_bin".into(), "bin-1".into()],
                vec![
                    "quantity".into(),
                    "1".into(),
                    "lb".into(),
                    "bag".into(),
                    "5".into(),
                ],
                vec![
                    "price".into(),
                    "20".into(),
                    "US".into(),
                    "1".into(),
                    "lb".into(),
                ],
                vec![
                    "location".into(),
                    "Farm".into(),
                    "Town".into(),
                    "Region".into(),
                ],
                vec!["status".into(), "active".into()],
                vec!["delivery".into(), "pickup".into()],
            ],
            String::new(),
        );
        let err = validate_operational_listing_event(&event).unwrap_err();
        assert!(format!("{err:?}").starts_with("ParseError"));
    }

    #[test]
    fn validate_listing_rejects_mismatched_seller() {
        let listing = base_listing();
        let event = event_with_parts(
            OTHER_SELLER,
            KIND_CLASSIFIED_LISTING,
            base_event(&listing).event().tags_as_vec(),
            String::new(),
        );
        let err = validate_operational_listing_event(&event).unwrap_err();
        assert_eq!(err, OperationalListingValidationError::InvalidSeller);
    }

    #[test]
    fn validate_listing_rejects_missing_inventory() {
        let mut listing = base_listing();
        listing.inventory_available = None;
        let event = base_event(&listing);
        let err = validate_operational_listing_event(&event).unwrap_err();
        assert_eq!(err, OperationalListingValidationError::MissingInventory);
    }

    #[test]
    fn validate_listing_rejects_invalid_kind() {
        let listing = base_listing();
        let event = event_with_parts(
            SELLER,
            0,
            base_event(&listing).event().tags_as_vec(),
            String::new(),
        );
        let err = validate_operational_listing_event(&event).unwrap_err();
        assert_eq!(
            err,
            OperationalListingValidationError::InvalidKind { kind: 0 }
        );
    }

    #[test]
    fn validate_listing_rejects_missing_title() {
        let mut listing = base_listing();
        listing.product.title = " ".into();
        assert_validation_err(listing, OperationalListingValidationError::MissingTitle);
    }

    #[test]
    fn tampered_envelope_cannot_reach_operational_validation() {
        let verified = base_event(&base_listing());
        let event = verified.into_event();
        let tampered = EventEnvelope::new(EventEnvelopeParts {
            id: event.id_hex(),
            author: event.author().to_hex().to_owned(),
            created_at: event.created_at_u64(),
            kind: event.kind_u32(),
            tags: event.tags_as_vec(),
            content: "tampered".to_owned(),
            sig: event.signature_hex(),
        })
        .expect("well-shaped tampered envelope");

        assert!(verify_nip01_event(tampered).is_err());
    }

    #[test]
    fn validate_listing_rejects_missing_description() {
        let mut listing = base_listing();
        listing.product.summary = Some(" ".into());
        assert_validation_err(
            listing,
            OperationalListingValidationError::MissingDescription,
        );
    }

    #[test]
    fn validate_listing_rejects_missing_product_type() {
        let mut listing = base_listing();
        listing.product.category = " ".into();
        listing.product.key = " ".into();
        assert_validation_err(
            listing,
            OperationalListingValidationError::MissingProductType,
        );
    }

    #[test]
    fn validate_listing_rejects_missing_bins() {
        let mut listing = base_listing();
        listing.bins.clear();
        assert_validation_err(
            listing,
            OperationalListingValidationError::ParseError {
                error:
                    radroots_event::listing::operational::OperationalListingParseError::InvalidTag(
                        "radroots:primary_bin".into(),
                    ),
            },
        );
    }

    #[test]
    fn validate_listing_rejects_missing_primary_bin_id() {
        assert!(InventoryBinId::parse(" ").is_err());
    }

    #[test]
    fn validate_listing_rejects_primary_bin_not_found() {
        let mut listing = base_listing();
        listing.primary_bin_id = bin_id("missing");
        assert_validation_err(
            listing,
            OperationalListingValidationError::ParseError {
                error:
                    radroots_event::listing::operational::OperationalListingParseError::InvalidTag(
                        "radroots:primary_bin".into(),
                    ),
            },
        );
    }

    #[test]
    fn validate_listing_rejects_non_canonical_quantity() {
        let mut listing = base_listing();
        listing.bins[0].quantity = Quantity::try_new(Decimal::ONE, Unit::MassKg).unwrap();
        assert_validation_err(
            listing,
            OperationalListingValidationError::ParseError {
                error:
                    radroots_event::listing::operational::OperationalListingParseError::InvalidTag(
                        "radroots:bin".into(),
                    ),
            },
        );
    }

    #[test]
    fn validate_listing_rejects_non_canonical_price_quantity() {
        let mut listing = base_listing();
        listing.bins[0].price_per_canonical_unit = QuantityPrice::try_new(
            listing.bins[0].price_per_canonical_unit.amount().clone(),
            Quantity::try_new(Decimal::ONE, Unit::MassKg).unwrap(),
        )
        .unwrap();
        assert_validation_err(
            listing,
            OperationalListingValidationError::ParseError {
                error:
                    radroots_event::listing::operational::OperationalListingParseError::InvalidTag(
                        "radroots:price".into(),
                    ),
            },
        );
    }

    #[test]
    fn validate_listing_rejects_price_unit_mismatch() {
        let mut listing = base_listing();
        listing.bins[0].price_per_canonical_unit = QuantityPrice::try_new(
            listing.bins[0].price_per_canonical_unit.amount().clone(),
            Quantity::try_new(Decimal::ONE, Unit::Each).unwrap(),
        )
        .unwrap();
        assert_validation_err(
            listing,
            OperationalListingValidationError::ParseError {
                error:
                    radroots_event::listing::operational::OperationalListingParseError::InvalidTag(
                        "radroots:price".into(),
                    ),
            },
        );
    }

    #[test]
    fn validate_listing_rejects_negative_inventory() {
        let mut listing = base_listing();
        listing.inventory_available = Some("-1".parse().unwrap());
        assert_validation_err(listing, OperationalListingValidationError::InvalidInventory);
    }

    #[test]
    fn validate_listing_rejects_missing_availability() {
        let mut listing = base_listing();
        listing.availability = None;
        assert_validation_err(
            listing,
            OperationalListingValidationError::MissingAvailability,
        );
    }

    #[test]
    fn validate_listing_rejects_missing_location() {
        let mut listing = base_listing();
        listing.location = None;
        assert_validation_err(listing, OperationalListingValidationError::MissingLocation);
    }

    #[test]
    fn validate_listing_rejects_missing_location_locality() {
        let mut listing = base_listing();
        let location = listing.location.as_mut().expect("location");
        location.city = None;
        location.region = None;
        location.country = None;
        assert_validation_err(
            listing,
            OperationalListingValidationError::MissingLocationLocality,
        );
    }

    #[test]
    fn validate_listing_rejects_missing_location_geohash() {
        let mut listing = base_listing();
        listing.location.as_mut().expect("location").geohash = " ".into();
        assert_validation_err(
            listing,
            OperationalListingValidationError::ParseError {
                error:
                    radroots_event::listing::operational::OperationalListingParseError::InvalidTag(
                        "g".to_string(),
                    ),
            },
        );
    }

    #[test]
    fn validate_listing_rejects_invalid_location_geohash() {
        let mut listing = base_listing();
        listing.location.as_mut().expect("location").geohash = "9q8yyz".into();
        assert_validation_err(
            listing,
            OperationalListingValidationError::ParseError {
                error:
                    radroots_event::listing::operational::OperationalListingParseError::InvalidTag(
                        "g".to_string(),
                    ),
            },
        );
    }

    #[test]
    fn validate_listing_rejects_missing_delivery_method() {
        let mut listing = base_listing();
        listing.delivery_method = None;
        assert_validation_err(
            listing,
            OperationalListingValidationError::MissingDeliveryMethod,
        );
    }

    #[test]
    fn validation_error_display_covers_all_variants() {
        let errors = vec![
            OperationalListingValidationError::InvalidKind { kind: 9 },
            OperationalListingValidationError::InvalidProfile,
            OperationalListingValidationError::MissingListingId,
            OperationalListingValidationError::ListingEventNotFound {
                listing_addr: "addr".into(),
            },
            OperationalListingValidationError::ListingEventFetchFailed {
                listing_addr: "addr".into(),
            },
            OperationalListingValidationError::ParseError {
                error:
                    radroots_event::listing::operational::OperationalListingParseError::InvalidTag(
                        "d".into(),
                    ),
            },
            OperationalListingValidationError::InvalidSeller,
            OperationalListingValidationError::MissingFarmProfile,
            OperationalListingValidationError::MissingFarmRecord,
            OperationalListingValidationError::MissingTitle,
            OperationalListingValidationError::MissingDescription,
            OperationalListingValidationError::MissingProductType,
            OperationalListingValidationError::MissingBins,
            OperationalListingValidationError::MissingPrimaryBin,
            OperationalListingValidationError::InvalidBin,
            OperationalListingValidationError::MissingPrice,
            OperationalListingValidationError::InvalidPrice,
            OperationalListingValidationError::MissingInventory,
            OperationalListingValidationError::InvalidInventory,
            OperationalListingValidationError::MissingAvailability,
            OperationalListingValidationError::MissingLocation,
            OperationalListingValidationError::MissingLocationLocality,
            OperationalListingValidationError::MissingLocationGeohash,
            OperationalListingValidationError::InvalidLocationGeohash,
            OperationalListingValidationError::MissingDeliveryMethod,
        ];
        for error in errors {
            assert!(!error.to_string().trim().is_empty());
        }
    }
}

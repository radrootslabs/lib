#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use radroots_core::pricing::{Discount, DiscountScope, DiscountThreshold, DiscountValue};
use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
use radroots_event::envelope::kind::{
    KIND_ARTICLE, KIND_JOB_FEEDBACK, KIND_JOB_REQUEST_MIN, KIND_JOB_RESULT_MIN,
};
use radroots_event::farm::change_set::{GcsLocation, GeoJsonPoint, GeoJsonPolygon};
use radroots_event::farm::coop::Coop;
use radroots_event::farm::plot::{Plot, PlotRef};
use radroots_event::farm::resource_area::{ResourceArea, ResourceAreaLocation, ResourceAreaRef};
use radroots_event::farm::resource_cap::{ResourceHarvestCap, ResourceHarvestProduct};
use radroots_event::farm::{Farm, FarmRef};
use radroots_event::id::{DTag, InventoryBinId};
use radroots_event::listing::operational::{
    OperationalListing, OperationalListingAvailability, OperationalListingBin,
    OperationalListingImage, OperationalListingImageSize, OperationalListingProduct,
    OperationalListingPublicLocation, OperationalListingStatus,
};
use radroots_event::post::document::{Document, DocumentSubject};
use radroots_event::post::reaction::Reaction;
use radroots_event::social::SocialTarget;
use radroots_event::social::app_data::AppData;
use radroots_event::social::follow::{Follow, FollowProfile};
use radroots_event::social::geochat::GeoChat;
use radroots_event::social::gift_wrap::{GiftWrap, GiftWrapRecipient};
use radroots_event::social::job::{JobFeedbackStatus, JobInputType, JobPaymentRequest};
use radroots_event::social::job_feedback::JobFeedback;
use radroots_event::social::job_request::{JobInput, JobParam, JobRequest};
use radroots_event::social::job_result::JobResult;
use radroots_event::social::list::{List, ListEntry};
use radroots_event::social::list_set::ListSet;
use radroots_event::social::message::{Message, MessageRecipient};
use radroots_event::social::message_file::MessageFile;
use radroots_event::social::seal::Seal;
use radroots_event::tag::EventPtr;
use radroots_event_codec::error::EventEncodeError;
use radroots_event_codec::job::encode::JobEncodeError;
use radroots_event_codec::operational_listing::encode::operational_listing_build_tags;
use radroots_event_codec::operational_listing::tags::{
    OperationalListingTagOptions, operational_listing_tags_with_options,
};
use radroots_event_codec::tag_builders::RadrootsEventTagBuilder;
use test_fixtures::{
    CDN_PRIMARY_HTTPS, FIXTURE_ALICE_NPUB, FIXTURE_ALICE_PUBLIC_KEY_HEX, RELAY_PRIMARY_WSS,
};

const TEST_PUBKEY_HEX: &str = FIXTURE_ALICE_PUBLIC_KEY_HEX;
const TEST_NPUB: &str = FIXTURE_ALICE_NPUB;

fn cdn_url(path: &str) -> String {
    format!("{CDN_PRIMARY_HTTPS}/{path}")
}

fn d_tag(raw: &str) -> DTag {
    raw.parse().unwrap()
}

fn bin_id(raw: &str) -> InventoryBinId {
    raw.parse().unwrap()
}

fn sample_social_target(id: &str) -> SocialTarget {
    SocialTarget::Event {
        id: id.to_string(),
        author: Some(TEST_PUBKEY_HEX.to_string()),
        event_kind: Some(KIND_ARTICLE),
        relays: None,
    }
}

fn sample_gcs() -> GcsLocation {
    GcsLocation {
        lat: 37.0,
        lng: -122.0,
        geohash: "9q8yy".to_string(),
        point: GeoJsonPoint {
            r#type: "Point".to_string(),
            coordinates: [-122.0, 37.0],
        },
        polygon: GeoJsonPolygon {
            r#type: "Polygon".to_string(),
            coordinates: vec![vec![
                [-122.0, 37.0],
                [-122.0, 37.0001],
                [-122.0001, 37.0001],
                [-122.0, 37.0],
            ]],
        },
        accuracy: None,
        altitude: None,
        tag_0: None,
        label: None,
        area: None,
        elevation: None,
        soil: None,
        climate: None,
        gc_id: None,
        gc_name: None,
        gc_admin1_id: None,
        gc_admin1_name: None,
        gc_country_id: None,
        gc_country_name: None,
    }
}

fn sample_listing() -> OperationalListing {
    let quantity = Quantity::try_new(Decimal::from(1u32), Unit::Each).unwrap();
    let price = QuantityPrice::try_new(
        Money::try_new(Decimal::from(10u32), Currency::USD).unwrap(),
        quantity.clone(),
    )
    .unwrap();

    OperationalListing {
        d_tag: d_tag("AAAAAAAAAAAAAAAAAAAAAg"),
        published_at: None,
        farm: FarmRef {
            pubkey: TEST_NPUB.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        },
        product: OperationalListingProduct {
            key: "sku".to_string(),
            title: "Widget".to_string(),
            category: "Tools".to_string(),
            summary: None,
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        primary_bin_id: bin_id("bin-1"),
        bins: vec![OperationalListingBin {
            bin_id: bin_id("bin-1"),
            quantity,
            price_per_canonical_unit: price,
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        }],
        resource_area: None,
        plot: None,
        discounts: None,
        inventory_available: None,
        availability: None,
        delivery_method: None,
        location: None,
        images: None,
    }
}

#[test]
fn event_tag_builder_impls_build_tags_for_all_supported_types() {
    let listing = sample_listing();
    assert!(!listing.build_tags().unwrap().is_empty());
    assert!(!operational_listing_build_tags(&listing).unwrap().is_empty());

    let app_data = AppData {
        d_tag: "radroots.app".to_string(),
        content: "payload".to_string(),
    };
    assert!(!app_data.build_tags().unwrap().is_empty());

    let reaction = Reaction {
        target: sample_social_target(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        ),
        content: "+".to_string(),
    };
    assert!(!reaction.build_tags().unwrap().is_empty());

    let message = Message {
        recipients: vec![MessageRecipient {
            public_key: TEST_PUBKEY_HEX.to_string(),
            relay_url: Some(RELAY_PRIMARY_WSS.to_string()),
        }],
        content: "hello".to_string(),
        reply_to: Some(EventPtr {
            id: "reply".to_string(),
            relays: Some(RELAY_PRIMARY_WSS.to_string()),
        }),
        subject: Some("topic".to_string()),
    };
    assert!(!message.build_tags().unwrap().is_empty());

    let message_file = MessageFile {
        recipients: vec![MessageRecipient {
            public_key: TEST_PUBKEY_HEX.to_string(),
            relay_url: None,
        }],
        file_url: cdn_url("blob"),
        reply_to: None,
        subject: None,
        file_type: "image/jpeg".to_string(),
        encryption_algorithm: "aes-gcm".to_string(),
        decryption_key: "key".to_string(),
        decryption_nonce: "nonce".to_string(),
        encrypted_hash: "hash".to_string(),
        original_hash: None,
        size: None,
        dimensions: None,
        blurhash: None,
        thumb: None,
        fallbacks: vec![cdn_url("fallback")],
    };
    assert!(!message_file.build_tags().unwrap().is_empty());

    let geochat = GeoChat {
        geohash: "dr5rsj7".to_string(),
        content: "hello".to_string(),
        nickname: Some("alex".to_string()),
        teleported: true,
    };
    assert!(!geochat.build_tags().unwrap().is_empty());

    let follow = Follow {
        list: vec![FollowProfile {
            published_at: 1,
            public_key: TEST_PUBKEY_HEX.to_string(),
            relay_url: Some(RELAY_PRIMARY_WSS.to_string()),
            contact_name: Some("alex".to_string()),
        }],
    };
    assert!(!follow.build_tags().unwrap().is_empty());

    let farm = Farm {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        name: "Farm".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: None,
    };
    assert!(!farm.build_tags().unwrap().is_empty());

    let resource_area = ResourceArea {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        name: "Area".to_string(),
        about: None,
        location: ResourceAreaLocation {
            primary: None,
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs(),
        },
        tags: None,
    };
    assert!(!resource_area.build_tags().unwrap().is_empty());

    let resource_cap = ResourceHarvestCap {
        d_tag: "AAAAAAAAAAAAAAAAAAAABA".to_string(),
        resource_area: ResourceAreaRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        },
        product: ResourceHarvestProduct {
            key: "nutmeg".to_string(),
            category: Some("spice".to_string()),
        },
        start: 1,
        end: 2,
        cap_quantity: Quantity::try_new(Decimal::from(1000u32), Unit::MassG).unwrap(),
        display_amount: None,
        display_unit: None,
        display_label: None,
        tags: None,
    };
    assert!(!resource_cap.build_tags().unwrap().is_empty());

    let coop = Coop {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
        name: "Coop".to_string(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: None,
    };
    assert!(!coop.build_tags().unwrap().is_empty());

    let document = Document {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
        doc_type: "charter".to_string(),
        title: "Charter".to_string(),
        version: "1.0.0".to_string(),
        summary: None,
        effective_at: None,
        body_markdown: None,
        subject: DocumentSubject {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            address: Some(format!("30340:{TEST_PUBKEY_HEX}:AAAAAAAAAAAAAAAAAAAAAA")),
        },
        tags: None,
    };
    assert!(!document.build_tags().unwrap().is_empty());

    let list = List {
        content: "private".to_string(),
        entries: vec![ListEntry {
            tag: "p".to_string(),
            values: vec![TEST_PUBKEY_HEX.to_string()],
        }],
    };
    assert!(!list.build_tags().unwrap().is_empty());

    let list_set = ListSet {
        d_tag: "members.owners".to_string(),
        content: "private".to_string(),
        entries: vec![ListEntry {
            tag: "p".to_string(),
            values: vec![TEST_PUBKEY_HEX.to_string()],
        }],
        title: Some("owners".to_string()),
        description: Some("team".to_string()),
        image: Some(format!("{CDN_PRIMARY_HTTPS}/team.png")),
    };
    assert!(!list_set.build_tags().unwrap().is_empty());

    let plot = Plot {
        d_tag: "AAAAAAAAAAAAAAAAAAAABQ".to_string(),
        farm: FarmRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        },
        name: "Plot".to_string(),
        about: None,
        location: None,
        tags: None,
    };
    assert!(!plot.build_tags().unwrap().is_empty());

    let job_request = JobRequest {
        kind: u16::try_from(KIND_JOB_REQUEST_MIN + 1).expect("request kind must fit NIP-01"),
        inputs: vec![JobInput {
            data: "hello".to_string(),
            input_type: JobInputType::Text,
            relay: None,
            marker: None,
        }],
        output: None,
        params: vec![JobParam {
            key: "foo".to_string(),
            value: "bar".to_string(),
        }],
        bid_sat: None,
        relays: vec![RELAY_PRIMARY_WSS.to_string()],
        providers: vec![TEST_PUBKEY_HEX.to_string()],
        topics: vec!["topic".to_string()],
        encrypted: false,
    };
    assert!(!job_request.build_tags().unwrap().is_empty());

    let job_result = JobResult {
        kind: u16::try_from(KIND_JOB_RESULT_MIN + 1).expect("result kind must fit NIP-01"),
        request_event: EventPtr {
            id: "req".to_string(),
            relays: Some(RELAY_PRIMARY_WSS.to_string()),
        },
        request_json: None,
        inputs: vec![JobInput {
            data: "hello".to_string(),
            input_type: JobInputType::Text,
            relay: None,
            marker: None,
        }],
        customer_pubkey: Some(TEST_PUBKEY_HEX.to_string()),
        payment: Some(JobPaymentRequest {
            amount_sat: 1,
            bolt11: None,
        }),
        content: Some("payload".to_string()),
        encrypted: false,
    };
    assert!(!job_result.build_tags().unwrap().is_empty());

    let job_feedback = JobFeedback {
        kind: u16::try_from(KIND_JOB_FEEDBACK).expect("feedback kind must fit NIP-01"),
        status: JobFeedbackStatus::Processing,
        extra_info: Some("queued".to_string()),
        request_event: EventPtr {
            id: "req".to_string(),
            relays: Some(RELAY_PRIMARY_WSS.to_string()),
        },
        customer_pubkey: Some(TEST_PUBKEY_HEX.to_string()),
        payment: Some(JobPaymentRequest {
            amount_sat: 1,
            bolt11: None,
        }),
        content: Some("payload".to_string()),
        encrypted: false,
    };
    assert!(!job_feedback.build_tags().unwrap().is_empty());

    let seal = Seal {
        content: "sealed".to_string(),
    };
    assert!(seal.build_tags().unwrap().is_empty());

    let gift_wrap = GiftWrap {
        recipient: GiftWrapRecipient {
            public_key: TEST_PUBKEY_HEX.to_string(),
            relay_url: Some(RELAY_PRIMARY_WSS.to_string()),
        },
        content: "encrypted".to_string(),
        expiration: Some(1700000000),
    };
    assert!(!gift_wrap.build_tags().unwrap().is_empty());
}

#[test]
fn listing_and_message_builders_cover_optional_shapes() {
    let mut listing = sample_listing();
    listing.resource_area = Some(ResourceAreaRef {
        pubkey: TEST_PUBKEY_HEX.to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
    });
    listing.plot = Some(PlotRef {
        pubkey: TEST_PUBKEY_HEX.to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
    });
    listing.product.summary = Some("summary".to_string());
    listing.product.process = Some("washed".to_string());
    listing.product.lot = Some("lot-1".to_string());
    listing.product.location = Some("Moyobamba".to_string());
    listing.product.profile = Some("fruity".to_string());
    listing.product.year = Some("2024".to_string());
    listing.location = Some(OperationalListingPublicLocation {
        primary: "Moyobamba".to_string(),
        city: Some("Moyobamba".to_string()),
        region: Some("San Martin".to_string()),
        country: Some("PE".to_string()),
        geohash: "9q8yy".to_string(),
    });
    listing.images = Some(vec![OperationalListingImage {
        url: cdn_url("a.jpg"),
        size: Some(OperationalListingImageSize { w: 1200, h: 800 }),
    }]);
    assert!(!operational_listing_build_tags(&listing).unwrap().is_empty());

    let mut listing_with_trade = listing.clone();
    listing_with_trade.inventory_available = Some(Decimal::from(12u32));
    let with_trade_fields: fn() -> OperationalListingTagOptions =
        OperationalListingTagOptions::with_trade_fields;
    let trade_options = with_trade_fields();
    listing_with_trade.availability = Some(OperationalListingAvailability::Status {
        status: OperationalListingStatus::Active,
    });
    let operational_listing_tags_full_fn: fn(
        &OperationalListing,
    ) -> Result<Vec<Vec<String>>, EventEncodeError> =
        radroots_event_codec::operational_listing::tags::operational_listing_tags_full;
    let full_tags = operational_listing_tags_full_fn(&listing_with_trade).unwrap();
    assert!(full_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("inventory")
            && tag.get(1).map(|v| v.as_str()) == Some("12")
    }));

    let trade_tags =
        operational_listing_tags_with_options(&listing_with_trade, trade_options).unwrap();
    assert!(trade_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("inventory")
            && tag.get(1).map(|v| v.as_str()) == Some("12")
    }));
    assert!(trade_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("status")
            && tag.get(1).map(|v| v.as_str()) == Some("active")
    }));

    let mut listing_status_sold = listing_with_trade.clone();
    listing_status_sold.availability = Some(OperationalListingAvailability::Status {
        status: OperationalListingStatus::Sold,
    });
    let sold_tags =
        operational_listing_tags_with_options(&listing_status_sold, trade_options).unwrap();
    assert!(sold_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("status")
            && tag.get(1).map(|v| v.as_str()) == Some("sold")
    }));

    let mut listing_status_other = listing_with_trade.clone();
    listing_status_other.availability = Some(OperationalListingAvailability::Status {
        status: OperationalListingStatus::Other {
            value: "paused".to_string(),
        },
    });
    let other_tags =
        operational_listing_tags_with_options(&listing_status_other, trade_options).unwrap();
    assert!(other_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("status")
            && tag.get(1).map(|v| v.as_str()) == Some("paused")
    }));

    let mut listing_geohash_only = listing_with_trade.clone();
    listing_geohash_only.location = Some(OperationalListingPublicLocation {
        primary: "Moyobamba".to_string(),
        city: Some("Moyobamba".to_string()),
        region: None,
        country: None,
        geohash: "6gkzw".to_string(),
    });
    let geohash_tags = operational_listing_tags_with_options(
        &listing_geohash_only,
        OperationalListingTagOptions::default(),
    )
    .unwrap();
    assert!(geohash_tags.iter().any(|tag| {
        tag.first().map(|v| v.as_str()) == Some("g")
            && tag.get(1).map(|v| v.as_str()) == Some("6gkzw")
    }));

    let mut listing_no_coordinates = listing_with_trade.clone();
    listing_no_coordinates.location = Some(OperationalListingPublicLocation {
        primary: "Moyobamba".to_string(),
        city: Some("Moyobamba".to_string()),
        region: None,
        country: None,
        geohash: "9q8yy".to_string(),
    });
    let no_coordinates_tags = operational_listing_tags_with_options(
        &listing_no_coordinates,
        OperationalListingTagOptions::default(),
    )
    .unwrap();
    assert!(
        !no_coordinates_tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("L"))
    );

    let without_private_location_tags = operational_listing_tags_with_options(
        &listing_with_trade,
        OperationalListingTagOptions {
            ..OperationalListingTagOptions::default()
        },
    )
    .unwrap();
    assert!(
        !without_private_location_tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("L"))
    );

    let mut listing_with_empty_primary_location = listing_with_trade.clone();
    listing_with_empty_primary_location.location = Some(OperationalListingPublicLocation {
        primary: " null ".to_string(),
        city: Some("Moyobamba".to_string()),
        region: None,
        country: None,
        geohash: "9q8yy".to_string(),
    });
    assert!(matches!(
        operational_listing_tags_with_options(&listing_with_empty_primary_location, trade_options),
        Err(EventEncodeError::EmptyRequiredField("location.primary"))
    ));

    let mut listing_with_discount_payload = listing_with_trade.clone();
    listing_with_discount_payload.discounts = Some(vec![
        Discount::try_new(
            DiscountScope::Bin,
            DiscountThreshold::BinCount {
                bin_id: "bin-1".to_string(),
                min: 2,
            },
            DiscountValue::MoneyPerBin(Money::try_new(Decimal::from(1u32), Currency::USD).unwrap()),
        )
        .unwrap(),
    ]);
    #[cfg(feature = "serde_json")]
    {
        let tags =
            operational_listing_tags_with_options(&listing_with_discount_payload, trade_options)
                .expect("discount serialization works");
        assert!(
            tags.iter()
                .any(|tag| tag.first().map(|v| v.as_str()) == Some("radroots:discount"))
        );
    }
    #[cfg(not(feature = "serde_json"))]
    {
        let err =
            operational_listing_tags_with_options(&listing_with_discount_payload, trade_options)
                .expect_err("discounts require serde_json in non-serde lane");
        assert!(matches!(err, EventEncodeError::Json));
    }

    let message_without_relays = Message {
        recipients: vec![MessageRecipient {
            public_key: TEST_PUBKEY_HEX.to_string(),
            relay_url: None,
        }],
        content: "hello".to_string(),
        reply_to: Some(EventPtr {
            id: "reply".to_string(),
            relays: None,
        }),
        subject: None,
    };
    assert!(!message_without_relays.build_tags().unwrap().is_empty());

    let message_invalid_reply = Message {
        recipients: vec![MessageRecipient {
            public_key: TEST_PUBKEY_HEX.to_string(),
            relay_url: None,
        }],
        content: "hello".to_string(),
        reply_to: Some(EventPtr {
            id: " ".to_string(),
            relays: None,
        }),
        subject: None,
    };
    let err = message_invalid_reply
        .build_tags()
        .expect_err("empty reply id should fail");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("reply_to.id")
    ));
}

#[test]
fn listing_builder_rejects_required_field_errors() {
    let mut listing = sample_listing();
    listing.d_tag = d_tag("listing:invalid");
    let err = operational_listing_build_tags(&listing).expect_err("invalid listing d_tag");
    assert!(matches!(err, EventEncodeError::InvalidField("d")));

    let mut listing = sample_listing();
    listing.bins.clear();
    let err = operational_listing_build_tags(&listing).expect_err("empty bins");
    assert!(matches!(err, EventEncodeError::EmptyRequiredField("bins")));

    let mut listing = sample_listing();
    listing.farm.pubkey = " ".to_string();
    let err = operational_listing_build_tags(&listing).expect_err("empty farm pubkey");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.pubkey")
    ));

    let mut listing = sample_listing();
    listing.farm.d_tag = " ".to_string();
    let err = operational_listing_build_tags(&listing).expect_err("empty farm d_tag");
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("farm.d_tag")
    ));
}

#[test]
fn job_request_tag_builder_rejects_encrypted_without_provider() {
    let request = JobRequest {
        kind: u16::try_from(KIND_JOB_REQUEST_MIN + 1).expect("request kind must fit NIP-01"),
        inputs: vec![JobInput {
            data: "hello".to_string(),
            input_type: JobInputType::Text,
            relay: None,
            marker: None,
        }],
        output: None,
        params: Vec::new(),
        bid_sat: None,
        relays: Vec::new(),
        providers: Vec::new(),
        topics: Vec::new(),
        encrypted: true,
    };
    let err = request.build_tags().unwrap_err();
    assert!(matches!(err, JobEncodeError::MissingProvidersForEncrypted));
}

#[test]
fn job_request_tag_builder_accepts_encrypted_with_provider() {
    let request = JobRequest {
        kind: u16::try_from(KIND_JOB_REQUEST_MIN + 1).expect("request kind must fit NIP-01"),
        inputs: vec![JobInput {
            data: "hello".to_string(),
            input_type: JobInputType::Text,
            relay: None,
            marker: None,
        }],
        output: None,
        params: Vec::new(),
        bid_sat: None,
        relays: Vec::new(),
        providers: vec![TEST_PUBKEY_HEX.to_string()],
        topics: Vec::new(),
        encrypted: true,
    };
    let tags = request.build_tags().unwrap();
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("encrypted"))
    );
}

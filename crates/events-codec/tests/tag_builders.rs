use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::app_data::RadrootsAppData;
use radroots_events::comment::RadrootsComment;
use radroots_events::coop::RadrootsCoop;
use radroots_events::document::{RadrootsDocument, RadrootsDocumentSubject};
use radroots_events::farm::{
    RadrootsFarm, RadrootsFarmRef, RadrootsGcsLocation, RadrootsGeoJsonPoint,
    RadrootsGeoJsonPolygon,
};
use radroots_events::follow::{RadrootsFollow, RadrootsFollowProfile};
use radroots_events::geochat::RadrootsGeoChat;
use radroots_events::gift_wrap::{RadrootsGiftWrap, RadrootsGiftWrapRecipient};
use radroots_events::job::{JobFeedbackStatus, JobInputType, JobPaymentRequest};
use radroots_events::job_feedback::RadrootsJobFeedback;
use radroots_events::job_request::{RadrootsJobInput, RadrootsJobParam, RadrootsJobRequest};
use radroots_events::job_result::RadrootsJobResult;
use radroots_events::kinds::{
    KIND_JOB_FEEDBACK, KIND_JOB_REQUEST_MIN, KIND_JOB_RESULT_MIN, KIND_POST,
};
use radroots_events::list::{RadrootsList, RadrootsListEntry};
use radroots_events::list_set::RadrootsListSet;
use radroots_events::listing::{
    RadrootsListing, RadrootsListingBin, RadrootsListingFarmRef, RadrootsListingProduct,
};
use radroots_events::message::{RadrootsMessage, RadrootsMessageRecipient};
use radroots_events::message_file::RadrootsMessageFile;
use radroots_events::plot::RadrootsPlot;
use radroots_events::post::RadrootsPost;
use radroots_events::profile::RadrootsProfile;
use radroots_events::reaction::RadrootsReaction;
use radroots_events::resource_area::{
    RadrootsResourceArea, RadrootsResourceAreaLocation, RadrootsResourceAreaRef,
};
use radroots_events::resource_cap::{RadrootsResourceHarvestCap, RadrootsResourceHarvestProduct};
use radroots_events::seal::RadrootsSeal;
use radroots_events::RadrootsNostrEventPtr;
use radroots_events::RadrootsNostrEventRef;
use radroots_events_codec::job::encode::JobEncodeError;
use radroots_events_codec::listing::encode::listing_build_tags;
use radroots_events_codec::tag_builders::RadrootsEventTagBuilder;

const TEST_PUBKEY_HEX: &str = "58e318557257f2ab58a415d21bb57082b4824cf667a1d64e72bcbc5acc018c62";
const TEST_NPUB: &str = "npub1tr33s4tj2le2kk9yzhfphdtss26gyn8kv7savnnjhj794nqp333q8e7grr";

fn sample_event_ref(id: &str) -> RadrootsNostrEventRef {
    RadrootsNostrEventRef {
        id: id.to_string(),
        author: TEST_PUBKEY_HEX.to_string(),
        kind: KIND_POST,
        d_tag: None,
        relays: None,
    }
}

fn sample_gcs() -> RadrootsGcsLocation {
    RadrootsGcsLocation {
        lat: 37.0,
        lng: -122.0,
        geohash: "9q8yy".to_string(),
        point: RadrootsGeoJsonPoint {
            r#type: "Point".to_string(),
            coordinates: [-122.0, 37.0],
        },
        polygon: RadrootsGeoJsonPolygon {
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

fn sample_listing() -> RadrootsListing {
    let quantity =
        RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each);
    let price = RadrootsCoreQuantityPrice::new(
        RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
        quantity.clone(),
    );

    RadrootsListing {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
        farm: RadrootsListingFarmRef {
            pubkey: TEST_NPUB.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        },
        product: RadrootsListingProduct {
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
        primary_bin_id: "bin-1".to_string(),
        bins: vec![RadrootsListingBin {
            bin_id: "bin-1".to_string(),
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
    assert!(!listing_build_tags(&listing).unwrap().is_empty());

    let app_data = RadrootsAppData {
        d_tag: "radroots.app".to_string(),
        content: "payload".to_string(),
    };
    assert!(!app_data.build_tags().unwrap().is_empty());

    let comment = RadrootsComment {
        root: sample_event_ref("root"),
        parent: sample_event_ref("parent"),
        content: "hello".to_string(),
    };
    assert!(!comment.build_tags().unwrap().is_empty());

    let reaction = RadrootsReaction {
        root: sample_event_ref("root"),
        content: "+".to_string(),
    };
    assert!(!reaction.build_tags().unwrap().is_empty());

    let message = RadrootsMessage {
        recipients: vec![RadrootsMessageRecipient {
            public_key: TEST_PUBKEY_HEX.to_string(),
            relay_url: Some("wss://relay.example.com".to_string()),
        }],
        content: "hello".to_string(),
        reply_to: Some(RadrootsNostrEventPtr {
            id: "reply".to_string(),
            relays: Some("wss://relay.example.com".to_string()),
        }),
        subject: Some("topic".to_string()),
    };
    assert!(!message.build_tags().unwrap().is_empty());

    let message_file = RadrootsMessageFile {
        recipients: vec![RadrootsMessageRecipient {
            public_key: TEST_PUBKEY_HEX.to_string(),
            relay_url: None,
        }],
        file_url: "https://files.example.com/blob".to_string(),
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
        fallbacks: vec!["https://files.example.com/fallback".to_string()],
    };
    assert!(!message_file.build_tags().unwrap().is_empty());

    let geochat = RadrootsGeoChat {
        geohash: "dr5rsj7".to_string(),
        content: "hello".to_string(),
        nickname: Some("alex".to_string()),
        teleported: true,
    };
    assert!(!geochat.build_tags().unwrap().is_empty());

    let follow = RadrootsFollow {
        list: vec![RadrootsFollowProfile {
            published_at: 1,
            public_key: TEST_PUBKEY_HEX.to_string(),
            relay_url: Some("wss://relay.example.com".to_string()),
            contact_name: Some("alex".to_string()),
        }],
    };
    assert!(!follow.build_tags().unwrap().is_empty());

    let farm = RadrootsFarm {
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

    let resource_area = RadrootsResourceArea {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        name: "Area".to_string(),
        about: None,
        location: RadrootsResourceAreaLocation {
            primary: None,
            city: None,
            region: None,
            country: None,
            gcs: sample_gcs(),
        },
        tags: None,
    };
    assert!(!resource_area.build_tags().unwrap().is_empty());

    let resource_cap = RadrootsResourceHarvestCap {
        d_tag: "AAAAAAAAAAAAAAAAAAAABA".to_string(),
        resource_area: RadrootsResourceAreaRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
        },
        product: RadrootsResourceHarvestProduct {
            key: "nutmeg".to_string(),
            category: Some("spice".to_string()),
        },
        start: 1,
        end: 2,
        cap_quantity: RadrootsCoreQuantity::new(
            RadrootsCoreDecimal::from(1000u32),
            RadrootsCoreUnit::MassG,
        ),
        display_amount: None,
        display_unit: None,
        display_label: None,
        tags: None,
    };
    assert!(!resource_cap.build_tags().unwrap().is_empty());

    let coop = RadrootsCoop {
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

    let document = RadrootsDocument {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
        doc_type: "charter".to_string(),
        title: "Charter".to_string(),
        version: "1.0.0".to_string(),
        summary: None,
        effective_at: None,
        body_markdown: None,
        subject: RadrootsDocumentSubject {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            address: Some("30340:58e318557257f2ab58a415d21bb57082b4824cf667a1d64e72bcbc5acc018c62:AAAAAAAAAAAAAAAAAAAAAA".to_string()),
        },
        tags: None,
    };
    assert!(!document.build_tags().unwrap().is_empty());

    let list = RadrootsList {
        content: "private".to_string(),
        entries: vec![RadrootsListEntry {
            tag: "p".to_string(),
            values: vec![TEST_PUBKEY_HEX.to_string()],
        }],
    };
    assert!(!list.build_tags().unwrap().is_empty());

    let list_set = RadrootsListSet {
        d_tag: "members.owners".to_string(),
        content: "private".to_string(),
        entries: vec![RadrootsListEntry {
            tag: "p".to_string(),
            values: vec![TEST_PUBKEY_HEX.to_string()],
        }],
        title: Some("owners".to_string()),
        description: Some("team".to_string()),
        image: Some("https://example.com/team.png".to_string()),
    };
    assert!(!list_set.build_tags().unwrap().is_empty());

    let plot = RadrootsPlot {
        d_tag: "AAAAAAAAAAAAAAAAAAAABQ".to_string(),
        farm: RadrootsFarmRef {
            pubkey: TEST_PUBKEY_HEX.to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        },
        name: "Plot".to_string(),
        about: None,
        location: None,
        tags: None,
    };
    assert!(!plot.build_tags().unwrap().is_empty());

    let job_request = RadrootsJobRequest {
        kind: (KIND_JOB_REQUEST_MIN + 1) as u16,
        inputs: vec![RadrootsJobInput {
            data: "hello".to_string(),
            input_type: JobInputType::Text,
            relay: None,
            marker: None,
        }],
        output: None,
        params: vec![RadrootsJobParam {
            key: "foo".to_string(),
            value: "bar".to_string(),
        }],
        bid_sat: None,
        relays: vec!["wss://relay.example.com".to_string()],
        providers: vec![TEST_PUBKEY_HEX.to_string()],
        topics: vec!["topic".to_string()],
        encrypted: false,
    };
    assert!(!job_request.build_tags().unwrap().is_empty());

    let job_result = RadrootsJobResult {
        kind: (KIND_JOB_RESULT_MIN + 1) as u16,
        request_event: RadrootsNostrEventPtr {
            id: "req".to_string(),
            relays: Some("wss://relay.example.com".to_string()),
        },
        request_json: None,
        inputs: vec![RadrootsJobInput {
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

    let job_feedback = RadrootsJobFeedback {
        kind: KIND_JOB_FEEDBACK as u16,
        status: JobFeedbackStatus::Processing,
        extra_info: Some("queued".to_string()),
        request_event: RadrootsNostrEventPtr {
            id: "req".to_string(),
            relays: Some("wss://relay.example.com".to_string()),
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

    let seal = RadrootsSeal {
        content: "sealed".to_string(),
    };
    assert!(seal.build_tags().unwrap().is_empty());

    let gift_wrap = RadrootsGiftWrap {
        recipient: RadrootsGiftWrapRecipient {
            public_key: TEST_PUBKEY_HEX.to_string(),
            relay_url: Some("wss://relay.example.com".to_string()),
        },
        content: "encrypted".to_string(),
        expiration: Some(1700000000),
    };
    assert!(!gift_wrap.build_tags().unwrap().is_empty());

    let profile = RadrootsProfile {
        name: "alice".to_string(),
        display_name: None,
        nip05: None,
        about: None,
        website: None,
        picture: None,
        banner: None,
        lud06: None,
        lud16: None,
        bot: None,
    };
    assert!(profile.build_tags().unwrap().is_empty());

    let post = RadrootsPost {
        content: "hello".to_string(),
    };
    assert!(post.build_tags().unwrap().is_empty());
}

#[test]
fn job_request_tag_builder_rejects_encrypted_without_provider() {
    let request = RadrootsJobRequest {
        kind: (KIND_JOB_REQUEST_MIN + 1) as u16,
        inputs: vec![RadrootsJobInput {
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

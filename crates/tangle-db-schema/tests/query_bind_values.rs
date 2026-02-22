use radroots_tangle_db_schema::farm::FarmQueryBindValues;
use radroots_tangle_db_schema::farm_gcs_location::FarmGcsLocationQueryBindValues;
use radroots_tangle_db_schema::farm_member::FarmMemberQueryBindValues;
use radroots_tangle_db_schema::farm_member_claim::FarmMemberClaimQueryBindValues;
use radroots_tangle_db_schema::farm_tag::FarmTagQueryBindValues;
use radroots_tangle_db_schema::gcs_location::GcsLocationQueryBindValues;
use radroots_tangle_db_schema::log_error::LogErrorQueryBindValues;
use radroots_tangle_db_schema::media_image::MediaImageQueryBindValues;
use radroots_tangle_db_schema::nostr_event_state::NostrEventStateQueryBindValues;
use radroots_tangle_db_schema::nostr_profile::NostrProfileQueryBindValues;
use radroots_tangle_db_schema::nostr_relay::NostrRelayQueryBindValues;
use radroots_tangle_db_schema::plot::PlotQueryBindValues;
use radroots_tangle_db_schema::plot_gcs_location::PlotGcsLocationQueryBindValues;
use radroots_tangle_db_schema::plot_tag::PlotTagQueryBindValues;
use radroots_tangle_db_schema::trade_product::TradeProductQueryBindValues;
use serde_json::Value;

macro_rules! assert_query_bind_values {
    ($test_name:ident, $id_expr:expr, $id_param:literal, $id_lookup:literal, [$(($expr:expr, $param:literal, $lookup:literal)),* $(,)?]) => {
        #[test]
        fn $test_name() {
            let id_case = $id_expr;
            let (id_param, id_value) = id_case.to_filter_param();
            assert_eq!(id_param, $id_param);
            assert_eq!(id_value, Value::from($id_lookup.to_string()));
            assert_eq!(id_case.primary_key(), Some($id_lookup.to_string()));
            assert_eq!(id_case.lookup_key(), $id_lookup.to_string());

            $(
                let alt_case = $expr;
                let (alt_param, alt_value) = alt_case.to_filter_param();
                assert_eq!(alt_param, $param);
                assert_eq!(alt_value, Value::from($lookup.to_string()));
                assert_eq!(alt_case.primary_key(), None);
                assert_eq!(alt_case.lookup_key(), $lookup.to_string());
            )*
        }
    };
}

assert_query_bind_values!(
    farm_query_bind_values_cover_all_variants,
    FarmQueryBindValues::Id {
        id: "farm-id".to_string()
    },
    "id",
    "farm-id",
    [
        (
            FarmQueryBindValues::DTag {
                d_tag: "farm-d".to_string()
            },
            "d_tag",
            "farm-d"
        ),
        (
            FarmQueryBindValues::Pubkey {
                pubkey: "farm-pk".to_string()
            },
            "pubkey",
            "farm-pk"
        ),
    ]
);

assert_query_bind_values!(
    farm_gcs_location_query_bind_values_cover_all_variants,
    FarmGcsLocationQueryBindValues::Id {
        id: "farm-gcs-id".to_string()
    },
    "id",
    "farm-gcs-id",
    [
        (
            FarmGcsLocationQueryBindValues::FarmId {
                farm_id: "farm-id".to_string()
            },
            "farm_id",
            "farm-id"
        ),
        (
            FarmGcsLocationQueryBindValues::GcsLocationId {
                gcs_location_id: "gcs-id".to_string()
            },
            "gcs_location_id",
            "gcs-id"
        ),
    ]
);

assert_query_bind_values!(
    farm_member_query_bind_values_cover_all_variants,
    FarmMemberQueryBindValues::Id {
        id: "farm-member-id".to_string()
    },
    "id",
    "farm-member-id",
    [
        (
            FarmMemberQueryBindValues::FarmId {
                farm_id: "farm-id".to_string()
            },
            "farm_id",
            "farm-id"
        ),
        (
            FarmMemberQueryBindValues::MemberPubkey {
                member_pubkey: "member-pk".to_string()
            },
            "member_pubkey",
            "member-pk"
        ),
    ]
);

assert_query_bind_values!(
    farm_member_claim_query_bind_values_cover_all_variants,
    FarmMemberClaimQueryBindValues::Id {
        id: "farm-member-claim-id".to_string()
    },
    "id",
    "farm-member-claim-id",
    [
        (
            FarmMemberClaimQueryBindValues::MemberPubkey {
                member_pubkey: "member-pk".to_string()
            },
            "member_pubkey",
            "member-pk"
        ),
        (
            FarmMemberClaimQueryBindValues::FarmPubkey {
                farm_pubkey: "farm-pk".to_string()
            },
            "farm_pubkey",
            "farm-pk"
        ),
    ]
);

assert_query_bind_values!(
    farm_tag_query_bind_values_cover_all_variants,
    FarmTagQueryBindValues::Id {
        id: "farm-tag-id".to_string()
    },
    "id",
    "farm-tag-id",
    [
        (
            FarmTagQueryBindValues::FarmId {
                farm_id: "farm-id".to_string()
            },
            "farm_id",
            "farm-id"
        ),
        (
            FarmTagQueryBindValues::Tag {
                tag: "organic".to_string()
            },
            "tag",
            "organic"
        ),
    ]
);

assert_query_bind_values!(
    gcs_location_query_bind_values_cover_all_variants,
    GcsLocationQueryBindValues::Id {
        id: "gcs-location-id".to_string()
    },
    "id",
    "gcs-location-id",
    [
        (
            GcsLocationQueryBindValues::DTag {
                d_tag: "gcs-d".to_string()
            },
            "d_tag",
            "gcs-d"
        ),
        (
            GcsLocationQueryBindValues::Geohash {
                geohash: "9q8yy".to_string()
            },
            "geohash",
            "9q8yy"
        ),
    ]
);

assert_query_bind_values!(
    log_error_query_bind_values_cover_all_variants,
    LogErrorQueryBindValues::Id {
        id: "log-error-id".to_string()
    },
    "id",
    "log-error-id",
    [(
        LogErrorQueryBindValues::NostrPubkey {
            nostr_pubkey: "nostr-pk".to_string()
        },
        "nostr_pubkey",
        "nostr-pk"
    ),]
);

assert_query_bind_values!(
    media_image_query_bind_values_cover_all_variants,
    MediaImageQueryBindValues::Id {
        id: "media-image-id".to_string()
    },
    "id",
    "media-image-id",
    [(
        MediaImageQueryBindValues::FilePath {
            file_path: "/tmp/a.jpg".to_string()
        },
        "file_path",
        "/tmp/a.jpg"
    ),]
);

assert_query_bind_values!(
    nostr_event_state_query_bind_values_cover_all_variants,
    NostrEventStateQueryBindValues::Id {
        id: "nostr-event-state-id".to_string()
    },
    "id",
    "nostr-event-state-id",
    [(
        NostrEventStateQueryBindValues::Key {
            key: "event-key".to_string()
        },
        "key",
        "event-key"
    ),]
);

assert_query_bind_values!(
    nostr_profile_query_bind_values_cover_all_variants,
    NostrProfileQueryBindValues::Id {
        id: "nostr-profile-id".to_string()
    },
    "id",
    "nostr-profile-id",
    [(
        NostrProfileQueryBindValues::PublicKey {
            public_key: "nostr-public-key".to_string()
        },
        "public_key",
        "nostr-public-key"
    ),]
);

assert_query_bind_values!(
    nostr_relay_query_bind_values_cover_all_variants,
    NostrRelayQueryBindValues::Id {
        id: "nostr-relay-id".to_string()
    },
    "id",
    "nostr-relay-id",
    [(
        NostrRelayQueryBindValues::Url {
            url: "wss://relay.example.com".to_string()
        },
        "url",
        "wss://relay.example.com"
    ),]
);

assert_query_bind_values!(
    plot_query_bind_values_cover_all_variants,
    PlotQueryBindValues::Id {
        id: "plot-id".to_string()
    },
    "id",
    "plot-id",
    [
        (
            PlotQueryBindValues::DTag {
                d_tag: "plot-d".to_string()
            },
            "d_tag",
            "plot-d"
        ),
        (
            PlotQueryBindValues::FarmId {
                farm_id: "farm-id".to_string()
            },
            "farm_id",
            "farm-id"
        ),
    ]
);

assert_query_bind_values!(
    plot_gcs_location_query_bind_values_cover_all_variants,
    PlotGcsLocationQueryBindValues::Id {
        id: "plot-gcs-id".to_string()
    },
    "id",
    "plot-gcs-id",
    [
        (
            PlotGcsLocationQueryBindValues::PlotId {
                plot_id: "plot-id".to_string()
            },
            "plot_id",
            "plot-id"
        ),
        (
            PlotGcsLocationQueryBindValues::GcsLocationId {
                gcs_location_id: "gcs-id".to_string()
            },
            "gcs_location_id",
            "gcs-id"
        ),
    ]
);

assert_query_bind_values!(
    plot_tag_query_bind_values_cover_all_variants,
    PlotTagQueryBindValues::Id {
        id: "plot-tag-id".to_string()
    },
    "id",
    "plot-tag-id",
    [
        (
            PlotTagQueryBindValues::PlotId {
                plot_id: "plot-id".to_string()
            },
            "plot_id",
            "plot-id"
        ),
        (
            PlotTagQueryBindValues::Tag {
                tag: "steep".to_string()
            },
            "tag",
            "steep"
        ),
    ]
);

assert_query_bind_values!(
    trade_product_query_bind_values_cover_all_variants,
    TradeProductQueryBindValues::Id {
        id: "trade-product-id".to_string()
    },
    "id",
    "trade-product-id",
    []
);

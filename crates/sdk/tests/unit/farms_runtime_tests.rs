use super::*;
use crate::{RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider};
use radroots_nostr::prelude::RadrootsNostrKeys;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[path = "../support/fixture_signer.rs"]
mod fixture_signer;
#[path = "../support/serializer_failure.rs"]
mod serializer_failure;

use fixture_signer::FixtureSigner;
use serializer_failure::assert_struct_serialize_error_paths;

const FARMER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FARM_A_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const FARM_B_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const FARM_C_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const RELAY_A: &str = "wss://relay-a.radroots.test";
const RELAY_B: &str = "wss://relay-b.radroots.test";

fn farmer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(FARMER, [RadrootsActorRole::Farmer]).expect("actor")
}

fn farm(d_tag: &str, name: &str) -> RadrootsFarm {
    RadrootsFarm {
        d_tag: d_tag.to_owned(),
        name: name.to_owned(),
        about: Some("Vegetable farm".to_owned()),
        website: Some("https://example.invalid/farm".to_owned()),
        picture: None,
        banner: None,
        location: None,
        tags: Some(vec!["vegetables".to_owned(), "local".to_owned()]),
    }
}

async fn fixture_geocoder(tempdir: &tempfile::TempDir, feature_name: Option<&str>) -> Geocoder {
    let path = tempdir.path().join(match feature_name {
        Some(name) if name.trim().is_empty() => "geonames-blank.db",
        Some(_) => "geonames-fixture.db",
        None => "geonames-empty.db",
    });
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("geonames fixture pool");
    sqlx::raw_sql(
        r#"
        CREATE TABLE countries(
          id TEXT,
          name TEXT,
          PRIMARY KEY (id)
        );
        CREATE TABLE admin1(
          country_id TEXT,
          id INTEGER,
          name TEXT,
          PRIMARY KEY (country_id, id)
        );
        CREATE TABLE features(
          id INTEGER,
          name TEXT,
          country_id TEXT,
          admin1_id INTEGER,
          PRIMARY KEY (id)
        );
        CREATE TABLE coordinates(
          feature_id INTEGER,
          latitude REAL,
          longitude REAL,
          PRIMARY KEY (feature_id)
        );
        CREATE INDEX coordinates_lat_lng ON coordinates (latitude, longitude);
        CREATE VIEW geonames AS
          SELECT
            features.id,
            features.name,
            admin1.id AS admin1_id,
            admin1.name AS admin1_name,
            countries.id AS country_id,
            countries.name AS country_name,
            coordinates.latitude AS latitude,
            coordinates.longitude AS longitude
          FROM features
            LEFT JOIN countries ON features.country_id = countries.id
            LEFT JOIN admin1 ON features.country_id = admin1.country_id AND features.admin1_id = admin1.id
            JOIN coordinates ON features.id = coordinates.feature_id;
        "#,
    )
    .execute(&pool)
    .await
    .expect("schema geonames fixture");
    if let Some(name) = feature_name {
        sqlx::raw_sql(
            r#"
            INSERT INTO countries (id, name) VALUES ('FX', 'Fixture Country');
            INSERT INTO admin1 (country_id, id, name) VALUES ('FX', 1, 'Fixture Region');
            INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (1, 12.25, -34.50);
            "#,
        )
        .execute(&pool)
        .await
        .expect("seed geonames fixture");
        sqlx::query(
            "INSERT INTO features (id, name, country_id, admin1_id) VALUES (1, ?1, 'FX', 1)",
        )
        .bind(name)
        .execute(&pool)
        .await
        .expect("seed geonames feature");
    }
    pool.close().await;
    Geocoder::open_path(path).expect("open geonames fixture")
}

#[test]
fn farm_publish_plan_rejects_invalid_draft_tags() {
    let actor = RadrootsActorContext::test(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        [RadrootsActorRole::Farmer],
    )
    .expect("actor");
    let farm = RadrootsFarm {
        d_tag: "AAAAAAAAAAAAAAAAAAAAA!".to_owned(),
        name: "Invalid Farm".to_owned(),
        about: None,
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: None,
    };
    let error = farm_publish_plan(
        &actor,
        farm,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
    )
    .err()
    .expect("invalid farm plan");
    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { message } if message.contains("draft encode failed")
    ));

    assert!(matches!(
        farm_addr(&actor, ""),
        Err(RadrootsSdkError::InvalidRequest { message }) if message.contains("farm address")
    ));
}

#[test]
fn farm_runtime_request_builders_and_serializers_cover_success_paths() {
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_321);
    let prepare =
        FarmPreparePublishRequest::new(farmer_actor(), farm(FARM_A_D_TAG, "Serialized Farm"))
            .with_created_at(created_at);
    assert_struct_serialize_error_paths(&prepare, 3);
    let prepare_json = serde_json::to_value(&prepare).expect("prepare json");
    assert_eq!(prepare_json["actor"]["pubkey"], FARMER);
    assert_eq!(prepare_json["created_at"], 1_700_000_321);

    let enqueue = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_B_D_TAG, "Queued Farm"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY_A, RELAY_B], SdkRelayUrlPolicy::Public)
    .expect("relay targets")
    .with_idempotency_key(SdkIdempotencyKey::new("farm-unit-key").expect("key"))
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&enqueue, 5);
    let enqueue_json = serde_json::to_value(&enqueue).expect("enqueue json");
    assert_eq!(enqueue_json["target_relays"]["kind"], "explicit");
    assert_eq!(enqueue_json["created_at"], 1_700_000_321);
    assert!(!enqueue_json.to_string().contains("farm-unit-key"));

    let try_key = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_C_D_TAG, "Try Key Farm"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_idempotency_key("farm-unit-try-key")
    .expect("try key");
    assert_eq!(
        serde_json::to_value(&try_key).expect("try key json")["idempotency_key"]["len"],
        "farm-unit-try-key".len()
    );
}

#[test]
fn farm_request_builders_reject_invalid_options_and_timestamp_bounds() {
    let invalid_relays = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_A_D_TAG, "Invalid Relay Farm"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays(["http://relay.radroots.test"], SdkRelayUrlPolicy::Public);
    assert!(invalid_relays.is_err());

    let invalid_key = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_B_D_TAG, "Invalid Key Farm"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_idempotency_key("");
    assert!(invalid_key.is_err());

    let timestamp_error = farm_publish_plan(
        &farmer_actor(),
        farm(FARM_C_D_TAG, "Future Farm"),
        RadrootsSdkTimestamp::from_unix_seconds(u64::MAX),
    )
    .err()
    .expect("timestamp error");
    assert!(matches!(
        timestamp_error,
        RadrootsSdkError::TimestampOutOfRange { .. }
    ));
}

#[test]
fn farm_public_locality_derivation_covers_country_fallback_and_empty_names() {
    let reverse = GeocoderReverseResult {
        id: 1,
        name: " Fixture Town ".to_owned(),
        admin1_id: None,
        admin1_name: None,
        country_id: "FX".to_owned(),
        country_name: None,
        latitude: 12.25,
        longitude: -34.50,
    };
    let locality = public_locality_from_reverse(SdkExactLocation::new(12.26, -34.51), &reverse)
        .expect("locality");
    assert_eq!(locality.primary, "Fixture Town");
    assert_eq!(locality.city.as_deref(), Some("Fixture Town"));
    assert_eq!(locality.region, None);
    assert_eq!(locality.country.as_deref(), Some("FX"));
    assert_eq!(locality.geohash5, "e4pmw");

    let named_region = GeocoderReverseResult {
        admin1_name: Some(" Fixture Region ".to_owned()),
        country_name: Some(" Fixture Country ".to_owned()),
        ..reverse.clone()
    };
    let named_locality =
        public_locality_from_reverse(SdkExactLocation::new(12.26, -34.51), &named_region)
            .expect("named locality");
    assert_eq!(named_locality.region.as_deref(), Some("Fixture Region"));
    assert_eq!(named_locality.country.as_deref(), Some("Fixture Country"));

    let blank_optional_names = GeocoderReverseResult {
        admin1_name: Some(" ".to_owned()),
        country_name: Some(" ".to_owned()),
        ..reverse.clone()
    };
    let fallback_locality =
        public_locality_from_reverse(SdkExactLocation::new(12.26, -34.51), &blank_optional_names)
            .expect("fallback locality");
    assert_eq!(fallback_locality.region, None);
    assert_eq!(fallback_locality.country.as_deref(), Some("FX"));

    let blank_name = GeocoderReverseResult {
        name: " ".to_owned(),
        ..reverse
    };
    assert!(matches!(
        public_locality_from_reverse(SdkExactLocation::new(12.26, -34.51), &blank_name),
        Err(RadrootsSdkError::GeoNames {
            kind: crate::RadrootsSdkGeoNamesErrorKind::Lookup,
            ..
        })
    ));

    for location in [
        SdkExactLocation::new(f64::NAN, -34.51),
        SdkExactLocation::new(12.26, f64::INFINITY),
        SdkExactLocation::new(-90.1, -34.51),
        SdkExactLocation::new(90.1, -34.51),
        SdkExactLocation::new(12.26, -180.1),
        SdkExactLocation::new(12.26, 180.1),
    ] {
        assert!(matches!(
            validate_exact_location(location),
            Err(RadrootsSdkError::InvalidRequest { .. })
        ));
        assert!(matches!(
            geohash5(location),
            Err(RadrootsSdkError::InvalidRequest { .. })
        ));
    }
    assert!(matches!(
        sdk_timestamp_ms(RadrootsSdkTimestamp::from_unix_seconds(u64::MAX)),
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        sdk_timestamp_ms(RadrootsSdkTimestamp::from_unix_seconds(
            (i64::MAX as u64 / 1_000) + 1
        )),
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
}

#[tokio::test]
async fn farm_client_prepare_resolves_default_and_explicit_created_at() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_400))
        .build()
        .await
        .expect("sdk");
    let default_plan = sdk
        .farms()
        .prepare_publish(FarmPreparePublishRequest::new(
            farmer_actor(),
            farm(FARM_A_D_TAG, "Default Clock Farm"),
        ))
        .expect("default plan");
    assert_eq!(
        default_plan.created_at,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_400)
    );

    let explicit_plan = sdk
        .farms()
        .prepare_publish(
            FarmPreparePublishRequest::new(
                farmer_actor(),
                farm(FARM_B_D_TAG, "Explicit Clock Farm"),
            )
            .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_401)),
        )
        .expect("explicit plan");
    assert_eq!(
        explicit_plan.created_at,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_401)
    );
}

#[tokio::test]
async fn farm_client_prepare_reports_clock_errors() {
    let sdk = crate::RadrootsClient::builder()
        .clock(crate::RadrootsSdkClock::BeforeUnixEpoch)
        .build()
        .await
        .expect("sdk");
    let error = sdk
        .farms()
        .prepare_publish(FarmPreparePublishRequest::new(
            farmer_actor(),
            farm(FARM_A_D_TAG, "Clock Error Farm"),
        ))
        .expect_err("clock error");
    assert!(matches!(error, RadrootsSdkError::ClockBeforeUnixEpoch));
}

#[tokio::test]
async fn farm_enqueue_publish_reports_prepare_errors_before_signing() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .build()
        .await
        .expect("sdk");
    let error = sdk
        .farms()
        .enqueue_publish_with_explicit_signer(
            FarmEnqueuePublishRequest::new(
                farmer_actor(),
                farm("AAAAAAAAAAAAAAAAAAAAA!", "Invalid Enqueue Farm"),
                SdkRelayTargetPolicy::try_explicit([RELAY_A], SdkRelayUrlPolicy::Public)
                    .expect("target relays"),
            ),
            &FixtureSigner::new(FARMER),
        )
        .await
        .expect_err("prepare error");
    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
}

#[tokio::test]
async fn farm_client_enqueue_methods_cover_source_attached_workflow_paths() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .build()
        .await
        .expect("sdk");
    let signer = FixtureSigner::new(FARMER);
    let actor = farmer_actor();
    let receipt = sdk
        .farms()
        .enqueue_publish_with_explicit_signer(
            FarmEnqueuePublishRequest::new(
                actor.clone(),
                farm(FARM_A_D_TAG, "Enqueued Farm"),
                SdkRelayTargetPolicy::try_explicit([RELAY_A], SdkRelayUrlPolicy::Public)
                    .expect("target relays"),
            )
            .try_with_idempotency_key("farm-source-attached-enqueue")
            .expect("idempotency"),
            &signer,
        )
        .await
        .expect("enqueue farm");
    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);

    let plan = sdk
        .farms()
        .prepare_publish(FarmPreparePublishRequest::new(
            actor.clone(),
            farm(FARM_B_D_TAG, "Prepared Farm"),
        ))
        .expect("prepared farm");
    let prepared = sdk
        .farms()
        .enqueue_prepared_publish_with_explicit_signer(
            &actor,
            plan,
            SdkRelayTargetPolicy::try_explicit([RELAY_B], SdkRelayUrlPolicy::Public)
                .expect("prepared target relays"),
            None,
            &signer,
        )
        .await
        .expect("enqueue prepared farm");
    assert_eq!(prepared.signed_event_id, prepared.expected_event_id);
    assert_eq!(prepared.local_event_seq, 2);
}

#[tokio::test]
async fn farm_configured_local_signer_enqueues_publish_without_explicit_signer() {
    let keys = RadrootsNostrKeys::generate();
    let farmer = keys.public_key().to_hex();
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(
            RadrootsSdkLocalKeySigner::new(keys).expect("signer"),
        ))
        .build()
        .await
        .expect("sdk");
    let actor =
        RadrootsActorContext::test(farmer.as_str(), [RadrootsActorRole::Farmer]).expect("actor");

    let receipt = sdk
        .farms()
        .enqueue_publish(
            FarmEnqueuePublishRequest::new(
                actor,
                farm(FARM_C_D_TAG, "Configured Farm"),
                SdkRelayTargetPolicy::try_explicit([RELAY_A], SdkRelayUrlPolicy::Public)
                    .expect("target relays"),
            )
            .try_with_idempotency_key("farm-configured-local")
            .expect("idempotency"),
        )
        .await
        .expect("enqueue farm");

    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);
}

#[tokio::test]
async fn farm_configured_enqueue_reports_prepare_and_signer_errors() {
    let keys = RadrootsNostrKeys::generate();
    let farmer = keys.public_key().to_hex();
    let configured_sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(
            RadrootsSdkLocalKeySigner::new(keys).expect("signer"),
        ))
        .build()
        .await
        .expect("configured sdk");
    let actor =
        RadrootsActorContext::test(farmer.as_str(), [RadrootsActorRole::Farmer]).expect("actor");

    assert!(matches!(
        configured_sdk
            .farms()
            .enqueue_publish(FarmEnqueuePublishRequest::new(
                actor.clone(),
                farm("AAAAAAAAAAAAAAAAAAAAA!", "Invalid Configured Farm"),
                SdkRelayTargetPolicy::try_explicit([RELAY_A], SdkRelayUrlPolicy::Public)
                    .expect("target relays"),
            ))
            .await,
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let no_signer_sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .build()
        .await
        .expect("no signer sdk");
    let plan = no_signer_sdk
        .farms()
        .prepare_publish(FarmPreparePublishRequest::new(
            actor.clone(),
            farm(FARM_A_D_TAG, "Missing Configured Signer Farm"),
        ))
        .expect("plan");
    assert!(matches!(
        no_signer_sdk
            .farms()
            .enqueue_prepared_publish(
                &actor,
                plan,
                SdkRelayTargetPolicy::try_explicit([RELAY_A], SdkRelayUrlPolicy::Public)
                    .expect("target relays"),
                None,
            )
            .await,
        Err(RadrootsSdkError::SignerUnavailable { .. })
    ));
}

#[tokio::test]
async fn farm_private_location_default_client_and_lookup_report_store_edges() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let geocoder = fixture_geocoder(&tempdir, Some("Fixture Town")).await;
    let empty_geocoder = fixture_geocoder(&tempdir, None).await;
    let blank_geocoder = fixture_geocoder(&tempdir, Some(" ")).await;
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .build()
        .await
        .expect("sdk");
    let actor = farmer_actor();
    let request = FarmPrivateLocationUpsertRequest::new(
        actor.clone(),
        FARM_A_D_TAG,
        SdkExactLocation::new(12.26, -34.51),
    );

    assert!(matches!(
        sdk.farms().upsert_private_location(request).await,
        Err(RadrootsSdkError::GeoNames {
            kind: crate::RadrootsSdkGeoNamesErrorKind::Configuration,
            ..
        })
    ));

    let farm_addr = farm_addr(&actor, FARM_A_D_TAG).expect("farm addr");
    assert_eq!(
        sdk.farms()
            .private_location(&farm_addr)
            .await
            .expect("missing location"),
        None
    );

    let stored = sdk
        .farms()
        .upsert_private_location_with_geocoder(
            FarmPrivateLocationUpsertRequest::new(
                actor.clone(),
                FARM_A_D_TAG,
                SdkExactLocation::new(12.26, -34.51),
            )
            .with_updated_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_501)),
            &geocoder,
        )
        .await
        .expect("stored location");
    assert_eq!(stored.farm_addr, farm_addr);
    assert_eq!(stored.public_locality.primary, "Fixture Town");
    assert_eq!(
        sdk.farms()
            .private_location(&farm_addr)
            .await
            .expect("stored lookup")
            .expect("stored location")
            .updated_at_ms,
        1_700_000_501_000
    );

    assert!(matches!(
        sdk.farms()
            .upsert_private_location_with_geocoder(
                FarmPrivateLocationUpsertRequest::new(
                    actor.clone(),
                    FARM_B_D_TAG,
                    SdkExactLocation::new(12.26, -34.51),
                )
                .with_updated_at(RadrootsSdkTimestamp::from_unix_seconds(u64::MAX)),
                &geocoder,
            )
            .await,
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        sdk.farms()
            .upsert_private_location_with_geocoder(
                FarmPrivateLocationUpsertRequest::new(
                    actor.clone(),
                    FARM_B_D_TAG,
                    SdkExactLocation::new(12.26, -34.51),
                ),
                &empty_geocoder,
            )
            .await,
        Err(RadrootsSdkError::GeoNames {
            kind: crate::RadrootsSdkGeoNamesErrorKind::Lookup,
            ..
        })
    ));
    assert!(matches!(
        sdk.farms()
            .upsert_private_location_with_geocoder(
                FarmPrivateLocationUpsertRequest::new(
                    actor.clone(),
                    "bad d tag",
                    SdkExactLocation::new(12.26, -34.51),
                ),
                &geocoder,
            )
            .await,
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
    assert!(matches!(
        sdk.farms()
            .upsert_private_location_with_geocoder(
                FarmPrivateLocationUpsertRequest::new(
                    actor.clone(),
                    FARM_B_D_TAG,
                    SdkExactLocation::new(12.26, -34.51),
                ),
                &blank_geocoder,
            )
            .await,
        Err(RadrootsSdkError::GeoNames {
            kind: crate::RadrootsSdkGeoNamesErrorKind::Lookup,
            ..
        })
    ));
    let clock_error_sdk = crate::RadrootsClient::builder()
        .clock(crate::RadrootsSdkClock::BeforeUnixEpoch)
        .build()
        .await
        .expect("clock error sdk");
    assert!(matches!(
        clock_error_sdk
            .farms()
            .upsert_private_location_with_geocoder(
                FarmPrivateLocationUpsertRequest::new(
                    actor.clone(),
                    FARM_B_D_TAG,
                    SdkExactLocation::new(12.26, -34.51),
                ),
                &geocoder,
            )
            .await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));

    sdk._private_store.pool().close().await;
    assert!(matches!(
        sdk.farms()
            .upsert_private_location_with_geocoder(
                FarmPrivateLocationUpsertRequest::new(
                    actor.clone(),
                    FARM_C_D_TAG,
                    SdkExactLocation::new(12.26, -34.51),
                ),
                &geocoder,
            )
            .await,
        Err(RadrootsSdkError::PrivateStore { .. })
    ));
    assert!(matches!(
        sdk.farms().private_location(&farm_addr).await,
        Err(RadrootsSdkError::PrivateStore { .. })
    ));
}

#![cfg(feature = "runtime")]

use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity,
};
use radroots_event_store::RadrootsEventStore;
use radroots_events::{
    contract::RadrootsActorRole,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent, RadrootsSignedNostrEventParts},
    farm::RadrootsFarm,
    ids::RadrootsAddressableCoordinate,
    kinds::{KIND_FARM, KIND_PROFILE},
};
use radroots_outbox::{RadrootsOutbox, RadrootsOutboxEventState};
use radroots_sdk::{
    FARM_PUBLISH_OPERATION_KIND, FarmEnqueuePublishRequest, FarmPreparePublishRequest,
    FarmPrivateLocationClearRequest, FarmPrivateLocationInput, FarmPrivateLocationLookupCandidate,
    FarmPrivateLocationLookupReceipt, FarmPrivateLocationReceipt, FarmPrivateLocationSetRequest,
    FarmPrivateLocationSetResult, FarmPrivateLocationUpsertRequest, Geocoder,
    GeocoderLocalityQuery, NostrProfile, NostrRelayUrlPolicy, PushOutboxEventState,
    PushOutboxRequest, PushOutboxTargetOutcomeKind, RadrootsClient, RadrootsSdkError,
    RadrootsSdkErrorClass, RadrootsSdkGeoNamesErrorKind, RadrootsSdkRecoveryAction,
    RadrootsSdkTimestamp, SdkExactLocation, SdkIdempotencyKey, SdkMutationState, SdkPublicLocality,
    StorageStatusRequest, TargetPolicy, TargetSet, TransportProfile,
};
use radroots_transport_nostr::RadrootsMockRelayPublishAdapter;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[path = "support/serializer_failure.rs"]
mod serializer_failure;

use serializer_failure::assert_struct_serialize_error_paths;

const FARMER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FARM_A_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const FARM_B_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const FARM_C_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const FARM_D_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAw";
const FARM_E_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABA";
const FARM_F_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAABQ";
const RELAY: &str = "wss://relay.example.com";
const RELAY_B: &str = "wss://relay-b.example.com";

#[derive(Clone)]
struct FixtureSigner {
    identity: RadrootsSignerIdentity,
}

impl FixtureSigner {
    fn new(pubkey: &str) -> Self {
        Self {
            identity: RadrootsSignerIdentity::new(pubkey).expect("identity"),
        }
    }
}

impl RadrootsEventSigner for FixtureSigner {
    fn pubkey(&self) -> &radroots_events::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsFrozenEventDraft,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
        if self.pubkey().as_str() != draft.expected_pubkey.as_str() {
            return Err(RadrootsSignerError::SigningFailed {
                message: "wrong fixture signer".to_owned(),
            });
        }
        let sig = "f".repeat(128);
        let raw_json = serde_json::json!({
            "id": draft.expected_event_id,
            "pubkey": self.pubkey().as_str(),
            "created_at": draft.created_at,
            "kind": draft.kind,
            "tags": draft.tags,
            "content": draft.content,
            "sig": sig,
        })
        .to_string();
        RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: draft.expected_event_id.clone(),
            pubkey: self.pubkey().as_str().to_owned(),
            created_at: draft.created_at,
            kind: draft.kind,
            tags: draft.tags.clone(),
            content: draft.content.clone(),
            sig,
            raw_json,
        })
        .map_err(|error| RadrootsSignerError::SigningFailed {
            message: error.to_string(),
        })
    }
}

fn farmer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(FARMER, [RadrootsActorRole::Farmer]).expect("actor")
}

fn non_farmer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(FARMER, [RadrootsActorRole::Buyer]).expect("actor")
}

fn farm(d_tag: &str, name: &str) -> RadrootsFarm {
    RadrootsFarm {
        d_tag: d_tag.to_owned(),
        name: name.to_owned(),
        about: Some("Vegetable farm".to_owned()),
        website: Some("https://example.invalid/north-farm".to_owned()),
        picture: None,
        banner: None,
        location: None,
        tags: Some(vec!["vegetables".to_owned(), "local".to_owned()]),
    }
}

fn farm_addr(actor: &RadrootsActorContext, d_tag: &str) -> RadrootsAddressableCoordinate {
    RadrootsAddressableCoordinate::parse(format!("{KIND_FARM}:{}:{d_tag}", actor.pubkey()))
        .expect("farm addr")
}

fn stored_location(result: FarmPrivateLocationSetResult) -> FarmPrivateLocationReceipt {
    let FarmPrivateLocationSetResult::Stored(receipt) = result else {
        panic!("expected stored location");
    };
    receipt
}

async fn directory_sdk() -> (tempfile::TempDir, RadrootsClient) {
    directory_sdk_with_relays(&[RELAY]).await
}

async fn directory_sdk_with_relays(relays: &[&str]) -> (tempfile::TempDir, RadrootsClient) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let mut builder = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000));
    if !relays.is_empty() {
        builder = builder.transport_profile(TransportProfile::nostr(
            NostrProfile::new(relays.iter().copied(), NostrRelayUrlPolicy::Public)
                .expect("Nostr profile"),
        ));
    }
    let sdk = builder.build().await.expect("sdk");
    (tempdir, sdk)
}

async fn fixture_geocoder(tempdir: &tempfile::TempDir) -> Geocoder {
    let path = tempdir.path().join("geonames-fixture.db");
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
        INSERT INTO countries (id, name) VALUES ('FX', 'Fixture Country');
        INSERT INTO countries (id, name) VALUES ('CA', 'Canada');
        INSERT INTO countries (id, name) VALUES ('US', 'United States');
        INSERT INTO admin1 (country_id, id, name) VALUES ('FX', 1, 'Fixture Region');
        INSERT INTO admin1 (country_id, id, name) VALUES ('CA', 2, 'British Columbia');
        INSERT INTO admin1 (country_id, id, name) VALUES ('CA', 3, 'Prairie Region');
        INSERT INTO admin1 (country_id, id, name) VALUES ('US', 4, 'River Region');
        INSERT INTO features (id, name, country_id, admin1_id) VALUES (1, 'Fixture Town', 'FX', 1);
        INSERT INTO features (id, name, country_id, admin1_id) VALUES (3001, 'Fixture Victoria', 'CA', 2);
        INSERT INTO features (id, name, country_id, admin1_id) VALUES (3002, 'Shared Market', 'CA', 2);
        INSERT INTO features (id, name, country_id, admin1_id) VALUES (3003, 'Shared Market', 'CA', 3);
        INSERT INTO features (id, name, country_id, admin1_id) VALUES (3004, 'Identifier Grove', 'CA', 2);
        INSERT INTO features (id, name, country_id, admin1_id) VALUES (3005, 'Query Hamlet', 'US', 4);
        INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (1, 12.25, -34.50);
        INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (3001, 48.4359, -123.35155);
        INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (3002, 48.7, -123.2);
        INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (3003, 50.2, -110.4);
        INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (3004, 48.9, -123.4);
        INSERT INTO coordinates (feature_id, latitude, longitude) VALUES (3005, 39.25, -77.5);
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed geonames fixture");
    pool.close().await;
    Geocoder::open_path(path).expect("open geonames fixture")
}

#[tokio::test]
async fn farm_prepare_publish_is_side_effect_free() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = FarmPreparePublishRequest::new(farmer_actor(), farm(FARM_A_D_TAG, "North Farm"));
    let prepared = sdk.farms().prepare_publish(request).expect("prepared");

    assert_eq!(prepared.frozen_draft.kind, KIND_FARM);
    assert_eq!(prepared.created_at.unix_seconds(), 1_700_000_000);
    assert_eq!(
        prepared.expected_event_id,
        prepared.frozen_draft.expected_event_id
    );
    assert_eq!(
        prepared.farm_addr.as_str(),
        format!("{KIND_FARM}:{FARMER}:{FARM_A_D_TAG}")
    );

    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    assert_eq!(
        event_store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
    assert!(
        event_store
            .get_event(prepared.expected_event_id.as_str())
            .await
            .expect("event lookup")
            .is_none()
    );
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    assert!(
        outbox
            .claim_next_ready_event("worker", "claim", 2_000, 1_700_000_000_000)
            .await
            .expect("claim")
            .is_none()
    );
}

#[tokio::test]
async fn farm_prepare_publish_rejects_non_farmer_actor() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request =
        FarmPreparePublishRequest::new(non_farmer_actor(), farm(FARM_B_D_TAG, "North Farm"));

    let error = sdk
        .farms()
        .prepare_publish(request)
        .expect_err("non farmer");

    assert!(matches!(error, RadrootsSdkError::UnauthorizedActor { .. }));
}

#[tokio::test]
async fn farm_private_location_upsert_stores_exact_location_and_public_locality_without_events() {
    let (tempdir, sdk) = directory_sdk().await;
    let geocoder = fixture_geocoder(&tempdir).await;
    let request = FarmPrivateLocationUpsertRequest::new(
        farmer_actor(),
        FARM_A_D_TAG,
        SdkExactLocation::new(12.26, -34.51),
    )
    .with_updated_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_123));

    let receipt = sdk
        .farms()
        .upsert_private_location_with_geocoder(request, &geocoder)
        .await
        .expect("upsert private location");

    assert_eq!(
        receipt.farm_addr.as_str(),
        format!("{KIND_FARM}:{FARMER}:{FARM_A_D_TAG}")
    );
    assert_eq!(receipt.farm_pubkey, FARMER);
    assert_eq!(receipt.farm_d_tag, FARM_A_D_TAG);
    assert_eq!(receipt.label, None);
    assert_eq!(receipt.exact_location, SdkExactLocation::new(12.26, -34.51));
    assert_eq!(receipt.public_locality.primary, "Fixture Town");
    assert_eq!(
        receipt.public_locality.city.as_deref(),
        Some("Fixture Town")
    );
    assert_eq!(
        receipt.public_locality.region.as_deref(),
        Some("Fixture Region")
    );
    assert_eq!(
        receipt.public_locality.country.as_deref(),
        Some("Fixture Country")
    );
    assert_eq!(receipt.public_locality.geohash5, "e4pmw");
    assert_eq!(receipt.geonames_feature_id, Some(1));
    assert_eq!(receipt.geonames_country_id.as_deref(), Some("FX"));
    assert_eq!(receipt.updated_at_ms, 1_700_000_123_000);
    let farm_public = receipt.public_locality.to_farm_public_location();
    assert_eq!(farm_public.primary, "Fixture Town");
    assert_eq!(farm_public.geohash, "e4pmw");
    let listing_public = receipt.public_locality.to_listing_public_location();
    assert_eq!(listing_public.primary, "Fixture Town");
    assert_eq!(listing_public.geohash, "e4pmw");

    let stored = sdk
        .farms()
        .private_location(&receipt.farm_addr)
        .await
        .expect("private location read")
        .expect("stored private location");
    assert_eq!(stored, receipt);
    let status = sdk
        .storage_status(StorageStatusRequest::new())
        .await
        .expect("status");
    assert_eq!(status.private_store.farm_private_locations, 1);
    assert_eq!(status.event_store.total_events, 0);
    assert_eq!(status.outbox.total_events, 0);

    let clock_receipt = sdk
        .farms()
        .upsert_private_location_with_geocoder(
            FarmPrivateLocationUpsertRequest::new(
                farmer_actor(),
                FARM_B_D_TAG,
                SdkExactLocation::new(12.26, -34.51),
            ),
            &geocoder,
        )
        .await
        .expect("upsert with sdk clock");
    assert_eq!(clock_receipt.updated_at_ms, 1_700_000_000_000);
}

#[tokio::test]
async fn farm_private_location_set_resolves_forward_inputs_and_preserves_no_mutation_failures() {
    let (tempdir, sdk) = directory_sdk().await;
    let geocoder = fixture_geocoder(&tempdir).await;
    let actor = farmer_actor();

    let exact = stored_location(
        sdk.farms()
            .set_private_location_with_geocoder(
                FarmPrivateLocationSetRequest::exact(
                    actor.clone(),
                    FARM_A_D_TAG,
                    SdkExactLocation::new(12.26, -34.51),
                )
                .with_label("  main pickup point  ")
                .with_updated_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_200)),
                &geocoder,
            )
            .await
            .expect("exact set"),
    );
    assert_eq!(exact.label.as_deref(), Some("main pickup point"));
    assert_eq!(exact.geonames_feature_id, Some(1));
    assert_eq!(exact.updated_at_ms, 1_700_000_200_000);

    let city = stored_location(
        sdk.farms()
            .set_private_location_with_geocoder(
                FarmPrivateLocationSetRequest::city(
                    actor.clone(),
                    FARM_B_D_TAG,
                    "Fixture Victoria",
                )
                .with_updated_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_201)),
                &geocoder,
            )
            .await
            .expect("city set"),
    );
    assert_eq!(city.geonames_feature_id, Some(3001));
    assert_eq!(city.public_locality.primary, "Fixture Victoria");
    assert_eq!(
        city.public_locality.region.as_deref(),
        Some("British Columbia")
    );
    assert_eq!(city.public_locality.country.as_deref(), Some("Canada"));
    assert_eq!(
        city.exact_location,
        SdkExactLocation::new(48.4359, -123.35155)
    );

    let query = stored_location(
        sdk.farms()
            .set_private_location_with_geocoder(
                FarmPrivateLocationSetRequest::query(
                    actor.clone(),
                    FARM_C_D_TAG,
                    "Fixture Victoria, BC, CA",
                ),
                &geocoder,
            )
            .await
            .expect("query set"),
    );
    assert_eq!(query.geonames_feature_id, Some(3001));

    let selected = stored_location(
        sdk.farms()
            .set_private_location_with_geocoder(
                FarmPrivateLocationSetRequest::geonames_id(actor.clone(), FARM_D_D_TAG, 3004),
                &geocoder,
            )
            .await
            .expect("id set"),
    );
    assert_eq!(selected.geonames_feature_id, Some(3004));
    assert_eq!(selected.public_locality.primary, "Identifier Grove");

    let narrowed = stored_location(
        sdk.farms()
            .set_private_location_with_geocoder(
                FarmPrivateLocationSetRequest::new(
                    actor.clone(),
                    FARM_E_D_TAG,
                    FarmPrivateLocationInput::Locality(
                        GeocoderLocalityQuery::structured("Shared Market")
                            .with_region("Prairie Region")
                            .with_country("CA"),
                    ),
                ),
                &geocoder,
            )
            .await
            .expect("structured narrowed set"),
    );
    assert_eq!(narrowed.geonames_feature_id, Some(3003));
    assert_eq!(
        narrowed.public_locality.region.as_deref(),
        Some("Prairie Region")
    );

    let before_failure_status = sdk
        .storage_status(StorageStatusRequest::new())
        .await
        .expect("pre-failure status")
        .private_store
        .farm_private_locations;

    let ambiguous = sdk
        .farms()
        .set_private_location_with_geocoder(
            FarmPrivateLocationSetRequest::new(
                actor.clone(),
                FARM_F_D_TAG,
                FarmPrivateLocationInput::Locality(
                    GeocoderLocalityQuery::structured("Shared Market").with_country("CA"),
                ),
            ),
            &geocoder,
        )
        .await
        .expect("ambiguous set");
    let FarmPrivateLocationSetResult::Ambiguous(ambiguous) = ambiguous else {
        panic!("expected ambiguous result");
    };
    assert_eq!(
        ambiguous
            .candidates
            .iter()
            .map(|candidate| candidate.geonames_feature_id)
            .collect::<Vec<_>>(),
        vec![3002, 3003]
    );

    let missing = sdk
        .farms()
        .set_private_location_with_geocoder(
            FarmPrivateLocationSetRequest::query(actor.clone(), FARM_F_D_TAG, "Missing Market, CA"),
            &geocoder,
        )
        .await
        .expect("missing set");
    assert!(matches!(missing, FarmPrivateLocationSetResult::NoMatch(_)));

    assert_eq!(
        sdk.storage_status(StorageStatusRequest::new())
            .await
            .expect("post-failure status")
            .private_store
            .farm_private_locations,
        before_failure_status
    );
    assert_eq!(
        sdk.farms()
            .private_location(&farm_addr(&actor, FARM_F_D_TAG))
            .await
            .expect("failure location lookup"),
        None
    );

    assert!(matches!(
        sdk.farms()
            .set_private_location_with_geocoder(
                FarmPrivateLocationSetRequest::city(actor, FARM_F_D_TAG, "Fixture Victoria")
                    .with_label(" "),
                &geocoder,
            )
            .await,
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
}

#[tokio::test]
async fn farm_private_location_requires_farmer_role_and_valid_coordinates() {
    let (tempdir, sdk) = directory_sdk().await;
    let geocoder = fixture_geocoder(&tempdir).await;
    let non_farmer = sdk
        .farms()
        .upsert_private_location_with_geocoder(
            FarmPrivateLocationUpsertRequest::new(
                non_farmer_actor(),
                FARM_B_D_TAG,
                SdkExactLocation::new(12.26, -34.51),
            ),
            &geocoder,
        )
        .await
        .expect_err("non farmer");
    assert!(matches!(
        non_farmer,
        RadrootsSdkError::UnauthorizedActor { .. }
    ));

    let invalid = sdk
        .farms()
        .upsert_private_location_with_geocoder(
            FarmPrivateLocationUpsertRequest::new(
                farmer_actor(),
                FARM_B_D_TAG,
                SdkExactLocation::new(91.0, -34.51),
            ),
            &geocoder,
        )
        .await
        .expect_err("invalid coordinates");
    assert!(matches!(invalid, RadrootsSdkError::InvalidRequest { .. }));
}

#[tokio::test]
async fn farm_private_location_requires_configured_geonames_for_default_upsert() {
    let (_tempdir, sdk) = directory_sdk().await;
    let error = sdk
        .farms()
        .upsert_private_location(FarmPrivateLocationUpsertRequest::new(
            farmer_actor(),
            FARM_C_D_TAG,
            SdkExactLocation::new(12.26, -34.51),
        ))
        .await
        .expect_err("missing geonames config");

    assert!(matches!(
        error,
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Configuration,
            ..
        }
    ));
    assert_eq!(error.class(), RadrootsSdkErrorClass::Configuration);
    assert_eq!(
        error.recovery_actions(),
        vec![RadrootsSdkRecoveryAction::ConfigureGeoNamesCache]
    );
}

#[tokio::test]
async fn farm_enqueue_publish_stores_event_and_queues_signed_outbox_without_profile_event() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_B_D_TAG, "North Farm"),
        TargetPolicy::use_transport_profile(),
    )
    .try_with_idempotency_key("farm-idem-b")
    .expect("idempotency key");
    let prepared = sdk
        .farms()
        .prepare_publish(FarmPreparePublishRequest::new(
            farmer_actor(),
            farm(FARM_B_D_TAG, "North Farm"),
        ))
        .expect("prepared");
    let receipt = sdk
        .farms()
        .enqueue_publish_with_explicit_signer(request, &FixtureSigner::new(FARMER))
        .await
        .expect("enqueue");

    assert_eq!(receipt.expected_event_id, prepared.expected_event_id);
    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.farm_addr, prepared.farm_addr);
    assert_eq!(receipt.local_event_seq, 1);
    assert_eq!(receipt.outbox_operation_id, 1);
    assert_eq!(receipt.outbox_event_id, 1);
    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);
    assert!(receipt.idempotency_digest_prefix.is_some());

    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    let status = event_store
        .status_summary()
        .await
        .expect("event store status");
    assert_eq!(status.total_events, 1);
    let stored_event = event_store
        .get_event(receipt.signed_event_id.as_str())
        .await
        .expect("event lookup")
        .expect("stored event");
    assert_eq!(stored_event.kind, KIND_FARM);
    assert_ne!(stored_event.kind, KIND_PROFILE);
    assert_eq!(
        stored_event.contract_id.as_deref(),
        Some("radroots.farm.profile.v1")
    );

    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    let outbox_event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("outbox event")
        .expect("outbox event");
    assert_eq!(outbox_event.state, RadrootsOutboxEventState::Signed);
    assert_eq!(outbox_event.draft.kind, KIND_FARM);
    assert!(outbox_event.signed_event.is_some());
}

#[tokio::test]
async fn farm_enqueue_publish_returns_sanitized_signer_errors_before_mutation() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_C_D_TAG, "North Farm"),
        TargetPolicy::use_transport_profile(),
    );
    let error = sdk
        .farms()
        .enqueue_publish_with_explicit_signer(request, &FixtureSigner::new(OTHER))
        .await
        .expect_err("signer error");
    let message = error.to_string();

    assert!(matches!(
        error,
        RadrootsSdkError::SignerPubkeyMismatch { .. }
    ));
    assert!(!message.contains("raw"));
    assert!(!message.contains("ffff"));

    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    assert_eq!(
        event_store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    assert!(
        outbox
            .claim_next_ready_event("worker", "claim", 2_000, 1_700_000_000_000)
            .await
            .expect("claim")
            .is_none()
    );
}

#[tokio::test]
async fn farm_enqueue_publish_derives_order_independent_idempotency_key() {
    let (_tempdir, sdk) = directory_sdk().await;
    let first = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_D_D_TAG, "North Farm"),
        TargetPolicy::use_transport_profile(),
    )
    .try_with_nostr_targets([RELAY_B, RELAY], NostrRelayUrlPolicy::Public)
    .expect("first transport targets");
    let second = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_D_D_TAG, "North Farm"),
        TargetPolicy::explicit(
            TargetSet::new([RELAY, RELAY_B], NostrRelayUrlPolicy::Public)
                .expect("second transport targets"),
        ),
    );

    let first_receipt = sdk
        .farms()
        .enqueue_publish_with_explicit_signer(first, &FixtureSigner::new(FARMER))
        .await
        .expect("first enqueue");
    let second_receipt = sdk
        .farms()
        .enqueue_publish_with_explicit_signer(second, &FixtureSigner::new(FARMER))
        .await
        .expect("second enqueue");

    assert_eq!(
        first_receipt.outbox_event_id,
        second_receipt.outbox_event_id
    );
    assert_eq!(
        first_receipt.idempotency_digest_prefix,
        second_receipt.idempotency_digest_prefix
    );
    assert_eq!(second_receipt.state, SdkMutationState::AlreadyQueued);

    let paths = sdk.storage_paths().expect("paths");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    let relay_urls = outbox
        .delivery_targets(first_receipt.outbox_event_id)
        .await
        .expect("delivery targets")
        .into_iter()
        .map(|target| target.endpoint_uri.to_string())
        .collect::<Vec<_>>();
    assert_eq!(relay_urls, vec![RELAY_B.to_owned(), RELAY.to_owned()]);
}

#[tokio::test]
async fn farm_enqueue_publish_pushes_queued_event_with_mock_relay_sync() {
    let (_tempdir, sdk) = directory_sdk().await;
    let enqueue_request = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_D_D_TAG, "Sync Farm"),
        TargetPolicy::use_transport_profile(),
    )
    .try_with_nostr_targets([RELAY], NostrRelayUrlPolicy::Public)
    .expect("transport targets");
    let enqueue_receipt = sdk
        .farms()
        .enqueue_publish_with_explicit_signer(enqueue_request, &FixtureSigner::new(FARMER))
        .await
        .expect("enqueue");
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let push_receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await
        .expect("push");

    assert_eq!(push_receipt.attempted_events, 1);
    assert_eq!(push_receipt.published_events, 1);
    assert_eq!(push_receipt.retryable_events, 0);
    assert_eq!(push_receipt.terminal_events, 0);
    assert_eq!(push_receipt.events.len(), 1);
    let event = &push_receipt.events[0];
    assert_eq!(event.event_id, enqueue_receipt.signed_event_id);
    assert_eq!(event.outbox_event_id, enqueue_receipt.outbox_event_id);
    assert_eq!(event.final_state, PushOutboxEventState::Published);
    assert_eq!(event.attempted_count, 1);
    assert_eq!(event.accepted_count, 1);
    assert_eq!(event.retryable_count, 0);
    assert_eq!(event.terminal_count, 0);
    assert_eq!(event.quorum, 1);
    assert!(event.quorum_met);
    assert_eq!(event.targets.len(), 1);
    assert_eq!(event.targets[0].endpoint_uri, RELAY);
    assert_eq!(
        event.targets[0].outcome_kind,
        PushOutboxTargetOutcomeKind::Accepted
    );
    assert_eq!(adapter.captured_raw_events().len(), 1);

    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
        .await
        .expect("outbox");
    let stored = outbox
        .get_event(enqueue_receipt.outbox_event_id)
        .await
        .expect("stored")
        .expect("stored");
    assert_eq!(stored.state, RadrootsOutboxEventState::Published);
}

#[tokio::test]
async fn farm_enqueue_publish_reports_preflight_idempotency_conflict_without_mutation() {
    let (_tempdir, sdk) = directory_sdk().await;
    let first = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_E_D_TAG, "North Farm"),
        TargetPolicy::use_transport_profile(),
    )
    .try_with_idempotency_key("farm-idem-e")
    .expect("idempotency key");
    sdk.farms()
        .enqueue_publish_with_explicit_signer(first, &FixtureSigner::new(FARMER))
        .await
        .expect("first enqueue");
    let paths = sdk.storage_paths().expect("paths");
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path)
        .await
        .expect("event store");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    assert_eq!(
        event_store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        1
    );
    assert_eq!(
        outbox
            .status_summary(0)
            .await
            .expect("outbox status")
            .total_events,
        1
    );

    let second = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_F_D_TAG, "Changed Farm"),
        TargetPolicy::use_transport_profile(),
    )
    .try_with_idempotency_key("farm-idem-e")
    .expect("idempotency key");
    let error = sdk
        .farms()
        .enqueue_publish_with_explicit_signer(second, &FixtureSigner::new(FARMER))
        .await
        .expect_err("conflict");

    assert!(matches!(
        error,
        RadrootsSdkError::IdempotencyConflict { ref operation_kind, .. }
            if operation_kind == FARM_PUBLISH_OPERATION_KIND
    ));
    assert_eq!(
        error.recovery_actions(),
        vec![RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey]
    );
    assert!(!error.to_string().contains("farm-idem-e"));
    assert_eq!(
        event_store
            .status_summary()
            .await
            .expect("event store status after conflict")
            .total_events,
        1
    );
    assert_eq!(
        outbox
            .status_summary(0)
            .await
            .expect("outbox status after conflict")
            .total_events,
        1
    );
}

#[tokio::test]
async fn farm_runtime_dtos_serialize_deterministically() {
    let (_tempdir, sdk) = directory_sdk().await;
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_123);
    let prepare_request =
        FarmPreparePublishRequest::new(farmer_actor(), farm(FARM_A_D_TAG, "Serialized Farm"))
            .with_created_at(created_at);
    let prepare_json = serde_json::to_value(&prepare_request).expect("prepare request json");
    assert_struct_serialize_error_paths(&prepare_request, 3);

    assert_eq!(
        prepare_json,
        serde_json::json!({
            "actor": {
                "pubkey": FARMER,
                "roles": ["farmer"],
                "account_id": null,
                "source": "test"
            },
            "farm": {
                "d_tag": FARM_A_D_TAG,
                "name": "Serialized Farm",
                "about": "Vegetable farm",
                "website": "https://example.invalid/north-farm",
                "picture": null,
                "banner": null,
                "location": null,
                "tags": ["vegetables", "local"]
            },
            "created_at": 1_700_000_123
        })
    );

    let enqueue_request = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_B_D_TAG, "Queued Farm"),
        TargetPolicy::use_transport_profile(),
    )
    .try_with_nostr_targets([RELAY, RELAY_B], NostrRelayUrlPolicy::Public)
    .expect("relay targets")
    .with_idempotency_key(
        SdkIdempotencyKey::new("farm-serialized-idempotency").expect("idempotency"),
    )
    .with_created_at(created_at);
    let enqueue_json = serde_json::to_value(&enqueue_request).expect("enqueue request json");
    assert_struct_serialize_error_paths(&enqueue_request, 5);

    assert_eq!(
        enqueue_json,
        serde_json::json!({
            "actor": {
                "pubkey": FARMER,
                "roles": ["farmer"],
                "account_id": null,
                "source": "test"
            },
            "farm": {
                "d_tag": FARM_B_D_TAG,
                "name": "Queued Farm",
                "about": "Vegetable farm",
                "website": "https://example.invalid/north-farm",
                "picture": null,
                "banner": null,
                "location": null,
                "tags": ["vegetables", "local"]
            },
            "target_policy": {
                "kind": "explicit",
                "targets": [
                    {
                        "kind": "Nostr",
                        "uri": RELAY,
                        "fingerprint": "a1997ec4596596af6ffc65e6a30ab7cffa53ea71f524c1c86d64018b96d130af"
                    },
                    {
                        "kind": "Nostr",
                        "uri": RELAY_B,
                        "fingerprint": "5136077cfe7eddcbfaddc5d7bf1f42cdbb8191f3691b86ccc3a81047851cef05"
                    }
                ],
                "canonical_targets": [
                    "5136077cfe7eddcbfaddc5d7bf1f42cdbb8191f3691b86ccc3a81047851cef05",
                    "a1997ec4596596af6ffc65e6a30ab7cffa53ea71f524c1c86d64018b96d130af"
                ]
            },
            "idempotency_key": { "value": "<redacted>", "len": 27 },
            "created_at": 1_700_000_123
        })
    );
    assert!(
        !enqueue_json
            .to_string()
            .contains("farm-serialized-idempotency")
    );

    let try_key_request = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_C_D_TAG, "Queued Farm"),
        TargetPolicy::use_transport_profile(),
    )
    .try_with_idempotency_key("farm-serialized-try-key")
    .expect("try idempotency key");
    assert_eq!(
        serde_json::to_value(&try_key_request).expect("try key request json")["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 23 })
    );

    let private_upsert = FarmPrivateLocationUpsertRequest::new(
        farmer_actor(),
        FARM_C_D_TAG,
        SdkExactLocation::new(48.4359, -123.35155),
    )
    .with_label("north gate")
    .with_updated_at(created_at);
    assert_eq!(
        serde_json::to_value(&private_upsert).expect("private upsert json"),
        serde_json::json!({
            "actor": {
                "pubkey": FARMER,
                "roles": ["farmer"],
                "account_id": null,
                "source": "test"
            },
            "farm_d_tag": FARM_C_D_TAG,
            "exact_location": {
                "latitude": 48.4359,
                "longitude": -123.35155
            },
            "label": "north gate",
            "updated_at": 1_700_000_123
        })
    );
    assert_struct_serialize_error_paths(&private_upsert, 5);

    let private_set = FarmPrivateLocationSetRequest::exact(
        farmer_actor(),
        FARM_D_D_TAG,
        SdkExactLocation::new(48.9, -123.4),
    )
    .with_label("identifier gate")
    .with_updated_at(created_at);
    assert_eq!(
        serde_json::to_value(&private_set).expect("private set json"),
        serde_json::json!({
            "actor": {
                "pubkey": FARMER,
                "roles": ["farmer"],
                "account_id": null,
                "source": "test"
            },
            "farm_d_tag": FARM_D_D_TAG,
            "input": {
                "kind": "exact",
                "value": {
                    "latitude": 48.9,
                    "longitude": -123.4
                }
            },
            "label": "identifier gate",
            "updated_at": 1_700_000_123
        })
    );
    assert_struct_serialize_error_paths(&private_set, 5);

    assert_eq!(
        serde_json::to_value(FarmPrivateLocationSetRequest::query(
            farmer_actor(),
            FARM_D_D_TAG,
            "Shared Market, BC, CA"
        ))
        .expect("private query set json")["input"],
        serde_json::json!({
            "kind": "locality",
            "value": {
                "input": {
                    "Query": "Shared Market, BC, CA"
                },
                "limit": 10
            }
        })
    );
    assert_eq!(
        serde_json::to_value(FarmPrivateLocationSetRequest::geonames_id(
            farmer_actor(),
            FARM_D_D_TAG,
            3004
        ))
        .expect("private geonames id set json")["input"],
        serde_json::json!({
            "kind": "locality",
            "value": {
                "input": {
                    "FeatureId": 3004
                },
                "limit": 10
            }
        })
    );

    let private_clear = FarmPrivateLocationClearRequest::new(farmer_actor(), FARM_E_D_TAG);
    assert_eq!(
        serde_json::to_value(&private_clear).expect("private clear json"),
        serde_json::json!({
            "actor": {
                "pubkey": FARMER,
                "roles": ["farmer"],
                "account_id": null,
                "source": "test"
            },
            "farm_d_tag": FARM_E_D_TAG
        })
    );
    assert_struct_serialize_error_paths(&private_clear, 2);

    let private_receipt = FarmPrivateLocationReceipt {
        farm_addr: farm_addr(&farmer_actor(), FARM_D_D_TAG),
        farm_pubkey: FARMER.to_owned(),
        farm_d_tag: FARM_D_D_TAG.to_owned(),
        label: Some("identifier gate".to_owned()),
        exact_location: SdkExactLocation::new(48.9, -123.4),
        public_locality: SdkPublicLocality {
            primary: "Identifier Grove, British Columbia, Canada".to_owned(),
            city: Some("Identifier Grove".to_owned()),
            region: Some("British Columbia".to_owned()),
            country: Some("Canada".to_owned()),
            geohash5: "c28rn".to_owned(),
        },
        geonames_feature_id: Some(3004),
        geonames_country_id: Some("CA".to_owned()),
        updated_at_ms: 1_700_000_123_000,
    };
    let private_receipt_json =
        serde_json::to_value(&private_receipt).expect("private receipt json");
    assert_struct_serialize_error_paths(&private_receipt, 9);
    assert_struct_serialize_error_paths(&private_receipt.exact_location, 2);
    assert_struct_serialize_error_paths(&private_receipt.public_locality, 5);
    assert_eq!(private_receipt_json["updated_at_ms"], 1_700_000_123_000_i64);
    let listing_location = private_receipt.public_locality.to_listing_public_location();
    assert_eq!(
        listing_location.primary,
        "Identifier Grove, British Columbia, Canada"
    );
    assert_eq!(listing_location.city.as_deref(), Some("Identifier Grove"));
    assert_eq!(listing_location.region.as_deref(), Some("British Columbia"));
    assert_eq!(listing_location.country.as_deref(), Some("Canada"));
    assert_eq!(listing_location.geohash, "c28rn");
    let farm_location = private_receipt.public_locality.to_farm_public_location();
    assert_eq!(
        farm_location.primary,
        "Identifier Grove, British Columbia, Canada"
    );
    assert_eq!(farm_location.city.as_deref(), Some("Identifier Grove"));
    assert_eq!(farm_location.region.as_deref(), Some("British Columbia"));
    assert_eq!(farm_location.country.as_deref(), Some("Canada"));
    assert_eq!(farm_location.geohash, "c28rn");
    assert_eq!(
        serde_json::from_value::<FarmPrivateLocationReceipt>(private_receipt_json)
            .expect("private receipt round trip"),
        private_receipt
    );

    let candidate = FarmPrivateLocationLookupCandidate {
        geonames_feature_id: 3002,
        geonames_country_id: "CA".to_owned(),
        name: "Shared Market".to_owned(),
        display_name: "Shared Market, British Columbia, Canada".to_owned(),
        exact_location: SdkExactLocation::new(48.7, -123.2),
        region: Some("British Columbia".to_owned()),
        country: Some("Canada".to_owned()),
    };
    assert_struct_serialize_error_paths(&candidate, 7);
    let lookup = FarmPrivateLocationLookupReceipt {
        farm_addr: farm_addr(&farmer_actor(), FARM_F_D_TAG),
        farm_pubkey: FARMER.to_owned(),
        farm_d_tag: FARM_F_D_TAG.to_owned(),
        input: FarmPrivateLocationInput::query("Shared Market"),
        candidates: vec![candidate],
    };
    let lookup_json = serde_json::to_value(&lookup).expect("lookup receipt json");
    assert_struct_serialize_error_paths(&lookup, 5);
    assert_eq!(lookup_json["candidates"][0]["geonames_feature_id"], 3002);
    assert_eq!(
        serde_json::from_value::<FarmPrivateLocationLookupReceipt>(lookup_json)
            .expect("lookup receipt round trip"),
        lookup
    );
    for result in [
        FarmPrivateLocationSetResult::Stored(private_receipt),
        FarmPrivateLocationSetResult::Ambiguous(lookup.clone()),
        FarmPrivateLocationSetResult::NoMatch(FarmPrivateLocationLookupReceipt {
            candidates: Vec::new(),
            ..lookup
        }),
    ] {
        let value = serde_json::to_value(&result).expect("location set result json");
        let round_trip = serde_json::from_value::<FarmPrivateLocationSetResult>(value)
            .expect("location set result round trip");
        assert_eq!(round_trip, result);
    }

    let receipt = sdk
        .farms()
        .enqueue_publish_with_explicit_signer(enqueue_request, &FixtureSigner::new(FARMER))
        .await
        .expect("enqueue");
    let receipt_json = serde_json::to_value(&receipt).expect("receipt json");

    assert_eq!(
        receipt_json,
        serde_json::json!({
            "farm_addr": receipt.farm_addr.as_str(),
            "expected_event_id": receipt.expected_event_id.as_str(),
            "signed_event_id": receipt.signed_event_id.as_str(),
            "local_event_seq": 1,
            "outbox_operation_id": 1,
            "outbox_event_id": 1,
            "state": "stored_and_queued",
            "idempotency_digest_prefix": receipt.idempotency_digest_prefix.as_deref()
        })
    );
}

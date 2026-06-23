use super::*;

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

#[tokio::test]
async fn farm_client_prepare_resolves_default_and_explicit_created_at() {
    let sdk = crate::RadrootsSdk::builder()
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
    let sdk = crate::RadrootsSdk::builder()
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
    let sdk = crate::RadrootsSdk::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .build()
        .await
        .expect("sdk");
    let error = sdk
        .farms()
        .enqueue_publish(
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
    let sdk = crate::RadrootsSdk::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_500))
        .build()
        .await
        .expect("sdk");
    let signer = FixtureSigner::new(FARMER);
    let actor = farmer_actor();
    let receipt = sdk
        .farms()
        .enqueue_publish(
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
        .enqueue_prepared_publish(
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

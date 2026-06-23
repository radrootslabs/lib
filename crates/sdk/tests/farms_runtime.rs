#![cfg(feature = "runtime")]

use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity,
};
use radroots_event_store::RadrootsEventStore;
use radroots_events::{
    contract::RadrootsActorRole,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent, RadrootsSignedNostrEventParts},
    farm::RadrootsFarm,
    kinds::{KIND_FARM, KIND_PROFILE},
};
use radroots_outbox::{RadrootsOutbox, RadrootsOutboxEventState};
use radroots_relay_transport::RadrootsMockRelayPublishAdapter;
use radroots_sdk::{
    FARM_PUBLISH_OPERATION_KIND, FarmEnqueuePublishRequest, FarmPreparePublishRequest,
    PushOutboxEventState, PushOutboxRelayOutcomeKind, PushOutboxRequest, RadrootsSdk,
    RadrootsSdkError, RadrootsSdkPartialLocalMutationFailure, RadrootsSdkRecoveryAction,
    RadrootsSdkTimestamp, SdkIdempotencyKey, SdkMutationState, SdkRelayTargetPolicy,
    SdkRelayTargetSet, SdkRelayUrlPolicy,
};

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

async fn directory_sdk() -> (tempfile::TempDir, RadrootsSdk) {
    directory_sdk_with_relays(&[RELAY]).await
}

async fn directory_sdk_with_relays(relays: &[&str]) -> (tempfile::TempDir, RadrootsSdk) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let mut builder = RadrootsSdk::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000));
    for relay in relays {
        builder = builder.relay_url(*relay);
    }
    let sdk = builder.build().await.expect("sdk");
    (tempdir, sdk)
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
async fn farm_enqueue_publish_stores_event_and_queues_signed_outbox_without_profile_event() {
    let (_tempdir, sdk) = directory_sdk().await;
    let request = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_B_D_TAG, "North Farm"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
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
        .enqueue_publish(request, &FixtureSigner::new(FARMER))
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
        SdkRelayTargetPolicy::UseConfiguredRelays,
    );
    let error = sdk
        .farms()
        .enqueue_publish(request, &FixtureSigner::new(OTHER))
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
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY_B, RELAY, RELAY], SdkRelayUrlPolicy::Public)
    .expect("first target relays");
    let second = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_D_D_TAG, "North Farm"),
        SdkRelayTargetPolicy::explicit(
            SdkRelayTargetSet::new([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
                .expect("second target relays"),
        ),
    );

    let first_receipt = sdk
        .farms()
        .enqueue_publish(first, &FixtureSigner::new(FARMER))
        .await
        .expect("first enqueue");
    let second_receipt = sdk
        .farms()
        .enqueue_publish(second, &FixtureSigner::new(FARMER))
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
        .relay_statuses(first_receipt.outbox_event_id)
        .await
        .expect("relay statuses")
        .into_iter()
        .map(|status| status.relay_url)
        .collect::<Vec<_>>();
    assert_eq!(relay_urls, vec![RELAY_B.to_owned(), RELAY.to_owned()]);
}

#[tokio::test]
async fn farm_enqueue_publish_pushes_queued_event_with_mock_relay_sync() {
    let (_tempdir, sdk) = directory_sdk().await;
    let enqueue_request = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_D_D_TAG, "Sync Farm"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");
    let enqueue_receipt = sdk
        .farms()
        .enqueue_publish(enqueue_request, &FixtureSigner::new(FARMER))
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
    assert_eq!(event.relays.len(), 1);
    assert_eq!(event.relays[0].relay_url, RELAY);
    assert_eq!(
        event.relays[0].outcome_kind,
        PushOutboxRelayOutcomeKind::Accepted
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
async fn farm_enqueue_publish_reports_partial_local_mutation_after_outbox_conflict() {
    let (_tempdir, sdk) = directory_sdk().await;
    let first = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_E_D_TAG, "North Farm"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_idempotency_key("farm-idem-e")
    .expect("idempotency key");
    sdk.farms()
        .enqueue_publish(first, &FixtureSigner::new(FARMER))
        .await
        .expect("first enqueue");

    let second = FarmEnqueuePublishRequest::new(
        farmer_actor(),
        farm(FARM_F_D_TAG, "Changed Farm"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_idempotency_key("farm-idem-e")
    .expect("idempotency key");
    let error = sdk
        .farms()
        .enqueue_publish(second, &FixtureSigner::new(FARMER))
        .await
        .expect_err("partial");

    assert!(matches!(
        error,
        RadrootsSdkError::PartialLocalMutation(ref partial)
            if partial.stored
                && !partial.queued
                && partial.event_id.is_some()
                && partial.operation_kind == FARM_PUBLISH_OPERATION_KIND
                && partial.idempotency_digest_prefix.is_some()
                && partial.failure == RadrootsSdkPartialLocalMutationFailure::OutboxIdempotencyConflict
                && partial.recovery == RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey
    ));
    assert!(!error.to_string().contains("farm-idem-e"));
}

#[tokio::test]
async fn farm_runtime_dtos_serialize_deterministically() {
    let (_tempdir, sdk) = directory_sdk().await;
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_123);
    let prepare_request =
        FarmPreparePublishRequest::new(farmer_actor(), farm(FARM_A_D_TAG, "Serialized Farm"))
            .with_created_at(created_at);
    let prepare_json = serde_json::to_value(&prepare_request).expect("prepare request json");

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
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
    .expect("relay targets")
    .with_idempotency_key(
        SdkIdempotencyKey::new("farm-serialized-idempotency").expect("idempotency"),
    )
    .with_created_at(created_at);
    let enqueue_json = serde_json::to_value(&enqueue_request).expect("enqueue request json");

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
            "target_relays": {
                "kind": "explicit",
                "relays": [RELAY, RELAY_B],
                "canonical_relays": [RELAY_B, RELAY]
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

    let receipt = sdk
        .farms()
        .enqueue_publish(enqueue_request, &FixtureSigner::new(FARMER))
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

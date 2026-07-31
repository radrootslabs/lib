use super::*;
#[cfg(feature = "signer-adapters")]
use crate::{RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider};
use radroots_event::contract::AuthorRole;
use radroots_event::draft::{EventDraft, SignedEvent, SignedEventParts};
use radroots_event::envelope::kind::{KIND_FARM, KIND_GEOCHAT};
use radroots_nostr::draft_signing::radroots_nostr_sign_frozen_draft;
use radroots_nostr::types::RadrootsNostrKeys;
use radroots_signing::{
    Error as SigningError, SignReceipt, SignRequest, SignerStatus, actor::ActorSource,
    error::Kind as SigningErrorKind, signer::BoxFuture,
};
use std::sync::LazyLock;

struct WorkflowKeyMaterial {
    keys: RadrootsNostrKeys,
    pubkey: String,
}

static WORKFLOW_KEY_MATERIAL: LazyLock<WorkflowKeyMaterial> = LazyLock::new(|| {
    let keys = RadrootsNostrKeys::generate();
    let pubkey = keys.public_key().to_hex();
    WorkflowKeyMaterial { keys, pubkey }
});

fn farmer_pubkey() -> &'static str {
    WORKFLOW_KEY_MATERIAL.pubkey.as_str()
}

fn workflow_idempotency_key(index: u16) -> SdkIdempotencyKey {
    SdkIdempotencyKey::new(format!("01890f0e-6c00-7000-8000-00000000{index:04x}"))
        .expect("workflow idempotency")
}

struct WorkflowSigner {
    keys: RadrootsNostrKeys,
}

impl WorkflowSigner {
    fn new() -> Self {
        Self {
            keys: WORKFLOW_KEY_MATERIAL.keys.clone(),
        }
    }
}

struct FailIfCalledSigner;

impl FailIfCalledSigner {
    fn new() -> Self {
        Self
    }
}

impl Signer for FailIfCalledSigner {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, SigningError>> {
        Box::pin(async { Ok(SignerStatus::unavailable()) })
    }

    fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, SigningError>> {
        panic!("ephemeral workflow preflight must not invoke the signer")
    }
}

struct InvalidSignatureSigner(WorkflowSigner);

impl InvalidSignatureSigner {
    fn new() -> Self {
        Self(WorkflowSigner::new())
    }
}

impl Signer for InvalidSignatureSigner {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, SigningError>> {
        Box::pin(async { Ok(SignerStatus::unavailable()) })
    }

    fn sign(&self, request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, SigningError>> {
        Box::pin(async move {
            let receipt = self.0.sign(request.clone()).await?;
            let mut wire = receipt.signed_event().wire().clone();
            wire.sig = "0".repeat(128);
            let raw_json =
                serde_json::to_string(&wire).expect("invalid-signature fixture must serialize");
            let signed_event =
                SignedEvent::from_wire_verified_id(wire, raw_json).map_err(|source| {
                    SigningError::with_source(SigningErrorKind::InternalError, source)
                })?;
            SignReceipt::from_signed_event(&request, signed_event, 1_700_000_001)
        })
    }
}

impl Signer for WorkflowSigner {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, SigningError>> {
        Box::pin(async { Ok(SignerStatus::unavailable()) })
    }

    fn sign(&self, request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, SigningError>> {
        Box::pin(async move {
            let signed_event = radroots_nostr_sign_frozen_draft(&self.keys, request.draft())
                .map_err(|source| {
                    SigningError::with_source(SigningErrorKind::InternalError, source)
                })?;
            SignReceipt::from_signed_event(&request, signed_event, 1_700_000_001)
        })
    }
}

fn frozen_draft_for(pubkey: &str) -> EventDraft {
    frozen_draft_for_d_tag(pubkey, "test")
}

fn frozen_draft_for_d_tag(pubkey: &str, d_tag: &str) -> EventDraft {
    EventDraft::new(
        "radroots.farm.profile.v1",
        KIND_FARM,
        1_700_000_000,
        vec![vec!["d".to_owned(), d_tag.to_owned()]],
        "{}",
        pubkey,
    )
    .expect("frozen draft")
}

fn frozen_draft() -> EventDraft {
    frozen_draft_for("a".repeat(64).as_str())
}

fn ephemeral_draft_for(pubkey: &str) -> EventDraft {
    EventDraft::new(
        "radroots.social.geochat.v1",
        KIND_GEOCHAT,
        1_700_000_000,
        Vec::new(),
        "Transient local message",
        pubkey,
    )
    .expect("ephemeral draft")
}

fn signed_event() -> SignedEvent {
    let draft = frozen_draft();
    let sig = "c".repeat(128);
    let raw_json = serde_json::json!({
        "id": draft.expected_event_id_hex(),
        "pubkey": draft.expected_pubkey().to_hex(),
        "created_at": draft.created_at_u64(),
        "kind": draft.kind_u32(),
        "tags": draft.tags_as_vec(),
        "content": draft.content(),
        "sig": sig,
    })
    .to_string();
    SignedEvent::new(SignedEventParts {
        id: draft.expected_event_id_hex().to_owned(),
        pubkey: draft.expected_pubkey().to_hex().to_owned(),
        created_at: draft.created_at_u64(),
        kind: draft.kind_u32(),
        tags: draft.tags_as_vec(),
        content: draft.content().to_owned(),
        sig,
        raw_json,
    })
    .expect("signed event")
}

fn nostr_profile(relay: &'static str) -> crate::TransportProfile {
    crate::TransportProfile::nostr(
        crate::NostrProfile::new([relay], crate::NostrRelayUrlPolicy::Public)
            .expect("Nostr profile"),
    )
}

fn workflow_delivery_plan() -> radroots_outbox::RadrootsOutboxDeliveryPlanInput {
    let target_set = TargetSet::nostr_relays(
        ["wss://relay.example.com"],
        crate::NostrRelayUrlPolicy::Public,
    )
    .expect("target set");
    radroots_outbox::RadrootsOutboxDeliveryPlanInput::new(
        "explicit",
        1,
        radroots_transport::RadrootsTransportSatisfactionPolicy::all_accepted(),
        target_set.into_targets(),
    )
}

#[test]
fn workflow_digest_and_event_helpers_cover_error_and_input_paths() {
    assert_eq!(digest_prefix("abcdef1234567890"), "abcdef123456");
    assert_eq!(
        parse_event_id("b".repeat(64).as_str(), "event id").expect("event id"),
        EventId::parse("b".repeat(64)).expect("event id")
    );
    assert!(matches!(
        parse_event_id("not-an-event-id", "signed event id"),
        Err(RadrootsSdkError::InvalidRequest { message })
            if message.contains("signed event id is invalid")
    ));

    let draft = frozen_draft();

    let signed = signed_event();
    let event = signed.envelope();
    assert_eq!(event.id(), signed.id());
    assert_eq!(event.author(), signed.pubkey());

    let idempotency_key =
        SdkIdempotencyKey::new("01890f0e-6c00-7000-8000-000000000237").expect("idempotency");
    let input = signed_outbox_input(
        "farm.publish.v1",
        &draft,
        signed_event(),
        workflow_delivery_plan(),
        idempotency_key,
        true,
        1_700_000_000_000,
    );
    assert_eq!(input.operation_kind, "farm.publish.v1");
    assert_eq!(
        input.delivery_plan.targets[0].uri().as_str(),
        "wss://relay.example.com"
    );
    assert!(input.event_store_inserted);
    assert_eq!(
        durable_event_persistence(
            &draft.expected_event_id_hex(),
            &RadrootsEventPersistence::Inserted { seq: 7 },
        )
        .expect("inserted persistence"),
        (true, 7)
    );
    assert_eq!(
        durable_event_persistence(
            &draft.expected_event_id_hex(),
            &RadrootsEventPersistence::Duplicate { seq: 7 },
        )
        .expect("duplicate persistence"),
        (false, 7)
    );
    assert!(matches!(
        durable_event_persistence(
            &draft.expected_event_id_hex(),
            &RadrootsEventPersistence::NotPersisted,
        ),
        Err(RadrootsSdkError::InvalidRequest { message })
            if message.contains("requires durable local event-store persistence")
    ));
    let frozen = frozen_draft_json(&draft).expect("frozen draft json");
    assert!(frozen.contains("\"expected_event_id\""));
    let receipt = SdkWorkflowEnqueueReceipt {
        signed_event_id: EventId::parse(draft.expected_event_id_hex()).expect("event id"),
        local_event_seq: 1,
        outbox_operation_id: 2,
        outbox_event_id: 3,
        state: radroots_outbox::RadrootsOutboxEnqueueStatus::Inserted,
        idempotency_digest_prefix: "abcdef123456".to_owned(),
    };
    let receipt_json = workflow_receipt_result_json(&receipt);
    let decoded =
        workflow_receipt_from_result_json(receipt_json.to_string().as_str()).expect("receipt");
    assert_eq!(decoded.outbox_event_id, receipt.outbox_event_id);
    assert_eq!(outbox_enqueue_status_str(decoded.state), "inserted");
}

#[tokio::test]
async fn enqueue_signed_workflow_rejects_ephemeral_event_before_durable_commit() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(crate::RadrootsSdkTimestamp::from_unix_seconds(
            1_700_000_013,
        ))
        .build()
        .await
        .expect("sdk");
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let draft = ephemeral_draft_for(farmer_pubkey());
    let error = enqueue_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind: "workflow.ephemeral.test.v1",
            actor: &actor,
            frozen_draft: &draft,
            target_policy: TargetPolicy::LocalOnly,
            satisfaction_policy: SatisfactionPolicy::NoWait,
            idempotency_key: Some(workflow_idempotency_key(0x247)),
        },
        &FailIfCalledSigner::new(),
    )
    .await
    .expect_err("ephemeral workflow");

    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { ref message }
            if message.contains("cannot enqueue ephemeral event kind")
    ));
    assert!(!error.retryable());
    assert_eq!(
        sdk._event_store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
    assert_eq!(
        sdk._outbox
            .status_summary(0)
            .await
            .expect("outbox status")
            .total_events,
        0
    );
    let journal_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sdk_runtime_operation_journal WHERE operation_kind = ?",
    )
    .bind("workflow.ephemeral.test.v1")
    .fetch_one(sdk._event_store.pool())
    .await
    .expect("journal count");
    assert_eq!(journal_count, 0);
}

#[tokio::test]
async fn enqueue_signed_workflow_rejects_invalid_signer_signature_without_storage_mutation() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(crate::RadrootsSdkTimestamp::from_unix_seconds(
            1_700_000_013,
        ))
        .build()
        .await
        .expect("sdk");
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let draft = frozen_draft_for_d_tag(farmer_pubkey(), "workflow-invalid-signature");
    let operation_kind = "farm.publish.v1";

    let error = enqueue_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind,
            actor: &actor,
            frozen_draft: &draft,
            target_policy: TargetPolicy::LocalOnly,
            satisfaction_policy: SatisfactionPolicy::NoWait,
            idempotency_key: Some(workflow_idempotency_key(0x248)),
        },
        &InvalidSignatureSigner::new(),
    )
    .await
    .expect_err("invalid signer signature");

    assert!(matches!(
        error,
        RadrootsSdkError::SignerReturnedEventDrift {
            ref operation,
            ref reason,
        } if operation == operation_kind
            && reason.contains("failed NIP-01 verification")
    ));
    assert!(!error.retryable());
    assert_eq!(
        error.recovery_actions(),
        vec![crate::RadrootsSdkRecoveryAction::ConfigureSigner]
    );
    assert_eq!(
        sdk._event_store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
    assert_eq!(
        sdk._outbox
            .status_summary(0)
            .await
            .expect("outbox status")
            .total_events,
        0
    );
    let journal_state: String = sqlx::query_scalar(
        "SELECT state FROM sdk_runtime_operation_journal WHERE operation_kind = ?",
    )
    .bind(operation_kind)
    .fetch_one(sdk._event_store.pool())
    .await
    .expect("journal state");
    assert_eq!(journal_state, "rejected");
}

#[tokio::test]
async fn workflow_idempotency_replays_original_receipt_and_conflicts_on_new_command_hash() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(crate::RadrootsSdkTimestamp::from_unix_seconds(
            1_700_000_012,
        ))
        .build()
        .await
        .expect("sdk");
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let signer = WorkflowSigner::new();
    let draft = frozen_draft_for_d_tag(farmer_pubkey(), "workflow-target-policy");
    let first_target_policy = TargetPolicy::try_nostr_relays(
        ["wss://relay-a.example.com"],
        crate::NostrRelayUrlPolicy::Public,
    )
    .expect("first target policy");
    let second_target_policy = TargetPolicy::try_nostr_relays(
        ["wss://relay-b.example.com"],
        crate::NostrRelayUrlPolicy::Public,
    )
    .expect("second target policy");

    let first = enqueue_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind: "farm.publish.v1",
            actor: &actor,
            frozen_draft: &draft,
            target_policy: first_target_policy.clone(),
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: Some(workflow_idempotency_key(0x240)),
        },
        &signer,
    )
    .await
    .expect("first enqueue");
    let replay = enqueue_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind: "farm.publish.v1",
            actor: &actor,
            frozen_draft: &draft,
            target_policy: first_target_policy.clone(),
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: Some(workflow_idempotency_key(0x240)),
        },
        &signer,
    )
    .await
    .expect("replay enqueue");
    let conflict = enqueue_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind: "farm.publish.v1",
            actor: &actor,
            frozen_draft: &draft,
            target_policy: second_target_policy,
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: Some(workflow_idempotency_key(0x240)),
        },
        &signer,
    )
    .await
    .expect_err("conflicting command hash");

    assert_eq!(
        first.state,
        radroots_outbox::RadrootsOutboxEnqueueStatus::Inserted
    );
    assert_eq!(
        replay.state,
        radroots_outbox::RadrootsOutboxEnqueueStatus::Inserted
    );
    assert_eq!(first.outbox_operation_id, replay.outbox_operation_id);
    assert_eq!(first.outbox_event_id, replay.outbox_event_id);
    assert_eq!(
        first.idempotency_digest_prefix,
        replay.idempotency_digest_prefix
    );
    assert!(matches!(
        conflict,
        RadrootsSdkError::IdempotencyConflict { .. }
    ));
    let plans = sdk
        ._outbox
        .delivery_plans(first.outbox_event_id)
        .await
        .expect("delivery plans");
    assert_eq!(plans.len(), 1);
    let targets = sdk
        ._outbox
        .delivery_targets(first.outbox_event_id)
        .await
        .expect("delivery targets");
    let target_uris = targets
        .iter()
        .map(|target| target.endpoint_uri.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        target_uris,
        std::collections::BTreeSet::from(["wss://relay-a.example.com"])
    );
    let recovery_count: i64 = sqlx::query(
        "SELECT COUNT(*) FROM sdk_runtime_recovery_receipt WHERE recovery_code = 'idempotency_conflict'",
    )
    .fetch_one(sdk._event_store.pool())
    .await
    .expect("recovery count")
    .try_get(0)
    .expect("recovery count value");
    assert_eq!(recovery_count, 1);
}

#[tokio::test]
async fn enqueue_signed_workflow_maps_no_wait_directly_and_allows_local_only_profile() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(crate::RadrootsSdkTimestamp::from_unix_seconds(
            1_700_000_013,
        ))
        .build()
        .await
        .expect("sdk");
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let signer = WorkflowSigner::new();
    let draft = frozen_draft_for_d_tag(farmer_pubkey(), "workflow-no-wait");

    let receipt = enqueue_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind: "farm.publish.v1",
            actor: &actor,
            frozen_draft: &draft,
            target_policy: TargetPolicy::default_profile(),
            satisfaction_policy: SatisfactionPolicy::NoWait,
            idempotency_key: Some(workflow_idempotency_key(0x241)),
        },
        &signer,
    )
    .await
    .expect("no-wait enqueue");

    let event = sdk
        ._outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    let plans = sdk
        ._outbox
        .delivery_plans(receipt.outbox_event_id)
        .await
        .expect("plans");
    let targets = sdk
        ._outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");

    assert_eq!(
        event.state,
        radroots_outbox::RadrootsOutboxEventState::Published
    );
    assert_eq!(plans.len(), 1);
    assert_eq!(
        plans[0].satisfaction_policy,
        radroots_transport::RadrootsTransportSatisfactionPolicy::no_wait()
    );
    assert_ne!(
        plans[0].satisfaction_policy,
        radroots_transport::RadrootsTransportSatisfactionPolicy::all_accepted()
    );
    assert_eq!(plans[0].required_success_count, 0);
    assert_eq!(
        plans[0].status,
        radroots_outbox::RadrootsOutboxDeliveryPlanStatus::Complete
    );
    assert!(targets.is_empty());
    assert!(
        sdk._outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .is_none()
    );
}

#[tokio::test]
async fn enqueue_signed_workflow_rejects_missing_explicit_idempotency_key_without_mutation() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(crate::RadrootsSdkTimestamp::from_unix_seconds(
            1_700_000_013,
        ))
        .build()
        .await
        .expect("sdk");
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let draft = frozen_draft_for_d_tag(farmer_pubkey(), "workflow-missing-idempotency");

    let error = match enqueue_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind: "farm.publish.v1",
            actor: &actor,
            frozen_draft: &draft,
            target_policy: TargetPolicy::default_profile(),
            satisfaction_policy: SatisfactionPolicy::NoWait,
            idempotency_key: None,
        },
        &WorkflowSigner::new(),
    )
    .await
    {
        Err(error) => error,
        Ok(_) => panic!("expected missing idempotency error"),
    };

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert_eq!(
        sdk._event_store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
    assert_eq!(
        sdk._outbox
            .status_summary(0)
            .await
            .expect("outbox status")
            .total_events,
        0
    );
}

#[tokio::test]
async fn enqueue_signed_workflow_stores_signed_event_and_reports_idempotency_conflicts() {
    let sdk = crate::RadrootsClient::builder()
        .transport_profile(nostr_profile("wss://relay.example.com"))
        .fixed_clock(crate::RadrootsSdkTimestamp::from_unix_seconds(
            1_700_000_010,
        ))
        .build()
        .await
        .expect("sdk");
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let signer = WorkflowSigner::new();
    let first_draft = frozen_draft_for_d_tag(farmer_pubkey(), "workflow-success");
    let idempotency_key =
        SdkIdempotencyKey::new("01890f0e-6c00-7000-8000-000000000237").expect("idempotency");
    let receipt = enqueue_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind: "farm.publish.v1",
            actor: &actor,
            frozen_draft: &first_draft,
            target_policy: TargetPolicy::default_profile(),
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: Some(idempotency_key.clone()),
        },
        &signer,
    )
    .await
    .expect("enqueue signed workflow");

    assert_eq!(
        receipt.signed_event_id.to_hex(),
        first_draft.expected_event_id_hex()
    );
    assert!(receipt.local_event_seq > 0);
    assert!(receipt.outbox_operation_id > 0);
    assert!(receipt.outbox_event_id > 0);
    assert_eq!(receipt.idempotency_digest_prefix.len(), 12);
    assert_eq!(
        sdk._event_store
            .status_summary()
            .await
            .expect("event store summary")
            .total_events,
        1
    );
    assert_eq!(
        sdk._outbox
            .status_summary(i64::MAX)
            .await
            .expect("outbox summary")
            .total_events,
        1
    );

    let second_draft = frozen_draft_for_d_tag(farmer_pubkey(), "workflow-conflict");
    let error = match enqueue_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind: "farm.publish.v1",
            actor: &actor,
            frozen_draft: &second_draft,
            target_policy: TargetPolicy::default_profile(),
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: Some(idempotency_key),
        },
        &signer,
    )
    .await
    {
        Err(error) => error,
        Ok(_) => panic!("expected idempotency conflict"),
    };

    assert!(matches!(
        error,
        RadrootsSdkError::IdempotencyConflict {
            operation_kind,
            ..
        } if operation_kind == "farm.publish.v1"
    ));
    assert_eq!(
        sdk._event_store
            .status_summary()
            .await
            .expect("event store summary")
            .total_events,
        1
    );
    assert_eq!(
        sdk._outbox
            .status_summary(i64::MAX)
            .await
            .expect("outbox summary")
            .total_events,
        1
    );
}

#[cfg(feature = "signer-adapters")]
#[tokio::test]
async fn enqueue_configured_signed_workflow_uses_sdk_signer_provider() {
    let sdk = crate::RadrootsClient::builder()
        .transport_profile(nostr_profile("wss://relay.example.com"))
        .fixed_clock(crate::RadrootsSdkTimestamp::from_unix_seconds(
            1_700_000_011,
        ))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(
            RadrootsSdkLocalKeySigner::from_signer(WorkflowSigner::new(), farmer_pubkey())
                .expect("local signer"),
        ))
        .build()
        .await
        .expect("sdk");
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let draft = frozen_draft_for_d_tag(farmer_pubkey(), "workflow-configured");

    let receipt = enqueue_configured_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind: "farm.publish.v1",
            actor: &actor,
            frozen_draft: &draft,
            target_policy: TargetPolicy::default_profile(),
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: Some(workflow_idempotency_key(0x242)),
        },
    )
    .await
    .expect("configured enqueue");

    assert_eq!(
        receipt.signed_event_id.to_hex(),
        draft.expected_event_id_hex()
    );
    assert_eq!(receipt.idempotency_digest_prefix.len(), 12);
}

#[tokio::test]
async fn enqueue_signed_workflow_reports_runtime_pool_failure_before_mutation() {
    let sdk = crate::RadrootsClient::builder()
        .transport_profile(nostr_profile("wss://relay.example.com"))
        .build()
        .await
        .expect("sdk");
    assert_eq!(
        sdk._event_store
            .status_summary()
            .await
            .expect("event store summary")
            .total_events,
        0
    );
    sdk._outbox.pool().close().await;
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let draft = frozen_draft_for(farmer_pubkey());
    let request = SdkWorkflowEnqueueRequest {
        operation_kind: "farm.publish.v1",
        actor: &actor,
        frozen_draft: &draft,
        target_policy: TargetPolicy::default_profile(),
        satisfaction_policy: SatisfactionPolicy::AllAccepted,
        idempotency_key: Some(workflow_idempotency_key(0x243)),
    };

    let error = match enqueue_signed_workflow(&sdk, request, &WorkflowSigner::new()).await {
        Err(error) => error,
        Ok(_) => panic!("expected closed outbox error"),
    };

    assert!(matches!(error, RadrootsSdkError::EventStore { .. }));
}

#[tokio::test]
async fn enqueue_signed_workflow_reports_store_failures() {
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let draft = frozen_draft_for(farmer_pubkey());
    let closed_store_sdk = crate::RadrootsClient::builder()
        .transport_profile(nostr_profile("wss://relay.example.com"))
        .build()
        .await
        .expect("sdk");
    closed_store_sdk._event_store.pool().close().await;
    let store_failure_request = SdkWorkflowEnqueueRequest {
        operation_kind: "farm.publish.v1",
        actor: &actor,
        frozen_draft: &draft,
        target_policy: TargetPolicy::default_profile(),
        satisfaction_policy: SatisfactionPolicy::AllAccepted,
        idempotency_key: Some(workflow_idempotency_key(0x244)),
    };
    assert!(matches!(
        enqueue_signed_workflow(
            &closed_store_sdk,
            store_failure_request,
            &WorkflowSigner::new()
        )
        .await,
        Err(RadrootsSdkError::EventStore { .. })
    ));
}

#[tokio::test]
async fn enqueue_signed_workflow_reports_clock_failures() {
    let sdk = crate::RadrootsClient::builder()
        .clock(crate::RadrootsSdkClock::BeforeUnixEpoch)
        .transport_profile(nostr_profile("wss://relay.example.com"))
        .build()
        .await
        .expect("sdk");
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let draft = frozen_draft_for(farmer_pubkey());
    let request = SdkWorkflowEnqueueRequest {
        operation_kind: "farm.publish.v1",
        actor: &actor,
        frozen_draft: &draft,
        target_policy: TargetPolicy::default_profile(),
        satisfaction_policy: SatisfactionPolicy::AllAccepted,
        idempotency_key: Some(workflow_idempotency_key(0x245)),
    };
    assert!(matches!(
        enqueue_signed_workflow(&sdk, request, &WorkflowSigner::new()).await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
}

#[tokio::test]
async fn enqueue_signed_workflow_rejects_transport_profile_targets_without_radrootsd_execution() {
    let sdk = crate::RadrootsClient::builder().build().await.expect("sdk");
    let actor = Actor::from_public_key_hex(
        farmer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor");
    let draft = frozen_draft_for(farmer_pubkey());
    let request = SdkWorkflowEnqueueRequest {
        operation_kind: "farm.publish.v1",
        actor: &actor,
        frozen_draft: &draft,
        target_policy: TargetPolicy::DefaultProfile,
        satisfaction_policy: SatisfactionPolicy::AllAccepted,
        idempotency_key: Some(workflow_idempotency_key(0x246)),
    };

    assert!(matches!(
        enqueue_signed_workflow(&sdk, request, &WorkflowSigner::new()).await,
        Err(RadrootsSdkError::EmptyTransportTargets { operation })
            if operation == "publish transport profile"
    ));
}

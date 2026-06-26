use super::*;
use radroots_authority::{RadrootsSignerError, RadrootsSignerIdentity};
use radroots_events::contract::RadrootsActorRole;
use radroots_events::draft::RadrootsSignedNostrEvent;
use radroots_events::kinds::KIND_FARM;
use radroots_events_codec::wire::{WireEventParts, to_frozen_draft};
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, radroots_nostr_sign_frozen_draft,
};

const FARMER_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const FARMER_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

struct WorkflowSigner {
    identity: RadrootsSignerIdentity,
    keys: RadrootsNostrKeys,
}

impl WorkflowSigner {
    fn new() -> Self {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FARMER_SECRET_KEY_HEX).expect("secret key");
        let keys = RadrootsNostrKeys::new(secret_key);
        Self {
            identity: RadrootsSignerIdentity::new(FARMER_PUBLIC_KEY_HEX).expect("identity"),
            keys,
        }
    }
}

impl RadrootsEventSigner for WorkflowSigner {
    fn pubkey(&self) -> &radroots_events::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsFrozenEventDraft,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
        radroots_nostr_sign_frozen_draft(&self.keys, draft).map_err(|error| {
            RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            }
        })
    }
}

fn frozen_draft_for(pubkey: &str) -> RadrootsFrozenEventDraft {
    to_frozen_draft(
        WireEventParts {
            kind: KIND_FARM,
            content: "{}".to_owned(),
            tags: vec![vec!["d".to_owned(), "test".to_owned()]],
        },
        "radroots.farm.profile.v1",
        pubkey,
        1_700_000_000,
    )
    .expect("frozen draft")
}

fn frozen_draft() -> RadrootsFrozenEventDraft {
    frozen_draft_for("a".repeat(64).as_str())
}

fn signed_event() -> RadrootsSignedNostrEvent {
    RadrootsSignedNostrEvent {
        id: "b".repeat(64),
        pubkey: "a".repeat(64),
        created_at: 1_700_000_000,
        kind: 1,
        tags: vec![vec!["d".to_owned(), "test".to_owned()]],
        content: "{}".to_owned(),
        sig: "c".repeat(128),
        raw_json: "{}".to_owned(),
    }
}

#[test]
fn workflow_digest_and_event_helpers_cover_error_and_input_paths() {
    assert_eq!(digest_prefix("abcdef1234567890"), "abcdef123456");
    assert_eq!(
        parse_event_id("b".repeat(64).as_str(), "event id").expect("event id"),
        RadrootsEventId::parse("b".repeat(64)).expect("event id")
    );
    assert!(matches!(
        parse_event_id("not-an-event-id", "signed event id"),
        Err(RadrootsSdkError::InvalidRequest { message })
            if message.contains("signed event id is invalid")
    ));

    let draft = frozen_draft();
    let digest = outbox_idempotency_digest_prefix(
        "workflow.test.v1",
        &draft,
        &["wss://relay.example.com".to_owned()],
    );
    assert_eq!(digest.len(), 12);

    let signed = signed_event();
    let event = event_from_signed(&signed);
    assert_eq!(event.id, signed.id);
    assert_eq!(event.author, signed.pubkey);

    let idempotency_key = SdkIdempotencyKey::new("workflow-idempotency").expect("idempotency");
    let input = signed_outbox_input(
        "workflow.test.v1",
        &draft,
        signed_event(),
        vec!["wss://relay.example.com".to_owned()],
        idempotency_key,
        false,
        true,
        1_700_000_000_000,
    );
    assert_eq!(input.operation_kind, "workflow.test.v1");
    assert_eq!(
        input.target_relays,
        vec!["wss://relay.example.com".to_owned()]
    );
    assert!(input.event_store_inserted);
}

#[tokio::test]
async fn enqueue_signed_workflow_reports_partial_mutation_when_outbox_fails() {
    let sdk = crate::RadrootsClient::builder()
        .relay_url("wss://relay.example.com")
        .build()
        .await
        .expect("sdk");
    sdk._outbox.pool().close().await;
    let actor = RadrootsActorContext::test(FARMER_PUBLIC_KEY_HEX, [RadrootsActorRole::Farmer])
        .expect("actor");
    let draft = frozen_draft_for(FARMER_PUBLIC_KEY_HEX);
    let request = SdkWorkflowEnqueueRequest {
        operation_kind: "workflow.test.v1",
        actor: &actor,
        frozen_draft: &draft,
        target_relays: SdkRelayTargetPolicy::UseConfiguredRelays,
        idempotency_key: None,
    };

    let error = match enqueue_signed_workflow(&sdk, request, &WorkflowSigner::new()).await {
        Err(error) => error,
        Ok(_) => panic!("expected closed outbox error"),
    };

    match error {
        RadrootsSdkError::PartialLocalMutation(partial) => {
            assert!(partial.stored);
            assert!(!partial.queued);
            assert_eq!(partial.operation_kind, "workflow.test.v1");
            assert_eq!(
                partial.failure,
                crate::RadrootsSdkPartialLocalMutationFailure::OutboxEnqueue
            );
        }
        other => panic!("unexpected workflow error: {other:?}"),
    }
}

#[tokio::test]
async fn enqueue_signed_workflow_reports_store_failures() {
    let actor = RadrootsActorContext::test(FARMER_PUBLIC_KEY_HEX, [RadrootsActorRole::Farmer])
        .expect("actor");
    let draft = frozen_draft_for(FARMER_PUBLIC_KEY_HEX);
    let closed_store_sdk = crate::RadrootsClient::builder()
        .relay_url("wss://relay.example.com")
        .build()
        .await
        .expect("sdk");
    closed_store_sdk._event_store.pool().close().await;
    let store_failure_request = SdkWorkflowEnqueueRequest {
        operation_kind: "workflow.test.v1",
        actor: &actor,
        frozen_draft: &draft,
        target_relays: SdkRelayTargetPolicy::UseConfiguredRelays,
        idempotency_key: None,
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
        .relay_url("wss://relay.example.com")
        .build()
        .await
        .expect("sdk");
    let actor = RadrootsActorContext::test(FARMER_PUBLIC_KEY_HEX, [RadrootsActorRole::Farmer])
        .expect("actor");
    let draft = frozen_draft_for(FARMER_PUBLIC_KEY_HEX);
    let request = SdkWorkflowEnqueueRequest {
        operation_kind: "workflow.test.v1",
        actor: &actor,
        frozen_draft: &draft,
        target_relays: SdkRelayTargetPolicy::UseConfiguredRelays,
        idempotency_key: None,
    };
    assert!(matches!(
        enqueue_signed_workflow(&sdk, request, &WorkflowSigner::new()).await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
}

#[tokio::test]
async fn enqueue_signed_workflow_rejects_publish_transport_targets_without_proxy_transport() {
    let sdk = crate::RadrootsClient::builder().build().await.expect("sdk");
    let actor = RadrootsActorContext::test(FARMER_PUBLIC_KEY_HEX, [RadrootsActorRole::Farmer])
        .expect("actor");
    let draft = frozen_draft_for(FARMER_PUBLIC_KEY_HEX);
    let request = SdkWorkflowEnqueueRequest {
        operation_kind: "workflow.test.v1",
        actor: &actor,
        frozen_draft: &draft,
        target_relays: SdkRelayTargetPolicy::UsePublishTransport,
        idempotency_key: None,
    };

    assert!(matches!(
        enqueue_signed_workflow(&sdk, request, &WorkflowSigner::new()).await,
        Err(RadrootsSdkError::EmptyTargetRelays { operation })
            if operation == "publish transport relay resolution"
    ));
}

use super::{
    RadrootsSdkError, RadrootsSdkPartialLocalMutationError, RadrootsSdkPartialLocalMutationFailure,
    RadrootsSdkRecoveryAction, redacted_relay_url,
};
use radroots_authority::RadrootsAuthorityError;
use radroots_events::contract::RadrootsActorRole;

#[test]
fn partial_local_mutation_constructor_preserves_supplied_error() {
    let error = RadrootsSdkPartialLocalMutationError {
        event_id: None,
        operation_kind: "listing.publish.v1".to_owned(),
        idempotency_digest_prefix: None,
        stored: true,
        queued: false,
        recovery: RadrootsSdkRecoveryAction::RetryOutboxEnqueue,
        failure: RadrootsSdkPartialLocalMutationFailure::OutboxEnqueue,
    };

    assert!(matches!(
        RadrootsSdkError::partial_local_mutation(error),
        RadrootsSdkError::PartialLocalMutation(RadrootsSdkPartialLocalMutationError {
            stored: true,
            queued: false,
            recovery: RadrootsSdkRecoveryAction::RetryOutboxEnqueue,
            failure: RadrootsSdkPartialLocalMutationFailure::OutboxEnqueue,
            ..
        })
    ));

    assert!(matches!(
        RadrootsSdkError::partial_outbox_enqueue_mutation(
            "a".repeat(64),
            "listing.publish.v1",
            "digest-prefix",
        ),
        RadrootsSdkError::PartialLocalMutation(RadrootsSdkPartialLocalMutationError {
            event_id: Some(_),
            operation_kind,
            idempotency_digest_prefix: Some(_),
            stored: true,
            queued: false,
            recovery: RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey,
            failure: RadrootsSdkPartialLocalMutationFailure::OutboxEnqueue,
        }) if operation_kind == "listing.publish.v1"
    ));

    assert!(matches!(
        RadrootsSdkError::partial_outbox_idempotency_conflict_mutation(
            "a".repeat(64),
            "listing.publish.v1",
            "digest-prefix",
        ),
        RadrootsSdkError::PartialLocalMutation(RadrootsSdkPartialLocalMutationError {
            event_id: Some(_),
            operation_kind,
            idempotency_digest_prefix: Some(_),
            stored: true,
            queued: false,
            recovery: RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey,
            failure: RadrootsSdkPartialLocalMutationFailure::OutboxIdempotencyConflict,
        }) if operation_kind == "listing.publish.v1"
    ));
}

#[test]
fn authority_error_conversion_redacts_pubkey_mismatches_and_falls_back() {
    let actor_error = RadrootsSdkError::from(RadrootsAuthorityError::ActorPubkeyMismatch {
        expected_pubkey: "a".repeat(64),
        actor_pubkey: "b".repeat(64),
    });
    assert!(matches!(
        actor_error,
        RadrootsSdkError::UnauthorizedActor { ref reason, .. }
            if reason == "actor_pubkey_prefix=bbbbbbbbbbbb expected_pubkey_prefix=aaaaaaaaaaaa"
    ));

    let fallback = RadrootsSdkError::from(RadrootsAuthorityError::UnknownContract {
        contract_id: "contract-x".to_owned(),
    });
    assert!(matches!(
        fallback,
        RadrootsSdkError::Authority { ref message } if message.contains("contract-x")
    ));
}

#[test]
fn listing_and_store_errors_convert_to_sdk_error_classes() {
    let draft = RadrootsSdkError::from(
        radroots_trade::listing::RadrootsListingDraftError::ActorRoleUnsatisfied {
            required_role: RadrootsActorRole::Seller,
        },
    );
    assert!(matches!(
        draft,
        RadrootsSdkError::UnauthorizedActor { ref operation, ref reason }
            if operation == "listing.prepare_publish" && reason == "missing role Seller"
    ));

    let draft_fallback = RadrootsSdkError::from(
        radroots_trade::listing::RadrootsListingDraftError::InvalidFarmPubkey(
            radroots_events::ids::RadrootsIdParseError::InvalidCharacter,
        ),
    );
    assert!(matches!(
        draft_fallback,
        RadrootsSdkError::ListingDraft { ref message }
            if message.contains("invalid listing draft farm pubkey")
    ));

    let mutation = RadrootsSdkError::from(
        radroots_trade::listing::RadrootsListingMutationError::UnsupportedMutation,
    );
    assert!(matches!(
        mutation,
        RadrootsSdkError::ListingMutation { ref message }
            if message == "listing mutation is not supported"
    ));

    let store = RadrootsSdkError::from(
        radroots_event_store::RadrootsEventStoreError::MissingEvent("event-a".to_owned()),
    );
    assert!(matches!(
        store,
        RadrootsSdkError::EventStore { ref message } if message.contains("event-a")
    ));
}

#[test]
fn outbox_error_conversion_handles_empty_targets_and_fallbacks() {
    assert!(matches!(
        RadrootsSdkError::from(radroots_outbox::RadrootsOutboxError::EmptyTargetRelays),
        RadrootsSdkError::EmptyTargetRelays { ref operation } if operation == "outbox enqueue"
    ));

    assert!(matches!(
        RadrootsSdkError::from(radroots_outbox::RadrootsOutboxError::EventNotFound(42)),
        RadrootsSdkError::Outbox { ref message } if message.contains("42")
    ));
}

#[test]
fn relay_transport_error_conversion_redacts_and_classifies_url_errors() {
    let unsupported = RadrootsSdkError::from(
        radroots_relay_transport::RadrootsRelayTransportError::UnsupportedRelayScheme {
            url: "ftp://user:secret@relay.example.com/path?token=secret".to_owned(),
            scheme: "ftp".to_owned(),
        },
    );
    assert!(matches!(
        unsupported,
        RadrootsSdkError::InvalidRelayUrl { ref url, ref reason }
            if url == "ftp://<redacted>@relay.example.com/path?<redacted>"
                && reason == "unsupported scheme `ftp`"
    ));

    assert!(matches!(
        RadrootsSdkError::from(
            radroots_relay_transport::RadrootsRelayTransportError::EmptyRelayHost {
                url: "wss://".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref reason, .. }
            if reason == "relay URL must include a host"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_relay_transport::RadrootsRelayTransportError::RelayUrlQueryOrFragment {
                url: "wss://relay.example.com?token=secret".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref url, .. }
            if url == "wss://relay.example.com?<redacted>"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_relay_transport::RadrootsRelayTransportError::EmptyTargetSet
        ),
        RadrootsSdkError::EmptyTargetRelays { ref operation } if operation == "relay publish"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_relay_transport::RadrootsRelayTransportError::RelayUrlParse {
                url: "wss://user:secret@relay.example.com/path?token=secret".to_owned(),
                reason: "bad relay URL".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref url, ref reason }
            if url == "wss://<redacted>@relay.example.com/path?<redacted>"
                && reason == "bad relay URL"
    ));
    assert!(matches!(
        RadrootsSdkError::from(radroots_relay_transport::RadrootsRelayTransportError::Outbox(
            radroots_outbox::RadrootsOutboxError::EmptyTargetRelays,
        )),
        RadrootsSdkError::EmptyTargetRelays { ref operation } if operation == "outbox enqueue"
    ));
    assert!(matches!(
        RadrootsSdkError::from(radroots_relay_transport::RadrootsRelayTransportError::Transport(
            "offline".to_owned(),
        )),
        RadrootsSdkError::RelayTransport { ref message } if message == "Relay transport error: offline"
    ));
}

#[test]
fn relay_url_redaction_handles_plain_values_and_userinfo() {
    assert_eq!(redacted_relay_url("not-a-url".to_owned()), "not-a-url");
    assert_eq!(
        redacted_relay_url("not-a-url?token=secret".to_owned()),
        "not-a-url?<redacted>"
    );
    assert_eq!(
        redacted_relay_url("not-a-url#fragment".to_owned()),
        "not-a-url#<redacted>"
    );
    assert_eq!(
        redacted_relay_url("wss://relay.example.com/path?token=secret".to_owned()),
        "wss://relay.example.com/path?<redacted>"
    );
    assert_eq!(
        redacted_relay_url("wss://user:secret@relay.example.com/path#frag".to_owned()),
        "wss://<redacted>@relay.example.com/path#<redacted>"
    );
    assert_eq!(
        redacted_relay_url("wss://relay.example.com/path#fragment".to_owned()),
        "wss://relay.example.com/path#<redacted>"
    );
    assert_eq!(
        redacted_relay_url("wss://relay.example.com/path".to_owned()),
        "wss://relay.example.com/path"
    );
}

#[test]
fn sdk_error_contract_methods_cover_representative_classes_and_details() {
    let errors = vec![
        RadrootsSdkError::Io {
            path: "store.sqlite".into(),
            message: "readonly".to_owned(),
        },
        RadrootsSdkError::ClockBeforeUnixEpoch,
        RadrootsSdkError::TimestampOutOfRange { value: u64::MAX },
        RadrootsSdkError::UnauthorizedActor {
            operation: "listing.publish".to_owned(),
            reason: "missing seller".to_owned(),
        },
        RadrootsSdkError::SignerPubkeyMismatch {
            operation: "listing.publish".to_owned(),
            expected_pubkey_prefix: "aaaaaaaaaaaa".to_owned(),
            signer_pubkey_prefix: "bbbbbbbbbbbb".to_owned(),
        },
        RadrootsSdkError::EmptyTargetRelays {
            operation: "relay publish".to_owned(),
        },
        RadrootsSdkError::RelayTargetLimitExceeded { max: 2, actual: 3 },
        RadrootsSdkError::invalid_relay_url(
            "wss://user:secret@relay.example.com/path?token=secret",
            "userinfo",
        ),
        RadrootsSdkError::IdempotencyConflict {
            operation_kind: "listing.publish.v1".to_owned(),
            expected_pubkey_prefix: "aaaaaaaaaaaa".to_owned(),
            existing_digest_prefix: "existing".to_owned(),
            new_digest_prefix: "new".to_owned(),
        },
        RadrootsSdkError::order_status_limit_invalid(0, 1, 100),
        RadrootsSdkError::invalid_order_id("bad order", "bad id"),
        RadrootsSdkError::ProductSyncUnsupported {
            operation: "sync.push_outbox",
            required_feature: "relay-runtime",
        },
        RadrootsSdkError::ProductSyncRelaySetupFailure {
            message: "offline".to_owned(),
        },
        RadrootsSdkError::Authority {
            message: "authority".to_owned(),
        },
        RadrootsSdkError::EventStore {
            message: "event store".to_owned(),
        },
        RadrootsSdkError::InvalidRequest {
            message: "invalid".to_owned(),
        },
        RadrootsSdkError::ListingDraft {
            message: "draft".to_owned(),
        },
        RadrootsSdkError::ListingMutation {
            message: "mutation".to_owned(),
        },
        RadrootsSdkError::Outbox {
            message: "outbox".to_owned(),
        },
        RadrootsSdkError::RelayTransport {
            message: "transport".to_owned(),
        },
        RadrootsSdkError::Projection {
            message: "projection".to_owned(),
        },
        RadrootsSdkError::partial_outbox_idempotency_conflict_mutation(
            "a".repeat(64),
            "listing.publish.v1",
            "digest-prefix",
        ),
    ];

    for error in errors {
        let detail = error.detail_json();
        assert_eq!(detail["code"], error.code());
        assert_eq!(
            detail["class"],
            serde_json::to_value(error.class()).expect("class json")
        );
        assert_eq!(detail["retryable"], error.retryable());
        assert_eq!(
            detail["recovery_actions"],
            serde_json::to_value(error.recovery_actions()).expect("recovery json")
        );
        assert!(error.to_string().starts_with("sdk "));
    }
}

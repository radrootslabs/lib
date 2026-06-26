use super::{
    RadrootsSdkError, RadrootsSdkGeoNamesErrorKind, RadrootsSdkPartialLocalMutationError,
    RadrootsSdkPartialLocalMutationFailure, RadrootsSdkRecoveryAction, redacted_relay_url,
};
use radroots_authority::RadrootsAuthorityError;
use radroots_events::contract::RadrootsActorRole;
use radroots_geocoder::GeocoderError;

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
    assert!(matches!(
        RadrootsSdkError::from(radroots_outbox::RadrootsOutboxError::IdempotencyConflict {
            operation_kind: "listing.publish.v1".to_owned(),
            expected_pubkey: "a".repeat(64),
            idempotency_key: "idem-1".to_owned(),
            existing_digest: "b".repeat(64),
            new_digest: "c".repeat(64),
        }),
        RadrootsSdkError::IdempotencyConflict {
            ref operation_kind,
            ref expected_pubkey_prefix,
            ref existing_digest_prefix,
            ref new_digest_prefix,
        } if operation_kind == "listing.publish.v1"
            && expected_pubkey_prefix == "aaaaaaaaaaaa"
            && existing_digest_prefix == "bbbbbbbbbbbb"
            && new_digest_prefix == "cccccccccccc"
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
            radroots_relay_transport::RadrootsRelayTransportError::RelayUrlUserinfo {
                url: "wss://user:secret@relay.example.com".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref url, ref reason }
            if url == "wss://<redacted>@relay.example.com"
                && reason == "relay URL must not include userinfo"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_relay_transport::RadrootsRelayTransportError::WsRequiresLocalhostPolicy {
                url: "ws://relay.example.com".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref reason, .. }
            if reason == "ws relay URL requires localhost policy"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_relay_transport::RadrootsRelayTransportError::RelayUrlForbiddenDestination {
                url: "ws://127.0.0.1:9000".to_owned(),
                reason: "localhost disabled".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref reason, .. }
            if reason == "localhost disabled"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_relay_transport::RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination {
                url: "ws://relay.example.com".to_owned(),
                address: "127.0.0.1".to_owned(),
                reason: "loopback disabled".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref reason, .. }
            if reason == "relay URL resolved to forbidden address `127.0.0.1`: loopback disabled"
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
        RadrootsSdkError::SignerUnavailable {
            mode: "configured".to_owned(),
            reason: "missing".to_owned(),
        },
        RadrootsSdkError::SignerRequestRejected {
            mode: "myc_nip46".to_owned(),
            reason: "denied".to_owned(),
        },
        RadrootsSdkError::SignerRequestTimedOut {
            mode: "myc_nip46".to_owned(),
        },
        RadrootsSdkError::SignerAuthChallengePending {
            mode: "myc_nip46".to_owned(),
            auth_url: Some("https://auth.example.com/challenge".to_owned()),
        },
        RadrootsSdkError::SignerAuthChallengePending {
            mode: "myc_nip46".to_owned(),
            auth_url: None,
        },
        RadrootsSdkError::SignerTransport {
            mode: "myc_nip46".to_owned(),
            reason: "offline".to_owned(),
        },
        RadrootsSdkError::SignerProtocol {
            mode: "myc_nip46".to_owned(),
            reason: "bad envelope".to_owned(),
        },
        RadrootsSdkError::SignerReturnedEventDrift {
            operation: "listing.publish".to_owned(),
            reason: "id changed".to_owned(),
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
        RadrootsSdkError::PrivateStore {
            message: "private".to_owned(),
        },
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Configuration,
            message: "missing cache root".to_owned(),
        },
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Download,
            message: "download".to_owned(),
        },
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Cache,
            message: "cache".to_owned(),
        },
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Integrity,
            message: "integrity".to_owned(),
        },
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Schema,
            message: "schema".to_owned(),
        },
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Lookup,
            message: "lookup".to_owned(),
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

#[test]
fn geonames_error_conversion_maps_source_errors_to_sdk_kinds() {
    let path = std::path::PathBuf::from("geonames-test.db");
    let cases = vec![
        (
            GeocoderError::InvalidAssetUrl {
                url: "http://assets.radroots.io/geonames.db".to_owned(),
            },
            RadrootsSdkGeoNamesErrorKind::Configuration,
        ),
        (
            GeocoderError::InvalidAssetHost {
                url: "https://example.com/geonames.db".to_owned(),
                expected_host: "assets.radroots.io".to_owned(),
                actual_host: "example.com".to_owned(),
            },
            RadrootsSdkGeoNamesErrorKind::Configuration,
        ),
        (
            GeocoderError::InvalidAssetLength {
                path: path.clone(),
                expected: 4,
                actual: 3,
            },
            RadrootsSdkGeoNamesErrorKind::Integrity,
        ),
        (
            GeocoderError::InvalidAssetSha256 {
                path: path.clone(),
                expected: "a".repeat(64),
                actual: "b".repeat(64),
            },
            RadrootsSdkGeoNamesErrorKind::Integrity,
        ),
        (
            GeocoderError::InvalidAssetSqlite {
                path: path.clone(),
                detail: "file is not a database".to_owned(),
            },
            RadrootsSdkGeoNamesErrorKind::Integrity,
        ),
        (
            GeocoderError::InvalidAssetIntegrity {
                path: path.clone(),
                result: "row mismatch".to_owned(),
            },
            RadrootsSdkGeoNamesErrorKind::Integrity,
        ),
        (
            GeocoderError::InvalidAssetSchema {
                path: path.clone(),
                detail: "missing table".to_owned(),
            },
            RadrootsSdkGeoNamesErrorKind::Schema,
        ),
        (
            GeocoderError::AssetLockUnavailable { path: path.clone() },
            RadrootsSdkGeoNamesErrorKind::Cache,
        ),
        (
            GeocoderError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing")),
            RadrootsSdkGeoNamesErrorKind::Cache,
        ),
        (
            GeocoderError::CountryCenterNotFound {
                country_id: "XX".to_owned(),
            },
            RadrootsSdkGeoNamesErrorKind::Lookup,
        ),
    ];

    for (source, expected_kind) in cases {
        let error = RadrootsSdkError::from(source);

        assert!(matches!(
            error,
            RadrootsSdkError::GeoNames { kind, .. } if kind == expected_kind
        ));
    }
}

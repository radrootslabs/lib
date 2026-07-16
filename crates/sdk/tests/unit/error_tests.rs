use super::{
    RadrootsSdkError, RadrootsSdkGeoNamesErrorKind, RadrootsSdkTradeErrorKind,
    radroots_sdk_error_catalog, redacted_relay_url,
};
use crate::privacy::{PrivacyPreflightStatus, ProductSensitivityField};
use crate::transport::ReticulumBehavior;
use radroots_authority::RadrootsAuthorityError;
use radroots_event::contract::RadrootsActorRole;
use radroots_geocoder::{GeoNamesAssetFetcher, GeoNamesBlockingHttpFetcher, GeocoderError};
use std::collections::BTreeSet;

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
        radroots_trade::listing::RadrootsListingEditError::ActorRoleUnsatisfied {
            required_role: RadrootsActorRole::Seller,
        },
    );
    assert!(matches!(
        draft,
        RadrootsSdkError::UnauthorizedActor { ref operation, ref reason }
            if operation == "listing.prepare_publish" && reason == "missing role Seller"
    ));

    let draft_fallback = RadrootsSdkError::from(
        radroots_trade::listing::RadrootsListingEditError::InvalidFarmPubkey(
            radroots_event::ids::RadrootsIdParseError::InvalidCharacter,
        ),
    );
    assert!(matches!(
        draft_fallback,
        RadrootsSdkError::ListingEdit { ref message }
            if message.contains("invalid listing edit farm pubkey")
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
        RadrootsSdkError::from(radroots_outbox::RadrootsOutboxError::EmptyDeliveryTargets),
        RadrootsSdkError::EmptyTransportTargets { ref operation } if operation == "outbox enqueue"
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
        radroots_transport_nostr::RadrootsRelayTransportError::UnsupportedRelayScheme {
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
            radroots_transport_nostr::RadrootsRelayTransportError::EmptyRelayHost {
                url: "wss://".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref reason, .. }
            if reason == "relay URL must include a host"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_transport_nostr::RadrootsRelayTransportError::RelayUrlQueryOrFragment {
                url: "wss://relay.example.com?token=secret".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref url, .. }
            if url == "wss://relay.example.com?<redacted>"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_transport_nostr::RadrootsRelayTransportError::RelayUrlUserinfo {
                url: "wss://user:secret@relay.example.com".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref url, ref reason }
            if url == "wss://<redacted>@relay.example.com"
                && reason == "relay URL must not include userinfo"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_transport_nostr::RadrootsRelayTransportError::WsRequiresLocalhostPolicy {
                url: "ws://relay.example.com".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref reason, .. }
            if reason == "ws relay URL requires localhost policy"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_transport_nostr::RadrootsRelayTransportError::RelayUrlForbiddenDestination {
                url: "ws://127.0.0.1:9000".to_owned(),
                reason: "localhost disabled".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref reason, .. }
            if reason == "localhost disabled"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_transport_nostr::RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination {
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
            radroots_transport_nostr::RadrootsRelayTransportError::EmptyTargetSet
        ),
        RadrootsSdkError::EmptyTransportTargets { ref operation } if operation == "nostr relay publish"
    ));
    assert!(matches!(
        RadrootsSdkError::from(
            radroots_transport_nostr::RadrootsRelayTransportError::RelayUrlParse {
                url: "wss://user:secret@relay.example.com/path?token=secret".to_owned(),
                reason: "bad relay URL".to_owned(),
            },
        ),
        RadrootsSdkError::InvalidRelayUrl { ref url, ref reason }
            if url == "wss://<redacted>@relay.example.com/path?<redacted>"
                && reason == "bad relay URL"
    ));
    assert!(matches!(
        RadrootsSdkError::from(radroots_transport_nostr::RadrootsRelayTransportError::Outbox(
            radroots_outbox::RadrootsOutboxError::EmptyDeliveryTargets,
        )),
        RadrootsSdkError::EmptyTransportTargets { ref operation } if operation == "outbox enqueue"
    ));
    assert!(matches!(
        RadrootsSdkError::from(radroots_transport_nostr::RadrootsRelayTransportError::Transport(
            "offline".to_owned(),
        )),
        RadrootsSdkError::Transport { ref message } if message == "Relay transport error: offline"
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
        RadrootsSdkError::EmptyTransportTargets {
            operation: "nostr relay publish".to_owned(),
        },
        RadrootsSdkError::TransportTargetLimitExceeded { max: 2, actual: 3 },
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
        RadrootsSdkError::Trade {
            kind: RadrootsSdkTradeErrorKind::QueryLimitInvalid,
            operation: "trade.list".to_owned(),
            message: "limit out of range".to_owned(),
        },
        RadrootsSdkError::Trade {
            kind: RadrootsSdkTradeErrorKind::PrivateArtifactMissing,
            operation: "trade.submit_proposal".to_owned(),
            message: "private artifact missing".to_owned(),
        },
        RadrootsSdkError::PrivacyPreflight {
            operation: "trade.cancel".to_owned(),
            status: PrivacyPreflightStatus::ExplicitConfirmationRequired,
            fields: vec![ProductSensitivityField::PublicButSensitiveNotes],
        },
        RadrootsSdkError::ProductSyncUnsupported {
            operation: "sync.push_outbox",
            required_feature: "transport-nostr-runtime",
        },
        RadrootsSdkError::ReticulumTransportUnavailable {
            operation: "sync.push_outbox".to_owned(),
            endpoint_uri: "reticulum:local".to_owned(),
            behavior: ReticulumBehavior::RejectDeliveryAttempts,
        },
        RadrootsSdkError::ReticulumTransportUnavailable {
            operation: "sync.push_outbox".to_owned(),
            endpoint_uri: "reticulum:local".to_owned(),
            behavior: ReticulumBehavior::DeferDeliveryPlans,
        },
        RadrootsSdkError::ProductSyncTransportSetupFailure {
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
        RadrootsSdkError::ListingEdit {
            message: "edit".to_owned(),
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
        RadrootsSdkError::Transport {
            message: "transport".to_owned(),
        },
        RadrootsSdkError::Projection {
            message: "projection".to_owned(),
        },
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
fn sdk_error_catalog_exposes_stable_codes_and_metadata() {
    let catalog = radroots_sdk_error_catalog();
    let codes = catalog
        .iter()
        .map(|entry| entry.code)
        .collect::<BTreeSet<_>>();

    assert_eq!(codes.len(), catalog.len());
    assert!(codes.contains("io"));
    assert!(codes.contains("signer_auth_challenge_pending"));
    assert!(codes.contains("product_sync_unsupported"));
    assert!(codes.contains("reticulum_transport_deferred"));
    assert!(codes.contains("unsupported_profile_schema"));
    assert!(codes.contains("geonames_lookup"));

    for trade_kind in [
        RadrootsSdkTradeErrorKind::InvalidEnvelope,
        RadrootsSdkTradeErrorKind::InvalidCommandBody,
        RadrootsSdkTradeErrorKind::PrivateArtifactMissing,
        RadrootsSdkTradeErrorKind::PrivateArtifactCommitmentMismatch,
        RadrootsSdkTradeErrorKind::PrivateArtifactAcknowledgementMissing,
        RadrootsSdkTradeErrorKind::TradeNotFound,
        RadrootsSdkTradeErrorKind::QueryLimitInvalid,
        RadrootsSdkTradeErrorKind::CursorInvalid,
    ] {
        assert!(codes.contains(trade_kind.code()));
    }

    for entry in catalog {
        assert!(!entry.code.is_empty());
        assert!(!entry.recovery_actions.is_empty());
    }

    let timeout = RadrootsSdkError::SignerRequestTimedOut {
        mode: "myc_nip46".to_owned(),
    };
    let timeout_entry = catalog
        .iter()
        .find(|entry| entry.code == timeout.code())
        .expect("timeout catalog entry");
    assert_eq!(timeout_entry.class, timeout.class());
    assert_eq!(timeout_entry.retryable, timeout.retryable());
    let timeout_recovery_actions = timeout.recovery_actions();
    assert_eq!(
        timeout_entry.recovery_actions,
        timeout_recovery_actions.as_slice()
    );

    let trade = RadrootsSdkError::Trade {
        kind: RadrootsSdkTradeErrorKind::PrivateArtifactCommitmentMismatch,
        operation: "trade.decide_candidate".to_owned(),
        message: "private artifact commitment mismatch".to_owned(),
    };
    let trade_entry = catalog
        .iter()
        .find(|entry| entry.code == trade.code())
        .expect("trade catalog entry");
    assert_eq!(trade_entry.class, trade.class());
    assert_eq!(trade_entry.retryable, trade.retryable());
    let trade_recovery_actions = trade.recovery_actions();
    assert_eq!(
        trade_entry.recovery_actions,
        trade_recovery_actions.as_slice()
    );
}

#[test]
fn geonames_error_conversion_maps_source_errors_to_sdk_kinds() {
    let path = std::path::PathBuf::from("geonames-test.db");
    let download_error = GeoNamesBlockingHttpFetcher
        .fetch("not-a-url")
        .expect_err("invalid URL download error");
    assert!(matches!(
        RadrootsSdkError::from(download_error),
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Download,
            ..
        }
    ));

    let cases = vec![
        (
            GeocoderError::InvalidAssetUrl {
                url: "http://assets.radroots.io/geonames-1.0.db".to_owned(),
            },
            RadrootsSdkGeoNamesErrorKind::Configuration,
        ),
        (
            GeocoderError::InvalidAssetHost {
                url: "https://example.com/geonames-1.0.db".to_owned(),
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
            GeocoderError::SqliteConnectionLockUnavailable,
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

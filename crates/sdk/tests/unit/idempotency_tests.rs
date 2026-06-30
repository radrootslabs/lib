use super::{SdkIdempotencyKey, SdkTradeIdempotencyRecord};
use crate::RadrootsSdkError;
use radroots_events::ids::{RadrootsEventId, RadrootsPublicKey};

#[path = "../support/serializer_failure.rs"]
mod serializer_failure;

use serializer_failure::assert_struct_serialize_error_paths;

#[test]
fn empty_key_is_rejected_before_redacted_storage() {
    assert!(matches!(
        SdkIdempotencyKey::new(""),
        Err(RadrootsSdkError::InvalidRequest { ref message })
            if message == "idempotency key must not be empty"
    ));
    assert!(matches!(
        SdkIdempotencyKey::new(" key"),
        Err(RadrootsSdkError::InvalidRequest { ref message })
            if message == "idempotency key must not include boundary whitespace"
    ));
    assert!(matches!(
        SdkIdempotencyKey::new("key\nvalue"),
        Err(RadrootsSdkError::InvalidRequest { ref message })
            if message == "idempotency key must not contain control characters"
    ));
    assert!(matches!(
        SdkIdempotencyKey::new("k".repeat(super::SDK_IDEMPOTENCY_KEY_MAX_LEN + 1)),
        Err(RadrootsSdkError::InvalidRequest { ref message })
            if message.contains("idempotency key must be at most")
    ));
}

#[test]
fn derived_key_is_deterministic_and_consumable() {
    let relays = vec![
        "wss://relay-b.example.com".to_owned(),
        "wss://relay-a.example.com".to_owned(),
    ];
    let first = SdkIdempotencyKey::derive("listing.publish.v1", "event-a", "pubkey-a", &relays);
    let second = SdkIdempotencyKey::derive("listing.publish.v1", "event-a", "pubkey-a", &relays);

    assert_eq!(first.as_str(), second.as_str());
    assert!(first.into_string().starts_with("listing.publish.v1:"));
}

#[test]
fn idempotency_key_reports_serializer_failures() {
    let key = SdkIdempotencyKey::new("idempotent").expect("key");

    assert_struct_serialize_error_paths(&key, 2);
}

#[test]
fn trade_idempotency_record_binds_payload_and_reports_conflicts() {
    let record = SdkTradeIdempotencyRecord {
        idempotency_key: SdkIdempotencyKey::new("trade-idempotent").expect("key"),
        operation_kind: "trade.submit.v1".to_owned(),
        actor_pubkey: RadrootsPublicKey::parse(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .expect("actor pubkey"),
        digest: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
        canonical_payload_hash: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
            .to_owned(),
        expected_event_id: RadrootsEventId::parse(
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        )
        .expect("event id"),
        outbox_operation_id: 42,
    };

    assert!(
        record.matches_payload("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc")
    );
    assert!(
        !record.matches_payload("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
    );
    assert!(matches!(
        record.conflict_error("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
        RadrootsSdkError::IdempotencyConflict {
            ref operation_kind,
            ref expected_pubkey_prefix,
            ref existing_digest_prefix,
            ref new_digest_prefix,
        } if operation_kind == "trade.submit.v1"
            && expected_pubkey_prefix == "aaaaaaaaaaaa"
            && existing_digest_prefix == "bbbbbbbbbbbb"
            && new_digest_prefix == "ffffffffffff"
    ));
}

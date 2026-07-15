use super::{SdkIdempotencyKey, SdkTradeIdempotencyRecord};
use crate::RadrootsSdkError;
use radroots_event::ids::{RadrootsEventId, RadrootsPublicKey};

use crate::serializer_failure::assert_struct_serialize_error_paths;

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
fn explicit_uuid_v7_key_is_accepted_and_other_shapes_are_rejected() {
    let key = SdkIdempotencyKey::new("01890f0e-6c00-7000-8000-000000000001").expect("key");

    assert_eq!(key.as_str(), "01890f0e-6c00-7000-8000-000000000001");
    assert_eq!(key.into_string(), "01890f0e-6c00-7000-8000-000000000001");
    assert!(matches!(
        SdkIdempotencyKey::new("01890f0e-6c00-6000-8000-000000000001"),
        Err(RadrootsSdkError::InvalidRequest { ref message })
            if message == "idempotency key must be a UUIDv7"
    ));
}

#[test]
fn idempotency_key_reports_serializer_failures() {
    let key = SdkIdempotencyKey::new("01890f0e-6c00-7000-8000-000000000002").expect("key");

    assert_struct_serialize_error_paths(&key, 2);
}

#[test]
fn trade_idempotency_record_binds_payload_and_reports_conflicts() {
    let record = SdkTradeIdempotencyRecord {
        idempotency_key: SdkIdempotencyKey::new("01890f0e-6c00-7000-8000-000000000003")
            .expect("key"),
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

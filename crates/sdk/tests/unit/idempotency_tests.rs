use super::SdkIdempotencyKey;
use crate::RadrootsSdkError;

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

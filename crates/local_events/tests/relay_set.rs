use radroots_local_events::{
    CANONICAL_RELAY_SET_FINGERPRINT_VERSION, canonical_relay_set_fingerprint,
};

#[test]
fn relay_set_fingerprint_trims_sorts_and_dedupes() {
    let first = canonical_relay_set_fingerprint([
        " wss://relay-b.example ",
        "wss://relay-a.example",
        "wss://relay-b.example",
    ])
    .expect("fingerprint");
    let second =
        canonical_relay_set_fingerprint(["wss://relay-a.example", "wss://relay-b.example"])
            .expect("fingerprint");

    assert_eq!(first, second);
    assert!(first.starts_with(CANONICAL_RELAY_SET_FINGERPRINT_VERSION));
}

#[test]
fn relay_set_fingerprint_rejects_empty_entries() {
    let fingerprint = canonical_relay_set_fingerprint([" ", "", "\t"]);

    assert_eq!(fingerprint, None);
}

#[test]
fn relay_set_fingerprint_changes_when_relay_set_changes() {
    let first = canonical_relay_set_fingerprint(["wss://relay-a.example"]).expect("fingerprint");
    let second =
        canonical_relay_set_fingerprint(["wss://relay-a.example", "wss://relay-b.example"])
            .expect("fingerprint");

    assert_ne!(first, second);
}

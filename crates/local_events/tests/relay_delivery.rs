use radroots_local_events::{
    RelayDeliveryEvidence, RelayDeliveryFailure, RelayDeliveryState,
    canonical_relay_set_fingerprint,
};
use serde_json::json;

#[test]
fn pending_delivery_evidence_uses_canonical_json_shape() {
    let evidence = RelayDeliveryEvidence::pending([
        " wss://relay-b.example ",
        "wss://relay-a.example",
        "wss://relay-b.example",
    ])
    .expect("pending evidence");

    assert_eq!(evidence.state, RelayDeliveryState::Pending);
    assert_eq!(
        evidence.target_relays,
        vec![
            "wss://relay-b.example".to_owned(),
            "wss://relay-a.example".to_owned()
        ]
    );
    assert_eq!(
        evidence.to_json_value().expect("json"),
        json!({
            "state": "pending",
            "target_relays": ["wss://relay-b.example", "wss://relay-a.example"],
            "connected_relays": [],
            "acknowledged_relays": [],
            "failed_relays": []
        })
    );
}

#[test]
fn acknowledged_delivery_evidence_uses_canonical_failure_fields() {
    let evidence = RelayDeliveryEvidence::acknowledged(
        ["wss://relay-a.example", "wss://relay-b.example"],
        [" wss://relay-a.example "],
        ["wss://relay-a.example"],
        vec![RelayDeliveryFailure::new(" wss://relay-b.example ", " timeout ").expect("failure")],
    )
    .expect("acknowledged evidence");

    assert_eq!(
        evidence.to_json_value().expect("json"),
        json!({
            "state": "acknowledged",
            "target_relays": ["wss://relay-a.example", "wss://relay-b.example"],
            "connected_relays": ["wss://relay-a.example"],
            "acknowledged_relays": ["wss://relay-a.example"],
            "failed_relays": [
                {"relay_url": "wss://relay-b.example", "error": "timeout"}
            ]
        })
    );
}

#[test]
fn failed_delivery_evidence_requires_failures_without_acknowledgements() {
    let evidence = RelayDeliveryEvidence::failed(
        ["wss://relay-a.example"],
        ["wss://relay-a.example"],
        vec![RelayDeliveryFailure::new("wss://relay-a.example", "closed").expect("failure")],
    )
    .expect("failed evidence");

    assert_eq!(evidence.state, RelayDeliveryState::Failed);
    assert!(evidence.acknowledged_relays.is_empty());
    assert_eq!(evidence.failed_relays.len(), 1);
}

#[test]
fn delivery_evidence_fingerprint_uses_target_relays() {
    let evidence = RelayDeliveryEvidence::acknowledged(
        ["wss://relay-b.example", "wss://relay-a.example"],
        ["wss://relay-a.example"],
        ["wss://relay-a.example"],
        Vec::new(),
    )
    .expect("evidence");

    assert_eq!(
        evidence.relay_set_fingerprint(),
        canonical_relay_set_fingerprint(["wss://relay-a.example", "wss://relay-b.example"])
    );
}

#[test]
fn delivery_evidence_rejects_invalid_json_shape() {
    let err = RelayDeliveryEvidence::from_json_value(&json!({
        "state": "acknowledged",
        "target_relays": ["wss://relay-a.example"],
        "connected_relays": [],
        "acknowledged_relays": [],
        "failed_relays": []
    }))
    .expect_err("invalid evidence");

    assert!(err.to_string().contains("acknowledged_relays"));
}

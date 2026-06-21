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
    assert_eq!(evidence.state.as_str(), "pending");
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

    assert_eq!(evidence.state.as_str(), "acknowledged");
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
fn observed_delivery_evidence_tracks_observed_relays_without_acknowledgement() {
    let evidence = RelayDeliveryEvidence::observed(
        ["wss://relay-a.example", "wss://relay-b.example"],
        [" wss://relay-a.example ", "wss://relay-b.example"],
        ["wss://relay-b.example"],
        Vec::new(),
    )
    .expect("observed evidence");

    assert_eq!(evidence.state, RelayDeliveryState::Observed);
    assert_eq!(evidence.state.as_str(), "observed");
    assert!(evidence.acknowledged_relays.is_empty());
    assert_eq!(
        evidence.to_json_value().expect("json"),
        json!({
            "state": "observed",
            "target_relays": ["wss://relay-a.example", "wss://relay-b.example"],
            "connected_relays": ["wss://relay-a.example", "wss://relay-b.example"],
            "acknowledged_relays": [],
            "observed_relays": ["wss://relay-b.example"],
            "failed_relays": []
        })
    );
}

#[test]
fn observed_delivery_evidence_allows_unknown_exact_relay_when_connected() {
    let evidence = RelayDeliveryEvidence::observed(
        ["wss://relay-a.example", "wss://relay-b.example"],
        ["wss://relay-a.example", "wss://relay-b.example"],
        Vec::<String>::new(),
        Vec::new(),
    )
    .expect("observed evidence");

    assert_eq!(evidence.state, RelayDeliveryState::Observed);
    assert!(evidence.observed_relays.is_empty());
    assert_eq!(
        evidence.to_json_value().expect("json"),
        json!({
            "state": "observed",
            "target_relays": ["wss://relay-a.example", "wss://relay-b.example"],
            "connected_relays": ["wss://relay-a.example", "wss://relay-b.example"],
            "acknowledged_relays": [],
            "failed_relays": []
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
    assert_eq!(evidence.state.as_str(), "failed");
    assert!(evidence.acknowledged_relays.is_empty());
    assert_eq!(evidence.failed_relays.len(), 1);
}

#[test]
fn acknowledged_delivery_evidence_rejects_observed_relays() {
    let err = RelayDeliveryEvidence::from_json_value(&json!({
        "state": "acknowledged",
        "target_relays": ["wss://relay-a.example"],
        "connected_relays": ["wss://relay-a.example"],
        "acknowledged_relays": ["wss://relay-a.example"],
        "observed_relays": ["wss://relay-a.example"],
        "failed_relays": []
    }))
    .expect_err("invalid evidence");

    assert!(err.to_string().contains("observed_relays"));
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

#[test]
fn delivery_evidence_rejects_pending_and_failed_cross_state_fields() {
    let pending_err = RelayDeliveryEvidence::from_json_value(&json!({
        "state": "pending",
        "target_relays": ["wss://relay-a.example"],
        "connected_relays": [],
        "acknowledged_relays": [],
        "observed_relays": ["wss://relay-a.example"],
        "failed_relays": [
            {"relay_url": "wss://relay-a.example", "error": "timeout"}
        ]
    }))
    .expect_err("pending evidence with terminal fields");

    assert!(
        pending_err
            .to_string()
            .contains("pending delivery evidence")
    );

    let failed_err = RelayDeliveryEvidence::from_json_value(&json!({
        "state": "failed",
        "target_relays": ["wss://relay-a.example"],
        "connected_relays": [],
        "acknowledged_relays": ["wss://relay-a.example"],
        "observed_relays": ["wss://relay-a.example"],
        "failed_relays": [
            {"relay_url": "wss://relay-a.example", "error": "timeout"}
        ]
    }))
    .expect_err("failed evidence with success fields");

    assert!(failed_err.to_string().contains("failed delivery evidence"));
}

#[test]
fn delivery_evidence_rejects_invalid_failure_and_relay_values() {
    let normalized_failure_err = RelayDeliveryEvidence::from_json_value(&json!({
        "state": "failed",
        "target_relays": ["wss://relay-a.example"],
        "connected_relays": [],
        "acknowledged_relays": [],
        "failed_relays": [
            {"relay_url": " wss://relay-a.example ", "error": "timeout"}
        ]
    }))
    .expect_err("non-normalized failure relay");

    assert!(
        normalized_failure_err
            .to_string()
            .contains("failed_relays.relay_url")
    );

    let trimmed_error_err = RelayDeliveryEvidence::from_json_value(&json!({
        "state": "failed",
        "target_relays": ["wss://relay-a.example"],
        "connected_relays": [],
        "acknowledged_relays": [],
        "failed_relays": [
            {"relay_url": "wss://relay-a.example", "error": " timeout "}
        ]
    }))
    .expect_err("non-normalized failure text");

    assert!(trimmed_error_err.to_string().contains("must be trimmed"));

    let constructor_err =
        RelayDeliveryEvidence::pending(["http://relay-a.example"]).expect_err("invalid relay");

    assert!(constructor_err.to_string().contains("target_relays"));
}

#[test]
fn delivery_evidence_rejects_invalid_relay_sets_in_each_field() {
    for (field, evidence) in [
        (
            "connected_relays",
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Pending,
                target_relays: vec!["wss://relay-a.example".to_owned()],
                connected_relays: vec!["http://relay-a.example".to_owned()],
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            },
        ),
        (
            "acknowledged_relays",
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Acknowledged,
                target_relays: vec!["wss://relay-a.example".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: vec!["http://relay-a.example".to_owned()],
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            },
        ),
        (
            "observed_relays",
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Observed,
                target_relays: vec!["wss://relay-a.example".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: vec!["http://relay-a.example".to_owned()],
                failed_relays: Vec::new(),
            },
        ),
    ] {
        let error = evidence.validate().expect_err("invalid relay set");

        assert!(
            error.to_string().contains(field),
            "expected error to contain {field}, got {error}"
        );
    }
}

#[test]
fn delivery_evidence_rejects_constructor_and_json_error_paths() {
    let empty_failure_error = RelayDeliveryEvidence::failed(
        ["wss://relay-a.example"],
        Vec::<String>::new(),
        vec![RelayDeliveryFailure {
            relay_url: "wss://relay-a.example".to_owned(),
            error: " ".to_owned(),
        }],
    )
    .expect_err("empty failure error");

    assert!(
        empty_failure_error
            .to_string()
            .contains("failed_relays.error")
    );

    let invalid_json_error = RelayDeliveryEvidence::from_json_value(&json!({
        "state": 1,
        "target_relays": [],
        "connected_relays": [],
        "acknowledged_relays": [],
        "failed_relays": []
    }))
    .expect_err("invalid json");

    assert!(!invalid_json_error.to_string().is_empty());

    let to_json_error = RelayDeliveryEvidence {
        state: RelayDeliveryState::Pending,
        target_relays: Vec::new(),
        connected_relays: Vec::new(),
        acknowledged_relays: Vec::new(),
        observed_relays: Vec::new(),
        failed_relays: Vec::new(),
    }
    .to_json_value()
    .expect_err("invalid to json evidence");

    assert!(to_json_error.to_string().contains("target_relays"));

    for result in [
        RelayDeliveryEvidence::acknowledged(
            ["wss://relay-a.example"],
            ["http://relay-a.example"],
            ["wss://relay-a.example"],
            Vec::new(),
        ),
        RelayDeliveryEvidence::acknowledged(
            ["wss://relay-a.example"],
            Vec::<String>::new(),
            ["http://relay-a.example"],
            Vec::new(),
        ),
        RelayDeliveryEvidence::observed(
            ["wss://relay-a.example"],
            Vec::<String>::new(),
            ["http://relay-a.example"],
            Vec::new(),
        ),
        RelayDeliveryEvidence::acknowledged(
            ["wss://relay-a.example"],
            Vec::<String>::new(),
            Vec::<String>::new(),
            Vec::new(),
        ),
    ] {
        assert!(result.is_err());
    }
}

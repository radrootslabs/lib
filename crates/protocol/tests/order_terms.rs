#![cfg(feature = "commands")]
use radroots_protocol::command::{
    COMMAND_MAX_BYTES, CapturedCommand, CapturedInventoryCommand, CapturedOrderAcceptance,
    CapturedStockCommitment, CapturedTermsInstallation, CommandError,
};
use serde_json::{Value, json};

fn vectors() -> Value {
    serde_json::from_str(include_str!("fixtures/terms_installation.v1.json")).unwrap()
}
fn parse(value: &Value) -> Result<CapturedTermsInstallation, CommandError> {
    CapturedTermsInstallation::parse(&serde_json::to_vec(value).unwrap())
}

#[test]
fn automatic_and_explicit_capture_match_independent_ecmascript_bytes_and_digests() {
    for case in vectors()["cases"].as_array().unwrap() {
        let captured = parse(&case["wire"]).unwrap();
        assert_eq!(
            captured.protected_wire(),
            case["protected_canonical_utf8"]
                .as_str()
                .unwrap()
                .as_bytes()
        );
        assert_eq!(
            captured.digest().to_string(),
            case["command_digest"].as_str().unwrap()
        );
        assert_eq!(
            captured.approval_intent_digest().to_string(),
            case["approval_intent_digest"].as_str().unwrap()
        );
        assert_eq!(
            CapturedTermsInstallation::parse(captured.protected_wire())
                .unwrap()
                .digest(),
            captured.digest()
        );
        assert_eq!(captured.input().proposal().to_string(), "31".repeat(32));
        assert_eq!(
            captured.input().authorization().to_string(),
            "32".repeat(32)
        );
        assert_eq!(
            captured.input().terms_digest().to_string(),
            case["wire"]["input"]["terms_digest"].as_str().unwrap()
        );
        assert_eq!(
            format!("{captured:?}"),
            "CapturedTermsInstallation(<redacted>)"
        );
        assert_eq!(
            format!("{:?}", captured.input()),
            "InstallTermsInput(<redacted>)"
        );
        assert!(CapturedCommand::parse(captured.protected_wire()).is_err());
        assert!(CapturedInventoryCommand::parse(captured.protected_wire()).is_err());
        assert!(CapturedOrderAcceptance::parse(captured.protected_wire()).is_err());
    }
    let values = vectors();
    assert!(
        parse(&values["cases"][0]["wire"])
            .unwrap()
            .input()
            .allocation()
            .is_automatic()
    );
    assert_eq!(
        parse(&values["cases"][1]["wire"])
            .unwrap()
            .input()
            .allocation()
            .explicit_allocations()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn proposal_authorization_digest_base_and_allocation_each_bind_the_displayed_intent() {
    let data = vectors();
    let original = &data["cases"][0]["wire"];
    let expected = parse(original).unwrap();
    for (path, value) in [
        ("/input/proposal", json!("61".repeat(32))),
        ("/input/authorization", json!("62".repeat(32))),
        ("/input/terms_digest", json!("63".repeat(32))),
        ("/context/expected/agreement", json!("64".repeat(32))),
        ("/context/expected/seller_sequence", json!("2")),
        (
            "/input/allocation",
            data["cases"][1]["wire"]["input"]["allocation"].clone(),
        ),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = value;
        let changed = parse(&changed).unwrap();
        assert_ne!(changed.digest(), expected.digest());
        assert_ne!(
            changed.approval_intent_digest(),
            expected.approval_intent_digest()
        );
    }
    let mut correlated = original.clone();
    correlated["context"]["correlation_id"] = json!("different observer");
    let correlated = parse(&correlated).unwrap();
    assert_eq!(correlated.digest(), expected.digest());
    assert_eq!(
        correlated.approval_intent_digest(),
        expected.approval_intent_digest()
    );
    assert_ne!(correlated.protected_wire(), expected.protected_wire());
    let mut approved = original.clone();
    approved["context"]["approval"] = json!({"intent_digest":expected.approval_intent_digest(),"scope":"orders.install_terms","expires_at":"190"});
    let approved = parse(&approved).unwrap();
    assert_ne!(approved.digest(), expected.digest());
    assert_eq!(
        approved.approval_intent_digest(),
        expected.approval_intent_digest()
    );
}

#[test]
fn closed_shapes_and_seller_order_descriptors_cannot_turn_capture_into_a_fallback_command() {
    let data = vectors();
    let original = &data["cases"][0]["wire"];
    for (path, value) in [
        ("/type", json!("orders.accept")),
        ("/context/actor/role", json!("buyer")),
        ("/context/expected", Value::Null),
        ("/input/proposal", json!("2A".repeat(32))),
        ("/input/authorization", json!("2A".repeat(32))),
        ("/input/terms_digest", json!("2A".repeat(32))),
        (
            "/input/allocation",
            json!({"type":"automatic","allocations":[]}),
        ),
        (
            "/input/allocation",
            json!({"type":"explicit","allocations":[]}),
        ),
        ("/input/allocation", json!({"type":"unknown"})),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(parse(&changed).is_err(), "{path}");
    }
    for field in ["proposal", "authorization", "terms_digest", "allocation"] {
        let mut changed = original.clone();
        changed["input"].as_object_mut().unwrap().remove(field);
        assert!(parse(&changed).is_err(), "{field}");
    }
    let mut extra = original.clone();
    extra["input"]["stock_free"] = json!(true);
    assert!(parse(&extra).is_err());
    let mut extra = original.clone();
    extra["unrecognized"] = json!(true);
    assert!(parse(&extra).is_err());
}

#[test]
fn duplicate_members_and_allocation_or_frame_overflow_refuse_before_any_execution() {
    let data = vectors();
    let original = &data["cases"][0]["wire"];
    let raw = serde_json::to_string(original).unwrap();
    let member = format!("\"proposal\":\"{}\"", "31".repeat(32));
    let duplicate = raw.replacen(&member, &format!("{member},{member}"), 1);
    assert!(CapturedTermsInstallation::parse(duplicate.as_bytes()).is_err());
    let mut excess = data["cases"][1]["wire"].clone();
    let allocation = excess["input"]["allocation"]["allocations"][0].clone();
    excess["input"]["allocation"]["allocations"] = json!(vec![allocation; 65]);
    assert!(parse(&excess).is_err());
    let mut oversized = raw.into_bytes();
    oversized.resize(COMMAND_MAX_BYTES + 1, b' ');
    assert!(matches!(
        CapturedTermsInstallation::parse(&oversized),
        Err(CommandError::LimitExceeded)
    ));
}

#[test]
fn shared_commitment_dispatch_preserves_exact_bytes_and_rejects_ambiguous_roots() {
    let data = vectors();
    let acceptance: Value =
        serde_json::from_str(include_str!("fixtures/order_acceptance.v1.json")).unwrap();
    for (cases, name) in [
        (&data["cases"], "orders.install_terms"),
        (&acceptance["cases"], "orders.accept"),
    ] {
        for case in cases.as_array().unwrap() {
            let raw = serde_json::to_vec(&case["wire"]).unwrap();
            let captured = CapturedStockCommitment::parse(&raw).unwrap();
            assert_eq!(captured.command_name(), name);
            assert_eq!(
                captured.proposal().is_some(),
                name == "orders.install_terms"
            );
            assert_eq!(
                CapturedStockCommitment::parse(captured.protected_wire())
                    .unwrap()
                    .digest(),
                captured.digest()
            );
            match &captured {
                CapturedStockCommitment::Acceptance(value) => {
                    assert_eq!(value.protected_wire(), captured.protected_wire());
                    assert_eq!(value.input().request(), captured.authorization());
                    assert_eq!(
                        value.approval_intent_digest(),
                        captured.approval_intent_digest()
                    );
                }
                CapturedStockCommitment::TermsInstallation(value) => {
                    assert_eq!(value.protected_wire(), captured.protected_wire());
                    assert_eq!(value.input().authorization(), captured.authorization());
                    assert_eq!(
                        value.approval_intent_digest(),
                        captured.approval_intent_digest()
                    );
                }
            }
        }
    }
    let raw = serde_json::to_string(&data["cases"][0]["wire"]).unwrap();
    let duplicate = raw.replacen(
        "\"type\":\"orders.install_terms\"",
        "\"type\":\"orders.install_terms\",\"type\":\"orders.accept\"",
        1,
    );
    assert!(CapturedStockCommitment::parse(duplicate.as_bytes()).is_err());
    let mut unknown = data["cases"][0]["wire"].clone();
    unknown["type"] = json!("orders.handover");
    assert!(CapturedStockCommitment::parse(&serde_json::to_vec(&unknown).unwrap()).is_err());
}

#![cfg(feature = "commerce")]
use radroots_domain::{CommandDigest, OperationId, PublicKey, RecordId, Sequence, UnixSeconds};
use radroots_protocol::{
    command::CapturedOrderAcceptance,
    commerce::{self, CommerceSeal},
    nip01::{UnsignedEvent, WireError},
    nip59::{PrivateEnvelopeBudget, SIGNED_MAX_BYTES},
};
use serde_json::{Value, json};
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/commerce_signing.v1.json")).unwrap()
}
fn command(value: &Value) -> CapturedOrderAcceptance {
    CapturedOrderAcceptance::parse(value["command_wire"].as_str().unwrap().as_bytes()).unwrap()
}

#[test]
fn exact_initial_record_rumor_command_and_both_seal_intents_match_independent_crypto_vectors() {
    let value = fixture();
    let command = command(&value);
    assert_eq!(command.digest().to_string(), value["command_digest"]);
    let request = commerce::decode(value["request_wire"].as_str().unwrap().as_bytes()).unwrap();
    let rumor = commerce::initial_commitment_rumor(
        &command,
        &request,
        None,
        UnixSeconds::new(451).unwrap(),
    )
    .unwrap();
    assert_eq!(rumor.record_id().to_string(), value["record_id"]);
    assert_eq!(rumor.event_id().to_string(), value["rumor_id"]);
    assert_eq!(
        rumor.protected_wire(),
        value["rumor_wire"].as_str().unwrap()
    );
    let parsed: Value = serde_json::from_str(rumor.protected_wire()).unwrap();
    assert_eq!(parsed["content"], value["record_wire"]);
    assert_eq!(rumor.author(), command.context().actor().participant());
    assert_eq!(rumor.created_at().get(), 451);
    assert_eq!(
        *rumor.recipients(),
        [
            value["seals"][0]["recipient"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
            value["seals"][1]["recipient"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap()
        ]
    );
    for row in value["seals"].as_array().unwrap() {
        let recipient: PublicKey = row["recipient"].as_str().unwrap().parse().unwrap();
        let seal = CommerceSeal::new(
            command.context().operation_id(),
            command.digest(),
            &rumor,
            recipient,
            UnixSeconds::new(450).unwrap(),
            row["ciphertext"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(seal.record_id(), rumor.record_id());
        assert_eq!(seal.rumor_id(), rumor.event_id());
        assert_eq!(seal.operation_id(), command.context().operation_id());
        assert_eq!(seal.command_digest(), command.digest());
        let encoded = UnsignedEvent::encode(&seal.fields(), SIGNED_MAX_BYTES).unwrap();
        assert_eq!(encoded.event_id(), seal.event_id());
        let expected: Value = serde_json::from_str(row["raw"].as_str().unwrap()).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(encoded.json()).unwrap()["id"],
            expected["id"]
        );
        assert_eq!(hex(&seal.event_id()), row["event_id"]);
        assert!(row["raw"].as_str().unwrap().len() <= rumor.budget().signed_seal_max_bytes());
        assert_eq!(seal.fields().tags.len(), 0);
        assert_eq!(seal.fields().kind, 13);
        assert_eq!(format!("{seal:?}"), "CommerceSeal(<redacted>)");
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn complete_peer_self_plan_matches_independent_binary_bytes_and_refuses_changed_originals() {
    use radroots_protocol::{
        commerce::CommerceSigningPlan, intent::SigningReplay, nip59::CapturedDmRoute,
    };
    let value = fixture();
    let command = command(&value);
    let request = commerce::decode(value["request_wire"].as_str().unwrap().as_bytes()).unwrap();
    let rumor = commerce::initial_commitment_rumor(
        &command,
        &request,
        None,
        UnixSeconds::new(451).unwrap(),
    )
    .unwrap();
    let seals = || {
        value["seals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                CommerceSeal::new(
                    command.context().operation_id(),
                    command.digest(),
                    &rumor,
                    row["recipient"].as_str().unwrap().parse().unwrap(),
                    UnixSeconds::new(450).unwrap(),
                    row["ciphertext"].as_str().unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    };
    let routes = || {
        value["seals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                CapturedDmRoute::from_observed_descriptor(
                    row["recipient"].as_str().unwrap().parse().unwrap(),
                    row["route"]["event_id"].as_str().unwrap().parse().unwrap(),
                    row["route"]["raw"].as_str().unwrap(),
                    &["wss://relay.example".into()],
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    };
    for replay in [
        SigningReplay::NonReplayable,
        SigningReplay::LocalReplaySafe,
        SigningReplay::ExactReplayByRequestId,
    ] {
        let plan = CommerceSigningPlan::new(
            command.context().operation_id(),
            command.digest(),
            &rumor,
            seals(),
            routes(),
            replay,
        )
        .unwrap();
        if replay == SigningReplay::NonReplayable {
            assert_eq!(hex(plan.protected_wire()), value["plan_hex"]);
            assert_eq!(hex(plan.digest()), value["plan_digest"]);
        }
        let restored = CommerceSigningPlan::parse(
            command.context().operation_id(),
            command.digest(),
            &rumor,
            plan.protected_wire(),
        )
        .unwrap();
        assert_eq!(restored.protected_wire(), plan.protected_wire());
        assert_eq!(restored.replay(), replay);
        for end in (0..plan.protected_wire().len()).step_by(73) {
            assert!(
                CommerceSigningPlan::parse(
                    command.context().operation_id(),
                    command.digest(),
                    &rumor,
                    &plan.protected_wire()[..end]
                )
                .is_err()
            );
        }
        let changed = commerce::initial_commitment_rumor(
            &command,
            &request,
            None,
            UnixSeconds::new(452).unwrap(),
        )
        .unwrap();
        assert!(
            CommerceSigningPlan::parse(
                command.context().operation_id(),
                command.digest(),
                &changed,
                plan.protected_wire()
            )
            .is_err()
        );
        let mut extra = plan.protected_wire().to_vec();
        extra.push(0);
        assert!(
            CommerceSigningPlan::parse(
                command.context().operation_id(),
                command.digest(),
                &rumor,
                &extra
            )
            .is_err()
        );
    }
    let mut wrong = seals();
    wrong.swap(0, 1);
    assert!(
        CommerceSigningPlan::new(
            command.context().operation_id(),
            command.digest(),
            &rumor,
            wrong,
            routes(),
            SigningReplay::NonReplayable
        )
        .is_err()
    );
}

#[test]
fn initial_intent_rejects_wrong_request_terms_actor_state_deadline_and_unusable_author_head_claims()
{
    let value = fixture();
    let request = commerce::decode(value["request_wire"].as_str().unwrap().as_bytes()).unwrap();
    let original: Value = serde_json::from_str(value["command_wire"].as_str().unwrap()).unwrap();
    for (path, changed) in [
        ("/input/request", json!("fa".repeat(32))),
        ("/input/terms_digest", json!("fb".repeat(32))),
        (
            "/context/actor/participant",
            json!(request.content().header().buyer()),
        ),
        ("/context/expected/agreement", json!("fc".repeat(32))),
        ("/context/expected/seller_sequence", json!("1")),
        ("/context/expected/buyer_head", Value::Null),
        ("/context/not_after", json!("451")),
    ] {
        let mut wire = original.clone();
        *wire.pointer_mut(path).unwrap() = changed;
        let command = CapturedOrderAcceptance::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
        assert!(
            commerce::initial_commitment_rumor(
                &command,
                &request,
                None,
                UnixSeconds::new(451).unwrap()
            )
            .is_err(),
            "{path}"
        );
    }
    let command = command(&value);
    for at in [0, 449, 600] {
        assert!(
            commerce::initial_commitment_rumor(
                &command,
                &request,
                None,
                UnixSeconds::new(at).unwrap()
            )
            .is_err()
        );
    }
    assert!(
        commerce::initial_commitment_rumor(
            &command,
            &request,
            Some((RecordId::from_bytes([1; 32]), Sequence::new(0).unwrap())),
            UnixSeconds::new(451).unwrap()
        )
        .is_err()
    );
    assert!(
        commerce::initial_commitment_rumor(
            &command,
            &request,
            Some((
                RecordId::from_bytes([1; 32]),
                Sequence::new(i64::MAX as u64).unwrap()
            )),
            UnixSeconds::new(451).unwrap()
        )
        .is_err()
    );
}

#[test]
fn commerce_uses_the_shared_nested_budget_and_refuses_wrong_recipient_ciphertext_and_time() {
    let value = fixture();
    let command = command(&value);
    let request = commerce::decode(value["request_wire"].as_str().unwrap().as_bytes()).unwrap();
    let rumor = commerce::initial_commitment_rumor(
        &command,
        &request,
        None,
        UnixSeconds::new(451).unwrap(),
    )
    .unwrap();
    assert_eq!(
        rumor.budget(),
        PrivateEnvelopeBudget::for_rumor(rumor.protected_wire().len(), rumor.author()).unwrap()
    );
    let row = &value["seals"][0];
    let recipient: PublicKey = row["recipient"].as_str().unwrap().parse().unwrap();
    let ciphertext = row["ciphertext"].as_str().unwrap();
    let other: PublicKey = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
        .parse()
        .unwrap();
    assert_eq!(
        CommerceSeal::new(
            OperationId::from_bytes([1; 32]),
            CommandDigest::from_bytes([2; 32]),
            &rumor,
            other,
            UnixSeconds::new(450).unwrap(),
            ciphertext
        )
        .err(),
        Some(WireError::InvalidInput)
    );
    for (at, ciphertext) in [(0, ciphertext), (452, ciphertext), (450, "invalid")] {
        assert!(
            CommerceSeal::new(
                command.context().operation_id(),
                command.digest(),
                &rumor,
                recipient,
                UnixSeconds::new(at).unwrap(),
                ciphertext
            )
            .is_err()
        );
    }
}

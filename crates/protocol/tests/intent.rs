#![cfg(feature = "intent")]
use core::num::NonZeroU64;
use radroots_domain::{OperationId, PublicKey};
use radroots_protocol::intent::{ArtifactRole, IntentBinding};

#[test]
fn exact_v1_commitments_match_independent_ecmascript_vectors() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/exact_intent.v1.json")).unwrap();
    for row in vectors["vectors"].as_array().unwrap() {
        let role = match row["role"].as_str().unwrap() {
            "public_event" => ArtifactRole::PublicEvent,
            "business_seal" => ArtifactRole::BusinessSeal,
            "gift_wrap" => ArtifactRole::GiftWrap,
            _ => unreachable!(),
        };
        let peer = row["recipient"]
            .as_str()
            .map(|value| PublicKey::from_hex(value).unwrap());
        let binding = IntentBinding::from_components(
            OperationId::from_bytes([0x11; 32]),
            [0x22; 32],
            [0x33; 32],
            role,
            peer,
            NonZeroU64::new(row["deadline"].as_str().unwrap().parse().unwrap()).unwrap(),
        )
        .unwrap();
        let expected: OperationId = row["digest"].as_str().unwrap().parse().unwrap();
        assert_eq!(binding.digest(), expected.as_bytes());
        assert_eq!(binding.operation_id().as_bytes(), &[0x11; 32]);
        assert_eq!(binding.command_digest(), &[0x22; 32]);
        for (operation, command, plan) in [
            (OperationId::from_bytes([0x12; 32]), [0x22; 32], [0x33; 32]),
            (OperationId::from_bytes([0x11; 32]), [0x23; 32], [0x33; 32]),
            (OperationId::from_bytes([0x11; 32]), [0x22; 32], [0x34; 32]),
        ] {
            assert_ne!(
                IntentBinding::from_components(
                    operation,
                    command,
                    plan,
                    role,
                    peer,
                    NonZeroU64::new(row["deadline"].as_str().unwrap().parse().unwrap()).unwrap()
                )
                .unwrap(),
                binding
            );
        }
    }
}
#[test]
fn binding_data_rejects_invalid_role_recipient_and_time_without_authority() {
    let peer =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .unwrap();
    for (role, recipient, deadline) in [
        (ArtifactRole::PublicEvent, Some(peer), 1),
        (ArtifactRole::BusinessSeal, None, 1),
        (ArtifactRole::GiftWrap, None, 1),
        (ArtifactRole::BusinessSeal, Some(peer), 253_402_300_800_000),
    ] {
        let error = IntentBinding::from_components(
            OperationId::from_bytes([1; 32]),
            [2; 32],
            [3; 32],
            role,
            recipient,
            NonZeroU64::new(deadline).unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "invalid exact intent binding");
        assert!(core::error::Error::source(&error).is_none());
    }
}

#![cfg(feature = "chat")]

use radroots_domain::PublicKey;
use radroots_protocol::{
    chat::{GiftWrapBudget, SIGNED_MAX_BYTES, nip44_v2_payload_len},
    nip01::WireError,
};

#[test]
fn complete_budget_matches_independent_json_serialization_and_refuses_excess() {
    let key =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .unwrap();
    for size in [1, 32, 33, 257, 1024, 16_384, 32_768, 45_000, 65_535] {
        let ciphertext_bytes = nip44_v2_payload_len(size).unwrap();
        let json = serde_json::json!({
            "id": "0".repeat(64), "pubkey": key.to_hex(), "created_at": 253_402_300_799_u64,
            "kind": 1059, "tags": [["p", key.to_hex()]], "content": "A".repeat(ciphertext_bytes),
            "sig": "0".repeat(128),
        })
        .to_string();
        if json.len() <= SIGNED_MAX_BYTES {
            let budget = GiftWrapBudget::for_seal(size, key).unwrap();
            assert_eq!(budget.ciphertext_bytes(), ciphertext_bytes);
            assert_eq!(budget.signed_max_bytes(), json.len());
            assert!(format!("{budget:?}").contains("GiftWrapBudget"));
        } else {
            assert_eq!(
                GiftWrapBudget::for_seal(size, key),
                Err(WireError::LimitExceeded)
            );
        }
    }
    assert_eq!(
        GiftWrapBudget::for_seal(0, key),
        Err(WireError::InvalidInput)
    );
    for size in [65_536, 65_537, usize::MAX] {
        assert_eq!(
            GiftWrapBudget::for_seal(size, key),
            Err(WireError::LimitExceeded)
        );
    }
}

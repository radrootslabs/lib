#![cfg(feature = "serde")]

use std::collections::BTreeSet;

use radroots_identity::{
    AccountId, IdentityId, Profile, PublicIdentity, PublicKey, Username,
    account::{Record, Status},
};
use serde_json::Value;

const PUBLIC_VALUE_VECTORS: &str =
    include_str!("../../../contracts/conformance/vectors/identity/public_values.v1.json");

#[test]
fn public_value_vectors_are_unique_complete_and_executable() {
    let suite: Value = serde_json::from_str(PUBLIC_VALUE_VECTORS).expect("valid vector suite");
    assert_eq!(suite["suite"], "identity_public_values");
    assert_eq!(suite["contract_version"], "1.0.0");

    let vectors = suite["vectors"].as_array().expect("vector array");
    let mut ids = BTreeSet::new();
    for vector in vectors {
        let id = vector["id"].as_str().expect("vector id");
        assert!(ids.insert(id), "duplicate vector id {id}");
        let kind = vector["kind"].as_str().expect("vector kind");
        let input = vector["input"].clone();
        if let Some(expected) = vector.get("expected") {
            let actual = execute_valid(kind, input);
            assert_eq!(&actual, expected, "vector {id}");
        } else {
            let expected = vector["expected_error_contains"]
                .as_str()
                .expect("invalid vector error fragment");
            let error = execute_invalid(kind, input);
            assert!(
                error.contains(expected),
                "vector {id} expected error containing {expected:?}, got {error:?}"
            );
        }
    }
    assert_eq!(ids.len(), 14);
}

fn execute_valid(kind: &str, input: Value) -> Value {
    match kind {
        "identity.public_key.serde" => round_trip::<PublicKey>(input),
        "identity.identity_id.serde" => round_trip::<IdentityId>(input),
        "identity.account_id.serde" => round_trip::<AccountId>(input),
        "identity.username.serde" => round_trip::<Username>(input),
        "identity.profile.serde" => round_trip::<Profile>(input),
        "identity.public_identity.serde" => round_trip::<PublicIdentity>(input),
        "identity.account_record.serde" => round_trip::<Record>(input),
        "identity.account_status.serde" => round_trip::<Status>(input),
        unsupported => panic!("unsupported valid identity vector kind {unsupported}"),
    }
}

fn execute_invalid(kind: &str, input: Value) -> String {
    match kind {
        "identity.public_key.invalid" => decode_error::<PublicKey>(input),
        "identity.public_identity.invalid" => decode_error::<PublicIdentity>(input),
        "identity.account_record.invalid" => decode_error::<Record>(input),
        "identity.username.invalid" => decode_error::<Username>(input),
        unsupported => panic!("unsupported invalid identity vector kind {unsupported}"),
    }
}

fn round_trip<T>(input: Value) -> Value
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    serde_json::to_value(serde_json::from_value::<T>(input).expect("valid vector input"))
        .expect("serialize canonical value")
}

fn decode_error<T>(input: Value) -> String
where
    T: serde::de::DeserializeOwned + core::fmt::Debug,
{
    serde_json::from_value::<T>(input)
        .expect_err("invalid vector must be rejected")
        .to_string()
}

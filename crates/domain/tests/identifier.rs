use radroots_domain::{ExecutorId, IdentifierError, OperationId, OrderId, ThreadId, WorkspaceId};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

const SEQUENTIAL: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

#[test]
fn captured_context_identities_preserve_full_width_and_canonical_encoding() {
    macro_rules! check {
        ($t:ty) => {{
            let bytes = core::array::from_fn(|i| i as u8);
            let id = <$t>::from_bytes(bytes);
            assert_eq!(id.as_bytes(), &bytes);
            assert_eq!(id.into_bytes(), bytes);
            assert_eq!(id.to_string(), SEQUENTIAL);
            assert_eq!(SEQUENTIAL.parse::<$t>().unwrap(), id);
            assert!("A".repeat(64).parse::<$t>().is_err());
            assert!("0".repeat(63).parse::<$t>().is_err());
            #[cfg(feature = "serde")]
            {
                let encoded = format!("\"{SEQUENTIAL}\"");
                assert_eq!(serde_json::to_string(&id).unwrap(), encoded);
                assert_eq!(serde_json::from_str::<$t>(&encoded).unwrap(), id);
                assert!(serde_json::from_str::<$t>("1").is_err());
                assert!(serde_json::from_str::<$t>("\"x\"").is_err());
            }
        }};
    }
    check!(radroots_domain::GrantId);
    check!(radroots_domain::RecordId);
    check!(radroots_domain::SourceId);
    check!(radroots_domain::CommandDigest);
    check!(radroots_domain::RouteDigest);
    check!(radroots_domain::CheckpointDigest);
    check!(radroots_domain::ApprovalIntentDigest);
    check!(radroots_domain::TransactionId);
    check!(radroots_domain::MessageId);
    check!(radroots_domain::EnvelopeEventId);
}

#[test]
fn all_namespaces_match_independent_byte_and_hex_vectors() {
    let bytes = core::array::from_fn(|i| i as u8);
    macro_rules! check {
        ($t:ty) => {{
            let id = <$t>::from_bytes(bytes);
            assert_eq!(id.as_bytes(), &bytes);
            assert_eq!(id.into_bytes(), bytes);
            assert_eq!(id.to_string(), SEQUENTIAL);
            assert_eq!(SEQUENTIAL.parse::<$t>().unwrap(), id);
            let largest = <$t>::from_bytes([255; 32]);
            assert_eq!(largest.to_string(), "f".repeat(64));
            assert_eq!(largest.to_string().parse::<$t>().unwrap(), largest);
        }};
    }
    check!(OrderId);
    check!(ThreadId);
    check!(WorkspaceId);
    check!(ExecutorId);
    check!(OperationId);
}

#[test]
fn all_namespaces_reject_wrong_lengths_before_decoding() {
    for input in [
        String::new(),
        "0".repeat(63),
        "0".repeat(65),
        "0x".to_owned() + SEQUENTIAL,
        "0".repeat(65_536),
    ] {
        macro_rules! check {
            ($t:ty) => {
                assert_eq!(input.parse::<$t>(), Err(IdentifierError::InvalidLength));
            };
        }
        check!(OrderId);
        check!(ThreadId);
        check!(WorkspaceId);
        check!(ExecutorId);
        check!(OperationId);
    }
}

#[test]
fn encoding_is_ascii_lowercase_without_trimming_or_case_folding() {
    for input in [
        "A".repeat(64),
        "g".repeat(64),
        "é".repeat(32),
        " ".repeat(64),
        "0".repeat(63) + "\n",
        "0".repeat(63) + "\0",
        "a".repeat(63) + "F",
    ] {
        macro_rules! check {
            ($t:ty) => {
                assert_eq!(input.parse::<$t>(), Err(IdentifierError::InvalidEncoding));
            };
        }
        check!(OrderId);
        check!(ThreadId);
        check!(WorkspaceId);
        check!(ExecutorId);
        check!(OperationId);
    }
}

#[test]
fn every_ascii_character_is_checked_in_each_nibble_position() {
    for byte in 0..=127_u8 {
        for offset in 0..64 {
            let mut input = [b'0'; 64];
            input[offset] = byte;
            let result = core::str::from_utf8(&input).unwrap().parse::<OperationId>();
            if byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) {
                assert_eq!(result.unwrap().to_string().as_bytes(), input);
            } else {
                assert_eq!(result, Err(IdentifierError::InvalidEncoding));
            }
        }
    }
}

#[test]
fn equality_order_and_hash_are_exact_bytes_within_each_namespace() {
    fn hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }
    macro_rules! check {
        ($t:ty) => {{
            let id = SEQUENTIAL.parse::<$t>().unwrap();
            let copy = <$t>::from_bytes(id.into_bytes());
            assert_eq!(id, copy);
            assert_eq!(hash(&id), hash(&copy));
            let mut bytes = id.into_bytes();
            bytes[31] += 1;
            let later = <$t>::from_bytes(bytes);
            assert_ne!(id, later);
            assert!(id < later);
            assert_eq!(id.partial_cmp(&later), Some(core::cmp::Ordering::Less));
        }};
    }
    check!(OrderId);
    check!(ThreadId);
    check!(WorkspaceId);
    check!(ExecutorId);
    check!(OperationId);
}

#[test]
fn errors_are_typed_bounded_and_do_not_retain_input() {
    let errors = [
        IdentifierError::InvalidLength,
        IdentifierError::InvalidEncoding,
    ];
    for error in errors {
        let object: &dyn core::error::Error = &error;
        assert!(object.source().is_none());
        assert!(error.to_string().len() <= 64);
        assert!(!format!("{error:?}").contains("sensitive_fixture"));
    }
    assert_ne!(errors[0].to_string(), errors[1].to_string());
}

#[cfg(feature = "serde")]
mod json {
    use super::*;

    #[test]
    fn each_namespace_round_trips_as_one_lossless_canonical_string() {
        let encoded = format!("\"{SEQUENTIAL}\"");
        macro_rules! check {
            ($t:ty) => {{
                let id = SEQUENTIAL.parse::<$t>().unwrap();
                assert_eq!(serde_json::to_string(&id).unwrap(), encoded);
                assert_eq!(serde_json::from_str::<$t>(&encoded).unwrap(), id);
            }};
        }
        check!(OrderId);
        check!(ThreadId);
        check!(WorkspaceId);
        check!(ExecutorId);
        check!(OperationId);
    }

    #[test]
    fn json_cannot_bypass_string_width_or_encoding_validation() {
        for encoded in [
            "0".to_owned(),
            "null".to_owned(),
            "{}".to_owned(),
            "[0]".to_owned(),
            "\"0\"".to_owned(),
            format!("\"{}\"", "F".repeat(64)),
        ] {
            macro_rules! check {
                ($t:ty) => {
                    assert!(serde_json::from_str::<$t>(&encoded).is_err(), "{encoded}");
                };
            }
            check!(OrderId);
            check!(ThreadId);
            check!(WorkspaceId);
            check!(ExecutorId);
            check!(OperationId);
        }
    }
}

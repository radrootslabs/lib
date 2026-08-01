use radroots_secrets::error::SecretIdError;
use radroots_secrets::id::{BackendKind, KeyVersion, SECRET_ID_MAX_BYTES};
use radroots_secrets::{Error, SecretId, SecretRef};

#[test]
fn identifiers_accept_the_portable_alphabet_and_reject_unsafe_values() {
    for valid in ["a", "account-01", "farm.primary_key:v2", "A_B"] {
        assert_eq!(SecretId::parse(valid).expect("valid id").as_str(), valid);
    }

    assert_eq!(
        SecretId::parse(""),
        Err(Error::InvalidSecretId(SecretIdError::Empty))
    );
    assert_eq!(
        SecretId::parse("-leading-separator"),
        Err(Error::InvalidSecretId(SecretIdError::InvalidCharacter {
            byte_offset: 0
        }))
    );
    assert_eq!(
        SecretId::parse("path/traversal"),
        Err(Error::InvalidSecretId(SecretIdError::InvalidCharacter {
            byte_offset: 4
        }))
    );
    assert_eq!(
        SecretId::parse("é"),
        Err(Error::InvalidSecretId(SecretIdError::InvalidCharacter {
            byte_offset: 0
        }))
    );
    assert_eq!(
        SecretId::parse("a".repeat(SECRET_ID_MAX_BYTES + 1)),
        Err(Error::InvalidSecretId(SecretIdError::TooLong {
            actual_bytes: SECRET_ID_MAX_BYTES + 1,
            max_bytes: SECRET_ID_MAX_BYTES,
        }))
    );
}

#[test]
fn identifiers_and_references_are_redacted_in_diagnostics() {
    let id = SecretId::parse("account-signing-key").expect("valid id");
    assert_eq!(format!("{id:?}"), "SecretId(<redacted>)");
    assert_eq!(id.to_string(), "<redacted secret id>");

    let reference = SecretRef::new(
        id,
        BackendKind::Keyring,
        KeyVersion::new(7).expect("non-zero version"),
    );
    let diagnostic = format!("{reference:?}");
    assert!(diagnostic.contains("<redacted>"));
    assert!(diagnostic.contains("Keyring"));
    assert!(diagnostic.contains("KeyVersion(7)"));
    assert!(!diagnostic.contains("account-signing-key"));
}

#[test]
fn references_expose_only_validated_metadata() {
    assert_eq!(KeyVersion::new(0), Err(Error::InvalidKeyVersion));
    let version = KeyVersion::new(3).expect("non-zero version");
    let reference = SecretRef::new(
        SecretId::parse("service-token").expect("valid id"),
        BackendKind::External,
        version,
    );

    assert_eq!(reference.id().as_str(), "service-token");
    assert_eq!(reference.backend(), BackendKind::External);
    assert_eq!(reference.key_version().get(), 3);
}

#[cfg(feature = "serde")]
#[test]
fn identifier_serde_round_trips_through_validation() {
    let id = SecretId::parse("farm.primary-key:v1").expect("valid id");
    let json = serde_json::to_string(&id).expect("serialize id");
    assert_eq!(json, "\"farm.primary-key:v1\"");
    assert_eq!(
        serde_json::from_str::<SecretId>(&json)
            .expect("deserialize id")
            .as_str(),
        id.as_str()
    );
    assert!(serde_json::from_str::<SecretId>("\"../escape\"").is_err());
}

#[test]
fn normalized_errors_never_echo_identifier_input() {
    let raw = "secret/value";
    let error = SecretId::parse(raw).expect_err("invalid id");
    let display = error.to_string();
    let debug = format!("{error:?}");
    assert!(!display.contains(raw));
    assert!(!debug.contains(raw));
}

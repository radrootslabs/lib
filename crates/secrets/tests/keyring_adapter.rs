#![cfg(feature = "keyring")]

use radroots_secrets::id::BackendKind;
use radroots_secrets::keyring::KeyringProvider;
use radroots_secrets::{Error, SecretProvider};

#[test]
fn construction_is_validated_redacted_and_side_effect_free() {
    let provider = KeyringProvider::new("org.radroots.application").expect("provider");
    assert_eq!(provider.backend_kind(), BackendKind::Keyring);
    assert!(provider.capabilities().is_available());
    assert_eq!(format!("{provider:?}"), "KeyringProvider(<redacted>)");

    for invalid in ["", ".leading", "contains/slash", "contains space"] {
        assert!(matches!(
            KeyringProvider::new(invalid),
            Err(Error::InvalidServiceName)
        ));
    }
}

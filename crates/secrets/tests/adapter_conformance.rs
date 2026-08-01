#![cfg(any(feature = "memory", feature = "file", feature = "keyring"))]

use radroots_secrets::id::BackendKind;
use radroots_secrets::provider::{
    AccessPolicy, CapabilitySupport, ResidencySupport, SelectionPolicy,
};
use radroots_secrets::{Error, SecretProvider};

fn assert_provider_contract(
    provider: &dyn SecretProvider,
    backend: BackendKind,
    residency: ResidencySupport,
) {
    assert_eq!(provider.backend_kind(), backend);
    let capabilities = provider.capabilities();
    assert!(capabilities.is_available());
    assert_eq!(capabilities.residency(), residency);
    assert_eq!(capabilities.user_presence(), CapabilitySupport::Unavailable);
    assert_eq!(
        capabilities.hardware_backed(),
        CapabilitySupport::Unavailable
    );

    let candidates = [provider];
    let selected = SelectionPolicy::new(backend, AccessPolicy::standard())
        .select(&candidates)
        .expect("exact provider selection");
    assert_eq!(selected.backend_kind(), backend);
    assert!(matches!(
        SelectionPolicy::new(BackendKind::External, AccessPolicy::standard()).select(&candidates),
        Err(Error::BackendUnavailable {
            backend: BackendKind::External
        })
    ));
}

#[cfg(feature = "memory")]
#[test]
fn memory_provider_satisfies_shared_capability_contract() {
    let provider = radroots_secrets::memory::MemoryProvider::new();
    assert_provider_contract(&provider, BackendKind::Memory, ResidencySupport::Volatile);
}

#[cfg(feature = "file")]
#[test]
fn file_provider_satisfies_shared_capability_contract() {
    use radroots_secrets::file::{FileOpenMode, FileProvider};
    use radroots_secrets::wrapping::SecretMaterial;

    let temporary = tempfile::tempdir().expect("temporary directory");
    let provider = FileProvider::open(
        temporary.path().join("secrets"),
        FileOpenMode::CreateNew,
        SecretMaterial::from_slice(&[0x5a; 32]).expect("master key"),
    )
    .expect("file provider");
    assert_provider_contract(&provider, BackendKind::File, ResidencySupport::UserProfile);
}

#[cfg(feature = "keyring")]
#[test]
fn keyring_provider_satisfies_shared_capability_contract_without_access() {
    let provider = radroots_secrets::keyring::KeyringProvider::new("org.radroots.conformance")
        .expect("keyring provider");
    assert_provider_contract(
        &provider,
        BackendKind::Keyring,
        ResidencySupport::UserProfile,
    );
}

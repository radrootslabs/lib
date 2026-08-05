#![cfg(feature = "memory")]

use futures_executor::block_on;
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_secrets::id::{BackendKind, KeyVersion};
use radroots_secrets::memory::MemoryProvider;
use radroots_secrets::wrapping::{SecretMaterial, UnwrapRequest, WrapRequest};
use radroots_secrets::{Error, KeyWrapping, SecretId, SecretProvider, SecretRef};

fn reference(id: &str, version: u32) -> SecretRef {
    SecretRef::new(
        SecretId::parse(id).expect("valid id"),
        BackendKind::Memory,
        KeyVersion::new(version).expect("valid version"),
    )
}

fn context() -> EnvelopeContext {
    EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.memory_test").expect("purpose"),
        EnvelopeSubject::parse("memory_test", "fixture").expect("subject"),
        PayloadSchemaId::parse("radroots.memory_test.v1").expect("schema"),
    )
}

#[test]
fn memory_provider_has_explicit_lifecycle_and_round_trips() {
    let provider = MemoryProvider::new();
    let reference = reference("memory-key", 1);
    assert!(!provider.contains(&reference).expect("contains"));

    provider
        .provision(
            &reference,
            SecretMaterial::from_slice(&[0x41; 32]).expect("material"),
        )
        .expect("provision");
    assert!(provider.contains(&reference).expect("contains"));

    let plaintext = SecretMaterial::from_slice(&[0x41; 32]).expect("material");
    let context = context();
    let wrapped =
        block_on(provider.wrap(WrapRequest::new(&reference, &context, &plaintext))).expect("wrap");
    let opened = block_on(provider.unwrap(UnwrapRequest::new(&reference, &context, &wrapped)))
        .expect("unwrap");
    opened.expose_secret(|bytes| assert_eq!(bytes, &[0x41; 32]));

    assert!(provider.remove(&reference).expect("remove"));
    assert!(!provider.remove(&reference).expect("idempotent remove"));
    assert!(!provider.contains(&reference).expect("contains"));
}

#[test]
fn missing_and_mismatched_material_fail_closed() {
    let provider = MemoryProvider::new();
    let reference = reference("missing-key", 1);
    let plaintext = SecretMaterial::from_slice(&[0x11; 32]).expect("material");
    assert!(matches!(
        block_on(provider.wrap(WrapRequest::new(&reference, &context(), &plaintext))),
        Err(Error::SecretNotFound {
            backend: BackendKind::Memory,
            key_version: 1,
        })
    ));

    provider
        .provision(
            &reference,
            SecretMaterial::from_slice(&[0x22; 32]).expect("material"),
        )
        .expect("provision");
    assert!(matches!(
        block_on(provider.wrap(WrapRequest::new(&reference, &context(), &plaintext))),
        Err(Error::BackendFailure { .. })
    ));
    assert!(matches!(
        provider.provision(
            &reference,
            SecretMaterial::from_slice(&[0x33; 32]).expect("material"),
        ),
        Err(Error::SecretAlreadyExists {
            backend: BackendKind::Memory,
            key_version: 1,
        })
    ));
}

#[test]
fn rotation_is_monotonic_atomic_and_invalidates_the_old_version() {
    let provider = MemoryProvider::new();
    let current = reference("rotating-key", 1);
    let next = reference("rotating-key", 2);
    provider
        .provision(
            &current,
            SecretMaterial::from_slice(&[0x11; 32]).expect("material"),
        )
        .expect("provision");
    provider
        .rotate(
            &current,
            &next,
            SecretMaterial::from_slice(&[0x22; 32]).expect("material"),
        )
        .expect("rotate");

    assert!(!provider.contains(&current).expect("old contains"));
    assert!(provider.contains(&next).expect("new contains"));
    let new_material = SecretMaterial::from_slice(&[0x22; 32]).expect("material");
    assert!(block_on(provider.wrap(WrapRequest::new(&next, &context(), &new_material))).is_ok());

    let invalid = reference("different-key", 3);
    assert_eq!(
        provider.rotate(
            &next,
            &invalid,
            SecretMaterial::from_slice(&[0x33; 32]).expect("material"),
        ),
        Err(Error::InvalidRotation)
    );
    assert!(provider.contains(&next).expect("rotation stayed atomic"));
}

#[test]
fn provider_capabilities_and_diagnostics_are_explicit() {
    let provider = MemoryProvider::new();
    assert_eq!(provider.backend_kind(), BackendKind::Memory);
    assert!(provider.capabilities().is_available());
    assert_eq!(format!("{provider:?}"), "MemoryProvider(<redacted>)");
}

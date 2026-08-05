use futures_executor::block_on;
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_secrets::error::{Operation, PolicyRequirement};
use radroots_secrets::id::{BackendKind, KeyVersion};
use radroots_secrets::provider::{
    AccessPolicy, CapabilitySupport, HardwarePolicy, ResidencyPolicy, ResidencySupport,
    SecretCapabilities, SelectionPolicy, UserPresencePolicy,
};
use radroots_secrets::wrapping::{
    BoxFuture, SecretMaterial, UnwrapRequest, WrapRequest, WrappedSecret,
};
use radroots_secrets::{Error, KeyWrapping, SecretId, SecretProvider, SecretRef};

struct MockProvider {
    backend: BackendKind,
    capabilities: SecretCapabilities,
    fail: bool,
}

impl KeyWrapping for MockProvider {
    fn wrap<'a>(&'a self, request: WrapRequest<'a>) -> BoxFuture<'a, Result<WrappedSecret, Error>> {
        Box::pin(async move {
            self.validate_reference(request.reference())?;
            if self.fail {
                return Err(Error::BackendFailure {
                    backend: self.backend,
                    operation: Operation::Wrap,
                });
            }
            let encoded = request.plaintext().expose_secret(|plaintext| {
                plaintext.iter().map(|byte| byte ^ 0xA5).collect::<Vec<_>>()
            });
            WrappedSecret::from_bytes(encoded)
        })
    }

    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        Box::pin(async move {
            self.validate_reference(request.reference())?;
            if self.fail {
                return Err(Error::BackendFailure {
                    backend: self.backend,
                    operation: Operation::Unwrap,
                });
            }
            let decoded = request
                .wrapped()
                .as_bytes()
                .iter()
                .map(|byte| byte ^ 0xA5)
                .collect::<Vec<_>>();
            SecretMaterial::from_slice(decoded.as_slice())
        })
    }
}

impl SecretProvider for MockProvider {
    fn backend_kind(&self) -> BackendKind {
        self.backend
    }

    fn capabilities(&self) -> SecretCapabilities {
        self.capabilities
    }
}

impl MockProvider {
    fn validate_reference(&self, reference: &SecretRef) -> Result<(), Error> {
        if reference.backend() != self.backend {
            return Err(Error::BackendMismatch {
                provider: self.backend,
                reference: reference.backend(),
            });
        }
        Ok(())
    }
}

fn provider(backend: BackendKind, capabilities: SecretCapabilities) -> MockProvider {
    MockProvider {
        backend,
        capabilities,
        fail: false,
    }
}

fn reference(backend: BackendKind) -> SecretRef {
    SecretRef::new(
        SecretId::parse("test-wrapping-key").expect("valid id"),
        backend,
        KeyVersion::new(1).expect("valid version"),
    )
}

fn context() -> EnvelopeContext {
    EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.provider_test").expect("purpose"),
        EnvelopeSubject::parse("provider_test", "fixture").expect("subject"),
        PayloadSchemaId::parse("radroots.provider_test.v1").expect("schema"),
    )
}

#[test]
fn provider_traits_are_dyn_compatible_and_round_trip_opaque_material() {
    fn accept_dyn(_: &dyn SecretProvider) {}

    let provider = provider(
        BackendKind::Memory,
        SecretCapabilities::available(
            ResidencySupport::Volatile,
            CapabilitySupport::Unavailable,
            CapabilitySupport::Unavailable,
        ),
    );
    accept_dyn(&provider);

    let reference = reference(BackendKind::Memory);
    let plaintext = SecretMaterial::from_slice(b"caller-owned-data-key").expect("material");
    let context = context();
    let wrapped =
        block_on(provider.wrap(WrapRequest::new(&reference, &context, &plaintext))).expect("wrap");
    let opened = block_on(provider.unwrap(UnwrapRequest::new(&reference, &context, &wrapped)))
        .expect("unwrap");
    opened.expose_secret(|bytes| assert_eq!(bytes, b"caller-owned-data-key"));

    assert_eq!(format!("{plaintext:?}"), "SecretMaterial(<redacted>)");
    assert_eq!(format!("{wrapped:?}"), "WrappedSecret(<redacted>)");
}

#[test]
fn exact_selection_never_falls_back_to_another_backend() {
    let memory = provider(
        BackendKind::Memory,
        SecretCapabilities::available(
            ResidencySupport::Volatile,
            CapabilitySupport::Unavailable,
            CapabilitySupport::Unavailable,
        ),
    );
    let file = provider(
        BackendKind::File,
        SecretCapabilities::available(
            ResidencySupport::DeviceLocal,
            CapabilitySupport::Unavailable,
            CapabilitySupport::Unavailable,
        ),
    );
    let candidates: [&dyn SecretProvider; 2] = [&memory, &file];

    let selected = SelectionPolicy::new(BackendKind::File, AccessPolicy::standard())
        .select(&candidates)
        .expect("file selected");
    assert_eq!(selected.backend_kind(), BackendKind::File);

    assert!(matches!(
        SelectionPolicy::new(BackendKind::Keyring, AccessPolicy::standard()).select(&candidates),
        Err(Error::BackendUnavailable {
            backend: BackendKind::Keyring
        })
    ));
}

#[test]
fn selection_enforces_device_user_presence_and_hardware_policy() {
    let keyring = provider(
        BackendKind::Keyring,
        SecretCapabilities::available(
            ResidencySupport::UserProfile,
            CapabilitySupport::Unavailable,
            CapabilitySupport::Unavailable,
        ),
    );
    let candidates: [&dyn SecretProvider; 1] = [&keyring];

    let cases = [
        (
            AccessPolicy::new(
                ResidencyPolicy::DeviceLocal,
                UserPresencePolicy::NotRequired,
                HardwarePolicy::Any,
            ),
            PolicyRequirement::DeviceLocal,
        ),
        (
            AccessPolicy::new(
                ResidencyPolicy::Any,
                UserPresencePolicy::Required,
                HardwarePolicy::Any,
            ),
            PolicyRequirement::UserPresence,
        ),
        (
            AccessPolicy::new(
                ResidencyPolicy::Any,
                UserPresencePolicy::NotRequired,
                HardwarePolicy::RequireHardwareBacked,
            ),
            PolicyRequirement::HardwareBacked,
        ),
    ];

    for (access, expected) in cases {
        assert_eq!(
            SelectionPolicy::new(BackendKind::Keyring, access)
                .select(&candidates)
                .map(SecretProvider::backend_kind),
            Err(Error::PolicyUnsupported {
                backend: BackendKind::Keyring,
                requirement: expected,
            })
        );
    }
}

#[test]
fn provider_errors_are_normalized_and_secret_safe() {
    let provider = MockProvider {
        backend: BackendKind::External,
        capabilities: SecretCapabilities::available(
            ResidencySupport::DeviceLocal,
            CapabilitySupport::Supported,
            CapabilitySupport::Supported,
        ),
        fail: true,
    };
    let reference = reference(BackendKind::External);
    let plaintext = SecretMaterial::from_slice(b"must-not-appear").expect("material");
    let error = block_on(provider.wrap(WrapRequest::new(&reference, &context(), &plaintext)))
        .expect_err("backend failure");
    assert_eq!(
        error,
        Error::BackendFailure {
            backend: BackendKind::External,
            operation: Operation::Wrap,
        }
    );
    assert!(!error.to_string().contains("must-not-appear"));
    assert!(!format!("{error:?}").contains("must-not-appear"));
}

#[test]
fn reference_backend_mismatch_fails_before_wrapping() {
    let provider = provider(
        BackendKind::Memory,
        SecretCapabilities::available(
            ResidencySupport::Volatile,
            CapabilitySupport::Unavailable,
            CapabilitySupport::Unavailable,
        ),
    );
    let reference = reference(BackendKind::File);
    let plaintext = SecretMaterial::from_slice(b"data-key").expect("material");
    assert_eq!(
        block_on(provider.wrap(WrapRequest::new(&reference, &context(), &plaintext))),
        Err(Error::BackendMismatch {
            provider: BackendKind::Memory,
            reference: BackendKind::File,
        })
    );
}

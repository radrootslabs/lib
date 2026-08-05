//! Normalized, secret-safe signing failures.

use core::fmt;

use radroots_protocol::{
    error::v1::{Class, Descriptor as ProtocolDescriptor, ErrorReport, KnownCode, RecoveryAction},
    runtime::v1::OperationId,
};

use crate::recovery::RemoteEffect;

#[cfg(feature = "std")]
use std::boxed::Box;

/// Stable native signing failure kinds.
///
/// These variants describe the signing contract rather than any concrete
/// signer library. Additive variants remain possible before 1.0.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Kind {
    InvalidArgument,
    AuthorizationDenied,
    SignerCapabilityMissing,
    SignerUnavailable,
    SignerRejected,
    SignerTimeout,
    SignerCancelled,
    SignerOutputInvalid,
    DeadlineExceeded,
    InternalError,
}

/// One signing error descriptor generated from the native-to-protocol map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Descriptor {
    kind: Kind,
    code: KnownCode,
    message: &'static str,
}

impl Descriptor {
    #[must_use]
    pub const fn kind(self) -> Kind {
        self.kind
    }

    #[must_use]
    pub const fn known_code(self) -> KnownCode {
        self.code
    }

    #[must_use]
    pub const fn code(self) -> &'static str {
        self.code.as_str()
    }

    #[must_use]
    pub const fn class(self) -> Class {
        self.protocol_descriptor().class
    }

    #[must_use]
    pub const fn retryable(self) -> bool {
        self.protocol_descriptor().retryable
    }

    #[must_use]
    pub const fn recovery_actions(self) -> &'static [RecoveryAction] {
        self.protocol_descriptor().recovery_actions
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        self.message
    }

    const fn protocol_descriptor(self) -> ProtocolDescriptor {
        self.code.descriptor()
    }
}

macro_rules! signing_error_catalog {
    ($( $variant:ident => ($code:ident, $message:literal) ),+ $(,)?) => {
        impl Kind {
            /// Every native signing failure kind in stable catalog order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Returns metadata generated from this package's single mapping
            /// and the protocol catalog's single class/recovery authority.
            #[must_use]
            pub const fn descriptor(self) -> Descriptor {
                match self {
                    $(Self::$variant => Descriptor {
                        kind: Self::$variant,
                        code: KnownCode::$code,
                        message: $message,
                    },)+
                }
            }
        }

        /// Complete stable native signing error catalog.
        pub const CATALOG: &[Descriptor] = &[
            $(Descriptor {
                kind: Kind::$variant,
                code: KnownCode::$code,
                message: $message,
            },)+
        ];
    };
}

signing_error_catalog! {
    InvalidArgument => (InvalidArgument, "signing request is invalid"),
    AuthorizationDenied => (AuthorizationDenied, "signing authorization was denied"),
    SignerCapabilityMissing => (SignerCapabilityMissing, "required signer capability is missing"),
    SignerUnavailable => (SignerUnavailable, "signer is unavailable"),
    SignerRejected => (SignerRejected, "signer rejected the request"),
    SignerTimeout => (SignerTimeout, "signer timed out"),
    SignerCancelled => (SignerCancelled, "signing was cancelled"),
    SignerOutputInvalid => (SignerOutputInvalid, "signer output did not match the frozen draft"),
    DeadlineExceeded => (DeadlineExceeded, "signing deadline was exceeded"),
    InternalError => (InternalError, "internal signing failure"),
}

/// A normalized signing failure with an optional native source.
///
/// Display and debug output are stable and never copy source text. Under the
/// `std` feature callers may inspect the explicit `source()` chain for local
/// diagnostics; protocol conversion always discards it.
pub struct Error {
    kind: Kind,
    remote_effect: RemoteEffect,
    #[cfg(feature = "std")]
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl Error {
    /// Creates a source-free normalized failure.
    #[must_use]
    pub const fn new(kind: Kind) -> Self {
        Self {
            kind,
            remote_effect: RemoteEffect::None,
            #[cfg(feature = "std")]
            source: None,
        }
    }

    /// Preserves a native source without exposing it through display, debug,
    /// or protocol serialization.
    #[cfg(feature = "std")]
    pub fn with_source<E>(kind: Kind, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            kind,
            remote_effect: RemoteEffect::None,
            source: Some(Box::new(source)),
        }
    }

    /// Marks that a failed remote invocation may already have taken effect.
    #[must_use]
    pub const fn with_possible_remote_effect(mut self) -> Self {
        self.remote_effect = RemoteEffect::MayHaveOccurred;
        self
    }

    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    #[must_use]
    pub const fn remote_effect(&self) -> RemoteEffect {
        self.remote_effect
    }

    #[must_use]
    pub const fn descriptor(&self) -> Descriptor {
        self.kind.descriptor()
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.descriptor().code()
    }

    #[must_use]
    pub const fn class(&self) -> Class {
        self.descriptor().class()
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.descriptor().retryable()
    }

    #[must_use]
    pub const fn recovery_actions(&self) -> &'static [RecoveryAction] {
        self.descriptor().recovery_actions()
    }

    /// Produces the versioned boundary report without copying source text.
    #[must_use]
    pub fn to_report(&self, operation_id: Option<OperationId>) -> ErrorReport {
        ErrorReport::redacted_from_source(self.descriptor().known_code(), operation_id, None)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = formatter.debug_struct("Error");
        value.field("kind", &self.kind);
        value.field("remote_effect", &self.remote_effect);
        #[cfg(feature = "std")]
        value.field("source", &self.source.as_ref().map(|_| "[redacted]"));
        value.finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.descriptor().message())
    }
}

impl core::error::Error for Error {
    #[cfg(feature = "std")]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn core::error::Error + 'static))
    }
}

impl From<&Error> for ErrorReport {
    fn from(error: &Error) -> Self {
        error.to_report(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    use std::{collections::BTreeSet, error::Error as _};

    #[test]
    fn catalog_codes_are_unique_and_metadata_matches_protocol_authority() {
        assert_eq!(CATALOG.len(), Kind::ALL.len());
        let mut codes = alloc_or_std_set();
        for (index, descriptor) in CATALOG.iter().copied().enumerate() {
            assert!(codes.insert(descriptor.code()));
            assert_eq!(descriptor.kind(), Kind::ALL[index]);
            assert_eq!(descriptor.kind().descriptor(), descriptor);
            let protocol = descriptor.known_code().descriptor();
            assert_eq!(descriptor.class(), protocol.class);
            assert_eq!(descriptor.retryable(), protocol.retryable);
            assert_eq!(descriptor.recovery_actions(), protocol.recovery_actions);
            assert!(!descriptor.message().is_empty());
            let report = Error::new(descriptor.kind()).to_report(None);
            assert_eq!(report.code().known_code(), Some(descriptor.known_code()));
            assert_eq!(report.class(), descriptor.class());
            assert_eq!(report.retryable(), descriptor.retryable());
            assert_eq!(report.recovery_actions(), descriptor.recovery_actions());
            assert_eq!(report.message().as_str(), "[redacted]");
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn native_source_is_preserved_but_diagnostics_and_reports_are_redacted() {
        #[derive(Debug)]
        struct SensitiveSource;

        impl fmt::Display for SensitiveSource {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("nsec1-do-not-disclose")
            }
        }

        impl std::error::Error for SensitiveSource {}

        let error = Error::with_source(Kind::SignerUnavailable, SensitiveSource);
        assert_eq!(
            error.source().expect("source").to_string(),
            "nsec1-do-not-disclose"
        );
        assert!(!error.to_string().contains("nsec1"));
        assert!(!format!("{error:?}").contains("nsec1"));
        assert_eq!(error.code(), "signer_unavailable");
        assert!(error.retryable());

        let report = error.to_report(Some(OperationId::SyncPush));
        assert_eq!(report.code().as_str(), "signer_unavailable");
        assert_eq!(report.operation_id(), Some(OperationId::SyncPush));
        assert_eq!(report.message().as_str(), "[redacted]");
        #[cfg(feature = "serde")]
        assert!(
            !serde_json::to_string(&report)
                .expect("report")
                .contains("nsec1")
        );
    }

    #[cfg(feature = "std")]
    fn alloc_or_std_set() -> BTreeSet<&'static str> {
        BTreeSet::new()
    }

    #[cfg(not(feature = "std"))]
    fn alloc_or_std_set() -> alloc::collections::BTreeSet<&'static str> {
        alloc::collections::BTreeSet::new()
    }
}

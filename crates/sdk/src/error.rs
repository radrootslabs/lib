//! SDK-owned native errors and secret-safe protocol conversion.

use std::{error, fmt};

use radroots_protocol::{
    error::v1::{
        CapabilityId as ProtocolCapabilityId, Class, ErrorReport, KnownCode, RecoveryAction,
        SafeDetails, SafeMessage,
    },
    runtime::v1::OperationId,
};

use crate::capability::CapabilityId;

macro_rules! error_catalog {
    ($(
        $kind:ident => {
            code: $code:ident,
            operation: $operation:expr,
            capability: $capability:expr,
            message: $message:literal,
            safe_detail_keys: [$($detail_key:literal),* $(,)?]
        }
    ),+ $(,)?) => {
        /// Stable native SDK error category.
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[non_exhaustive]
        pub enum ErrorKind {
            $($kind,)+
        }

        impl ErrorKind {
            /// Every SDK error category in catalog order.
            pub const ALL: &'static [Self] = &[$(Self::$kind),+];

            /// Returns metadata generated from the single SDK authority.
            #[must_use]
            pub const fn descriptor(self) -> ErrorDescriptor {
                match self {
                    $(Self::$kind => ErrorDescriptor {
                        kind: Self::$kind,
                        code: KnownCode::$code,
                        operation: $operation,
                        capability: $capability,
                        message: $message,
                        safe_detail_keys: &[$($detail_key),*],
                    },)+
                }
            }
        }

        /// Complete SDK-native error metadata catalog.
        pub const CATALOG: &[ErrorDescriptor] = &[
            $(ErrorDescriptor {
                kind: ErrorKind::$kind,
                code: KnownCode::$code,
                operation: $operation,
                capability: $capability,
                message: $message,
                safe_detail_keys: &[$($detail_key),*],
            },)+
        ];
    };
}

error_catalog! {
    MissingStorage => {
        code: MissingStorage,
        operation: None,
        capability: Some(CapabilityId::CANONICAL_STORAGE),
        message: "SDK storage capability is not configured",
        safe_detail_keys: []
    },
    SignerWithoutSink => {
        code: SignerWithoutSink,
        operation: None,
        capability: None,
        message: "SDK signer requires an outbound event sink",
        safe_detail_keys: []
    },
    CloseInProgress => {
        code: ClientCloseInProgress,
        operation: None,
        capability: Some(CapabilityId::CANONICAL_STORAGE),
        message: "SDK client close is in progress",
        safe_detail_keys: []
    },
    ClientClosing => {
        code: ClientClosing,
        operation: None,
        capability: Some(CapabilityId::CANONICAL_STORAGE),
        message: "SDK client close requires completion or retry",
        safe_detail_keys: []
    },
    ClientClosed => {
        code: ClientClosed,
        operation: None,
        capability: Some(CapabilityId::CANONICAL_STORAGE),
        message: "SDK client is closed",
        safe_detail_keys: []
    },
    StorageCloseFailed => {
        code: StorageCloseFailed,
        operation: None,
        capability: Some(CapabilityId::CANONICAL_STORAGE),
        message: "SDK storage close failed",
        safe_detail_keys: []
    },
}

/// Stable metadata for one native SDK failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ErrorDescriptor {
    kind: ErrorKind,
    code: KnownCode,
    operation: Option<OperationId>,
    capability: Option<CapabilityId>,
    message: &'static str,
    safe_detail_keys: &'static [&'static str],
}

impl ErrorDescriptor {
    /// Returns the native category.
    #[must_use]
    pub const fn kind(self) -> ErrorKind {
        self.kind
    }

    /// Returns the stable protocol code.
    #[must_use]
    pub const fn code(self) -> KnownCode {
        self.code
    }

    /// Returns the class from the generated protocol authority.
    #[must_use]
    pub const fn class(self) -> Class {
        self.code.descriptor().class
    }

    /// Returns retryability from the generated protocol authority.
    #[must_use]
    pub const fn retryable(self) -> bool {
        self.code.descriptor().retryable
    }

    /// Returns recovery actions from the generated protocol authority.
    #[must_use]
    pub const fn recovery_actions(self) -> &'static [RecoveryAction] {
        self.code.descriptor().recovery_actions
    }

    /// Returns the related operation when the protocol catalog defines one.
    #[must_use]
    pub const fn operation(self) -> Option<OperationId> {
        self.operation
    }

    /// Returns the related runtime capability.
    #[must_use]
    pub const fn capability(self) -> Option<CapabilityId> {
        self.capability
    }

    /// Returns the secret-safe native display message.
    #[must_use]
    pub const fn message(self) -> &'static str {
        self.message
    }

    /// Returns the only structured detail keys permitted for this category.
    #[must_use]
    pub const fn safe_detail_keys(self) -> &'static [&'static str] {
        self.safe_detail_keys
    }
}

/// Native SDK failure retaining an optional private source chain.
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn error::Error + Send + Sync>>,
}

impl Error {
    pub(crate) fn missing_storage() -> Self {
        Self::without_source(ErrorKind::MissingStorage)
    }

    pub(crate) fn signer_without_sink() -> Self {
        Self::without_source(ErrorKind::SignerWithoutSink)
    }

    pub(crate) fn close_in_progress() -> Self {
        Self::without_source(ErrorKind::CloseInProgress)
    }

    pub(crate) fn client_closing() -> Self {
        Self::without_source(ErrorKind::ClientClosing)
    }

    pub(crate) fn client_closed() -> Self {
        Self::without_source(ErrorKind::ClientClosed)
    }

    pub(crate) fn storage_close_failed(source: radroots_storage::Error) -> Self {
        Self {
            kind: ErrorKind::StorageCloseFailed,
            source: Some(Box::new(source)),
        }
    }

    fn without_source(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }

    /// Returns the stable native category.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns metadata from the single SDK catalog.
    #[must_use]
    pub const fn descriptor(&self) -> ErrorDescriptor {
        self.kind.descriptor()
    }

    /// Converts to the V1 secret-safe protocol boundary.
    ///
    /// Native source messages are deliberately excluded. Catalog validation
    /// tests guarantee that static messages and capability IDs are valid; this
    /// conversion still fails closed to redacted text or no capability if a
    /// future catalog edit violates those invariants.
    #[must_use]
    pub fn to_report(&self) -> ErrorReport {
        let descriptor = self.descriptor();
        let capability = descriptor
            .capability()
            .and_then(|id| ProtocolCapabilityId::parse(id.as_str().to_owned()).ok());
        let message = SafeMessage::parse(descriptor.message().to_owned())
            .unwrap_or_else(|_| SafeMessage::redacted());
        ErrorReport::known(
            descriptor.code(),
            descriptor.operation(),
            capability,
            message,
            SafeDetails::default(),
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.descriptor().message())
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Error")
            .field("kind", &self.kind)
            .field("code", &self.descriptor().code())
            .finish_non_exhaustive()
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn error::Error + 'static))
    }
}

/// SDK result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, error::Error as _};

    use super::*;

    #[test]
    fn catalog_is_exhaustive_unique_and_protocol_valid() {
        assert_eq!(CATALOG.len(), ErrorKind::ALL.len());
        let mut kinds = BTreeSet::new();
        let mut codes = BTreeSet::new();
        for (index, descriptor) in CATALOG.iter().copied().enumerate() {
            assert!(kinds.insert(descriptor.kind()));
            assert!(codes.insert(descriptor.code().as_str()));
            assert_eq!(descriptor.kind(), ErrorKind::ALL[index]);
            assert_eq!(descriptor, descriptor.kind().descriptor());
            assert!(!descriptor.recovery_actions().is_empty());
            assert!(SafeMessage::parse(descriptor.message()).is_ok());
            if let Some(capability) = descriptor.capability() {
                assert!(ProtocolCapabilityId::parse(capability.as_str()).is_ok());
            }
            assert!(descriptor.safe_detail_keys().is_empty());
            Error::without_source(descriptor.kind())
                .to_report()
                .validate()
                .expect("protocol report");
        }
    }

    #[test]
    fn native_source_is_preserved_but_protocol_report_is_redacted_from_it() {
        let error = Error::storage_close_failed(radroots_storage::Error::BackendUnavailable);
        assert!(error.source().is_some());
        assert_eq!(error.kind(), ErrorKind::StorageCloseFailed);
        assert_eq!(error.to_string(), "SDK storage close failed");
        let report = error.to_report();
        assert_eq!(report.code().as_str(), "storage_close_failed");
        assert_eq!(report.message().as_str(), "SDK storage close failed");
        assert!(!report.message().as_str().contains("backend"));
        assert!(format!("{error:?}").contains("StorageCloseFailed"));
        assert!(!format!("{error:?}").contains("BackendUnavailable"));
    }
}

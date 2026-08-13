//! Strict, service-neutral configuration document mechanics.

mod document;
mod value;

pub use document::{
    CONFIG_DOCUMENT_MAX_UTF8_BYTES, CONFIG_SCHEMA_ID_MAX_UTF8_BYTES, ConfigDocumentError,
    ConfigDocumentErrorKind, ConfigDocumentExpectation, ConfigDocumentExpectationError,
    ConfigDocumentLocation, load_config_document,
};
pub use value::{
    BoundedCount, BoundedCountError, ByteLimit, ByteLimitError, LoggingFormat, LoggingFormatError,
    OptionalOperationsBind, OptionalOperationsBindError, PositiveDuration, PositiveDurationError,
};

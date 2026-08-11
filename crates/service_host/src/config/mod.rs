//! Strict, service-neutral configuration document mechanics.

mod document;

pub use document::{
    CONFIG_DOCUMENT_MAX_UTF8_BYTES, CONFIG_SCHEMA_ID_MAX_UTF8_BYTES, ConfigDocumentError,
    ConfigDocumentErrorKind, ConfigDocumentExpectation, ConfigDocumentExpectationError,
    ConfigDocumentLocation, load_config_document,
};

//! Bounded loading and exact versioned TOML document admission.

use core::fmt;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Hard upper bound for one complete service configuration document.
pub const CONFIG_DOCUMENT_MAX_UTF8_BYTES: usize = 1024 * 1024;

/// Hard upper bound for a versioned configuration schema identifier.
pub const CONFIG_SCHEMA_ID_MAX_UTF8_BYTES: usize = 128;

/// Validated schema identity and exact version expected from one document.
#[derive(Clone, PartialEq, Eq)]
pub struct ConfigDocumentExpectation {
    schema: Box<str>,
    schema_version: u32,
}

impl ConfigDocumentExpectation {
    /// Creates an exact expected document identity.
    pub fn new(
        schema: impl AsRef<str>,
        schema_version: u32,
    ) -> Result<Self, ConfigDocumentExpectationError> {
        let schema = schema.as_ref();
        if !valid_schema_id(schema) {
            return Err(ConfigDocumentExpectationError::InvalidSchema);
        }
        if schema_version == 0 {
            return Err(ConfigDocumentExpectationError::InvalidSchemaVersion);
        }
        Ok(Self {
            schema: schema.to_owned().into_boxed_str(),
            schema_version,
        })
    }

    /// Returns the exact schema identifier.
    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Returns the exact schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
}

impl fmt::Debug for ConfigDocumentExpectation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigDocumentExpectation")
            .field("schema", &self.schema)
            .field("schema_version", &self.schema_version)
            .finish()
    }
}

/// Invalid caller-provided schema expectation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigDocumentExpectationError {
    InvalidSchema,
    InvalidSchemaVersion,
}

impl fmt::Display for ConfigDocumentExpectationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("configuration document expectation is invalid")
    }
}

impl Error for ConfigDocumentExpectationError {}

/// Stable classification for a configuration document failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigDocumentErrorKind {
    NotFound,
    Read,
    TooLarge,
    InvalidUtf8,
    MalformedToml,
    MissingSchema,
    InvalidSchema,
    SchemaMismatch,
    MissingSchemaVersion,
    InvalidSchemaVersion,
    UnsupportedSchemaVersion,
    TypedDocument,
}

impl ConfigDocumentErrorKind {
    const fn message(self) -> &'static str {
        match self {
            Self::NotFound => "configuration document was not found",
            Self::Read => "configuration document could not be read",
            Self::TooLarge => "configuration document exceeds its size limit",
            Self::InvalidUtf8 => "configuration document is not valid UTF-8",
            Self::MalformedToml => "configuration document is not valid TOML",
            Self::MissingSchema => "configuration document schema is missing",
            Self::InvalidSchema => "configuration document schema is invalid",
            Self::SchemaMismatch => "configuration document schema is unsupported",
            Self::MissingSchemaVersion => "configuration document schema version is missing",
            Self::InvalidSchemaVersion => "configuration document schema version is invalid",
            Self::UnsupportedSchemaVersion => {
                "configuration document schema version is unsupported"
            }
            Self::TypedDocument => "configuration document fields are invalid",
        }
    }
}

/// One-based location inside the selected source document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigDocumentLocation {
    line: u32,
    column: u32,
}

impl ConfigDocumentLocation {
    /// Returns the one-based line.
    #[must_use]
    pub const fn line(self) -> u32 {
        self.line
    }

    /// Returns the one-based UTF-8 character column.
    #[must_use]
    pub const fn column(self) -> u32 {
        self.column
    }
}

/// Source-located failure with a redacted ordinary representation.
pub struct ConfigDocumentError {
    kind: ConfigDocumentErrorKind,
    source_path: PathBuf,
    location: Option<ConfigDocumentLocation>,
    io_kind: Option<io::ErrorKind>,
}

impl ConfigDocumentError {
    /// Returns the stable failure classification.
    #[must_use]
    pub const fn kind(&self) -> ConfigDocumentErrorKind {
        self.kind
    }

    /// Returns the selected source path for trusted local diagnostics.
    #[must_use]
    pub fn trusted_source_path(&self) -> &Path {
        &self.source_path
    }

    /// Returns a parser-provided source location when one is available.
    #[must_use]
    pub const fn location(&self) -> Option<ConfigDocumentLocation> {
        self.location
    }

    /// Returns only the safe I/O classification when the failure came from I/O.
    #[must_use]
    pub const fn io_kind(&self) -> Option<io::ErrorKind> {
        self.io_kind
    }

    fn without_source(kind: ConfigDocumentErrorKind, source_path: &Path) -> Self {
        Self {
            kind,
            source_path: source_path.to_path_buf(),
            location: None,
            io_kind: None,
        }
    }

    fn with_io_kind(
        kind: ConfigDocumentErrorKind,
        source_path: &Path,
        io_kind: io::ErrorKind,
    ) -> Self {
        Self {
            kind,
            source_path: source_path.to_path_buf(),
            location: None,
            io_kind: Some(io_kind),
        }
    }

    fn with_toml_source(
        kind: ConfigDocumentErrorKind,
        source_path: &Path,
        document: &str,
        source: &toml::de::Error,
    ) -> Self {
        let location = source
            .span()
            .and_then(|span| location_at(document, span.start));
        Self {
            kind,
            source_path: source_path.to_path_buf(),
            location,
            io_kind: None,
        }
    }
}

impl fmt::Debug for ConfigDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigDocumentError")
            .field("kind", &self.kind)
            .field("source_path", &"[redacted]")
            .field("location", &self.location)
            .field("io_kind", &self.io_kind)
            .finish()
    }
}

impl fmt::Display for ConfigDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.kind.message())
    }
}

impl Error for ConfigDocumentError {}

/// Reads and strictly deserializes one selected, exact-version TOML document.
pub fn load_config_document<T>(
    source_path: &Path,
    expectation: &ConfigDocumentExpectation,
) -> Result<T, ConfigDocumentError>
where
    T: DeserializeOwned,
{
    load_with_source(&FileDocumentSource, source_path, expectation)
}

trait DocumentSource {
    type Reader: Read;

    fn open_selected(&self, source_path: &Path) -> io::Result<Self::Reader>;
}

struct FileDocumentSource;

impl DocumentSource for FileDocumentSource {
    type Reader = File;

    fn open_selected(&self, source_path: &Path) -> io::Result<Self::Reader> {
        File::open(source_path)
    }
}

fn load_with_source<T, S>(
    source: &S,
    source_path: &Path,
    expectation: &ConfigDocumentExpectation,
) -> Result<T, ConfigDocumentError>
where
    T: DeserializeOwned,
    S: DocumentSource,
{
    let reader = source.open_selected(source_path).map_err(|error| {
        let kind = if error.kind() == io::ErrorKind::NotFound {
            ConfigDocumentErrorKind::NotFound
        } else {
            ConfigDocumentErrorKind::Read
        };
        ConfigDocumentError::with_io_kind(kind, source_path, error.kind())
    })?;
    let bytes = read_bounded(reader, source_path)?;
    let document = String::from_utf8(bytes).map_err(|_| {
        ConfigDocumentError::without_source(ConfigDocumentErrorKind::InvalidUtf8, source_path)
    })?;
    let header = document.parse::<toml::Table>().map_err(|error| {
        ConfigDocumentError::with_toml_source(
            ConfigDocumentErrorKind::MalformedToml,
            source_path,
            &document,
            &error,
        )
    })?;
    validate_header(&header, source_path, expectation)?;
    toml::from_str(&document).map_err(|error| {
        ConfigDocumentError::with_toml_source(
            ConfigDocumentErrorKind::TypedDocument,
            source_path,
            &document,
            &error,
        )
    })
}

fn read_bounded(reader: impl Read, source_path: &Path) -> Result<Vec<u8>, ConfigDocumentError> {
    let allocation = CONFIG_DOCUMENT_MAX_UTF8_BYTES + 1;
    let mut bytes = Vec::with_capacity(allocation);
    reader
        .take(allocation as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            ConfigDocumentError::with_io_kind(
                ConfigDocumentErrorKind::Read,
                source_path,
                error.kind(),
            )
        })?;
    if bytes.len() > CONFIG_DOCUMENT_MAX_UTF8_BYTES {
        return Err(ConfigDocumentError::without_source(
            ConfigDocumentErrorKind::TooLarge,
            source_path,
        ));
    }
    Ok(bytes)
}

fn validate_header(
    header: &toml::Table,
    source_path: &Path,
    expectation: &ConfigDocumentExpectation,
) -> Result<(), ConfigDocumentError> {
    let schema = header
        .get("schema")
        .ok_or_else(|| {
            ConfigDocumentError::without_source(ConfigDocumentErrorKind::MissingSchema, source_path)
        })?
        .as_str()
        .ok_or_else(|| {
            ConfigDocumentError::without_source(ConfigDocumentErrorKind::InvalidSchema, source_path)
        })?;
    if !valid_schema_id(schema) {
        return Err(ConfigDocumentError::without_source(
            ConfigDocumentErrorKind::InvalidSchema,
            source_path,
        ));
    }
    if schema != expectation.schema() {
        return Err(ConfigDocumentError::without_source(
            ConfigDocumentErrorKind::SchemaMismatch,
            source_path,
        ));
    }

    let version = header
        .get("schema_version")
        .ok_or_else(|| {
            ConfigDocumentError::without_source(
                ConfigDocumentErrorKind::MissingSchemaVersion,
                source_path,
            )
        })?
        .as_integer()
        .ok_or_else(|| {
            ConfigDocumentError::without_source(
                ConfigDocumentErrorKind::InvalidSchemaVersion,
                source_path,
            )
        })?;
    let version = u32::try_from(version).map_err(|_| {
        ConfigDocumentError::without_source(
            ConfigDocumentErrorKind::InvalidSchemaVersion,
            source_path,
        )
    })?;
    if version == 0 {
        return Err(ConfigDocumentError::without_source(
            ConfigDocumentErrorKind::InvalidSchemaVersion,
            source_path,
        ));
    }
    if version != expectation.schema_version() {
        return Err(ConfigDocumentError::without_source(
            ConfigDocumentErrorKind::UnsupportedSchemaVersion,
            source_path,
        ));
    }
    Ok(())
}

fn valid_schema_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= CONFIG_SCHEMA_ID_MAX_UTF8_BYTES
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn location_at(document: &str, byte_offset: usize) -> Option<ConfigDocumentLocation> {
    let prefix = document.get(..byte_offset)?;
    let line = u32::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count())
        .ok()?
        .checked_add(1)?;
    let column = u32::try_from(
        prefix
            .rsplit_once('\n')
            .map_or(prefix, |(_, tail)| tail)
            .chars()
            .count(),
    )
    .ok()?
    .checked_add(1)?;
    Some(ConfigDocumentLocation { line, column })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io::Cursor;

    use serde::Deserialize;
    use tempfile::tempdir;

    use super::*;

    const SOURCE_PATH: &str = "/private/config/secret-instance.toml";

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct StrictDocument {
        schema: String,
        schema_version: u32,
        service: StrictService,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct StrictService {
        instance: String,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct PermissiveDocument {
        schema: String,
        schema_version: u32,
    }

    fn expectation() -> ConfigDocumentExpectation {
        ConfigDocumentExpectation::new("radroots.example.config", 1).unwrap()
    }

    fn valid_document() -> &'static str {
        concat!(
            "schema = \"radroots.example.config\"\n",
            "schema_version = 1\n",
            "[service]\n",
            "instance = \"default\"\n",
        )
    }

    #[test]
    fn exact_document_is_bounded_and_strictly_deserialized() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("config.toml");
        std::fs::write(&path, valid_document()).unwrap();

        let loaded = load_config_document::<StrictDocument>(&path, &expectation()).unwrap();
        assert_eq!(loaded.schema, "radroots.example.config");
        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.service.instance, "default");

        let exact_padding = CONFIG_DOCUMENT_MAX_UTF8_BYTES - valid_document().len() - 2;
        let exact = format!("{}#{}\n", valid_document(), "x".repeat(exact_padding));
        assert_eq!(exact.len(), CONFIG_DOCUMENT_MAX_UTF8_BYTES);
        std::fs::write(&path, exact).unwrap();
        assert!(load_config_document::<StrictDocument>(&path, &expectation()).is_ok());

        std::fs::write(&path, vec![b'x'; CONFIG_DOCUMENT_MAX_UTF8_BYTES + 1]).unwrap();
        assert_eq!(
            load_config_document::<StrictDocument>(&path, &expectation())
                .unwrap_err()
                .kind(),
            ConfigDocumentErrorKind::TooLarge
        );
    }

    #[test]
    fn missing_unreadable_and_invalid_utf8_documents_fail_closed() {
        let directory = tempdir().unwrap();
        let missing = directory.path().join("missing.toml");
        assert_eq!(
            load_config_document::<StrictDocument>(&missing, &expectation())
                .unwrap_err()
                .kind(),
            ConfigDocumentErrorKind::NotFound
        );

        let invalid_utf8 = directory.path().join("invalid.toml");
        std::fs::write(&invalid_utf8, [0xff, 0xfe]).unwrap();
        assert_eq!(
            load_config_document::<StrictDocument>(&invalid_utf8, &expectation())
                .unwrap_err()
                .kind(),
            ConfigDocumentErrorKind::InvalidUtf8
        );

        struct FailingSource;
        impl DocumentSource for FailingSource {
            type Reader = Cursor<Vec<u8>>;

            fn open_selected(&self, _source_path: &Path) -> io::Result<Self::Reader> {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "sensitive"))
            }
        }
        let error = load_with_source::<StrictDocument, _>(
            &FailingSource,
            Path::new(SOURCE_PATH),
            &expectation(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ConfigDocumentErrorKind::Read);
        assert_eq!(error.io_kind(), Some(io::ErrorKind::PermissionDenied));
        assert!(error.source().is_none());
    }

    #[test]
    fn malformed_toml_and_typed_errors_retain_safe_locations() {
        let malformed = RecordingSource::new(b"token = \"secret-value\n".to_vec());
        let error = load_with_source::<StrictDocument, _>(
            &malformed,
            Path::new(SOURCE_PATH),
            &expectation(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ConfigDocumentErrorKind::MalformedToml);
        assert!(error.location().is_some());
        assert_public_error_chain_is_safe(&error, &[SOURCE_PATH, "secret-value", "token"]);

        let typed = RecordingSource::new(
            concat!(
                "schema = \"radroots.example.config\"\n",
                "schema_version = 1\n",
                "credential_secret = \"secret-value\"\n",
                "[service]\n",
                "instance = \"default\"\n",
            )
            .as_bytes()
            .to_vec(),
        );
        let error =
            load_with_source::<StrictDocument, _>(&typed, Path::new(SOURCE_PATH), &expectation())
                .unwrap_err();
        assert_eq!(error.kind(), ConfigDocumentErrorKind::TypedDocument);
        let location = error.location().unwrap();
        assert!(location.line() >= 3);
        assert!(location.column() >= 1);
        assert_eq!(error.trusted_source_path(), Path::new(SOURCE_PATH));
        assert_public_error_chain_is_safe(
            &error,
            &[SOURCE_PATH, "credential_secret", "secret-value"],
        );
        assert_eq!(
            error.to_string(),
            "configuration document fields are invalid"
        );
    }

    #[test]
    fn header_identity_is_required_valid_and_checked_before_typed_fields() {
        for (source, kind) in [
            (
                "schema_version = 1\n",
                ConfigDocumentErrorKind::MissingSchema,
            ),
            (
                "schema = 7\nschema_version = 1\n",
                ConfigDocumentErrorKind::InvalidSchema,
            ),
            (
                "schema = \"radroots.other.config\"\nschema_version = 1\nunknown = true\n",
                ConfigDocumentErrorKind::SchemaMismatch,
            ),
            (
                "schema = \"radroots.example.config\"\n",
                ConfigDocumentErrorKind::MissingSchemaVersion,
            ),
            (
                "schema = \"radroots.example.config\"\nschema_version = \"one\"\n",
                ConfigDocumentErrorKind::InvalidSchemaVersion,
            ),
            (
                "schema = \"radroots.example.config\"\nschema_version = 0\n",
                ConfigDocumentErrorKind::InvalidSchemaVersion,
            ),
            (
                "schema = \"radroots.example.config\"\nschema_version = 4294967296\n",
                ConfigDocumentErrorKind::InvalidSchemaVersion,
            ),
            (
                "schema = \"radroots.example.config\"\nschema_version = 2\n",
                ConfigDocumentErrorKind::UnsupportedSchemaVersion,
            ),
        ] {
            let source = RecordingSource::new(source.as_bytes().to_vec());
            assert_eq!(
                load_with_source::<StrictDocument, _>(
                    &source,
                    Path::new(SOURCE_PATH),
                    &expectation(),
                )
                .unwrap_err()
                .kind(),
                kind
            );
        }
    }

    #[test]
    fn unknown_field_policy_is_owned_by_the_exact_typed_document() {
        let source = RecordingSource::new(
            concat!(
                "schema = \"radroots.example.config\"\n",
                "schema_version = 1\n",
                "extension = true\n",
            )
            .as_bytes()
            .to_vec(),
        );
        let permissive = load_with_source::<PermissiveDocument, _>(
            &source,
            Path::new(SOURCE_PATH),
            &expectation(),
        )
        .unwrap();
        assert_eq!(permissive.schema, "radroots.example.config");
        assert_eq!(permissive.schema_version, 1);
    }

    #[test]
    fn expectation_validation_is_exact_and_bounded() {
        assert_eq!(
            ConfigDocumentExpectation::new("", 1).unwrap_err(),
            ConfigDocumentExpectationError::InvalidSchema
        );
        assert_eq!(
            ConfigDocumentExpectation::new(".radroots.config", 1).unwrap_err(),
            ConfigDocumentExpectationError::InvalidSchema
        );
        assert_eq!(
            ConfigDocumentExpectation::new("bad schema", 1).unwrap_err(),
            ConfigDocumentExpectationError::InvalidSchema
        );
        assert!(
            ConfigDocumentExpectation::new("a".repeat(CONFIG_SCHEMA_ID_MAX_UTF8_BYTES), 1).is_ok()
        );
        assert_eq!(
            ConfigDocumentExpectation::new("a".repeat(CONFIG_SCHEMA_ID_MAX_UTF8_BYTES + 1), 1,)
                .unwrap_err(),
            ConfigDocumentExpectationError::InvalidSchema
        );
        assert_eq!(
            ConfigDocumentExpectation::new("a".repeat(4 * 1024 * 1024), 1).unwrap_err(),
            ConfigDocumentExpectationError::InvalidSchema
        );
        assert_eq!(
            ConfigDocumentExpectation::new("radroots.example.config", 0).unwrap_err(),
            ConfigDocumentExpectationError::InvalidSchemaVersion
        );
    }

    struct RecordingSource {
        bytes: Vec<u8>,
        opened: RefCell<Vec<PathBuf>>,
    }

    impl RecordingSource {
        fn new(bytes: Vec<u8>) -> Self {
            Self {
                bytes,
                opened: RefCell::new(Vec::new()),
            }
        }
    }

    impl DocumentSource for RecordingSource {
        type Reader = Cursor<Vec<u8>>;

        fn open_selected(&self, source_path: &Path) -> io::Result<Self::Reader> {
            self.opened.borrow_mut().push(source_path.to_path_buf());
            Ok(Cursor::new(self.bytes.clone()))
        }
    }

    #[test]
    fn loader_performs_exactly_one_selected_read_through_its_only_io_capability() {
        let source = RecordingSource::new(valid_document().as_bytes().to_vec());
        let selected = Path::new(SOURCE_PATH);
        let loaded = load_with_source::<StrictDocument, _>(&source, selected, &expectation())
            .expect("selected document");

        assert_eq!(loaded.service.instance, "default");
        assert_eq!(source.opened.borrow().as_slice(), [selected]);
    }

    fn assert_public_error_chain_is_safe(error: &ConfigDocumentError, forbidden: &[&str]) {
        let mut rendered = format!("{error:?}\n{error}");
        let mut source = error.source();
        while let Some(current) = source {
            rendered.push_str(&format!("\n{current:?}\n{current}"));
            source = current.source();
        }
        for value in forbidden {
            assert!(!rendered.contains(value), "public error leaked `{value}`");
        }
    }
}

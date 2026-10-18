#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

use core::fmt;
use radroots_storage::event::SourceGeneration;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::Database;

#[cfg(all(feature = "workspace", target_arch = "wasm32"))]
pub mod workspace;

/// Maximum opaque record size; private codecs must enforce their lower limit.
pub const RECORD_BYTES_MAX: usize = 64 * 1024;
/// Maximum atomic batch width.
pub const BATCH_RECORDS_MAX: usize = 32;
/// Maximum retained input bytes across accepted work on one connection.
pub const LIVE_BYTES_MAX: usize = 4 * 1024 * 1024;

/// Fixed source-redacted host errors. No browser exception message is exposed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    InvalidInput,
    Unavailable,
    MissingNamespace,
    NamespaceExists,
    IncompatibleSchema,
    Closed,
    Capacity,
    Conflict,
    GuardRefused,
    CorruptRecord,
    Aborted,
    CommitUnknown,
}
impl Error {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::Unavailable => "indexeddb_unavailable",
            Self::MissingNamespace => "missing_namespace",
            Self::NamespaceExists => "namespace_exists",
            Self::IncompatibleSchema => "incompatible_schema",
            Self::Closed => "closed",
            Self::Capacity => "capacity_exhausted",
            Self::Conflict => "record_conflict",
            Self::GuardRefused => "transaction_precondition_refused",
            Self::CorruptRecord => "corrupt_record",
            Self::Aborted => "transaction_aborted",
            Self::CommitUnknown => "commit_unknown",
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl core::error::Error for Error {}

/// Exact new-format namespace identity; never inferred from missing data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Namespace(SourceGeneration);
impl Namespace {
    pub const fn new(generation: SourceGeneration) -> Self {
        Self(generation)
    }
    pub fn name(self) -> String {
        use core::fmt::Write;
        let mut name = String::from("radroots.indexeddb.v1.");
        for byte in self.0.as_bytes() {
            let _ = write!(name, "{byte:02x}");
        }
        name
    }
}

/// Explicit first creation or exact existing-namespace opening.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenMode {
    CreateNew,
    Existing,
}

/// Opaque exact record identity, separate from any authorization claim.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RecordKey([u8; 32]);
impl RecordKey {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// One compare/exchange entry. None denotes absence/deletion; empty is data.
/// Debug intentionally omits all bytes and identities.
pub struct Mutation {
    key: RecordKey,
    expected: Option<Vec<u8>>,
    replacement: Option<Vec<u8>>,
}
impl Mutation {
    pub fn new(
        key: RecordKey,
        expected: Option<Vec<u8>>,
        replacement: Option<Vec<u8>>,
    ) -> Result<Self, Error> {
        if expected
            .as_ref()
            .is_some_and(|v| v.len() > RECORD_BYTES_MAX)
            || replacement
                .as_ref()
                .is_some_and(|v| v.len() > RECORD_BYTES_MAX)
        {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            key,
            expected,
            replacement,
        })
    }
    fn bytes(&self) -> usize {
        32 + self.expected.as_ref().map_or(0, Vec::len)
            + self.replacement.as_ref().map_or(0, Vec::len)
    }
}
impl fmt::Debug for Mutation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Mutation(<redacted>)")
    }
}

/// Validated bounded batch; duplicate keys are rejected before host work.
pub struct Batch {
    entries: Vec<Mutation>,
    bytes: usize,
}
impl Batch {
    pub fn new(entries: Vec<Mutation>) -> Result<Self, Error> {
        if entries.is_empty() || entries.len() > BATCH_RECORDS_MAX {
            return Err(Error::InvalidInput);
        }
        let mut keys = std::collections::BTreeSet::new();
        let mut bytes = 0usize;
        for entry in &entries {
            if !keys.insert(entry.key) {
                return Err(Error::InvalidInput);
            }
            bytes = bytes
                .checked_add(entry.bytes())
                .ok_or(Error::InvalidInput)?;
        }
        if bytes > LIVE_BYTES_MAX {
            return Err(Error::Capacity);
        }
        Ok(Self { entries, bytes })
    }
    pub const fn retained_bytes(&self) -> usize {
        self.bytes
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
impl fmt::Debug for Batch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Batch(<redacted>)")
    }
}

/// Witness returned only after actual IndexedDB transaction completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Commit {
    records: usize,
}
impl Commit {
    pub const fn records(self) -> usize {
        self.records
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BATCH_RECORDS_MAX, Batch, Error, LIVE_BYTES_MAX, Mutation, Namespace, RECORD_BYTES_MAX,
        RecordKey,
    };
    use radroots_storage::event::SourceGeneration;
    #[test]
    fn exact_namespace_and_bounded_redacted_batches() {
        let ns = Namespace::new(SourceGeneration::new([1; 32]).unwrap());
        assert_eq!(
            ns.name(),
            format!("radroots.indexeddb.v1.{}", "01".repeat(32))
        );
        let entry = |n, size| Mutation::new(RecordKey::new([n; 32]), None, Some(vec![7; size]));
        assert_eq!(RecordKey::new([2; 32]).as_bytes(), &[2; 32]);
        assert_eq!(
            entry(1, RECORD_BYTES_MAX + 1).unwrap_err(),
            Error::InvalidInput
        );
        assert_eq!(
            Mutation::new(
                RecordKey::new([1; 32]),
                Some(vec![0; RECORD_BYTES_MAX + 1]),
                None
            )
            .unwrap_err(),
            Error::InvalidInput
        );
        assert_eq!(Batch::new(vec![]).unwrap_err(), Error::InvalidInput);
        assert_eq!(
            Batch::new(
                (0..=BATCH_RECORDS_MAX)
                    .map(|n| entry(n as u8, 0).unwrap())
                    .collect()
            )
            .unwrap_err(),
            Error::InvalidInput
        );
        assert_eq!(
            Batch::new(vec![entry(1, 0).unwrap(), entry(1, 1).unwrap()]).unwrap_err(),
            Error::InvalidInput
        );
        let maximum = Batch::new(
            (0..BATCH_RECORDS_MAX)
                .map(|n| {
                    Mutation::new(
                        RecordKey::new([n as u8; 32]),
                        Some(vec![0; RECORD_BYTES_MAX]),
                        Some(vec![1; RECORD_BYTES_MAX]),
                    )
                    .unwrap()
                })
                .collect(),
        );
        assert_eq!(maximum.unwrap_err(), Error::Capacity);
        let batch = Batch::new(vec![
            entry(1, RECORD_BYTES_MAX).unwrap(),
            Mutation::new(RecordKey::new([2; 32]), Some(vec![]), None).unwrap(),
        ])
        .unwrap();
        assert!(!batch.is_empty());
        assert_eq!(batch.len(), 2);
        assert_eq!(batch.retained_bytes(), RECORD_BYTES_MAX + 64);
        assert!(batch.retained_bytes() < LIVE_BYTES_MAX);
        assert_eq!(format!("{batch:?}"), "Batch(<redacted>)");
        assert_eq!(
            format!("{:?}", entry(4, 1).unwrap()),
            "Mutation(<redacted>)"
        );
        for error in [
            Error::InvalidInput,
            Error::Unavailable,
            Error::MissingNamespace,
            Error::NamespaceExists,
            Error::IncompatibleSchema,
            Error::Closed,
            Error::Capacity,
            Error::Conflict,
            Error::GuardRefused,
            Error::CorruptRecord,
            Error::Aborted,
            Error::CommitUnknown,
        ] {
            assert_eq!(error.to_string(), error.code());
        }
    }
}

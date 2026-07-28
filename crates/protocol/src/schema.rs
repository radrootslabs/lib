//! Validated schema identities and deterministic module dispatch.

use alloc::{string::String, vec::Vec};
use core::{fmt, str::FromStr};

/// Maximum UTF-8 byte length accepted for a schema identifier.
pub const MAX_SCHEMA_ID_BYTES: usize = 255;

/// Passive metadata that preserves an externally governed schema identity.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Metadata {
    /// Historical generated type name bound by the schema contract.
    pub type_name: &'static str,
    /// Canonical schema identifier.
    pub schema_id: &'static str,
    /// Declared schema generation.
    pub schema_version: u16,
}

/// A canonical, version-suffixed schema identifier.
///
/// Schema identifiers contain one or more dot-separated namespace segments
/// followed by a positive canonical version segment such as `v1`. Namespace
/// segments begin with a lowercase ASCII letter and contain only lowercase
/// ASCII letters, digits, and underscores.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SchemaId {
    value: String,
    version: u16,
}

impl SchemaId {
    /// Parses and validates a schema identifier.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.is_empty() {
            return Err(Error::EmptySchemaId);
        }
        if value.len() > MAX_SCHEMA_ID_BYTES {
            return Err(Error::SchemaIdTooLong {
                actual: value.len(),
                max: MAX_SCHEMA_ID_BYTES,
            });
        }

        let (namespace, version_segment) = value
            .rsplit_once('.')
            .ok_or(Error::MissingSchemaNamespace)?;
        if namespace.is_empty() {
            return Err(Error::MissingSchemaNamespace);
        }
        for (index, segment) in namespace.split('.').enumerate() {
            if !valid_namespace_segment(segment) {
                return Err(Error::InvalidSchemaNamespaceSegment { index });
            }
        }

        let version = parse_version_segment(version_segment)?;
        Ok(Self { value, version })
    }

    /// Returns the canonical identifier text.
    pub fn as_str(&self) -> &str {
        self.value.as_str()
    }

    /// Returns the positive schema generation encoded by the final segment.
    pub const fn version(&self) -> u16 {
        self.version
    }
}

impl AsRef<str> for SchemaId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SchemaId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for SchemaId {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for SchemaId {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for SchemaId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

/// A supported versioned module in the protocol package.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ModuleVersion {
    /// `capability::v1`.
    CapabilityV1,
    /// `error::v1`.
    ErrorV1,
    /// `event::v1`.
    EventV1,
    /// `runtime::v1`.
    RuntimeV1,
    /// `radrootsd::transport_publish::v5`.
    RadrootsdTransportPublishV5,
}

impl ModuleVersion {
    /// Every module generation supported by this package version.
    pub const ALL: [Self; 5] = [
        Self::CapabilityV1,
        Self::ErrorV1,
        Self::EventV1,
        Self::RuntimeV1,
        Self::RadrootsdTransportPublishV5,
    ];

    /// Returns the stable Rust module path relative to `radroots_protocol`.
    pub const fn path(self) -> &'static str {
        match self {
            Self::CapabilityV1 => "capability::v1",
            Self::ErrorV1 => "error::v1",
            Self::EventV1 => "event::v1",
            Self::RuntimeV1 => "runtime::v1",
            Self::RadrootsdTransportPublishV5 => "radrootsd::transport_publish::v5",
        }
    }

    /// Returns the explicit contract generation for the module.
    pub const fn generation(self) -> u16 {
        match self {
            Self::CapabilityV1 | Self::ErrorV1 | Self::EventV1 | Self::RuntimeV1 => 1,
            Self::RadrootsdTransportPublishV5 => 5,
        }
    }
}

/// One validated schema-to-module registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Descriptor {
    id: SchemaId,
    module: ModuleVersion,
}

impl Descriptor {
    /// Creates a registration after validating its schema identifier.
    pub fn try_new(id: impl Into<String>, module: ModuleVersion) -> Result<Self, Error> {
        Ok(Self {
            id: SchemaId::parse(id)?,
            module,
        })
    }

    /// Returns the schema identifier.
    pub const fn id(&self) -> &SchemaId {
        &self.id
    }

    /// Returns the module generation that owns the schema.
    pub const fn module(&self) -> ModuleVersion {
        self.module
    }
}

/// A canonical registry of unique schema identifiers.
///
/// Construction sorts descriptors by schema identifier so iteration and
/// lookup remain deterministic regardless of input order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Registry {
    descriptors: Vec<Descriptor>,
}

impl Registry {
    /// Builds a canonical registry and rejects duplicate schema identifiers.
    pub fn try_new(descriptors: impl IntoIterator<Item = Descriptor>) -> Result<Self, Error> {
        let mut descriptors: Vec<_> = descriptors.into_iter().collect();
        descriptors.sort_by(|left, right| left.id.cmp(&right.id));

        for adjacent in descriptors.windows(2) {
            if adjacent[0].id == adjacent[1].id {
                return Err(Error::DuplicateSchemaId {
                    schema_id: adjacent[0].id.as_str().into(),
                });
            }
        }

        Ok(Self { descriptors })
    }

    /// Builds a registry from governed schema metadata and module ownership.
    pub fn try_from_metadata(
        entries: impl IntoIterator<Item = (Metadata, ModuleVersion)>,
    ) -> Result<Self, Error> {
        let descriptors = entries
            .into_iter()
            .map(|(metadata, module)| {
                let descriptor = Descriptor::try_new(metadata.schema_id, module)?;
                let encoded = descriptor.id().version();
                if metadata.schema_version != encoded {
                    return Err(Error::SchemaVersionMismatch {
                        schema_id: metadata.schema_id.into(),
                        declared: metadata.schema_version,
                        encoded,
                    });
                }
                Ok(descriptor)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new(descriptors)
    }

    /// Returns the canonical descriptor sequence.
    pub fn descriptors(&self) -> &[Descriptor] {
        self.descriptors.as_slice()
    }

    /// Returns the number of registered schemas.
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// Reports whether the registry contains no schemas.
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }

    /// Returns the descriptor for an exact schema identifier.
    pub fn descriptor(&self, id: &SchemaId) -> Option<&Descriptor> {
        self.descriptors
            .binary_search_by(|descriptor| descriptor.id.cmp(id))
            .ok()
            .map(|index| &self.descriptors[index])
    }

    /// Dispatches an exact schema identifier to its owning module generation.
    pub fn module_for(&self, id: &SchemaId) -> Option<ModuleVersion> {
        self.descriptor(id).map(Descriptor::module)
    }
}

/// Builds the complete protocol V1 schema registry currently owned here.
pub fn protocol_v1_registry() -> Result<Registry, Error> {
    let capability = crate::capability::v1::SCHEMAS
        .iter()
        .copied()
        .map(|metadata| (metadata, ModuleVersion::CapabilityV1));
    let event = crate::event::v1::SCHEMAS
        .iter()
        .copied()
        .map(|metadata| (metadata, ModuleVersion::EventV1));
    let mut descriptors = Registry::try_from_metadata(capability.chain(event))?
        .descriptors()
        .to_vec();
    descriptors.extend(
        crate::runtime::v1::schema_registry()?
            .descriptors()
            .iter()
            .cloned(),
    );
    Registry::try_new(descriptors)
}

/// Schema identity or registry validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The schema identifier is empty.
    EmptySchemaId,
    /// The schema identifier exceeds the byte-length limit.
    SchemaIdTooLong {
        /// Actual UTF-8 byte length.
        actual: usize,
        /// Maximum accepted UTF-8 byte length.
        max: usize,
    },
    /// The schema identifier does not include a namespace before its version.
    MissingSchemaNamespace,
    /// A namespace segment is empty or noncanonical.
    InvalidSchemaNamespaceSegment {
        /// Zero-based namespace segment index.
        index: usize,
    },
    /// The final segment is not a canonical positive `vN` generation.
    InvalidSchemaVersion,
    /// Metadata declares a generation different from the schema ID suffix.
    SchemaVersionMismatch {
        /// Canonical schema identifier.
        schema_id: String,
        /// Generation stored in metadata.
        declared: u16,
        /// Generation encoded in the schema ID.
        encoded: u16,
    },
    /// The registry contains an identifier more than once.
    DuplicateSchemaId {
        /// Duplicated canonical schema identifier.
        schema_id: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySchemaId => formatter.write_str("schema id must not be empty"),
            Self::SchemaIdTooLong { actual, max } => {
                write!(formatter, "schema id length {actual} exceeds {max} bytes")
            }
            Self::MissingSchemaNamespace => {
                formatter.write_str("schema id must contain a namespace and version")
            }
            Self::InvalidSchemaNamespaceSegment { index } => {
                write!(formatter, "schema id namespace segment {index} is invalid")
            }
            Self::InvalidSchemaVersion => {
                formatter.write_str("schema id version must be canonical positive vN")
            }
            Self::SchemaVersionMismatch {
                schema_id,
                declared,
                encoded,
            } => write!(
                formatter,
                "schema id {schema_id} encodes v{encoded} but metadata declares v{declared}"
            ),
            Self::DuplicateSchemaId { schema_id } => {
                write!(formatter, "duplicate schema id {schema_id}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

fn valid_namespace_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn parse_version_segment(segment: &str) -> Result<u16, Error> {
    let Some(digits) = segment.strip_prefix('v') else {
        return Err(Error::InvalidSchemaVersion);
    };
    if digits.is_empty()
        || digits.starts_with('0')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::InvalidSchemaVersion);
    }
    digits
        .parse::<u16>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or(Error::InvalidSchemaVersion)
}

#[cfg(test)]
mod tests {
    use alloc::{string::ToString, vec};
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn schema_id_parsing_accepts_existing_contract_shape() {
        let id = SchemaId::parse("radroots.protocol.transport_kind.v1").expect("schema id");
        assert_eq!(id.as_str(), "radroots.protocol.transport_kind.v1");
        assert_eq!(id.version(), 1);
        assert_eq!(id.to_string(), id.as_str());
        assert_eq!(id, id.as_str().parse().expect("FromStr schema id"));
    }

    #[test]
    fn schema_id_parsing_rejects_every_noncanonical_shape() {
        let too_long = alloc::format!("{}.v1", "a".repeat(MAX_SCHEMA_ID_BYTES));
        for (value, expected) in [
            ("", Error::EmptySchemaId),
            ("v1", Error::MissingSchemaNamespace),
            (".v1", Error::MissingSchemaNamespace),
            (
                "radroots..event.v1",
                Error::InvalidSchemaNamespaceSegment { index: 1 },
            ),
            (
                "Radroots.protocol.event.v1",
                Error::InvalidSchemaNamespaceSegment { index: 0 },
            ),
            (
                "radroots.protocol.event-name.v1",
                Error::InvalidSchemaNamespaceSegment { index: 2 },
            ),
            ("radroots.protocol.event", Error::InvalidSchemaVersion),
            ("radroots.protocol.event.v0", Error::InvalidSchemaVersion),
            ("radroots.protocol.event.v01", Error::InvalidSchemaVersion),
            (
                "radroots.protocol.event.v65536",
                Error::InvalidSchemaVersion,
            ),
        ] {
            assert_eq!(SchemaId::parse(value), Err(expected), "{value}");
        }
        assert_eq!(
            SchemaId::parse(too_long),
            Err(Error::SchemaIdTooLong {
                actual: MAX_SCHEMA_ID_BYTES + 3,
                max: MAX_SCHEMA_ID_BYTES,
            })
        );
    }

    #[test]
    fn module_inventory_has_unique_paths_and_explicit_generations() {
        let paths = ModuleVersion::ALL
            .into_iter()
            .map(ModuleVersion::path)
            .collect::<BTreeSet<_>>();
        assert_eq!(paths.len(), ModuleVersion::ALL.len());
        assert_eq!(ModuleVersion::CapabilityV1.generation(), 1);
        assert_eq!(ModuleVersion::ErrorV1.generation(), 1);
        assert_eq!(ModuleVersion::EventV1.generation(), 1);
        assert_eq!(ModuleVersion::RuntimeV1.generation(), 1);
        assert_eq!(ModuleVersion::RadrootsdTransportPublishV5.generation(), 5);
    }

    #[test]
    fn registry_is_canonical_unique_and_dispatches_exact_ids() {
        let event = Descriptor::try_new(
            "radroots.protocol.event_descriptor.v1",
            ModuleVersion::EventV1,
        )
        .expect("event descriptor");
        let capability = Descriptor::try_new(
            "radroots.protocol.transport_kind.v1",
            ModuleVersion::CapabilityV1,
        )
        .expect("capability descriptor");
        let registry = Registry::try_new(vec![event, capability.clone()]).expect("registry");

        assert_eq!(registry.len(), 2);
        assert!(!registry.is_empty());
        assert_eq!(
            registry
                .descriptors()
                .iter()
                .map(|descriptor| descriptor.id().as_str())
                .collect::<Vec<_>>(),
            vec![
                "radroots.protocol.event_descriptor.v1",
                "radroots.protocol.transport_kind.v1",
            ]
        );
        assert_eq!(
            registry.module_for(capability.id()),
            Some(ModuleVersion::CapabilityV1)
        );
        let unknown = SchemaId::parse("radroots.protocol.unknown.v1").expect("unknown id");
        assert_eq!(registry.module_for(&unknown), None);
    }

    #[test]
    fn registry_rejects_duplicate_schema_ids() {
        let first = Descriptor::try_new(
            "radroots.protocol.event_descriptor.v1",
            ModuleVersion::EventV1,
        )
        .expect("first descriptor");
        let second = Descriptor::try_new(
            "radroots.protocol.event_descriptor.v1",
            ModuleVersion::CapabilityV1,
        )
        .expect("second descriptor");
        assert_eq!(
            Registry::try_new(vec![first, second]),
            Err(Error::DuplicateSchemaId {
                schema_id: "radroots.protocol.event_descriptor.v1".into(),
            })
        );
    }

    #[test]
    fn metadata_registry_rejects_version_mismatch() {
        let metadata = Metadata {
            type_name: "EventDescriptor",
            schema_id: "radroots.protocol.event_descriptor.v1",
            schema_version: 2,
        };
        assert_eq!(
            Registry::try_from_metadata([(metadata, ModuleVersion::EventV1)]),
            Err(Error::SchemaVersionMismatch {
                schema_id: metadata.schema_id.into(),
                declared: 2,
                encoded: 1,
            })
        );
    }

    #[test]
    fn protocol_v1_registry_dispatches_all_migrated_schemas() {
        let registry = protocol_v1_registry().expect("protocol V1 registry");
        assert_eq!(registry.len(), 5 + crate::runtime::v1::CATALOG.len() * 2);
        for descriptor in registry.descriptors() {
            let expected = if descriptor.id().as_str().starts_with("radroots.runtime.") {
                ModuleVersion::RuntimeV1
            } else if descriptor.id().as_str().contains("event_descriptor")
                || descriptor.id().as_str().contains("trade_state")
            {
                ModuleVersion::EventV1
            } else {
                ModuleVersion::CapabilityV1
            };
            assert_eq!(registry.module_for(descriptor.id()), Some(expected));
        }
    }
}

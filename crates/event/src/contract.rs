#![forbid(unsafe_code)]

#[cfg(all(not(feature = "std"), not(test)))]
use alloc::{string::String, vec::Vec};
#[cfg(any(feature = "std", test))]
use std::{string::String, vec::Vec};

use core::fmt;

/// Exact package version implementing this event contract surface.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[doc(hidden)]
pub mod registry_v7;

pub use registry_v7::*;

/// Maximum UTF-8 byte length of a versioned Radroots event-contract ID.
pub const CONTRACT_ID_MAX_BYTES: usize = 255;

/// A syntactically valid, bounded Radroots event-contract identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractId(String);

impl ContractId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContractIdentityError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ContractIdentityError::ContractIdMissing);
        }
        if value.len() > CONTRACT_ID_MAX_BYTES {
            return Err(ContractIdentityError::ContractIdTooLong {
                max: CONTRACT_ID_MAX_BYTES,
                actual: value.len(),
            });
        }
        if !value.starts_with("radroots.") {
            return Err(ContractIdentityError::ContractIdNamespace);
        }
        let mut segments = value.split('.');
        let namespace = segments.next();
        let remaining = segments.collect::<Vec<_>>();
        let Some(version) = remaining.last() else {
            return Err(ContractIdentityError::ContractIdSyntax);
        };
        if namespace != Some("radroots")
            || remaining.len() < 3
            || remaining[..remaining.len() - 1].iter().any(|segment| {
                segment.is_empty()
                    || !segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            })
            || !version.strip_prefix('v').is_some_and(|number| {
                !number.is_empty()
                    && !number.starts_with('0')
                    && number.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return Err(ContractIdentityError::ContractIdSyntax);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for ContractId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A nonzero immutable event-contract registry version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistryVersion(u32);

impl RegistryVersion {
    pub const CURRENT: Self = Self(RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION);

    pub const fn new(value: u32) -> Result<Self, ContractIdentityError> {
        if value == 0 {
            return Err(ContractIdentityError::RegistryVersionZero);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for RegistryVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.get())
    }
}

/// One event contract resolved against an immutable historical registry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractKey {
    registry_version: RegistryVersion,
    contract_id: ContractId,
}

impl ContractKey {
    pub fn current(contract_id: impl Into<String>) -> Result<Self, ContractIdentityError> {
        Self::new(RegistryVersion::CURRENT, ContractId::parse(contract_id)?)
    }

    pub fn new(
        registry_version: RegistryVersion,
        contract_id: ContractId,
    ) -> Result<Self, ContractIdentityError> {
        if registry_version != RegistryVersion::CURRENT {
            return Err(ContractIdentityError::UnsupportedRegistryVersion {
                actual: registry_version.get(),
            });
        }
        if event_contract_registry_v7(contract_id.as_str()).is_none() {
            return Err(ContractIdentityError::UnknownContract {
                contract_id: contract_id.into_string(),
            });
        }
        Ok(Self {
            registry_version,
            contract_id,
        })
    }

    #[must_use]
    pub const fn registry_version(&self) -> RegistryVersion {
        self.registry_version
    }

    #[must_use]
    pub const fn contract_id(&self) -> &ContractId {
        &self.contract_id
    }

    #[must_use]
    pub fn contract(&self) -> &'static EventContract {
        event_contract_registry_v7(self.contract_id.as_str())
            .expect("validated registry-v7 contract key must remain resolvable")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractIdentityError {
    ContractIdMissing,
    ContractIdTooLong { max: usize, actual: usize },
    ContractIdNamespace,
    ContractIdSyntax,
    RegistryVersionZero,
    UnsupportedRegistryVersion { actual: u32 },
    UnknownContract { contract_id: String },
}

impl ContractIdentityError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContractIdMissing => "contract_id_missing",
            Self::ContractIdTooLong { .. } => "contract_id_too_long",
            Self::ContractIdNamespace => "contract_id_namespace",
            Self::ContractIdSyntax => "contract_id_syntax",
            Self::RegistryVersionZero => "registry_version_zero",
            Self::UnsupportedRegistryVersion { .. } => "unsupported_registry_version",
            Self::UnknownContract { .. } => "unknown_contract",
        }
    }
}

impl fmt::Display for ContractIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContractIdMissing => formatter.write_str("event contract ID must not be empty"),
            Self::ContractIdTooLong { max, actual } => {
                write!(
                    formatter,
                    "event contract ID is {actual} bytes; max is {max}"
                )
            }
            Self::ContractIdNamespace => {
                formatter.write_str("event contract ID must use the `radroots.` namespace")
            }
            Self::ContractIdSyntax => {
                formatter.write_str("event contract ID must be a canonical versioned identifier")
            }
            Self::RegistryVersionZero => {
                formatter.write_str("event contract registry version must be nonzero")
            }
            Self::UnsupportedRegistryVersion { actual } => {
                write!(
                    formatter,
                    "unsupported event contract registry version {actual}"
                )
            }
            Self::UnknownContract { contract_id } => {
                write!(formatter, "unknown event contract `{contract_id}`")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ContractIdentityError {}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for ContractId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for ContractId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for RegistryVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(self.get())
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for RegistryVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::new(u32::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for ContractKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("ContractKey", 2)?;
        state.serialize_field("registry_version", &self.registry_version)?;
        state.serialize_field("contract_id", &self.contract_id)?;
        state.end()
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for ContractKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ContractKeySerde {
            registry_version: RegistryVersion,
            contract_id: ContractId,
        }

        let value = ContractKeySerde::deserialize(deserializer)?;
        Self::new(value.registry_version, value.contract_id).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn contract_identity_values_are_bounded_canonical_and_resolved() {
        let id = ContractId::parse("radroots.social.geochat.v1").expect("contract ID");
        assert_eq!(id.as_str(), "radroots.social.geochat.v1");
        assert_eq!(RegistryVersion::CURRENT.get(), 7);
        let key = ContractKey::new(RegistryVersion::CURRENT, id).expect("contract key");
        assert_eq!(key.contract().id, "radroots.social.geochat.v1");

        for invalid in [
            "",
            "social.geochat.v1",
            "radroots.social.Geochat.v1",
            "radroots.social.geochat",
            "radroots.social.geochat.v01",
            "radroots..geochat.v1",
        ] {
            assert!(ContractId::parse(invalid).is_err(), "{invalid}");
        }
        assert!(ContractId::parse(format!("radroots.{}.event.v1", "x".repeat(256))).is_err());
        assert_eq!(
            RegistryVersion::new(0),
            Err(ContractIdentityError::RegistryVersionZero)
        );
        assert!(matches!(
            ContractKey::new(
                RegistryVersion::new(8).expect("nonzero version"),
                ContractId::parse("radroots.social.geochat.v1").expect("contract ID")
            ),
            Err(ContractIdentityError::UnsupportedRegistryVersion { actual: 8 })
        ));
        assert!(matches!(
            ContractKey::current("radroots.social.unknown.v1"),
            Err(ContractIdentityError::UnknownContract { .. })
        ));
    }

    #[test]
    fn serde_reconstruction_revalidates_identity_and_registry_membership() {
        let key = ContractKey::current("radroots.social.geochat.v1").expect("contract key");
        let value = serde_json::to_value(&key).expect("key JSON");
        assert_eq!(
            serde_json::from_value::<ContractKey>(value.clone()).expect("decoded key"),
            key
        );

        let mut invalid = value.clone();
        invalid["registry_version"] = serde_json::json!(0);
        assert!(serde_json::from_value::<ContractKey>(invalid).is_err());
        let mut invalid = value.clone();
        invalid["registry_version"] = serde_json::json!(8);
        assert!(serde_json::from_value::<ContractKey>(invalid).is_err());
        let mut invalid = value.clone();
        invalid["contract_id"] = serde_json::json!("radroots.social.unknown.v1");
        assert!(serde_json::from_value::<ContractKey>(invalid).is_err());
        let mut invalid = value;
        invalid["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<ContractKey>(invalid).is_err());
    }
}

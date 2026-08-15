//! Validated identifiers for canonical service-instance paths.

use core::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Maximum encoded length of a service identifier.
pub const SERVICE_ID_MAX_BYTES: usize = 128;

/// Maximum encoded length of an instance identifier.
pub const INSTANCE_ID_MAX_BYTES: usize = 128;

/// Identifies which service-instance path component failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceIdentityKind {
    Service,
    Instance,
}

impl fmt::Display for ServiceIdentityKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service => formatter.write_str("service"),
            Self::Instance => formatter.write_str("instance"),
        }
    }
}

/// Validation failure for a service or instance identifier.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ServiceIdentityError {
    #[error("{kind} identifier must not be empty")]
    Empty { kind: ServiceIdentityKind },
    #[error("{kind} identifier exceeds its {maximum}-byte limit")]
    TooLong {
        kind: ServiceIdentityKind,
        maximum: usize,
    },
    #[error("{kind} identifier must start and end with a lowercase ASCII letter or digit")]
    InvalidBoundary { kind: ServiceIdentityKind },
    #[error("{kind} identifier contains a forbidden character")]
    InvalidCharacter { kind: ServiceIdentityKind },
}

fn validate(
    value: &str,
    kind: ServiceIdentityKind,
    maximum: usize,
) -> Result<(), ServiceIdentityError> {
    if value.is_empty() {
        return Err(ServiceIdentityError::Empty { kind });
    }
    if value.len() > maximum {
        return Err(ServiceIdentityError::TooLong { kind, maximum });
    }

    let is_alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    let bytes = value.as_bytes();
    if !is_alphanumeric(bytes[0]) || !is_alphanumeric(bytes[bytes.len() - 1]) {
        return Err(ServiceIdentityError::InvalidBoundary { kind });
    }
    if !bytes
        .iter()
        .all(|byte| is_alphanumeric(*byte) || matches!(*byte, b'-' | b'_'))
    {
        return Err(ServiceIdentityError::InvalidCharacter { kind });
    }

    Ok(())
}

macro_rules! service_identity {
    ($name:ident, $kind:expr, $maximum:ident) => {
        #[doc = concat!("A validated canonical ", stringify!($name), " path component.")]
        #[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// Parses and validates a canonical identifier.
            pub fn new(value: impl AsRef<str>) -> Result<Self, ServiceIdentityError> {
                let value = value.as_ref();
                validate(value, $kind, $maximum)?;
                Ok(Self(value.to_owned()))
            }

            fn from_string(value: String) -> Result<Self, ServiceIdentityError> {
                validate(&value, $kind, $maximum)?;
                Ok(Self(value))
            }

            /// Returns the canonical identifier text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            /// Consumes the identifier and returns its canonical text.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = ServiceIdentityError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = ServiceIdentityError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::from_string(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.into_string()
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct Visitor;

                impl<'de> serde::de::Visitor<'de> for Visitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a bounded canonical service identity")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        $name::new(value).map_err(E::custom)
                    }

                    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        $name::from_string(value).map_err(E::custom)
                    }
                }

                deserializer.deserialize_str(Visitor)
            }
        }
    };
}

service_identity!(
    ServiceId,
    ServiceIdentityKind::Service,
    SERVICE_ID_MAX_BYTES
);
service_identity!(
    InstanceId,
    ServiceIdentityKind::Instance,
    INSTANCE_ID_MAX_BYTES
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_accept_exact_boundaries_and_display_canonically() {
        for service in ["a", "myc", "farm_service", "service-01"] {
            let id = ServiceId::new(service).expect("valid service id");
            assert_eq!(id.as_str(), service);
            assert_eq!(id.to_string(), service);
        }
        for instance in ["0", "default", "north_farm", "west-01"] {
            let id = InstanceId::new(instance).expect("valid instance id");
            assert_eq!(id.as_str(), instance);
            assert_eq!(id.to_string(), instance);
        }

        assert!(ServiceId::new("a".repeat(SERVICE_ID_MAX_BYTES)).is_ok());
        assert!(InstanceId::new("a".repeat(INSTANCE_ID_MAX_BYTES)).is_ok());
    }

    #[test]
    fn identifiers_reject_empty_overlong_and_noncanonical_text() {
        assert_eq!(
            ServiceId::new(""),
            Err(ServiceIdentityError::Empty {
                kind: ServiceIdentityKind::Service
            })
        );
        assert_eq!(
            InstanceId::new(""),
            Err(ServiceIdentityError::Empty {
                kind: ServiceIdentityKind::Instance
            })
        );
        assert_eq!(
            ServiceId::new("a".repeat(SERVICE_ID_MAX_BYTES + 1)),
            Err(ServiceIdentityError::TooLong {
                kind: ServiceIdentityKind::Service,
                maximum: SERVICE_ID_MAX_BYTES,
            })
        );
        assert_eq!(
            InstanceId::new("a".repeat(INSTANCE_ID_MAX_BYTES + 1)),
            Err(ServiceIdentityError::TooLong {
                kind: ServiceIdentityKind::Instance,
                maximum: INSTANCE_ID_MAX_BYTES,
            })
        );
        let very_large = "a".repeat(4 * 1024 * 1024);
        assert!(matches!(
            ServiceId::new(&very_large),
            Err(ServiceIdentityError::TooLong {
                kind: ServiceIdentityKind::Service,
                maximum: SERVICE_ID_MAX_BYTES,
            })
        ));
        assert!(matches!(
            InstanceId::new(&very_large),
            Err(ServiceIdentityError::TooLong {
                kind: ServiceIdentityKind::Instance,
                maximum: INSTANCE_ID_MAX_BYTES,
            })
        ));

        let service_json = serde_json::to_string(&very_large).expect("large service JSON");
        assert!(serde_json::from_str::<ServiceId>(&service_json).is_err());
        let instance_json = serde_json::to_string(&very_large).expect("large instance JSON");
        assert!(serde_json::from_str::<InstanceId>(&instance_json).is_err());

        for invalid in ["Myc", "café", "a.b", "a:b", "a b", "a%b", "a/b", r"a\b"] {
            assert!(ServiceId::new(invalid).is_err(), "accepted `{invalid}`");
            assert!(InstanceId::new(invalid).is_err(), "accepted `{invalid}`");
        }
        for invalid in ["-a", "a-", "_a", "a_"] {
            assert!(ServiceId::new(invalid).is_err(), "accepted `{invalid}`");
            assert!(InstanceId::new(invalid).is_err(), "accepted `{invalid}`");
        }
    }

    #[test]
    fn identifiers_reject_traversal_and_separators() {
        for invalid in [".", "..", "../a", "a/../b", r"..\a", "%2e%2e", "a//b"] {
            assert!(ServiceId::new(invalid).is_err(), "accepted `{invalid}`");
            assert!(InstanceId::new(invalid).is_err(), "accepted `{invalid}`");
        }
    }

    #[test]
    fn serde_round_trips_revalidate_identifiers() {
        let service = ServiceId::new("myc").expect("service id");
        let encoded = serde_json::to_string(&service).expect("serialize service id");
        assert_eq!(encoded, "\"myc\"");
        assert_eq!(
            serde_json::from_str::<ServiceId>(&encoded).expect("deserialize service id"),
            service
        );

        let instance = InstanceId::new("default-01").expect("instance id");
        let encoded = serde_json::to_string(&instance).expect("serialize instance id");
        assert_eq!(
            serde_json::from_str::<InstanceId>(&encoded).expect("deserialize instance id"),
            instance
        );

        assert!(serde_json::from_str::<ServiceId>("\"../myc\"").is_err());
        assert!(serde_json::from_str::<InstanceId>("\"UPPER\"").is_err());
    }

    #[test]
    fn identifier_conversion_traits_preserve_validated_text() {
        let service = "myc"
            .parse::<ServiceId>()
            .expect("parse service identifier");
        assert_eq!(service.as_ref(), "myc");
        assert_eq!(String::from(service), "myc");

        let service =
            ServiceId::try_from(String::from("rhi")).expect("convert owned service identifier");
        assert_eq!(service.into_string(), "rhi");

        let instance = "primary"
            .parse::<InstanceId>()
            .expect("parse instance identifier");
        assert_eq!(instance.as_ref(), "primary");
        assert_eq!(String::from(instance), "primary");

        let instance = InstanceId::try_from(String::from("secondary"))
            .expect("convert owned instance identifier");
        assert_eq!(instance.into_string(), "secondary");
    }
}

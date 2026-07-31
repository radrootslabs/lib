//! Canonical NIP-46 permissions and bounded permission sets.

use crate::error::RadrootsNostrConnectError;
use crate::method::Method;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Maximum UTF-8 byte length of one permission parameter.
pub const PERMISSION_PARAMETER_MAX_BYTES: usize = 64;
/// Maximum number of permissions accepted from one wire value.
pub const PERMISSION_COUNT_MAX: usize = 64;
/// Maximum UTF-8 byte length of the comma-separated permission wire value.
pub const PERMISSIONS_MAX_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Permission {
    #[doc(hidden)]
    pub method: Method,
    #[doc(hidden)]
    pub parameter: Option<String>,
}

impl Permission {
    #[must_use]
    pub fn new(method: Method) -> Self {
        Self {
            method,
            parameter: None,
        }
    }

    /// Creates a permission with a bounded canonical parameter.
    pub fn try_with_parameter(
        method: Method,
        parameter: impl Into<String>,
    ) -> Result<Self, RadrootsNostrConnectError> {
        let parameter = parameter.into();
        validate_parameter(&parameter)?;
        Ok(Self {
            method,
            parameter: Some(parameter),
        })
    }

    /// Compatibility constructor retained until the Step 141 consumer cutover.
    #[doc(hidden)]
    #[must_use]
    pub fn with_parameter(method: Method, parameter: impl Into<String>) -> Self {
        Self {
            method,
            parameter: Some(parameter.into()),
        }
    }

    /// Returns the permission method.
    #[must_use]
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Returns the optional method-specific parameter.
    #[must_use]
    pub fn parameter(&self) -> Option<&str> {
        self.parameter.as_deref()
    }

    pub fn matches_request(&self, method: &Method, parameter: Option<&str>) -> bool {
        if self.method != *method {
            return false;
        }
        match (&self.method, self.parameter.as_deref(), parameter) {
            (Method::SignEvent, None, _) => true,
            (Method::SignEvent, Some(configured), Some(requested)) => {
                match (
                    sign_event_kind_parameter(configured),
                    sign_event_kind_parameter(requested),
                ) {
                    (Some(configured), Some(requested)) => configured == requested,
                    _ => false,
                }
            }
            (_, None, None) => true,
            (_, Some(configured), Some(requested)) => configured == requested,
            _ => false,
        }
    }

    pub fn matches_sign_event_kind(&self, event_kind: u32) -> bool {
        let event_kind = event_kind.to_string();
        self.matches_request(&Method::SignEvent, Some(event_kind.as_str()))
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.parameter.as_deref() {
            Some(parameter) => write!(f, "{}:{parameter}", self.method),
            None => write!(f, "{}", self.method),
        }
    }
}

impl FromStr for Permission {
    type Err = RadrootsNostrConnectError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(RadrootsNostrConnectError::InvalidPermission(
                value.to_owned(),
            ));
        }

        let (method, parameter) = match trimmed.split_once(':') {
            Some((method, parameter)) if !parameter.is_empty() => (method, Some(parameter)),
            Some(_) => {
                return Err(RadrootsNostrConnectError::InvalidPermission(
                    value.to_owned(),
                ));
            }
            None => (trimmed, None),
        };

        let method = Method::from_str(method)?;
        match parameter {
            Some(parameter) => Self::try_with_parameter(method, parameter),
            None => Ok(Self::new(method)),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Permissions(Vec<Permission>);

impl Permissions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_slice(&self) -> &[Permission] {
        self.0.as_slice()
    }

    pub fn into_vec(self) -> Vec<Permission> {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Validates and canonicalizes a permission collection.
    pub fn try_from_vec(value: Vec<Permission>) -> Result<Self, RadrootsNostrConnectError> {
        let value = canonicalize(value);
        validate_permissions(&value)?;
        Ok(Self(value))
    }

    pub fn allows_request(&self, method: &Method, parameter: Option<&str>) -> bool {
        self.0
            .iter()
            .any(|permission| permission.matches_request(method, parameter))
    }

    pub fn allows_sign_event_kind(&self, event_kind: u32) -> bool {
        self.0
            .iter()
            .any(|permission| permission.matches_sign_event_kind(event_kind))
    }
}

fn sign_event_kind_parameter(value: &str) -> Option<u32> {
    let value = value.strip_prefix("kind:").unwrap_or(value);
    if value.is_empty() || !value.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

impl From<Vec<Permission>> for Permissions {
    fn from(value: Vec<Permission>) -> Self {
        Self(canonicalize(value))
    }
}

impl fmt::Display for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rendered = self
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        f.write_str(&rendered)
    }
}

impl FromStr for Permissions {
    type Err = RadrootsNostrConnectError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(Self::default());
        }
        if trimmed.len() > PERMISSIONS_MAX_BYTES {
            return Err(invalid_permissions(
                "serialized permission set exceeds its byte limit",
            ));
        }

        let permissions = trimmed
            .split(',')
            .map(Permission::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_from_vec(permissions)
    }
}

impl Serialize for Permissions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        validate_permissions(&self.0).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Permissions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(serde::de::Error::custom)
    }
}

fn canonicalize(mut permissions: Vec<Permission>) -> Vec<Permission> {
    permissions.sort_by_key(ToString::to_string);
    permissions.dedup();
    permissions
}

fn validate_permissions(permissions: &[Permission]) -> Result<(), RadrootsNostrConnectError> {
    if permissions.len() > PERMISSION_COUNT_MAX {
        return Err(invalid_permissions("permission count exceeds its limit"));
    }
    for permission in permissions {
        Method::from_str(permission.method.as_str())?;
        if let Some(parameter) = permission.parameter.as_deref() {
            validate_parameter(parameter)?;
        }
    }
    let rendered = permissions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    if rendered.len() > PERMISSIONS_MAX_BYTES {
        return Err(invalid_permissions(
            "serialized permission set exceeds its byte limit",
        ));
    }
    Ok(())
}

fn validate_parameter(value: &str) -> Result<(), RadrootsNostrConnectError> {
    if value.is_empty()
        || value.len() > PERMISSION_PARAMETER_MAX_BYTES
        || value.trim() != value
        || value.contains(',')
        || value.chars().any(char::is_control)
    {
        return Err(RadrootsNostrConnectError::InvalidPermission(
            value.to_owned(),
        ));
    }
    Ok(())
}

fn invalid_permissions(reason: &str) -> RadrootsNostrConnectError {
    RadrootsNostrConnectError::InvalidPermission(reason.to_owned())
}

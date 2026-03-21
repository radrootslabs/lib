use crate::error::RadrootsNostrConnectError;
use crate::method::RadrootsNostrConnectMethod;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsNostrConnectPermission {
    pub method: RadrootsNostrConnectMethod,
    pub parameter: Option<String>,
}

impl RadrootsNostrConnectPermission {
    pub fn new(method: RadrootsNostrConnectMethod) -> Self {
        Self {
            method,
            parameter: None,
        }
    }

    pub fn with_parameter(
        method: RadrootsNostrConnectMethod,
        parameter: impl Into<String>,
    ) -> Self {
        Self {
            method,
            parameter: Some(parameter.into()),
        }
    }
}

impl fmt::Display for RadrootsNostrConnectPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.parameter.as_deref() {
            Some(parameter) => write!(f, "{}:{parameter}", self.method),
            None => write!(f, "{}", self.method),
        }
    }
}

impl FromStr for RadrootsNostrConnectPermission {
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

        Ok(Self {
            method: RadrootsNostrConnectMethod::from_str(method)?,
            parameter: parameter.map(ToOwned::to_owned),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsNostrConnectPermissions(Vec<RadrootsNostrConnectPermission>);

impl RadrootsNostrConnectPermissions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_slice(&self) -> &[RadrootsNostrConnectPermission] {
        self.0.as_slice()
    }

    pub fn into_vec(self) -> Vec<RadrootsNostrConnectPermission> {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<RadrootsNostrConnectPermission>> for RadrootsNostrConnectPermissions {
    fn from(value: Vec<RadrootsNostrConnectPermission>) -> Self {
        Self(value)
    }
}

impl fmt::Display for RadrootsNostrConnectPermissions {
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

impl FromStr for RadrootsNostrConnectPermissions {
    type Err = RadrootsNostrConnectError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(Self::default());
        }

        let permissions = trimmed
            .split(',')
            .map(RadrootsNostrConnectPermission::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self(permissions))
    }
}

impl Serialize for RadrootsNostrConnectPermissions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for RadrootsNostrConnectPermissions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(serde::de::Error::custom)
    }
}

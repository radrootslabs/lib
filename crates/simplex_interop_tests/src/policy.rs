use alloc::string::{String, ToString};
#[cfg(feature = "std")]
use alloc::vec;
use core::fmt;
use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpQueueUri;

#[cfg(feature = "std")]
pub const RADROOTS_SIMPLEX_INTEROP_REQUIRE_UPSTREAM_ENV: &str =
    "RADROOTS_SIMPLEX_INTEROP_REQUIRE_UPSTREAM";
#[cfg(feature = "std")]
pub const RADROOTS_SIMPLEX_INTEROP_SMP_HOST_ENV: &str = "RADROOTS_SIMPLEX_INTEROP_SMP_HOST";
#[cfg(feature = "std")]
pub const RADROOTS_SIMPLEX_INTEROP_SMP_PORT_ENV: &str = "RADROOTS_SIMPLEX_INTEROP_SMP_PORT";
#[cfg(feature = "std")]
pub const RADROOTS_SIMPLEX_INTEROP_SMP_IDENTITY_ENV: &str = "RADROOTS_SIMPLEX_INTEROP_SMP_IDENTITY";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexInteropFixturePolicy {
    pub namespace_prefix: &'static str,
}

impl Default for RadrootsSimplexInteropFixturePolicy {
    fn default() -> Self {
        Self {
            namespace_prefix: "rr-synth/",
        }
    }
}

impl RadrootsSimplexInteropFixturePolicy {
    pub fn assert_fixture_id(&self, id: &str) -> Result<(), RadrootsSimplexInteropPolicyError> {
        if id.starts_with(self.namespace_prefix) {
            return Ok(());
        }
        Err(RadrootsSimplexInteropPolicyError::InvalidFixtureId(
            id.into(),
        ))
    }

    pub fn assert_queue_uri(
        &self,
        queue_uri: &RadrootsSimplexSmpQueueUri,
    ) -> Result<(), RadrootsSimplexInteropPolicyError> {
        for host in &queue_uri.server.hosts {
            if host.ends_with(".invalid") || host.ends_with(".example") || host.ends_with(".test") {
                continue;
            }
            return Err(RadrootsSimplexInteropPolicyError::InvalidFixtureHost(
                host.clone(),
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexInteropLocalUpstream {
    pub host: String,
    pub port: u16,
    pub server_identity: Option<String>,
}

#[cfg(feature = "std")]
impl RadrootsSimplexInteropLocalUpstream {
    pub fn from_env() -> Option<Self> {
        Self::from_env_values(false).ok().flatten()
    }

    pub fn required_from_env() -> Result<Option<Self>, RadrootsSimplexInteropPolicyError> {
        Self::from_env_values(required_upstream_enabled())
    }

    fn from_env_values(required: bool) -> Result<Option<Self>, RadrootsSimplexInteropPolicyError> {
        let host = optional_env_value(RADROOTS_SIMPLEX_INTEROP_SMP_HOST_ENV);
        let port = optional_env_value(RADROOTS_SIMPLEX_INTEROP_SMP_PORT_ENV);
        let server_identity = optional_env_value(RADROOTS_SIMPLEX_INTEROP_SMP_IDENTITY_ENV);
        Self::from_values(host, port, server_identity, required)
    }

    pub fn from_values(
        host: Option<String>,
        port: Option<String>,
        server_identity: Option<String>,
        required: bool,
    ) -> Result<Option<Self>, RadrootsSimplexInteropPolicyError> {
        let Some(host) =
            required_or_optional(host, required, RADROOTS_SIMPLEX_INTEROP_SMP_HOST_ENV)?
        else {
            return Ok(None);
        };
        let Some(port) =
            required_or_optional(port, required, RADROOTS_SIMPLEX_INTEROP_SMP_PORT_ENV)?
        else {
            return Ok(None);
        };
        let server_identity = match required_or_optional(
            server_identity,
            required,
            RADROOTS_SIMPLEX_INTEROP_SMP_IDENTITY_ENV,
        )? {
            Some(value) => Some(value),
            None => None,
        };
        Ok(Some(Self {
            host,
            port: port.parse::<u16>().map_err(|_| {
                RadrootsSimplexInteropPolicyError::InvalidLocalUpstreamPort(port.clone())
            })?,
            server_identity,
        }))
    }

    pub fn server_address(
        &self,
    ) -> Option<radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpServerAddress> {
        Some(
            radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpServerAddress {
                server_identity: self.server_identity.clone()?,
                hosts: vec![self.host.clone()],
                port: Some(self.port),
            },
        )
    }

    pub fn assert_reachable(&self) -> Result<(), RadrootsSimplexInteropPolicyError> {
        use std::net::{TcpStream, ToSocketAddrs};
        use std::time::Duration;

        let mut addrs = (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|source| {
                RadrootsSimplexInteropPolicyError::LocalUpstreamIo(source.to_string())
            })?;
        let Some(addr) = addrs.next() else {
            return Err(RadrootsSimplexInteropPolicyError::LocalUpstreamIo(
                "no socket addresses resolved".into(),
            ));
        };
        TcpStream::connect_timeout(&addr, Duration::from_millis(500)).map_err(|source| {
            RadrootsSimplexInteropPolicyError::LocalUpstreamIo(source.to_string())
        })?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexInteropPolicyError {
    InvalidFixtureId(String),
    InvalidFixtureHost(String),
    MissingLocalUpstreamEnv(&'static str),
    InvalidLocalUpstreamPort(String),
    LocalUpstreamIo(String),
}

impl fmt::Display for RadrootsSimplexInteropPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFixtureId(id) => {
                write!(
                    f,
                    "interop fixture id `{id}` is outside the rr-synth namespace"
                )
            }
            Self::InvalidFixtureHost(host) => {
                write!(
                    f,
                    "interop fixture host `{host}` is not in a synthetic domain"
                )
            }
            Self::MissingLocalUpstreamEnv(name) => {
                write!(
                    f,
                    "required SimpleX upstream environment `{name}` is not set"
                )
            }
            Self::InvalidLocalUpstreamPort(port) => {
                write!(f, "invalid SimpleX upstream port `{port}`")
            }
            Self::LocalUpstreamIo(message) => write!(f, "{message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexInteropPolicyError {}

#[cfg(feature = "std")]
fn optional_env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(feature = "std")]
fn required_or_optional(
    value: Option<String>,
    required: bool,
    name: &'static str,
) -> Result<Option<String>, RadrootsSimplexInteropPolicyError> {
    match value {
        Some(value) => Ok(Some(value)),
        None if required => Err(RadrootsSimplexInteropPolicyError::MissingLocalUpstreamEnv(
            name,
        )),
        None => Ok(None),
    }
}

#[cfg(feature = "std")]
fn required_upstream_enabled() -> bool {
    optional_env_value(RADROOTS_SIMPLEX_INTEROP_REQUIRE_UPSTREAM_ENV)
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "required" | "REQUIRED"
            )
        })
        .unwrap_or(false)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn optional_upstream_config_returns_none_when_unset() {
        assert_eq!(
            RadrootsSimplexInteropLocalUpstream::from_values(None, None, None, false).unwrap(),
            None
        );
    }

    #[test]
    fn required_upstream_config_reports_first_missing_value() {
        let error =
            RadrootsSimplexInteropLocalUpstream::from_values(None, None, None, true).unwrap_err();
        assert!(matches!(
            error,
            RadrootsSimplexInteropPolicyError::MissingLocalUpstreamEnv(
                RADROOTS_SIMPLEX_INTEROP_SMP_HOST_ENV
            )
        ));
    }

    #[test]
    fn required_upstream_config_requires_identity() {
        let error = RadrootsSimplexInteropLocalUpstream::from_values(
            Some("127.0.0.1".to_owned()),
            Some("5223".to_owned()),
            None,
            true,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RadrootsSimplexInteropPolicyError::MissingLocalUpstreamEnv(
                RADROOTS_SIMPLEX_INTEROP_SMP_IDENTITY_ENV
            )
        ));
    }

    #[test]
    fn required_upstream_config_rejects_invalid_port() {
        let error = RadrootsSimplexInteropLocalUpstream::from_values(
            Some("127.0.0.1".to_owned()),
            Some("not-a-port".to_owned()),
            Some("server-identity".to_owned()),
            true,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RadrootsSimplexInteropPolicyError::InvalidLocalUpstreamPort(_)
        ));
    }
}

use alloc::string::{String, ToString};
#[cfg(feature = "std")]
use alloc::vec;
use core::fmt;
use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpQueueUri;

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
        let host = std::env::var("RADROOTS_SIMPLEX_INTEROP_SMP_HOST").ok()?;
        let port = std::env::var("RADROOTS_SIMPLEX_INTEROP_SMP_PORT")
            .ok()?
            .parse::<u16>()
            .ok()?;
        let server_identity = std::env::var("RADROOTS_SIMPLEX_INTEROP_SMP_IDENTITY").ok();
        Some(Self {
            host,
            port,
            server_identity,
        })
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
            Self::LocalUpstreamIo(message) => write!(f, "{message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexInteropPolicyError {}

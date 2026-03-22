use crate::error::RadrootsSimplexSmpProtoError;
use crate::version::{
    RADROOTS_SIMPLEX_SMP_INITIAL_CLIENT_VERSION,
    RADROOTS_SIMPLEX_SMP_SERVER_HOSTNAMES_CLIENT_VERSION,
    RADROOTS_SIMPLEX_SMP_SHORT_LINKS_CLIENT_VERSION, RadrootsSimplexSmpVersionRange,
};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use base64::Engine as _;
use core::fmt;
use core::str::FromStr;

pub const RADROOTS_SIMPLEX_SMP_URI_SCHEME: &str = "smp";
pub const RADROOTS_SIMPLEX_SMP_DEFAULT_PORT: u16 = 5223;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpServerAddress {
    pub server_identity: String,
    pub hosts: Vec<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsSimplexSmpQueueMode {
    Messaging,
    Contact,
}

impl RadrootsSimplexSmpQueueMode {
    const fn as_query_value(self) -> &'static str {
        match self {
            Self::Messaging => "m",
            Self::Contact => "c",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpQueueUri {
    pub server: RadrootsSimplexSmpServerAddress,
    pub sender_id: String,
    pub version_range: RadrootsSimplexSmpVersionRange,
    pub recipient_dh_public_key: String,
    pub queue_mode: Option<RadrootsSimplexSmpQueueMode>,
}

impl RadrootsSimplexSmpQueueUri {
    pub const fn sender_can_secure(&self) -> bool {
        matches!(
            self.queue_mode,
            Some(RadrootsSimplexSmpQueueMode::Messaging)
        )
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsSimplexSmpProtoError> {
        let without_scheme = value
            .strip_prefix("smp://")
            .ok_or_else(|| RadrootsSimplexSmpProtoError::InvalidUri(value.to_string()))?;
        let (authority, sender_and_fragment) = without_scheme
            .split_once('/')
            .ok_or_else(|| RadrootsSimplexSmpProtoError::InvalidUri(value.to_string()))?;
        let server = parse_server_address(authority)?;
        let (sender_id, fragment) = sender_and_fragment
            .split_once('#')
            .ok_or_else(|| RadrootsSimplexSmpProtoError::InvalidUri(value.to_string()))?;
        let sender_id = sender_id.strip_suffix('/').unwrap_or(sender_id).to_string();
        validate_base64_url("sender_id", &sender_id)?;
        let (fragment_dh_public_key, query) = parse_fragment_query(fragment, value)?;

        let mut version_range = if query.is_none() {
            Some(RadrootsSimplexSmpVersionRange::single(
                RADROOTS_SIMPLEX_SMP_INITIAL_CLIENT_VERSION,
            ))
        } else {
            None
        };
        let mut recipient_dh_public_key: Option<String> = fragment_dh_public_key;
        let mut queue_mode: Option<RadrootsSimplexSmpQueueMode> = None;
        let mut extra_hosts: Option<Vec<String>> = None;

        if let Some(query) = query {
            version_range = None;
            for pair in query.split('&') {
                if pair.is_empty() {
                    continue;
                }

                let (key, raw_value) = pair
                    .split_once('=')
                    .ok_or_else(|| RadrootsSimplexSmpProtoError::InvalidUri(value.to_string()))?;

                match key {
                    "v" => {
                        version_range = Some(raw_value.parse()?);
                    }
                    "dh" => {
                        validate_base64_url("recipient_dh_public_key", raw_value)?;
                        if recipient_dh_public_key
                            .replace(raw_value.to_string())
                            .is_some()
                        {
                            return Err(RadrootsSimplexSmpProtoError::InvalidUri(
                                value.to_string(),
                            ));
                        }
                    }
                    "q" => {
                        let next_mode = match raw_value {
                            "m" => RadrootsSimplexSmpQueueMode::Messaging,
                            "c" => RadrootsSimplexSmpQueueMode::Contact,
                            _ => {
                                return Err(RadrootsSimplexSmpProtoError::InvalidUri(
                                    value.to_string(),
                                ));
                            }
                        };
                        if queue_mode.replace(next_mode).is_some() {
                            return Err(RadrootsSimplexSmpProtoError::InvalidUri(
                                value.to_string(),
                            ));
                        }
                    }
                    "k" if raw_value == "s" => {
                        if queue_mode
                            .replace(RadrootsSimplexSmpQueueMode::Messaging)
                            .is_some()
                        {
                            return Err(RadrootsSimplexSmpProtoError::InvalidUri(
                                value.to_string(),
                            ));
                        }
                    }
                    "srv" => {
                        if extra_hosts
                            .replace(parse_host_list(raw_value, value)?)
                            .is_some()
                        {
                            return Err(RadrootsSimplexSmpProtoError::InvalidUri(
                                value.to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(RadrootsSimplexSmpProtoError::InvalidUri(value.to_string()));
                    }
                }
            }
        }

        let mut server = server;
        if let Some(hosts) = extra_hosts {
            server.hosts.extend(hosts);
        }

        Ok(Self {
            server,
            sender_id,
            version_range: version_range
                .ok_or(RadrootsSimplexSmpProtoError::MissingField("version_range"))?,
            recipient_dh_public_key: recipient_dh_public_key.ok_or(
                RadrootsSimplexSmpProtoError::MissingField("recipient_dh_public_key"),
            )?,
            queue_mode,
        })
    }
}

impl fmt::Display for RadrootsSimplexSmpQueueUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let authority_hosts =
            if self.version_range.min >= RADROOTS_SIMPLEX_SMP_SERVER_HOSTNAMES_CLIENT_VERSION {
                self.server.hosts.join(",")
            } else {
                self.server.hosts.first().cloned().ok_or(fmt::Error)?
            };
        write!(
            f,
            "{RADROOTS_SIMPLEX_SMP_URI_SCHEME}://{}@{}",
            self.server.server_identity, authority_hosts,
        )?;
        if let Some(port) = self.server.port {
            write!(f, ":{port}")?;
        }
        write!(f, "/{}#/?v={}", self.sender_id, self.version_range)?;
        write!(f, "&dh={}", self.recipient_dh_public_key)?;
        if self.version_range.min >= RADROOTS_SIMPLEX_SMP_SHORT_LINKS_CLIENT_VERSION {
            if let Some(queue_mode) = self.queue_mode {
                write!(f, "&q={}", queue_mode.as_query_value())?;
            }
        } else if self.sender_can_secure() {
            write!(f, "&k=s")?;
        }
        if self.version_range.min < RADROOTS_SIMPLEX_SMP_SERVER_HOSTNAMES_CLIENT_VERSION
            && self.server.hosts.len() > 1
        {
            write!(f, "&srv={}", self.server.hosts[1..].join(","))?;
        }
        Ok(())
    }
}

impl FromStr for RadrootsSimplexSmpQueueUri {
    type Err = RadrootsSimplexSmpProtoError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

fn parse_server_address(
    authority: &str,
) -> Result<RadrootsSimplexSmpServerAddress, RadrootsSimplexSmpProtoError> {
    let (server_identity, host_part) = authority
        .split_once('@')
        .ok_or_else(|| RadrootsSimplexSmpProtoError::InvalidUri(authority.to_string()))?;
    validate_base64_url("server_identity", server_identity)?;

    let (hosts_raw, port) = match host_part.rsplit_once(':') {
        Some((hosts, port)) if port.chars().all(|ch| ch.is_ascii_digit()) => {
            let port = port
                .parse::<u16>()
                .map_err(|_| RadrootsSimplexSmpProtoError::InvalidPort(port.to_string()))?;
            (hosts, Some(port))
        }
        _ => (host_part, None),
    };

    if hosts_raw.is_empty() {
        return Err(RadrootsSimplexSmpProtoError::InvalidHostList(
            hosts_raw.to_string(),
        ));
    }

    let hosts = hosts_raw
        .split(',')
        .map(|host| host.trim().to_string())
        .collect::<Vec<_>>();
    if hosts.iter().any(|host| host.is_empty()) {
        return Err(RadrootsSimplexSmpProtoError::InvalidHostList(
            hosts_raw.to_string(),
        ));
    }

    Ok(RadrootsSimplexSmpServerAddress {
        server_identity: server_identity.to_string(),
        hosts,
        port,
    })
}

fn parse_fragment_query<'a>(
    fragment: &'a str,
    original: &str,
) -> Result<(Option<String>, Option<&'a str>), RadrootsSimplexSmpProtoError> {
    let fragment = fragment.strip_prefix('/').unwrap_or(fragment);
    if let Some(query) = fragment.strip_prefix('?') {
        return Ok((None, Some(query)));
    }
    if let Some((dh_public_key, query)) = fragment.split_once("/?") {
        validate_base64_url("recipient_dh_public_key", dh_public_key)?;
        return Ok((Some(dh_public_key.to_string()), Some(query)));
    }
    if let Some((dh_public_key, query)) = fragment.split_once('?') {
        validate_base64_url("recipient_dh_public_key", dh_public_key)?;
        return Ok((Some(dh_public_key.to_string()), Some(query)));
    }
    if !fragment.is_empty() {
        validate_base64_url("recipient_dh_public_key", fragment)?;
        return Ok((Some(fragment.to_string()), None));
    }
    Err(RadrootsSimplexSmpProtoError::InvalidUri(
        original.to_string(),
    ))
}

fn parse_host_list(
    value: &str,
    original: &str,
) -> Result<Vec<String>, RadrootsSimplexSmpProtoError> {
    let hosts = value
        .split(',')
        .map(|host| host.trim().to_string())
        .collect::<Vec<_>>();
    if hosts.is_empty() || hosts.iter().any(|host| host.is_empty()) {
        return Err(RadrootsSimplexSmpProtoError::InvalidHostList(
            original.to_string(),
        ));
    }
    Ok(hosts)
}

fn validate_base64_url(
    field: &'static str,
    value: &str,
) -> Result<(), RadrootsSimplexSmpProtoError> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map(|_| ())
        .map_err(|_| RadrootsSimplexSmpProtoError::InvalidBase64Url {
            field,
            value: value.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_queue_uri() {
        let uri = RadrootsSimplexSmpQueueUri::parse(
            "smp://YWJjZA@server1.example,server2.example:5223/cXVldWU#/?v=4&dh=ZGhLZXk&q=m",
        )
        .unwrap();

        assert_eq!(uri.server.server_identity, "YWJjZA");
        assert_eq!(
            uri.server.hosts,
            vec!["server1.example".to_string(), "server2.example".to_string()]
        );
        assert_eq!(uri.server.port, Some(5223));
        assert_eq!(uri.sender_id, "cXVldWU");
        assert_eq!(uri.version_range, RadrootsSimplexSmpVersionRange::single(4));
        assert_eq!(uri.recipient_dh_public_key, "ZGhLZXk");
        assert_eq!(uri.queue_mode, Some(RadrootsSimplexSmpQueueMode::Messaging));
        assert!(uri.sender_can_secure());
        assert_eq!(
            uri.to_string(),
            "smp://YWJjZA@server1.example,server2.example:5223/cXVldWU#/?v=4&dh=ZGhLZXk&q=m"
        );
    }

    #[test]
    fn rejects_invalid_base64_fields() {
        let error =
            RadrootsSimplexSmpQueueUri::parse("smp://***@server.example/cXVldWU#/?v=4&dh=ZGhLZXk")
                .unwrap_err();
        assert!(matches!(
            error,
            RadrootsSimplexSmpProtoError::InvalidBase64Url {
                field: "server_identity",
                ..
            }
        ));
    }

    #[test]
    fn parses_legacy_sender_secure_queue_uri() {
        let uri = RadrootsSimplexSmpQueueUri::parse(
            "smp://YWJjZA@server1.example:5223/cXVldWU#/?v=1-3&dh=ZGhLZXk&k=s&srv=server2.example",
        )
        .unwrap();

        assert_eq!(
            uri.server.hosts,
            vec!["server1.example".to_string(), "server2.example".to_string()]
        );
        assert_eq!(uri.queue_mode, Some(RadrootsSimplexSmpQueueMode::Messaging));
        assert_eq!(
            uri.version_range,
            RadrootsSimplexSmpVersionRange::new(1, 3).unwrap()
        );
        assert_eq!(
            uri.to_string(),
            "smp://YWJjZA@server1.example:5223/cXVldWU#/?v=1-3&dh=ZGhLZXk&k=s&srv=server2.example"
        );
    }

    #[test]
    fn parses_legacy_unversioned_queue_uri() {
        let uri =
            RadrootsSimplexSmpQueueUri::parse("smp://YWJjZA@server1.example/cXVldWU/#ZGhLZXk")
                .unwrap();

        assert_eq!(uri.version_range, RadrootsSimplexSmpVersionRange::single(1));
        assert_eq!(uri.recipient_dh_public_key, "ZGhLZXk");
        assert_eq!(uri.queue_mode, None);
        assert_eq!(
            uri.to_string(),
            "smp://YWJjZA@server1.example/cXVldWU#/?v=1&dh=ZGhLZXk"
        );
    }
}

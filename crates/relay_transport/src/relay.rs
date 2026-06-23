#![forbid(unsafe_code)]

use crate::RadrootsRelayTransportError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsRelayUrlPolicy {
    Public,
    Localhost,
}

impl RadrootsRelayUrlPolicy {
    fn accepts_ws_host(self, host: &str) -> bool {
        matches!(self, Self::Localhost)
            && matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RadrootsRelayUrl(String);

impl RadrootsRelayUrl {
    pub fn parse(
        value: impl AsRef<str>,
        policy: RadrootsRelayUrlPolicy,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let original = value.as_ref().trim();
        let parsed =
            Url::parse(original).map_err(|error| RadrootsRelayTransportError::RelayUrlParse {
                url: original.to_owned(),
                reason: error.to_string(),
            })?;
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(RadrootsRelayTransportError::RelayUrlUserinfo {
                url: original.to_owned(),
            });
        }
        let Some(host) = parsed.host_str().filter(|host| !host.is_empty()) else {
            return Err(RadrootsRelayTransportError::EmptyRelayHost {
                url: original.to_owned(),
            });
        };
        validate_host_destination(original, host, policy)?;
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(RadrootsRelayTransportError::RelayUrlQueryOrFragment {
                url: original.to_owned(),
            });
        }
        let scheme = parsed.scheme();
        match scheme {
            "wss" => {}
            "ws" if policy.accepts_ws_host(host) => {}
            "ws" => {
                return Err(RadrootsRelayTransportError::WsRequiresLocalhostPolicy {
                    url: original.to_owned(),
                });
            }
            other => {
                return Err(RadrootsRelayTransportError::UnsupportedRelayScheme {
                    url: original.to_owned(),
                    scheme: other.to_owned(),
                });
            }
        }
        let mut normalized = parsed.to_string();
        if parsed.path() == "/" {
            normalized.pop();
        }
        Ok(Self(normalized))
    }

    pub fn validate_public_resolved_ip_addrs<I>(
        &self,
        addrs: I,
    ) -> Result<(), RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = IpAddr>,
    {
        for address in addrs {
            if let Some(reason) = forbidden_public_ip_reason(address) {
                return Err(
                    RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination {
                        url: self.0.clone(),
                        address: address.to_string(),
                        reason: reason.to_owned(),
                    },
                );
            }
        }
        Ok(())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

fn validate_host_destination(
    original: &str,
    host: &str,
    policy: RadrootsRelayUrlPolicy,
) -> Result<(), RadrootsRelayTransportError> {
    let host = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    if matches!(policy, RadrootsRelayUrlPolicy::Public)
        && let Ok(address) = host.parse::<IpAddr>()
        && let Some(reason) = forbidden_public_ip_reason(address)
    {
        return Err(RadrootsRelayTransportError::RelayUrlForbiddenDestination {
            url: original.to_owned(),
            reason: reason.to_owned(),
        });
    }
    Ok(())
}

fn forbidden_public_ip_reason(address: IpAddr) -> Option<&'static str> {
    match address {
        IpAddr::V4(address) => forbidden_public_ipv4_reason(address),
        IpAddr::V6(address) => forbidden_public_ipv6_reason(address),
    }
}

fn forbidden_public_ipv4_reason(address: Ipv4Addr) -> Option<&'static str> {
    let octets = address.octets();
    if address.is_unspecified() || octets[0] == 0 {
        Some("unspecified or this-network IPv4 address")
    } else if address.is_loopback() {
        Some("loopback IPv4 address")
    } else if address.is_private() {
        Some("private IPv4 address")
    } else if address.is_link_local() {
        Some("link-local IPv4 address")
    } else if address.is_multicast() {
        Some("multicast IPv4 address")
    } else if address.is_broadcast() {
        Some("broadcast IPv4 address")
    } else if address.is_documentation() {
        Some("documentation IPv4 address")
    } else if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        Some("shared IPv4 address space")
    } else if octets[0] == 192 && octets[1] == 0 && octets[2] == 0 {
        Some("IETF protocol-assignment IPv4 address")
    } else if octets[0] == 198 && matches!(octets[1], 18 | 19) {
        Some("benchmark IPv4 address")
    } else if octets[0] >= 240 {
        Some("reserved IPv4 address")
    } else {
        None
    }
}

fn forbidden_public_ipv6_reason(address: Ipv6Addr) -> Option<&'static str> {
    let segments = address.segments();
    if let Some(mapped) = address.to_ipv4_mapped() {
        return forbidden_public_ipv4_reason(mapped);
    }
    if address.is_unspecified() {
        Some("unspecified IPv6 address")
    } else if address.is_loopback() {
        Some("loopback IPv6 address")
    } else if address.is_multicast() {
        Some("multicast IPv6 address")
    } else if (segments[0] & 0xfe00) == 0xfc00 {
        Some("unique-local IPv6 address")
    } else if (segments[0] & 0xffc0) == 0xfe80 {
        Some("link-local IPv6 address")
    } else if segments[0] == 0x2001 && segments[1] == 0x0db8 {
        Some("documentation IPv6 address")
    } else if segments[0] == 0x2001 && segments[1] < 0x0200 {
        Some("IETF protocol-assignment IPv6 address")
    } else {
        None
    }
}

impl fmt::Display for RadrootsRelayUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayTargetSet {
    relays: Vec<RadrootsRelayUrl>,
}

impl RadrootsRelayTargetSet {
    pub fn new<I, S>(
        relays: I,
        policy: RadrootsRelayUrlPolicy,
    ) -> Result<Self, RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut ordered_relays = Vec::new();
        for relay in relays {
            let relay = RadrootsRelayUrl::parse(relay, policy)?;
            if !ordered_relays.iter().any(|existing| existing == &relay) {
                ordered_relays.push(relay);
            }
        }
        let relays = ordered_relays;
        if relays.is_empty() {
            return Err(RadrootsRelayTransportError::EmptyTargetSet);
        }
        Ok(Self { relays })
    }

    pub fn from_urls(relays: Vec<RadrootsRelayUrl>) -> Result<Self, RadrootsRelayTransportError> {
        let mut ordered_relays = Vec::new();
        for relay in relays {
            if !ordered_relays.iter().any(|existing| existing == &relay) {
                ordered_relays.push(relay);
            }
        }
        let relays = ordered_relays;
        if relays.is_empty() {
            return Err(RadrootsRelayTransportError::EmptyTargetSet);
        }
        Ok(Self { relays })
    }

    pub fn relays(&self) -> &[RadrootsRelayUrl] {
        &self.relays
    }

    pub fn relay_strings(&self) -> Vec<String> {
        self.relays
            .iter()
            .map(|relay| relay.as_str().to_owned())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.relays.len()
    }

    pub fn is_empty(&self) -> bool {
        self.relays.is_empty()
    }
}

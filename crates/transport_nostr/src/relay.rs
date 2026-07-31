#![forbid(unsafe_code)]

use crate::RadrootsRelayTransportError;
use radroots_transport::{Target, TransportId};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsRelayUrlPolicy {
    /// Allows `wss` endpoints from trusted configuration.
    ///
    /// This performs canonical syntax, literal-address, and local-hostname
    /// checks. It is not an SSRF boundary for attacker-controlled hostnames
    /// because the default SDK connector does not pin DNS resolution.
    Public,
    /// Allows `ws` or `wss` only for exact loopback hosts.
    Localhost,
}

impl RadrootsRelayUrlPolicy {
    fn accepts_ws_host(self, host: &str) -> bool {
        matches!(self, Self::Localhost)
            && matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelayUrl(String);

impl RelayUrl {
    pub(crate) fn from_normalized_transport(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn parse(
        value: impl AsRef<str>,
        policy: RadrootsRelayUrlPolicy,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let original = value.as_ref();
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
        let target = Target::new(TransportId::NOSTR, original).map_err(|error| {
            RadrootsRelayTransportError::RelayUrlParse {
                url: original.to_owned(),
                reason: error.to_string(),
            }
        })?;
        Ok(Self(target.uri().as_str().to_owned()))
    }

    pub fn validate_public_resolved_ip_addrs<I>(
        &self,
        addrs: I,
    ) -> Result<(), RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = IpAddr>,
    {
        let mut resolved_any = false;
        for address in addrs {
            resolved_any = true;
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
        if !resolved_any {
            return Err(RadrootsRelayTransportError::RelayUrlResolvedNoAddresses {
                url: self.0.clone(),
            });
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
    if matches!(policy, RadrootsRelayUrlPolicy::Localhost) {
        if !policy.accepts_ws_host(host) {
            return Err(RadrootsRelayTransportError::RelayUrlForbiddenDestination {
                url: original.to_owned(),
                reason: "localhost policy permits only exact loopback hosts".to_owned(),
            });
        }
        return Ok(());
    }
    if let Ok(address) = host.parse::<IpAddr>() {
        if let Some(reason) = forbidden_public_ip_reason(address) {
            return Err(RadrootsRelayTransportError::RelayUrlForbiddenDestination {
                url: original.to_owned(),
                reason: reason.to_owned(),
            });
        }
    } else if let Some(reason) = forbidden_public_hostname_reason(host) {
        return Err(RadrootsRelayTransportError::RelayUrlForbiddenDestination {
            url: original.to_owned(),
            reason: reason.to_owned(),
        });
    }
    Ok(())
}

fn forbidden_public_hostname_reason(host: &str) -> Option<&'static str> {
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        Some("localhost DNS name")
    } else if host.ends_with(".local") {
        Some("link-local multicast DNS name")
    } else if host.ends_with(".home.arpa") {
        Some("special-use home network DNS name")
    } else if !host.contains('.') {
        Some("single-label DNS name")
    } else {
        None
    }
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
    } else if octets[0] == 192 && octets[1] == 88 && octets[2] == 99 {
        Some("deprecated or local-use relay-anycast IPv4 address")
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
    } else if segments[0] == 0x0064
        && segments[1] == 0xff9b
        && segments[2..6].iter().all(|segment| *segment == 0)
    {
        Some("IPv4/IPv6 translation address")
    } else if !is_supported_global_ipv6_unicast(segments) {
        Some("non-global or reserved IPv6 unicast address")
    } else if segments[0] == 0x2001 && segments[1] == 0x0db8 {
        Some("documentation IPv6 address")
    } else if segments[0] == 0x2001 && segments[1] < 0x0200 {
        Some("IETF protocol-assignment IPv6 address")
    } else if segments[0] == 0x2002 {
        Some("6to4 IPv6 address")
    } else if segments[0] == 0x3fff && (segments[1] & 0xf000) == 0 {
        Some("documentation IPv6 address")
    } else {
        None
    }
}

fn is_supported_global_ipv6_unicast(segments: [u16; 8]) -> bool {
    (segments[0] & 0xe000) == 0x2000
}

impl fmt::Display for RelayUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayTargetSet {
    relays: Vec<RelayUrl>,
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
            let relay = RelayUrl::parse(relay, policy)?;
            if ordered_relays.iter().any(|existing| existing == &relay) {
                return Err(RadrootsRelayTransportError::DuplicateRelayUrl {
                    url: relay.into_string(),
                });
            }
            ordered_relays.push(relay);
        }
        let relays = ordered_relays;
        if relays.is_empty() {
            return Err(RadrootsRelayTransportError::EmptyTargetSet);
        }
        Ok(Self { relays })
    }

    pub fn from_urls(relays: Vec<RelayUrl>) -> Result<Self, RadrootsRelayTransportError> {
        let mut ordered_relays = Vec::new();
        for relay in relays {
            if ordered_relays.iter().any(|existing| existing == &relay) {
                return Err(RadrootsRelayTransportError::DuplicateRelayUrl {
                    url: relay.into_string(),
                });
            }
            ordered_relays.push(relay);
        }
        let relays = ordered_relays;
        if relays.is_empty() {
            return Err(RadrootsRelayTransportError::EmptyTargetSet);
        }
        Ok(Self { relays })
    }

    pub fn relays(&self) -> &[RelayUrl] {
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

#[cfg(test)]
mod tests {
    use super::{
        RadrootsRelayUrlPolicy, forbidden_public_ipv4_reason, forbidden_public_ipv6_reason,
        validate_host_destination,
    };
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn host_destination_validation_covers_public_and_local_policy_edges() {
        assert!(!RadrootsRelayUrlPolicy::Public.accepts_ws_host("localhost"));
        assert!(RadrootsRelayUrlPolicy::Localhost.accepts_ws_host("localhost"));
        validate_host_destination(
            "wss://93.184.216.34",
            "93.184.216.34",
            RadrootsRelayUrlPolicy::Public,
        )
        .expect("public ipv4 host");
        validate_host_destination(
            "wss://relay.example.com",
            "relay.example.com",
            RadrootsRelayUrlPolicy::Public,
        )
        .expect("public dns host");
        validate_host_destination(
            "ws://127.0.0.1",
            "127.0.0.1",
            RadrootsRelayUrlPolicy::Localhost,
        )
        .expect("localhost policy host");
    }

    #[test]
    fn public_ipv4_classifier_covers_forbidden_ranges_and_global_addresses() {
        let cases = [
            Ipv4Addr::new(0, 0, 0, 0),
            Ipv4Addr::new(0, 1, 2, 3),
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(10, 1, 2, 3),
            Ipv4Addr::new(169, 254, 1, 2),
            Ipv4Addr::new(224, 0, 0, 1),
            Ipv4Addr::new(255, 255, 255, 255),
            Ipv4Addr::new(192, 0, 2, 1),
            Ipv4Addr::new(100, 64, 0, 1),
            Ipv4Addr::new(192, 0, 0, 8),
            Ipv4Addr::new(192, 88, 99, 2),
            Ipv4Addr::new(198, 18, 0, 1),
            Ipv4Addr::new(240, 0, 0, 1),
        ];
        for address in cases {
            assert!(forbidden_public_ipv4_reason(address).is_some());
        }
        assert_eq!(
            forbidden_public_ipv4_reason(Ipv4Addr::new(93, 184, 216, 34)),
            None
        );
        assert_eq!(
            forbidden_public_ipv4_reason(Ipv4Addr::new(100, 128, 0, 1)),
            None
        );
        assert_eq!(
            forbidden_public_ipv4_reason(Ipv4Addr::new(193, 0, 0, 8)),
            None
        );
        assert_eq!(
            forbidden_public_ipv4_reason(Ipv4Addr::new(192, 1, 0, 8)),
            None
        );
        assert_eq!(
            forbidden_public_ipv4_reason(Ipv4Addr::new(192, 0, 1, 8)),
            None
        );
        assert_eq!(
            forbidden_public_ipv4_reason(Ipv4Addr::new(198, 20, 0, 1)),
            None
        );
    }

    #[test]
    fn public_ipv6_classifier_covers_forbidden_ranges_and_global_addresses() {
        let cases = [
            "::ffff:192.168.1.10",
            "::",
            "::1",
            "ff02::1",
            "fd00::1",
            "fe80::1",
            "64:ff9b::7f00:1",
            "64:ff9b::a00:1",
            "64:ff9b::5db8:d822",
            "64:ff9b:1::1",
            "100::1",
            "100:0:0:1::1",
            "2001:db8::1",
            "2001:1::1",
            "2002:db8::1",
            "2002:1::1",
            "3fff::1",
            "5f00::1",
        ];
        for address in cases {
            assert!(
                forbidden_public_ipv6_reason(address.parse::<Ipv6Addr>().expect("ipv6")).is_some()
            );
        }
        assert_eq!(
            forbidden_public_ipv6_reason(
                "2001:4860:4860::8888"
                    .parse::<Ipv6Addr>()
                    .expect("public ipv6")
            ),
            None
        );
        assert_eq!(
            forbidden_public_ipv6_reason("2001:db9::1".parse::<Ipv6Addr>().expect("ipv6")),
            None
        );
        assert_eq!(
            forbidden_public_ipv6_reason("2001:200::1".parse::<Ipv6Addr>().expect("ipv6")),
            None
        );
    }
}

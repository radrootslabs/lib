//! Nostr relay identifiers and network policy.

use crate::Error;
use core::fmt;
use nostr_sdk::error::Error as TransportError;
use radroots_transport::{Target, TargetNetworkPolicy, TransportId};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::Url;

/// Validated canonical Nostr relay URL.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelayUrl(String);

impl RelayUrl {
    /// Parses, canonicalizes, and applies an explicit destination policy.
    pub fn parse(value: impl AsRef<str>, policy: RelayUrlPolicy) -> Result<Self, Error> {
        let original = value.as_ref();
        let target = match policy {
            RelayUrlPolicy::PrivateNetwork => {
                Target::nostr_relay_with_policy(original, TargetNetworkPolicy::PrivateDevice)
            }
            RelayUrlPolicy::Public | RelayUrlPolicy::Local => Target::nostr_relay(original),
        }
        .map_err(|_| Error::InvalidRelayUrl)?;
        let canonical = target.uri().as_str();
        let parsed = Url::parse(canonical).map_err(|_| Error::InvalidRelayUrl)?;
        let host = parsed.host_str().ok_or(Error::InvalidRelayUrl)?;
        validate_scheme(canonical, parsed.scheme(), policy)?;
        validate_host(canonical, host, policy)?;
        Ok(Self(canonical.to_owned()))
    }

    /// Converts a validated relay URL into the generic Nostr target model.
    pub fn to_target(&self) -> Result<Target, Error> {
        Target::nostr_relay(self.as_str())
            .or_else(|_| {
                Target::nostr_relay_with_policy(self.as_str(), TargetNetworkPolicy::PrivateDevice)
            })
            .map_err(|_| Error::Target)
    }

    /// Validates and converts a generic target under the selected policy.
    pub fn from_target(target: &Target, policy: RelayUrlPolicy) -> Result<Self, Error> {
        if *target.kind() != TransportId::NOSTR {
            return Err(Error::UnexpectedTransport);
        }
        Self::parse(target.uri().as_str(), policy)
    }

    /// Revalidates every address returned by DNS before a connection is made.
    pub fn validate_resolved_addresses(
        &self,
        policy: RelayUrlPolicy,
        addresses: impl IntoIterator<Item = IpAddr>,
    ) -> Result<(), Error> {
        let mut resolved = false;
        for address in addresses {
            resolved = true;
            if !policy.accepts_address(address) {
                return Err(Error::ResolvedAddressDenied);
            }
        }
        if !resolved {
            return Err(Error::EmptyResolution);
        }
        Ok(())
    }

    /// Returns the canonical relay URL.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for RelayUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug)]
struct NetworkPolicyError(&'static str);

impl fmt::Display for NetworkPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for NetworkPolicyError {}

pub(crate) fn policy_error(message: &'static str) -> TransportError {
    TransportError::policy(NetworkPolicyError(message))
}

/// Destination class authorized for relay connections.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RelayUrlPolicy {
    /// TLS-only public Internet endpoints; resolved addresses must be global.
    Public,
    /// Exact loopback endpoints; plaintext WebSocket is allowed.
    Local,
    /// Exact RFC1918 IPv4 or ULA IPv6 device endpoints.
    PrivateNetwork,
}

impl RelayUrlPolicy {
    pub(crate) fn accepts_address(self, address: IpAddr) -> bool {
        match self {
            Self::Public => public_address(address),
            Self::Local => address.is_loopback(),
            Self::PrivateNetwork => trusted_network_address(address),
        }
    }
}

fn validate_scheme(_url: &str, scheme: &str, policy: RelayUrlPolicy) -> Result<(), Error> {
    if scheme == "wss"
        || scheme == "ws"
            && matches!(
                policy,
                RelayUrlPolicy::Local | RelayUrlPolicy::PrivateNetwork
            )
    {
        return Ok(());
    }
    Err(Error::RelaySchemeDenied)
}

fn validate_host(_url: &str, host: &str, policy: RelayUrlPolicy) -> Result<(), Error> {
    let address = host.trim_matches(['[', ']']).parse::<IpAddr>().ok();
    let accepted = match (policy, address) {
        (RelayUrlPolicy::Public, Some(address)) => public_address(address),
        (RelayUrlPolicy::Public, None) => public_hostname(host),
        (RelayUrlPolicy::Local, Some(address)) => address.is_loopback(),
        (RelayUrlPolicy::Local, None) => host.eq_ignore_ascii_case("localhost"),
        (RelayUrlPolicy::PrivateNetwork, Some(address)) => trusted_network_address(address),
        (RelayUrlPolicy::PrivateNetwork, None) => false,
    };
    if accepted {
        Ok(())
    } else {
        Err(Error::RelayDestinationDenied)
    }
}

fn public_hostname(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host.contains('.')
        && host != "localhost"
        && !host.ends_with(".localhost")
        && !host.ends_with(".local")
        && !host.ends_with(".home.arpa")
}

fn public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => public_ipv4(address),
        IpAddr::V6(address) => public_ipv6(address),
    }
}

fn trusted_network_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => address.is_private(),
        IpAddr::V6(address) => address.segments()[0] & 0xfe00 == 0xfc00,
    }
}

fn public_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    !(octets[0] == 0
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_documentation()
        || octets[0] == 100 && (64..=127).contains(&octets[1])
        || octets[0] == 192 && octets[1] == 0 && octets[2] == 0
        || octets[0] == 192 && octets[1] == 88 && octets[2] == 99
        || octets[0] == 198 && matches!(octets[1], 18 | 19)
        || octets[0] >= 240)
}

fn public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return public_ipv4(mapped);
    }
    let segments = address.segments();
    (segments[0] & 0xe000) == 0x2000
        && !(segments[0] == 0x2001 && segments[1] <= 0x01ff)
        && !(segments[0] == 0x2001 && segments[1] == 0x0db8)
        && segments[0] != 0x2002
        && !(segments[0] == 0x3fff && (segments[1] & 0xf000) == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_classify_literal_and_named_destinations() {
        assert!(RelayUrl::parse("wss://relay.example.com", RelayUrlPolicy::Public).is_ok());
        assert!(RelayUrl::parse("wss://10.0.0.1", RelayUrlPolicy::Public).is_err());
        assert!(RelayUrl::parse("wss://10.0.0.1", RelayUrlPolicy::PrivateNetwork).is_ok());
        assert!(RelayUrl::parse("ws://127.0.0.1", RelayUrlPolicy::Local).is_ok());
        assert!(RelayUrl::parse("ws://relay.example.com", RelayUrlPolicy::Public).is_err());
        assert!(RelayUrl::parse("wss://localhost", RelayUrlPolicy::Local).is_ok());
        assert!(RelayUrl::parse("wss://localhost", RelayUrlPolicy::Public).is_err());
        assert!(!public_hostname("localhost."));
        assert!(!public_hostname("intranet"));
        assert!(!public_hostname("host.localhost"));
        assert!(RelayUrl::parse("wss://host.local", RelayUrlPolicy::Public).is_err());
        assert!(RelayUrl::parse("wss://host.home.arpa", RelayUrlPolicy::Public).is_err());
        assert!(RelayUrl::parse("wss://private.example", RelayUrlPolicy::PrivateNetwork).is_err());
        assert!(RelayUrl::parse("wss://localhost", RelayUrlPolicy::PrivateNetwork).is_err());
        assert!(RelayUrl::parse("ws://10.0.0.1", RelayUrlPolicy::PrivateNetwork).is_ok());
        assert!(RelayUrl::parse("ws://8.8.8.8", RelayUrlPolicy::PrivateNetwork).is_err());

        let relay =
            RelayUrl::parse("wss://relay.example.com", RelayUrlPolicy::Public).expect("relay");
        assert_eq!(relay.to_string(), relay.as_str());
        assert_eq!(
            RelayUrl::from_target(&relay.to_target().expect("target"), RelayUrlPolicy::Public),
            Ok(relay)
        );
        let local = Target::local("local:device").expect("local target");
        assert!(matches!(
            RelayUrl::from_target(&local, RelayUrlPolicy::Public),
            Err(Error::UnexpectedTransport)
        ));
        assert!(!policy_error("denied").to_string().is_empty());
    }

    #[test]
    fn device_network_policy_matches_the_shared_conformance_vectors() {
        let document: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/conformance/vectors/transport/device_network_policy.v1.json"
        ))
        .expect("device network policy vectors");
        for vector in document["vectors"].as_array().expect("vectors") {
            let input = &vector["input"];
            if input["surface"] != "relay" {
                continue;
            }
            let policy = match input["policy"].as_str().expect("policy") {
                "public" => RelayUrlPolicy::Public,
                "loopback" => RelayUrlPolicy::Local,
                "private_device" => RelayUrlPolicy::PrivateNetwork,
                other => panic!("unknown relay policy {other}"),
            };
            let accepted =
                RelayUrl::parse(input["endpoint"].as_str().expect("endpoint"), policy).is_ok();
            assert_eq!(
                accepted,
                vector["expected"]["accepted"].as_bool().expect("accepted"),
                "{}",
                vector["id"].as_str().expect("id")
            );
        }
    }

    #[test]
    fn patched_url_parser_rejects_ascii_masking_punycode() {
        for denied in ["wss://xn--example-.org", "wss://example.org.xn--"] {
            assert!(RelayUrl::parse(denied, RelayUrlPolicy::Public).is_err());
        }
    }

    #[test]
    fn resolved_addresses_are_revalidated() {
        let relay = RelayUrl::parse("wss://relay.example.com", RelayUrlPolicy::Public)
            .expect("public relay");
        assert!(
            relay
                .validate_resolved_addresses(
                    RelayUrlPolicy::Public,
                    [IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))],
                )
                .is_ok()
        );
        assert!(
            relay
                .validate_resolved_addresses(
                    RelayUrlPolicy::Public,
                    [IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
                )
                .is_err()
        );
        assert!(
            relay
                .validate_resolved_addresses(
                    RelayUrlPolicy::Public,
                    [
                        IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
                        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                    ],
                )
                .is_err()
        );
        assert!(
            relay
                .validate_resolved_addresses(RelayUrlPolicy::Public, [])
                .is_err()
        );
    }

    #[test]
    fn address_policies_fail_closed_for_special_use_ranges() {
        for denied in [
            Ipv4Addr::new(0, 0, 0, 0),
            Ipv4Addr::new(0, 1, 1, 1),
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::new(100, 64, 0, 1),
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(169, 254, 1, 1),
            Ipv4Addr::new(192, 0, 2, 1),
            Ipv4Addr::new(198, 18, 0, 1),
            Ipv4Addr::new(224, 0, 0, 1),
            Ipv4Addr::new(240, 0, 0, 1),
        ] {
            assert!(!RelayUrlPolicy::Public.accepts_address(denied.into()));
        }
        for denied in [Ipv6Addr::UNSPECIFIED, Ipv6Addr::LOCALHOST] {
            assert!(!RelayUrlPolicy::Public.accepts_address(denied.into()));
        }
        assert!(RelayUrlPolicy::Local.accepts_address(Ipv4Addr::LOCALHOST.into()));
        assert!(!RelayUrlPolicy::Local.accepts_address(Ipv4Addr::new(10, 0, 0, 1).into()));
        assert!(RelayUrlPolicy::PrivateNetwork.accepts_address(Ipv4Addr::new(10, 0, 0, 1).into()));
        assert!(!RelayUrlPolicy::PrivateNetwork.accepts_address(Ipv4Addr::UNSPECIFIED.into()));
        assert!(!RelayUrlPolicy::PrivateNetwork.accepts_address(Ipv4Addr::LOCALHOST.into()));
        assert!(!RelayUrlPolicy::PrivateNetwork.accepts_address(Ipv4Addr::BROADCAST.into()));
        assert!(
            !RelayUrlPolicy::PrivateNetwork.accepts_address(Ipv4Addr::new(224, 0, 0, 1).into())
        );
        assert!(
            RelayUrlPolicy::PrivateNetwork
                .accepts_address("fd00::1".parse::<Ipv6Addr>().expect("private v6").into())
        );
        assert!(!RelayUrlPolicy::PrivateNetwork.accepts_address(Ipv6Addr::UNSPECIFIED.into()));
        assert!(!RelayUrlPolicy::PrivateNetwork.accepts_address(Ipv6Addr::LOCALHOST.into()));
        assert!(
            !RelayUrlPolicy::PrivateNetwork
                .accepts_address("ff02::1".parse::<Ipv6Addr>().expect("multicast").into())
        );

        for denied in [
            "192.0.0.1",
            "192.88.99.1",
            "198.19.0.1",
            "255.0.0.1",
            "::ffff:10.0.0.1",
            "2001:db8::1",
            "2002::1",
            "3fff::1",
            "fc00::1",
            "fe80::1",
            "ff02::1",
        ] {
            let address = denied.parse::<IpAddr>().expect("address");
            assert!(
                !RelayUrlPolicy::Public.accepts_address(address),
                "accepted {denied}"
            );
        }
        for allowed in ["8.8.8.8", "2606:4700:4700::1111"] {
            assert!(
                RelayUrlPolicy::Public.accepts_address(allowed.parse::<IpAddr>().expect("address"))
            );
        }
    }

    #[test]
    fn address_policy_keeps_special_use_range_edges_and_adjacent_global_addresses_distinct() {
        for (address, accepted) in [
            ("100.63.255.255", true),
            ("100.64.0.0", false),
            ("100.127.255.255", false),
            ("100.128.0.0", true),
            ("192.0.0.0", false),
            ("192.0.0.255", false),
            ("192.0.1.0", true),
            ("192.88.98.255", true),
            ("192.88.99.0", false),
            ("192.88.99.255", false),
            ("192.88.100.0", true),
            ("2001:1ff::1", false),
            ("2001:200::1", true),
            ("2001:db8::1", false),
            ("2001:db9::1", true),
        ] {
            assert_eq!(
                RelayUrlPolicy::Public.accepts_address(address.parse().unwrap()),
                accepted,
                "{address}"
            );
        }
        assert!(validate_scheme("ftp://relay.example", "ftp", RelayUrlPolicy::Public).is_err());
    }
}

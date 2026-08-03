//! Nostr relay identifiers and network policy.

use crate::Error;
use core::fmt;
use radroots_transport::{Target, TransportId};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::Url;

/// Validated canonical Nostr relay URL.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelayUrl(String);

impl RelayUrl {
    /// Parses, canonicalizes, and applies an explicit destination policy.
    pub fn parse(value: impl AsRef<str>, policy: RelayUrlPolicy) -> Result<Self, Error> {
        let original = value.as_ref();
        let target = Target::nostr_relay(original).map_err(|error| Error::InvalidRelayUrl {
            url: original.to_owned(),
            reason: error.to_string(),
        })?;
        let canonical = target.uri().as_str();
        let parsed = Url::parse(canonical).map_err(|error| Error::InvalidRelayUrl {
            url: original.to_owned(),
            reason: error.to_string(),
        })?;
        let host = parsed.host_str().ok_or_else(|| Error::InvalidRelayUrl {
            url: original.to_owned(),
            reason: "host is required".to_owned(),
        })?;
        validate_scheme(canonical, parsed.scheme(), policy)?;
        validate_host(canonical, host, policy)?;
        Ok(Self(canonical.to_owned()))
    }

    /// Converts a validated relay URL into the generic Nostr target model.
    pub fn to_target(&self) -> Result<Target, Error> {
        Target::nostr_relay(self.as_str()).map_err(|error| Error::Target(error.to_string()))
    }

    /// Validates and converts a generic target under the selected policy.
    pub fn from_target(target: &Target, policy: RelayUrlPolicy) -> Result<Self, Error> {
        if *target.kind() != TransportId::NOSTR {
            return Err(Error::UnexpectedTransport {
                actual: target.kind().to_string(),
            });
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
                return Err(Error::ResolvedAddressDenied {
                    url: self.0.clone(),
                    address: address.to_string(),
                });
            }
        }
        if !resolved {
            return Err(Error::EmptyResolution {
                url: self.0.clone(),
            });
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

/// Destination class authorized for relay connections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RelayUrlPolicy {
    /// TLS-only public Internet endpoints; resolved addresses must be global.
    Public,
    /// Exact loopback endpoints; plaintext WebSocket is allowed.
    Local,
    /// TLS-only endpoints on explicitly trusted private or public networks.
    PrivateNetwork,
}

impl RelayUrlPolicy {
    fn accepts_address(self, address: IpAddr) -> bool {
        match self {
            Self::Public => public_address(address),
            Self::Local => address.is_loopback(),
            Self::PrivateNetwork => trusted_network_address(address),
        }
    }
}

fn validate_scheme(url: &str, scheme: &str, policy: RelayUrlPolicy) -> Result<(), Error> {
    if scheme == "wss" || scheme == "ws" && matches!(policy, RelayUrlPolicy::Local) {
        return Ok(());
    }
    Err(Error::RelaySchemeDenied {
        url: url.to_owned(),
    })
}

fn validate_host(url: &str, host: &str, policy: RelayUrlPolicy) -> Result<(), Error> {
    let address = host.parse::<IpAddr>().ok();
    let accepted = match (policy, address) {
        (RelayUrlPolicy::Public, Some(address)) => public_address(address),
        (RelayUrlPolicy::Public, None) => public_hostname(host),
        (RelayUrlPolicy::Local, Some(address)) => address.is_loopback(),
        (RelayUrlPolicy::Local, None) => host.eq_ignore_ascii_case("localhost"),
        (RelayUrlPolicy::PrivateNetwork, Some(address)) => trusted_network_address(address),
        (RelayUrlPolicy::PrivateNetwork, None) => !host.eq_ignore_ascii_case("localhost"),
    };
    if accepted {
        Ok(())
    } else {
        Err(Error::RelayDestinationDenied {
            url: url.to_owned(),
            reason: "destination class does not match relay policy",
        })
    }
}

fn public_hostname(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
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
        IpAddr::V4(address) => {
            !address.is_unspecified()
                && !address.is_loopback()
                && !address.is_multicast()
                && !address.is_broadcast()
        }
        IpAddr::V6(address) => {
            !address.is_unspecified() && !address.is_loopback() && !address.is_multicast()
        }
    }
}

fn public_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    !(address.is_unspecified()
        || octets[0] == 0
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
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
        && !address.is_multicast()
        && (segments[0] & 0xfe00) != 0xfc00
        && (segments[0] & 0xffc0) != 0xfe80
        && !(segments[0] == 0x2001 && segments[1] <= 0x01ff)
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
    }
}

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
use core::{borrow::Borrow, fmt, net::Ipv6Addr, ops::Deref, str::FromStr};

use crate::ids::RadrootsIdParseError;

/// One canonical, byte-stable Nostr relay hint.
///
/// This intentionally accepts a conservative subset of WebSocket URLs. It
/// does not inherit browser URL normalization, legacy IPv4 syntax, Unicode
/// host processing, user information, or fragments. It is separate from the
/// generic [`crate::ids::RadrootsRelayUrl`] type because protocol-tag
/// validation must be portable across implementations.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsNostrRelayHint(String);

impl RadrootsNostrRelayHint {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsIdParseError> {
        validate_nostr_relay_hint(value.as_ref()).map(Self)
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    #[inline]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for RadrootsNostrRelayHint {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for RadrootsNostrRelayHint {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for RadrootsNostrRelayHint {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<RadrootsNostrRelayHint> for String {
    #[inline]
    fn from(value: RadrootsNostrRelayHint) -> Self {
        value.into_string()
    }
}

impl fmt::Display for RadrootsNostrRelayHint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RadrootsNostrRelayHint {
    type Err = RadrootsIdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for RadrootsNostrRelayHint {
    type Error = RadrootsIdParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for RadrootsNostrRelayHint {
    type Error = RadrootsIdParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

fn validate_nostr_relay_hint(value: &str) -> Result<String, RadrootsIdParseError> {
    if value.is_empty() {
        return Err(RadrootsIdParseError::Empty);
    }
    if !value.bytes().all(|byte| matches!(byte, 0x21..=0x7e)) {
        return Err(RadrootsIdParseError::InvalidCharacter);
    }
    if value.bytes().any(|byte| matches!(byte, b'#' | b'\\')) {
        return Err(RadrootsIdParseError::InvalidFormat);
    }

    let remainder = value
        .strip_prefix("wss://")
        .or_else(|| value.strip_prefix("ws://"))
        .ok_or(RadrootsIdParseError::InvalidFormat)?;
    let authority_end = remainder
        .bytes()
        .position(|byte| matches!(byte, b'/' | b'?'))
        .unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty()
        || authority.bytes().any(|byte| byte == b'@')
        || matches!(remainder.as_bytes().first(), Some(b'/' | b'?'))
    {
        return Err(RadrootsIdParseError::InvalidFormat);
    }
    if !relay_authority_is_valid(authority)
        || !relay_path_and_query_are_valid(&remainder[authority_end..])
    {
        return Err(RadrootsIdParseError::InvalidFormat);
    }
    Ok(value.to_string())
}

fn relay_authority_is_valid(authority: &str) -> bool {
    if let Some(ipv6) = authority.strip_prefix('[') {
        let Some(closing_index) = ipv6.find(']') else {
            return false;
        };
        let address = &ipv6[..closing_index];
        let suffix = &ipv6[closing_index + 1..];
        return canonical_ipv6(address)
            && (suffix.is_empty() || suffix.strip_prefix(':').is_some_and(canonical_port));
    }

    let mut parts = authority.split(':');
    let host = parts.next().unwrap_or_default();
    let port = parts.next();
    if parts.next().is_some() || port.is_some_and(|port| !canonical_port(port)) || host.is_empty() {
        return false;
    }
    canonical_ipv4(host) || canonical_dns_host(host)
}

fn canonical_port(value: &str) -> bool {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || value.len() > 5
        || value.len() > 1 && value.starts_with('0')
    {
        return false;
    }
    value
        .parse::<u32>()
        .is_ok_and(|port| (1..=u16::MAX.into()).contains(&port))
}

fn canonical_ipv4(value: &str) -> bool {
    let mut count = 0usize;
    for part in value.split('.') {
        if part.is_empty()
            || !part.bytes().all(|byte| byte.is_ascii_digit())
            || part.len() > 1 && part.starts_with('0')
            || part.parse::<u8>().is_err()
        {
            return false;
        }
        count += 1;
    }
    count == 4
}

fn canonical_dns_host(value: &str) -> bool {
    if value.len() > 253 {
        return false;
    }
    let mut final_label = "";
    for label in value.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with("xn--")
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || !label
                .as_bytes()
                .first()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            || !label
                .as_bytes()
                .last()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            return false;
        }
        final_label = label;
    }
    !dns_label_is_whatwg_number(final_label)
}

fn dns_label_is_whatwg_number(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_digit())
        || value
            .strip_prefix("0x")
            .is_some_and(|digits| digits.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn canonical_ipv6(value: &str) -> bool {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f' | b':'))
    {
        return false;
    }
    let Ok(address) = value.parse::<Ipv6Addr>() else {
        return false;
    };
    canonical_ipv6_string(address) == value
}

fn canonical_ipv6_string(address: Ipv6Addr) -> String {
    let segments = address.segments();
    let mut best_start = 0usize;
    let mut best_len = 0usize;
    let mut index = 0usize;
    while index < segments.len() {
        if segments[index] != 0 {
            index += 1;
            continue;
        }
        let start = index;
        while index < segments.len() && segments[index] == 0 {
            index += 1;
        }
        let len = index - start;
        if len >= 2 && len > best_len {
            best_start = start;
            best_len = len;
        }
    }

    let mut canonical = String::new();
    if best_len == 0 {
        push_ipv6_segments(&mut canonical, &segments);
        return canonical;
    }
    push_ipv6_segments(&mut canonical, &segments[..best_start]);
    canonical.push_str("::");
    push_ipv6_segments(&mut canonical, &segments[best_start + best_len..]);
    canonical
}

fn push_ipv6_segments(output: &mut String, segments: &[u16]) {
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            output.push(':');
        }
        push_ipv6_segment(output, *segment);
    }
}

fn push_ipv6_segment(output: &mut String, segment: u16) {
    let mut shift = 12u32;
    while shift > 0 && segment >> shift == 0 {
        shift -= 4;
    }
    loop {
        let nibble = ((segment >> shift) & 0x0f) as u8;
        output.push(match nibble {
            0..=9 => (b'0' + nibble) as char,
            _ => (b'a' + nibble - 10) as char,
        });
        if shift == 0 {
            break;
        }
        shift -= 4;
    }
}

fn relay_path_and_query_are_valid(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if let Some(query) = value.strip_prefix('?') {
        return relay_component_is_valid(query, true);
    }
    let Some(path) = value.strip_prefix('/') else {
        return false;
    };
    let (path, query) = path
        .split_once('?')
        .map_or((path, None), |(path, query)| (path, Some(query)));
    relay_component_is_valid(path, false)
        && query.is_none_or(|query| relay_component_is_valid(query, true))
}

fn relay_component_is_valid(value: &str, query: bool) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !upper_hex_digit(bytes[index + 1])
                || !upper_hex_digit(bytes[index + 2])
            {
                return false;
            }
            index += 3;
            continue;
        }
        if !relay_pchar(bytes[index]) && !(query && bytes[index] == b'?') {
            return false;
        }
        index += 1;
    }
    true
}

fn relay_pchar(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
                | b'/'
        )
}

fn upper_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'A'..=b'F')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::RadrootsRelayUrl;

    #[test]
    fn canonical_relay_hints_accept_portable_hosts_paths_and_queries() {
        for value in [
            "wss://relay.example",
            "ws://127.0.0.1:21003",
            "wss://localhost",
            "wss://[::1]",
            "wss://[2001:db8::1]:65535/nostr/v1?region=ca-bc&next=%2Ffeed",
            "wss://[::ffff:c000:201]",
            "wss://relay.example:443?",
            "wss://relay.example/a/b:@!$&'()*+,;=~_-?next=/feed??page=1",
        ] {
            let relay = RadrootsNostrRelayHint::parse(value)
                .unwrap_or_else(|error| panic!("{value} must be canonical: {error}"));
            assert_eq!(relay.as_str(), value);
            assert_eq!(relay.to_string(), value);
            assert_eq!(
                value.parse::<RadrootsNostrRelayHint>().expect("FromStr"),
                relay
            );
        }

        let maximum_host = format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(61)
        );
        assert_eq!(maximum_host.len(), 253);
        RadrootsNostrRelayHint::parse(format!("wss://{maximum_host}")).expect("253-byte DNS host");
    }

    #[test]
    fn canonical_relay_hints_reject_normalizing_or_ambiguous_forms() {
        assert_eq!(
            RadrootsNostrRelayHint::parse("").unwrap_err(),
            RadrootsIdParseError::Empty
        );
        for value in [
            "WSS://relay.example",
            "https://relay.example",
            "wss://",
            "wss:///relay.example",
            "wss:////relay.example",
            "wss://?region=ca-bc",
            "wss://Relay.example",
            "wss://user@relay.example",
            "wss://@relay.example",
            "wss://relay.example#read",
            "wss://relay.example\\path",
            "wss://relay_example",
            "wss://-relay.example",
            "wss://relay-.example",
            "wss://relay..example",
            "wss://relay.example.",
            "wss://xn--fa-hia.example",
            "wss://example.999",
            "wss://example.0x",
            "wss://example.0x1",
            "wss://%65xample.com",
            "wss://127.1",
            "wss://2130706433",
            "wss://0x7f.1",
            "wss://01.2.3.4",
            "wss://256.1.1.1",
            "wss://[2001:DB8::1]",
            "wss://[2001:0db8::1]",
            "wss://[2001:db8:0:0:0:0:0:1]",
            "wss://[2001:0:0:1::1:1]",
            "wss://[::ffff:192.0.2.1]",
            "wss://[fe80::1%25en0]",
            "wss://[v1.foo]",
            "wss://2001:db8::1",
            "wss://relay.example:",
            "wss://relay.example:0",
            "wss://relay.example:01",
            "wss://relay.example:+443",
            "wss://relay.example:65536",
            "wss://relay.example/[raw]",
            "wss://relay.example/%",
            "wss://relay.example/%2",
            "wss://relay.example/%2f",
            "wss://relay.example/%GG",
        ] {
            assert_eq!(
                RadrootsNostrRelayHint::parse(value).unwrap_err(),
                RadrootsIdParseError::InvalidFormat,
                "{value}"
            );
        }

        for value in [
            "wss://relay.example path",
            "wss://relay.example/\u{007f}",
            "wss://relay.example/é",
        ] {
            assert_eq!(
                RadrootsNostrRelayHint::parse(value).unwrap_err(),
                RadrootsIdParseError::InvalidCharacter,
                "{value}"
            );
        }

        let label_too_long = "a".repeat(64);
        assert_eq!(
            RadrootsNostrRelayHint::parse(format!("wss://{label_too_long}.example")).unwrap_err(),
            RadrootsIdParseError::InvalidFormat
        );
        let host_too_long = format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(62)
        );
        assert_eq!(host_too_long.len(), 254);
        assert_eq!(
            RadrootsNostrRelayHint::parse(format!("wss://{host_too_long}")).unwrap_err(),
            RadrootsIdParseError::InvalidFormat
        );
    }

    #[test]
    fn generic_relay_url_remains_a_distinct_surface() {
        assert!(RadrootsRelayUrl::parse("wss://Relay.Example").is_ok());
        assert!(RadrootsNostrRelayHint::parse("wss://Relay.Example").is_err());
    }
}

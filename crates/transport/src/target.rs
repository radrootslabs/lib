use crate::{
    RadrootsTransportError, RadrootsTransportKind,
    endpoint::{ENDPOINT_URI_MAX_BYTES, TARGET_LABEL_MAX_BYTES, TARGET_SCOPE_MAX_BYTES},
};

const LEGACY_RETICULUM_ENDPOINT_URI: &str = "reticulum:local";
const LEGACY_RETICULUM_SCOPE_ID: &str = "local";
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::net::Ipv6Addr;
use core::str::FromStr;
use sha2::{Digest, Sha256};

/// Maximum number of targets in one operation.
pub const TARGET_SET_MAX_ITEMS: usize = 64;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Canonical transport endpoint URI.
pub struct EndpointUri(String);

impl EndpointUri {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let canonical = canonicalize_uri(raw.as_ref())?;
        Ok(Self(canonical))
    }

    fn parse_nostr_relay(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let canonical = canonicalize_nostr_relay_uri(raw.as_ref())?;
        Ok(Self(canonical))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for EndpointUri {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for EndpointUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for EndpointUri {
    type Err = RadrootsTransportError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for EndpointUri {
    type Error = RadrootsTransportError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for EndpointUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
        let parsed = Self::parse(raw.as_str()).map_err(serde::de::Error::custom)?;
        if parsed.as_str() != raw {
            return Err(serde::de::Error::custom(
                "transport target URI is not canonical",
            ));
        }
        Ok(parsed)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Optional transport-neutral target scope.
pub struct TargetScope(String);

impl TargetScope {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let value = raw.as_ref();
        if value.is_empty() {
            return Err(RadrootsTransportError::EmptyTargetScope);
        }
        if value.len() > TARGET_SCOPE_MAX_BYTES
            || value != value.trim()
            || value
                .chars()
                .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')))
        {
            return Err(RadrootsTransportError::InvalidTargetScope);
        }
        Ok(Self(value.to_string()))
    }

    pub fn local_reticulum() -> Self {
        Self::parse(LEGACY_RETICULUM_SCOPE_ID).expect("default Reticulum scope id")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for TargetScope {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TargetScope {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for TargetScope {
    type Err = RadrootsTransportError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for TargetScope {
    type Error = RadrootsTransportError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TargetScope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(raw).map_err(serde::de::Error::custom)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Optional human-readable target label excluded from target identity.
pub struct TargetLabel(String);

impl TargetLabel {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let raw = raw.as_ref();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(RadrootsTransportError::EmptyTargetLabel);
        }
        if raw.len() > TARGET_LABEL_MAX_BYTES || trimmed.chars().any(char::is_control) {
            return Err(RadrootsTransportError::InvalidTargetLabel);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for TargetLabel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TargetLabel {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for TargetLabel {
    type Err = RadrootsTransportError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for TargetLabel {
    type Error = RadrootsTransportError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TargetLabel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
        let parsed = Self::parse(raw.as_str()).map_err(serde::de::Error::custom)?;
        if parsed.as_str() != raw {
            return Err(serde::de::Error::custom(
                "transport target label is not canonical",
            ));
        }
        Ok(parsed)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Deterministic SHA-256 fingerprint of canonical target identity fields.
pub struct TargetFingerprint(String);

impl TargetFingerprint {
    pub fn from_target(
        kind: &RadrootsTransportKind,
        uri: &EndpointUri,
        scope: Option<&TargetScope>,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(kind.canonical_label().as_bytes());
        hasher.update([0]);
        hasher.update(uri.as_str().as_bytes());
        if let Some(scope) = scope {
            hasher.update([0]);
            hasher.update(scope.as_str().as_bytes());
        }
        let digest = hasher.finalize();
        Self(hex_encode(&digest))
    }

    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let raw = raw.as_ref();
        if raw.len() != 64 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(RadrootsTransportError::InvalidTargetFingerprint);
        }
        Ok(Self(raw.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

impl core::fmt::Display for TargetFingerprint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TargetFingerprint {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for TargetFingerprint {
    type Err = RadrootsTransportError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for TargetFingerprint {
    type Error = RadrootsTransportError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TargetFingerprint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
        let parsed = Self::parse(raw.as_str()).map_err(serde::de::Error::custom)?;
        if parsed.as_str() != raw {
            return Err(serde::de::Error::custom(
                "transport target fingerprint is not canonical",
            ));
        }
        Ok(parsed)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
/// Validated transport-neutral target.
pub struct Target {
    kind: RadrootsTransportKind,
    uri: EndpointUri,
    scope: Option<TargetScope>,
    label: Option<TargetLabel>,
    fingerprint: TargetFingerprint,
}

impl Target {
    pub fn new(
        kind: RadrootsTransportKind,
        uri: impl AsRef<str>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::new_with_metadata(kind, uri, None, None)
    }

    pub fn nostr_relay(uri: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        Self::nostr_relay_with_metadata(uri, None, None)
    }

    pub fn nostr_relay_with_metadata(
        uri: impl AsRef<str>,
        scope: Option<TargetScope>,
        label: Option<TargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::new_with_metadata(RadrootsTransportKind::Nostr, uri, scope, label)
    }

    pub fn reticulum() -> Result<Self, RadrootsTransportError> {
        Self::reticulum_with_metadata(LEGACY_RETICULUM_ENDPOINT_URI, None, None)
    }

    pub fn reticulum_with_metadata(
        uri: impl AsRef<str>,
        scope: Option<TargetScope>,
        label: Option<TargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::new_with_metadata(RadrootsTransportKind::Reticulum, uri, scope, label)
    }

    pub fn local(uri: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        Self::local_with_metadata(uri, None, None)
    }

    pub fn local_with_metadata(
        uri: impl AsRef<str>,
        scope: Option<TargetScope>,
        label: Option<TargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::new_with_metadata(RadrootsTransportKind::Local, uri, scope, label)
    }

    pub fn new_with_metadata(
        kind: RadrootsTransportKind,
        uri: impl AsRef<str>,
        scope: Option<TargetScope>,
        label: Option<TargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        let raw_uri = uri.as_ref();
        let uri = match kind {
            RadrootsTransportKind::Nostr => EndpointUri::parse_nostr_relay(raw_uri)?,
            _ => EndpointUri::parse(raw_uri)?,
        };
        if kind == RadrootsTransportKind::Reticulum && raw_uri != LEGACY_RETICULUM_ENDPOINT_URI {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        let scope = scope.or_else(|| default_scope_for_kind(&kind));
        let fingerprint = TargetFingerprint::from_target(&kind, &uri, scope.as_ref());
        Ok(Self {
            kind,
            uri,
            scope,
            label,
            fingerprint,
        })
    }

    pub fn kind(&self) -> &RadrootsTransportKind {
        &self.kind
    }

    pub fn uri(&self) -> &EndpointUri {
        &self.uri
    }

    pub fn scope(&self) -> Option<&TargetScope> {
        self.scope.as_ref()
    }

    pub fn label(&self) -> Option<&TargetLabel> {
        self.label.as_ref()
    }

    pub fn fingerprint(&self) -> &TargetFingerprint {
        &self.fingerprint
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetWire {
    kind: RadrootsTransportKind,
    uri: String,
    scope: Option<String>,
    label: Option<String>,
    fingerprint: String,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Target {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = TargetWire::deserialize(deserializer)?;
        let scope = wire
            .scope
            .map(TargetScope::parse)
            .transpose()
            .map_err(serde::de::Error::custom)?;
        let label = wire
            .label
            .map(|label| {
                let parsed = TargetLabel::parse(label.as_str())?;
                if parsed.as_str() != label {
                    return Err(RadrootsTransportError::InvalidTargetLabel);
                }
                Ok(parsed)
            })
            .transpose()
            .map_err(serde::de::Error::custom)?;
        let fingerprint = TargetFingerprint::parse(wire.fingerprint.as_str())
            .map_err(serde::de::Error::custom)?;
        if fingerprint.as_str() != wire.fingerprint {
            return Err(serde::de::Error::custom(
                "transport target fingerprint is not canonical",
            ));
        }
        let target =
            Self::new_with_metadata(wire.kind, wire.uri.as_str(), scope.clone(), label.clone())
                .map_err(serde::de::Error::custom)?;
        if target.uri.as_str() != wire.uri
            || target.scope != scope
            || target.label != label
            || target.fingerprint != fingerprint
        {
            return Err(serde::de::Error::custom(
                "transport target identity does not match its canonical fields",
            ));
        }
        Ok(target)
    }
}

fn default_scope_for_kind(kind: &RadrootsTransportKind) -> Option<TargetScope> {
    (*kind == RadrootsTransportKind::Reticulum).then(TargetScope::local_reticulum)
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
/// Bounded non-empty set of targets with unique fingerprints.
pub struct TargetSet {
    targets: Vec<Target>,
}

impl TargetSet {
    pub fn new(targets: Vec<Target>) -> Result<Self, RadrootsTransportError> {
        if targets.is_empty() {
            return Err(RadrootsTransportError::EmptyTargetSet);
        }
        if targets.len() > TARGET_SET_MAX_ITEMS {
            return Err(RadrootsTransportError::TargetSetTooLarge);
        }
        let mut fingerprints = BTreeSet::new();
        for target in &targets {
            if !fingerprints.insert(target.fingerprint.as_str().to_string()) {
                return Err(RadrootsTransportError::DuplicateTargetFingerprint);
            }
        }
        Ok(Self { targets })
    }

    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetSetWire {
    targets: Vec<Target>,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TargetSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = TargetSetWire::deserialize(deserializer)?;
        Self::new(wire.targets).map_err(serde::de::Error::custom)
    }
}

fn canonicalize_uri(raw: &str) -> Result<String, RadrootsTransportError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(RadrootsTransportError::EmptyTargetUri);
    }
    if raw != trimmed {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    if trimmed.len() > ENDPOINT_URI_MAX_BYTES {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    if trimmed
        .chars()
        .any(|ch| ch.is_ascii_control() || ch.is_ascii_whitespace())
    {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    if let Some(colon) = trimmed.find(':') {
        let scheme = &trimmed[..colon];
        if !is_valid_scheme(scheme) {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        let rest = &trimmed[colon + 1..];
        if let Some(authority_rest) = rest.strip_prefix("//") {
            let authority_end = authority_rest
                .find(['/', '?', '#'])
                .unwrap_or(authority_rest.len());
            let authority = &authority_rest[..authority_end];
            let suffix = &authority_rest[authority_end..];
            return Ok(format!(
                "{}://{}{}",
                scheme.to_ascii_lowercase(),
                authority.to_ascii_lowercase(),
                suffix
            ));
        }
        return Ok(format!("{}:{rest}", scheme.to_ascii_lowercase()));
    }
    Ok(trimmed.to_string())
}

fn is_valid_scheme(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

fn canonicalize_nostr_relay_uri(raw: &str) -> Result<String, RadrootsTransportError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(RadrootsTransportError::EmptyTargetUri);
    }
    if raw != trimmed {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    if trimmed.len() > ENDPOINT_URI_MAX_BYTES {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    if trimmed
        .chars()
        .any(|ch| ch.is_ascii_control() || ch.is_ascii_whitespace())
    {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    if trimmed.contains('?') || trimmed.contains('#') || trimmed.contains('\\') {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    let Some(scheme_end) = trimmed.find("://") else {
        return Err(RadrootsTransportError::InvalidTargetUri);
    };
    let scheme = trimmed[..scheme_end].to_ascii_lowercase();
    if !matches!(scheme.as_str(), "wss" | "ws") {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    let endpoint = &trimmed[scheme_end + 3..];
    let authority_end = endpoint.find('/').unwrap_or(endpoint.len());
    let authority = &endpoint[..authority_end];
    let path = &endpoint[authority_end..];
    let authority = canonicalize_nostr_relay_authority(authority, scheme.as_str())?;
    validate_nostr_relay_path(path)?;
    if path == "/" {
        return Ok(format!("{scheme}://{authority}"));
    }
    Ok(format!("{scheme}://{authority}{path}"))
}

fn canonicalize_nostr_relay_authority(
    authority: &str,
    scheme: &str,
) -> Result<String, RadrootsTransportError> {
    if authority.is_empty() || authority.contains('@') {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let Some(host_end) = rest.find(']') else {
            return Err(RadrootsTransportError::InvalidTargetUri);
        };
        let host = &rest[..host_end];
        let suffix = &rest[host_end + 1..];
        if host.is_empty()
            || host
                .chars()
                .any(|ch| matches!(ch, '[' | ']' | '/' | '?' | '#' | '@' | '\\'))
        {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        let canonical_host = canonicalize_nostr_relay_ipv6(host)?;
        (
            format!("[{canonical_host}]"),
            parse_nostr_relay_port(suffix)?,
        )
    } else {
        if authority.contains(['[', ']', '\\']) {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        let mut parts = authority.splitn(2, ':');
        let host = parts.next().unwrap_or_default();
        let port = parts
            .next()
            .map(parse_nostr_relay_port_with_prefix)
            .transpose()?;
        (canonicalize_nostr_relay_host(host)?, port)
    };
    if scheme == "ws" && !is_local_ws_relay_host(host.as_str()) {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    let port =
        port.filter(|port| !matches!((scheme, port.as_str()), ("wss", "443") | ("ws", "80")));
    Ok(match port {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

fn parse_nostr_relay_port(suffix: &str) -> Result<Option<String>, RadrootsTransportError> {
    if suffix.is_empty() {
        return Ok(None);
    }
    let Some(port) = suffix.strip_prefix(':') else {
        return Err(RadrootsTransportError::InvalidTargetUri);
    };
    parse_nostr_relay_port_with_prefix(port).map(Some)
}

fn parse_nostr_relay_port_with_prefix(port: &str) -> Result<String, RadrootsTransportError> {
    if port.is_empty()
        || !port.bytes().all(|byte| byte.is_ascii_digit())
        || port.len() > 1 && port.starts_with('0')
    {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    let value = port
        .parse::<u32>()
        .map_err(|_| RadrootsTransportError::InvalidTargetUri)?;
    if !(1..=u16::MAX as u32).contains(&value) {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    Ok(port.to_string())
}

fn canonicalize_nostr_relay_host(host: &str) -> Result<String, RadrootsTransportError> {
    let canonical = host.to_ascii_lowercase();
    if canonical_ipv4(canonical.as_str()) || canonical_dns_host(canonical.as_str()) {
        return Ok(canonical);
    }
    Err(RadrootsTransportError::InvalidTargetUri)
}

fn canonicalize_nostr_relay_ipv6(host: &str) -> Result<String, RadrootsTransportError> {
    if host.is_empty()
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f' | b'A'..=b'F' | b':'))
    {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    host.parse::<Ipv6Addr>()
        .map(|address| address.to_string())
        .map_err(|_| RadrootsTransportError::InvalidTargetUri)
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
    if value.is_empty() || value.len() > 253 {
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

fn validate_nostr_relay_path(path: &str) -> Result<(), RadrootsTransportError> {
    if path.is_empty() || path == "/" {
        return Ok(());
    }
    let Some(component) = path.strip_prefix('/') else {
        return Err(RadrootsTransportError::InvalidTargetUri);
    };
    if relay_path_component_is_valid(component) {
        Ok(())
    } else {
        Err(RadrootsTransportError::InvalidTargetUri)
    }
}

fn relay_path_component_is_valid(value: &str) -> bool {
    if value.split('/').any(relay_path_segment_is_dot) {
        return false;
    }
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
        if !relay_path_character(bytes[index]) {
            return false;
        }
        index += 1;
    }
    true
}

fn relay_path_segment_is_dot(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    let mut index = 0usize;
    let mut dots = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'.' {
            dots += 1;
            index += 1;
        } else if bytes[index..].starts_with(b"%2E") {
            dots += 1;
            index += 3;
        } else {
            return false;
        }
    }
    matches!(dots, 1 | 2)
}

fn relay_path_character(byte: u8) -> bool {
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

fn is_local_ws_relay_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// Compatibility endpoint URI name retained until the workspace cutover.
#[doc(hidden)]
pub type RadrootsTransportTargetUri = EndpointUri;

/// Compatibility target scope name retained until the workspace cutover.
#[doc(hidden)]
pub type RadrootsTransportMeshScopeId = TargetScope;

/// Compatibility target label name retained until the workspace cutover.
#[doc(hidden)]
pub type RadrootsTransportTargetLabel = TargetLabel;

/// Compatibility target fingerprint name retained until the workspace cutover.
#[doc(hidden)]
pub type RadrootsTransportTargetFingerprint = TargetFingerprint;

/// Compatibility target name retained until the workspace cutover.
#[doc(hidden)]
pub type RadrootsTransportTarget = Target;

/// Compatibility target-set name retained until the workspace cutover.
#[doc(hidden)]
pub type RadrootsTransportTargetSet = TargetSet;

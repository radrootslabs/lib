use crate::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_SCOPE_ID,
    RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES, RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES,
    RADROOTS_TRANSPORT_TARGET_MAX_COUNT, RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES,
    RadrootsTransportError, RadrootsTransportKind, limits::ensure_resource_limit,
};
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::net::Ipv6Addr;
use sha2::{Digest, Sha256};

pub const RADROOTS_TRANSPORT_TARGET_FINGERPRINT_BYTES: usize = 64;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsTransportTargetUri(String);

impl RadrootsTransportTargetUri {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        ensure_resource_limit(
            "target_uri",
            raw.as_ref().len(),
            RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES,
        )?;
        let canonical = canonicalize_uri(raw.as_ref())?;
        Ok(Self(canonical))
    }

    fn parse_nostr_relay(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        ensure_resource_limit(
            "target_uri",
            raw.as_ref().len(),
            RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES,
        )?;
        let canonical = canonicalize_nostr_relay_uri(raw.as_ref())?;
        Ok(Self(canonical))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for RadrootsTransportTargetUri {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportTargetUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = crate::serde_bounds::deserialize_string(
            deserializer,
            "target_uri",
            RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES,
        )?;
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
pub struct RadrootsTransportMeshScopeId(String);

impl RadrootsTransportMeshScopeId {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let value = raw.as_ref();
        if value.is_empty() {
            return Err(RadrootsTransportError::EmptyTargetScope);
        }
        ensure_resource_limit(
            "target_scope",
            value.len(),
            RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES,
        )?;
        if value != value.trim()
            || value
                .chars()
                .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')))
        {
            return Err(RadrootsTransportError::InvalidTargetScope);
        }
        Ok(Self(value.to_string()))
    }

    pub fn local_reticulum() -> Self {
        Self::parse(RADROOTS_RETICULUM_SCOPE_ID).expect("default Reticulum scope id")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for RadrootsTransportMeshScopeId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportMeshScopeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = crate::serde_bounds::deserialize_string(
            deserializer,
            "target_scope",
            RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES,
        )?;
        Self::parse(raw).map_err(serde::de::Error::custom)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsTransportTargetLabel(String);

impl RadrootsTransportTargetLabel {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let raw = raw.as_ref();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(RadrootsTransportError::EmptyTargetLabel);
        }
        ensure_resource_limit(
            "target_label",
            raw.len(),
            RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES,
        )?;
        if raw != trimmed || trimmed.chars().any(char::is_control) {
            return Err(RadrootsTransportError::InvalidTargetLabel);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for RadrootsTransportTargetLabel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportTargetLabel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = crate::serde_bounds::deserialize_string(
            deserializer,
            "target_label",
            RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES,
        )?;
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
pub struct RadrootsTransportTargetFingerprint(String);

impl RadrootsTransportTargetFingerprint {
    pub fn from_target(
        kind: &RadrootsTransportKind,
        uri: &RadrootsTransportTargetUri,
        scope: Option<&RadrootsTransportMeshScopeId>,
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
        if raw.len() != RADROOTS_TRANSPORT_TARGET_FINGERPRINT_BYTES
            || !raw.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
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

impl core::fmt::Display for RadrootsTransportTargetFingerprint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportTargetFingerprint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = crate::serde_bounds::deserialize_string(
            deserializer,
            "target_fingerprint",
            RADROOTS_TRANSPORT_TARGET_FINGERPRINT_BYTES,
        )?;
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
pub struct RadrootsTransportTarget {
    kind: RadrootsTransportKind,
    uri: RadrootsTransportTargetUri,
    scope: Option<RadrootsTransportMeshScopeId>,
    label: Option<RadrootsTransportTargetLabel>,
    fingerprint: RadrootsTransportTargetFingerprint,
}

impl RadrootsTransportTarget {
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
        scope: Option<RadrootsTransportMeshScopeId>,
        label: Option<RadrootsTransportTargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::new_with_metadata(RadrootsTransportKind::Nostr, uri, scope, label)
    }

    pub fn reticulum() -> Result<Self, RadrootsTransportError> {
        Self::reticulum_with_metadata(RADROOTS_RETICULUM_ENDPOINT_URI, None, None)
    }

    pub fn reticulum_with_metadata(
        uri: impl AsRef<str>,
        scope: Option<RadrootsTransportMeshScopeId>,
        label: Option<RadrootsTransportTargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::new_with_metadata(RadrootsTransportKind::Reticulum, uri, scope, label)
    }

    pub fn local(uri: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        Self::local_with_metadata(uri, None, None)
    }

    pub fn local_with_metadata(
        uri: impl AsRef<str>,
        scope: Option<RadrootsTransportMeshScopeId>,
        label: Option<RadrootsTransportTargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::new_with_metadata(RadrootsTransportKind::Local, uri, scope, label)
    }

    pub fn new_with_metadata(
        kind: RadrootsTransportKind,
        uri: impl AsRef<str>,
        scope: Option<RadrootsTransportMeshScopeId>,
        label: Option<RadrootsTransportTargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        let raw_uri = uri.as_ref();
        let uri = match kind {
            RadrootsTransportKind::Nostr => RadrootsTransportTargetUri::parse_nostr_relay(raw_uri)?,
            _ => RadrootsTransportTargetUri::parse(raw_uri)?,
        };
        if kind == RadrootsTransportKind::Reticulum && raw_uri != RADROOTS_RETICULUM_ENDPOINT_URI {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        let scope = scope.or_else(|| default_scope_for_kind(&kind));
        let fingerprint =
            RadrootsTransportTargetFingerprint::from_target(&kind, &uri, scope.as_ref());
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

    pub fn uri(&self) -> &RadrootsTransportTargetUri {
        &self.uri
    }

    pub fn scope(&self) -> Option<&RadrootsTransportMeshScopeId> {
        self.scope.as_ref()
    }

    pub fn label(&self) -> Option<&RadrootsTransportTargetLabel> {
        self.label.as_ref()
    }

    pub fn fingerprint(&self) -> &RadrootsTransportTargetFingerprint {
        &self.fingerprint
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsTransportTargetWire {
    kind: RadrootsTransportKind,
    #[serde(deserialize_with = "deserialize_target_uri")]
    uri: String,
    #[serde(deserialize_with = "deserialize_target_scope")]
    scope: Option<String>,
    #[serde(deserialize_with = "deserialize_target_label")]
    label: Option<String>,
    #[serde(deserialize_with = "deserialize_target_fingerprint")]
    fingerprint: String,
}

#[cfg(feature = "serde")]
fn deserialize_target_uri<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_string(
        deserializer,
        "target_uri",
        RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES,
    )
}

#[cfg(feature = "serde")]
fn deserialize_target_scope<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_option_string(
        deserializer,
        "target_scope",
        RADROOTS_TRANSPORT_TARGET_SCOPE_MAX_BYTES,
    )
}

#[cfg(feature = "serde")]
fn deserialize_target_label<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_option_string(
        deserializer,
        "target_label",
        RADROOTS_TRANSPORT_TARGET_LABEL_MAX_BYTES,
    )
}

#[cfg(feature = "serde")]
fn deserialize_target_fingerprint<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_string(
        deserializer,
        "target_fingerprint",
        RADROOTS_TRANSPORT_TARGET_FINGERPRINT_BYTES,
    )
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RadrootsTransportTargetWire::deserialize(deserializer)?;
        let scope = wire
            .scope
            .map(RadrootsTransportMeshScopeId::parse)
            .transpose()
            .map_err(serde::de::Error::custom)?;
        let label = wire
            .label
            .map(|label| {
                let parsed = RadrootsTransportTargetLabel::parse(label.as_str())?;
                if parsed.as_str() != label {
                    return Err(RadrootsTransportError::InvalidTargetLabel);
                }
                Ok(parsed)
            })
            .transpose()
            .map_err(serde::de::Error::custom)?;
        let fingerprint = RadrootsTransportTargetFingerprint::parse(wire.fingerprint.as_str())
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

fn default_scope_for_kind(kind: &RadrootsTransportKind) -> Option<RadrootsTransportMeshScopeId> {
    (*kind == RadrootsTransportKind::Reticulum).then(RadrootsTransportMeshScopeId::local_reticulum)
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportTargetSet {
    targets: Vec<RadrootsTransportTarget>,
}

impl RadrootsTransportTargetSet {
    pub fn new(targets: Vec<RadrootsTransportTarget>) -> Result<Self, RadrootsTransportError> {
        if targets.is_empty() {
            return Err(RadrootsTransportError::EmptyTargetSet);
        }
        ensure_resource_limit(
            "target_count",
            targets.len(),
            RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
        )?;
        let mut fingerprints = BTreeSet::new();
        for target in &targets {
            if !fingerprints.insert(target.fingerprint.as_str().to_string()) {
                return Err(RadrootsTransportError::DuplicateTargetFingerprint);
            }
        }
        Ok(Self { targets })
    }

    pub fn targets(&self) -> &[RadrootsTransportTarget] {
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
struct RadrootsTransportTargetSetWire {
    #[serde(deserialize_with = "deserialize_targets")]
    targets: Vec<RadrootsTransportTarget>,
}

#[cfg(feature = "serde")]
fn deserialize_targets<'de, D>(deserializer: D) -> Result<Vec<RadrootsTransportTarget>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_vec(
        deserializer,
        "target_count",
        RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
    )
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportTargetSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RadrootsTransportTargetSetWire::deserialize(deserializer)?;
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

use crate::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_SCOPE_ID, RadrootsTransportError,
    RadrootsTransportKind,
};
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsTransportTargetUri(String);

impl RadrootsTransportTargetUri {
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

impl core::fmt::Display for RadrootsTransportTargetUri {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsTransportMeshScopeId(String);

impl RadrootsTransportMeshScopeId {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let value = raw.as_ref();
        if value.is_empty() {
            return Err(RadrootsTransportError::EmptyTargetScope);
        }
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsTransportTargetLabel(String);

impl RadrootsTransportTargetLabel {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsTransportError> {
        let trimmed = raw.as_ref().trim();
        if trimmed.is_empty() {
            return Err(RadrootsTransportError::EmptyTargetLabel);
        }
        if trimmed.chars().any(char::is_control) {
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

impl core::fmt::Display for RadrootsTransportTargetFingerprint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportTarget {
    pub kind: RadrootsTransportKind,
    pub uri: RadrootsTransportTargetUri,
    pub scope: Option<RadrootsTransportMeshScopeId>,
    pub label: Option<RadrootsTransportTargetLabel>,
    pub fingerprint: RadrootsTransportTargetFingerprint,
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
        if kind == RadrootsTransportKind::Reticulum
            && (raw_uri != RADROOTS_RETICULUM_ENDPOINT_URI
                || uri.as_str() != RADROOTS_RETICULUM_ENDPOINT_URI)
        {
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
}

fn default_scope_for_kind(kind: &RadrootsTransportKind) -> Option<RadrootsTransportMeshScopeId> {
    (*kind == RadrootsTransportKind::Reticulum).then(RadrootsTransportMeshScopeId::local_reticulum)
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportTargetSet {
    targets: Vec<RadrootsTransportTarget>,
}

impl RadrootsTransportTargetSet {
    pub fn new(targets: Vec<RadrootsTransportTarget>) -> Result<Self, RadrootsTransportError> {
        if targets.is_empty() {
            return Err(RadrootsTransportError::EmptyTargetSet);
        }
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
    if trimmed.contains('?') || trimmed.contains('#') {
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
                .any(|ch| matches!(ch, '[' | ']' | '/' | '?' | '#' | '@'))
        {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        (
            format!("[{}]", host.to_ascii_lowercase()),
            parse_nostr_relay_port(suffix)?,
        )
    } else {
        if authority.contains(['[', ']']) {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        let mut parts = authority.splitn(2, ':');
        let host = parts.next().unwrap_or_default();
        let port = parts
            .next()
            .map(parse_nostr_relay_port_with_prefix)
            .transpose()?;
        if host.is_empty() || !is_valid_nostr_relay_host(host) {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        (host.to_ascii_lowercase(), port)
    };
    if scheme == "ws" && !is_local_ws_relay_host(host.as_str()) {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
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
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    let value = port
        .parse::<u32>()
        .map_err(|_| RadrootsTransportError::InvalidTargetUri)?;
    if value > u16::MAX as u32 {
        return Err(RadrootsTransportError::InvalidTargetUri);
    }
    Ok(port.to_string())
}

fn is_valid_nostr_relay_host(host: &str) -> bool {
    if host.contains("..") || host.starts_with('.') || host.ends_with('.') {
        return false;
    }
    host.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
}

fn is_local_ws_relay_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

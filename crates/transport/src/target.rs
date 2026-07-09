use crate::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RADROOTS_RETICULUM_PREVIEW_SCOPE_ID,
    RadrootsTransportError, RadrootsTransportKind,
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

    pub fn local_preview() -> Self {
        Self::parse(RADROOTS_RETICULUM_PREVIEW_SCOPE_ID)
            .expect("default Reticulum preview scope id")
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

    pub fn new_with_metadata(
        kind: RadrootsTransportKind,
        uri: impl AsRef<str>,
        scope: Option<RadrootsTransportMeshScopeId>,
        label: Option<RadrootsTransportTargetLabel>,
    ) -> Result<Self, RadrootsTransportError> {
        let raw_uri = uri.as_ref();
        if kind == RadrootsTransportKind::Reticulum
            && raw_uri != RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
        {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        let uri = RadrootsTransportTargetUri::parse(raw_uri)?;
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
    (*kind == RadrootsTransportKind::Reticulum).then(RadrootsTransportMeshScopeId::local_preview)
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

use std::{collections::BTreeSet, fmt, fmt::Write as _, fs, io::Read as _, path::Path};

use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use sha2::{Digest, Sha256};

const CONTRACT_RELATIVE: &str =
    "contracts/architecture/decisions/services_hardening_source_lock.v2.json";
const LOCK_SCHEMA: &str = "radroots.service.source-lock.v2";
pub(crate) const LOCK_FILENAME: &str = "radroots.service.source-lock.v2.toml";
pub(crate) const PREDECESSOR_LOCK_FILENAME: &str = "radroots.service.source-lock.v1.toml";
pub(crate) const LIB_REPOSITORY: &str = "https://github.com/radrootslabs/lib";
const ARCHITECTURE: &str = "radroots.crates.release.v2";
const LIB_VERSION: &str = "0.1.0-alpha";
const RUST_VERSION: &str = "1.97.1";
const HOST_FEATURE_PROFILE: &str = "service-host";
const MAX_LOCK_BYTES: usize = 4096;
const MAX_SERVICE_BYTES: usize = 128;
const MAX_CONTRACT_BYTES: usize = 32_768;
const MAX_FLAKE_NIX_BYTES: usize = 1_048_576;
const MAX_FLAKE_LOCK_BYTES: usize = 4_194_304;

const DEFERRED_FIELD_ORDER: [&str; 20] = [
    "schema",
    "contract_version",
    "service",
    "repository",
    "revision",
    "architecture",
    "workspace_catalog_sha256",
    "version",
    "source_archive_sha256",
    "cargo_lock_sha256",
    "rust_version",
    "host_feature_profile",
    "nix.material",
    "nix.lib_revision",
    "nix.flake_lock_sha256",
    "contract_versions.config",
    "contract_versions.state",
    "contract_versions.admin",
    "contract_versions.status",
    "contract_versions.provider",
];

const ABSENT_FIELD_ORDER: [&str; 18] = [
    "schema",
    "contract_version",
    "service",
    "repository",
    "revision",
    "architecture",
    "workspace_catalog_sha256",
    "version",
    "source_archive_sha256",
    "cargo_lock_sha256",
    "rust_version",
    "host_feature_profile",
    "nix.material",
    "contract_versions.config",
    "contract_versions.state",
    "contract_versions.admin",
    "contract_versions.status",
    "contract_versions.provider",
];

const ERROR_CODES: [&str; 11] = [
    "invalid_contract_version",
    "invalid_digest",
    "invalid_feature_profile",
    "invalid_fixed_identity",
    "invalid_nix_material",
    "invalid_revision",
    "invalid_service",
    "invalid_toolchain",
    "malformed",
    "noncanonical",
    "too_large",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServiceSourceLockError {
    TooLarge,
    Malformed,
    Noncanonical,
    InvalidFixedIdentity,
    InvalidNixMaterial,
    InvalidService,
    InvalidRevision,
    InvalidDigest,
    InvalidToolchain,
    InvalidFeatureProfile,
    InvalidContractVersion,
}

impl ServiceSourceLockError {
    fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "too_large",
            Self::Malformed => "malformed",
            Self::Noncanonical => "noncanonical",
            Self::InvalidFixedIdentity => "invalid_fixed_identity",
            Self::InvalidNixMaterial => "invalid_nix_material",
            Self::InvalidService => "invalid_service",
            Self::InvalidRevision => "invalid_revision",
            Self::InvalidDigest => "invalid_digest",
            Self::InvalidToolchain => "invalid_toolchain",
            Self::InvalidFeatureProfile => "invalid_feature_profile",
            Self::InvalidContractVersion => "invalid_contract_version",
        }
    }
}

impl fmt::Display for ServiceSourceLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "service source lock exceeds its byte limit",
            Self::Malformed => "service source lock is malformed",
            Self::Noncanonical => "service source lock is not canonical",
            Self::InvalidFixedIdentity => "service source lock identity is invalid",
            Self::InvalidNixMaterial => "service source lock Nix material is invalid",
            Self::InvalidService => "service source lock service is invalid",
            Self::InvalidRevision => "service source lock revision is invalid",
            Self::InvalidDigest => "service source lock digest is invalid",
            Self::InvalidToolchain => "service source lock toolchain is invalid",
            Self::InvalidFeatureProfile => "service source lock feature profile is invalid",
            Self::InvalidContractVersion => "service source lock contract version is invalid",
        })
    }
}

impl std::error::Error for ServiceSourceLockError {}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContractVersions {
    config: u32,
    state: u32,
    admin: u32,
    status: u32,
    provider: u32,
}

impl ContractVersions {
    pub(crate) const fn new(
        config: u32,
        state: u32,
        admin: u32,
        status: u32,
        provider: u32,
    ) -> Self {
        Self {
            config,
            state,
            admin,
            status,
            provider,
        }
    }

    fn is_valid(self) -> bool {
        self.config != 0
            && self.state != 0
            && self.admin != 0
            && self.status != 0
            && self.provider != 0
    }

    pub(crate) const fn config(self) -> u32 {
        self.config
    }

    pub(crate) const fn state(self) -> u32 {
        self.state
    }

    pub(crate) const fn admin(self) -> u32 {
        self.admin
    }

    pub(crate) const fn status(self) -> u32 {
        self.status
    }

    pub(crate) const fn provider(self) -> u32 {
        self.provider
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawServiceSourceLock {
    schema: String,
    contract_version: u32,
    service: String,
    repository: String,
    revision: String,
    architecture: String,
    workspace_catalog_sha256: String,
    version: String,
    source_archive_sha256: String,
    cargo_lock_sha256: String,
    nix: RawNixMaterial,
    rust_version: String,
    host_feature_profile: String,
    contract_versions: ContractVersions,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "material", rename_all = "snake_case", deny_unknown_fields)]
enum RawNixMaterial {
    Absent,
    Deferred {
        lib_revision: String,
        flake_lock_sha256: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NixMaterialParts<'a> {
    Absent,
    Deferred {
        lib_revision: &'a str,
        flake_lock_sha256: &'a str,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NixMaterialState {
    Absent,
    Deferred,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeferredNixMaterialEvidence {
    lib_revision: String,
    flake_lock_sha256: String,
}

impl DeferredNixMaterialEvidence {
    pub(crate) fn lib_revision(&self) -> &str {
        &self.lib_revision
    }

    pub(crate) fn flake_lock_sha256(&self) -> &str {
        &self.flake_lock_sha256
    }
}

pub(crate) fn validate_deferred_nix_material(
    expression: &[u8],
    lock: &[u8],
) -> Result<DeferredNixMaterialEvidence, ServiceSourceLockError> {
    if expression.len() > MAX_FLAKE_NIX_BYTES || lock.len() > MAX_FLAKE_LOCK_BYTES {
        return Err(ServiceSourceLockError::InvalidNixMaterial);
    }
    let revision = validate_deferred_nix_lock(lock)?;
    let text =
        std::str::from_utf8(expression).map_err(|_| ServiceSourceLockError::InvalidNixMaterial)?;
    let expected = format!("url = \"github:radrootslabs/lib/{revision}\";");
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let direct_blocks = lines
        .windows(4)
        .filter(|window| {
            window[0] == "inputs.lib = {"
                && window[1] == expected
                && window[2] == "flake = false;"
                && window[3] == "};"
        })
        .count();
    let nested_blocks = lines
        .windows(5)
        .filter(|window| {
            window[0] == "inputs = {"
                && window[1] == "lib = {"
                && window[2] == expected
                && window[3] == "flake = false;"
                && window[4] == "};"
        })
        .count();
    let selecting_lines = lines
        .iter()
        .filter(|line| line.contains("github:radrootslabs/lib"))
        .count();
    if direct_blocks + nested_blocks != 1 || selecting_lines != 1 {
        return Err(ServiceSourceLockError::InvalidNixMaterial);
    }
    Ok(DeferredNixMaterialEvidence {
        lib_revision: revision,
        flake_lock_sha256: hex::encode(Sha256::digest(lock)),
    })
}

pub(crate) fn validate_deferred_nix_lock(bytes: &[u8]) -> Result<String, ServiceSourceLockError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = NoDuplicateJsonValue::deserialize(&mut deserializer)
        .map_err(|_| ServiceSourceLockError::InvalidNixMaterial)?
        .0;
    deserializer
        .end()
        .map_err(|_| ServiceSourceLockError::InvalidNixMaterial)?;
    if value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect::<BTreeSet<_>>())
        != Some(BTreeSet::from(["nodes", "root", "version"]))
    {
        return Err(ServiceSourceLockError::InvalidNixMaterial);
    }
    let version = value.get("version").and_then(serde_json::Value::as_u64);
    let root_name = value.get("root").and_then(serde_json::Value::as_str);
    let nodes = value.get("nodes").and_then(serde_json::Value::as_object);
    if version != Some(7) || root_name.is_none_or(str::is_empty) || nodes.is_none() {
        return Err(ServiceSourceLockError::InvalidNixMaterial);
    }
    let nodes = nodes.ok_or(ServiceSourceLockError::InvalidNixMaterial)?;
    let root = nodes
        .get(root_name.ok_or(ServiceSourceLockError::InvalidNixMaterial)?)
        .and_then(|node| node.get("inputs"))
        .and_then(serde_json::Value::as_object)
        .ok_or(ServiceSourceLockError::InvalidNixMaterial)?;
    let direct_lib_node = root.get("lib").and_then(serde_json::Value::as_str);
    let mut lib_nodes = 0_usize;
    let mut exact_direct = 0_usize;
    let mut selected_revision = None;
    for (name, node) in nodes {
        let locked = node.get("locked").and_then(serde_json::Value::as_object);
        let original = node.get("original").and_then(serde_json::Value::as_object);
        let is_lib = locked.is_some_and(|locked| {
            locked.get("owner").and_then(serde_json::Value::as_str) == Some("radrootslabs")
                && locked.get("repo").and_then(serde_json::Value::as_str) == Some("lib")
        }) || original.is_some_and(|original| {
            original.get("owner").and_then(serde_json::Value::as_str) == Some("radrootslabs")
                && original.get("repo").and_then(serde_json::Value::as_str) == Some("lib")
        });
        if !is_lib {
            continue;
        }
        lib_nodes += 1;
        let locked = locked.ok_or(ServiceSourceLockError::InvalidNixMaterial)?;
        let original = original.ok_or(ServiceSourceLockError::InvalidNixMaterial)?;
        let locked_keys = locked.keys().map(String::as_str).collect::<BTreeSet<_>>();
        let original_keys = original.keys().map(String::as_str).collect::<BTreeSet<_>>();
        let locked_revision = locked.get("rev").and_then(serde_json::Value::as_str);
        let original_revision = original.get("rev").and_then(serde_json::Value::as_str);
        let exact = locked_keys
            == BTreeSet::from(["lastModified", "narHash", "owner", "repo", "rev", "type"])
            && original_keys == BTreeSet::from(["owner", "repo", "rev", "type"])
            && node.as_object().is_some_and(|node| {
                node.keys().map(String::as_str).collect::<BTreeSet<_>>()
                    == BTreeSet::from(["locked", "original"])
            })
            && locked
                .get("lastModified")
                .and_then(serde_json::Value::as_u64)
                .is_some()
            && locked.get("type").and_then(serde_json::Value::as_str) == Some("github")
            && locked.get("owner").and_then(serde_json::Value::as_str) == Some("radrootslabs")
            && locked.get("repo").and_then(serde_json::Value::as_str) == Some("lib")
            && locked_revision.is_some_and(|revision| valid_lower_hex(revision, 40))
            && locked
                .get("narHash")
                .and_then(serde_json::Value::as_str)
                .is_some_and(valid_nix_sha256)
            && original.get("type").and_then(serde_json::Value::as_str) == Some("github")
            && original.get("owner").and_then(serde_json::Value::as_str) == Some("radrootslabs")
            && original.get("repo").and_then(serde_json::Value::as_str) == Some("lib")
            && original_revision == locked_revision
            && original.get("ref").is_none();
        if direct_lib_node == Some(name) && root_name != Some(name.as_str()) && exact {
            exact_direct += 1;
            selected_revision = locked_revision.map(str::to_owned);
        }
    }
    if lib_nodes == 1 && exact_direct == 1 {
        selected_revision.ok_or(ServiceSourceLockError::InvalidNixMaterial)
    } else {
        Err(ServiceSourceLockError::InvalidNixMaterial)
    }
}

struct NoDuplicateJsonValue(serde_json::Value);

impl<'de> Deserialize<'de> for NoDuplicateJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(NoDuplicateJsonValueVisitor)
    }
}

struct NoDuplicateJsonValueVisitor;

impl<'de> Visitor<'de> for NoDuplicateJsonValueVisitor {
    type Value = NoDuplicateJsonValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(serde_json::Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(serde_json::Value::Number(
            value.into(),
        )))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(serde_json::Value::Number(
            value.into(),
        )))
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom("unsupported JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(serde_json::Value::String(
            value.to_owned(),
        )))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(serde_json::Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(serde_json::Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateJsonValue(serde_json::Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<NoDuplicateJsonValue>()? {
            values.push(value.0);
        }
        Ok(NoDuplicateJsonValue(serde_json::Value::Array(values)))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(<A::Error as de::Error>::custom("duplicate JSON object key"));
            }
            let value = map.next_value::<NoDuplicateJsonValue>()?;
            values.insert(key, value.0);
        }
        Ok(NoDuplicateJsonValue(serde_json::Value::Object(values)))
    }
}

fn valid_nix_sha256(value: &str) -> bool {
    let Some(encoded) = value.strip_prefix("sha256-") else {
        return false;
    };
    let bytes = encoded.as_bytes();
    bytes.len() == 44
        && bytes[43] == b'='
        && bytes[..43]
            .iter()
            .copied()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
}

pub(crate) struct ServiceSourceLockParts<'a> {
    pub(crate) service: &'a str,
    pub(crate) revision: &'a str,
    pub(crate) workspace_catalog_sha256: &'a str,
    pub(crate) source_archive_sha256: &'a str,
    pub(crate) cargo_lock_sha256: &'a str,
    pub(crate) nix: NixMaterialParts<'a>,
    pub(crate) contract_versions: ContractVersions,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct ServiceSourceLockV2 {
    raw: RawServiceSourceLock,
    canonical: Box<[u8]>,
}

impl fmt::Debug for ServiceSourceLockV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceSourceLockV2")
            .finish_non_exhaustive()
    }
}

impl ServiceSourceLockV2 {
    pub(crate) fn new(parts: ServiceSourceLockParts<'_>) -> Result<Self, ServiceSourceLockError> {
        validate_parts(&parts)?;
        let raw = RawServiceSourceLock {
            schema: LOCK_SCHEMA.to_owned(),
            contract_version: 2,
            service: parts.service.to_owned(),
            repository: LIB_REPOSITORY.to_owned(),
            revision: parts.revision.to_owned(),
            architecture: ARCHITECTURE.to_owned(),
            workspace_catalog_sha256: parts.workspace_catalog_sha256.to_owned(),
            version: LIB_VERSION.to_owned(),
            source_archive_sha256: parts.source_archive_sha256.to_owned(),
            cargo_lock_sha256: parts.cargo_lock_sha256.to_owned(),
            nix: match parts.nix {
                NixMaterialParts::Absent => RawNixMaterial::Absent,
                NixMaterialParts::Deferred {
                    lib_revision,
                    flake_lock_sha256,
                } => RawNixMaterial::Deferred {
                    lib_revision: lib_revision.to_owned(),
                    flake_lock_sha256: flake_lock_sha256.to_owned(),
                },
            },
            rust_version: RUST_VERSION.to_owned(),
            host_feature_profile: HOST_FEATURE_PROFILE.to_owned(),
            contract_versions: parts.contract_versions,
        };
        Ok(Self::from_validated_raw(raw))
    }

    pub(crate) fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, ServiceSourceLockError> {
        if bytes.len() > MAX_LOCK_BYTES {
            return Err(ServiceSourceLockError::TooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| ServiceSourceLockError::Malformed)?;
        let raw = toml::from_str::<RawServiceSourceLock>(text)
            .map_err(|_| ServiceSourceLockError::Malformed)?;
        validate_raw(&raw)?;
        let lock = Self::from_validated_raw(raw);
        if lock.canonical.as_ref() != bytes {
            return Err(ServiceSourceLockError::Noncanonical);
        }
        Ok(lock)
    }

    fn from_validated_raw(raw: RawServiceSourceLock) -> Self {
        let canonical = render(&raw).into_bytes().into_boxed_slice();
        debug_assert!(canonical.len() <= MAX_LOCK_BYTES);
        Self { raw, canonical }
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        debug_assert_eq!(self.raw.schema, LOCK_SCHEMA);
        &self.canonical
    }

    pub(crate) fn service(&self) -> &str {
        &self.raw.service
    }

    pub(crate) fn revision(&self) -> &str {
        &self.raw.revision
    }

    pub(crate) fn workspace_catalog_sha256(&self) -> &str {
        &self.raw.workspace_catalog_sha256
    }

    pub(crate) fn source_archive_sha256(&self) -> &str {
        &self.raw.source_archive_sha256
    }

    pub(crate) fn cargo_lock_sha256(&self) -> &str {
        &self.raw.cargo_lock_sha256
    }

    pub(crate) const fn nix_material_state(&self) -> NixMaterialState {
        match &self.raw.nix {
            RawNixMaterial::Absent => NixMaterialState::Absent,
            RawNixMaterial::Deferred { .. } => NixMaterialState::Deferred,
        }
    }

    pub(crate) fn nix_lib_revision(&self) -> Option<&str> {
        match &self.raw.nix {
            RawNixMaterial::Absent => None,
            RawNixMaterial::Deferred { lib_revision, .. } => Some(lib_revision),
        }
    }

    pub(crate) fn flake_lock_sha256(&self) -> Option<&str> {
        match &self.raw.nix {
            RawNixMaterial::Absent => None,
            RawNixMaterial::Deferred {
                flake_lock_sha256, ..
            } => Some(flake_lock_sha256),
        }
    }

    pub(crate) const fn contract_versions(&self) -> ContractVersions {
        self.raw.contract_versions
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceLockDecision {
    schema: String,
    contract_version: u32,
    decision_state: String,
    predecessor: PredecessorDecision,
    lock_filename: String,
    lock_schema: String,
    canonical_encoding: String,
    maximum_lock_utf8_bytes: usize,
    maximum_service_utf8_bytes: usize,
    canonical_field_order_deferred: Vec<String>,
    canonical_field_order_absent: Vec<String>,
    fixed: FixedDecision,
    revision_encoding: String,
    digest_encoding: String,
    digest_subjects: DigestSubjects,
    service_identifier: String,
    contract_version_rule: String,
    negative_error_codes: Vec<String>,
    canonical_vectors: CanonicalVectors,
    operations: OperationsDecision,
    deferred_operations: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredecessorDecision {
    schema: String,
    filename: String,
    transition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationsDecision {
    command: String,
    modes: Vec<String>,
    required_arguments: Vec<String>,
    service_metadata_path: String,
    service_metadata_fields: Vec<String>,
    lib_dependency_inventory: String,
    source_cleanliness: String,
    predecessor_lock_presence: String,
    service_revision_stability: String,
    active_revision_agreement: Vec<String>,
    nix_material_states: Vec<String>,
    deferred_nix_agreement: Vec<String>,
    maximum_source_archive_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DigestSubjects {
    workspace_catalog_sha256: String,
    source_archive_sha256: String,
    cargo_lock_sha256: String,
    flake_lock_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixedDecision {
    repository: String,
    architecture: String,
    version: String,
    rust_version: String,
    host_feature_profile: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CanonicalVector {
    toml: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CanonicalVectors {
    deferred: CanonicalVector,
    absent: CanonicalVector,
}

pub(crate) fn validate_contract(workspace_root: &Path) -> Result<(), String> {
    validate_contract_inner(workspace_root).map_err(|error| error.to_string())
}

fn validate_contract_inner(workspace_root: &Path) -> Result<(), ServiceSourceLockError> {
    let path = workspace_root.join(CONTRACT_RELATIVE);
    let metadata = fs::symlink_metadata(&path).map_err(|_| ServiceSourceLockError::Malformed)?;
    if !metadata.is_file() || metadata.len() > MAX_CONTRACT_BYTES as u64 {
        return Err(ServiceSourceLockError::Malformed);
    }
    let file = fs::File::open(path).map_err(|_| ServiceSourceLockError::Malformed)?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_CONTRACT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ServiceSourceLockError::Malformed)?;
    if bytes.len() > MAX_CONTRACT_BYTES {
        return Err(ServiceSourceLockError::Malformed);
    }
    let decision = serde_json::from_slice::<SourceLockDecision>(&bytes)
        .map_err(|_| ServiceSourceLockError::Malformed)?;
    validate_decision(&decision)?;

    for (vector, expected) in [
        (
            &decision.canonical_vectors.deferred,
            canonical_deferred_vector(),
        ),
        (
            &decision.canonical_vectors.absent,
            canonical_absent_vector(),
        ),
    ] {
        let parsed = ServiceSourceLockV2::from_canonical_bytes(vector.toml.as_bytes())?;
        if parsed != expected
            || hex::encode(Sha256::digest(parsed.canonical_bytes())) != vector.sha256
        {
            return Err(ServiceSourceLockError::Noncanonical);
        }
    }
    Ok(())
}

fn validate_decision(decision: &SourceLockDecision) -> Result<(), ServiceSourceLockError> {
    let error_codes = [
        ServiceSourceLockError::InvalidContractVersion,
        ServiceSourceLockError::InvalidDigest,
        ServiceSourceLockError::InvalidFeatureProfile,
        ServiceSourceLockError::InvalidFixedIdentity,
        ServiceSourceLockError::InvalidNixMaterial,
        ServiceSourceLockError::InvalidRevision,
        ServiceSourceLockError::InvalidService,
        ServiceSourceLockError::InvalidToolchain,
        ServiceSourceLockError::Malformed,
        ServiceSourceLockError::Noncanonical,
        ServiceSourceLockError::TooLarge,
    ]
    .map(ServiceSourceLockError::code);
    let exact = decision.schema == "radroots.services-hardening.source-lock-decisions.v2"
        && decision.contract_version == 2
        && decision.decision_state == "active"
        && decision.predecessor.schema == "radroots.service.source-lock.v1"
        && decision.predecessor.filename == PREDECESSOR_LOCK_FILENAME
        && decision.predecessor.transition == "forward_only_replace"
        && decision.lock_filename == LOCK_FILENAME
        && decision.lock_schema == LOCK_SCHEMA
        && decision.canonical_encoding == "compact_canonical_toml_with_final_newline"
        && decision.maximum_lock_utf8_bytes == MAX_LOCK_BYTES
        && decision.maximum_service_utf8_bytes == MAX_SERVICE_BYTES
        && decision.canonical_field_order_deferred == DEFERRED_FIELD_ORDER
        && decision.canonical_field_order_absent == ABSENT_FIELD_ORDER
        && decision.fixed.repository == LIB_REPOSITORY
        && decision.fixed.architecture == ARCHITECTURE
        && decision.fixed.version == LIB_VERSION
        && decision.fixed.rust_version == RUST_VERSION
        && decision.fixed.host_feature_profile == HOST_FEATURE_PROFILE
        && decision.revision_encoding == "git_oid_lowercase_hex_40"
        && decision.digest_encoding == "sha256_lowercase_hex_64"
        && decision.digest_subjects.workspace_catalog_sha256
            == "lib/contracts/crates/catalog.v2.toml"
        && decision.digest_subjects.source_archive_sha256
            == "canonical_lib_revision_source_archive"
        && decision.digest_subjects.cargo_lock_sha256 == "service/Cargo.lock"
        && decision.digest_subjects.flake_lock_sha256 == "service/flake.lock"
        && decision.service_identifier
            == "ascii_lower_snake_case_starting_with_letter_ending_with_letter_or_digit_no_empty_segments_1_to_128_bytes"
        && decision.contract_version_rule == "u32_nonzero"
        && error_codes == ERROR_CODES
        && decision.negative_error_codes == error_codes
        && decision.operations.command == "cargo xtask service-source-lock"
        && decision.operations.modes == ["check", "write"]
        && decision.operations.required_arguments == ["mode", "service_root", "source_archive"]
        && decision.operations.service_metadata_path
            == "Cargo.toml.workspace.metadata.radroots.service_source_lock"
        && decision.operations.service_metadata_fields
            == [
                "service",
                "host_feature_profile",
                "nix_material",
                "config_contract_version",
                "state_contract_version",
                "admin_contract_version",
                "status_contract_version",
                "provider_contract_version",
            ]
        && decision.operations.lib_dependency_inventory
            == "verified_source_archive_workspace_catalog"
        && decision.operations.source_cleanliness
            == "all_changes_forbidden_except_exact_generated_lock_path"
        && decision.operations.predecessor_lock_presence == "forbidden"
        && decision.operations.service_revision_stability
            == "same_head_before_and_after_evidence_and_output"
        && decision.operations.active_revision_agreement
            == [
                "cargo_manifests",
                "cargo_lock",
                "source_archive",
                "canonical_public_remote",
            ]
        && decision.operations.nix_material_states == ["absent", "deferred"]
        && decision.operations.deferred_nix_agreement
            == [
                "flake_expression",
                "flake_lock",
                "source_lock_nix_revision",
                "canonical_public_remote",
            ]
        && decision.operations.maximum_source_archive_bytes == 1_073_741_824
        && decision.deferred_operations
            == [
                "embedded_build_information_agreement",
                "nix_qualification",
                "nix_active_revision_alignment",
            ];
    if exact {
        Ok(())
    } else {
        Err(ServiceSourceLockError::InvalidFixedIdentity)
    }
}

fn canonical_deferred_vector() -> ServiceSourceLockV2 {
    ServiceSourceLockV2::new(ServiceSourceLockParts {
        service: "fixture_service",
        revision: "2222222222222222222222222222222222222222",
        workspace_catalog_sha256: "2222222222222222222222222222222222222222222222222222222222222222",
        source_archive_sha256: "3333333333333333333333333333333333333333333333333333333333333333",
        cargo_lock_sha256: "3f32f227550b26ffccf6ee73ceab7471b3d8ce40b3e7c345d2ed65af7e9affa0",
        nix: NixMaterialParts::Deferred {
            lib_revision: "1111111111111111111111111111111111111111",
            flake_lock_sha256: "13638c254efcc7ccc5798242d2c095934e84fbc406a9af244fc754b18a6f9353",
        },
        contract_versions: ContractVersions::new(1, 2, 3, 4, 5),
    })
    .expect("the governed source-lock vector is valid")
}

fn canonical_absent_vector() -> ServiceSourceLockV2 {
    ServiceSourceLockV2::new(ServiceSourceLockParts {
        service: "fixture_service",
        revision: "2222222222222222222222222222222222222222",
        workspace_catalog_sha256: "2222222222222222222222222222222222222222222222222222222222222222",
        source_archive_sha256: "3333333333333333333333333333333333333333333333333333333333333333",
        cargo_lock_sha256: "3f32f227550b26ffccf6ee73ceab7471b3d8ce40b3e7c345d2ed65af7e9affa0",
        nix: NixMaterialParts::Absent,
        contract_versions: ContractVersions::new(1, 2, 3, 4, 5),
    })
    .expect("the governed absent-Nix source-lock vector is valid")
}

fn validate_raw(raw: &RawServiceSourceLock) -> Result<(), ServiceSourceLockError> {
    if raw.schema != LOCK_SCHEMA
        || raw.contract_version != 2
        || raw.repository != LIB_REPOSITORY
        || raw.architecture != ARCHITECTURE
        || raw.version != LIB_VERSION
    {
        return Err(ServiceSourceLockError::InvalidFixedIdentity);
    }
    if raw.rust_version != RUST_VERSION {
        return Err(ServiceSourceLockError::InvalidToolchain);
    }
    if raw.host_feature_profile != HOST_FEATURE_PROFILE {
        return Err(ServiceSourceLockError::InvalidFeatureProfile);
    }
    validate_parts(&ServiceSourceLockParts {
        service: &raw.service,
        revision: &raw.revision,
        workspace_catalog_sha256: &raw.workspace_catalog_sha256,
        source_archive_sha256: &raw.source_archive_sha256,
        cargo_lock_sha256: &raw.cargo_lock_sha256,
        nix: match &raw.nix {
            RawNixMaterial::Absent => NixMaterialParts::Absent,
            RawNixMaterial::Deferred {
                lib_revision,
                flake_lock_sha256,
            } => NixMaterialParts::Deferred {
                lib_revision,
                flake_lock_sha256,
            },
        },
        contract_versions: raw.contract_versions,
    })
}

fn validate_parts(parts: &ServiceSourceLockParts<'_>) -> Result<(), ServiceSourceLockError> {
    if !valid_service(parts.service) {
        return Err(ServiceSourceLockError::InvalidService);
    }
    if !valid_lower_hex(parts.revision, 40) {
        return Err(ServiceSourceLockError::InvalidRevision);
    }
    if ![
        parts.workspace_catalog_sha256,
        parts.source_archive_sha256,
        parts.cargo_lock_sha256,
    ]
    .into_iter()
    .all(|value| valid_lower_hex(value, 64))
    {
        return Err(ServiceSourceLockError::InvalidDigest);
    }
    if let NixMaterialParts::Deferred {
        lib_revision,
        flake_lock_sha256,
    } = parts.nix
        && (!valid_lower_hex(lib_revision, 40) || !valid_lower_hex(flake_lock_sha256, 64))
    {
        return Err(ServiceSourceLockError::InvalidNixMaterial);
    }
    if !parts.contract_versions.is_valid() {
        return Err(ServiceSourceLockError::InvalidContractVersion);
    }
    Ok(())
}

fn valid_service(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_SERVICE_BYTES
        && bytes[0].is_ascii_lowercase()
        && (bytes[bytes.len() - 1].is_ascii_lowercase() || bytes[bytes.len() - 1].is_ascii_digit())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
        && !bytes.windows(2).any(|pair| pair == b"__")
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn render(raw: &RawServiceSourceLock) -> String {
    let mut output = String::with_capacity(1024);
    writeln!(output, "schema = \"{}\"", raw.schema).expect("render to String");
    writeln!(output, "contract_version = {}", raw.contract_version).expect("render to String");
    writeln!(output, "service = \"{}\"", raw.service).expect("render to String");
    writeln!(output, "repository = \"{}\"", raw.repository).expect("render to String");
    writeln!(output, "revision = \"{}\"", raw.revision).expect("render to String");
    writeln!(output, "architecture = \"{}\"", raw.architecture).expect("render to String");
    writeln!(
        output,
        "workspace_catalog_sha256 = \"{}\"",
        raw.workspace_catalog_sha256
    )
    .expect("render to String");
    writeln!(output, "version = \"{}\"", raw.version).expect("render to String");
    writeln!(
        output,
        "source_archive_sha256 = \"{}\"",
        raw.source_archive_sha256
    )
    .expect("render to String");
    writeln!(output, "cargo_lock_sha256 = \"{}\"", raw.cargo_lock_sha256)
        .expect("render to String");
    writeln!(output, "rust_version = \"{}\"", raw.rust_version).expect("render to String");
    writeln!(
        output,
        "host_feature_profile = \"{}\"",
        raw.host_feature_profile
    )
    .expect("render to String");
    output.push_str("\n[nix]\n");
    match &raw.nix {
        RawNixMaterial::Absent => {
            output.push_str("material = \"absent\"\n");
        }
        RawNixMaterial::Deferred {
            lib_revision,
            flake_lock_sha256,
        } => {
            output.push_str("material = \"deferred\"\n");
            writeln!(output, "lib_revision = \"{lib_revision}\"").expect("render to String");
            writeln!(output, "flake_lock_sha256 = \"{flake_lock_sha256}\"")
                .expect("render to String");
        }
    }
    output.push_str("\n[contract_versions]\n");
    writeln!(output, "config = {}", raw.contract_versions.config).expect("render to String");
    writeln!(output, "state = {}", raw.contract_versions.state).expect("render to String");
    writeln!(output, "admin = {}", raw.contract_versions.admin).expect("render to String");
    writeln!(output, "status = {}", raw.contract_versions.status).expect("render to String");
    writeln!(output, "provider = {}", raw.contract_versions.provider).expect("render to String");
    output
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use crate::workspace_root;

    use super::*;

    #[test]
    fn canonical_vectors_round_trip_with_exact_digests() {
        let lock = canonical_deferred_vector();
        assert_eq!(
            hex::encode(Sha256::digest(lock.canonical_bytes())),
            "da7b8894a6480e7022d9937369a5c9bafbf90128a901652923e501c52aced1a2"
        );
        assert_eq!(
            ServiceSourceLockV2::from_canonical_bytes(lock.canonical_bytes()),
            Ok(lock)
        );
        let lock = canonical_absent_vector();
        assert_eq!(
            hex::encode(Sha256::digest(lock.canonical_bytes())),
            "2af058ba042509c77efd6dc264c60a7abf4371378223e52e34eb441cab7e263c"
        );
        assert_eq!(
            ServiceSourceLockV2::from_canonical_bytes(lock.canonical_bytes()),
            Ok(lock)
        );
    }

    #[test]
    fn decision_rejects_every_independent_governed_field_drift() {
        let bytes = fs::read(workspace_root().join(CONTRACT_RELATIVE)).expect("decision");
        let canonical = serde_json::from_slice::<serde_json::Value>(&bytes).expect("decision json");
        for (pointer, replacement) in [
            ("/schema", serde_json::json!("other")),
            ("/contract_version", serde_json::json!(1)),
            ("/decision_state", serde_json::json!("draft")),
            ("/predecessor/schema", serde_json::json!("other")),
            ("/predecessor/filename", serde_json::json!("other")),
            ("/predecessor/transition", serde_json::json!("other")),
            ("/lock_filename", serde_json::json!("other")),
            ("/lock_schema", serde_json::json!("other")),
            ("/canonical_encoding", serde_json::json!("other")),
            ("/maximum_lock_utf8_bytes", serde_json::json!(1)),
            ("/maximum_service_utf8_bytes", serde_json::json!(1)),
            ("/canonical_field_order_deferred", serde_json::json!([])),
            ("/canonical_field_order_absent", serde_json::json!([])),
            ("/fixed/repository", serde_json::json!("other")),
            ("/fixed/architecture", serde_json::json!("other")),
            ("/fixed/version", serde_json::json!("other")),
            ("/fixed/rust_version", serde_json::json!("other")),
            ("/fixed/host_feature_profile", serde_json::json!("other")),
            ("/revision_encoding", serde_json::json!("other")),
            ("/digest_encoding", serde_json::json!("other")),
            (
                "/digest_subjects/workspace_catalog_sha256",
                serde_json::json!("other"),
            ),
            (
                "/digest_subjects/source_archive_sha256",
                serde_json::json!("other"),
            ),
            (
                "/digest_subjects/cargo_lock_sha256",
                serde_json::json!("other"),
            ),
            (
                "/digest_subjects/flake_lock_sha256",
                serde_json::json!("other"),
            ),
            ("/service_identifier", serde_json::json!("other")),
            ("/contract_version_rule", serde_json::json!("other")),
            ("/negative_error_codes", serde_json::json!([])),
            ("/operations/command", serde_json::json!("other")),
            ("/operations/modes", serde_json::json!([])),
            ("/operations/required_arguments", serde_json::json!([])),
            (
                "/operations/service_metadata_path",
                serde_json::json!("other"),
            ),
            ("/operations/service_metadata_fields", serde_json::json!([])),
            (
                "/operations/lib_dependency_inventory",
                serde_json::json!("other"),
            ),
            ("/operations/source_cleanliness", serde_json::json!("other")),
            (
                "/operations/predecessor_lock_presence",
                serde_json::json!("other"),
            ),
            (
                "/operations/service_revision_stability",
                serde_json::json!("other"),
            ),
            (
                "/operations/active_revision_agreement",
                serde_json::json!([]),
            ),
            ("/operations/nix_material_states", serde_json::json!([])),
            ("/operations/deferred_nix_agreement", serde_json::json!([])),
            (
                "/operations/maximum_source_archive_bytes",
                serde_json::json!(1),
            ),
            ("/deferred_operations", serde_json::json!(["future"])),
        ] {
            let mut drifted = canonical.clone();
            *drifted.pointer_mut(pointer).expect("governed field") = replacement;
            let decision = serde_json::from_value::<SourceLockDecision>(drifted)
                .expect("structurally valid drift");
            assert_eq!(
                validate_decision(&decision),
                Err(ServiceSourceLockError::InvalidFixedIdentity),
                "accepted drift at {pointer}"
            );
        }
    }

    #[test]
    fn parser_rejects_noncanonical_and_ambiguous_toml() {
        let canonical = String::from_utf8(canonical_deferred_vector().canonical_bytes().to_vec())
            .expect("canonical UTF-8");
        for malformed in [
            canonical.replacen("schema =", "unknown = 1\nschema =", 1),
            canonical.replacen(
                "contract_version = 2\nservice = \"fixture_service\"",
                "service = \"fixture_service\"\ncontract_version = 2",
                1,
            ),
            format!(" {canonical}"),
            canonical.replacen("schema =", "schema=", 1),
            canonical.replacen("contract_version = 2", "contract_version = 02", 1),
        ] {
            assert!(matches!(
                ServiceSourceLockV2::from_canonical_bytes(malformed.as_bytes()),
                Err(ServiceSourceLockError::Malformed | ServiceSourceLockError::Noncanonical)
            ));
        }
        let duplicate = canonical.replacen(
            "service = \"fixture_service\"",
            "service = \"fixture_service\"\nservice = \"fixture_service\"",
            1,
        );
        assert_eq!(
            ServiceSourceLockV2::from_canonical_bytes(duplicate.as_bytes()),
            Err(ServiceSourceLockError::Malformed)
        );
    }

    #[test]
    fn nix_material_variants_are_closed_and_independently_bounded() {
        let absent = canonical_absent_vector();
        assert_eq!(absent.nix_material_state(), NixMaterialState::Absent);
        assert_eq!(absent.nix_lib_revision(), None);
        assert_eq!(absent.flake_lock_sha256(), None);

        let deferred = canonical_deferred_vector();
        assert_eq!(deferred.nix_material_state(), NixMaterialState::Deferred);
        assert_eq!(
            deferred.nix_lib_revision(),
            Some("1111111111111111111111111111111111111111")
        );
        assert_eq!(
            deferred.flake_lock_sha256(),
            Some("13638c254efcc7ccc5798242d2c095934e84fbc406a9af244fc754b18a6f9353")
        );

        let absent_text = String::from_utf8(absent.canonical_bytes().to_vec()).expect("UTF-8");
        let deferred_text = String::from_utf8(deferred.canonical_bytes().to_vec()).expect("UTF-8");
        for malformed in [
            absent_text.replacen(
                "material = \"absent\"",
                "material = \"absent\"\nlib_revision = \"1111111111111111111111111111111111111111\"",
                1,
            ),
            absent_text.replacen("material = \"absent\"", "material = \"other\"", 1),
            deferred_text.replacen(
                "lib_revision = \"1111111111111111111111111111111111111111\"\n",
                "",
                1,
            ),
            deferred_text.replacen(
                "flake_lock_sha256 = \"13638c254efcc7ccc5798242d2c095934e84fbc406a9af244fc754b18a6f9353\"\n",
                "",
                1,
            ),
        ] {
            assert!(matches!(
                ServiceSourceLockV2::from_canonical_bytes(malformed.as_bytes()),
                Err(ServiceSourceLockError::Malformed | ServiceSourceLockError::Noncanonical)
            ));
        }

        assert_eq!(
            validate_deferred_nix_material(&vec![b'x'; MAX_FLAKE_NIX_BYTES + 1], b"{}"),
            Err(ServiceSourceLockError::InvalidNixMaterial)
        );
        assert_eq!(
            validate_deferred_nix_material(b"", &vec![b'x'; MAX_FLAKE_LOCK_BYTES + 1]),
            Err(ServiceSourceLockError::InvalidNixMaterial)
        );
    }

    #[test]
    fn parser_rejects_every_identity_and_bound_drift() {
        let canonical = String::from_utf8(canonical_deferred_vector().canonical_bytes().to_vec())
            .expect("canonical UTF-8");
        let cases = [
            (
                "radroots.service.source-lock.v2",
                "wrong",
                ServiceSourceLockError::InvalidFixedIdentity,
            ),
            (
                "contract_version = 2",
                "contract_version = 1",
                ServiceSourceLockError::InvalidFixedIdentity,
            ),
            (
                LIB_REPOSITORY,
                "https://example.invalid/lib",
                ServiceSourceLockError::InvalidFixedIdentity,
            ),
            (
                ARCHITECTURE,
                "radroots.crates.release.v1",
                ServiceSourceLockError::InvalidFixedIdentity,
            ),
            (
                LIB_VERSION,
                "0.1.1",
                ServiceSourceLockError::InvalidFixedIdentity,
            ),
            (
                "fixture_service",
                "Fixture",
                ServiceSourceLockError::InvalidService,
            ),
            (
                "service = \"fixture_service\"",
                "service = \"\"",
                ServiceSourceLockError::InvalidService,
            ),
            (
                "2222222222222222222222222222222222222222",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                ServiceSourceLockError::InvalidRevision,
            ),
            (
                "2222222222222222222222222222222222222222",
                "222222222222222222222222222222222222222",
                ServiceSourceLockError::InvalidRevision,
            ),
            (
                "2222222222222222222222222222222222222222222222222222222222222222",
                "GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG",
                ServiceSourceLockError::InvalidDigest,
            ),
            (
                "3333333333333333333333333333333333333333333333333333333333333333",
                "333333333333333333333333333333333333333333333333333333333333333",
                ServiceSourceLockError::InvalidDigest,
            ),
            (
                "3f32f227550b26ffccf6ee73ceab7471b3d8ce40b3e7c345d2ed65af7e9affa0",
                "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
                ServiceSourceLockError::InvalidDigest,
            ),
            (
                "13638c254efcc7ccc5798242d2c095934e84fbc406a9af244fc754b18a6f9353",
                "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
                ServiceSourceLockError::InvalidNixMaterial,
            ),
            (
                "1111111111111111111111111111111111111111",
                "invalid",
                ServiceSourceLockError::InvalidNixMaterial,
            ),
            (
                RUST_VERSION,
                "1.97.0",
                ServiceSourceLockError::InvalidToolchain,
            ),
            (
                HOST_FEATURE_PROFILE,
                "default",
                ServiceSourceLockError::InvalidFeatureProfile,
            ),
            (
                "config = 1",
                "config = 0",
                ServiceSourceLockError::InvalidContractVersion,
            ),
            (
                "state = 2",
                "state = 0",
                ServiceSourceLockError::InvalidContractVersion,
            ),
            (
                "admin = 3",
                "admin = 0",
                ServiceSourceLockError::InvalidContractVersion,
            ),
            (
                "status = 4",
                "status = 0",
                ServiceSourceLockError::InvalidContractVersion,
            ),
            (
                "provider = 5",
                "provider = 0",
                ServiceSourceLockError::InvalidContractVersion,
            ),
        ];
        for (from, to, expected) in cases {
            let mutated = canonical.replacen(from, to, 1);
            assert_eq!(
                ServiceSourceLockV2::from_canonical_bytes(mutated.as_bytes()),
                Err(expected)
            );
        }

        let too_large = vec![b' '; MAX_LOCK_BYTES + 1];
        assert_eq!(
            ServiceSourceLockV2::from_canonical_bytes(&too_large),
            Err(ServiceSourceLockError::TooLarge)
        );
    }

    #[test]
    fn exact_service_and_contract_version_maxima_are_admitted() {
        let service = "a".repeat(MAX_SERVICE_BYTES);
        let maximum = ServiceSourceLockV2::new(ServiceSourceLockParts {
            service: &service,
            revision: &"a".repeat(40),
            workspace_catalog_sha256: &"b".repeat(64),
            source_archive_sha256: &"c".repeat(64),
            cargo_lock_sha256: &"d".repeat(64),
            nix: NixMaterialParts::Deferred {
                lib_revision: &"f".repeat(40),
                flake_lock_sha256: &"e".repeat(64),
            },
            contract_versions: ContractVersions::new(
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
            ),
        })
        .expect("maximum lock");
        assert!(maximum.canonical_bytes().len() <= MAX_LOCK_BYTES);

        let overlong_service = "a".repeat(MAX_SERVICE_BYTES + 1);
        assert_eq!(
            ServiceSourceLockV2::new(ServiceSourceLockParts {
                service: &overlong_service,
                revision: &"a".repeat(40),
                workspace_catalog_sha256: &"b".repeat(64),
                source_archive_sha256: &"c".repeat(64),
                cargo_lock_sha256: &"d".repeat(64),
                nix: NixMaterialParts::Absent,
                contract_versions: ContractVersions::new(1, 1, 1, 1, 1),
            }),
            Err(ServiceSourceLockError::InvalidService)
        );

        for invalid in [
            "1service",
            "_service",
            "service_",
            "service__name",
            "service-name",
        ] {
            assert_eq!(
                ServiceSourceLockV2::new(ServiceSourceLockParts {
                    service: invalid,
                    revision: &"a".repeat(40),
                    workspace_catalog_sha256: &"b".repeat(64),
                    source_archive_sha256: &"c".repeat(64),
                    cargo_lock_sha256: &"d".repeat(64),
                    nix: NixMaterialParts::Absent,
                    contract_versions: ContractVersions::new(1, 1, 1, 1, 1),
                }),
                Err(ServiceSourceLockError::InvalidService)
            );
        }
    }

    #[test]
    fn diagnostics_are_fixed_and_source_free() {
        let lock = canonical_deferred_vector();
        let debug = format!("{lock:?}");
        assert_eq!(debug, "ServiceSourceLockV2 { .. }");
        for sensitive in [
            "fixture_service",
            "radrootslabs",
            "1111111111111111111111111111111111111111",
            "2222222222222222222222222222222222222222222222222222222222222222",
        ] {
            assert!(!debug.contains(sensitive));
        }
        let errors = [
            ServiceSourceLockError::TooLarge,
            ServiceSourceLockError::Malformed,
            ServiceSourceLockError::Noncanonical,
            ServiceSourceLockError::InvalidFixedIdentity,
            ServiceSourceLockError::InvalidNixMaterial,
            ServiceSourceLockError::InvalidService,
            ServiceSourceLockError::InvalidRevision,
            ServiceSourceLockError::InvalidDigest,
            ServiceSourceLockError::InvalidToolchain,
            ServiceSourceLockError::InvalidFeatureProfile,
            ServiceSourceLockError::InvalidContractVersion,
        ];
        for error in errors {
            assert!(error.source().is_none());
            assert!(!error.to_string().contains("fixture_service"));
        }
        let mut codes = errors.map(ServiceSourceLockError::code);
        codes.sort_unstable();
        assert_eq!(codes, ERROR_CODES);
    }
}

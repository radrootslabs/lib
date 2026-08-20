use std::{fmt, fmt::Write as _, fs, io::Read as _, path::Path};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const CONTRACT_RELATIVE: &str =
    "contracts/architecture/decisions/services_hardening_source_lock.v1.json";
const LOCK_SCHEMA: &str = "radroots.service.source-lock.v1";
pub(crate) const LOCK_FILENAME: &str = "radroots.service.source-lock.v1.toml";
pub(crate) const LIB_REPOSITORY: &str = "https://github.com/radrootslabs/lib";
const ARCHITECTURE: &str = "radroots.crates.release.v2";
const LIB_VERSION: &str = "0.1.0-alpha";
const RUST_VERSION: &str = "1.97.1";
const HOST_FEATURE_PROFILE: &str = "service-host";
const MAX_LOCK_BYTES: usize = 4096;
const MAX_SERVICE_BYTES: usize = 128;
const MAX_CONTRACT_BYTES: usize = 32_768;

const FIELD_ORDER: [&str; 18] = [
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
    "flake_lock_sha256",
    "rust_version",
    "host_feature_profile",
    "contract_versions.config",
    "contract_versions.state",
    "contract_versions.admin",
    "contract_versions.status",
    "contract_versions.provider",
];

const ERROR_CODES: [&str; 10] = [
    "invalid_contract_version",
    "invalid_digest",
    "invalid_feature_profile",
    "invalid_fixed_identity",
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
    flake_lock_sha256: String,
    rust_version: String,
    host_feature_profile: String,
    contract_versions: ContractVersions,
}

pub(crate) struct ServiceSourceLockParts<'a> {
    pub(crate) service: &'a str,
    pub(crate) revision: &'a str,
    pub(crate) workspace_catalog_sha256: &'a str,
    pub(crate) source_archive_sha256: &'a str,
    pub(crate) cargo_lock_sha256: &'a str,
    pub(crate) flake_lock_sha256: &'a str,
    pub(crate) contract_versions: ContractVersions,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct ServiceSourceLockV1 {
    raw: RawServiceSourceLock,
    canonical: Box<[u8]>,
}

impl fmt::Debug for ServiceSourceLockV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceSourceLockV1")
            .finish_non_exhaustive()
    }
}

impl ServiceSourceLockV1 {
    pub(crate) fn new(parts: ServiceSourceLockParts<'_>) -> Result<Self, ServiceSourceLockError> {
        validate_parts(&parts)?;
        let raw = RawServiceSourceLock {
            schema: LOCK_SCHEMA.to_owned(),
            contract_version: 1,
            service: parts.service.to_owned(),
            repository: LIB_REPOSITORY.to_owned(),
            revision: parts.revision.to_owned(),
            architecture: ARCHITECTURE.to_owned(),
            workspace_catalog_sha256: parts.workspace_catalog_sha256.to_owned(),
            version: LIB_VERSION.to_owned(),
            source_archive_sha256: parts.source_archive_sha256.to_owned(),
            cargo_lock_sha256: parts.cargo_lock_sha256.to_owned(),
            flake_lock_sha256: parts.flake_lock_sha256.to_owned(),
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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceLockDecision {
    schema: String,
    contract_version: u32,
    decision_state: String,
    lock_filename: String,
    lock_schema: String,
    canonical_encoding: String,
    maximum_lock_utf8_bytes: usize,
    maximum_service_utf8_bytes: usize,
    canonical_field_order: Vec<String>,
    fixed: FixedDecision,
    revision_encoding: String,
    digest_encoding: String,
    digest_subjects: DigestSubjects,
    service_identifier: String,
    contract_version_rule: String,
    negative_error_codes: Vec<String>,
    canonical_vector: CanonicalVector,
    operations: OperationsDecision,
    deferred_operations: Vec<String>,
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
    service_revision_stability: String,
    revision_agreement: Vec<String>,
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

    let parsed =
        ServiceSourceLockV1::from_canonical_bytes(decision.canonical_vector.toml.as_bytes())?;
    let expected = canonical_vector();
    if parsed != expected
        || hex::encode(Sha256::digest(parsed.canonical_bytes())) != decision.canonical_vector.sha256
    {
        return Err(ServiceSourceLockError::Noncanonical);
    }
    Ok(())
}

fn validate_decision(decision: &SourceLockDecision) -> Result<(), ServiceSourceLockError> {
    let error_codes = [
        ServiceSourceLockError::InvalidContractVersion,
        ServiceSourceLockError::InvalidDigest,
        ServiceSourceLockError::InvalidFeatureProfile,
        ServiceSourceLockError::InvalidFixedIdentity,
        ServiceSourceLockError::InvalidRevision,
        ServiceSourceLockError::InvalidService,
        ServiceSourceLockError::InvalidToolchain,
        ServiceSourceLockError::Malformed,
        ServiceSourceLockError::Noncanonical,
        ServiceSourceLockError::TooLarge,
    ]
    .map(ServiceSourceLockError::code);
    let exact = decision.schema == "radroots.services-hardening.source-lock-decisions.v1"
        && decision.contract_version == 1
        && decision.decision_state == "active"
        && decision.lock_filename == LOCK_FILENAME
        && decision.lock_schema == LOCK_SCHEMA
        && decision.canonical_encoding == "compact_canonical_toml_with_final_newline"
        && decision.maximum_lock_utf8_bytes == MAX_LOCK_BYTES
        && decision.maximum_service_utf8_bytes == MAX_SERVICE_BYTES
        && decision.canonical_field_order == FIELD_ORDER
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
        && decision.operations.service_revision_stability
            == "same_head_before_and_after_evidence_and_output"
        && decision.operations.revision_agreement
            == [
                "cargo_manifests",
                "cargo_lock",
                "direct_exact_nix_input",
                "source_archive",
                "canonical_public_remote",
            ]
        && decision.operations.maximum_source_archive_bytes == 1_073_741_824
        && decision.deferred_operations
            == [
                "embedded_build_information_agreement",
                "service_fixture_release_graph",
            ];
    if exact {
        Ok(())
    } else {
        Err(ServiceSourceLockError::InvalidFixedIdentity)
    }
}

fn canonical_vector() -> ServiceSourceLockV1 {
    ServiceSourceLockV1::new(ServiceSourceLockParts {
        service: "fixture_service",
        revision: "1111111111111111111111111111111111111111",
        workspace_catalog_sha256: "2222222222222222222222222222222222222222222222222222222222222222",
        source_archive_sha256: "3333333333333333333333333333333333333333333333333333333333333333",
        cargo_lock_sha256: "4444444444444444444444444444444444444444444444444444444444444444",
        flake_lock_sha256: "5555555555555555555555555555555555555555555555555555555555555555",
        contract_versions: ContractVersions::new(1, 2, 3, 4, 5),
    })
    .expect("the governed source-lock vector is valid")
}

fn validate_raw(raw: &RawServiceSourceLock) -> Result<(), ServiceSourceLockError> {
    if raw.schema != LOCK_SCHEMA
        || raw.contract_version != 1
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
        flake_lock_sha256: &raw.flake_lock_sha256,
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
        parts.flake_lock_sha256,
    ]
    .into_iter()
    .all(|value| valid_lower_hex(value, 64))
    {
        return Err(ServiceSourceLockError::InvalidDigest);
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
    writeln!(output, "flake_lock_sha256 = \"{}\"", raw.flake_lock_sha256)
        .expect("render to String");
    writeln!(output, "rust_version = \"{}\"", raw.rust_version).expect("render to String");
    writeln!(
        output,
        "host_feature_profile = \"{}\"",
        raw.host_feature_profile
    )
    .expect("render to String");
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

    use super::*;

    #[test]
    fn canonical_vector_round_trips_with_exact_digest() {
        let lock = canonical_vector();
        assert_eq!(
            hex::encode(Sha256::digest(lock.canonical_bytes())),
            "2257efc8fb3ff4ee8e429e326effdfe622c5e898b429ee1a8ea3aac38f9810cc"
        );
        assert_eq!(
            ServiceSourceLockV1::from_canonical_bytes(lock.canonical_bytes()),
            Ok(lock)
        );
    }

    #[test]
    fn parser_rejects_noncanonical_and_ambiguous_toml() {
        let canonical = String::from_utf8(canonical_vector().canonical_bytes().to_vec())
            .expect("canonical UTF-8");
        for malformed in [
            canonical.replacen("schema =", "unknown = 1\nschema =", 1),
            canonical.replacen(
                "contract_version = 1\nservice = \"fixture_service\"",
                "service = \"fixture_service\"\ncontract_version = 1",
                1,
            ),
            format!(" {canonical}"),
            canonical.replacen("schema =", "schema=", 1),
            canonical.replacen("contract_version = 1", "contract_version = 01", 1),
        ] {
            assert!(matches!(
                ServiceSourceLockV1::from_canonical_bytes(malformed.as_bytes()),
                Err(ServiceSourceLockError::Malformed | ServiceSourceLockError::Noncanonical)
            ));
        }
        let duplicate = canonical.replacen(
            "service = \"fixture_service\"",
            "service = \"fixture_service\"\nservice = \"fixture_service\"",
            1,
        );
        assert_eq!(
            ServiceSourceLockV1::from_canonical_bytes(duplicate.as_bytes()),
            Err(ServiceSourceLockError::Malformed)
        );
    }

    #[test]
    fn parser_rejects_every_identity_and_bound_drift() {
        let canonical = String::from_utf8(canonical_vector().canonical_bytes().to_vec())
            .expect("canonical UTF-8");
        let cases = [
            (
                "radroots.service.source-lock.v1",
                "wrong",
                ServiceSourceLockError::InvalidFixedIdentity,
            ),
            (
                "contract_version = 1",
                "contract_version = 2",
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
                "1111111111111111111111111111111111111111",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                ServiceSourceLockError::InvalidRevision,
            ),
            (
                "1111111111111111111111111111111111111111",
                "111111111111111111111111111111111111111",
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
                "4444444444444444444444444444444444444444444444444444444444444444",
                "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
                ServiceSourceLockError::InvalidDigest,
            ),
            (
                "5555555555555555555555555555555555555555555555555555555555555555",
                "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
                ServiceSourceLockError::InvalidDigest,
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
                ServiceSourceLockV1::from_canonical_bytes(mutated.as_bytes()),
                Err(expected)
            );
        }

        let too_large = vec![b' '; MAX_LOCK_BYTES + 1];
        assert_eq!(
            ServiceSourceLockV1::from_canonical_bytes(&too_large),
            Err(ServiceSourceLockError::TooLarge)
        );
    }

    #[test]
    fn exact_service_and_contract_version_maxima_are_admitted() {
        let service = "a".repeat(MAX_SERVICE_BYTES);
        let maximum = ServiceSourceLockV1::new(ServiceSourceLockParts {
            service: &service,
            revision: &"a".repeat(40),
            workspace_catalog_sha256: &"b".repeat(64),
            source_archive_sha256: &"c".repeat(64),
            cargo_lock_sha256: &"d".repeat(64),
            flake_lock_sha256: &"e".repeat(64),
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
            ServiceSourceLockV1::new(ServiceSourceLockParts {
                service: &overlong_service,
                revision: &"a".repeat(40),
                workspace_catalog_sha256: &"b".repeat(64),
                source_archive_sha256: &"c".repeat(64),
                cargo_lock_sha256: &"d".repeat(64),
                flake_lock_sha256: &"e".repeat(64),
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
                ServiceSourceLockV1::new(ServiceSourceLockParts {
                    service: invalid,
                    revision: &"a".repeat(40),
                    workspace_catalog_sha256: &"b".repeat(64),
                    source_archive_sha256: &"c".repeat(64),
                    cargo_lock_sha256: &"d".repeat(64),
                    flake_lock_sha256: &"e".repeat(64),
                    contract_versions: ContractVersions::new(1, 1, 1, 1, 1),
                }),
                Err(ServiceSourceLockError::InvalidService)
            );
        }
    }

    #[test]
    fn diagnostics_are_fixed_and_source_free() {
        let lock = canonical_vector();
        let debug = format!("{lock:?}");
        assert_eq!(debug, "ServiceSourceLockV1 { .. }");
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

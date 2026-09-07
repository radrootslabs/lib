use std::fmt;

use serde::Deserialize;

use crate::service_source_lock::{ContractVersions, LIB_REPOSITORY};

pub(crate) const LOCK_FILENAME: &str = "radroots.service.source-lock.v3.toml";
pub(crate) const PREDECESSOR_LOCK_FILENAME: &str = "radroots.service.source-lock.v2.toml";
const MAX_LOCK_BYTES: usize = 16 * 1024;
const ARCHITECTURE: &str = "radroots.crates.release.v2";
const LIB_VERSION: &str = "0.1.0-alpha";
const RUST_VERSION: &str = "1.97.1";
const HOST_FEATURE_PROFILE: &str = "service-host";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawSourceArchiveContract {
    binding: String,
    format: String,
    compression: String,
    compression_timestamp: String,
    entry_order: String,
    path_prefix: String,
    file_mode: String,
    uid: u32,
    gid: u32,
    uname: String,
    gname: String,
    mtime: String,
    pax_headers: String,
    directory_entries: String,
    symlinks: String,
    hardlinks: String,
    submodules: String,
    trailer: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawPublicInputLock {
    path: String,
    sha256: String,
    binding: String,
    mutable_reference: String,
    lib_input: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawParentResult {
    embedded_in_public_input_lock: bool,
    embedded_in_source_lock: bool,
    storage: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawQualifiedNix {
    material: String,
    lib_revision: String,
    supported_systems: Vec<String>,
    public_input_lock: RawPublicInputLock,
    parent_result: RawParentResult,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawArtifactContract {
    path: String,
    sha256: String,
    binding: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawSqliteContract {
    high_level_authority: String,
    second_pool_connection_query_transaction_migration_authority: String,
    incremental_backup_adapter: String,
    native_linkage_count: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawServiceSourceLockV3 {
    schema: String,
    contract_version: u32,
    service: String,
    repository: String,
    revision: String,
    architecture: String,
    workspace_catalog_sha256: String,
    version: String,
    source_archive_sha256: String,
    source_archive_contract: RawSourceArchiveContract,
    cargo_lock_sha256: String,
    rust_version: String,
    host_feature_profile: String,
    nix: RawQualifiedNix,
    artifact_contract: RawArtifactContract,
    sqlite: RawSqliteContract,
    contract_versions: ContractVersions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServiceSourceLockV3Error {
    TooLarge,
    Malformed,
    Noncanonical,
    Invalid,
}

impl fmt::Display for ServiceSourceLockV3Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "service source lock v3 exceeds its byte limit",
            Self::Malformed => "service source lock v3 is malformed",
            Self::Noncanonical => "service source lock v3 is not canonical",
            Self::Invalid => "service source lock v3 is invalid",
        })
    }
}

impl std::error::Error for ServiceSourceLockV3Error {}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct ServiceSourceLockV3 {
    raw: RawServiceSourceLockV3,
    canonical: Box<[u8]>,
}

#[cfg(test)]
pub(crate) struct FixtureParts<'a> {
    pub(crate) service: &'a str,
    pub(crate) revision: &'a str,
    pub(crate) workspace_catalog_sha256: &'a str,
    pub(crate) source_archive_sha256: &'a str,
    pub(crate) cargo_lock_sha256: &'a str,
    pub(crate) flake_lock_sha256: &'a str,
    pub(crate) artifact_contract_sha256: &'a str,
    pub(crate) contract_versions: ContractVersions,
}

impl fmt::Debug for ServiceSourceLockV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceSourceLockV3")
            .finish_non_exhaustive()
    }
}

impl ServiceSourceLockV3 {
    pub(crate) fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, ServiceSourceLockV3Error> {
        if bytes.len() > MAX_LOCK_BYTES {
            return Err(ServiceSourceLockV3Error::TooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| ServiceSourceLockV3Error::Malformed)?;
        let raw = toml::from_str::<RawServiceSourceLockV3>(text)
            .map_err(|_| ServiceSourceLockV3Error::Malformed)?;
        validate(&raw)?;
        let canonical = canonical_bytes(&raw);
        if bytes != canonical.as_slice() {
            return Err(ServiceSourceLockV3Error::Noncanonical);
        }
        Ok(Self {
            raw,
            canonical: canonical.into_boxed_slice(),
        })
    }

    #[cfg(test)]
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
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

    pub(crate) fn flake_lock_sha256(&self) -> &str {
        &self.raw.nix.public_input_lock.sha256
    }

    pub(crate) fn artifact_contract_path(&self) -> &str {
        &self.raw.artifact_contract.path
    }

    pub(crate) fn artifact_contract_sha256(&self) -> &str {
        &self.raw.artifact_contract.sha256
    }

    pub(crate) const fn contract_versions(&self) -> ContractVersions {
        self.raw.contract_versions
    }

    #[cfg(test)]
    pub(crate) fn fixture(parts: FixtureParts<'_>) -> Result<Self, ServiceSourceLockV3Error> {
        let FixtureParts {
            service,
            revision,
            workspace_catalog_sha256,
            source_archive_sha256,
            cargo_lock_sha256,
            flake_lock_sha256,
            artifact_contract_sha256,
            contract_versions,
        } = parts;
        let artifact_path = format!("contracts/release/{service}-artifact-contract.v3.json");
        let raw = RawServiceSourceLockV3 {
            schema: "radroots.service.source-lock.v3".to_owned(),
            contract_version: 3,
            service: service.to_owned(),
            repository: LIB_REPOSITORY.to_owned(),
            revision: revision.to_owned(),
            architecture: ARCHITECTURE.to_owned(),
            workspace_catalog_sha256: workspace_catalog_sha256.to_owned(),
            version: LIB_VERSION.to_owned(),
            source_archive_sha256: source_archive_sha256.to_owned(),
            source_archive_contract: RawSourceArchiveContract {
                binding: "sha256_of_canonical_exact_lib_revision_tree_archive".to_owned(),
                format: "ustar".to_owned(),
                compression: "none".to_owned(),
                compression_timestamp: "not_applicable".to_owned(),
                entry_order: "bytewise_git_path".to_owned(),
                path_prefix: "none".to_owned(),
                file_mode: "git_index_100644_or_100755".to_owned(),
                uid: 0,
                gid: 0,
                uname: String::new(),
                gname: String::new(),
                mtime: "lib_revision_commit_timestamp".to_owned(),
                pax_headers: "forbidden".to_owned(),
                directory_entries: "omitted".to_owned(),
                symlinks: "forbidden".to_owned(),
                hardlinks: "forbidden".to_owned(),
                submodules: "forbidden".to_owned(),
                trailer: "two_zero_blocks".to_owned(),
            },
            cargo_lock_sha256: cargo_lock_sha256.to_owned(),
            rust_version: RUST_VERSION.to_owned(),
            host_feature_profile: HOST_FEATURE_PROFILE.to_owned(),
            nix: RawQualifiedNix {
                material: "qualified".to_owned(),
                lib_revision: revision.to_owned(),
                supported_systems: vec!["aarch64-darwin".to_owned(), "x86_64-linux".to_owned()],
                public_input_lock: RawPublicInputLock {
                    path: "flake.lock".to_owned(),
                    sha256: flake_lock_sha256.to_owned(),
                    binding: "exact_regular_file_bytes".to_owned(),
                    mutable_reference: "forbidden".to_owned(),
                    lib_input: "lib".to_owned(),
                },
                parent_result: RawParentResult {
                    embedded_in_public_input_lock: false,
                    embedded_in_source_lock: false,
                    storage: "separate_generation_scoped_evidence".to_owned(),
                },
            },
            artifact_contract: RawArtifactContract {
                path: artifact_path,
                sha256: artifact_contract_sha256.to_owned(),
                binding: "exact_regular_file_bytes_in_same_source_revision".to_owned(),
            },
            sqlite: RawSqliteContract {
                high_level_authority: "sqlx_only".to_owned(),
                second_pool_connection_query_transaction_migration_authority: "forbidden"
                    .to_owned(),
                incremental_backup_adapter: "sealed_native_sqlx_owned_locked_handle_only"
                    .to_owned(),
                native_linkage_count: 1,
            },
            contract_versions,
        };
        validate(&raw)?;
        let canonical = canonical_bytes(&raw).into_boxed_slice();
        Ok(Self { raw, canonical })
    }
}

fn validate(raw: &RawServiceSourceLockV3) -> Result<(), ServiceSourceLockV3Error> {
    let expected_artifact = match raw.service.as_str() {
        "myc" => "contracts/release/myc-artifact-contract.v3.json",
        "rhi" => "contracts/release/rhi-artifact-contract.v3.json",
        _ => return Err(ServiceSourceLockV3Error::Invalid),
    };
    let archive = &raw.source_archive_contract;
    if raw.schema != "radroots.service.source-lock.v3"
        || raw.contract_version != 3
        || raw.repository != LIB_REPOSITORY
        || !valid_lower_hex(&raw.revision, 40)
        || raw.architecture != ARCHITECTURE
        || !valid_lower_hex(&raw.workspace_catalog_sha256, 64)
        || raw.version != LIB_VERSION
        || !valid_lower_hex(&raw.source_archive_sha256, 64)
        || !valid_lower_hex(&raw.cargo_lock_sha256, 64)
        || raw.rust_version != RUST_VERSION
        || raw.host_feature_profile != HOST_FEATURE_PROFILE
        || archive.binding != "sha256_of_canonical_exact_lib_revision_tree_archive"
        || archive.format != "ustar"
        || archive.compression != "none"
        || archive.compression_timestamp != "not_applicable"
        || archive.entry_order != "bytewise_git_path"
        || archive.path_prefix != "none"
        || archive.file_mode != "git_index_100644_or_100755"
        || archive.uid != 0
        || archive.gid != 0
        || !archive.uname.is_empty()
        || !archive.gname.is_empty()
        || archive.mtime != "lib_revision_commit_timestamp"
        || archive.pax_headers != "forbidden"
        || archive.directory_entries != "omitted"
        || archive.symlinks != "forbidden"
        || archive.hardlinks != "forbidden"
        || archive.submodules != "forbidden"
        || archive.trailer != "two_zero_blocks"
        || raw.nix.material != "qualified"
        || raw.nix.lib_revision != raw.revision
        || raw.nix.supported_systems != ["aarch64-darwin", "x86_64-linux"]
        || raw.nix.public_input_lock.path != "flake.lock"
        || !valid_lower_hex(&raw.nix.public_input_lock.sha256, 64)
        || raw.nix.public_input_lock.binding != "exact_regular_file_bytes"
        || raw.nix.public_input_lock.mutable_reference != "forbidden"
        || raw.nix.public_input_lock.lib_input != "lib"
        || raw.nix.parent_result.embedded_in_public_input_lock
        || raw.nix.parent_result.embedded_in_source_lock
        || raw.nix.parent_result.storage != "separate_generation_scoped_evidence"
        || raw.artifact_contract.path != expected_artifact
        || !valid_lower_hex(&raw.artifact_contract.sha256, 64)
        || raw.artifact_contract.binding != "exact_regular_file_bytes_in_same_source_revision"
        || raw.sqlite.high_level_authority != "sqlx_only"
        || raw
            .sqlite
            .second_pool_connection_query_transaction_migration_authority
            != "forbidden"
        || raw.sqlite.incremental_backup_adapter != "sealed_native_sqlx_owned_locked_handle_only"
        || raw.sqlite.native_linkage_count != 1
        || !contract_versions_valid(raw.contract_versions)
    {
        Err(ServiceSourceLockV3Error::Invalid)
    } else {
        Ok(())
    }
}

fn contract_versions_valid(versions: ContractVersions) -> bool {
    versions.config() != 0
        && versions.state() != 0
        && versions.admin() != 0
        && versions.status() != 0
        && versions.provider() != 0
}

fn canonical_bytes(raw: &RawServiceSourceLockV3) -> Vec<u8> {
    format!(
        concat!(
            "schema = \"radroots.service.source-lock.v3\"\n",
            "contract_version = 3\n",
            "service = \"{}\"\n",
            "repository = \"https://github.com/radrootslabs/lib\"\n",
            "revision = \"{}\"\n",
            "architecture = \"radroots.crates.release.v2\"\n",
            "workspace_catalog_sha256 = \"{}\"\n",
            "version = \"0.1.0-alpha\"\n",
            "source_archive_sha256 = \"{}\"\n",
            "cargo_lock_sha256 = \"{}\"\n",
            "rust_version = \"1.97.1\"\n",
            "host_feature_profile = \"service-host\"\n\n",
            "[source_archive_contract]\n",
            "binding = \"sha256_of_canonical_exact_lib_revision_tree_archive\"\n",
            "format = \"ustar\"\n",
            "compression = \"none\"\n",
            "compression_timestamp = \"not_applicable\"\n",
            "entry_order = \"bytewise_git_path\"\n",
            "path_prefix = \"none\"\n",
            "file_mode = \"git_index_100644_or_100755\"\n",
            "uid = 0\n",
            "gid = 0\n",
            "uname = \"\"\n",
            "gname = \"\"\n",
            "mtime = \"lib_revision_commit_timestamp\"\n",
            "pax_headers = \"forbidden\"\n",
            "directory_entries = \"omitted\"\n",
            "symlinks = \"forbidden\"\n",
            "hardlinks = \"forbidden\"\n",
            "submodules = \"forbidden\"\n",
            "trailer = \"two_zero_blocks\"\n\n",
            "[nix]\n",
            "material = \"qualified\"\n",
            "lib_revision = \"{}\"\n",
            "supported_systems = [\"aarch64-darwin\", \"x86_64-linux\"]\n\n",
            "[nix.public_input_lock]\n",
            "path = \"flake.lock\"\n",
            "sha256 = \"{}\"\n",
            "binding = \"exact_regular_file_bytes\"\n",
            "mutable_reference = \"forbidden\"\n",
            "lib_input = \"lib\"\n\n",
            "[nix.parent_result]\n",
            "embedded_in_public_input_lock = false\n",
            "embedded_in_source_lock = false\n",
            "storage = \"separate_generation_scoped_evidence\"\n\n",
            "[artifact_contract]\n",
            "path = \"{}\"\n",
            "sha256 = \"{}\"\n",
            "binding = \"exact_regular_file_bytes_in_same_source_revision\"\n\n",
            "[sqlite]\n",
            "high_level_authority = \"sqlx_only\"\n",
            "second_pool_connection_query_transaction_migration_authority = \"forbidden\"\n",
            "incremental_backup_adapter = \"sealed_native_sqlx_owned_locked_handle_only\"\n",
            "native_linkage_count = 1\n\n",
            "[contract_versions]\n",
            "config = {}\n",
            "state = {}\n",
            "admin = {}\n",
            "status = {}\n",
            "provider = {}\n"
        ),
        raw.service,
        raw.revision,
        raw.workspace_catalog_sha256,
        raw.source_archive_sha256,
        raw.cargo_lock_sha256,
        raw.nix.lib_revision,
        raw.nix.public_input_lock.sha256,
        raw.artifact_contract.path,
        raw.artifact_contract.sha256,
        raw.contract_versions.config(),
        raw.contract_versions.state(),
        raw.contract_versions.admin(),
        raw.contract_versions.status(),
        raw.contract_versions.provider(),
    )
    .into_bytes()
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MYC_LOCK: &[u8] = br#"schema = "radroots.service.source-lock.v3"
contract_version = 3
service = "myc"
repository = "https://github.com/radrootslabs/lib"
revision = "1111111111111111111111111111111111111111"
architecture = "radroots.crates.release.v2"
workspace_catalog_sha256 = "2222222222222222222222222222222222222222222222222222222222222222"
version = "0.1.0-alpha"
source_archive_sha256 = "3333333333333333333333333333333333333333333333333333333333333333"
cargo_lock_sha256 = "4444444444444444444444444444444444444444444444444444444444444444"
rust_version = "1.97.1"
host_feature_profile = "service-host"

[source_archive_contract]
binding = "sha256_of_canonical_exact_lib_revision_tree_archive"
format = "ustar"
compression = "none"
compression_timestamp = "not_applicable"
entry_order = "bytewise_git_path"
path_prefix = "none"
file_mode = "git_index_100644_or_100755"
uid = 0
gid = 0
uname = ""
gname = ""
mtime = "lib_revision_commit_timestamp"
pax_headers = "forbidden"
directory_entries = "omitted"
symlinks = "forbidden"
hardlinks = "forbidden"
submodules = "forbidden"
trailer = "two_zero_blocks"

[nix]
material = "qualified"
lib_revision = "1111111111111111111111111111111111111111"
supported_systems = ["aarch64-darwin", "x86_64-linux"]

[nix.public_input_lock]
path = "flake.lock"
sha256 = "5555555555555555555555555555555555555555555555555555555555555555"
binding = "exact_regular_file_bytes"
mutable_reference = "forbidden"
lib_input = "lib"

[nix.parent_result]
embedded_in_public_input_lock = false
embedded_in_source_lock = false
storage = "separate_generation_scoped_evidence"

[artifact_contract]
path = "contracts/release/myc-artifact-contract.v3.json"
sha256 = "6666666666666666666666666666666666666666666666666666666666666666"
binding = "exact_regular_file_bytes_in_same_source_revision"

[sqlite]
high_level_authority = "sqlx_only"
second_pool_connection_query_transaction_migration_authority = "forbidden"
incremental_backup_adapter = "sealed_native_sqlx_owned_locked_handle_only"
native_linkage_count = 1

[contract_versions]
config = 1
state = 2
admin = 3
status = 4
provider = 5
"#;

    #[test]
    fn canonical_v3_lock_round_trips_and_exposes_exact_bindings() {
        let lock = ServiceSourceLockV3::from_canonical_bytes(MYC_LOCK).expect("v3 lock");
        assert_eq!(lock.canonical_bytes(), MYC_LOCK);
        assert_eq!(lock.service(), "myc");
        assert_eq!(lock.revision(), "1".repeat(40));
        assert_eq!(
            lock.artifact_contract_path(),
            "contracts/release/myc-artifact-contract.v3.json"
        );
        assert_eq!(
            lock.contract_versions(),
            ContractVersions::new(1, 2, 3, 4, 5)
        );
    }

    #[test]
    fn v3_lock_rejects_noncanonical_and_independent_semantic_drift() {
        let noncanonical = String::from_utf8(MYC_LOCK.to_vec())
            .expect("UTF-8")
            .replace("schema =", "schema  =");
        assert!(ServiceSourceLockV3::from_canonical_bytes(noncanonical.as_bytes()).is_err());
        for (from, to) in [
            ("service = \"myc\"", "service = \"other\""),
            ("material = \"qualified\"", "material = \"deferred\""),
            ("native_linkage_count = 1", "native_linkage_count = 2"),
            ("trailer = \"two_zero_blocks\"", "trailer = \"other\""),
        ] {
            let drifted = String::from_utf8(MYC_LOCK.to_vec())
                .expect("UTF-8")
                .replace(from, to);
            assert!(
                ServiceSourceLockV3::from_canonical_bytes(drifted.as_bytes()).is_err(),
                "{from}"
            );
        }
    }
}

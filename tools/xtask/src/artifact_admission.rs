use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    io::{Seek as _, SeekFrom},
    path::{Component, Path},
    time::Duration,
};

use goblin::{
    Object,
    elf::{header::EM_X86_64, program_header::PF_X},
    mach::{Mach, constants::cputype::CPU_TYPE_ARM64, header::MH_EXECUTE},
};
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};

use crate::{bounded_process, safe_artifact_io};
use safe_artifact_io::TarGzipLimits;

const CONTRACT_RELATIVE: &str =
    "contracts/architecture/decisions/services_hardening_artifact_admission.v1.json";
const MAX_CONTRACT_BYTES: u64 = 65_536;
const MAX_BINARY_PARSE_BYTES: u64 = 67_108_864;
const MAX_OCI_BYTES: u64 = 2_147_483_648;
const MAX_ARCHIVE_EXPANDED_BYTES: u64 = 17_179_869_184;
const MAX_ARCHIVE_MEMBERS: u64 = 65_536;
const MAX_JSON_BYTES: u64 = 1_048_576;
const MAX_PATH_BYTES: usize = 4_096;
const MAX_DEPTH: usize = 64;
const MAX_LAYERS: usize = 2;
const MAX_RUNTIME_STREAM_BYTES: usize = 65_536;
const RUNTIME_DEADLINE: Duration = Duration::from_secs(10);
const AGPL_LICENSE: &str = "AGPL-3.0-or-later";
const LINUX_TARGET: &str = "x86_64-unknown-linux-gnu";
const MACOS_TARGET: &str = "aarch64-apple-darwin";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionError {
    InvalidContract,
    InvalidBinary,
    BinarySmokeFailure,
    InvalidOci,
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidContract => "artifact admission contract is invalid",
            Self::InvalidBinary => "service binary admission failed",
            Self::BinarySmokeFailure => "service binary smoke admission failed",
            Self::InvalidOci => "service OCI admission failed",
        })
    }
}

impl std::error::Error for AdmissionError {}

#[derive(Clone, Copy)]
pub(crate) struct ContractVersions {
    pub(crate) admin: u32,
    pub(crate) config: u32,
    pub(crate) provider: u32,
    pub(crate) state: u32,
    pub(crate) status: u32,
}

pub(crate) struct OciExpectation<'a> {
    pub(crate) service: &'a str,
    pub(crate) binary_name: &'a str,
    pub(crate) version: &'a str,
    pub(crate) service_revision: &'a str,
    pub(crate) lib_revision: &'a str,
    pub(crate) license: &'a str,
    pub(crate) contract_versions: ContractVersions,
}

pub(crate) fn validate_contract(workspace_root: &Path) -> Result<(), String> {
    let bytes = safe_artifact_io::read_regular_path(
        &workspace_root.join(CONTRACT_RELATIVE),
        MAX_CONTRACT_BYTES,
    )
    .map_err(|_| AdmissionError::InvalidContract.to_string())?;
    let observed = serde_json::from_slice::<Value>(&bytes)
        .map_err(|_| AdmissionError::InvalidContract.to_string())?;
    if observed == expected_contract() {
        Ok(())
    } else {
        Err(AdmissionError::InvalidContract.to_string())
    }
}

fn expected_contract() -> Value {
    json!({
        "schema": "radroots.services-hardening.artifact-admission-decisions.v1",
        "contract_version": 1,
        "decision_state": "active",
        "owner_step": 304,
        "command_owner": "tools/xtask",
        "supported_binary_targets": [MACOS_TARGET, LINUX_TARGET],
        "binary": {
            "formats": {
                MACOS_TARGET: "thin_macho64_arm64_execute",
                LINUX_TARGET: "elf64_little_endian_x86_64_execute_or_pie"
            },
            "maximum_parse_bytes": MAX_BINARY_PARSE_BYTES,
            "architecture": "exact_target_match",
            "linkage": "parse_declared_dynamic_libraries_and_forbid_external_sqlite",
            "sqlite": "one_bundled_native_linkage_under_the_separate_sqlx_only_source_contract",
            "structural_smoke": "executable_type_nonzero_entrypoint_and_executable_segment",
            "runtime_smoke": "bounded_help_execution_on_the_matching_native_host",
            "fat_or_multi_arch": "forbidden"
        },
        "oci": {
            "format": "single_image_docker_archive_tar_gzip",
            "platform": "linux_amd64",
            "maximum_layers": MAX_LAYERS,
            "outer_archive": "descriptor_bound_bounded_safe_materialization_with_exact_inventory",
            "manifest": "one_entry_exact_config_repo_tag_and_layer_references",
            "config": "content_addressed_json_with_exact_rootless_runtime_and_build_labels",
            "license": "Cargo.toml package license derived AGPL-3.0-or-later",
            "layers": "parse_only_bounded_tar_validation_with_content_digest_reconciliation",
            "entrypoint": "one_regular_executable_payload_at_the_configured_store_path",
            "unpacking": "forbidden"
        },
        "maximums": {
            "outer_compressed_bytes": MAX_OCI_BYTES,
            "outer_expanded_bytes": MAX_ARCHIVE_EXPANDED_BYTES,
            "outer_members": MAX_ARCHIVE_MEMBERS,
            "outer_member_bytes": MAX_ARCHIVE_EXPANDED_BYTES,
            "json_bytes": MAX_JSON_BYTES,
            "layer_members": MAX_ARCHIVE_MEMBERS,
            "layer_payload_bytes": MAX_ARCHIVE_EXPANDED_BYTES,
            "path_bytes": MAX_PATH_BYTES,
            "depth": MAX_DEPTH,
            "runtime_seconds": RUNTIME_DEADLINE.as_secs(),
            "runtime_stream_bytes": MAX_RUNTIME_STREAM_BYTES
        },
        "required_negative_vectors": [
            "arbitrary_binary_bytes", "wrong_binary_architecture", "fat_macho",
            "missing_binary_entrypoint", "external_sqlite_linkage", "runtime_smoke_failure",
            "unsafe_outer_tar_member", "wrong_oci_license", "multiple_manifest_entries",
            "config_digest_mismatch", "wrong_oci_platform", "wrong_rootless_config",
            "unexpected_label", "missing_layer", "layer_digest_mismatch",
            "unsafe_layer_path", "missing_entrypoint_payload"
        ],
        "nonclaims": [
            "Linux artifact build on a macOS host without a Linux builder",
            "signature notarization publication deployment or production activation"
        ]
    })
}

pub(crate) fn admit_binary(path: &Path, target: &str, runtime_smoke: bool) -> Result<(), String> {
    admit_binary_inner(path, target, runtime_smoke).map_err(|error| error.to_string())
}

fn admit_binary_inner(
    path: &Path,
    target: &str,
    runtime_smoke: bool,
) -> Result<(), AdmissionError> {
    if ![MACOS_TARGET, LINUX_TARGET].contains(&target) {
        return Err(AdmissionError::InvalidBinary);
    }
    let bytes = safe_artifact_io::read_regular_path(path, MAX_BINARY_PARSE_BYTES)
        .map_err(|_| AdmissionError::InvalidBinary)?;
    admit_binary_bytes(&bytes, target)?;
    if runtime_smoke && target_matches_host(target) {
        runtime_help_smoke(path)?;
    }
    Ok(())
}

fn admit_binary_bytes(bytes: &[u8], target: &str) -> Result<(), AdmissionError> {
    let libraries = match (
        target,
        Object::parse(bytes).map_err(|_| AdmissionError::InvalidBinary)?,
    ) {
        (LINUX_TARGET, Object::Elf(binary)) => {
            if binary.header.e_machine != EM_X86_64
                || !matches!(
                    binary.header.e_type,
                    goblin::elf::header::ET_EXEC | goblin::elf::header::ET_DYN
                )
                || binary.entry == 0
                || !binary.program_headers.iter().any(|header| {
                    header.p_flags & PF_X != 0
                        && binary.entry >= header.p_vaddr
                        && binary.entry < header.p_vaddr.saturating_add(header.p_memsz)
                })
            {
                return Err(AdmissionError::InvalidBinary);
            }
            binary
                .libraries
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        }
        (MACOS_TARGET, Object::Mach(Mach::Binary(binary))) => {
            if binary.header.cputype != CPU_TYPE_ARM64
                || binary.header.filetype != MH_EXECUTE
                || binary.entry == 0
                || !binary.segments.iter().any(|segment| {
                    segment.initprot & 0x4 != 0
                        && binary.entry >= segment.vmaddr
                        && binary.entry < segment.vmaddr.saturating_add(segment.vmsize)
                })
            {
                return Err(AdmissionError::InvalidBinary);
            }
            binary
                .libs
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        }
        _ => return Err(AdmissionError::InvalidBinary),
    };
    validate_dynamic_libraries(&libraries)
}

fn validate_dynamic_libraries(libraries: &[String]) -> Result<(), AdmissionError> {
    if libraries.iter().any(|library| {
        library.is_empty()
            || library.len() > 1_024
            || library.contains(['\n', '\r', '\0'])
            || library.to_ascii_lowercase().contains("sqlite")
    }) {
        Err(AdmissionError::InvalidBinary)
    } else {
        Ok(())
    }
}

fn target_matches_host(target: &str) -> bool {
    matches!(
        (target, std::env::consts::OS, std::env::consts::ARCH),
        (MACOS_TARGET, "macos", "aarch64") | (LINUX_TARGET, "linux", "x86_64")
    )
}

fn runtime_help_smoke(path: &Path) -> Result<(), AdmissionError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o500))
            .map_err(|_| AdmissionError::BinarySmokeFailure)?;
    }
    let parent = path.parent().ok_or(AdmissionError::BinarySmokeFailure)?;
    let output = bounded_process::run(
        &bounded_process::ProcessRequest::new(path.as_os_str())
            .arg("--help")
            .current_dir(parent)
            .deadline(RUNTIME_DEADLINE)
            .output_limits(MAX_RUNTIME_STREAM_BYTES, MAX_RUNTIME_STREAM_BYTES),
    )
    .map_err(|_| AdmissionError::BinarySmokeFailure)?;
    if output.status().success() {
        Ok(())
    } else {
        Err(AdmissionError::BinarySmokeFailure)
    }
}

pub(crate) fn admit_oci(
    path: &Path,
    trusted_parent: &Path,
    expected: &OciExpectation<'_>,
) -> Result<(), String> {
    admit_oci_inner(path, trusted_parent, expected).map_err(|error| error.to_string())
}

fn admit_oci_inner(
    path: &Path,
    trusted_parent: &Path,
    expected: &OciExpectation<'_>,
) -> Result<(), AdmissionError> {
    if expected.license != AGPL_LICENSE
        || !valid_identifier(expected.service)
        || !valid_binary_name(expected.binary_name)
        || !valid_hex(expected.service_revision, 40)
        || !valid_hex(expected.lib_revision, 40)
    {
        return Err(AdmissionError::InvalidOci);
    }
    let limits = TarGzipLimits {
        max_compressed_bytes: MAX_OCI_BYTES,
        max_expanded_bytes: MAX_ARCHIVE_EXPANDED_BYTES,
        max_members: MAX_ARCHIVE_MEMBERS,
        max_member_bytes: MAX_ARCHIVE_EXPANDED_BYTES,
        max_payload_bytes: MAX_ARCHIVE_EXPANDED_BYTES,
        max_depth: MAX_DEPTH,
        max_path_bytes: MAX_PATH_BYTES,
    };
    let materialized = safe_artifact_io::materialize_tar_gzip_path(path, trusted_parent, limits)
        .map_err(|_| AdmissionError::InvalidOci)?;
    let snapshot = materialized.snapshot();
    let manifest = read_json_member(snapshot, "manifest.json")?;
    let manifest = manifest.as_array().ok_or(AdmissionError::InvalidOci)?;
    if manifest.len() != 1 {
        return Err(AdmissionError::InvalidOci);
    }
    let record = manifest[0].as_object().ok_or(AdmissionError::InvalidOci)?;
    require_exact_keys(record, &["Config", "Layers", "RepoTags"])?;
    let config_name = json_string(record, "Config")?;
    if !config_name.ends_with(".json") || !valid_hex(config_name.trim_end_matches(".json"), 64) {
        return Err(AdmissionError::InvalidOci);
    }
    let image_name = expected.service.replace('_', "-");
    if json_string_array(record, "RepoTags")? != [format!("{image_name}:{}", expected.version)] {
        return Err(AdmissionError::InvalidOci);
    }
    let layers = json_string_array(record, "Layers")?;
    if layers.is_empty() || layers.len() > MAX_LAYERS || !all_unique(&layers) {
        return Err(AdmissionError::InvalidOci);
    }
    for layer in &layers {
        validate_layer_name(layer)?;
    }
    let config_bytes = read_member(snapshot, config_name, MAX_JSON_BYTES)?;
    if sha256(&config_bytes) != config_name.trim_end_matches(".json") {
        return Err(AdmissionError::InvalidOci);
    }
    let config =
        serde_json::from_slice::<Value>(&config_bytes).map_err(|_| AdmissionError::InvalidOci)?;
    let entrypoint = validate_config(&config, expected)?;
    validate_repositories(snapshot, &image_name, expected.version, &layers)?;
    validate_outer_inventory(snapshot, config_name, &layers)?;
    let rootfs = config
        .get("rootfs")
        .and_then(Value::as_object)
        .ok_or(AdmissionError::InvalidOci)?;
    require_exact_keys(rootfs, &["diff_ids", "type"])?;
    if json_string(rootfs, "type")? != "layers" {
        return Err(AdmissionError::InvalidOci);
    }
    let diff_ids = json_string_array(rootfs, "diff_ids")?;
    if diff_ids.len() != layers.len() {
        return Err(AdmissionError::InvalidOci);
    }
    let mut entrypoint_count = 0_u32;
    for (layer, diff_id) in layers.iter().zip(diff_ids) {
        let evidence = snapshot
            .hash(
                snapshot_member(snapshot, layer)?,
                MAX_ARCHIVE_EXPANDED_BYTES,
            )
            .map_err(|_| AdmissionError::InvalidOci)?;
        if diff_id != format!("sha256:{}", evidence.sha256) {
            return Err(AdmissionError::InvalidOci);
        }
        entrypoint_count = entrypoint_count
            .checked_add(validate_layer_tar(materialized.root(), layer, &entrypoint)?)
            .ok_or(AdmissionError::InvalidOci)?;
    }
    materialized
        .revalidate()
        .map_err(|_| AdmissionError::InvalidOci)?;
    if entrypoint_count == 1 {
        Ok(())
    } else {
        Err(AdmissionError::InvalidOci)
    }
}

fn validate_config(
    config: &Value,
    expected: &OciExpectation<'_>,
) -> Result<String, AdmissionError> {
    let object = config.as_object().ok_or(AdmissionError::InvalidOci)?;
    if json_string(object, "architecture")? != "amd64"
        || json_string(object, "os")? != "linux"
        || json_string(object, "created")? != "1970-01-01T00:00:01+00:00"
    {
        return Err(AdmissionError::InvalidOci);
    }
    let runtime = object
        .get("config")
        .and_then(Value::as_object)
        .ok_or(AdmissionError::InvalidOci)?;
    require_exact_keys(
        runtime,
        &[
            "Entrypoint",
            "Env",
            "Labels",
            "StopSignal",
            "User",
            "WorkingDir",
        ],
    )?;
    if json_string(runtime, "User")? != "65532:65532"
        || json_string(runtime, "WorkingDir")? != "/"
        || json_string(runtime, "StopSignal")? != "SIGTERM"
        || json_string_array(runtime, "Env")?
            != ["SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt"]
    {
        return Err(AdmissionError::InvalidOci);
    }
    let entrypoints = json_string_array(runtime, "Entrypoint")?;
    if entrypoints.len() != 1 {
        return Err(AdmissionError::InvalidOci);
    }
    let entrypoint = entrypoints[0].clone();
    if !entrypoint.starts_with("/nix/store/")
        || !entrypoint.ends_with(&format!("/bin/{}", expected.binary_name))
        || !valid_absolute_path(&entrypoint)
    {
        return Err(AdmissionError::InvalidOci);
    }
    let labels = runtime
        .get("Labels")
        .and_then(Value::as_object)
        .ok_or(AdmissionError::InvalidOci)?;
    let expected_labels = expected_labels(expected);
    if labels.len() != expected_labels.len()
        || expected_labels
            .iter()
            .any(|(key, value)| labels.get(key).and_then(Value::as_str) != Some(value))
    {
        return Err(AdmissionError::InvalidOci);
    }
    Ok(entrypoint)
}

fn expected_labels(expected: &OciExpectation<'_>) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    for (key, value) in [
        (
            "dev.radroots.build.feature-profile",
            "service-host".to_owned(),
        ),
        (
            "dev.radroots.build.lib-revision",
            expected.lib_revision.to_owned(),
        ),
        ("dev.radroots.build.rust-version", "1.97.1".to_owned()),
        ("dev.radroots.build.target", LINUX_TARGET.to_owned()),
        (
            "dev.radroots.contract.admin-version",
            expected.contract_versions.admin.to_string(),
        ),
        (
            "dev.radroots.contract.config-version",
            expected.contract_versions.config.to_string(),
        ),
        (
            "dev.radroots.contract.provider-version",
            expected.contract_versions.provider.to_string(),
        ),
        (
            "dev.radroots.contract.state-version",
            expected.contract_versions.state.to_string(),
        ),
        (
            "dev.radroots.contract.status-version",
            expected.contract_versions.status.to_string(),
        ),
        (
            "dev.radroots.mount.config",
            format!("/etc/radroots/services/{}", expected.service),
        ),
        ("dev.radroots.mount.config.mode", "read-only".to_owned()),
        (
            "dev.radroots.mount.credentials",
            format!("/etc/radroots/secrets/services/{}", expected.service),
        ),
        (
            "dev.radroots.mount.credentials.mode",
            "read-only".to_owned(),
        ),
        (
            "dev.radroots.mount.runtime",
            format!("/run/radroots/services/{}", expected.service),
        ),
        ("dev.radroots.mount.runtime.mode", "read-write".to_owned()),
        (
            "dev.radroots.mount.state",
            format!("/var/lib/radroots/services/{}", expected.service),
        ),
        ("dev.radroots.mount.state.mode", "read-write".to_owned()),
        ("dev.radroots.rootfs", "read-only-compatible".to_owned()),
        (
            "org.opencontainers.image.description",
            format!("Hardened {} service image", expected.service),
        ),
        (
            "org.opencontainers.image.licenses",
            expected.license.to_owned(),
        ),
        (
            "org.opencontainers.image.revision",
            expected.service_revision.to_owned(),
        ),
        (
            "org.opencontainers.image.title",
            expected.service.to_owned(),
        ),
        (
            "org.opencontainers.image.version",
            expected.version.to_owned(),
        ),
    ] {
        labels.insert(key.to_owned(), value);
    }
    labels
}

#[cfg(test)]
pub(crate) fn fixture_labels(expected: &OciExpectation<'_>) -> BTreeMap<String, String> {
    expected_labels(expected)
}

fn validate_repositories(
    snapshot: &safe_artifact_io::TraversalSnapshot,
    image_name: &str,
    version: &str,
    layers: &[String],
) -> Result<(), AdmissionError> {
    let repositories = read_json_member(snapshot, "repositories")?;
    let last_layer = layers
        .last()
        .and_then(|path| path.split('/').next())
        .ok_or(AdmissionError::InvalidOci)?;
    if repositories == json!({ image_name: { version: last_layer } }) {
        Ok(())
    } else {
        Err(AdmissionError::InvalidOci)
    }
}

fn validate_outer_inventory(
    snapshot: &safe_artifact_io::TraversalSnapshot,
    config_name: &str,
    layers: &[String],
) -> Result<(), AdmissionError> {
    let mut expected_files = BTreeSet::from([
        "manifest.json".to_owned(),
        "repositories".to_owned(),
        config_name.to_owned(),
    ]);
    let mut expected_directories = BTreeSet::new();
    for layer in layers {
        let directory = layer.split('/').next().ok_or(AdmissionError::InvalidOci)?;
        expected_directories.insert(directory.to_owned());
        expected_files.insert(format!("{directory}/VERSION"));
        expected_files.insert(format!("{directory}/json"));
        expected_files.insert(layer.clone());
    }
    let observed_files = snapshot
        .files()
        .iter()
        .map(|file| {
            file.relative_path()
                .to_str()
                .map(str::to_owned)
                .ok_or(AdmissionError::InvalidOci)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let observed_directories = snapshot
        .directories()
        .filter_map(|(path, _)| (!path.as_os_str().is_empty()).then_some(path))
        .map(|path| {
            path.to_str()
                .map(str::to_owned)
                .ok_or(AdmissionError::InvalidOci)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if observed_files != expected_files || observed_directories != expected_directories {
        return Err(AdmissionError::InvalidOci);
    }
    for layer in layers {
        let directory = layer.split('/').next().ok_or(AdmissionError::InvalidOci)?;
        if read_member(snapshot, &format!("{directory}/VERSION"), 16)? != b"1.0" {
            return Err(AdmissionError::InvalidOci);
        }
        serde_json::from_slice::<Value>(&read_member(
            snapshot,
            &format!("{directory}/json"),
            MAX_JSON_BYTES,
        )?)
        .map_err(|_| AdmissionError::InvalidOci)?;
    }
    Ok(())
}

fn validate_layer_tar(
    materialized_root: &Path,
    relative: &str,
    entrypoint: &str,
) -> Result<u32, AdmissionError> {
    let path = materialized_root.join(relative);
    let mut file = fs::File::open(&path).map_err(|_| AdmissionError::InvalidOci)?;
    let length = file
        .metadata()
        .map_err(|_| AdmissionError::InvalidOci)?
        .len();
    if length == 0 || length > MAX_ARCHIVE_EXPANDED_BYTES {
        return Err(AdmissionError::InvalidOci);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| AdmissionError::InvalidOci)?;
    let mut archive = tar::Archive::new(file);
    let mut count = 0_u64;
    let mut payload = 0_u64;
    let mut paths = BTreeSet::new();
    let mut entrypoint_count = 0_u32;
    let expected_entrypoint = entrypoint.trim_start_matches('/').as_bytes();
    for entry in archive.entries().map_err(|_| AdmissionError::InvalidOci)? {
        let mut entry = entry.map_err(|_| AdmissionError::InvalidOci)?;
        count = count.checked_add(1).ok_or(AdmissionError::InvalidOci)?;
        if count > MAX_ARCHIVE_MEMBERS {
            return Err(AdmissionError::InvalidOci);
        }
        let path = entry.path_bytes();
        validate_relative_path(&path)?;
        let normalized = path.strip_suffix(b"/").unwrap_or(&path).to_vec();
        if !paths.insert(normalized.clone()) {
            return Err(AdmissionError::InvalidOci);
        }
        let kind = entry.header().entry_type();
        if kind.is_file() {
            payload = payload
                .checked_add(entry.size())
                .ok_or(AdmissionError::InvalidOci)?;
            if payload > MAX_ARCHIVE_EXPANDED_BYTES {
                return Err(AdmissionError::InvalidOci);
            }
            if normalized == expected_entrypoint {
                if entry
                    .header()
                    .mode()
                    .map_err(|_| AdmissionError::InvalidOci)?
                    & 0o111
                    == 0
                {
                    return Err(AdmissionError::InvalidOci);
                }
                entrypoint_count = entrypoint_count
                    .checked_add(1)
                    .ok_or(AdmissionError::InvalidOci)?;
            }
            std::io::copy(&mut entry, &mut std::io::sink())
                .map_err(|_| AdmissionError::InvalidOci)?;
        } else if kind.is_dir() {
            if !path.ends_with(b"/") || entry.size() != 0 {
                return Err(AdmissionError::InvalidOci);
            }
        } else if kind.is_symlink() || kind.is_hard_link() {
            if entry.size() != 0 {
                return Err(AdmissionError::InvalidOci);
            }
            let target = entry.link_name_bytes().ok_or(AdmissionError::InvalidOci)?;
            validate_link_target(&normalized, &target)?;
        } else {
            return Err(AdmissionError::InvalidOci);
        }
    }
    if count == 0 {
        Err(AdmissionError::InvalidOci)
    } else {
        Ok(entrypoint_count)
    }
}

fn validate_layer_name(path: &str) -> Result<(), AdmissionError> {
    let Some((directory, leaf)) = path.split_once('/') else {
        return Err(AdmissionError::InvalidOci);
    };
    if leaf == "layer.tar" && valid_hex(directory, 64) {
        Ok(())
    } else {
        Err(AdmissionError::InvalidOci)
    }
}

fn validate_relative_path(path: &[u8]) -> Result<(), AdmissionError> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.starts_with(b"/")
        || path.contains(&0)
        || path.contains(&b'\\')
        || std::str::from_utf8(path).is_err()
    {
        return Err(AdmissionError::InvalidOci);
    }
    let path = path.strip_suffix(b"/").unwrap_or(path);
    let mut depth = 0_usize;
    for component in path.split(|byte| *byte == b'/') {
        depth = depth.checked_add(1).ok_or(AdmissionError::InvalidOci)?;
        if component.is_empty() || matches!(component, b"." | b"..") || depth > MAX_DEPTH {
            return Err(AdmissionError::InvalidOci);
        }
    }
    Ok(())
}

fn validate_link_target(path: &[u8], target: &[u8]) -> Result<(), AdmissionError> {
    if target.is_empty()
        || target.len() > MAX_PATH_BYTES
        || target.starts_with(b"/")
        || target.contains(&0)
        || target.contains(&b'\\')
        || std::str::from_utf8(target).is_err()
    {
        return Err(AdmissionError::InvalidOci);
    }
    let mut depth = path.split(|byte| *byte == b'/').count().saturating_sub(1);
    for component in target.split(|byte| *byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => depth = depth.checked_sub(1).ok_or(AdmissionError::InvalidOci)?,
            _ => {
                depth = depth.checked_add(1).ok_or(AdmissionError::InvalidOci)?;
                if depth > MAX_DEPTH {
                    return Err(AdmissionError::InvalidOci);
                }
            }
        }
    }
    Ok(())
}

fn valid_absolute_path(value: &str) -> bool {
    let path = Path::new(value);
    path.is_absolute()
        && value.len() <= MAX_PATH_BYTES
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}
fn read_json_member(
    snapshot: &safe_artifact_io::TraversalSnapshot,
    name: &str,
) -> Result<Value, AdmissionError> {
    serde_json::from_slice(&read_member(snapshot, name, MAX_JSON_BYTES)?)
        .map_err(|_| AdmissionError::InvalidOci)
}
fn read_member(
    snapshot: &safe_artifact_io::TraversalSnapshot,
    name: &str,
    maximum: u64,
) -> Result<Vec<u8>, AdmissionError> {
    snapshot
        .read(snapshot_member(snapshot, name)?, maximum)
        .map_err(|_| AdmissionError::InvalidOci)
}
fn snapshot_member<'a>(
    snapshot: &'a safe_artifact_io::TraversalSnapshot,
    name: &str,
) -> Result<&'a safe_artifact_io::TraversedFile, AdmissionError> {
    snapshot
        .files()
        .iter()
        .find(|file| file.relative_path() == Path::new(name))
        .ok_or(AdmissionError::InvalidOci)
}
fn require_exact_keys(
    object: &Map<String, Value>,
    expected: &[&str],
) -> Result<(), AdmissionError> {
    let observed = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if observed == expected.iter().copied().collect() {
        Ok(())
    } else {
        Err(AdmissionError::InvalidOci)
    }
}
fn json_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, AdmissionError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(AdmissionError::InvalidOci)
}
fn json_string_array(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Vec<String>, AdmissionError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or(AdmissionError::InvalidOci)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(AdmissionError::InvalidOci)
        })
        .collect()
}
fn all_unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}
fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn valid_identifier(value: &str) -> bool {
    value.len() <= 128
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
fn valid_binary_name(value: &str) -> bool {
    value.len() <= 128
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}
fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_is_exact() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("root");
        validate_contract(root).expect("artifact admission contract");
    }

    #[test]
    fn native_binary_and_negative_formats_are_bounded() {
        let target = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            MACOS_TARGET
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            LINUX_TARGET
        } else {
            return;
        };
        let binary = std::env::current_exe().expect("test binary");
        let bytes = fs::read(&binary).expect("test binary bytes");
        admit_binary_bytes(&bytes, target).expect("native binary structure");
        if bytes.len() <= MAX_BINARY_PARSE_BYTES as usize {
            admit_binary_inner(&binary, target, false).expect("bounded native admission");
        } else {
            assert_eq!(
                admit_binary_inner(&binary, target, false),
                Err(AdmissionError::InvalidBinary)
            );
        }
        let wrong = if target == MACOS_TARGET {
            LINUX_TARGET
        } else {
            MACOS_TARGET
        };
        assert_eq!(
            admit_binary_inner(&binary, wrong, false),
            Err(AdmissionError::InvalidBinary)
        );
        let root = tempfile::tempdir().expect("tempdir");
        let arbitrary = root.path().join("binary");
        fs::write(&arbitrary, b"arbitrary bytes").expect("fixture");
        assert_eq!(
            admit_binary_inner(&arbitrary, target, false),
            Err(AdmissionError::InvalidBinary)
        );
        assert_eq!(
            validate_dynamic_libraries(&["libsqlite3.so.0".to_owned()]),
            Err(AdmissionError::InvalidBinary)
        );
    }

    #[test]
    fn archive_paths_and_agpl_labels_fail_closed() {
        assert!(validate_relative_path(b"nix/store/hash/bin/service").is_ok());
        assert!(validate_relative_path(b"../escape").is_err());
        assert!(validate_link_target(b"nix/store/hash/lib/link", b"../target").is_ok());
        assert!(validate_link_target(b"link", b"../escape").is_err());
        let expected = OciExpectation {
            service: "fixture_service",
            binary_name: "fixture-service",
            version: "0.1.0-alpha",
            service_revision: "1111111111111111111111111111111111111111",
            lib_revision: "2222222222222222222222222222222222222222",
            license: AGPL_LICENSE,
            contract_versions: ContractVersions {
                admin: 3,
                config: 1,
                provider: 5,
                state: 2,
                status: 4,
            },
        };
        let labels = expected_labels(&expected);
        assert_eq!(labels["org.opencontainers.image.licenses"], AGPL_LICENSE);
        assert_eq!(labels.len(), 23);
    }
}

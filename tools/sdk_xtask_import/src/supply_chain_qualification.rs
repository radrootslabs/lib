use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CONTRACT_PATH: &str = "contracts/releases/supply_chain.toml";
const DENY_PATH: &str = "deny.toml";
const SBOM_FILENAME: &str = "radroots-release-v1-sbom.json";

#[derive(Debug, Deserialize)]
struct Contract {
    schema_version: u16,
    spec_id: String,
    package_version: String,
    tools: Tools,
    sbom: Sbom,
    advisory_exception: Vec<AdvisoryException>,
    package: Vec<Package>,
}

#[derive(Debug, Deserialize)]
struct Tools {
    cargo_deny: String,
    cargo_cyclonedx: String,
}

#[derive(Debug, Deserialize)]
struct Sbom {
    format: String,
    spec_version: String,
    target: String,
    all_features: bool,
    source_date_epoch: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AdvisoryException {
    id: String,
    package: String,
    affected_version: String,
    introduced_by: String,
    classification: String,
    mitigation: String,
    remove_when: String,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    manifest_path: String,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<MetadataPackage>,
}

#[derive(Debug, Deserialize)]
struct MetadataPackage {
    name: String,
    version: String,
    manifest_path: PathBuf,
}

struct GeneratedSboms(Vec<PathBuf>);

impl Drop for GeneratedSboms {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

pub fn run(root: &Path) -> Result<(), String> {
    let contract = load(root)?;
    validate(root, &contract, 2)?;
    verify_tools(&contract)?;
    let metadata = load_metadata(root)?;
    qualify_dependencies(root, &contract)?;
    let sbom_hashes = qualify_sboms(root, &contract, &metadata)?;
    let provenance = build_provenance(root, &contract, &sbom_hashes)?;
    validate_provenance(&provenance, &contract)?;
    eprintln!(
        "qualified {} package supply chains; provenance sha256={}",
        contract.package.len(),
        sha256(&serde_json::to_vec(&provenance).map_err(|error| error.to_string())?)
    );
    Ok(())
}

fn load(root: &Path) -> Result<Contract, String> {
    let path = root.join(CONTRACT_PATH);
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn validate(root: &Path, contract: &Contract, expected_packages: usize) -> Result<(), String> {
    if contract.schema_version != 1
        || contract.spec_id != "radroots.crates.release.v1"
        || contract.package_version != "0.1.0-alpha"
        || contract.tools.cargo_deny != "0.19.8"
        || contract.tools.cargo_cyclonedx != "0.5.9"
        || contract.sbom.format != "json"
        || contract.sbom.spec_version != "1.5"
        || contract.sbom.target != "all"
        || !contract.sbom.all_features
        || contract.sbom.source_date_epoch != 0
    {
        return Err("invalid supply-chain qualification contract".to_owned());
    }

    let names = contract
        .package
        .iter()
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();
    let manifests = contract
        .package
        .iter()
        .map(|package| package.manifest_path.as_str())
        .collect::<BTreeSet<_>>();
    if names.len() != expected_packages
        || names.len() != contract.package.len()
        || manifests.len() != contract.package.len()
    {
        return Err(format!(
            "supply-chain contract requires exactly {expected_packages} unique packages and manifests"
        ));
    }
    for package in &contract.package {
        let path = root.join(&package.manifest_path);
        let raw = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let manifest: toml::Value = toml::from_str(&raw)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        let package_table = manifest
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("{} has no package table", path.display()))?;
        let name = package_table.get("name").and_then(toml::Value::as_str);
        let version = package_table.get("version").and_then(toml::Value::as_str);
        let inherits_workspace_version = package_table
            .get("version")
            .and_then(toml::Value::as_table)
            .and_then(|version| version.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true);
        if name != Some(package.name.as_str())
            || version != Some(contract.package_version.as_str()) && !inherits_workspace_version
        {
            return Err(format!(
                "{} does not declare {} {}",
                path.display(),
                package.name,
                contract.package_version
            ));
        }
    }

    validate_exceptions(root, contract)
}

fn validate_exceptions(root: &Path, contract: &Contract) -> Result<(), String> {
    let expected = BTreeSet::from([
        "CARGO-YANKED-SPIN-0.9.8".to_owned(),
        "RUSTSEC-2024-0384".to_owned(),
        "RUSTSEC-2024-0421".to_owned(),
    ]);
    let actual = contract
        .advisory_exception
        .iter()
        .map(|exception| exception.id.clone())
        .collect::<BTreeSet<_>>();
    if actual != expected || actual.len() != contract.advisory_exception.len() {
        return Err("supply-chain advisory exceptions are not the exact approved set".to_owned());
    }
    for exception in &contract.advisory_exception {
        if exception.package.is_empty()
            || exception.affected_version.is_empty()
            || exception.introduced_by.is_empty()
            || exception.classification.is_empty()
            || exception.mitigation.is_empty()
            || exception.remove_when.is_empty()
        {
            return Err(format!("advisory exception {} is incomplete", exception.id));
        }
        let exact = match exception.id.as_str() {
            "RUSTSEC-2024-0384" => {
                exception.package == "instant"
                    && exception.affected_version == "0.1.13"
                    && exception.classification == "unmaintained"
                    && exception.remove_when == "nostr >=0.45.0 stable"
            }
            "RUSTSEC-2024-0421" => {
                exception.package == "idna"
                    && exception.affected_version == "0.5.0"
                    && exception.classification == "vulnerability"
                    && exception.remove_when == "nostr >=0.45.0 stable"
            }
            "CARGO-YANKED-SPIN-0.9.8" => {
                exception.package == "spin"
                    && exception.affected_version == "0.9.8"
                    && exception.classification == "yanked"
                    && exception.remove_when
                        == "sqlx no longer resolves flume 0.12.0 with spin 0.9.8"
            }
            _ => false,
        };
        if !exact {
            return Err(format!(
                "advisory exception {} differs from its exact approved policy",
                exception.id
            ));
        }
    }

    let lock_raw = fs::read_to_string(root.join("Cargo.lock"))
        .map_err(|error| format!("failed to read Cargo.lock: {error}"))?;
    let lock: toml::Value = toml::from_str(&lock_raw)
        .map_err(|error| format!("failed to parse Cargo.lock: {error}"))?;
    let locked_packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "Cargo.lock contains no packages".to_owned())?;
    for exception in &contract.advisory_exception {
        let present = locked_packages.iter().any(|package| {
            package.get("name").and_then(toml::Value::as_str) == Some(exception.package.as_str())
                && package.get("version").and_then(toml::Value::as_str)
                    == Some(exception.affected_version.as_str())
        });
        if !present {
            return Err(format!(
                "advisory exception {} is stale because {} {} is absent from Cargo.lock",
                exception.id, exception.package, exception.affected_version
            ));
        }
    }
    let patched_url_present = locked_packages.iter().any(|package| {
        package.get("name").and_then(toml::Value::as_str) == Some("url")
            && package
                .get("version")
                .and_then(toml::Value::as_str)
                .and_then(|version| semver::Version::parse(version).ok())
                .is_some_and(|version| version >= semver::Version::new(2, 5, 4))
    });
    if !patched_url_present {
        return Err("the IDNA exception requires url 2.5.4 or newer in Cargo.lock".to_owned());
    }

    let deny_raw = fs::read_to_string(root.join(DENY_PATH))
        .map_err(|error| format!("failed to read {DENY_PATH}: {error}"))?;
    let deny: toml::Value = toml::from_str(&deny_raw)
        .map_err(|error| format!("failed to parse {DENY_PATH}: {error}"))?;
    let ignored = deny
        .get("advisories")
        .and_then(|value| value.get("ignore"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "deny.toml advisories.ignore is missing".to_owned())?
        .iter()
        .filter_map(toml::Value::as_str)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let expected_ignored = expected
        .iter()
        .filter(|id| id.starts_with("RUSTSEC-"))
        .cloned()
        .collect::<BTreeSet<_>>();
    if ignored != expected_ignored {
        return Err("deny.toml advisory ignores differ from the governed exceptions".to_owned());
    }

    let relay_source = root.join("crates/transport_nostr/src/relay.rs");
    if relay_source.exists() {
        let source = fs::read_to_string(&relay_source)
            .map_err(|error| format!("failed to read {}: {error}", relay_source.display()))?;
        if !source.contains("Url::parse(canonical)") {
            return Err(
                "the IDNA exception requires patched relay URL canonicalization".to_owned(),
            );
        }
    }
    Ok(())
}

fn verify_tools(contract: &Contract) -> Result<(), String> {
    verify_tool(
        &["deny", "--version"],
        "cargo-deny",
        &contract.tools.cargo_deny,
    )?;
    verify_tool(
        &["cyclonedx", "--version"],
        "cargo-cyclonedx",
        &contract.tools.cargo_cyclonedx,
    )
}

fn verify_tool(args: &[&str], name: &str, expected: &str) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(args)
        .output()
        .map_err(|error| format!("failed to start {name}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{name} {expected} is required"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let installed = stdout
        .split_whitespace()
        .find_map(|value| semver::Version::parse(value).ok())
        .ok_or_else(|| format!("could not parse {name} version: {stdout}"))?;
    let expected = semver::Version::parse(expected)
        .map_err(|error| format!("invalid governed {name} version: {error}"))?;
    if installed != expected {
        return Err(format!("{name} {expected} is required, found {installed}"));
    }
    Ok(())
}

fn load_metadata(root: &Path) -> Result<Metadata, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to start cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err("locked cargo metadata failed".to_owned());
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse cargo metadata: {error}"))
}

fn qualify_dependencies(root: &Path, contract: &Contract) -> Result<(), String> {
    let mut saw_governed_yank = false;
    for package in &contract.package {
        let manifest = root.join(&package.manifest_path);
        let common = [
            "deny",
            "-L",
            "error",
            "--manifest-path",
            manifest
                .to_str()
                .ok_or_else(|| "non-UTF-8 package manifest path".to_owned())?,
            "--all-features",
            "--locked",
            "check",
        ];
        saw_governed_yank |= qualify_advisories(root, &manifest, &package.name)?;
        run_command(
            root,
            "cargo",
            common
                .iter()
                .copied()
                .chain(["bans", "licenses", "sources"])
                .collect::<Vec<_>>(),
            &format!("dependency policy failed for {}", package.name),
        )?;
    }
    if !saw_governed_yank {
        return Err("the governed spin 0.9.8 yank is stale and must be removed".to_owned());
    }
    Ok(())
}

fn qualify_advisories(root: &Path, manifest: &Path, package: &str) -> Result<bool, String> {
    let output = Command::new("cargo")
        .args([
            "deny",
            "--format",
            "json",
            "--log-level",
            "warn",
            "--manifest-path",
            manifest
                .to_str()
                .ok_or_else(|| "non-UTF-8 package manifest path".to_owned())?,
            "--all-features",
            "--locked",
            "check",
            "--allow",
            "advisory-not-detected",
            "advisories",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to start cargo-deny: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!(
            "advisory qualification failed for {package}: {stdout}{stderr}"
        ));
    }

    let mut saw_governed_yank = false;
    for line in stdout.lines().chain(stderr.lines()) {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if message.get("type").and_then(Value::as_str) != Some("diagnostic") {
            continue;
        }
        let fields = message
            .get("fields")
            .and_then(Value::as_object)
            .ok_or_else(|| "cargo-deny emitted a malformed diagnostic".to_owned())?;
        let code = fields
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let krate = fields
            .get("graphs")
            .and_then(Value::as_array)
            .and_then(|graphs| graphs.first())
            .and_then(|graph| graph.get("Krate"));
        let exact_governed_yank = code == "yanked"
            && krate
                .and_then(|krate| krate.get("name"))
                .and_then(Value::as_str)
                == Some("spin")
            && krate
                .and_then(|krate| krate.get("version"))
                .and_then(Value::as_str)
                == Some("0.9.8");
        if exact_governed_yank {
            saw_governed_yank = true;
        } else {
            return Err(format!(
                "unapproved cargo-deny diagnostic for {package}: {line}"
            ));
        }
    }
    Ok(saw_governed_yank)
}

fn qualify_sboms(
    root: &Path,
    contract: &Contract,
    metadata: &Metadata,
) -> Result<BTreeMap<String, String>, String> {
    let mut paths = metadata
        .packages
        .iter()
        .map(|package| {
            package
                .manifest_path
                .parent()
                .expect("manifest has parent")
                .join(SBOM_FILENAME)
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    if let Some(path) = paths.iter().find(|path| path.exists()) {
        return Err(format!(
            "refusing to overwrite pre-existing SBOM {}",
            path.display()
        ));
    }
    let _generated = GeneratedSboms(paths);

    let status = Command::new("cargo")
        .args([
            "cyclonedx",
            "--quiet",
            "--manifest-path",
            "Cargo.toml",
            "--format",
            &contract.sbom.format,
            "--all-features",
            "--target",
            &contract.sbom.target,
            "--spec-version",
            &contract.sbom.spec_version,
            "--license-strict",
            "--license-accept-named",
            "MIT/Apache-2.0",
            "--license-accept-named",
            "Apache-2.0/MIT",
            "--license-accept-named",
            "Apache-2.0 / MIT",
            "--override-filename",
            "radroots-release-v1-sbom",
        ])
        .env(
            "SOURCE_DATE_EPOCH",
            contract.sbom.source_date_epoch.to_string(),
        )
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to start cargo-cyclonedx: {error}"))?;
    if !status.success() {
        return Err("CycloneDX SBOM generation failed".to_owned());
    }

    let by_name = metadata
        .packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let mut hashes = BTreeMap::new();
    for package in &contract.package {
        let metadata_package = by_name
            .get(package.name.as_str())
            .ok_or_else(|| format!("cargo metadata omitted {}", package.name))?;
        if metadata_package.version != contract.package_version {
            return Err(format!("{} metadata version drifted", package.name));
        }
        let path = metadata_package
            .manifest_path
            .parent()
            .expect("manifest has parent")
            .join(SBOM_FILENAME);
        let raw = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let mut sbom: Value = serde_json::from_slice(&raw)
            .map_err(|error| format!("invalid SBOM {}: {error}", path.display()))?;
        validate_sbom(&sbom, &package.name, &contract.package_version)?;
        normalize_sbom(&mut sbom, root);
        hashes.insert(
            package.name.clone(),
            sha256(&serde_json::to_vec(&sbom).map_err(|error| error.to_string())?),
        );
    }
    Ok(hashes)
}

fn validate_sbom(sbom: &Value, name: &str, version: &str) -> Result<(), String> {
    let component = sbom
        .pointer("/metadata/component")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{name} SBOM has no root component"))?;
    if sbom.get("bomFormat").and_then(Value::as_str) != Some("CycloneDX")
        || sbom.get("specVersion").and_then(Value::as_str) != Some("1.5")
        || component.get("name").and_then(Value::as_str) != Some(name)
        || component.get("version").and_then(Value::as_str) != Some(version)
        || sbom
            .get("components")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
        || sbom
            .get("dependencies")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        return Err(format!(
            "{name} SBOM is incomplete or identifies the wrong package"
        ));
    }
    Ok(())
}

fn normalize_sbom(value: &mut Value, root: &Path) {
    normalize_sbom_value(value, root.to_string_lossy().as_ref());
}

fn normalize_sbom_value(value: &mut Value, root: &str) {
    match value {
        Value::Object(object) => {
            object.remove("serialNumber");
            for value in object.values_mut() {
                normalize_sbom_value(value, root);
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize_sbom_value(value, root);
            }
        }
        Value::String(string) if string.contains(root) => {
            *string = string.replace(root, "$REPOSITORY");
        }
        _ => {}
    }
}

fn build_provenance(
    root: &Path,
    contract: &Contract,
    sbom_hashes: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let revision = command_stdout(root, "git", &["rev-parse", "HEAD"])?;
    if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Git provenance revision is not a full commit ID".to_owned());
    }
    let lock = fs::read(root.join("Cargo.lock"))
        .map_err(|error| format!("failed to read Cargo.lock: {error}"))?;
    let packages = contract
        .package
        .iter()
        .map(|package| {
            json!({
                "name": package.name,
                "version": contract.package_version,
                "manifestPath": package.manifest_path,
                "sbomSha256": sbom_hashes.get(&package.name),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schemaVersion": 1,
        "specId": contract.spec_id,
        "source": {
            "gitCommit": revision,
            "cargoLockSha256": sha256(&lock),
        },
        "tools": {
            "cargoDeny": contract.tools.cargo_deny,
            "cargoCyclonedx": contract.tools.cargo_cyclonedx,
        },
        "packages": packages,
        "advisoryExceptions": contract.advisory_exception,
    }))
}

fn validate_provenance(provenance: &Value, contract: &Contract) -> Result<(), String> {
    if provenance.get("schemaVersion").and_then(Value::as_u64) != Some(1)
        || provenance.get("specId").and_then(Value::as_str) != Some(contract.spec_id.as_str())
        || provenance
            .get("packages")
            .and_then(Value::as_array)
            .is_none_or(|packages| packages.len() != contract.package.len())
        || provenance
            .pointer("/source/cargoLockSha256")
            .and_then(Value::as_str)
            .is_none_or(|hash| hash.len() != 64)
    {
        return Err("generated supply-chain provenance is incomplete".to_owned());
    }
    Ok(())
}

fn run_command(root: &Path, program: &str, args: Vec<&str>, failure: &str) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to start {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(failure.to_owned())
    }
}

fn command_stdout(root: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to start {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{program} {} failed", args.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("{program} emitted non-UTF-8 output: {error}"))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn current_contract_is_exact_and_exception_bound() {
        let root = root();
        let contract = load(&root).expect("contract");
        validate(&root, &contract, 2).expect("valid contract");
    }

    #[test]
    fn sbom_validation_rejects_wrong_identity() {
        let sbom = json!({
            "bomFormat": "CycloneDX",
            "specVersion": "1.5",
            "metadata": {"component": {"name": "wrong", "version": "0.1.0-alpha"}},
            "components": [{}],
            "dependencies": [{}],
        });
        assert!(validate_sbom(&sbom, "radroots_core", "0.1.0-alpha").is_err());
    }

    #[test]
    fn normalization_removes_random_and_absolute_identity() {
        let mut sbom = json!({
            "serialNumber": "urn:uuid:random",
            "path": "/workspace/crate",
        });
        normalize_sbom(&mut sbom, Path::new("/workspace"));
        assert!(sbom.get("serialNumber").is_none());
        assert_eq!(
            sbom.get("path").and_then(Value::as_str),
            Some("$REPOSITORY/crate")
        );
    }
}

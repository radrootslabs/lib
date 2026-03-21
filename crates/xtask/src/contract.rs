#![forbid(unsafe_code)]

use crate::coverage::{CoverageThresholds, read_coverage_policy};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct ContractManifest {
    pub contract: ManifestContract,
    pub surface: Surface,
    pub policy: Policy,
}

#[derive(Debug, Deserialize)]
pub struct ManifestContract {
    pub name: String,
    pub version: String,
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct Surface {
    pub model_crates: Vec<String>,
    pub algorithm_crates: Vec<String>,
    pub wasm_crates: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Policy {
    pub exclude_internal_workspace_crates: bool,
    pub require_reproducible_exports: bool,
    pub require_conformance_vectors: bool,
}

#[derive(Debug, Deserialize)]
pub struct VersionPolicy {
    pub contract: VersionContract,
    pub semver: SemverRules,
    pub compatibility: CompatibilityRules,
}

#[derive(Debug, Deserialize)]
pub struct VersionContract {
    pub version: String,
    pub stability: String,
}

#[derive(Debug, Deserialize)]
pub struct SemverRules {
    pub major_on: Vec<String>,
    pub minor_on: Vec<String>,
    pub patch_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompatibilityRules {
    pub requires_conformance_pass: bool,
    pub requires_export_manifest_diff: bool,
    pub requires_release_notes: bool,
}

#[derive(Debug, Deserialize)]
pub struct ExportMapping {
    pub language: ExportLanguage,
    pub packages: BTreeMap<String, String>,
    pub artifacts: Option<ExportArtifacts>,
}

#[derive(Debug, Deserialize)]
pub struct ExportLanguage {
    pub id: String,
    pub repository: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ExportArtifacts {
    pub models_dir: Option<String>,
    pub constants_dir: Option<String>,
    pub wasm_dist_dir: Option<String>,
    pub manifest_file: Option<String>,
}

#[derive(Debug)]
pub struct ContractBundle {
    pub root: PathBuf,
    pub manifest: ContractManifest,
    pub version: VersionPolicy,
    pub exports: Vec<ExportMapping>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceCargoManifest {
    workspace: WorkspaceSection,
}

#[derive(Debug, Deserialize)]
struct WorkspaceSection {
    members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PackageCargoManifest {
    package: PackageSection,
}

#[derive(Debug, Deserialize)]
struct PackageSection {
    name: String,
    publish: Option<PackagePublish>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PackagePublish {
    Bool(bool),
    Registries(Vec<String>),
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Deserialize)]
struct CoverageRequiredFile {
    required: CoverageRequiredSection,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Deserialize)]
struct CoverageRequiredSection {
    crates: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseContractFile {
    release: ReleaseSection,
    publish: ReleaseCrateSet,
    internal: ReleaseCrateSet,
    publish_order: ReleaseCrateSet,
}

#[derive(Debug, Deserialize)]
struct ReleaseSection {
    version: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseCrateSet {
    crates: Vec<String>,
}

fn parse_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    match toml::from_str::<T>(&raw) {
        Ok(parsed) => Ok(parsed),
        Err(e) => Err(format!("parse {}: {e}", path.display())),
    }
}

fn contract_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join("contract")
}

#[derive(Debug)]
struct WorkspacePackageRecord {
    name: String,
    #[cfg_attr(not(test), allow(dead_code))]
    manifest_path: PathBuf,
    publish_enabled: bool,
    manifest_value: toml::Value,
}

fn workspace_package_records(workspace_root: &Path) -> Result<Vec<WorkspacePackageRecord>, String> {
    let workspace_manifest =
        parse_toml::<WorkspaceCargoManifest>(&workspace_root.join("Cargo.toml"))?;
    let mut records = Vec::with_capacity(workspace_manifest.workspace.members.len());
    for member in workspace_manifest.workspace.members {
        let manifest_path = workspace_root.join(&member).join("Cargo.toml");
        let raw = match fs::read_to_string(&manifest_path) {
            Ok(raw) => raw,
            Err(e) => return Err(format!("read {}: {e}", manifest_path.display())),
        };
        let manifest_value = match toml::from_str::<toml::Value>(&raw) {
            Ok(value) => value,
            Err(e) => return Err(format!("parse {}: {e}", manifest_path.display())),
        };
        let package_manifest = match toml::from_str::<PackageCargoManifest>(&raw) {
            Ok(manifest) => manifest,
            Err(e) => return Err(format!("parse {}: {e}", manifest_path.display())),
        };
        let name = package_manifest.package.name;
        let publish_enabled = package_publish_enabled(package_manifest.package.publish.as_ref());
        records.push(WorkspacePackageRecord {
            name,
            manifest_path,
            publish_enabled,
            manifest_value,
        });
    }
    Ok(records)
}

fn workspace_package_names(workspace_root: &Path) -> Result<Vec<String>, String> {
    Ok(workspace_package_records(workspace_root)?
        .into_iter()
        .map(|record| record.name)
        .collect())
}

#[cfg_attr(not(test), allow(dead_code))]
fn workspace_package_manifests(workspace_root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut manifests = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if manifests
            .insert(record.name, record.manifest_path)
            .is_some()
        {
            return Err("duplicate workspace package name in manifest map".to_string());
        }
    }
    Ok(manifests)
}

fn load_coverage_policy(
    contract_root: &Path,
) -> Result<crate::coverage::CoveragePolicyFile, String> {
    read_coverage_policy(&contract_root.join("coverage").join("policy.toml"))
}

fn load_release_contract(contract_root: &Path) -> Result<ReleaseContractFile, String> {
    parse_toml::<ReleaseContractFile>(&contract_root.join("release").join("publish-set.toml"))
}

fn package_publish_enabled(publish: Option<&PackagePublish>) -> bool {
    match publish {
        None => true,
        Some(PackagePublish::Bool(flag)) => *flag,
        Some(PackagePublish::Registries(registries)) => !registries.is_empty(),
    }
}

fn workspace_package_publish_flags(
    workspace_root: &Path,
) -> Result<BTreeMap<String, bool>, String> {
    let mut flags = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if flags
            .insert(record.name.clone(), record.publish_enabled)
            .is_some()
        {
            return Err(format!("duplicate workspace package name {}", record.name));
        }
    }
    Ok(flags)
}

fn read_workspace_package_dependencies(
    workspace_root: &Path,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let package_records = workspace_package_records(workspace_root)?;
    let workspace_names = package_records
        .iter()
        .map(|record| record.name.clone())
        .collect::<BTreeSet<_>>();

    let mut deps = BTreeMap::new();
    for record in package_records {
        let mut package_deps = BTreeSet::new();
        for section in ["dependencies", "build-dependencies"] {
            let Some(table) = record
                .manifest_value
                .get(section)
                .and_then(toml::Value::as_table)
            else {
                continue;
            };
            for dep_name in table.keys() {
                if workspace_names.contains(dep_name) {
                    package_deps.insert(dep_name.clone());
                }
            }
        }
        deps.insert(record.name, package_deps);
    }

    Ok(deps)
}

fn join_set(items: &BTreeSet<String>) -> String {
    items.iter().cloned().collect::<Vec<_>>().join(", ")
}

fn collect_unique_set(items: &[String], field: &str) -> Result<BTreeSet<String>, String> {
    let mut set = BTreeSet::new();
    for item in items {
        if item.trim().is_empty() {
            return Err(format!("{field} contains an empty crate name"));
        }
        if !set.insert(item.clone()) {
            return Err(format!("{field} has duplicate crate {}", item));
        }
    }
    Ok(set)
}

fn package_field_configured(table: &toml::value::Table, field: &str) -> bool {
    let Some(value) = table.get(field) else {
        return false;
    };
    match value {
        toml::Value::String(raw) => !raw.trim().is_empty(),
        toml::Value::Table(inner) => inner
            .get("workspace")
            .and_then(toml::Value::as_bool)
            .is_some_and(|configured| configured),
        _ => false,
    }
}

fn validate_publish_package_metadata(
    workspace_root: &Path,
    publish_crates: &BTreeSet<String>,
) -> Result<(), String> {
    let mut package_tables = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if package_tables
            .insert(record.name, record.manifest_value)
            .is_some()
        {
            return Err("duplicate workspace package name in package metadata map".to_string());
        }
    }
    for crate_name in publish_crates {
        let parsed = match package_tables.get(crate_name) {
            Some(parsed) => parsed,
            None => {
                return Err(format!(
                    "publish crate {} has no workspace manifest",
                    crate_name
                ));
            }
        };
        let package = parsed
            .get("package")
            .and_then(toml::Value::as_table)
            .expect("workspace package records include [package] table");

        if !package_field_configured(package, "description") {
            return Err(format!(
                "publish crate {} must define a non-empty package.description",
                crate_name
            ));
        }
        for field in ["repository", "homepage", "documentation", "readme"] {
            if !package_field_configured(package, field) {
                return Err(format!(
                    "publish crate {} must configure package.{}",
                    crate_name, field
                ));
            }
        }
    }
    Ok(())
}

fn parse_coverage_percent(raw: &str, field: &str, crate_name: &str) -> Result<f64, String> {
    match raw.parse::<f64>() {
        Ok(value) => Ok(value),
        Err(e) => Err(format!("parse {} for {}: {e}", field, crate_name)),
    }
}

fn load_coverage_refresh_rows(
    workspace_root: &Path,
) -> Result<BTreeMap<String, (String, f64, f64, f64, f64)>, String> {
    let report_path = workspace_root
        .join("target")
        .join("coverage")
        .join("coverage-refresh.tsv");
    let raw = match fs::read_to_string(&report_path) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", report_path.display())),
    };
    let mut rows = BTreeMap::new();
    for line in raw.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts = trimmed.split('\t').collect::<Vec<_>>();
        if parts.len() < 6 {
            return Err(format!(
                "coverage row must have at least 6 columns in {}: {}",
                report_path.display(),
                trimmed
            ));
        }
        let crate_name = parts[0].to_string();
        let status = parts[1].to_string();
        let exec = parse_coverage_percent(parts[2], "exec", &crate_name)?;
        let func = parse_coverage_percent(parts[3], "func", &crate_name)?;
        let branch = parse_coverage_percent(parts[4], "branch", &crate_name)?;
        let region = parse_coverage_percent(parts[5], "region", &crate_name)?;
        if rows
            .insert(crate_name.clone(), (status, exec, func, branch, region))
            .is_some()
        {
            return Err(format!(
                "duplicate coverage row for crate {} in {}",
                crate_name,
                report_path.display()
            ));
        }
    }
    Ok(rows)
}

fn validate_required_coverage_summary(
    workspace_root: &Path,
    required_crates: &BTreeSet<String>,
    thresholds: CoverageThresholds,
) -> Result<(), String> {
    let rows = load_coverage_refresh_rows(workspace_root)?;
    for crate_name in required_crates {
        let (status, exec, func, branch, region) = rows.get(crate_name).ok_or_else(|| {
            format!(
                "required coverage crate {} missing from coverage-refresh.tsv",
                crate_name
            )
        })?;
        if status != "pass" {
            return Err(format!(
                "required coverage crate {} has non-pass status {}",
                crate_name, status
            ));
        }
        if *exec < thresholds.fail_under_exec_lines
            || *func < thresholds.fail_under_functions
            || *branch < thresholds.fail_under_branches
            || *region < thresholds.fail_under_regions
        {
            return Err(format!(
                "required coverage crate {} must satisfy coverage policy {},{},{},{}, found {}/{}/{}/{}",
                crate_name,
                thresholds.fail_under_exec_lines,
                thresholds.fail_under_functions,
                thresholds.fail_under_branches,
                thresholds.fail_under_regions,
                exec,
                func,
                branch,
                region
            ));
        }
    }
    Ok(())
}

const CORE_UNIT_DIMENSION_ENUM: &str = "RadrootsCoreUnitDimension";
const CORE_UNIT_DIMENSION_ORDER: [&str; 3] = ["Count", "Mass", "Volume"];

fn extract_enum_body<'a>(source: &'a str, enum_name: &str) -> Result<&'a str, String> {
    let marker = format!("pub enum {enum_name}");
    let enum_start = match source.find(&marker) {
        Some(index) => index,
        None => return Err(format!("missing enum {enum_name}")),
    };
    let after_start = &source[enum_start..];
    let open_rel = match after_start.find('{') {
        Some(index) => index,
        None => return Err(format!("missing opening brace for enum {enum_name}")),
    };
    let open_idx = enum_start + open_rel;
    let mut depth = 0usize;
    for (offset, ch) in source[open_idx..].char_indices() {
        if ch == '{' {
            depth += 1;
            continue;
        }
        if ch != '}' {
            continue;
        }
        depth = depth.saturating_sub(1);
        if depth == 0 {
            let close_idx = open_idx + offset;
            return Ok(&source[(open_idx + 1)..close_idx]);
        }
    }
    Err(format!("missing closing brace for enum {enum_name}"))
}

fn parse_enum_variants(enum_body: &str) -> Vec<String> {
    enum_body
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                return None;
            }
            let before_comma = trimmed
                .split_once(',')
                .map_or(trimmed, |(head, _)| head)
                .trim();
            if before_comma.is_empty() {
                return None;
            }
            let before_discriminant = before_comma
                .split_once('=')
                .map_or(before_comma, |(head, _)| head)
                .trim();
            if before_discriminant.is_empty() {
                return None;
            }
            let ident = before_discriminant
                .split_whitespace()
                .next()
                .unwrap_or_default();
            Some(ident.to_string())
        })
        .collect()
}

fn validate_core_unit_dimension_variant_order(workspace_root: &Path) -> Result<(), String> {
    let source_path = workspace_root
        .join("crates")
        .join("core")
        .join("src")
        .join("unit.rs");
    let source = match fs::read_to_string(&source_path) {
        Ok(source) => source,
        Err(e) => return Err(format!("read {}: {e}", source_path.display())),
    };
    let enum_body = extract_enum_body(&source, CORE_UNIT_DIMENSION_ENUM)?;
    let variants = parse_enum_variants(enum_body);
    let expected = CORE_UNIT_DIMENSION_ORDER
        .iter()
        .map(|item| (*item).to_string())
        .collect::<Vec<_>>();
    if variants != expected {
        return Err(format!(
            "core unit dimension variant order must be {} but was {}",
            CORE_UNIT_DIMENSION_ORDER.join(", "),
            variants.join(", ")
        ));
    }
    Ok(())
}

fn validate_coverage_policy_parity(
    workspace_root: &Path,
    contract_root: &Path,
) -> Result<(), String> {
    let workspace_packages = workspace_package_names(workspace_root)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let policy = load_coverage_policy(contract_root)?;
    let thresholds = policy.thresholds();
    if thresholds.fail_under_exec_lines != 100.0
        || thresholds.fail_under_functions != 100.0
        || thresholds.fail_under_regions != 100.0
        || thresholds.fail_under_branches != 100.0
        || !thresholds.require_branches
    {
        return Err(
            "coverage policy must enforce 100/100/100/100 with required branches".to_string(),
        );
    }

    let required_packages = policy
        .required_crate_entries()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if workspace_packages != required_packages {
        let missing = workspace_packages
            .difference(&required_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = required_packages
            .difference(&workspace_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "coverage policy missing workspace crates: {}; coverage policy includes unknown crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    Ok(())
}

fn validate_release_publish_policy(
    workspace_root: &Path,
    contract_root: &Path,
    contract_version: &str,
) -> Result<(), String> {
    let release = load_release_contract(contract_root)?;
    if release.release.version.trim().is_empty() {
        return Err("release.version must not be empty".to_string());
    }
    if release.release.version != contract_version {
        return Err(format!(
            "release.version {} must match contract version {}",
            release.release.version, contract_version
        ));
    }

    let workspace_packages = workspace_package_names(workspace_root)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let publish_set = collect_unique_set(&release.publish.crates, "publish.crates")?;
    let internal_set = collect_unique_set(&release.internal.crates, "internal.crates")?;
    let publish_order = &release.publish_order.crates;
    let publish_order_set = collect_unique_set(publish_order, "publish_order.crates")?;

    let overlap = publish_set
        .intersection(&internal_set)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !overlap.is_empty() {
        return Err(format!(
            "release publish/internal overlap is not allowed: {}",
            join_set(&overlap)
        ));
    }

    let combined = publish_set
        .union(&internal_set)
        .cloned()
        .collect::<BTreeSet<_>>();
    if combined != workspace_packages {
        let missing = workspace_packages
            .difference(&combined)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = combined
            .difference(&workspace_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "release publish/internal sets are missing workspace crates: {}; release publish/internal sets include unknown crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    if publish_order_set != publish_set {
        let missing = publish_set
            .difference(&publish_order_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = publish_order_set
            .difference(&publish_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "publish_order.crates is missing publish crates: {}; publish_order.crates has non-publish crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    let order_index = publish_order
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx))
        .collect::<BTreeMap<_, _>>();
    let dependencies = read_workspace_package_dependencies(workspace_root)
        .expect("workspace package manifests were already parsed");
    for crate_name in &publish_set {
        let crate_deps = &dependencies[crate_name];
        let crate_order = order_index[crate_name];
        for dep in crate_deps {
            if !publish_set.contains(dep) {
                continue;
            }
            let dep_order = order_index[dep];
            if dep_order >= crate_order {
                return Err(format!(
                    "publish order must place dependency {} before {}",
                    dep, crate_name
                ));
            }
        }
    }

    let publish_flags = workspace_package_publish_flags(workspace_root)
        .expect("workspace publish flags are stable");
    for crate_name in &publish_set {
        let flag = publish_flags[crate_name];
        if !flag {
            return Err(format!(
                "publish crate {} must not set publish = false",
                crate_name
            ));
        }
    }
    for crate_name in &internal_set {
        let flag = publish_flags[crate_name];
        if flag {
            return Err(format!(
                "internal crate {} must set publish = false",
                crate_name
            ));
        }
    }

    Ok(())
}

pub fn validate_release_preflight(workspace_root: &Path) -> Result<(), String> {
    let bundle = load_contract_bundle(workspace_root)?;
    validate_contract_bundle(&bundle)?;
    let release =
        load_release_contract(&bundle.root).expect("validated contract includes release metadata");
    let policy =
        load_coverage_policy(&bundle.root).expect("validated contract includes coverage metadata");
    let publish_crates = collect_unique_set(&release.publish.crates, "publish.crates")
        .expect("validated contract enforces unique publish.crates");
    let required_crate_list = policy
        .required_crates()
        .expect("validated contract includes required crates");
    let required_crates = collect_unique_set(&required_crate_list, "required.crates")
        .expect("validated contract enforces unique required.crates");
    validate_publish_package_metadata(workspace_root, &publish_crates)?;
    validate_required_coverage_summary(workspace_root, &required_crates, policy.thresholds())?;
    Ok(())
}

pub fn load_contract_bundle(workspace_root: &Path) -> Result<ContractBundle, String> {
    let root = contract_root(workspace_root);
    let manifest = parse_toml::<ContractManifest>(&root.join("manifest.toml"))?;
    let version = parse_toml::<VersionPolicy>(&root.join("version.toml"))?;
    let exports_dir = root.join("exports");
    let mut exports = Vec::new();
    let read_dir = match fs::read_dir(&exports_dir) {
        Ok(read_dir) => read_dir,
        Err(e) => return Err(format!("read dir {}: {e}", exports_dir.display())),
    };
    let mut entries = read_dir.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        exports.push(parse_toml::<ExportMapping>(&path)?);
    }
    Ok(ContractBundle {
        root,
        manifest,
        version,
        exports,
    })
}

pub fn validate_contract_bundle(bundle: &ContractBundle) -> Result<(), String> {
    if bundle.manifest.contract.name.trim().is_empty() {
        return Err("contract name is required".to_string());
    }
    if bundle.manifest.contract.version.trim().is_empty() {
        return Err("contract version is required".to_string());
    }
    if bundle.manifest.contract.source.trim().is_empty() {
        return Err("contract source is required".to_string());
    }
    if bundle.manifest.surface.model_crates.is_empty() {
        return Err("contract surface.model_crates must not be empty".to_string());
    }
    if bundle.manifest.surface.algorithm_crates.is_empty() {
        return Err("contract surface.algorithm_crates must not be empty".to_string());
    }
    if bundle.manifest.surface.wasm_crates.is_empty() {
        return Err("contract surface.wasm_crates must not be empty".to_string());
    }
    if bundle.exports.is_empty() {
        return Err("at least one language export mapping is required".to_string());
    }
    for mapping in &bundle.exports {
        if mapping.language.id.trim().is_empty() {
            return Err("language.id is required".to_string());
        }
        if mapping.language.repository.trim().is_empty() {
            return Err(format!(
                "language.repository is required for {}",
                mapping.language.id
            ));
        }
        if mapping.packages.is_empty() {
            return Err(format!(
                "packages map is required for {}",
                mapping.language.id
            ));
        }
        if mapping.language.id == "ts" {
            let artifacts = match mapping.artifacts.as_ref() {
                Some(artifacts) => artifacts,
                None => return Err("artifacts map is required for ts".to_string()),
            };
            if artifacts
                .models_dir
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
                || artifacts
                    .constants_dir
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                || artifacts
                    .wasm_dist_dir
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                || artifacts
                    .manifest_file
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
            {
                return Err("artifacts fields must be non-empty for ts".to_string());
            }
        }
    }
    if bundle.version.contract.version.trim().is_empty() {
        return Err("version.contract.version is required".to_string());
    }
    if bundle.version.contract.stability.trim().is_empty() {
        return Err("version.contract.stability is required".to_string());
    }
    if bundle.version.semver.major_on.is_empty()
        || bundle.version.semver.minor_on.is_empty()
        || bundle.version.semver.patch_on.is_empty()
    {
        return Err("version.semver rules must all be non-empty".to_string());
    }
    if !bundle.version.compatibility.requires_conformance_pass {
        return Err("compatibility.requires_conformance_pass must be true".to_string());
    }
    if !bundle.version.compatibility.requires_export_manifest_diff {
        return Err("compatibility.requires_export_manifest_diff must be true".to_string());
    }
    if !bundle.version.compatibility.requires_release_notes {
        return Err("compatibility.requires_release_notes must be true".to_string());
    }
    if !bundle.manifest.policy.exclude_internal_workspace_crates
        || !bundle.manifest.policy.require_reproducible_exports
        || !bundle.manifest.policy.require_conformance_vectors
    {
        return Err("contract policy flags must all be true".to_string());
    }
    let workspace_root = bundle
        .root
        .parent()
        .expect("contract root must have a workspace parent");
    validate_core_unit_dimension_variant_order(workspace_root)?;
    validate_coverage_policy_parity(workspace_root, &bundle.root)?;
    validate_release_publish_policy(
        workspace_root,
        &bundle.root,
        bundle.version.contract.version.as_str(),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .join("../..")
            .canonicalize()
            .expect("canonical workspace root")
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("radroots_xtask_{prefix}_{nanos}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_file(path: &Path, content: &str) {
        let _ = fs::create_dir_all(path.parent().unwrap_or(Path::new("")));
        fs::write(path, content).expect("write file");
    }

    fn strict_thresholds() -> CoverageThresholds {
        CoverageThresholds {
            fail_under_exec_lines: 100.0,
            fail_under_functions: 100.0,
            fail_under_regions: 100.0,
            fail_under_branches: 100.0,
            require_branches: true,
        }
    }

    fn create_synthetic_workspace(prefix: &str) -> PathBuf {
        let root = temp_root(prefix);
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
resolver = "2"
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots-a"
version = "0.1.0"
edition = "2024"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.example.com/a"
readme = "README.md"
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots-b"
version = "0.1.0"
edition = "2024"
publish = false
"#,
        );
        write_file(
            &root.join("crates").join("core").join("src").join("unit.rs"),
            r#"pub enum RadrootsCoreUnitDimension {
    Count,
    Mass,
    Volume,
}
"#,
        );

        write_file(
            &root.join("contract").join("manifest.toml"),
            r#"[contract]
name = "radroots-contract"
version = "1.0.0"
source = "synthetic"

[surface]
model_crates = ["radroots-a"]
algorithm_crates = ["radroots-b"]
wasm_crates = ["radroots-a-wasm"]

[policy]
exclude_internal_workspace_crates = true
require_reproducible_exports = true
require_conformance_vectors = true
"#,
        );
        write_file(
            &root.join("contract").join("version.toml"),
            r#"[contract]
version = "1.0.0"
stability = "alpha"

[semver]
major_on = ["breaking"]
minor_on = ["feature"]
patch_on = ["fix"]

[compatibility]
requires_conformance_pass = true
requires_export_manifest_diff = true
requires_release_notes = true
"#,
        );
        write_file(
            &root.join("contract").join("exports").join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_file(
            &root.join("contract").join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        write_file(
            &root
                .join("contract")
                .join("release")
                .join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a"]
"#,
        );
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-a\tpass\t100.0\t100.0\t100.0\t100.0\tfile\nradroots-b\tpass\t100.0\t100.0\t100.0\t100.0\tfile\n",
        );
        root
    }

    #[test]
    fn validate_current_contract_bundle() {
        let root = workspace_root();
        let bundle = load_contract_bundle(&root).expect("load contract");
        validate_contract_bundle(&bundle).expect("validate contract");
    }

    #[test]
    fn ts_export_mapping_covers_model_and_wasm_surface() {
        let root = workspace_root();
        let bundle = load_contract_bundle(&root).expect("load contract");
        let ts = bundle
            .exports
            .iter()
            .find(|mapping| mapping.language.id == "ts")
            .expect("ts export mapping");
        let expected = bundle
            .manifest
            .surface
            .model_crates
            .iter()
            .chain(bundle.manifest.surface.wasm_crates.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mapped = ts.packages.keys().cloned().collect::<BTreeSet<_>>();
        assert_eq!(mapped, expected);
    }

    #[test]
    fn exports_follow_package_scope_rules() {
        let root = workspace_root();
        let bundle = load_contract_bundle(&root).expect("load contract");
        for mapping in &bundle.exports {
            if mapping.language.id == "ts" {
                for package in mapping.packages.values() {
                    assert!(package.starts_with("@radroots/"));
                }
            } else {
                for package in mapping.packages.values() {
                    assert!(!package.trim().is_empty());
                }
            }
        }
    }

    #[test]
    fn non_ts_exports_only_include_model_surface() {
        let root = workspace_root();
        let bundle = load_contract_bundle(&root).expect("load contract");
        let expected = bundle
            .manifest
            .surface
            .model_crates
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for mapping in &bundle.exports {
            if mapping.language.id == "ts" {
                continue;
            }
            let mapped = mapping.packages.keys().cloned().collect::<BTreeSet<_>>();
            assert_eq!(mapped, expected);
        }
    }

    #[test]
    fn parses_enum_variants_in_declared_order() {
        let source = r#"
pub enum RadrootsCoreUnitDimension {
    Count,
    Mass,
    Volume,
}
"#;
        let enum_body = extract_enum_body(source, "RadrootsCoreUnitDimension").expect("enum body");
        let variants = parse_enum_variants(enum_body);
        assert_eq!(variants, vec!["Count", "Mass", "Volume"]);
    }

    #[test]
    fn fails_when_enum_order_does_not_match_contract() {
        let source = r#"
pub enum RadrootsCoreUnitDimension {
    Mass,
    Count,
    Volume,
}
"#;
        let enum_body = extract_enum_body(source, "RadrootsCoreUnitDimension").expect("enum body");
        let variants = parse_enum_variants(enum_body);
        let expected = CORE_UNIT_DIMENSION_ORDER
            .iter()
            .map(|item| (*item).to_string())
            .collect::<Vec<_>>();
        assert_ne!(variants, expected);
    }

    #[test]
    fn coverage_policy_includes_workspace_crates() {
        let root = workspace_root();
        let workspace_names = workspace_package_names(&root)
            .expect("workspace crates")
            .into_iter()
            .collect::<BTreeSet<_>>();
        let policy = load_coverage_policy(&root.join("contract")).expect("coverage policy");
        let required_names = policy
            .required_crates()
            .expect("required crates")
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(workspace_names, required_names);
    }

    #[test]
    fn coverage_required_crates_match_policy_required_status() {
        let root = workspace_root();
        let contract_root = root.join("contract");
        let policy = load_coverage_policy(&contract_root).expect("coverage policy");
        let required = CoverageRequiredFile {
            required: CoverageRequiredSection {
                crates: policy.required_crates().expect("coverage required"),
            },
        };
        let required_names = required
            .required
            .crates
            .into_iter()
            .collect::<BTreeSet<_>>();
        let policy_required = policy
            .required_crates()
            .expect("policy required crates")
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(required_names, policy_required);
    }

    #[test]
    fn coverage_policy_required_crates_report_policy_errors() {
        let missing_root = temp_root("load_coverage_required_missing_policy");
        let missing_err =
            load_coverage_policy(&missing_root).expect_err("missing policy should fail");
        assert!(missing_err.contains("policy.toml"));
        let _ = fs::remove_dir_all(&missing_root);

        let duplicate_root =
            create_synthetic_workspace("load_coverage_required_duplicate_required");
        let contract_root = duplicate_root.join("contract");
        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots-a\", \"radroots-a\"]\n",
        );
        let duplicate_err =
            load_coverage_policy(&contract_root).expect_err("duplicate required crates");
        assert!(duplicate_err.contains("duplicate crate"));
        let _ = fs::remove_dir_all(&duplicate_root);
    }

    #[test]
    fn package_field_configured_accepts_workspace_table() {
        let mut package = toml::value::Table::new();
        let mut repository = toml::value::Table::new();
        repository.insert("workspace".to_string(), toml::Value::Boolean(true));
        package.insert("repository".to_string(), toml::Value::Table(repository));
        assert!(package_field_configured(&package, "repository"));
    }

    #[test]
    fn validate_required_coverage_summary_enforces_strict_threshold() {
        let root = temp_root("coverage_summary");
        let coverage_dir = root.join("target").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-core\tpass\t100.0\t100.0\t100.0\t100.0\tfile\n",
        )
        .expect("write coverage file");
        let required = ["radroots-core".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect("coverage summary");

        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-core\tpass\t100.0\t99.9\t100.0\t100.0\tfile\n",
        )
        .expect("write function coverage file");
        let func_err = validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect_err("function coverage below 100");
        assert!(func_err.contains("must satisfy coverage policy"));

        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-core\tpass\t100.0\t100.0\t99.9\t100.0\tfile\n",
        )
        .expect("write branch coverage file");
        let branch_err = validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect_err("branch coverage below 100");
        assert!(branch_err.contains("must satisfy coverage policy"));

        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-core\tpass\t100.0\t100.0\t100.0\t99.9\tfile\n",
        )
        .expect("write region coverage file");
        let region_err = validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect_err("region coverage below 100");
        assert!(region_err.contains("must satisfy coverage policy"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_publish_package_metadata_requires_description() {
        let root = temp_root("publish_metadata");
        fs::create_dir_all(root.join("crates").join("a")).expect("create crate dir");
        fs::write(
            root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a"]
"#,
        )
        .expect("write workspace manifest");
        fs::write(
            root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots-a"
version = "0.1.0"
edition = "2024"
repository = { workspace = true }
homepage = { workspace = true }
documentation = "https://docs.rs/radroots-a"
readme = { workspace = true }
"#,
        )
        .expect("write package manifest");
        let publish = ["radroots-a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let err =
            validate_publish_package_metadata(&root, &publish).expect_err("missing description");
        assert!(err.contains("package.description"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn synthetic_workspace_validates_contract_and_release_preflight() {
        let root = create_synthetic_workspace("synthetic_valid");
        let bundle = load_contract_bundle(&root).expect("load synthetic bundle");
        validate_contract_bundle(&bundle).expect("validate synthetic bundle");
        validate_release_preflight(&root).expect("validate synthetic preflight");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn helper_functions_cover_error_paths() {
        let empty = collect_unique_set(&["".to_string()], "field").expect_err("empty value");
        assert!(empty.contains("field contains an empty crate name"));
        let duplicate = collect_unique_set(&["a".to_string(), "a".to_string()], "field")
            .expect_err("duplicate value");
        assert!(duplicate.contains("field has duplicate crate a"));

        let values = ["b".to_string(), "a".to_string()];
        let set = collect_unique_set(&values, "field").expect("unique values");
        assert_eq!(join_set(&set), "a, b".to_string());

        assert!(package_publish_enabled(None));
        assert!(package_publish_enabled(Some(&PackagePublish::Bool(true))));
        assert!(!package_publish_enabled(Some(&PackagePublish::Bool(false))));
        assert!(package_publish_enabled(Some(&PackagePublish::Registries(
            vec!["crates-io".to_string(),]
        ))));
        assert!(!package_publish_enabled(Some(&PackagePublish::Registries(
            Vec::new()
        ))));

        let mut package = toml::value::Table::new();
        package.insert("description".to_string(), toml::Value::Integer(42));
        assert!(!package_field_configured(&package, "description"));
    }

    #[test]
    fn workspace_package_manifests_reject_duplicate_package_names() {
        let root = temp_root("workspace_manifest_duplicates");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
"#,
        );
        let package_manifest =
            "[package]\nname = \"duplicate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            package_manifest,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            package_manifest,
        );
        let err = workspace_package_manifests(&root)
            .expect_err("duplicate package names in manifest map");
        assert!(err.contains("duplicate workspace package name in manifest map"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coverage_refresh_parsing_and_summary_errors_are_reported() {
        let root = temp_root("coverage_refresh_errors");
        let coverage_dir = root.join("target").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nbad-row\n",
        );
        let bad_row = load_coverage_refresh_rows(&root).expect_err("invalid coverage row");
        assert!(bad_row.contains("at least 6 columns"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-a\tpass\tnot-a-number\t100\t100\t100\tfile\n",
        );
        let bad_percent = load_coverage_refresh_rows(&root).expect_err("invalid coverage percent");
        assert!(bad_percent.contains("parse exec"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-a\tpass\t100\t100\t100\tnot-a-number\tfile\n",
        );
        let bad_region =
            load_coverage_refresh_rows(&root).expect_err("invalid region coverage percent");
        assert!(bad_region.contains("parse region"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-a\tpass\t100\t100\t100\t100\tfile\nradroots-a\tpass\t100\t100\t100\t100\tfile\n",
        );
        let duplicate_row = load_coverage_refresh_rows(&root).expect_err("duplicate coverage row");
        assert!(duplicate_row.contains("duplicate coverage row"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-a\tfail\t100\t100\t100\t100\tfile\n",
        );
        let required = ["radroots-a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let non_pass = validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect_err("non-pass status");
        assert!(non_pass.contains("non-pass status"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-a\tpass\t99.9\t100\t100\t100\tfile\n",
        );
        let below_100 = validate_required_coverage_summary(&root, &required, strict_thresholds())
            .expect_err("coverage below 100");
        assert!(below_100.contains("must satisfy coverage policy"));

        let missing = ["missing".to_string()].into_iter().collect::<BTreeSet<_>>();
        let missing_err = validate_required_coverage_summary(&root, &missing, strict_thresholds())
            .expect_err("missing required row");
        assert!(missing_err.contains("missing from coverage-refresh.tsv"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn enum_extract_and_parse_error_paths_are_reported() {
        let missing = extract_enum_body("pub struct X;", "RadrootsCoreUnitDimension")
            .expect_err("missing enum");
        assert!(missing.contains("missing enum"));

        let missing_brace = extract_enum_body(
            "pub enum RadrootsCoreUnitDimension",
            "RadrootsCoreUnitDimension",
        )
        .expect_err("missing opening brace");
        assert!(missing_brace.contains("missing opening brace"));

        let missing_close = extract_enum_body(
            "pub enum RadrootsCoreUnitDimension { Count, Mass",
            "RadrootsCoreUnitDimension",
        )
        .expect_err("missing closing brace");
        assert!(missing_close.contains("missing closing brace"));

        let variants = parse_enum_variants(
            r#"
            ,
            = 1,
            // skip
            #![cfg(test)]
            Count,
            "#,
        );
        assert_eq!(variants, vec!["Count".to_string()]);

        let nested = extract_enum_body(
            "pub enum RadrootsCoreUnitDimension { Count = { 1 }, Mass = 2 }",
            "RadrootsCoreUnitDimension",
        )
        .expect("nested braces in enum body");
        assert!(nested.contains("Count"));
    }

    #[test]
    fn coverage_policy_parity_reports_contract_errors() {
        let root = create_synthetic_workspace("coverage_policy_errors");
        let contract_root = root.join("contract");

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = []
"#,
        );
        let empty_required =
            validate_coverage_policy_parity(&root, &contract_root).expect_err("empty required");
        assert!(empty_required.contains("required crates list must not be empty"));

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 99.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        let invalid_gate = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid policy thresholds");
        assert!(invalid_gate.contains("100/100/100/100"));

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 99.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        let invalid_functions = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid function threshold");
        assert!(invalid_functions.contains("100/100/100/100"));

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 99.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        let invalid_regions = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid region threshold");
        assert!(invalid_regions.contains("100/100/100/100"));

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 99.0
require_branches = true

[required]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        let invalid_branches = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid branch threshold");
        assert!(invalid_branches.contains("100/100/100/100"));

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots-a", "radroots-a"]
"#,
        );
        let duplicate_required = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("duplicate required crate");
        assert!(duplicate_required.contains("duplicate crate"));

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = false

[required]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        let branches_optional = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("branches must be required");
        assert!(branches_optional.contains("required branches"));

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots-a"]
"#,
        );
        let missing_workspace = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("missing workspace crate in policy");
        assert!(missing_workspace.contains("missing workspace crates"));

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["unknown"]
"#,
        );
        let required_unknown = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("unknown required crate");
        assert!(required_unknown.contains("includes unknown crates"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn release_publish_policy_reports_contract_errors() {
        let root = create_synthetic_workspace("release_policy_errors");
        let contract_root = root.join("contract");

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = ""

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a"]
"#,
        );
        let empty_version = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("empty release version");
        assert!(empty_version.contains("must not be empty"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "2.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a"]
"#,
        );
        let version_mismatch = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("release version mismatch");
        assert!(version_mismatch.contains("must match contract version"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-a"]

[publish_order]
crates = ["radroots-a"]
"#,
        );
        let overlap = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("publish/internal overlap");
        assert!(overlap.contains("overlap is not allowed"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = []

[publish_order]
crates = ["radroots-a"]
"#,
        );
        let missing_workspace = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("missing workspace crate");
        assert!(missing_workspace.contains("missing workspace crates"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = []
"#,
        );
        let missing_publish_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("missing publish order entries");
        assert!(missing_publish_order.contains("missing publish crates"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        let extra_publish_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("extra publish order entries");
        assert!(extra_publish_order.contains("non-publish crates"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots-a"
version = "0.1.0"
edition = "2024"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.example.com/a"
readme = "README.md"

[dependencies]
radroots-b = { path = "../b" }
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots-b"
version = "0.1.0"
edition = "2024"
description = "crate b"
repository = "https://example.com/b"
homepage = "https://example.com/b"
documentation = "https://docs.example.com/b"
readme = "README.md"
"#,
        );
        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a", "radroots-b"]

[internal]
crates = []

[publish_order]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        let dependency_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("dependency order violation");
        assert!(dependency_order.contains("must place dependency"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a"]
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots-b"
version = "0.1.0"
edition = "2024"
publish = false
"#,
        );
        validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect("internal dependency should be ignored in publish ordering");

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots-a"
version = "0.1.0"
edition = "2024"
publish = false
"#,
        );
        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a"]
"#,
        );
        let publish_flag = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("publish crate must be publishable");
        assert!(publish_flag.contains("must not set publish = false"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots-a"
version = "0.1.0"
edition = "2024"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.example.com/a"
readme = "README.md"
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots-b"
version = "0.1.0"
edition = "2024"
"#,
        );
        let internal_flag = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("internal crate must be non-publishable");
        assert!(internal_flag.contains("must set publish = false"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_contract_bundle_reports_required_field_errors() {
        let root = create_synthetic_workspace("contract_bundle_errors");

        let assert_bundle_error = |expected: &str, mutator: fn(&mut ContractBundle)| {
            let mut bundle = load_contract_bundle(&root).expect("load bundle");
            mutator(&mut bundle);
            let err = validate_contract_bundle(&bundle).expect_err("bundle validation error");
            assert!(err.contains(expected), "expected `{expected}` in `{err}`");
        };

        assert_bundle_error("contract name is required", |bundle| {
            bundle.manifest.contract.name.clear();
        });
        assert_bundle_error("contract version is required", |bundle| {
            bundle.manifest.contract.version.clear();
        });
        assert_bundle_error("contract source is required", |bundle| {
            bundle.manifest.contract.source.clear();
        });
        assert_bundle_error("surface.model_crates must not be empty", |bundle| {
            bundle.manifest.surface.model_crates.clear();
        });
        assert_bundle_error("surface.algorithm_crates must not be empty", |bundle| {
            bundle.manifest.surface.algorithm_crates.clear();
        });
        assert_bundle_error("surface.wasm_crates must not be empty", |bundle| {
            bundle.manifest.surface.wasm_crates.clear();
        });
        assert_bundle_error(
            "at least one language export mapping is required",
            |bundle| {
                bundle.exports.clear();
            },
        );
        assert_bundle_error("language.id is required", |bundle| {
            bundle.exports[0].language.id.clear();
        });
        assert_bundle_error("language.repository is required", |bundle| {
            bundle.exports[0].language.repository.clear();
        });
        assert_bundle_error("packages map is required", |bundle| {
            bundle.exports[0].packages.clear();
        });
        assert_bundle_error("artifacts fields must be non-empty for ts", |bundle| {
            bundle.exports[0]
                .artifacts
                .as_mut()
                .expect("ts artifacts")
                .models_dir = Some(String::new());
        });
        assert_bundle_error("artifacts fields must be non-empty for ts", |bundle| {
            bundle.exports[0]
                .artifacts
                .as_mut()
                .expect("ts artifacts")
                .constants_dir = Some(String::new());
        });
        assert_bundle_error("artifacts fields must be non-empty for ts", |bundle| {
            bundle.exports[0]
                .artifacts
                .as_mut()
                .expect("ts artifacts")
                .wasm_dist_dir = Some(String::new());
        });
        assert_bundle_error("artifacts fields must be non-empty for ts", |bundle| {
            bundle.exports[0]
                .artifacts
                .as_mut()
                .expect("ts artifacts")
                .manifest_file = Some(String::new());
        });
        assert_bundle_error("version.contract.version is required", |bundle| {
            bundle.version.contract.version.clear();
        });
        assert_bundle_error("version.contract.stability is required", |bundle| {
            bundle.version.contract.stability.clear();
        });
        assert_bundle_error("version.semver rules must all be non-empty", |bundle| {
            bundle.version.semver.major_on.clear();
        });
        assert_bundle_error("version.semver rules must all be non-empty", |bundle| {
            bundle.version.semver.minor_on.clear();
        });
        assert_bundle_error("version.semver rules must all be non-empty", |bundle| {
            bundle.version.semver.patch_on.clear();
        });
        assert_bundle_error(
            "compatibility.requires_conformance_pass must be true",
            |bundle| {
                bundle.version.compatibility.requires_conformance_pass = false;
            },
        );
        assert_bundle_error(
            "compatibility.requires_export_manifest_diff must be true",
            |bundle| {
                bundle.version.compatibility.requires_export_manifest_diff = false;
            },
        );
        assert_bundle_error(
            "compatibility.requires_release_notes must be true",
            |bundle| {
                bundle.version.compatibility.requires_release_notes = false;
            },
        );
        assert_bundle_error("contract policy flags must all be true", |bundle| {
            bundle.manifest.policy.exclude_internal_workspace_crates = false;
        });
        assert_bundle_error("contract policy flags must all be true", |bundle| {
            bundle.manifest.policy.require_reproducible_exports = false;
        });
        assert_bundle_error("contract policy flags must all be true", |bundle| {
            bundle.manifest.policy.require_conformance_vectors = false;
        });

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parse_toml_and_publish_flags_report_failures() {
        let missing = temp_root("parse_toml_missing");
        let read_err =
            parse_toml::<WorkspaceCargoManifest>(&missing.join("Cargo.toml")).expect_err("missing");
        assert!(read_err.contains("read"));
        let _ = fs::remove_dir_all(&missing);

        let invalid = temp_root("parse_toml_invalid");
        write_file(&invalid.join("Cargo.toml"), "[workspace]\nmembers = [");
        let parse_err = parse_toml::<WorkspaceCargoManifest>(&invalid.join("Cargo.toml"))
            .expect_err("invalid manifest");
        assert!(parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&invalid);

        let contract_manifest_missing = temp_root("parse_contract_manifest_missing");
        let contract_manifest_read_err =
            parse_toml::<ContractManifest>(&contract_manifest_missing.join("manifest.toml"))
                .expect_err("missing contract manifest");
        assert!(contract_manifest_read_err.contains("read"));
        let _ = fs::remove_dir_all(&contract_manifest_missing);

        let contract_manifest_invalid = temp_root("parse_contract_manifest_invalid");
        write_file(
            &contract_manifest_invalid.join("manifest.toml"),
            "[contract",
        );
        let contract_manifest_parse_err =
            parse_toml::<ContractManifest>(&contract_manifest_invalid.join("manifest.toml"))
                .expect_err("invalid contract manifest");
        assert!(contract_manifest_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&contract_manifest_invalid);

        let version_missing = temp_root("parse_version_policy_missing");
        let version_read_err = parse_toml::<VersionPolicy>(&version_missing.join("version.toml"))
            .expect_err("missing version policy");
        assert!(version_read_err.contains("read"));
        let _ = fs::remove_dir_all(&version_missing);

        let version_invalid = temp_root("parse_version_policy_invalid");
        write_file(&version_invalid.join("version.toml"), "[version");
        let version_parse_err = parse_toml::<VersionPolicy>(&version_invalid.join("version.toml"))
            .expect_err("invalid version policy");
        assert!(version_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&version_invalid);

        let release_missing = temp_root("parse_release_contract_missing");
        let release_read_err =
            parse_toml::<ReleaseContractFile>(&release_missing.join("publish-set.toml"))
                .expect_err("missing release contract");
        assert!(release_read_err.contains("read"));
        let _ = fs::remove_dir_all(&release_missing);

        let release_invalid = temp_root("parse_release_contract_invalid");
        write_file(&release_invalid.join("publish-set.toml"), "[release");
        let release_parse_err =
            parse_toml::<ReleaseContractFile>(&release_invalid.join("publish-set.toml"))
                .expect_err("invalid release contract");
        assert!(release_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&release_invalid);

        let export_missing = temp_root("parse_export_mapping_missing");
        let export_read_err = parse_toml::<ExportMapping>(&export_missing.join("model.toml"))
            .expect_err("missing export mapping");
        assert!(export_read_err.contains("read"));
        let _ = fs::remove_dir_all(&export_missing);

        let export_invalid = temp_root("parse_export_mapping_invalid");
        write_file(&export_invalid.join("model.toml"), "[export");
        let export_parse_err = parse_toml::<ExportMapping>(&export_invalid.join("model.toml"))
            .expect_err("invalid export mapping");
        assert!(export_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&export_invalid);

        let dup = temp_root("publish_flags_duplicate");
        write_file(
            &dup.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
"#,
        );
        let member_manifest =
            "[package]\nname = \"duplicate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
        write_file(
            &dup.join("crates").join("a").join("Cargo.toml"),
            member_manifest,
        );
        write_file(
            &dup.join("crates").join("b").join("Cargo.toml"),
            member_manifest,
        );
        let dup_err = workspace_package_publish_flags(&dup).expect_err("duplicate publish flags");
        assert!(dup_err.contains("duplicate workspace package name"));
        let _ = fs::remove_dir_all(&dup);
    }

    #[test]
    fn workspace_package_records_and_callers_report_member_manifest_errors() {
        let root = temp_root("workspace_package_record_errors");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a"]
"#,
        );

        let read_err =
            workspace_package_records(&root).expect_err("missing member manifest should fail");
        assert!(read_err.contains("read"));

        let names_err = workspace_package_names(&root).expect_err("names should fail");
        assert!(names_err.contains("read"));
        let manifests_err = workspace_package_manifests(&root).expect_err("manifests should fail");
        assert!(manifests_err.contains("read"));
        let flags_err = workspace_package_publish_flags(&root).expect_err("flags should fail");
        assert!(flags_err.contains("read"));
        let deps_err = read_workspace_package_dependencies(&root).expect_err("deps should fail");
        assert!(deps_err.contains("read"));

        let publish = ["radroots-a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let publish_err =
            validate_publish_package_metadata(&root, &publish).expect_err("publish metadata");
        assert!(publish_err.contains("read"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            "[package",
        );
        let parse_value_err =
            workspace_package_records(&root).expect_err("invalid toml should fail");
        assert!(parse_value_err.contains("parse"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[workspace]
resolver = "2"
"#,
        );
        let parse_package_err =
            workspace_package_records(&root).expect_err("missing package table should fail");
        assert!(parse_package_err.contains("parse"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_package_manifests_success_and_publish_metadata_duplicate_names() {
        let root = create_synthetic_workspace("workspace_manifest_success");
        let manifests = workspace_package_manifests(&root).expect("workspace manifests");
        assert_eq!(manifests.len(), 2);
        assert!(manifests.contains_key("radroots-a"));
        assert!(manifests.contains_key("radroots-b"));

        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots-a"
version = "0.1.0"
edition = "2024"
description = "crate b duplicate name"
repository = "https://example.com/b"
homepage = "https://example.com/b"
documentation = "https://docs.example.com/b"
readme = "README.md"
publish = false
"#,
        );
        let publish = ["radroots-a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let duplicate_err =
            validate_publish_package_metadata(&root, &publish).expect_err("duplicate package map");
        assert!(duplicate_err.contains("duplicate workspace package name"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn coverage_release_and_bundle_loaders_report_parse_and_read_errors() {
        let root = create_synthetic_workspace("coverage_release_loader_errors");
        let contract_root = root.join("contract");

        let missing_workspace = temp_root("coverage_missing_workspace_manifest");
        let policy_workspace_err =
            validate_coverage_policy_parity(&missing_workspace, &contract_root)
                .expect_err("coverage workspace manifest read error");
        assert!(policy_workspace_err.contains("Cargo.toml"));
        let _ = fs::remove_dir_all(&missing_workspace);

        let _ = fs::remove_file(contract_root.join("coverage").join("policy.toml"));
        let policy_load_err = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("coverage policy read error");
        assert!(policy_load_err.contains("policy.toml"));
        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots-a", "radroots-b"]
"#,
        );

        let missing_release = temp_root("release_missing_workspace_manifest");
        let release_workspace_err =
            validate_release_publish_policy(&missing_release, &contract_root, "1.0.0")
                .expect_err("release workspace read error");
        assert!(release_workspace_err.contains("Cargo.toml"));
        let _ = fs::remove_dir_all(&missing_release);

        let _ = fs::remove_file(contract_root.join("release").join("publish-set.toml"));
        let release_load_err = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("release contract read error");
        assert!(release_load_err.contains("publish-set.toml"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a", "radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a"]
"#,
        );
        let duplicate_publish = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("duplicate publish crates");
        assert!(duplicate_publish.contains("publish.crates has duplicate crate"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b", "radroots-b"]

[publish_order]
crates = ["radroots-a"]
"#,
        );
        let duplicate_internal = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("duplicate internal crates");
        assert!(duplicate_internal.contains("internal.crates has duplicate crate"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a", "radroots-a"]
"#,
        );
        let duplicate_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("duplicate publish order");
        assert!(duplicate_order.contains("publish_order.crates has duplicate crate"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a"]
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            "[package",
        );
        let dependency_err = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("workspace dependency parse error");
        assert!(dependency_err.contains("parse"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_release_preflight_reports_each_stage_error() {
        let missing_contract_root = temp_root("preflight_missing_contract");
        let missing_contract_err =
            validate_release_preflight(&missing_contract_root).expect_err("missing contract");
        assert!(missing_contract_err.contains("manifest.toml"));
        let _ = fs::remove_dir_all(&missing_contract_root);

        let invalid_bundle = create_synthetic_workspace("preflight_invalid_bundle");
        write_file(
            &invalid_bundle.join("contract").join("manifest.toml"),
            r#"[contract]
name = "radroots-contract"
version = "1.0.0"
source = "synthetic"

[surface]
model_crates = ["radroots-a"]
algorithm_crates = ["radroots-b"]
wasm_crates = ["radroots-a-wasm"]

[policy]
exclude_internal_workspace_crates = false
require_reproducible_exports = true
require_conformance_vectors = true
"#,
        );
        let invalid_bundle_err =
            validate_release_preflight(&invalid_bundle).expect_err("bundle validation");
        assert!(invalid_bundle_err.contains("contract policy flags must all be true"));
        let _ = fs::remove_dir_all(&invalid_bundle);

        let missing_release = create_synthetic_workspace("preflight_missing_release");
        let _ = fs::remove_file(
            missing_release
                .join("contract")
                .join("release")
                .join("publish-set.toml"),
        );
        let missing_release_err =
            validate_release_preflight(&missing_release).expect_err("missing release");
        assert!(missing_release_err.contains("publish-set.toml"));
        let _ = fs::remove_dir_all(&missing_release);

        let missing_required = create_synthetic_workspace("preflight_missing_required");
        let _ = fs::remove_file(
            missing_required
                .join("contract")
                .join("coverage")
                .join("policy.toml"),
        );
        let missing_required_err =
            validate_release_preflight(&missing_required).expect_err("missing required list");
        assert!(missing_required_err.contains("policy.toml"));
        let _ = fs::remove_dir_all(&missing_required);

        let duplicate_publish = create_synthetic_workspace("preflight_duplicate_publish");
        write_file(
            &duplicate_publish
                .join("contract")
                .join("release")
                .join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a", "radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a"]
"#,
        );
        let duplicate_publish_err =
            validate_release_preflight(&duplicate_publish).expect_err("duplicate publish crates");
        assert!(duplicate_publish_err.contains("publish.crates has duplicate crate"));
        let _ = fs::remove_dir_all(&duplicate_publish);

        let duplicate_required = create_synthetic_workspace("preflight_duplicate_required");
        write_file(
            &duplicate_required
                .join("contract")
                .join("coverage")
                .join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots-a\", \"radroots-a\"]\n",
        );
        let duplicate_required_err =
            validate_release_preflight(&duplicate_required).expect_err("duplicate required crates");
        assert!(duplicate_required_err.contains("duplicate crate"));
        let _ = fs::remove_dir_all(&duplicate_required);

        let publish_metadata = create_synthetic_workspace("preflight_publish_metadata");
        write_file(
            &publish_metadata.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots-a"
version = "0.1.0"
edition = "2024"
"#,
        );
        let publish_metadata_err =
            validate_release_preflight(&publish_metadata).expect_err("publish metadata validation");
        assert!(publish_metadata_err.contains("must define a non-empty package.description"));
        let _ = fs::remove_dir_all(&publish_metadata);

        let missing_coverage_row = create_synthetic_workspace("preflight_missing_coverage_row");
        write_file(
            &missing_coverage_row
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\n",
        );
        let missing_coverage_row_err = validate_release_preflight(&missing_coverage_row)
            .expect_err("required coverage refresh row missing");
        assert!(missing_coverage_row_err.contains("missing from coverage-refresh.tsv"));
        let _ = fs::remove_dir_all(&missing_coverage_row);
    }

    #[test]
    fn load_contract_bundle_and_validation_report_version_export_and_coverage_errors() {
        let root = create_synthetic_workspace("bundle_version_export_and_coverage_errors");
        write_file(&root.join("contract").join("version.toml"), "[contract");
        let version_parse_err = load_contract_bundle(&root).expect_err("invalid version file");
        assert!(version_parse_err.contains("version.toml"));

        write_file(
            &root.join("contract").join("version.toml"),
            r#"[contract]
version = "1.0.0"
stability = "alpha"

[semver]
major_on = ["breaking"]
minor_on = ["feature"]
patch_on = ["fix"]

[compatibility]
requires_conformance_pass = true
requires_export_manifest_diff = true
requires_release_notes = true
"#,
        );
        write_file(
            &root.join("contract").join("exports").join("ts.toml"),
            "[language",
        );
        let export_parse_err = load_contract_bundle(&root).expect_err("invalid export mapping");
        assert!(export_parse_err.contains("ts.toml"));

        write_file(
            &root.join("contract").join("exports").join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        let bundle = load_contract_bundle(&root).expect("load bundle");
        write_file(
            &root.join("crates").join("core").join("src").join("unit.rs"),
            r#"pub enum RadrootsCoreUnitDimension {
Mass,
Count,
Volume,
}
"#,
        );
        let core_err = validate_contract_bundle(&bundle).expect_err("core unit mismatch");
        assert!(core_err.contains("variant order must be"));

        write_file(
            &root.join("crates").join("core").join("src").join("unit.rs"),
            r#"pub enum RadrootsCoreUnitDimension {
Count,
Mass,
Volume,
}
"#,
        );
        write_file(
            &root.join("contract").join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = false

[required]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        let policy_err = validate_contract_bundle(&bundle).expect_err("coverage policy validation");
        assert!(policy_err.contains("100/100/100/100"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn coverage_summary_and_core_enum_additional_error_paths() {
        let coverage_root = temp_root("coverage_summary_additional_errors");
        write_file(
            &coverage_root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-a\tpass\t100\tbad\t100\t100\tfile\n",
        );
        let func_err = load_coverage_refresh_rows(&coverage_root).expect_err("func parse error");
        assert!(func_err.contains("parse func"));
        write_file(
            &coverage_root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots-a\tpass\t100\t100\tbad\t100\tfile\n",
        );
        let branch_err =
            load_coverage_refresh_rows(&coverage_root).expect_err("branch parse error");
        assert!(branch_err.contains("parse branch"));
        let _ = fs::remove_dir_all(&coverage_root);

        let missing_refresh_root = temp_root("coverage_summary_missing_refresh");
        let required = ["radroots-a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let missing_refresh_err = validate_required_coverage_summary(
            &missing_refresh_root,
            &required,
            strict_thresholds(),
        )
        .expect_err("missing refresh should fail");
        assert!(missing_refresh_err.contains("coverage-refresh.tsv"));
        let _ = fs::remove_dir_all(&missing_refresh_root);

        let enum_root = temp_root("core_unit_missing_enum");
        write_file(
            &enum_root
                .join("crates")
                .join("core")
                .join("src")
                .join("unit.rs"),
            "pub struct NotTheEnum;",
        );
        let enum_err =
            validate_core_unit_dimension_variant_order(&enum_root).expect_err("missing enum");
        assert!(enum_err.contains("missing enum"));
        let _ = fs::remove_dir_all(&enum_root);
    }

    #[test]
    fn publish_metadata_and_coverage_refresh_report_missing_paths() {
        let root = temp_root("publish_missing_manifest");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a"]
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots-a"
version = "0.1.0"
edition = "2024"
description = "crate a"
repository = { workspace = true }
homepage = { workspace = true }
readme = { workspace = true }
"#,
        );
        let missing_manifest = ["radroots-b".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let missing_err = validate_publish_package_metadata(&root, &missing_manifest)
            .expect_err("missing workspace manifest");
        assert!(missing_err.contains("has no workspace manifest"));

        let missing_field = ["radroots-a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let field_err = validate_publish_package_metadata(&root, &missing_field)
            .expect_err("missing configured field");
        assert!(field_err.contains("must configure package.documentation"));

        let refresh_missing =
            load_coverage_refresh_rows(&root).expect_err("missing coverage-refresh.tsv");
        assert!(refresh_missing.contains("coverage-refresh.tsv"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn coverage_refresh_parser_skips_blank_lines() {
        let root = temp_root("coverage_refresh_blank_lines");
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\n\nradroots-a\tpass\t100\t100\t100\t100\tfile\n",
        );
        let rows = load_coverage_refresh_rows(&root).expect("rows");
        assert_eq!(rows.len(), 1);
        assert!(rows.contains_key("radroots-a"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn core_unit_dimension_validation_reports_missing_and_mismatch() {
        let missing = temp_root("core_unit_missing");
        let missing_err = validate_core_unit_dimension_variant_order(&missing)
            .expect_err("missing unit file should fail");
        assert!(missing_err.contains("unit.rs"));
        let _ = fs::remove_dir_all(&missing);

        let mismatch = temp_root("core_unit_mismatch");
        write_file(
            &mismatch
                .join("crates")
                .join("core")
                .join("src")
                .join("unit.rs"),
            r#"pub enum RadrootsCoreUnitDimension {
Mass,
Count,
Volume,
}
"#,
        );
        let mismatch_err = validate_core_unit_dimension_variant_order(&mismatch)
            .expect_err("mismatched enum order should fail");
        assert!(mismatch_err.contains("variant order must be"));
        let _ = fs::remove_dir_all(&mismatch);
    }

    #[test]
    fn coverage_and_release_additional_error_branches_are_reported() {
        let root = create_synthetic_workspace("coverage_release_extra_errors");
        let contract_root = root.join("contract");

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots-a", "radroots-b", "radroots-extra"]
"#,
        );
        let coverage_extra = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("coverage unknown crate");
        assert!(coverage_extra.contains("includes unknown crates"));

        write_file(
            &contract_root.join("coverage").join("policy.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots-a"]
"#,
        );
        let required_list_mismatch = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("required list must match workspace crates");
        assert!(required_list_mismatch.contains("missing workspace crates"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a", "radroots-b", "radroots-extra"]

[internal]
crates = []

[publish_order]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        let release_extra = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("release extra crate");
        assert!(release_extra.contains("include unknown crates"));

        write_file(
            &contract_root.join("release").join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = ["radroots-a", "radroots-b"]
"#,
        );
        let publish_order_extra = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("publish order non-publish crate");
        assert!(publish_order_extra.contains("non-publish crates"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_contract_bundle_reports_exports_dir_errors_and_skips_non_toml() {
        let root = create_synthetic_workspace("bundle_exports_dir_errors");
        let exports_dir = root.join("contract").join("exports");
        let _ = fs::remove_dir_all(&exports_dir);
        write_file(&exports_dir, "not-a-dir");

        let dir_err = load_contract_bundle(&root).expect_err("exports path must be a directory");
        assert!(dir_err.contains("read dir"));

        let _ = fs::remove_file(&exports_dir);
        fs::create_dir_all(&exports_dir).expect("recreate exports dir");
        write_file(
            &exports_dir.join("typescript.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
radroots-a = "@radroots/a"
radroots-b = "@radroots/b"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_file(&exports_dir.join("README.txt"), "ignore");
        let bundle = load_contract_bundle(&root).expect("load bundle");
        assert_eq!(bundle.exports.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_contract_bundle_reports_ts_artifacts_and_release_policy_errors() {
        let missing_artifacts = create_synthetic_workspace("bundle_missing_ts_artifacts");
        let mut no_artifacts_bundle =
            load_contract_bundle(&missing_artifacts).expect("load missing artifacts bundle");
        no_artifacts_bundle.exports[0].artifacts = None;
        let artifacts_err =
            validate_contract_bundle(&no_artifacts_bundle).expect_err("missing ts artifacts");
        assert!(artifacts_err.contains("artifacts map is required for ts"));
        let _ = fs::remove_dir_all(&missing_artifacts);

        let release_error_root = create_synthetic_workspace("bundle_release_policy_error");
        write_file(
            &release_error_root
                .join("contract")
                .join("release")
                .join("publish-set.toml"),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots-a"]

[internal]
crates = ["radroots-b"]

[publish_order]
crates = []
"#,
        );
        let bundle = load_contract_bundle(&release_error_root).expect("load release error bundle");
        let release_err = validate_contract_bundle(&bundle).expect_err("release policy failure");
        assert!(release_err.contains("publish_order.crates is missing publish crates"));
        let _ = fs::remove_dir_all(&release_error_root);
    }
}

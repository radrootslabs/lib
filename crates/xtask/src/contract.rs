#![forbid(unsafe_code)]

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

#[derive(Debug, Deserialize)]
struct CoverageRolloutFile {
    rollout: CoverageRolloutSection,
}

#[derive(Debug, Deserialize)]
struct CoverageRolloutSection {
    crates: Vec<CoverageRolloutCrate>,
}

#[derive(Debug, Deserialize)]
struct CoverageRolloutCrate {
    name: String,
    status: String,
    order: u32,
}

#[derive(Debug, Deserialize)]
struct CoverageRequiredFile {
    required: CoverageRequiredSection,
}

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
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    toml::from_str::<T>(&raw).map_err(|e| format!("parse {}: {e}", path.display()))
}

fn contract_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join("contract")
}

fn workspace_package_names(workspace_root: &Path) -> Result<Vec<String>, String> {
    let workspace_manifest =
        parse_toml::<WorkspaceCargoManifest>(&workspace_root.join("Cargo.toml"))?;
    let mut names = Vec::with_capacity(workspace_manifest.workspace.members.len());
    for member in workspace_manifest.workspace.members {
        let member_manifest = workspace_root.join(member).join("Cargo.toml");
        let package_manifest = parse_toml::<PackageCargoManifest>(&member_manifest)?;
        names.push(package_manifest.package.name);
    }
    Ok(names)
}

fn workspace_package_manifests(workspace_root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let workspace_manifest =
        parse_toml::<WorkspaceCargoManifest>(&workspace_root.join("Cargo.toml"))?;
    let mut manifests = BTreeMap::new();
    for member in workspace_manifest.workspace.members {
        let member_manifest = workspace_root.join(&member).join("Cargo.toml");
        let package_manifest = parse_toml::<PackageCargoManifest>(&member_manifest)?;
        if manifests
            .insert(package_manifest.package.name, member_manifest)
            .is_some()
        {
            return Err("duplicate workspace package name in manifest map".to_string());
        }
    }
    Ok(manifests)
}

fn load_coverage_rollout(contract_root: &Path) -> Result<CoverageRolloutFile, String> {
    parse_toml::<CoverageRolloutFile>(&contract_root.join("coverage").join("rollout.toml"))
}

fn load_coverage_required(contract_root: &Path) -> Result<CoverageRequiredFile, String> {
    parse_toml::<CoverageRequiredFile>(&contract_root.join("coverage").join("required-crates.toml"))
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
    let workspace_manifest =
        parse_toml::<WorkspaceCargoManifest>(&workspace_root.join("Cargo.toml"))?;
    let mut flags = BTreeMap::new();
    for member in workspace_manifest.workspace.members {
        let member_manifest = workspace_root.join(member).join("Cargo.toml");
        let package_manifest = parse_toml::<PackageCargoManifest>(&member_manifest)?;
        if flags
            .insert(
                package_manifest.package.name.clone(),
                package_publish_enabled(package_manifest.package.publish.as_ref()),
            )
            .is_some()
        {
            return Err(format!(
                "duplicate workspace package name {}",
                package_manifest.package.name
            ));
        }
    }
    Ok(flags)
}

fn read_workspace_package_dependencies(
    workspace_root: &Path,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let workspace_manifest =
        parse_toml::<WorkspaceCargoManifest>(&workspace_root.join("Cargo.toml"))?;
    let mut member_manifests = Vec::with_capacity(workspace_manifest.workspace.members.len());
    let mut workspace_names = BTreeSet::new();
    for member in workspace_manifest.workspace.members {
        let member_manifest = workspace_root.join(&member).join("Cargo.toml");
        let package_manifest = parse_toml::<PackageCargoManifest>(&member_manifest)?;
        workspace_names.insert(package_manifest.package.name.clone());
        member_manifests.push((package_manifest.package.name, member_manifest));
    }

    let mut deps = BTreeMap::new();
    for (package_name, manifest_path) in member_manifests {
        let parsed = parse_toml::<toml::Value>(&manifest_path)?;
        let mut package_deps = BTreeSet::new();
        for section in ["dependencies", "build-dependencies"] {
            let Some(table) = parsed.get(section).and_then(toml::Value::as_table) else {
                continue;
            };
            for dep_name in table.keys() {
                if workspace_names.contains(dep_name) {
                    package_deps.insert(dep_name.clone());
                }
            }
        }
        deps.insert(package_name, package_deps);
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
    let manifests = workspace_package_manifests(workspace_root)?;
    for crate_name in publish_crates {
        let manifest_path = manifests
            .get(crate_name)
            .ok_or_else(|| format!("publish crate {} has no workspace manifest", crate_name))?;
        let parsed = parse_toml::<toml::Value>(manifest_path)?;
        let package = parsed
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("{} missing [package] table", manifest_path.display()))?;

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
    raw.parse::<f64>()
        .map_err(|e| format!("parse {} for {}: {e}", field, crate_name))
}

fn load_coverage_refresh_rows(
    workspace_root: &Path,
) -> Result<BTreeMap<String, (String, f64, f64, f64)>, String> {
    let report_path = workspace_root
        .join("target")
        .join("coverage")
        .join("coverage-refresh.tsv");
    let raw = fs::read_to_string(&report_path)
        .map_err(|e| format!("read {}: {e}", report_path.display()))?;
    let mut rows = BTreeMap::new();
    for line in raw.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts = trimmed.split('\t').collect::<Vec<_>>();
        if parts.len() < 5 {
            return Err(format!(
                "coverage row must have at least 5 columns in {}: {}",
                report_path.display(),
                trimmed
            ));
        }
        let crate_name = parts[0].to_string();
        let status = parts[1].to_string();
        let exec = parse_coverage_percent(parts[2], "exec", &crate_name)?;
        let func = parse_coverage_percent(parts[3], "func", &crate_name)?;
        let branch = parse_coverage_percent(parts[4], "branch", &crate_name)?;
        rows.insert(crate_name, (status, exec, func, branch));
    }
    Ok(rows)
}

fn validate_required_coverage_summary(
    workspace_root: &Path,
    required_crates: &BTreeSet<String>,
) -> Result<(), String> {
    let rows = load_coverage_refresh_rows(workspace_root)?;
    for crate_name in required_crates {
        let (status, exec, func, branch) = rows.get(crate_name).ok_or_else(|| {
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
        if *exec < 100.0 || *func < 100.0 || *branch < 100.0 {
            return Err(format!(
                "required coverage crate {} must be 100/100/100, found {}/{}/{}",
                crate_name, exec, func, branch
            ));
        }
    }
    Ok(())
}

const CORE_UNIT_DIMENSION_ENUM: &str = "RadrootsCoreUnitDimension";
const CORE_UNIT_DIMENSION_ORDER: [&str; 3] = ["Count", "Mass", "Volume"];

fn extract_enum_body<'a>(source: &'a str, enum_name: &str) -> Result<&'a str, String> {
    let marker = format!("pub enum {enum_name}");
    let enum_start = source
        .find(&marker)
        .ok_or_else(|| format!("missing enum {enum_name}"))?;
    let after_start = &source[enum_start..];
    let open_rel = after_start
        .find('{')
        .ok_or_else(|| format!("missing opening brace for enum {enum_name}"))?;
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
            if ident.is_empty() {
                return None;
            }
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
    let source = fs::read_to_string(&source_path)
        .map_err(|e| format!("read {}: {e}", source_path.display()))?;
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

fn validate_coverage_rollout_parity(
    workspace_root: &Path,
    contract_root: &Path,
) -> Result<(), String> {
    let workspace_packages = workspace_package_names(workspace_root)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let rollout = load_coverage_rollout(contract_root)?;
    if rollout.rollout.crates.is_empty() {
        return Err("coverage rollout crates list must not be empty".to_string());
    }
    let mut rollout_packages = BTreeSet::new();
    let mut rollout_status = BTreeMap::new();
    let mut orders = Vec::with_capacity(rollout.rollout.crates.len());
    for entry in &rollout.rollout.crates {
        if !matches!(entry.status.as_str(), "required" | "planned") {
            return Err(format!(
                "coverage rollout status must be required or planned for {}",
                entry.name
            ));
        }
        if !rollout_packages.insert(entry.name.clone()) {
            return Err(format!("duplicate coverage rollout crate {}", entry.name));
        }
        rollout_status.insert(entry.name.clone(), entry.status.clone());
        orders.push(entry.order);
    }
    let mut sorted_orders = orders;
    sorted_orders.sort_unstable();
    for (index, order) in sorted_orders.iter().enumerate() {
        let expected = (index + 1) as u32;
        if *order != expected {
            return Err(format!(
                "coverage rollout order must be contiguous from 1; expected {expected} but found {order}"
            ));
        }
    }

    if workspace_packages != rollout_packages {
        let missing = workspace_packages
            .difference(&rollout_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = rollout_packages
            .difference(&workspace_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "coverage rollout missing workspace crates: {}",
                join_set(&missing)
            ));
        }
        if !extra.is_empty() {
            return Err(format!(
                "coverage rollout includes unknown crates: {}",
                join_set(&extra)
            ));
        }
    }

    let required = load_coverage_required(contract_root)?;
    if required.required.crates.is_empty() {
        return Err("coverage required crates list must not be empty".to_string());
    }
    let mut required_set = BTreeSet::new();
    for crate_name in &required.required.crates {
        if !required_set.insert(crate_name.clone()) {
            return Err(format!("duplicate coverage required crate {}", crate_name));
        }
        if !workspace_packages.contains(crate_name) {
            return Err(format!(
                "coverage required crate is not a workspace crate: {}",
                crate_name
            ));
        }
        match rollout_status.get(crate_name) {
            Some(status) if status == "required" => {}
            Some(status) => {
                return Err(format!(
                    "coverage required crate {} must have rollout status required, found {}",
                    crate_name, status
                ));
            }
            None => {
                return Err(format!(
                    "coverage required crate {} missing from rollout",
                    crate_name
                ));
            }
        }
    }

    let rollout_required = rollout_status
        .iter()
        .filter(|(_, status)| *status == "required")
        .map(|(name, _)| name.clone())
        .collect::<BTreeSet<_>>();
    if rollout_required != required_set {
        let missing = rollout_required
            .difference(&required_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = required_set
            .difference(&rollout_required)
            .cloned()
            .collect::<BTreeSet<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "coverage required list missing rollout required crates: {}",
                join_set(&missing)
            ));
        }
        if !extra.is_empty() {
            return Err(format!(
                "coverage required list has crates without rollout required status: {}",
                join_set(&extra)
            ));
        }
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
        if !missing.is_empty() {
            return Err(format!(
                "release publish/internal sets are missing workspace crates: {}",
                join_set(&missing)
            ));
        }
        if !extra.is_empty() {
            return Err(format!(
                "release publish/internal sets include unknown crates: {}",
                join_set(&extra)
            ));
        }
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
        if !missing.is_empty() {
            return Err(format!(
                "publish_order.crates is missing publish crates: {}",
                join_set(&missing)
            ));
        }
        if !extra.is_empty() {
            return Err(format!(
                "publish_order.crates has non-publish crates: {}",
                join_set(&extra)
            ));
        }
    }

    let order_index = publish_order
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx))
        .collect::<BTreeMap<_, _>>();
    let dependencies = read_workspace_package_dependencies(workspace_root)?;
    for crate_name in &publish_set {
        let crate_deps = dependencies
            .get(crate_name)
            .ok_or_else(|| format!("missing dependency graph entry for {}", crate_name))?;
        let crate_order = *order_index
            .get(crate_name)
            .ok_or_else(|| format!("missing publish order entry for {}", crate_name))?;
        for dep in crate_deps {
            if !publish_set.contains(dep) {
                continue;
            }
            let dep_order = *order_index
                .get(dep)
                .ok_or_else(|| format!("missing publish order entry for {}", dep))?;
            if dep_order >= crate_order {
                return Err(format!(
                    "publish order must place dependency {} before {}",
                    dep, crate_name
                ));
            }
        }
    }

    let publish_flags = workspace_package_publish_flags(workspace_root)?;
    for crate_name in &publish_set {
        let flag = publish_flags
            .get(crate_name)
            .ok_or_else(|| format!("missing publish flag entry for {}", crate_name))?;
        if !*flag {
            return Err(format!(
                "publish crate {} must not set publish = false",
                crate_name
            ));
        }
    }
    for crate_name in &internal_set {
        let flag = publish_flags
            .get(crate_name)
            .ok_or_else(|| format!("missing publish flag entry for {}", crate_name))?;
        if *flag {
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
    let release = load_release_contract(&bundle.root)?;
    let required = load_coverage_required(&bundle.root)?;
    let publish_crates = collect_unique_set(&release.publish.crates, "publish.crates")?;
    let required_crates = collect_unique_set(&required.required.crates, "required.crates")?;
    validate_publish_package_metadata(workspace_root, &publish_crates)?;
    validate_required_coverage_summary(workspace_root, &required_crates)?;
    Ok(())
}

pub fn load_contract_bundle(workspace_root: &Path) -> Result<ContractBundle, String> {
    let root = contract_root(workspace_root);
    let manifest = parse_toml::<ContractManifest>(&root.join("manifest.toml"))?;
    let version = parse_toml::<VersionPolicy>(&root.join("version.toml"))?;
    let exports_dir = root.join("exports");
    let mut exports = Vec::new();
    let mut entries = fs::read_dir(&exports_dir)
        .map_err(|e| format!("read dir {}: {e}", exports_dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read dir entries {}: {e}", exports_dir.display()))?;
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
            let artifacts = mapping
                .artifacts
                .as_ref()
                .ok_or_else(|| "artifacts map is required for ts".to_string())?;
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
        .ok_or_else(|| "failed to resolve workspace root from contract root".to_string())?;
    validate_core_unit_dimension_variant_order(workspace_root)?;
    validate_coverage_rollout_parity(workspace_root, &bundle.root)?;
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
    use std::path::PathBuf;
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
    fn coverage_rollout_includes_workspace_crates() {
        let root = workspace_root();
        let workspace_names = workspace_package_names(&root)
            .expect("workspace crates")
            .into_iter()
            .collect::<BTreeSet<_>>();
        let rollout = load_coverage_rollout(&root.join("contract")).expect("coverage rollout");
        let rollout_names = rollout
            .rollout
            .crates
            .iter()
            .map(|entry| entry.name.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(workspace_names, rollout_names);
    }

    #[test]
    fn coverage_required_crates_match_rollout_required_status() {
        let root = workspace_root();
        let contract_root = root.join("contract");
        let required = load_coverage_required(&contract_root).expect("coverage required");
        let required_names = required
            .required
            .crates
            .into_iter()
            .collect::<BTreeSet<_>>();
        let rollout = load_coverage_rollout(&contract_root).expect("coverage rollout");
        let rollout_required = rollout
            .rollout
            .crates
            .iter()
            .filter(|entry| entry.status == "required")
            .map(|entry| entry.name.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(required_names, rollout_required);
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
            "crate\tstatus\texec\tfunc\tbranch\treport\nradroots-core\tpass\t100.0\t100.0\t100.0\tfile\n",
        )
        .expect("write coverage file");
        let required = ["radroots-core".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        validate_required_coverage_summary(&root, &required).expect("coverage summary");
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
}

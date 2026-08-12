use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

use serde::Deserialize;
use syn::{
    Expr, ExprLit, Fields, ForeignItem, ImplItem, Item, Lit, Meta, Path as SynPath, TraitItem,
    Type, UseTree, Visibility, visit::Visit,
};

const API_BOUNDARIES_RELATIVE: &str = "contracts/releases/api_boundaries.toml";
const API_DECISION_RELATIVE: &str =
    "contracts/architecture/decisions/public_api_leakage_migration_baseline.v1.toml";
const API_DECISION_ID: &str = "radroots.public_api_leakage_migration_baseline.v1";
const SPEC_ID: &str = "radroots.crates.release.v1";
const POLICY_SCHEMA_VERSION: u16 = 1;
const DECISION_SCHEMA_VERSION: u16 = 1;
const CURRENT_STEP: u16 = 313;
const HISTORICAL_EXCEPTION_IDS: [&str; 8] = [
    "RCRV1-API-001",
    "RCRV1-API-002",
    "RCRV1-API-003",
    "RCRV1-API-004",
    "RCRV1-API-005",
    "RCRV1-API-006",
    "RCRV1-API-007",
    "RCRV1-API-008",
];
const FORBIDDEN_EXPANSION: [&str; 8] = [
    "broader_aliases",
    "keyring",
    "new_items",
    "new_upstream_paths",
    "platform_specific_types",
    "reqwest",
    "sqlx",
    "tokio",
];
const REMOVAL_MILESTONES: [(&str, u16); 3] = [
    ("radroots_identity", 42),
    ("radroots_nostr", 124),
    ("radroots_nostr_connect", 140),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiBoundaryPolicy {
    schema_version: u16,
    spec_id: String,
    forbidden_public_paths: Vec<String>,
    package: Vec<ApiPackagePolicy>,
    #[serde(default)]
    exception: Vec<ApiException>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiPackagePolicy {
    name: String,
    allowed_public_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiException {
    id: String,
    package: String,
    source: String,
    forbidden_path: String,
    items: Vec<String>,
    observed_paths: Vec<String>,
    decision: String,
    removal_step: u16,
    rationale: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiLeakageDecision {
    schema_version: u16,
    decision_id: String,
    status: String,
    accepted_date: String,
    completed_step: u16,
    publication_authorized: bool,
    exception_ids: Vec<String>,
    active_exception_ids: Vec<String>,
    forbidden_expansion: Vec<String>,
    removal_milestone: Vec<ApiRemovalMilestone>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiRemovalMilestone {
    package: String,
    step: u16,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    manifest_path: String,
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    kind: Vec<String>,
    src_path: String,
}

struct ModuleUnit {
    module: Vec<String>,
    parent: Option<Vec<String>>,
    declared_public: bool,
    initially_effective: bool,
    source_relative: String,
    items: Vec<Item>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct ApiFinding {
    package: String,
    source: String,
    item: String,
    forbidden_path: String,
    observed_path: String,
}

#[derive(Debug)]
enum InternalExport {
    Module(Vec<String>),
    Item(Vec<String>, String),
    External,
}

pub(super) fn validate_policy_catalog(
    workspace_root: &Path,
    expected_packages: &BTreeSet<String>,
) -> Result<(), String> {
    let policy = load_policy(workspace_root)?;
    validate_policy(workspace_root, &policy, expected_packages)
}

pub(super) fn validate_public_api(workspace_root: &Path) -> Result<(), String> {
    let policy = load_policy(workspace_root)?;
    let expected_packages = policy
        .package
        .iter()
        .map(|package| package.name.clone())
        .collect::<BTreeSet<_>>();
    validate_policy(workspace_root, &policy, &expected_packages)?;
    let metadata = load_metadata(workspace_root)?;
    let findings = scan_workspace(workspace_root, &policy, &metadata)?;
    let unapproved = findings
        .into_iter()
        .filter(|finding| !exception_matches(&policy.exception, finding))
        .collect::<Vec<_>>();
    if unapproved.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "forbidden public implementation type leakage:\n{}",
            unapproved
                .iter()
                .map(format_finding)
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

fn load_policy(workspace_root: &Path) -> Result<ApiBoundaryPolicy, String> {
    let path = workspace_root.join(API_BOUNDARIES_RELATIVE);
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn validate_policy(
    workspace_root: &Path,
    policy: &ApiBoundaryPolicy,
    expected_packages: &BTreeSet<String>,
) -> Result<(), String> {
    if policy.schema_version != POLICY_SCHEMA_VERSION {
        return Err(format!(
            "{API_BOUNDARIES_RELATIVE} schema_version must be {POLICY_SCHEMA_VERSION}"
        ));
    }
    if policy.spec_id != SPEC_ID {
        return Err(format!(
            "{API_BOUNDARIES_RELATIVE} spec_id must be {SPEC_ID}"
        ));
    }
    let decision = load_and_validate_decision(workspace_root)?;

    let forbidden = sorted_unique(
        "forbidden_public_paths",
        &policy.forbidden_public_paths,
        false,
    )?;
    if forbidden.len() != policy.forbidden_public_paths.len() {
        return Err(format!(
            "{API_BOUNDARIES_RELATIVE} forbidden_public_paths must be unique"
        ));
    }
    for required in [
        "keyring",
        "nostr_sdk",
        "reqwest",
        "sqlx",
        "std::os",
        "tokio",
    ] {
        if !forbidden.contains(required) {
            return Err(format!(
                "{API_BOUNDARIES_RELATIVE} must forbid public {required} paths"
            ));
        }
    }

    let mut package_names = BTreeSet::new();
    for package in &policy.package {
        if !package_names.insert(package.name.as_str()) {
            return Err(format!(
                "{API_BOUNDARIES_RELATIVE} package {} is declared more than once",
                package.name
            ));
        }
        let allowed = sorted_unique(
            &format!("package {} allowed_public_paths", package.name),
            &package.allowed_public_paths,
            true,
        )?;
        for path in allowed {
            if !forbidden.contains(path) {
                return Err(format!(
                    "{API_BOUNDARIES_RELATIVE} package {} allows {path}, which is not forbidden globally",
                    package.name
                ));
            }
        }
    }
    let actual_packages = package_names
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if &actual_packages != expected_packages {
        return Err(package_set_mismatch(expected_packages, &actual_packages));
    }

    let package_policy = policy
        .package
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let mut exception_ids = BTreeSet::new();
    let mut exception_keys = BTreeSet::new();
    for exception in &policy.exception {
        if !exception_ids.insert(exception.id.as_str()) {
            return Err(format!(
                "{API_BOUNDARIES_RELATIVE} exception id {} is duplicated",
                exception.id
            ));
        }
        if !valid_exception_id(&exception.id) {
            return Err(format!(
                "{API_BOUNDARIES_RELATIVE} exception id {} must match RCRV1-API-NNN",
                exception.id
            ));
        }
        let package = package_policy
            .get(exception.package.as_str())
            .ok_or_else(|| {
                format!(
                    "{API_BOUNDARIES_RELATIVE} exception {} names unknown package {}",
                    exception.id, exception.package
                )
            })?;
        if !forbidden.contains(exception.forbidden_path.as_str()) {
            return Err(format!(
                "{API_BOUNDARIES_RELATIVE} exception {} names unknown forbidden path {}",
                exception.id, exception.forbidden_path
            ));
        }
        if package
            .allowed_public_paths
            .iter()
            .any(|allowed| allowed == &exception.forbidden_path)
        {
            return Err(format!(
                "{API_BOUNDARIES_RELATIVE} exception {} is redundant with package allowance {}",
                exception.id, exception.forbidden_path
            ));
        }
        validate_relative_source(&exception.id, &exception.source)?;
        let items = sorted_unique(
            &format!("exception {} items", exception.id),
            &exception.items,
            false,
        )?;
        if items.len() != exception.items.len() {
            return Err(format!(
                "{API_BOUNDARIES_RELATIVE} exception {} items must be unique",
                exception.id
            ));
        }
        let observed_paths = sorted_unique(
            &format!("exception {} observed_paths", exception.id),
            &exception.observed_paths,
            false,
        )?;
        if observed_paths.len() != exception.observed_paths.len()
            || observed_paths.iter().any(|path| {
                !(path == &exception.forbidden_path
                    || path
                        .strip_prefix(&exception.forbidden_path)
                        .is_some_and(|suffix| suffix.starts_with("::")))
            })
        {
            return Err(format!(
                "{API_BOUNDARIES_RELATIVE} exception {} observed_paths must be sorted, unique, and rooted at {}",
                exception.id, exception.forbidden_path
            ));
        }
        if exception.removal_step <= CURRENT_STEP {
            return Err(format!(
                "{API_BOUNDARIES_RELATIVE} exception {} expired at Step {}",
                exception.id, exception.removal_step
            ));
        }
        if exception.rationale.trim().is_empty() {
            return Err(format!(
                "{API_BOUNDARIES_RELATIVE} exception {} must include a rationale",
                exception.id
            ));
        }
        validate_decision_reference(exception, &decision)?;
        for item in &exception.items {
            let key = (
                exception.package.as_str(),
                exception.source.as_str(),
                item.as_str(),
                exception.forbidden_path.as_str(),
            );
            if !exception_keys.insert(key) {
                return Err(format!(
                    "{API_BOUNDARIES_RELATIVE} exception {} duplicates an item-scoped allowance",
                    exception.id
                ));
            }
        }
    }
    let active_exception_ids = decision
        .active_exception_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if exception_ids != active_exception_ids {
        return Err(format!(
            "{API_BOUNDARIES_RELATIVE} active exception ids must exactly match {API_DECISION_RELATIVE}"
        ));
    }
    Ok(())
}

fn sorted_unique<'a>(
    label: &str,
    values: &'a [String],
    allow_empty: bool,
) -> Result<BTreeSet<&'a str>, String> {
    if !allow_empty && values.is_empty() {
        return Err(format!(
            "{API_BOUNDARIES_RELATIVE} {label} must not be empty"
        ));
    }
    let unique = values.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if unique.iter().copied().ne(values.iter().map(String::as_str)) {
        return Err(format!(
            "{API_BOUNDARIES_RELATIVE} {label} must be sorted and unique"
        ));
    }
    Ok(unique)
}

fn valid_exception_id(id: &str) -> bool {
    id.strip_prefix("RCRV1-API-")
        .is_some_and(|suffix| suffix.len() == 3 && suffix.bytes().all(|byte| byte.is_ascii_digit()))
}

fn validate_relative_source(id: &str, source: &str) -> Result<(), String> {
    let path = Path::new(source);
    if source.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || path.extension().and_then(|extension| extension.to_str()) != Some("rs")
    {
        return Err(format!(
            "{API_BOUNDARIES_RELATIVE} exception {id} source must be a normalized relative Rust path"
        ));
    }
    Ok(())
}

fn load_and_validate_decision(workspace_root: &Path) -> Result<ApiLeakageDecision, String> {
    let path = workspace_root.join(API_DECISION_RELATIVE);
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let decision = toml::from_str::<ApiLeakageDecision>(&raw)
        .map_err(|error| format!("parse {}: {error}", path.display()))?;

    if decision.schema_version != DECISION_SCHEMA_VERSION
        || decision.decision_id != API_DECISION_ID
        || decision.status != "accepted_historical"
        || decision.accepted_date != "2026-07-27"
        || decision.completed_step != CURRENT_STEP
        || decision.publication_authorized
    {
        return Err(format!(
            "{API_DECISION_RELATIVE} identity, lifecycle, or publication policy drifted"
        ));
    }
    if decision
        .exception_ids
        .iter()
        .map(String::as_str)
        .ne(HISTORICAL_EXCEPTION_IDS)
    {
        return Err(format!(
            "{API_DECISION_RELATIVE} must preserve the exact historical exception ids"
        ));
    }
    if !decision.active_exception_ids.is_empty() {
        return Err(format!(
            "{API_DECISION_RELATIVE} must not authorize active exceptions after Step {CURRENT_STEP}"
        ));
    }
    if decision
        .forbidden_expansion
        .iter()
        .map(String::as_str)
        .ne(FORBIDDEN_EXPANSION)
    {
        return Err(format!(
            "{API_DECISION_RELATIVE} forbidden expansion set drifted"
        ));
    }
    if decision
        .removal_milestone
        .iter()
        .map(|milestone| (milestone.package.as_str(), milestone.step))
        .ne(REMOVAL_MILESTONES)
    {
        return Err(format!(
            "{API_DECISION_RELATIVE} removal milestones drifted"
        ));
    }
    Ok(decision)
}

fn validate_decision_reference(
    exception: &ApiException,
    decision: &ApiLeakageDecision,
) -> Result<(), String> {
    let path = Path::new(&exception.decision);
    if exception.decision != API_DECISION_RELATIVE
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || path.extension().and_then(|extension| extension.to_str()) != Some("toml")
    {
        return Err(format!(
            "{API_BOUNDARIES_RELATIVE} exception {} decision must be {API_DECISION_RELATIVE}",
            exception.id
        ));
    }
    if !decision
        .active_exception_ids
        .iter()
        .any(|id| id == &exception.id)
    {
        return Err(format!(
            "{API_BOUNDARIES_RELATIVE} exception {} is not active in {API_DECISION_RELATIVE}",
            exception.id
        ));
    }
    Ok(())
}

fn package_set_mismatch(expected: &BTreeSet<String>, actual: &BTreeSet<String>) -> String {
    let missing = expected
        .difference(actual)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let extra = actual
        .difference(expected)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{API_BOUNDARIES_RELATIVE} package catalog mismatch; missing: {missing}; extra: {extra}"
    )
}

fn load_metadata(workspace_root: &Path) -> Result<CargoMetadata, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed while checking public API boundaries: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse cargo metadata for public API boundaries: {error}"))
}

fn scan_workspace(
    _workspace_root: &Path,
    policy: &ApiBoundaryPolicy,
    metadata: &CargoMetadata,
) -> Result<Vec<ApiFinding>, String> {
    let workspace_members = metadata
        .workspace_members
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let package_policy = policy
        .package
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let forbidden = policy
        .forbidden_public_paths
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut findings = BTreeSet::new();

    for package in &metadata.packages {
        if !workspace_members.contains(package.id.as_str()) {
            continue;
        }
        let Some(package_policy) = package_policy.get(package.name.as_str()).copied() else {
            continue;
        };
        let manifest_path = Path::new(&package.manifest_path);
        let package_root = manifest_path.parent().ok_or_else(|| {
            format!(
                "package {} manifest has no parent: {}",
                package.name, package.manifest_path
            )
        })?;
        let allowed = package_policy
            .allowed_public_paths
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let library_targets = package
            .targets
            .iter()
            .filter(|target| {
                target
                    .kind
                    .iter()
                    .any(|kind| kind == "lib" || kind == "proc-macro")
            })
            .collect::<Vec<_>>();
        if library_targets.len() != 1 {
            return Err(format!(
                "public package {} must expose exactly one library target for API leakage analysis",
                package.name
            ));
        }
        let source_path = Path::new(&library_targets[0].src_path);
        let source_module_directory = module_directory(source_path)?;
        let mut units = BTreeMap::new();
        collect_module(
            package_root,
            Vec::new(),
            None,
            true,
            true,
            source_path,
            &source_module_directory,
            None,
            &mut units,
        )?;
        findings.extend(scan_package_units(
            &package.name,
            &units,
            &forbidden,
            &allowed,
        )?);
    }

    Ok(findings.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
fn collect_module(
    package_root: &Path,
    module: Vec<String>,
    parent: Option<Vec<String>>,
    declared_public: bool,
    initially_effective: bool,
    source_path: &Path,
    module_directory: &Path,
    inline_items: Option<Vec<Item>>,
    units: &mut BTreeMap<Vec<String>, ModuleUnit>,
) -> Result<(), String> {
    if units.contains_key(&module) {
        return Ok(());
    }
    let (items, source_relative) = if let Some(items) = inline_items {
        let relative = source_path
            .strip_prefix(package_root)
            .map_err(|_| {
                format!(
                    "module source {} escapes package {}",
                    source_path.display(),
                    package_root.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        (items, relative)
    } else {
        let raw = fs::read_to_string(source_path)
            .map_err(|error| format!("read {}: {error}", source_path.display()))?;
        let file = syn::parse_file(&raw)
            .map_err(|error| format!("parse {}: {error}", source_path.display()))?;
        let relative = source_path
            .strip_prefix(package_root)
            .map_err(|_| {
                format!(
                    "module source {} escapes package {}",
                    source_path.display(),
                    package_root.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        (file.items, relative)
    };

    let children = items
        .iter()
        .filter_map(|item| match item {
            Item::Mod(item_mod) => Some(item_mod.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    units.insert(
        module.clone(),
        ModuleUnit {
            module: module.clone(),
            parent,
            declared_public,
            initially_effective,
            source_relative,
            items,
        },
    );

    for child in children {
        let mut child_module = module.clone();
        child_module.push(child.ident.to_string());
        let child_public = is_public(&child.vis);
        let child_effective = initially_effective && child_public;
        if let Some((_, inline)) = child.content {
            collect_module(
                package_root,
                child_module,
                Some(module.clone()),
                child_public,
                child_effective,
                source_path,
                &module_directory.join(child.ident.to_string()),
                Some(inline),
                units,
            )?;
        } else {
            let child_path = module_source_path(source_path, module_directory, &child)?;
            collect_module(
                package_root,
                child_module,
                Some(module.clone()),
                child_public,
                child_effective,
                &child_path,
                &module_directory.join(child.ident.to_string()),
                None,
                units,
            )?;
        }
    }
    Ok(())
}

fn module_directory(source_path: &Path) -> Result<PathBuf, String> {
    let parent = source_path
        .parent()
        .ok_or_else(|| format!("module source has no parent: {}", source_path.display()))?;
    let file_name = source_path.file_name().and_then(|name| name.to_str());
    if matches!(file_name, Some("lib.rs" | "main.rs" | "mod.rs")) {
        Ok(parent.to_path_buf())
    } else {
        Ok(parent.join(
            source_path
                .file_stem()
                .ok_or_else(|| format!("module source has no stem: {}", source_path.display()))?,
        ))
    }
}

fn module_source_path(
    source_path: &Path,
    module_directory: &Path,
    item_mod: &syn::ItemMod,
) -> Result<PathBuf, String> {
    if let Some(path) = path_attribute(item_mod) {
        return Ok(source_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path));
    }
    let name = item_mod.ident.to_string();
    let flat = module_directory.join(format!("{name}.rs"));
    let nested = module_directory.join(&name).join("mod.rs");
    match (flat.is_file(), nested.is_file()) {
        (true, false) => Ok(flat),
        (false, true) => Ok(nested),
        (true, true) => Err(format!(
            "module {} is ambiguous between {} and {}",
            item_mod.ident,
            flat.display(),
            nested.display()
        )),
        (false, false) => Err(format!(
            "module {} source is missing under {}",
            item_mod.ident,
            module_directory.display()
        )),
    }
}

fn path_attribute(item_mod: &syn::ItemMod) -> Option<String> {
    item_mod.attrs.iter().find_map(|attribute| {
        if !attribute.path().is_ident("path") {
            return None;
        }
        let Meta::NameValue(value) = &attribute.meta else {
            return None;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(path),
            ..
        }) = &value.value
        else {
            return None;
        };
        Some(path.value())
    })
}

fn scan_package_units(
    package: &str,
    units: &BTreeMap<Vec<String>, ModuleUnit>,
    forbidden: &BTreeSet<&str>,
    allowed: &BTreeSet<&str>,
) -> Result<Vec<ApiFinding>, String> {
    let mut effective = units
        .values()
        .filter(|unit| unit.initially_effective)
        .map(|unit| unit.module.clone())
        .collect::<BTreeSet<_>>();
    let mut exported_items = BTreeSet::new();
    let mut changed = true;
    while changed {
        changed = propagate_public_modules(units, &mut effective);
        let scopes = effective.iter().cloned().collect::<Vec<_>>();
        for scope in scopes {
            let unit = units
                .get(&scope)
                .ok_or_else(|| format!("missing module unit {}", module_label(&scope)))?;
            for item_use in unit.items.iter().filter_map(|item| match item {
                Item::Use(item_use) if is_public(&item_use.vis) => Some(item_use),
                _ => None,
            }) {
                for (segments, glob) in flatten_use_tree(&item_use.tree) {
                    match resolve_internal_export(&scope, &segments, glob, units) {
                        InternalExport::Module(module) => {
                            changed |= effective.insert(module);
                        }
                        InternalExport::Item(module, item) => {
                            changed |= exported_items.insert((module, item));
                        }
                        InternalExport::External => {}
                    }
                }
            }
        }
    }
    propagate_public_modules(units, &mut effective);

    let mut findings = BTreeSet::new();
    for unit in units.values() {
        let imports = import_map(&unit.items);
        let unit_effective = effective.contains(&unit.module);
        let explicitly_exported = exported_items
            .iter()
            .filter(|(module, _)| module == &unit.module)
            .map(|(_, item)| item.as_str())
            .collect::<BTreeSet<_>>();

        for item in &unit.items {
            if let Item::Use(item_use) = item {
                if unit_effective && is_public(&item_use.vis) {
                    scan_public_use(
                        package,
                        unit,
                        item_use,
                        &imports,
                        forbidden,
                        allowed,
                        &mut findings,
                    );
                }
                continue;
            }
            if let Item::Impl(item_impl) = item {
                let Some(self_name) = impl_self_name(&item_impl.self_ty) else {
                    continue;
                };
                if unit_effective || explicitly_exported.contains(self_name.as_str()) {
                    scan_impl(
                        package,
                        unit,
                        item_impl,
                        &self_name,
                        &imports,
                        forbidden,
                        allowed,
                        &mut findings,
                    );
                }
                continue;
            }
            let Some(name) = item_name(item) else {
                continue;
            };
            if (unit_effective && item_is_public(item))
                || explicitly_exported.contains(name.as_str())
            {
                scan_item(
                    package,
                    unit,
                    item,
                    &imports,
                    forbidden,
                    allowed,
                    &mut findings,
                );
            }
        }
    }
    Ok(findings.into_iter().collect())
}

fn propagate_public_modules(
    units: &BTreeMap<Vec<String>, ModuleUnit>,
    effective: &mut BTreeSet<Vec<String>>,
) -> bool {
    let mut changed = false;
    loop {
        let before = effective.len();
        for unit in units.values() {
            if unit.declared_public
                && unit
                    .parent
                    .as_ref()
                    .is_some_and(|parent| effective.contains(parent))
            {
                effective.insert(unit.module.clone());
            }
        }
        if effective.len() == before {
            break;
        }
        changed = true;
    }
    changed
}

fn flatten_use_tree(tree: &UseTree) -> Vec<(Vec<String>, bool)> {
    fn visit(tree: &UseTree, prefix: &mut Vec<String>, output: &mut Vec<(Vec<String>, bool)>) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                visit(&path.tree, prefix, output);
                prefix.pop();
            }
            UseTree::Name(name) => {
                if name.ident == "self" {
                    output.push((prefix.clone(), false));
                } else {
                    let mut path = prefix.clone();
                    path.push(name.ident.to_string());
                    output.push((path, false));
                }
            }
            UseTree::Rename(rename) => {
                let mut path = prefix.clone();
                path.push(rename.ident.to_string());
                output.push((path, false));
            }
            UseTree::Glob(_) => output.push((prefix.clone(), true)),
            UseTree::Group(group) => {
                for item in &group.items {
                    visit(item, prefix, output);
                }
            }
        }
    }

    let mut output = Vec::new();
    visit(tree, &mut Vec::new(), &mut output);
    output
}

fn resolve_internal_export(
    current: &[String],
    segments: &[String],
    glob: bool,
    units: &BTreeMap<Vec<String>, ModuleUnit>,
) -> InternalExport {
    if segments.is_empty() {
        return InternalExport::External;
    }
    let candidates = internal_candidates(current, segments);
    for candidate in candidates {
        if glob && units.contains_key(&candidate) {
            return InternalExport::Module(candidate);
        }
        if units.contains_key(&candidate) {
            return InternalExport::Module(candidate);
        }
        if let Some((item, module)) = candidate.split_last() {
            let module = module.to_vec();
            if units.contains_key(&module) {
                return InternalExport::Item(module, item.clone());
            }
        }
    }
    InternalExport::External
}

fn internal_candidates(current: &[String], segments: &[String]) -> Vec<Vec<String>> {
    let mut candidates = Vec::new();
    match segments.first().map(String::as_str) {
        Some("crate") => candidates.push(segments[1..].to_vec()),
        Some("self") => {
            let mut path = current.to_vec();
            path.extend_from_slice(&segments[1..]);
            candidates.push(path);
        }
        Some("super") => {
            let mut base = current.to_vec();
            let mut index = 0;
            while segments.get(index).map(String::as_str) == Some("super") {
                base.pop();
                index += 1;
            }
            base.extend_from_slice(&segments[index..]);
            candidates.push(base);
        }
        _ => {
            candidates.push(segments.to_vec());
            let mut local = current.to_vec();
            local.extend_from_slice(segments);
            if !candidates.contains(&local) {
                candidates.push(local);
            }
        }
    }
    candidates
}

fn import_map(items: &[Item]) -> BTreeMap<String, Vec<String>> {
    fn visit(tree: &UseTree, prefix: &mut Vec<String>, output: &mut BTreeMap<String, Vec<String>>) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                visit(&path.tree, prefix, output);
                prefix.pop();
            }
            UseTree::Name(name) => {
                if name.ident == "self" {
                    if let Some(local) = prefix.last() {
                        output.insert(local.clone(), prefix.clone());
                    }
                } else {
                    let mut path = prefix.clone();
                    path.push(name.ident.to_string());
                    output.insert(name.ident.to_string(), path);
                }
            }
            UseTree::Rename(rename) => {
                let mut path = prefix.clone();
                path.push(rename.ident.to_string());
                output.insert(rename.rename.to_string(), path);
            }
            UseTree::Glob(_) => {}
            UseTree::Group(group) => {
                for item in &group.items {
                    visit(item, prefix, output);
                }
            }
        }
    }

    let mut output = BTreeMap::new();
    for item_use in items.iter().filter_map(|item| match item {
        Item::Use(item_use) => Some(item_use),
        _ => None,
    }) {
        visit(&item_use.tree, &mut Vec::new(), &mut output);
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn scan_item(
    package: &str,
    unit: &ModuleUnit,
    item: &Item,
    imports: &BTreeMap<String, Vec<String>>,
    forbidden: &BTreeSet<&str>,
    allowed: &BTreeSet<&str>,
    findings: &mut BTreeSet<ApiFinding>,
) {
    let name = item_name(item).unwrap_or_else(|| "<item>".to_owned());
    let label = qualified_item(&unit.module, &name);
    let mut visitor =
        LeakageVisitor::new(package, unit, label, imports, forbidden, allowed, findings);
    match item {
        Item::Const(item) => visitor.visit_type(&item.ty),
        Item::Enum(item) => {
            visitor.visit_generics(&item.generics);
            for variant in &item.variants {
                visit_fields(&mut visitor, &variant.fields, true);
            }
        }
        Item::Fn(item) => visitor.visit_signature(&item.sig),
        Item::ForeignMod(item) => {
            for foreign in &item.items {
                if let ForeignItem::Fn(function) = foreign {
                    visitor.visit_signature(&function.sig);
                }
            }
        }
        Item::Static(item) => visitor.visit_type(&item.ty),
        Item::Struct(item) => {
            visitor.visit_generics(&item.generics);
            visit_fields(&mut visitor, &item.fields, false);
        }
        Item::Trait(item) => {
            visitor.visit_generics(&item.generics);
            for bound in &item.supertraits {
                visitor.visit_type_param_bound(bound);
            }
            for trait_item in &item.items {
                match trait_item {
                    TraitItem::Const(item) => visitor.visit_type(&item.ty),
                    TraitItem::Fn(item) => visitor.visit_signature(&item.sig),
                    TraitItem::Type(item) => {
                        visitor.visit_generics(&item.generics);
                        for bound in &item.bounds {
                            visitor.visit_type_param_bound(bound);
                        }
                        if let Some((_, ty)) = &item.default {
                            visitor.visit_type(ty);
                        }
                    }
                    _ => {}
                }
            }
        }
        Item::TraitAlias(item) => {
            visitor.visit_generics(&item.generics);
            for bound in &item.bounds {
                visitor.visit_type_param_bound(bound);
            }
        }
        Item::Type(item) => {
            visitor.visit_generics(&item.generics);
            visitor.visit_type(&item.ty);
        }
        Item::Union(item) => {
            visitor.visit_generics(&item.generics);
            for field in &item.fields.named {
                if is_public(&field.vis) {
                    visitor.visit_type(&field.ty);
                }
            }
        }
        _ => {}
    }
}

fn visit_fields(visitor: &mut LeakageVisitor<'_>, fields: &Fields, all_visible: bool) {
    for field in fields {
        if all_visible || is_public(&field.vis) {
            visitor.visit_type(&field.ty);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn scan_impl(
    package: &str,
    unit: &ModuleUnit,
    item_impl: &syn::ItemImpl,
    self_name: &str,
    imports: &BTreeMap<String, Vec<String>>,
    forbidden: &BTreeSet<&str>,
    allowed: &BTreeSet<&str>,
    findings: &mut BTreeSet<ApiFinding>,
) {
    let trait_impl = item_impl.trait_.is_some();
    for item in &item_impl.items {
        let (name, is_exposed) = match item {
            ImplItem::Const(item) => (item.ident.to_string(), trait_impl || is_public(&item.vis)),
            ImplItem::Fn(item) => (
                item.sig.ident.to_string(),
                trait_impl || is_public(&item.vis),
            ),
            ImplItem::Type(item) => (item.ident.to_string(), trait_impl || is_public(&item.vis)),
            _ => continue,
        };
        if !is_exposed {
            continue;
        }
        let label = format!("{}::{name}", qualified_item(&unit.module, self_name));
        let mut visitor =
            LeakageVisitor::new(package, unit, label, imports, forbidden, allowed, findings);
        if let Some((_, trait_path, _)) = &item_impl.trait_ {
            visitor.visit_path(trait_path);
        }
        match item {
            ImplItem::Const(item) => visitor.visit_type(&item.ty),
            ImplItem::Fn(item) => visitor.visit_signature(&item.sig),
            ImplItem::Type(item) => {
                visitor.visit_generics(&item.generics);
                visitor.visit_type(&item.ty);
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn scan_public_use(
    package: &str,
    unit: &ModuleUnit,
    item_use: &syn::ItemUse,
    imports: &BTreeMap<String, Vec<String>>,
    forbidden: &BTreeSet<&str>,
    allowed: &BTreeSet<&str>,
    findings: &mut BTreeSet<ApiFinding>,
) {
    for (segments, _) in flatten_use_tree(&item_use.tree) {
        if segments.is_empty() {
            continue;
        }
        let label = format!("{}::pub use", module_label(&unit.module));
        record_path(
            package, unit, &label, &segments, imports, forbidden, allowed, findings,
        );
    }
}

struct LeakageVisitor<'a> {
    package: &'a str,
    unit: &'a ModuleUnit,
    item: String,
    imports: &'a BTreeMap<String, Vec<String>>,
    forbidden: &'a BTreeSet<&'a str>,
    allowed: &'a BTreeSet<&'a str>,
    findings: &'a mut BTreeSet<ApiFinding>,
}

impl<'a> LeakageVisitor<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        package: &'a str,
        unit: &'a ModuleUnit,
        item: String,
        imports: &'a BTreeMap<String, Vec<String>>,
        forbidden: &'a BTreeSet<&'a str>,
        allowed: &'a BTreeSet<&'a str>,
        findings: &'a mut BTreeSet<ApiFinding>,
    ) -> Self {
        Self {
            package,
            unit,
            item,
            imports,
            forbidden,
            allowed,
            findings,
        }
    }
}

impl<'ast> Visit<'ast> for LeakageVisitor<'_> {
    fn visit_path(&mut self, path: &'ast SynPath) {
        let segments = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        record_path(
            self.package,
            self.unit,
            &self.item,
            &segments,
            self.imports,
            self.forbidden,
            self.allowed,
            self.findings,
        );
        syn::visit::visit_path(self, path);
    }
}

#[allow(clippy::too_many_arguments)]
fn record_path(
    package: &str,
    unit: &ModuleUnit,
    item: &str,
    segments: &[String],
    imports: &BTreeMap<String, Vec<String>>,
    forbidden: &BTreeSet<&str>,
    allowed: &BTreeSet<&str>,
    findings: &mut BTreeSet<ApiFinding>,
) {
    if segments.is_empty() {
        return;
    }
    let resolved = if let Some(imported) = imports.get(&segments[0]) {
        let mut resolved = imported.clone();
        resolved.extend_from_slice(&segments[1..]);
        resolved
    } else {
        segments.to_vec()
    };
    let observed = resolved.join("::");
    for forbidden_path in forbidden {
        if allowed.contains(forbidden_path)
            || !(observed == *forbidden_path
                || observed
                    .strip_prefix(forbidden_path)
                    .is_some_and(|suffix| suffix.starts_with("::")))
        {
            continue;
        }
        findings.insert(ApiFinding {
            package: package.to_owned(),
            source: unit.source_relative.clone(),
            item: item.to_owned(),
            forbidden_path: (*forbidden_path).to_owned(),
            observed_path: observed.clone(),
        });
    }
}

fn exception_matches(exceptions: &[ApiException], finding: &ApiFinding) -> bool {
    exceptions.iter().any(|exception| {
        exception.package == finding.package
            && exception.source == finding.source
            && exception.forbidden_path == finding.forbidden_path
            && exception.items.binary_search(&finding.item).is_ok()
            && exception
                .observed_paths
                .binary_search(&finding.observed_path)
                .is_ok()
    })
}

fn format_finding(finding: &ApiFinding) -> String {
    format!(
        "package={} source={} item={} forbidden_path={} observed_path={}",
        finding.package,
        finding.source,
        finding.item,
        finding.forbidden_path,
        finding.observed_path
    )
}

fn item_is_public(item: &Item) -> bool {
    match item {
        Item::Const(item) => is_public(&item.vis),
        Item::Enum(item) => is_public(&item.vis),
        Item::Fn(item) => is_public(&item.vis),
        Item::ForeignMod(_) => true,
        Item::Static(item) => is_public(&item.vis),
        Item::Struct(item) => is_public(&item.vis),
        Item::Trait(item) => is_public(&item.vis),
        Item::TraitAlias(item) => is_public(&item.vis),
        Item::Type(item) => is_public(&item.vis),
        Item::Union(item) => is_public(&item.vis),
        _ => false,
    }
}

fn item_name(item: &Item) -> Option<String> {
    match item {
        Item::Const(item) => Some(item.ident.to_string()),
        Item::Enum(item) => Some(item.ident.to_string()),
        Item::Fn(item) => Some(item.sig.ident.to_string()),
        Item::Static(item) => Some(item.ident.to_string()),
        Item::Struct(item) => Some(item.ident.to_string()),
        Item::Trait(item) => Some(item.ident.to_string()),
        Item::TraitAlias(item) => Some(item.ident.to_string()),
        Item::Type(item) => Some(item.ident.to_string()),
        Item::Union(item) => Some(item.ident.to_string()),
        _ => None,
    }
}

fn impl_self_name(self_ty: &Type) -> Option<String> {
    let Type::Path(path) = self_ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn qualified_item(module: &[String], item: &str) -> String {
    if module.is_empty() {
        item.to_owned()
    } else {
        format!("{}::{item}", module.join("::"))
    }
}

fn module_label(module: &[String]) -> String {
    if module.is_empty() {
        "crate".to_owned()
    } else {
        module.join("::")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        API_DECISION_RELATIVE, ApiBoundaryPolicy, ApiException, CargoMetadata, scan_workspace,
        validate_policy,
    };
    use serde::Deserialize;
    use std::{collections::BTreeSet, fs};

    const POLICY: &str = include_str!("../../../../contracts/releases/api_boundaries.toml");
    const DECISION: &str = include_str!(
        "../../../../contracts/architecture/decisions/public_api_leakage_migration_baseline.v1.toml"
    );
    const ARCHITECTURE: &str =
        include_str!("../../../../contracts/crates/release_v1/radroots_crates_release_v1.toml");
    const GENERIC_SQLX: &str = include_str!("../../tests/fixtures/api-leakage/generic-sqlx.rs");
    const GENERIC_RENAMED_TOKIO: &str =
        include_str!("../../tests/fixtures/api-leakage/generic-renamed-tokio.rs");
    const ALLOWED_ADAPTER: &str =
        include_str!("../../tests/fixtures/api-leakage/allowed-concrete-adapter.rs");
    const PRIVATE_IMPLEMENTATION: &str =
        include_str!("../../tests/fixtures/api-leakage/private-implementation.rs");
    #[derive(Deserialize)]
    struct ArchitectureCatalog {
        package: Vec<ArchitecturePackage>,
    }

    #[derive(Deserialize)]
    struct ArchitecturePackage {
        name: String,
    }

    fn policy() -> ApiBoundaryPolicy {
        toml::from_str(POLICY).expect("API boundary policy")
    }

    fn expected_packages() -> BTreeSet<String> {
        toml::from_str::<ArchitectureCatalog>(ARCHITECTURE)
            .expect("architecture")
            .package
            .into_iter()
            .map(|package| package.name)
            .collect()
    }

    fn fixture_metadata(root: &std::path::Path, package: &str, source: &str) -> CargoMetadata {
        let package_root = root.join(package);
        fs::create_dir_all(package_root.join("src")).expect("create fixture source");
        fs::write(
            package_root.join("Cargo.toml"),
            "[package]\nname='fixture'\n",
        )
        .expect("write fixture manifest");
        fs::write(package_root.join("src/lib.rs"), source).expect("write fixture source");
        let id = format!("fixture#{package}@0.1.0");
        CargoMetadata {
            workspace_members: vec![id.clone()],
            packages: vec![super::CargoPackage {
                id,
                name: package.to_owned(),
                manifest_path: package_root
                    .join("Cargo.toml")
                    .to_string_lossy()
                    .into_owned(),
                targets: vec![super::CargoTarget {
                    kind: vec!["lib".to_owned()],
                    src_path: package_root
                        .join("src/lib.rs")
                        .to_string_lossy()
                        .into_owned(),
                }],
            }],
        }
    }

    fn scan_fixture(package: &str, source: &str) -> Vec<super::ApiFinding> {
        let root = tempfile::TempDir::new().expect("fixture root");
        scan_workspace(
            root.path(),
            &policy(),
            &fixture_metadata(root.path(), package, source),
        )
        .expect("scan fixture")
    }

    fn write_baseline_decision(root: &std::path::Path) {
        fs::create_dir_all(root.join("contracts/architecture/decisions"))
            .expect("baseline decision directory");
        fs::write(
            root.join(
                "contracts/architecture/decisions/public_api_leakage_migration_baseline.v1.toml",
            ),
            DECISION,
        )
        .expect("baseline decision");
    }

    #[test]
    fn policy_covers_exact_architecture_catalog() {
        let root = tempfile::TempDir::new().expect("policy root");
        write_baseline_decision(root.path());
        validate_policy(root.path(), &policy(), &expected_packages())
            .expect("policy without exceptions");
    }

    #[test]
    fn generic_sqlx_public_field_is_forbidden() {
        let findings = scan_fixture("radroots_storage", GENERIC_SQLX);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].item, "StorageHandle");
        assert_eq!(findings[0].forbidden_path, "sqlx");
        assert_eq!(findings[0].observed_path, "sqlx::SqlitePool");
    }

    #[test]
    fn renamed_tokio_return_type_is_resolved() {
        let findings = scan_fixture("radroots_sync", GENERIC_RENAMED_TOKIO);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].item, "executor");
        assert_eq!(findings[0].forbidden_path, "tokio");
        assert_eq!(findings[0].observed_path, "tokio::runtime::Handle");
    }

    #[test]
    fn specified_nostr_adapter_may_expose_protocol_types() {
        let findings = scan_fixture("radroots_nostr", ALLOWED_ADAPTER);
        assert!(findings.is_empty());
    }

    #[test]
    fn private_implementation_types_are_not_public_api() {
        let findings = scan_fixture("radroots_sdk", PRIVATE_IMPLEMENTATION);
        assert!(findings.is_empty());
    }

    #[test]
    fn invented_exception_is_rejected_by_the_completed_decision() {
        let root = tempfile::TempDir::new().expect("exception root");
        write_baseline_decision(root.path());
        let mut policy = policy();
        policy.exception.push(ApiException {
            id: "RCRV1-API-999".to_owned(),
            package: "radroots_sdk".to_owned(),
            source: "src/lib.rs".to_owned(),
            forbidden_path: "reqwest".to_owned(),
            items: vec!["temporary_client".to_owned()],
            observed_paths: vec!["reqwest::Client".to_owned()],
            decision: API_DECISION_RELATIVE.to_owned(),
            removal_step: 400,
            rationale: "test-only exception".to_owned(),
        });
        let error = validate_policy(root.path(), &policy, &expected_packages())
            .expect_err("invented exception must fail");
        assert!(error.contains("is not active"));
    }

    #[test]
    fn expired_historical_exception_cannot_return() {
        let root = tempfile::TempDir::new().expect("expired exception root");
        write_baseline_decision(root.path());
        let mut policy = policy();
        policy.exception.push(ApiException {
            id: "RCRV1-API-006".to_owned(),
            package: "radroots_nostr_connect".to_owned(),
            source: "src/client.rs".to_owned(),
            forbidden_path: "nostr".to_owned(),
            items: vec!["client::RadrootsNostrConnectClientTarget".to_owned()],
            observed_paths: vec!["nostr::PublicKey".to_owned()],
            decision: API_DECISION_RELATIVE.to_owned(),
            removal_step: 313,
            rationale: "retired historical exception".to_owned(),
        });
        let error = validate_policy(root.path(), &policy, &expected_packages())
            .expect_err("expired historical exception must fail");
        assert!(error.contains("expired at Step 313"));
    }

    #[test]
    fn malformed_or_publication_authorizing_decision_fails_closed() {
        let root = tempfile::TempDir::new().expect("decision validation root");
        write_baseline_decision(root.path());
        let decision_path = root.path().join(API_DECISION_RELATIVE);
        fs::write(&decision_path, "not valid toml = [\n").expect("malformed decision");
        let malformed = validate_policy(root.path(), &policy(), &expected_packages())
            .expect_err("malformed decision must fail");
        assert!(malformed.contains("parse"));

        fs::write(
            &decision_path,
            DECISION.replace(
                "publication_authorized = false",
                "publication_authorized = true",
            ),
        )
        .expect("publication-authorizing decision");
        let authorized = validate_policy(root.path(), &policy(), &expected_packages())
            .expect_err("publication authorization must fail");
        assert!(authorized.contains("publication policy drifted"));
    }
}

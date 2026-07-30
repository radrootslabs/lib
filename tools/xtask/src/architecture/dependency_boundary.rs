use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

use serde::Deserialize;

const PACKAGE_TIERS_RELATIVE: &str = "contracts/releases/package_tiers.toml";
const SPEC_ID: &str = "radroots.crates.release.v1";
const DIRECTION: &str = "same_or_lower";
const POLICY_SCHEMA_VERSION: u16 = 1;
const CURRENT_STEP: u16 = 24;
const EXPECTED_TIERS: &[(&str, u8)] = &[
    ("foundation", 0),
    ("domain", 1),
    ("spi", 2),
    ("adapter", 3),
    ("orchestration", 4),
    ("sdk", 5),
    ("facade", 6),
];
const EXPECTED_DEPENDENCY_KINDS: &[&str] = &["build", "dev", "normal"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DependencyBoundaryPolicy {
    schema_version: u16,
    spec_id: String,
    direction: String,
    enforced_dependency_kinds: Vec<String>,
    tier: Vec<PackageTier>,
    #[serde(default)]
    temporary_exception: Vec<TemporaryException>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageTier {
    id: String,
    rank: u8,
    packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemporaryException {
    owner: String,
    dependency: String,
    kind: String,
    target: String,
    features: Vec<String>,
    uses_default_features: bool,
    removal_step: u16,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    resolve: Option<CargoResolve>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    #[serde(default)]
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Deserialize)]
struct CargoDependency {
    name: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default = "default_true")]
    uses_default_features: bool,
}

#[derive(Debug, Deserialize)]
struct CargoResolve {
    nodes: Vec<CargoNode>,
}

#[derive(Debug, Deserialize)]
struct CargoNode {
    id: String,
    #[serde(default)]
    deps: Vec<CargoNodeDependency>,
}

#[derive(Debug, Deserialize)]
struct CargoNodeDependency {
    name: String,
    pkg: String,
    #[serde(default)]
    dep_kinds: Vec<CargoDependencyKind>,
}

#[derive(Debug, Deserialize)]
struct CargoDependencyKind {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Clone, Copy)]
struct PackagePlacement<'a> {
    tier: &'a str,
    rank: u8,
}

fn default_true() -> bool {
    true
}

pub(super) fn validate_policy_catalog(
    workspace_root: &Path,
    expected_packages: &BTreeSet<String>,
) -> Result<(), String> {
    let policy = load_policy(workspace_root)?;
    validate_policy(&policy, expected_packages).map(|_| ())
}

pub(super) fn validate_resolved_boundaries(workspace_root: &Path) -> Result<(), String> {
    let policy = load_policy(workspace_root)?;
    let expected_packages = policy
        .tier
        .iter()
        .flat_map(|tier| tier.packages.iter().cloned())
        .collect::<BTreeSet<_>>();
    let placements = validate_policy(&policy, &expected_packages)?;
    let metadata = load_metadata(workspace_root)?;
    validate_metadata(&policy, &placements, &metadata)
}

fn load_policy(workspace_root: &Path) -> Result<DependencyBoundaryPolicy, String> {
    let path = workspace_root.join(PACKAGE_TIERS_RELATIVE);
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn validate_policy<'a>(
    policy: &'a DependencyBoundaryPolicy,
    expected_packages: &BTreeSet<String>,
) -> Result<BTreeMap<&'a str, PackagePlacement<'a>>, String> {
    if policy.schema_version != POLICY_SCHEMA_VERSION {
        return Err(format!(
            "{PACKAGE_TIERS_RELATIVE} schema_version must be {POLICY_SCHEMA_VERSION}"
        ));
    }
    if policy.spec_id != SPEC_ID {
        return Err(format!(
            "{PACKAGE_TIERS_RELATIVE} spec_id must be {SPEC_ID}"
        ));
    }
    if policy.direction != DIRECTION {
        return Err(format!(
            "{PACKAGE_TIERS_RELATIVE} direction must be {DIRECTION}"
        ));
    }

    let actual_kinds = unique_strings(
        "enforced_dependency_kinds",
        &policy.enforced_dependency_kinds,
    )?;
    let expected_kinds = EXPECTED_DEPENDENCY_KINDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if actual_kinds != expected_kinds {
        return Err(format!(
            "{PACKAGE_TIERS_RELATIVE} enforced_dependency_kinds must be build, dev, normal"
        ));
    }

    let actual_tiers = policy
        .tier
        .iter()
        .map(|tier| (tier.id.as_str(), tier.rank))
        .collect::<Vec<_>>();
    if actual_tiers != EXPECTED_TIERS {
        return Err(format!(
            "{PACKAGE_TIERS_RELATIVE} tiers must match the approved ordered architecture"
        ));
    }

    let mut placements = BTreeMap::new();
    for tier in &policy.tier {
        if tier.packages.is_empty() {
            return Err(format!(
                "{PACKAGE_TIERS_RELATIVE} tier {} must allocate at least one package",
                tier.id
            ));
        }
        for package in &tier.packages {
            if placements
                .insert(
                    package.as_str(),
                    PackagePlacement {
                        tier: &tier.id,
                        rank: tier.rank,
                    },
                )
                .is_some()
            {
                return Err(format!(
                    "{PACKAGE_TIERS_RELATIVE} package {package} is allocated more than once"
                ));
            }
        }
    }

    let actual_packages = placements
        .keys()
        .map(|package| (*package).to_owned())
        .collect::<BTreeSet<_>>();
    if &actual_packages != expected_packages {
        return Err(package_set_mismatch(expected_packages, &actual_packages));
    }

    let mut exception_keys = BTreeSet::new();
    for exception in &policy.temporary_exception {
        let owner = placements.get(exception.owner.as_str()).ok_or_else(|| {
            format!(
                "{PACKAGE_TIERS_RELATIVE} exception owner {} is not an approved package",
                exception.owner
            )
        })?;
        let dependency = placements
            .get(exception.dependency.as_str())
            .ok_or_else(|| {
                format!(
                    "{PACKAGE_TIERS_RELATIVE} exception dependency {} is not an approved package",
                    exception.dependency
                )
            })?;
        if dependency.rank <= owner.rank {
            return Err(format!(
                "{PACKAGE_TIERS_RELATIVE} exception {} -> {} does not describe an upward edge",
                exception.owner, exception.dependency
            ));
        }
        if !expected_kinds.contains(exception.kind.as_str()) {
            return Err(format!(
                "{PACKAGE_TIERS_RELATIVE} exception {} -> {} uses unknown dependency kind {}",
                exception.owner, exception.dependency, exception.kind
            ));
        }
        if exception.target.trim().is_empty() {
            return Err(format!(
                "{PACKAGE_TIERS_RELATIVE} exception {} -> {} must name its exact target",
                exception.owner, exception.dependency
            ));
        }
        let normalized_features = normalized_features(&exception.features);
        if normalized_features.len() != exception.features.len()
            || normalized_features.iter().ne(exception.features.iter())
        {
            return Err(format!(
                "{PACKAGE_TIERS_RELATIVE} exception {} -> {} features must be sorted and unique",
                exception.owner, exception.dependency
            ));
        }
        if exception.removal_step <= CURRENT_STEP {
            return Err(format!(
                "{PACKAGE_TIERS_RELATIVE} exception {} -> {} expired at Step {}",
                exception.owner, exception.dependency, exception.removal_step
            ));
        }
        if exception.rationale.trim().is_empty() {
            return Err(format!(
                "{PACKAGE_TIERS_RELATIVE} exception {} -> {} must include a rationale",
                exception.owner, exception.dependency
            ));
        }
        let key = (
            exception.owner.as_str(),
            exception.dependency.as_str(),
            exception.kind.as_str(),
            exception.target.as_str(),
            exception.features.clone(),
            exception.uses_default_features,
        );
        if !exception_keys.insert(key) {
            return Err(format!(
                "{PACKAGE_TIERS_RELATIVE} contains a duplicate exception for {} -> {}",
                exception.owner, exception.dependency
            ));
        }
    }

    Ok(placements)
}

fn unique_strings<'a>(label: &str, values: &'a [String]) -> Result<BTreeSet<&'a str>, String> {
    let unique = values.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        return Err(format!(
            "{PACKAGE_TIERS_RELATIVE} {label} must not contain duplicates"
        ));
    }
    Ok(unique)
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
    format!("{PACKAGE_TIERS_RELATIVE} package catalog mismatch; missing: {missing}; extra: {extra}")
}

fn load_metadata(workspace_root: &Path) -> Result<CargoMetadata, String> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--all-features",
        ])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed while checking dependency boundaries: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse cargo metadata for dependency boundaries: {error}"))
}

fn validate_metadata(
    policy: &DependencyBoundaryPolicy,
    placements: &BTreeMap<&str, PackagePlacement<'_>>,
    metadata: &CargoMetadata,
) -> Result<(), String> {
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| "cargo metadata did not include a resolved dependency graph".to_owned())?;
    let mut packages_by_id = BTreeMap::new();
    for package in &metadata.packages {
        if packages_by_id
            .insert(package.id.as_str(), package)
            .is_some()
        {
            return Err(format!(
                "cargo metadata contains duplicate package id {}",
                package.id
            ));
        }
    }

    let mut nodes_by_id = BTreeMap::new();
    for node in &resolve.nodes {
        if nodes_by_id.insert(node.id.as_str(), node).is_some() {
            return Err(format!(
                "cargo metadata contains duplicate resolve node {}",
                node.id
            ));
        }
    }

    let enforced_kinds = policy
        .enforced_dependency_kinds
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut violations = Vec::new();

    for node in &resolve.nodes {
        let Some(owner_package) = packages_by_id.get(node.id.as_str()).copied() else {
            return Err(format!(
                "cargo metadata resolve node {} has no package record",
                node.id
            ));
        };
        let Some(owner_placement) = placements.get(owner_package.name.as_str()).copied() else {
            continue;
        };

        for resolved_dependency in &node.deps {
            let dependency_package = packages_by_id
                .get(resolved_dependency.pkg.as_str())
                .copied()
                .ok_or_else(|| {
                    format!(
                        "cargo metadata resolved dependency {} has no package record",
                        resolved_dependency.pkg
                    )
                })?;
            let Some(dependency_placement) =
                placements.get(dependency_package.name.as_str()).copied()
            else {
                continue;
            };
            if resolved_dependency.dep_kinds.is_empty() {
                return Err(format!(
                    "cargo metadata edge {} -> {} has no dependency kind",
                    owner_package.name, dependency_package.name
                ));
            }

            for dependency_kind in &resolved_dependency.dep_kinds {
                let kind = normalized_kind(dependency_kind.kind.as_deref());
                if !enforced_kinds.contains(kind) {
                    continue;
                }
                let declaration =
                    resolve_declaration(owner_package, dependency_package, dependency_kind)?;
                if dependency_placement.rank <= owner_placement.rank {
                    continue;
                }
                if exception_matches(
                    &policy.temporary_exception,
                    owner_package,
                    dependency_package,
                    dependency_kind,
                    declaration,
                ) {
                    continue;
                }
                violations.push(format_violation(
                    owner_package,
                    owner_placement,
                    dependency_package,
                    dependency_placement,
                    resolved_dependency,
                    dependency_kind,
                    declaration,
                ));
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        violations.sort();
        Err(format!(
            "forbidden package dependency direction(s):\n{}",
            violations.join("\n")
        ))
    }
}

fn resolve_declaration<'a>(
    owner: &'a CargoPackage,
    dependency: &CargoPackage,
    dependency_kind: &CargoDependencyKind,
) -> Result<&'a CargoDependency, String> {
    let kind = normalized_kind(dependency_kind.kind.as_deref());
    let candidates = owner
        .dependencies
        .iter()
        .filter(|candidate| {
            candidate.name == dependency.name
                && normalized_kind(candidate.kind.as_deref()) == kind
                && candidate.target == dependency_kind.target
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [declaration] => Ok(*declaration),
        [] => Err(format!(
            "cargo metadata edge {} -> {} kind={kind} target={} has no matching package dependency declaration",
            owner.name,
            dependency.name,
            target_label(dependency_kind.target.as_deref())
        )),
        _ => Err(format!(
            "cargo metadata edge {} -> {} kind={kind} target={} has ambiguous package dependency declarations",
            owner.name,
            dependency.name,
            target_label(dependency_kind.target.as_deref())
        )),
    }
}

fn exception_matches(
    exceptions: &[TemporaryException],
    owner: &CargoPackage,
    dependency: &CargoPackage,
    dependency_kind: &CargoDependencyKind,
    declaration: &CargoDependency,
) -> bool {
    let kind = normalized_kind(dependency_kind.kind.as_deref());
    let target = target_label(dependency_kind.target.as_deref());
    let features = normalized_features(&declaration.features);
    exceptions.iter().any(|exception| {
        exception.owner == owner.name
            && exception.dependency == dependency.name
            && exception.kind == kind
            && exception.target == target
            && exception.features == features
            && exception.uses_default_features == declaration.uses_default_features
    })
}

fn format_violation(
    owner: &CargoPackage,
    owner_placement: PackagePlacement<'_>,
    dependency: &CargoPackage,
    dependency_placement: PackagePlacement<'_>,
    resolved_dependency: &CargoNodeDependency,
    dependency_kind: &CargoDependencyKind,
    declaration: &CargoDependency,
) -> String {
    let features = normalized_features(&declaration.features);
    format!(
        "{} (tier={} rank={}) -> {} (tier={} rank={}); owner_id={}; dependency_id={}; alias={}; kind={}; target={}; features=[{}]; default_features={}",
        owner.name,
        owner_placement.tier,
        owner_placement.rank,
        dependency.name,
        dependency_placement.tier,
        dependency_placement.rank,
        owner.id,
        dependency.id,
        resolved_dependency.name,
        normalized_kind(dependency_kind.kind.as_deref()),
        target_label(dependency_kind.target.as_deref()),
        features.join(","),
        declaration.uses_default_features,
    )
}

fn normalized_kind(kind: Option<&str>) -> &str {
    kind.unwrap_or("normal")
}

fn target_label(target: Option<&str>) -> &str {
    target.unwrap_or("all")
}

fn normalized_features(features: &[String]) -> Vec<String> {
    features
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        CargoMetadata, DependencyBoundaryPolicy, exception_matches, validate_metadata,
        validate_policy,
    };
    use serde::Deserialize;
    use std::collections::BTreeSet;

    const POLICY: &str = include_str!("../../../../contracts/releases/package_tiers.toml");
    const ARCHITECTURE: &str =
        include_str!("../../../../docs/specs/radroots_crates_release_v1.toml");
    const DOMAIN_TO_STORAGE: &str =
        include_str!("../../tests/fixtures/dependency-boundaries/domain-to-storage.json");
    const SPI_TO_ADAPTER: &str =
        include_str!("../../tests/fixtures/dependency-boundaries/spi-to-adapter.json");
    const SERVICE_TO_SDK: &str =
        include_str!("../../tests/fixtures/dependency-boundaries/service-to-sdk.json");
    const VALID_DOWNWARD: &str =
        include_str!("../../tests/fixtures/dependency-boundaries/valid-downward.json");

    #[derive(Deserialize)]
    struct ArchitectureCatalog {
        package: Vec<ArchitecturePackage>,
    }

    #[derive(Deserialize)]
    struct ArchitecturePackage {
        name: String,
    }

    fn policy() -> DependencyBoundaryPolicy {
        toml::from_str(POLICY).expect("package tier policy")
    }

    fn metadata(raw: &str) -> CargoMetadata {
        serde_json::from_str(raw).expect("Cargo metadata fixture")
    }

    fn placements<'a>(
        policy: &'a DependencyBoundaryPolicy,
    ) -> std::collections::BTreeMap<&'a str, super::PackagePlacement<'a>> {
        let architecture =
            toml::from_str::<ArchitectureCatalog>(ARCHITECTURE).expect("architecture catalog");
        let expected = architecture
            .package
            .into_iter()
            .map(|package| package.name)
            .collect::<BTreeSet<_>>();
        validate_policy(policy, &expected).expect("valid package tier policy")
    }

    #[test]
    fn policy_covers_the_exact_architecture_catalog() {
        let policy = policy();
        let placements = placements(&policy);
        assert_eq!(placements.len(), 19);
    }

    #[test]
    fn domain_to_storage_reports_resolved_alias_kind_target_and_features() {
        let policy = policy();
        let error = validate_metadata(&policy, &placements(&policy), &metadata(DOMAIN_TO_STORAGE))
            .expect_err("domain must not depend on storage");
        assert!(error.contains("radroots_event (tier=domain rank=1)"));
        assert!(error.contains("radroots_storage (tier=spi rank=2)"));
        assert!(error.contains("alias=storage_alias"));
        assert!(error.contains("kind=normal"));
        assert!(error.contains("target=cfg(unix)"));
        assert!(error.contains("features=[memory]"));
    }

    #[test]
    fn spi_to_adapter_reports_build_and_target_specific_edge() {
        let policy = policy();
        let error = validate_metadata(&policy, &placements(&policy), &metadata(SPI_TO_ADAPTER))
            .expect_err("SPI must not depend on adapter");
        assert!(error.contains("radroots_transport (tier=spi rank=2)"));
        assert!(error.contains("radroots_nostr (tier=adapter rank=3)"));
        assert!(error.contains("alias=protocol_adapter"));
        assert!(error.contains("kind=build"));
        assert!(error.contains("target=cfg(target_arch = \"wasm32\")"));
        assert!(error.contains("features=[events]"));
    }

    #[test]
    fn service_orchestration_to_sdk_is_forbidden() {
        let policy = policy();
        let error = validate_metadata(&policy, &placements(&policy), &metadata(SERVICE_TO_SDK))
            .expect_err("service orchestration must not depend on SDK");
        assert!(error.contains("radroots_sync (tier=orchestration rank=4)"));
        assert!(error.contains("radroots_sdk (tier=sdk rank=5)"));
        assert!(error.contains("alias=client_engine"));
    }

    #[test]
    fn valid_downward_dev_target_edge_passes() {
        let policy = policy();
        validate_metadata(&policy, &placements(&policy), &metadata(VALID_DOWNWARD))
            .expect("adapter dev dependency on SPI points downward");
    }

    #[test]
    fn temporary_exception_is_exact_about_features() {
        let policy = policy();
        assert!(
            policy.temporary_exception.is_empty(),
            "expired dependency exceptions must not remain in production policy"
        );
        let exceptions = [super::TemporaryException {
            owner: "radroots_trade".to_owned(),
            dependency: "radroots_nostr".to_owned(),
            kind: "dev".to_owned(),
            target: "all".to_owned(),
            features: vec!["events".to_owned(), "std".to_owned()],
            uses_default_features: false,
            removal_step: 88,
            rationale: "synthetic exact-match fixture".to_owned(),
        }];
        let owner = super::CargoPackage {
            id: "owner".to_owned(),
            name: "radroots_trade".to_owned(),
            dependencies: Vec::new(),
        };
        let dependency = super::CargoPackage {
            id: "dependency".to_owned(),
            name: "radroots_nostr".to_owned(),
            dependencies: Vec::new(),
        };
        let kind = super::CargoDependencyKind {
            kind: Some("dev".to_owned()),
            target: None,
        };
        let exact = super::CargoDependency {
            name: "radroots_nostr".to_owned(),
            kind: Some("dev".to_owned()),
            features: vec!["std".to_owned(), "events".to_owned()],
            target: None,
            uses_default_features: false,
        };
        assert!(exception_matches(
            &exceptions,
            &owner,
            &dependency,
            &kind,
            &exact
        ));

        let broadened = super::CargoDependency {
            features: vec!["client".to_owned(), "events".to_owned(), "std".to_owned()],
            ..exact
        };
        assert!(!exception_matches(
            &exceptions,
            &owner,
            &dependency,
            &kind,
            &broadened
        ));
    }

    #[test]
    fn metadata_without_resolve_graph_fails_closed() {
        let policy = policy();
        let mut fixture = metadata(VALID_DOWNWARD);
        fixture.resolve = None;
        let error = validate_metadata(&policy, &placements(&policy), &fixture)
            .expect_err("missing resolved graph must fail");
        assert!(error.contains("did not include a resolved dependency graph"));
    }
}

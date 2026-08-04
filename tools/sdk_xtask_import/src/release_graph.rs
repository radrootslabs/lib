use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

use serde::Deserialize;

const ARCHITECTURE_RELATIVE: &str = "docs/specs/radroots_crates_release_v1.toml";
const SPEC_ID: &str = "radroots.crates.release.v1";
const RELEASE_VERSION: &str = "0.1.0-alpha";
const REGISTRY_KINDS: &[&str] = &["build", "normal"];
const ALL_KINDS: &[&str] = &["build", "dev", "normal"];

#[derive(Debug, Deserialize)]
struct Architecture {
    spec_id: String,
    package_count: usize,
    package: Vec<ArchitecturePackage>,
}

#[derive(Debug, Deserialize)]
struct ArchitecturePackage {
    name: String,
    publish_order_hint: usize,
    #[serde(default)]
    required_radroots_dependencies: Vec<String>,
    #[serde(default)]
    optional_radroots_dependencies: Vec<String>,
    #[serde(flatten)]
    _other: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
    resolve: Option<CargoResolve>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    version: String,
    source: Option<String>,
    manifest_path: String,
    #[serde(default)]
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Deserialize)]
struct CargoDependency {
    name: String,
    rename: Option<String>,
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

fn default_true() -> bool {
    true
}

pub fn run(workspace_root: &Path) -> Result<(), String> {
    let architecture = load_architecture(workspace_root)?;
    let metadata = load_metadata(workspace_root)?;
    let order = validate(&architecture, &metadata)?;
    println!("resolved publication order:");
    for (index, package) in order.iter().enumerate() {
        println!("{:02} {package}", index + 1);
    }
    Ok(())
}

fn load_architecture(workspace_root: &Path) -> Result<Architecture, String> {
    let path = workspace_root.join(ARCHITECTURE_RELATIVE);
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("parse {}: {error}", path.display()))
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
            "cargo metadata failed while resolving the release graph: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse cargo metadata release graph: {error}"))
}

fn validate(architecture: &Architecture, metadata: &CargoMetadata) -> Result<Vec<String>, String> {
    let hints = validate_architecture(architecture)?;
    let approved = hints.keys().copied().collect::<BTreeSet<_>>();
    let packages = unique_packages(metadata)?;
    let nodes = unique_nodes(metadata)?;
    let workspace_members = metadata
        .workspace_members
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let facade_is_local = metadata.workspace_members.iter().any(|id| {
        packages
            .get(id.as_str())
            .is_some_and(|package| package.name == "radroots")
    });

    let mut selected = BTreeSet::new();
    for name in &approved {
        let matching = packages
            .values()
            .filter(|package| package.name == **name)
            .copied()
            .collect::<Vec<_>>();
        if matching.len() > 1 {
            return Err(format!(
                "release package {name} resolves to multiple package identities"
            ));
        }
        let Some(package) = matching.first().copied() else {
            if facade_is_local {
                return Err(format!(
                    "complete SDK release graph is missing approved package {name}"
                ));
            }
            continue;
        };
        if facade_is_local || workspace_members.contains(package.id.as_str()) {
            selected.insert(*name);
        }
        if package.version != RELEASE_VERSION {
            return Err(format!(
                "release package {name} resolved version {} instead of {RELEASE_VERSION}",
                package.version
            ));
        }
        if package.source.is_some() {
            return Err(format!(
                "release package {name} must resolve from the staged source graph; source={:?}",
                package.source
            ));
        }
        if !nodes.contains_key(package.id.as_str()) {
            return Err(format!(
                "release package {name} has no Cargo resolve node; manifest={}",
                package.manifest_path
            ));
        }
    }
    if selected.is_empty() {
        return Err("release graph selected no approved workspace packages".to_owned());
    }
    if facade_is_local && selected.len() != architecture.package_count {
        return Err(format!(
            "complete SDK release graph selected {} packages instead of {}",
            selected.len(),
            architecture.package_count
        ));
    }

    let mut dependency_sets = selected
        .iter()
        .map(|name| (*name, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut violations = Vec::new();

    for owner_name in &selected {
        let owner = packages
            .values()
            .find(|package| package.name == **owner_name)
            .copied()
            .ok_or_else(|| format!("selected release package {owner_name} disappeared"))?;
        let node = nodes
            .get(owner.id.as_str())
            .copied()
            .ok_or_else(|| format!("release package {owner_name} has no resolve node"))?;
        for edge in &node.deps {
            let dependency = packages
                .get(edge.pkg.as_str())
                .copied()
                .ok_or_else(|| format!("resolved dependency {} has no package record", edge.pkg))?;
            if edge.dep_kinds.is_empty() {
                return Err(format!(
                    "resolved edge {} -> {} has no dependency kind",
                    owner.name, dependency.name
                ));
            }
            for edge_kind in &edge.dep_kinds {
                let kind = normalized_kind(edge_kind.kind.as_deref());
                if !ALL_KINDS.contains(&kind) {
                    return Err(format!(
                        "resolved edge {} -> {} uses unsupported dependency kind {kind}",
                        owner.name, dependency.name
                    ));
                }
                let declaration = resolve_declaration(owner, dependency, edge_kind)?;
                if is_radroots_family(&dependency.name)
                    && !approved.contains(dependency.name.as_str())
                {
                    violations.push(format_edge(
                        owner,
                        dependency,
                        edge,
                        edge_kind,
                        declaration,
                        "public-to-private edge",
                    ));
                }
                if REGISTRY_KINDS.contains(&kind) && selected.contains(dependency.name.as_str()) {
                    dependency_sets
                        .get_mut(owner_name)
                        .expect("selected owner dependency set")
                        .insert(dependency.name.as_str());
                }
            }
        }
    }
    if !violations.is_empty() {
        violations.sort();
        return Err(format!(
            "release graph contains forbidden edge(s):\n{}",
            violations.join("\n")
        ));
    }

    validate_declared_architecture_edges(architecture, &selected, &dependency_sets)?;
    let order = dependency_first_order(&selected, &dependency_sets, &hints)?;
    let expected = selected.iter().copied().collect::<Vec<_>>();
    let mut expected = expected;
    expected.sort_by_key(|name| hints[name]);
    if order != expected {
        return Err(format!(
            "Cargo-derived publication order [{}] differs from architecture order [{}]",
            order.join(", "),
            expected.join(", ")
        ));
    }
    Ok(order.into_iter().map(str::to_owned).collect())
}

fn validate_architecture(architecture: &Architecture) -> Result<BTreeMap<&str, usize>, String> {
    if architecture.spec_id != SPEC_ID {
        return Err(format!("architecture spec_id must be {SPEC_ID}"));
    }
    if architecture.package_count != 19 || architecture.package.len() != 19 {
        return Err("architecture must define exactly 19 release packages".to_owned());
    }
    let mut hints = BTreeMap::new();
    let mut seen_hints = BTreeSet::new();
    for package in &architecture.package {
        if hints
            .insert(package.name.as_str(), package.publish_order_hint)
            .is_some()
        {
            return Err(format!("duplicate architecture package {}", package.name));
        }
        if !seen_hints.insert(package.publish_order_hint) {
            return Err(format!(
                "duplicate publish_order_hint {}",
                package.publish_order_hint
            ));
        }
    }
    if seen_hints != (1..=19).collect::<BTreeSet<_>>() {
        return Err("publish_order_hint values must be the exact range 1..=19".to_owned());
    }
    Ok(hints)
}

fn unique_packages(metadata: &CargoMetadata) -> Result<BTreeMap<&str, &CargoPackage>, String> {
    let mut packages = BTreeMap::new();
    for package in &metadata.packages {
        if packages.insert(package.id.as_str(), package).is_some() {
            return Err(format!("duplicate Cargo package id {}", package.id));
        }
    }
    Ok(packages)
}

fn unique_nodes(metadata: &CargoMetadata) -> Result<BTreeMap<&str, &CargoNode>, String> {
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| "cargo metadata did not include a resolved release graph".to_owned())?;
    let mut nodes = BTreeMap::new();
    for node in &resolve.nodes {
        if nodes.insert(node.id.as_str(), node).is_some() {
            return Err(format!("duplicate Cargo resolve node {}", node.id));
        }
    }
    Ok(nodes)
}

fn resolve_declaration<'a>(
    owner: &'a CargoPackage,
    dependency: &CargoPackage,
    edge_kind: &CargoDependencyKind,
) -> Result<&'a CargoDependency, String> {
    let kind = normalized_kind(edge_kind.kind.as_deref());
    let candidates = owner
        .dependencies
        .iter()
        .filter(|candidate| {
            candidate.name == dependency.name
                && normalized_kind(candidate.kind.as_deref()) == kind
                && candidate.target == edge_kind.target
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [declaration] => Ok(*declaration),
        [] => Err(format!(
            "resolved edge {} -> {} kind={kind} target={} has no matching declaration",
            owner.name,
            dependency.name,
            target_label(edge_kind.target.as_deref())
        )),
        _ => Err(format!(
            "resolved edge {} -> {} kind={kind} target={} has ambiguous declarations",
            owner.name,
            dependency.name,
            target_label(edge_kind.target.as_deref())
        )),
    }
}

fn format_edge(
    owner: &CargoPackage,
    dependency: &CargoPackage,
    edge: &CargoNodeDependency,
    edge_kind: &CargoDependencyKind,
    declaration: &CargoDependency,
    reason: &str,
) -> String {
    let features = declaration
        .features
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{reason}: {} -> {}; alias={}; declaration_alias={}; kind={}; target={}; features=[{}]; default_features={}; owner_manifest={}; dependency_manifest={}",
        owner.name,
        dependency.name,
        edge.name,
        declaration.rename.as_deref().unwrap_or(&declaration.name),
        normalized_kind(edge_kind.kind.as_deref()),
        target_label(edge_kind.target.as_deref()),
        features,
        declaration.uses_default_features,
        owner.manifest_path,
        dependency.manifest_path,
    )
}

fn validate_declared_architecture_edges<'a>(
    architecture: &'a Architecture,
    selected: &BTreeSet<&'a str>,
    resolved: &BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> Result<(), String> {
    for package in &architecture.package {
        if !selected.contains(package.name.as_str()) {
            continue;
        }
        for optional in &package.optional_radroots_dependencies {
            if !selected.contains(optional.as_str()) {
                return Err(format!(
                    "{} optional architecture dependency {optional} is outside the selected public graph",
                    package.name
                ));
            }
        }
        let declared = package
            .required_radroots_dependencies
            .iter()
            .map(String::as_str)
            .filter(|dependency| selected.contains(dependency))
            .collect::<BTreeSet<_>>();
        let actual = resolved
            .get(package.name.as_str())
            .ok_or_else(|| format!("missing resolved dependency set for {}", package.name))?;
        if !declared.is_subset(actual) {
            let missing = declared.difference(actual).copied().collect::<Vec<_>>();
            return Err(format!(
                "{} architecture dependency declarations are absent from the all-feature Cargo graph: {}",
                package.name,
                missing.join(", ")
            ));
        }
    }
    Ok(())
}

fn dependency_first_order<'a>(
    selected: &BTreeSet<&'a str>,
    dependency_sets: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    hints: &BTreeMap<&'a str, usize>,
) -> Result<Vec<&'a str>, String> {
    let mut remaining = dependency_sets.clone();
    let mut order = Vec::with_capacity(selected.len());
    while !remaining.is_empty() {
        let next = remaining
            .iter()
            .filter(|(_, dependencies)| dependencies.iter().all(|name| order.contains(name)))
            .map(|(name, _)| *name)
            .min_by_key(|name| hints[name]);
        let Some(next) = next else {
            let blocked = remaining
                .iter()
                .map(|(name, dependencies)| {
                    format!(
                        "{name}->[{}]",
                        dependencies.iter().copied().collect::<Vec<_>>().join(",")
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!(
                "release graph contains a publication cycle: {blocked}"
            ));
        };
        remaining.remove(next);
        order.push(next);
    }
    Ok(order)
}

fn is_radroots_family(name: &str) -> bool {
    name == "radroots" || name.starts_with("radroots_")
}

fn normalized_kind(kind: Option<&str>) -> &str {
    kind.unwrap_or("normal")
}

fn target_label(target: Option<&str>) -> &str {
    target.unwrap_or("all")
}

#[cfg(test)]
mod tests {
    use super::{Architecture, CargoMetadata, validate};

    const ARCHITECTURE: &str = include_str!("../../../docs/specs/radroots_crates_release_v1.toml");

    #[test]
    fn current_architecture_contract_has_a_complete_order() {
        let architecture = toml::from_str::<Architecture>(ARCHITECTURE).expect("architecture");
        let hints = super::validate_architecture(&architecture).expect("valid order");
        assert_eq!(hints.len(), 19);
        assert_eq!(hints["radroots_core"], 1);
        assert_eq!(hints["radroots"], 19);
    }

    #[test]
    fn metadata_without_resolve_graph_fails_closed() {
        let architecture = toml::from_str::<Architecture>(ARCHITECTURE).expect("architecture");
        let metadata: CargoMetadata =
            serde_json::from_str(r#"{"packages":[],"workspace_members":[],"resolve":null}"#)
                .expect("metadata");
        let error = validate(&architecture, &metadata).expect_err("missing graph");
        assert!(
            error.contains("selected no approved workspace packages")
                || error.contains("resolved release graph")
        );
    }

    #[test]
    fn public_to_private_target_edge_reports_complete_provenance() {
        let architecture = toml::from_str::<Architecture>(ARCHITECTURE).expect("architecture");
        let metadata: CargoMetadata = serde_json::from_str(
            r#"{
              "packages": [
                {
                  "id": "core",
                  "name": "radroots_core",
                  "version": "0.1.0-alpha",
                  "source": null,
                  "manifest_path": "/capsule/crates/core/Cargo.toml",
                  "dependencies": [{
                    "name": "radroots_private_runtime",
                    "rename": "private_alias",
                    "kind": "build",
                    "features": ["danger"],
                    "target": "cfg(unix)",
                    "uses_default_features": false
                  }]
                },
                {
                  "id": "private",
                  "name": "radroots_private_runtime",
                  "version": "0.1.0-alpha",
                  "source": null,
                  "manifest_path": "/capsule/crates/private/Cargo.toml",
                  "dependencies": []
                }
              ],
              "workspace_members": ["core"],
              "resolve": {"nodes": [
                {"id": "core", "deps": [{
                  "name": "private_alias",
                  "pkg": "private",
                  "dep_kinds": [{"kind": "build", "target": "cfg(unix)"}]
                }]},
                {"id": "private", "deps": []}
              ]}
            }"#,
        )
        .expect("metadata");
        let error = validate(&architecture, &metadata).expect_err("private edge");
        assert!(error.contains("public-to-private edge"));
        assert!(error.contains("alias=private_alias"));
        assert!(error.contains("kind=build"));
        assert!(error.contains("target=cfg(unix)"));
        assert!(error.contains("features=[danger]"));
        assert!(error.contains("default_features=false"));
    }

    #[test]
    fn registry_graph_cycle_fails_closed() {
        let architecture = toml::from_str::<Architecture>(ARCHITECTURE).expect("architecture");
        let metadata: CargoMetadata = serde_json::from_str(
            r#"{
              "packages": [
                {"id":"core","name":"radroots_core","version":"0.1.0-alpha","source":null,"manifest_path":"/core/Cargo.toml","dependencies":[{"name":"radroots_identity","rename":null,"kind":null,"features":[],"target":null,"uses_default_features":true}]},
                {"id":"identity","name":"radroots_identity","version":"0.1.0-alpha","source":null,"manifest_path":"/identity/Cargo.toml","dependencies":[{"name":"radroots_core","rename":null,"kind":null,"features":[],"target":null,"uses_default_features":true}]}
              ],
              "workspace_members":["core","identity"],
              "resolve":{"nodes":[
                {"id":"core","deps":[{"name":"radroots_identity","pkg":"identity","dep_kinds":[{"kind":null,"target":null}]}]},
                {"id":"identity","deps":[{"name":"radroots_core","pkg":"core","dep_kinds":[{"kind":null,"target":null}]}]}
              ]}
            }"#,
        )
        .expect("metadata");
        let error = validate(&architecture, &metadata).expect_err("cycle");
        assert!(error.contains("publication cycle"));
        assert!(error.contains("radroots_core"));
        assert!(error.contains("radroots_identity"));
    }
}

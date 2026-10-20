use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const WORKSPACE: &str = include_str!("../../../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const README: &str = include_str!("../README.md");

#[test]
fn package_has_final_identity_and_exact_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots\""));
    assert!(MANIFEST.contains("publish = [\"crates-io\"]"));
    assert_eq!(
        dependency_names(MANIFEST),
        BTreeSet::from([
            "futures",
            "radroots_core",
            "radroots_domain",
            "radroots_event",
            "radroots_execution",
            "radroots_identity",
            "radroots_nostr",
            "radroots_protocol",
            "radroots_sdk",
            "radroots_signing",
            "radroots_storage",
            "radroots_storage_sqlite",
            "radroots_trade",
            "radroots_transport",
            "radroots_transport_nostr",
            "radroots_transport_nostr_native",
            "tokio",
        ])
    );
    assert!(WORKSPACE.contains(
        "radroots_sdk = { path = \"crates/sdk\", version = \"=0.1.0-alpha\", default-features = false }"
    ));
}

#[test]
fn features_forward_the_exact_user_oriented_graph() {
    for forwarding in [
        "default = [\"client\"]",
        "client = [\"radroots_sdk/default\", \"radroots_storage/memory\"]",
        "native = [\"client\", \"radroots_sdk/native\", \"dep:radroots_storage_sqlite\", \"dep:futures\", \"dep:tokio\", \"dep:radroots_nostr\", \"dep:radroots_transport_nostr\", \"dep:radroots_transport_nostr_native\"]",
        "nostr = [\"client\", \"radroots_sdk/nostr\"]",
        "nip46 = [\"nostr\", \"radroots_sdk/nip46\"]",
        "radrootsd = [\"client\", \"radroots_sdk/radrootsd\"]",
        "geonames = [\"client\", \"radroots_sdk/geonames\"]",
        "knowledge = [\"client\", \"radroots_sdk/knowledge\"]",
        "full = [\"native\", \"radroots_sdk/full\"]",
    ] {
        assert!(MANIFEST.contains(forwarding), "missing `{forwarding}`");
    }
    assert!(feature_guard(MANIFEST).is_ok());
}

#[test]
fn root_has_exact_exports_and_no_sdk_namespace() {
    assert!(ROOT.contains("pub use client::{Client, CloseReceipt, SessionStatus, StartMode};"));
    assert!(ROOT.contains("pub use radroots_execution::{Error, Result};"));
    assert!(!ROOT.contains("ClientBuilder"));
    assert!(!ROOT.contains("pub use radroots_sdk::{Client"));
    assert!(!ROOT.contains("pub mod sdk"));
    assert!(!ROOT.contains("pub use radroots_sdk::*"));
    assert!(ROOT.contains("pub use radroots_domain::OperationId;"));
    assert!(
        ROOT.contains("pub use radroots_execution::workspace::{CommandContext, WorkspaceSession};")
    );
    assert!(
        ROOT.contains("pub use radroots_storage::workspace::{AttachmentInput, AttachmentProof};")
    );
    assert!(ROOT.contains(
        "pub use radroots_execution::{\n    ArtifactObservation, BusinessInstallation, Delivery, DeliveryObservation, Issuance,\n    OperationKind, OperationReceipt, OperationStatus, OperationValue, RecoveryRequirement,\n    TargetObservation, WaitTermination,\n};"
    ));
    assert!(ROOT.contains("pub mod listings;"));
    assert_eq!(ROOT.matches("pub use ").count(), 7);
    assert!(README.contains("```compile_fail\nuse radroots::sdk;\n```"));
}

#[test]
fn modules_use_deliberate_reexports_without_wildcards() {
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(source_root).expect("read facade source") {
        let path = entry.expect("read facade source entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read facade module");
        assert!(
            !source.contains("::*"),
            "{} contains a wildcard reexport",
            path.display()
        );
        assert!(
            !source.contains("pub mod sdk"),
            "{} exposes the forbidden SDK namespace",
            path.display()
        );
    }
}

fn dependency_names(manifest: &str) -> BTreeSet<&str> {
    manifest
        .split_once("[dependencies]")
        .expect("dependency section")
        .1
        .split_once("\n[")
        .expect("section after dependencies")
        .0
        .lines()
        .filter_map(|line| line.split_once(" = ").map(|(name, _)| name.trim()))
        .collect()
}

fn feature_guard(manifest: &str) -> Result<(), &'static str> {
    if manifest.len() > 65_536 {
        return Err("manifest byte limit exceeded");
    }
    let value: toml::Value = toml::from_str(manifest).map_err(|_| "invalid manifest")?;
    let dependencies = value
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or("missing dependency table")?;
    let features = value
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or("missing feature table")?;
    let features = features
        .iter()
        .map(|(name, values)| {
            let values = values.as_array().ok_or("feature must be an array")?;
            let tokens = values
                .iter()
                .map(toml::Value::as_str)
                .collect::<Option<Vec<_>>>()
                .ok_or("feature values must be strings")?;
            Ok((name.as_str(), tokens))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, &'static str>>()?;
    if features.contains_key("sp1") {
        return Err("SP1 remains a separate capability");
    }

    let policy: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/architecture/decisions/shared_sdk_refactor.v1.json"
    ))
    .map_err(|_| "invalid approved target contract")?;
    let mut approved = policy
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing approved package graph")?
        .iter()
        .map(|p| p.get("package").and_then(serde_json::Value::as_str))
        .collect::<Option<BTreeSet<_>>>()
        .ok_or("invalid approved package identity")?;
    let advanced = BTreeSet::from([
        "radroots_transport_reticulum",
        "radroots_mesh",
        "radroots_simplex_chat_proto",
        "radroots_nostrdb",
        "radroots_replica_sync",
    ]);
    if !advanced.is_subset(&approved) {
        return Err("advanced owners must exist in approved graph");
    }
    // The original exact-dependency test still governs the current base. New
    // optional owners must come from the prospective target, not arbitrary text.
    let base: toml::Value = toml::from_str(MANIFEST).map_err(|_| "invalid base manifest")?;
    approved.extend(
        base.get("dependencies")
            .and_then(toml::Value::as_table)
            .ok_or("missing base dependencies")?
            .keys()
            .map(String::as_str),
    );
    let mut advanced_aliases = BTreeSet::new();
    for (alias, declaration) in dependencies {
        let table = declaration.as_table();
        if table.is_none() && declaration.as_str().is_none() {
            return Err("invalid dependency shape");
        }
        let package = match table.and_then(|t| t.get("package")) {
            Some(p) => p.as_str().ok_or("invalid package alias")?,
            None => alias.as_str(),
        };
        if !approved.contains(package) || ["xtask", "radroots_test_fixtures"].contains(&package) {
            return Err("unapproved runtime dependency owner");
        }
        for name in [alias.as_str(), package] {
            if ["radroots_ee_", "radroots_ops_", "radroots_platform_"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
                || (name.starts_with("radroots_") && !approved.contains(name))
            {
                return Err("unapproved package owner");
            }
            if name.starts_with("sp1") {
                return Err("SP1 remains a separate capability");
            }
        }
        if advanced.contains(package) {
            if table
                .and_then(|t| t.get("optional"))
                .and_then(toml::Value::as_bool)
                != Some(true)
            {
                return Err("advanced dependency must be optional");
            }
            let explicit = format!("dep:{alias}");
            if !features
                .values()
                .any(|tokens| tokens.iter().any(|token| *token == explicit))
            {
                return Err("advanced dependency needs explicit activation");
            }
            advanced_aliases.insert(alias.as_str());
        }
    }
    for tokens in features.values() {
        for token in tokens {
            if let Some(alias) = token.strip_prefix("dep:")
                && !dependencies.contains_key(alias)
            {
                return Err("feature activates an undeclared dependency");
            }
        }
    }
    let mut pending = features.get("default").cloned().unwrap_or_default();
    let mut visited = BTreeSet::new();
    while let Some(token) = pending.pop() {
        let (dependency, activates) = if let Some(alias) = token.strip_prefix("dep:") {
            (alias, true)
        } else if let Some((alias, _)) = token.split_once('/') {
            (alias.trim_end_matches('?'), !alias.ends_with('?'))
        } else {
            (token, true)
        };
        if activates && advanced_aliases.contains(dependency) {
            return Err("advanced dependency cannot be enabled by default");
        }
        if visited.insert(token)
            && let Some(tokens) = features.get(token)
        {
            pending.extend(tokens);
        }
    }
    Ok(())
}

fn advanced_fixture(feature: &str, package: &str, optional: bool) -> String {
    let mut value: toml::Value = toml::from_str(MANIFEST).unwrap();
    let dependency = toml::Table::from_iter([
        ("workspace".to_owned(), toml::Value::Boolean(true)),
        ("optional".to_owned(), toml::Value::Boolean(optional)),
    ]);
    value["dependencies"]
        .as_table_mut()
        .unwrap()
        .insert(package.to_owned(), toml::Value::Table(dependency));
    value["features"].as_table_mut().unwrap().insert(
        feature.to_owned(),
        toml::Value::Array(vec![toml::Value::String(format!("dep:{package}"))]),
    );
    let text = toml::to_string(&value).unwrap();
    assert!(text.len() < 65_536);
    text
}

#[test]
fn approved_optional_reticulum_is_not_rejected_by_vocabulary() {
    assert!(
        feature_guard(&advanced_fixture(
            "reticulum",
            "radroots_transport_reticulum",
            true
        ))
        .is_ok()
    );
}

#[test]
fn approved_optional_mesh_is_not_rejected_by_vocabulary() {
    assert!(feature_guard(&advanced_fixture("mesh", "radroots_mesh", true)).is_ok());
}

#[test]
fn approved_optional_simplex_is_not_rejected_by_vocabulary() {
    assert!(
        feature_guard(&advanced_fixture(
            "simplex",
            "radroots_simplex_chat_proto",
            true
        ))
        .is_ok()
    );
}

#[test]
fn approved_optional_nostrdb_is_not_rejected_by_vocabulary() {
    assert!(feature_guard(&advanced_fixture("nostrdb", "radroots_nostrdb", true)).is_ok());
}

#[test]
fn approved_optional_replica_is_not_rejected_by_vocabulary() {
    assert!(feature_guard(&advanced_fixture("replica", "radroots_replica_sync", true)).is_ok());
}

#[test]
fn advanced_dependency_must_be_optional() {
    assert!(feature_guard(&advanced_fixture("mesh", "radroots_mesh", false)).is_err());
}

#[test]
fn advanced_dependency_needs_explicit_activation() {
    let mut value: toml::Value =
        toml::from_str(&advanced_fixture("mesh", "radroots_mesh", true)).unwrap();
    value["features"].as_table_mut().unwrap().remove("mesh");
    assert!(feature_guard(&toml::to_string(&value).unwrap()).is_err());
}

#[test]
fn advanced_dependency_cannot_be_enabled_by_default() {
    for route in ["direct", "feature", "transitive_feature"] {
        let mut value: toml::Value =
            toml::from_str(&advanced_fixture("mesh", "radroots_mesh", true)).unwrap();
        let token = match route {
            "direct" => "dep:radroots_mesh",
            "feature" => "mesh",
            _ => {
                value["features"].as_table_mut().unwrap().insert(
                    "feature_hop".to_owned(),
                    toml::Value::Array(vec![toml::Value::String("mesh".to_owned())]),
                );
                "feature_hop"
            }
        };
        value["features"]["default"]
            .as_array_mut()
            .unwrap()
            .push(toml::Value::String(token.to_owned()));
        assert!(feature_guard(&toml::to_string(&value).unwrap()).is_err());
    }
}

#[test]
fn private_package_cannot_hide_behind_a_feature_alias() {
    assert!(
        feature_guard(&advanced_fixture(
            "innocent_feature",
            "radroots_ee_hidden",
            true
        ))
        .is_err()
    );
    let mut value: toml::Value = toml::from_str(&advanced_fixture(
        "innocent_feature",
        "ordinary_alias",
        true,
    ))
    .unwrap();
    value["dependencies"]["ordinary_alias"]
        .as_table_mut()
        .unwrap()
        .insert(
            "package".to_owned(),
            toml::Value::String("radroots_ee_hidden".to_owned()),
        );
    assert!(feature_guard(&toml::to_string(&value).unwrap()).is_err());
}

#[test]
fn malformed_or_wrong_shape_manifest_cannot_pass_as_harmless_text() {
    for value in ["not TOML input", "a = 1", "[features]\ndefault = 7\n"] {
        assert!(feature_guard(value).is_err());
    }
}

#[test]
fn oversized_manifest_is_rejected_before_parsing() {
    assert!(feature_guard(&" ".repeat(65_537)).is_err());
}

#[test]
fn sp1_remains_separate_from_the_ordinary_facade() {
    assert!(feature_guard(&advanced_fixture("sp1", "sp1_sdk", true)).is_err());
}

#[test]
fn only_approved_dependency_owners_are_admitted() {
    for package in ["unapproved_owner", "xtask", "radroots_test_fixtures"] {
        assert!(feature_guard(&advanced_fixture("innocent_feature", package, true)).is_err());
    }
}

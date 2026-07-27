use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Attribute, Meta, Token};

const NOSTR_MANIFEST_RELATIVE: &str = "crates/nostr/Cargo.toml";
const NOSTR_LIB_RELATIVE: &str = "crates/nostr/src/lib.rs";
const PHASE1_FEATURE_MATRIX_RELATIVE: &str = "contracts/phase1_feature_matrix.toml";
const WASM_TARGET: &str = "wasm32-unknown-unknown";
const REQUIRED_CATEGORIES: &[FeatureCategory] = &[
    FeatureCategory::AllFeatures,
    FeatureCategory::Minimal,
    FeatureCategory::Raster,
    FeatureCategory::Runtime,
    FeatureCategory::Serde,
    FeatureCategory::Signature,
    FeatureCategory::Sqlite,
    FeatureCategory::Std,
    FeatureCategory::Wasm,
];
const AFFECTED_PACKAGES: &[(&str, &str)] = &[
    ("radroots_authority", "crates/authority/Cargo.toml"),
    ("radroots_blossom", "crates/blossom/Cargo.toml"),
    ("radroots_event", "crates/event/Cargo.toml"),
    ("radroots_event_codec", "crates/event_codec/Cargo.toml"),
    ("radroots_event_store", "crates/event_store/Cargo.toml"),
    ("radroots_nostr", "crates/nostr/Cargo.toml"),
    ("radroots_outbox", "crates/outbox/Cargo.toml"),
    ("radroots_replica_sync", "crates/replica_sync/Cargo.toml"),
    ("radroots_runtime", "crates/runtime/Cargo.toml"),
    ("radroots_transport", "crates/transport/Cargo.toml"),
    (
        "radroots_transport_nostr",
        "crates/transport_nostr/Cargo.toml",
    ),
    (
        "radroots_transport_reticulum",
        "crates/transport_reticulum/Cargo.toml",
    ),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Phase1FeatureMatrix {
    schema_version: u32,
    wasm_target: String,
    required_categories: Vec<FeatureCategory>,
    packages: BTreeMap<String, String>,
    profiles: Vec<FeatureProfile>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
enum FeatureCategory {
    AllFeatures,
    Minimal,
    Raster,
    Runtime,
    Serde,
    Signature,
    Sqlite,
    Std,
    Wasm,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureProfile {
    id: String,
    package: String,
    category: FeatureCategory,
    no_default_features: bool,
    all_features: bool,
    features: Vec<String>,
    requires: Vec<String>,
    all_targets: bool,
    target: String,
}

pub(super) fn validate_feature_support(workspace_root: &Path) -> Result<(), String> {
    validate_nostr_manifest(workspace_root)?;
    let path = workspace_root.join(NOSTR_LIB_RELATIVE);
    let source =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    validate_nostr_std_only_source(&source)?;
    let matrix = load_phase1_feature_matrix(workspace_root)?;
    validate_phase1_feature_matrix(workspace_root, &matrix)
}

fn load_phase1_feature_matrix(workspace_root: &Path) -> Result<Phase1FeatureMatrix, String> {
    let path = workspace_root.join(PHASE1_FEATURE_MATRIX_RELATIVE);
    let source =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    toml::from_str(&source).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn validate_phase1_feature_matrix(
    workspace_root: &Path,
    matrix: &Phase1FeatureMatrix,
) -> Result<(), String> {
    if matrix.schema_version != 1 || matrix.wasm_target != WASM_TARGET {
        return Err(
            "Phase 1 feature matrix identity must remain schema v1 with the governed wasm target"
                .to_owned(),
        );
    }
    let expected_categories = REQUIRED_CATEGORIES.iter().copied().collect::<BTreeSet<_>>();
    let actual_categories = unique_values(
        matrix.required_categories.iter().copied(),
        "Phase 1 required feature categories",
    )?;
    if actual_categories != expected_categories {
        return Err("Phase 1 feature matrix required categories drifted".to_owned());
    }
    let expected_packages = AFFECTED_PACKAGES
        .iter()
        .map(|(package, manifest)| ((*package).to_owned(), (*manifest).to_owned()))
        .collect::<BTreeMap<_, _>>();
    if matrix.packages != expected_packages {
        return Err("Phase 1 feature matrix affected package inventory drifted".to_owned());
    }

    let mut cargo_features = BTreeMap::new();
    for (package, relative) in &matrix.packages {
        cargo_features.insert(
            package.as_str(),
            load_cargo_features(workspace_root, package, relative)?,
        );
    }

    let mut ids = BTreeSet::new();
    let mut observed_categories = BTreeSet::new();
    let mut package_categories = BTreeMap::<&str, BTreeSet<FeatureCategory>>::new();
    for profile in &matrix.profiles {
        if profile.id.trim().is_empty() || !ids.insert(profile.id.as_str()) {
            return Err(format!(
                "Phase 1 feature profile id must be nonempty and unique: {}",
                profile.id
            ));
        }
        let features = cargo_features
            .get(profile.package.as_str())
            .ok_or_else(|| {
                format!(
                    "Phase 1 feature profile {} references unknown package {}",
                    profile.id, profile.package
                )
            })?;
        validate_feature_profile(profile, features)?;
        observed_categories.insert(profile.category);
        package_categories
            .entry(profile.package.as_str())
            .or_default()
            .insert(profile.category);
    }
    if observed_categories != expected_categories {
        return Err("Phase 1 feature profiles do not execute every required category".to_owned());
    }
    for package in matrix.packages.keys() {
        let categories = package_categories
            .get(package.as_str())
            .ok_or_else(|| format!("Phase 1 feature matrix has no profiles for {package}"))?;
        if !categories.contains(&FeatureCategory::Minimal)
            || !categories.contains(&FeatureCategory::AllFeatures)
        {
            return Err(format!(
                "Phase 1 feature matrix must execute minimal and all-features profiles for {package}"
            ));
        }
    }
    Ok(())
}

fn load_cargo_features(
    workspace_root: &Path,
    expected_package: &str,
    relative: &str,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let path = workspace_root.join(relative);
    let source =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let manifest = toml::from_str::<toml::Value>(&source)
        .map_err(|error| format!("parse {}: {error}", path.display()))?;
    let actual_package = manifest
        .get("package")
        .and_then(|value| value.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{relative} must declare package.name"))?;
    if actual_package != expected_package {
        return Err(format!(
            "{relative} package name drifted: expected {expected_package}, found {actual_package}"
        ));
    }
    let table = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{relative} must declare [features]"))?;
    table
        .iter()
        .map(|(feature, value)| {
            let edges = value
                .as_array()
                .ok_or_else(|| format!("{relative} feature {feature} must be an array"))?
                .iter()
                .map(|edge| {
                    edge.as_str().map(str::to_owned).ok_or_else(|| {
                        format!("{relative} feature {feature} edges must be strings")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((feature.clone(), edges))
        })
        .collect()
}

fn validate_feature_profile(
    profile: &FeatureProfile,
    feature_edges: &BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    let selected = unique_values(
        profile.features.iter().map(String::as_str),
        &format!("Phase 1 feature profile {} features", profile.id),
    )?;
    let required = unique_values(
        profile.requires.iter().map(String::as_str),
        &format!("Phase 1 feature profile {} requirements", profile.id),
    )?;
    if profile.all_features && (!selected.is_empty() || !required.is_empty()) {
        return Err(format!(
            "Phase 1 all-features profile {} must not duplicate explicit features",
            profile.id
        ));
    }
    for feature in selected.iter().chain(&required) {
        if !feature_edges.contains_key(*feature) {
            return Err(format!(
                "Phase 1 feature profile {} references undeclared feature {feature}",
                profile.id
            ));
        }
    }
    match profile.category {
        FeatureCategory::Minimal => {
            if !profile.no_default_features
                || profile.all_features
                || !selected.is_empty()
                || profile.target != "host"
            {
                return Err(format!(
                    "Phase 1 minimal profile {} must be a host no-default check",
                    profile.id
                ));
            }
        }
        FeatureCategory::AllFeatures => {
            if profile.no_default_features
                || !profile.all_features
                || !profile.all_targets
                || profile.target != "host"
            {
                return Err(format!(
                    "Phase 1 all-features profile {} must be a host all-target check",
                    profile.id
                ));
            }
        }
        FeatureCategory::Wasm => {
            if !profile.no_default_features
                || profile.all_features
                || profile.all_targets
                || profile.target != WASM_TARGET
            {
                return Err(format!(
                    "Phase 1 wasm profile {} must be a no-default wasm lib check",
                    profile.id
                ));
            }
        }
        _ => {
            if !profile.no_default_features
                || profile.all_features
                || selected.is_empty()
                || profile.target != "host"
            {
                return Err(format!(
                    "Phase 1 named profile {} must select explicit host features",
                    profile.id
                ));
            }
        }
    }

    let mut closure = if profile.all_features {
        feature_edges
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
    } else {
        selected
    };
    if !profile.no_default_features {
        closure.insert("default");
    }
    let mut pending = closure.iter().copied().collect::<Vec<_>>();
    while let Some(feature) = pending.pop() {
        for edge in feature_edges.get(feature).into_iter().flatten() {
            if feature_edges.contains_key(edge) && closure.insert(edge) {
                pending.push(edge);
            }
        }
    }
    if !required.is_subset(&closure) {
        return Err(format!(
            "Phase 1 feature profile {} does not imply every declared requirement",
            profile.id
        ));
    }
    Ok(())
}

fn unique_values<T: Ord>(
    values: impl IntoIterator<Item = T>,
    label: &str,
) -> Result<BTreeSet<T>, String> {
    let mut unique = BTreeSet::new();
    for value in values {
        if !unique.insert(value) {
            return Err(format!("{label} contains a duplicate"));
        }
    }
    Ok(unique)
}

fn validate_nostr_manifest(workspace_root: &Path) -> Result<(), String> {
    let path = workspace_root.join(NOSTR_MANIFEST_RELATIVE);
    let source =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let manifest = toml::from_str::<toml::Value>(&source)
        .map_err(|error| format!("parse {}: {error}", path.display()))?;
    let features = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{NOSTR_MANIFEST_RELATIVE} must define features"))?;
    let default = string_array(features.get("default"), "radroots_nostr default feature")?;
    let std = string_array(features.get("std"), "radroots_nostr std feature")?;
    if default != ["std"] || !std.is_empty() {
        return Err(
            "radroots_nostr must remain std-only with default = [\"std\"] and an empty std marker"
                .to_owned(),
        );
    }
    Ok(())
}

fn string_array<'a>(value: Option<&'a toml::Value>, label: &str) -> Result<Vec<&'a str>, String> {
    value
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{label} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{label} entries must be strings"))
        })
        .collect()
}

fn validate_nostr_std_only_source(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|error| format!("parse {NOSTR_LIB_RELATIVE} as Rust: {error}"))?;
    for attribute in &file.attrs {
        if attribute.path().is_ident("no_std") || cfg_attr_contains_no_std(attribute)? {
            return Err(format!(
                "{NOSTR_LIB_RELATIVE} must not declare a no_std crate mode"
            ));
        }
    }
    Ok(())
}

fn cfg_attr_contains_no_std(attribute: &Attribute) -> Result<bool, String> {
    if !attribute.path().is_ident("cfg_attr") {
        return Ok(false);
    }
    let Meta::List(list) = &attribute.meta else {
        return Err("cfg_attr must use list syntax".to_owned());
    };
    let entries = Punctuated::<Meta, Token![,]>::parse_terminated
        .parse2(list.tokens.clone())
        .map_err(|error| format!("parse cfg_attr in {NOSTR_LIB_RELATIVE}: {error}"))?;
    Ok(entries.iter().skip(1).any(meta_contains_no_std))
}

fn meta_contains_no_std(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("no_std"),
        Meta::List(list) => Punctuated::<Meta, Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .map(|entries| entries.iter().any(meta_contains_no_std))
            .unwrap_or(false),
        Meta::NameValue(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn std_only_source_rejects_direct_and_conditional_no_std_modes() {
        validate_nostr_std_only_source("#![forbid(unsafe_code)]\nextern crate alloc;")
            .expect("std-only source");
        for invalid in [
            "#![no_std]\nextern crate alloc;",
            "#![cfg_attr(not(feature = \"std\"), no_std)]\nextern crate alloc;",
        ] {
            assert!(validate_nostr_std_only_source(invalid).is_err());
        }
    }

    #[test]
    fn phase1_feature_matrix_closes_profiles_and_declared_implications() {
        let root = workspace_root();
        let matrix = load_phase1_feature_matrix(&root).expect("load feature matrix");
        validate_phase1_feature_matrix(&root, &matrix).expect("validate feature matrix");

        let mut missing_all_features = load_phase1_feature_matrix(&root).unwrap();
        missing_all_features
            .profiles
            .retain(|profile| profile.id != "authority-all-features");
        assert!(
            validate_phase1_feature_matrix(&root, &missing_all_features)
                .unwrap_err()
                .contains("minimal and all-features")
        );

        let mut invalid_implication = load_phase1_feature_matrix(&root).unwrap();
        invalid_implication.profiles[0]
            .requires
            .push("undeclared".to_owned());
        assert!(
            validate_phase1_feature_matrix(&root, &invalid_implication)
                .unwrap_err()
                .contains("undeclared feature")
        );
    }

    #[test]
    fn phase1_feature_matrix_schema_rejects_unknown_fields() {
        let root = workspace_root();
        let source = fs::read_to_string(root.join(PHASE1_FEATURE_MATRIX_RELATIVE)).unwrap();
        let mutated = source.replacen(
            "schema_version = 1",
            "schema_version = 1\nunknown = true",
            1,
        );
        let error = toml::from_str::<Phase1FeatureMatrix>(&mutated).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }
}

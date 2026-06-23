use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportContract {
    language: LanguageContract,
    packages: BTreeMap<String, String>,
    artifacts: Option<ExportArtifacts>,
    runtime: RuntimeContract,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageContract {
    language: LanguageContract,
    sdk: SdkPackageContract,
    rollout: RolloutContract,
    operations: BTreeMap<String, String>,
    shared_types: BTreeMap<String, String>,
    artifacts: Option<SdkArtifacts>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LanguageContract {
    id: String,
    repository: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeContract {
    networking: String,
    signing: String,
    deterministic_codec: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportArtifacts {
    models_dir: String,
    constants_dir: String,
    wasm_dist_dir: Option<String>,
    manifest_file: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SdkPackageContract {
    package: String,
    module_format: Option<String>,
    deterministic_codec: String,
    signing: String,
    networking: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RolloutContract {
    stage: String,
    order: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SdkArtifacts {
    models_dir: String,
    runtime_dir: String,
    wasm_dist_dir: String,
    manifest_file: String,
}

pub fn validate_sdk_contracts(root: &Path) -> Result<(), String> {
    let exports = load_contract_dir::<ExportContract>(&root.join("contracts").join("exports"))?;
    let packages = load_contract_dir::<PackageContract>(&root.join("contracts").join("packages"))?;
    if exports.is_empty() {
        return Err("contracts/exports must define at least one language".to_owned());
    }
    if packages.is_empty() {
        return Err("contracts/packages must define at least one language".to_owned());
    }

    let mut export_packages = BTreeMap::new();
    let mut export_languages = BTreeSet::new();
    for export in &exports {
        validate_language(&export.language, "exports")?;
        validate_non_empty_map(&export.packages, "exports packages")?;
        validate_runtime(
            &export.runtime.networking,
            &export.runtime.signing,
            &export.runtime.deterministic_codec,
            &format!("exports {}", export.language.id),
        )?;
        let artifacts = export
            .artifacts
            .as_ref()
            .ok_or_else(|| format!("exports {} artifacts are required", export.language.id))?;
        validate_non_empty(&artifacts.models_dir, "exports artifacts.models_dir")?;
        validate_non_empty(&artifacts.constants_dir, "exports artifacts.constants_dir")?;
        validate_non_empty(&artifacts.manifest_file, "exports artifacts.manifest_file")?;
        if export.language.id == "ts" {
            validate_non_empty(
                artifacts.wasm_dist_dir.as_deref().unwrap_or(""),
                "exports ts artifacts.wasm_dist_dir",
            )?;
        }
        if !export_languages.insert(export.language.id.clone()) {
            return Err(format!("duplicate exports language {}", export.language.id));
        }
        let packages = export
            .packages
            .values()
            .cloned()
            .collect::<BTreeSet<String>>();
        if packages.len() != 1 {
            return Err(format!(
                "exports {} must resolve to one curated package",
                export.language.id
            ));
        }
        export_packages.insert(export.language.id.clone(), packages);
    }

    let mut package_languages = BTreeSet::new();
    let mut operation_keys: Option<BTreeSet<String>> = None;
    let mut shared_type_keys: Option<BTreeSet<String>> = None;
    let mut rollout_orders = BTreeMap::new();
    for package in &packages {
        validate_language(&package.language, "packages")?;
        validate_non_empty(&package.sdk.package, "packages sdk.package")?;
        validate_runtime(
            &package.sdk.networking,
            &package.sdk.signing,
            &package.sdk.deterministic_codec,
            &format!("packages {}", package.language.id),
        )?;
        if let Some(module_format) = package.sdk.module_format.as_deref() {
            validate_non_empty(module_format, "packages sdk.module_format")?;
        }
        validate_rollout(&package.language.id, &package.rollout)?;
        validate_non_empty_map(&package.operations, "packages operations")?;
        validate_non_empty_map(&package.shared_types, "packages shared_types")?;
        if package.language.id == "ts" {
            let artifacts = package
                .artifacts
                .as_ref()
                .ok_or_else(|| "packages ts artifacts are required".to_owned())?;
            validate_non_empty(&artifacts.models_dir, "packages ts artifacts.models_dir")?;
            validate_non_empty(&artifacts.runtime_dir, "packages ts artifacts.runtime_dir")?;
            validate_non_empty(
                &artifacts.wasm_dist_dir,
                "packages ts artifacts.wasm_dist_dir",
            )?;
            validate_non_empty(
                &artifacts.manifest_file,
                "packages ts artifacts.manifest_file",
            )?;
        }
        if !package_languages.insert(package.language.id.clone()) {
            return Err(format!(
                "duplicate packages language {}",
                package.language.id
            ));
        }
        let Some(packages_for_language) = export_packages.get(&package.language.id) else {
            return Err(format!(
                "packages {} is missing a matching export contract",
                package.language.id
            ));
        };
        let expected = [package.sdk.package.clone()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        if packages_for_language != &expected {
            return Err(format!(
                "exports {} must resolve to package {}",
                package.language.id, package.sdk.package
            ));
        }
        let current_operations = package.operations.keys().cloned().collect::<BTreeSet<_>>();
        match &operation_keys {
            Some(expected) if expected != &current_operations => {
                return Err(format!(
                    "packages {} operations must match the shared operation set",
                    package.language.id
                ));
            }
            None => operation_keys = Some(current_operations),
            _ => {}
        }
        let current_shared_types = package
            .shared_types
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        match &shared_type_keys {
            Some(expected) if expected != &current_shared_types => {
                return Err(format!(
                    "packages {} shared_types must match the shared type set",
                    package.language.id
                ));
            }
            None => shared_type_keys = Some(current_shared_types),
            _ => {}
        }
        rollout_orders.insert(package.language.id.clone(), package.rollout.order);
    }

    if export_languages != package_languages {
        return Err("contracts/exports and contracts/packages languages must match".to_owned());
    }
    if rollout_orders.get("ts") != Some(&1) {
        return Err("packages ts rollout.order must be 1".to_owned());
    }
    Ok(())
}

fn load_contract_dir<T>(dir: &Path) -> Result<Vec<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    let read_dir =
        fs::read_dir(dir).map_err(|error| format!("failed to read {}: {error}", dir.display()))?;
    let mut entries = read_dir
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read {} entry: {error}", dir.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut contracts = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
            continue;
        }
        contracts.push(parse_toml(&path)?);
    }
    Ok(contracts)
}

fn parse_toml<T>(path: &PathBuf) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn validate_language(language: &LanguageContract, family: &str) -> Result<(), String> {
    validate_non_empty(&language.id, &format!("{family} language.id"))?;
    validate_non_empty(
        &language.repository,
        &format!("{family} language.repository"),
    )
}

fn validate_runtime(
    networking: &str,
    signing: &str,
    deterministic_codec: &str,
    family: &str,
) -> Result<(), String> {
    validate_non_empty(networking, &format!("{family} networking"))?;
    validate_non_empty(signing, &format!("{family} signing"))?;
    validate_non_empty(
        deterministic_codec,
        &format!("{family} deterministic_codec"),
    )
}

fn validate_rollout(language: &str, rollout: &RolloutContract) -> Result<(), String> {
    validate_non_empty(&rollout.stage, "packages rollout.stage")?;
    if !matches!(rollout.stage.as_str(), "active" | "next" | "deferred") {
        return Err(format!("packages {language} rollout.stage is invalid"));
    }
    if rollout.order == 0 {
        return Err(format!(
            "packages {language} rollout.order must be greater than zero"
        ));
    }
    Ok(())
}

fn validate_non_empty(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.trim() != value {
        return Err(format!("{field} must be non-empty"));
    }
    Ok(())
}

fn validate_non_empty_map(map: &BTreeMap<String, String>, field: &str) -> Result<(), String> {
    if map.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    for (key, value) in map {
        validate_non_empty(key, field)?;
        validate_non_empty(value, field)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::validate_sdk_contracts;

    #[test]
    fn current_sdk_contracts_validate() {
        let root = crate::fs::workspace_root().expect("workspace root");
        validate_sdk_contracts(&root).expect("sdk contracts validate");
    }

    #[test]
    fn rejects_mismatched_language_sets() {
        let root = test_root("language_mismatch");
        write_contract(
            &root,
            "contracts/exports/ts.toml",
            EXPORT_TS.replace("@radroots/sdk", "@radroots/sdk").as_str(),
        );
        let error = validate_sdk_contracts(&root).expect_err("missing packages should fail");
        assert!(error.contains("contracts/packages"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_package_export_mismatch() {
        let root = test_root("package_mismatch");
        write_contract(&root, "contracts/exports/ts.toml", EXPORT_TS);
        write_contract(
            &root,
            "contracts/packages/ts.toml",
            PACKAGE_TS
                .replace("@radroots/sdk", "@radroots/other")
                .as_str(),
        );
        let error = validate_sdk_contracts(&root).expect_err("mismatch should fail");
        assert!(error.contains("exports ts must resolve"));
        let _ = fs::remove_dir_all(root);
    }

    fn test_root(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("radroots_sdk_contracts_{name}_{stamp}"))
    }

    fn write_contract(root: &std::path::Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, contents).expect("write contract");
    }

    const EXPORT_TS: &str = r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots_core" = "@radroots/sdk"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"

[runtime]
networking = "native"
signing = "native"
deterministic_codec = "wasm"
"#;

    const PACKAGE_TS: &str = r#"[language]
id = "ts"
repository = "sdk-typescript"

[sdk]
package = "@radroots/sdk"
module_format = "esm"
deterministic_codec = "wasm"
signing = "native"
networking = "native"

[rollout]
stage = "active"
order = 1

[operations]
"profile.build_draft" = "profile.buildDraft"

[shared_types]
"WireEventParts" = "WireEventParts"

[artifacts]
models_dir = "src/generated"
runtime_dir = "src/runtime"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#;
}

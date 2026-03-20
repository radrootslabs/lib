#![forbid(unsafe_code)]

use crate::contract;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
struct ExportManifest {
    language: String,
    files: Vec<ExportManifestEntry>,
}

#[derive(Serialize)]
struct ExportManifestEntry {
    path: String,
    sha256: String,
}

fn to_package_dir(base: &Path, package_name: &str) -> PathBuf {
    let stripped = package_name
        .strip_prefix("@radroots/")
        .unwrap_or(package_name);
    base.join(stripped)
}

fn ts_export_mapping(
    bundle: &contract::ContractBundle,
) -> Result<&contract::ExportMapping, String> {
    if let Some(mapping) = bundle
        .exports
        .iter()
        .find(|mapping| mapping.language.id == "ts")
    {
        return Ok(mapping);
    }
    Err("missing ts export mapping".to_string())
}

fn ts_artifacts(mapping: &contract::ExportMapping) -> Result<&contract::ExportArtifacts, String> {
    if let Some(artifacts) = mapping.artifacts.as_ref() {
        return Ok(artifacts);
    }
    Err("missing ts artifacts mapping".to_string())
}

fn selected_package_entries<'a>(
    mapping: &'a contract::ExportMapping,
    selector: Option<&str>,
) -> Result<Vec<(&'a String, &'a String)>, String> {
    if let Some(selector) = selector {
        if let Some(entry) = mapping.packages.get_key_value(selector) {
            return Ok(vec![entry]);
        }
        if let Some(entry) = mapping
            .packages
            .iter()
            .find(|(_, package_name)| package_name.as_str() == selector)
        {
            return Ok(vec![entry]);
        }
        if !selector.starts_with("radroots-") {
            let crate_candidate = format!("radroots-{selector}");
            if let Some(entry) = mapping.packages.get_key_value(&crate_candidate) {
                return Ok(vec![entry]);
            }
        }
        if !selector.starts_with("@radroots/") {
            let package_candidate = format!("@radroots/{selector}");
            if let Some(entry) = mapping
                .packages
                .iter()
                .find(|(_, package_name)| package_name.as_str() == package_candidate)
            {
                return Ok(vec![entry]);
            }
        }
        let known_crates = mapping
            .packages
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "unknown ts export crate selector {selector}; available crates: {known_crates}"
        ));
    }
    Ok(mapping.packages.iter().collect())
}

fn required_artifact_value<'a>(value: &'a Option<String>, field: &str) -> Result<&'a str, String> {
    if let Some(raw) = value.as_deref() {
        if !raw.trim().is_empty() {
            return Ok(raw);
        }
    }
    Err(format!("missing ts artifacts.{field}"))
}

fn crate_supports_ts_rs(workspace_root: &Path, crate_dir: &str) -> Result<bool, String> {
    let manifest = workspace_root
        .join("crates")
        .join(crate_dir)
        .join("Cargo.toml");
    if !manifest.exists() {
        return Ok(false);
    }
    let raw = match fs::read_to_string(&manifest) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", manifest.display())),
    };
    Ok(raw.contains("ts-rs"))
}

fn copy_if_exists(src: &Path, dst: &Path) -> Result<bool, String> {
    if !src.exists() {
        return Ok(false);
    }
    let parent = dst.parent().expect("destination path must have parent");
    if let Err(e) = fs::create_dir_all(parent) {
        return Err(format!("create {}: {e}", parent.display()));
    }
    if let Err(e) = fs::copy(src, dst) {
        return Err(format!("copy {} -> {}: {e}", src.display(), dst.display()));
    }
    Ok(true)
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<usize, String> {
    if !src.exists() {
        return Ok(0);
    }
    if let Err(e) = fs::create_dir_all(dst) {
        return Err(format!("create {}: {e}", dst.display()));
    }
    let mut copied = 0usize;
    let read_dir = match fs::read_dir(src) {
        Ok(entries) => entries,
        Err(e) => return Err(format!("read dir {}: {e}", src.display())),
    };
    let mut entries = read_dir.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copied += copy_dir_contents(&path, &target)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        if let Err(e) = fs::copy(&path, &target) {
            return Err(format!(
                "copy {} -> {}: {e}",
                path.display(),
                target.display()
            ));
        }
        copied += 1;
    }
    Ok(copied)
}

fn collect_manifest_entries(
    root: &Path,
    current: &Path,
    skip_path: &Path,
    entries: &mut Vec<ExportManifestEntry>,
) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }
    let read_dir = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(e) => return Err(format!("read dir {}: {e}", current.display())),
    };
    let mut dir_entries = read_dir.filter_map(Result::ok).collect::<Vec<_>>();
    dir_entries.sort_by_key(|entry| entry.file_name());
    for entry in dir_entries {
        let path = entry.path();
        if path == skip_path {
            continue;
        }
        if path.is_dir() {
            collect_manifest_entries(root, &path, skip_path, entries)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let digest = Sha256::digest(&bytes);
        let relative = match path.strip_prefix(root) {
            Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
            Err(e) => return Err(format!("strip prefix {}: {e}", path.display())),
        };
        entries.push(ExportManifestEntry {
            path: relative,
            sha256: hex::encode(digest),
        });
    }
    Ok(())
}

fn export_ts_models_with_selector(
    workspace_root: &Path,
    out_dir: &Path,
    selector: Option<&str>,
) -> Result<(), String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = ts_export_mapping(&bundle)?;
    let selected_entries = selected_package_entries(ts_export, selector)?;
    let artifacts = ts_artifacts(ts_export).expect("validated contract includes ts artifacts");
    let models_dir = required_artifact_value(&artifacts.models_dir, "models_dir")
        .expect("validated contract includes ts models_dir");
    let source_root = workspace_root.join("target").join("ts-rs");
    if !source_root.exists() {
        return Err(format!(
            "missing ts-rs source root {}",
            source_root.display()
        ));
    }
    let ts_out_root = out_dir.join("ts").join("packages");
    let mut expected = 0usize;
    let mut copied = 0usize;
    for (crate_name, package_name) in selected_entries {
        let crate_dir = crate_name.strip_prefix("radroots-").unwrap_or(crate_name);
        if crate_name.ends_with("-wasm") {
            continue;
        }
        if !crate_supports_ts_rs(workspace_root, crate_dir)
            .expect("validated workspace crate manifests are readable")
        {
            continue;
        }
        expected += 1;
        let src = source_root.join(crate_dir).join("types.ts");
        let dst = to_package_dir(&ts_out_root, package_name)
            .join(models_dir)
            .join("types.ts");
        if copy_if_exists(&src, &dst)? {
            copied += 1;
        }
    }
    if expected > 0 && copied == 0 {
        return Err("no ts model files were exported".to_string());
    }
    Ok(())
}

pub fn export_ts_models(workspace_root: &Path, out_dir: &Path) -> Result<(), String> {
    export_ts_models_with_selector(workspace_root, out_dir, None)
}

pub fn export_ts_models_for_crate(
    workspace_root: &Path,
    out_dir: &Path,
    crate_selector: &str,
) -> Result<(), String> {
    export_ts_models_with_selector(workspace_root, out_dir, Some(crate_selector))
}

fn export_ts_constants_with_selector(
    workspace_root: &Path,
    out_dir: &Path,
    selector: Option<&str>,
) -> Result<(), String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = ts_export_mapping(&bundle)?;
    let selected_entries = selected_package_entries(ts_export, selector)?;
    let artifacts = ts_artifacts(ts_export).expect("validated contract includes ts artifacts");
    let constants_dir = required_artifact_value(&artifacts.constants_dir, "constants_dir")
        .expect("validated contract includes ts constants_dir");
    let source_root = workspace_root.join("target").join("ts-rs");
    if !source_root.exists() {
        return Err(format!(
            "missing ts-rs source root {}",
            source_root.display()
        ));
    }
    let ts_out_root = out_dir.join("ts").join("packages");
    for (crate_name, package_name) in selected_entries {
        let crate_dir = crate_name.strip_prefix("radroots-").unwrap_or(crate_name);
        for filename in ["constants.ts", "kinds.ts"] {
            let src = source_root.join(crate_dir).join(filename);
            let dst = to_package_dir(&ts_out_root, package_name)
                .join(constants_dir)
                .join(filename);
            copy_if_exists(&src, &dst)?;
        }
    }
    Ok(())
}

pub fn export_ts_constants(workspace_root: &Path, out_dir: &Path) -> Result<(), String> {
    export_ts_constants_with_selector(workspace_root, out_dir, None)
}

pub fn export_ts_constants_for_crate(
    workspace_root: &Path,
    out_dir: &Path,
    crate_selector: &str,
) -> Result<(), String> {
    export_ts_constants_with_selector(workspace_root, out_dir, Some(crate_selector))
}

fn export_ts_wasm_artifacts_with_selector(
    workspace_root: &Path,
    out_dir: &Path,
    selector: Option<&str>,
) -> Result<(), String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = ts_export_mapping(&bundle)?;
    let selected_entries = selected_package_entries(ts_export, selector)?;
    let artifacts = ts_artifacts(ts_export).expect("validated contract includes ts artifacts");
    let wasm_dist_dir = required_artifact_value(&artifacts.wasm_dist_dir, "wasm_dist_dir")
        .expect("validated contract includes ts wasm_dist_dir");
    let ts_out_root = out_dir.join("ts").join("packages");
    let mut copied = 0usize;
    for (crate_name, package_name) in selected_entries {
        if !crate_name.ends_with("-wasm") {
            continue;
        }
        let crate_dir = crate_name.strip_prefix("radroots-").unwrap_or(crate_name);
        let source_root = workspace_root
            .join("crates")
            .join(crate_dir)
            .join("pkg")
            .join("dist");
        let target_root = to_package_dir(&ts_out_root, package_name).join(wasm_dist_dir);
        copied += copy_dir_contents(&source_root, &target_root)?;
    }
    if copied == 0 {
        return Ok(());
    }
    Ok(())
}

pub fn export_ts_wasm_artifacts(workspace_root: &Path, out_dir: &Path) -> Result<(), String> {
    export_ts_wasm_artifacts_with_selector(workspace_root, out_dir, None)
}

pub fn export_ts_wasm_artifacts_for_crate(
    workspace_root: &Path,
    out_dir: &Path,
    crate_selector: &str,
) -> Result<(), String> {
    export_ts_wasm_artifacts_with_selector(workspace_root, out_dir, Some(crate_selector))
}

pub fn write_ts_export_manifest(workspace_root: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = ts_export_mapping(&bundle)?;
    let artifacts = ts_artifacts(ts_export).expect("validated contract includes ts artifacts");
    let manifest_file = required_artifact_value(&artifacts.manifest_file, "manifest_file")
        .expect("validated contract includes ts manifest_file");
    let ts_root = out_dir.join("ts");
    let manifest_path = ts_root.join(manifest_file);
    let mut files = Vec::new();
    let packages_root = ts_root.join("packages");
    collect_manifest_entries(&ts_root, &packages_root, &manifest_path, &mut files)?;
    let manifest = ExportManifest {
        language: ts_export.language.id.clone(),
        files,
    };
    let parent = manifest_path
        .parent()
        .expect("manifest path must have parent");
    if let Err(e) = fs::create_dir_all(parent) {
        return Err(format!("create {}: {e}", parent.display()));
    }
    let bytes = serde_json::to_vec_pretty(&manifest)
        .expect("serializing export manifest should be infallible");
    if let Err(e) = fs::write(&manifest_path, bytes) {
        return Err(format!("write {}: {e}", manifest_path.display()));
    }
    Ok(manifest_path)
}

fn generate_ts_rs_sources_with_selector(
    workspace_root: &Path,
    selector: Option<&str>,
) -> Result<PathBuf, String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = ts_export_mapping(&bundle)?;
    let selected_entries = selected_package_entries(ts_export, selector)?;
    let source_root = workspace_root.join("target").join("ts-rs");
    if source_root.exists() {
        if let Err(e) = fs::remove_dir_all(&source_root) {
            return Err(format!("remove {}: {e}", source_root.display()));
        }
    }
    if let Err(e) = fs::create_dir_all(&source_root) {
        return Err(format!("create {}: {e}", source_root.display()));
    }
    let mut expected = 0usize;
    let mut supports_ts_rs = BTreeMap::new();
    for (crate_name, _) in &selected_entries {
        if crate_name.ends_with("-wasm") {
            continue;
        }
        let crate_dir = crate_name.strip_prefix("radroots-").unwrap_or(crate_name);
        let supports = crate_supports_ts_rs(workspace_root, crate_dir)
            .expect("validated workspace crate manifests are readable");
        supports_ts_rs.insert(crate_name.as_str(), supports);
        if supports {
            expected += 1;
        }
    }
    if expected == 0 {
        return Ok(source_root);
    }
    for (crate_name, package_name) in selected_entries {
        if crate_name.ends_with("-wasm") {
            continue;
        }
        if !supports_ts_rs
            .get(crate_name.as_str())
            .copied()
            .unwrap_or(false)
        {
            continue;
        }
        let package_dir = package_name
            .strip_prefix("@radroots/")
            .unwrap_or(package_name);
        let export_dir = source_root.join(package_dir);
        let _ = fs::create_dir_all(&export_dir);
        let status = Command::new("cargo")
            .arg("test")
            .arg("-q")
            .arg("-p")
            .arg(crate_name)
            .arg("--features")
            .arg("ts-rs")
            .env("RADROOTS_TS_RS_EXPORT_DIR", &export_dir)
            .current_dir(workspace_root)
            .status();
        if !status.is_ok_and(|status| status.success()) {
            return Err(format!("cargo test failed for {crate_name}"));
        }
    }
    Ok(source_root)
}

pub fn generate_ts_rs_sources(workspace_root: &Path) -> Result<PathBuf, String> {
    generate_ts_rs_sources_with_selector(workspace_root, None)
}

pub fn generate_ts_rs_sources_for_crate(
    workspace_root: &Path,
    crate_selector: &str,
) -> Result<PathBuf, String> {
    generate_ts_rs_sources_with_selector(workspace_root, Some(crate_selector))
}

pub fn export_ts_bundle(workspace_root: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    generate_ts_rs_sources(workspace_root)?;
    export_ts_models(workspace_root, out_dir)?;
    export_ts_constants(workspace_root, out_dir)?;
    export_ts_wasm_artifacts(workspace_root, out_dir)?;
    write_ts_export_manifest(workspace_root, out_dir)
}

pub fn export_ts_bundle_for_crate(
    workspace_root: &Path,
    out_dir: &Path,
    crate_selector: &str,
) -> Result<PathBuf, String> {
    generate_ts_rs_sources_for_crate(workspace_root, crate_selector)?;
    export_ts_models_for_crate(workspace_root, out_dir, crate_selector)?;
    export_ts_constants_for_crate(workspace_root, out_dir, crate_selector)?;
    export_ts_wasm_artifacts_for_crate(workspace_root, out_dir, crate_selector)?;
    write_ts_export_manifest(workspace_root, out_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .join("../..")
            .canonicalize()
            .expect("workspace root")
    }

    fn workspace_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("radroots_xtask_{prefix}_{ns}"))
    }

    fn write_file(path: &Path, content: &str) {
        let _ = fs::create_dir_all(path.parent().unwrap_or(Path::new("")));
        fs::write(path, content).expect("write file");
    }

    fn create_synthetic_workspace(prefix: &str, crate_a_ts_rs: bool) -> PathBuf {
        let root = unique_temp_dir(prefix);
        fs::create_dir_all(&root).expect("create root");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
resolver = "2"
"#,
        );
        let crate_a_features = if crate_a_ts_rs {
            "\n[features]\nts-rs = []\n"
        } else {
            ""
        };
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            &format!(
                r#"[package]
name = "radroots-a"
version = "0.1.0"
edition = "2024"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.example.com/a"
readme = "README.md"
{}"#,
                crate_a_features
            ),
        );
        write_file(
            &root.join("crates").join("a").join("src").join("lib.rs"),
            "pub fn crate_a() {}\n",
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
            &root.join("crates").join("b").join("src").join("lib.rs"),
            "pub fn crate_b() {}\n",
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
            &root
                .join("contract")
                .join("coverage")
                .join("policy.toml"),
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

    fn write_ts_rs_probe_lib(root: &Path) {
        write_file(
            &root.join("crates").join("a").join("src").join("lib.rs"),
            r#"pub fn crate_a() {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn write_ts_exports() {
        if let Ok(path) = std::env::var("RADROOTS_TS_RS_EXPORT_DIR") {
            let export_dir = PathBuf::from(path);
            let _ = fs::create_dir_all(&export_dir);
            fs::write(
                export_dir.join("types.ts"),
                "export type Probe = { id: string };\n",
            )
            .expect("write types");
            fs::write(export_dir.join("constants.ts"), "export const PROBE = 1;\n")
                .expect("write constants");
            fs::write(export_dir.join("kinds.ts"), "export const KIND = 1;\n")
                .expect("write kinds");
        }
    }
}
"#,
        );
    }

    fn test_ts_mapping() -> contract::ExportMapping {
        let mut packages = BTreeMap::new();
        packages.insert("radroots-core".to_string(), "@radroots/core".to_string());
        packages.insert(
            "radroots-events-codec-wasm".to_string(),
            "@radroots/events-codec-wasm".to_string(),
        );
        contract::ExportMapping {
            language: contract::ExportLanguage {
                id: "ts".to_string(),
                repository: "sdk-typescript".to_string(),
            },
            packages,
            artifacts: Some(contract::ExportArtifacts {
                models_dir: Some("src/generated".to_string()),
                constants_dir: Some("src/generated".to_string()),
                wasm_dist_dir: Some("dist".to_string()),
                manifest_file: Some("export-manifest.json".to_string()),
            }),
        }
    }

    #[test]
    fn selected_package_entries_match_crate_and_package_selectors() {
        let mapping = test_ts_mapping();

        let all = selected_package_entries(&mapping, None).expect("select all");
        assert_eq!(all.len(), 2);

        let by_crate = selected_package_entries(&mapping, Some("radroots-core")).expect("by crate");
        assert_eq!(by_crate.len(), 1);
        assert_eq!(by_crate[0].0.as_str(), "radroots-core");

        let by_short = selected_package_entries(&mapping, Some("core")).expect("by short crate");
        assert_eq!(by_short.len(), 1);
        assert_eq!(by_short[0].1.as_str(), "@radroots/core");

        let by_package =
            selected_package_entries(&mapping, Some("@radroots/core")).expect("by package");
        assert_eq!(by_package.len(), 1);
        assert_eq!(by_package[0].0.as_str(), "radroots-core");

        let wasm = selected_package_entries(&mapping, Some("events-codec-wasm")).expect("wasm");
        assert_eq!(wasm.len(), 1);
        assert_eq!(wasm[0].0.as_str(), "radroots-events-codec-wasm");
    }

    #[test]
    fn selected_package_entries_fail_for_unknown_selector() {
        let mapping = test_ts_mapping();
        let err = selected_package_entries(&mapping, Some("unknown")).expect_err("unknown");
        assert!(err.contains("unknown ts export crate selector"));
    }

    #[test]
    fn selected_package_entries_handle_prefixed_unknown_selectors() {
        let mapping = test_ts_mapping();
        let by_crate = selected_package_entries(&mapping, Some("radroots-missing"))
            .expect_err("unknown prefixed crate selector");
        assert!(by_crate.contains("unknown ts export crate selector"));
        let by_package = selected_package_entries(&mapping, Some("@radroots/missing"))
            .expect_err("unknown prefixed package selector");
        assert!(by_package.contains("unknown ts export crate selector"));
    }

    #[test]
    fn ts_mapping_and_artifacts_report_missing_entries() {
        let root = unique_temp_dir("missing_ts_mapping");
        fs::create_dir_all(&root).expect("create root");
        let bundle = contract::ContractBundle {
            root: root.clone(),
            manifest: contract::ContractManifest {
                contract: contract::ManifestContract {
                    name: "name".to_string(),
                    version: "1.0.0".to_string(),
                    source: "source".to_string(),
                },
                surface: contract::Surface {
                    model_crates: vec!["radroots-a".to_string()],
                    algorithm_crates: vec!["radroots-b".to_string()],
                    wasm_crates: vec![],
                },
                policy: contract::Policy {
                    exclude_internal_workspace_crates: true,
                    require_reproducible_exports: true,
                    require_conformance_vectors: true,
                },
            },
            version: contract::VersionPolicy {
                contract: contract::VersionContract {
                    version: "1.0.0".to_string(),
                    stability: "alpha".to_string(),
                },
                semver: contract::SemverRules {
                    major_on: vec!["breaking".to_string()],
                    minor_on: vec!["feature".to_string()],
                    patch_on: vec!["fix".to_string()],
                },
                compatibility: contract::CompatibilityRules {
                    requires_conformance_pass: true,
                    requires_export_manifest_diff: true,
                    requires_release_notes: true,
                },
            },
            exports: Vec::new(),
        };
        let mapping_err = ts_export_mapping(&bundle).expect_err("missing ts mapping");
        assert!(mapping_err.contains("missing ts export mapping"));

        let mut packages = BTreeMap::new();
        packages.insert("radroots-a".to_string(), "@radroots/a".to_string());
        let no_artifacts = contract::ExportMapping {
            language: contract::ExportLanguage {
                id: "ts".to_string(),
                repository: "sdk-typescript".to_string(),
            },
            packages,
            artifacts: None,
        };
        let artifacts_err = ts_artifacts(&no_artifacts).expect_err("missing ts artifacts mapping");
        assert!(artifacts_err.contains("missing ts artifacts mapping"));
        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn selected_package_entries_supports_package_candidate_lookup() {
        let mut packages = BTreeMap::new();
        packages.insert(
            "radroots-special".to_string(),
            "@radroots/special-pkg".to_string(),
        );
        let mapping = contract::ExportMapping {
            language: contract::ExportLanguage {
                id: "ts".to_string(),
                repository: "sdk-typescript".to_string(),
            },
            packages,
            artifacts: Some(contract::ExportArtifacts::default()),
        };
        let selected =
            selected_package_entries(&mapping, Some("special-pkg")).expect("package candidate");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0.as_str(), "radroots-special");
        assert_eq!(selected[0].1.as_str(), "@radroots/special-pkg");
    }

    #[test]
    fn package_dir_and_artifact_helpers_validate_values() {
        let base = PathBuf::from("/tmp/base");
        assert_eq!(to_package_dir(&base, "@radroots/core"), base.join("core"));
        assert_eq!(to_package_dir(&base, "custom"), base.join("custom"));

        let some = Some("src/generated".to_string());
        assert_eq!(
            required_artifact_value(&some, "models_dir").expect("required value"),
            "src/generated"
        );
        let none = None;
        assert!(required_artifact_value(&none, "models_dir").is_err());
        let blank = Some("   ".to_string());
        assert!(required_artifact_value(&blank, "models_dir").is_err());
    }

    #[test]
    fn helper_error_paths_cover_copy_manifest_and_support_checks() {
        let root = unique_temp_dir("helper_errors");
        fs::create_dir_all(&root).expect("create root");

        let manifest_dir = root.join("crates").join("probe").join("Cargo.toml");
        fs::create_dir_all(&manifest_dir).expect("create directory in place of manifest");
        let supports_err = crate_supports_ts_rs(&root, "probe").expect_err("manifest read error");
        assert!(supports_err.contains("read"));

        let src_file = root.join("src").join("one.txt");
        write_file(&src_file, "one");
        let dst_parent_file = root.join("dst-parent-file");
        write_file(&dst_parent_file, "block");
        let create_err = copy_if_exists(&src_file, &dst_parent_file.join("out.txt"))
            .expect_err("create parent error");
        assert!(create_err.contains("create"));

        let dst_file = root.join("dst-dir");
        fs::create_dir_all(&dst_file).expect("create destination directory");
        let copy_err =
            copy_if_exists(&src_file, &dst_file).expect_err("copy to directory should fail");
        assert!(copy_err.contains("copy"));

        let missing_dir = root.join("missing-dir");
        assert_eq!(
            copy_dir_contents(&missing_dir, &root.join("dst-missing")).expect("missing dir"),
            0
        );

        let src_dir = root.join("src-dir");
        fs::create_dir_all(&src_dir).expect("create src dir");
        let dst_blocker = root.join("dst-blocker");
        write_file(&dst_blocker, "blocker");
        let dst_err = copy_dir_contents(&src_dir, &dst_blocker).expect_err("create dst error");
        assert!(dst_err.contains("create"));

        let src_file_not_dir = root.join("src-file-not-dir");
        write_file(&src_file_not_dir, "not dir");
        let read_dir_err =
            copy_dir_contents(&src_file_not_dir, &root.join("dst-ok")).expect_err("read dir error");
        assert!(read_dir_err.contains("read dir"));

        let src_tree = root.join("src-tree");
        let dst_tree = root.join("dst-tree");
        fs::create_dir_all(&src_tree).expect("create src-tree");
        fs::create_dir_all(&dst_tree).expect("create dst-tree");
        write_file(&src_tree.join("entry.txt"), "entry");
        fs::create_dir_all(dst_tree.join("entry.txt")).expect("create colliding directory");
        let copy_tree_err = copy_dir_contents(&src_tree, &dst_tree).expect_err("copy tree error");
        assert!(copy_tree_err.contains("copy"));

        let current_is_file = root.join("manifest-file");
        write_file(&current_is_file, "x");
        let mut entries = Vec::new();
        let collect_err = collect_manifest_entries(
            &root,
            &current_is_file,
            &root.join("skip.json"),
            &mut entries,
        )
        .expect_err("read dir error for file");
        assert!(collect_err.contains("read dir"));

        let other_root = unique_temp_dir("manifest_strip_prefix");
        fs::create_dir_all(&other_root).expect("create other root");
        write_file(&other_root.join("x.txt"), "x");
        let mut entries = Vec::new();
        let strip_err = collect_manifest_entries(
            &root,
            &other_root,
            &other_root.join("skip.json"),
            &mut entries,
        )
        .expect_err("strip prefix error");
        assert!(strip_err.contains("strip prefix"));
        fs::remove_dir_all(other_root).expect("remove other root");

        fs::remove_dir_all(root).expect("remove helper root");
    }

    #[test]
    fn copy_helpers_and_manifest_collection_cover_file_paths() {
        let root = unique_temp_dir("copy_helpers");
        let src_file = root.join("src").join("one.txt");
        let dst_file = root.join("dst").join("one.txt");
        fs::create_dir_all(src_file.parent().expect("src parent")).expect("create src parent");
        fs::write(&src_file, "one").expect("write src file");
        assert!(copy_if_exists(&src_file, &dst_file).expect("copy file"));
        assert_eq!(fs::read_to_string(&dst_file).expect("read dst file"), "one");

        let missing = root.join("src").join("missing.txt");
        assert!(!copy_if_exists(&missing, &root.join("dst").join("missing.txt")).expect("missing"));

        let src_dir = root.join("src-tree");
        fs::create_dir_all(src_dir.join("nested")).expect("create src dir");
        fs::write(src_dir.join("a.txt"), "a").expect("write a");
        fs::write(src_dir.join("nested").join("b.txt"), "b").expect("write b");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(src_dir.join("missing.txt"), src_dir.join("broken-link"))
                .expect("create broken link");
        }
        let dst_dir = root.join("dst-tree");
        let copied = copy_dir_contents(&src_dir, &dst_dir).expect("copy dir");
        assert_eq!(copied, 2);
        assert!(dst_dir.join("a.txt").exists());
        assert!(dst_dir.join("nested").join("b.txt").exists());

        let manifest_skip = dst_dir.join("export-manifest.json");
        fs::write(&manifest_skip, "{}").expect("write manifest skip");
        let mut entries = Vec::new();
        collect_manifest_entries(&dst_dir, &dst_dir, &manifest_skip, &mut entries)
            .expect("collect entries");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.path == "a.txt"));
        assert!(entries.iter().any(|entry| entry.path == "nested/b.txt"));

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[cfg(unix)]
    #[test]
    fn collect_manifest_entries_skips_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let root = unique_temp_dir("manifest_symlink_skip");
        let source = root.join("source");
        fs::create_dir_all(&source).expect("create source");
        write_file(&source.join("a.txt"), "a");
        symlink(source.join("a.txt"), source.join("a-link")).expect("create symlink");
        symlink(source.join("missing.txt"), source.join("broken-link"))
            .expect("create broken link");
        write_file(&source.join("secret.txt"), "secret");
        let mut restricted = fs::metadata(source.join("secret.txt"))
            .expect("secret metadata")
            .permissions();
        restricted.set_mode(0o000);
        fs::set_permissions(source.join("secret.txt"), restricted).expect("set restricted mode");
        let mut entries = Vec::new();
        collect_manifest_entries(
            &source,
            &source,
            &source.join("export-manifest.json"),
            &mut entries,
        )
        .expect("collect entries");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.path == "a.txt"));
        assert!(entries.iter().any(|entry| entry.path == "a-link"));
        assert!(!entries.iter().any(|entry| entry.path == "broken-link"));
        assert!(!entries.iter().any(|entry| entry.path == "secret.txt"));
        let mut readable = fs::metadata(source.join("secret.txt"))
            .expect("secret metadata")
            .permissions();
        readable.set_mode(0o600);
        fs::set_permissions(source.join("secret.txt"), readable).expect("restore readable mode");
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn export_ts_files_with_workspace_contract() {
        let _guard = workspace_lock().lock().expect("workspace lock");
        let root = workspace_root();
        let bundle = contract::load_contract_bundle(&root).expect("load contract");
        contract::validate_contract_bundle(&bundle).expect("validate contract");
        let ts = ts_export_mapping(&bundle).expect("ts mapping");
        let artifacts = ts_artifacts(ts).expect("ts artifacts");
        let models_dir = required_artifact_value(&artifacts.models_dir, "models_dir")
            .expect("models dir")
            .to_string();
        let constants_dir = required_artifact_value(&artifacts.constants_dir, "constants_dir")
            .expect("constants dir")
            .to_string();

        let source_root = root.join("target").join("ts-rs").join("core");
        fs::create_dir_all(&source_root).expect("create ts-rs source root");
        fs::write(
            source_root.join("types.ts"),
            "export type CoreProbe = { id: string };\n",
        )
        .expect("write ts-rs model");
        let events_source = root.join("target").join("ts-rs").join("events");
        fs::create_dir_all(&events_source).expect("create events source");
        fs::write(events_source.join("constants.ts"), "export const A = 1;\n")
            .expect("write events constants");
        fs::write(events_source.join("kinds.ts"), "export const K = 1;\n")
            .expect("write events kinds");

        let out_dir = root.join("target").join("xtask-export-tests").join(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
                .to_string(),
        );
        fs::create_dir_all(&out_dir).expect("create out dir");

        export_ts_models(&root, &out_dir).expect("export models");
        assert!(
            out_dir
                .join("ts")
                .join("packages")
                .join("core")
                .join(&models_dir)
                .join("types.ts")
                .exists()
        );

        export_ts_constants(&root, &out_dir).expect("export constants");
        let events_constants = out_dir
            .join("ts")
            .join("packages")
            .join("events")
            .join(&constants_dir)
            .join("constants.ts");
        let events_kinds = out_dir
            .join("ts")
            .join("packages")
            .join("events")
            .join(&constants_dir)
            .join("kinds.ts");
        assert!(events_source.join("constants.ts").exists());
        assert!(events_source.join("kinds.ts").exists());
        assert!(events_constants.exists());
        assert!(events_kinds.exists());

        export_ts_wasm_artifacts(&root, &out_dir).expect("export wasm");
        let manifest_path = write_ts_export_manifest(&root, &out_dir).expect("write manifest");
        let manifest_raw = fs::read_to_string(&manifest_path).expect("read manifest");
        assert!(manifest_raw.contains("\"language\": \"ts\""));
        assert!(manifest_raw.contains("packages/core"));

        fs::remove_dir_all(&out_dir).expect("remove out dir");
    }

    #[test]
    fn crate_supports_ts_rs_reflects_manifest_presence() {
        let root = unique_temp_dir("crate_supports_ts_rs");
        let crate_dir = root.join("crates").join("probe");
        fs::create_dir_all(&crate_dir).expect("create crate dir");
        fs::write(
            crate_dir.join("Cargo.toml"),
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[features]\nts-rs = []\n",
        )
        .expect("write manifest");
        assert!(crate_supports_ts_rs(&root, "probe").expect("supports ts-rs"));
        assert!(!crate_supports_ts_rs(&root, "missing").expect("missing crate"));
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn export_models_and_constants_report_missing_source_roots() {
        let root = create_synthetic_workspace("export_missing_source", true);
        let out_dir = root.join("out");
        fs::create_dir_all(&out_dir).expect("create out dir");

        let models_err = export_ts_models(&root, &out_dir).expect_err("missing models source root");
        assert!(models_err.contains("missing ts-rs source root"));

        let constants_err =
            export_ts_constants(&root, &out_dir).expect_err("missing constants source root");
        assert!(constants_err.contains("missing ts-rs source root"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn export_models_reports_when_expected_files_are_missing() {
        let root = create_synthetic_workspace("export_models_missing_files", true);
        fs::create_dir_all(root.join("target").join("ts-rs")).expect("create ts-rs root");
        let out_dir = root.join("out");
        fs::create_dir_all(&out_dir).expect("create out dir");

        let err = export_ts_models(&root, &out_dir).expect_err("expected model files are missing");
        assert!(err.contains("no ts model files were exported"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn export_models_succeeds_when_expected_files_exist() {
        let root = create_synthetic_workspace("export_models_success", true);
        write_file(
            &root.join("target").join("ts-rs").join("a").join("types.ts"),
            "export type Probe = { id: string };\n",
        );
        let out_dir = root.join("out");
        fs::create_dir_all(&out_dir).expect("create out dir");
        export_ts_models(&root, &out_dir).expect("export models");
        assert!(
            out_dir
                .join("ts")
                .join("packages")
                .join("a")
                .join("src/generated")
                .join("types.ts")
                .exists()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn export_models_skip_non_ts_rs_crates() {
        let root = create_synthetic_workspace("export_models_skip_non_ts_rs", false);
        write_file(
            &root.join("target").join("ts-rs").join("a").join("types.ts"),
            "export type Probe = { id: string };\n",
        );
        let out_dir = root.join("out");
        fs::create_dir_all(&out_dir).expect("create out dir");
        export_ts_models(&root, &out_dir).expect("skip non ts-rs");
        assert!(!out_dir.join("ts").join("packages").join("a").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_manifest_reports_write_failures() {
        let root = create_synthetic_workspace("manifest_write_failure", false);
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
manifest_file = "packages"
"#,
        );
        let out_dir = root.join("out");
        fs::create_dir_all(out_dir.join("ts").join("packages")).expect("create packages directory");
        let err = write_ts_export_manifest(&root, &out_dir).expect_err("manifest write to dir");
        assert!(err.contains("write"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_manifest_reports_parent_create_failures() {
        let root = create_synthetic_workspace("manifest_create_failure", false);
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
manifest_file = "nested/export-manifest.json"
"#,
        );
        let out_dir = root.join("out");
        fs::create_dir_all(&out_dir).expect("create out dir");
        write_file(&out_dir.join("ts"), "blocker");
        let err =
            write_ts_export_manifest(&root, &out_dir).expect_err("manifest parent create fail");
        assert!(err.contains("create"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_manifest_succeeds_for_synthetic_workspace() {
        let root = create_synthetic_workspace("manifest_success", false);
        let out_dir = root.join("out");
        write_file(
            &out_dir
                .join("ts")
                .join("packages")
                .join("a")
                .join("src")
                .join("generated")
                .join("types.ts"),
            "export type Probe = { id: string };\n",
        );
        let manifest = write_ts_export_manifest(&root, &out_dir).expect("manifest success");
        assert!(manifest.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn generate_ts_rs_sources_reports_path_and_command_failures() {
        let root_remove = create_synthetic_workspace("generate_remove_fail", true);
        write_file(&root_remove.join("target").join("ts-rs"), "not-a-directory");
        let remove_err = generate_ts_rs_sources(&root_remove)
            .expect_err("remove existing source root should fail");
        assert!(remove_err.contains("remove"));
        let _ = fs::remove_dir_all(root_remove);

        let root_create = create_synthetic_workspace("generate_create_fail", true);
        let _ = fs::remove_dir_all(root_create.join("target"));
        write_file(&root_create.join("target"), "blocker");
        let create_err = generate_ts_rs_sources(&root_create)
            .expect_err("create source root parent should fail");
        assert!(create_err.contains("create"));
        let _ = fs::remove_dir_all(root_create);

        let root_no_expected = create_synthetic_workspace("generate_no_expected", false);
        let generated = generate_ts_rs_sources(&root_no_expected).expect("no expected crates");
        assert_eq!(generated, root_no_expected.join("target").join("ts-rs"));
        let _ = fs::remove_dir_all(root_no_expected);

        let root_command_fail = create_synthetic_workspace("generate_command_fail", true);
        write_file(
            &root_command_fail
                .join("crates")
                .join("a")
                .join("src")
                .join("lib.rs"),
            "pub fn broken( {\n",
        );
        let command_fail_err = generate_ts_rs_sources(&root_command_fail)
            .expect_err("cargo test failure should surface");
        assert!(command_fail_err.contains("cargo test failed for radroots-a"));
        let _ = fs::remove_dir_all(root_command_fail);
    }

    #[test]
    fn generate_ts_rs_sources_succeeds_and_skips_non_ts_rs_crates() {
        let _guard = workspace_lock().lock().expect("workspace lock");
        let root = create_synthetic_workspace("generate_skip_non_ts_rs", true);
        write_file(
            &root.join("contract").join("exports").join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"
"radroots-b" = "@radroots/b"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_file(
            &root.join("crates").join("a").join("src").join("lib.rs"),
            "pub fn probe() {}\n",
        );
        let generated = generate_ts_rs_sources(&root).expect("ts-rs generation should pass");
        assert!(generated.join("a").exists());
        assert!(!generated.join("b").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wrapper_calls_succeed_for_bundle_and_single_crate() {
        let _guard = workspace_lock().lock().expect("workspace lock");
        let root = create_synthetic_workspace("wrapper_bundle_success", true);
        write_file(
            &root.join("contract").join("exports").join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"
"radroots-b" = "@radroots/b"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_ts_rs_probe_lib(&root);

        let out_dir = root.join("out");
        fs::create_dir_all(&out_dir).expect("create bundle out");
        let manifest = export_ts_bundle(&root, &out_dir).expect("bundle success");
        assert!(manifest.exists());
        assert!(
            out_dir
                .join("ts")
                .join("packages")
                .join("a")
                .join("src/generated")
                .join("types.ts")
                .exists()
        );
        assert!(
            out_dir
                .join("ts")
                .join("packages")
                .join("a")
                .join("src/generated")
                .join("constants.ts")
                .exists()
        );
        assert!(
            out_dir
                .join("ts")
                .join("packages")
                .join("a")
                .join("src/generated")
                .join("kinds.ts")
                .exists()
        );

        let single_out = root.join("single-out");
        fs::create_dir_all(&single_out).expect("create single out");
        let single_manifest =
            export_ts_bundle_for_crate(&root, &single_out, "radroots-a").expect("single bundle");
        assert!(single_manifest.exists());
        assert!(
            single_out
                .join("ts")
                .join("packages")
                .join("a")
                .join("src/generated")
                .join("types.ts")
                .exists()
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn wrapper_calls_surface_mid_pipeline_errors() {
        let _guard = workspace_lock().lock().expect("workspace lock");

        let bundle_models = create_synthetic_workspace("wrapper_bundle_models_fail", true);
        write_ts_rs_probe_lib(&bundle_models);
        let bundle_models_out = bundle_models.join("out");
        write_file(
            &bundle_models_out.join("ts").join("packages").join("a"),
            "blocker",
        );
        let bundle_models_err =
            export_ts_bundle(&bundle_models, &bundle_models_out).expect_err("bundle models fail");
        assert!(bundle_models_err.contains("create"));
        let _ = fs::remove_dir_all(&bundle_models);

        let bundle_constants = create_synthetic_workspace("wrapper_bundle_constants_fail", true);
        write_file(
            &bundle_constants
                .join("contract")
                .join("exports")
                .join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated/types.ts"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_ts_rs_probe_lib(&bundle_constants);
        let bundle_constants_out = bundle_constants.join("out");
        fs::create_dir_all(&bundle_constants_out).expect("create bundle constants out");
        let bundle_constants_err = export_ts_bundle(&bundle_constants, &bundle_constants_out)
            .expect_err("bundle constants fail");
        assert!(bundle_constants_err.contains("create"));
        let _ = fs::remove_dir_all(&bundle_constants);

        let bundle_wasm = create_synthetic_workspace("wrapper_bundle_wasm_fail", true);
        write_file(
            &bundle_wasm.join("contract").join("exports").join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"
"radroots-a-wasm" = "@radroots/a-wasm"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_ts_rs_probe_lib(&bundle_wasm);
        write_file(
            &bundle_wasm
                .join("crates")
                .join("a-wasm")
                .join("pkg")
                .join("dist")
                .join("nested")
                .join("artifact.js"),
            "export const wasm = 1;\n",
        );
        let bundle_wasm_out = bundle_wasm.join("out");
        write_file(
            &bundle_wasm_out
                .join("ts")
                .join("packages")
                .join("a-wasm")
                .join("dist")
                .join("nested"),
            "blocker",
        );
        let bundle_wasm_err =
            export_ts_bundle(&bundle_wasm, &bundle_wasm_out).expect_err("bundle wasm fail");
        assert!(bundle_wasm_err.contains("create"));
        let _ = fs::remove_dir_all(&bundle_wasm);

        let crate_models = create_synthetic_workspace("wrapper_crate_models_fail", true);
        write_ts_rs_probe_lib(&crate_models);
        let crate_models_out = crate_models.join("out");
        write_file(
            &crate_models_out.join("ts").join("packages").join("a"),
            "blocker",
        );
        let crate_models_err =
            export_ts_bundle_for_crate(&crate_models, &crate_models_out, "radroots-a")
                .expect_err("crate models fail");
        assert!(crate_models_err.contains("create"));
        let _ = fs::remove_dir_all(&crate_models);

        let crate_constants = create_synthetic_workspace("wrapper_crate_constants_fail", true);
        write_file(
            &crate_constants
                .join("contract")
                .join("exports")
                .join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated/types.ts"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_ts_rs_probe_lib(&crate_constants);
        let crate_constants_out = crate_constants.join("out");
        fs::create_dir_all(&crate_constants_out).expect("create crate constants out");
        let crate_constants_err =
            export_ts_bundle_for_crate(&crate_constants, &crate_constants_out, "radroots-a")
                .expect_err("crate constants fail");
        assert!(crate_constants_err.contains("create"));
        let _ = fs::remove_dir_all(&crate_constants);

        let crate_wasm = create_synthetic_workspace("wrapper_crate_wasm_fail", true);
        write_file(
            &crate_wasm.join("contract").join("exports").join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a-wasm" = "@radroots/a-wasm"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_file(
            &crate_wasm
                .join("crates")
                .join("a-wasm")
                .join("pkg")
                .join("dist")
                .join("nested")
                .join("artifact.js"),
            "export const wasm = 1;\n",
        );
        let crate_wasm_out = crate_wasm.join("out");
        write_file(
            &crate_wasm_out
                .join("ts")
                .join("packages")
                .join("a-wasm")
                .join("dist")
                .join("nested"),
            "blocker",
        );
        let crate_wasm_err =
            export_ts_bundle_for_crate(&crate_wasm, &crate_wasm_out, "radroots-a-wasm")
                .expect_err("crate wasm fail");
        assert!(crate_wasm_err.contains("create"));
        let _ = fs::remove_dir_all(&crate_wasm);
    }

    #[test]
    fn wrapper_calls_surface_contract_and_selector_errors() {
        let missing_root = unique_temp_dir("wrapper_missing_contract");
        fs::create_dir_all(&missing_root).expect("create missing root");
        let missing_out = missing_root.join("out");
        fs::create_dir_all(&missing_out).expect("create missing out");
        assert!(export_ts_models(&missing_root, &missing_out).is_err());
        assert!(export_ts_constants(&missing_root, &missing_out).is_err());
        assert!(export_ts_wasm_artifacts(&missing_root, &missing_out).is_err());
        assert!(write_ts_export_manifest(&missing_root, &missing_out).is_err());
        assert!(generate_ts_rs_sources(&missing_root).is_err());
        assert!(export_ts_bundle(&missing_root, &missing_out).is_err());
        assert!(export_ts_bundle_for_crate(&missing_root, &missing_out, "radroots-a").is_err());
        let _ = fs::remove_dir_all(&missing_root);

        let invalid_contract = create_synthetic_workspace("wrapper_invalid_contract", true);
        write_file(
            &invalid_contract.join("contract").join("manifest.toml"),
            r#"[contract]
name = ""
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
        let invalid_out = invalid_contract.join("out");
        fs::create_dir_all(&invalid_out).expect("create invalid out");
        assert!(export_ts_models(&invalid_contract, &invalid_out).is_err());
        assert!(export_ts_constants(&invalid_contract, &invalid_out).is_err());
        assert!(export_ts_wasm_artifacts(&invalid_contract, &invalid_out).is_err());
        assert!(write_ts_export_manifest(&invalid_contract, &invalid_out).is_err());
        assert!(generate_ts_rs_sources(&invalid_contract).is_err());
        let _ = fs::remove_dir_all(&invalid_contract);

        let missing_ts_export = create_synthetic_workspace("wrapper_missing_ts_export", true);
        write_file(
            &missing_ts_export
                .join("contract")
                .join("exports")
                .join("ts.toml"),
            r#"[language]
id = "py"
repository = "sdk-python"

[packages]
"radroots-a" = "radroots-a"
"#,
        );
        let missing_ts_out = missing_ts_export.join("out");
        fs::create_dir_all(&missing_ts_out).expect("create missing ts out");
        assert!(export_ts_models(&missing_ts_export, &missing_ts_out).is_err());
        assert!(export_ts_constants(&missing_ts_export, &missing_ts_out).is_err());
        assert!(export_ts_wasm_artifacts(&missing_ts_export, &missing_ts_out).is_err());
        assert!(write_ts_export_manifest(&missing_ts_export, &missing_ts_out).is_err());
        assert!(generate_ts_rs_sources(&missing_ts_export).is_err());
        let _ = fs::remove_dir_all(&missing_ts_export);

        let selector_root = create_synthetic_workspace("wrapper_selector_errors", true);
        let selector_out = selector_root.join("out");
        fs::create_dir_all(&selector_out).expect("create selector out");
        let models_selector_err =
            export_ts_models_for_crate(&selector_root, &selector_out, "missing-crate")
                .expect_err("models unknown selector");
        assert!(models_selector_err.contains("unknown ts export crate selector"));
        let constants_selector_err =
            export_ts_constants_for_crate(&selector_root, &selector_out, "missing-crate")
                .expect_err("constants unknown selector");
        assert!(constants_selector_err.contains("unknown ts export crate selector"));
        let wasm_selector_err =
            export_ts_wasm_artifacts_for_crate(&selector_root, &selector_out, "missing-crate")
                .expect_err("wasm unknown selector");
        assert!(wasm_selector_err.contains("unknown ts export crate selector"));
        let generate_selector_err =
            generate_ts_rs_sources_for_crate(&selector_root, "missing-crate")
                .expect_err("generate unknown selector");
        assert!(generate_selector_err.contains("unknown ts export crate selector"));
        let bundle_selector_err =
            export_ts_bundle_for_crate(&selector_root, &selector_out, "missing-crate")
                .expect_err("bundle unknown selector");
        assert!(bundle_selector_err.contains("unknown ts export crate selector"));
        let _ = fs::remove_dir_all(&selector_root);
    }

    #[test]
    fn wrapper_calls_surface_artifact_and_copy_errors() {
        let missing_artifacts = create_synthetic_workspace("wrapper_missing_artifacts", true);
        write_file(
            &missing_artifacts
                .join("contract")
                .join("exports")
                .join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"
"#,
        );
        let missing_artifacts_out = missing_artifacts.join("out");
        fs::create_dir_all(&missing_artifacts_out).expect("create missing artifacts out");
        assert!(export_ts_models(&missing_artifacts, &missing_artifacts_out).is_err());
        assert!(export_ts_constants(&missing_artifacts, &missing_artifacts_out).is_err());
        assert!(export_ts_wasm_artifacts(&missing_artifacts, &missing_artifacts_out).is_err());
        assert!(write_ts_export_manifest(&missing_artifacts, &missing_artifacts_out).is_err());
        let _ = fs::remove_dir_all(&missing_artifacts);

        let missing_models_dir = create_synthetic_workspace("wrapper_missing_models_dir", true);
        write_file(
            &missing_models_dir
                .join("contract")
                .join("exports")
                .join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"

[artifacts]
models_dir = ""
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        let missing_models_out = missing_models_dir.join("out");
        fs::create_dir_all(&missing_models_out).expect("create missing models out");
        let missing_models_err =
            export_ts_models(&missing_models_dir, &missing_models_out).expect_err("models dir");
        assert!(missing_models_err.contains("artifacts fields must be non-empty for ts"));
        let _ = fs::remove_dir_all(&missing_models_dir);

        let missing_constants_dir =
            create_synthetic_workspace("wrapper_missing_constants_dir", true);
        write_file(
            &missing_constants_dir
                .join("contract")
                .join("exports")
                .join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"

[artifacts]
models_dir = "src/generated"
constants_dir = ""
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        let missing_constants_out = missing_constants_dir.join("out");
        fs::create_dir_all(&missing_constants_out).expect("create missing constants out");
        let missing_constants_err =
            export_ts_constants(&missing_constants_dir, &missing_constants_out)
                .expect_err("constants dir");
        assert!(missing_constants_err.contains("artifacts fields must be non-empty for ts"));
        let _ = fs::remove_dir_all(&missing_constants_dir);

        let missing_wasm_dir = create_synthetic_workspace("wrapper_missing_wasm_dir", true);
        write_file(
            &missing_wasm_dir
                .join("contract")
                .join("exports")
                .join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a-wasm" = "@radroots/a-wasm"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = ""
manifest_file = "export-manifest.json"
"#,
        );
        let missing_wasm_out = missing_wasm_dir.join("out");
        fs::create_dir_all(&missing_wasm_out).expect("create missing wasm out");
        let missing_wasm_err =
            export_ts_wasm_artifacts(&missing_wasm_dir, &missing_wasm_out).expect_err("wasm dir");
        assert!(missing_wasm_err.contains("artifacts fields must be non-empty for ts"));
        let _ = fs::remove_dir_all(&missing_wasm_dir);

        let missing_manifest_file =
            create_synthetic_workspace("wrapper_missing_manifest_file", true);
        write_file(
            &missing_manifest_file
                .join("contract")
                .join("exports")
                .join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a" = "@radroots/a"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = ""
"#,
        );
        let missing_manifest_out = missing_manifest_file.join("out");
        fs::create_dir_all(&missing_manifest_out).expect("create missing manifest out");
        let missing_manifest_err =
            write_ts_export_manifest(&missing_manifest_file, &missing_manifest_out)
                .expect_err("manifest file");
        assert!(missing_manifest_err.contains("artifacts fields must be non-empty for ts"));
        let _ = fs::remove_dir_all(&missing_manifest_file);

        let models_copy_err_root = create_synthetic_workspace("wrapper_models_copy_err", true);
        write_file(
            &models_copy_err_root
                .join("target")
                .join("ts-rs")
                .join("a")
                .join("types.ts"),
            "export type Probe = { id: string };\n",
        );
        let models_copy_out = models_copy_err_root.join("out");
        write_file(
            &models_copy_out.join("ts").join("packages").join("a"),
            "blocker",
        );
        let models_copy_err =
            export_ts_models(&models_copy_err_root, &models_copy_out).expect_err("copy models");
        assert!(models_copy_err.contains("create"));
        let _ = fs::remove_dir_all(&models_copy_err_root);

        let constants_copy_err_root =
            create_synthetic_workspace("wrapper_constants_copy_err", true);
        write_file(
            &constants_copy_err_root
                .join("target")
                .join("ts-rs")
                .join("a")
                .join("constants.ts"),
            "export const A = 1;\n",
        );
        write_file(
            &constants_copy_err_root
                .join("target")
                .join("ts-rs")
                .join("a")
                .join("kinds.ts"),
            "export const K = 1;\n",
        );
        let constants_copy_out = constants_copy_err_root.join("out");
        write_file(
            &constants_copy_out.join("ts").join("packages").join("a"),
            "blocker",
        );
        let constants_copy_err = export_ts_constants(&constants_copy_err_root, &constants_copy_out)
            .expect_err("copy constants");
        assert!(constants_copy_err.contains("create"));
        let _ = fs::remove_dir_all(&constants_copy_err_root);

        let wasm_copy_err_root = create_synthetic_workspace("wrapper_wasm_copy_err", true);
        write_file(
            &wasm_copy_err_root
                .join("contract")
                .join("exports")
                .join("ts.toml"),
            r#"[language]
id = "ts"
repository = "sdk-typescript"

[packages]
"radroots-a-wasm" = "@radroots/a-wasm"

[artifacts]
models_dir = "src/generated"
constants_dir = "src/generated"
wasm_dist_dir = "dist"
manifest_file = "export-manifest.json"
"#,
        );
        write_file(
            &wasm_copy_err_root
                .join("crates")
                .join("a-wasm")
                .join("pkg")
                .join("dist")
                .join("nested")
                .join("artifact.js"),
            "export const wasm = 1;\n",
        );
        let wasm_copy_out = wasm_copy_err_root.join("out");
        write_file(
            &wasm_copy_out
                .join("ts")
                .join("packages")
                .join("a-wasm")
                .join("dist")
                .join("nested"),
            "blocker",
        );
        let wasm_copy_err =
            export_ts_wasm_artifacts(&wasm_copy_err_root, &wasm_copy_out).expect_err("copy wasm");
        assert!(wasm_copy_err.contains("create"));
        let _ = fs::remove_dir_all(&wasm_copy_err_root);

        let manifest_collect_err_root =
            create_synthetic_workspace("wrapper_manifest_collect_err", true);
        let manifest_collect_out = manifest_collect_err_root.join("out");
        write_file(
            &manifest_collect_out.join("ts").join("packages"),
            "not-a-directory",
        );
        let manifest_collect_err =
            write_ts_export_manifest(&manifest_collect_err_root, &manifest_collect_out)
                .expect_err("manifest collect error");
        assert!(manifest_collect_err.contains("read dir"));
        let _ = fs::remove_dir_all(&manifest_collect_err_root);

        let recursive_copy_root = unique_temp_dir("wrapper_recursive_copy");
        let recursive_src = recursive_copy_root.join("src");
        let recursive_dst = recursive_copy_root.join("dst");
        write_file(&recursive_src.join("nested").join("value.txt"), "value");
        write_file(&recursive_dst.join("nested"), "blocker");
        let recursive_copy_err =
            copy_dir_contents(&recursive_src, &recursive_dst).expect_err("recursive copy error");
        assert!(recursive_copy_err.contains("create"));
        let _ = fs::remove_dir_all(&recursive_copy_root);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let recursive_manifest_root = unique_temp_dir("wrapper_recursive_manifest");
            let recursive_manifest_src = recursive_manifest_root.join("src");
            write_file(
                &recursive_manifest_src.join("nested").join("value.txt"),
                "value",
            );
            let mut restricted = fs::metadata(recursive_manifest_src.join("nested"))
                .expect("nested metadata")
                .permissions();
            restricted.set_mode(0o000);
            fs::set_permissions(recursive_manifest_src.join("nested"), restricted)
                .expect("set nested restricted mode");

            let mut entries = Vec::new();
            let recursive_manifest_err = collect_manifest_entries(
                &recursive_manifest_src,
                &recursive_manifest_src,
                &recursive_manifest_src.join("skip.json"),
                &mut entries,
            )
            .expect_err("recursive manifest error");
            assert!(recursive_manifest_err.contains("read dir"));

            let mut readable = fs::metadata(recursive_manifest_src.join("nested"))
                .expect("nested metadata")
                .permissions();
            readable.set_mode(0o755);
            fs::set_permissions(recursive_manifest_src.join("nested"), readable)
                .expect("restore nested readable mode");
            let _ = fs::remove_dir_all(&recursive_manifest_root);
        }
    }
}

#![forbid(unsafe_code)]

use crate::contract;
use serde::Serialize;
use sha2::{Digest, Sha256};
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
    bundle
        .exports
        .iter()
        .find(|mapping| mapping.language.id == "ts")
        .ok_or_else(|| "missing ts export mapping".to_string())
}

fn ts_artifacts(mapping: &contract::ExportMapping) -> Result<&contract::ExportArtifacts, String> {
    mapping
        .artifacts
        .as_ref()
        .ok_or_else(|| "missing ts artifacts mapping".to_string())
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
    value
        .as_deref()
        .filter(|item| !item.trim().is_empty())
        .ok_or_else(|| format!("missing ts artifacts.{field}"))
}

fn crate_supports_ts_rs(workspace_root: &Path, crate_dir: &str) -> Result<bool, String> {
    let manifest = workspace_root
        .join("crates")
        .join(crate_dir)
        .join("Cargo.toml");
    if !manifest.exists() {
        return Ok(false);
    }
    let raw =
        fs::read_to_string(&manifest).map_err(|e| format!("read {}: {e}", manifest.display()))?;
    Ok(raw.contains("ts-rs"))
}

fn copy_if_exists(src: &Path, dst: &Path) -> Result<bool, String> {
    if !src.exists() {
        return Ok(false);
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    fs::copy(src, dst).map_err(|e| format!("copy {} -> {}: {e}", src.display(), dst.display()))?;
    Ok(true)
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<usize, String> {
    if !src.exists() {
        return Ok(0);
    }
    fs::create_dir_all(dst).map_err(|e| format!("create {}: {e}", dst.display()))?;
    let mut copied = 0usize;
    let mut entries = fs::read_dir(src)
        .map_err(|e| format!("read dir {}: {e}", src.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read dir entries {}: {e}", src.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let target = dst.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|e| format!("read type {}: {e}", path.display()))?;
        if file_type.is_dir() {
            copied += copy_dir_contents(&path, &target)?;
        } else if file_type.is_file() {
            fs::copy(&path, &target)
                .map_err(|e| format!("copy {} -> {}: {e}", path.display(), target.display()))?;
            copied += 1;
        }
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
    let mut dir_entries = fs::read_dir(current)
        .map_err(|e| format!("read dir {}: {e}", current.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read dir entries {}: {e}", current.display()))?;
    dir_entries.sort_by_key(|entry| entry.file_name());
    for entry in dir_entries {
        let path = entry.path();
        if path == skip_path {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|e| format!("read type {}: {e}", path.display()))?;
        if file_type.is_dir() {
            collect_manifest_entries(root, &path, skip_path, entries)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let digest = Sha256::digest(&bytes);
        let relative = path
            .strip_prefix(root)
            .map_err(|e| format!("strip prefix {}: {e}", path.display()))?
            .to_string_lossy()
            .replace('\\', "/");
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
    let artifacts = ts_artifacts(ts_export)?;
    let models_dir = required_artifact_value(&artifacts.models_dir, "models_dir")?;
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
        if crate_name.ends_with("-wasm") || !crate_supports_ts_rs(workspace_root, crate_dir)? {
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
    let artifacts = ts_artifacts(ts_export)?;
    let constants_dir = required_artifact_value(&artifacts.constants_dir, "constants_dir")?;
    let ts_out_root = out_dir.join("ts").join("packages");
    for (crate_name, package_name) in selected_entries {
        let crate_dir = crate_name.strip_prefix("radroots-").unwrap_or(crate_name);
        let crate_root = workspace_root.join("crates").join(crate_dir);
        for filename in ["constants.ts", "kinds.ts"] {
            let candidates = [
                crate_root.join("bindings").join(filename),
                crate_root
                    .join("bindings")
                    .join("ts")
                    .join("src")
                    .join(filename),
            ];
            let src = candidates
                .iter()
                .find(|path| path.exists())
                .cloned()
                .unwrap_or_else(|| candidates[0].clone());
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
    let artifacts = ts_artifacts(ts_export)?;
    let wasm_dist_dir = required_artifact_value(&artifacts.wasm_dist_dir, "wasm_dist_dir")?;
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
    let artifacts = ts_artifacts(ts_export)?;
    let manifest_file = required_artifact_value(&artifacts.manifest_file, "manifest_file")?;
    let ts_root = out_dir.join("ts");
    let manifest_path = ts_root.join(manifest_file);
    let mut files = Vec::new();
    collect_manifest_entries(
        &ts_root,
        &ts_root.join("packages"),
        &manifest_path,
        &mut files,
    )?;
    let manifest = ExportManifest {
        language: ts_export.language.id.clone(),
        files,
    };
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| format!("serialize manifest {}: {e}", manifest_path.display()))?;
    fs::write(&manifest_path, bytes)
        .map_err(|e| format!("write {}: {e}", manifest_path.display()))?;
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
        fs::remove_dir_all(&source_root)
            .map_err(|e| format!("remove {}: {e}", source_root.display()))?;
    }
    fs::create_dir_all(&source_root)
        .map_err(|e| format!("create {}: {e}", source_root.display()))?;
    let mut expected = 0usize;
    for (crate_name, _) in &selected_entries {
        if crate_name.ends_with("-wasm") {
            continue;
        }
        let crate_dir = crate_name.strip_prefix("radroots-").unwrap_or(crate_name);
        if crate_supports_ts_rs(workspace_root, crate_dir)? {
            expected += 1;
        }
    }
    if expected == 0 {
        return Ok(source_root);
    }
    let mut generated = 0usize;
    for (crate_name, package_name) in selected_entries {
        if crate_name.ends_with("-wasm") {
            continue;
        }
        let crate_dir = crate_name.strip_prefix("radroots-").unwrap_or(crate_name);
        if !crate_supports_ts_rs(workspace_root, crate_dir)? {
            continue;
        }
        let package_dir = package_name
            .strip_prefix("@radroots/")
            .unwrap_or(package_name);
        let export_dir = source_root.join(package_dir);
        fs::create_dir_all(&export_dir)
            .map_err(|e| format!("create {}: {e}", export_dir.display()))?;
        let status = Command::new("cargo")
            .arg("test")
            .arg("-q")
            .arg("-p")
            .arg(crate_name)
            .arg("--features")
            .arg("ts-rs")
            .env("RADROOTS_TS_RS_EXPORT_DIR", &export_dir)
            .current_dir(workspace_root)
            .status()
            .map_err(|e| format!("run cargo test for {crate_name}: {e}"))?;
        if !status.success() {
            return Err(format!("cargo test failed for {crate_name}"));
        }
        generated += 1;
    }
    if generated == 0 {
        return Err("no ts-rs model sources were generated".to_string());
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
        let events_root = root.join("crates").join("events");
        let constants_exists = events_root.join("bindings").join("constants.ts").exists()
            || events_root
                .join("bindings")
                .join("ts")
                .join("src")
                .join("constants.ts")
                .exists();
        let kinds_exists = events_root.join("bindings").join("kinds.ts").exists()
            || events_root
                .join("bindings")
                .join("ts")
                .join("src")
                .join("kinds.ts")
                .exists();
        if constants_exists {
            assert!(events_constants.exists());
        }
        if kinds_exists {
            assert!(events_kinds.exists());
        }

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
}

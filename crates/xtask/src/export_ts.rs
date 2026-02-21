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

pub fn export_ts_models(workspace_root: &Path, out_dir: &Path) -> Result<(), String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = ts_export_mapping(&bundle)?;
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
    let mut copied = 0usize;
    for (crate_name, package_name) in &ts_export.packages {
        let crate_dir = crate_name.strip_prefix("radroots-").unwrap_or(crate_name);
        let src = source_root.join(crate_dir).join("types.ts");
        let dst = to_package_dir(&ts_out_root, package_name)
            .join(models_dir)
            .join("types.ts");
        if copy_if_exists(&src, &dst)? {
            copied += 1;
        }
    }
    if copied == 0 {
        return Err("no ts model files were exported".to_string());
    }
    Ok(())
}

pub fn export_ts_constants(workspace_root: &Path, out_dir: &Path) -> Result<(), String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = ts_export_mapping(&bundle)?;
    let artifacts = ts_artifacts(ts_export)?;
    let constants_dir = required_artifact_value(&artifacts.constants_dir, "constants_dir")?;
    let ts_out_root = out_dir.join("ts").join("packages");
    for (crate_name, package_name) in &ts_export.packages {
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

pub fn export_ts_wasm_artifacts(workspace_root: &Path, out_dir: &Path) -> Result<(), String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = ts_export_mapping(&bundle)?;
    let artifacts = ts_artifacts(ts_export)?;
    let wasm_dist_dir = required_artifact_value(&artifacts.wasm_dist_dir, "wasm_dist_dir")?;
    let ts_out_root = out_dir.join("ts").join("packages");
    let mut copied = 0usize;
    for (crate_name, package_name) in &ts_export.packages {
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

pub fn generate_ts_rs_sources(workspace_root: &Path) -> Result<PathBuf, String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = ts_export_mapping(&bundle)?;
    let source_root = workspace_root.join("target").join("ts-rs");
    if source_root.exists() {
        fs::remove_dir_all(&source_root)
            .map_err(|e| format!("remove {}: {e}", source_root.display()))?;
    }
    fs::create_dir_all(&source_root)
        .map_err(|e| format!("create {}: {e}", source_root.display()))?;
    let mut generated = 0usize;
    for (crate_name, package_name) in &ts_export.packages {
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

pub fn export_ts_bundle(workspace_root: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    generate_ts_rs_sources(workspace_root)?;
    export_ts_models(workspace_root, out_dir)?;
    export_ts_constants(workspace_root, out_dir)?;
    export_ts_wasm_artifacts(workspace_root, out_dir)?;
    write_ts_export_manifest(workspace_root, out_dir)
}

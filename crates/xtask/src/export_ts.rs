#![forbid(unsafe_code)]

use crate::contract;
use std::fs;
use std::path::{Path, PathBuf};

fn to_package_dir(base: &Path, package_name: &str) -> PathBuf {
    let stripped = package_name
        .strip_prefix("@radroots/")
        .unwrap_or(package_name);
    base.join(stripped)
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

pub fn export_ts_models(workspace_root: &Path, out_dir: &Path) -> Result<(), String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = bundle
        .exports
        .iter()
        .find(|mapping| mapping.language.id == "ts")
        .ok_or_else(|| "missing ts export mapping".to_string())?;
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
            .join("src")
            .join("generated")
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
    let ts_export = bundle
        .exports
        .iter()
        .find(|mapping| mapping.language.id == "ts")
        .ok_or_else(|| "missing ts export mapping".to_string())?;
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
                .join("src")
                .join("generated")
                .join(filename);
            copy_if_exists(&src, &dst)?;
        }
    }
    Ok(())
}

pub fn export_ts_wasm_artifacts(workspace_root: &Path, out_dir: &Path) -> Result<(), String> {
    let bundle = contract::load_contract_bundle(workspace_root)?;
    contract::validate_contract_bundle(&bundle)?;
    let ts_export = bundle
        .exports
        .iter()
        .find(|mapping| mapping.language.id == "ts")
        .ok_or_else(|| "missing ts export mapping".to_string())?;
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
        let target_root = to_package_dir(&ts_out_root, package_name).join("dist");
        copied += copy_dir_contents(&source_root, &target_root)?;
    }
    if copied == 0 {
        return Err("no ts wasm files were exported".to_string());
    }
    Ok(())
}

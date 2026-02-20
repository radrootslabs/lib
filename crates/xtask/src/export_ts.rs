#![forbid(unsafe_code)]

use crate::contract;
use std::fs;
use std::path::{Path, PathBuf};

fn to_package_dir(base: &Path, package_name: &str) -> PathBuf {
    let stripped = package_name.strip_prefix("@radroots/").unwrap_or(package_name);
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
        return Err(format!("missing ts-rs source root {}", source_root.display()));
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

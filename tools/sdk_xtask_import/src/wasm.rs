use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    check::check_wasm_package_surface,
    fs::workspace_root,
    package_matrix::{WasmPackageSpec, validate_package_matrix, wasm_package_specs},
};

const WASM_TARGET: &str = "wasm32-unknown-unknown";

pub fn generate(args: &[String]) -> Result<(), String> {
    validate_package_matrix()?;
    let specs = selected_specs(args)?;
    let root = workspace_root()?;
    let toolchain = resolve_wasm_toolchain()?;
    for spec in specs {
        let dist_dir = root.join(spec.package_dir).join("dist");
        if dist_dir.exists() {
            fs::remove_dir_all(&dist_dir)
                .map_err(|error| format!("failed to remove {}: {error}", dist_dir.display()))?;
        }
        let mut command = Command::new(&toolchain.wasm_pack);
        command.current_dir(&root);
        for arg in wasm_pack_args(spec) {
            command.arg(arg);
        }
        if let Some(parent) = toolchain.rustc.parent() {
            prepend_path(&mut command, parent);
        }
        command.env("RUSTC", &toolchain.rustc);
        command.env("CARGO", &toolchain.cargo);
        let status = command.status().map_err(|error| {
            format!(
                "failed to start wasm-pack for {} while generating {}: {error}",
                spec.key, spec.package_name
            )
        })?;
        if !status.success() {
            return Err(format!(
                "wasm-pack failed for {} while generating {} with status {status}; rerun `cargo xtask generate wasm --package {}` after fixing the wasm toolchain",
                spec.key, spec.package_name, spec.key
            ));
        }
        check_wasm_package_surface(&root, spec)?;
        println!("generated wasm package {}", spec.package_name);
    }
    Ok(())
}

struct WasmToolchain {
    wasm_pack: PathBuf,
    rustc: PathBuf,
    cargo: PathBuf,
}

fn resolve_wasm_toolchain() -> Result<WasmToolchain, String> {
    let wasm_pack = resolve_required_path_tool("wasm-pack")?;
    let rustc = resolve_required_rust_tool("rustc", "RUSTC")?;
    let cargo = resolve_required_rust_tool("cargo", "CARGO")?;
    ensure_wasm_target_installed()?;
    Ok(WasmToolchain {
        wasm_pack,
        rustc,
        cargo,
    })
}

fn wasm_pack_args(spec: WasmPackageSpec) -> Vec<&'static str> {
    vec![
        "build",
        spec.crate_dir,
        "--release",
        "--target",
        "web",
        "--out-dir",
        spec.out_dir,
        "--out-name",
        spec.out_name,
        "--no-pack",
    ]
}

fn selected_specs(args: &[String]) -> Result<Vec<WasmPackageSpec>, String> {
    match args {
        [] => Ok(wasm_package_specs().to_vec()),
        [flag, key] if flag == "--package" => wasm_package_specs()
            .iter()
            .copied()
            .find(|spec| spec.key == key)
            .map(|spec| vec![spec])
            .ok_or_else(|| format!("unknown wasm package: {key}")),
        _ => Err("usage: cargo xtask generate wasm [--package <key>]".to_owned()),
    }
}

fn resolve_required_path_tool(name: &str) -> Result<PathBuf, String> {
    let path = env::var_os("PATH").ok_or_else(|| {
        format!("missing {name}: PATH is not set; install {name} and expose it on PATH")
    })?;
    resolve_path_tool_from_path(name, &path)
}

fn resolve_path_tool_from_path(name: &str, path: &std::ffi::OsStr) -> Result<PathBuf, String> {
    let matches = executable_matches(name, path);
    match matches.as_slice() {
        [] => Err(format!(
            "missing {name}: install {name} and rerun `cargo xtask generate wasm`"
        )),
        [tool] => Ok(tool.clone()),
        _ => Err(format!(
            "ambiguous {name}: found {}; remove duplicate {name} entries from PATH before running `cargo xtask generate wasm`",
            matches
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn executable_matches(name: &str, path: &std::ffi::OsStr) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut matches = Vec::new();
    for dir in env::split_paths(path) {
        let candidate = dir.join(name);
        if !is_executable_file(&candidate) {
            continue;
        }
        let key = fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
        if seen.insert(key) {
            matches.push(candidate);
        }
    }
    matches
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn resolve_required_rust_tool(name: &str, env_var: &str) -> Result<PathBuf, String> {
    if let Some(path) = explicit_tool_path(env_var) {
        return Ok(PathBuf::from(path));
    }
    rustup_tool(name).ok_or_else(|| {
        format!(
            "missing rustup resolution for {name}: set {env_var} explicitly or install rustup with the {WASM_TARGET} target"
        )
    })
}

fn explicit_tool_path(env_var: &str) -> Option<String> {
    let value = env::var(env_var).ok()?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn rustup_tool(name: &str) -> Option<PathBuf> {
    let output = Command::new("rustup")
        .arg("which")
        .arg(name)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let trimmed = path.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn ensure_wasm_target_installed() -> Result<(), String> {
    let output = Command::new("rustup")
        .arg("target")
        .arg("list")
        .arg("--installed")
        .output()
        .map_err(|error| {
            format!(
                "failed to verify {WASM_TARGET} target with rustup: {error}; install rustup or set RUSTC/CARGO from a toolchain that supports {WASM_TARGET}"
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "failed to verify {WASM_TARGET} target with rustup: {}; run `rustup target add {WASM_TARGET}`",
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !target_list_contains(&stdout, WASM_TARGET) {
        return Err(format!(
            "missing Rust target {WASM_TARGET}: run `rustup target add {WASM_TARGET}`"
        ));
    }
    Ok(())
}

fn target_list_contains(output: &str, target: &str) -> bool {
    output.lines().any(|line| line.trim() == target)
}

fn prepend_path(command: &mut Command, prefix: &Path) {
    let existing = env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![prefix.to_path_buf()];
    paths.extend(env::split_paths(&existing));
    if let Ok(joined) = env::join_paths(paths) {
        command.env("PATH", joined);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::package_matrix::wasm_package_specs;

    use super::{
        resolve_path_tool_from_path, rustup_tool, selected_specs, target_list_contains,
        wasm_pack_args,
    };

    #[test]
    fn selects_all_specs_by_default() {
        assert_eq!(selected_specs(&[]).expect("all specs").len(), 3);
    }

    #[test]
    fn selects_one_spec_by_key() {
        let specs = selected_specs(&["--package".to_owned(), "replica_db".to_owned()])
            .expect("replica db spec");
        assert_eq!(specs[0].package_name, "@radroots/replica-db-wasm");
    }

    #[test]
    fn rejects_unknown_spec_key() {
        assert!(selected_specs(&["--package".to_owned(), "missing".to_owned()]).is_err());
    }

    #[test]
    fn wasm_pack_arguments_disable_package_manifest_generation() {
        let args = wasm_pack_args(wasm_package_specs()[0]);
        assert!(args.contains(&"--no-pack"));
    }

    #[test]
    fn path_tool_resolution_reports_missing_tools() {
        let error = resolve_path_tool_from_path("wasm-pack", std::ffi::OsStr::new(""))
            .expect_err("missing");
        assert!(error.contains("missing wasm-pack"));
    }

    #[test]
    fn path_tool_resolution_reports_ambiguous_tools() {
        let root = test_root("ambiguous_wasm_pack");
        let first = root.join("first");
        let second = root.join("second");
        fs::create_dir_all(&first).expect("create first dir");
        fs::create_dir_all(&second).expect("create second dir");
        write_executable(first.join("wasm-pack"));
        write_executable(second.join("wasm-pack"));
        let path = env::join_paths([first, second]).expect("join path");

        let error =
            resolve_path_tool_from_path("wasm-pack", &path).expect_err("ambiguous wasm-pack");

        assert!(error.contains("ambiguous wasm-pack"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn target_list_parser_requires_exact_target() {
        assert!(target_list_contains(
            "aarch64-apple-darwin\nwasm32-unknown-unknown\n",
            "wasm32-unknown-unknown"
        ));
        assert!(!target_list_contains(
            "wasm32-unknown-emscripten\n",
            "wasm32-unknown-unknown"
        ));
    }

    #[test]
    fn rustup_tool_resolution_is_non_panicking() {
        let _ = rustup_tool("rustc");
    }

    fn write_executable(path: PathBuf) {
        fs::write(&path, "#!/bin/sh\n").expect("write executable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).expect("set executable permissions");
        }
    }

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        env::temp_dir().join(format!("radroots_sdk_xtask_{name}_{stamp}"))
    }
}

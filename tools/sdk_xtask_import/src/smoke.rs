use std::{fs, path::Path, process::Command};

use crate::{
    fs::workspace_root,
    wasm::{ResolvedRustToolchain, resolve_rust_toolchain},
};

pub fn run(args: &[String]) -> Result<(), String> {
    match args {
        [target] if target == "sdk-rust-local" => sdk_rust_local(),
        [target] if target == "facade-rust-local" => facade_rust_local(),
        [target] if target == "front-doors-rust-local" => {
            facade_rust_local()?;
            sdk_rust_local()
        }
        _ => Err(
            "usage: cargo xtask smoke facade-rust-local | sdk-rust-local | front-doors-rust-local"
                .to_owned(),
        ),
    }
}

fn facade_rust_local() -> Result<(), String> {
    let root = workspace_root()?;
    let facade_path = root.join("crates/radroots");
    let lib_root = sibling_lib_root(&root)?;
    let toolchain = resolve_rust_toolchain(&root)?;
    run_clean_consumer(
        "facade",
        &render_facade_consumer_manifest(&facade_path, &lib_root)?,
        FACADE_CONSUMER_MAIN,
        &toolchain,
    )
}

fn sdk_rust_local() -> Result<(), String> {
    let root = workspace_root()?;
    let sdk_path = root.join("crates/sdk");
    let lib_root = sibling_lib_root(&root)?;
    let toolchain = resolve_rust_toolchain(&root)?;
    run_clean_consumer(
        "SDK",
        &render_sdk_consumer_manifest(&sdk_path, &lib_root)?,
        SDK_CONSUMER_MAIN,
        &toolchain,
    )
}

fn sibling_lib_root(root: &Path) -> Result<std::path::PathBuf, String> {
    Ok(root
        .parent()
        .ok_or_else(|| format!("{} has no repository parent", root.display()))?
        .join("lib"))
}

fn run_clean_consumer(
    label: &str,
    manifest: &str,
    main: &str,
    toolchain: &ResolvedRustToolchain,
) -> Result<(), String> {
    let tempdir =
        tempfile::tempdir().map_err(|error| format!("failed to create smoke tempdir: {error}"))?;
    fs::write(tempdir.path().join("Cargo.toml"), manifest)
        .map_err(|error| format!("failed to write {label} smoke manifest: {error}"))?;
    let src_dir = tempdir.path().join("src");
    fs::create_dir_all(&src_dir)
        .map_err(|error| format!("failed to create {}: {error}", src_dir.display()))?;
    fs::write(src_dir.join("main.rs"), main)
        .map_err(|error| format!("failed to write {label} smoke consumer: {error}"))?;
    let status = smoke_cargo_run_command(tempdir.path(), toolchain)
        .status()
        .map_err(|error| format!("failed to run {label} smoke consumer: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{label} Rust local smoke failed with status {status}"
        ))
    }
}

fn render_facade_consumer_manifest(facade_path: &Path, lib_root: &Path) -> Result<String, String> {
    let facade_path = serde_json::to_string(&facade_path.to_string_lossy())
        .map_err(|error| format!("failed to render facade path: {error}"))?;
    let mut manifest = format!(
        r#"[package]
name = "radroots_facade_host_smoke"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
radroots = {{ path = {facade_path} }}

[patch.crates-io]
"#,
    );
    append_lib_patches(&mut manifest, lib_root)?;
    Ok(manifest)
}

fn render_sdk_consumer_manifest(sdk_path: &Path, lib_root: &Path) -> Result<String, String> {
    let sdk_path = serde_json::to_string(&sdk_path.to_string_lossy())
        .map_err(|error| format!("failed to render SDK path: {error}"))?;
    let mut manifest = format!(
        r#"[package]
name = "radroots_sdk_host_smoke"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
radroots_sdk = {{ path = {sdk_path}, default-features = true }}

[patch.crates-io]
"#,
    );
    append_lib_patches(&mut manifest, lib_root)?;
    Ok(manifest)
}

fn append_lib_patches(manifest: &mut String, lib_root: &Path) -> Result<(), String> {
    for (package, relative_path) in [
        ("radroots_blossom", "crates/blossom"),
        ("radroots_core", "crates/core"),
        ("radroots_event", "crates/event"),
        ("radroots_event_codec", "crates/event_codec"),
        ("radroots_geonames", "crates/geonames"),
        ("radroots_identity", "crates/identity"),
        ("radroots_nostr", "crates/nostr"),
        ("radroots_nostr_connect", "crates/nostr_connect"),
        ("radroots_protocol", "crates/protocol"),
        ("radroots_secrets", "crates/secrets"),
        ("radroots_signing", "crates/signing"),
        ("radroots_storage", "crates/storage"),
        ("radroots_storage_sqlite", "crates/storage_sqlite"),
        ("radroots_sync", "crates/sync"),
        ("radroots_trade", "crates/trade"),
        ("radroots_transport", "crates/transport"),
        ("radroots_transport_nostr", "crates/transport_nostr"),
    ] {
        let path = serde_json::to_string(&lib_root.join(relative_path).to_string_lossy())
            .map_err(|error| format!("failed to render {package} patch path: {error}"))?;
        manifest.push_str(&format!("{package} = {{ path = {path} }}\n"));
    }
    Ok(())
}

fn smoke_cargo_run_command(path: &Path, toolchain: &ResolvedRustToolchain) -> Command {
    let mut command = Command::new(&toolchain.cargo);
    command.arg("run").arg("--quiet").current_dir(path);
    toolchain.apply_to_command(&mut command);
    command
}

const SDK_CONSUMER_MAIN: &str = r#"use radroots_sdk::{ClientBuilder, error::ErrorKind};

fn main() {
    let error = match ClientBuilder::new().build() {
        Ok(_) => panic!("empty SDK builder must fail closed"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), ErrorKind::MissingStorage);
}
"#;

const FACADE_CONSUMER_MAIN: &str = r#"fn main() -> radroots::Result<()> {
    let client = radroots::client::memory().build()?;
    assert!(!client.is_closed());
    assert!(radroots::client::local_only().is_local_only());
    Ok(())
}
"#;

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::wasm::ResolvedRustToolchain;

    use super::{
        render_facade_consumer_manifest, render_sdk_consumer_manifest, run, smoke_cargo_run_command,
    };

    #[test]
    fn retired_sdk_knowledge_smoke_is_not_a_command_surface() {
        assert!(run(&["knowledge-rust-local".to_owned()]).is_err());
    }

    #[test]
    fn sdk_smoke_consumer_uses_final_api_and_local_lower_package_patches() {
        let manifest = render_sdk_consumer_manifest(
            Path::new("/tmp/radroots_sdk"),
            Path::new("/tmp/radroots_lib"),
        )
        .expect("manifest");

        assert!(manifest.contains("name = \"radroots_sdk_host_smoke\""));
        assert!(manifest.contains("radroots_sdk = { path = \"/tmp/radroots_sdk\""));
        assert!(
            manifest.contains("radroots_storage = { path = \"/tmp/radroots_lib/crates/storage\" }")
        );
        assert!(!manifest.contains("runtime ="));
        assert!(!manifest.contains("signer-adapters"));
    }

    #[test]
    fn facade_smoke_consumer_uses_curated_front_door() {
        let manifest = render_facade_consumer_manifest(
            Path::new("/tmp/radroots_facade"),
            Path::new("/tmp/radroots_lib"),
        )
        .expect("manifest");

        assert!(manifest.contains("name = \"radroots_facade_host_smoke\""));
        assert!(manifest.contains("radroots = { path = \"/tmp/radroots_facade\" }"));
        assert!(!manifest.contains("radroots_sdk = { path"));
    }

    #[test]
    fn smoke_command_uses_resolved_cargo_path() {
        let cargo = PathBuf::from("/tmp/rust-toolchain/bin/cargo");
        let toolchain = ResolvedRustToolchain {
            channel: "1.97.0".to_owned(),
            rustc: PathBuf::from("/tmp/rust-toolchain/bin/rustc"),
            cargo: cargo.clone(),
            bin_dir: PathBuf::from("/tmp/rust-toolchain/bin"),
        };

        let command = smoke_cargo_run_command(Path::new("/tmp/smoke"), &toolchain);

        assert_eq!(command.get_program(), cargo.as_os_str());
        assert_eq!(command.get_args().next(), Some(std::ffi::OsStr::new("run")));
    }
}

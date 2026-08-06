use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{build_control::Mode, sdk_generation::fs::write_or_check};

const FFI_PACKAGE: &str = "radroots_sdk_ffi";
const FFI_VERSION: &str = "0.1.0-alpha";
const BINDGEN_VERSION: &str = "0.29.5";

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SourceLock {
    schema_version: u16,
    source_package: String,
    source_version: String,
    source_sha256: String,
    generator: String,
    generator_version: String,
    language: String,
    outputs: BTreeMap<String, String>,
}

pub fn generate(
    source_root: &Path,
    consumer_root: &Path,
    language: &str,
    mode: Mode,
) -> Result<(), String> {
    if !matches!(language, "swift" | "kotlin") {
        return Err("SDK binding language must be swift or kotlin".to_owned());
    }
    build_bindgen(source_root)?;
    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| "CARGO_TARGET_DIR must be set by extbuild for FFI generation".to_owned())?;
    let library = target_dir.join("debug").join(format!(
        "{}radroots_sdk_ffi{}",
        env::consts::DLL_PREFIX,
        env::consts::DLL_SUFFIX
    ));
    if !library.is_file() {
        return Err(format!("missing built FFI library: {}", library.display()));
    }
    let bindgen = target_dir
        .join("debug")
        .join(format!("radroots-sdk-bindgen{}", env::consts::EXE_SUFFIX));
    if !bindgen.is_file() {
        return Err(format!(
            "missing FFI bindgen executable: {}",
            bindgen.display()
        ));
    }
    let temporary = tempfile::tempdir()
        .map_err(|error| format!("failed to create binding output directory: {error}"))?;
    let status = Command::new(&bindgen)
        .args(["generate", "--library"])
        .arg(&library)
        .args([
            "--crate",
            FFI_PACKAGE,
            "--language",
            language,
            "--no-format",
            "--out-dir",
        ])
        .arg(temporary.path())
        .current_dir(source_root)
        .status()
        .map_err(|error| format!("failed to start {}: {error}", bindgen.display()))?;
    if !status.success() {
        return Err(format!(
            "{language} binding generation failed with {status}"
        ));
    }
    install_outputs(source_root, consumer_root, language, temporary.path(), mode)?;
    println!("generated {language} bindings for {FFI_PACKAGE}");
    Ok(())
}

fn build_bindgen(root: &Path) -> Result<(), String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .args([
            "build",
            "--locked",
            "-p",
            FFI_PACKAGE,
            "--lib",
            "--bin",
            "radroots-sdk-bindgen",
        ])
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to start Cargo for FFI generation: {error}"))?;
    if !status.success() {
        return Err(format!(
            "FFI library and bindgen build failed with {status}"
        ));
    }
    Ok(())
}

fn install_outputs(
    source_root: &Path,
    consumer_root: &Path,
    language: &str,
    temporary: &Path,
    mode: Mode,
) -> Result<(), String> {
    let output_dir = consumer_root.join("generated").join(language);
    let mut generated = Vec::new();
    collect_files(temporary, &mut generated)?;
    generated.sort();
    let mut outputs = BTreeMap::new();
    for path in generated {
        let name = path
            .strip_prefix(temporary)
            .map_err(|error| format!("failed to relativize {}: {error}", path.display()))?
            .to_str()
            .ok_or_else(|| format!("non-UTF-8 generated binding path: {}", path.display()))?
            .replace('\\', "/");
        let contents = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let destination = output_dir.join(&name);
        let text = String::from_utf8(contents)
            .map_err(|error| format!("generated text binding is not UTF-8: {error}"))?;
        let text = normalize_generated_text(&text);
        outputs.insert(
            name.clone(),
            format!("{:x}", Sha256::digest(text.as_bytes())),
        );
        write_or_check(&destination, &text, mode)?;
    }
    if outputs.is_empty() {
        return Err(format!("{language} binding generator emitted no files"));
    }
    let lock = SourceLock {
        schema_version: 1,
        source_package: FFI_PACKAGE.to_owned(),
        source_version: FFI_VERSION.to_owned(),
        source_sha256: ffi_source_sha256(source_root)?,
        generator: "uniffi".to_owned(),
        generator_version: BINDGEN_VERSION.to_owned(),
        language: language.to_owned(),
        outputs,
    };
    let mut rendered = serde_json::to_string_pretty(&lock)
        .map_err(|error| format!("failed to render {language} source lock: {error}"))?;
    rendered.push('\n');
    write_or_check(&output_dir.join("source.lock"), &rendered, mode)
}

fn normalize_generated_text(contents: &str) -> String {
    let mut normalized = contents
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    while normalized.ends_with('\n') {
        normalized.pop();
    }
    normalized.push('\n');
    normalized
}

pub fn check(source_root: &Path, consumer_root: &Path) -> Result<(), String> {
    check_language(
        source_root,
        consumer_root,
        "swift",
        &[
            "radroots_sdk.swift",
            "radroots_sdkFFI.h",
            "radroots_sdkFFI.modulemap",
        ],
    )?;
    check_language(
        source_root,
        consumer_root,
        "kotlin",
        &["uniffi/radroots_sdk/radroots_sdk.kt"],
    )?;
    check_kotlin_schema_inventory(consumer_root)
}

fn check_kotlin_schema_inventory(root: &Path) -> Result<(), String> {
    let path = root.join("generated/kotlin/uniffi/radroots_sdk/radroots_sdk.kt");
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    for required in [
        "class MobileClient",
        "data class CapabilityStatus",
        "enum class CapabilityAvailability",
        "enum class CapabilityMaturity",
        "sealed class Exception",
    ] {
        if !source.contains(required) {
            return Err(format!(
                "generated Kotlin schema inventory is missing {required}: {}",
                path.display()
            ));
        }
    }
    for forbidden in ["Keychain", "BackgroundTask", "Presentation"] {
        if source.contains(forbidden) {
            return Err(format!(
                "generated Kotlin bindings contain host-owned {forbidden} API: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn check_language(
    source_root: &Path,
    consumer_root: &Path,
    language: &str,
    expected: &[&str],
) -> Result<(), String> {
    let output_dir = consumer_root.join("generated").join(language);
    let mut actual_paths = Vec::new();
    collect_files(&output_dir, &mut actual_paths)?;
    let actual = actual_paths
        .into_iter()
        .map(|path| {
            path.strip_prefix(&output_dir)
                .map(|relative| relative.to_string_lossy().replace('\\', "/"))
                .map_err(|error| format!("failed to relativize {}: {error}", path.display()))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let expected_inventory = expected
        .iter()
        .map(|name| (*name).to_owned())
        .chain(["source.lock".to_owned()])
        .collect::<BTreeSet<_>>();
    if actual != expected_inventory {
        return Err(format!(
            "unexpected generated {language} binding inventory: expected {expected_inventory:?}, found {actual:?}"
        ));
    }
    let lock_path = output_dir.join("source.lock");
    let raw = fs::read_to_string(&lock_path)
        .map_err(|error| format!("failed to read {}: {error}", lock_path.display()))?;
    let actual: SourceLock = serde_json::from_str(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", lock_path.display()))?;
    let mut outputs = BTreeMap::new();
    for name in expected {
        let path = output_dir.join(name);
        let contents = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        outputs.insert(
            (*name).to_owned(),
            format!("{:x}", Sha256::digest(contents)),
        );
    }
    let expected_lock = SourceLock {
        schema_version: 1,
        source_package: FFI_PACKAGE.to_owned(),
        source_version: FFI_VERSION.to_owned(),
        source_sha256: ffi_source_sha256(source_root)?,
        generator: "uniffi".to_owned(),
        generator_version: BINDGEN_VERSION.to_owned(),
        language: language.to_owned(),
        outputs,
    };
    if actual != expected_lock {
        return Err(format!(
            "stale generated {language} bindings: {}",
            lock_path.display()
        ));
    }
    Ok(())
}

fn ffi_source_sha256(root: &Path) -> Result<String, String> {
    let crate_dir = root.join("crates/sdk_ffi");
    let mut paths = vec![crate_dir.join("Cargo.toml"), crate_dir.join("README.md")];
    collect_files(&crate_dir.join("src"), &mut paths)?;
    paths.sort();
    let mut hasher = Sha256::new();
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("failed to relativize {}: {error}", path.display()))?;
        hasher.update(relative.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(
            fs::read(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?,
        );
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_files(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(dir)
        .map_err(|error| format!("failed to read {}: {error}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to enumerate {}: {error}", dir.display()))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, paths)?;
        } else if path.is_file() {
            paths.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use sha2::{Digest, Sha256};

    use crate::build_control::Mode;

    use super::{check, generate};

    #[test]
    #[ignore = "requires the native UniFFI bindgen toolchain"]
    fn canonical_bindgen_matches_the_frozen_sdk_surface() {
        let consumer = tempfile::TempDir::new().expect("consumer fixture");
        let source = crate::workspace_root();
        generate(&source, consumer.path(), "swift", Mode::Write).expect("Swift generation");
        generate(&source, consumer.path(), "kotlin", Mode::Write).expect("Kotlin generation");
        check(&source, consumer.path()).expect("fresh generated bindings");

        let expected = BTreeMap::from([
            (
                "generated/swift/radroots_sdk.swift",
                "0f81256c17243e485d9bca8f2dd4771c0ed2c2c1dc4fb03eb352389aab8599b0",
            ),
            (
                "generated/swift/radroots_sdkFFI.h",
                "511d3a22f969ef21d1a6d7357241756c2fda5d96adbb8bb17f2da5e8c65bcc54",
            ),
            (
                "generated/swift/radroots_sdkFFI.modulemap",
                "b18fa5af1567a940fc699afe371a3a4653dca206f3233bcdf322062ab8192170",
            ),
            (
                "generated/kotlin/uniffi/radroots_sdk/radroots_sdk.kt",
                "63a91b7c7790ee786d1c94cfad30cbe6e3ac5c737108ec61c74696fc068ef7da",
            ),
        ]);
        for (relative, expected_sha256) in expected {
            let bytes = fs::read(consumer.path().join(relative)).expect("generated binding");
            assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected_sha256);
        }
    }
}

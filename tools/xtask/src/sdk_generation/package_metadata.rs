use std::{fs, path::Path};

use crate::{
    build_control::Mode,
    sdk_generation::{
        fs::{write_bytes_if_changed, write_or_check},
        package_matrix::{package_specs, wasm_package_specs},
    },
};

pub(crate) const PACKAGE_README_FILE: &str = "README.md";
pub(crate) const PACKAGE_LICENSE_FILES: [&str; 2] = ["LICENSE-MIT", "LICENSE-APACHE"];
pub(crate) fn generate_package_metadata(
    source_root: &Path,
    consumer_root: &Path,
    mode: Mode,
) -> Result<(), String> {
    for spec in package_specs() {
        write_package_metadata(source_root, &consumer_root.join(spec.package_dir), mode)?;
        println!("generated package metadata {}", spec.package_name);
    }
    for spec in wasm_package_specs() {
        write_package_metadata(source_root, &consumer_root.join(spec.package_dir), mode)?;
        println!("generated package metadata {}", spec.package_name);
    }
    Ok(())
}

pub(crate) fn package_description<'a>(
    json: &'a serde_json::Value,
    package_json_path: &Path,
) -> Result<&'a str, String> {
    let description = json
        .get("description")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "package.json missing description: {}",
                package_json_path.display()
            )
        })?;
    if description.trim().is_empty() || description.trim() != description {
        return Err(format!(
            "package.json description must be non-empty and trimmed: {}",
            package_json_path.display()
        ));
    }
    Ok(description)
}

pub(crate) fn package_readme(package_name: &str, description: &str) -> String {
    format!(
        "# {package_name}\n\n{description}\n\nThis package publishes generated ESM JavaScript, TypeScript declarations, and any runtime artifacts from the Radroots SDK build pipeline. Runtime files are distributed from `dist/`; source and provenance metadata are kept outside the npm package payload.\n\n## License\n\nLicensed under either MIT or Apache-2.0, at your option. See `LICENSE-MIT` and `LICENSE-APACHE`.\n"
    )
}

fn write_package_metadata(
    source_root: &Path,
    package_dir: &Path,
    mode: Mode,
) -> Result<bool, String> {
    let package_json_path = package_dir.join("package.json");
    let json = read_package_json(&package_json_path)?;
    let package_name = package_name(&json, &package_json_path)?;
    let description = package_description(&json, &package_json_path)?;
    let readme_path = package_dir.join(PACKAGE_README_FILE);
    let readme = package_readme(package_name, description);
    let previous_readme = fs::read_to_string(&readme_path).ok();
    write_or_check(&readme_path, &readme, mode)?;
    let mut changed = previous_readme.as_deref() != Some(readme.as_str());
    for file_name in PACKAGE_LICENSE_FILES {
        let source_path = source_root.join(file_name);
        let contents = fs::read(&source_path)
            .map_err(|error| format!("failed to read {}: {error}", source_path.display()))?;
        let destination = package_dir.join(file_name);
        match mode {
            Mode::Write => changed |= write_bytes_if_changed(&destination, &contents)?,
            Mode::Check => {
                let current = fs::read(&destination).map_err(|error| {
                    format!("failed to read {}: {error}", destination.display())
                })?;
                if current != contents {
                    return Err(format!(
                        "stale package license metadata: {}",
                        destination.display()
                    ));
                }
            }
        }
    }
    Ok(changed)
}

fn read_package_json(path: &Path) -> Result<serde_json::Value, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn package_name<'a>(
    json: &'a serde_json::Value,
    package_json_path: &Path,
) -> Result<&'a str, String> {
    json.get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("package.json missing name: {}", package_json_path.display()))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::build_control::Mode;

    use super::{package_readme, write_package_metadata};

    #[test]
    fn package_metadata_generation_is_deterministic() {
        let root = test_root("metadata_generation");
        let package_dir = root.join("packages/example");
        fs::create_dir_all(&package_dir).expect("create package");
        fs::write(root.join("LICENSE-MIT"), "MIT license\n").expect("write MIT license");
        fs::write(root.join("LICENSE-APACHE"), "Apache license\n").expect("write Apache license");
        fs::write(
            package_dir.join("package.json"),
            r#"{
  "name": "@radroots/example",
  "description": "Example package"
}"#,
        )
        .expect("write package json");

        assert!(
            write_package_metadata(&root, &package_dir, Mode::Write).expect("first generation")
        );
        assert_eq!(
            fs::read_to_string(package_dir.join("README.md")).expect("read README"),
            package_readme("@radroots/example", "Example package")
        );
        assert_eq!(
            fs::read_to_string(package_dir.join("LICENSE-MIT")).expect("read MIT"),
            "MIT license\n"
        );
        assert!(
            !write_package_metadata(&root, &package_dir, Mode::Write).expect("second generation")
        );
        write_package_metadata(&root, &package_dir, Mode::Check).expect("fresh metadata");

        let _ = fs::remove_dir_all(root);
    }

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "radroots_sdk_xtask_package_metadata_{name}_{}_{}",
            std::process::id(),
            stamp
        ));
        let _ = fs::remove_dir_all(&root);
        root
    }
}

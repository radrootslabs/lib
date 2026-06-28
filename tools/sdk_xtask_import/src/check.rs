use std::{collections::BTreeSet, fs, path::Path};

use crate::{
    contracts::validate_sdk_contracts,
    fs::workspace_root,
    output::{PackageOutput, package_outputs},
    package_matrix::{
        FORBIDDEN_PACKAGE_NAMES, WasmPackageSpec, package_specs, validate_package_matrix,
        wasm_package_specs,
    },
    ts::generated_header,
    wasm_declarations::declaration_files,
};

pub fn check() -> Result<(), String> {
    validate_package_matrix()?;
    let root = workspace_root()?;
    validate_sdk_contracts(&root)?;
    check_forbidden_packages(&root)?;
    check_binding_crate_sources(&root)?;
    for spec in package_specs() {
        let package_dir = root.join(spec.package_dir);
        let package_json_path = package_dir.join("package.json");
        let index_path = package_dir.join("src/index.ts");
        check_package_json(&package_json_path, spec.package_name)?;
        if !index_path.is_file() {
            return Err(format!("missing package index: {}", index_path.display()));
        }
        check_package_index(&index_path)?;
    }
    for spec in wasm_package_specs() {
        check_wasm_package_surface(&root, *spec)?;
    }
    let outputs = package_outputs()?;
    for output in &outputs {
        check_generated_package_artifact_inventory(&root, output)?;
    }
    for output in outputs {
        for expected in output.files() {
            let path = root
                .join(output.spec.package_dir)
                .join(expected.relative_path);
            let actual = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            if actual != expected.contents {
                return Err(format!("stale generated output: {}", path.display()));
            }
        }
    }
    Ok(())
}

fn check_generated_package_artifact_inventory(
    root: &Path,
    output: &PackageOutput,
) -> Result<(), String> {
    let package_dir = root.join(output.spec.package_dir);
    let generated_dir = package_dir.join("src/generated");
    let expected = output
        .files()
        .into_iter()
        .map(|file| file.relative_path)
        .collect::<BTreeSet<_>>();
    let actual = generated_package_files(&package_dir, &generated_dir)?;
    if actual != expected {
        let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
        let extra = actual.difference(&expected).cloned().collect::<Vec<_>>();
        return Err(format!(
            "generated artifact inventory mismatch for {}: missing {:?}, extra {:?}",
            output.spec.package_name, missing, extra
        ));
    }
    Ok(())
}

fn generated_package_files(
    package_dir: &Path,
    generated_dir: &Path,
) -> Result<BTreeSet<String>, String> {
    let mut files = BTreeSet::new();
    collect_package_files(package_dir, generated_dir, &mut files)?;
    Ok(files)
}

fn collect_package_files(
    package_dir: &Path,
    dir: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|error| format!("failed to read {}: {error}", dir.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read {} entry: {error}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_package_files(package_dir, &path, files)?;
        } else if file_type.is_file() {
            files.insert(relative_path_string(package_dir, &path)?);
        }
    }
    Ok(())
}

fn relative_path_string(base: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(base)
        .map_err(|error| format!("failed to relativize {}: {error}", path.display()))?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn check_package_index(path: &Path) -> Result<(), String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if raw.starts_with(generated_header()) {
        return Err(format!(
            "package index must be handwritten source: {}",
            path.display()
        ));
    }
    Ok(())
}

fn check_binding_crate_sources(root: &Path) -> Result<(), String> {
    for spec in package_specs() {
        let crate_src_dir = root.join(spec.crate_dir).join("src");
        let typescript_dir = crate_src_dir.join("typescript");
        if typescript_dir.exists() {
            return Err(format!(
                "forbidden crate TypeScript source directory exists: {}",
                typescript_dir.display()
            ));
        }
        check_no_typescript_files(&crate_src_dir)?;
    }
    for spec in wasm_package_specs() {
        check_no_typescript_files(&root.join(spec.crate_dir).join("src"))?;
    }
    Ok(())
}

fn check_no_typescript_files(dir: &Path) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|error| format!("failed to read {}: {error}", dir.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read {} entry: {error}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            check_no_typescript_files(&path)?;
        } else if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("ts")
        {
            return Err(format!(
                "forbidden crate TypeScript source file exists: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn check_forbidden_packages(root: &Path) -> Result<(), String> {
    for forbidden in FORBIDDEN_PACKAGE_NAMES {
        let package_leaf = forbidden.trim_start_matches("@radroots/").to_owned();
        let forbidden_dir = root.join("packages").join(package_leaf);
        if forbidden_dir.exists() {
            return Err(format!(
                "forbidden package directory exists: {}",
                forbidden_dir.display()
            ));
        }
    }
    Ok(())
}

fn check_package_json(path: &Path, expected_name: &str) -> Result<(), String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let json = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let actual_name = json
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("package.json missing name: {}", path.display()))?;
    if actual_name != expected_name {
        return Err(format!(
            "package name mismatch in {}: expected {expected_name}, found {actual_name}",
            path.display()
        ));
    }
    let private = json
        .get("private")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !private {
        return Err(format!("package must be private: {}", path.display()));
    }
    Ok(())
}

pub(crate) fn check_wasm_package_surface(root: &Path, spec: WasmPackageSpec) -> Result<(), String> {
    let package_dir = root.join(spec.package_dir);
    let package_json_path = package_dir.join("package.json");
    check_package_json(&package_json_path, spec.package_name)?;
    let raw = fs::read_to_string(&package_json_path)
        .map_err(|error| format!("failed to read {}: {error}", package_json_path.display()))?;
    let json = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", package_json_path.display()))?;
    let dist_manifest = package_dir.join("dist").join("package.json");
    if dist_manifest.exists() {
        return Err(format!(
            "generated package manifest is forbidden: {}",
            dist_manifest.display()
        ));
    }
    let surface_paths = package_surface_paths(&json, &package_json_path)?;
    check_public_wasm_declaration_inventory(&surface_paths, spec)?;
    for relative in surface_paths {
        let normalized = relative.trim_start_matches("./");
        let path = package_dir.join(normalized);
        if !path.is_file() {
            return Err(format!(
                "missing package export artifact for {}: {}",
                spec.package_name,
                path.display()
            ));
        }
    }
    check_wasm_declaration_files(&package_dir, spec)?;
    Ok(())
}

fn check_public_wasm_declaration_inventory(
    surface_paths: &BTreeSet<String>,
    spec: WasmPackageSpec,
) -> Result<(), String> {
    let actual = surface_paths
        .iter()
        .filter(|path| path.ends_with(".d.ts"))
        .cloned()
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([format!("./dist/{}.d.ts", spec.out_name)]);
    if actual != expected {
        return Err(format!(
            "public wasm declaration inventory mismatch for {}: expected {:?}, found {:?}",
            spec.package_name, expected, actual
        ));
    }
    Ok(())
}

fn check_wasm_declaration_files(package_dir: &Path, spec: WasmPackageSpec) -> Result<(), String> {
    for expected in declaration_files(spec)? {
        let path = package_dir.join(&expected.relative_path);
        let actual = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if actual != expected.contents {
            return Err(format!(
                "stale dto_bindgen wasm declaration: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn package_surface_paths(
    json: &serde_json::Value,
    package_json_path: &Path,
) -> Result<BTreeSet<String>, String> {
    let mut paths = BTreeSet::new();
    collect_required_package_path(json, package_json_path, "main", &mut paths)?;
    collect_required_package_path(json, package_json_path, "types", &mut paths)?;
    let exports = json.get("exports").ok_or_else(|| {
        format!(
            "package.json missing exports: {}",
            package_json_path.display()
        )
    })?;
    match exports {
        serde_json::Value::String(path) => {
            validate_package_surface_path(path, package_json_path, "exports")?;
            paths.insert(path.clone());
        }
        serde_json::Value::Object(map) => {
            if map.keys().any(|key| key != ".") {
                return Err(format!(
                    "package.json only supports root exports: {}",
                    package_json_path.display()
                ));
            }
            let root_export = map.get(".").ok_or_else(|| {
                format!(
                    "package.json missing root export: {}",
                    package_json_path.display()
                )
            })?;
            collect_export_paths(root_export, package_json_path, "exports[\".\"]", &mut paths)?;
        }
        _ => {
            return Err(format!(
                "package.json exports must be a string or object: {}",
                package_json_path.display()
            ));
        }
    }
    Ok(paths)
}

fn collect_required_package_path(
    json: &serde_json::Value,
    package_json_path: &Path,
    field: &'static str,
    paths: &mut BTreeSet<String>,
) -> Result<(), String> {
    let value = json
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "package.json missing {field}: {}",
                package_json_path.display()
            )
        })?;
    validate_package_surface_path(value, package_json_path, field)?;
    paths.insert(value.to_owned());
    Ok(())
}

fn collect_export_paths(
    value: &serde_json::Value,
    package_json_path: &Path,
    field: &str,
    paths: &mut BTreeSet<String>,
) -> Result<(), String> {
    match value {
        serde_json::Value::String(path) => {
            validate_package_surface_path(path, package_json_path, field)?;
            paths.insert(path.clone());
            Ok(())
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                collect_export_paths(value, package_json_path, &format!("{field}.{key}"), paths)?;
            }
            Ok(())
        }
        _ => Err(format!(
            "package.json {field} must name file paths: {}",
            package_json_path.display()
        )),
    }
}

fn validate_package_surface_path(
    value: &str,
    package_json_path: &Path,
    field: &str,
) -> Result<(), String> {
    if value.trim().is_empty()
        || value.trim() != value
        || !value.starts_with("./dist/")
        || value.contains('\\')
        || value.split('/').any(|segment| segment == "..")
    {
        return Err(format!(
            "package.json {field} must be a relative dist path: {}",
            package_json_path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        output::package_outputs,
        package_matrix::{WasmPackageSpec, validate_package_matrix},
    };

    use super::{
        check_binding_crate_sources, check_generated_package_artifact_inventory,
        check_no_typescript_files, check_package_index, check_wasm_package_surface,
    };

    #[test]
    fn package_skeleton_is_valid() {
        validate_package_matrix().expect("package matrix validates");
    }

    #[test]
    fn rejects_crate_typescript_directories() {
        let root = test_root("typescript_dir");
        let typescript_dir = root
            .join("crates")
            .join("core_bindings")
            .join("src")
            .join("typescript");
        fs::create_dir_all(&typescript_dir).expect("create forbidden directory");

        let error = check_binding_crate_sources(&root).expect_err("forbidden directory rejected");

        assert!(error.contains("forbidden crate TypeScript source directory"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_crate_typescript_files() {
        let root = test_root("typescript_file");
        let src_dir = root.join("crates/core_bindings/src");
        fs::create_dir_all(&src_dir).expect("create crate source directory");
        fs::write(src_dir.join("types.ts"), "export type A = string;\n")
            .expect("write forbidden file");

        let error = check_no_typescript_files(&src_dir).expect_err("forbidden file rejected");

        assert!(error.contains("forbidden crate TypeScript source file"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_generated_package_index_source() {
        let root = test_root("generated_index");
        let path = root.join("src/index.ts");
        fs::create_dir_all(path.parent().expect("parent")).expect("create source directory");
        fs::write(
            &path,
            "// @generated by cargo xtask generate ts\n// Do not edit by hand.\nexport {};\n",
        )
        .expect("write generated index");

        let error = check_package_index(&path).expect_err("generated index rejected");

        assert!(error.contains("package index must be handwritten source"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn generated_package_artifact_inventory_rejects_extra_files() {
        let root = test_root("generated_inventory_extra");
        let output = package_outputs()
            .expect("package outputs")
            .into_iter()
            .find(|output| output.spec.key == "core")
            .expect("core output");
        let package_dir = root.join(output.spec.package_dir);
        fs::create_dir_all(package_dir.join("src/generated")).expect("create generated dir");
        for file in output.files() {
            let path = package_dir.join(file.relative_path);
            fs::write(path, file.contents).expect("write expected file");
        }
        fs::write(
            package_dir.join("src/generated").join("extra.ts"),
            "export type Extra = string;\n",
        )
        .expect("write extra file");

        let error = check_generated_package_artifact_inventory(&root, &output)
            .expect_err("extra generated file rejected");

        assert!(error.contains("generated artifact inventory mismatch"));
        assert!(error.contains("extra.ts"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wasm_package_surface_requires_exported_dist_files() {
        let root = test_root("wasm_surface");
        let package_dir = root.join("packages").join("example-wasm");
        fs::create_dir_all(package_dir.join("dist")).expect("create dist");
        fs::write(
            package_dir.join("package.json"),
            r#"{
  "name": "@radroots/example-wasm",
  "private": true,
  "main": "./dist/example.js",
  "types": "./dist/example.d.ts",
  "exports": {
    ".": {
      "types": "./dist/example.d.ts",
      "import": "./dist/example.js",
      "default": "./dist/example.js"
    }
  }
}"#,
        )
        .expect("write package json");
        fs::write(package_dir.join("dist").join("example.js"), "export {};\n").expect("write js");
        let spec = WasmPackageSpec {
            key: "example",
            crate_name: "radroots_example_wasm",
            crate_dir: "crates/example_wasm",
            package_name: "@radroots/example-wasm",
            package_dir: "packages/example-wasm",
            out_name: "example",
            out_dir: "../../packages/example-wasm/dist",
        };

        let missing =
            check_wasm_package_surface(&root, spec).expect_err("missing d.ts should fail");
        assert!(missing.contains("example.d.ts"));
        fs::write(
            package_dir.join("dist").join("example.d.ts"),
            dto_declaration("example.d.ts"),
        )
        .expect("write d.ts");
        fs::write(
            package_dir.join("dist").join("example_bg.wasm.d.ts"),
            dto_declaration("example_bg.wasm.d.ts"),
        )
        .expect("write d.ts");
        let error = check_wasm_package_surface(&root, spec)
            .expect_err("unknown declaration inventory rejected");
        assert!(error.contains("missing wasm declaration inventory"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wasm_package_surface_rejects_generated_package_manifest() {
        let root = test_root("wasm_dist_package_manifest");
        let package_dir = root.join("packages").join("example-wasm");
        fs::create_dir_all(package_dir.join("dist")).expect("create dist");
        fs::write(
            package_dir.join("package.json"),
            r#"{
  "name": "@radroots/example-wasm",
  "private": true,
  "main": "./dist/example.js",
  "types": "./dist/example.d.ts",
  "exports": "./dist/example.js"
}"#,
        )
        .expect("write package json");
        fs::write(package_dir.join("dist").join("example.js"), "export {};\n").expect("write js");
        fs::write(
            package_dir.join("dist").join("example.d.ts"),
            dto_declaration("example.d.ts"),
        )
        .expect("write d.ts");
        fs::write(
            package_dir.join("dist").join("example_bg.wasm.d.ts"),
            dto_declaration("example_bg.wasm.d.ts"),
        )
        .expect("write wasm d.ts");
        fs::write(package_dir.join("dist").join("package.json"), "{}\n")
            .expect("write forbidden manifest");
        let spec = WasmPackageSpec {
            key: "example",
            crate_name: "radroots_example_wasm",
            crate_dir: "crates/example_wasm",
            package_name: "@radroots/example-wasm",
            package_dir: "packages/example-wasm",
            out_name: "example",
            out_dir: "../../packages/example-wasm/dist",
        };

        let error =
            check_wasm_package_surface(&root, spec).expect_err("dist package manifest rejected");
        assert!(error.contains("generated package manifest is forbidden"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wasm_package_surface_rejects_subpath_exports() {
        let root = test_root("wasm_subpath_exports");
        let package_dir = root.join("packages").join("example-wasm");
        fs::create_dir_all(package_dir.join("dist")).expect("create dist");
        fs::write(
            package_dir.join("package.json"),
            r#"{
  "name": "@radroots/example-wasm",
  "private": true,
  "main": "./dist/example.js",
  "types": "./dist/example.d.ts",
  "exports": {
    ".": "./dist/example.js",
    "./extra": "./dist/extra.js"
  }
}"#,
        )
        .expect("write package json");
        fs::write(package_dir.join("dist").join("example.js"), "export {};\n").expect("write js");
        fs::write(
            package_dir.join("dist").join("example.d.ts"),
            dto_declaration("example.d.ts"),
        )
        .expect("write d.ts");
        fs::write(
            package_dir.join("dist").join("example_bg.wasm.d.ts"),
            dto_declaration("example_bg.wasm.d.ts"),
        )
        .expect("write wasm d.ts");
        let spec = WasmPackageSpec {
            key: "example",
            crate_name: "radroots_example_wasm",
            crate_dir: "crates/example_wasm",
            package_name: "@radroots/example-wasm",
            package_dir: "packages/example-wasm",
            out_name: "example",
            out_dir: "../../packages/example-wasm/dist",
        };

        let error = check_wasm_package_surface(&root, spec).expect_err("subpath export rejected");
        assert!(error.contains("only supports root exports"));

        let _ = fs::remove_dir_all(root);
    }

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "radroots_sdk_xtask_check_{name}_{}_{}",
            std::process::id(),
            stamp
        ));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn dto_declaration(name: &str) -> String {
        format!(
            "// @generated by cargo xtask generate wasm via dto_bindgen\n// Do not edit by hand.\nexport type Generated = \"{name}\";\n"
        )
    }
}

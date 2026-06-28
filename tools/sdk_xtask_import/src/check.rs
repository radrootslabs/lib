use std::{collections::BTreeSet, fs, path::Path, process::Command};

use serde::Deserialize;

use crate::{
    contracts::validate_sdk_contracts,
    fs::workspace_root,
    output::{PackageOutput, package_outputs},
    package_matrix::{
        FORBIDDEN_PACKAGE_NAMES, WasmPackageSpec, package_specs, validate_package_matrix,
        wasm_package_specs,
    },
    package_metadata::{PACKAGE_FILES, check_package_distribution_metadata, package_description},
    ts::generated_header,
    wasm_declarations::declaration_files,
};

const PACKAGE_VERSION: &str = "0.1.0";
const PACKAGE_LICENSE: &str = "MIT OR Apache-2.0";
const PACKAGE_HOMEPAGE: &str = "https://radroots.org";
const PACKAGE_REPOSITORY_URL: &str = "git+https://github.com/radrootslabs/sdk.git";
const PUBLISH_ACCESS: &str = "public";

#[derive(Debug, Deserialize)]
struct NpmPackEntry {
    files: Vec<NpmPackFile>,
}

#[derive(Debug, Deserialize)]
struct NpmPackFile {
    path: String,
}

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
        let package_json =
            check_package_json(&package_json_path, spec.package_name, spec.package_dir)?;
        check_package_distribution_metadata(
            &root,
            &package_dir,
            &package_json_path,
            &package_json,
        )?;
        let surface_paths = package_surface_paths(&package_json, &package_json_path)?;
        check_package_surface_artifacts(&package_dir, spec.package_name, &surface_paths)?;
        if !index_path.is_file() {
            return Err(format!("missing package index: {}", index_path.display()));
        }
        check_package_index(&index_path)?;
    }
    for spec in wasm_package_specs() {
        check_wasm_package_surface(&root, *spec)?;
    }
    check_npm_pack_payloads(&root)?;
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
        let expected = output.provenance_file();
        let path = root.join(&expected.relative_path);
        let actual = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if actual != expected.contents {
            return Err(format!("stale generated provenance: {}", path.display()));
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

fn check_package_json(
    path: &Path,
    expected_name: &str,
    expected_directory: &str,
) -> Result<serde_json::Value, String> {
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
    if json.get("private").is_some() {
        return Err(format!(
            "public package must not set private: {}",
            path.display()
        ));
    }
    let _ = package_description(&json, path)?;
    require_string_field(&json, path, "version", PACKAGE_VERSION)?;
    require_string_field(&json, path, "license", PACKAGE_LICENSE)?;
    require_string_field(&json, path, "homepage", PACKAGE_HOMEPAGE)?;
    require_string_field(&json, path, "type", "module")?;
    require_bool_field(&json, path, "sideEffects", false)?;
    check_publish_config(&json, path)?;
    check_repository(&json, path, expected_directory)?;
    check_package_files(&json, path)?;
    check_workspace_dependencies(&json, path)?;
    Ok(json)
}

pub(crate) fn check_wasm_package_surface(root: &Path, spec: WasmPackageSpec) -> Result<(), String> {
    let package_dir = root.join(spec.package_dir);
    let package_json_path = package_dir.join("package.json");
    let json = check_package_json(&package_json_path, spec.package_name, spec.package_dir)?;
    check_package_distribution_metadata(root, &package_dir, &package_json_path, &json)?;
    let dist_manifest = package_dir.join("dist").join("package.json");
    if dist_manifest.exists() {
        return Err(format!(
            "generated package manifest is forbidden: {}",
            dist_manifest.display()
        ));
    }
    check_no_wasm_dist_ignore_files(&package_dir, spec)?;
    let surface_paths = package_surface_paths(&json, &package_json_path)?;
    check_public_wasm_declaration_inventory(&surface_paths, spec)?;
    check_package_surface_artifacts(&package_dir, spec.package_name, &surface_paths)?;
    check_wasm_runtime_files(&package_dir, spec)?;
    check_wasm_declaration_files(&package_dir, spec)?;
    Ok(())
}

fn require_string_field(
    json: &serde_json::Value,
    package_json_path: &Path,
    field: &'static str,
    expected: &str,
) -> Result<(), String> {
    let actual = json
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "package.json missing {field}: {}",
                package_json_path.display()
            )
        })?;
    if actual != expected {
        return Err(format!(
            "package.json {field} mismatch in {}: expected {expected}, found {actual}",
            package_json_path.display()
        ));
    }
    Ok(())
}

fn require_bool_field(
    json: &serde_json::Value,
    package_json_path: &Path,
    field: &'static str,
    expected: bool,
) -> Result<(), String> {
    let actual = json
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            format!(
                "package.json missing {field}: {}",
                package_json_path.display()
            )
        })?;
    if actual != expected {
        return Err(format!(
            "package.json {field} mismatch in {}: expected {expected}, found {actual}",
            package_json_path.display()
        ));
    }
    Ok(())
}

fn check_publish_config(json: &serde_json::Value, package_json_path: &Path) -> Result<(), String> {
    let access = json
        .get("publishConfig")
        .and_then(|value| value.get("access"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "package.json missing publishConfig.access: {}",
                package_json_path.display()
            )
        })?;
    if access != PUBLISH_ACCESS {
        return Err(format!(
            "package.json publishConfig.access mismatch in {}: expected {PUBLISH_ACCESS}, found {access}",
            package_json_path.display()
        ));
    }
    Ok(())
}

fn check_repository(
    json: &serde_json::Value,
    package_json_path: &Path,
    expected_directory: &str,
) -> Result<(), String> {
    let repository = json.get("repository").ok_or_else(|| {
        format!(
            "package.json missing repository: {}",
            package_json_path.display()
        )
    })?;
    let repository_type = repository
        .get("type")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "package.json missing repository.type: {}",
                package_json_path.display()
            )
        })?;
    if repository_type != "git" {
        return Err(format!(
            "package.json repository.type mismatch in {}: expected git, found {repository_type}",
            package_json_path.display()
        ));
    }
    let repository_url = repository
        .get("url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "package.json missing repository.url: {}",
                package_json_path.display()
            )
        })?;
    if repository_url != PACKAGE_REPOSITORY_URL {
        return Err(format!(
            "package.json repository.url mismatch in {}: expected {PACKAGE_REPOSITORY_URL}, found {repository_url}",
            package_json_path.display()
        ));
    }
    let repository_directory = repository
        .get("directory")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "package.json missing repository.directory: {}",
                package_json_path.display()
            )
        })?;
    if repository_directory != expected_directory {
        return Err(format!(
            "package.json repository.directory mismatch in {}: expected {expected_directory}, found {repository_directory}",
            package_json_path.display()
        ));
    }
    Ok(())
}

fn check_package_files(json: &serde_json::Value, package_json_path: &Path) -> Result<(), String> {
    let files = json
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            format!(
                "package.json missing files: {}",
                package_json_path.display()
            )
        })?;
    let actual = files
        .iter()
        .map(|value| {
            value.as_str().ok_or_else(|| {
                format!(
                    "package.json files entries must be strings: {}",
                    package_json_path.display()
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if actual != PACKAGE_FILES {
        return Err(format!(
            "package.json files must publish dist plus approved metadata only: {}",
            package_json_path.display()
        ));
    }
    Ok(())
}

fn check_workspace_dependencies(
    json: &serde_json::Value,
    package_json_path: &Path,
) -> Result<(), String> {
    let Some(dependencies) = json.get("dependencies") else {
        return Ok(());
    };
    let dependencies = dependencies.as_object().ok_or_else(|| {
        format!(
            "package.json dependencies must be an object: {}",
            package_json_path.display()
        )
    })?;
    for (name, value) in dependencies {
        if name.starts_with("@radroots/") {
            let version = value.as_str().ok_or_else(|| {
                format!(
                    "package.json dependency versions must be strings: {}",
                    package_json_path.display()
                )
            })?;
            if version != "workspace:^" {
                return Err(format!(
                    "package.json workspace dependency {name} must use workspace:^ in {}",
                    package_json_path.display()
                ));
            }
        }
    }
    Ok(())
}

fn check_npm_pack_payloads(root: &Path) -> Result<(), String> {
    for spec in package_specs() {
        let package_dir = root.join(spec.package_dir);
        let package_json_path = package_dir.join("package.json");
        let json = read_package_json_value(&package_json_path)?;
        let required_files = required_npm_payload_files(&json, &package_json_path, None)?;
        let payload_files = npm_pack_payload_files(&package_dir, spec.package_name)?;
        validate_npm_pack_payload(spec.package_name, &payload_files, &required_files, None)?;
    }
    for spec in wasm_package_specs() {
        let package_dir = root.join(spec.package_dir);
        let package_json_path = package_dir.join("package.json");
        let json = read_package_json_value(&package_json_path)?;
        let required_files = required_npm_payload_files(&json, &package_json_path, Some(*spec))?;
        let payload_files = npm_pack_payload_files(&package_dir, spec.package_name)?;
        validate_npm_pack_payload(
            spec.package_name,
            &payload_files,
            &required_files,
            Some(&format!("dist/{}_bg.wasm", spec.out_name)),
        )?;
    }
    Ok(())
}

fn read_package_json_value(path: &Path) -> Result<serde_json::Value, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn required_npm_payload_files(
    json: &serde_json::Value,
    package_json_path: &Path,
    wasm_spec: Option<WasmPackageSpec>,
) -> Result<BTreeSet<String>, String> {
    let mut required = BTreeSet::from([
        "package.json".to_owned(),
        "README.md".to_owned(),
        "LICENSE-MIT".to_owned(),
        "LICENSE-APACHE".to_owned(),
    ]);
    for path in package_surface_paths(json, package_json_path)? {
        required.insert(normalized_package_path(&path));
    }
    if let Some(spec) = wasm_spec {
        for path in wasm_runtime_files(spec) {
            required.insert(path);
        }
    }
    Ok(required)
}

fn npm_pack_payload_files(
    package_dir: &Path,
    package_name: &str,
) -> Result<BTreeSet<String>, String> {
    let output = Command::new("npm")
        .args(["pack", "--json", "--dry-run", "--ignore-scripts"])
        .current_dir(package_dir)
        .output()
        .map_err(|error| {
            format!(
                "failed to run npm pack dry-run for {package_name} in {}: {error}",
                package_dir.display()
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "npm pack dry-run failed for {package_name} in {}: {}",
            package_dir.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let entries = serde_json::from_slice::<Vec<NpmPackEntry>>(&output.stdout).map_err(|error| {
        format!(
            "failed to parse npm pack dry-run output for {package_name}: {error}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })?;
    let [entry] = entries.as_slice() else {
        return Err(format!(
            "npm pack dry-run for {package_name} must return one package entry"
        ));
    };
    Ok(entry
        .files
        .iter()
        .map(|file| normalized_package_path(&file.path))
        .collect())
}

fn validate_npm_pack_payload(
    package_name: &str,
    payload_files: &BTreeSet<String>,
    required_files: &BTreeSet<String>,
    expected_wasm_file: Option<&str>,
) -> Result<(), String> {
    let missing = required_files
        .difference(payload_files)
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "npm pack payload for {package_name} is missing required files: {missing:?}"
        ));
    }
    for path in payload_files {
        if let Some(reason) = forbidden_npm_payload_path(path) {
            return Err(format!(
                "npm pack payload for {package_name} includes forbidden {reason}: {path}"
            ));
        }
    }
    if let Some(expected_wasm_file) = expected_wasm_file {
        let wasm_files = payload_files
            .iter()
            .filter(|path| path.ends_with(".wasm"))
            .cloned()
            .collect::<Vec<_>>();
        if wasm_files != [expected_wasm_file.to_owned()] {
            return Err(format!(
                "npm pack payload for {package_name} must include exactly {expected_wasm_file}; found {wasm_files:?}"
            ));
        }
    }
    Ok(())
}

fn normalized_package_path(path: &str) -> String {
    path.trim_start_matches("./").to_owned()
}

fn forbidden_npm_payload_path(path: &str) -> Option<&'static str> {
    if path.starts_with("src/") {
        return Some("source path");
    }
    if path.starts_with("contracts/") {
        return Some("contract path");
    }
    if path.contains("sdk-manifest") {
        return Some("manifest provenance path");
    }
    if path.ends_with(".tsbuildinfo") {
        return Some("TypeScript build info path");
    }
    if path
        .split('/')
        .any(|segment| matches!(segment, ".gitignore" | ".npmignore"))
    {
        return Some("ignore file");
    }
    None
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

fn check_no_wasm_dist_ignore_files(
    package_dir: &Path,
    spec: WasmPackageSpec,
) -> Result<(), String> {
    for file_name in [".gitignore", ".npmignore"] {
        let path = package_dir.join("dist").join(file_name);
        if path.exists() {
            return Err(format!(
                "wasm dist ignore file would hide package payload from npm for {}: {}",
                spec.package_name,
                path.display()
            ));
        }
    }
    Ok(())
}

fn check_wasm_runtime_files(package_dir: &Path, spec: WasmPackageSpec) -> Result<(), String> {
    for relative in wasm_runtime_files(spec) {
        let path = package_dir.join(&relative);
        if !path.is_file() {
            return Err(format!(
                "missing wasm package runtime artifact for {}: {}",
                spec.package_name,
                path.display()
            ));
        }
    }
    Ok(())
}

fn wasm_runtime_files(spec: WasmPackageSpec) -> [String; 3] {
    [
        format!("dist/{}.js", spec.out_name),
        format!("dist/{}_bg.wasm", spec.out_name),
        format!("dist/{}_bg.wasm.d.ts", spec.out_name),
    ]
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

fn check_package_surface_artifacts(
    package_dir: &Path,
    package_name: &str,
    surface_paths: &BTreeSet<String>,
) -> Result<(), String> {
    for relative in surface_paths {
        let normalized = relative.trim_start_matches("./");
        let path = package_dir.join(normalized);
        if !path.is_file() {
            return Err(format!(
                "missing package export artifact for {package_name}: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        output::package_outputs,
        package_matrix::{WasmPackageSpec, validate_package_matrix},
        package_metadata::package_readme,
    };

    use super::{
        check_binding_crate_sources, check_generated_package_artifact_inventory,
        check_no_typescript_files, check_package_distribution_metadata, check_package_index,
        check_package_json, check_package_surface_artifacts, check_wasm_package_surface,
        validate_npm_pack_payload,
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
    fn public_package_metadata_rejects_private_packages() {
        let root = test_root("private_package_json");
        let package_dir = root.join("packages").join("example");
        fs::create_dir_all(&package_dir).expect("create package");
        let package_json = package_json("example").replace(
            r#""sideEffects": false,"#,
            r#""private": true, "sideEffects": false,"#,
        );
        fs::write(package_dir.join("package.json"), package_json).expect("write package json");

        let error = check_package_json(
            &package_dir.join("package.json"),
            "@radroots/example",
            "packages/example",
        )
        .expect_err("private package rejected");

        assert!(error.contains("must not set private"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn public_package_metadata_rejects_src_generated_payloads() {
        let root = test_root("src_generated_package_payload");
        let package_dir = root.join("packages").join("example");
        fs::create_dir_all(&package_dir).expect("create package");
        let package_json = package_json("example").replace(
            r#""files": ["dist", "README.md", "LICENSE-MIT", "LICENSE-APACHE"]"#,
            r#""files": ["dist", "README.md", "LICENSE-MIT", "LICENSE-APACHE", "src/generated"]"#,
        );
        fs::write(package_dir.join("package.json"), package_json).expect("write package json");

        let error = check_package_json(
            &package_dir.join("package.json"),
            "@radroots/example",
            "packages/example",
        )
        .expect_err("src generated package payload rejected");

        assert!(error.contains("files must publish dist plus approved metadata only"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn package_distribution_metadata_matches_root_license_files() {
        let root = test_root("package_distribution_metadata");
        let package_dir = root.join("packages").join("example");
        fs::create_dir_all(&package_dir).expect("create package");
        fs::write(root.join("LICENSE-MIT"), "MIT license\n").expect("write MIT license");
        fs::write(root.join("LICENSE-APACHE"), "Apache license\n").expect("write Apache license");
        fs::write(package_dir.join("package.json"), package_json("example"))
            .expect("write package json");
        fs::write(
            package_dir.join("README.md"),
            package_readme("@radroots/example", "Example package"),
        )
        .expect("write readme");
        fs::write(package_dir.join("LICENSE-MIT"), "MIT license\n")
            .expect("write package MIT license");
        fs::write(package_dir.join("LICENSE-APACHE"), "Apache license\n")
            .expect("write package Apache license");
        let json = check_package_json(
            &package_dir.join("package.json"),
            "@radroots/example",
            "packages/example",
        )
        .expect("valid package json");

        check_package_distribution_metadata(
            &root,
            &package_dir,
            &package_dir.join("package.json"),
            &json,
        )
        .expect("metadata matches");

        fs::write(package_dir.join("README.md"), "stale\n").expect("stale readme");
        let error = check_package_distribution_metadata(
            &root,
            &package_dir,
            &package_dir.join("package.json"),
            &json,
        )
        .expect_err("stale readme rejected");
        assert!(error.contains("stale package README"));

        fs::write(
            package_dir.join("README.md"),
            package_readme("@radroots/example", "Example package"),
        )
        .expect("restore readme");
        fs::write(package_dir.join("LICENSE-MIT"), "stale\n").expect("stale license");
        let error = check_package_distribution_metadata(
            &root,
            &package_dir,
            &package_dir.join("package.json"),
            &json,
        )
        .expect_err("stale license rejected");
        assert!(error.contains("stale package license metadata"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn package_surface_artifacts_require_dist_files() {
        let root = test_root("package_surface_artifacts");
        let package_dir = root.join("packages").join("example");
        fs::create_dir_all(package_dir.join("dist")).expect("create dist");
        fs::write(package_dir.join("dist").join("index.js"), "export {};\n").expect("write js");
        let surface_paths =
            BTreeSet::from(["./dist/index.js".to_owned(), "./dist/index.d.ts".to_owned()]);

        let error =
            check_package_surface_artifacts(&package_dir, "@radroots/example", &surface_paths)
                .expect_err("missing declaration should fail");
        assert!(error.contains("missing package export artifact"));
        assert!(error.contains("index.d.ts"));

        fs::write(package_dir.join("dist").join("index.d.ts"), "export {};\n").expect("write d.ts");
        check_package_surface_artifacts(&package_dir, "@radroots/example", &surface_paths)
            .expect("surface artifacts present");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn npm_pack_payload_requires_metadata_and_export_files() {
        let payload_files = BTreeSet::from([
            "package.json".to_owned(),
            "README.md".to_owned(),
            "LICENSE-MIT".to_owned(),
            "dist/index.js".to_owned(),
        ]);
        let required_files = BTreeSet::from([
            "package.json".to_owned(),
            "README.md".to_owned(),
            "LICENSE-MIT".to_owned(),
            "LICENSE-APACHE".to_owned(),
            "dist/index.js".to_owned(),
            "dist/index.d.ts".to_owned(),
        ]);

        let error =
            validate_npm_pack_payload("@radroots/example", &payload_files, &required_files, None)
                .expect_err("missing files should fail");
        assert!(error.contains("missing required files"));
        assert!(error.contains("LICENSE-APACHE"));
        assert!(error.contains("dist/index.d.ts"));
    }

    #[test]
    fn npm_pack_payload_rejects_source_and_provenance_internals() {
        let required_files = BTreeSet::from(["package.json".to_owned()]);
        for forbidden in [
            "src/generated/types.ts",
            "contracts/provenance/typescript/core.json",
            "dist/sdk-manifest.json",
            "dist/index.tsbuildinfo",
            "dist/.gitignore",
            ".npmignore",
        ] {
            let payload_files = BTreeSet::from(["package.json".to_owned(), forbidden.to_owned()]);

            let error = validate_npm_pack_payload(
                "@radroots/example",
                &payload_files,
                &required_files,
                None,
            )
            .expect_err("forbidden payload path should fail");
            assert!(error.contains("includes forbidden"));
            assert!(error.contains(forbidden));
        }
    }

    #[test]
    fn npm_pack_payload_requires_exact_wasm_file() {
        let required_files = BTreeSet::from([
            "package.json".to_owned(),
            "dist/example.js".to_owned(),
            "dist/example_bg.wasm".to_owned(),
        ]);
        let missing_wasm =
            BTreeSet::from(["package.json".to_owned(), "dist/example.js".to_owned()]);
        let error = validate_npm_pack_payload(
            "@radroots/example-wasm",
            &missing_wasm,
            &required_files,
            Some("dist/example_bg.wasm"),
        )
        .expect_err("missing wasm should fail");
        assert!(error.contains("missing required files"));

        let extra_wasm = BTreeSet::from([
            "package.json".to_owned(),
            "dist/example.js".to_owned(),
            "dist/example_bg.wasm".to_owned(),
            "dist/extra_bg.wasm".to_owned(),
        ]);
        let error = validate_npm_pack_payload(
            "@radroots/example-wasm",
            &extra_wasm,
            &required_files,
            Some("dist/example_bg.wasm"),
        )
        .expect_err("extra wasm should fail");
        assert!(error.contains("must include exactly dist/example_bg.wasm"));
    }

    #[test]
    fn wasm_package_surface_requires_exported_dist_files() {
        let root = test_root("wasm_surface");
        let package_dir = root.join("packages").join("example-wasm");
        fs::create_dir_all(package_dir.join("dist")).expect("create dist");
        fs::write(
            package_dir.join("package.json"),
            package_json("example-wasm").replace("./dist/index", "./dist/example"),
        )
        .expect("write package json");
        write_distribution_metadata(
            &root,
            &package_dir,
            "@radroots/example-wasm",
            "Example package",
        );
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
        fs::write(package_dir.join("dist").join("example_bg.wasm"), b"\0asm").expect("write wasm");
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
            package_json("example-wasm").replace("./dist/index", "./dist/example"),
        )
        .expect("write package json");
        write_distribution_metadata(
            &root,
            &package_dir,
            "@radroots/example-wasm",
            "Example package",
        );
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
    fn wasm_package_surface_rejects_dist_ignore_files() {
        let root = test_root("wasm_dist_ignore");
        let package_dir = root.join("packages").join("example-wasm");
        fs::create_dir_all(package_dir.join("dist")).expect("create dist");
        fs::write(
            package_dir.join("package.json"),
            package_json("example-wasm").replace("./dist/index", "./dist/example"),
        )
        .expect("write package json");
        write_distribution_metadata(
            &root,
            &package_dir,
            "@radroots/example-wasm",
            "Example package",
        );
        fs::write(package_dir.join("dist").join(".gitignore"), "*\n").expect("write ignore");
        let spec = WasmPackageSpec {
            key: "example",
            crate_name: "radroots_example_wasm",
            crate_dir: "crates/example_wasm",
            package_name: "@radroots/example-wasm",
            package_dir: "packages/example-wasm",
            out_name: "example",
            out_dir: "../../packages/example-wasm/dist",
        };

        let error = check_wasm_package_surface(&root, spec).expect_err("dist ignore file rejected");
        assert!(error.contains("would hide package payload"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wasm_package_surface_requires_runtime_wasm_artifact() {
        let root = test_root("wasm_runtime_artifact");
        let package_dir = root.join("packages").join("example-wasm");
        fs::create_dir_all(package_dir.join("dist")).expect("create dist");
        fs::write(
            package_dir.join("package.json"),
            package_json("example-wasm").replace("./dist/index", "./dist/example"),
        )
        .expect("write package json");
        write_distribution_metadata(
            &root,
            &package_dir,
            "@radroots/example-wasm",
            "Example package",
        );
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

        let error = check_wasm_package_surface(&root, spec).expect_err("missing wasm rejected");
        assert!(error.contains("missing wasm package runtime artifact"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wasm_package_surface_rejects_subpath_exports() {
        let root = test_root("wasm_subpath_exports");
        let package_dir = root.join("packages").join("example-wasm");
        fs::create_dir_all(package_dir.join("dist")).expect("create dist");
        fs::write(
            package_dir.join("package.json"),
            package_json("example-wasm")
                .replace("./dist/index", "./dist/example")
                .replace(
                    r#""exports": {
    ".": {
      "types": "./dist/example.d.ts",
      "import": "./dist/example.js",
      "default": "./dist/example.js"
    }
  }"#,
                    r#""exports": {
    ".": "./dist/example.js",
    "./extra": "./dist/extra.js"
  }"#,
                ),
        )
        .expect("write package json");
        write_distribution_metadata(
            &root,
            &package_dir,
            "@radroots/example-wasm",
            "Example package",
        );
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

    fn write_distribution_metadata(
        root: &PathBuf,
        package_dir: &PathBuf,
        package_name: &str,
        description: &str,
    ) {
        fs::write(root.join("LICENSE-MIT"), "MIT license\n").expect("write MIT license");
        fs::write(root.join("LICENSE-APACHE"), "Apache license\n").expect("write Apache license");
        fs::write(package_dir.join("LICENSE-MIT"), "MIT license\n")
            .expect("write package MIT license");
        fs::write(package_dir.join("LICENSE-APACHE"), "Apache license\n")
            .expect("write package Apache license");
        fs::write(
            package_dir.join("README.md"),
            package_readme(package_name, description),
        )
        .expect("write package README");
    }

    fn package_json(name: &str) -> String {
        format!(
            r#"{{
  "name": "@radroots/{name}",
  "version": "0.1.0",
  "description": "Example package",
  "license": "MIT OR Apache-2.0",
  "homepage": "https://radroots.org",
  "repository": {{
    "type": "git",
    "url": "git+https://github.com/radrootslabs/sdk.git",
    "directory": "packages/{name}"
  }},
  "publishConfig": {{
    "access": "public"
  }},
  "type": "module",
  "sideEffects": false,
  "files": ["dist", "README.md", "LICENSE-MIT", "LICENSE-APACHE"],
  "main": "./dist/index.js",
  "types": "./dist/index.d.ts",
  "exports": {{
    ".": {{
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js",
      "default": "./dist/index.js"
    }}
  }}
}}"#
        )
    }
}

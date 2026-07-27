use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use syn::{Item, UseTree, Visibility};

const ARCHITECTURE_RELATIVE: &str = "docs/specs/radroots_crates_release_v1.toml";
const CORE_MANIFEST_RELATIVE: &str = "crates/core/Cargo.toml";
const CORE_LIB_RELATIVE: &str = "crates/core/src/lib.rs";

const CORE_RUNTIME_DEPENDENCIES: [&str; 2] = ["rust_decimal", "serde"];
const CORE_DEV_DEPENDENCIES: [&str; 3] = ["dto_bindgen", "rust_decimal", "serde_json"];
const CORE_ROOT_EXPORTS: [(&str, &str); 8] = [
    ("Currency", "currency::Currency"),
    ("Decimal", "decimal::Decimal"),
    ("Error", "error::Error"),
    ("Money", "money::Money"),
    ("Percent", "percent::Percent"),
    ("Quantity", "quantity::Quantity"),
    ("QuantityPrice", "pricing::QuantityPrice"),
    ("Unit", "unit::Unit"),
];
pub(super) fn validate(workspace_root: &Path) -> Result<(), String> {
    let architecture = read_toml(workspace_root, ARCHITECTURE_RELATIVE)?;
    let core_spec = architecture
        .get("package")
        .and_then(toml::Value::as_array)
        .and_then(|packages| {
            packages.iter().find(|package| {
                package.get("name").and_then(toml::Value::as_str) == Some("radroots-core")
            })
        })
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{ARCHITECTURE_RELATIVE} is missing radroots-core"))?;

    let manifest = read_toml(workspace_root, CORE_MANIFEST_RELATIVE)?;
    validate_manifest(core_spec, &manifest)?;
    validate_crate_root(workspace_root, core_spec)
}

fn validate_manifest(core_spec: &toml::value::Table, manifest: &toml::Value) -> Result<(), String> {
    let manifest = manifest
        .as_table()
        .ok_or_else(|| format!("{CORE_MANIFEST_RELATIVE} must be a TOML table"))?;
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{CORE_MANIFEST_RELATIVE} is missing [package]"))?;
    if package.get("name").and_then(toml::Value::as_str) != Some("radroots-core") {
        return Err(format!(
            "{CORE_MANIFEST_RELATIVE} package.name must be radroots-core"
        ));
    }
    let library = manifest
        .get("lib")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{CORE_MANIFEST_RELATIVE} is missing [lib]"))?;
    if library.get("name").and_then(toml::Value::as_str) != Some("radroots_core")
        || library.keys().map(String::as_str).collect::<BTreeSet<_>>() != BTreeSet::from(["name"])
    {
        return Err(format!(
            "{CORE_MANIFEST_RELATIVE} must use the conventional radroots_core library target"
        ));
    }

    let features = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{CORE_MANIFEST_RELATIVE} is missing [features]"))?;
    let expected_features = spec_strings(core_spec, "features")?;
    let actual_features = features
        .keys()
        .filter(|name| name.as_str() != "default")
        .cloned()
        .collect::<BTreeSet<_>>();
    if actual_features != expected_features {
        return Err(format!(
            "radroots-core public features must be exactly {expected_features:?}, found {actual_features:?}"
        ));
    }
    let expected_default = spec_strings(core_spec, "default_features")?;
    let actual_default = value_strings(
        features.get("default"),
        &format!("{CORE_MANIFEST_RELATIVE} features.default"),
    )?;
    if actual_default != expected_default {
        return Err(format!(
            "radroots-core default features must be exactly {expected_default:?}, found {actual_default:?}"
        ));
    }
    let std_enables = value_strings(
        features.get("std"),
        &format!("{CORE_MANIFEST_RELATIVE} features.std"),
    )?;
    if !std_enables.is_empty() {
        return Err("radroots-core std must remain an empty additive marker".to_owned());
    }
    let serde_enables = value_strings(
        features.get("serde"),
        &format!("{CORE_MANIFEST_RELATIVE} features.serde"),
    )?;
    let expected_serde = BTreeSet::from(["dep:serde".to_owned(), "rust_decimal/serde".to_owned()]);
    if serde_enables != expected_serde {
        return Err(format!(
            "radroots-core serde feature must enable {expected_serde:?}, found {serde_enables:?}"
        ));
    }

    validate_dependency_names(
        manifest,
        "dependencies",
        BTreeSet::from(CORE_RUNTIME_DEPENDENCIES.map(str::to_owned)),
    )?;
    validate_dependency_names(
        manifest,
        "dev-dependencies",
        BTreeSet::from(CORE_DEV_DEPENDENCIES.map(str::to_owned)),
    )?;
    reject_nonempty_dependency_section(manifest, "build-dependencies")?;
    if manifest
        .get("target")
        .and_then(toml::Value::as_table)
        .is_some_and(|targets| {
            targets.values().any(|target| {
                target.as_table().is_some_and(|table| {
                    ["dependencies", "dev-dependencies", "build-dependencies"]
                        .iter()
                        .any(|section| {
                            table
                                .get(*section)
                                .and_then(toml::Value::as_table)
                                .is_some_and(|dependencies| !dependencies.is_empty())
                        })
                })
            })
        })
    {
        return Err("radroots-core must not declare target-specific dependencies".to_owned());
    }

    let dependencies = manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .expect("validated dependencies");
    validate_dependency_shape(dependencies, "rust_decimal", false, false, &[])?;
    validate_dependency_shape(dependencies, "serde", true, false, &["alloc", "derive"])
}

fn validate_crate_root(
    workspace_root: &Path,
    core_spec: &toml::value::Table,
) -> Result<(), String> {
    let path = workspace_root.join(CORE_LIB_RELATIVE);
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let file =
        syn::parse_file(&raw).map_err(|error| format!("parse {CORE_LIB_RELATIVE}: {error}"))?;
    let mut modules = BTreeSet::new();
    let mut exports = BTreeMap::new();
    for item in &file.items {
        match item {
            Item::Mod(item) if matches!(item.vis, Visibility::Public(_)) => {
                modules.insert(item.ident.to_string());
            }
            Item::Use(item) if matches!(item.vis, Visibility::Public(_)) => {
                collect_use_exports(&item.tree, &mut Vec::new(), &mut exports)?;
            }
            Item::Macro(item)
                if item
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("macro_export")) =>
            {
                return Err(format!(
                    "{CORE_LIB_RELATIVE} must not declare a root macro export"
                ));
            }
            item if is_public(item) => {
                return Err(format!(
                    "{CORE_LIB_RELATIVE} contains an unsupported public root item: {}",
                    public_item_kind(item)
                ));
            }
            _ => {}
        }
    }

    let expected_modules = spec_strings(core_spec, "modules")?;
    if modules != expected_modules {
        return Err(format!(
            "radroots-core public modules must be exactly {expected_modules:?}, found {modules:?}"
        ));
    }
    let specified_exports = spec_strings(core_spec, "root_exports")?;
    let canonical_exports = CORE_ROOT_EXPORTS
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    if specified_exports != canonical_exports {
        return Err(format!(
            "radroots-core canonical root exports must be exactly {canonical_exports:?}, found {specified_exports:?}"
        ));
    }
    let expected_exports = CORE_ROOT_EXPORTS
        .into_iter()
        .map(|(name, source)| (name.to_owned(), source.to_owned()))
        .collect::<BTreeMap<_, _>>();
    if exports != expected_exports {
        return Err(format!(
            "radroots-core root exports must match the canonical contract; expected {expected_exports:?}, found {exports:?}"
        ));
    }
    Ok(())
}

fn validate_dependency_names(
    manifest: &toml::value::Table,
    section: &str,
    expected: BTreeSet<String>,
) -> Result<(), String> {
    let actual = manifest
        .get(section)
        .and_then(toml::Value::as_table)
        .map(|dependencies| dependencies.keys().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    if actual != expected {
        return Err(format!(
            "radroots-core {section} must be exactly {expected:?}, found {actual:?}"
        ));
    }
    Ok(())
}

fn reject_nonempty_dependency_section(
    manifest: &toml::value::Table,
    section: &str,
) -> Result<(), String> {
    if manifest
        .get(section)
        .and_then(toml::Value::as_table)
        .is_some_and(|dependencies| !dependencies.is_empty())
    {
        return Err(format!("radroots-core must not declare {section}"));
    }
    Ok(())
}

fn validate_dependency_shape(
    dependencies: &toml::value::Table,
    name: &str,
    optional: bool,
    default_features: bool,
    expected_features: &[&str],
) -> Result<(), String> {
    let dependency = dependencies
        .get(name)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("radroots-core dependency {name} must use a dependency table"))?;
    if dependency.get("workspace").and_then(toml::Value::as_bool) != Some(true)
        || dependency
            .get("optional")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false)
            != optional
        || dependency
            .get("default-features")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true)
            != default_features
    {
        return Err(format!(
            "radroots-core dependency {name} has a noncanonical workspace/optional/default-features shape"
        ));
    }
    let mut expected_keys = BTreeSet::from(["workspace", "default-features"]);
    if optional {
        expected_keys.insert("optional");
    }
    if !expected_features.is_empty() {
        expected_keys.insert("features");
    }
    let actual_keys = dependency
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual_keys != expected_keys {
        return Err(format!(
            "radroots-core dependency {name} keys must be exactly {expected_keys:?}, found {actual_keys:?}"
        ));
    }
    let actual_features = match dependency.get("features") {
        Some(features) => value_strings(
            Some(features),
            &format!("{CORE_MANIFEST_RELATIVE} dependencies.{name}.features"),
        )?,
        None => BTreeSet::new(),
    };
    let expected_features = expected_features
        .iter()
        .map(|feature| (*feature).to_owned())
        .collect::<BTreeSet<_>>();
    if actual_features != expected_features {
        return Err(format!(
            "radroots-core dependency {name} features must be {expected_features:?}, found {actual_features:?}"
        ));
    }
    Ok(())
}

fn spec_strings(package: &toml::value::Table, field: &str) -> Result<BTreeSet<String>, String> {
    value_strings(
        package.get(field),
        &format!("{ARCHITECTURE_RELATIVE} radroots-core.{field}"),
    )
}

fn value_strings(value: Option<&toml::Value>, label: &str) -> Result<BTreeSet<String>, String> {
    value
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{label} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{label} must contain strings"))
        })
        .collect()
}

fn collect_use_exports(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    exports: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_exports(&path.tree, prefix, exports)?;
            prefix.pop();
        }
        UseTree::Name(name) => {
            let source_name = name.ident.to_string();
            let export_name = if source_name == "self" {
                prefix.last().cloned().ok_or_else(|| {
                    format!("{CORE_LIB_RELATIVE} contains an invalid root self re-export")
                })?
            } else {
                source_name.clone()
            };
            let source = if source_name == "self" {
                prefix.join("::")
            } else {
                qualified_source(prefix, &source_name)
            };
            insert_export(exports, export_name, source)?;
        }
        UseTree::Rename(rename) if rename.rename != "_" => {
            insert_export(
                exports,
                rename.rename.to_string(),
                qualified_source(prefix, &rename.ident.to_string()),
            )?;
        }
        UseTree::Rename(_) => {}
        UseTree::Glob(_) => {
            return Err(format!(
                "{CORE_LIB_RELATIVE} must not use public glob re-exports"
            ));
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_exports(item, prefix, exports)?;
            }
        }
    }
    Ok(())
}

fn qualified_source(prefix: &[String], name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{}::{name}", prefix.join("::"))
    }
}

fn insert_export(
    exports: &mut BTreeMap<String, String>,
    name: String,
    source: String,
) -> Result<(), String> {
    if let Some(previous) = exports.insert(name.clone(), source.clone()) {
        return Err(format!(
            "{CORE_LIB_RELATIVE} exports {name} more than once from {previous} and {source}"
        ));
    }
    Ok(())
}

fn is_public(item: &Item) -> bool {
    match item {
        Item::Const(item) => matches!(item.vis, Visibility::Public(_)),
        Item::Enum(item) => matches!(item.vis, Visibility::Public(_)),
        Item::ExternCrate(item) => matches!(item.vis, Visibility::Public(_)),
        Item::Fn(item) => matches!(item.vis, Visibility::Public(_)),
        Item::Static(item) => matches!(item.vis, Visibility::Public(_)),
        Item::Struct(item) => matches!(item.vis, Visibility::Public(_)),
        Item::Trait(item) => matches!(item.vis, Visibility::Public(_)),
        Item::TraitAlias(item) => matches!(item.vis, Visibility::Public(_)),
        Item::Type(item) => matches!(item.vis, Visibility::Public(_)),
        Item::Union(item) => matches!(item.vis, Visibility::Public(_)),
        _ => false,
    }
}

fn public_item_kind(item: &Item) -> &'static str {
    match item {
        Item::Const(_) => "const",
        Item::Enum(_) => "enum",
        Item::ExternCrate(_) => "extern crate",
        Item::Fn(_) => "function",
        Item::Static(_) => "static",
        Item::Struct(_) => "struct",
        Item::Trait(_) => "trait",
        Item::TraitAlias(_) => "trait alias",
        Item::Type(_) => "type alias",
        Item::Union(_) => "union",
        _ => "item",
    }
}

fn read_toml(workspace_root: &Path, relative: &str) -> Result<toml::Value, String> {
    let path = workspace_path(workspace_root, relative);
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    raw.parse::<toml::Value>()
        .map_err(|error| format!("parse {}: {error}", path.display()))
}

fn workspace_path(workspace_root: &Path, relative: &str) -> PathBuf {
    workspace_root.join(relative)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::validate;

    const SPEC: &str = r#"
[[package]]
name = "radroots-core"
features = ["std", "serde"]
default_features = ["std", "serde"]
modules = ["currency", "decimal", "money", "percent", "pricing", "quantity", "unit"]
root_exports = ["Currency", "Decimal", "Money", "Percent", "Quantity", "QuantityPrice", "Unit", "Error"]
"#;

    const MANIFEST: &str = r#"
[package]
name = "radroots-core"

[lib]
name = "radroots_core"

[features]
default = ["std", "serde"]
std = []
serde = ["dep:serde", "rust_decimal/serde"]

[dependencies]
rust_decimal = { workspace = true, default-features = false }
serde = { workspace = true, default-features = false, features = ["alloc", "derive"], optional = true }

[dev-dependencies]
dto_bindgen = { workspace = true }
rust_decimal = { workspace = true }
serde_json = { workspace = true }
"#;

    fn fixture() -> tempfile::TempDir {
        let root = tempdir().expect("temporary workspace");
        fs::create_dir_all(root.path().join("docs/specs")).expect("spec directory");
        fs::create_dir_all(root.path().join("crates/core/src")).expect("core directory");
        fs::write(
            root.path()
                .join("docs/specs/radroots_crates_release_v1.toml"),
            SPEC,
        )
        .expect("architecture spec");
        fs::write(root.path().join("crates/core/Cargo.toml"), MANIFEST).expect("core manifest");
        let exports = super::CORE_ROOT_EXPORTS
            .into_iter()
            .map(|(_, source)| format!("pub use {source};"))
            .collect::<Vec<_>>()
            .join(" ");
        fs::write(
            root.path().join("crates/core/src/lib.rs"),
            format!(
                "pub mod currency; pub mod decimal; pub mod money; pub mod percent; pub mod pricing; pub mod quantity; pub mod unit; {exports}"
            ),
        )
        .expect("core root");
        root
    }

    #[test]
    fn accepts_exact_core_contract() {
        let root = fixture();
        validate(root.path()).expect("exact core contract");
    }

    #[test]
    fn rejects_public_codegen_feature_and_runtime_dependency() {
        let root = fixture();
        let manifest_path = root.path().join("crates/core/Cargo.toml");
        let manifest = fs::read_to_string(&manifest_path).expect("manifest");
        fs::write(
            &manifest_path,
            manifest.replace("std = []", "std = []\ndto-bindgen = [\"dep:dto_bindgen\"]"),
        )
        .expect("mutated manifest");
        let error = validate(root.path()).expect_err("codegen feature must fail");
        assert!(error.contains("public features must be exactly"), "{error}");

        fs::write(
            &manifest_path,
            MANIFEST.replace(
                "[dev-dependencies]",
                "tokio = { workspace = true }\n\n[dev-dependencies]",
            ),
        )
        .expect("runtime dependency manifest");
        let error = validate(root.path()).expect_err("runtime dependency must fail");
        assert!(error.contains("dependencies must be exactly"), "{error}");

        fs::write(
            &manifest_path,
            MANIFEST.replace(
                "rust_decimal = { workspace = true, default-features = false }",
                "rust_decimal = { workspace = true, default-features = false, features = [] }",
            ),
        )
        .expect("noncanonical dependency shape");
        let error = validate(root.path()).expect_err("extra dependency keys must fail");
        assert!(error.contains("keys must be exactly"), "{error}");
    }

    #[test]
    fn rejects_extra_public_module_or_export() {
        let root = fixture();
        let lib_path = root.path().join("crates/core/src/lib.rs");
        let source = fs::read_to_string(&lib_path).expect("core root");
        fs::write(&lib_path, format!("{source} pub mod serde_ext;")).expect("extra module source");
        let error = validate(root.path()).expect_err("extra module must fail");
        assert!(error.contains("public modules must be exactly"), "{error}");

        fs::write(&lib_path, format!("{source} pub use fixture::Clock;"))
            .expect("extra export source");
        let error = validate(root.path()).expect_err("extra export must fail");
        assert!(error.contains("root exports must match"), "{error}");
    }

    #[test]
    fn rejects_rebound_duplicate_and_glob_exports() {
        let root = fixture();
        let lib_path = root.path().join("crates/core/src/lib.rs");
        let source = fs::read_to_string(&lib_path).expect("core root");
        fs::write(
            &lib_path,
            source.replace("pub use currency::Currency;", "pub use money::Currency;"),
        )
        .expect("rebound export source");
        let error = validate(root.path()).expect_err("rebound export must fail");
        assert!(error.contains("root exports must match"), "{error}");

        fs::write(&lib_path, format!("{source} pub use currency::*;")).expect("glob export source");
        let error = validate(root.path()).expect_err("glob export must fail");
        assert!(error.contains("must not use public glob"), "{error}");

        fs::write(&lib_path, format!("{source} pub use currency::Currency;"))
            .expect("duplicate export source");
        let error = validate(root.path()).expect_err("duplicate export must fail");
        assert!(error.contains("exports Currency more than once"), "{error}");
    }
}

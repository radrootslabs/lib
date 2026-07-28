use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{ErrorKind, Write},
    path::{Component, Path, PathBuf},
};

use quote::ToTokens;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use syn::{Item, Type, Visibility};
use tempfile::NamedTempFile;

const CONFIG_PATH: &str = "contracts/codegen/protocol_v1.toml";
const PACKAGE: &str = "radroots_protocol";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const EXPECTED_SOURCES: &[(&str, &str)] = &[
    ("capability::v1", "crates/protocol/src/capability/v1.rs"),
    ("error::v1", "crates/protocol/src/error/v1.rs"),
    ("event::v1", "crates/protocol/src/event/v1.rs"),
    (
        "radrootsd::transport_publish::v5",
        "crates/protocol/src/radrootsd/transport_publish/v5.rs",
    ),
    ("runtime::v1", "crates/protocol/src/runtime/v1.rs"),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema_version: u16,
    package: String,
    source_hash_algorithm: String,
    inventory_path: String,
    inventory_sha256_path: String,
    source: Vec<SourceConfig>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
struct SourceConfig {
    module: String,
    path: String,
}

#[derive(Debug, Serialize)]
struct Inventory {
    schema_version: u16,
    generator: &'static str,
    package: &'static str,
    source_hash_algorithm: &'static str,
    sources: Vec<SourceInventory>,
    schemas: Vec<SchemaInventory>,
}

#[derive(Debug, Serialize)]
struct SourceInventory {
    module: String,
    path: String,
    sha256: String,
    types: Vec<TypeInventory>,
}

#[derive(Debug, Serialize)]
struct TypeInventory {
    rust_path: String,
    kind: &'static str,
}

#[derive(Clone, Debug, Serialize)]
struct SchemaInventory {
    schema_id: String,
    module: String,
    generation: u16,
}

struct GeneratedFile {
    path: PathBuf,
    display_path: String,
    bytes: Vec<u8>,
}

pub(crate) fn run(mode: &str, workspace_root: &Path) -> Result<(), String> {
    let config = load_config(workspace_root)?;
    validate_config(&config)?;
    let schemas = protocol_schemas()?;
    let generated = render_outputs(workspace_root, &config, schemas)?;
    match mode {
        "--check" => check_outputs(&generated),
        "--write" => write_outputs(&generated),
        _ => Err("usage: cargo xtask generate protocol --check|--write".to_owned()),
    }
}

pub(crate) fn check(workspace_root: &Path) -> Result<(), String> {
    run("--check", workspace_root)
}

fn load_config(workspace_root: &Path) -> Result<Config, String> {
    let path = safe_workspace_file(workspace_root, CONFIG_PATH, false, "protocol codegen input")?;
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read `{CONFIG_PATH}`: {error}"))?;
    toml::from_str(&source).map_err(|error| format!("invalid `{CONFIG_PATH}`: {error}"))
}

fn validate_config(config: &Config) -> Result<(), String> {
    if config.schema_version != 1 {
        return Err("protocol codegen schema_version must be 1".to_owned());
    }
    if config.package != PACKAGE {
        return Err(format!("protocol codegen package must be `{PACKAGE}`"));
    }
    if config.source_hash_algorithm != HASH_ALGORITHM {
        return Err(format!(
            "protocol codegen source_hash_algorithm must be `{HASH_ALGORITHM}`"
        ));
    }
    if config.inventory_path != "contracts/codegen/protocol_v1.inventory.json"
        || config.inventory_sha256_path != "contracts/codegen/protocol_v1.inventory.sha256"
    {
        return Err("protocol codegen output paths are fixed by contract".to_owned());
    }

    let mut actual = config
        .source
        .iter()
        .map(|source| (source.module.as_str(), source.path.as_str()))
        .collect::<Vec<_>>();
    actual.sort_unstable();
    if actual != EXPECTED_SOURCES {
        return Err(format!(
            "protocol codegen source inventory drifted: expected {EXPECTED_SOURCES:?}, found {actual:?}"
        ));
    }
    if config
        .source
        .iter()
        .map(|source| &source.module)
        .collect::<BTreeSet<_>>()
        .len()
        != config.source.len()
    {
        return Err("protocol codegen modules must be unique".to_owned());
    }
    Ok(())
}

fn protocol_schemas() -> Result<Vec<SchemaInventory>, String> {
    let registry = radroots_protocol::schema::protocol_v1_registry()
        .map_err(|error| format!("invalid protocol schema registry: {error}"))?;
    Ok(registry
        .descriptors()
        .iter()
        .map(|descriptor| SchemaInventory {
            schema_id: descriptor.id().as_str().to_owned(),
            module: descriptor.module().path().to_owned(),
            generation: descriptor.module().generation(),
        })
        .collect())
}

fn render_outputs(
    workspace_root: &Path,
    config: &Config,
    schemas: Vec<SchemaInventory>,
) -> Result<Vec<GeneratedFile>, String> {
    let mut sources = config.source.clone();
    sources.sort();
    let mut inventories = Vec::with_capacity(sources.len());
    for source in sources {
        let path = safe_workspace_file(workspace_root, &source.path, false, "protocol DTO source")?;
        let bytes = fs::read(&path).map_err(|error| {
            format!(
                "failed to read protocol DTO source `{}`: {error}",
                source.path
            )
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            format!(
                "protocol DTO source `{}` is not UTF-8: {error}",
                source.path
            )
        })?;
        inventories.push(SourceInventory {
            module: source.module.clone(),
            path: source.path.clone(),
            sha256: sha256_hex(&bytes),
            types: serialized_public_types(&source.module, &source.path, text)?,
        });
    }

    let inventory = Inventory {
        schema_version: 1,
        generator: "radroots_xtask.protocol_codegen.v1",
        package: PACKAGE,
        source_hash_algorithm: HASH_ALGORITHM,
        sources: inventories,
        schemas,
    };
    let mut inventory_bytes = serde_json::to_vec_pretty(&inventory)
        .map_err(|error| format!("failed to serialize protocol DTO inventory: {error}"))?;
    inventory_bytes.push(b'\n');
    let digest_bytes = format!("{}\n", sha256_hex(&inventory_bytes)).into_bytes();

    Ok(vec![
        GeneratedFile {
            path: safe_workspace_file(
                workspace_root,
                &config.inventory_path,
                true,
                "protocol DTO inventory output",
            )?,
            display_path: config.inventory_path.clone(),
            bytes: inventory_bytes,
        },
        GeneratedFile {
            path: safe_workspace_file(
                workspace_root,
                &config.inventory_sha256_path,
                true,
                "protocol DTO inventory digest output",
            )?,
            display_path: config.inventory_sha256_path.clone(),
            bytes: digest_bytes,
        },
    ])
}

fn serialized_public_types(
    module: &str,
    source_path: &str,
    source: &str,
) -> Result<Vec<TypeInventory>, String> {
    let syntax = syn::parse_file(source)
        .map_err(|error| format!("failed to parse protocol DTO source `{source_path}`: {error}"))?;
    let manual_serializers = syntax
        .items
        .iter()
        .filter_map(manual_serialize_target)
        .collect::<BTreeSet<_>>();
    let mut types = BTreeMap::new();
    for item in &syntax.items {
        let (name, kind, visibility, attributes) = match item {
            Item::Enum(item) => (&item.ident, "enum", &item.vis, &item.attrs),
            Item::Struct(item) => (&item.ident, "struct", &item.vis, &item.attrs),
            _ => continue,
        };
        if !matches!(visibility, Visibility::Public(_)) {
            continue;
        }
        let name = name.to_string();
        let derives_serialize = attributes.iter().any(|attribute| {
            attribute
                .meta
                .to_token_stream()
                .to_string()
                .contains("Serialize")
        });
        if derives_serialize || manual_serializers.contains(&name) {
            types.insert(
                name.clone(),
                TypeInventory {
                    rust_path: format!("radroots_protocol::{module}::{name}"),
                    kind,
                },
            );
        }
    }
    if types.is_empty() {
        return Err(format!(
            "protocol DTO source `{source_path}` exposes no serialized public types"
        ));
    }
    Ok(types.into_values().collect())
}

fn manual_serialize_target(item: &Item) -> Option<String> {
    let Item::Impl(item) = item else {
        return None;
    };
    let (_, trait_path, _) = item.trait_.as_ref()?;
    if trait_path.segments.last()?.ident != "Serialize" {
        return None;
    }
    let Type::Path(self_type) = item.self_ty.as_ref() else {
        return None;
    };
    Some(self_type.path.segments.last()?.ident.to_string())
}

fn check_outputs(generated: &[GeneratedFile]) -> Result<(), String> {
    let stale = generated
        .iter()
        .filter_map(|output| match fs::read(&output.path) {
            Ok(actual) if actual == output.bytes => None,
            Ok(_) => Some(format!("stale `{}`", output.display_path)),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                Some(format!("missing `{}`", output.display_path))
            }
            Err(error) => Some(format!("unreadable `{}`: {error}", output.display_path)),
        })
        .collect::<Vec<_>>();
    if stale.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "generated protocol DTO inventory is not fresh:\n- {}\nrun `cargo xtask generate protocol --write`",
            stale.join("\n- ")
        ))
    }
}

fn write_outputs(generated: &[GeneratedFile]) -> Result<(), String> {
    let mut staged = Vec::new();
    for output in generated {
        if fs::read(&output.path).is_ok_and(|actual| actual == output.bytes) {
            continue;
        }
        let parent = output
            .path
            .parent()
            .ok_or_else(|| format!("generated output has no parent: `{}`", output.display_path))?;
        let mut temporary = NamedTempFile::new_in(parent)
            .map_err(|error| format!("failed to stage `{}`: {error}", output.display_path))?;
        temporary
            .write_all(&output.bytes)
            .map_err(|error| format!("failed to stage `{}`: {error}", output.display_path))?;
        set_generated_permissions(temporary.path()).map_err(|error| {
            format!(
                "failed to set generated permissions for `{}`: {error}",
                output.display_path
            )
        })?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| format!("failed to sync `{}`: {error}", output.display_path))?;
        staged.push((output, temporary));
    }
    for (output, temporary) in staged {
        temporary.persist(&output.path).map_err(|error| {
            format!(
                "failed to commit `{}`: {}",
                output.display_path, error.error
            )
        })?;
    }
    check_outputs(generated)
}

#[cfg(unix)]
fn set_generated_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o644))
}

#[cfg(not(unix))]
fn set_generated_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn safe_workspace_file(
    workspace_root: &Path,
    relative: &str,
    allow_missing: bool,
    role: &str,
) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || relative.contains('\\')
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "{role} path must be normalized and workspace-relative: `{relative}`"
        ));
    }
    let mut current = workspace_root.to_path_buf();
    let count = path.components().count();
    for (index, component) in path.components().enumerate() {
        let Component::Normal(segment) = component else {
            return Err(format!("{role} path is not normalized: `{relative}`"));
        };
        current.push(segment);
        match current.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "{role} path contains a symlink: `{}`",
                    current.display()
                ));
            }
            Ok(metadata) if index + 1 == count && !metadata.is_file() => {
                return Err(format!("{role} must be a regular file: `{relative}`"));
            }
            Ok(_) => {}
            Err(error)
                if error.kind() == ErrorKind::NotFound && allow_missing && index + 1 == count => {}
            Err(error) => {
                return Err(format!("failed to inspect {role} `{relative}`: {error}"));
            }
        }
    }
    Ok(current)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn public_serialized_type_inventory_is_sorted_and_excludes_native_types() {
        let types = serialized_public_types(
            "demo::v1",
            "demo.rs",
            r#"
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Zebra { pub value: String }
pub struct NativeError;
pub struct Alpha(String);
impl serde::Serialize for Alpha {}
struct PrivateWire;
"#,
        )
        .expect("inventory");
        assert_eq!(
            types
                .iter()
                .map(|item| item.rust_path.as_str())
                .collect::<Vec<_>>(),
            [
                "radroots_protocol::demo::v1::Alpha",
                "radroots_protocol::demo::v1::Zebra"
            ]
        );
    }

    #[test]
    fn generated_bytes_and_freshness_check_are_deterministic() {
        let workspace = TempDir::new().expect("workspace");
        let root = workspace.path();
        fs::create_dir_all(root.join("src")).expect("source directory");
        fs::create_dir_all(root.join("out")).expect("output directory");
        fs::write(
            root.join("src/types.rs"),
            "#[derive(serde::Serialize)] pub struct Demo { pub value: String }\n",
        )
        .expect("source");
        let config = Config {
            schema_version: 1,
            package: PACKAGE.to_owned(),
            source_hash_algorithm: HASH_ALGORITHM.to_owned(),
            inventory_path: "out/inventory.json".to_owned(),
            inventory_sha256_path: "out/inventory.sha256".to_owned(),
            source: vec![SourceConfig {
                module: "demo::v1".to_owned(),
                path: "src/types.rs".to_owned(),
            }],
        };
        let schemas = vec![SchemaInventory {
            schema_id: "demo.message.v1".to_owned(),
            module: "demo::v1".to_owned(),
            generation: 1,
        }];
        let first = render_outputs(root, &config, schemas.clone()).expect("first render");
        let second = render_outputs(root, &config, schemas).expect("second render");
        assert_eq!(first[0].bytes, second[0].bytes);
        assert_eq!(first[1].bytes, second[1].bytes);
        write_outputs(&first).expect("write");
        check_outputs(&second).expect("fresh");
        fs::write(root.join("out/inventory.json"), "stale\n").expect("drift");
        let error = check_outputs(&second).expect_err("reject drift");
        assert!(error.contains("stale `out/inventory.json`"));
    }

    #[test]
    fn normalized_paths_reject_escape_and_symlinks() {
        let workspace = TempDir::new().expect("workspace");
        for path in ["", "../escape", "/absolute", "a/./b", "a\\b"] {
            assert!(safe_workspace_file(workspace.path(), path, true, "fixture").is_err());
        }
    }
}

use std::fs;
use std::io::{self, ErrorKind, Write};
use std::path::{Component, Path, PathBuf};

use dto_bindgen_core::{
    Config, RootDiscoveryConfig, RootDiscoveryMode, generate_root_module, scan_rust_source,
};
use tempfile::NamedTempFile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Check,
    Write,
}

#[derive(Debug)]
struct GeneratedRootFile {
    path: PathBuf,
    display_path: String,
    contents: String,
}

struct StagedRootFile {
    path: PathBuf,
    display_path: String,
    temporary: NamedTempFile,
    original: Option<OriginalFileState>,
}

struct OriginalRootFile {
    path: PathBuf,
    display_path: String,
    contents: Option<Vec<u8>>,
    permissions: Option<fs::Permissions>,
}

struct OriginalFileState {
    contents: Vec<u8>,
    permissions: fs::Permissions,
}

type PersistFailure = (NamedTempFile, io::Error);

pub(crate) fn run(args: &[String], workspace_root: &Path) -> Result<(), String> {
    let mode = match args {
        [arg] if arg == "--check" => Mode::Check,
        [arg] if arg == "--write" => Mode::Write,
        _ => return Err("usage: cargo xtask dto-roots --check|--write".to_owned()),
    };
    execute(workspace_root, mode)
}

pub(crate) fn check(workspace_root: &Path) -> Result<(), String> {
    execute(workspace_root, Mode::Check)
}

fn execute(workspace_root: &Path, mode: Mode) -> Result<(), String> {
    let config_relative_path = "dto_bindgen.toml";
    let config_path = workspace_path(workspace_root, config_relative_path)?;
    validate_existing_regular_file(workspace_root, config_relative_path, "DTO root authority")?;
    let config = Config::from_toml_path(&config_path).map_err(|error| {
        format!(
            "failed to load DTO root authority `{}`: {error}",
            config_path.display()
        )
    })?;
    let generated = generate_configured_roots(workspace_root, &config)?;
    validate_output_paths(workspace_root, &generated)?;

    match mode {
        Mode::Check => check_generated_roots(&generated),
        Mode::Write => write_generated_roots(&generated),
    }
}

fn generate_configured_roots(
    workspace_root: &Path,
    config: &Config,
) -> Result<Vec<GeneratedRootFile>, String> {
    let mut generated = Vec::new();
    if config.root_discovery.mode == RootDiscoveryMode::SourceManifest {
        generated.push(generate_discovery_roots(
            workspace_root,
            config,
            "top-level root discovery",
            &config.root_discovery,
        )?);
    }
    for package in &config.packages {
        if package.root_discovery.mode != RootDiscoveryMode::SourceManifest {
            continue;
        }
        let discovery = RootDiscoveryConfig {
            mode: package.root_discovery.mode,
            source_files: package.root_discovery.source_files.clone(),
            root_module_file: package.root_discovery.root_module_file.clone(),
        };
        generated.push(generate_discovery_roots(
            workspace_root,
            config,
            &format!("package `{}`", package.key),
            &discovery,
        )?);
    }

    if generated.is_empty() {
        return Err("dto_bindgen.toml defines no source-manifest DTO roots".to_owned());
    }

    generated.sort_by(|left, right| left.display_path.cmp(&right.display_path));
    Ok(generated)
}

fn generate_discovery_roots(
    workspace_root: &Path,
    config: &Config,
    authority_label: &str,
    discovery: &RootDiscoveryConfig,
) -> Result<GeneratedRootFile, String> {
    let mut inventories = Vec::with_capacity(discovery.source_files.len());
    for source_file in &discovery.source_files {
        let path = workspace_path(workspace_root, source_file)?;
        validate_existing_regular_file(workspace_root, source_file, "DTO source")?;
        let input = fs::read_to_string(&path).map_err(|error| {
            format!("failed to read DTO source `{source_file}` for {authority_label}: {error}")
        })?;
        inventories.push(
            scan_rust_source(source_file.clone(), &input).map_err(|error| error.to_string())?,
        );
    }

    let mut package_config = config.clone();
    package_config.root_discovery = discovery.clone();
    let module = generate_root_module(&package_config, &inventories)
        .map_err(|error| format!("failed to generate DTO roots for {authority_label}: {error}"))?;
    let path = workspace_path(workspace_root, &module.path)?;

    Ok(GeneratedRootFile {
        path,
        display_path: module.path,
        contents: module.contents,
    })
}

fn workspace_path(workspace_root: &Path, configured_path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(configured_path);
    if configured_path.is_empty()
        || configured_path.contains('\\')
        || configured_path
            .split('/')
            .any(|segment| matches!(segment, "" | "." | ".."))
        || has_windows_drive_prefix(configured_path)
        || relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "DTO root authority path must be a normalized workspace-relative path: `{configured_path}`"
        ));
    }
    Ok(workspace_root.join(relative))
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn validate_no_symlink_components(
    workspace_root: &Path,
    configured_path: &str,
    allow_missing_final: bool,
    role: &str,
) -> Result<(), String> {
    let relative = Path::new(configured_path);
    let component_count = relative.components().count();
    let mut current = workspace_root.to_path_buf();
    for (index, component) in relative.components().enumerate() {
        let Component::Normal(segment) = component else {
            return Err(format!(
                "{role} path is not normalized: `{configured_path}`"
            ));
        };
        current.push(segment);
        match current.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "{role} path contains a symlink component: `{}`",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error)
                if error.kind() == ErrorKind::NotFound
                    && allow_missing_final
                    && index + 1 == component_count =>
            {
                return Ok(());
            }
            Err(error) => {
                return Err(format!(
                    "failed to inspect {role} path component `{}`: {error}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn validate_existing_regular_file(
    workspace_root: &Path,
    configured_path: &str,
    role: &str,
) -> Result<(), String> {
    validate_no_symlink_components(workspace_root, configured_path, false, role)?;
    let path = workspace_path(workspace_root, configured_path)?;
    let metadata = path
        .metadata()
        .map_err(|error| format!("failed to inspect {role} `{configured_path}`: {error}"))?;
    if !metadata.is_file() {
        return Err(format!(
            "{role} must be a regular file: `{configured_path}`"
        ));
    }
    Ok(())
}

fn check_generated_roots(generated: &[GeneratedRootFile]) -> Result<(), String> {
    let mut stale = Vec::new();
    for output in generated {
        match fs::read(&output.path) {
            Ok(current) if current == output.contents.as_bytes() => {}
            Ok(_) => stale.push(format!("stale `{}`", output.display_path)),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                stale.push(format!("missing `{}`", output.display_path));
            }
            Err(error) => {
                return Err(format!(
                    "failed to read generated DTO roots `{}`: {error}",
                    output.display_path
                ));
            }
        }
    }

    if stale.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "generated DTO roots are not fresh:\n- {}\nrun `cargo xtask dto-roots --write`",
            stale.join("\n- ")
        ))
    }
}

fn validate_output_paths(
    workspace_root: &Path,
    generated: &[GeneratedRootFile],
) -> Result<(), String> {
    let canonical_root = workspace_root.canonicalize().map_err(|error| {
        format!(
            "failed to resolve DTO workspace root `{}`: {error}",
            workspace_root.display()
        )
    })?;
    for output in generated {
        validate_no_symlink_components(
            workspace_root,
            &output.display_path,
            true,
            "generated DTO root",
        )?;
        let parent = output.path.parent().ok_or_else(|| {
            format!(
                "generated DTO root path has no parent: `{}`",
                output.display_path
            )
        })?;
        if !parent.is_dir() {
            return Err(format!(
                "generated DTO root parent does not exist: `{}`",
                parent.display()
            ));
        }
        let canonical_parent = parent.canonicalize().map_err(|error| {
            format!(
                "failed to resolve generated DTO root parent `{}`: {error}",
                parent.display()
            )
        })?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(format!(
                "generated DTO root parent escapes the workspace: `{}`",
                parent.display()
            ));
        }
        let metadata = match output.path.symlink_metadata() {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => {
                return Err(format!(
                    "failed to inspect generated DTO roots `{}`: {error}",
                    output.display_path
                ));
            }
        };
        if metadata
            .as_ref()
            .is_some_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(format!(
                "generated DTO root cannot be a symlink: `{}`",
                output.display_path
            ));
        }
        if metadata
            .as_ref()
            .is_some_and(|metadata| !metadata.is_file())
        {
            return Err(format!(
                "generated DTO root is not a regular file: `{}`",
                output.display_path
            ));
        }
    }
    Ok(())
}

fn write_generated_roots(generated: &[GeneratedRootFile]) -> Result<(), String> {
    write_generated_roots_with(generated, persist_temporary_file)
}

fn write_generated_roots_with<F>(
    generated: &[GeneratedRootFile],
    mut persist: F,
) -> Result<(), String>
where
    F: FnMut(NamedTempFile, &Path) -> Result<(), PersistFailure>,
{
    let staged = stage_generated_roots(generated)?;
    let mut committed = Vec::with_capacity(staged.len());
    for output in staged {
        let original = OriginalRootFile {
            path: output.path.clone(),
            display_path: output.display_path.clone(),
            permissions: output
                .original
                .as_ref()
                .map(|original| original.permissions.clone()),
            contents: output.original.map(|original| original.contents),
        };
        if let Err((_temporary, error)) = persist(output.temporary, &output.path) {
            let rollback = rollback_original_roots(&committed);
            return Err(write_failure_with_rollback(
                &output.display_path,
                &error,
                rollback,
            ));
        }
        committed.push(original);
    }

    if let Err(error) = check_generated_roots(generated) {
        let rollback = rollback_original_roots(&committed);
        return Err(match rollback {
            Ok(()) => format!("{error}; restored all generated DTO roots"),
            Err(rollback_error) => format!("{error}; rollback also failed: {rollback_error}"),
        });
    }
    Ok(())
}

fn stage_generated_roots(generated: &[GeneratedRootFile]) -> Result<Vec<StagedRootFile>, String> {
    let mut staged = Vec::new();
    for output in generated {
        let original = match fs::read(&output.path) {
            Ok(current) if current == output.contents.as_bytes() => continue,
            Ok(current) => {
                let permissions = fs::metadata(&output.path)
                    .map_err(|error| {
                        format!(
                            "failed to inspect generated DTO roots `{}` before staging: {error}",
                            output.display_path
                        )
                    })?
                    .permissions();
                Some(OriginalFileState {
                    contents: current,
                    permissions,
                })
            }
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => {
                return Err(format!(
                    "failed to read generated DTO roots `{}` before staging: {error}",
                    output.display_path
                ));
            }
        };
        let parent = output.path.parent().ok_or_else(|| {
            format!(
                "generated DTO root path has no parent: `{}`",
                output.display_path
            )
        })?;
        let desired_permissions = original
            .as_ref()
            .map(|original| original.permissions.clone())
            .or_else(default_generated_file_permissions);
        let temporary = stage_bytes(
            parent,
            output.contents.as_bytes(),
            desired_permissions.as_ref(),
        )
        .map_err(|error| {
            format!(
                "failed to stage generated DTO roots `{}`: {error}",
                output.display_path
            )
        })?;
        staged.push(StagedRootFile {
            path: output.path.clone(),
            display_path: output.display_path.clone(),
            temporary,
            original,
        });
    }
    Ok(staged)
}

fn stage_bytes(
    parent: &Path,
    contents: &[u8],
    permissions: Option<&fs::Permissions>,
) -> io::Result<NamedTempFile> {
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(contents)?;
    temporary.flush()?;
    if let Some(permissions) = permissions {
        fs::set_permissions(temporary.path(), permissions.clone())?;
    }
    temporary.as_file().sync_all()?;
    let staged = fs::read(temporary.path())?;
    if staged != contents {
        return Err(io::Error::other("staged DTO root bytes do not match input"));
    }
    Ok(temporary)
}

#[cfg(unix)]
fn default_generated_file_permissions() -> Option<fs::Permissions> {
    use std::os::unix::fs::PermissionsExt;

    Some(fs::Permissions::from_mode(0o644))
}

#[cfg(not(unix))]
fn default_generated_file_permissions() -> Option<fs::Permissions> {
    None
}

fn persist_temporary_file(
    temporary: NamedTempFile,
    destination: &Path,
) -> Result<(), PersistFailure> {
    temporary
        .persist(destination)
        .map(|_| ())
        .map_err(|error| (error.file, error.error))
}

fn rollback_original_roots(committed: &[OriginalRootFile]) -> Result<(), String> {
    let mut failures = Vec::new();
    for original in committed.iter().rev() {
        let result = if let Some(contents) = &original.contents {
            let parent = original.path.parent().ok_or_else(|| {
                io::Error::other(format!(
                    "generated DTO root path has no parent: `{}`",
                    original.display_path
                ))
            });
            parent
                .and_then(|parent| stage_bytes(parent, contents, original.permissions.as_ref()))
                .and_then(|temporary| {
                    persist_temporary_file(temporary, &original.path)
                        .map_err(|(_temporary, error)| error)
                })
        } else {
            match fs::remove_file(&original.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        };
        if let Err(error) = result {
            failures.push(format!("`{}`: {error}", original.display_path));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn write_failure_with_rollback(
    display_path: &str,
    error: &io::Error,
    rollback: Result<(), String>,
) -> String {
    match rollback {
        Ok(()) => format!(
            "failed to commit generated DTO roots `{display_path}`: {error}; restored all previously committed outputs"
        ),
        Err(rollback_error) => format!(
            "failed to commit generated DTO roots `{display_path}`: {error}; rollback also failed: {rollback_error}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use tempfile::TempDir;

    fn fixture_workspace() -> TempDir {
        let workspace = TempDir::new().expect("create fixture workspace");
        let root = workspace.path();
        fs::create_dir_all(root.join("crates/demo/src/generated"))
            .expect("create fixture workspace");
        fs::write(
            root.join("dto_bindgen.toml"),
            r#"schema_version = 1

[[package]]
key = "demo"
rust_package = "demo"
rust_crate = "demo"
npm = "@radroots/demo"
out_dir = "target/demo"

[package.root_discovery]
mode = "source_manifest"
source_files = ["crates/demo/src/model.rs"]
root_module_file = "crates/demo/src/generated/dto_roots.rs"
"#,
        )
        .expect("write fixture config");
        fs::write(
            root.join("crates/demo/src/model.rs"),
            r#"#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
pub struct DemoDto {
    pub value: String,
}
"#,
        )
        .expect("write fixture source");
        workspace
    }

    fn add_second_package(root: &Path) {
        fs::create_dir_all(root.join("crates/second/src/generated"))
            .expect("create second package");
        let config_path = root.join("dto_bindgen.toml");
        let mut config = fs::read_to_string(&config_path).expect("read fixture config");
        config.push_str(
            r#"
[[package]]
key = "second"
rust_package = "second"
rust_crate = "second"
npm = "@radroots/second"
out_dir = "target/second"

[package.root_discovery]
mode = "source_manifest"
source_files = ["crates/second/src/model.rs"]
root_module_file = "crates/second/src/generated/dto_roots.rs"
"#,
        );
        fs::write(config_path, config).expect("write two-package config");
        fs::write(
            root.join("crates/second/src/model.rs"),
            r#"#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
pub struct SecondDto {
    pub value: String,
}
"#,
        )
        .expect("write second fixture source");
    }

    #[test]
    fn write_then_check_is_deterministic() {
        let workspace = fixture_workspace();
        let root = workspace.path();
        run(&["--write".to_owned()], root).expect("write roots");
        let output = root.join("crates/demo/src/generated/dto_roots.rs");
        let first = fs::read_to_string(&output).expect("read generated roots");
        assert!(first.contains("crate::model::DemoDto"));

        run(&["--write".to_owned()], root).expect("repeat write roots");
        assert_eq!(
            fs::read_to_string(&output).expect("read repeated roots"),
            first
        );
        check(root).expect("fresh roots");
    }

    #[test]
    fn check_aggregates_two_package_drift_without_mutation() {
        let workspace = fixture_workspace();
        let root = workspace.path();
        add_second_package(root);
        run(&["--write".to_owned()], root).expect("write roots");
        let demo = root.join("crates/demo/src/generated/dto_roots.rs");
        let second = root.join("crates/second/src/generated/dto_roots.rs");
        fs::write(&demo, "stale\n").expect("write drift");
        fs::remove_file(&second).expect("remove generated roots");

        let stale = check(root).expect_err("reject stale roots");
        assert!(stale.contains("stale `crates/demo/src/generated/dto_roots.rs`"));
        assert!(stale.contains("missing `crates/second/src/generated/dto_roots.rs`"));
        assert!(stale.contains("cargo xtask dto-roots --write"));
        assert_eq!(fs::read_to_string(&demo).expect("read drift"), "stale\n");
        assert!(!second.exists());

        run(&["--write".to_owned()], root).expect("repair both roots");
        check(root).expect("both roots fresh");
        assert!(
            fs::read_to_string(demo)
                .expect("read demo roots")
                .contains("DemoDto")
        );
        assert!(
            fs::read_to_string(second)
                .expect("read second roots")
                .contains("SecondDto")
        );
    }

    #[test]
    fn rejects_invalid_modes_paths_and_missing_authority() {
        let workspace = fixture_workspace();
        let root = workspace.path();
        for args in [
            Vec::<String>::new(),
            vec!["--unknown".to_owned()],
            vec!["--check".to_owned(), "extra".to_owned()],
            vec!["--check".to_owned(), "--write".to_owned()],
        ] {
            let mode = run(&args, root).expect_err("require one explicit valid mode");
            assert!(mode.contains("--check|--write"));
        }

        for invalid in ["", "/tmp/output.rs", "../output.rs", "a\\b.rs", "a/./b.rs"] {
            assert!(workspace_path(root, invalid).is_err(), "accepted {invalid}");
        }

        fs::remove_file(root.join("dto_bindgen.toml")).expect("remove authority");
        let missing = check(root).expect_err("reject missing authority");
        assert!(missing.contains("failed to inspect DTO root authority path component"));
    }

    #[test]
    fn supports_top_level_source_manifest_authority() {
        let workspace = fixture_workspace();
        let root = workspace.path();
        fs::write(
            root.join("dto_bindgen.toml"),
            r#"schema_version = 1

[root_discovery]
mode = "source_manifest"
source_files = ["crates/demo/src/model.rs"]
root_module_file = "crates/demo/src/generated/dto_roots.rs"
"#,
        )
        .expect("write top-level root authority");

        run(&["--write".to_owned()], root).expect("write top-level roots");
        check(root).expect("top-level roots fresh");
        assert!(
            fs::read_to_string(root.join("crates/demo/src/generated/dto_roots.rs"))
                .expect("read top-level roots")
                .contains("DemoDto")
        );
    }

    #[test]
    fn persist_failure_rolls_back_every_committed_output() {
        let workspace = fixture_workspace();
        let root = workspace.path();
        add_second_package(root);
        let config = Config::from_toml_path(root.join("dto_bindgen.toml"))
            .expect("load two-package authority");
        let generated = generate_configured_roots(root, &config).expect("generate roots");
        validate_output_paths(root, &generated).expect("validate root paths");
        let demo = root.join("crates/demo/src/generated/dto_roots.rs");
        let second = root.join("crates/second/src/generated/dto_roots.rs");
        fs::write(&demo, "original demo\n").expect("write original demo");
        fs::write(&second, "original second\n").expect("write original second");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(&demo, fs::Permissions::from_mode(0o640))
                .expect("set demo permissions");
            fs::set_permissions(&second, fs::Permissions::from_mode(0o604))
                .expect("set second permissions");
        }

        let persist_count = Cell::new(0_u8);
        let error = write_generated_roots_with(&generated, |temporary, destination| {
            persist_count.set(persist_count.get() + 1);
            if persist_count.get() == 2 {
                Err((temporary, io::Error::other("injected persist failure")))
            } else {
                persist_temporary_file(temporary, destination)
            }
        })
        .expect_err("second persist must fail");

        assert!(error.contains("injected persist failure"));
        assert!(error.contains("restored all previously committed outputs"));
        assert_eq!(
            fs::read_to_string(&demo).expect("read demo"),
            "original demo\n"
        );
        assert_eq!(
            fs::read_to_string(&second).expect("read second"),
            "original second\n"
        );
        #[cfg(unix)]
        {
            assert_eq!(unix_mode(&demo), 0o640);
            assert_eq!(unix_mode(&second), 0o604);
        }
    }

    #[test]
    fn persist_failure_removes_a_newly_committed_output() {
        let workspace = fixture_workspace();
        let root = workspace.path();
        add_second_package(root);
        let config = Config::from_toml_path(root.join("dto_bindgen.toml"))
            .expect("load two-package authority");
        let generated = generate_configured_roots(root, &config).expect("generate roots");
        validate_output_paths(root, &generated).expect("validate root paths");
        let demo = root.join("crates/demo/src/generated/dto_roots.rs");
        let second = root.join("crates/second/src/generated/dto_roots.rs");
        fs::write(&second, "original second\n").expect("write original second");

        let persist_count = Cell::new(0_u8);
        write_generated_roots_with(&generated, |temporary, destination| {
            persist_count.set(persist_count.get() + 1);
            if persist_count.get() == 2 {
                Err((temporary, io::Error::other("injected persist failure")))
            } else {
                persist_temporary_file(temporary, destination)
            }
        })
        .expect_err("second persist must fail");

        assert!(!demo.exists());
        assert_eq!(
            fs::read_to_string(second).expect("read second"),
            "original second\n"
        );
    }

    #[cfg(unix)]
    fn unix_mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;

        fs::metadata(path)
            .expect("read permissions")
            .permissions()
            .mode()
            & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn write_creates_and_preserves_governed_source_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let workspace = fixture_workspace();
        let root = workspace.path();
        let output = root.join("crates/demo/src/generated/dto_roots.rs");
        run(&["--write".to_owned()], root).expect("create generated roots");
        assert_eq!(unix_mode(&output), 0o644);

        fs::write(&output, "stale\n").expect("make roots stale");
        fs::set_permissions(&output, fs::Permissions::from_mode(0o640))
            .expect("set custom permissions");
        run(&["--write".to_owned()], root).expect("replace generated roots");
        assert_eq!(unix_mode(&output), 0o640);
    }

    #[cfg(unix)]
    #[test]
    fn check_rejects_generated_output_symlinks() {
        use std::os::unix::fs::symlink;

        let workspace = fixture_workspace();
        let root = workspace.path();
        let output = root.join("crates/demo/src/generated/dto_roots.rs");
        let target = root.join("target.rs");
        fs::write(&target, "outside authority\n").expect("write symlink target");
        symlink(&target, &output).expect("create generated root symlink");

        let error = check(root).expect_err("reject generated root symlink");
        assert!(error.contains("generated DTO root path contains a symlink component"));
    }

    #[cfg(unix)]
    #[test]
    fn check_rejects_source_and_output_parent_symlinks() {
        use std::os::unix::fs::symlink;

        let source_workspace = fixture_workspace();
        let source_root = source_workspace.path();
        let source = source_root.join("crates/demo/src/model.rs");
        let outside_workspace = TempDir::new().expect("create outside workspace");
        let outside_source = outside_workspace.path().join("outside_source.rs");
        fs::write(
            &outside_source,
            fs::read_to_string(&source).expect("read source"),
        )
        .expect("write outside source");
        fs::remove_file(&source).expect("remove source");
        symlink(&outside_source, &source).expect("create source symlink");

        let source_error = check(source_root).expect_err("reject source symlink");
        assert!(source_error.contains("DTO source path contains a symlink component"));

        let output_workspace = fixture_workspace();
        let output_root = output_workspace.path();
        let output_parent = output_root.join("crates/demo/src/generated");
        let outside_parent_workspace = TempDir::new().expect("create outside output workspace");
        let outside_parent = outside_parent_workspace.path().join("generated");
        fs::create_dir_all(&outside_parent).expect("create outside output parent");
        fs::remove_dir(&output_parent).expect("remove output parent");
        symlink(&outside_parent, &output_parent).expect("create output parent symlink");

        let output_error = check(output_root).expect_err("reject output parent symlink");
        assert!(output_error.contains("generated DTO root path contains a symlink component"));
    }

    #[test]
    fn authority_and_sources_must_be_regular_files() {
        let source_workspace = fixture_workspace();
        let source_root = source_workspace.path();
        let source = source_root.join("crates/demo/src/model.rs");
        fs::remove_file(&source).expect("remove source file");
        fs::create_dir(&source).expect("create source directory");
        let source_error = check(source_root).expect_err("reject source directory");
        assert!(source_error.contains("DTO source must be a regular file"));

        let authority_workspace = fixture_workspace();
        let authority_root = authority_workspace.path();
        let authority = authority_root.join("dto_bindgen.toml");
        fs::remove_file(&authority).expect("remove authority file");
        fs::create_dir(&authority).expect("create authority directory");
        let authority_error = check(authority_root).expect_err("reject authority directory");
        assert!(authority_error.contains("DTO root authority must be a regular file"));
    }
}

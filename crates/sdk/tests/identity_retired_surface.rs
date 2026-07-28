#![cfg(feature = "identity-models")]

use std::{
    fs,
    path::{Path, PathBuf},
};

const RETIRED_IDENTIFIERS: &[&str] = &[
    "IdentityError",
    "RadrootsEncryptedIdentityFile",
    "RadrootsIdentity",
    "RadrootsIdentityEncryptedSecretKeyOptions",
    "RadrootsIdentityEncryptedSecretKeySecurity",
    "RadrootsIdentityFile",
    "RadrootsIdentityId",
    "RadrootsIdentityProfile",
    "RadrootsIdentityPublic",
    "RadrootsIdentitySecretKeyFormat",
    "RadrootsPublicKey",
];

#[test]
fn sdk_production_surfaces_do_not_restore_retired_identity_apis() {
    let workspace_root = workspace_root();
    let mut sources = Vec::new();
    for root in ["crates", "packages"] {
        collect_production_sources(&workspace_root.join(root), &mut sources);
    }
    assert!(!sources.is_empty(), "SDK production sources are required");

    let mut findings = Vec::new();
    for path in sources {
        let source = fs::read_to_string(&path).expect("read SDK production source");
        for (line_index, line) in source.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for retired in RETIRED_IDENTIFIERS {
                if contains_identifier(line, retired) {
                    findings.push(format!(
                        "{}:{} restores retired identity identifier `{retired}`",
                        relative_path(&workspace_root, &path),
                        line_index + 1,
                    ));
                }
            }
            if line.contains("identity-storage") {
                findings.push(format!(
                    "{}:{} restores retired identity-storage feature",
                    relative_path(&workspace_root, &path),
                    line_index + 1,
                ));
            }
        }
    }

    assert!(
        findings.is_empty(),
        "retired SDK identity surface violations:\n{}",
        findings.join("\n")
    );
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("SDK workspace root")
        .to_path_buf()
}

fn collect_production_sources(directory: &Path, paths: &mut Vec<PathBuf>) {
    if !directory.exists() {
        return;
    }
    for entry in fs::read_dir(directory).expect("read SDK source directory") {
        let path = entry.expect("SDK source entry").path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) != Some("target") {
                collect_production_sources(&path, paths);
            }
            continue;
        }

        let in_production_root = path
            .components()
            .any(|component| matches!(component.as_os_str().to_str(), Some("src" | "examples")));
        let supported_extension = matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "ts" | "js")
        );
        if in_production_root && supported_extension
            || path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml")
        {
            paths.push(path);
        }
    }
}

fn contains_identifier(source: &str, identifier: &str) -> bool {
    source.match_indices(identifier).any(|(index, _)| {
        let before = source[..index].chars().next_back();
        let after = source[index + identifier.len()..].chars().next();
        before.is_none_or(|character| !is_identifier_character(character))
            && after.is_none_or(|character| !is_identifier_character(character))
    })
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("source path is under workspace root")
        .to_string_lossy()
        .replace('\\', "/")
}

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const RELEASE_POLICY: &str = include_str!("../../../contracts/releases/publish_policy.toml");

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
    concat!("Radroots", "PublicKey"),
];

const RETIRED_FUNCTIONS_AND_CONSTANTS: &[&str] = &[
    "DEFAULT_IDENTITY_PATH",
    "RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT",
    "RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX",
    "encrypted_identity_wrapping_key_path",
    "load_encrypted_identity",
    "load_identity_profile",
    "rotate_encrypted_identity",
    "store_encrypted_identity",
    "store_identity_profile",
];

#[test]
fn first_party_production_sources_do_not_restore_the_retired_identity_surface() {
    let workspace_root = workspace_root();
    let mut sources = Vec::new();
    collect_production_rust_sources(&workspace_root.join("crates"), &mut sources);
    assert!(
        !sources.is_empty(),
        "workspace production sources are required"
    );

    let mut findings = Vec::new();
    for path in sources {
        let source = fs::read_to_string(&path).expect("read production source");
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
            for retired in RETIRED_FUNCTIONS_AND_CONSTANTS {
                if contains_identifier(line, retired) {
                    findings.push(format!(
                        "{}:{} restores retired identity operation `{retired}`",
                        relative_path(&workspace_root, &path),
                        line_index + 1,
                    ));
                }
            }
        }
    }

    assert!(
        findings.is_empty(),
        "retired identity surface violations:\n{}",
        findings.join("\n")
    );
}

#[test]
fn retired_identity_implementation_modules_are_absent() {
    let identity_source = workspace_root().join("crates/identity/src");
    for retired in ["identity.rs", "storage.rs", "test_fixtures.rs"] {
        assert!(
            !identity_source.join(retired).exists(),
            "retired identity module must remain absent: {retired}"
        );
    }
}

#[test]
fn release_policy_keeps_replacement_public_and_mixed_packages_private() {
    assert!(
        table(RELEASE_POLICY, "[publication]").contains("frozen = true"),
        "publication must remain frozen during the refactor"
    );

    let approved = string_array(RELEASE_POLICY, "[publication]", "approved_packages");
    assert!(approved.contains("radroots_identity"));
    for mixed in [
        "radroots_authority",
        "radroots_nostr_accounts",
        "radroots_nostr_signer",
    ] {
        assert!(
            !approved.contains(mixed),
            "mixed package must not be approved for publication: {mixed}"
        );
    }

    let private = string_array(RELEASE_POLICY, "[workspace_classification]", "private");
    for mixed in [
        "radroots_authority",
        "radroots_nostr_accounts",
        "radroots_nostr_signer",
    ] {
        assert!(
            private.contains(mixed),
            "mixed package must remain explicitly private until its scheduled retirement: {mixed}"
        );
        let manifest = workspace_root()
            .join("crates")
            .join(mixed.strip_prefix("radroots_").expect("package prefix"))
            .join("Cargo.toml");
        assert!(
            fs::read_to_string(&manifest)
                .unwrap_or_else(|error| panic!("read {}: {error}", manifest.display()))
                .lines()
                .any(|line| line.trim() == "publish = false"),
            "mixed package manifest must remain unpublished: {}",
            manifest.display()
        );
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lib workspace root")
        .to_path_buf()
}

fn collect_production_rust_sources(directory: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read workspace source directory") {
        let path = entry.expect("workspace source entry").path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) != Some("target") {
                collect_production_rust_sources(&path, paths);
            }
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs")
            && path
                .components()
                .any(|component| component.as_os_str() == "src")
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

fn table<'a>(source: &'a str, heading: &str) -> &'a str {
    let body = source
        .split_once(heading)
        .unwrap_or_else(|| panic!("missing table {heading}"))
        .1;
    body.split_once("\n[").map_or(body, |(current, _)| current)
}

fn string_array<'a>(source: &'a str, heading: &str, key: &str) -> BTreeSet<&'a str> {
    let table = table(source, heading);
    let marker = format!("{key} = [");
    let values = table
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing {heading} {key}"))
        .1
        .split_once(']')
        .unwrap_or_else(|| panic!("unterminated {heading} {key}"))
        .0;
    values
        .split(',')
        .map(str::trim)
        .filter_map(|value| {
            value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
        })
        .collect()
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("source path is under workspace root")
        .to_string_lossy()
        .replace('\\', "/")
}

use std::fs;
use std::path::{Path, PathBuf};

struct ForbiddenEventName {
    pattern: &'static str,
    reason: &'static str,
}

const FORBIDDEN_EVENT_NAMES: &[ForbiddenEventName] = &[
    ForbiddenEventName {
        pattern: "pub fn from_wire_unchecked",
        reason: "unchecked signed-event construction must not be public API",
    },
    ForbiddenEventName {
        pattern: "WireEventParts",
        reason: "event construction must use RadrootsNip01EventWireParts",
    },
    ForbiddenEventName {
        pattern: "RadrootsFrozenEventDraft",
        reason: "event construction must use RadrootsEventDraft",
    },
    ForbiddenEventName {
        pattern: "RadrootsNostrEvent",
        reason: "product-level event surfaces must use protocol-neutral domain names",
    },
    ForbiddenEventName {
        pattern: "RadrootsNostrEventRef",
        reason: "product-level event references must use RadrootsEventRef",
    },
    ForbiddenEventName {
        pattern: "RadrootsNostrEventPtr",
        reason: "product-level event pointers must use RadrootsEventPtr",
    },
    ForbiddenEventName {
        pattern: "RadrootsSignedNostrEvent",
        reason: "signed-event surfaces must use RadrootsSignedEvent",
    },
    ForbiddenEventName {
        pattern: "RadrootsSignedNostrEventParts",
        reason: "signed-event parts must use protocol-neutral domain names",
    },
];

#[test]
fn generic_event_source_does_not_reintroduce_old_core_nostr_event_names() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lib repo root");
    let mut findings = Vec::new();

    for path in source_boundary_guard_files(repo_root) {
        let relative_path = relative_path(repo_root, path.as_path());
        let source = fs::read_to_string(path.as_path()).expect("read source file");
        for forbidden in FORBIDDEN_EVENT_NAMES {
            if forbidden_event_name_allowed(relative_path.as_str(), forbidden.pattern) {
                continue;
            }
            if contains_forbidden_event_name(source.as_str(), forbidden.pattern) {
                findings.push(format!(
                    "{} contains retired event concept `{}`: {}",
                    relative_path, forbidden.pattern, forbidden.reason
                ));
            }
        }
    }

    assert!(
        findings.is_empty(),
        "radroots_event source-boundary violations:\n{}",
        findings.join("\n")
    );
}

fn source_boundary_guard_files(repo_root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for relative_root in ["crates", "contracts"] {
        let root = repo_root.join(relative_root);
        if root.exists() {
            collect_source_boundary_guard_files(root.as_path(), &mut paths);
        }
    }
    paths.sort();
    paths
}

fn collect_source_boundary_guard_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("read source directory") {
        let entry = entry.expect("source entry");
        let path = entry.path();
        if path.is_dir() {
            if matches!(
                path.file_name().and_then(|file_name| file_name.to_str()),
                Some("target")
            ) {
                continue;
            }
            collect_source_boundary_guard_files(path.as_path(), paths);
            continue;
        }

        if path.file_name().and_then(|file_name| file_name.to_str()) == Some("source_boundary.rs") {
            continue;
        }

        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs") | Some("toml") | Some("md")
        ) {
            paths.push(path);
        }
    }
}

fn forbidden_event_name_allowed(relative_path: &str, pattern: &str) -> bool {
    pattern == "RadrootsNostrEvent" && is_nostr_protocol_context(relative_path)
}

fn is_nostr_protocol_context(relative_path: &str) -> bool {
    relative_path.starts_with("crates/nostr/")
        || relative_path.starts_with("crates/nostrdb/")
        || relative_path.starts_with("crates/nostr_runtime/")
        || relative_path.starts_with("crates/nostr_signer/")
        || relative_path.starts_with("crates/transport_nostr/")
        || relative_path.starts_with("crates/net/src/nostr_client/")
}

fn contains_forbidden_event_name(source: &str, pattern: &str) -> bool {
    source.match_indices(pattern).any(|(index, _)| {
        let before = source[..index].chars().next_back();
        let after = source[index + pattern.len()..].chars().next();
        before.is_none_or(|character| !is_identifier_character(character))
            && after.is_none_or(|character| !is_identifier_character(character))
    })
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("source path is under repo root")
        .to_string_lossy()
        .replace('\\', "/")
}

use std::fs;
use std::path::{Path, PathBuf};

struct ForbiddenEventName {
    pattern: &'static str,
    reason: &'static str,
}

const FORBIDDEN_EVENT_NAMES: &[ForbiddenEventName] = &[
    ForbiddenEventName {
        pattern: "RadrootsPublicKey",
        reason: "public author keys must use radroots_identity::PublicKey",
    },
    ForbiddenEventName {
        pattern: "pub fn from_wire_unchecked",
        reason: "unchecked signed-event construction must not be public API",
    },
    ForbiddenEventName {
        pattern: "WireEventParts",
        reason: "event construction must use Nip01EventWireParts",
    },
    ForbiddenEventName {
        pattern: "RadrootsFrozenEventDraft",
        reason: "generic event construction must use GenericEventDraft",
    },
    ForbiddenEventName {
        pattern: "RadrootsNostrEvent",
        reason: "product-level event surfaces must use protocol-neutral domain names",
    },
    ForbiddenEventName {
        pattern: "RadrootsNostrEventRef",
        reason: "product-level event references must use EventRef",
    },
    ForbiddenEventName {
        pattern: "RadrootsNostrEventPtr",
        reason: "product-level event pointers must use EventPtr",
    },
    ForbiddenEventName {
        pattern: "RadrootsSignedNostrEvent",
        reason: "signed-event surfaces must use SignedEvent",
    },
    ForbiddenEventName {
        pattern: "RadrootsSignedNostrEventParts",
        reason: "signed-event parts must use protocol-neutral domain names",
    },
];

const RETIRED_EVENT_MODULE_PATHS: &[&str] = &[
    "radroots_event::event_head",
    "radroots_event::events",
    "radroots_event::ids",
    "radroots_event::kinds",
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

#[test]
fn first_party_sources_do_not_reintroduce_retired_event_module_paths() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lib repo root");
    let mut findings = Vec::new();

    for path in source_boundary_guard_files(repo_root) {
        let relative_path = relative_path(repo_root, path.as_path());
        let source = fs::read_to_string(path.as_path()).expect("read source file");
        for retired_path in RETIRED_EVENT_MODULE_PATHS {
            if source.contains(retired_path) {
                findings.push(format!(
                    "{relative_path} contains retired event module path `{retired_path}`"
                ));
            }
        }
    }

    assert!(
        findings.is_empty(),
        "retired radroots_event module paths remain:\n{}",
        findings.join("\n")
    );
}

#[test]
fn public_api_has_no_redundant_radroots_type_prefixes() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lib repo root");
    let baseline = fs::read_to_string(repo_root.join("docs/api/radroots_event.txt"))
        .expect("read radroots_event public API baseline");
    let mut prefixed = baseline
        .split(|character: char| !is_identifier_character(character))
        .filter(|identifier| {
            identifier
                .strip_prefix("Radroots")
                .and_then(|suffix| suffix.chars().next())
                .is_some_and(|character| character.is_ascii_uppercase())
        })
        .collect::<Vec<_>>();
    prefixed.sort_unstable();
    prefixed.dedup();

    assert!(
        prefixed.is_empty(),
        "radroots_event public API contains redundant prefixed identifiers: {}",
        prefixed.join(", ")
    );
}

#[test]
fn typed_authoring_mapping_is_not_implemented_in_the_generic_event_draft() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lib repo root");
    let draft = fs::read_to_string(repo_root.join("crates/event/src/draft.rs"))
        .expect("read generic event draft source");

    for retired in [
        "from_authored_update",
        "from_authored_reply",
        "from_authored_profile",
        "from_typed_parts",
        "TypedAuthoringKind",
        "typed_authoring",
    ] {
        assert!(
            !draft.contains(retired),
            "generic authored input retains codec-owned typed mapping `{retired}`"
        );
    }
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
                Some("generated" | "target")
            ) {
                continue;
            }
            collect_source_boundary_guard_files(path.as_path(), paths);
            continue;
        }

        if path.file_name().and_then(|file_name| file_name.to_str()) == Some("source_boundary.rs") {
            continue;
        }

        if path.ends_with("event_store/src/nip09/reconciliation_v1/result_vector_executor.rs") {
            // This predecessor is an authenticated immutable artifact. Active NIP-09
            // execution uses the maintained successor surface; the frozen source must
            // retain its historical bytes so its provenance check remains meaningful.
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
        || relative_path.starts_with("crates/nostr_signer/")
        || relative_path.starts_with("crates/transport_nostr/")
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

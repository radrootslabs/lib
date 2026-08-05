use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn public_source_does_not_expose_the_upstream_event_builder() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lib repo root");
    let mut findings = Vec::new();

    for path in rust_source_files(repo_root.join("crates").as_path()) {
        if path.file_name().and_then(|name| name.to_str()) == Some("generic_builder_boundary.rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read Rust source");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");
        for statement in normalized.split(';') {
            let statement = statement.trim();
            let exposes_alias =
                statement.starts_with("pub type ") && statement.contains("EventBuilder");
            let exposes_reexport =
                statement.starts_with("pub use nostr") && statement.contains("EventBuilder");
            if exposes_alias || exposes_reexport {
                findings.push(format!(
                    "{} exposes upstream EventBuilder: {statement}",
                    relative_path(repo_root, &path)
                ));
            }
        }

        for forbidden in [
            "impl Deref for GenericBuilder",
            "impl AsRef<nostr::EventBuilder> for GenericBuilder",
            "impl From<GenericBuilder> for nostr::EventBuilder",
            "impl Into<nostr::EventBuilder> for GenericBuilder",
            "impl Deref for ExternalSigningRequest",
            "impl AsRef<nostr::UnsignedEvent> for ExternalSigningRequest",
            "impl From<ExternalSigningRequest> for nostr::UnsignedEvent",
            "impl Into<nostr::UnsignedEvent> for ExternalSigningRequest",
            "impl Deserialize for ExternalSigningRequest",
            "fn into_unsigned_event",
            "fn as_unsigned_event",
            "fn unsigned_event_mut",
        ] {
            if normalized.contains(forbidden) {
                findings.push(format!(
                    "{} contains forbidden raw-builder escape `{forbidden}`",
                    relative_path(repo_root, &path)
                ));
            }
        }
    }

    assert!(
        findings.is_empty(),
        "public generic-builder boundary violations:\n{}",
        findings.join("\n")
    );
}

#[test]
fn every_strict_builder_is_plan_backed_and_has_no_unchecked_mapping_path() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (module, expected_body_conversions) in [
        ("metadata.rs", 1),
        ("post.rs", 3),
        ("reply.rs", 1),
        ("deletion.rs", 1),
        ("comment.rs", 1),
        ("food_availability.rs", 1),
    ] {
        let source = fs::read_to_string(crate_root.join("src/events").join(module))
            .unwrap_or_else(|error| panic!("read {module}: {error}"));
        assert!(
            source.contains("SealedBuilderCore"),
            "strict builder module {module} does not use the shared plan core"
        );
        assert_eq!(
            source.matches("AuthoredEventBody::from_").count(),
            expected_body_conversions,
            "strict builder module {module} has an incomplete body-conversion inventory"
        );
        for forbidden in [
            "build_event_unchecked",
            "RadrootsNostrEventBuilderUnchecked",
            "_to_wire_parts",
        ] {
            assert!(
                !source.contains(forbidden),
                "strict builder module {module} retains unchecked mapping `{forbidden}`"
            );
        }
        for required in ["sign_with_keys", "into_external_signing_request"] {
            assert!(
                source.contains(required),
                "strict builder module {module} is missing `{required}`"
            );
        }
    }
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_source_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("read source directory") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            collect_rust_source_files(&path, paths);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("source path is under repo root")
        .to_string_lossy()
        .replace('\\', "/")
}

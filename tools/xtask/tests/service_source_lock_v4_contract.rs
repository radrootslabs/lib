#![forbid(unsafe_code)]

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CONTRACT: &str = include_str!(
    "../../../contracts/architecture/decisions/services_hardening_source_lock.v4.json"
);
const PREDECESSOR: &[u8] = include_bytes!(
    "../../../contracts/architecture/decisions/services_hardening_source_lock.v3.json"
);

fn schema() -> Value {
    serde_json::from_str(CONTRACT).expect("source-lock contract must be valid JSON")
}

// Independent structural input, not a generated source lock or execution receipt.
fn input(service: &str) -> Value {
    json!({
        "schema": "radroots.service.source_lock.v4",
        "contract_version": 4,
        "service": service,
        "repository": "https://github.com/radrootslabs/lib",
        "revision": "1111111111111111111111111111111111111111",
        "architecture": "radroots.crates.release.v2",
        "workspace_catalog_sha256": "2".repeat(64),
        "version": "0.1.0-alpha",
        "source_archive_sha256": "3".repeat(64),
        "source_archive_contract": {
            "binding": "sha256_of_canonical_exact_lib_revision_tree_archive",
            "format": "ustar",
            "compression": "none",
            "compression_timestamp": "not_applicable",
            "entry_order": "bytewise_git_path",
            "path_prefix": "none",
            "file_mode": "git_index_100644_or_100755",
            "uid": 0,
            "gid": 0,
            "uname": "",
            "gname": "",
            "mtime": "lib_revision_commit_timestamp",
            "pax_headers": "forbidden",
            "directory_entries": "omitted",
            "symlinks": "forbidden",
            "hardlinks": "forbidden",
            "submodules": "forbidden",
            "trailer": "two_zero_blocks"
        },
        "cargo_lock_sha256": "4".repeat(64),
        "rust_version": "1.99.0",
        "host_feature_profile": "service-host",
        "nix": {
            "material": "bound",
            "lib_revision": "1111111111111111111111111111111111111111",
            "supported_systems": ["aarch64-darwin", "x86_64-linux"],
            "public_input_lock": {
                "path": "flake.lock",
                "sha256": "5".repeat(64),
                "binding": "exact_regular_file_bytes",
                "mutable_reference": "forbidden",
                "lib_input": "lib"
            },
            "parent_result": {
                "embedded_in_public_input_lock": false,
                "embedded_in_source_lock": false,
                "storage": "separate_source_scoped_execution_evidence"
            }
        },
        "artifact_contract": {
            "path": format!("contracts/release/{service}-artifact-contract.v4.json"),
            "sha256": "6".repeat(64),
            "binding": "exact_regular_file_bytes_in_same_source_revision"
        },
        "sqlite": {
            "high_level_authority": "sqlx_only",
            "second_pool_connection_query_transaction_migration_authority": "forbidden",
            "incremental_backup_adapter": "sealed_native_sqlx_owned_locked_handle_only",
            "native_linkage_count": 1
        },
        "contract_versions": {"config": 1, "state": 2, "admin": 3, "status": 4, "provider": 5}
    })
}

#[test]
fn both_services_accept_complete_current_input_bindings() {
    let schema = schema();
    assert_eq!(schema["$id"], "radroots.service.source_lock.v4");
    let validator = jsonschema::validator_for(&schema).expect("valid source-lock schema");
    for service in ["myc", "rhi"] {
        assert!(validator.is_valid(&input(service)), "{service}");
    }
}

#[test]
fn compiler_identity_is_exact_stable_1_99_0() {
    let validator = jsonschema::validator_for(&schema()).expect("valid source-lock schema");
    for compiler in [
        "1.97.1", "1.98.0", "1.99", "1.99.1", "1.100.0", "stable", "nightly",
    ] {
        let mut value = input("myc");
        value["rust_version"] = json!(compiler);
        assert!(!validator.is_valid(&value), "{compiler}");
    }
}

#[test]
fn required_bindings_cannot_be_omitted() {
    let validator = jsonschema::validator_for(&schema()).expect("valid source-lock schema");
    for (parent, fields) in [
        (
            "",
            &[
                "schema",
                "contract_version",
                "service",
                "repository",
                "revision",
                "architecture",
                "workspace_catalog_sha256",
                "version",
                "source_archive_sha256",
                "source_archive_contract",
                "cargo_lock_sha256",
                "rust_version",
                "host_feature_profile",
                "nix",
                "artifact_contract",
                "sqlite",
                "contract_versions",
            ][..],
        ),
        (
            "/nix",
            &[
                "material",
                "lib_revision",
                "supported_systems",
                "public_input_lock",
                "parent_result",
            ][..],
        ),
        (
            "/nix/public_input_lock",
            &[
                "path",
                "sha256",
                "binding",
                "mutable_reference",
                "lib_input",
            ][..],
        ),
        ("/artifact_contract", &["path", "sha256", "binding"][..]),
        (
            "/contract_versions",
            &["config", "state", "admin", "status", "provider"][..],
        ),
    ] {
        for field in fields {
            let mut value = input("rhi");
            value
                .pointer_mut(parent)
                .expect("fixture parent")
                .as_object_mut()
                .expect("fixture object")
                .remove(*field);
            assert!(!validator.is_valid(&value), "{parent}/{field}");
        }
    }
}

#[test]
fn every_object_rejects_unknown_material() {
    let validator = jsonschema::validator_for(&schema()).expect("valid source-lock schema");
    for parent in [
        "",
        "/source_archive_contract",
        "/nix",
        "/nix/public_input_lock",
        "/nix/parent_result",
        "/artifact_contract",
        "/sqlite",
        "/contract_versions",
    ] {
        let mut value = input("myc");
        value
            .pointer_mut(parent)
            .expect("fixture object")
            .as_object_mut()
            .expect("fixture object")
            .insert("unrecognized_material".to_owned(), json!(true));
        assert!(!validator.is_valid(&value), "{parent}");
    }
}

#[test]
fn identities_and_positive_contract_versions_are_bounded() {
    let validator = jsonschema::validator_for(&schema()).expect("valid source-lock schema");
    for (pointer, invalid) in [
        ("/revision", json!("main")),
        ("/revision", json!("A".repeat(40))),
        ("/workspace_catalog_sha256", json!("2".repeat(63))),
        ("/source_archive_sha256", json!("3".repeat(65))),
        ("/cargo_lock_sha256", json!("G".repeat(64))),
        ("/nix/lib_revision", json!("HEAD")),
        ("/nix/public_input_lock/sha256", json!(null)),
        ("/artifact_contract/sha256", json!("")),
        ("/repository", json!("https://example.invalid/lib")),
        ("/contract_versions/config", json!(0)),
        ("/contract_versions/state", json!(-1)),
        ("/contract_versions/admin", json!(4_294_967_296_u64)),
        ("/contract_versions/status", json!(1.5)),
        ("/contract_versions/provider", json!("1")),
    ] {
        let mut value = input("rhi");
        *value.pointer_mut(pointer).expect("fixture field") = invalid;
        assert!(!validator.is_valid(&value), "{pointer}");
    }
}

#[test]
fn artifact_binding_belongs_to_the_declared_service_and_revision() {
    let validator = jsonschema::validator_for(&schema()).expect("valid source-lock schema");
    for (service, path) in [
        ("myc", "contracts/release/rhi-artifact-contract.v4.json"),
        ("rhi", "contracts/release/myc-artifact-contract.v4.json"),
        ("myc", "contracts/release/myc-artifact-contract.v3.json"),
        ("rhi", "../rhi-artifact-contract.v4.json"),
    ] {
        let mut value = input(service);
        value["artifact_contract"]["path"] = json!(path);
        assert!(!validator.is_valid(&value), "{service}: {path}");
    }
    let mut value = input("myc");
    value["artifact_contract"]["binding"] = json!("latest_available_contract");
    assert!(!validator.is_valid(&value));
}

#[test]
fn source_binding_cannot_embed_or_assert_executed_qualification() {
    let validator = jsonschema::validator_for(&schema()).expect("valid source-lock schema");
    for (pointer, invalid) in [
        ("/nix/material", json!("qualified")),
        ("/nix/material", json!("absent")),
        ("/nix/material", json!("deferred")),
        ("/nix/public_input_lock/mutable_reference", json!("allowed")),
        (
            "/nix/parent_result/embedded_in_public_input_lock",
            json!(true),
        ),
        ("/nix/parent_result/embedded_in_source_lock", json!(true)),
        (
            "/nix/parent_result/storage",
            json!("separate_generation_scoped_evidence"),
        ),
    ] {
        let mut value = input("myc");
        *value.pointer_mut(pointer).expect("fixture field") = invalid;
        assert!(!validator.is_valid(&value), "{pointer}");
    }
}

#[test]
fn archive_and_single_sqlite_authority_cannot_be_weakened() {
    let validator = jsonschema::validator_for(&schema()).expect("valid source-lock schema");
    for (pointer, invalid) in [
        ("/source_archive_contract/format", json!("pax")),
        ("/source_archive_contract/mtime", json!("wall_clock")),
        ("/source_archive_contract/symlinks", json!("allowed")),
        ("/sqlite/high_level_authority", json!("multiple")),
        ("/sqlite/native_linkage_count", json!(2)),
        (
            "/sqlite/incremental_backup_adapter",
            json!("independent_connection"),
        ),
    ] {
        let mut value = input("rhi");
        *value.pointer_mut(pointer).expect("fixture field") = invalid;
        assert!(!validator.is_valid(&value), "{pointer}");
    }
}

#[test]
fn predecessor_contract_bytes_and_compiler_identity_remain_historical() {
    assert_eq!(
        hex::encode(Sha256::digest(PREDECESSOR)),
        "3bc32c8ca2cecb06c8f8239ab1fe1fcfba93fe3ef0d60e9b078390347d08f817"
    );
    let old: Value = serde_json::from_slice(PREDECESSOR).expect("historical schema");
    assert_eq!(old["properties"]["rust_version"]["const"], "1.97.1");
    assert_eq!(
        old["properties"]["nix"]["properties"]["material"]["const"],
        "qualified"
    );
}

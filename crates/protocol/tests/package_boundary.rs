use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const CAPABILITY: &str = include_str!("../src/capability.rs");
const ERROR: &str = include_str!("../src/error.rs");
const EVENT: &str = include_str!("../src/event.rs");
const RADROOTSD: &str = include_str!("../src/radrootsd.rs");
const RUNTIME: &str = include_str!("../src/runtime.rs");
#[cfg(feature = "terms")]
const TERMS_CONTRACT: &str = include_str!("../../../contracts/protocol/terms.v1.json");
#[cfg(feature = "commerce")]
const COMMERCE_CONTRACT: &str = include_str!("../../../contracts/protocol/commerce.v1.json");
const VERSIONED_SOURCES: &[&str] = &[
    include_str!("../src/capability/v1.rs"),
    include_str!("../src/error/v1.rs"),
    include_str!("../src/event/v1.rs"),
    include_str!("../src/radrootsd/transport_publish/v5.rs"),
    include_str!("../src/runtime/v1.rs"),
    include_str!("../src/schema.rs"),
    include_str!("../src/profile.rs"),
    include_str!("../src/terms.rs"),
    include_str!("../src/terms/writer.rs"),
    include_str!("../src/terms/decode.rs"),
    include_str!("../src/terms/text.rs"),
    include_str!("../src/commerce.rs"),
    include_str!("../src/commerce/decode.rs"),
    include_str!("../src/commerce/payload.rs"),
    include_str!("../src/command.rs"),
    include_str!("../src/nip01.rs"),
    include_str!("../src/intent.rs"),
    include_str!("../src/chat.rs"),
    include_str!("../src/chat/routes.rs"),
    include_str!("../src/chat/plan.rs"),
    include_str!("../src/listing.rs"),
    include_str!("../src/listing/input.rs"),
    include_str!("../src/listing_values.rs"),
    include_str!("../src/listing/command.rs"),
    include_str!("../src/listing/plan.rs"),
    include_str!("../src/plan.rs"),
];

#[cfg(feature = "commerce")]
#[test]
fn complete_record_contract_binds_closed_families_limits_and_independent_fixtures() {
    use sha2::{Digest as _, Sha256};
    let contract: serde_json::Value = serde_json::from_str(COMMERCE_CONTRACT).unwrap();
    assert_eq!(contract["schema"], "radroots.commerce.codec.v1");
    assert_eq!(
        contract["content_max_utf8_bytes"],
        radroots_protocol::commerce::RECORD_MAX_BYTES
    );
    assert_eq!(
        contract["carrier_kind"],
        radroots_protocol::profile::COMMERCE_KIND
    );
    assert_eq!(
        contract["record_id"]["domain_utf8_with_terminal_nul"],
        "radroots.record.v1\0"
    );
    assert_eq!(
        contract["limits"]["structured_rumor_authoring_enabled"],
        false
    );
    assert_eq!(
        contract["carrier_matching"]["creates_authenticated_capability"],
        false
    );
    let fixture_bytes = include_bytes!("fixtures/commerce_wire.v1.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(fixture_bytes)),
        contract["independent_vectors"]["sha256"]
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(include_bytes!("support/commerce_wire_oracle.mjs"))
        ),
        contract["independent_vectors"]["producer_sha256"]
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(include_bytes!("fixtures/terms_wire.v1.json"))
        ),
        contract["independent_vectors"]["source_terms_fixture_sha256"]
    );
    let fixture: serde_json::Value = serde_json::from_slice(fixture_bytes).unwrap();
    let required: BTreeSet<_> = contract["header_required_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(required.len(), 14);
    assert_eq!(
        contract["payload_required_fields"]
            .as_object()
            .unwrap()
            .len(),
        19
    );
    for row in fixture["vectors"].as_array().unwrap() {
        let value: serde_json::Value = serde_json::from_str(row["wire"].as_str().unwrap()).unwrap();
        assert_eq!(
            value
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            required
        );
        let fields: BTreeSet<_> = contract["payload_required_fields"]
            [row["type"].as_str().unwrap()]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
        assert_eq!(
            value["payload"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            fields
        );
    }
}

#[cfg(feature = "terms")]
#[test]
fn terms_contract_binds_limits_hash_domain_and_packaged_independent_fixtures() {
    use sha2::{Digest as _, Sha256};
    let contract: serde_json::Value = serde_json::from_str(TERMS_CONTRACT).unwrap();
    assert_eq!(contract["schema"], "radroots.terms.codec.v1");
    assert_eq!(contract["decoding"]["objects_only"], true);
    assert_eq!(contract["decoding"]["max_json_depth"], 16);
    assert_eq!(contract["decoding"]["max_lines"], 32);
    assert_eq!(contract["decoding"]["max_fulfillment_groups"], 8);
    assert_eq!(
        contract["limits"]["authenticated_admission_implemented_by_this_contract"],
        false
    );
    assert_eq!(
        contract["content_max_utf8_bytes"],
        radroots_protocol::terms::TERMS_MAX_BYTES
    );
    assert_eq!(
        contract["digest"]["domain_utf8_with_terminal_nul"],
        "radroots.terms.v1\0"
    );
    assert_eq!(
        contract["limits"]["structured_record_authoring_enabled"],
        false
    );
    assert_eq!(
        contract["retention"]["automatic_private_serialize_or_clone"],
        false
    );
    assert_eq!(
        contract["retention"]["private_fields_in_temporary_json_trees"],
        false
    );
    let fixture_bytes = include_bytes!("fixtures/terms_wire.v1.json");
    let producer = include_bytes!("support/terms_wire_oracle.mjs");
    assert_eq!(
        format!("{:x}", Sha256::digest(fixture_bytes)),
        contract["independent_vectors"]["sha256"]
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(producer)),
        contract["independent_vectors"]["producer_sha256"]
    );
    let fixture: serde_json::Value = serde_json::from_slice(fixture_bytes).unwrap();
    let required: BTreeSet<_> = contract["required_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(required.len(), 9);
    for row in fixture["vectors"].as_array().unwrap() {
        let object: serde_json::Value =
            serde_json::from_str(row["wire"].as_str().unwrap()).unwrap();
        assert_eq!(
            object
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            required
        );
    }
}

#[test]
fn manifest_has_approved_data_only_features_and_dependencies() {
    assert!(MANIFEST.contains("name = \"radroots_protocol\""));
    assert!(MANIFEST.contains("version = \"0.1.0-alpha\""));
    assert!(MANIFEST.contains("publish = [\"crates-io\"]"));
    assert!(MANIFEST.contains("[lib]\nname = \"radroots_protocol\""));
    assert!(MANIFEST.contains("default = [\"std\", \"serde\"]"));
    assert_eq!(
        table_keys(MANIFEST, "[features]"),
        BTreeSet::from([
            "chat", "commands", "commerce", "default", "intent", "listing", "nip01", "serde",
            "std", "terms"
        ])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dependencies]"),
        BTreeSet::from([
            "radroots_domain",
            "radroots_identity",
            "serde",
            "serde_json",
            "sha2",
            "zeroize"
        ])
    );
    assert_eq!(
        table_keys(MANIFEST, "[dev-dependencies]"),
        BTreeSet::from(["serde_json"])
    );
}

#[test]
fn crate_root_exposes_only_the_approved_versioned_skeleton() {
    assert!(ROOT.contains("#![cfg_attr(not(feature = \"std\"), no_std)]"));
    assert_eq!(
        root_declarations("pub mod "),
        BTreeSet::from([
            "capability",
            "chat",
            "command",
            "commerce",
            "error",
            "event",
            "nip01",
            "intent",
            "listing",
            "plan",
            "profile",
            "radrootsd",
            "runtime",
            "schema",
            "terms"
        ])
    );
    for source in [CAPABILITY, EVENT] {
        assert!(source.lines().any(|line| line.trim() == "pub mod v1;"));
    }
    assert!(ERROR.lines().any(|line| line.trim() == "pub mod v1;"));
    assert!(RUNTIME.lines().any(|line| line.trim() == "pub mod v1;"));
    assert!(RADROOTSD.contains("pub mod transport_publish {"));
    assert!(RADROOTSD.lines().any(|line| line.trim() == "pub mod v5;"));
    assert!(
        !ROOT
            .lines()
            .map(str::trim)
            .any(|line| line.starts_with("pub use "))
    );
}

#[test]
fn public_contract_sources_exclude_runtime_dependencies_and_wrapper_traits() {
    assert!(ROOT.contains("#![forbid(unsafe_code)]"));
    for source in VERSIONED_SOURCES {
        for forbidden in [
            "dto_bindgen",
            "wasm_bindgen",
            "uniffi",
            "tokio::",
            "sqlx::",
            "reqwest::",
            "nostr::",
            "std::fs",
            "std::net",
            "std::process",
            "impl core::ops::Deref",
            "impl std::ops::Deref",
            "unsafe fn",
            "unsafe impl",
            "unsafe {",
        ] {
            assert!(
                !source.contains(forbidden),
                "protocol contract source contains forbidden surface `{forbidden}`"
            );
        }
    }
}

fn table_keys<'a>(manifest: &'a str, heading: &str) -> BTreeSet<&'a str> {
    let Some((_, table)) = manifest.split_once(heading) else {
        return BTreeSet::new();
    };
    table
        .lines()
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| {
            let line = line.trim();
            (line
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_')
                && !line.starts_with('#'))
            .then(|| line.split_once('=').map(|(key, _)| key.trim()))
            .flatten()
        })
        .collect()
}

fn root_declarations(prefix: &str) -> BTreeSet<&str> {
    ROOT.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix(prefix))
        .filter_map(|name| name.strip_suffix(';'))
        .collect()
}

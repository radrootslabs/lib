use std::{fs, path::Path};

#[test]
fn packaged_conformance_fixtures_match_canonical_contracts() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    for (canonical, packaged) in [
        (
            "contracts/conformance/vectors/core/value_serialization.v1.json",
            "crates/core/tests/fixtures/value_serialization.v1.json",
        ),
        (
            "contracts/conformance/vectors/identity/public_values.v1.json",
            "crates/identity/tests/fixtures/public_values.v1.json",
        ),
        (
            "contracts/conformance/vectors/protocol/wire_values.v1.json",
            "crates/protocol/tests/fixtures/wire_values.v1.json",
        ),
        (
            "contracts/codegen/protocol_v1.inventory.json",
            "crates/protocol/tests/fixtures/protocol_v1.inventory.json",
        ),
        (
            "contracts/conformance/vectors/event/nip01_wire.v1.json",
            "crates/event/tests/fixtures/nip01_wire.v1.json",
        ),
        (
            "contracts/conformance/vectors/nip46/current_session.v1.json",
            "crates/nostr_connect/tests/fixtures/current_session.v1.json",
        ),
        (
            "contracts/conformance/vectors/transport/target_uri.v1.json",
            "crates/transport/tests/fixtures/target_uri.v1.json",
        ),
        (
            "contracts/knowledge/knowledge_event_contract_manifest.v2.json",
            "crates/event_codec/tests/fixtures/knowledge_event_contract_manifest.v2.json",
        ),
        (
            "contracts/knowledge/knowledge_event_contract_manifest.v2.sha256",
            "crates/event_codec/tests/fixtures/knowledge_event_contract_manifest.v2.sha256",
        ),
        (
            "contracts/conformance/vectors/knowledge/public_surface.v1.json",
            "crates/event_codec/tests/fixtures/knowledge_public_surface.v1.json",
        ),
    ] {
        assert_eq!(
            fs::read(workspace.join(canonical)).expect("canonical fixture"),
            fs::read(workspace.join(packaged)).expect("packaged fixture"),
            "packaged fixture {packaged} drifted from {canonical}",
        );
    }
}

use std::collections::BTreeMap;

use radroots_protocol::{
    capability::v1 as capability,
    event::v1 as event,
    schema::{ModuleVersion, SchemaId, protocol_v1_registry},
};
use radroots_protocol_contract_v1 as predecessor;

#[test]
fn capability_and_event_catalog_json_is_byte_identical() {
    assert_eq!(
        serde_json::to_vec(predecessor::TRANSPORT_CAPABILITY_CATALOG_V1)
            .expect("predecessor capability JSON"),
        serde_json::to_vec(capability::CATALOG).expect("successor capability JSON")
    );
    assert_eq!(
        serde_json::to_vec(predecessor::PROTOCOL_EVENT_CATALOG_V1).expect("predecessor event JSON"),
        serde_json::to_vec(event::CATALOG).expect("successor event JSON")
    );
    assert_eq!(
        serde_json::to_vec(predecessor::PROTOCOL_TRADE_STATE_VOCABULARY_V1)
            .expect("predecessor trade state JSON"),
        serde_json::to_vec(event::TRADE_STATE_VOCABULARY).expect("successor trade state JSON")
    );
}

#[test]
fn capability_value_json_is_byte_identical() {
    for (predecessor, successor) in [
        (
            predecessor::TransportKindV1::Local,
            capability::TransportKind::Local,
        ),
        (
            predecessor::TransportKindV1::Nostr,
            capability::TransportKind::Nostr,
        ),
        (
            predecessor::TransportKindV1::Reticulum,
            capability::TransportKind::Reticulum,
        ),
    ] {
        assert_eq!(
            serde_json::to_vec(&predecessor).expect("predecessor transport JSON"),
            serde_json::to_vec(&successor).expect("successor transport JSON")
        );
    }

    let predecessor = predecessor::ReticulumTargetV1 {
        destination: predecessor::ReticulumDestinationV1::parse("reticulum:local")
            .expect("predecessor destination"),
        mesh_scope: Some(
            predecessor::MeshScopeIdV1::parse("local_preview").expect("predecessor scope"),
        ),
    };
    let successor = capability::ReticulumTarget {
        destination: capability::ReticulumDestination::parse("reticulum:local")
            .expect("successor destination"),
        mesh_scope: Some(capability::MeshScopeId::parse("local_preview").expect("successor scope")),
    };
    assert_eq!(
        serde_json::to_vec(&predecessor).expect("predecessor target JSON"),
        serde_json::to_vec(&successor).expect("successor target JSON")
    );
}

#[test]
fn schema_metadata_bytes_and_dispatch_are_preserved() {
    let predecessor = predecessor::PROTOCOL_SCHEMA_METADATA_V1
        .iter()
        .map(|metadata| {
            (
                metadata.schema_id,
                serde_json::to_vec(metadata).expect("predecessor metadata JSON"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let successor = capability::SCHEMAS
        .iter()
        .chain(event::SCHEMAS)
        .map(|metadata| {
            (
                metadata.schema_id,
                serde_json::to_vec(metadata).expect("successor metadata JSON"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(successor, predecessor);

    let registry = protocol_v1_registry().expect("successor schema registry");
    for schema_id in predecessor.keys() {
        let schema_id = SchemaId::parse(*schema_id).expect("predecessor schema id");
        let module = registry.module_for(&schema_id).expect("registered module");
        assert!(matches!(
            module,
            ModuleVersion::CapabilityV1 | ModuleVersion::EventV1
        ));
    }
}

#[test]
fn predecessor_and_successor_validation_are_green() {
    predecessor::validate_protocol_contract_v1().expect("predecessor contract");
    capability::validate_catalog(capability::CATALOG).expect("successor capability catalog");
    event::validate_catalog(event::CATALOG).expect("successor event catalog");
    event::validate_trade_state_vocabulary(event::TRADE_STATE_VOCABULARY)
        .expect("successor trade vocabulary");
    protocol_v1_registry().expect("successor schema registry");
}

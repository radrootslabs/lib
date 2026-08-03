#![cfg(feature = "serde")]

use std::collections::BTreeSet;

use radroots_protocol::{
    capability::v1::{ReticulumTarget, TransportKind},
    error::v1::ErrorReport,
    event::v1::{EventClass, TradeState},
    radrootsd::transport_publish::v5::{DeliveryPolicy, TargetPolicy},
    runtime::v1::{OperationId, Risk, TransportRoute},
    schema::{ModuleVersion, protocol_v1_registry},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

const WIRE_VECTORS: &str =
    include_str!("../../../contracts/conformance/vectors/protocol/wire_values.v1.json");
const GENERATED_INVENTORY: &str =
    include_str!("../../../contracts/codegen/protocol_v1.inventory.json");

#[test]
fn language_neutral_wire_vectors_are_unique_and_executable() {
    let document: Value = serde_json::from_str(WIRE_VECTORS).expect("protocol wire vectors");
    assert_eq!(document["suite"], "protocol_wire_values_v1");
    assert_eq!(document["contract_version"], "1.0.0");
    let vectors = document["vectors"].as_array().expect("vector array");
    let mut ids = BTreeSet::new();
    for vector in vectors {
        let id = vector["id"].as_str().expect("vector id");
        assert!(ids.insert(id), "duplicate protocol vector id `{id}`");
        let kind = vector["kind"].as_str().expect("vector kind");
        let input = vector["input"].clone();
        let result = match kind {
            "protocol.capability.transport_kind" => execute::<TransportKind>(input),
            "protocol.capability.reticulum_target" => execute::<ReticulumTarget>(input),
            "protocol.event.event_class" => execute::<EventClass>(input),
            "protocol.event.trade_state" => execute::<TradeState>(input),
            "protocol.runtime.operation_id" => execute::<OperationId>(input),
            "protocol.runtime.risk" => execute::<Risk>(input),
            "protocol.runtime.transport_route" => execute::<TransportRoute>(input),
            "protocol.radrootsd.target_policy" => execute::<TargetPolicy>(input),
            "protocol.radrootsd.delivery_policy" => execute::<DeliveryPolicy>(input),
            "protocol.error.report" => execute::<ErrorReport>(input),
            other => panic!("unimplemented protocol vector kind `{other}`"),
        };
        match (
            vector.get("expected"),
            vector.get("expected_error_contains"),
            result,
        ) {
            (Some(expected), None, Ok(actual)) => assert_eq!(&actual, expected, "vector `{id}`"),
            (None, Some(expected), Err(error)) => assert!(
                error.contains(expected.as_str().expect("error fragment")),
                "vector `{id}` expected `{expected}`, found `{error}`"
            ),
            (Some(_), None, Err(error)) => panic!("vector `{id}` unexpectedly failed: {error}"),
            (None, Some(_), Ok(actual)) => {
                panic!("vector `{id}` unexpectedly succeeded: {actual}")
            }
            _ => panic!("vector `{id}` has an invalid expectation shape"),
        }
    }
    assert_eq!(ids.len(), 13);
}

#[test]
fn generated_inventory_is_complete_unique_and_matches_the_schema_registry() {
    let inventory: Value =
        serde_json::from_str(GENERATED_INVENTORY).expect("generated protocol inventory");
    assert_eq!(inventory["schema_version"], 1);
    assert_eq!(inventory["package"], "radroots_protocol");

    let sources = inventory["sources"].as_array().expect("sources");
    let actual_modules = sources
        .iter()
        .map(|source| source["module"].as_str().expect("source module"))
        .collect::<BTreeSet<_>>();
    let expected_modules = ModuleVersion::ALL
        .iter()
        .map(|module| module.path())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_modules, expected_modules);

    let type_paths = sources
        .iter()
        .flat_map(|source| source["types"].as_array().expect("source types"))
        .map(|item| item["rust_path"].as_str().expect("Rust path"))
        .collect::<Vec<_>>();
    assert_eq!(type_paths.len(), 60);
    assert_eq!(
        type_paths.iter().copied().collect::<BTreeSet<_>>().len(),
        60
    );
    assert!(type_paths.contains(&"radroots_protocol::runtime::v1::OperationId"));

    let inventory_schemas = inventory["schemas"]
        .as_array()
        .expect("schemas")
        .iter()
        .map(|schema| {
            (
                schema["schema_id"].as_str().expect("schema id").to_owned(),
                schema["module"].as_str().expect("schema module").to_owned(),
                schema["generation"].as_u64().expect("generation") as u16,
            )
        })
        .collect::<BTreeSet<_>>();
    let registry_schemas = protocol_v1_registry()
        .expect("protocol registry")
        .descriptors()
        .iter()
        .map(|descriptor| {
            (
                descriptor.id().as_str().to_owned(),
                descriptor.module().path().to_owned(),
                descriptor.module().generation(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(inventory_schemas.len(), 121);
    assert_eq!(inventory_schemas, registry_schemas);
}

fn execute<T>(input: Value) -> Result<Value, String>
where
    T: DeserializeOwned + Serialize,
{
    let decoded: T = serde_json::from_value(input).map_err(|error| error.to_string())?;
    serde_json::to_value(decoded).map_err(|error| error.to_string())
}

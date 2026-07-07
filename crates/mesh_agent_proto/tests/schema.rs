use radroots_mesh_agent_proto::{
    RADROOTS_MESH_AGENT_SCHEMA, RADROOTS_MESH_AGENT_SCHEMA_ID,
    RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE, RadrootsMeshAgentProtoError, schema_sha256_hex,
    validate_schema, validate_schema_text,
};

#[test]
fn schema_declares_mesh_agent_v1_surface() {
    validate_schema().expect("schema validates");

    assert!(RADROOTS_MESH_AGENT_SCHEMA.contains(RADROOTS_MESH_AGENT_SCHEMA_ID));
    assert!(RADROOTS_MESH_AGENT_SCHEMA.contains(RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE));
    assert!(RADROOTS_MESH_AGENT_SCHEMA.contains("struct MeshAgentRequest"));
    assert!(RADROOTS_MESH_AGENT_SCHEMA.contains("struct MeshAgentResponse"));
}

#[test]
fn schema_hash_is_deterministic() {
    let first = schema_sha256_hex();
    let second = schema_sha256_hex();

    assert_eq!(first, second);
    assert_eq!(
        first,
        "ceaaa2968805a21c9f08b76b22ac45ae819312ee595a60675f560ec81d6f8fe0"
    );
}

#[test]
fn schema_validator_rejects_missing_required_surface() {
    assert_eq!(
        validate_schema_text("@0xb83e0c4f71838d9a;")
            .expect_err("schema missing namespace and structs"),
        RadrootsMeshAgentProtoError::MissingNamespace
    );
}

#[test]
fn schema_validator_reports_each_missing_required_surface() {
    let valid = RADROOTS_MESH_AGENT_SCHEMA;
    let cases = [
        (
            valid.replace(RADROOTS_MESH_AGENT_SCHEMA_ID, "0x0000000000000000"),
            RadrootsMeshAgentProtoError::MissingSchemaId,
        ),
        (
            valid.replace(RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE, "radroots::missing"),
            RadrootsMeshAgentProtoError::MissingNamespace,
        ),
        (
            valid.replace("struct MeshAgentRequest", "struct MissingMeshAgentRequest"),
            RadrootsMeshAgentProtoError::MissingRequest,
        ),
        (
            valid.replace("frameCbor @2 :Data;", "frameBytes @2 :Data;"),
            RadrootsMeshAgentProtoError::MissingRequest,
        ),
        (
            valid.replace("enum MeshAgentAction", "enum MissingMeshAgentAction"),
            RadrootsMeshAgentProtoError::MissingAction,
        ),
        (
            valid.replace("validateFrame @0;", "validateSomethingElse @0;"),
            RadrootsMeshAgentProtoError::MissingAction,
        ),
        (
            valid.replace("stageDelivery @1;", "stageSomethingElse @1;"),
            RadrootsMeshAgentProtoError::MissingAction,
        ),
        (
            valid.replace("observeEventHead @2;", "observeSomethingElse @2;"),
            RadrootsMeshAgentProtoError::MissingAction,
        ),
        (
            valid.replace(
                "struct MeshAgentResponse",
                "struct MissingMeshAgentResponse",
            ),
            RadrootsMeshAgentProtoError::MissingResponse,
        ),
        (
            valid.replace(
                "enum MeshAgentResponseStatus",
                "enum MissingMeshAgentResponseStatus",
            ),
            RadrootsMeshAgentProtoError::MissingResponse,
        ),
        (
            valid.replace("struct MeshAgentReceipt", "struct MissingMeshAgentReceipt"),
            RadrootsMeshAgentProtoError::MissingReceipt,
        ),
        (
            valid.replace(
                "acceptedEventHeads @1 :List(Text);",
                "acceptedEventIds @1 :List(Text);",
            ),
            RadrootsMeshAgentProtoError::MissingReceipt,
        ),
        (
            valid.replace("struct MeshAgentError", "struct MissingMeshAgentError"),
            RadrootsMeshAgentProtoError::MissingError,
        ),
    ];

    for (schema, error) in cases {
        assert_eq!(validate_schema_text(schema.as_str()), Err(error));
    }
}

#[test]
fn mesh_agent_proto_errors_have_stable_display_strings() {
    let cases = [
        (
            RadrootsMeshAgentProtoError::MissingSchemaId,
            "mesh agent schema id is missing",
        ),
        (
            RadrootsMeshAgentProtoError::MissingNamespace,
            "mesh agent schema namespace is missing",
        ),
        (
            RadrootsMeshAgentProtoError::MissingRequest,
            "mesh agent request schema is missing",
        ),
        (
            RadrootsMeshAgentProtoError::MissingAction,
            "mesh agent action schema is missing",
        ),
        (
            RadrootsMeshAgentProtoError::MissingResponse,
            "mesh agent response schema is missing",
        ),
        (
            RadrootsMeshAgentProtoError::MissingReceipt,
            "mesh agent receipt schema is missing",
        ),
        (
            RadrootsMeshAgentProtoError::MissingError,
            "mesh agent error schema is missing",
        ),
    ];

    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
}

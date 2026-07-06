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

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
    assert!(RADROOTS_MESH_AGENT_SCHEMA.contains("struct MeshAgentStatusRequest"));
    assert!(RADROOTS_MESH_AGENT_SCHEMA.contains("struct MeshAgentStatusResponse"));
    assert!(RADROOTS_MESH_AGENT_SCHEMA.contains("struct MeshAgentPublishRequest"));
    assert!(RADROOTS_MESH_AGENT_SCHEMA.contains("struct MeshAgentPublishResponse"));
    assert!(RADROOTS_MESH_AGENT_SCHEMA.contains("enum MeshAgentTransportKind"));
    assert!(RADROOTS_MESH_AGENT_SCHEMA.contains("enum MeshAgentTransportOutcome"));
}

#[test]
fn schema_is_canonical_transport_v1_authority() {
    validate_schema().expect("schema validates");

    assert_eq!(RADROOTS_MESH_AGENT_SCHEMA_ID, "0xb83e0c4f71838d9a");
    assert_ne!(RADROOTS_MESH_AGENT_SCHEMA_ID, "0xdecafbaddecaf001");
    assert_eq!(
        validate_schema_text(
            RADROOTS_MESH_AGENT_SCHEMA
                .replace(RADROOTS_MESH_AGENT_SCHEMA_ID, "0xdecafbaddecaf001")
                .as_str(),
        ),
        Err(RadrootsMeshAgentProtoError::MissingSchemaId)
    );
}

#[test]
fn schema_covers_transport_v1_status_and_publish_intent() {
    for required in [
        "requestId @0 :Text;",
        "statusRequest @3 :MeshAgentStatusRequest;",
        "publishRequest @4 :MeshAgentPublishRequest;",
        "statusResponse @4 :MeshAgentStatusResponse;",
        "publishResponse @5 :MeshAgentPublishResponse;",
        "includeTransports @0 :Bool;",
        "readiness @0 :MeshAgentReadinessState;",
        "implementationState @1 :MeshAgentImplementationState;",
        "transports @2 :List(MeshAgentTransportStatus);",
        "transportKind @0 :MeshAgentTransportKind;",
        "profileId @1 :Text;",
        "publishUsable @5 :Bool;",
        "fetchUsable @6 :Bool;",
        "previewUnavailable @3;",
        "previewNoop @0;",
        "reticulum @0;",
        "publishRequestId @0 :Text;",
        "payloadCbor @1 :Data;",
        "eventId @2 :Text;",
        "targetFingerprint @3 :Text;",
        "transportReceipts @2 :List(MeshAgentTransportReceipt);",
        "eventId @3 :Text;",
        "outcome @2 :MeshAgentTransportOutcome;",
        "deferredUntilImplemented @4;",
        "transportUnavailable @8;",
        "errors @3 :List(MeshAgentError);",
    ] {
        assert!(RADROOTS_MESH_AGENT_SCHEMA.contains(required), "{required}");
    }
}

#[test]
fn schema_hash_is_deterministic() {
    let first = schema_sha256_hex();
    let second = schema_sha256_hex();

    assert_eq!(first, second);
    assert_eq!(
        first,
        "eff807dfb46416296b51fa0214fee869fbe7ec1da068ffc9c1ce4facc308be17"
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
            valid.replace("status @3;", "agentStatus @3;"),
            RadrootsMeshAgentProtoError::MissingAction,
        ),
        (
            valid.replace("publish @4;", "publishSomethingElse @4;"),
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
            valid.replace(
                "struct MeshAgentStatusRequest",
                "struct MissingMeshAgentStatusRequest",
            ),
            RadrootsMeshAgentProtoError::MissingStatusSurface,
        ),
        (
            valid.replace(
                "enum MeshAgentReadinessState",
                "enum MissingMeshAgentReadinessState",
            ),
            RadrootsMeshAgentProtoError::MissingStatusSurface,
        ),
        (
            valid.replace(
                "implementationState @1 :MeshAgentImplementationState;",
                "implementation @1 :MeshAgentImplementationState;",
            ),
            RadrootsMeshAgentProtoError::MissingStatusSurface,
        ),
        (
            valid.replace("profileId @1 :Text;", "transportProfileId @1 :Text;"),
            RadrootsMeshAgentProtoError::MissingStatusSurface,
        ),
        (
            valid.replace("reticulum @0;", "reticulumPreview @0;"),
            RadrootsMeshAgentProtoError::MissingStatusSurface,
        ),
        (
            valid.replace("previewNoop @0;", "previewUnavailable @0;"),
            RadrootsMeshAgentProtoError::MissingStatusSurface,
        ),
        (
            valid.replace(
                "struct MeshAgentPublishRequest",
                "struct MissingMeshAgentPublishRequest",
            ),
            RadrootsMeshAgentProtoError::MissingPublishSurface,
        ),
        (
            valid.replace("publishRequestId @0 :Text;", "operationId @0 :Text;"),
            RadrootsMeshAgentProtoError::MissingPublishSurface,
        ),
        (
            valid.replace("eventId @2 :Text;", "eventIdentifier @2 :Text;"),
            RadrootsMeshAgentProtoError::MissingPublishSurface,
        ),
        (
            valid.replace("targetFingerprint @3 :Text;", "transportTarget @3 :Text;"),
            RadrootsMeshAgentProtoError::MissingPublishSurface,
        ),
        (
            valid.replace(
                "transportReceipts @2 :List(MeshAgentTransportReceipt);",
                "transportResults @2 :List(MeshAgentTransportReceipt);",
            ),
            RadrootsMeshAgentProtoError::MissingPublishSurface,
        ),
        (
            valid.replace(
                "outcome @2 :MeshAgentTransportOutcome;",
                "outcome @2 :Text;",
            ),
            RadrootsMeshAgentProtoError::MissingPublishSurface,
        ),
        (
            valid.replace(
                "transportUnavailable @8;",
                "transportUnavailablePreview @8;",
            ),
            RadrootsMeshAgentProtoError::MissingPublishSurface,
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
fn schema_validator_rejects_commented_required_declarations() {
    let schema =
        RADROOTS_MESH_AGENT_SCHEMA.replace("  frameCbor @2 :Data;", "  # frameCbor @2 :Data;");

    assert_eq!(
        validate_schema_text(schema.as_str()),
        Err(RadrootsMeshAgentProtoError::MissingRequest)
    );
}

#[test]
fn schema_validator_rejects_misplaced_required_declarations() {
    let schema = RADROOTS_MESH_AGENT_SCHEMA
        .replace(
            "  acceptedEventHeads @1 :List(Text);",
            "  acceptedEventIds @1 :List(Text);",
        )
        .replace(
            "  message @1 :Text;",
            "  message @1 :Text;\n  acceptedEventHeads @2 :List(Text);",
        );

    assert_eq!(
        validate_schema_text(schema.as_str()),
        Err(RadrootsMeshAgentProtoError::MissingReceipt)
    );
}

#[test]
fn schema_validator_rejects_duplicate_incompatible_declarations() {
    let duplicate_ordinal = RADROOTS_MESH_AGENT_SCHEMA.replace(
        "  frameCbor @2 :Data;",
        "  frameCbor @2 :Data;\n  frameBytes @2 :Data;",
    );
    let duplicate_name = RADROOTS_MESH_AGENT_SCHEMA.replace(
        "  frameCbor @2 :Data;",
        "  frameCbor @2 :Data;\n  frameCbor @9 :Text;",
    );

    assert_eq!(
        validate_schema_text(duplicate_ordinal.as_str()),
        Err(RadrootsMeshAgentProtoError::InvalidSchema)
    );
    assert_eq!(
        validate_schema_text(duplicate_name.as_str()),
        Err(RadrootsMeshAgentProtoError::InvalidSchema)
    );
}

#[test]
fn schema_validator_rejects_type_drift() {
    let request_type_drift =
        RADROOTS_MESH_AGENT_SCHEMA.replace("  frameCbor @2 :Data;", "  frameCbor @2 :Text;");
    let status_type_drift = RADROOTS_MESH_AGENT_SCHEMA.replace(
        "  includeTransports @0 :Bool;",
        "  includeTransports @0 :Text;",
    );

    assert_eq!(
        validate_schema_text(request_type_drift.as_str()),
        Err(RadrootsMeshAgentProtoError::MissingRequest)
    );
    assert_eq!(
        validate_schema_text(status_type_drift.as_str()),
        Err(RadrootsMeshAgentProtoError::MissingStatusSurface)
    );
}

#[test]
fn mesh_agent_proto_errors_have_stable_display_strings() {
    let cases = [
        (
            RadrootsMeshAgentProtoError::InvalidSchema,
            "mesh agent schema is invalid",
        ),
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
            RadrootsMeshAgentProtoError::MissingStatusSurface,
            "mesh agent status schema surface is missing",
        ),
        (
            RadrootsMeshAgentProtoError::MissingPublishSurface,
            "mesh agent publish schema surface is missing",
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

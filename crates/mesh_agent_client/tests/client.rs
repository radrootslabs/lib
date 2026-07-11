use std::fs;
use std::path::Path;

use radroots_mesh::{RADROOTS_MESH_PREVIEW_DENIAL_MESSAGE, RADROOTS_MESH_PREVIEW_POLICY_ID};
use radroots_mesh_agent_client::{
    MeshAgentImplementation, MeshAgentPublishRequest, MeshAgentResponseStatus,
    MeshAgentStatusRequest, MeshAgentTransportKind, MeshAgentTransportOutcome,
    RADROOTS_MESH_AGENT_CLIENT_SCHEMA_ID, RADROOTS_MESH_AGENT_CLIENT_SCHEMA_NAMESPACE,
    RadrootsMeshAgentClient, RadrootsMockMeshAgentClient,
};
use radroots_mesh_agent_proto::{
    RADROOTS_MESH_AGENT_SCHEMA_ID, RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE, schema_sha256_hex,
};
use radroots_transport::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RADROOTS_RETICULUM_PREVIEW_SCOPE_ID,
    RadrootsTransportKind,
};
use serde_json::json;

fn publish_request() -> MeshAgentPublishRequest {
    MeshAgentPublishRequest {
        publish_request_id: "publish-preview-1".to_owned(),
        payload_cbor: vec![1, 2, 3],
        event_id: "event-preview-1".to_owned(),
        target_fingerprint: "target-preview-1".to_owned(),
    }
}

#[test]
fn mock_client_status_reports_reticulum_preview_unavailable() {
    let client = RadrootsMockMeshAgentClient::preview_unavailable();
    let response = client.status(MeshAgentStatusRequest {
        include_transports: true,
    });

    assert_eq!(client.scope().as_str(), RADROOTS_RETICULUM_PREVIEW_SCOPE_ID);
    assert_eq!(client.policy().policy_id(), RADROOTS_MESH_PREVIEW_POLICY_ID);
    assert_eq!(response.transports.len(), 1);
    let status = &response.transports[0];
    assert_eq!(status.transport, MeshAgentTransportKind::Reticulum);
    assert_eq!(
        status.transport.transport_kind(),
        RadrootsTransportKind::Reticulum
    );
    assert_eq!(status.profile_id, RADROOTS_MESH_PREVIEW_POLICY_ID);
    assert_eq!(status.endpoint_uri, RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI);
    assert!(status.configured);
    assert_eq!(
        status.implementation,
        MeshAgentImplementation::PreviewUnavailable
    );
    assert!(!status.usable_for_delivery);
    assert_eq!(status.message, RADROOTS_MESH_PREVIEW_DENIAL_MESSAGE);
}

#[test]
fn mock_client_status_can_omit_transport_rows_without_enabling_delivery() {
    let client = RadrootsMockMeshAgentClient::preview_unavailable();
    let response = client.status(MeshAgentStatusRequest {
        include_transports: false,
    });

    assert!(response.transports.is_empty());
    assert!(!client.policy().usable_for_delivery());
}

#[test]
fn mock_client_publish_never_reports_success_outcomes() {
    let client = RadrootsMockMeshAgentClient::preview_unavailable();
    let response = client.publish(publish_request());

    assert_eq!(response.publish_request_id, "publish-preview-1");
    assert_eq!(response.status, MeshAgentResponseStatus::Rejected);
    assert_eq!(response.event_id, "event-preview-1");
    assert_eq!(response.transport_receipts.len(), 1);
    let receipt = &response.transport_receipts[0];
    assert_eq!(receipt.transport_kind, MeshAgentTransportKind::Reticulum);
    assert_eq!(
        receipt.endpoint_uri,
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
    );
    assert_eq!(
        receipt.outcome,
        MeshAgentTransportOutcome::TransportUnavailable
    );
    assert!(!receipt.outcome.is_success());
    assert_eq!(receipt.message, RADROOTS_MESH_PREVIEW_DENIAL_MESSAGE);
    for outcome in [
        MeshAgentTransportOutcome::Accepted,
        MeshAgentTransportOutcome::Delivered,
        MeshAgentTransportOutcome::Forwarded,
        MeshAgentTransportOutcome::StoredByGateway,
    ] {
        assert!(outcome.is_success());
        assert_ne!(receipt.outcome, outcome);
    }
}

#[test]
fn dto_schema_names_match_capnp_surface() {
    assert_eq!(MeshAgentTransportKind::Reticulum.schema_name(), "reticulum");
    assert_eq!(
        MeshAgentImplementation::PreviewUnavailable.schema_name(),
        "previewUnavailable"
    );
    assert_eq!(MeshAgentResponseStatus::Accepted.schema_name(), "accepted");
    assert_eq!(MeshAgentResponseStatus::Deferred.schema_name(), "deferred");
    assert_eq!(MeshAgentResponseStatus::Rejected.schema_name(), "rejected");
    assert_eq!(
        MeshAgentTransportOutcome::TransportUnavailable.schema_name(),
        "transportUnavailable"
    );
    assert_eq!(
        MeshAgentTransportOutcome::StoredByGateway.schema_name(),
        "storedByGateway"
    );
}

#[test]
fn serde_output_uses_schema_field_names() {
    let request = publish_request();
    let request_json = serde_json::to_value(&request).expect("serialize request");

    assert_eq!(
        request_json,
        json!({
            "publishRequestId": "publish-preview-1",
            "payloadCbor": [1, 2, 3],
            "eventId": "event-preview-1",
            "targetFingerprint": "target-preview-1"
        })
    );

    let client = RadrootsMockMeshAgentClient::preview_unavailable();
    let response_json = serde_json::to_value(client.publish(request)).expect("serialize response");

    assert_eq!(response_json["publishRequestId"], "publish-preview-1");
    assert_eq!(response_json["status"], "rejected");
    assert_eq!(
        response_json["transportReceipts"][0]["transportKind"],
        "reticulum"
    );
    assert_eq!(
        response_json["transportReceipts"][0]["outcome"],
        "transportUnavailable"
    );
}

#[test]
fn client_schema_hash_matches_proto_authority() {
    let client = RadrootsMockMeshAgentClient::preview_unavailable();

    assert_eq!(
        RADROOTS_MESH_AGENT_CLIENT_SCHEMA_ID,
        RADROOTS_MESH_AGENT_SCHEMA_ID
    );
    assert_eq!(
        RADROOTS_MESH_AGENT_CLIENT_SCHEMA_NAMESPACE,
        RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE
    );
    assert_eq!(client.schema_sha256_hex(), schema_sha256_hex());
    assert_eq!(
        client.schema_sha256_hex(),
        "712aaa11dfec25abf44edb3b0be447f0596442271d46a8b1d9fedb7c3df00bb2"
    );
}

#[test]
fn client_source_uses_schema_field_names_without_legacy_status_terms() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(manifest_dir.join("src/lib.rs")).expect("read source");

    for required in [
        "MeshAgentStatusRequest",
        "include_transports",
        "MeshAgentStatusResponse",
        "transports",
        "MeshAgentTransportStatus",
        "profile_id",
        "endpoint_uri",
        "usable_for_delivery",
        "MeshAgentPublishRequest",
        "publish_request_id",
        "payload_cbor",
        "event_id",
        "target_fingerprint",
        "MeshAgentPublishResponse",
        "transport_receipts",
    ] {
        assert!(source.contains(required), "{required}");
    }
    for forbidden in [
        "readiness",
        "implementation_state",
        "publish_usable",
        "fetch_usable",
        "redacted_message",
        "preview_noop",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
}

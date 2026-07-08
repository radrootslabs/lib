@0xb83e0c4f71838d9a;

using Cxx = import "/capnp/c++.capnp";

$Cxx.namespace("radroots::mesh_agent::v1");

struct MeshAgentRequest {
  requestId @0 :Text;
  action @1 :MeshAgentAction;
  frameCbor @2 :Data;
  statusRequest @3 :MeshAgentStatusRequest;
  publishRequest @4 :MeshAgentPublishRequest;
}

enum MeshAgentAction {
  validateFrame @0;
  stageDelivery @1;
  observeEventHead @2;
  status @3;
  publish @4;
}

struct MeshAgentResponse {
  requestId @0 :Text;
  status @1 :MeshAgentResponseStatus;
  receipt @2 :MeshAgentReceipt;
  errors @3 :List(MeshAgentError);
  statusResponse @4 :MeshAgentStatusResponse;
  publishResponse @5 :MeshAgentPublishResponse;
}

enum MeshAgentResponseStatus {
  accepted @0;
  deferred @1;
  rejected @2;
}

struct MeshAgentReceipt {
  frameDigest @0 :Text;
  acceptedEventHeads @1 :List(Text);
}

struct MeshAgentStatusRequest {
  includeTransports @0 :Bool;
}

struct MeshAgentStatusResponse {
  readiness @0 :MeshAgentReadinessState;
  implementationState @1 :MeshAgentImplementationState;
  transports @2 :List(MeshAgentTransportStatus);
}

struct MeshAgentTransportStatus {
  transportKind @0 :Text;
  profileId @1 :Text;
  endpointUri @2 :Text;
  readiness @3 :MeshAgentReadinessState;
  implementationState @4 :MeshAgentImplementationState;
  publishUsable @5 :Bool;
  fetchUsable @6 :Bool;
  redactedMessage @7 :Text;
}

enum MeshAgentReadinessState {
  ready @0;
  disabled @1;
  misconfigured @2;
  previewUnavailable @3;
}

enum MeshAgentImplementationState {
  available @0;
  disabled @1;
  misconfigured @2;
  previewUnavailable @3;
}

struct MeshAgentPublishRequest {
  publishRequestId @0 :Text;
  payloadCbor @1 :Data;
}

struct MeshAgentPublishResponse {
  publishRequestId @0 :Text;
  status @1 :MeshAgentResponseStatus;
  transportReceipts @2 :List(MeshAgentTransportReceipt);
}

struct MeshAgentTransportReceipt {
  transportKind @0 :Text;
  endpointUri @1 :Text;
  deliveryStatus @2 :Text;
  redactedMessage @3 :Text;
}

struct MeshAgentError {
  code @0 :Text;
  message @1 :Text;
}

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
  transports @0 :List(MeshAgentTransportStatus);
}

struct MeshAgentTransportStatus {
  transport @0 :MeshAgentTransportKind;
  profileId @1 :Text;
  endpointUri @2 :Text;
  configured @3 :Bool;
  implementation @4 :MeshAgentImplementation;
  usableForDelivery @5 :Bool;
  message @6 :Text;
}

enum MeshAgentImplementation {
  real @0;
  mock @1;
  previewUnavailable @2;
}

enum MeshAgentTransportKind {
  reticulum @0;
}

struct MeshAgentPublishRequest {
  publishRequestId @0 :Text;
  payloadCbor @1 :Data;
  eventId @2 :Text;
  targetFingerprint @3 :Text;
}

struct MeshAgentPublishResponse {
  publishRequestId @0 :Text;
  status @1 :MeshAgentResponseStatus;
  transportReceipts @2 :List(MeshAgentTransportReceipt);
  eventId @3 :Text;
}

struct MeshAgentTransportReceipt {
  transportKind @0 :MeshAgentTransportKind;
  endpointUri @1 :Text;
  outcome @2 :MeshAgentTransportOutcome;
  message @3 :Text;
}

enum MeshAgentTransportOutcome {
  accepted @0;
  delivered @1;
  forwarded @2;
  storedByGateway @3;
  deferredUntilImplemented @4;
  rejected @5;
  routeUnavailable @6;
  timeout @7;
  transportUnavailable @8;
}

struct MeshAgentError {
  code @0 :Text;
  message @1 :Text;
}

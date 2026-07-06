@0xb83e0c4f71838d9a;

using Cxx = import "/capnp/c++.capnp";

$Cxx.namespace("radroots::mesh_agent::v1");

struct MeshAgentRequest {
  requestId @0 :Text;
  action @1 :MeshAgentAction;
  frameCbor @2 :Data;
}

enum MeshAgentAction {
  validateFrame @0;
  stageDelivery @1;
  observeEventHead @2;
}

struct MeshAgentResponse {
  requestId @0 :Text;
  status @1 :MeshAgentResponseStatus;
  receipt @2 :MeshAgentReceipt;
  errors @3 :List(MeshAgentError);
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

struct MeshAgentError {
  code @0 :Text;
  message @1 :Text;
}

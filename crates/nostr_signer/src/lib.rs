#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

pub mod backend;
pub mod capability;
pub mod error;
pub mod evaluation;
pub mod manager;
#[cfg(feature = "native")]
pub mod migrations;
pub mod model;
pub mod nip46;
#[cfg(feature = "native")]
pub mod sqlite;
pub mod store;

#[cfg(test)]
mod test_fixtures;
#[cfg(test)]
mod test_support;

pub mod prelude {
    pub use crate::backend::{
        RadrootsNostrEmbeddedSignerBackend, RadrootsNostrSignerBackend,
        RadrootsNostrSignerBackendCapabilities, RadrootsNostrSignerPublishTransition,
        RadrootsNostrSignerSignOutput,
    };
    pub use crate::capability::{
        RadrootsNostrLocalSignerAvailability, RadrootsNostrLocalSignerCapability,
        RadrootsNostrRemoteSessionSignerCapability, RadrootsNostrSignerCapability,
    };
    pub use crate::error::RadrootsNostrSignerError;
    pub use crate::evaluation::{
        RadrootsNostrSignerConnectEvaluation, RadrootsNostrSignerConnectProposal,
        RadrootsNostrSignerRequestAction, RadrootsNostrSignerRequestEvaluation,
        RadrootsNostrSignerRequestResponseHint, RadrootsNostrSignerSessionLookup,
    };
    pub use crate::manager::RadrootsNostrSignerManager;
    pub use crate::model::{
        RADROOTS_NOSTR_SIGNER_STORE_VERSION, RadrootsNostrSignerApprovalRequirement,
        RadrootsNostrSignerApprovalState, RadrootsNostrSignerAuthChallenge,
        RadrootsNostrSignerAuthState, RadrootsNostrSignerAuthorizationOutcome,
        RadrootsNostrSignerConnectSecretHash, RadrootsNostrSignerConnectionDraft,
        RadrootsNostrSignerConnectionId, RadrootsNostrSignerConnectionRecord,
        RadrootsNostrSignerConnectionStatus, RadrootsNostrSignerPendingRequest,
        RadrootsNostrSignerPermissionGrant, RadrootsNostrSignerPublishWorkflowKind,
        RadrootsNostrSignerPublishWorkflowRecord, RadrootsNostrSignerPublishWorkflowState,
        RadrootsNostrSignerRequestAuditRecord, RadrootsNostrSignerRequestDecision,
        RadrootsNostrSignerRequestId, RadrootsNostrSignerSecretDigestAlgorithm,
        RadrootsNostrSignerStoreState, RadrootsNostrSignerWorkflowId,
    };
    pub use crate::nip46::{
        RadrootsNostrSignerHandledRequest, RadrootsNostrSignerHandledRequestOutcome,
        RadrootsNostrSignerNip46Codec, RadrootsNostrSignerNip46ConnectDecision,
        RadrootsNostrSignerNip46Handler, RadrootsNostrSignerNip46Policy,
        RadrootsNostrSignerNip46Signer, connect_response_outcome, handled_request_for_action,
        response_from_hint,
    };
    #[cfg(feature = "native")]
    pub use crate::sqlite::RadrootsNostrSignerSqliteDb;
    #[cfg(feature = "native")]
    pub use crate::store::RadrootsNostrSqliteSignerStore;
    pub use crate::store::{
        RadrootsNostrFileSignerStore, RadrootsNostrMemorySignerStore, RadrootsNostrSignerStore,
    };
}

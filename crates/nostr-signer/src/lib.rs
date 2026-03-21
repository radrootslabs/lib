#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

pub mod error;
pub mod evaluation;
pub mod manager;
pub mod model;
pub mod store;

pub mod prelude {
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
        RadrootsNostrSignerPermissionGrant, RadrootsNostrSignerRequestAuditRecord,
        RadrootsNostrSignerRequestDecision, RadrootsNostrSignerRequestId,
        RadrootsNostrSignerSecretDigestAlgorithm, RadrootsNostrSignerStoreState,
    };
    pub use crate::store::{
        RadrootsNostrFileSignerStore, RadrootsNostrMemorySignerStore, RadrootsNostrSignerStore,
    };
}

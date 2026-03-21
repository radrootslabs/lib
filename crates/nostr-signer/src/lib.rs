#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

pub mod error;
pub mod manager;
pub mod model;
pub mod store;

pub mod prelude {
    pub use crate::error::RadrootsNostrSignerError;
    pub use crate::manager::RadrootsNostrSignerManager;
    pub use crate::model::{
        RADROOTS_NOSTR_SIGNER_STORE_VERSION, RadrootsNostrSignerApprovalRequirement,
        RadrootsNostrSignerApprovalState, RadrootsNostrSignerConnectionDraft,
        RadrootsNostrSignerConnectionId, RadrootsNostrSignerConnectionRecord,
        RadrootsNostrSignerConnectionStatus, RadrootsNostrSignerPermissionGrant,
        RadrootsNostrSignerRequestAuditRecord, RadrootsNostrSignerRequestDecision,
        RadrootsNostrSignerRequestId, RadrootsNostrSignerStoreState,
    };
    pub use crate::store::{
        RadrootsNostrFileSignerStore, RadrootsNostrMemorySignerStore, RadrootsNostrSignerStore,
    };
}

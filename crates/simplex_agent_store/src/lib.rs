#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod error;
pub mod store;

pub mod prelude {
    pub use crate::error::RadrootsSimplexAgentStoreError;
    #[cfg(feature = "std")]
    pub use crate::store::RadrootsSimplexAgentStoreProtectedSecretsDiagnostics;
    pub use crate::store::{
        RadrootsSimplexAgentConnectionRecord, RadrootsSimplexAgentDeliveryCursor,
        RadrootsSimplexAgentOutboundMessage, RadrootsSimplexAgentPendingCommand,
        RadrootsSimplexAgentPendingCommandKind, RadrootsSimplexAgentPqKeypair,
        RadrootsSimplexAgentPreparedOutboundMessage, RadrootsSimplexAgentQueueAuthState,
        RadrootsSimplexAgentQueueRecord, RadrootsSimplexAgentQueueRole,
        RadrootsSimplexAgentRecentMessageRecord, RadrootsSimplexAgentStore,
        RadrootsSimplexAgentX3dhKeypair,
    };
}

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod error;
pub mod store;

pub mod prelude {
    pub use crate::error::RadrootsSimplexAgentStoreError;
    pub use crate::store::{
        RadrootsSimplexAgentConnectionRecord, RadrootsSimplexAgentDeliveryCursor,
        RadrootsSimplexAgentPendingCommand, RadrootsSimplexAgentPendingCommandKind,
        RadrootsSimplexAgentQueueRecord, RadrootsSimplexAgentQueueRole,
        RadrootsSimplexAgentRecentMessageRecord, RadrootsSimplexAgentStore,
    };
}

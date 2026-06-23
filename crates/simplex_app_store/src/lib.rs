#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod error;
pub mod model;
#[cfg(all(feature = "std", feature = "sqlcipher"))]
pub mod store;

pub mod prelude {
    pub use crate::error::RadrootsSimplexAppStoreError;
    pub use crate::model::{
        RadrootsSimplexAppChatDirection, RadrootsSimplexAppChatItem, RadrootsSimplexAppConnection,
        RadrootsSimplexAppContact, RadrootsSimplexAppConversation, RadrootsSimplexAppDiagnostics,
        RadrootsSimplexAppInboundMessageLogEntry, RadrootsSimplexAppOutboxMessage,
        RadrootsSimplexAppProfile, RadrootsSimplexAppQueueEndpoint,
        RadrootsSimplexAppUnsupportedProtocolEvent,
    };
    #[cfg(all(feature = "std", feature = "sqlcipher"))]
    pub use crate::store::RadrootsSimplexAppStore;
}

pub use error::RadrootsSimplexAppStoreError;
pub use model::{
    RadrootsSimplexAppChatDirection, RadrootsSimplexAppChatItem, RadrootsSimplexAppConnection,
    RadrootsSimplexAppContact, RadrootsSimplexAppConversation, RadrootsSimplexAppDiagnostics,
    RadrootsSimplexAppInboundMessageLogEntry, RadrootsSimplexAppOutboxMessage,
    RadrootsSimplexAppProfile, RadrootsSimplexAppQueueEndpoint,
    RadrootsSimplexAppUnsupportedProtocolEvent,
};
#[cfg(all(feature = "std", feature = "sqlcipher"))]
pub use store::RadrootsSimplexAppStore;

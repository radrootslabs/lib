#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod error;
pub mod model;

pub mod prelude {
    pub use crate::error::RadrootsSimplexAppStoreError;
    pub use crate::model::{
        RadrootsSimplexAppChatDirection, RadrootsSimplexAppChatItem, RadrootsSimplexAppConnection,
        RadrootsSimplexAppContact, RadrootsSimplexAppConversation, RadrootsSimplexAppDiagnostics,
        RadrootsSimplexAppInboundChildEvent, RadrootsSimplexAppInboundCommit,
        RadrootsSimplexAppInboundMessageLogEntry, RadrootsSimplexAppInboundTextRequest,
        RadrootsSimplexAppInboundUnsupportedEventRequest, RadrootsSimplexAppOutboundTextDraft,
        RadrootsSimplexAppOutboundTextRequest, RadrootsSimplexAppOutboxMessage,
        RadrootsSimplexAppProfile, RadrootsSimplexAppQueueEndpoint,
        RadrootsSimplexAppUnsupportedProtocolEvent,
    };
}

pub use error::RadrootsSimplexAppStoreError;
pub use model::{
    RadrootsSimplexAppChatDirection, RadrootsSimplexAppChatItem, RadrootsSimplexAppConnection,
    RadrootsSimplexAppContact, RadrootsSimplexAppConversation, RadrootsSimplexAppDiagnostics,
    RadrootsSimplexAppInboundChildEvent, RadrootsSimplexAppInboundCommit,
    RadrootsSimplexAppInboundMessageLogEntry, RadrootsSimplexAppInboundTextRequest,
    RadrootsSimplexAppInboundUnsupportedEventRequest, RadrootsSimplexAppOutboundTextDraft,
    RadrootsSimplexAppOutboundTextRequest, RadrootsSimplexAppOutboxMessage,
    RadrootsSimplexAppProfile, RadrootsSimplexAppQueueEndpoint,
    RadrootsSimplexAppUnsupportedProtocolEvent,
};

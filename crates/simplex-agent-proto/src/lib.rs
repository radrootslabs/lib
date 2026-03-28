#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod codec;
pub mod error;
pub mod model;

pub mod prelude {
    pub use crate::codec::{
        decode_agent_message_frame, decode_connection_link, decode_decrypted_message,
        decode_envelope, encode_agent_message_frame, encode_connection_link,
        encode_decrypted_message, encode_envelope,
    };
    pub use crate::error::RadrootsSimplexAgentProtoError;
    pub use crate::model::{
        RADROOTS_SIMPLEX_AGENT_CURRENT_VERSION, RadrootsSimplexAgentConnectionLink,
        RadrootsSimplexAgentConnectionMode, RadrootsSimplexAgentConnectionStatus,
        RadrootsSimplexAgentDecryptedMessage, RadrootsSimplexAgentEncryptedPayload,
        RadrootsSimplexAgentEnvelope, RadrootsSimplexAgentMessage,
        RadrootsSimplexAgentMessageFrame, RadrootsSimplexAgentMessageHeader,
        RadrootsSimplexAgentMessageId, RadrootsSimplexAgentMessageReceipt,
        RadrootsSimplexAgentQueueAddress, RadrootsSimplexAgentQueueDescriptor,
        RadrootsSimplexAgentQueueUseDecision,
    };
    pub use radroots_simplex_smp_crypto::prelude::{
        RadrootsSimplexSmpRatchetHeader, RadrootsSimplexSmpRatchetState,
    };
}

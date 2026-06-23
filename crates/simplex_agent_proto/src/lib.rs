#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod codec;
pub mod error;
pub mod model;
pub mod short_link;

pub mod prelude {
    pub use crate::codec::{
        decode_agent_message_frame, decode_connection_link, decode_decrypted_message,
        decode_envelope, encode_agent_message_frame, encode_connection_link,
        encode_decrypted_message, encode_envelope,
    };
    pub use crate::error::{
        RadrootsSimplexAgentProtoError, RadrootsSimplexAgentUnsupportedLinkKind,
    };
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
    pub use crate::short_link::{
        RADROOTS_SIMPLEX_AGENT_SHORT_LINK_ID_LENGTH, RADROOTS_SIMPLEX_AGENT_SHORT_LINK_KEY_LENGTH,
        RADROOTS_SIMPLEX_AGENT_SHORT_LINK_SERVER_KEY_HASH_LENGTH,
        RadrootsSimplexAgentShortInvitationLink, RadrootsSimplexAgentShortLinkScheme,
        parse_short_invitation_link,
    };
    pub use radroots_simplex_smp_crypto::prelude::{
        RadrootsSimplexOfficialX3dhParams, RadrootsSimplexSmpRatchetHeader,
        RadrootsSimplexSmpRatchetState, decode_official_x3dh_params_uri,
        encode_official_x3dh_params_uri, official_x448_keypair_from_seed,
    };
}

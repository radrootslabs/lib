#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod codec;
pub mod error;
pub mod model;
pub mod version;

pub mod prelude {
    pub use crate::codec::{
        RADROOTS_SIMPLEX_CHAT_COMPRESSION_LEVEL, RADROOTS_SIMPLEX_CHAT_MAX_COMPRESSED_LENGTH,
        RADROOTS_SIMPLEX_CHAT_MAX_DECOMPRESSED_LENGTH,
        RADROOTS_SIMPLEX_CHAT_MAX_PASSTHROUGH_LENGTH, decode_messages, encode_batch,
        encode_compressed_batch, encode_message,
    };
    pub use crate::error::RadrootsSimplexChatProtoError;
    pub use crate::model::{
        RadrootsSimplexChatBase64Url, RadrootsSimplexChatContactEvent,
        RadrootsSimplexChatContainerKind, RadrootsSimplexChatContent,
        RadrootsSimplexChatDeleteEvent, RadrootsSimplexChatEvent,
        RadrootsSimplexChatFileAcceptEvent, RadrootsSimplexChatFileAcceptInvitationEvent,
        RadrootsSimplexChatFileCancelEvent, RadrootsSimplexChatFileDescription,
        RadrootsSimplexChatFileDescriptionEvent, RadrootsSimplexChatFileInvitation,
        RadrootsSimplexChatForwardMarker, RadrootsSimplexChatInfoEvent,
        RadrootsSimplexChatLinkContent, RadrootsSimplexChatLinkPreview, RadrootsSimplexChatMention,
        RadrootsSimplexChatMessage, RadrootsSimplexChatMessageContainer,
        RadrootsSimplexChatMessageContentReference, RadrootsSimplexChatMessageRef,
        RadrootsSimplexChatMsgNewEvent, RadrootsSimplexChatMsgUpdateEvent,
        RadrootsSimplexChatNoParamsEvent, RadrootsSimplexChatObject, RadrootsSimplexChatPeerType,
        RadrootsSimplexChatProbeCheckEvent, RadrootsSimplexChatProbeEvent,
        RadrootsSimplexChatProfile, RadrootsSimplexChatQuotedMessage, RadrootsSimplexChatScope,
    };
    pub use crate::version::{
        RADROOTS_SIMPLEX_CHAT_COMPRESSION_VERSION, RADROOTS_SIMPLEX_CHAT_CURRENT_VERSION,
        RADROOTS_SIMPLEX_CHAT_INITIAL_VERSION, RadrootsSimplexChatVersionRange,
    };
}

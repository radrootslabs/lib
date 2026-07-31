#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
#[cfg(not(feature = "std"))]
extern crate core as std;

#[cfg(feature = "blossom")]
pub mod blossom;

pub mod error;
pub mod event;
pub mod events;
pub mod filter;
pub mod key;
pub mod tag;
mod tags;
pub mod types;
pub mod util;

pub use error::RadrootsNostrError as Error;

#[cfg(test)]
mod test_fixtures;

#[cfg(feature = "codec")]
pub mod codec_adapters;

#[cfg(feature = "codec")]
pub mod job_adapter;

#[cfg(feature = "nip17")]
pub mod nip17;

#[cfg(feature = "signing")]
pub mod signing;

#[cfg(feature = "events")]
pub mod event_adapters;

#[cfg(feature = "events")]
pub mod draft_signing;
#[cfg(feature = "events")]
mod event_convert;
#[cfg(feature = "events")]
pub mod event_verify;

pub mod prelude {

    #[cfg(feature = "blossom")]
    pub use crate::blossom::{
        RadrootsNostrBlossomAuthorizationHeader, RadrootsNostrBlossomError,
        RadrootsNostrSignedBlossomAuthorization, RadrootsNostrVerifiedBlossomAuthorization,
        radroots_nostr_decode_verify_blossom_authorization_header,
        radroots_nostr_encode_blossom_authorization_header,
        radroots_nostr_sign_blossom_authorization,
    };

    pub use crate::error::{RadrootsNostrError, RadrootsNostrTagsResolveError};
    #[cfg(feature = "std")]
    pub use crate::filter::radroots_nostr_filter_new_events;
    pub use crate::filter::{
        radroots_nostr_filter_kind, radroots_nostr_filter_tag, radroots_nostr_kind,
    };

    pub use crate::events::{
        jobs::{radroots_nostr_build_event_job_feedback, radroots_nostr_build_event_job_result},
        post::radroots_nostr_post_events_filter,
    };

    #[cfg(feature = "events")]
    pub use crate::events::comment::{
        RadrootsNostrNip22CommentEventBuilder, radroots_nostr_build_nip22_comment_event,
    };

    #[cfg(feature = "events")]
    pub use crate::events::deletion::{
        RadrootsNostrNip09DeletionRequestEventBuilder,
        radroots_nostr_build_nip09_deletion_request_event,
    };

    #[cfg(feature = "events")]
    pub use crate::events::food_availability::{
        RadrootsNostrFoodAvailabilityEventBuilder, radroots_nostr_build_food_availability_event,
    };

    #[cfg(feature = "events")]
    pub use crate::events::metadata::{
        RadrootsNostrProfileEventBuilder, radroots_nostr_build_profile_event,
    };

    #[cfg(feature = "events")]
    pub use crate::events::post::{
        RadrootsNostrPostEventBuilder, radroots_nostr_build_ask_event,
        radroots_nostr_build_photo_update_event, radroots_nostr_build_update_event,
    };

    #[cfg(feature = "events")]
    pub use crate::events::reply::{
        RadrootsNostrNip10ReplyEventBuilder, radroots_nostr_build_nip10_reply_event,
    };

    #[cfg(feature = "events")]
    pub use crate::events::application_handler::{
        RadrootsNostrApplicationHandlerSpec, radroots_nostr_build_application_handler_event,
        radroots_nostr_metadata_has_fields,
    };

    pub use crate::event::{
        Coordinate as RadrootsNostrCoordinate, Event as RadrootsNostrEvent,
        EventId as RadrootsNostrEventId, Kind as RadrootsNostrKind,
        Metadata as RadrootsNostrMetadata, Timestamp as RadrootsNostrTimestamp,
    };
    pub use crate::filter::Filter as RadrootsNostrFilter;
    #[cfg(feature = "nip17")]
    pub use crate::nip17::{
        RadrootsNip17Error, RadrootsNip17Rumor, RadrootsNip17WrapOptions,
        radroots_nostr_unwrap_gift_wrap, radroots_nostr_wrap_message,
        radroots_nostr_wrap_message_file,
    };
    pub use crate::tag::{
        Tag as RadrootsNostrTag, TagKind as RadrootsNostrTagKind,
        TagStandard as RadrootsNostrTagStandard,
    };
    pub use crate::tag::{
        radroots_nostr_tag_at_value, radroots_nostr_tag_first_value,
        radroots_nostr_tag_match_geohash, radroots_nostr_tag_match_l,
        radroots_nostr_tag_match_location, radroots_nostr_tag_match_summary,
        radroots_nostr_tag_match_title, radroots_nostr_tag_relays_parse, radroots_nostr_tag_slice,
        radroots_nostr_tags_match, radroots_nostr_tags_resolve,
    };
    pub use crate::types::{
        RadrootsNostrExternalSigningRequest, RadrootsNostrFromBech32,
        RadrootsNostrGenericEventBuilder, RadrootsNostrKeys, RadrootsNostrPublicKey,
        RadrootsNostrRelayUrl, RadrootsNostrSecp256k1SecretKey, RadrootsNostrSecretKey,
        RadrootsNostrSubscriptionId, RadrootsNostrToBech32, RadrootsNostrUrl,
    };

    #[cfg(feature = "events")]
    pub use crate::event_adapters::{to_post_event_metadata, to_profile_event_metadata};

    #[cfg(feature = "events")]
    pub use crate::draft_signing::radroots_nostr_sign_frozen_draft;

    #[cfg(feature = "events")]
    pub use crate::event::{
        nostr_event_from_radroots, radroots_event_from_nostr, radroots_event_ptr_from_nostr,
    };

    #[cfg(feature = "events")]
    pub use crate::event_verify::{
        NostrSignatureVerifier, RadrootsNostrEventVerification, radroots_nostr_verify_event,
        radroots_nostr_verify_event_id,
    };

    #[cfg(feature = "codec")]
    pub use crate::job_adapter::RadrootsNostrEventAdapter;
}

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "client")]
pub mod client;

pub mod error;
pub mod events;
pub mod filter;
pub mod parse;
#[cfg(feature = "client")]
pub mod relays;
pub mod types;
pub mod tags;
pub mod util;

#[cfg(feature = "codec")]
pub mod codec_adapters;

#[cfg(feature = "codec")]
pub mod job_adapter;

#[cfg(feature = "nip17")]
pub mod nip17;

#[cfg(feature = "http")]
pub mod nip11;

#[cfg(feature = "events")]
pub mod event_adapters;

#[cfg(feature = "events")]
pub mod event_convert;

pub mod prelude {
    pub use crate::events::radroots_nostr_build_event;

    #[cfg(feature = "client")]
    pub use crate::client::{
        radroots_nostr_fetch_event_by_id,
        radroots_nostr_send_event,
        RadrootsNostrClient,
    };

    pub use crate::error::{RadrootsNostrError, RadrootsNostrTagsResolveError};
    pub use crate::filter::{
        radroots_nostr_filter_kind,
        radroots_nostr_filter_new_events,
        radroots_nostr_kind,
    };

    pub use crate::events::{
        jobs::{
            radroots_nostr_build_event_job_feedback,
            radroots_nostr_build_event_job_result,
        },
        metadata::radroots_nostr_build_metadata_event,
        post::{
            radroots_nostr_build_post_event,
            radroots_nostr_build_post_reply_event,
            radroots_nostr_post_events_filter,
        },
    };

    #[cfg(feature = "client")]
    pub use crate::events::metadata::{
        radroots_nostr_fetch_metadata_for_author,
        radroots_nostr_post_metadata_event,
    };

    #[cfg(all(feature = "client", feature = "events"))]
    pub use crate::events::post::radroots_nostr_fetch_post_events;

    pub use crate::parse::{radroots_nostr_parse_pubkey, radroots_nostr_parse_pubkeys};
    #[cfg(feature = "client")]
    pub use crate::relays::{
        radroots_nostr_add_relay,
        radroots_nostr_connect,
        radroots_nostr_remove_relay,
    };
    pub use crate::tags::*;
    pub use crate::types::{
        RadrootsNostrCoordinate,
        RadrootsNostrEvent,
        RadrootsNostrEventBuilder,
        RadrootsNostrEventId,
        RadrootsNostrFilter,
        RadrootsNostrFromBech32,
        RadrootsNostrKind,
        RadrootsNostrKeys,
        RadrootsNostrMetadata,
        RadrootsNostrPublicKey,
        RadrootsNostrRelayUrl,
        RadrootsNostrSecretKey,
        RadrootsNostrSecp256k1SecretKey,
        RadrootsNostrSubscriptionId,
        RadrootsNostrTag,
        RadrootsNostrTagKind,
        RadrootsNostrTagStandard,
        RadrootsNostrTimestamp,
        RadrootsNostrToBech32,
        RadrootsNostrUrl,
    };
    #[cfg(feature = "client")]
    pub use crate::types::{
        RadrootsNostrMonitor,
        RadrootsNostrMonitorNotification,
        RadrootsNostrOutput,
        RadrootsNostrRelay,
        RadrootsNostrRelayPoolNotification,
        RadrootsNostrRelayStatus,
        RadrootsNostrSubscribeAutoCloseOptions,
    };
    pub use crate::util::radroots_nostr_npub_string;

    #[cfg(feature = "nip17")]
    pub use crate::nip17::{
        radroots_nostr_unwrap_gift_wrap,
        radroots_nostr_wrap_message,
        radroots_nostr_wrap_message_file,
        RadrootsNip17Error,
        RadrootsNip17Rumor,
        RadrootsNip17WrapOptions,
    };

    #[cfg(feature = "http")]
    pub use crate::nip11::fetch_nip11;

    #[cfg(feature = "events")]
    pub use crate::event_adapters::{to_post_event_metadata, to_profile_event_metadata};

    #[cfg(feature = "events")]
    pub use crate::event_convert::{radroots_event_from_nostr, radroots_event_ptr_from_nostr};

    #[cfg(feature = "codec")]
    pub use crate::job_adapter::RadrootsNostrEventAdapter;
}

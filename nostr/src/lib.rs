#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "sdk")]
pub mod client;

pub mod error;
pub mod events;
pub mod filter;
pub mod parse;
pub mod relays;
pub mod tags;
pub mod util;

#[cfg(feature = "codec")]
pub mod codec_adapters;

#[cfg(feature = "http")]
pub mod nip11;

#[cfg(feature = "events")]
pub mod event_adapters;

pub mod prelude {
    pub use crate::events::build_nostr_event;

    #[cfg(feature = "sdk")]
    pub use crate::client::{nostr_fetch_event_by_id, nostr_send_event};

    pub use crate::error::{NostrTagsResolveError, NostrUtilsError};
    pub use crate::filter::{nostr_filter_kind, nostr_filter_new_events, nostr_kind};

    pub use crate::events::{
        jobs::{nostr_build_event_job_feedback, nostr_build_event_job_result},
        metadata::{build_metadata_event, fetch_metadata_for_author, post_metadata_event},
        post::{build_post_event, build_post_reply_event},
    };

    #[cfg(all(feature = "sdk", feature = "events"))]
    pub use crate::events::post::fetch_post_events;

    pub use crate::parse::{parse_pubkey, parse_pubkeys};
    pub use crate::relays::{add_relay, connect, remove_relay};
    pub use crate::tags::*;
    pub use crate::util::npub_string;

    #[cfg(feature = "http")]
    pub use crate::nip11::fetch_nip11;

    #[cfg(feature = "events")]
    pub use crate::event_adapters::{to_post_event_metadata, to_profile_event_metadata};
}

#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod auth;
mod client;
mod cursor;
mod error;
mod profile;
mod relay;
mod sink;
mod source;
mod status;

pub use client::{Config, NostrTransport, ReconnectBackoff};
pub use cursor::RelayCursor;
pub use error::Error;
pub use profile::{
    DEFAULT_PUBLIC_RELAY, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind,
};
pub use relay::{RelayUrl, RelayUrlPolicy};
pub use status::{
    RelayAggregateState, RelayCapabilityEvidence, RelayEvidenceState, RelayStatus,
    RelayStatusReport,
};

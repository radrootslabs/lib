#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod auth;
mod client;
mod connections;
mod cursor;
mod error;
mod exact_delivery;
mod host_time;
mod profile;
mod relay;
mod sink;
mod socket_write;
mod source;
mod source_ingress;
mod source_wire;
mod status;
mod subscription;

pub use client::{Config, NostrTransport, ReconnectBackoff};
pub use cursor::RelayCursor;
pub use error::Error;
pub use profile::{RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind};
pub use relay::{RelayUrl, RelayUrlPolicy};
pub use sink::PreparedDelivery;
pub use status::{
    RelayAggregateState, RelayCapabilityEvidence, RelayEvidenceState, RelayStatus,
    RelayStatusReport,
};

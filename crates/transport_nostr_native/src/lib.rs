#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod connector;
mod socket;
mod wire;

pub use connector::NativeConnector;

/// Composes the shared Nostr session with the native socket provider.
/// Construction opens no socket and creates no runtime or worker.
pub fn create_transport(
    config: radroots_transport_nostr::Config,
) -> radroots_transport_nostr::NostrTransport {
    radroots_transport_nostr::NostrTransport::new(config, std::sync::Arc::new(NativeConnector))
}

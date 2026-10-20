#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg(target_arch = "wasm32")]
mod browser;
mod connector;
#[cfg(any(target_arch = "wasm32", test))]
mod state;

pub use connector::BrowserConnector;

/// Composes the shared Nostr session with origin-thread browser socket I/O.
/// Construction performs no I/O and spawns no task.
pub fn create_transport(
    config: radroots_transport_nostr::Config,
) -> radroots_transport_nostr::NostrTransport {
    radroots_transport_nostr::NostrTransport::new(config, std::sync::Arc::new(BrowserConnector))
}

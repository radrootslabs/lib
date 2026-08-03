//! Curated client construction and operation entry points.

#[cfg(any(feature = "nostr", feature = "full"))]
use std::sync::Arc;

#[cfg(any(feature = "nostr", feature = "full"))]
use radroots_transport::{EventSink, EventSource};

#[cfg(any(
    feature = "client",
    feature = "native",
    feature = "nostr",
    feature = "nip46",
    feature = "full"
))]
use crate::ClientBuilder;
use crate::transport::Profile;

/// Creates the safe ordinary client builder with deterministic in-process
/// storage and no transport, signer, runtime, worker, or file authority.
#[cfg(feature = "client")]
#[must_use]
pub fn memory() -> ClientBuilder {
    ClientBuilder::memory_default()
}

/// Returns the explicit no-transport profile used by ordinary local work.
#[must_use]
pub const fn local_only() -> Profile {
    Profile::local_only()
}

/// Adds caller-owned source and sink capabilities without connecting them or
/// selecting a fallback transport.
#[cfg(any(feature = "nostr", feature = "full"))]
#[must_use]
pub fn with_transport(
    builder: ClientBuilder,
    source: Arc<dyn EventSource>,
    sink: Arc<dyn EventSink>,
) -> ClientBuilder {
    builder.source(source).sink(sink)
}

/// Adds an explicitly constructed NIP-46 signer provider.
#[cfg(any(feature = "nip46", feature = "full"))]
#[must_use]
pub fn with_nip46_signer(
    builder: ClientBuilder,
    provider: crate::signing::Provider,
) -> ClientBuilder {
    builder.signing(provider)
}

/// Explicitly opens validated native SQLite storage.
#[cfg(any(feature = "native", feature = "full"))]
pub async fn native(options: crate::storage::SqliteOptions) -> crate::Result<ClientBuilder> {
    ClientBuilder::sqlite(options).await
}

/// Reports whether the concrete GeoNames capability was selected at compile
/// time. Asset inspection, acquisition, and database opening remain explicit.
#[cfg(any(feature = "geonames", feature = "full"))]
#[must_use]
pub const fn geonames_enabled() -> bool {
    true
}

/// Reports whether canonical knowledge contracts were selected at compile
/// time. Merely selecting the feature performs no event or storage operation.
#[cfg(any(feature = "knowledge", feature = "full"))]
#[must_use]
pub const fn knowledge_enabled() -> bool {
    true
}

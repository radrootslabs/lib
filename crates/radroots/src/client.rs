//! Curated client construction and operation entry points.

#[cfg(feature = "nostr")]
use std::sync::Arc;

#[cfg(feature = "nostr")]
use radroots_transport::{EventSink, EventSource};

use crate::{ClientBuilder, transport::Profile};

/// Creates the safe ordinary client builder with deterministic in-process
/// storage and no transport, signer, runtime, worker, or file authority.
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
#[cfg(feature = "nostr")]
#[must_use]
pub fn with_transport(
    builder: ClientBuilder,
    source: Arc<dyn EventSource>,
    sink: Arc<dyn EventSink>,
) -> ClientBuilder {
    builder.source(source).sink(sink)
}

/// Adds an explicitly constructed NIP-46 signer provider.
#[cfg(feature = "nip46")]
#[must_use]
pub fn with_nip46_signer(
    builder: ClientBuilder,
    provider: crate::signing::Provider,
) -> ClientBuilder {
    builder.signing(provider)
}

/// Explicitly opens validated native SQLite storage.
#[cfg(feature = "native")]
pub async fn native(options: crate::storage::SqliteOptions) -> crate::Result<ClientBuilder> {
    ClientBuilder::sqlite(options).await
}

/// Reports whether the concrete GeoNames capability was selected at compile
/// time. Asset inspection, acquisition, and database opening remain explicit.
#[cfg(feature = "geonames")]
#[must_use]
pub const fn geonames_enabled() -> bool {
    true
}

/// Reports whether canonical knowledge contracts were selected at compile
/// time. Merely selecting the feature performs no event or storage operation.
#[cfg(feature = "knowledge")]
#[must_use]
pub const fn knowledge_enabled() -> bool {
    true
}

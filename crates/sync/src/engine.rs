use std::sync::Arc;

use radroots_signing::Signer;
use radroots_storage::Storage;
use radroots_transport::{EventSink, EventSource};

use crate::policy::{Clock, DeadlinePolicy, EngineBuilder, IdSource};

/// Injected, executor-neutral synchronization composition boundary.
#[derive(Clone)]
pub struct Engine {
    pub(crate) storage: Arc<dyn Storage>,
    pub(crate) source: Option<Arc<dyn EventSource>>,
    pub(crate) sink: Option<Arc<dyn EventSink>>,
    pub(crate) signer: Option<Arc<dyn Signer>>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) ids: Arc<dyn IdSource>,
    pub(crate) deadlines: DeadlinePolicy,
}

impl Engine {
    /// Starts an explicit capability builder around required host policies.
    pub fn builder(
        storage: Arc<dyn Storage>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdSource>,
        deadlines: DeadlinePolicy,
    ) -> EngineBuilder {
        EngineBuilder::new(storage, clock, ids, deadlines)
    }

    /// Returns the canonical storage capability.
    pub fn storage(&self) -> &dyn Storage {
        self.storage.as_ref()
    }

    /// Returns the configured event source, when pull is enabled.
    pub fn source(&self) -> Option<&dyn EventSource> {
        self.source.as_deref()
    }

    /// Returns the configured event sink, when delivery is enabled.
    pub fn sink(&self) -> Option<&dyn EventSink> {
        self.sink.as_deref()
    }

    /// Returns the configured signer, when outbound authoring is enabled.
    pub fn signer(&self) -> Option<&dyn Signer> {
        self.signer.as_deref()
    }

    /// Returns the injected clock policy.
    pub fn clock(&self) -> &dyn Clock {
        self.clock.as_ref()
    }

    /// Returns the injected operation identity source.
    pub fn ids(&self) -> &dyn IdSource {
        self.ids.as_ref()
    }

    /// Returns the bounded deadline policy.
    pub const fn deadlines(&self) -> DeadlinePolicy {
        self.deadlines
    }
}

impl core::fmt::Debug for Engine {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Engine")
            .field("source", &self.source.is_some())
            .field("sink", &self.sink.is_some())
            .field("signer", &self.signer.is_some())
            .field("deadlines", &self.deadlines)
            .finish_non_exhaustive()
    }
}

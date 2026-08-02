#[path = "conformance/suite.rs"]
mod suite;

use radroots_storage::{
    AtomicStorage, EventStore, Journal, Outbox, PrivateArtifactStore, ProjectionStore,
    StorageReliability, memory::MemoryStorage,
};

use suite::StorageConformanceHarness;

#[derive(Default)]
struct MemoryHarness {
    storage: MemoryStorage,
}

impl StorageConformanceHarness for MemoryHarness {
    fn event_store(&self) -> &dyn EventStore {
        &self.storage
    }

    fn journal(&self) -> &dyn Journal {
        &self.storage
    }

    fn outbox(&self) -> &dyn Outbox {
        &self.storage
    }

    fn projection_store(&self) -> &dyn ProjectionStore {
        &self.storage
    }

    fn private_artifact_store(&self) -> &dyn PrivateArtifactStore {
        &self.storage
    }

    fn atomic_storage(&self) -> &dyn AtomicStorage {
        &self.storage
    }

    fn reliability(&self) -> &dyn StorageReliability {
        &self.storage
    }
}

#[test]
fn memory_backend_shares_state_across_every_storage_spi() {
    suite::assert_shared_state_conformance(&MemoryHarness::default());
}

#[test]
fn memory_backend_atomic_failure_is_all_or_nothing() {
    suite::assert_atomic_failure_isolation(&MemoryHarness::default());
}

#[test]
fn memory_backend_rejects_identity_and_digest_conflicts() {
    suite::assert_conflict_conformance(&MemoryHarness::default());
}

#[test]
fn memory_backend_supports_every_atomic_workflow() {
    suite::assert_atomic_workflow_conformance(&MemoryHarness::default());
}

#[test]
fn memory_backend_supports_reliability_and_explicit_close() {
    suite::assert_reliability_and_close_conformance(&MemoryHarness::default());
}

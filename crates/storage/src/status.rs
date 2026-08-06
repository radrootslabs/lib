//! Storage capability, health, and integrity status contracts.

use radroots_transport::BoxFuture;

use crate::{Error, event::SourceGeneration};

/// Passive backend-level status capability independent of backup workflows.
pub trait StorageStatusProvider: Send + Sync {
    fn storage_status(&self) -> BoxFuture<'_, Result<StorageStatus, Error>>;
}

/// Storage-engine family needed to interpret durability-specific status.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageBackend {
    Memory,
    Sqlite,
}

impl StorageBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Sqlite => "sqlite",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageOpenMode {
    ReadOnly,
    ReadWriteExisting,
    Create,
}

impl StorageOpenMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::ReadWriteExisting => "read_write_existing",
            Self::Create => "create",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriterPolicy {
    NoWriter,
    AdvisoryProcessLock,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownState {
    Open,
    Closing,
    Closed,
}

impl ShutdownState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closing => "closing",
            Self::Closed => "closed",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityHealth {
    Healthy,
    Degraded,
    Corrupt,
    Unknown,
}

impl IntegrityHealth {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Corrupt => "corrupt",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegrityStatus {
    health: IntegrityHealth,
    checked_at_unix_ms: Option<u64>,
    verified_members: u32,
    failed_members: u32,
}

impl IntegrityStatus {
    pub fn new(
        health: IntegrityHealth,
        checked_at_unix_ms: Option<u64>,
        verified_members: u32,
        failed_members: u32,
    ) -> Result<Self, Error> {
        if matches!(checked_at_unix_ms, Some(0))
            || (health == IntegrityHealth::Healthy && failed_members != 0)
            || (health == IntegrityHealth::Corrupt && failed_members == 0)
            || (health == IntegrityHealth::Unknown && checked_at_unix_ms.is_some())
        {
            return Err(Error::InvalidIntegrityStatus);
        }
        Ok(Self {
            health,
            checked_at_unix_ms,
            verified_members,
            failed_members,
        })
    }
    pub const fn health(self) -> IntegrityHealth {
        self.health
    }
    pub const fn checked_at_unix_ms(self) -> Option<u64> {
        self.checked_at_unix_ms
    }
    pub const fn verified_members(self) -> u32 {
        self.verified_members
    }
    pub const fn failed_members(self) -> u32 {
        self.failed_members
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageStatus {
    backend: StorageBackend,
    open_mode: StorageOpenMode,
    writer_policy: WriterPolicy,
    shutdown: ShutdownState,
    integrity: IntegrityStatus,
    wal_enabled: bool,
    busy_timeout_ms: u32,
}

impl StorageStatus {
    pub fn new(
        backend: StorageBackend,
        open_mode: StorageOpenMode,
        writer_policy: WriterPolicy,
        shutdown: ShutdownState,
        integrity: IntegrityStatus,
        wal_enabled: bool,
        busy_timeout_ms: u32,
    ) -> Result<Self, Error> {
        let valid_engine_status = match backend {
            StorageBackend::Memory => {
                writer_policy == WriterPolicy::NoWriter && !wal_enabled && busy_timeout_ms == 0
            }
            StorageBackend::Sqlite => {
                (open_mode == StorageOpenMode::ReadOnly && writer_policy == WriterPolicy::NoWriter)
                    || (open_mode != StorageOpenMode::ReadOnly
                        && writer_policy == WriterPolicy::AdvisoryProcessLock
                        && wal_enabled
                        && busy_timeout_ms != 0)
            }
        };
        if !valid_engine_status {
            return Err(Error::InvalidStorageStatus);
        }
        Ok(Self {
            backend,
            open_mode,
            writer_policy,
            shutdown,
            integrity,
            wal_enabled,
            busy_timeout_ms,
        })
    }
    pub const fn backend(self) -> StorageBackend {
        self.backend
    }
    pub const fn open_mode(self) -> StorageOpenMode {
        self.open_mode
    }
    pub const fn writer_policy(self) -> WriterPolicy {
        self.writer_policy
    }
    pub const fn shutdown(self) -> ShutdownState {
        self.shutdown
    }
    pub const fn integrity(self) -> IntegrityStatus {
        self.integrity
    }
    pub const fn wal_enabled(self) -> bool {
        self.wal_enabled
    }
    pub const fn busy_timeout_ms(self) -> u32 {
        self.busy_timeout_ms
    }
}

/// Current event-store operating mode.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventStoreMode {
    ReadOnly,
    ReadWrite,
}

/// Current health of the canonical event source.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventStoreHealth {
    Available,
    Degraded,
    Unavailable,
}

/// Passive event-store capability and cardinality report.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventStoreStatus {
    generation: SourceGeneration,
    mode: EventStoreMode,
    health: EventStoreHealth,
    raw_events: u64,
    verified_events: u64,
    visible_events: u64,
}

impl EventStoreStatus {
    /// Creates a consistent event-store status report.
    pub const fn new(
        generation: SourceGeneration,
        mode: EventStoreMode,
        health: EventStoreHealth,
        raw_events: u64,
        verified_events: u64,
        visible_events: u64,
    ) -> Result<Self, Error> {
        if verified_events > raw_events || visible_events > verified_events {
            return Err(Error::CorruptStoredEvent);
        }
        Ok(Self {
            generation,
            mode,
            health,
            raw_events,
            verified_events,
            visible_events,
        })
    }

    pub const fn generation(&self) -> SourceGeneration {
        self.generation
    }

    pub const fn mode(&self) -> EventStoreMode {
        self.mode
    }

    pub const fn health(&self) -> EventStoreHealth {
        self.health
    }

    pub const fn raw_events(&self) -> u64 {
        self.raw_events
    }

    pub const fn verified_events(&self) -> u64 {
        self.verified_events
    }

    pub const fn visible_events(&self) -> u64 {
        self.visible_events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_status_labels_are_explicit_and_stable() {
        assert_eq!(StorageBackend::Memory.as_str(), "memory");
        assert_eq!(StorageBackend::Sqlite.as_str(), "sqlite");
        assert_eq!(StorageOpenMode::ReadOnly.as_str(), "read_only");
        assert_eq!(
            StorageOpenMode::ReadWriteExisting.as_str(),
            "read_write_existing"
        );
        assert_eq!(StorageOpenMode::Create.as_str(), "create");
        assert_eq!(ShutdownState::Open.as_str(), "open");
        assert_eq!(ShutdownState::Closing.as_str(), "closing");
        assert_eq!(ShutdownState::Closed.as_str(), "closed");
        assert_eq!(IntegrityHealth::Healthy.as_str(), "healthy");
        assert_eq!(IntegrityHealth::Degraded.as_str(), "degraded");
        assert_eq!(IntegrityHealth::Corrupt.as_str(), "corrupt");
        assert_eq!(IntegrityHealth::Unknown.as_str(), "unknown");
    }

    fn integrity() -> IntegrityStatus {
        IntegrityStatus::new(IntegrityHealth::Healthy, Some(1), 3, 0).unwrap()
    }

    #[test]
    fn integrity_status_covers_every_invariant_and_accessor() {
        assert_eq!(
            IntegrityStatus::new(IntegrityHealth::Healthy, Some(0), 0, 0),
            Err(Error::InvalidIntegrityStatus)
        );
        assert_eq!(
            IntegrityStatus::new(IntegrityHealth::Healthy, Some(1), 0, 1),
            Err(Error::InvalidIntegrityStatus)
        );
        assert_eq!(
            IntegrityStatus::new(IntegrityHealth::Corrupt, Some(1), 1, 0),
            Err(Error::InvalidIntegrityStatus)
        );
        assert_eq!(
            IntegrityStatus::new(IntegrityHealth::Unknown, Some(1), 0, 0),
            Err(Error::InvalidIntegrityStatus)
        );

        let status = integrity();
        assert_eq!(status.health(), IntegrityHealth::Healthy);
        assert_eq!(status.checked_at_unix_ms(), Some(1));
        assert_eq!(status.verified_members(), 3);
        assert_eq!(status.failed_members(), 0);
        assert!(IntegrityStatus::new(IntegrityHealth::Degraded, None, 0, 1).is_ok());
        assert!(IntegrityStatus::new(IntegrityHealth::Corrupt, None, 0, 1).is_ok());
        assert!(IntegrityStatus::new(IntegrityHealth::Unknown, None, 0, 0).is_ok());
    }

    #[test]
    fn storage_status_covers_memory_and_sqlite_policy_matrix() {
        let memory = StorageStatus::new(
            StorageBackend::Memory,
            StorageOpenMode::Create,
            WriterPolicy::NoWriter,
            ShutdownState::Open,
            integrity(),
            false,
            0,
        )
        .unwrap();
        assert_eq!(memory.backend(), StorageBackend::Memory);
        assert_eq!(memory.open_mode(), StorageOpenMode::Create);
        assert_eq!(memory.writer_policy(), WriterPolicy::NoWriter);
        assert_eq!(memory.shutdown(), ShutdownState::Open);
        assert_eq!(memory.integrity(), integrity());
        assert!(!memory.wal_enabled());
        assert_eq!(memory.busy_timeout_ms(), 0);

        for (writer, wal, timeout) in [
            (WriterPolicy::AdvisoryProcessLock, false, 0),
            (WriterPolicy::NoWriter, true, 0),
            (WriterPolicy::NoWriter, false, 1),
        ] {
            assert_eq!(
                StorageStatus::new(
                    StorageBackend::Memory,
                    StorageOpenMode::ReadOnly,
                    writer,
                    ShutdownState::Closed,
                    integrity(),
                    wal,
                    timeout,
                ),
                Err(Error::InvalidStorageStatus)
            );
        }

        assert!(
            StorageStatus::new(
                StorageBackend::Sqlite,
                StorageOpenMode::ReadOnly,
                WriterPolicy::NoWriter,
                ShutdownState::Closing,
                integrity(),
                false,
                0,
            )
            .is_ok()
        );
        assert!(
            StorageStatus::new(
                StorageBackend::Sqlite,
                StorageOpenMode::ReadWriteExisting,
                WriterPolicy::AdvisoryProcessLock,
                ShutdownState::Open,
                integrity(),
                true,
                1,
            )
            .is_ok()
        );
        for (mode, writer, wal, timeout) in [
            (
                StorageOpenMode::ReadOnly,
                WriterPolicy::AdvisoryProcessLock,
                false,
                0,
            ),
            (StorageOpenMode::Create, WriterPolicy::NoWriter, true, 1),
            (
                StorageOpenMode::Create,
                WriterPolicy::AdvisoryProcessLock,
                false,
                1,
            ),
            (
                StorageOpenMode::Create,
                WriterPolicy::AdvisoryProcessLock,
                true,
                0,
            ),
        ] {
            assert_eq!(
                StorageStatus::new(
                    StorageBackend::Sqlite,
                    mode,
                    writer,
                    ShutdownState::Open,
                    integrity(),
                    wal,
                    timeout,
                ),
                Err(Error::InvalidStorageStatus)
            );
        }
    }

    #[test]
    fn event_store_status_covers_bounds_and_accessors() {
        let generation = SourceGeneration::new([1; 32]).unwrap();
        assert_eq!(
            EventStoreStatus::new(
                generation,
                EventStoreMode::ReadOnly,
                EventStoreHealth::Unavailable,
                1,
                2,
                0,
            ),
            Err(Error::CorruptStoredEvent)
        );
        assert_eq!(
            EventStoreStatus::new(
                generation,
                EventStoreMode::ReadWrite,
                EventStoreHealth::Degraded,
                2,
                1,
                2,
            ),
            Err(Error::CorruptStoredEvent)
        );
        let status = EventStoreStatus::new(
            generation,
            EventStoreMode::ReadWrite,
            EventStoreHealth::Available,
            3,
            2,
            1,
        )
        .unwrap();
        assert_eq!(status.generation(), generation);
        assert_eq!(status.mode(), EventStoreMode::ReadWrite);
        assert_eq!(status.health(), EventStoreHealth::Available);
        assert_eq!(status.raw_events(), 3);
        assert_eq!(status.verified_events(), 2);
        assert_eq!(status.visible_events(), 1);
    }
}

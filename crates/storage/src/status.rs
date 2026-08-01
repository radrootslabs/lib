//! Storage capability, health, and integrity status contracts.

use crate::{Error, event::SourceGeneration};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageOpenMode {
    ReadOnly,
    ReadWriteExisting,
    Create,
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityHealth {
    Healthy,
    Degraded,
    Corrupt,
    Unknown,
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
    open_mode: StorageOpenMode,
    writer_policy: WriterPolicy,
    shutdown: ShutdownState,
    integrity: IntegrityStatus,
    wal_enabled: bool,
    busy_timeout_ms: u32,
}

impl StorageStatus {
    pub fn new(
        open_mode: StorageOpenMode,
        writer_policy: WriterPolicy,
        shutdown: ShutdownState,
        integrity: IntegrityStatus,
        wal_enabled: bool,
        busy_timeout_ms: u32,
    ) -> Result<Self, Error> {
        if (open_mode == StorageOpenMode::ReadOnly && writer_policy != WriterPolicy::NoWriter)
            || (open_mode != StorageOpenMode::ReadOnly
                && writer_policy != WriterPolicy::AdvisoryProcessLock)
            || (open_mode != StorageOpenMode::ReadOnly && (!wal_enabled || busy_timeout_ms == 0))
        {
            return Err(Error::InvalidStorageStatus);
        }
        Ok(Self {
            open_mode,
            writer_policy,
            shutdown,
            integrity,
            wal_enabled,
            busy_timeout_ms,
        })
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

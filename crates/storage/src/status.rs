//! Storage capability, health, and integrity status contracts.

use crate::event::{Error, SourceGeneration};

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

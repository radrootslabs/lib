//! Passive storage status values for the service-owned status envelope.

mod disk;

use core::num::NonZeroU32;

use serde::Serialize;

pub use disk::{
    MinimumFreeBytes, PlatformStateFilesystemCapacitySource, StateFilesystemCapacity,
    StateFilesystemCapacityError, StateFilesystemCapacityReadiness, StateFilesystemCapacitySource,
    inspect_state_filesystem_capacity,
};

/// Service-neutral storage health classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageHealth {
    Ready,
    ReadOnly,
    RepairRequired,
    Unavailable,
}

/// Service-neutral storage integrity classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageIntegrity {
    Verified,
    VerificationRequired,
    Failed,
}

/// Passive storage facts supplied to a versioned service-status envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct StorageStatus {
    health: StorageHealth,
    schema_version: NonZeroU32,
    generation: u64,
    integrity: StorageIntegrity,
}

impl StorageStatus {
    /// Constructs a passive status from already-validated storage facts.
    #[must_use]
    pub const fn new(
        health: StorageHealth,
        schema_version: NonZeroU32,
        generation: u64,
        integrity: StorageIntegrity,
    ) -> Self {
        Self {
            health,
            schema_version,
            generation,
            integrity,
        }
    }

    #[must_use]
    pub const fn health(self) -> StorageHealth {
        self.health
    }

    #[must_use]
    pub const fn schema_version(self) -> NonZeroU32 {
        self.schema_version
    }

    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn integrity(self) -> StorageIntegrity {
        self.integrity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_projection_and_enum_spellings_are_exact() {
        let health = [
            (StorageHealth::Ready, "ready"),
            (StorageHealth::ReadOnly, "read_only"),
            (StorageHealth::RepairRequired, "repair_required"),
            (StorageHealth::Unavailable, "unavailable"),
        ];
        for (value, wire) in health {
            assert_eq!(
                serde_json::to_string(&value).unwrap(),
                format!(r#""{wire}""#)
            );
        }

        let integrity = [
            (StorageIntegrity::Verified, "verified"),
            (
                StorageIntegrity::VerificationRequired,
                "verification_required",
            ),
            (StorageIntegrity::Failed, "failed"),
        ];
        for (value, wire) in integrity {
            assert_eq!(
                serde_json::to_string(&value).unwrap(),
                format!(r#""{wire}""#)
            );
        }

        let status = StorageStatus::new(
            StorageHealth::RepairRequired,
            NonZeroU32::new(1).unwrap(),
            7,
            StorageIntegrity::VerificationRequired,
        );
        assert_eq!(status.health(), StorageHealth::RepairRequired);
        assert_eq!(status.schema_version().get(), 1);
        assert_eq!(status.generation(), 7);
        assert_eq!(status.integrity(), StorageIntegrity::VerificationRequired);
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            r#"{"health":"repair_required","schema_version":1,"generation":7,"integrity":"verification_required"}"#
        );
    }

    #[test]
    fn zero_schema_version_cannot_cross_the_construction_boundary() {
        assert!(NonZeroU32::new(0).is_none());
        let maximum = NonZeroU32::new(u32::MAX).unwrap();
        let status = StorageStatus::new(
            StorageHealth::Ready,
            maximum,
            u64::MAX,
            StorageIntegrity::Verified,
        );
        assert_eq!(status.schema_version(), maximum);
        assert_eq!(status.generation(), u64::MAX);
    }
}

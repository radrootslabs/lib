//! Bounded canonical visibility-index preparation contracts.

use super::EventSourceToken;
use crate::Error;

/// Validated logical work envelope for one canonical index preparation call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibilityIndexBounds {
    rows: u16,
    bytes: u64,
}

impl VisibilityIndexBounds {
    /// Validates 1..=64 source rows and 1..=16 MiB of logical allocation charge.
    pub fn new(rows: u16, bytes: u64) -> Result<Self, Error> {
        if rows == 0 || rows > 64 || bytes == 0 || bytes > 16 * 1024 * 1024 {
            return Err(Error::InvalidVisibilityIndexBounds);
        }
        Ok(Self { rows, bytes })
    }

    /// Maximum source rows materialized in this call.
    pub const fn rows(self) -> u16 {
        self.rows
    }

    /// Maximum conservative logical allocation charge in this call.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    /// Charges retained bytes, decoded variable data and derived metadata.
    ///
    /// This conservative envelope is not a measurement of allocator RSS.
    #[doc(hidden)]
    pub fn charge(signed_bytes: u64) -> Result<u64, Error> {
        signed_bytes
            .checked_mul(16)
            .and_then(|value| value.checked_add(4096))
            .ok_or(Error::SpaceInsufficient)
    }
}

/// Durable progress observed after one canonical index preparation transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibilityIndexProgress {
    token: EventSourceToken,
    source_sequence: u64,
    replay_epoch: u64,
    processed_rows: u16,
    processed_keys: u16,
    charged_bytes: u64,
    complete: bool,
}

impl VisibilityIndexProgress {
    #[doc(hidden)]
    pub const fn new(
        token: EventSourceToken,
        source_sequence: u64,
        replay_epoch: u64,
        processed_rows: u16,
        charged_bytes: u64,
        complete: bool,
    ) -> Self {
        Self {
            token,
            source_sequence,
            replay_epoch,
            processed_rows,
            processed_keys: 0,
            charged_bytes,
            complete,
        }
    }

    /// Source fence read inside the completed preparation transaction.
    pub const fn token(self) -> EventSourceToken {
        self.token
    }
    /// Last initial-backfill sequence durably prepared; it never restarts on admission.
    pub const fn source_sequence(self) -> u64 {
        self.source_sequence
    }
    /// Last durable changed-event replay epoch.
    pub const fn replay_epoch(self) -> u64 {
        self.replay_epoch
    }
    /// Actual source rows materialized, including repeated changed rows.
    pub const fn processed_rows(self) -> u16 {
        self.processed_rows
    }
    /// Affected deletion keys refreshed within the same total row-unit budget.
    pub const fn processed_keys(self) -> u16 {
        self.processed_keys
    }

    #[doc(hidden)]
    pub const fn with_processed_keys(mut self, keys: u16) -> Self {
        self.processed_keys = keys;
        self
    }
    /// Conservative logical allocation charge of this call.
    pub const fn charged_bytes(self) -> u64 {
        self.charged_bytes
    }
    /// Whether canonical metadata matches `token`; it is not a consumer-content claim.
    pub const fn is_complete(self) -> bool {
        self.complete
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_envelope_rejects_zero_and_one_over_without_relaxing_caps() {
        assert!(VisibilityIndexBounds::new(64, 16 * 1024 * 1024).is_ok());
        for (rows, bytes) in [(0, 1), (65, 1), (1, 0), (1, 16 * 1024 * 1024 + 1)] {
            assert_eq!(
                VisibilityIndexBounds::new(rows, bytes),
                Err(Error::InvalidVisibilityIndexBounds)
            );
        }
        assert_eq!(VisibilityIndexBounds::charge(10).unwrap(), 4256);
        assert_eq!(
            VisibilityIndexBounds::charge(u64::MAX),
            Err(Error::SpaceInsufficient)
        );
    }
}

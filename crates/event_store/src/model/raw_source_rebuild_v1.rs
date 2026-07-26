use super::RadrootsEventStoreSourceGeneration;
use crate::RadrootsEventStoreSourceCapacityV1;

/// SHA-256 digest of the ordered immutable raw-event and raw-tag authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RadrootsEventStoreImmutableRawDigestV1(pub(crate) [u8; 32]);

impl RadrootsEventStoreImmutableRawDigestV1 {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the fixed-width digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// SHA-256 digest of generation-normalized active product state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RadrootsEventStoreActiveProductStateDigestV1(pub(crate) [u8; 32]);

impl RadrootsEventStoreActiveProductStateDigestV1 {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the fixed-width digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Committed result of rebuilding all active product state from immutable raw rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsEventStoreRawSourceRebuildReportV1 {
    pub(crate) prior_source_generation: RadrootsEventStoreSourceGeneration,
    pub(crate) new_source_generation: RadrootsEventStoreSourceGeneration,
    pub(crate) source_capacity: RadrootsEventStoreSourceCapacityV1,
    pub(crate) immutable_raw_digest: RadrootsEventStoreImmutableRawDigestV1,
    pub(crate) active_product_state_digest: RadrootsEventStoreActiveProductStateDigestV1,
}

impl RadrootsEventStoreRawSourceRebuildReportV1 {
    /// Returns the active generation replaced by this rebuild.
    pub const fn prior_source_generation(&self) -> RadrootsEventStoreSourceGeneration {
        self.prior_source_generation
    }

    /// Returns the generation committed by this rebuild.
    pub const fn new_source_generation(&self) -> RadrootsEventStoreSourceGeneration {
        self.new_source_generation
    }

    /// Returns the raw-source capacity seal committed for the new generation.
    pub const fn source_capacity(&self) -> RadrootsEventStoreSourceCapacityV1 {
        self.source_capacity
    }

    /// Returns the greatest retained raw event sequence.
    pub const fn raw_high_water_seq(&self) -> i64 {
        self.source_capacity.raw_high_water_seq()
    }

    /// Returns the digest of ordered immutable raw authority.
    pub const fn immutable_raw_digest(&self) -> RadrootsEventStoreImmutableRawDigestV1 {
        self.immutable_raw_digest
    }

    /// Returns the generation-normalized active product-state digest.
    pub const fn active_product_state_digest(
        &self,
    ) -> RadrootsEventStoreActiveProductStateDigestV1 {
        self.active_product_state_digest
    }
}

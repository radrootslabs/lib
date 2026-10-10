//! Explicit derived-cache budgets and exact, bounded retirement requests.
use super::{Error, ProjectionGeneration, ProjectionId};
use crate::event::EventSourceToken;

pub const PROJECTION_CACHE_COLLECTION_ROWS_MAX: usize = 64;

/// Caller-owned positive limits. Configuring a cache never silently raises them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionCacheLimits {
    value_bytes: u64,
    protected_generations: u64,
    legacy_snapshots: u64,
    legacy_age_ms: u64,
}
impl ProjectionCacheLimits {
    pub fn new(
        value_bytes: u64,
        protected_generations: u64,
        legacy_snapshots: u64,
        legacy_age_ms: u64,
    ) -> Result<Self, Error> {
        if [
            value_bytes,
            protected_generations,
            legacy_snapshots,
            legacy_age_ms,
        ]
        .iter()
        .any(|value| *value == 0 || *value > i64::MAX as u64)
        {
            return Err(Error::InvalidProjectionCacheLimits);
        }
        Ok(Self {
            value_bytes,
            protected_generations,
            legacy_snapshots,
            legacy_age_ms,
        })
    }
    pub const fn value_bytes(self) -> u64 {
        self.value_bytes
    }
    pub const fn protected_generations(self) -> u64 {
        self.protected_generations
    }
    pub const fn legacy_snapshots(self) -> u64 {
        self.legacy_snapshots
    }
    pub const fn legacy_age_ms(self) -> u64 {
        self.legacy_age_ms
    }
}

/// An exact cache owner/generation and the source/active identity observed by its caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionCacheScope {
    id: ProjectionId,
    generation: ProjectionGeneration,
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
}
impl ProjectionCacheScope {
    pub fn new(
        id: ProjectionId,
        generation: ProjectionGeneration,
        source: EventSourceToken,
        active: Option<ProjectionGeneration>,
    ) -> Result<Self, Error> {
        ProjectionId::parse(id.as_str())?;
        ProjectionGeneration::new(*generation.as_bytes())?;
        crate::event::SourceGeneration::new(*source.generation().as_bytes())?;
        if let Some(value) = active {
            ProjectionGeneration::new(*value.as_bytes())?;
        }
        Ok(Self {
            id,
            generation,
            source,
            active,
        })
    }
    pub const fn projection_id(&self) -> &ProjectionId {
        &self.id
    }
    pub const fn generation(&self) -> ProjectionGeneration {
        self.generation
    }
    pub const fn source(&self) -> EventSourceToken {
        self.source
    }
    pub const fn expected_active(&self) -> Option<ProjectionGeneration> {
        self.active
    }
}

/// Cached logical usage; physical database and WAL allocation are separate.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProjectionCacheUsage {
    pub value_bytes: u64,
    pub protected_generations: u64,
    pub legacy_snapshots: u64,
}

/// Actual collection progress. A pass removes at most64 total derived rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionCacheCollection {
    pub removed_rows: u64,
    pub removed_value_bytes: u64,
    pub more: bool,
    pub usage: ProjectionCacheUsage,
}

//! Projection checkpoint, event-index manifest, and rebuild contracts.
//!
//! Storage owns durable coordination metadata. Domain reducers and projected
//! row representations remain in their domain packages.

pub use radroots_event::EventId;
pub use radroots_transport::BoxFuture;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use crate::{
    Error,
    event::{EventPosition, SourceGeneration},
};

pub const PROJECTION_ID_MAX_BYTES: usize = 128;
pub const EVENT_INDEX_SHARD_ID_MAX_BYTES: usize = 128;
pub const EVENT_INDEX_ARTIFACT_PATH_MAX_BYTES: usize = 512;
pub const EVENT_INDEX_CURSOR_MAX_BYTES: usize = 2_048;
pub const EVENT_INDEX_SHARDS_MAX: usize = 4_096;

/// Stable, backend-neutral projection identity.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectionId(String);

impl ProjectionId {
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if !valid_label(value.as_str(), PROJECTION_ID_MAX_BYTES) {
            return Err(Error::InvalidProjectionId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Content-derived generation of a projection implementation and its inputs.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectionGeneration([u8; 32]);

impl ProjectionGeneration {
    pub const fn new(bytes: [u8; 32]) -> Result<Self, Error> {
        if bytes32_are_zero(&bytes) {
            return Err(Error::InvalidProjectionGeneration);
        }
        Ok(Self(bytes))
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// SHA-256 digest of one ordered, immutable canonical raw-event snapshot.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RawSourceDigest([u8; 32]);

impl RawSourceDigest {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Non-zero optimistic revision for projection coordination state.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProjectionRevision(u64);

impl ProjectionRevision {
    pub const INITIAL: Self = Self(1);
    pub const fn new(value: u64) -> Result<Self, Error> {
        if value == 0 {
            return Err(Error::InvalidProjectionRevision);
        }
        Ok(Self(value))
    }
    pub const fn get(self) -> u64 {
        self.0
    }
    fn next(self) -> Result<Self, Error> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(Error::CorruptProjectionRecord)
    }
}

/// Last canonical event incorporated by a projection generation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionCheckpoint {
    projection_id: ProjectionId,
    generation: ProjectionGeneration,
    source_position: Option<EventPosition>,
    projected_rows: u64,
    updated_at_unix_ms: u64,
}

impl ProjectionCheckpoint {
    pub fn new(
        projection_id: ProjectionId,
        generation: ProjectionGeneration,
        source_position: Option<EventPosition>,
        projected_rows: u64,
        updated_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if updated_at_unix_ms == 0 {
            return Err(Error::InvalidProjectionTimestamp);
        }
        Ok(Self {
            projection_id,
            generation,
            source_position,
            projected_rows,
            updated_at_unix_ms,
        })
    }
    pub const fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }
    pub const fn generation(&self) -> ProjectionGeneration {
        self.generation
    }
    pub const fn source_position(&self) -> Option<EventPosition> {
        self.source_position
    }
    pub const fn projected_rows(&self) -> u64 {
        self.projected_rows
    }
    pub const fn updated_at_unix_ms(&self) -> u64 {
        self.updated_at_unix_ms
    }

    pub fn advances(&self, prior: &Self) -> bool {
        self.projection_id == prior.projection_id
            && self.generation == prior.generation
            && self.updated_at_unix_ms >= prior.updated_at_unix_ms
            && self.projected_rows >= prior.projected_rows
            && match (self.source_position, prior.source_position) {
                (Some(next), Some(previous)) => {
                    next.generation() == previous.generation()
                        && next.sequence() >= previous.sequence()
                }
                (Some(_), None) | (None, None) => true,
                (None, Some(_)) => false,
            }
    }
}

/// Stable reason a projection can no longer be trusted.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidationReason {
    SourceGenerationChanged,
    ProjectionGenerationChanged,
    EventIndexManifestChanged,
    IntegrityFailure,
    OperatorRequested,
}

/// Durable projection invalidation evidence.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionInvalidation {
    projection_id: ProjectionId,
    invalid_generation: ProjectionGeneration,
    replacement_generation: ProjectionGeneration,
    reason: InvalidationReason,
    invalidated_at_unix_ms: u64,
}

impl ProjectionInvalidation {
    pub fn new(
        projection_id: ProjectionId,
        invalid_generation: ProjectionGeneration,
        replacement_generation: ProjectionGeneration,
        reason: InvalidationReason,
        invalidated_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if invalid_generation.0 == replacement_generation.0 || invalidated_at_unix_ms == 0 {
            return Err(Error::InvalidProjectionInvalidation);
        }
        Ok(Self {
            projection_id,
            invalid_generation,
            replacement_generation,
            reason,
            invalidated_at_unix_ms,
        })
    }
    pub const fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }
    pub const fn invalid_generation(&self) -> ProjectionGeneration {
        self.invalid_generation
    }
    pub const fn replacement_generation(&self) -> ProjectionGeneration {
        self.replacement_generation
    }
    pub const fn reason(&self) -> InvalidationReason {
        self.reason
    }
    pub const fn invalidated_at_unix_ms(&self) -> u64 {
        self.invalidated_at_unix_ms
    }
}

/// Stable identity of a projection rebuild execution.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RebuildTicketId([u8; 16]);

impl RebuildTicketId {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, Error> {
        if bytes16_are_zero(&bytes) {
            return Err(Error::InvalidRebuildTicketId);
        }
        Ok(Self(bytes))
    }
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RebuildStage {
    Requested,
    Running,
    Completed,
    Failed,
}

/// Stable, secret-safe classification retained for a failed rebuild.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RebuildFailure {
    ReducerRejected,
    SourceChanged,
    IntegrityFailure,
    PromotionRejected,
}

/// Optimistic, monotonic projection rebuild state.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RebuildTicket {
    ticket_id: RebuildTicketId,
    invalidation: ProjectionInvalidation,
    revision: ProjectionRevision,
    stage: RebuildStage,
    source_generation: SourceGeneration,
    source_high_water: Option<EventPosition>,
    source_digest: RawSourceDigest,
    checkpoint: Option<ProjectionCheckpoint>,
    failure: Option<RebuildFailure>,
    requested_at_unix_ms: u64,
    updated_at_unix_ms: u64,
}

impl RebuildTicket {
    pub fn requested(
        ticket_id: RebuildTicketId,
        invalidation: ProjectionInvalidation,
        source_generation: SourceGeneration,
        source_high_water: Option<EventPosition>,
        source_digest: RawSourceDigest,
    ) -> Result<Self, Error> {
        if source_high_water.is_some_and(|position| position.generation() != source_generation) {
            return Err(Error::SourceGenerationChanged);
        }
        let at = invalidation.invalidated_at_unix_ms();
        Ok(Self {
            ticket_id,
            invalidation,
            revision: ProjectionRevision::INITIAL,
            stage: RebuildStage::Requested,
            source_generation,
            source_high_water,
            source_digest,
            checkpoint: None,
            failure: None,
            requested_at_unix_ms: at,
            updated_at_unix_ms: at,
        })
    }

    /// Reconstructs and validates one durable rebuild ticket.
    #[allow(clippy::too_many_arguments)]
    pub fn from_durable_parts(
        ticket_id: RebuildTicketId,
        invalidation: ProjectionInvalidation,
        revision: ProjectionRevision,
        stage: RebuildStage,
        source_generation: SourceGeneration,
        source_high_water: Option<EventPosition>,
        source_digest: RawSourceDigest,
        checkpoint: Option<ProjectionCheckpoint>,
        failure: Option<RebuildFailure>,
        requested_at_unix_ms: u64,
        updated_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if source_high_water.is_some_and(|position| position.generation() != source_generation)
            || requested_at_unix_ms != invalidation.invalidated_at_unix_ms()
            || updated_at_unix_ms < requested_at_unix_ms
            || matches!(stage, RebuildStage::Requested)
                && (revision != ProjectionRevision::INITIAL
                    || updated_at_unix_ms != requested_at_unix_ms)
            || !matches!(stage, RebuildStage::Requested) && revision == ProjectionRevision::INITIAL
            || matches!(stage, RebuildStage::Requested) && checkpoint.is_some()
            || matches!(stage, RebuildStage::Completed) && checkpoint.is_none()
            || matches!(stage, RebuildStage::Failed) != failure.is_some()
            || !matches!(stage, RebuildStage::Failed) && failure.is_some()
            || checkpoint.as_ref().is_some_and(|checkpoint| {
                checkpoint.projection_id() != invalidation.projection_id()
                    || checkpoint.generation() != invalidation.replacement_generation()
                    || checkpoint
                        .source_position()
                        .is_some_and(|position| position.generation() != source_generation)
                    || checkpoint.updated_at_unix_ms() > updated_at_unix_ms
            })
        {
            return Err(Error::CorruptProjectionRecord);
        }
        Ok(Self {
            ticket_id,
            invalidation,
            revision,
            stage,
            source_generation,
            source_high_water,
            source_digest,
            checkpoint,
            failure,
            requested_at_unix_ms,
            updated_at_unix_ms,
        })
    }
    pub const fn ticket_id(&self) -> RebuildTicketId {
        self.ticket_id
    }
    pub const fn invalidation(&self) -> &ProjectionInvalidation {
        &self.invalidation
    }
    pub const fn revision(&self) -> ProjectionRevision {
        self.revision
    }
    pub const fn stage(&self) -> RebuildStage {
        self.stage
    }
    pub const fn source_generation(&self) -> SourceGeneration {
        self.source_generation
    }
    pub const fn source_high_water(&self) -> Option<EventPosition> {
        self.source_high_water
    }
    pub const fn source_digest(&self) -> RawSourceDigest {
        self.source_digest
    }
    pub const fn checkpoint(&self) -> Option<&ProjectionCheckpoint> {
        self.checkpoint.as_ref()
    }
    pub const fn requested_at_unix_ms(&self) -> u64 {
        self.requested_at_unix_ms
    }
    pub const fn updated_at_unix_ms(&self) -> u64 {
        self.updated_at_unix_ms
    }
    pub const fn failure(&self) -> Option<RebuildFailure> {
        self.failure
    }

    pub fn transition(&self, transition: RebuildTransition) -> Result<Self, Error> {
        if transition.ticket_id != self.ticket_id || transition.expected_revision != self.revision {
            return Err(Error::ProjectionRevisionConflict);
        }
        if transition.at_unix_ms < self.updated_at_unix_ms {
            return Err(Error::InvalidProjectionTimestamp);
        }
        let (stage, checkpoint, failure) = match (&self.stage, transition.kind) {
            (RebuildStage::Requested, RebuildTransitionKind::Start) => {
                (RebuildStage::Running, None, None)
            }
            (RebuildStage::Running, RebuildTransitionKind::Checkpoint(checkpoint)) => {
                self.validate_checkpoint(&checkpoint)?;
                if self
                    .checkpoint
                    .as_ref()
                    .is_some_and(|prior| !checkpoint.advances(prior))
                {
                    return Err(Error::ProjectionCheckpointRegression);
                }
                (RebuildStage::Running, Some(checkpoint), None)
            }
            (RebuildStage::Running, RebuildTransitionKind::Complete(checkpoint)) => {
                self.validate_checkpoint(&checkpoint)?;
                if self
                    .checkpoint
                    .as_ref()
                    .is_some_and(|prior| !checkpoint.advances(prior))
                {
                    return Err(Error::ProjectionCheckpointRegression);
                }
                (RebuildStage::Completed, Some(checkpoint), None)
            }
            (
                RebuildStage::Requested | RebuildStage::Running,
                RebuildTransitionKind::Fail(failure),
            ) => (RebuildStage::Failed, self.checkpoint.clone(), Some(failure)),
            (RebuildStage::Completed | RebuildStage::Failed, _) => {
                return Err(Error::RebuildTicketTerminal);
            }
            _ => return Err(Error::InvalidRebuildTransition),
        };
        Ok(Self {
            ticket_id: self.ticket_id,
            invalidation: self.invalidation.clone(),
            revision: self.revision.next()?,
            stage,
            source_generation: self.source_generation,
            source_high_water: self.source_high_water,
            source_digest: self.source_digest,
            checkpoint,
            failure,
            requested_at_unix_ms: self.requested_at_unix_ms,
            updated_at_unix_ms: transition.at_unix_ms,
        })
    }

    fn validate_checkpoint(&self, checkpoint: &ProjectionCheckpoint) -> Result<(), Error> {
        if checkpoint.projection_id() != self.invalidation.projection_id()
            || checkpoint.generation() != self.invalidation.replacement_generation()
            || checkpoint
                .source_position()
                .is_some_and(|position| position.generation() != self.source_generation)
        {
            return Err(Error::ProjectionCheckpointMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RebuildTransition {
    ticket_id: RebuildTicketId,
    expected_revision: ProjectionRevision,
    at_unix_ms: u64,
    kind: RebuildTransitionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RebuildTransitionKind {
    Start,
    Checkpoint(ProjectionCheckpoint),
    Complete(ProjectionCheckpoint),
    Fail(RebuildFailure),
}

impl RebuildTransition {
    pub const fn start(
        ticket_id: RebuildTicketId,
        expected_revision: ProjectionRevision,
        at_unix_ms: u64,
    ) -> Self {
        Self {
            ticket_id,
            expected_revision,
            at_unix_ms,
            kind: RebuildTransitionKind::Start,
        }
    }
    pub const fn checkpoint(
        ticket_id: RebuildTicketId,
        expected_revision: ProjectionRevision,
        at_unix_ms: u64,
        checkpoint: ProjectionCheckpoint,
    ) -> Self {
        Self {
            ticket_id,
            expected_revision,
            at_unix_ms,
            kind: RebuildTransitionKind::Checkpoint(checkpoint),
        }
    }
    pub const fn complete(
        ticket_id: RebuildTicketId,
        expected_revision: ProjectionRevision,
        at_unix_ms: u64,
        checkpoint: ProjectionCheckpoint,
    ) -> Self {
        Self {
            ticket_id,
            expected_revision,
            at_unix_ms,
            kind: RebuildTransitionKind::Complete(checkpoint),
        }
    }
    pub const fn fail(
        ticket_id: RebuildTicketId,
        expected_revision: ProjectionRevision,
        at_unix_ms: u64,
        failure: RebuildFailure,
    ) -> Self {
        Self {
            ticket_id,
            expected_revision,
            at_unix_ms,
            kind: RebuildTransitionKind::Fail(failure),
        }
    }
    pub const fn ticket_id(&self) -> RebuildTicketId {
        self.ticket_id
    }
}

/// SHA-256 digest of an immutable event-index shard artifact.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactDigest([u8; 32]);

impl ArtifactDigest {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EventIndexShardId(String);

impl EventIndexShardId {
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if !valid_label(value.as_str(), EVENT_INDEX_SHARD_ID_MAX_BYTES) {
            return Err(Error::InvalidEventIndexShardId);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventIdRange {
    first: EventId,
    last: EventId,
}

impl EventIdRange {
    pub fn new(first: EventId, last: EventId) -> Result<Self, Error> {
        if first > last {
            return Err(Error::InvalidEventIndexRange);
        }
        Ok(Self { first, last })
    }
    pub const fn first(&self) -> &EventId {
        &self.first
    }
    pub const fn last(&self) -> &EventId {
        &self.last
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventIndexShard {
    shard_id: EventIndexShardId,
    artifact_path: String,
    event_count: u32,
    event_ids: EventIdRange,
    first_published_at_unix_s: u64,
    last_published_at_unix_s: u64,
    sha256: ArtifactDigest,
}

impl EventIndexShard {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shard_id: EventIndexShardId,
        artifact_path: impl Into<String>,
        event_count: u32,
        event_ids: EventIdRange,
        first_published_at_unix_s: u64,
        last_published_at_unix_s: u64,
        sha256: ArtifactDigest,
    ) -> Result<Self, Error> {
        let artifact_path = artifact_path.into();
        if !valid_artifact_path(artifact_path.as_str()) {
            return Err(Error::InvalidEventIndexArtifactPath);
        }
        if event_count == 0 {
            return Err(Error::InvalidEventIndexShardCount);
        }
        if first_published_at_unix_s == 0 || last_published_at_unix_s < first_published_at_unix_s {
            return Err(Error::InvalidEventIndexTimestamp);
        }
        Ok(Self {
            shard_id,
            artifact_path,
            event_count,
            event_ids,
            first_published_at_unix_s,
            last_published_at_unix_s,
            sha256,
        })
    }
    pub const fn shard_id(&self) -> &EventIndexShardId {
        &self.shard_id
    }
    pub fn artifact_path(&self) -> &str {
        self.artifact_path.as_str()
    }
    pub const fn event_count(&self) -> u32 {
        self.event_count
    }
    pub const fn event_ids(&self) -> &EventIdRange {
        &self.event_ids
    }
    pub const fn first_published_at_unix_s(&self) -> u64 {
        self.first_published_at_unix_s
    }
    pub const fn last_published_at_unix_s(&self) -> u64 {
        self.last_published_at_unix_s
    }
    pub const fn sha256(&self) -> ArtifactDigest {
        self.sha256
    }
}

/// Validated, immutable event-index artifact inventory.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventIndexManifest {
    generation: ProjectionGeneration,
    total_events: u64,
    target_shard_size: u32,
    first_published_at_unix_s: u64,
    last_published_at_unix_s: u64,
    shards: Vec<EventIndexShard>,
}

impl EventIndexManifest {
    pub fn new(
        generation: ProjectionGeneration,
        total_events: u64,
        target_shard_size: u32,
        first_published_at_unix_s: u64,
        last_published_at_unix_s: u64,
        shards: Vec<EventIndexShard>,
    ) -> Result<Self, Error> {
        if shards.is_empty() || shards.len() > EVENT_INDEX_SHARDS_MAX {
            return Err(Error::InvalidEventIndexShardCount);
        }
        if target_shard_size == 0 || total_events == 0 {
            return Err(Error::InvalidEventIndexManifest);
        }
        let sum = shards.iter().try_fold(0_u64, |sum, shard| {
            if shard.event_count() > target_shard_size {
                return Err(Error::InvalidEventIndexManifest);
            }
            sum.checked_add(u64::from(shard.event_count()))
                .ok_or(Error::InvalidEventIndexManifest)
        })?;
        if sum != total_events
            || first_published_at_unix_s != shards[0].first_published_at_unix_s()
            || last_published_at_unix_s != shards[shards.len() - 1].last_published_at_unix_s()
        {
            return Err(Error::InvalidEventIndexManifest);
        }
        let mut shard_ids = BTreeSet::new();
        let mut artifact_paths = BTreeSet::new();
        if shards.iter().any(|shard| {
            !shard_ids.insert(shard.shard_id()) || !artifact_paths.insert(shard.artifact_path())
        }) {
            return Err(Error::InvalidEventIndexManifest);
        }
        for pair in shards.windows(2) {
            if pair[0].shard_id() >= pair[1].shard_id()
                || pair[0].event_ids().last() >= pair[1].event_ids().first()
                || pair[0].last_published_at_unix_s() > pair[1].first_published_at_unix_s()
            {
                return Err(Error::InvalidEventIndexManifest);
            }
        }
        Ok(Self {
            generation,
            total_events,
            target_shard_size,
            first_published_at_unix_s,
            last_published_at_unix_s,
            shards,
        })
    }
    pub const fn generation(&self) -> ProjectionGeneration {
        self.generation
    }
    pub const fn total_events(&self) -> u64 {
        self.total_events
    }
    pub const fn target_shard_size(&self) -> u32 {
        self.target_shard_size
    }
    pub const fn first_published_at_unix_s(&self) -> u64 {
        self.first_published_at_unix_s
    }
    pub const fn last_published_at_unix_s(&self) -> u64 {
        self.last_published_at_unix_s
    }
    pub fn shards(&self) -> &[EventIndexShard] {
        self.shards.as_slice()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventIndexShardCheckpoint {
    shard_id: EventIndexShardId,
    last_created_at_unix_s: u64,
    last_event_id: Option<EventId>,
    cursor: Option<String>,
}

impl EventIndexShardCheckpoint {
    pub fn new(
        shard_id: EventIndexShardId,
        last_created_at_unix_s: u64,
        last_event_id: Option<EventId>,
        cursor: Option<String>,
    ) -> Result<Self, Error> {
        if last_created_at_unix_s == 0 {
            return Err(Error::InvalidEventIndexTimestamp);
        }
        if let Some(value) = cursor.as_deref()
            && (value.is_empty()
                || value.len() > EVENT_INDEX_CURSOR_MAX_BYTES
                || value != value.trim()
                || value.chars().any(char::is_control))
        {
            return Err(Error::InvalidEventIndexCursor);
        }
        Ok(Self {
            shard_id,
            last_created_at_unix_s,
            last_event_id,
            cursor,
        })
    }
    pub const fn shard_id(&self) -> &EventIndexShardId {
        &self.shard_id
    }
    pub const fn last_created_at_unix_s(&self) -> u64 {
        self.last_created_at_unix_s
    }
    pub const fn last_event_id(&self) -> Option<&EventId> {
        self.last_event_id.as_ref()
    }
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventIndexCheckpoint {
    generation: ProjectionGeneration,
    generated_at_unix_ms: u64,
    shards: Vec<EventIndexShardCheckpoint>,
}

impl EventIndexCheckpoint {
    pub fn new(
        generation: ProjectionGeneration,
        generated_at_unix_ms: u64,
        mut shards: Vec<EventIndexShardCheckpoint>,
    ) -> Result<Self, Error> {
        if generated_at_unix_ms == 0 || shards.len() > EVENT_INDEX_SHARDS_MAX {
            return Err(Error::InvalidEventIndexCheckpoint);
        }
        shards.sort_by(|left, right| left.shard_id().cmp(right.shard_id()));
        if shards
            .windows(2)
            .any(|pair| pair[0].shard_id() == pair[1].shard_id())
        {
            return Err(Error::DuplicateEventIndexShard);
        }
        Ok(Self {
            generation,
            generated_at_unix_ms,
            shards,
        })
    }
    pub const fn generation(&self) -> ProjectionGeneration {
        self.generation
    }
    pub const fn generated_at_unix_ms(&self) -> u64 {
        self.generated_at_unix_ms
    }
    pub fn shards(&self) -> &[EventIndexShardCheckpoint] {
        self.shards.as_slice()
    }
    pub fn shard(&self, id: &EventIndexShardId) -> Option<&EventIndexShardCheckpoint> {
        self.shards
            .binary_search_by(|candidate| candidate.shard_id().cmp(id))
            .ok()
            .map(|index| &self.shards[index])
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionHealth {
    Ready,
    Invalidated,
    Rebuilding,
    Failed,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionStatus {
    projection_id: ProjectionId,
    generation: ProjectionGeneration,
    health: ProjectionHealth,
    checkpoint: Option<ProjectionCheckpoint>,
    active_rebuild: Option<RebuildTicketId>,
}

/// Maximum UTF-8 bytes in one materialized projection document key.
pub const PROJECTION_DOCUMENT_KEY_MAX_BYTES: usize = 512;
/// Maximum bytes in one materialized projection document value.
pub const PROJECTION_DOCUMENT_VALUE_MAX_BYTES: usize = 16 * 1024 * 1024;

/// One backend-neutral, opaque materialized projection document.
///
/// Projection owners define the value encoding. Storage verifies its digest
/// and treats the bytes as opaque so product-specific DTOs do not leak into
/// the generic persistence boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionDocument {
    key: String,
    value: Vec<u8>,
    value_sha256: [u8; 32],
}

impl ProjectionDocument {
    pub fn new(key: String, value: Vec<u8>) -> Result<Self, Error> {
        if !valid_document_key(&key)
            || value.is_empty()
            || value.len() > PROJECTION_DOCUMENT_VALUE_MAX_BYTES
        {
            return Err(Error::InvalidProjectionDocument);
        }
        let value_sha256 = Sha256::digest(&value).into();
        Ok(Self {
            key,
            value,
            value_sha256,
        })
    }

    pub fn from_stored_parts(
        key: String,
        value: Vec<u8>,
        value_sha256: [u8; 32],
    ) -> Result<Self, Error> {
        let document = Self::new(key, value)?;
        if document.value_sha256 != value_sha256 {
            return Err(Error::CorruptProjectionDocument);
        }
        Ok(document)
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }

    pub const fn value_sha256(&self) -> &[u8; 32] {
        &self.value_sha256
    }
}

/// One immutable, durable frozen-query snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionSnapshot {
    projection_id: ProjectionId,
    snapshot_id: [u8; 32],
    generation: ProjectionGeneration,
    created_at_unix_ms: u64,
    value: Vec<u8>,
    value_sha256: [u8; 32],
}

impl ProjectionSnapshot {
    pub fn new(
        projection_id: ProjectionId,
        snapshot_id: [u8; 32],
        generation: ProjectionGeneration,
        created_at_unix_ms: u64,
        value: Vec<u8>,
    ) -> Result<Self, Error> {
        if snapshot_id.iter().all(|byte| *byte == 0)
            || created_at_unix_ms == 0
            || value.is_empty()
            || value.len() > PROJECTION_DOCUMENT_VALUE_MAX_BYTES
        {
            return Err(Error::InvalidProjectionSnapshot);
        }
        let value_sha256 = Sha256::digest(&value).into();
        Ok(Self {
            projection_id,
            snapshot_id,
            generation,
            created_at_unix_ms,
            value,
            value_sha256,
        })
    }

    pub fn from_stored_parts(
        projection_id: ProjectionId,
        snapshot_id: [u8; 32],
        generation: ProjectionGeneration,
        created_at_unix_ms: u64,
        value: Vec<u8>,
        value_sha256: [u8; 32],
    ) -> Result<Self, Error> {
        let snapshot = Self::new(
            projection_id,
            snapshot_id,
            generation,
            created_at_unix_ms,
            value,
        )?;
        if snapshot.value_sha256 != value_sha256 {
            return Err(Error::CorruptProjectionDocument);
        }
        Ok(snapshot)
    }

    pub const fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }

    pub const fn snapshot_id(&self) -> &[u8; 32] {
        &self.snapshot_id
    }

    pub const fn generation(&self) -> ProjectionGeneration {
        self.generation
    }

    pub const fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }

    pub const fn value_sha256(&self) -> &[u8; 32] {
        &self.value_sha256
    }
}

impl ProjectionStatus {
    pub fn new(
        projection_id: ProjectionId,
        generation: ProjectionGeneration,
        health: ProjectionHealth,
        checkpoint: Option<ProjectionCheckpoint>,
        active_rebuild: Option<RebuildTicketId>,
    ) -> Result<Self, Error> {
        if checkpoint.as_ref().is_some_and(|value| {
            value.projection_id() != &projection_id || value.generation() != generation
        }) || (health == ProjectionHealth::Rebuilding) != active_rebuild.is_some()
        {
            return Err(Error::CorruptProjectionRecord);
        }
        Ok(Self {
            projection_id,
            generation,
            health,
            checkpoint,
            active_rebuild,
        })
    }
    pub const fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }
    pub const fn generation(&self) -> ProjectionGeneration {
        self.generation
    }
    pub const fn health(&self) -> ProjectionHealth {
        self.health
    }
    pub const fn checkpoint(&self) -> Option<&ProjectionCheckpoint> {
        self.checkpoint.as_ref()
    }
    pub const fn active_rebuild(&self) -> Option<RebuildTicketId> {
        self.active_rebuild
    }
}

/// Backend-neutral projection coordination SPI.
pub trait ProjectionStore: Send + Sync {
    fn status(
        &self,
        projection_id: ProjectionId,
    ) -> BoxFuture<'_, Result<Option<ProjectionStatus>, Error>>;
    fn checkpoint(
        &self,
        checkpoint: ProjectionCheckpoint,
    ) -> BoxFuture<'_, Result<ProjectionStatus, Error>>;
    fn invalidate(
        &self,
        invalidation: ProjectionInvalidation,
    ) -> BoxFuture<'_, Result<ProjectionStatus, Error>>;
    /// Returns the latest durable invalidation selecting a replacement generation.
    fn invalidation(
        &self,
        projection_id: ProjectionId,
        replacement_generation: ProjectionGeneration,
    ) -> BoxFuture<'_, Result<Option<ProjectionInvalidation>, Error>>;
    fn request_rebuild(&self, ticket: RebuildTicket)
    -> BoxFuture<'_, Result<RebuildTicket, Error>>;
    /// Returns one durable rebuild execution by identity.
    fn rebuild(
        &self,
        ticket_id: RebuildTicketId,
    ) -> BoxFuture<'_, Result<Option<RebuildTicket>, Error>>;
    fn transition_rebuild(
        &self,
        transition: RebuildTransition,
    ) -> BoxFuture<'_, Result<RebuildTicket, Error>>;
    fn event_index_manifest(
        &self,
        generation: ProjectionGeneration,
    ) -> BoxFuture<'_, Result<Option<EventIndexManifest>, Error>>;
    fn put_event_index_manifest(
        &self,
        manifest: EventIndexManifest,
    ) -> BoxFuture<'_, Result<(), Error>>;
    fn event_index_checkpoint(
        &self,
        generation: ProjectionGeneration,
    ) -> BoxFuture<'_, Result<Option<EventIndexCheckpoint>, Error>>;
    fn put_event_index_checkpoint(
        &self,
        checkpoint: EventIndexCheckpoint,
    ) -> BoxFuture<'_, Result<(), Error>>;
    /// Replaces one named materialized document for a projection generation.
    fn put_projection_document(
        &self,
        projection_id: ProjectionId,
        generation: ProjectionGeneration,
        document: ProjectionDocument,
    ) -> BoxFuture<'_, Result<(), Error>>;
    /// Loads one named materialized document for an exact generation.
    fn projection_document(
        &self,
        projection_id: ProjectionId,
        generation: ProjectionGeneration,
        key: String,
    ) -> BoxFuture<'_, Result<Option<ProjectionDocument>, Error>>;
    /// Persists one immutable frozen-query snapshot idempotently.
    fn put_projection_snapshot(
        &self,
        snapshot: ProjectionSnapshot,
    ) -> BoxFuture<'_, Result<(), Error>>;
    /// Loads one immutable frozen-query snapshot by exact identity.
    fn projection_snapshot(
        &self,
        projection_id: ProjectionId,
        snapshot_id: [u8; 32],
    ) -> BoxFuture<'_, Result<Option<ProjectionSnapshot>, Error>>;
}

fn valid_label(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value == value.trim()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

fn valid_document_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= PROJECTION_DOCUMENT_KEY_MAX_BYTES
        && value == value.trim()
        && !value.chars().any(char::is_control)
}

fn valid_artifact_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= EVENT_INDEX_ARTIFACT_PATH_MAX_BYTES
        && value == value.trim()
        && !value.starts_with('/')
        && !value.contains('\\')
        && value.split('/').all(|part| {
            !part.is_empty() && part != "." && part != ".." && !part.chars().any(char::is_control)
        })
}

const fn bytes16_are_zero(bytes: &[u8; 16]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

const fn bytes32_are_zero(bytes: &[u8; 32]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod materialized_tests {
    use super::*;

    #[test]
    fn materialized_document_and_snapshot_bounds_and_digests_fail_closed() {
        assert_eq!(
            ProjectionDocument::new(String::new(), vec![1]),
            Err(Error::InvalidProjectionDocument)
        );
        assert_eq!(
            ProjectionDocument::new("key".into(), Vec::new()),
            Err(Error::InvalidProjectionDocument)
        );
        assert_eq!(
            ProjectionDocument::new("k".repeat(PROJECTION_DOCUMENT_KEY_MAX_BYTES + 1), vec![1],),
            Err(Error::InvalidProjectionDocument)
        );
        assert_eq!(
            ProjectionDocument::new(" key".into(), vec![1]),
            Err(Error::InvalidProjectionDocument)
        );
        assert_eq!(
            ProjectionDocument::new("key\npart".into(), vec![1]),
            Err(Error::InvalidProjectionDocument)
        );
        assert_eq!(
            ProjectionDocument::new(
                "key".into(),
                vec![0; PROJECTION_DOCUMENT_VALUE_MAX_BYTES + 1],
            ),
            Err(Error::InvalidProjectionDocument)
        );
        let document = ProjectionDocument::new("key".into(), vec![1, 2]).unwrap();
        assert_eq!(document.key(), "key");
        assert_eq!(document.value(), [1, 2]);
        assert_eq!(document.value_sha256().len(), 32);
        assert_eq!(
            ProjectionDocument::from_stored_parts("key".into(), vec![1, 2], [9; 32]),
            Err(Error::CorruptProjectionDocument)
        );

        let projection_id = ProjectionId::parse("today").unwrap();
        let generation = ProjectionGeneration::new([1; 32]).unwrap();
        assert_eq!(
            ProjectionSnapshot::new(projection_id.clone(), [0; 32], generation, 1, vec![1]),
            Err(Error::InvalidProjectionSnapshot)
        );
        assert_eq!(
            ProjectionSnapshot::new(projection_id.clone(), [2; 32], generation, 0, vec![1]),
            Err(Error::InvalidProjectionSnapshot)
        );
        assert_eq!(
            ProjectionSnapshot::new(projection_id.clone(), [2; 32], generation, 1, Vec::new()),
            Err(Error::InvalidProjectionSnapshot)
        );
        assert_eq!(
            ProjectionSnapshot::new(
                projection_id.clone(),
                [2; 32],
                generation,
                1,
                vec![0; PROJECTION_DOCUMENT_VALUE_MAX_BYTES + 1],
            ),
            Err(Error::InvalidProjectionSnapshot)
        );
        let snapshot =
            ProjectionSnapshot::new(projection_id.clone(), [2; 32], generation, 1, vec![1])
                .unwrap();
        assert_eq!(snapshot.projection_id(), &projection_id);
        assert_eq!(snapshot.snapshot_id(), &[2; 32]);
        assert_eq!(snapshot.generation(), generation);
        assert_eq!(snapshot.created_at_unix_ms(), 1);
        assert_eq!(snapshot.value(), [1]);
        assert_eq!(snapshot.value_sha256().len(), 32);
        assert_eq!(
            ProjectionSnapshot::from_stored_parts(
                projection_id,
                [2; 32],
                generation,
                1,
                vec![1],
                [9; 32],
            ),
            Err(Error::CorruptProjectionDocument)
        );
    }
}

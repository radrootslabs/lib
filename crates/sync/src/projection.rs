//! Projection refresh and rebuild orchestration.

use radroots_storage::{
    Error as StorageError, ProjectionStore,
    event::{
        AdmissionStage, EVENT_QUERY_LIMIT_MAX, EventPosition, EventQuery, EventQueryBounds,
        SourceGeneration, StoredVisibleEvent,
    },
    projection::{
        InvalidationReason, ProjectionCheckpoint, ProjectionGeneration, ProjectionHealth,
        ProjectionId, ProjectionInvalidation, ProjectionRevision, ProjectionStatus,
        RawSourceDigest, RebuildFailure, RebuildStage, RebuildTicket, RebuildTicketId,
        RebuildTransition,
    },
};
use sha2::{Digest, Sha256};

use crate::{
    Engine,
    policy::{Error, OperationKind},
};

/// Maximum number of reducer batches in one explicit refresh call.
pub const PROJECTION_REFRESH_MAX_BATCHES: u16 = 1_000;
/// Maximum canonical raw events included in one rebuild source preflight.
pub const PROJECTION_RAW_SOURCE_MAX_EVENTS: u64 = 1_000_000;
const RAW_SOURCE_DIGEST_DOMAIN: &[u8] = b"radroots:projection:raw-source:v1\0";

/// Bounded refresh request for one exact reducer generation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshRequest {
    projection_id: ProjectionId,
    generation: ProjectionGeneration,
    batch_limit: u16,
    max_batches: u16,
}

impl RefreshRequest {
    pub fn new(
        projection_id: ProjectionId,
        generation: ProjectionGeneration,
        batch_limit: u16,
        max_batches: u16,
    ) -> Result<Self, Error> {
        if batch_limit == 0
            || batch_limit > EVENT_QUERY_LIMIT_MAX
            || max_batches == 0
            || max_batches > PROJECTION_REFRESH_MAX_BATCHES
        {
            return Err(Error::InvalidProjectionRequest);
        }
        Ok(Self {
            projection_id,
            generation,
            batch_limit,
            max_batches,
        })
    }

    pub const fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }

    pub const fn generation(&self) -> ProjectionGeneration {
        self.generation
    }

    pub const fn batch_limit(&self) -> u16 {
        self.batch_limit
    }

    pub const fn max_batches(&self) -> u16 {
        self.max_batches
    }
}

/// Owning-domain deterministic reducer capability.
///
/// Reducers own domain semantics and projected row calculation. They receive
/// canonical visible events in storage order and must perform no durable
/// metadata mutation; sync owns the checkpoint/rebuild coordination boundary.
pub trait Reducer: Send + Sync {
    fn projection_id(&self) -> &ProjectionId;
    fn generation(&self) -> ProjectionGeneration;
    /// Opens an isolated replacement generation. Existing readers must remain
    /// bound to the active generation until storage promotes the ticket.
    fn begin_rebuild(
        &self,
        ticket_id: RebuildTicketId,
        source_generation: SourceGeneration,
        source_digest: RawSourceDigest,
    ) -> Result<(), ReducerError>;
    fn reduce(
        &self,
        events: &[StoredVisibleEvent],
        prior_projected_rows: u64,
        rebuild_ticket: Option<RebuildTicketId>,
    ) -> Result<u64, ReducerError>;
    /// Discards an isolated replacement generation after durable failure.
    fn abort_rebuild(
        &self,
        ticket_id: RebuildTicketId,
        failure: RebuildFailure,
    ) -> Result<(), ReducerError>;
}

/// Secret-safe reducer rejection normalized at the orchestration boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReducerError;

/// Refresh execution class.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshKind {
    Incremental,
    Rebuild,
}

/// Deterministic state returned to the host scheduler.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshState {
    Complete,
    Partial,
    Failed,
}

/// Normalized projection progress after one bounded refresh call.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshReceipt {
    kind: RefreshKind,
    state: RefreshState,
    batches: u16,
    events_reduced: usize,
    checkpoint: Option<ProjectionCheckpoint>,
    rebuild_ticket: Option<RebuildTicketId>,
}

impl RefreshReceipt {
    pub const fn kind(&self) -> RefreshKind {
        self.kind
    }
    pub const fn state(&self) -> RefreshState {
        self.state
    }
    pub const fn batches(&self) -> u16 {
        self.batches
    }
    pub const fn events_reduced(&self) -> usize {
        self.events_reduced
    }
    pub const fn checkpoint(&self) -> Option<&ProjectionCheckpoint> {
        self.checkpoint.as_ref()
    }
    pub const fn rebuild_ticket(&self) -> Option<RebuildTicketId> {
        self.rebuild_ticket
    }
}

impl Engine {
    /// Runs at most the requested number of deterministic reducer batches.
    pub async fn refresh_projection(
        &self,
        request: RefreshRequest,
        reducer: &dyn Reducer,
    ) -> Result<RefreshReceipt, Error> {
        if reducer.projection_id() != request.projection_id()
            || reducer.generation() != request.generation()
        {
            return Err(Error::InvalidProjectionRequest);
        }
        let status = ProjectionStore::status(self.storage.as_ref(), request.projection_id.clone())
            .await
            .map_err(map_storage_error)?;
        let source = if status.as_ref().is_some_and(|status| {
            status.generation() != request.generation
                || status.health() == ProjectionHealth::Rebuilding
        }) {
            Some(self.raw_source_snapshot().await?)
        } else {
            None
        };
        let mut coordination = self
            .projection_coordination(&request, status, source.as_ref())
            .await?;
        let kind = if coordination.ticket.is_some() {
            RefreshKind::Rebuild
        } else {
            RefreshKind::Incremental
        };
        let mut receipt = RefreshReceipt {
            kind,
            state: RefreshState::Partial,
            batches: 0,
            events_reduced: 0,
            checkpoint: coordination.checkpoint.clone(),
            rebuild_ticket: coordination.ticket.as_ref().map(RebuildTicket::ticket_id),
        };

        if let Some(ticket) = coordination.ticket.as_ref()
            && !source.is_some_and(|source| source.matches_ticket(ticket))
        {
            self.fail_rebuild(ticket, reducer, RebuildFailure::SourceChanged)
                .await?;
            receipt.state = RefreshState::Failed;
            return Ok(receipt);
        }

        if coordination.started
            && let Some(ticket) = coordination.ticket.as_ref()
            && reducer
                .begin_rebuild(
                    ticket.ticket_id(),
                    ticket.source_generation(),
                    ticket.source_digest(),
                )
                .is_err()
        {
            self.fail_rebuild(ticket, reducer, RebuildFailure::ReducerRejected)
                .await?;
            receipt.state = RefreshState::Failed;
            return Ok(receipt);
        }

        for batch_index in 0..request.max_batches {
            let mut bounds =
                EventQueryBounds::first(request.batch_limit).map_err(map_storage_error)?;
            if let Some(position) = coordination
                .checkpoint
                .as_ref()
                .and_then(ProjectionCheckpoint::source_position)
            {
                bounds = bounds.after(position);
            }
            let page = self
                .storage
                .query_visible(EventQuery::all(bounds))
                .await
                .map_err(map_storage_error)?;
            let prior_rows = coordination
                .checkpoint
                .as_ref()
                .map_or(0, ProjectionCheckpoint::projected_rows);
            let projected_rows = if page.items().is_empty() {
                prior_rows
            } else {
                match reducer.reduce(
                    page.items(),
                    prior_rows,
                    coordination.ticket.as_ref().map(RebuildTicket::ticket_id),
                ) {
                    Ok(rows) if rows >= prior_rows => rows,
                    Ok(_) => return Err(Error::InvalidReducerOutput),
                    Err(_) => {
                        if let Some(ticket) = coordination.ticket.as_ref() {
                            self.fail_rebuild(ticket, reducer, RebuildFailure::ReducerRejected)
                                .await?;
                        }
                        receipt.state = RefreshState::Failed;
                        return Ok(receipt);
                    }
                }
            };
            let source_position = page
                .items()
                .last()
                .map(StoredVisibleEvent::position)
                .or_else(|| {
                    coordination
                        .checkpoint
                        .as_ref()
                        .and_then(ProjectionCheckpoint::source_position)
                });
            let checkpoint = ProjectionCheckpoint::new(
                request.projection_id.clone(),
                request.generation,
                source_position,
                projected_rows,
                self.clock.now_unix_ms()?,
            )
            .map_err(map_storage_error)?;
            let complete = page.items().len() < usize::from(request.batch_limit);
            if let Some(ticket) = coordination.ticket.as_mut() {
                if complete {
                    let current_source = self.raw_source_snapshot().await?;
                    if !current_source.matches_ticket(ticket) {
                        self.fail_rebuild(ticket, reducer, RebuildFailure::SourceChanged)
                            .await?;
                        receipt.state = RefreshState::Failed;
                        return Ok(receipt);
                    }
                }
                let transition = if complete {
                    RebuildTransition::complete(
                        ticket.ticket_id(),
                        ticket.revision(),
                        checkpoint.updated_at_unix_ms(),
                        checkpoint.clone(),
                    )
                } else {
                    RebuildTransition::checkpoint(
                        ticket.ticket_id(),
                        ticket.revision(),
                        checkpoint.updated_at_unix_ms(),
                        checkpoint.clone(),
                    )
                };
                match self.storage.transition_rebuild(transition).await {
                    Ok(next) => *ticket = next,
                    Err(StorageError::SourceGenerationChanged) if complete => {
                        self.fail_rebuild(ticket, reducer, RebuildFailure::SourceChanged)
                            .await?;
                        receipt.state = RefreshState::Failed;
                        return Ok(receipt);
                    }
                    Err(error) => return Err(map_storage_error(error)),
                }
            } else {
                self.storage
                    .checkpoint(checkpoint.clone())
                    .await
                    .map_err(map_storage_error)?;
            }
            receipt.batches += 1;
            receipt.events_reduced += page.items().len();
            receipt.checkpoint = Some(checkpoint.clone());
            coordination.checkpoint = Some(checkpoint);
            if complete {
                receipt.state = RefreshState::Complete;
                return Ok(receipt);
            }
            if batch_index + 1 == request.max_batches {
                receipt.state = RefreshState::Partial;
                return Ok(receipt);
            }
        }
        unreachable!("validated refresh requests execute at least one batch")
    }

    async fn projection_coordination(
        &self,
        request: &RefreshRequest,
        status: Option<ProjectionStatus>,
        source: Option<&RawSourceSnapshot>,
    ) -> Result<ProjectionCoordination, Error> {
        let Some(status) = status else {
            return Ok(ProjectionCoordination::default());
        };
        if status.generation() == request.generation && status.health() == ProjectionHealth::Ready {
            return Ok(ProjectionCoordination {
                checkpoint: status.checkpoint().cloned(),
                ticket: None,
                started: false,
            });
        }
        if status.health() == ProjectionHealth::Rebuilding {
            let ticket_id = status.active_rebuild().ok_or(Error::StorageFailed)?;
            let ticket = self
                .storage
                .rebuild(ticket_id)
                .await
                .map_err(map_storage_error)?
                .ok_or(Error::StorageFailed)?;
            if ticket.invalidation().replacement_generation() != request.generation {
                return Err(Error::StorageConflict);
            }
            return Ok(ProjectionCoordination {
                checkpoint: ticket.checkpoint().cloned(),
                ticket: Some(ticket),
                started: false,
            });
        }

        let invalidation = if status.generation() != request.generation {
            if status.health() != ProjectionHealth::Ready {
                return Err(Error::StorageConflict);
            }
            let invalidation = match self
                .storage
                .invalidation(request.projection_id.clone(), request.generation)
                .await
                .map_err(map_storage_error)?
            {
                Some(existing) if existing.invalid_generation() == status.generation() => existing,
                Some(_) => return Err(Error::StorageConflict),
                None => ProjectionInvalidation::new(
                    request.projection_id.clone(),
                    status.generation(),
                    request.generation,
                    InvalidationReason::ProjectionGenerationChanged,
                    self.clock.now_unix_ms()?,
                )
                .map_err(map_storage_error)?,
            };
            self.storage
                .invalidate(invalidation.clone())
                .await
                .map_err(map_storage_error)?;
            invalidation
        } else if status.health() == ProjectionHealth::Invalidated {
            self.storage
                .invalidation(request.projection_id.clone(), request.generation)
                .await
                .map_err(map_storage_error)?
                .ok_or(Error::StorageFailed)?
        } else {
            return Err(Error::StorageConflict);
        };
        let sync_id = self.ids.next_id(OperationKind::Projection)?;
        let source = source.ok_or(Error::StorageFailed)?;
        let ticket = RebuildTicket::requested(
            RebuildTicketId::new(*sync_id.as_bytes()).map_err(map_storage_error)?,
            invalidation,
            source.generation,
            source.high_water,
            source.digest,
        )
        .map_err(map_storage_error)?;
        let requested = self
            .storage
            .request_rebuild(ticket)
            .await
            .map_err(map_storage_error)?;
        let running = self
            .storage
            .transition_rebuild(RebuildTransition::start(
                requested.ticket_id(),
                ProjectionRevision::INITIAL,
                self.clock.now_unix_ms()?,
            ))
            .await
            .map_err(map_storage_error)?;
        Ok(ProjectionCoordination {
            checkpoint: None,
            ticket: Some(running),
            started: true,
        })
    }

    async fn raw_source_snapshot(&self) -> Result<RawSourceSnapshot, Error> {
        let mut hasher = Sha256::new();
        hasher.update(RAW_SOURCE_DIGEST_DOMAIN);
        let mut cursor = None;
        let mut count = 0_u64;
        let mut generation = None;
        let mut high_water = None;
        loop {
            let mut bounds =
                EventQueryBounds::first(EVENT_QUERY_LIMIT_MAX).map_err(map_storage_error)?;
            if let Some(position) = cursor {
                bounds = bounds.after(position);
            }
            let page = self
                .storage
                .query_raw(EventQuery::all(bounds))
                .await
                .map_err(map_storage_error)?;
            if generation
                .replace(page.generation())
                .is_some_and(|prior| prior != page.generation())
            {
                return Err(Error::StorageConflict);
            }
            hasher.update(page.generation().as_bytes());
            for event in page.items() {
                count = count.checked_add(1).ok_or(Error::StorageFailed)?;
                if count > PROJECTION_RAW_SOURCE_MAX_EVENTS {
                    return Err(Error::InvalidProjectionRequest);
                }
                let position = event.position();
                hasher.update(position.sequence().get().to_be_bytes());
                hasher.update([admission_stage_byte(event.stage())]);
                let raw = event.event().raw_json().as_bytes();
                hasher.update(
                    u64::try_from(raw.len())
                        .map_err(|_| Error::StorageFailed)?
                        .to_be_bytes(),
                );
                hasher.update(raw);
                high_water = Some(position);
            }
            cursor = page.next_cursor();
            if cursor.is_none() {
                break;
            }
        }
        Ok(RawSourceSnapshot {
            generation: generation.ok_or(Error::StorageFailed)?,
            high_water,
            digest: RawSourceDigest::new(hasher.finalize().into()),
        })
    }

    async fn fail_rebuild(
        &self,
        ticket: &RebuildTicket,
        reducer: &dyn Reducer,
        failure: RebuildFailure,
    ) -> Result<(), Error> {
        let failed = self
            .storage
            .transition_rebuild(RebuildTransition::fail(
                ticket.ticket_id(),
                ticket.revision(),
                self.clock.now_unix_ms()?,
                failure,
            ))
            .await
            .map_err(map_storage_error)?;
        debug_assert_eq!(failed.stage(), RebuildStage::Failed);
        reducer
            .abort_rebuild(ticket.ticket_id(), failure)
            .map_err(|_| Error::InvalidReducerOutput)
    }
}

#[derive(Default)]
struct ProjectionCoordination {
    checkpoint: Option<ProjectionCheckpoint>,
    ticket: Option<RebuildTicket>,
    started: bool,
}

#[derive(Clone, Copy)]
struct RawSourceSnapshot {
    generation: SourceGeneration,
    high_water: Option<EventPosition>,
    digest: RawSourceDigest,
}

impl RawSourceSnapshot {
    fn matches_ticket(self, ticket: &RebuildTicket) -> bool {
        self.generation == ticket.source_generation()
            && self.high_water == ticket.source_high_water()
            && self.digest == ticket.source_digest()
    }
}

const fn admission_stage_byte(stage: AdmissionStage) -> u8 {
    match stage {
        AdmissionStage::Raw => 0,
        AdmissionStage::Verified => 1,
        AdmissionStage::Visible => 2,
    }
}

fn map_storage_error(error: StorageError) -> Error {
    match error {
        StorageError::ProjectionCheckpointMismatch
        | StorageError::ProjectionCheckpointRegression
        | StorageError::ProjectionRevisionConflict
        | StorageError::SourceGenerationChanged => Error::StorageConflict,
        _ => Error::StorageFailed,
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn raw_source_identity_stage_encoding_and_error_mapping_are_exact() {
        let invalidation = ProjectionInvalidation::new(
            ProjectionId::parse("projection-helper").unwrap(),
            ProjectionGeneration::new([1; 32]).unwrap(),
            ProjectionGeneration::new([2; 32]).unwrap(),
            InvalidationReason::ProjectionGenerationChanged,
            1,
        )
        .unwrap();
        let source_generation = SourceGeneration::new([11; 32]).unwrap();
        let digest = RawSourceDigest::new([12; 32]);
        let ticket = RebuildTicket::requested(
            RebuildTicketId::new([13; 16]).unwrap(),
            invalidation,
            source_generation,
            None,
            digest,
        )
        .unwrap();
        let matching = RawSourceSnapshot {
            generation: source_generation,
            high_water: None,
            digest,
        };
        assert!(matching.matches_ticket(&ticket));
        assert!(
            !RawSourceSnapshot {
                generation: SourceGeneration::new([14; 32]).unwrap(),
                ..matching
            }
            .matches_ticket(&ticket)
        );
        assert!(
            !RawSourceSnapshot {
                high_water: Some(EventPosition::new(
                    source_generation,
                    radroots_storage::event::EventSequence::new(1).unwrap(),
                )),
                ..matching
            }
            .matches_ticket(&ticket)
        );
        assert!(
            !RawSourceSnapshot {
                digest: RawSourceDigest::new([15; 32]),
                ..matching
            }
            .matches_ticket(&ticket)
        );
        assert_eq!(admission_stage_byte(AdmissionStage::Raw), 0);
        assert_eq!(admission_stage_byte(AdmissionStage::Verified), 1);
        assert_eq!(admission_stage_byte(AdmissionStage::Visible), 2);
        assert_eq!(
            map_storage_error(StorageError::ProjectionRevisionConflict),
            Error::StorageConflict
        );
        assert_eq!(
            map_storage_error(StorageError::BackendUnavailable),
            Error::StorageFailed
        );
    }
}

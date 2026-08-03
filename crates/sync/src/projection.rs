//! Projection refresh and rebuild orchestration.

use radroots_storage::{
    Error as StorageError, ProjectionStore,
    event::{EVENT_QUERY_LIMIT_MAX, EventQuery, EventQueryBounds, StoredVisibleEvent},
    projection::{
        InvalidationReason, ProjectionCheckpoint, ProjectionGeneration, ProjectionHealth,
        ProjectionId, ProjectionInvalidation, ProjectionRevision, ProjectionStatus, RebuildTicket,
        RebuildTicketId, RebuildTransition,
    },
};

use crate::{
    Engine,
    policy::{Error, OperationKind},
};

/// Maximum number of reducer batches in one explicit refresh call.
pub const PROJECTION_REFRESH_MAX_BATCHES: u16 = 1_000;

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
    fn reduce(
        &self,
        events: &[StoredVisibleEvent],
        prior_projected_rows: u64,
    ) -> Result<u64, ReducerError>;
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
        let mut coordination = self.projection_coordination(&request, status).await?;
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
                match reducer.reduce(page.items(), prior_rows) {
                    Ok(rows) if rows >= prior_rows => rows,
                    Ok(_) => return Err(Error::InvalidReducerOutput),
                    Err(_) => {
                        if let Some(ticket) = coordination.ticket.as_mut() {
                            let failed = self
                                .storage
                                .transition_rebuild(RebuildTransition::fail(
                                    ticket.ticket_id(),
                                    ticket.revision(),
                                    self.clock.now_unix_ms()?,
                                ))
                                .await
                                .map_err(map_storage_error)?;
                            *ticket = failed;
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
                *ticket = self
                    .storage
                    .transition_rebuild(transition)
                    .await
                    .map_err(map_storage_error)?;
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
    ) -> Result<ProjectionCoordination, Error> {
        let Some(status) = status else {
            return Ok(ProjectionCoordination::default());
        };
        if status.generation() == request.generation && status.health() == ProjectionHealth::Ready {
            return Ok(ProjectionCoordination {
                checkpoint: status.checkpoint().cloned(),
                ticket: None,
            });
        }
        if status.generation() == request.generation
            && status.health() == ProjectionHealth::Rebuilding
        {
            let ticket_id = status.active_rebuild().ok_or(Error::StorageFailed)?;
            let ticket = self
                .storage
                .rebuild(ticket_id)
                .await
                .map_err(map_storage_error)?
                .ok_or(Error::StorageFailed)?;
            return Ok(ProjectionCoordination {
                checkpoint: ticket.checkpoint().cloned(),
                ticket: Some(ticket),
            });
        }

        let invalidation = if status.generation() != request.generation {
            if status.health() != ProjectionHealth::Ready {
                return Err(Error::StorageConflict);
            }
            let invalidation = ProjectionInvalidation::new(
                request.projection_id.clone(),
                status.generation(),
                request.generation,
                InvalidationReason::ProjectionGenerationChanged,
                self.clock.now_unix_ms()?,
            )
            .map_err(map_storage_error)?;
            self.storage
                .invalidate(invalidation.clone())
                .await
                .map_err(map_storage_error)?;
            invalidation
        } else if matches!(
            status.health(),
            ProjectionHealth::Invalidated | ProjectionHealth::Failed
        ) {
            self.storage
                .invalidation(request.projection_id.clone(), request.generation)
                .await
                .map_err(map_storage_error)?
                .ok_or(Error::StorageFailed)?
        } else {
            return Err(Error::StorageConflict);
        };
        let sync_id = self.ids.next_id(OperationKind::Projection)?;
        let ticket = RebuildTicket::requested(
            RebuildTicketId::new(*sync_id.as_bytes()).map_err(map_storage_error)?,
            invalidation,
        );
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
        })
    }
}

#[derive(Default)]
struct ProjectionCoordination {
    checkpoint: Option<ProjectionCheckpoint>,
    ticket: Option<RebuildTicket>,
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

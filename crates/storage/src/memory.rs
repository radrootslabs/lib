//! Deterministic in-memory reference storage backend.

use radroots_event::EventId;
use radroots_protocol::runtime::v1::OperationId;
use radroots_transport::{BoxFuture, source::EventProvenance};
use std::sync::{Mutex, MutexGuard};

use crate::{
    Error, EventStore, Journal, Outbox, ProjectionStore,
    atomic::{
        AtomicCommit, AtomicCommitDisposition, AtomicCommitId, AtomicCommitOutcome,
        AtomicCommitReceipt, AtomicStorage, AtomicWorkflow,
    },
    authored::{AdmissionState, FailureClass, WorkFailure, WorkPhase},
    authored_atomic::{
        AuthoredAtomicCommand, AuthoredAtomicOutcome, AuthoredAtomicReceipt, AuthoredAtomicStorage,
        AuthoredWorkTarget, CancelAuthoredTarget, ClaimAuthoredTarget,
    },
    authored_delivery::DeliveryAttemptOutcome,
    backup::{
        BackupId, BackupOperation, BackupPlan, BackupTransition, ReliabilityRevision,
        RestoreOperation, RestorePlan, RestoreTransition, StorageReliability,
    },
    event::{
        AdmissionDisposition, AdmissionReceipt, AdmissionStage, EventAdmission, EventPage,
        EventPosition, EventQuery, EventQueryBounds, EventSequence, SourceGeneration,
        StoredEventProvenance, StoredRawEvent, StoredVerifiedEvent, StoredVisibleEvent,
    },
    journal::{
        IdempotencyKey, JournalStage, JournalTransition, OperationInstanceId, OperationRecord,
        PrepareDisposition, PrepareOperation, PrepareReceipt, RECOVERABLE_QUERY_LIMIT_MAX,
    },
    outbox::{
        ClaimOutboxItems, ClaimedOutboxItem, DeliveryAttemptEvidence, EnqueueDisposition,
        EnqueueOutboxItem, EnqueueReceipt, LeaseId, OutboxItemId, OutboxLease, OutboxRecord,
        OutboxRevision, OutboxStage, OutboxStatus,
    },
    private_artifact::{
        DeletionReason, EXPIRED_ARTIFACT_QUERY_LIMIT_MAX, PrivateArtifactId,
        PrivateArtifactMetadata, PrivateArtifactResealReceipt, PrivateArtifactResealRequest,
        PrivateArtifactRevision, PrivateArtifactStage, PrivateArtifactStatus, PrivateArtifactStore,
    },
    projection::{
        EventIndexCheckpoint, EventIndexManifest, ProjectionCheckpoint, ProjectionGeneration,
        ProjectionHealth, ProjectionId, ProjectionInvalidation, ProjectionStatus, RebuildStage,
        RebuildTicket, RebuildTicketId, RebuildTransition,
    },
    status::{
        EventStoreHealth, EventStoreMode, EventStoreStatus, IntegrityHealth, IntegrityStatus,
        ShutdownState, StorageBackend, StorageOpenMode, StorageStatus, StorageStatusProvider,
        WriterPolicy,
    },
};

#[derive(Clone)]
struct EventEntry {
    position: EventPosition,
    admission: EventAdmission,
    provenance: Vec<EventProvenance>,
}

#[derive(Clone, Default)]
struct State {
    events: Vec<EventEntry>,
    journal: Vec<OperationRecord>,
    outbox: Vec<OutboxRecord>,
    projections: Vec<ProjectionStatus>,
    projection_invalidations: Vec<ProjectionInvalidation>,
    rebuilds: Vec<RebuildTicket>,
    event_index_manifests: Vec<EventIndexManifest>,
    event_index_checkpoints: Vec<EventIndexCheckpoint>,
    private_artifacts: Vec<PrivateArtifactMetadata>,
    private_artifact_reseals: Vec<PrivateArtifactResealReceipt>,
    backups: Vec<BackupOperation>,
    restores: Vec<RestoreOperation>,
    atomic_receipts: Vec<AtomicCommitReceipt>,
    authored_operations: Vec<crate::authored::AuthoredOperation>,
    authored_artifacts: Vec<crate::authored::AuthoredArtifact>,
    authored_delivery_plans: Vec<crate::authored_delivery::AuthoredDeliveryPlan>,
    authored_atomic_receipts: Vec<AuthoredAtomicReceipt>,
    closed: bool,
}

/// Bounded deterministic reference backend with no hidden tasks or globals.
pub struct MemoryStorage {
    generation: SourceGeneration,
    state: Mutex<State>,
}

impl MemoryStorage {
    pub const fn new(generation: SourceGeneration) -> Self {
        Self {
            generation,
            state: Mutex::new(State {
                events: Vec::new(),
                journal: Vec::new(),
                outbox: Vec::new(),
                projections: Vec::new(),
                projection_invalidations: Vec::new(),
                rebuilds: Vec::new(),
                event_index_manifests: Vec::new(),
                event_index_checkpoints: Vec::new(),
                private_artifacts: Vec::new(),
                private_artifact_reseals: Vec::new(),
                backups: Vec::new(),
                restores: Vec::new(),
                atomic_receipts: Vec::new(),
                authored_operations: Vec::new(),
                authored_artifacts: Vec::new(),
                authored_delivery_plans: Vec::new(),
                authored_atomic_receipts: Vec::new(),
                closed: false,
            }),
        }
    }

    pub const fn generation(&self) -> SourceGeneration {
        self.generation
    }

    fn state(&self) -> Result<MutexGuard<'_, State>, Error> {
        let state = self.state_any()?;
        if state.closed {
            return Err(Error::BackendUnavailable);
        }
        Ok(state)
    }

    fn state_any(&self) -> Result<MutexGuard<'_, State>, Error> {
        self.state.lock().map_err(|_| Error::BackendUnavailable)
    }

    fn selected(&self, state: &State, query: &EventQuery) -> Result<Vec<EventEntry>, Error> {
        if query
            .bounds()
            .cursor()
            .is_some_and(|cursor| cursor.generation() != self.generation)
        {
            return Err(Error::SourceGenerationChanged);
        }
        let after = query
            .bounds()
            .cursor()
            .map_or(0, |cursor| cursor.sequence().get());
        Ok(state
            .events
            .iter()
            .filter(|entry| {
                entry.position.sequence().get() > after && query.selects(entry.admission.event_id())
            })
            .take(usize::from(query.bounds().limit()))
            .cloned()
            .collect())
    }

    fn admit_locked(
        &self,
        state: &mut State,
        admission: EventAdmission,
    ) -> Result<AdmissionReceipt, Error> {
        if let Some(entry) = state
            .events
            .iter_mut()
            .find(|entry| entry.admission.event_id() == admission.event_id())
        {
            if entry.admission.event() != admission.event() {
                return Err(Error::EventConflict);
            }
            if admission.stage() < entry.admission.stage() {
                return Err(Error::AdmissionRegression);
            }
            let disposition = if admission.stage() == entry.admission.stage() {
                AdmissionDisposition::Duplicate
            } else {
                AdmissionDisposition::Advanced
            };
            if !entry.provenance.contains(admission.provenance()) {
                entry.provenance.push(admission.provenance().clone());
            }
            entry.admission = admission;
            return Ok(AdmissionReceipt::new(
                *entry.admission.event_id(),
                entry.position,
                entry.admission.stage(),
                disposition,
            ));
        }
        let next = u64::try_from(state.events.len())
            .map_err(|_| Error::CorruptStoredEvent)?
            .checked_add(1)
            .ok_or(Error::CorruptStoredEvent)?;
        let position = EventPosition::new(self.generation, EventSequence::new(next)?);
        let receipt = AdmissionReceipt::new(
            *admission.event_id(),
            position,
            admission.stage(),
            AdmissionDisposition::Inserted,
        );
        let provenance = vec![admission.provenance().clone()];
        state.events.push(EventEntry {
            position,
            admission,
            provenance,
        });
        Ok(receipt)
    }

    fn prepare_locked(
        state: &mut State,
        operation: PrepareOperation,
    ) -> Result<PrepareReceipt, Error> {
        if let Some(record) = state
            .journal
            .iter()
            .find(|record| record.idempotency_key() == operation.idempotency_key())
        {
            if record.operation_id() != operation.operation_id()
                || record.input_digest() != operation.input_digest()
                || record.instance_id() != operation.instance_id()
            {
                return Err(Error::IdempotencyConflict);
            }
            return Ok(PrepareReceipt::new(
                PrepareDisposition::Replay,
                record.clone(),
            ));
        }
        if state
            .journal
            .iter()
            .any(|record| record.instance_id() == operation.instance_id())
        {
            return Err(Error::OperationIdentityMismatch);
        }
        let record = operation.into_record()?;
        state.journal.push(record.clone());
        Ok(PrepareReceipt::new(PrepareDisposition::Created, record))
    }

    fn transition_locked(
        state: &mut State,
        transition: JournalTransition,
    ) -> Result<OperationRecord, Error> {
        let record = state
            .journal
            .iter_mut()
            .find(|record| record.instance_id() == transition.instance_id())
            .ok_or(Error::OperationNotFound)?;
        let next = record.transition(&transition)?;
        *record = next.clone();
        Ok(next)
    }

    fn enqueue_locked(state: &mut State, item: EnqueueOutboxItem) -> Result<EnqueueReceipt, Error> {
        if let Some(record) = state
            .outbox
            .iter()
            .find(|record| record.item_id() == item.item_id())
        {
            let candidate = item.into_record();
            if record.operation_instance_id() != candidate.operation_instance_id()
                || record.plan_digest() != candidate.plan_digest()
                || record.request() != candidate.request()
                || record.created_at_unix_ms() != candidate.created_at_unix_ms()
            {
                return Err(Error::OutboxPlanConflict);
            }
            return Ok(EnqueueReceipt::new(
                EnqueueDisposition::Replay,
                record.clone(),
            ));
        }
        if state.outbox.iter().any(|record| {
            record.operation_instance_id() == item.operation_instance_id()
                && record.item_id() != item.item_id()
        }) {
            return Err(Error::OutboxPlanConflict);
        }
        let record = item.into_record();
        state.outbox.push(record.clone());
        Ok(EnqueueReceipt::new(EnqueueDisposition::Created, record))
    }

    fn checkpoint_locked(
        state: &mut State,
        checkpoint: ProjectionCheckpoint,
    ) -> Result<ProjectionStatus, Error> {
        if let Some(status) = state
            .projections
            .iter_mut()
            .find(|status| status.projection_id() == checkpoint.projection_id())
        {
            if status.generation() != checkpoint.generation() {
                return Err(Error::ProjectionCheckpointMismatch);
            }
            if status
                .checkpoint()
                .is_some_and(|prior| !checkpoint.advances(prior))
            {
                return Err(Error::ProjectionCheckpointRegression);
            }
            let next = ProjectionStatus::new(
                checkpoint.projection_id().clone(),
                checkpoint.generation(),
                ProjectionHealth::Ready,
                Some(checkpoint),
                None,
            )?;
            *status = next.clone();
            return Ok(next);
        }
        let status = ProjectionStatus::new(
            checkpoint.projection_id().clone(),
            checkpoint.generation(),
            ProjectionHealth::Ready,
            Some(checkpoint),
            None,
        )?;
        state.projections.push(status.clone());
        Ok(status)
    }

    fn integrity_locked(state: &State) -> Result<IntegrityStatus, Error> {
        let members = state
            .events
            .len()
            .checked_add(state.journal.len())
            .and_then(|count| count.checked_add(state.outbox.len()))
            .and_then(|count| count.checked_add(state.projections.len()))
            .and_then(|count| count.checked_add(state.private_artifacts.len()))
            .ok_or(Error::InvalidIntegrityStatus)?;
        IntegrityStatus::new(
            IntegrityHealth::Healthy,
            None,
            u32::try_from(members).map_err(|_| Error::InvalidIntegrityStatus)?,
            0,
        )
    }

    fn status_locked(state: &State) -> Result<StorageStatus, Error> {
        StorageStatus::new(
            StorageBackend::Memory,
            StorageOpenMode::Create,
            WriterPolicy::NoWriter,
            if state.closed {
                ShutdownState::Closed
            } else {
                ShutdownState::Open
            },
            Self::integrity_locked(state)?,
            false,
            0,
        )
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new(SourceGeneration::new([1; 32]).expect("fixed non-zero memory generation"))
    }
}

impl EventStore for MemoryStorage {
    fn status(&self) -> BoxFuture<'_, Result<EventStoreStatus, Error>> {
        Box::pin(async move {
            let state = self.state()?;
            let raw = u64::try_from(state.events.len()).map_err(|_| Error::CorruptStoredEvent)?;
            let verified = u64::try_from(
                state
                    .events
                    .iter()
                    .filter(|entry| entry.admission.stage() >= AdmissionStage::Verified)
                    .count(),
            )
            .map_err(|_| Error::CorruptStoredEvent)?;
            let visible = u64::try_from(
                state
                    .events
                    .iter()
                    .filter(|entry| entry.admission.stage() == AdmissionStage::Visible)
                    .count(),
            )
            .map_err(|_| Error::CorruptStoredEvent)?;
            EventStoreStatus::new(
                self.generation,
                EventStoreMode::ReadWrite,
                EventStoreHealth::Available,
                raw,
                verified,
                visible,
            )
        })
    }

    fn admit(&self, admission: EventAdmission) -> BoxFuture<'_, Result<AdmissionReceipt, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            self.admit_locked(&mut state, admission)
        })
    }

    fn query_raw(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredRawEvent>, Error>> {
        Box::pin(async move {
            let state = self.state()?;
            let items = self
                .selected(&state, &query)?
                .into_iter()
                .map(|entry| {
                    StoredRawEvent::new(
                        entry.position,
                        entry.admission.event().clone(),
                        entry.admission.stage(),
                    )
                })
                .collect();
            EventPage::new(self.generation, items, None, query.bounds())
        })
    }

    fn query_verified(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredVerifiedEvent>, Error>> {
        Box::pin(async move {
            let state = self.state()?;
            let items = self
                .selected(&state, &query)?
                .into_iter()
                .filter_map(|entry| {
                    (entry.admission.stage() >= AdmissionStage::Verified).then(|| {
                        StoredVerifiedEvent::new(entry.position, entry.admission.event().clone())
                    })
                })
                .collect();
            EventPage::new(self.generation, items, None, query.bounds())
        })
    }

    fn query_visible(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredVisibleEvent>, Error>> {
        Box::pin(async move {
            let state = self.state()?;
            let items = self
                .selected(&state, &query)?
                .into_iter()
                .filter_map(|entry| {
                    (entry.admission.stage() == AdmissionStage::Visible).then(|| {
                        StoredVisibleEvent::new(entry.position, entry.admission.event().clone())
                    })
                })
                .collect();
            EventPage::new(self.generation, items, None, query.bounds())
        })
    }

    fn query_provenance(
        &self,
        event_id: EventId,
        bounds: EventQueryBounds,
    ) -> BoxFuture<'_, Result<EventPage<StoredEventProvenance>, Error>> {
        Box::pin(async move {
            if bounds
                .cursor()
                .is_some_and(|cursor| cursor.generation() != self.generation)
            {
                return Err(Error::SourceGenerationChanged);
            }
            let state = self.state()?;
            let entry = state
                .events
                .iter()
                .find(|entry| entry.admission.event_id() == &event_id)
                .ok_or(Error::EventNotFound)?;
            let after = bounds.cursor().map_or(0, |cursor| cursor.sequence().get());
            let items = if entry.position.sequence().get() > after {
                entry
                    .provenance
                    .iter()
                    .take(usize::from(bounds.limit()))
                    .cloned()
                    .map(|provenance| StoredEventProvenance::new(entry.position, provenance))
                    .collect()
            } else {
                Vec::new()
            };
            EventPage::new(self.generation, items, None, bounds)
        })
    }
}

impl Journal for MemoryStorage {
    fn prepare(&self, operation: PrepareOperation) -> BoxFuture<'_, Result<PrepareReceipt, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            Self::prepare_locked(&mut state, operation)
        })
    }

    fn operation(
        &self,
        instance_id: OperationInstanceId,
    ) -> BoxFuture<'_, Result<Option<OperationRecord>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .journal
                .iter()
                .find(|record| record.instance_id() == instance_id)
                .cloned())
        })
    }

    fn by_idempotency_key(
        &self,
        operation_id: OperationId,
        idempotency_key: IdempotencyKey,
    ) -> BoxFuture<'_, Result<Option<OperationRecord>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .journal
                .iter()
                .find(|record| {
                    record.operation_id() == operation_id
                        && record.idempotency_key() == &idempotency_key
                })
                .cloned())
        })
    }

    fn transition(
        &self,
        transition: JournalTransition,
    ) -> BoxFuture<'_, Result<OperationRecord, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            Self::transition_locked(&mut state, transition)
        })
    }

    fn recoverable(&self, limit: u16) -> BoxFuture<'_, Result<Vec<OperationRecord>, Error>> {
        Box::pin(async move {
            if limit == 0 || limit > RECOVERABLE_QUERY_LIMIT_MAX {
                return Err(Error::InvalidJournalQueryLimit);
            }
            Ok(self
                .state()?
                .journal
                .iter()
                .filter(|record| record.state().stage() == JournalStage::Recoverable)
                .take(usize::from(limit))
                .cloned()
                .collect())
        })
    }
}

impl Outbox for MemoryStorage {
    fn enqueue(&self, item: EnqueueOutboxItem) -> BoxFuture<'_, Result<EnqueueReceipt, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            Self::enqueue_locked(&mut state, item)
        })
    }

    fn item(&self, item_id: OutboxItemId) -> BoxFuture<'_, Result<Option<OutboxRecord>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .outbox
                .iter()
                .find(|record| record.item_id() == item_id)
                .cloned())
        })
    }

    fn claim(
        &self,
        request: ClaimOutboxItems,
    ) -> BoxFuture<'_, Result<Vec<ClaimedOutboxItem>, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            let mut claimed = Vec::new();
            for record in &mut state.outbox {
                if claimed.len() >= usize::from(request.limit()) || record.stage().is_terminal() {
                    continue;
                }
                if record
                    .retry_not_before_unix_ms()
                    .is_some_and(|at| request.now_unix_ms() < at)
                    || record
                        .lease()
                        .is_some_and(|lease| lease.is_active_at(request.now_unix_ms()))
                {
                    continue;
                }
                let lease = OutboxLease::new(
                    request.lease_id_for(record.item_id()),
                    request.owner().clone(),
                    request.now_unix_ms(),
                    request.lease_expires_at_unix_ms(),
                )?;
                record.claim(lease.clone())?;
                claimed.push(ClaimedOutboxItem::new(record.clone(), lease));
            }
            Ok(claimed)
        })
    }

    fn record_attempt(
        &self,
        evidence: DeliveryAttemptEvidence,
    ) -> BoxFuture<'_, Result<OutboxRecord, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            let record = state
                .outbox
                .iter_mut()
                .find(|record| record.item_id() == evidence.item_id())
                .ok_or(Error::OutboxItemNotFound)?;
            record.record_attempt(evidence)?;
            Ok(record.clone())
        })
    }

    fn release(
        &self,
        item_id: OutboxItemId,
        lease_id: LeaseId,
        expected_revision: OutboxRevision,
        released_at_unix_ms: u64,
        retry_not_before_unix_ms: Option<u64>,
    ) -> BoxFuture<'_, Result<OutboxRecord, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            let record = state
                .outbox
                .iter_mut()
                .find(|record| record.item_id() == item_id)
                .ok_or(Error::OutboxItemNotFound)?;
            record.release(
                lease_id,
                expected_revision,
                released_at_unix_ms,
                retry_not_before_unix_ms,
            )?;
            Ok(record.clone())
        })
    }

    fn status(&self) -> BoxFuture<'_, Result<OutboxStatus, Error>> {
        Box::pin(async move {
            let state = self.state()?;
            let mut status = OutboxStatus {
                pending: 0,
                leased: 0,
                retryable: 0,
                satisfied: 0,
                exhausted: 0,
            };
            for record in &state.outbox {
                let count = match record.stage() {
                    OutboxStage::Pending => &mut status.pending,
                    OutboxStage::Leased => &mut status.leased,
                    OutboxStage::Retryable => &mut status.retryable,
                    OutboxStage::Satisfied => &mut status.satisfied,
                    OutboxStage::Exhausted => &mut status.exhausted,
                };
                *count = count.checked_add(1).ok_or(Error::CorruptOutboxRecord)?;
            }
            Ok(status)
        })
    }
}

impl ProjectionStore for MemoryStorage {
    fn status(
        &self,
        projection_id: ProjectionId,
    ) -> BoxFuture<'_, Result<Option<ProjectionStatus>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .projections
                .iter()
                .find(|status| status.projection_id() == &projection_id)
                .cloned())
        })
    }

    fn checkpoint(
        &self,
        checkpoint: ProjectionCheckpoint,
    ) -> BoxFuture<'_, Result<ProjectionStatus, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            Self::checkpoint_locked(&mut state, checkpoint)
        })
    }

    fn invalidate(
        &self,
        invalidation: ProjectionInvalidation,
    ) -> BoxFuture<'_, Result<ProjectionStatus, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            let status_index = state
                .projections
                .iter()
                .position(|status| status.projection_id() == invalidation.projection_id())
                .ok_or(Error::ProjectionCheckpointMismatch)?;
            if state.projections[status_index].generation() != invalidation.invalid_generation() {
                return Err(Error::ProjectionCheckpointMismatch);
            }
            if let Some(existing) = state.projection_invalidations.iter().find(|existing| {
                existing.projection_id() == invalidation.projection_id()
                    && existing.invalid_generation() == invalidation.invalid_generation()
            }) && existing != &invalidation
            {
                return Err(Error::ProjectionRevisionConflict);
            }
            let next = ProjectionStatus::new(
                invalidation.projection_id().clone(),
                state.projections[status_index].generation(),
                ProjectionHealth::Invalidated,
                state.projections[status_index].checkpoint().cloned(),
                None,
            )?;
            if !state
                .projection_invalidations
                .iter()
                .any(|existing| existing == &invalidation)
            {
                state.projection_invalidations.push(invalidation);
            }
            state.projections[status_index] = next.clone();
            Ok(next)
        })
    }

    fn invalidation(
        &self,
        projection_id: ProjectionId,
        replacement_generation: ProjectionGeneration,
    ) -> BoxFuture<'_, Result<Option<ProjectionInvalidation>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .projection_invalidations
                .iter()
                .rev()
                .find(|invalidation| {
                    invalidation.projection_id() == &projection_id
                        && invalidation.replacement_generation() == replacement_generation
                })
                .cloned())
        })
    }

    fn request_rebuild(
        &self,
        ticket: RebuildTicket,
    ) -> BoxFuture<'_, Result<RebuildTicket, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            if let Some(existing) = state
                .rebuilds
                .iter()
                .find(|existing| existing.ticket_id() == ticket.ticket_id())
            {
                return if existing == &ticket {
                    Ok(existing.clone())
                } else {
                    Err(Error::ProjectionRevisionConflict)
                };
            }
            let projection_id = ticket.invalidation().projection_id();
            let has_invalidation = state
                .projection_invalidations
                .iter()
                .any(|invalidation| invalidation == ticket.invalidation());
            let status = state
                .projections
                .iter_mut()
                .find(|status| status.projection_id() == projection_id)
                .ok_or(Error::ProjectionCheckpointMismatch)?;
            if status.generation() != ticket.invalidation().invalid_generation()
                || status.health() != ProjectionHealth::Invalidated
                || !has_invalidation
            {
                return Err(Error::ProjectionCheckpointMismatch);
            }
            *status = ProjectionStatus::new(
                projection_id.clone(),
                status.generation(),
                ProjectionHealth::Rebuilding,
                status.checkpoint().cloned(),
                Some(ticket.ticket_id()),
            )?;
            state.rebuilds.push(ticket.clone());
            Ok(ticket)
        })
    }

    fn rebuild(
        &self,
        ticket_id: RebuildTicketId,
    ) -> BoxFuture<'_, Result<Option<RebuildTicket>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .rebuilds
                .iter()
                .find(|ticket| ticket.ticket_id() == ticket_id)
                .cloned())
        })
    }

    fn transition_rebuild(
        &self,
        transition: RebuildTransition,
    ) -> BoxFuture<'_, Result<RebuildTicket, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            let index = state
                .rebuilds
                .iter()
                .position(|ticket| ticket.ticket_id() == transition.ticket_id())
                .ok_or(Error::ProjectionRevisionConflict)?;
            let next = state.rebuilds[index].transition(transition)?;
            if next.stage() == RebuildStage::Completed
                && (next.source_generation() != self.generation
                    || next
                        .source_high_water()
                        .map_or(0, |position| position.sequence().get())
                        != u64::try_from(state.events.len())
                            .map_err(|_| Error::CorruptProjectionRecord)?)
            {
                return Err(Error::SourceGenerationChanged);
            }
            let projection_id = next.invalidation().projection_id();
            let status = state
                .projections
                .iter_mut()
                .find(|status| status.projection_id() == projection_id)
                .ok_or(Error::CorruptProjectionRecord)?;
            if status.generation() != next.invalidation().invalid_generation()
                || status.health() != ProjectionHealth::Rebuilding
                || status.active_rebuild() != Some(next.ticket_id())
            {
                return Err(Error::CorruptProjectionRecord);
            }
            let (generation, health, checkpoint, active_rebuild) = match next.stage() {
                RebuildStage::Requested | RebuildStage::Running => (
                    status.generation(),
                    ProjectionHealth::Rebuilding,
                    status.checkpoint().cloned(),
                    Some(next.ticket_id()),
                ),
                RebuildStage::Completed => (
                    next.invalidation().replacement_generation(),
                    ProjectionHealth::Ready,
                    next.checkpoint().cloned(),
                    None,
                ),
                RebuildStage::Failed => (
                    status.generation(),
                    ProjectionHealth::Ready,
                    status.checkpoint().cloned(),
                    None,
                ),
            };
            *status = ProjectionStatus::new(
                projection_id.clone(),
                generation,
                health,
                checkpoint,
                active_rebuild,
            )?;
            state.rebuilds[index] = next.clone();
            Ok(next)
        })
    }

    fn event_index_manifest(
        &self,
        generation: ProjectionGeneration,
    ) -> BoxFuture<'_, Result<Option<EventIndexManifest>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .event_index_manifests
                .iter()
                .find(|manifest| manifest.generation() == generation)
                .cloned())
        })
    }

    fn put_event_index_manifest(
        &self,
        manifest: EventIndexManifest,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            if let Some(existing) = state
                .event_index_manifests
                .iter()
                .find(|existing| existing.generation() == manifest.generation())
            {
                return if existing == &manifest {
                    Ok(())
                } else {
                    Err(Error::CorruptProjectionRecord)
                };
            }
            state.event_index_manifests.push(manifest);
            Ok(())
        })
    }

    fn event_index_checkpoint(
        &self,
        generation: ProjectionGeneration,
    ) -> BoxFuture<'_, Result<Option<EventIndexCheckpoint>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .event_index_checkpoints
                .iter()
                .find(|checkpoint| checkpoint.generation() == generation)
                .cloned())
        })
    }

    fn put_event_index_checkpoint(
        &self,
        checkpoint: EventIndexCheckpoint,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            if let Some(existing) = state
                .event_index_checkpoints
                .iter_mut()
                .find(|existing| existing.generation() == checkpoint.generation())
            {
                if checkpoint.generated_at_unix_ms() < existing.generated_at_unix_ms() {
                    return Err(Error::InvalidEventIndexCheckpoint);
                }
                *existing = checkpoint;
            } else {
                state.event_index_checkpoints.push(checkpoint);
            }
            Ok(())
        })
    }
}

impl PrivateArtifactStore for MemoryStorage {
    fn put_metadata(
        &self,
        metadata: PrivateArtifactMetadata,
    ) -> BoxFuture<'_, Result<PrivateArtifactMetadata, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            if let Some(existing) = state
                .private_artifacts
                .iter()
                .find(|existing| existing.artifact_id() == metadata.artifact_id())
            {
                return if existing == &metadata {
                    Ok(existing.clone())
                } else {
                    Err(Error::PrivateArtifactConflict)
                };
            }
            state.private_artifacts.push(metadata.clone());
            Ok(metadata)
        })
    }

    fn metadata(
        &self,
        artifact_id: PrivateArtifactId,
    ) -> BoxFuture<'_, Result<Option<PrivateArtifactMetadata>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .private_artifacts
                .iter()
                .find(|metadata| metadata.artifact_id() == artifact_id)
                .cloned())
        })
    }

    fn reseal_metadata(
        &self,
        request: PrivateArtifactResealRequest,
    ) -> BoxFuture<'_, Result<PrivateArtifactResealReceipt, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            if let Some(receipt) = state
                .private_artifact_reseals
                .iter()
                .find(|receipt| receipt.reseal_id() == request.reseal_id())
            {
                return receipt.replay(&request);
            }
            let metadata = state
                .private_artifacts
                .iter_mut()
                .find(|metadata| metadata.artifact_id() == request.artifact_id())
                .ok_or(Error::PrivateArtifactNotFound)?;
            let next = metadata.resealed(&request)?;
            let receipt = PrivateArtifactResealReceipt::committed(&request, next.revision());
            *metadata = next;
            state.private_artifact_reseals.push(receipt);
            Ok(receipt)
        })
    }

    fn mark_expired(
        &self,
        artifact_id: PrivateArtifactId,
        expected_revision: PrivateArtifactRevision,
        at_unix_ms: u64,
    ) -> BoxFuture<'_, Result<PrivateArtifactMetadata, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            let metadata = state
                .private_artifacts
                .iter_mut()
                .find(|metadata| metadata.artifact_id() == artifact_id)
                .ok_or(Error::PrivateArtifactNotFound)?;
            let next = metadata.mark_expired(expected_revision, at_unix_ms)?;
            *metadata = next.clone();
            Ok(next)
        })
    }

    fn tombstone(
        &self,
        artifact_id: PrivateArtifactId,
        expected_revision: PrivateArtifactRevision,
        at_unix_ms: u64,
        reason: DeletionReason,
    ) -> BoxFuture<'_, Result<PrivateArtifactMetadata, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            let metadata = state
                .private_artifacts
                .iter_mut()
                .find(|metadata| metadata.artifact_id() == artifact_id)
                .ok_or(Error::PrivateArtifactNotFound)?;
            let next = metadata.tombstone(expected_revision, at_unix_ms, reason)?;
            *metadata = next.clone();
            Ok(next)
        })
    }

    fn expired(
        &self,
        at_unix_ms: u64,
        limit: u16,
    ) -> BoxFuture<'_, Result<Vec<PrivateArtifactMetadata>, Error>> {
        Box::pin(async move {
            if at_unix_ms == 0 || limit == 0 || limit > EXPIRED_ARTIFACT_QUERY_LIMIT_MAX {
                return Err(Error::InvalidExpiredArtifactQueryLimit);
            }
            Ok(self
                .state()?
                .private_artifacts
                .iter()
                .filter(|metadata| {
                    metadata.stage() == PrivateArtifactStage::Active
                        && metadata.retention().is_expired_at(at_unix_ms)
                })
                .take(usize::from(limit))
                .cloned()
                .collect())
        })
    }

    fn status(&self) -> BoxFuture<'_, Result<PrivateArtifactStatus, Error>> {
        Box::pin(async move {
            let state = self.state()?;
            let mut status = PrivateArtifactStatus {
                active: 0,
                expired: 0,
                tombstoned: 0,
            };
            for metadata in &state.private_artifacts {
                let count = match metadata.stage() {
                    PrivateArtifactStage::Active => &mut status.active,
                    PrivateArtifactStage::Expired => &mut status.expired,
                    PrivateArtifactStage::Tombstoned => &mut status.tombstoned,
                };
                *count = count
                    .checked_add(1)
                    .ok_or(Error::CorruptPrivateArtifactMetadata)?;
            }
            Ok(status)
        })
    }
}

impl StorageReliability for MemoryStorage {
    fn begin_backup(&self, plan: BackupPlan) -> BoxFuture<'_, Result<BackupOperation, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            if let Some(existing) = state
                .backups
                .iter()
                .find(|operation| operation.plan().backup_id() == plan.backup_id())
            {
                return if existing.plan() == &plan {
                    Ok(existing.clone())
                } else {
                    Err(Error::ReliabilityRevisionConflict)
                };
            }
            let operation = BackupOperation::planned(plan);
            state.backups.push(operation.clone());
            Ok(operation)
        })
    }

    fn transition_backup(
        &self,
        backup_id: BackupId,
        expected_revision: ReliabilityRevision,
        transition: BackupTransition,
        at_unix_ms: u64,
    ) -> BoxFuture<'_, Result<BackupOperation, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            let operation = state
                .backups
                .iter_mut()
                .find(|operation| operation.plan().backup_id() == backup_id)
                .ok_or(Error::CorruptReliabilityOperation)?;
            let next = operation.transition(expected_revision, transition, at_unix_ms)?;
            *operation = next.clone();
            Ok(next)
        })
    }

    fn begin_restore(&self, plan: RestorePlan) -> BoxFuture<'_, Result<RestoreOperation, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            let backup_id = plan.manifest().backup_id();
            if let Some(existing) = state
                .restores
                .iter()
                .find(|operation| operation.plan().manifest().backup_id() == backup_id)
            {
                return if existing.plan() == &plan {
                    Ok(existing.clone())
                } else {
                    Err(Error::ReliabilityRevisionConflict)
                };
            }
            let operation = RestoreOperation::staging(plan);
            state.restores.push(operation.clone());
            Ok(operation)
        })
    }

    fn transition_restore(
        &self,
        backup_id: BackupId,
        expected_revision: ReliabilityRevision,
        transition: RestoreTransition,
        at_unix_ms: u64,
    ) -> BoxFuture<'_, Result<RestoreOperation, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            let operation = state
                .restores
                .iter_mut()
                .find(|operation| operation.plan().manifest().backup_id() == backup_id)
                .ok_or(Error::CorruptReliabilityOperation)?;
            let next = operation.transition(expected_revision, transition, at_unix_ms)?;
            *operation = next.clone();
            Ok(next)
        })
    }

    fn integrity(&self) -> BoxFuture<'_, Result<IntegrityStatus, Error>> {
        Box::pin(async move {
            let state = self.state_any()?;
            Self::integrity_locked(&state)
        })
    }

    fn status(&self) -> BoxFuture<'_, Result<StorageStatus, Error>> {
        Box::pin(async move {
            let state = self.state_any()?;
            Self::status_locked(&state)
        })
    }

    fn close(&self) -> BoxFuture<'_, Result<StorageStatus, Error>> {
        Box::pin(async move {
            let mut state = self.state_any()?;
            state.closed = true;
            Self::status_locked(&state)
        })
    }
}

impl StorageStatusProvider for MemoryStorage {
    fn storage_status(&self) -> BoxFuture<'_, Result<StorageStatus, Error>> {
        Box::pin(async move {
            let state = self.state_any()?;
            Self::status_locked(&state)
        })
    }
}

impl AtomicStorage for MemoryStorage {
    fn commit(&self, request: AtomicCommit) -> BoxFuture<'_, Result<AtomicCommitReceipt, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            if let Some(existing) = state
                .atomic_receipts
                .iter()
                .find(|receipt| receipt.commit_id() == request.commit_id())
            {
                if existing.digest() != request.digest()
                    || existing.outcome().kind() != request.workflow().kind()
                {
                    return Err(Error::AtomicCommitConflict);
                }
                return AtomicCommitReceipt::new(
                    &request,
                    AtomicCommitDisposition::Replay,
                    existing.committed_at_unix_ms(),
                    existing.outcome().clone(),
                );
            }
            let mut candidate = state.clone();
            let outcome = match request.workflow().clone() {
                AtomicWorkflow::Ingested(ingested) => {
                    let admission =
                        self.admit_locked(&mut candidate, ingested.admission().clone())?;
                    let projection = ingested
                        .projection()
                        .cloned()
                        .map(|checkpoint| Self::checkpoint_locked(&mut candidate, checkpoint))
                        .transpose()?
                        .map(Box::new);
                    AtomicCommitOutcome::Ingested {
                        admission,
                        projection,
                    }
                }
            };
            let receipt = AtomicCommitReceipt::new(
                &request,
                AtomicCommitDisposition::Committed,
                request.requested_at_unix_ms(),
                outcome,
            )?;
            candidate.atomic_receipts.push(receipt.clone());
            *state = candidate;
            Ok(receipt)
        })
    }

    fn receipt(
        &self,
        commit_id: AtomicCommitId,
    ) -> BoxFuture<'_, Result<Option<AtomicCommitReceipt>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .atomic_receipts
                .iter()
                .find(|receipt| receipt.commit_id() == commit_id)
                .cloned())
        })
    }
}

impl AuthoredAtomicStorage for MemoryStorage {
    fn execute_authored(
        &self,
        command: AuthoredAtomicCommand,
    ) -> BoxFuture<'_, Result<AuthoredAtomicReceipt, Error>> {
        Box::pin(async move {
            let mut state = self.state()?;
            if let Some(existing) = state
                .authored_atomic_receipts
                .iter()
                .find(|receipt| receipt.commit_id() == command.commit_id())
            {
                if existing.digest() != command.digest() {
                    return Err(Error::AtomicCommitConflict);
                }
                return AuthoredAtomicReceipt::from_durable_parts(
                    existing.commit_id(),
                    existing.digest(),
                    AtomicCommitDisposition::Replay,
                    existing.committed_at_unix_ms(),
                    existing.outcome().clone(),
                );
            }

            let mut candidate = state.clone();
            let outcome = match command.clone() {
                AuthoredAtomicCommand::Prepare(value) => {
                    if candidate.authored_operations.iter().any(|operation| {
                        operation.operation_id() == value.operation().operation_id()
                    }) || value.artifacts().iter().any(|artifact| {
                        candidate
                            .authored_artifacts
                            .iter()
                            .any(|existing| existing.artifact_id() == artifact.artifact_id())
                    }) || value.delivery_plans().iter().any(|plan| {
                        candidate
                            .authored_delivery_plans
                            .iter()
                            .any(|existing| existing.plan_id() == plan.plan_id())
                    }) {
                        return Err(Error::AtomicCommitConflict);
                    }
                    candidate
                        .authored_operations
                        .push(value.operation().clone());
                    candidate
                        .authored_artifacts
                        .extend(value.artifacts().iter().cloned());
                    candidate
                        .authored_delivery_plans
                        .extend(value.delivery_plans().iter().cloned());
                    AuthoredAtomicOutcome::Prepared {
                        operation: value.operation().clone(),
                        artifacts: value.artifacts().to_vec(),
                        delivery_plans: value.delivery_plans().to_vec(),
                    }
                }
                AuthoredAtomicCommand::Claim(value) => match value.target() {
                    ClaimAuthoredTarget::ArtifactSigning(artifact_id) => {
                        let artifact = candidate
                            .authored_artifacts
                            .iter_mut()
                            .find(|artifact| artifact.artifact_id() == *artifact_id)
                            .ok_or(Error::InvalidAuthoredArtifact)?;
                        artifact.set_signing_claim(
                            value.claim().clone(),
                            value.claim().acquired_at_unix_ms(),
                        )?;
                        AuthoredAtomicOutcome::Artifact(artifact.clone())
                    }
                    ClaimAuthoredTarget::ArtifactAdmission(artifact_id) => {
                        let artifact = candidate
                            .authored_artifacts
                            .iter_mut()
                            .find(|artifact| artifact.artifact_id() == *artifact_id)
                            .ok_or(Error::InvalidAuthoredArtifact)?;
                        artifact.set_admission_claim(
                            value.claim().clone(),
                            value.claim().acquired_at_unix_ms(),
                        )?;
                        AuthoredAtomicOutcome::Artifact(artifact.clone())
                    }
                    ClaimAuthoredTarget::DeliveryPlan(plan_id) => {
                        let plan = candidate
                            .authored_delivery_plans
                            .iter_mut()
                            .find(|plan| plan.plan_id() == *plan_id)
                            .ok_or(Error::InvalidAuthoredDeliveryPlan)?;
                        plan.claim(value.claim().clone(), value.claim().acquired_at_unix_ms())?;
                        AuthoredAtomicOutcome::DeliveryPlan(plan.clone())
                    }
                },
                AuthoredAtomicCommand::ApplySigned(value) => {
                    let artifact = candidate
                        .authored_artifacts
                        .iter_mut()
                        .find(|artifact| artifact.artifact_id() == value.artifact_id())
                        .ok_or(Error::InvalidAuthoredArtifact)?;
                    require_artifact_claim(
                        artifact.signing_claim(),
                        value.fence(),
                        value.applied_at_unix_ms(),
                    )?;
                    artifact.record_signed(value.event().clone(), value.applied_at_unix_ms())?;
                    let artifact = artifact.clone();
                    for plan in candidate
                        .authored_delivery_plans
                        .iter_mut()
                        .filter(|plan| plan.artifact_id() == value.artifact_id())
                    {
                        plan.bind_signed_event(value.event().clone(), value.applied_at_unix_ms())?;
                    }
                    AuthoredAtomicOutcome::Artifact(artifact)
                }
                AuthoredAtomicCommand::ApplyAdmission(value) => {
                    let artifact = candidate
                        .authored_artifacts
                        .iter_mut()
                        .find(|artifact| artifact.artifact_id() == value.artifact_id())
                        .ok_or(Error::InvalidAuthoredArtifact)?;
                    require_artifact_claim(
                        artifact.admission_claim(),
                        value.fence(),
                        value.applied_at_unix_ms(),
                    )?;
                    artifact.record_admission(
                        value.state(),
                        value.failure().cloned(),
                        value.retry().cloned(),
                        value.applied_at_unix_ms(),
                    )?;
                    AuthoredAtomicOutcome::Artifact(artifact.clone())
                }
                AuthoredAtomicCommand::ApplyDelivery(value) => {
                    let plan = candidate
                        .authored_delivery_plans
                        .iter_mut()
                        .find(|plan| plan.plan_id() == value.plan_id())
                        .ok_or(Error::InvalidAuthoredDeliveryPlan)?;
                    match value.outcome().clone() {
                        DeliveryAttemptOutcome::Receipt(receipt) => plan.apply_receipt(
                            value.fence().token(),
                            value.fence().generation(),
                            value.fence().row_revision(),
                            receipt,
                            value.retry().cloned(),
                            value.applied_at_unix_ms(),
                        )?,
                        DeliveryAttemptOutcome::SinkFailure(failure) => plan.apply_sink_failure(
                            value.fence().token(),
                            value.fence().generation(),
                            value.fence().row_revision(),
                            failure,
                            value.retry().cloned(),
                            value.applied_at_unix_ms(),
                        )?,
                    }
                    AuthoredAtomicOutcome::DeliveryPlan(plan.clone())
                }
                AuthoredAtomicCommand::ApplyFailure(value) => match value.target() {
                    AuthoredWorkTarget::Artifact(artifact_id) => {
                        let artifact = candidate
                            .authored_artifacts
                            .iter_mut()
                            .find(|artifact| artifact.artifact_id() == *artifact_id)
                            .ok_or(Error::InvalidAuthoredArtifact)?;
                        match value.failure().phase() {
                            WorkPhase::Signing => {
                                require_artifact_claim(
                                    artifact.signing_claim(),
                                    value.fence(),
                                    value.applied_at_unix_ms(),
                                )?;
                                artifact.record_signing_failure(
                                    value.failure().clone(),
                                    value.retry().cloned(),
                                    value.applied_at_unix_ms(),
                                )?;
                            }
                            WorkPhase::Admission => {
                                require_artifact_claim(
                                    artifact.admission_claim(),
                                    value.fence(),
                                    value.applied_at_unix_ms(),
                                )?;
                                let state = match value.failure().class() {
                                    FailureClass::Retryable => AdmissionState::Retryable,
                                    FailureClass::Terminal => AdmissionState::Rejected,
                                    FailureClass::Indeterminate => {
                                        return Err(Error::InvalidAuthoredTransition);
                                    }
                                };
                                artifact.record_admission(
                                    state,
                                    Some(value.failure().clone()),
                                    value.retry().cloned(),
                                    value.applied_at_unix_ms(),
                                )?;
                            }
                            WorkPhase::Delivery => {
                                return Err(Error::AtomicWorkflowMismatch);
                            }
                        }
                        AuthoredAtomicOutcome::Artifact(artifact.clone())
                    }
                    AuthoredWorkTarget::DeliveryPlan(plan_id) => {
                        let plan = candidate
                            .authored_delivery_plans
                            .iter_mut()
                            .find(|plan| plan.plan_id() == *plan_id)
                            .ok_or(Error::InvalidAuthoredDeliveryPlan)?;
                        if value.failure().phase() != WorkPhase::Delivery
                            || value.failure().class() == FailureClass::Indeterminate
                        {
                            return Err(Error::AtomicWorkflowMismatch);
                        }
                        let retryability = match value.failure().class() {
                            FailureClass::Retryable => {
                                radroots_transport::outcome::Retryability::Retryable
                            }
                            FailureClass::Terminal => {
                                radroots_transport::outcome::Retryability::Terminal
                            }
                            FailureClass::Indeterminate => unreachable!(),
                        };
                        let failure = radroots_transport::SinkFailure::for_request(
                            plan.request().ok_or(Error::InvalidAuthoredDeliveryPlan)?,
                            value.failure().code(),
                            retryability,
                            value.failure().retry_after_unix_ms(),
                            value.failure().diagnostic().map(str::to_owned),
                            Vec::new(),
                        )
                        .map_err(|_| Error::AtomicWorkflowMismatch)?;
                        plan.apply_sink_failure(
                            value.fence().token(),
                            value.fence().generation(),
                            value.fence().row_revision(),
                            failure,
                            value.retry().cloned(),
                            value.applied_at_unix_ms(),
                        )?;
                        AuthoredAtomicOutcome::DeliveryPlan(plan.clone())
                    }
                },
                AuthoredAtomicCommand::Cancel(value) => match value.target() {
                    CancelAuthoredTarget::ArtifactSigning(artifact_id) => {
                        let artifact = candidate
                            .authored_artifacts
                            .iter_mut()
                            .find(|artifact| artifact.artifact_id() == *artifact_id)
                            .ok_or(Error::InvalidAuthoredArtifact)?;
                        if artifact.revision() != value.expected_revision() {
                            return Err(Error::InvalidAuthoredTransition);
                        }
                        artifact.cancel_signing(value.cancelled_at_unix_ms())?;
                        AuthoredAtomicOutcome::Artifact(artifact.clone())
                    }
                    CancelAuthoredTarget::ArtifactAdmission(artifact_id) => {
                        let artifact = candidate
                            .authored_artifacts
                            .iter_mut()
                            .find(|artifact| artifact.artifact_id() == *artifact_id)
                            .ok_or(Error::InvalidAuthoredArtifact)?;
                        if artifact.revision() != value.expected_revision() {
                            return Err(Error::InvalidAuthoredTransition);
                        }
                        let failure = WorkFailure::new(
                            "cancelled",
                            WorkPhase::Admission,
                            FailureClass::Terminal,
                            None,
                            None,
                        )?;
                        artifact.record_admission(
                            AdmissionState::Cancelled,
                            Some(failure),
                            None,
                            value.cancelled_at_unix_ms(),
                        )?;
                        AuthoredAtomicOutcome::Artifact(artifact.clone())
                    }
                    CancelAuthoredTarget::DeliveryPlan(plan_id) => {
                        let plan = candidate
                            .authored_delivery_plans
                            .iter_mut()
                            .find(|plan| plan.plan_id() == *plan_id)
                            .ok_or(Error::InvalidAuthoredDeliveryPlan)?;
                        if plan.revision() != value.expected_revision() {
                            return Err(Error::InvalidAuthoredDeliveryPlan);
                        }
                        plan.cancel(value.cancelled_at_unix_ms())?;
                        AuthoredAtomicOutcome::DeliveryPlan(plan.clone())
                    }
                },
            };
            let receipt = AuthoredAtomicReceipt::new(
                &command,
                AtomicCommitDisposition::Committed,
                command.requested_at_unix_ms(),
                outcome,
            )?;
            candidate.authored_atomic_receipts.push(receipt.clone());
            *state = candidate;
            Ok(receipt)
        })
    }

    fn authored_receipt(
        &self,
        commit_id: AtomicCommitId,
    ) -> BoxFuture<'_, Result<Option<AuthoredAtomicReceipt>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .authored_atomic_receipts
                .iter()
                .find(|receipt| receipt.commit_id() == commit_id)
                .cloned())
        })
    }

    fn authored_operation(
        &self,
        operation_id: OperationInstanceId,
    ) -> BoxFuture<'_, Result<Option<crate::authored::AuthoredOperation>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .authored_operations
                .iter()
                .find(|operation| operation.operation_id() == operation_id)
                .cloned())
        })
    }

    fn authored_artifact(
        &self,
        artifact_id: crate::authored::AuthoredArtifactId,
    ) -> BoxFuture<'_, Result<Option<crate::authored::AuthoredArtifact>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .authored_artifacts
                .iter()
                .find(|artifact| artifact.artifact_id() == artifact_id)
                .cloned())
        })
    }

    fn authored_delivery_plan(
        &self,
        plan_id: crate::authored_delivery::AuthoredDeliveryPlanId,
    ) -> BoxFuture<'_, Result<Option<crate::authored_delivery::AuthoredDeliveryPlan>, Error>> {
        Box::pin(async move {
            Ok(self
                .state()?
                .authored_delivery_plans
                .iter()
                .find(|plan| plan.plan_id() == plan_id)
                .cloned())
        })
    }
}

fn require_artifact_claim(
    claim: Option<&crate::authored::WorkClaim>,
    fence: &crate::authored_atomic::WorkFence,
    now_unix_ms: u64,
) -> Result<(), Error> {
    if !claim.is_some_and(|claim| {
        claim.matches_fence(
            fence.token(),
            fence.generation(),
            fence.row_revision(),
            now_unix_ms,
        )
    }) {
        return Err(Error::DeliveryPlanClaimConflict);
    }
    Ok(())
}

//! Deterministic in-memory reference storage backend.

use radroots_event::EventId;
use radroots_protocol::runtime::v1::OperationId;
use radroots_transport::{BoxFuture, source::EventProvenance};
use std::sync::{Mutex, MutexGuard};

use crate::{
    AtomicStorage, Error, EventStore, Journal,
    atomic::{
        AtomicCommit, AtomicCommitDisposition, AtomicCommitId, AtomicCommitOutcome,
        AtomicCommitReceipt, AtomicWorkflow,
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
    status::{EventStoreHealth, EventStoreMode, EventStoreStatus},
};

#[derive(Clone)]
struct EventEntry {
    position: EventPosition,
    admission: EventAdmission,
    provenance: Vec<EventProvenance>,
}

#[derive(Default)]
struct State {
    events: Vec<EventEntry>,
    journal: Vec<OperationRecord>,
    atomic_receipts: Vec<AtomicCommitReceipt>,
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
                atomic_receipts: Vec::new(),
            }),
        }
    }

    pub const fn generation(&self) -> SourceGeneration {
        self.generation
    }

    fn state(&self) -> Result<MutexGuard<'_, State>, Error> {
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
                    entry
                        .admission
                        .verified_event()
                        .cloned()
                        .map(|event| StoredVerifiedEvent::new(entry.position, event))
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
                    entry
                        .admission
                        .visible_event()
                        .cloned()
                        .map(|event| StoredVisibleEvent::new(entry.position, event))
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
            let outcome = match request.workflow().clone() {
                AtomicWorkflow::Prepared(operation) => AtomicCommitOutcome::Prepared {
                    journal: Self::prepare_locked(&mut state, operation)?
                        .record()
                        .clone(),
                },
                AtomicWorkflow::Signed(signed) => {
                    let event_id = *signed.event().id();
                    let journal = Self::transition_locked(
                        &mut state,
                        JournalTransition::signed(
                            signed.instance_id(),
                            signed.expected_revision(),
                            event_id,
                        ),
                    )?;
                    AtomicCommitOutcome::Signed { journal, event_id }
                }
                AtomicWorkflow::Ingested(ingested) if ingested.projection().is_none() => {
                    AtomicCommitOutcome::Ingested {
                        admission: self.admit_locked(&mut state, ingested.admission().clone())?,
                        projection: None,
                    }
                }
                AtomicWorkflow::Enqueued(_)
                | AtomicWorkflow::Delivered(_)
                | AtomicWorkflow::Ingested(_) => return Err(Error::BackendUnavailable),
            };
            let receipt = AtomicCommitReceipt::new(
                &request,
                AtomicCommitDisposition::Committed,
                request.requested_at_unix_ms(),
                outcome,
            )?;
            state.atomic_receipts.push(receipt.clone());
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

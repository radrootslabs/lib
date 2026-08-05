//! Passive synchronization status aggregation and host retry decisions.

use std::collections::BTreeSet;

use radroots_protocol::runtime::v1::{
    OPERATION_SCHEMA_VERSION, SyncCapabilityState, SyncHealth, SyncOutboxStatus,
    SyncProjectionStatus, SyncRetryDecision, SyncStatusReceipt,
};
use radroots_signing::{SignerStatus, status::SignerAvailability};
use radroots_storage::{
    EventStore, Outbox, ProjectionStore,
    authored_delivery::{AuthoredDeliveryPlan, AuthoredDeliveryState},
    outbox::OutboxStatus,
    projection::{ProjectionHealth, ProjectionId, ProjectionStatus},
    status::{
        EventStoreHealth, EventStoreStatus, IntegrityHealth, ShutdownState, StorageStatus,
        StorageStatusProvider,
    },
};
use radroots_transport::{SinkStatus, SourceStatus, capability::Availability};

use crate::{Engine, policy::Error};

const STATUS_PROJECTION_LIMIT: usize = 256;

/// Typed report for one optional injected host capability.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityReport<T> {
    state: SyncCapabilityState,
    status: Option<T>,
}

impl<T> CapabilityReport<T> {
    pub const fn state(&self) -> SyncCapabilityState {
        self.state
    }

    pub const fn status(&self) -> Option<&T> {
        self.status.as_ref()
    }

    const fn unsupported() -> Self {
        Self {
            state: SyncCapabilityState::Unsupported,
            status: None,
        }
    }

    const fn compiled(status: Option<T>) -> Self {
        Self {
            state: SyncCapabilityState::Compiled,
            status,
        }
    }

    const fn reported(state: SyncCapabilityState, status: T) -> Self {
        Self {
            state,
            status: Some(status),
        }
    }
}

/// Requested projection and its optional durable status record.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionReport {
    projection_id: ProjectionId,
    status: Option<ProjectionStatus>,
}

impl ProjectionReport {
    pub const fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }

    pub const fn status(&self) -> Option<&ProjectionStatus> {
        self.status.as_ref()
    }
}

/// One passive, side-effect-free synchronization health snapshot.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncStatus {
    health: SyncHealth,
    storage: StorageStatus,
    events: EventStoreStatus,
    outbox: OutboxStatus,
    source: CapabilityReport<SourceStatus>,
    sink: CapabilityReport<SinkStatus>,
    signer: CapabilityReport<SignerStatus>,
    projections: Vec<ProjectionReport>,
}

impl SyncStatus {
    pub const fn health(&self) -> SyncHealth {
        self.health
    }

    pub const fn storage(&self) -> StorageStatus {
        self.storage
    }

    pub const fn events(&self) -> &EventStoreStatus {
        &self.events
    }

    pub const fn outbox(&self) -> OutboxStatus {
        self.outbox
    }

    pub const fn source(&self) -> &CapabilityReport<SourceStatus> {
        &self.source
    }

    pub const fn sink(&self) -> &CapabilityReport<SinkStatus> {
        &self.sink
    }

    pub const fn signer(&self) -> &CapabilityReport<SignerStatus> {
        &self.signer
    }

    pub fn projections(&self) -> &[ProjectionReport] {
        self.projections.as_slice()
    }

    /// Converts native reports into the versioned passive protocol receipt.
    pub fn to_protocol(&self) -> SyncStatusReceipt {
        let mut projection = SyncProjectionStatus {
            ready: 0,
            invalidated: 0,
            rebuilding: 0,
            failed: 0,
            untracked: 0,
        };
        for report in &self.projections {
            match report.status.as_ref().map(ProjectionStatus::health) {
                Some(ProjectionHealth::Ready) => projection.ready += 1,
                Some(ProjectionHealth::Invalidated) => projection.invalidated += 1,
                Some(ProjectionHealth::Rebuilding) => projection.rebuilding += 1,
                Some(ProjectionHealth::Failed) => projection.failed += 1,
                None => projection.untracked += 1,
            }
        }
        SyncStatusReceipt {
            schema_version: OPERATION_SCHEMA_VERSION,
            health: self.health,
            storage: storage_state(self.storage, self.events.health()),
            source: self.source.state,
            sink: self.sink.state,
            signer: self.signer.state,
            outbox: SyncOutboxStatus {
                pending: self.outbox.pending,
                leased: self.outbox.leased,
                retryable: self.outbox.retryable,
                satisfied: self.outbox.satisfied,
                exhausted: self.outbox.exhausted,
            },
            projections: projection,
        }
    }
}

impl Engine {
    /// Aggregates passive status without spawning work or initiating recovery.
    pub async fn status(&self, projection_ids: &[ProjectionId]) -> Result<SyncStatus, Error> {
        if [
            projection_ids.len() > STATUS_PROJECTION_LIMIT,
            projection_ids.iter().collect::<BTreeSet<_>>().len() != projection_ids.len(),
        ]
        .contains(&true)
        {
            return Err(Error::InvalidStatusRequest);
        }
        let storage = StorageStatusProvider::storage_status(self.storage.as_ref())
            .await
            .map_err(|_| Error::StorageFailed)?;
        let events = EventStore::status(self.storage.as_ref())
            .await
            .map_err(|_| Error::StorageFailed)?;
        let outbox = Outbox::status(self.storage.as_ref())
            .await
            .map_err(|_| Error::StorageFailed)?;
        if outbox.total().is_none() {
            return Err(Error::StorageFailed);
        }
        let source = source_report(self).await;
        let sink = sink_report(self).await;
        let signer = signer_report(self).await;
        let mut projections = Vec::with_capacity(projection_ids.len());
        for projection_id in projection_ids {
            let status = ProjectionStore::status(self.storage.as_ref(), projection_id.clone())
                .await
                .map_err(|_| Error::StorageFailed)?;
            projections.push(ProjectionReport {
                projection_id: projection_id.clone(),
                status,
            });
        }
        let health = aggregate_health(storage, &events, &source, &sink, &signer, &projections);
        Ok(SyncStatus {
            health,
            storage,
            events,
            outbox,
            source,
            sink,
            signer,
            projections,
        })
    }

    /// Classifies host action for one durable plan without mutating it.
    pub fn retry_decision(
        &self,
        plan: &AuthoredDeliveryPlan,
        now_unix_ms: u64,
    ) -> Result<SyncRetryDecision, Error> {
        if now_unix_ms == 0 {
            return Err(Error::ClockUnavailable);
        }
        match plan.state() {
            AuthoredDeliveryState::Satisfied => return Ok(SyncRetryDecision::Satisfied),
            AuthoredDeliveryState::Exhausted
            | AuthoredDeliveryState::FailedTerminal
            | AuthoredDeliveryState::Cancelled => return Ok(SyncRetryDecision::Exhausted),
            AuthoredDeliveryState::Pending | AuthoredDeliveryState::Retryable => {}
        }
        if now_unix_ms >= plan.intent().deadline_unix_ms() {
            return Ok(SyncRetryDecision::Expired);
        }
        if let Some(claim) = plan.claim_evidence()
            && now_unix_ms < claim.expires_at_unix_ms()
        {
            return Ok(SyncRetryDecision::InFlightUntil {
                unix_ms: claim.expires_at_unix_ms(),
            });
        }
        if let Some(retry) = plan.retry()
            && now_unix_ms < retry.not_before_unix_ms()
        {
            return Ok(SyncRetryDecision::DeferredUntil {
                unix_ms: retry.not_before_unix_ms(),
            });
        }
        Ok(SyncRetryDecision::Ready)
    }
}

async fn source_report(engine: &Engine) -> CapabilityReport<SourceStatus> {
    let Some(source) = engine.source.as_deref() else {
        return CapabilityReport::unsupported();
    };
    match source.status().await {
        Ok(status) if !status.is_configured() => CapabilityReport::compiled(Some(status)),
        Ok(status) => {
            let state = availability_state(status.availability());
            CapabilityReport::reported(state, status)
        }
        Err(_) => CapabilityReport::compiled(None),
    }
}

async fn sink_report(engine: &Engine) -> CapabilityReport<SinkStatus> {
    let Some(sink) = engine.sink.as_deref() else {
        return CapabilityReport::unsupported();
    };
    match sink.status().await {
        Ok(status) if !status.is_configured() => CapabilityReport::compiled(Some(status)),
        Ok(status) => {
            let state = availability_state(status.availability());
            CapabilityReport::reported(state, status)
        }
        Err(_) => CapabilityReport::compiled(None),
    }
}

async fn signer_report(engine: &Engine) -> CapabilityReport<SignerStatus> {
    let Some(signer) = engine.signer.as_deref() else {
        return CapabilityReport::unsupported();
    };
    match signer.status().await {
        Ok(status) => {
            let state = match status.availability() {
                SignerAvailability::Ready => SyncCapabilityState::Available,
                SignerAvailability::Busy | SignerAvailability::AwaitingAuthentication => {
                    SyncCapabilityState::Degraded
                }
                SignerAvailability::Unavailable => SyncCapabilityState::Configured,
                _ => SyncCapabilityState::Degraded,
            };
            CapabilityReport::reported(state, status)
        }
        Err(_) => CapabilityReport::compiled(None),
    }
}

const fn availability_state(availability: Availability) -> SyncCapabilityState {
    match availability {
        Availability::Available => SyncCapabilityState::Available,
        Availability::Degraded => SyncCapabilityState::Degraded,
        Availability::Unavailable => SyncCapabilityState::Configured,
    }
}

fn aggregate_health(
    storage: StorageStatus,
    events: &EventStoreStatus,
    source: &CapabilityReport<SourceStatus>,
    sink: &CapabilityReport<SinkStatus>,
    signer: &CapabilityReport<SignerStatus>,
    projections: &[ProjectionReport],
) -> SyncHealth {
    if [
        matches!(
            storage.shutdown(),
            ShutdownState::Closing | ShutdownState::Closed
        ),
        storage.integrity().health() == IntegrityHealth::Corrupt,
        events.health() == EventStoreHealth::Unavailable,
    ]
    .contains(&true)
    {
        return SyncHealth::Unavailable;
    }
    let capability_degraded = [source.state, sink.state, signer.state]
        .into_iter()
        .any(|state| {
            matches!(
                state,
                SyncCapabilityState::Compiled
                    | SyncCapabilityState::Configured
                    | SyncCapabilityState::Degraded
            )
        });
    let projection_degraded = projections.iter().any(|projection| {
        !matches!(
            projection.status.as_ref().map(ProjectionStatus::health),
            Some(ProjectionHealth::Ready)
        )
    });
    if [
        storage.integrity().health() != IntegrityHealth::Healthy,
        events.health() == EventStoreHealth::Degraded,
        capability_degraded,
        projection_degraded,
    ]
    .contains(&true)
    {
        SyncHealth::Degraded
    } else {
        SyncHealth::Healthy
    }
}

const fn storage_state(
    storage: StorageStatus,
    event_health: EventStoreHealth,
) -> SyncCapabilityState {
    if matches!(
        storage.shutdown(),
        ShutdownState::Closing | ShutdownState::Closed
    ) || matches!(storage.integrity().health(), IntegrityHealth::Corrupt)
        || matches!(event_health, EventStoreHealth::Unavailable)
    {
        SyncCapabilityState::Configured
    } else if matches!(storage.integrity().health(), IntegrityHealth::Healthy)
        && matches!(event_health, EventStoreHealth::Available)
    {
        SyncCapabilityState::Available
    } else {
        SyncCapabilityState::Degraded
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use radroots_storage::{
        event::SourceGeneration,
        status::{EventStoreMode, IntegrityStatus, StorageBackend, StorageOpenMode, WriterPolicy},
    };

    fn storage(health: IntegrityHealth, shutdown: ShutdownState) -> StorageStatus {
        let failed = u32::from(health == IntegrityHealth::Corrupt);
        let checked = if health == IntegrityHealth::Unknown {
            None
        } else {
            Some(1)
        };
        StorageStatus::new(
            StorageBackend::Memory,
            StorageOpenMode::ReadWriteExisting,
            WriterPolicy::NoWriter,
            shutdown,
            IntegrityStatus::new(health, checked, 1, failed).unwrap(),
            false,
            0,
        )
        .unwrap()
    }

    fn events(health: EventStoreHealth) -> EventStoreStatus {
        EventStoreStatus::new(
            SourceGeneration::new([21; 32]).unwrap(),
            EventStoreMode::ReadWrite,
            health,
            0,
            0,
            0,
        )
        .unwrap()
    }

    #[test]
    fn health_and_protocol_classification_cover_every_state() {
        assert_eq!(
            availability_state(Availability::Available),
            SyncCapabilityState::Available
        );
        assert_eq!(
            availability_state(Availability::Degraded),
            SyncCapabilityState::Degraded
        );
        assert_eq!(
            availability_state(Availability::Unavailable),
            SyncCapabilityState::Configured
        );
        let source = CapabilityReport::<SourceStatus>::unsupported();
        let sink = CapabilityReport::<SinkStatus>::unsupported();
        let signer = CapabilityReport::<SignerStatus>::unsupported();
        for (storage, events, expected) in [
            (
                storage(IntegrityHealth::Healthy, ShutdownState::Open),
                events(EventStoreHealth::Available),
                SyncHealth::Healthy,
            ),
            (
                storage(IntegrityHealth::Corrupt, ShutdownState::Open),
                events(EventStoreHealth::Available),
                SyncHealth::Unavailable,
            ),
            (
                storage(IntegrityHealth::Healthy, ShutdownState::Closing),
                events(EventStoreHealth::Available),
                SyncHealth::Unavailable,
            ),
            (
                storage(IntegrityHealth::Healthy, ShutdownState::Open),
                events(EventStoreHealth::Unavailable),
                SyncHealth::Unavailable,
            ),
            (
                storage(IntegrityHealth::Degraded, ShutdownState::Open),
                events(EventStoreHealth::Available),
                SyncHealth::Degraded,
            ),
        ] {
            assert_eq!(
                aggregate_health(storage, &events, &source, &sink, &signer, &[]),
                expected
            );
        }
        let compiled = CapabilityReport::<SourceStatus>::compiled(None);
        assert_eq!(
            aggregate_health(
                storage(IntegrityHealth::Healthy, ShutdownState::Open),
                &events(EventStoreHealth::Available),
                &compiled,
                &sink,
                &signer,
                &[],
            ),
            SyncHealth::Degraded
        );
        assert_eq!(
            storage_state(
                storage(IntegrityHealth::Healthy, ShutdownState::Open),
                EventStoreHealth::Available,
            ),
            SyncCapabilityState::Available
        );
        assert_eq!(
            storage_state(
                storage(IntegrityHealth::Degraded, ShutdownState::Open),
                EventStoreHealth::Degraded,
            ),
            SyncCapabilityState::Degraded
        );
        assert_eq!(
            storage_state(
                storage(IntegrityHealth::Healthy, ShutdownState::Closed),
                EventStoreHealth::Available,
            ),
            SyncCapabilityState::Configured
        );
    }
}

//! Single-writer, latest-value publication for passive operations reads.

use core::fmt;
use std::sync::Arc;

use tokio::sync::watch;

use super::{ServiceOperationalState, StatusContractError};

/// One immutable cached observation and its service-owned metrics metadata.
///
/// The holder retains only the latest `Arc`, so publication has a fixed
/// one-snapshot capacity. The metadata remains typed and is completed by the
/// bounded metrics contract rather than interpreted by the host cache.
#[derive(Debug)]
pub struct CachedServiceState<M> {
    operational: ServiceOperationalState,
    metrics: M,
}

impl<M> CachedServiceState<M> {
    #[must_use]
    pub const fn new(operational: ServiceOperationalState, metrics: M) -> Self {
        Self {
            operational,
            metrics,
        }
    }

    #[must_use]
    pub const fn operational(&self) -> &ServiceOperationalState {
        &self.operational
    }

    #[must_use]
    pub const fn metrics(&self) -> &M {
        &self.metrics
    }
}

/// The sole update authority for one cached service-state channel.
///
/// This handle deliberately does not implement `Clone`. Services may create
/// any number of readers, but ownership of lifecycle publication stays
/// explicit and singular.
pub struct CachedServiceStatePublisher<M> {
    sender: watch::Sender<Arc<CachedServiceState<M>>>,
}

impl<M> CachedServiceStatePublisher<M> {
    /// Publishes one legal lifecycle update and atomically replaces the cache.
    pub fn publish(&mut self, next: CachedServiceState<M>) -> Result<(), StatusContractError> {
        let current_phase = self.sender.borrow().operational().phase();
        let next_phase = next.operational().phase();
        if !current_phase.can_transition_to(next_phase) {
            return Err(StatusContractError::IllegalTransition {
                from: current_phase,
                to: next_phase,
            });
        }
        self.sender.send_replace(Arc::new(next));
        Ok(())
    }

    /// Adds a passive reader without sharing publication authority.
    #[must_use]
    pub fn subscribe(&self) -> CachedServiceStateReader<M> {
        CachedServiceStateReader {
            receiver: self.sender.subscribe(),
        }
    }
}

/// A cloneable passive view of the latest cached service state.
pub struct CachedServiceStateReader<M> {
    receiver: watch::Receiver<Arc<CachedServiceState<M>>>,
}

impl<M> Clone for CachedServiceStateReader<M> {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
        }
    }
}

impl<M> CachedServiceStateReader<M> {
    /// Returns the latest snapshot without executing a probe or awaiting I/O.
    #[must_use]
    pub fn snapshot(&self) -> Arc<CachedServiceState<M>> {
        Arc::clone(&self.receiver.borrow())
    }

    /// Waits for a later publication and then returns that latest snapshot.
    pub async fn changed(&mut self) -> Result<Arc<CachedServiceState<M>>, StatusPublisherDropped> {
        self.receiver
            .changed()
            .await
            .map_err(|_| StatusPublisherDropped)?;
        Ok(self.snapshot_and_mark_seen())
    }

    fn snapshot_and_mark_seen(&mut self) -> Arc<CachedServiceState<M>> {
        Arc::clone(&self.receiver.borrow_and_update())
    }
}

/// Creates a bounded latest-value cache with one publisher and one reader.
#[must_use]
pub fn cached_service_state<M>(
    initial: CachedServiceState<M>,
) -> (CachedServiceStatePublisher<M>, CachedServiceStateReader<M>) {
    let (sender, receiver) = watch::channel(Arc::new(initial));
    (
        CachedServiceStatePublisher { sender },
        CachedServiceStateReader { receiver },
    )
}

/// Indicates that no owner remains able to publish another snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusPublisherDropped;

impl fmt::Display for StatusPublisherDropped {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("cached service-state publisher was dropped")
    }
}

impl std::error::Error for StatusPublisherDropped {}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::{CommonReasonCode, Readiness, ReasonCodes, ServicePhase};

    #[derive(Debug)]
    struct MetricsMetadata {
        revision: u64,
        probe_calls: Arc<AtomicUsize>,
    }

    fn state(
        phase: ServicePhase,
        readiness: Readiness,
        reasons: ReasonCodes,
    ) -> ServiceOperationalState {
        ServiceOperationalState::new(phase, readiness, reasons).unwrap()
    }

    fn metadata(revision: u64, probe_calls: &Arc<AtomicUsize>) -> MetricsMetadata {
        MetricsMetadata {
            revision,
            probe_calls: Arc::clone(probe_calls),
        }
    }

    #[tokio::test]
    async fn concurrent_readers_observe_the_latest_atomic_publication() {
        let probe_calls = Arc::new(AtomicUsize::new(0));
        let (mut publisher, reader) = cached_service_state(CachedServiceState::new(
            state(
                ServicePhase::Starting,
                Readiness::NOT_READY,
                ReasonCodes::empty(),
            ),
            metadata(0, &probe_calls),
        ));
        let readers: Vec<_> = (0..8)
            .map(|_| {
                let mut reader = reader.clone();
                tokio::spawn(async move {
                    loop {
                        let snapshot = reader.changed().await.unwrap();
                        if snapshot.metrics().revision == 100 {
                            return (
                                snapshot.operational().phase(),
                                snapshot.operational().readiness(),
                            );
                        }
                    }
                })
            })
            .collect();

        publisher
            .publish(CachedServiceState::new(
                state(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty()),
                metadata(1, &probe_calls),
            ))
            .unwrap();
        for revision in 2..=100 {
            publisher
                .publish(CachedServiceState::new(
                    state(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty()),
                    metadata(revision, &probe_calls),
                ))
                .unwrap();
        }

        for reader in readers {
            assert_eq!(
                reader.await.unwrap(),
                (ServicePhase::Ready, Readiness::READY)
            );
        }
        assert_eq!(probe_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn shutdown_state_and_illegal_updates_are_explicit() {
        let probe_calls = Arc::new(AtomicUsize::new(0));
        let (mut publisher, reader) = cached_service_state(CachedServiceState::new(
            state(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty()),
            metadata(4, &probe_calls),
        ));
        let shutdown_reasons =
            ReasonCodes::new([CommonReasonCode::ShutdownInProgress.into()]).unwrap();
        publisher
            .publish(CachedServiceState::new(
                state(
                    ServicePhase::Stopping,
                    Readiness::NOT_READY,
                    shutdown_reasons,
                ),
                metadata(5, &probe_calls),
            ))
            .unwrap();

        let stopping = reader.snapshot();
        assert_eq!(stopping.operational().phase(), ServicePhase::Stopping);
        assert!(!stopping.operational().readiness().is_ready());
        assert_eq!(
            stopping.operational().reasons().as_slice()[0].as_str(),
            CommonReasonCode::ShutdownInProgress.as_str()
        );

        assert_eq!(
            publisher.publish(CachedServiceState::new(
                state(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty(),),
                metadata(6, &probe_calls),
            )),
            Err(StatusContractError::IllegalTransition {
                from: ServicePhase::Stopping,
                to: ServicePhase::Ready,
            })
        );
        assert_eq!(reader.snapshot().metrics().revision, 5);
    }

    #[test]
    fn repeated_snapshot_reads_are_passive_and_preserve_one_arc() {
        let probe_calls = Arc::new(AtomicUsize::new(0));
        let (_publisher, reader) = cached_service_state(CachedServiceState::new(
            state(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty()),
            metadata(1, &probe_calls),
        ));
        let first = reader.snapshot();
        for _ in 0..1_000 {
            let next = reader.snapshot();
            assert!(Arc::ptr_eq(&first, &next));
            assert_eq!(next.metrics().probe_calls.load(Ordering::SeqCst), 0);
        }
    }

    #[tokio::test]
    async fn reader_clone_and_publisher_drop_retain_the_last_snapshot() {
        let probe_calls = Arc::new(AtomicUsize::new(0));
        let (mut publisher, reader) = cached_service_state(CachedServiceState::new(
            state(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty()),
            metadata(1, &probe_calls),
        ));
        let mut clone = reader.clone();
        publisher
            .publish(CachedServiceState::new(
                state(
                    ServicePhase::Degraded,
                    Readiness::READY,
                    ReasonCodes::new([CommonReasonCode::DatabaseLowDisk.into()]).unwrap(),
                ),
                metadata(2, &probe_calls),
            ))
            .unwrap();
        drop(publisher);

        assert_eq!(reader.snapshot().metrics().revision, 2);
        assert_eq!(clone.changed().await.unwrap().metrics().revision, 2);
        assert!(matches!(clone.changed().await, Err(StatusPublisherDropped)));
        assert_eq!(
            clone.snapshot().operational().phase(),
            ServicePhase::Degraded
        );
    }

    #[tokio::test]
    async fn race_between_wakeup_and_borrow_marks_the_returned_latest_value_seen() {
        let probe_calls = Arc::new(AtomicUsize::new(0));
        let (mut publisher, mut reader) = cached_service_state(CachedServiceState::new(
            state(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty()),
            metadata(0, &probe_calls),
        ));
        publisher
            .publish(CachedServiceState::new(
                state(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty()),
                metadata(1, &probe_calls),
            ))
            .unwrap();

        reader.receiver.changed().await.unwrap();
        publisher
            .publish(CachedServiceState::new(
                state(ServicePhase::Ready, Readiness::READY, ReasonCodes::empty()),
                metadata(2, &probe_calls),
            ))
            .unwrap();

        assert_eq!(reader.snapshot_and_mark_seen().metrics().revision, 2);
        assert!(!reader.receiver.has_changed().unwrap());
    }
}

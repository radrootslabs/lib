//! Constant-time liveness and readiness projections from cached service state.

use http::StatusCode;

use crate::{CachedServiceStateReader, ServicePhase};

pub const LIVEZ_PATH: &str = "/livez";
pub const READYZ_PATH: &str = "/readyz";
pub const OPERATIONS_HEALTH_CONTENT_TYPE: &str = "text/plain; charset=utf-8";

const LIVE_BODY: &[u8] = b"live\n";
const FAILED_BODY: &[u8] = b"failed\n";
const READY_BODY: &[u8] = b"ready\n";
const UNREADY_BODY: &[u8] = b"unready\n";

/// One fixed, bounded response for a passive health route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationsHealthResponse {
    status: StatusCode,
    body: &'static [u8],
}

impl OperationsHealthResponse {
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    #[must_use]
    pub const fn content_type(&self) -> &'static str {
        OPERATIONS_HEALTH_CONTENT_TYPE
    }

    #[must_use]
    pub const fn body(&self) -> &'static [u8] {
        self.body
    }
}

/// Projects `/livez` from the latest supervisor-owned lifecycle snapshot.
///
/// The failed terminal phase is not live. Every other phase describes a
/// process whose supervisor is still able to advance or complete shutdown.
#[must_use]
pub fn livez<M>(cache: &CachedServiceStateReader<M>) -> OperationsHealthResponse {
    if cache.snapshot().operational().phase() == ServicePhase::Failed {
        OperationsHealthResponse {
            status: StatusCode::SERVICE_UNAVAILABLE,
            body: FAILED_BODY,
        }
    } else {
        OperationsHealthResponse {
            status: StatusCode::OK,
            body: LIVE_BODY,
        }
    }
}

/// Projects `/readyz` from the latest service-owned cached readiness bit.
#[must_use]
pub fn readyz<M>(cache: &CachedServiceStateReader<M>) -> OperationsHealthResponse {
    if cache.snapshot().operational().readiness().is_ready() {
        OperationsHealthResponse {
            status: StatusCode::OK,
            body: READY_BODY,
        }
    } else {
        OperationsHealthResponse {
            status: StatusCode::SERVICE_UNAVAILABLE,
            body: UNREADY_BODY,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::{
        CachedServiceState, Readiness, ReasonCodes, ServiceOperationalState, cached_service_state,
    };

    struct PassiveMetrics {
        probe_calls: Arc<AtomicUsize>,
    }

    impl PassiveMetrics {
        fn probe_calls(&self) -> usize {
            self.probe_calls.load(Ordering::SeqCst)
        }
    }

    fn cache(
        phase: ServicePhase,
        readiness: Readiness,
    ) -> CachedServiceStateReader<PassiveMetrics> {
        let operational =
            ServiceOperationalState::new(phase, readiness, ReasonCodes::empty()).unwrap();
        let (_, reader) = cached_service_state(CachedServiceState::new(
            operational,
            PassiveMetrics {
                probe_calls: Arc::new(AtomicUsize::new(0)),
            },
        ));
        reader
    }

    #[test]
    fn every_phase_has_an_exact_bounded_health_projection() {
        let cases = [
            (
                ServicePhase::Starting,
                Readiness::NOT_READY,
                StatusCode::OK,
                LIVE_BODY,
                StatusCode::SERVICE_UNAVAILABLE,
                UNREADY_BODY,
            ),
            (
                ServicePhase::Ready,
                Readiness::READY,
                StatusCode::OK,
                LIVE_BODY,
                StatusCode::OK,
                READY_BODY,
            ),
            (
                ServicePhase::Degraded,
                Readiness::READY,
                StatusCode::OK,
                LIVE_BODY,
                StatusCode::OK,
                READY_BODY,
            ),
            (
                ServicePhase::Degraded,
                Readiness::NOT_READY,
                StatusCode::OK,
                LIVE_BODY,
                StatusCode::SERVICE_UNAVAILABLE,
                UNREADY_BODY,
            ),
            (
                ServicePhase::Unready,
                Readiness::NOT_READY,
                StatusCode::OK,
                LIVE_BODY,
                StatusCode::SERVICE_UNAVAILABLE,
                UNREADY_BODY,
            ),
            (
                ServicePhase::Stopping,
                Readiness::NOT_READY,
                StatusCode::OK,
                LIVE_BODY,
                StatusCode::SERVICE_UNAVAILABLE,
                UNREADY_BODY,
            ),
            (
                ServicePhase::Failed,
                Readiness::NOT_READY,
                StatusCode::SERVICE_UNAVAILABLE,
                FAILED_BODY,
                StatusCode::SERVICE_UNAVAILABLE,
                UNREADY_BODY,
            ),
        ];

        for (phase, readiness, live_status, live_body, ready_status, ready_body) in cases {
            let reader = cache(phase, readiness);
            let live = livez(&reader);
            let ready = readyz(&reader);

            assert_eq!((live.status(), live.body()), (live_status, live_body));
            assert_eq!((ready.status(), ready.body()), (ready_status, ready_body));
            assert_eq!(live.content_type(), OPERATIONS_HEALTH_CONTENT_TYPE);
            assert_eq!(ready.content_type(), OPERATIONS_HEALTH_CONTENT_TYPE);
            assert!(live.body().len() <= UNREADY_BODY.len());
            assert!(ready.body().len() <= UNREADY_BODY.len());
            assert_eq!(reader.snapshot().metrics().probe_calls(), 0);
        }
    }

    #[test]
    fn handlers_read_the_latest_snapshot_without_waiting_or_probing() {
        let probe_calls = Arc::new(AtomicUsize::new(0));
        let starting = ServiceOperationalState::new(
            ServicePhase::Starting,
            Readiness::NOT_READY,
            ReasonCodes::empty(),
        )
        .unwrap();
        let (mut publisher, reader) = cached_service_state(CachedServiceState::new(
            starting,
            PassiveMetrics {
                probe_calls: Arc::clone(&probe_calls),
            },
        ));

        assert_eq!(readyz(&reader).status(), StatusCode::SERVICE_UNAVAILABLE);
        let ready = ServiceOperationalState::new(
            ServicePhase::Ready,
            Readiness::READY,
            ReasonCodes::empty(),
        )
        .unwrap();
        publisher
            .publish(CachedServiceState::new(
                ready,
                PassiveMetrics {
                    probe_calls: Arc::clone(&probe_calls),
                },
            ))
            .unwrap();

        assert_eq!(readyz(&reader).status(), StatusCode::OK);
        assert_eq!(probe_calls.load(Ordering::SeqCst), 0);
    }
}

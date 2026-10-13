use std::{
    future::{Future, pending, ready},
    pin::pin,
    task::Poll,
    time::Duration,
};

use radroots_service_host::{
    GracefulShutdown, HostError, HostErrorKind, MonotonicClock, MonotonicTime, ShutdownDisposition,
    ShutdownPhase, ShutdownPhaseFuture, ShutdownPhaseHandler, SupervisedTaskExit,
    SupervisedTaskExitStatus, SupervisionFailure, SupervisionFailureKind, TaskClassification,
    TaskMetadata, TaskName, TaskSupervisor, UnfinishedWork,
};
use tokio::sync::oneshot;

fn metadata(name: &str, classification: TaskClassification) -> TaskMetadata {
    let phase = classification
        .requires_shutdown_phase()
        .then_some(ShutdownPhase::CancelIngress);
    TaskMetadata::new(TaskName::new(name).unwrap(), classification, phase).unwrap()
}

async fn consume_ready_outcome_and_cancel_bulk_wait(supervisor: &mut TaskSupervisor) {
    {
        let mut waiting = pin!(supervisor.supervise());
        std::future::poll_fn(|context| {
            assert!(waiting.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
    }
    // The first join was consumed before the waiting future was dropped.
    assert_eq!(supervisor.task_count(), 1);
}

#[tokio::test]
async fn cancelled_bulk_wait_retains_already_joined_exit_on_retry() {
    let mut supervisor = TaskSupervisor::new();
    let (finished, observed) = oneshot::channel();
    let (release, held) = oneshot::channel();
    supervisor
        .spawn(
            metadata("finished", TaskClassification::OneShot),
            |_| async {
                finished.send(()).unwrap();
                Ok(())
            },
        )
        .unwrap();
    supervisor
        .spawn(
            metadata("waiting", TaskClassification::OneShot),
            |_| async {
                held.await.unwrap();
                Ok(())
            },
        )
        .unwrap();
    observed.await.unwrap();
    consume_ready_outcome_and_cancel_bulk_wait(&mut supervisor).await;
    assert!(!supervisor.cancellation_token().is_cancelled());
    release.send(()).unwrap();
    let exits = supervisor.supervise().await.unwrap();
    assert!(supervisor.is_empty());
    let names = exits
        .iter()
        .map(|exit| exit.metadata().name().as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["finished", "waiting"]);
    assert!(supervisor.supervise().await.unwrap().is_empty());
}

#[tokio::test]
async fn cancelled_bulk_wait_retains_first_fatal_failure_on_retry() {
    let mut supervisor = TaskSupervisor::new();
    let (finished, observed) = oneshot::channel();
    let (release, held) = oneshot::channel();
    supervisor
        .spawn(
            metadata("failed", TaskClassification::Critical),
            |_| async {
                finished.send(()).unwrap();
                Err(HostError::new(HostErrorKind::TaskFailure))
            },
        )
        .unwrap();
    supervisor
        .spawn(
            metadata("waiting", TaskClassification::OneShot),
            |_| async {
                held.await.unwrap();
                Ok(())
            },
        )
        .unwrap();
    observed.await.unwrap();
    consume_ready_outcome_and_cancel_bulk_wait(&mut supervisor).await;
    assert!(supervisor.cancellation_token().is_cancelled());
    release.send(()).unwrap();
    let failure = supervisor.supervise().await.unwrap_err();
    assert!(supervisor.is_empty());
    assert_eq!(failure.kind(), SupervisionFailureKind::TaskReturnedError);
    assert_eq!(failure.metadata().unwrap().name().as_str(), "failed");
    assert!(supervisor.supervise().await.unwrap().is_empty());
}

async fn completed_pair(
    classification: TaskClassification,
    first_result: Result<(), HostError>,
    held_result: Result<(), HostError>,
) -> (TaskSupervisor, oneshot::Sender<()>) {
    let mut supervisor = TaskSupervisor::new();
    let (finished, observed) = oneshot::channel();
    let (release, held) = oneshot::channel();
    supervisor
        .spawn(metadata("observed", classification), |_| async move {
            finished.send(()).unwrap();
            first_result
        })
        .unwrap();
    supervisor
        .spawn(
            metadata("held", TaskClassification::OneShot),
            |_| async move {
                held.await.unwrap();
                held_result
            },
        )
        .unwrap();
    observed.await.unwrap();
    (supervisor, release)
}

async fn observe_retained(
    supervisor: &mut TaskSupervisor,
) -> Result<SupervisedTaskExit, SupervisionFailure> {
    let mut reading = pin!(supervisor.join_next());
    std::future::poll_fn(|context| match reading.as_mut().poll(context) {
        Poll::Ready(Some(outcome)) => Poll::Ready(outcome),
        _ => panic!("an already joined observation must be immediately available"),
    })
    .await
}

#[tokio::test]
async fn cancelled_bulk_optional_failure_transfers_once_to_per_join_observation() {
    let (mut supervisor, release) = completed_pair(
        TaskClassification::Optional,
        Err(HostError::new(HostErrorKind::TaskFailure)),
        Ok(()),
    )
    .await;
    consume_ready_outcome_and_cancel_bulk_wait(&mut supervisor).await;

    let exit = observe_retained(&mut supervisor).await.unwrap();
    assert_eq!(exit.metadata().name().as_str(), "observed");
    assert_eq!(exit.status(), SupervisedTaskExitStatus::OptionalFailure);
    assert!(exit.source().is_some());
    assert!(!supervisor.cancellation_token().is_cancelled());
    release.send(()).unwrap();
    let remaining = supervisor.supervise().await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].metadata().name().as_str(), "held");
    assert_eq!(
        remaining[0].status(),
        SupervisedTaskExitStatus::ExpectedCompletion
    );
    assert!(supervisor.join_next().await.is_none());
}

#[tokio::test]
async fn cancelled_bulk_fatal_transfers_once_before_a_later_fatal_join() {
    let (mut supervisor, release) = completed_pair(
        TaskClassification::Critical,
        Err(HostError::new(HostErrorKind::TaskFailure)),
        Err(HostError::new(HostErrorKind::Lifecycle)),
    )
    .await;
    consume_ready_outcome_and_cancel_bulk_wait(&mut supervisor).await;

    let first = observe_retained(&mut supervisor).await.unwrap_err();
    assert_eq!(first.kind(), SupervisionFailureKind::TaskReturnedError);
    assert_eq!(first.metadata().unwrap().name().as_str(), "observed");
    assert!(supervisor.cancellation_token().is_cancelled());
    release.send(()).unwrap();
    let later = supervisor.join_next().await.unwrap().unwrap_err();
    assert_eq!(later.kind(), SupervisionFailureKind::TaskReturnedError);
    assert_eq!(later.metadata().unwrap().name().as_str(), "held");
    assert!(supervisor.join_next().await.is_none());
    assert!(supervisor.supervise().await.unwrap().is_empty());
}

struct FixedClock(Duration);

impl MonotonicClock for FixedClock {
    fn now_monotonic(&self) -> MonotonicTime {
        MonotonicTime::from_duration_since_origin(self.0)
    }
}

#[derive(Default)]
struct RecordingHandler(Vec<ShutdownPhase>);

impl ShutdownPhaseHandler for RecordingHandler {
    fn enter(&mut self, phase: ShutdownPhase) -> ShutdownPhaseFuture<'_> {
        self.0.push(phase);
        Box::pin(ready(Ok(())))
    }
}

const PHASES: [ShutdownPhase; 7] = [
    ShutdownPhase::RejectNewMutations,
    ShutdownPhase::CancelIngress,
    ShutdownPhase::DrainOperations,
    ShutdownPhase::PersistRecoverableWork,
    ShutdownPhase::CloseNetwork,
    ShutdownPhase::CloseSqlite,
    ShutdownPhase::CloseSockets,
];

#[tokio::test]
async fn cancelled_bulk_fatal_is_retained_by_graceful_shutdown_before_empty_phase() {
    let (mut supervisor, release) = completed_pair(
        TaskClassification::Critical,
        Err(HostError::new(HostErrorKind::TaskFailure)),
        Ok(()),
    )
    .await;
    consume_ready_outcome_and_cancel_bulk_wait(&mut supervisor).await;
    release.send(()).unwrap();
    let mut shutdown = GracefulShutdown::new(Duration::from_secs(30)).unwrap();
    let mut handler = RecordingHandler::default();
    let clock = FixedClock(Duration::from_secs(5));

    let summary = shutdown
        .run(&clock, &mut supervisor, &mut handler, pending())
        .await
        .unwrap();
    assert_eq!(
        summary.disposition(),
        ShutdownDisposition::TaskFailed {
            kind: SupervisionFailureKind::TaskReturnedError,
        }
    );
    assert_eq!(
        shutdown
            .task_failure()
            .unwrap()
            .metadata()
            .unwrap()
            .name()
            .as_str(),
        "observed"
    );
    assert_eq!(handler.0, PHASES);
    assert!(supervisor.is_empty());
    assert!(supervisor.join_next().await.is_none());
    assert_eq!(
        shutdown
            .run(&clock, &mut supervisor, &mut handler, ready(()))
            .await
            .unwrap(),
        summary
    );
    assert_eq!(handler.0, PHASES);
}

#[tokio::test]
async fn cancelled_phase_drain_transfers_exit_and_resumes_original_deadline() {
    let (mut supervisor, release) =
        completed_pair(TaskClassification::OneShot, Ok(()), Ok(())).await;
    let mut shutdown = GracefulShutdown::new(Duration::from_secs(30)).unwrap();
    let mut handler = RecordingHandler::default();
    let clock = FixedClock(Duration::from_secs(5));
    {
        let mut waiting = pin!(shutdown.run(&clock, &mut supervisor, &mut handler, pending()));
        std::future::poll_fn(|context| {
            assert!(waiting.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
    }
    assert_eq!(supervisor.task_count(), 1);
    assert_eq!(handler.0, PHASES[..3]);
    let retained = observe_retained(&mut supervisor).await.unwrap();
    assert_eq!(retained.metadata().name().as_str(), "observed");
    assert_eq!(
        retained.status(),
        SupervisedTaskExitStatus::ExpectedCompletion
    );
    assert!(!supervisor.cancellation_token().is_cancelled());
    release.send(()).unwrap();
    let summary = shutdown
        .run(
            &FixedClock(Duration::from_secs(250)),
            &mut supervisor,
            &mut handler,
            pending(),
        )
        .await
        .unwrap();
    assert_eq!(summary.disposition(), ShutdownDisposition::Completed);
    assert_eq!(
        summary.deadline().time().duration_since_origin(),
        Duration::from_secs(35)
    );
    assert_eq!(handler.0, PHASES);
    assert!(supervisor.is_empty());
    assert!(supervisor.join_next().await.is_none());
}

#[tokio::test]
async fn forced_abort_drain_retains_previously_joined_fatal_and_aborted_peer() {
    let (mut supervisor, _release) = completed_pair(
        TaskClassification::Critical,
        Err(HostError::new(HostErrorKind::TaskFailure)),
        Ok(()),
    )
    .await;
    consume_ready_outcome_and_cancel_bulk_wait(&mut supervisor).await;
    let mut shutdown = GracefulShutdown::new(Duration::from_secs(30)).unwrap();
    let mut handler = RecordingHandler::default();
    let clock = FixedClock(Duration::from_secs(5));
    let summary = shutdown
        .run(&clock, &mut supervisor, &mut handler, ready(()))
        .await
        .unwrap();
    assert_eq!(
        summary.disposition(),
        ShutdownDisposition::Forced {
            unfinished: UnfinishedWork::FatalAuthoritative,
        }
    );
    assert!(handler.0.is_empty());
    assert!(supervisor.is_empty());
    assert_eq!(supervisor.task_count(), 0);
    let retained = observe_retained(&mut supervisor).await.unwrap_err();
    assert_eq!(retained.kind(), SupervisionFailureKind::TaskReturnedError);
    assert_eq!(retained.metadata().unwrap().name().as_str(), "observed");
    let aborted = observe_retained(&mut supervisor).await.unwrap();
    assert_eq!(aborted.metadata().name().as_str(), "held");
    assert_eq!(
        aborted.status(),
        SupervisedTaskExitStatus::ExpectedCompletion
    );
    assert!(supervisor.join_next().await.is_none());
    assert!(supervisor.supervise().await.unwrap().is_empty());
}

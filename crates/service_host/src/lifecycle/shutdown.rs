//! Bounded graceful-shutdown orchestration without signal installation.

use core::{fmt, future::Future, pin::Pin, time::Duration};
use std::error::Error;

use crate::{HostError, MonotonicClock, MonotonicClockError, MonotonicDeadline};

use super::{ShutdownPhase, SupervisionFailure, SupervisionFailureKind, TaskSupervisor};

const ORDERED_PHASES: [ShutdownPhase; 7] = [
    ShutdownPhase::RejectNewMutations,
    ShutdownPhase::CancelIngress,
    ShutdownPhase::DrainOperations,
    ShutdownPhase::PersistRecoverableWork,
    ShutdownPhase::CloseNetwork,
    ShutdownPhase::CloseSqlite,
    ShutdownPhase::CloseSockets,
];

/// Service-owned asynchronous work performed when entering one shutdown phase.
pub type ShutdownPhaseFuture<'a> = Pin<Box<dyn Future<Output = Result<(), HostError>> + Send + 'a>>;

/// Executes service-specific phase work without transferring lifecycle ownership.
///
/// If the caller cancels [`GracefulShutdown::run`] before one `enter` future
/// completes, a later call re-enters that incomplete phase under the original
/// deadline. Implementations must therefore make each phase idempotent and
/// cancellation safe. A completed phase is never re-entered.
pub trait ShutdownPhaseHandler: Send {
    fn enter(&mut self, phase: ShutdownPhase) -> ShutdownPhaseFuture<'_>;
}

/// Reusable, idempotent bounded shutdown coordinator.
pub struct GracefulShutdown {
    grace: Duration,
    progress: Option<ShutdownProgress>,
    completed: Option<ShutdownSummary>,
    phase_failure: Option<ShutdownPhaseFailure>,
    task_failure: Option<SupervisionFailure>,
}

#[derive(Clone, Copy)]
struct ShutdownProgress {
    deadline: MonotonicDeadline,
    runtime_deadline: tokio::time::Instant,
    phase_index: usize,
    stage: ShutdownPhaseStage,
    disposition: ShutdownDisposition,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShutdownPhaseStage {
    Enter,
    Drain,
    Abort(ShutdownDisposition),
}

impl GracefulShutdown {
    pub fn new(grace: Duration) -> Result<Self, ShutdownConfigError> {
        if grace.is_zero() {
            return Err(ShutdownConfigError::ZeroGrace);
        }
        Ok(Self {
            grace,
            progress: None,
            completed: None,
            phase_failure: None,
            task_failure: None,
        })
    }

    #[must_use]
    pub const fn grace(&self) -> Duration {
        self.grace
    }

    /// Returns the trusted phase failure retained by the completed run, if any.
    #[must_use]
    pub const fn phase_failure(&self) -> Option<&ShutdownPhaseFailure> {
        self.phase_failure.as_ref()
    }

    /// Returns the trusted task failure retained by the completed run, if any.
    #[must_use]
    pub const fn task_failure(&self) -> Option<&SupervisionFailure> {
        self.task_failure.as_ref()
    }

    /// Runs or resumes the exact shutdown sequence under one retained absolute deadline.
    ///
    /// Cancelling this future retains the last completed handler/drain boundary.
    /// A retry resumes that boundary with the original remaining duration.
    /// Completed runs return the first summary unchanged.
    pub async fn run<C, F>(
        &mut self,
        clock: &C,
        supervisor: &mut TaskSupervisor,
        handler: &mut dyn ShutdownPhaseHandler,
        force: F,
    ) -> Result<ShutdownSummary, ShutdownStartError>
    where
        C: MonotonicClock,
        F: Future<Output = ()> + Send,
    {
        if let Some(completed) = self.completed {
            return Ok(completed);
        }
        if self.progress.is_none() {
            let deadline = clock
                .deadline_after(self.grace)
                .map_err(ShutdownStartError::Deadline)?;
            let runtime_deadline = tokio::time::Instant::now().checked_add(self.grace).ok_or(
                ShutdownStartError::Deadline(MonotonicClockError::DeadlineOverflow),
            )?;
            self.progress = Some(ShutdownProgress {
                deadline,
                runtime_deadline,
                phase_index: 0,
                stage: ShutdownPhaseStage::Enter,
                disposition: ShutdownDisposition::Completed,
            });
        }
        tokio::pin!(force);

        loop {
            let progress = self
                .progress
                .expect("shutdown progress must be initialized");
            if let ShutdownPhaseStage::Abort(disposition) = progress.stage {
                supervisor.abort_and_drain().await;
                return Ok(self.complete(progress.deadline, disposition));
            }
            let Some(&phase) = ORDERED_PHASES.get(progress.phase_index) else {
                return Ok(self.complete(progress.deadline, progress.disposition));
            };
            let deadline_wait = tokio::time::sleep_until(progress.runtime_deadline);
            tokio::pin!(deadline_wait);

            let outcome = match progress.stage {
                ShutdownPhaseStage::Enter => wait_bounded(
                    || async {
                        supervisor.request_phase_cancellation(phase);
                        handler.enter(phase).await
                    },
                    force.as_mut(),
                    deadline_wait.as_mut(),
                )
                .await
                .map(ShutdownPhaseOutcome::Entered),
                ShutdownPhaseStage::Drain => wait_bounded(
                    || supervisor.supervise_phase(phase),
                    force.as_mut(),
                    deadline_wait.as_mut(),
                )
                .await
                .map(ShutdownPhaseOutcome::Drained),
                ShutdownPhaseStage::Abort(_) => unreachable!("abort stage handled before phase"),
            };

            match outcome {
                BoundedWait::Completed(ShutdownPhaseOutcome::Entered(result)) => {
                    if let Err(error) = result {
                        let unfinished = supervisor.unfinished_work();
                        if self.phase_failure.is_none() {
                            self.phase_failure = Some(ShutdownPhaseFailure { phase, error });
                        }
                        self.retain_first_disposition(ShutdownDisposition::PhaseFailed {
                            phase,
                            unfinished,
                        });
                    }
                    self.progress.as_mut().expect("shutdown progress").stage =
                        ShutdownPhaseStage::Drain;
                }
                BoundedWait::Completed(ShutdownPhaseOutcome::Drained(result)) => match result {
                    Ok(_exits) => {
                        let progress = self.progress.as_mut().expect("shutdown progress");
                        progress.phase_index += 1;
                        progress.stage = ShutdownPhaseStage::Enter;
                    }
                    Err(failure) => {
                        let kind = failure.kind();
                        if self.task_failure.is_none() {
                            self.task_failure = Some(failure);
                        }
                        self.retain_first_disposition(ShutdownDisposition::TaskFailed { kind });
                    }
                },
                BoundedWait::Forced => {
                    let unfinished = supervisor.unfinished_work();
                    self.progress.as_mut().expect("shutdown progress").stage =
                        ShutdownPhaseStage::Abort(ShutdownDisposition::Forced { unfinished });
                }
                BoundedWait::GraceExpired => {
                    let unfinished = supervisor.unfinished_work();
                    self.progress.as_mut().expect("shutdown progress").stage =
                        ShutdownPhaseStage::Abort(ShutdownDisposition::GraceExpired { unfinished });
                }
            }
        }
    }

    fn retain_first_disposition(&mut self, disposition: ShutdownDisposition) {
        let progress = self.progress.as_mut().expect("shutdown progress");
        if progress.disposition == ShutdownDisposition::Completed {
            progress.disposition = disposition;
        }
    }

    fn complete(
        &mut self,
        deadline: MonotonicDeadline,
        disposition: ShutdownDisposition,
    ) -> ShutdownSummary {
        let summary = ShutdownSummary {
            deadline,
            disposition,
        };
        self.completed = Some(summary);
        self.progress = None;
        summary
    }
}

enum ShutdownPhaseOutcome {
    Entered(Result<(), HostError>),
    Drained(Result<Vec<super::SupervisedTaskExit>, SupervisionFailure>),
}

enum BoundedWait<T> {
    Completed(T),
    Forced,
    GraceExpired,
}

impl<T> BoundedWait<T> {
    fn map<U>(self, map: impl FnOnce(T) -> U) -> BoundedWait<U> {
        match self {
            Self::Completed(value) => BoundedWait::Completed(map(value)),
            Self::Forced => BoundedWait::Forced,
            Self::GraceExpired => BoundedWait::GraceExpired,
        }
    }
}

async fn wait_bounded<T, MakeWork, Work, Force>(
    make_work: MakeWork,
    mut force: Pin<&mut Force>,
    mut deadline: Pin<&mut tokio::time::Sleep>,
) -> BoundedWait<T>
where
    MakeWork: FnOnce() -> Work,
    Work: Future<Output = T>,
    Force: Future<Output = ()>,
{
    tokio::select! {
        biased;
        () = force.as_mut() => BoundedWait::Forced,
        () = deadline.as_mut() => BoundedWait::GraceExpired,
        completed = async move { make_work().await } => BoundedWait::Completed(completed),
    }
}

/// Trusted phase failure retained separately from the stable shutdown summary.
pub struct ShutdownPhaseFailure {
    phase: ShutdownPhase,
    error: HostError,
}

impl ShutdownPhaseFailure {
    #[must_use]
    pub const fn phase(&self) -> ShutdownPhase {
        self.phase
    }

    /// Returns the original host error for trusted internal inspection.
    #[must_use]
    pub const fn error(&self) -> &HostError {
        &self.error
    }
}

impl fmt::Debug for ShutdownPhaseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ShutdownPhaseFailure")
            .field("phase", &self.phase)
            .field("error", &self.error.safe_error())
            .field("source", &self.error.source().map(|_| "<redacted>"))
            .finish()
    }
}

impl fmt::Display for ShutdownPhaseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("service shutdown phase failed")
    }
}

impl Error for ShutdownPhaseFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}

/// Immutable result of one shutdown run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShutdownSummary {
    deadline: MonotonicDeadline,
    disposition: ShutdownDisposition,
}

impl ShutdownSummary {
    #[must_use]
    pub const fn deadline(self) -> MonotonicDeadline {
        self.deadline
    }

    #[must_use]
    pub const fn disposition(self) -> ShutdownDisposition {
        self.disposition
    }
}

/// Final bounded shutdown classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownDisposition {
    Completed,
    TaskFailed {
        kind: SupervisionFailureKind,
    },
    PhaseFailed {
        phase: ShutdownPhase,
        unfinished: UnfinishedWork,
    },
    GraceExpired {
        unfinished: UnfinishedWork,
    },
    Forced {
        unfinished: UnfinishedWork,
    },
}

/// Whether forcibly stopped work may safely recover after restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnfinishedWork {
    None,
    RecoverableOptional,
    FatalAuthoritative,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownConfigError {
    ZeroGrace,
}

impl fmt::Display for ShutdownConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("shutdown grace must be greater than zero")
    }
}

impl Error for ShutdownConfigError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownStartError {
    Deadline(MonotonicClockError),
}

impl fmt::Display for ShutdownStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("shutdown grace deadline could not be represented")
    }
}

impl Error for ShutdownStartError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Deadline(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use core::future::{pending, ready};
    use std::{
        error::Error,
        sync::{Arc, Mutex},
    };

    use crate::{HostErrorKind, MonotonicTime, TaskClassification, TaskMetadata, TaskName};

    use super::*;

    struct FakeClock {
        now: MonotonicTime,
    }

    impl MonotonicClock for FakeClock {
        fn now_monotonic(&self) -> MonotonicTime {
            self.now
        }
    }

    struct RecordingHandler {
        phases: Arc<Mutex<Vec<ShutdownPhase>>>,
        fail_at: Option<ShutdownPhase>,
    }

    impl ShutdownPhaseHandler for RecordingHandler {
        fn enter(&mut self, phase: ShutdownPhase) -> ShutdownPhaseFuture<'_> {
            self.phases.lock().unwrap().push(phase);
            let fail = self.fail_at == Some(phase);
            Box::pin(async move {
                if fail {
                    Err(HostError::with_source(
                        HostErrorKind::Lifecycle,
                        SensitiveCause,
                    ))
                } else {
                    Ok(())
                }
            })
        }
    }

    #[derive(Debug)]
    struct SensitiveCause;

    impl fmt::Display for SensitiveCause {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("sensitive shutdown detail")
        }
    }

    impl Error for SensitiveCause {}

    struct BlockingHandler {
        phases: Arc<Mutex<Vec<ShutdownPhase>>>,
        block_at: ShutdownPhase,
        entered: Arc<tokio::sync::Notify>,
    }

    impl ShutdownPhaseHandler for BlockingHandler {
        fn enter(&mut self, phase: ShutdownPhase) -> ShutdownPhaseFuture<'_> {
            self.phases.lock().unwrap().push(phase);
            if phase == self.block_at {
                self.entered.notify_one();
                Box::pin(pending())
            } else {
                Box::pin(ready(Ok(())))
            }
        }
    }

    fn clock() -> FakeClock {
        FakeClock {
            now: MonotonicTime::from_duration_since_origin(Duration::from_secs(5)),
        }
    }

    fn handler(
        fail_at: Option<ShutdownPhase>,
    ) -> (RecordingHandler, Arc<Mutex<Vec<ShutdownPhase>>>) {
        let phases = Arc::new(Mutex::new(Vec::new()));
        (
            RecordingHandler {
                phases: Arc::clone(&phases),
                fail_at,
            },
            phases,
        )
    }

    fn task_metadata(name: &str, classification: TaskClassification) -> TaskMetadata {
        let shutdown_phase = classification
            .requires_shutdown_phase()
            .then_some(ShutdownPhase::CancelIngress);
        TaskMetadata::new(TaskName::new(name).unwrap(), classification, shutdown_phase).unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn phases_are_ordered_and_repeated_run_is_idempotent() {
        let mut supervisor = TaskSupervisor::new();
        supervisor
            .spawn(
                task_metadata("critical_worker", TaskClassification::Critical),
                |token| async move {
                    token.cancelled().await;
                    Ok(())
                },
            )
            .unwrap();
        let (mut handler, phases) = handler(None);
        let mut shutdown = GracefulShutdown::new(Duration::from_secs(30)).unwrap();
        assert_eq!(shutdown.grace(), Duration::from_secs(30));

        let first = shutdown
            .run(&clock(), &mut supervisor, &mut handler, pending())
            .await
            .unwrap();
        let second = shutdown
            .run(&clock(), &mut supervisor, &mut handler, ready(()))
            .await
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(first.disposition(), ShutdownDisposition::Completed);
        assert_eq!(
            first.deadline().time().duration_since_origin(),
            Duration::from_secs(35)
        );
        assert_eq!(*phases.lock().unwrap(), ORDERED_PHASES);
        assert!(supervisor.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn grace_timeout_classifies_and_drains_recoverable_and_fatal_work() {
        for (classification, expected) in [
            (
                TaskClassification::Optional,
                UnfinishedWork::RecoverableOptional,
            ),
            (
                TaskClassification::Critical,
                UnfinishedWork::FatalAuthoritative,
            ),
        ] {
            let mut supervisor = TaskSupervisor::new();
            supervisor
                .spawn(task_metadata("stuck_worker", classification), |_| async {
                    pending::<()>().await;
                    Ok(())
                })
                .unwrap();
            let (mut handler, _) = handler(None);
            let mut shutdown = GracefulShutdown::new(Duration::from_secs(1)).unwrap();

            let summary = shutdown
                .run(&clock(), &mut supervisor, &mut handler, pending())
                .await
                .unwrap();
            assert_eq!(
                summary.disposition(),
                ShutdownDisposition::GraceExpired {
                    unfinished: expected
                }
            );
            assert!(supervisor.is_empty());
        }
    }

    #[tokio::test]
    async fn force_input_aborts_and_classifies_active_authoritative_work() {
        let mut supervisor = TaskSupervisor::new();
        supervisor
            .spawn(
                task_metadata("stuck_worker", TaskClassification::Critical),
                |_| async {
                    pending::<()>().await;
                    Ok(())
                },
            )
            .unwrap();
        let (mut handler, phases) = handler(None);
        let mut shutdown = GracefulShutdown::new(Duration::from_secs(30)).unwrap();

        let summary = shutdown
            .run(&clock(), &mut supervisor, &mut handler, ready(()))
            .await
            .unwrap();
        assert_eq!(
            summary.disposition(),
            ShutdownDisposition::Forced {
                unfinished: UnfinishedWork::FatalAuthoritative
            }
        );
        assert!(phases.lock().unwrap().is_empty());
        assert!(supervisor.is_empty());
    }

    #[tokio::test]
    async fn phase_failure_is_fatal_and_records_exact_phase() {
        let mut supervisor = TaskSupervisor::new();
        let (mut handler, phases) = handler(Some(ShutdownPhase::PersistRecoverableWork));
        let mut shutdown = GracefulShutdown::new(Duration::from_secs(30)).unwrap();

        let summary = shutdown
            .run(&clock(), &mut supervisor, &mut handler, pending())
            .await
            .unwrap();
        assert_eq!(
            summary.disposition(),
            ShutdownDisposition::PhaseFailed {
                phase: ShutdownPhase::PersistRecoverableWork,
                unfinished: UnfinishedWork::None,
            }
        );
        assert_eq!(*phases.lock().unwrap(), ORDERED_PHASES);
        let failure = shutdown.phase_failure().unwrap();
        assert_eq!(failure.phase(), ShutdownPhase::PersistRecoverableWork);
        assert_eq!(
            failure.error().source().map(ToString::to_string).as_deref(),
            Some("sensitive shutdown detail")
        );
        assert!(!failure.to_string().contains("sensitive"));
        assert!(!format!("{failure:?}").contains("sensitive shutdown detail"));
        assert!(failure.source().is_some());
    }

    #[tokio::test]
    async fn fatal_task_result_still_runs_the_remaining_close_phases() {
        let mut supervisor = TaskSupervisor::new();
        supervisor
            .spawn(
                task_metadata("fatal_worker", TaskClassification::Critical),
                |_| async { Err(HostError::new(HostErrorKind::TaskFailure)) },
            )
            .unwrap();
        let (mut handler, phases) = handler(None);
        let mut shutdown = GracefulShutdown::new(Duration::from_secs(30)).unwrap();

        let summary = shutdown
            .run(&clock(), &mut supervisor, &mut handler, pending())
            .await
            .unwrap();
        assert_eq!(
            summary.disposition(),
            ShutdownDisposition::TaskFailed {
                kind: SupervisionFailureKind::TaskReturnedError
            }
        );
        assert_eq!(*phases.lock().unwrap(), ORDERED_PHASES);
        assert!(supervisor.is_empty());
        let failure = shutdown.task_failure().unwrap();
        assert_eq!(failure.metadata().unwrap().name().as_str(), "fatal_worker");
        assert!(failure.source().is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_run_retains_its_absolute_deadline_and_completed_phase_progress() {
        let mut supervisor = TaskSupervisor::new();
        let phases = Arc::new(Mutex::new(Vec::new()));
        let entered = Arc::new(tokio::sync::Notify::new());
        let mut handler = BlockingHandler {
            phases: Arc::clone(&phases),
            block_at: ShutdownPhase::PersistRecoverableWork,
            entered: Arc::clone(&entered),
        };
        let mut shutdown = GracefulShutdown::new(Duration::from_secs(10)).unwrap();
        let shutdown_clock = clock();

        {
            let mut run =
                Box::pin(shutdown.run(&shutdown_clock, &mut supervisor, &mut handler, pending()));
            tokio::select! {
                () = entered.notified() => {}
                result = &mut run => panic!("blocked phase completed unexpectedly: {result:?}"),
            }
        }
        assert_eq!(*phases.lock().unwrap(), ORDERED_PHASES[..=3].to_vec());

        tokio::time::advance(Duration::from_secs(10)).await;
        let summary = tokio::time::timeout(
            Duration::from_millis(1),
            shutdown.run(&shutdown_clock, &mut supervisor, &mut handler, pending()),
        )
        .await
        .expect("retry must use the original elapsed runtime deadline")
        .unwrap();
        assert_eq!(
            summary.disposition(),
            ShutdownDisposition::GraceExpired {
                unfinished: UnfinishedWork::None
            }
        );
        assert_eq!(
            summary.deadline().time().duration_since_origin(),
            Duration::from_secs(15)
        );
        assert_eq!(
            *phases.lock().unwrap(),
            ORDERED_PHASES[..=3].to_vec(),
            "an elapsed retry must not reconstruct the incomplete phase"
        );
    }

    #[tokio::test]
    async fn cancellation_during_phase_drain_resumes_without_reentering_completed_handler() {
        let (release, wait_for_release) = tokio::sync::oneshot::channel();
        let cleanup_started = Arc::new(tokio::sync::Notify::new());
        let mut supervisor = TaskSupervisor::new();
        let started = Arc::clone(&cleanup_started);
        let metadata = TaskMetadata::new(
            TaskName::new("mutation_gate").unwrap(),
            TaskClassification::Critical,
            Some(ShutdownPhase::RejectNewMutations),
        )
        .unwrap();
        supervisor
            .spawn(metadata, move |token| async move {
                token.cancelled().await;
                started.notify_one();
                let _ = wait_for_release.await;
                Ok(())
            })
            .unwrap();
        let (mut handler, phases) = handler(None);
        let mut shutdown = GracefulShutdown::new(Duration::from_secs(30)).unwrap();
        let shutdown_clock = clock();

        {
            let mut run =
                Box::pin(shutdown.run(&shutdown_clock, &mut supervisor, &mut handler, pending()));
            tokio::select! {
                () = cleanup_started.notified() => {}
                result = &mut run => panic!("phase drain completed unexpectedly: {result:?}"),
            }
        }
        assert_eq!(
            *phases.lock().unwrap(),
            vec![ShutdownPhase::RejectNewMutations]
        );

        release.send(()).unwrap();
        let summary = shutdown
            .run(&shutdown_clock, &mut supervisor, &mut handler, pending())
            .await
            .unwrap();
        assert_eq!(summary.disposition(), ShutdownDisposition::Completed);
        assert_eq!(*phases.lock().unwrap(), ORDERED_PHASES);
        assert!(supervisor.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_while_fatal_peers_drain_retains_the_first_task_failure() {
        let (release, wait_for_release) = tokio::sync::oneshot::channel();
        let peer_waiting = Arc::new(tokio::sync::Notify::new());
        let mut supervisor = TaskSupervisor::new();
        let phase_metadata = |name| {
            TaskMetadata::new(
                TaskName::new(name).unwrap(),
                TaskClassification::Critical,
                Some(ShutdownPhase::RejectNewMutations),
            )
            .unwrap()
        };
        supervisor
            .spawn(phase_metadata("failing_gate"), |_| async {
                Err(HostError::new(HostErrorKind::TaskFailure))
            })
            .unwrap();
        let waiting = Arc::clone(&peer_waiting);
        supervisor
            .spawn(phase_metadata("draining_peer"), move |token| async move {
                token.cancelled().await;
                tokio::time::sleep(Duration::from_secs(1)).await;
                waiting.notify_one();
                let _ = wait_for_release.await;
                Ok(())
            })
            .unwrap();
        let (mut handler, phases) = handler(None);
        let mut shutdown = GracefulShutdown::new(Duration::from_secs(30)).unwrap();
        let shutdown_clock = clock();

        {
            let mut run =
                Box::pin(shutdown.run(&shutdown_clock, &mut supervisor, &mut handler, pending()));
            tokio::select! {
                () = peer_waiting.notified() => {}
                result = &mut run => panic!("fatal peer drain completed unexpectedly: {result:?}"),
            }
        }
        assert_eq!(
            shutdown.task_failure().map(SupervisionFailure::kind),
            Some(SupervisionFailureKind::TaskReturnedError)
        );

        release.send(()).unwrap();
        let summary = shutdown
            .run(&shutdown_clock, &mut supervisor, &mut handler, pending())
            .await
            .unwrap();
        assert_eq!(
            summary.disposition(),
            ShutdownDisposition::TaskFailed {
                kind: SupervisionFailureKind::TaskReturnedError
            }
        );
        assert_eq!(*phases.lock().unwrap(), ORDERED_PHASES);
        assert!(supervisor.is_empty());
    }

    #[test]
    fn zero_grace_fails_closed() {
        let error = GracefulShutdown::new(Duration::ZERO).err().unwrap();
        assert_eq!(error, ShutdownConfigError::ZeroGrace);
        assert_eq!(
            error.to_string(),
            "shutdown grace must be greater than zero"
        );
    }

    #[tokio::test]
    async fn unrepresentable_deadline_fails_before_side_effects() {
        let overflow_clock = FakeClock {
            now: MonotonicTime::from_duration_since_origin(Duration::MAX),
        };
        let mut supervisor = TaskSupervisor::new();
        let cancellation = supervisor.cancellation_token();
        let (mut handler, phases) = handler(None);
        let mut shutdown = GracefulShutdown::new(Duration::from_nanos(1)).unwrap();

        let error = shutdown
            .run(&overflow_clock, &mut supervisor, &mut handler, pending())
            .await
            .unwrap_err();
        assert_eq!(
            error,
            ShutdownStartError::Deadline(MonotonicClockError::DeadlineOverflow)
        );
        assert_eq!(
            error.to_string(),
            "shutdown grace deadline could not be represented"
        );
        assert!(error.source().is_some());
        assert!(phases.lock().unwrap().is_empty());
        assert!(!cancellation.is_cancelled());
        assert!(shutdown.phase_failure().is_none());
        assert!(shutdown.task_failure().is_none());
    }
}

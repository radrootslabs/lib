//! Join-owned task supervision with typed completion policy.

use core::{fmt, future::Future};
use std::{collections::HashMap, error::Error};

use tokio::task::{Id, JoinError, JoinSet};

use crate::HostError;

use super::{CancellationToken, ShutdownPhase, TaskClassification, TaskMetadata, UnfinishedWork};

/// Owns every spawned service task until its join result is observed.
#[must_use = "a task supervisor must be run or drained so authoritative tasks are joined"]
pub struct TaskSupervisor {
    cancellation: CancellationToken,
    tasks: JoinSet<TaskCompletion>,
    controls: HashMap<Id, TaskControl>,
}

struct TaskControl {
    metadata: TaskMetadata,
    cancellation: CancellationToken,
}

impl TaskSupervisor {
    pub fn new() -> Self {
        Self {
            cancellation: CancellationToken::new(),
            tasks: JoinSet::new(),
            controls: HashMap::new(),
        }
    }

    /// Returns a cloneable observer and cancellation authority for composition boundaries.
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    /// Requests cooperative cancellation of every task child token.
    pub fn request_cancellation(&self) {
        self.cancellation.cancel();
    }

    #[must_use]
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Registers and spawns one task on the current runtime without exposing a detachable handle.
    pub fn spawn<F, Fut>(
        &mut self,
        metadata: TaskMetadata,
        task: F,
    ) -> Result<(), TaskRegistrationError>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), HostError>> + Send + 'static,
    {
        if self
            .controls
            .values()
            .any(|active| active.metadata.name() == metadata.name())
        {
            return Err(TaskRegistrationError::DuplicateName);
        }
        let runtime =
            tokio::runtime::Handle::try_current().map_err(|_| TaskRegistrationError::NoRuntime)?;
        let task_metadata = metadata.clone();
        let child = self.cancellation.child_token();
        let completion_observer = child.clone();
        let phase_cancellation = child.clone();
        let abort = self.tasks.spawn_on(
            async move {
                let result = task(child).await;
                TaskCompletion {
                    metadata: task_metadata,
                    cancelled_at_return: completion_observer.is_cancelled(),
                    result,
                }
            },
            &runtime,
        );
        self.controls.insert(
            abort.id(),
            TaskControl {
                metadata,
                cancellation: phase_cancellation,
            },
        );
        Ok(())
    }

    pub(crate) fn request_phase_cancellation(&self, phase: ShutdownPhase) {
        for control in self.controls.values() {
            if control.metadata.shutdown_phase() == Some(phase) {
                control.cancellation.cancel();
            }
        }
    }

    pub(crate) async fn supervise_phase(
        &mut self,
        phase: ShutdownPhase,
    ) -> Result<Vec<SupervisedTaskExit>, SupervisionFailure> {
        let mut exits = Vec::new();
        while self.has_phase_work(phase) {
            let outcome = self
                .join_next()
                .await
                .expect("phase work must retain a join-owned task");
            match outcome {
                Ok(exit) => exits.push(exit),
                Err(failure) => return Err(failure),
            }
        }
        Ok(exits)
    }

    /// Observes all task exits, cancels peers on the first fatal outcome, and drains every join.
    pub async fn supervise(&mut self) -> Result<Vec<SupervisedTaskExit>, SupervisionFailure> {
        let mut exits = Vec::with_capacity(self.tasks.len());
        let mut first_failure = None;
        while let Some(outcome) = self.join_next().await {
            match outcome {
                Ok(exit) => exits.push(exit),
                Err(failure) => {
                    if first_failure.is_none() {
                        first_failure = Some(failure);
                    }
                }
            }
        }
        if let Some(failure) = first_failure {
            Err(failure)
        } else {
            Ok(exits)
        }
    }

    /// Returns the next classified task outcome as soon as it is joined.
    ///
    /// Optional failures are returned without cancelling peers. A fatal
    /// outcome requests cancellation before it is returned. Cancelling this
    /// wait does not remove a task or lose its later outcome.
    pub async fn join_next(&mut self) -> Option<Result<SupervisedTaskExit, SupervisionFailure>> {
        let joined = self.tasks.join_next_with_id().await?;
        let outcome = self.classify_join(joined);
        if outcome.is_err() {
            self.cancellation.cancel();
        }
        Some(outcome)
    }

    fn classify_join(
        &mut self,
        joined: Result<(Id, TaskCompletion), JoinError>,
    ) -> Result<SupervisedTaskExit, SupervisionFailure> {
        match joined {
            Ok((id, completion)) => {
                self.controls.remove(&id);
                classify_completion(completion)
            }
            Err(error) => {
                let metadata = self
                    .controls
                    .remove(&error.id())
                    .map(|control| control.metadata);
                let cancelled_during_shutdown =
                    error.is_cancelled() && self.cancellation.is_cancelled();
                if cancelled_during_shutdown && let Some(metadata) = metadata {
                    return Ok(SupervisedTaskExit::expected(metadata));
                }
                let kind = if error.is_panic() {
                    SupervisionFailureKind::TaskPanicked
                } else if error.is_cancelled() {
                    SupervisionFailureKind::UnexpectedCancellation
                } else {
                    SupervisionFailureKind::JoinFailed
                };
                match metadata {
                    Some(metadata) if !metadata.classification().failure_is_fatal() => Ok(
                        SupervisedTaskExit::optional_failure(metadata, Box::new(error)),
                    ),
                    metadata => Err(SupervisionFailure::new(
                        metadata,
                        kind,
                        Some(Box::new(error)),
                    )),
                }
            }
        }
    }

    pub(crate) fn unfinished_work(&self) -> UnfinishedWork {
        if self.controls.is_empty() {
            UnfinishedWork::None
        } else if self
            .controls
            .values()
            .any(|control| control.metadata.classification().failure_is_fatal())
        {
            UnfinishedWork::FatalAuthoritative
        } else {
            UnfinishedWork::RecoverableOptional
        }
    }

    pub(crate) async fn abort_and_drain(&mut self) {
        self.cancellation.cancel();
        self.tasks.abort_all();
        while self.join_next().await.is_some() {}
    }

    fn has_phase_work(&self, phase: ShutdownPhase) -> bool {
        self.controls.values().any(|control| {
            control.metadata.shutdown_phase() == Some(phase)
                || (phase == ShutdownPhase::DrainOperations
                    && control.metadata.classification() == TaskClassification::OneShot)
        })
    }
}

impl Default for TaskSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

fn classify_completion(
    completion: TaskCompletion,
) -> Result<SupervisedTaskExit, SupervisionFailure> {
    match completion.result {
        Ok(())
            if completion.metadata.classification() == TaskClassification::Critical
                && !completion.cancelled_at_return =>
        {
            Err(SupervisionFailure::new(
                Some(completion.metadata),
                SupervisionFailureKind::UnexpectedCompletion,
                None,
            ))
        }
        Ok(()) => Ok(SupervisedTaskExit::expected(completion.metadata)),
        Err(error) if completion.metadata.classification().failure_is_fatal() => {
            Err(SupervisionFailure::new(
                Some(completion.metadata),
                SupervisionFailureKind::TaskReturnedError,
                Some(Box::new(error)),
            ))
        }
        Err(error) => Ok(SupervisedTaskExit::optional_failure(
            completion.metadata,
            Box::new(error),
        )),
    }
}

struct TaskCompletion {
    metadata: TaskMetadata,
    cancelled_at_return: bool,
    result: Result<(), HostError>,
}

/// Nonfatal observed task completion retained for metrics and status consumers.
pub struct SupervisedTaskExit {
    metadata: TaskMetadata,
    status: SupervisedTaskExitStatus,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl SupervisedTaskExit {
    fn expected(metadata: TaskMetadata) -> Self {
        Self {
            metadata,
            status: SupervisedTaskExitStatus::ExpectedCompletion,
            source: None,
        }
    }

    fn optional_failure(
        metadata: TaskMetadata,
        source: Box<dyn Error + Send + Sync + 'static>,
    ) -> Self {
        Self {
            metadata,
            status: SupervisedTaskExitStatus::OptionalFailure,
            source: Some(source),
        }
    }

    #[must_use]
    pub const fn metadata(&self) -> &TaskMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn status(&self) -> SupervisedTaskExitStatus {
        self.status
    }

    /// Returns the trusted internal cause for an optional failure.
    #[must_use]
    pub fn source(&self) -> Option<&(dyn Error + Send + Sync + 'static)> {
        self.source.as_deref()
    }
}

impl fmt::Debug for SupervisedTaskExit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SupervisedTaskExit")
            .field("metadata", &self.metadata)
            .field("status", &self.status)
            .field("source", &self.source.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisedTaskExitStatus {
    ExpectedCompletion,
    OptionalFailure,
}

/// Fatal supervisor outcome after every remaining task has been joined.
pub struct SupervisionFailure {
    metadata: Option<TaskMetadata>,
    kind: SupervisionFailureKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl SupervisionFailure {
    fn new(
        metadata: Option<TaskMetadata>,
        kind: SupervisionFailureKind,
        source: Option<Box<dyn Error + Send + Sync + 'static>>,
    ) -> Self {
        Self {
            metadata,
            kind,
            source,
        }
    }

    #[must_use]
    pub const fn metadata(&self) -> Option<&TaskMetadata> {
        self.metadata.as_ref()
    }

    #[must_use]
    pub const fn kind(&self) -> SupervisionFailureKind {
        self.kind
    }
}

impl fmt::Debug for SupervisionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SupervisionFailure")
            .field("metadata", &self.metadata)
            .field("kind", &self.kind)
            .field("source", &self.source.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl fmt::Display for SupervisionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("authoritative service task supervision failed")
    }
}

impl Error for SupervisionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisionFailureKind {
    TaskReturnedError,
    TaskPanicked,
    UnexpectedCompletion,
    UnexpectedCancellation,
    JoinFailed,
}

/// Failure to register a task without spawning it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskRegistrationError {
    DuplicateName,
    NoRuntime,
}

impl fmt::Display for TaskRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DuplicateName => "supervised task name is already active",
            Self::NoRuntime => "supervised task registration requires an active runtime",
        })
    }
}

impl Error for TaskRegistrationError {}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use crate::{HostErrorKind, ShutdownPhase, TaskClassification, TaskName};

    use super::*;

    fn metadata(name: &str, classification: TaskClassification) -> TaskMetadata {
        let shutdown_phase = classification
            .requires_shutdown_phase()
            .then_some(ShutdownPhase::CancelIngress);
        TaskMetadata::new(TaskName::new(name).unwrap(), classification, shutdown_phase).unwrap()
    }

    fn metadata_at(
        name: &str,
        classification: TaskClassification,
        shutdown_phase: Option<ShutdownPhase>,
    ) -> TaskMetadata {
        TaskMetadata::new(TaskName::new(name).unwrap(), classification, shutdown_phase).unwrap()
    }

    #[test]
    fn registration_without_a_runtime_fails_before_spawning() {
        let mut supervisor = TaskSupervisor::new();
        assert_eq!(
            supervisor.spawn(
                metadata("critical_worker", TaskClassification::Critical),
                |_| async { Ok(()) },
            ),
            Err(TaskRegistrationError::NoRuntime)
        );
        assert!(supervisor.is_empty());
    }

    #[tokio::test]
    async fn critical_error_cancels_peers_propagates_and_drains_all_joins() {
        let drained = Arc::new(AtomicUsize::new(0));
        let mut supervisor = TaskSupervisor::new();
        let peer_drained = Arc::clone(&drained);
        supervisor
            .spawn(
                metadata("peer_worker", TaskClassification::Critical),
                move |token| async move {
                    token.cancelled().await;
                    peer_drained.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            )
            .unwrap();
        supervisor
            .spawn(
                metadata("failing_worker", TaskClassification::Critical),
                |_| async { Err(HostError::new(HostErrorKind::TaskFailure)) },
            )
            .unwrap();

        let error = supervisor.supervise().await.unwrap_err();
        assert_eq!(error.kind(), SupervisionFailureKind::TaskReturnedError);
        assert_eq!(error.metadata().unwrap().name().as_str(), "failing_worker");
        assert!(error.source().is_some());
        assert_eq!(drained.load(Ordering::SeqCst), 1);
        assert!(supervisor.is_empty());
        assert!(supervisor.cancellation_token().is_cancelled());
    }

    #[tokio::test]
    async fn phase_cancellation_stops_and_joins_only_the_assigned_tasks() {
        let ingress_stopped = Arc::new(AtomicUsize::new(0));
        let network_stopped = Arc::new(AtomicUsize::new(0));
        let mut supervisor = TaskSupervisor::new();
        let ingress = Arc::clone(&ingress_stopped);
        supervisor
            .spawn(
                metadata_at(
                    "ingress_worker",
                    TaskClassification::Critical,
                    Some(ShutdownPhase::CancelIngress),
                ),
                move |token| async move {
                    token.cancelled().await;
                    ingress.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            )
            .unwrap();
        let network = Arc::clone(&network_stopped);
        supervisor
            .spawn(
                metadata_at(
                    "network_worker",
                    TaskClassification::Critical,
                    Some(ShutdownPhase::CloseNetwork),
                ),
                move |token| async move {
                    token.cancelled().await;
                    network.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            )
            .unwrap();

        supervisor.request_phase_cancellation(ShutdownPhase::CancelIngress);
        let ingress_exits = supervisor
            .supervise_phase(ShutdownPhase::CancelIngress)
            .await
            .unwrap();
        assert_eq!(ingress_exits.len(), 1);
        assert_eq!(ingress_stopped.load(Ordering::SeqCst), 1);
        assert_eq!(network_stopped.load(Ordering::SeqCst), 0);
        assert_eq!(supervisor.task_count(), 1);

        supervisor.request_phase_cancellation(ShutdownPhase::CloseNetwork);
        let network_exits = supervisor
            .supervise_phase(ShutdownPhase::CloseNetwork)
            .await
            .unwrap();
        assert_eq!(network_exits.len(), 1);
        assert_eq!(network_stopped.load(Ordering::SeqCst), 1);
        assert!(supervisor.is_empty());
    }

    #[tokio::test]
    async fn drain_operations_joins_one_shot_work_without_cancelling_it() {
        let mut supervisor = TaskSupervisor::new();
        supervisor
            .spawn(
                metadata_at("bounded_operation", TaskClassification::OneShot, None),
                |token| async move {
                    assert!(!token.is_cancelled());
                    Ok(())
                },
            )
            .unwrap();

        supervisor.request_phase_cancellation(ShutdownPhase::DrainOperations);
        let exits = supervisor
            .supervise_phase(ShutdownPhase::DrainOperations)
            .await
            .unwrap();
        assert_eq!(exits.len(), 1);
        assert_eq!(
            exits[0].status(),
            SupervisedTaskExitStatus::ExpectedCompletion
        );
        assert!(supervisor.is_empty());
    }

    #[tokio::test]
    async fn critical_panic_and_early_success_are_fatal() {
        for (name, expected, panic_task) in [
            ("panic_worker", SupervisionFailureKind::TaskPanicked, true),
            (
                "early_worker",
                SupervisionFailureKind::UnexpectedCompletion,
                false,
            ),
        ] {
            let mut supervisor = TaskSupervisor::new();
            supervisor
                .spawn(
                    metadata(name, TaskClassification::Critical),
                    move |_| async move {
                        assert!(!panic_task, "sensitive panic payload");
                        Ok(())
                    },
                )
                .unwrap();
            let error = supervisor.supervise().await.unwrap_err();
            assert_eq!(error.kind(), expected);
            assert!(!error.to_string().contains("sensitive"));
            assert!(supervisor.is_empty());
        }
    }

    #[tokio::test]
    async fn optional_error_and_one_shot_success_are_observed_without_failure() {
        let mut supervisor = TaskSupervisor::new();
        supervisor
            .spawn(
                metadata("optional_worker", TaskClassification::Optional),
                |_| async { Err(HostError::new(HostErrorKind::TaskFailure)) },
            )
            .unwrap();
        supervisor
            .spawn(
                metadata("startup_once", TaskClassification::OneShot),
                |_| async { Ok(()) },
            )
            .unwrap();

        let mut exits = supervisor.supervise().await.unwrap();
        exits.sort_by(|left, right| left.metadata().name().cmp(right.metadata().name()));
        assert_eq!(exits.len(), 2);
        assert_eq!(exits[0].status(), SupervisedTaskExitStatus::OptionalFailure);
        assert!(exits[0].source().is_some());
        assert_eq!(
            exits[1].status(),
            SupervisedTaskExitStatus::ExpectedCompletion
        );
        assert!(supervisor.is_empty());
    }

    #[tokio::test]
    async fn optional_failure_is_observable_while_a_critical_peer_is_alive() {
        let mut supervisor = TaskSupervisor::new();
        supervisor
            .spawn(
                metadata("critical_worker", TaskClassification::Critical),
                |token| async move {
                    token.cancelled().await;
                    Ok(())
                },
            )
            .unwrap();
        supervisor
            .spawn(
                metadata("optional_worker", TaskClassification::Optional),
                |_| async { Err(HostError::new(HostErrorKind::TaskFailure)) },
            )
            .unwrap();

        let exit = supervisor.join_next().await.unwrap().unwrap();
        assert_eq!(exit.metadata().name().as_str(), "optional_worker");
        assert_eq!(exit.status(), SupervisedTaskExitStatus::OptionalFailure);
        assert_eq!(supervisor.task_count(), 1);
        assert!(!supervisor.cancellation_token().is_cancelled());

        supervisor.request_cancellation();
        assert!(supervisor.join_next().await.unwrap().is_ok());
        assert!(supervisor.is_empty());
    }

    #[tokio::test]
    async fn optional_failure_remains_observed_before_a_later_fatal_exit() {
        let (release_fatal, wait_for_release) = tokio::sync::oneshot::channel();
        let mut supervisor = TaskSupervisor::new();
        supervisor
            .spawn(
                metadata("fatal_worker", TaskClassification::Critical),
                |_| async move {
                    let _ = wait_for_release.await;
                    Err(HostError::new(HostErrorKind::TaskFailure))
                },
            )
            .unwrap();
        supervisor
            .spawn(
                metadata("optional_worker", TaskClassification::Optional),
                |_| async { Err(HostError::new(HostErrorKind::TaskFailure)) },
            )
            .unwrap();

        let optional = supervisor.join_next().await.unwrap().unwrap();
        assert_eq!(optional.metadata().name().as_str(), "optional_worker");
        assert_eq!(optional.status(), SupervisedTaskExitStatus::OptionalFailure);
        release_fatal.send(()).unwrap();

        let fatal = supervisor.join_next().await.unwrap().unwrap_err();
        assert_eq!(fatal.kind(), SupervisionFailureKind::TaskReturnedError);
        assert_eq!(fatal.metadata().unwrap().name().as_str(), "fatal_worker");
        assert!(supervisor.cancellation_token().is_cancelled());
        assert!(supervisor.is_empty());
    }

    #[tokio::test]
    async fn supervision_retains_only_the_first_of_multiple_fatal_failures() {
        let mut supervisor = TaskSupervisor::new();
        for name in ["first_failure", "second_failure"] {
            supervisor
                .spawn(metadata(name, TaskClassification::Critical), |_| async {
                    Err(HostError::new(HostErrorKind::TaskFailure))
                })
                .unwrap();
        }

        let error = supervisor.supervise().await.unwrap_err();
        assert_eq!(error.kind(), SupervisionFailureKind::TaskReturnedError);
        assert!(matches!(
            error.metadata().unwrap().name().as_str(),
            "first_failure" | "second_failure"
        ));
        assert!(supervisor.is_empty());
    }

    #[tokio::test]
    async fn optional_panic_is_an_observable_nonfatal_exit() {
        let mut supervisor = TaskSupervisor::new();
        supervisor
            .spawn(
                metadata("optional_panic", TaskClassification::Optional),
                |_| async {
                    panic!("redacted optional panic");
                    #[allow(unreachable_code)]
                    Ok(())
                },
            )
            .unwrap();

        let exit = supervisor.join_next().await.unwrap().unwrap();
        assert_eq!(exit.status(), SupervisedTaskExitStatus::OptionalFailure);
        assert!(exit.source().is_some());
        assert!(supervisor.is_empty());
    }

    #[tokio::test]
    async fn externally_cancelled_critical_task_may_complete_successfully() {
        let mut supervisor = TaskSupervisor::new();
        supervisor
            .spawn(
                metadata("critical_worker", TaskClassification::Critical),
                |token| async move {
                    token.cancelled().await;
                    Ok(())
                },
            )
            .unwrap();
        supervisor.request_cancellation();

        let exits = supervisor.supervise().await.unwrap();
        assert_eq!(exits.len(), 1);
        assert_eq!(
            exits[0].status(),
            SupervisedTaskExitStatus::ExpectedCompletion
        );
        assert!(supervisor.is_empty());
    }

    #[tokio::test]
    async fn duplicate_active_names_are_rejected_without_detaching_work() {
        let mut supervisor = TaskSupervisor::new();
        supervisor
            .spawn(
                metadata("same_worker", TaskClassification::Critical),
                |token| async move {
                    token.cancelled().await;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(
            supervisor.spawn(
                metadata("same_worker", TaskClassification::Critical),
                |_| async { Ok(()) },
            ),
            Err(TaskRegistrationError::DuplicateName)
        );
        assert_eq!(supervisor.task_count(), 1);
        supervisor.request_cancellation();
        assert!(supervisor.supervise().await.is_ok());
    }
}

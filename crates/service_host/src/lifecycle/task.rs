//! Validated metadata for every supervisor-owned task.

use core::fmt;

use serde::Serialize;

pub const TASK_NAME_MAX_BYTES: usize = 64;

/// A stable, non-secret identifier for one supervised task.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct TaskName(String);

impl TaskName {
    pub fn new(value: impl AsRef<str>) -> Result<Self, TaskMetadataError> {
        let value = value.as_ref();
        if !valid_task_name(value) {
            return Err(TaskMetadataError::InvalidTaskName);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Service impact and lifetime class for a supervised task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClassification {
    /// A long-lived authoritative task whose failure or early success fails the service.
    Critical,
    /// A bounded optional capability whose failure is observable but not service-fatal.
    Optional,
    /// A finite authoritative operation whose successful completion is expected.
    OneShot,
}

impl TaskClassification {
    #[must_use]
    pub const fn completion_expectation(self) -> TaskCompletionExpectation {
        match self {
            Self::Critical => TaskCompletionExpectation::AfterCancellation,
            Self::Optional => TaskCompletionExpectation::MayComplete,
            Self::OneShot => TaskCompletionExpectation::CompletesOnce,
        }
    }

    /// Returns whether a returned error or panic fails the service.
    #[must_use]
    pub const fn failure_is_fatal(self) -> bool {
        matches!(self, Self::Critical | Self::OneShot)
    }

    #[must_use]
    pub const fn requires_shutdown_phase(self) -> bool {
        !matches!(self, Self::OneShot)
    }
}

/// When successful completion is valid for a task class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskCompletionExpectation {
    AfterCancellation,
    MayComplete,
    CompletesOnce,
}

/// Ordered shutdown phase assigned to each long-lived task.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownPhase {
    RejectNewMutations,
    CancelIngress,
    DrainOperations,
    PersistRecoverableWork,
    CloseNetwork,
    CloseSqlite,
    CloseSockets,
}

/// Complete static identity and lifecycle ownership for one task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TaskMetadata {
    name: TaskName,
    classification: TaskClassification,
    shutdown_phase: Option<ShutdownPhase>,
}

impl TaskMetadata {
    pub fn new(
        name: TaskName,
        classification: TaskClassification,
        shutdown_phase: Option<ShutdownPhase>,
    ) -> Result<Self, TaskMetadataError> {
        if classification.requires_shutdown_phase() != shutdown_phase.is_some() {
            return Err(TaskMetadataError::InvalidShutdownPhaseAssignment);
        }
        Ok(Self {
            name,
            classification,
            shutdown_phase,
        })
    }

    #[must_use]
    pub const fn name(&self) -> &TaskName {
        &self.name
    }

    #[must_use]
    pub const fn classification(&self) -> TaskClassification {
        self.classification
    }

    #[must_use]
    pub const fn shutdown_phase(&self) -> Option<ShutdownPhase> {
        self.shutdown_phase
    }
}

/// Validation failure for static supervisor metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskMetadataError {
    InvalidTaskName,
    InvalidShutdownPhaseAssignment,
}

impl fmt::Display for TaskMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidTaskName => "supervised task name is invalid",
            Self::InvalidShutdownPhaseAssignment => {
                "supervised task shutdown phase does not match its classification"
            }
        })
    }
}

impl std::error::Error for TaskMetadataError {}

fn valid_task_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= TASK_NAME_MAX_BYTES
        && first.is_ascii_lowercase()
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_names_are_bounded_stable_and_secret_safe() {
        for valid in ["a", "admin_listener", "outbox_worker_01"] {
            assert_eq!(TaskName::new(valid).unwrap().as_str(), valid);
        }
        assert!(TaskName::new("a".repeat(TASK_NAME_MAX_BYTES)).is_ok());
        for invalid in [
            "",
            "AdminListener",
            "1_worker",
            "admin-listener",
            "admin.listener",
            "admin listener",
            "worker/secret",
            "café",
        ] {
            assert_eq!(
                TaskName::new(invalid),
                Err(TaskMetadataError::InvalidTaskName)
            );
        }
        assert!(TaskName::new("a".repeat(TASK_NAME_MAX_BYTES + 1)).is_err());
        assert_eq!(
            TaskName::new("a".repeat(4 * 1024 * 1024)),
            Err(TaskMetadataError::InvalidTaskName)
        );
    }

    #[test]
    fn classification_semantics_are_exhaustive() {
        let expected = [
            (
                TaskClassification::Critical,
                TaskCompletionExpectation::AfterCancellation,
                true,
                true,
            ),
            (
                TaskClassification::Optional,
                TaskCompletionExpectation::MayComplete,
                false,
                true,
            ),
            (
                TaskClassification::OneShot,
                TaskCompletionExpectation::CompletesOnce,
                true,
                false,
            ),
        ];
        for (classification, completion, fatal_failure, requires_shutdown) in expected {
            assert_eq!(classification.completion_expectation(), completion);
            assert_eq!(classification.failure_is_fatal(), fatal_failure);
            assert_eq!(classification.requires_shutdown_phase(), requires_shutdown);
        }
    }

    #[test]
    fn shutdown_phase_assignment_matches_task_lifetime() {
        let name = || TaskName::new("relay_subscription").unwrap();
        assert!(
            TaskMetadata::new(
                name(),
                TaskClassification::Critical,
                Some(ShutdownPhase::CancelIngress),
            )
            .is_ok()
        );
        let optional = TaskMetadata::new(
            name(),
            TaskClassification::Optional,
            Some(ShutdownPhase::CloseNetwork),
        )
        .unwrap();
        assert_eq!(optional.shutdown_phase(), Some(ShutdownPhase::CloseNetwork));
        assert_eq!(
            TaskMetadata::new(name(), TaskClassification::Optional, None),
            Err(TaskMetadataError::InvalidShutdownPhaseAssignment)
        );
        assert_eq!(
            TaskMetadata::new(
                name(),
                TaskClassification::OneShot,
                Some(ShutdownPhase::CancelIngress),
            ),
            Err(TaskMetadataError::InvalidShutdownPhaseAssignment)
        );
        assert!(TaskMetadata::new(name(), TaskClassification::OneShot, None).is_ok());
    }

    #[test]
    fn serde_debug_and_shutdown_order_are_stable() {
        let phases = [
            ShutdownPhase::RejectNewMutations,
            ShutdownPhase::CancelIngress,
            ShutdownPhase::DrainOperations,
            ShutdownPhase::PersistRecoverableWork,
            ShutdownPhase::CloseNetwork,
            ShutdownPhase::CloseSqlite,
            ShutdownPhase::CloseSockets,
        ];
        assert!(phases.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            serde_json::to_string(&phases).unwrap(),
            r#"["reject_new_mutations","cancel_ingress","drain_operations","persist_recoverable_work","close_network","close_sqlite","close_sockets"]"#
        );

        let metadata = TaskMetadata::new(
            TaskName::new("admin_listener").unwrap(),
            TaskClassification::Critical,
            Some(ShutdownPhase::CloseSockets),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&metadata).unwrap(),
            r#"{"name":"admin_listener","classification":"critical","shutdown_phase":"close_sockets"}"#
        );
        assert_eq!(
            format!("{metadata:?}"),
            "TaskMetadata { name: TaskName(\"admin_listener\"), classification: Critical, shutdown_phase: Some(CloseSockets) }"
        );
    }
}

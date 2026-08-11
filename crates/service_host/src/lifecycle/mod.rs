//! Explicit task ownership and service lifecycle mechanics.

mod cancel;
mod supervisor;
mod task;

pub use cancel::CancellationToken;
pub use supervisor::{
    SupervisedTaskExit, SupervisedTaskExitStatus, SupervisionFailure, SupervisionFailureKind,
    TaskRegistrationError, TaskSupervisor,
};
pub use task::{
    ShutdownPhase, TaskClassification, TaskCompletionExpectation, TaskMetadata, TaskMetadataError,
    TaskName,
};

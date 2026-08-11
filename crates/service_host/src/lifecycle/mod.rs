//! Explicit task ownership and service lifecycle mechanics.

mod cancel;
mod shutdown;
mod supervisor;
mod task;

pub use cancel::CancellationToken;
pub use shutdown::{
    GracefulShutdown, ShutdownConfigError, ShutdownDisposition, ShutdownPhaseFailure,
    ShutdownPhaseFuture, ShutdownPhaseHandler, ShutdownStartError, ShutdownSummary, UnfinishedWork,
};
pub use supervisor::{
    SupervisedTaskExit, SupervisedTaskExitStatus, SupervisionFailure, SupervisionFailureKind,
    TaskRegistrationError, TaskSupervisor,
};
pub use task::{
    ShutdownPhase, TaskClassification, TaskCompletionExpectation, TaskMetadata, TaskMetadataError,
    TaskName,
};

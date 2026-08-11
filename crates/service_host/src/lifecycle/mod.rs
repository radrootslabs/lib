//! Explicit task ownership and service lifecycle mechanics.

mod cancel;
mod task;

pub use cancel::CancellationToken;
pub use task::{
    ShutdownPhase, TaskClassification, TaskCompletionExpectation, TaskMetadata, TaskMetadataError,
    TaskName,
};

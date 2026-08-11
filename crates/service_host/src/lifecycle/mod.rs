//! Explicit task ownership and service lifecycle mechanics.

mod task;

pub use task::{
    ShutdownPhase, TaskClassification, TaskCompletionExpectation, TaskMetadata, TaskMetadataError,
    TaskName,
};

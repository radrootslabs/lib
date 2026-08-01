//! Stable, secret-safe storage failures.

use core::fmt;

/// Backend-neutral storage failure.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidSourceGeneration,
    InvalidEventSequence,
    InvalidEventQueryLimit,
    EmptyEventQueryIds,
    TooManyEventQueryIds,
    DuplicateEventQueryId,
    AdmissionEventMismatch,
    AdmissionRegression,
    EventConflict,
    EventPageLimitExceeded,
    CursorGenerationMismatch,
    SourceGenerationChanged,
    EventNotFound,
    CorruptStoredEvent,
    BackendUnavailable,
    InvalidOperationInstanceId,
    InvalidIdempotencyKey,
    InvalidOperationTimestamp,
    InvalidJournalRevision,
    InvalidRecoveryAttempt,
    InvalidRecoveryDeadline,
    InvalidJournalQueryLimit,
    IdempotencyConflict,
    OperationNotFound,
    OperationIdentityMismatch,
    JournalRevisionConflict,
    InvalidJournalTransition,
    JournalOperationCommitted,
    CorruptJournalRecord,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSourceGeneration => "storage source generation is invalid",
            Self::InvalidEventSequence => "storage event sequence is invalid",
            Self::InvalidEventQueryLimit => "storage event query limit is invalid",
            Self::EmptyEventQueryIds => "storage event id query is empty",
            Self::TooManyEventQueryIds => "storage event id query exceeds its limit",
            Self::DuplicateEventQueryId => "storage event id query contains a duplicate",
            Self::AdmissionEventMismatch => "storage admission event identities do not match",
            Self::AdmissionRegression => "storage admission cannot regress event state",
            Self::EventConflict => "storage contains conflicting data for the event id",
            Self::EventPageLimitExceeded => "storage event page exceeds its requested limit",
            Self::CursorGenerationMismatch => "storage cursor belongs to another source generation",
            Self::SourceGenerationChanged => "storage source generation changed",
            Self::EventNotFound => "storage event was not found",
            Self::CorruptStoredEvent => "storage event data is corrupt",
            Self::BackendUnavailable => "storage backend is unavailable",
            Self::InvalidOperationInstanceId => "storage operation instance id is invalid",
            Self::InvalidIdempotencyKey => "storage idempotency key is invalid",
            Self::InvalidOperationTimestamp => "storage operation timestamp is invalid",
            Self::InvalidJournalRevision => "storage journal revision is invalid",
            Self::InvalidRecoveryAttempt => "storage recovery attempt is invalid",
            Self::InvalidRecoveryDeadline => "storage recovery deadline is invalid",
            Self::InvalidJournalQueryLimit => "storage journal query limit is invalid",
            Self::IdempotencyConflict => "storage idempotency key conflicts with prior input",
            Self::OperationNotFound => "storage journal operation was not found",
            Self::OperationIdentityMismatch => "storage journal operation identity does not match",
            Self::JournalRevisionConflict => {
                "storage journal revision conflicts with durable state"
            }
            Self::InvalidJournalTransition => "storage journal transition is invalid",
            Self::JournalOperationCommitted => "storage journal operation is already committed",
            Self::CorruptJournalRecord => "storage journal record is corrupt",
        })
    }
}

impl std::error::Error for Error {}

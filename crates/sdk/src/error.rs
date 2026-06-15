#[cfg(feature = "runtime")]
use std::{fmt, path::PathBuf};

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsSdkRecoveryAction {
    RetryOutboxEnqueue,
    InspectLocalStores,
    RetryOperationWithSameIdempotencyKey,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsSdkPartialLocalMutationFailure {
    OutboxEnqueue,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSdkPartialLocalMutationError {
    pub event_id: Option<String>,
    pub operation_kind: String,
    pub idempotency_digest_prefix: Option<String>,
    pub stored: bool,
    pub queued: bool,
    pub recovery: RadrootsSdkRecoveryAction,
    pub failure: RadrootsSdkPartialLocalMutationFailure,
}

#[cfg(feature = "runtime")]
#[derive(Debug)]
pub enum RadrootsSdkError {
    Io { path: PathBuf, message: String },
    ClockBeforeUnixEpoch,
    TimestampOutOfRange { value: u64 },
    Authority { message: String },
    EventStore { message: String },
    InvalidRequest { message: String },
    ListingDraft { message: String },
    ListingMutation { message: String },
    Outbox { message: String },
    RelayTransport { message: String },
    Projection { message: String },
    PartialLocalMutation(RadrootsSdkPartialLocalMutationError),
}

#[cfg(feature = "runtime")]
impl RadrootsSdkError {
    pub fn partial_local_mutation(error: RadrootsSdkPartialLocalMutationError) -> Self {
        Self::PartialLocalMutation(error)
    }

    pub fn partial_outbox_enqueue_mutation(
        event_id: impl Into<String>,
        operation_kind: impl Into<String>,
        idempotency_digest_prefix: impl Into<String>,
    ) -> Self {
        Self::PartialLocalMutation(RadrootsSdkPartialLocalMutationError {
            event_id: Some(event_id.into()),
            operation_kind: operation_kind.into(),
            idempotency_digest_prefix: Some(idempotency_digest_prefix.into()),
            stored: true,
            queued: false,
            recovery: RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey,
            failure: RadrootsSdkPartialLocalMutationFailure::OutboxEnqueue,
        })
    }
}

#[cfg(feature = "runtime")]
impl fmt::Display for RadrootsSdkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(f, "sdk storage path `{}` failed: {message}", path.display())
            }
            Self::ClockBeforeUnixEpoch => f.write_str("sdk clock is before the Unix epoch"),
            Self::TimestampOutOfRange { value } => {
                write!(
                    f,
                    "sdk timestamp {value} exceeds Nostr u32 created_at range"
                )
            }
            Self::Authority { message } => write!(f, "sdk authority error: {message}"),
            Self::EventStore { message } => write!(f, "sdk event store error: {message}"),
            Self::InvalidRequest { message } => write!(f, "sdk invalid request: {message}"),
            Self::ListingDraft { message } => write!(f, "sdk listing draft error: {message}"),
            Self::ListingMutation { message } => {
                write!(f, "sdk listing mutation error: {message}")
            }
            Self::Outbox { message } => write!(f, "sdk outbox error: {message}"),
            Self::RelayTransport { message } => {
                write!(f, "sdk relay transport error: {message}")
            }
            Self::Projection { message } => write!(f, "sdk projection error: {message}"),
            Self::PartialLocalMutation(error) => write!(
                f,
                "sdk local mutation partially completed: event_id={}, operation_kind={}, idempotency_digest_prefix={}, stored={}, queued={}, failure={:?}, recovery={:?}",
                error.event_id.as_deref().unwrap_or("<unknown>"),
                error.operation_kind,
                error
                    .idempotency_digest_prefix
                    .as_deref()
                    .unwrap_or("<none>"),
                error.stored,
                error.queued,
                error.failure,
                error.recovery
            ),
        }
    }
}

#[cfg(feature = "runtime")]
impl std::error::Error for RadrootsSdkError {}

#[cfg(feature = "runtime")]
impl From<radroots_authority::RadrootsAuthorityError> for RadrootsSdkError {
    fn from(error: radroots_authority::RadrootsAuthorityError) -> Self {
        Self::Authority {
            message: error.to_string(),
        }
    }
}

#[cfg(feature = "runtime")]
impl From<radroots_event_store::RadrootsEventStoreError> for RadrootsSdkError {
    fn from(error: radroots_event_store::RadrootsEventStoreError) -> Self {
        Self::EventStore {
            message: error.to_string(),
        }
    }
}

#[cfg(feature = "runtime")]
impl From<radroots_trade::listing::RadrootsListingDraftError> for RadrootsSdkError {
    fn from(error: radroots_trade::listing::RadrootsListingDraftError) -> Self {
        Self::ListingDraft {
            message: error.to_string(),
        }
    }
}

#[cfg(feature = "runtime")]
impl From<radroots_trade::listing::RadrootsListingMutationError> for RadrootsSdkError {
    fn from(error: radroots_trade::listing::RadrootsListingMutationError) -> Self {
        Self::ListingMutation {
            message: error.to_string(),
        }
    }
}

#[cfg(feature = "runtime")]
impl From<radroots_outbox::RadrootsOutboxError> for RadrootsSdkError {
    fn from(error: radroots_outbox::RadrootsOutboxError) -> Self {
        Self::Outbox {
            message: error.to_string(),
        }
    }
}

#[cfg(feature = "runtime")]
impl From<radroots_relay_transport::RadrootsRelayTransportError> for RadrootsSdkError {
    fn from(error: radroots_relay_transport::RadrootsRelayTransportError) -> Self {
        Self::RelayTransport {
            message: error.to_string(),
        }
    }
}

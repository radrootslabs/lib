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
    OutboxIdempotencyConflict,
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
    Io {
        path: PathBuf,
        message: String,
    },
    ClockBeforeUnixEpoch,
    TimestampOutOfRange {
        value: u64,
    },
    UnauthorizedActor {
        operation: String,
        reason: String,
    },
    SignerPubkeyMismatch {
        operation: String,
        expected_pubkey_prefix: String,
        signer_pubkey_prefix: String,
    },
    EmptyTargetRelays {
        operation: String,
    },
    RelayTargetLimitExceeded {
        max: usize,
        actual: usize,
    },
    InvalidRelayUrl {
        url: String,
        reason: String,
    },
    IdempotencyConflict {
        operation_kind: String,
        expected_pubkey_prefix: String,
        existing_digest_prefix: String,
        new_digest_prefix: String,
    },
    OrderStatusLimitInvalid {
        limit: u32,
        min: u32,
        max: u32,
    },
    InvalidOrderId {
        value: String,
        message: String,
    },
    ProductSyncUnsupported {
        operation: &'static str,
        required_feature: &'static str,
    },
    ProductSyncRelaySetupFailure {
        message: String,
    },
    Authority {
        message: String,
    },
    EventStore {
        message: String,
    },
    InvalidRequest {
        message: String,
    },
    ListingDraft {
        message: String,
    },
    ListingMutation {
        message: String,
    },
    Outbox {
        message: String,
    },
    RelayTransport {
        message: String,
    },
    Projection {
        message: String,
    },
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

    pub fn partial_outbox_idempotency_conflict_mutation(
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
            failure: RadrootsSdkPartialLocalMutationFailure::OutboxIdempotencyConflict,
        })
    }

    pub(crate) fn empty_target_relays(operation: impl Into<String>) -> Self {
        Self::EmptyTargetRelays {
            operation: operation.into(),
        }
    }

    pub(crate) fn relay_target_limit_exceeded(max: usize, actual: usize) -> Self {
        Self::RelayTargetLimitExceeded { max, actual }
    }

    pub(crate) fn invalid_relay_url(url: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidRelayUrl {
            url: redacted_relay_url(url.into()),
            reason: reason.into(),
        }
    }

    pub(crate) fn order_status_limit_invalid(limit: u32, min: u32, max: u32) -> Self {
        Self::OrderStatusLimitInvalid { limit, min, max }
    }

    pub(crate) fn invalid_order_id(value: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidOrderId {
            value: value.into(),
            message: message.into(),
        }
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
            Self::UnauthorizedActor { operation, reason } => {
                write!(f, "sdk unauthorized actor for {operation}: {reason}")
            }
            Self::SignerPubkeyMismatch {
                operation,
                expected_pubkey_prefix,
                signer_pubkey_prefix,
            } => write!(
                f,
                "sdk signer pubkey mismatch for {operation}: expected_pubkey_prefix={expected_pubkey_prefix}, signer_pubkey_prefix={signer_pubkey_prefix}"
            ),
            Self::EmptyTargetRelays { operation } => {
                write!(f, "sdk empty target relays for {operation}")
            }
            Self::RelayTargetLimitExceeded { max, actual } => {
                write!(
                    f,
                    "sdk relay target limit exceeded: max={max}, actual={actual}"
                )
            }
            Self::InvalidRelayUrl { url, reason } => {
                write!(f, "sdk invalid relay URL `{url}`: {reason}")
            }
            Self::IdempotencyConflict {
                operation_kind,
                expected_pubkey_prefix,
                existing_digest_prefix,
                new_digest_prefix,
            } => write!(
                f,
                "sdk idempotency conflict for {operation_kind}: expected_pubkey_prefix={expected_pubkey_prefix}, existing_digest_prefix={existing_digest_prefix}, new_digest_prefix={new_digest_prefix}"
            ),
            Self::OrderStatusLimitInvalid { limit, min, max } => write!(
                f,
                "sdk order status limit invalid: limit={limit}, min={min}, max={max}"
            ),
            Self::InvalidOrderId { value, message } => {
                write!(f, "sdk invalid order id `{value}`: {message}")
            }
            Self::ProductSyncUnsupported {
                operation,
                required_feature,
            } => write!(
                f,
                "sdk product sync operation {operation} requires feature `{required_feature}`"
            ),
            Self::ProductSyncRelaySetupFailure { message } => {
                write!(f, "sdk product sync relay setup failed: {message}")
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
        match error {
            radroots_authority::RadrootsAuthorityError::ActorRoleUnsatisfied {
                contract_id,
                required_role,
            } => Self::UnauthorizedActor {
                operation: contract_id,
                reason: format!("missing role {required_role:?}"),
            },
            radroots_authority::RadrootsAuthorityError::ActorPubkeyMismatch {
                expected_pubkey,
                actor_pubkey,
            } => Self::UnauthorizedActor {
                operation: "event authorization".to_owned(),
                reason: format!(
                    "actor_pubkey_prefix={} expected_pubkey_prefix={}",
                    redacted_prefix(actor_pubkey.as_str()),
                    redacted_prefix(expected_pubkey.as_str())
                ),
            },
            radroots_authority::RadrootsAuthorityError::SignerPubkeyMismatch {
                expected_pubkey,
                signer_pubkey,
            } => Self::SignerPubkeyMismatch {
                operation: "event signing".to_owned(),
                expected_pubkey_prefix: redacted_prefix(expected_pubkey.as_str()),
                signer_pubkey_prefix: redacted_prefix(signer_pubkey.as_str()),
            },
            error => Self::Authority {
                message: error.to_string(),
            },
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
        match error {
            radroots_trade::listing::RadrootsListingDraftError::ActorRoleUnsatisfied {
                required_role,
            } => Self::UnauthorizedActor {
                operation: "listing.prepare_publish".to_owned(),
                reason: format!("missing role {required_role:?}"),
            },
            error => Self::ListingDraft {
                message: error.to_string(),
            },
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
        match error {
            radroots_outbox::RadrootsOutboxError::EmptyTargetRelays => {
                Self::empty_target_relays("outbox enqueue")
            }
            radroots_outbox::RadrootsOutboxError::IdempotencyConflict {
                operation_kind,
                expected_pubkey,
                existing_digest,
                new_digest,
                ..
            } => Self::IdempotencyConflict {
                operation_kind,
                expected_pubkey_prefix: redacted_prefix(expected_pubkey.as_str()),
                existing_digest_prefix: redacted_prefix(existing_digest.as_str()),
                new_digest_prefix: redacted_prefix(new_digest.as_str()),
            },
            error => Self::Outbox {
                message: error.to_string(),
            },
        }
    }
}

#[cfg(feature = "runtime")]
impl From<radroots_relay_transport::RadrootsRelayTransportError> for RadrootsSdkError {
    fn from(error: radroots_relay_transport::RadrootsRelayTransportError) -> Self {
        match error {
            radroots_relay_transport::RadrootsRelayTransportError::RelayUrlParse {
                url,
                reason,
            } => Self::invalid_relay_url(url, reason),
            radroots_relay_transport::RadrootsRelayTransportError::WsRequiresLocalPolicy {
                url,
            } => Self::invalid_relay_url(url, "ws relay URL requires localhost policy"),
            radroots_relay_transport::RadrootsRelayTransportError::UnsupportedRelayScheme {
                url,
                scheme,
            } => Self::invalid_relay_url(url, format!("unsupported scheme `{scheme}`")),
            radroots_relay_transport::RadrootsRelayTransportError::RelayUrlUserinfo { url } => {
                Self::invalid_relay_url(url, "relay URL must not include userinfo")
            }
            radroots_relay_transport::RadrootsRelayTransportError::EmptyRelayHost { url } => {
                Self::invalid_relay_url(url, "relay URL must include a host")
            }
            radroots_relay_transport::RadrootsRelayTransportError::RelayUrlQueryOrFragment {
                url,
            } => Self::invalid_relay_url(url, "relay URL must not include query or fragment"),
            radroots_relay_transport::RadrootsRelayTransportError::EmptyTargetSet => {
                Self::empty_target_relays("relay publish")
            }
            #[cfg(feature = "runtime")]
            radroots_relay_transport::RadrootsRelayTransportError::Outbox(error) => error.into(),
            error => Self::RelayTransport {
                message: error.to_string(),
            },
        }
    }
}

#[cfg(feature = "runtime")]
fn redacted_prefix(value: &str) -> String {
    value.chars().take(12).collect()
}

#[cfg(feature = "runtime")]
fn redacted_relay_url(value: String) -> String {
    let Some((scheme, rest)) = value.split_once("://") else {
        return value;
    };
    let authority = rest.split('/').next().unwrap_or(rest);
    let Some((_, after_userinfo)) = authority.rsplit_once('@') else {
        return value;
    };
    let path = rest.strip_prefix(authority).unwrap_or_default();
    format!("{scheme}://<redacted>@{after_userinfo}{path}")
}

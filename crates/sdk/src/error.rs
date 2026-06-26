#[cfg(feature = "runtime")]
use std::{fmt, path::PathBuf};

#[cfg(feature = "runtime")]
use serde_json::{Value, json};

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RadrootsSdkErrorClass {
    Authorization,
    Clock,
    Configuration,
    LocalMutation,
    Request,
    Storage,
    Transport,
    Unsupported,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RadrootsSdkRecoveryAction {
    RetryOutboxEnqueue,
    InspectLocalStores,
    InspectGeoNamesAsset,
    RetryOperationWithSameIdempotencyKey,
    ConfigureRelayTargets,
    ConfigureGeoNamesCache,
    ConfigureSigner,
    FixRequest,
    SelectAuthorizedActor,
    CompleteSignerAuthentication,
    RetryAfterTransportFailure,
    RetryGeoNamesDownload,
    EnableRequiredFeature,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RadrootsSdkGeoNamesErrorKind {
    Configuration,
    Download,
    Cache,
    Integrity,
    Schema,
    Lookup,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RadrootsSdkPartialLocalMutationFailure {
    OutboxEnqueue,
    OutboxIdempotencyConflict,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
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
#[non_exhaustive]
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
    SignerUnavailable {
        mode: String,
        reason: String,
    },
    SignerRequestRejected {
        mode: String,
        reason: String,
    },
    SignerRequestTimedOut {
        mode: String,
    },
    SignerAuthChallengePending {
        mode: String,
        auth_url: Option<String>,
    },
    SignerTransport {
        mode: String,
        reason: String,
    },
    SignerProtocol {
        mode: String,
        reason: String,
    },
    SignerReturnedEventDrift {
        operation: String,
        reason: String,
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
    GeoNames {
        kind: RadrootsSdkGeoNamesErrorKind,
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
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "io",
            Self::ClockBeforeUnixEpoch => "clock_before_unix_epoch",
            Self::TimestampOutOfRange { .. } => "timestamp_out_of_range",
            Self::UnauthorizedActor { .. } => "unauthorized_actor",
            Self::SignerPubkeyMismatch { .. } => "signer_pubkey_mismatch",
            Self::SignerUnavailable { .. } => "signer_unavailable",
            Self::SignerRequestRejected { .. } => "signer_request_rejected",
            Self::SignerRequestTimedOut { .. } => "signer_request_timed_out",
            Self::SignerAuthChallengePending { .. } => "signer_auth_challenge_pending",
            Self::SignerTransport { .. } => "signer_transport",
            Self::SignerProtocol { .. } => "signer_protocol",
            Self::SignerReturnedEventDrift { .. } => "signer_returned_event_drift",
            Self::EmptyTargetRelays { .. } => "empty_target_relays",
            Self::RelayTargetLimitExceeded { .. } => "relay_target_limit_exceeded",
            Self::InvalidRelayUrl { .. } => "invalid_relay_url",
            Self::IdempotencyConflict { .. } => "idempotency_conflict",
            Self::OrderStatusLimitInvalid { .. } => "order_status_limit_invalid",
            Self::InvalidOrderId { .. } => "invalid_order_id",
            Self::ProductSyncUnsupported { .. } => "product_sync_unsupported",
            Self::ProductSyncRelaySetupFailure { .. } => "product_sync_relay_setup_failure",
            Self::Authority { .. } => "authority",
            Self::EventStore { .. } => "event_store",
            Self::InvalidRequest { .. } => "invalid_request",
            Self::ListingDraft { .. } => "listing_draft",
            Self::ListingMutation { .. } => "listing_mutation",
            Self::Outbox { .. } => "outbox",
            Self::GeoNames { kind, .. } => match kind {
                RadrootsSdkGeoNamesErrorKind::Configuration => "geonames_configuration",
                RadrootsSdkGeoNamesErrorKind::Download => "geonames_download",
                RadrootsSdkGeoNamesErrorKind::Cache => "geonames_cache",
                RadrootsSdkGeoNamesErrorKind::Integrity => "geonames_integrity",
                RadrootsSdkGeoNamesErrorKind::Schema => "geonames_schema",
                RadrootsSdkGeoNamesErrorKind::Lookup => "geonames_lookup",
            },
            Self::RelayTransport { .. } => "relay_transport",
            Self::Projection { .. } => "projection",
            Self::PartialLocalMutation(_) => "partial_local_mutation",
        }
    }

    pub fn class(&self) -> RadrootsSdkErrorClass {
        match self {
            Self::Io { .. }
            | Self::EventStore { .. }
            | Self::Outbox { .. }
            | Self::Projection { .. } => RadrootsSdkErrorClass::Storage,
            Self::GeoNames { kind, .. } => match kind {
                RadrootsSdkGeoNamesErrorKind::Configuration => RadrootsSdkErrorClass::Configuration,
                RadrootsSdkGeoNamesErrorKind::Download => RadrootsSdkErrorClass::Transport,
                RadrootsSdkGeoNamesErrorKind::Cache
                | RadrootsSdkGeoNamesErrorKind::Integrity
                | RadrootsSdkGeoNamesErrorKind::Schema => RadrootsSdkErrorClass::Storage,
                RadrootsSdkGeoNamesErrorKind::Lookup => RadrootsSdkErrorClass::Request,
            },
            Self::ClockBeforeUnixEpoch | Self::TimestampOutOfRange { .. } => {
                RadrootsSdkErrorClass::Clock
            }
            Self::UnauthorizedActor { .. }
            | Self::SignerPubkeyMismatch { .. }
            | Self::SignerRequestRejected { .. }
            | Self::SignerReturnedEventDrift { .. }
            | Self::Authority { .. } => RadrootsSdkErrorClass::Authorization,
            Self::SignerUnavailable { .. } => RadrootsSdkErrorClass::Configuration,
            Self::EmptyTargetRelays { .. }
            | Self::RelayTargetLimitExceeded { .. }
            | Self::InvalidRelayUrl { .. } => RadrootsSdkErrorClass::Configuration,
            Self::IdempotencyConflict { .. }
            | Self::OrderStatusLimitInvalid { .. }
            | Self::InvalidOrderId { .. }
            | Self::SignerProtocol { .. }
            | Self::SignerAuthChallengePending { .. }
            | Self::InvalidRequest { .. }
            | Self::ListingDraft { .. }
            | Self::ListingMutation { .. } => RadrootsSdkErrorClass::Request,
            Self::ProductSyncUnsupported { .. } => RadrootsSdkErrorClass::Unsupported,
            Self::ProductSyncRelaySetupFailure { .. }
            | Self::RelayTransport { .. }
            | Self::SignerRequestTimedOut { .. }
            | Self::SignerTransport { .. } => RadrootsSdkErrorClass::Transport,
            Self::PartialLocalMutation(_) => RadrootsSdkErrorClass::LocalMutation,
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Io { .. }
                | Self::ProductSyncRelaySetupFailure { .. }
                | Self::EventStore { .. }
                | Self::Outbox { .. }
                | Self::GeoNames {
                    kind: RadrootsSdkGeoNamesErrorKind::Cache
                        | RadrootsSdkGeoNamesErrorKind::Download,
                    ..
                }
                | Self::RelayTransport { .. }
                | Self::SignerRequestTimedOut { .. }
                | Self::SignerTransport { .. }
                | Self::Projection { .. }
                | Self::PartialLocalMutation(_)
        )
    }

    pub fn recovery_actions(&self) -> Vec<RadrootsSdkRecoveryAction> {
        match self {
            Self::Io { .. }
            | Self::EventStore { .. }
            | Self::Outbox { .. }
            | Self::Projection { .. } => vec![RadrootsSdkRecoveryAction::InspectLocalStores],
            Self::GeoNames { kind, .. } => match kind {
                RadrootsSdkGeoNamesErrorKind::Configuration => {
                    vec![RadrootsSdkRecoveryAction::ConfigureGeoNamesCache]
                }
                RadrootsSdkGeoNamesErrorKind::Download => {
                    vec![RadrootsSdkRecoveryAction::RetryGeoNamesDownload]
                }
                RadrootsSdkGeoNamesErrorKind::Cache
                | RadrootsSdkGeoNamesErrorKind::Integrity
                | RadrootsSdkGeoNamesErrorKind::Schema => {
                    vec![RadrootsSdkRecoveryAction::InspectGeoNamesAsset]
                }
                RadrootsSdkGeoNamesErrorKind::Lookup => vec![RadrootsSdkRecoveryAction::FixRequest],
            },
            Self::UnauthorizedActor { .. }
            | Self::SignerPubkeyMismatch { .. }
            | Self::SignerRequestRejected { .. }
            | Self::SignerReturnedEventDrift { .. }
            | Self::Authority { .. } => vec![RadrootsSdkRecoveryAction::SelectAuthorizedActor],
            Self::SignerUnavailable { .. } => vec![RadrootsSdkRecoveryAction::ConfigureSigner],
            Self::EmptyTargetRelays { .. }
            | Self::RelayTargetLimitExceeded { .. }
            | Self::InvalidRelayUrl { .. } => {
                vec![RadrootsSdkRecoveryAction::ConfigureRelayTargets]
            }
            Self::IdempotencyConflict { .. } => {
                vec![RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey]
            }
            Self::ProductSyncUnsupported { .. } => {
                vec![RadrootsSdkRecoveryAction::EnableRequiredFeature]
            }
            Self::ProductSyncRelaySetupFailure { .. } | Self::RelayTransport { .. } => {
                vec![RadrootsSdkRecoveryAction::RetryAfterTransportFailure]
            }
            Self::SignerRequestTimedOut { .. } | Self::SignerTransport { .. } => {
                vec![RadrootsSdkRecoveryAction::RetryAfterTransportFailure]
            }
            Self::SignerAuthChallengePending { .. } => {
                vec![RadrootsSdkRecoveryAction::CompleteSignerAuthentication]
            }
            Self::PartialLocalMutation(error) => vec![error.recovery],
            Self::ClockBeforeUnixEpoch
            | Self::TimestampOutOfRange { .. }
            | Self::OrderStatusLimitInvalid { .. }
            | Self::InvalidOrderId { .. }
            | Self::SignerProtocol { .. }
            | Self::InvalidRequest { .. }
            | Self::ListingDraft { .. }
            | Self::ListingMutation { .. } => vec![RadrootsSdkRecoveryAction::FixRequest],
        }
    }

    pub fn detail_json(&self) -> Value {
        let detail = match self {
            Self::Io { path, message } => {
                json!({ "path": path.display().to_string(), "message": message })
            }
            Self::ClockBeforeUnixEpoch => json!({}),
            Self::TimestampOutOfRange { value } => json!({ "value": value }),
            Self::UnauthorizedActor { operation, reason } => {
                json!({ "operation": operation, "reason": reason })
            }
            Self::SignerPubkeyMismatch {
                operation,
                expected_pubkey_prefix,
                signer_pubkey_prefix,
            } => json!({
                "operation": operation,
                "expected_pubkey_prefix": expected_pubkey_prefix,
                "signer_pubkey_prefix": signer_pubkey_prefix
            }),
            Self::SignerUnavailable { mode, reason }
            | Self::SignerRequestRejected { mode, reason }
            | Self::SignerTransport { mode, reason }
            | Self::SignerProtocol { mode, reason } => {
                json!({ "mode": mode, "reason": reason })
            }
            Self::SignerRequestTimedOut { mode } => json!({ "mode": mode }),
            Self::SignerAuthChallengePending { mode, auth_url } => {
                json!({ "mode": mode, "auth_url": auth_url })
            }
            Self::SignerReturnedEventDrift { operation, reason } => {
                json!({ "operation": operation, "reason": reason })
            }
            Self::EmptyTargetRelays { operation } => json!({ "operation": operation }),
            Self::RelayTargetLimitExceeded { max, actual } => {
                json!({ "max": max, "actual": actual })
            }
            Self::InvalidRelayUrl { url, reason } => json!({ "url": url, "reason": reason }),
            Self::IdempotencyConflict {
                operation_kind,
                expected_pubkey_prefix,
                existing_digest_prefix,
                new_digest_prefix,
            } => json!({
                "operation_kind": operation_kind,
                "expected_pubkey_prefix": expected_pubkey_prefix,
                "existing_digest_prefix": existing_digest_prefix,
                "new_digest_prefix": new_digest_prefix
            }),
            Self::OrderStatusLimitInvalid { limit, min, max } => {
                json!({ "limit": limit, "min": min, "max": max })
            }
            Self::InvalidOrderId { value, message } => {
                json!({ "value": value, "message": message })
            }
            Self::ProductSyncUnsupported {
                operation,
                required_feature,
            } => json!({ "operation": operation, "required_feature": required_feature }),
            Self::ProductSyncRelaySetupFailure { message }
            | Self::Authority { message }
            | Self::EventStore { message }
            | Self::InvalidRequest { message }
            | Self::ListingDraft { message }
            | Self::ListingMutation { message }
            | Self::Outbox { message }
            | Self::RelayTransport { message }
            | Self::Projection { message } => json!({ "message": message }),
            Self::GeoNames { kind, message } => json!({ "kind": kind, "message": message }),
            Self::PartialLocalMutation(error) => json!(error),
        };
        json!({
            "code": self.code(),
            "class": self.class(),
            "retryable": self.retryable(),
            "message": self.to_string(),
            "recovery_actions": self.recovery_actions(),
            "detail": detail
        })
    }

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

    pub(crate) fn missing_geonames_config() -> Self {
        Self::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Configuration,
            message: "GeoNames cache root is not configured".to_owned(),
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
            Self::SignerUnavailable { mode, reason } => {
                write!(f, "sdk {mode} signer unavailable: {reason}")
            }
            Self::SignerRequestRejected { mode, reason } => {
                write!(f, "sdk {mode} signer rejected request: {reason}")
            }
            Self::SignerRequestTimedOut { mode } => {
                write!(f, "sdk {mode} signer request timed out")
            }
            Self::SignerAuthChallengePending { mode, auth_url } => match auth_url {
                Some(auth_url) => {
                    write!(f, "sdk {mode} signer requires authentication at {auth_url}")
                }
                None => write!(f, "sdk {mode} signer requires authentication"),
            },
            Self::SignerTransport { mode, reason } => {
                write!(f, "sdk {mode} signer transport error: {reason}")
            }
            Self::SignerProtocol { mode, reason } => {
                write!(f, "sdk {mode} signer protocol error: {reason}")
            }
            Self::SignerReturnedEventDrift { operation, reason } => {
                write!(
                    f,
                    "sdk signer returned event drift for {operation}: {reason}"
                )
            }
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
            Self::GeoNames { kind, message } => {
                write!(f, "sdk GeoNames {kind:?} error: {message}")
            }
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
impl From<radroots_geocoder::GeocoderError> for RadrootsSdkError {
    fn from(error: radroots_geocoder::GeocoderError) -> Self {
        let kind = match &error {
            radroots_geocoder::GeocoderError::InvalidAssetUrl { .. }
            | radroots_geocoder::GeocoderError::InvalidAssetHost { .. } => {
                RadrootsSdkGeoNamesErrorKind::Configuration
            }
            radroots_geocoder::GeocoderError::AssetDownload { .. } => {
                RadrootsSdkGeoNamesErrorKind::Download
            }
            radroots_geocoder::GeocoderError::Io(_)
            | radroots_geocoder::GeocoderError::Sqlite(_)
            | radroots_geocoder::GeocoderError::AssetLockUnavailable { .. } => {
                RadrootsSdkGeoNamesErrorKind::Cache
            }
            radroots_geocoder::GeocoderError::InvalidAssetSchema { .. } => {
                RadrootsSdkGeoNamesErrorKind::Schema
            }
            radroots_geocoder::GeocoderError::InvalidAssetLength { .. }
            | radroots_geocoder::GeocoderError::InvalidAssetSha256 { .. }
            | radroots_geocoder::GeocoderError::InvalidAssetSqlite { .. }
            | radroots_geocoder::GeocoderError::InvalidAssetIntegrity { .. } => {
                RadrootsSdkGeoNamesErrorKind::Integrity
            }
            radroots_geocoder::GeocoderError::CountryCenterNotFound { .. } => {
                RadrootsSdkGeoNamesErrorKind::Lookup
            }
        };
        Self::GeoNames {
            kind,
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
impl From<radroots_trade::projection::RadrootsTradeProjectionError> for RadrootsSdkError {
    fn from(error: radroots_trade::projection::RadrootsTradeProjectionError) -> Self {
        match error {
            radroots_trade::projection::RadrootsTradeProjectionError::InvalidLimit { max } => {
                Self::InvalidRequest {
                    message: format!("projection query limit must be between 1 and {max}"),
                }
            }
            error => Self::Projection {
                message: error.to_string(),
            },
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
            radroots_relay_transport::RadrootsRelayTransportError::WsRequiresLocalhostPolicy {
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
            radroots_relay_transport::RadrootsRelayTransportError::RelayUrlForbiddenDestination {
                url,
                reason,
            } => Self::invalid_relay_url(url, reason),
            radroots_relay_transport::RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination {
                url,
                address,
                reason,
            } => Self::invalid_relay_url(
                url,
                format!("relay URL resolved to forbidden address `{address}`: {reason}"),
            ),
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
    let redacted = redact_query_or_fragment(value.as_str());
    let Some((scheme, rest)) = redacted.split_once("://") else {
        return redacted;
    };
    let authority = rest.split('/').next().unwrap_or(rest);
    let Some((_, after_userinfo)) = authority.rsplit_once('@') else {
        return redacted;
    };
    let path = rest.strip_prefix(authority).unwrap_or_default();
    format!("{scheme}://<redacted>@{after_userinfo}{path}")
}

#[cfg(feature = "runtime")]
fn redact_query_or_fragment(value: &str) -> String {
    let Some((index, marker)) = value.char_indices().find_map(|(index, character)| {
        matches!(character, '?' | '#').then_some((index, character))
    }) else {
        return value.to_owned();
    };
    format!("{}{}<redacted>", &value[..index], marker)
}

#[cfg(test)]
#[path = "../tests/unit/error_tests.rs"]
mod tests;

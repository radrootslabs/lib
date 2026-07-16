#[cfg(feature = "runtime")]
use std::{fmt, path::PathBuf};

#[cfg(feature = "runtime")]
use crate::privacy::{PrivacyPreflightStatus, ProductSensitivityField};
#[cfg(feature = "runtime")]
use crate::transport::ReticulumBehavior;
#[cfg(feature = "runtime")]
use radroots_trade::identity::RadrootsTradeLocator;
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
    InspectLocalStores,
    InspectGeoNamesAsset,
    RetryOperationWithSameIdempotencyKey,
    ConfigureTransportTargets,
    ConfigureGeoNamesCache,
    ConfigureSigner,
    FixRequest,
    SelectAuthorizedActor,
    CompleteSignerAuthentication,
    RetryAfterTransportFailure,
    RetryGeoNamesDownload,
    EnableRequiredFeature,
    SelectTradeRoot,
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
    EmptyTransportTargets {
        operation: String,
    },
    TransportTargetLimitExceeded {
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
    TradeStatusLimitInvalid {
        limit: u32,
        min: u32,
        max: u32,
    },
    InvalidTradeId {
        value: String,
        message: String,
    },
    TradeAmbiguous {
        operation: String,
        locator: Box<RadrootsTradeLocator>,
        candidates: Vec<RadrootsTradeLocator>,
    },
    PrivacyPreflight {
        operation: String,
        status: PrivacyPreflightStatus,
        fields: Vec<ProductSensitivityField>,
    },
    ProductSyncUnsupported {
        operation: &'static str,
        required_feature: &'static str,
    },
    ReticulumTransportUnavailable {
        operation: String,
        endpoint_uri: String,
        behavior: ReticulumBehavior,
    },
    ProductSyncTransportSetupFailure {
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
    UnsupportedProfileSchema {
        path: PathBuf,
        message: String,
    },
    ListingEdit {
        message: String,
    },
    ListingMutation {
        message: String,
    },
    Outbox {
        message: String,
    },
    PrivateStore {
        message: String,
    },
    StudioStore {
        message: String,
    },
    GeoNames {
        kind: RadrootsSdkGeoNamesErrorKind,
        message: String,
    },
    Transport {
        message: String,
    },
    Projection {
        message: String,
    },
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
            Self::EmptyTransportTargets { .. } => "empty_transport_targets",
            Self::TransportTargetLimitExceeded { .. } => "transport_target_limit_exceeded",
            Self::InvalidRelayUrl { .. } => "invalid_relay_url",
            Self::IdempotencyConflict { .. } => "idempotency_conflict",
            Self::TradeStatusLimitInvalid { .. } => "trade_status_limit_invalid",
            Self::InvalidTradeId { .. } => "invalid_trade_id",
            Self::TradeAmbiguous { .. } => "trade_ambiguous",
            Self::PrivacyPreflight { .. } => "privacy_preflight",
            Self::ProductSyncUnsupported { .. } => "product_sync_unsupported",
            Self::ReticulumTransportUnavailable { behavior, .. } => match behavior {
                ReticulumBehavior::RejectDeliveryAttempts => "reticulum_transport_unavailable",
                ReticulumBehavior::DeferDeliveryPlans => "reticulum_transport_deferred",
            },
            Self::ProductSyncTransportSetupFailure { .. } => "product_sync_transport_setup_failure",
            Self::Authority { .. } => "authority",
            Self::EventStore { .. } => "event_store",
            Self::InvalidRequest { .. } => "invalid_request",
            Self::UnsupportedProfileSchema { .. } => "unsupported_profile_schema",
            Self::ListingEdit { .. } => "listing_edit",
            Self::ListingMutation { .. } => "listing_mutation",
            Self::Outbox { .. } => "outbox",
            Self::PrivateStore { .. } => "private_store",
            Self::StudioStore { .. } => "studio_store",
            Self::GeoNames { kind, .. } => match kind {
                RadrootsSdkGeoNamesErrorKind::Configuration => "geonames_configuration",
                RadrootsSdkGeoNamesErrorKind::Download => "geonames_download",
                RadrootsSdkGeoNamesErrorKind::Cache => "geonames_cache",
                RadrootsSdkGeoNamesErrorKind::Integrity => "geonames_integrity",
                RadrootsSdkGeoNamesErrorKind::Schema => "geonames_schema",
                RadrootsSdkGeoNamesErrorKind::Lookup => "geonames_lookup",
            },
            Self::Transport { .. } => "transport",
            Self::Projection { .. } => "projection",
        }
    }

    pub fn class(&self) -> RadrootsSdkErrorClass {
        match self {
            Self::Io { .. }
            | Self::EventStore { .. }
            | Self::Outbox { .. }
            | Self::PrivateStore { .. }
            | Self::StudioStore { .. }
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
            Self::EmptyTransportTargets { .. }
            | Self::TransportTargetLimitExceeded { .. }
            | Self::InvalidRelayUrl { .. } => RadrootsSdkErrorClass::Configuration,
            Self::IdempotencyConflict { .. }
            | Self::TradeStatusLimitInvalid { .. }
            | Self::InvalidTradeId { .. }
            | Self::TradeAmbiguous { .. }
            | Self::PrivacyPreflight { .. }
            | Self::SignerProtocol { .. }
            | Self::SignerAuthChallengePending { .. }
            | Self::InvalidRequest { .. }
            | Self::UnsupportedProfileSchema { .. }
            | Self::ListingEdit { .. }
            | Self::ListingMutation { .. } => RadrootsSdkErrorClass::Request,
            Self::ProductSyncUnsupported { .. } | Self::ReticulumTransportUnavailable { .. } => {
                RadrootsSdkErrorClass::Unsupported
            }
            Self::ProductSyncTransportSetupFailure { .. }
            | Self::Transport { .. }
            | Self::SignerRequestTimedOut { .. }
            | Self::SignerTransport { .. } => RadrootsSdkErrorClass::Transport,
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Io { .. }
                | Self::ProductSyncTransportSetupFailure { .. }
                | Self::EventStore { .. }
                | Self::Outbox { .. }
                | Self::PrivateStore { .. }
                | Self::StudioStore { .. }
                | Self::GeoNames {
                    kind: RadrootsSdkGeoNamesErrorKind::Cache
                        | RadrootsSdkGeoNamesErrorKind::Download,
                    ..
                }
                | Self::Transport { .. }
                | Self::SignerRequestTimedOut { .. }
                | Self::SignerTransport { .. }
                | Self::Projection { .. }
        )
    }

    pub fn recovery_actions(&self) -> Vec<RadrootsSdkRecoveryAction> {
        match self {
            Self::Io { .. }
            | Self::EventStore { .. }
            | Self::Outbox { .. }
            | Self::PrivateStore { .. }
            | Self::StudioStore { .. }
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
            Self::EmptyTransportTargets { .. }
            | Self::TransportTargetLimitExceeded { .. }
            | Self::InvalidRelayUrl { .. } => {
                vec![RadrootsSdkRecoveryAction::ConfigureTransportTargets]
            }
            Self::IdempotencyConflict { .. } => {
                vec![RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey]
            }
            Self::TradeAmbiguous { .. } => vec![RadrootsSdkRecoveryAction::SelectTradeRoot],
            Self::PrivacyPreflight { .. } => vec![RadrootsSdkRecoveryAction::FixRequest],
            Self::UnsupportedProfileSchema { .. } => {
                vec![RadrootsSdkRecoveryAction::InspectLocalStores]
            }
            Self::ProductSyncUnsupported { .. } => {
                vec![RadrootsSdkRecoveryAction::EnableRequiredFeature]
            }
            Self::ReticulumTransportUnavailable { .. } => {
                vec![RadrootsSdkRecoveryAction::ConfigureTransportTargets]
            }
            Self::ProductSyncTransportSetupFailure { .. } | Self::Transport { .. } => {
                vec![RadrootsSdkRecoveryAction::RetryAfterTransportFailure]
            }
            Self::SignerRequestTimedOut { .. } | Self::SignerTransport { .. } => {
                vec![RadrootsSdkRecoveryAction::RetryAfterTransportFailure]
            }
            Self::SignerAuthChallengePending { .. } => {
                vec![RadrootsSdkRecoveryAction::CompleteSignerAuthentication]
            }
            Self::ClockBeforeUnixEpoch
            | Self::TimestampOutOfRange { .. }
            | Self::TradeStatusLimitInvalid { .. }
            | Self::InvalidTradeId { .. }
            | Self::SignerProtocol { .. }
            | Self::InvalidRequest { .. }
            | Self::ListingEdit { .. }
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
            Self::EmptyTransportTargets { operation } => json!({ "operation": operation }),
            Self::TransportTargetLimitExceeded { max, actual } => {
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
            Self::TradeStatusLimitInvalid { limit, min, max } => {
                json!({ "limit": limit, "min": min, "max": max })
            }
            Self::InvalidTradeId { value, message } => {
                json!({ "value": value, "message": message })
            }
            Self::TradeAmbiguous {
                operation,
                locator,
                candidates,
            } => json!({
                "operation": operation,
                "locator": locator,
                "candidates": candidates
            }),
            Self::PrivacyPreflight {
                operation,
                status,
                fields,
            } => json!({
                "operation": operation,
                "status": status,
                "fields": fields
            }),
            Self::ProductSyncUnsupported {
                operation,
                required_feature,
            } => json!({ "operation": operation, "required_feature": required_feature }),
            Self::ReticulumTransportUnavailable {
                operation,
                endpoint_uri,
                behavior,
            } => json!({
                "operation": operation,
                "endpoint_uri": endpoint_uri,
                "behavior": behavior.as_str()
            }),
            Self::ProductSyncTransportSetupFailure { message }
            | Self::Authority { message }
            | Self::EventStore { message }
            | Self::InvalidRequest { message }
            | Self::ListingEdit { message }
            | Self::ListingMutation { message }
            | Self::Outbox { message }
            | Self::PrivateStore { message }
            | Self::StudioStore { message }
            | Self::Transport { message }
            | Self::Projection { message } => json!({ "message": message }),
            Self::UnsupportedProfileSchema { path, message } => {
                json!({ "path": path.display().to_string(), "message": message })
            }
            Self::GeoNames { kind, message } => json!({ "kind": kind, "message": message }),
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

    pub(crate) fn empty_transport_targets(operation: impl Into<String>) -> Self {
        Self::EmptyTransportTargets {
            operation: operation.into(),
        }
    }

    pub(crate) fn transport_target_limit_exceeded(max: usize, actual: usize) -> Self {
        Self::TransportTargetLimitExceeded { max, actual }
    }

    pub(crate) fn invalid_relay_url(url: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidRelayUrl {
            url: redacted_relay_url(url.into()),
            reason: reason.into(),
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
            Self::EmptyTransportTargets { operation } => {
                write!(f, "sdk empty transport targets for {operation}")
            }
            Self::TransportTargetLimitExceeded { max, actual } => {
                write!(
                    f,
                    "sdk transport target limit exceeded: max={max}, actual={actual}"
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
            Self::TradeStatusLimitInvalid { limit, min, max } => write!(
                f,
                "sdk order status limit invalid: limit={limit}, min={min}, max={max}"
            ),
            Self::InvalidTradeId { value, message } => {
                write!(f, "sdk invalid order id `{value}`: {message}")
            }
            Self::TradeAmbiguous {
                operation,
                locator,
                candidates,
            } => write!(
                f,
                "sdk trade root is ambiguous for {operation}: trade_id={}, candidate_count={}",
                locator.order_id().as_str(),
                candidates.len()
            ),
            Self::PrivacyPreflight {
                operation,
                status,
                fields,
            } => write!(
                f,
                "sdk privacy preflight failed for {operation}: status={status:?}, field_count={}",
                fields.len()
            ),
            Self::ProductSyncUnsupported {
                operation,
                required_feature,
            } => write!(
                f,
                "sdk product sync operation {operation} requires feature `{required_feature}`"
            ),
            Self::ReticulumTransportUnavailable {
                operation,
                endpoint_uri,
                behavior,
            } => write!(
                f,
                "sdk product sync operation {operation} cannot deliver through Reticulum endpoint `{endpoint_uri}` with behavior `{}`",
                behavior.as_str()
            ),
            Self::ProductSyncTransportSetupFailure { message } => {
                write!(f, "sdk product sync transport setup failed: {message}")
            }
            Self::Authority { message } => write!(f, "sdk authority error: {message}"),
            Self::EventStore { message } => write!(f, "sdk event store error: {message}"),
            Self::InvalidRequest { message } => write!(f, "sdk invalid request: {message}"),
            Self::UnsupportedProfileSchema { path, message } => write!(
                f,
                "sdk unsupported profile schema at `{}`: {message}",
                path.display()
            ),
            Self::ListingEdit { message } => write!(f, "sdk listing edit error: {message}"),
            Self::ListingMutation { message } => {
                write!(f, "sdk listing mutation error: {message}")
            }
            Self::Outbox { message } => write!(f, "sdk outbox error: {message}"),
            Self::PrivateStore { message } => write!(f, "sdk private store error: {message}"),
            Self::StudioStore { message } => write!(f, "sdk studio store error: {message}"),
            Self::GeoNames { kind, message } => {
                write!(f, "sdk GeoNames {kind:?} error: {message}")
            }
            Self::Transport { message } => {
                write!(f, "sdk transport error: {message}")
            }
            Self::Projection { message } => write!(f, "sdk projection error: {message}"),
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
            | radroots_geocoder::GeocoderError::SqliteConnectionLockUnavailable
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
impl From<radroots_trade::listing::RadrootsListingEditError> for RadrootsSdkError {
    fn from(error: radroots_trade::listing::RadrootsListingEditError) -> Self {
        match error {
            radroots_trade::listing::RadrootsListingEditError::ActorRoleUnsatisfied {
                required_role,
            } => Self::UnauthorizedActor {
                operation: "listing.prepare_publish".to_owned(),
                reason: format!("missing role {required_role:?}"),
            },
            error => Self::ListingEdit {
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
            radroots_outbox::RadrootsOutboxError::EmptyDeliveryTargets => {
                Self::empty_transport_targets("outbox enqueue")
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
impl From<radroots_transport::RadrootsTransportError> for RadrootsSdkError {
    fn from(error: radroots_transport::RadrootsTransportError) -> Self {
        match error {
            radroots_transport::RadrootsTransportError::EmptyTargetSet => {
                Self::empty_transport_targets("transport target set")
            }
            error => Self::Transport {
                message: error.to_string(),
            },
        }
    }
}

#[cfg(feature = "runtime")]
impl From<radroots_transport_nostr::RadrootsRelayTransportError> for RadrootsSdkError {
    fn from(error: radroots_transport_nostr::RadrootsRelayTransportError) -> Self {
        match error {
            radroots_transport_nostr::RadrootsRelayTransportError::RelayUrlParse {
                url,
                reason,
            } => Self::invalid_relay_url(url, reason),
            radroots_transport_nostr::RadrootsRelayTransportError::WsRequiresLocalhostPolicy {
                url,
            } => Self::invalid_relay_url(url, "ws relay URL requires localhost policy"),
            radroots_transport_nostr::RadrootsRelayTransportError::UnsupportedRelayScheme {
                url,
                scheme,
            } => Self::invalid_relay_url(url, format!("unsupported scheme `{scheme}`")),
            radroots_transport_nostr::RadrootsRelayTransportError::RelayUrlUserinfo { url } => {
                Self::invalid_relay_url(url, "relay URL must not include userinfo")
            }
            radroots_transport_nostr::RadrootsRelayTransportError::EmptyRelayHost { url } => {
                Self::invalid_relay_url(url, "relay URL must include a host")
            }
            radroots_transport_nostr::RadrootsRelayTransportError::RelayUrlQueryOrFragment {
                url,
            } => Self::invalid_relay_url(url, "relay URL must not include query or fragment"),
            radroots_transport_nostr::RadrootsRelayTransportError::RelayUrlForbiddenDestination {
                url,
                reason,
            } => Self::invalid_relay_url(url, reason),
            radroots_transport_nostr::RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination {
                url,
                address,
                reason,
            } => Self::invalid_relay_url(
                url,
                format!("relay URL resolved to forbidden address `{address}`: {reason}"),
            ),
            radroots_transport_nostr::RadrootsRelayTransportError::EmptyTargetSet => {
                Self::empty_transport_targets("nostr relay publish")
            }
            #[cfg(feature = "runtime")]
            radroots_transport_nostr::RadrootsRelayTransportError::Outbox(error) => error.into(),
            error => Self::Transport {
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

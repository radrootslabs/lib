//! Stable serialized error-report contract generation 1.
//!
//! Native crate errors preserve their source chains in their owning packages.
//! This module accepts only validated, secret-safe data at the serialization
//! boundary and cannot contain a native source error.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;

use crate::{
    runtime::v1::OperationId,
    schema::{Metadata, ModuleVersion, Registry},
};

/// Error-report schema generation.
pub const SCHEMA_VERSION: u16 = 1;
/// Stable error-report schema identity.
pub const SCHEMA_ID: &str = "radroots.protocol.error_report.v1";
/// Replacement used when a native source message is not explicitly safe.
pub const REDACTED_MESSAGE: &str = "[redacted]";
const MAX_CODE_BYTES: usize = 96;
const MAX_CAPABILITY_ID_BYTES: usize = 128;
const MAX_SAFE_MESSAGE_BYTES: usize = 256;
const MAX_DETAIL_ENTRIES: usize = 32;
const MAX_DETAIL_TEXT_BYTES: usize = 128;

/// Stable error class.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Class {
    Validation,
    Contract,
    Storage,
    Resource,
    Conflict,
    Operation,
    Authorization,
    Signer,
    Network,
    Sync,
    Runtime,
    Projection,
    Query,
    Capability,
    Privacy,
    Security,
    Maintenance,
    Internal,
    Unknown,
}

/// Stable recovery action vocabulary established by the SDK surface.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RecoveryAction {
    InspectLocalStores,
    ConfigureStorage,
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
    RecreateClient,
}

/// One generated catalog descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Descriptor {
    pub code: KnownCode,
    pub class: Class,
    pub retryable: bool,
    pub recovery_actions: &'static [RecoveryAction],
}

macro_rules! error_catalog {
    ($( $variant:ident => ($value:literal, $class:ident, $retryable:literal, [$($action:ident),* $(,)?]) ),+ $(,)?) => {
        /// A code known to this protocol generation.
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum KnownCode {
            $( $variant, )+
        }

        impl KnownCode {
            /// Every code defined by this generation.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Returns the stable wire identity.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value,)+
                }
            }

            /// Parses a code known to this generation.
            pub fn parse(value: &str) -> Option<Self> {
                match value {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// Returns the generated descriptor from the same authority.
            pub const fn descriptor(self) -> Descriptor {
                match self {
                    $(Self::$variant => Descriptor {
                        code: Self::$variant,
                        class: Class::$class,
                        retryable: $retryable,
                        recovery_actions: &[$(RecoveryAction::$action),*],
                    },)+
                }
            }
        }

        /// Complete generated error catalog.
        pub const CATALOG: &[Descriptor] = &[
            $(Descriptor {
                code: KnownCode::$variant,
                class: Class::$class,
                retryable: $retryable,
                recovery_actions: &[$(RecoveryAction::$action),*],
            },)+
        ];
    };
}

error_catalog! {
    InvalidArgument => ("invalid_argument", Validation, false, [FixRequest]),
    UnsupportedContractVersion => ("unsupported_contract_version", Contract, false, [EnableRequiredFeature]),
    UnsupportedProfileSchema => ("unsupported_profile_schema", Storage, false, [InspectLocalStores]),
    SchemaTooNew => ("schema_too_new", Storage, false, [EnableRequiredFeature]),
    NotFound => ("not_found", Resource, false, [FixRequest]),
    AmbiguousTrade => ("ambiguous_trade", Conflict, false, [FixRequest]),
    StaleListingRevision => ("stale_listing_revision", Conflict, false, [FixRequest]),
    PreconditionChanged => ("precondition_changed", Conflict, true, [RetryOperationWithSameIdempotencyKey]),
    RevisionRequired => ("revision_required", Conflict, false, [FixRequest]),
    InventoryUnavailable => ("inventory_unavailable", Conflict, false, [RetryOperationWithSameIdempotencyKey]),
    IdempotencyConflict => ("idempotency_conflict", Conflict, false, [RetryOperationWithSameIdempotencyKey]),
    OperationInProgress => ("operation_in_progress", Operation, true, [RetryOperationWithSameIdempotencyKey]),
    ApprovalRequired => ("approval_required", Authorization, false, [SelectAuthorizedActor]),
    ApprovalInvalid => ("approval_invalid", Authorization, false, [SelectAuthorizedActor]),
    ApprovalExpired => ("approval_expired", Authorization, false, [SelectAuthorizedActor]),
    ApprovalReplayed => ("approval_replayed", Authorization, false, [SelectAuthorizedActor]),
    AuthorizationDenied => ("authorization_denied", Authorization, false, [SelectAuthorizedActor]),
    SignerCapabilityMissing => ("signer_capability_missing", Signer, false, [ConfigureSigner]),
    SignerUnavailable => ("signer_unavailable", Signer, true, [ConfigureSigner]),
    SignerRejected => ("signer_rejected", Signer, false, [SelectAuthorizedActor]),
    SignerTimeout => ("signer_timeout", Signer, true, [RetryAfterTransportFailure]),
    SignerCancelled => ("signer_cancelled", Signer, false, [ConfigureSigner]),
    SignerOutputInvalid => ("signer_output_invalid", Signer, false, [ConfigureSigner]),
    RelayAuthRequired => ("relay_auth_required", Network, true, [CompleteSignerAuthentication]),
    RelayAuthRejected => ("relay_auth_rejected", Network, false, [CompleteSignerAuthentication]),
    RelayPaymentRequired => ("relay_payment_required", Network, false, [ConfigureTransportTargets]),
    RelayPolicyRestricted => ("relay_policy_restricted", Network, false, [ConfigureTransportTargets]),
    RelayRateLimited => ("relay_rate_limited", Network, true, [RetryAfterTransportFailure]),
    RelayPowRequired => ("relay_pow_required", Network, false, [ConfigureTransportTargets]),
    TransportPartial => ("transport_partial", Network, true, [RetryAfterTransportFailure]),
    TransportOperationUnavailable => ("transport_operation_unavailable", Capability, false, [ConfigureTransportTargets]),
    SyncSaturated => ("sync_saturated", Sync, true, [RetryAfterTransportFailure]),
    SyncPartial => ("sync_partial", Sync, true, [RetryAfterTransportFailure]),
    DeadlineExceeded => ("deadline_exceeded", Runtime, true, [RetryAfterTransportFailure]),
    CancelledNoCommit => ("cancelled_no_commit", Runtime, false, [RetryOperationWithSameIdempotencyKey]),
    LocalCommittedDeliveryPending => ("local_committed_delivery_pending", Operation, true, [RetryAfterTransportFailure]),
    DatabaseBusy => ("database_busy", Storage, true, [InspectLocalStores]),
    ProfileWriterInUse => ("profile_writer_in_use", Storage, true, [InspectLocalStores]),
    MaintenanceInProgress => ("maintenance_in_progress", Storage, true, [RetryOperationWithSameIdempotencyKey]),
    StorageIntegrityFailed => ("storage_integrity_failed", Storage, false, [InspectLocalStores]),
    StorageSpaceInsufficient => ("storage_space_insufficient", Storage, true, [InspectLocalStores]),
    ProjectionStale => ("projection_stale", Projection, true, [InspectLocalStores]),
    ProjectionFailed => ("projection_failed", Projection, true, [InspectLocalStores]),
    ProjectionGenerationChanged => ("projection_generation_changed", Projection, true, [InspectLocalStores]),
    InvalidCursor => ("invalid_cursor", Query, false, [FixRequest]),
    UnsupportedCapability => ("unsupported_capability", Capability, false, [EnableRequiredFeature]),
    DmRelayUnconfigured => ("dm_relay_unconfigured", Privacy, false, [ConfigureTransportTargets]),
    PrivateDataUnavailable => ("private_data_unavailable", Privacy, false, [FixRequest]),
    ValidationPending => ("validation_pending", Validation, true, [RetryOperationWithSameIdempotencyKey]),
    ValidationExpired => ("validation_expired", Validation, false, [FixRequest]),
    ValidatorSetInvalid => ("validator_set_invalid", Validation, false, [FixRequest]),
    MediaPolicyDenied => ("media_policy_denied", Security, false, [FixRequest]),
    BackupInvalid => ("backup_invalid", Maintenance, false, [InspectLocalStores]),
    BackupAuthenticationFailed => ("backup_authentication_failed", Maintenance, false, [InspectLocalStores]),
    RestoreFailed => ("restore_failed", Maintenance, true, [InspectLocalStores]),
    Backpressure => ("backpressure", Runtime, true, [RetryAfterTransportFailure]),
    MissingStorage => ("missing_storage", Capability, false, [ConfigureStorage]),
    SignerWithoutSink => ("signer_without_sink", Validation, false, [ConfigureTransportTargets]),
    ClientCloseInProgress => ("client_close_in_progress", Operation, true, [RetryOperationWithSameIdempotencyKey]),
    ClientClosing => ("client_closing", Operation, true, [RetryOperationWithSameIdempotencyKey]),
    ClientClosed => ("client_closed", Runtime, false, [RecreateClient]),
    StorageCloseFailed => ("storage_close_failed", Storage, false, [InspectLocalStores]),
    InternalError => ("internal_error", Internal, false, [InspectLocalStores]),
}

/// A stable code that preserves unknown future values.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Code(String);

impl Code {
    /// Creates a code from a known catalog identity.
    pub fn known(code: KnownCode) -> Self {
        Self(code.as_str().to_string())
    }

    /// Parses a canonical code while preserving unknown future values.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if !valid_identifier(value.as_str(), MAX_CODE_BYTES) {
            return Err(Error::InvalidCode);
        }
        Ok(Self(value))
    }

    /// Returns the exact serialized identity.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Resolves this identity against the current generated catalog.
    pub fn known_code(&self) -> Option<KnownCode> {
        KnownCode::parse(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Code {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Code {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// Validated optional capability identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapabilityId(String);

impl CapabilityId {
    /// Parses a canonical public capability identity.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if !valid_identifier(value.as_str(), MAX_CAPABILITY_ID_BYTES) {
            return Err(Error::InvalidCapabilityId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for CapabilityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for CapabilityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// Validated secret-safe human-readable message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeMessage(String);

impl SafeMessage {
    /// Validates explicitly safe application-authored text.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_SAFE_MESSAGE_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(Error::InvalidSafeMessage);
        }
        if contains_sensitive_material(value.as_str()) {
            return Err(Error::SensitiveMessage);
        }
        Ok(Self(value))
    }

    /// Returns the mandatory redacted source-message replacement.
    pub fn redacted() -> Self {
        Self(REDACTED_MESSAGE.to_string())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for SafeMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SafeMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// Stable scalar value allowed in safe structured details.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", content = "value", rename_all = "snake_case")
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DetailValue {
    Text(String),
    Bool(bool),
    Signed(i64),
    Unsigned(u64),
}

/// One key/value detail entry.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Detail {
    pub key: String,
    pub value: DetailValue,
}

impl Detail {
    /// Creates a detail. Collection validation applies the allowlist.
    pub fn new(key: impl Into<String>, value: DetailValue) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

/// Deterministically ordered, validated safe detail collection.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "Vec<Detail>", into = "Vec<Detail>")
)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SafeDetails {
    entries: Vec<Detail>,
}

impl SafeDetails {
    /// Validates, sorts, and stores safe details.
    pub fn try_new(entries: impl IntoIterator<Item = Detail>) -> Result<Self, Error> {
        let mut entries: Vec<_> = entries.into_iter().collect();
        if entries.len() > MAX_DETAIL_ENTRIES {
            return Err(Error::TooManyDetails);
        }
        entries.sort_by(|left, right| left.key.cmp(&right.key));
        for (index, entry) in entries.iter().enumerate() {
            validate_detail(entry)?;
            if index > 0 && entries[index - 1].key == entry.key {
                return Err(Error::DuplicateDetailKey);
            }
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[Detail] {
        self.entries.as_slice()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl TryFrom<Vec<Detail>> for SafeDetails {
    type Error = Error;

    fn try_from(entries: Vec<Detail>) -> Result<Self, Self::Error> {
        Self::try_new(entries)
    }
}

impl From<SafeDetails> for Vec<Detail> {
    fn from(details: SafeDetails) -> Self {
        details.entries
    }
}

/// Secret-safe serialized error boundary.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorReport {
    schema_version: u16,
    code: Code,
    class: Class,
    retryable: bool,
    recovery_actions: Vec<RecoveryAction>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    operation_id: Option<OperationId>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    capability_id: Option<CapabilityId>,
    message: SafeMessage,
    #[cfg_attr(feature = "serde", serde(default))]
    details: SafeDetails,
}

impl ErrorReport {
    /// Builds a report for a known code from its generated descriptor.
    pub fn known(
        code: KnownCode,
        operation_id: Option<OperationId>,
        capability_id: Option<CapabilityId>,
        message: SafeMessage,
        details: SafeDetails,
    ) -> Self {
        let descriptor = code.descriptor();
        Self {
            schema_version: SCHEMA_VERSION,
            code: Code::known(code),
            class: descriptor.class,
            retryable: descriptor.retryable,
            recovery_actions: descriptor.recovery_actions.to_vec(),
            operation_id,
            capability_id,
            message,
            details,
        }
    }

    /// Converts an untrusted native source failure without copying its message.
    pub fn redacted_from_source(
        code: KnownCode,
        operation_id: Option<OperationId>,
        capability_id: Option<CapabilityId>,
    ) -> Self {
        Self::known(
            code,
            operation_id,
            capability_id,
            SafeMessage::redacted(),
            SafeDetails::default(),
        )
    }

    /// Builds the fail-closed representation of an unknown future code.
    pub fn unknown(code: Code) -> Result<Self, Error> {
        if code.known_code().is_some() {
            return Err(Error::ExpectedUnknownCode);
        }
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            code,
            class: Class::Unknown,
            retryable: false,
            recovery_actions: Vec::new(),
            operation_id: None,
            capability_id: None,
            message: SafeMessage::redacted(),
            details: SafeDetails::default(),
        })
    }

    /// Validates version, catalog agreement, and unknown-code policy.
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(Error::UnsupportedSchemaVersion {
                version: self.schema_version,
            });
        }
        if let Some(known) = self.code.known_code() {
            let descriptor = known.descriptor();
            if self.class != descriptor.class
                || self.retryable != descriptor.retryable
                || self.recovery_actions.as_slice() != descriptor.recovery_actions
            {
                return Err(Error::DescriptorMismatch { code: known });
            }
        } else if self.class != Class::Unknown
            || self.retryable
            || !self.recovery_actions.is_empty()
            || self.operation_id.is_some()
            || self.capability_id.is_some()
            || self.message.as_str() != REDACTED_MESSAGE
            || !self.details.is_empty()
        {
            return Err(Error::InvalidUnknownCodePolicy);
        }
        SafeDetails::try_new(self.details.entries.clone())?;
        Ok(())
    }

    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn code(&self) -> &Code {
        &self.code
    }

    pub const fn class(&self) -> Class {
        self.class
    }

    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    pub fn recovery_actions(&self) -> &[RecoveryAction] {
        self.recovery_actions.as_slice()
    }

    pub const fn operation_id(&self) -> Option<OperationId> {
        self.operation_id
    }

    pub fn capability_id(&self) -> Option<&CapabilityId> {
        self.capability_id.as_ref()
    }

    pub fn message(&self) -> &SafeMessage {
        &self.message
    }

    pub fn details(&self) -> &SafeDetails {
        &self.details
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WireReport {
    schema_version: u16,
    code: Code,
    class: Class,
    retryable: bool,
    recovery_actions: Vec<RecoveryAction>,
    #[serde(default)]
    operation_id: Option<OperationId>,
    #[serde(default)]
    capability_id: Option<CapabilityId>,
    message: SafeMessage,
    #[serde(default)]
    details: SafeDetails,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ErrorReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = WireReport::deserialize(deserializer)?;
        let report = Self {
            schema_version: wire.schema_version,
            code: wire.code,
            class: wire.class,
            retryable: wire.retryable,
            recovery_actions: wire.recovery_actions,
            operation_id: wire.operation_id,
            capability_id: wire.capability_id,
            message: wire.message,
            details: wire.details,
        };
        report.validate().map_err(serde::de::Error::custom)?;
        Ok(report)
    }
}

/// Exact schema metadata for generated-language authority.
pub const SCHEMAS: &[Metadata] = &[Metadata {
    type_name: "ErrorReport",
    schema_id: SCHEMA_ID,
    schema_version: SCHEMA_VERSION,
}];

/// Builds the stable error schema registry.
pub fn schema_registry() -> Result<Registry, crate::schema::Error> {
    Registry::try_from_metadata(
        SCHEMAS
            .iter()
            .copied()
            .map(|metadata| (metadata, ModuleVersion::ErrorV1)),
    )
}

/// Error-report validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    InvalidCode,
    InvalidCapabilityId,
    InvalidSafeMessage,
    SensitiveMessage,
    TooManyDetails,
    InvalidDetailKey,
    SensitiveDetailKey,
    DuplicateDetailKey,
    InvalidDetailText,
    ExpectedUnknownCode,
    UnsupportedSchemaVersion { version: u16 },
    DescriptorMismatch { code: KnownCode },
    InvalidUnknownCodePolicy,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCode => formatter.write_str("invalid error code"),
            Self::InvalidCapabilityId => formatter.write_str("invalid capability id"),
            Self::InvalidSafeMessage => formatter.write_str("invalid safe message"),
            Self::SensitiveMessage => formatter.write_str("sensitive error message rejected"),
            Self::TooManyDetails => formatter.write_str("too many safe detail entries"),
            Self::InvalidDetailKey => formatter.write_str("invalid safe detail key"),
            Self::SensitiveDetailKey => formatter.write_str("sensitive detail key rejected"),
            Self::DuplicateDetailKey => formatter.write_str("duplicate safe detail key"),
            Self::InvalidDetailText => formatter.write_str("invalid safe detail text"),
            Self::ExpectedUnknownCode => formatter.write_str("expected an unknown error code"),
            Self::UnsupportedSchemaVersion { version } => {
                write!(
                    formatter,
                    "unsupported error report schema version {version}"
                )
            }
            Self::DescriptorMismatch { code } => {
                write!(formatter, "error descriptor mismatch for {}", code.as_str())
            }
            Self::InvalidUnknownCodePolicy => {
                formatter.write_str("unknown error code violates fail-closed policy")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

fn validate_detail(detail: &Detail) -> Result<(), Error> {
    const ALLOWED_KEYS: &[&str] = &[
        "actual",
        "available_count",
        "committed",
        "delivery_pending",
        "expected",
        "field",
        "index",
        "limit",
        "mode",
        "required_count",
        "retry_after_ms",
        "schema_version",
        "status",
        "target_count",
    ];
    if !ALLOWED_KEYS.contains(&detail.key.as_str()) {
        if sensitive_identifier(detail.key.as_str()) {
            return Err(Error::SensitiveDetailKey);
        }
        return Err(Error::InvalidDetailKey);
    }
    if let DetailValue::Text(value) = &detail.value
        && (value.is_empty()
            || value.len() > MAX_DETAIL_TEXT_BYTES
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.')
            })
            || contains_sensitive_material(value.as_str()))
    {
        return Err(Error::InvalidDetailText);
    }
    Ok(())
}

fn valid_identifier(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

fn sensitive_identifier(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "authorization",
        "cookie",
        "credential",
        "mnemonic",
        "password",
        "private_key",
        "raw_event",
        "secret",
        "seed",
        "signature",
        "token",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn contains_sensitive_material(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    sensitive_identifier(value)
        || lower.contains("bearer ")
        || lower.contains("://")
        || lower.contains("sk_")
        || lower.contains("nsec1")
        || lower.contains("-----begin ")
        || lower.contains("private key")
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeSet;

    use super::*;

    #[test]
    fn generated_catalog_is_complete_unique_and_self_consistent() {
        assert_eq!(CATALOG.len(), 63);
        assert_eq!(KnownCode::ALL.len(), CATALOG.len());
        let mut codes = BTreeSet::new();
        for (index, descriptor) in CATALOG.iter().enumerate() {
            assert!(codes.insert(descriptor.code.as_str()));
            assert_eq!(descriptor.code, KnownCode::ALL[index]);
            assert_eq!(descriptor.code.descriptor(), *descriptor);
            assert_eq!(
                KnownCode::parse(descriptor.code.as_str()),
                Some(descriptor.code)
            );
            assert!(!descriptor.recovery_actions.is_empty());
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn known_report_round_trips_with_exact_v1_shape() {
        let details = SafeDetails::try_new([
            Detail::new("retry_after_ms", DetailValue::Unsigned(250)),
            Detail::new("status", DetailValue::Text("rate_limited".to_string())),
        ])
        .expect("safe details");
        let report = ErrorReport::known(
            KnownCode::RelayRateLimited,
            Some(OperationId::SyncPush),
            Some(CapabilityId::parse("nostr").expect("capability")),
            SafeMessage::parse("Relay rate limit requires a later retry").expect("safe message"),
            details,
        );
        report.validate().expect("report");

        let json = serde_json::to_string(&report).expect("serialize");
        let decoded: ErrorReport = serde_json::from_str(json.as_str()).expect("deserialize");
        assert_eq!(decoded, report);
        assert_eq!(
            serde_json::to_value(report).expect("value"),
            serde_json::json!({
                "schema_version": 1,
                "code": "relay_rate_limited",
                "class": "network",
                "retryable": true,
                "recovery_actions": ["retry_after_transport_failure"],
                "operation_id": "sync.push",
                "capability_id": "nostr",
                "message": "Relay rate limit requires a later retry",
                "details": [
                    {"key": "retry_after_ms", "value": {"kind": "unsigned", "value": 250}},
                    {"key": "status", "value": {"kind": "text", "value": "rate_limited"}}
                ]
            })
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn unknown_codes_are_preserved_but_fail_closed() {
        let report = ErrorReport::unknown(Code::parse("future_failure").expect("code"))
            .expect("unknown report");
        let json = serde_json::to_string(&report).expect("serialize");
        let decoded: ErrorReport = serde_json::from_str(json.as_str()).expect("deserialize");
        assert_eq!(decoded.code().as_str(), "future_failure");
        assert_eq!(decoded.class(), Class::Unknown);
        assert!(!decoded.retryable());
        assert!(decoded.recovery_actions().is_empty());
        assert_eq!(decoded.message().as_str(), REDACTED_MESSAGE);

        let invalid = json.replace("\"unknown\"", "\"network\"");
        assert!(serde_json::from_str::<ErrorReport>(invalid.as_str()).is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn unknown_fields_and_versions_fail_closed() {
        let report = ErrorReport::redacted_from_source(KnownCode::InternalError, None, None);
        let mut value = serde_json::to_value(report).expect("value");
        value["schema_version"] = serde_json::json!(2);
        assert!(serde_json::from_value::<ErrorReport>(value.clone()).is_err());
        value["schema_version"] = serde_json::json!(1);
        value
            .as_object_mut()
            .expect("object")
            .insert("source".to_string(), serde_json::json!("native error"));
        assert!(serde_json::from_value::<ErrorReport>(value).is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn native_source_messages_are_redacted_and_secrets_are_rejected() {
        for source in [
            "Bearer top-secret-token",
            "password=hunter2",
            "nsec1privatekeymaterial",
            "wss://user:password@relay.example.com?token=secret",
            "-----BEGIN PRIVATE KEY-----",
            "sk_live_sensitive",
        ] {
            assert!(SafeMessage::parse(source).is_err());
            let report = ErrorReport::redacted_from_source(KnownCode::InternalError, None, None);
            let json = serde_json::to_string(&report).expect("serialize");
            assert!(!json.contains(source));
            assert!(json.contains(REDACTED_MESSAGE));
        }

        assert_eq!(
            SafeDetails::try_new([Detail::new(
                "access_token",
                DetailValue::Text("secret".to_string())
            )]),
            Err(Error::SensitiveDetailKey)
        );
    }

    #[test]
    fn schema_registry_dispatches_error_report_v1() {
        let registry = schema_registry().expect("registry");
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.descriptors()[0].module(), ModuleVersion::ErrorV1);
        assert_eq!(registry.descriptors()[0].id().as_str(), SCHEMA_ID);
    }

    #[test]
    fn identifier_message_and_detail_validation_cover_bounds() {
        let known = Code::known(KnownCode::InternalError);
        assert_eq!(known.known_code(), Some(KnownCode::InternalError));
        assert_eq!(known.as_str(), "internal_error");
        for invalid in ["", "Upper", "1starts_with_digit", "has space", "has/slash"] {
            assert_eq!(Code::parse(invalid), Err(Error::InvalidCode));
            assert_eq!(
                CapabilityId::parse(invalid),
                Err(Error::InvalidCapabilityId)
            );
        }
        assert_eq!(
            Code::parse("a".repeat(MAX_CODE_BYTES + 1)),
            Err(Error::InvalidCode)
        );
        assert_eq!(
            CapabilityId::parse("a".repeat(MAX_CAPABILITY_ID_BYTES + 1)),
            Err(Error::InvalidCapabilityId)
        );
        let capability = CapabilityId::parse("transport.nostr-v1").expect("capability");
        assert_eq!(capability.as_str(), "transport.nostr-v1");

        assert_eq!(SafeMessage::parse(""), Err(Error::InvalidSafeMessage));
        assert_eq!(
            SafeMessage::parse("bad\nmessage"),
            Err(Error::InvalidSafeMessage)
        );
        assert_eq!(
            SafeMessage::parse("a".repeat(MAX_SAFE_MESSAGE_BYTES + 1)),
            Err(Error::InvalidSafeMessage)
        );
        let message = SafeMessage::parse("A safe diagnostic").expect("message");
        assert_eq!(message.as_str(), "A safe diagnostic");
        assert_eq!(SafeMessage::redacted().as_str(), REDACTED_MESSAGE);

        let details = SafeDetails::try_new([
            Detail::new("status", DetailValue::Text("ready_now".into())),
            Detail::new("actual", DetailValue::Signed(-1)),
            Detail::new("committed", DetailValue::Bool(true)),
            Detail::new("limit", DetailValue::Unsigned(5)),
        ])
        .expect("details");
        assert_eq!(details.entries()[0].key, "actual");
        let vector: Vec<Detail> = details.clone().into();
        assert_eq!(SafeDetails::try_from(vector).expect("converted"), details);
        assert_eq!(
            SafeDetails::try_new([Detail::new("unknown", DetailValue::Bool(true))]),
            Err(Error::InvalidDetailKey)
        );
        assert_eq!(
            SafeDetails::try_new([Detail::new("private_key", DetailValue::Bool(true))]),
            Err(Error::SensitiveDetailKey)
        );
        assert_eq!(
            SafeDetails::try_new([Detail::new("status", DetailValue::Text(String::new()))]),
            Err(Error::InvalidDetailText)
        );
        assert_eq!(
            SafeDetails::try_new([Detail::new("status", DetailValue::Text("BAD".into()))]),
            Err(Error::InvalidDetailText)
        );
        assert_eq!(
            SafeDetails::try_new([Detail::new(
                "status",
                DetailValue::Text("a".repeat(MAX_DETAIL_TEXT_BYTES + 1))
            )]),
            Err(Error::InvalidDetailText)
        );
        assert_eq!(
            SafeDetails::try_new([Detail::new(
                "status",
                DetailValue::Text("nsec1secret".into())
            )]),
            Err(Error::InvalidDetailText)
        );
        assert_eq!(
            SafeDetails::try_new([
                Detail::new("status", DetailValue::Bool(true)),
                Detail::new("status", DetailValue::Bool(false)),
            ]),
            Err(Error::DuplicateDetailKey)
        );
        let too_many = (0..=MAX_DETAIL_ENTRIES)
            .map(|index| Detail::new("status", DetailValue::Unsigned(index as u64)))
            .collect::<Vec<_>>();
        assert_eq!(SafeDetails::try_new(too_many), Err(Error::TooManyDetails));
    }

    #[test]
    fn report_validation_and_error_messages_cover_fail_closed_policy() {
        assert_eq!(
            ErrorReport::unknown(Code::known(KnownCode::InternalError)),
            Err(Error::ExpectedUnknownCode)
        );
        let report = ErrorReport::known(
            KnownCode::RelayRateLimited,
            Some(OperationId::SyncPush),
            Some(CapabilityId::parse("nostr").expect("capability")),
            SafeMessage::parse("Retry later").expect("message"),
            SafeDetails::default(),
        );
        assert_eq!(report.schema_version(), 1);
        assert_eq!(report.operation_id(), Some(OperationId::SyncPush));
        assert_eq!(
            report.capability_id().map(CapabilityId::as_str),
            Some("nostr")
        );
        assert!(report.details().is_empty());

        let mut invalid = report.clone();
        invalid.schema_version = 2;
        assert_eq!(
            invalid.validate(),
            Err(Error::UnsupportedSchemaVersion { version: 2 })
        );
        for mutate in 0..3 {
            let mut invalid = report.clone();
            match mutate {
                0 => invalid.class = Class::Unknown,
                1 => invalid.retryable = false,
                _ => invalid.recovery_actions.clear(),
            }
            assert_eq!(
                invalid.validate(),
                Err(Error::DescriptorMismatch {
                    code: KnownCode::RelayRateLimited
                })
            );
        }

        let unknown =
            ErrorReport::unknown(Code::parse("future_failure").expect("code")).expect("unknown");
        let mut variants = Vec::new();
        let mut value = unknown.clone();
        value.class = Class::Network;
        variants.push(value);
        let mut value = unknown.clone();
        value.retryable = true;
        variants.push(value);
        let mut value = unknown.clone();
        value
            .recovery_actions
            .push(RecoveryAction::RetryAfterTransportFailure);
        variants.push(value);
        let mut value = unknown.clone();
        value.operation_id = Some(OperationId::SyncPush);
        variants.push(value);
        let mut value = unknown.clone();
        value.capability_id = Some(CapabilityId::parse("nostr").expect("capability"));
        variants.push(value);
        let mut value = unknown.clone();
        value.message = SafeMessage::parse("Not redacted").expect("message");
        variants.push(value);
        let mut value = unknown;
        value.details =
            SafeDetails::try_new([Detail::new("status", DetailValue::Text("failed".into()))])
                .expect("details");
        variants.push(value);
        for invalid in variants {
            assert_eq!(invalid.validate(), Err(Error::InvalidUnknownCodePolicy));
        }

        let errors = [
            Error::InvalidCode,
            Error::InvalidCapabilityId,
            Error::InvalidSafeMessage,
            Error::SensitiveMessage,
            Error::TooManyDetails,
            Error::InvalidDetailKey,
            Error::SensitiveDetailKey,
            Error::DuplicateDetailKey,
            Error::InvalidDetailText,
            Error::ExpectedUnknownCode,
            Error::UnsupportedSchemaVersion { version: 2 },
            Error::DescriptorMismatch {
                code: KnownCode::InternalError,
            },
            Error::InvalidUnknownCodePolicy,
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}

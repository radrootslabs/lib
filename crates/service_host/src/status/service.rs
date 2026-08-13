//! Generic, bounded detailed-status envelope.

use core::{fmt, time::Duration};
use std::io::{self, Write};

use radroots_runtime_paths::{InstanceId, ServiceId};
use serde::{Serialize, ser::SerializeMap};

use crate::BuildInfo;

use super::ServiceOperationalState;

pub const SERVICE_STATUS_CONTRACT_VERSION: u32 = 1;
pub const CONFIGURATION_SCHEMA_VERSION: u32 = 1;
pub const SERVICE_STATUS_MAX_UTF8_BYTES: usize = 1_048_576;
pub const STATUS_ID_MAX_BYTES: usize = 128;

/// A bounded identifier matching the frozen operator-contract grammar.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct StatusId(String);

impl StatusId {
    pub fn new(value: impl Into<String>) -> Result<Self, StatusModelError> {
        let value = value.into();
        if !valid_status_id(&value) {
            return Err(StatusModelError::InvalidStatusId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A lowercase SHA-256 digest without a wire prefix.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    pub fn new(value: impl Into<String>) -> Result<Self, StatusModelError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(StatusModelError::InvalidSha256Digest);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Source of the effective service configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationSource {
    ExplicitConfig,
    DerivedRepoLocal,
}

/// Safe identity of the effective configuration document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConfigurationIdentity {
    schema: StatusId,
    schema_version: u32,
    digest: Sha256Digest,
    source: ConfigurationSource,
}

impl ConfigurationIdentity {
    /// Derives the exact v1 configuration-schema identity from the validated service.
    pub fn for_service(
        service: &ServiceId,
        digest: Sha256Digest,
        source: ConfigurationSource,
    ) -> Result<Self, StatusModelError> {
        let schema = StatusId::new(format!("radroots.{service}.config"))?;
        Ok(Self {
            schema,
            schema_version: CONFIGURATION_SCHEMA_VERSION,
            digest,
            source,
        })
    }

    fn validate_for_service(&self, service: &ServiceId) -> Result<(), StatusModelError> {
        validate_configuration_binding(service, &self.schema, self.schema_version)
    }
}

/// Common persistence health classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceHealth {
    Ready,
    ReadOnly,
    RepairRequired,
    Unavailable,
}

/// Common persistence integrity state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityState {
    Verified,
    VerificationRequired,
    Failed,
}

/// Shared persistence-status fields; service stores retain detailed policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PersistenceSummary {
    health: PersistenceHealth,
    schema_version: u32,
    generation: u64,
    integrity: IntegrityState,
    reason_codes: super::ReasonCodes,
}

impl PersistenceSummary {
    pub fn new(
        health: PersistenceHealth,
        schema_version: u32,
        generation: u64,
        integrity: IntegrityState,
        reason_codes: super::ReasonCodes,
    ) -> Result<Self, StatusModelError> {
        if schema_version == 0 {
            return Err(StatusModelError::InvalidSchemaVersion);
        }
        Ok(Self {
            health,
            schema_version,
            generation,
            integrity,
            reason_codes,
        })
    }
}

/// Common provider-health classification used inside service-owned summaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderHealth {
    Ready,
    Degraded,
    Unavailable,
}

/// Common transport-health classification used inside service-owned summaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportHealth {
    Ready,
    Degraded,
    Unavailable,
}

/// Monotonic process uptime in exact whole milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct UptimeMillis(u64);

impl UptimeMillis {
    pub fn from_duration(duration: Duration) -> Result<Self, StatusModelError> {
        u64::try_from(duration.as_millis())
            .map(Self)
            .map_err(|_| StatusModelError::UptimeOverflow)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Connects typed service detail with service-owned provider and transport summaries.
pub trait ServiceStatusDetail: Serialize {
    type Provider: Serialize;
    type Transport: Serialize;

    /// Frozen detail field. It must equal the service identifier.
    const FIELD_NAME: &'static str;
}

/// Shared detailed-status envelope with service-owned typed detail.
pub struct ServiceStatus<D>
where
    D: ServiceStatusDetail,
{
    service: ServiceId,
    instance: InstanceId,
    state: ServiceOperationalState,
    uptime: UptimeMillis,
    build: BuildInfo,
    configuration: ConfigurationIdentity,
    persistence: PersistenceSummary,
    provider: D::Provider,
    transport: D::Transport,
    detail: D,
}

impl<D> ServiceStatus<D>
where
    D: ServiceStatusDetail,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        service: ServiceId,
        instance: InstanceId,
        state: ServiceOperationalState,
        uptime: UptimeMillis,
        build: BuildInfo,
        configuration: ConfigurationIdentity,
        persistence: PersistenceSummary,
        provider: D::Provider,
        transport: D::Transport,
        detail: D,
    ) -> Result<Self, StatusModelError> {
        if D::FIELD_NAME != service.as_str() || !valid_status_detail_field(D::FIELD_NAME) {
            return Err(StatusModelError::InvalidDetailField);
        }
        configuration.validate_for_service(&service)?;
        Ok(Self {
            service,
            instance,
            state,
            uptime,
            build,
            configuration,
            persistence,
            provider,
            transport,
            detail,
        })
    }

    #[must_use]
    pub fn service(&self) -> &ServiceId {
        &self.service
    }

    #[must_use]
    pub fn instance(&self) -> &InstanceId {
        &self.instance
    }

    #[must_use]
    pub fn state(&self) -> &ServiceOperationalState {
        &self.state
    }

    /// Serializes without allowing the response buffer to exceed the frozen 1 MiB ceiling.
    pub fn to_bounded_json(&self) -> Result<Vec<u8>, StatusEncodingError> {
        let mut writer = BoundedWriter::new(SERVICE_STATUS_MAX_UTF8_BYTES);
        match serde_json::to_writer(&mut writer, self) {
            Ok(()) => Ok(writer.bytes),
            Err(_) if writer.exceeded => Err(StatusEncodingError::ResponseTooLarge),
            Err(_) => Err(StatusEncodingError::EncodingFailed),
        }
    }
}

impl<D> Serialize for ServiceStatus<D>
where
    D: ServiceStatusDetail,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(13))?;
        map.serialize_entry("contract_version", &SERVICE_STATUS_CONTRACT_VERSION)?;
        map.serialize_entry("service", &self.service)?;
        map.serialize_entry("instance", &self.instance)?;
        map.serialize_entry("phase", &self.state.phase())?;
        map.serialize_entry("ready", &self.state.readiness())?;
        map.serialize_entry("uptime_millis", &self.uptime)?;
        map.serialize_entry("reason_codes", self.state.reasons())?;
        map.serialize_entry("build_info", &self.build.status_projection())?;
        map.serialize_entry("configuration", &self.configuration)?;
        map.serialize_entry("persistence", &self.persistence)?;
        map.serialize_entry("provider", &self.provider)?;
        map.serialize_entry("transport", &self.transport)?;
        map.serialize_entry(D::FIELD_NAME, &self.detail)?;
        map.end()
    }
}

/// Validation failure for shared detailed-status models.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusModelError {
    InvalidStatusId,
    InvalidSha256Digest,
    InvalidSchemaVersion,
    ConfigurationServiceMismatch,
    InvalidDetailField,
    UptimeOverflow,
}

impl fmt::Display for StatusModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidStatusId => "status identifier is invalid",
            Self::InvalidSha256Digest => "status digest is invalid",
            Self::InvalidSchemaVersion => "status schema version is invalid",
            Self::ConfigurationServiceMismatch => "configuration schema does not match the service",
            Self::InvalidDetailField => "status detail field is invalid",
            Self::UptimeOverflow => "status uptime exceeds its u64 millisecond representation",
        })
    }
}

impl std::error::Error for StatusModelError {}

/// Safe result category for bounded JSON encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusEncodingError {
    EncodingFailed,
    ResponseTooLarge,
}

impl fmt::Display for StatusEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EncodingFailed => "status response encoding failed",
            Self::ResponseTooLarge => "status response exceeds its byte limit",
        })
    }
}

impl std::error::Error for StatusEncodingError {}

struct BoundedWriter {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}

impl BoundedWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(4096),
            maximum,
            exceeded: false,
        }
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let Some(next_length) = self.bytes.len().checked_add(buffer.len()) else {
            self.exceeded = true;
            return Err(io::Error::other("bounded status response overflow"));
        };
        if next_length > self.maximum {
            self.exceeded = true;
            return Err(io::Error::other("bounded status response exceeded"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn valid_status_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= STATUS_ID_MAX_BYTES
        && first.is_ascii_alphanumeric()
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn valid_status_detail_field(value: &str) -> bool {
    !matches!(
        value,
        "contract_version"
            | "service"
            | "instance"
            | "phase"
            | "ready"
            | "uptime_millis"
            | "reason_codes"
            | "build_info"
            | "configuration"
            | "persistence"
            | "provider"
            | "transport"
    )
}

fn validate_configuration_binding(
    service: &ServiceId,
    schema: &StatusId,
    schema_version: u32,
) -> Result<(), StatusModelError> {
    if schema_version != CONFIGURATION_SCHEMA_VERSION {
        return Err(StatusModelError::InvalidSchemaVersion);
    }
    let expected = format!("radroots.{service}.config");
    if schema.as_str() != expected {
        return Err(StatusModelError::ConfigurationServiceMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use crate::{
        BuildInfoEnvironment, BuildMode, CommonReasonCode, ContractVersions, Readiness,
        ServiceOperationalState, ServicePhase,
    };

    use super::*;

    const SERVICE_REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
    const LIB_REVISION: &str = "89abcdef0123456789abcdef0123456789abcdef";

    #[derive(Serialize)]
    struct Provider {
        health: ProviderHealth,
        reason_codes: super::super::ReasonCodes,
    }

    #[derive(Serialize)]
    struct Transport {
        health: TransportHealth,
        required_relays_ready: bool,
        connected_relay_count: u64,
        reason_codes: super::super::ReasonCodes,
    }

    #[derive(Serialize)]
    struct Detail {
        active_connections: u64,
    }

    impl ServiceStatusDetail for Detail {
        type Provider = Provider;
        type Transport = Transport;
        const FIELD_NAME: &'static str = "myc";
    }

    struct FailingDetail;

    impl Serialize for FailingDetail {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("injected detail failure"))
        }
    }

    impl ServiceStatusDetail for FailingDetail {
        type Provider = Provider;
        type Transport = Transport;
        const FIELD_NAME: &'static str = "myc";
    }

    #[derive(Serialize)]
    struct ReservedDetail;

    impl ServiceStatusDetail for ReservedDetail {
        type Provider = Provider;
        type Transport = Transport;
        const FIELD_NAME: &'static str = "service";
    }

    #[derive(Serialize)]
    struct MismatchedDetail;

    impl ServiceStatusDetail for MismatchedDetail {
        type Provider = Provider;
        type Transport = Transport;
        const FIELD_NAME: &'static str = "rhi";
    }

    fn status(detail: Detail) -> ServiceStatus<Detail> {
        let build = BuildInfo::from_compile_time(
            BuildMode::Release,
            BuildInfoEnvironment {
                service_version: Some("0.1.0-alpha"),
                service_commit: Some(SERVICE_REVISION),
                lib_revision: Some(LIB_REVISION),
                rust_version: Some("1.97.1"),
                target: Some("x86_64-unknown-linux-gnu"),
                feature_profile: Some("service-host"),
                contract_versions: ContractVersions::new(1, 1, 1, 1, 1).unwrap(),
            },
        )
        .unwrap();
        let state = ServiceOperationalState::new(
            ServicePhase::Degraded,
            Readiness::READY,
            super::super::ReasonCodes::new([CommonReasonCode::DatabaseLowDisk.into()]).unwrap(),
        )
        .unwrap();
        let service = ServiceId::new("myc").unwrap();
        let configuration = ConfigurationIdentity::for_service(
            &service,
            Sha256Digest::new("a".repeat(64)).unwrap(),
            ConfigurationSource::ExplicitConfig,
        )
        .unwrap();
        let persistence = PersistenceSummary::new(
            PersistenceHealth::Ready,
            1,
            7,
            IntegrityState::Verified,
            super::super::ReasonCodes::empty(),
        )
        .unwrap();
        ServiceStatus::new(
            service,
            InstanceId::new("default").unwrap(),
            state,
            UptimeMillis::from_duration(Duration::from_millis(120_000)).unwrap(),
            build,
            configuration,
            persistence,
            Provider {
                health: ProviderHealth::Degraded,
                reason_codes: super::super::ReasonCodes::empty(),
            },
            Transport {
                health: TransportHealth::Ready,
                required_relays_ready: true,
                connected_relay_count: 2,
                reason_codes: super::super::ReasonCodes::empty(),
            },
            detail,
        )
        .unwrap()
    }

    #[test]
    fn status_json_snapshot_matches_the_frozen_envelope() {
        let json = String::from_utf8(
            status(Detail {
                active_connections: 3,
            })
            .to_bounded_json()
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            json,
            r#"{"contract_version":1,"service":"myc","instance":"default","phase":"degraded","ready":true,"uptime_millis":120000,"reason_codes":["database_low_disk"],"build_info":{"version":"0.1.0-alpha","revision":"0123456789abcdef0123456789abcdef01234567","toolchain":"1.97.1","contract_versions":{"config":1,"state":1,"admin":1,"status":1,"provider":1}},"configuration":{"schema":"radroots.myc.config","schema_version":1,"digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","source":"explicit_config"},"persistence":{"health":"ready","schema_version":1,"generation":7,"integrity":"verified","reason_codes":[]},"provider":{"health":"degraded","reason_codes":[]},"transport":{"health":"ready","required_relays_ready":true,"connected_relay_count":2,"reason_codes":[]},"myc":{"active_connections":3}}"#
        );
        assert!(json.len() < SERVICE_STATUS_MAX_UTF8_BYTES);
        for forbidden in ["secret", "credential", "private_key", "password", "path"] {
            assert!(!json.contains(forbidden));
        }
    }

    #[derive(Serialize)]
    struct OversizedDetail {
        content: String,
    }

    impl ServiceStatusDetail for OversizedDetail {
        type Provider = Provider;
        type Transport = Transport;
        const FIELD_NAME: &'static str = "myc";
    }

    #[test]
    fn bounded_encoder_rejects_oversized_detail() {
        let ordinary = status(Detail {
            active_connections: 0,
        });
        let oversized = ServiceStatus::new(
            ordinary.service,
            ordinary.instance,
            ordinary.state,
            ordinary.uptime,
            ordinary.build,
            ordinary.configuration,
            ordinary.persistence,
            ordinary.provider,
            ordinary.transport,
            OversizedDetail {
                content: "x".repeat(SERVICE_STATUS_MAX_UTF8_BYTES),
            },
        )
        .unwrap();
        assert_eq!(
            oversized.to_bounded_json(),
            Err(StatusEncodingError::ResponseTooLarge)
        );
    }

    #[test]
    fn identifiers_versions_digests_and_uptime_fail_closed() {
        for invalid in ["", ".schema", "bad schema", "slash/name"] {
            assert!(StatusId::new(invalid).is_err());
        }
        assert!(StatusId::new("a".repeat(STATUS_ID_MAX_BYTES)).is_ok());
        assert!(StatusId::new("a".repeat(STATUS_ID_MAX_BYTES + 1)).is_err());
        assert!(Sha256Digest::new("a".repeat(63)).is_err());
        assert!(Sha256Digest::new("A".repeat(64)).is_err());
        let myc = ServiceId::new("myc").unwrap();
        assert_eq!(
            validate_configuration_binding(
                &myc,
                &StatusId::new("radroots.rhi.config").unwrap(),
                CONFIGURATION_SCHEMA_VERSION,
            ),
            Err(StatusModelError::ConfigurationServiceMismatch)
        );
        assert_eq!(
            validate_configuration_binding(
                &myc,
                &StatusId::new("radroots.myc.config").unwrap(),
                CONFIGURATION_SCHEMA_VERSION + 1,
            ),
            Err(StatusModelError::InvalidSchemaVersion)
        );
        assert_eq!(
            UptimeMillis::from_duration(Duration::MAX),
            Err(StatusModelError::UptimeOverflow)
        );
    }

    #[test]
    fn model_accessors_reserved_fields_and_encoding_errors_are_closed() {
        let ordinary = status(Detail {
            active_connections: 1,
        });
        assert_eq!(ordinary.service().as_str(), "myc");
        assert_eq!(ordinary.instance().as_str(), "default");
        assert_eq!(ordinary.state().phase(), ServicePhase::Degraded);
        assert_eq!(ordinary.uptime.get(), 120_000);
        assert_eq!(
            ordinary.configuration.schema.as_str(),
            "radroots.myc.config"
        );
        assert_eq!(ordinary.configuration.digest.as_str(), "a".repeat(64));

        assert_eq!(
            PersistenceSummary::new(
                PersistenceHealth::Unavailable,
                0,
                0,
                IntegrityState::Failed,
                super::super::ReasonCodes::empty(),
            ),
            Err(StatusModelError::InvalidSchemaVersion)
        );
        assert!(Sha256Digest::new("0".repeat(64)).is_ok());
        assert_eq!(
            Sha256Digest::new(format!("{}g", "a".repeat(63))),
            Err(StatusModelError::InvalidSha256Digest)
        );

        let failing = ServiceStatus::new(
            ordinary.service,
            ordinary.instance,
            ordinary.state,
            ordinary.uptime,
            ordinary.build,
            ordinary.configuration,
            ordinary.persistence,
            ordinary.provider,
            ordinary.transport,
            FailingDetail,
        )
        .unwrap();
        assert_eq!(
            failing.to_bounded_json(),
            Err(StatusEncodingError::EncodingFailed)
        );

        let ordinary = status(Detail {
            active_connections: 1,
        });
        let service = ServiceId::new("service").unwrap();
        let configuration = ConfigurationIdentity::for_service(
            &service,
            Sha256Digest::new("b".repeat(64)).unwrap(),
            ConfigurationSource::DerivedRepoLocal,
        )
        .unwrap();
        assert!(matches!(
            ServiceStatus::new(
                service,
                ordinary.instance,
                ordinary.state,
                ordinary.uptime,
                ordinary.build,
                configuration,
                ordinary.persistence,
                ordinary.provider,
                ordinary.transport,
                ReservedDetail,
            ),
            Err(StatusModelError::InvalidDetailField)
        ));

        let ordinary = status(Detail {
            active_connections: 1,
        });
        assert!(matches!(
            ServiceStatus::new(
                ordinary.service,
                ordinary.instance,
                ordinary.state,
                ordinary.uptime,
                ordinary.build,
                ordinary.configuration,
                ordinary.persistence,
                ordinary.provider,
                ordinary.transport,
                MismatchedDetail,
            ),
            Err(StatusModelError::InvalidDetailField)
        ));

        let mut writer = BoundedWriter::new(2);
        assert_eq!(writer.write(b"ab").unwrap(), 2);
        writer.flush().unwrap();
        assert!(writer.write(b"c").is_err());
        assert!(writer.exceeded);

        for error in [
            StatusModelError::InvalidStatusId,
            StatusModelError::InvalidSha256Digest,
            StatusModelError::InvalidSchemaVersion,
            StatusModelError::ConfigurationServiceMismatch,
            StatusModelError::InvalidDetailField,
            StatusModelError::UptimeOverflow,
        ] {
            assert!(!error.to_string().is_empty());
            assert!(std::error::Error::source(&error).is_none());
        }
        for error in [
            StatusEncodingError::EncodingFailed,
            StatusEncodingError::ResponseTooLarge,
        ] {
            assert!(!error.to_string().is_empty());
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}

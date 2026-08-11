use std::collections::BTreeMap;

use radroots_runtime_paths::ServiceId;
use serde::Deserialize;

/// Instance cardinality supported by a hardened service target.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceInstanceSupport {
    Multiple,
}

/// Configuration document format supported by a hardened service target.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceConfigurationFormat {
    Toml,
}

impl ServiceConfigurationFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Toml => "toml",
        }
    }
}

/// State initialization policy supported by a hardened service target.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStateInitialization {
    Explicit,
}

/// Daemon state-open policy supported by a hardened service target.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceRunStatePolicy {
    ExistingOnly,
}

/// Detailed local-administration transport supported by a hardened service.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAdminTransport {
    Http11OverUnixDomainSocket,
}

/// Versioned base path for detailed local administration.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum ServiceAdminBasePath {
    #[serde(rename = "/v1")]
    V1,
}

/// Detailed status surface supported by a hardened service.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatusSurface {
    LocalAdminServiceStatusV1,
}

/// Public operations surface supported by a hardened service.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceOperationsSurface {
    CachedLivezReadyzMetrics,
}

impl ServiceOperationsSurface {
    pub const ROUTES: [&str; 3] = ["/livez", "/readyz", "/metrics"];

    #[must_use]
    pub const fn routes(self) -> [&'static str; 3] {
        match self {
            Self::CachedLivezReadyzMetrics => Self::ROUTES,
        }
    }
}

/// Current evidence posture for an eligible service target.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceSupportPosture {
    Target,
}

/// Exact Linux target triples eligible for future Tier-1 qualification.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum ServiceTier1Target {
    #[serde(rename = "x86_64-unknown-linux-gnu")]
    X86_64UnknownLinuxGnu,
    #[serde(rename = "aarch64-unknown-linux-gnu")]
    Aarch64UnknownLinuxGnu,
}

impl ServiceTier1Target {
    pub const ALL: [Self; 2] = [Self::X86_64UnknownLinuxGnu, Self::Aarch64UnknownLinuxGnu];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::X86_64UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
            Self::Aarch64UnknownLinuxGnu => "aarch64-unknown-linux-gnu",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|target| target.as_str() == value)
    }
}

/// Closed metadata for one hardened standalone service target.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(try_from = "HardenedServiceTargetWire")]
pub struct HardenedServiceTarget {
    service_id: ServiceId,
    instance_support: ServiceInstanceSupport,
    config_format: ServiceConfigurationFormat,
    state_initialization: ServiceStateInitialization,
    run_state_policy: ServiceRunStatePolicy,
    admin_transport: ServiceAdminTransport,
    admin_base_path: ServiceAdminBasePath,
    admin_contract_version: u32,
    status_surface: ServiceStatusSurface,
    operations_surface: ServiceOperationsSurface,
    support_posture: ServiceSupportPosture,
    tier_1_targets: Vec<ServiceTier1Target>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HardenedServiceTargetWire {
    service_id: ServiceId,
    instance_support: ServiceInstanceSupport,
    config_format: ServiceConfigurationFormat,
    state_initialization: ServiceStateInitialization,
    run_state_policy: ServiceRunStatePolicy,
    admin_transport: ServiceAdminTransport,
    admin_base_path: ServiceAdminBasePath,
    admin_contract_version: u32,
    status_surface: ServiceStatusSurface,
    operations_surface: ServiceOperationsSurface,
    support_posture: ServiceSupportPosture,
    tier_1_targets: Vec<ServiceTier1Target>,
}

impl HardenedServiceTarget {
    #[must_use]
    pub fn service_id(&self) -> &ServiceId {
        &self.service_id
    }

    #[must_use]
    pub const fn instance_support(&self) -> ServiceInstanceSupport {
        self.instance_support
    }

    #[must_use]
    pub const fn config_format(&self) -> ServiceConfigurationFormat {
        self.config_format
    }

    #[must_use]
    pub const fn state_initialization(&self) -> ServiceStateInitialization {
        self.state_initialization
    }

    #[must_use]
    pub const fn run_state_policy(&self) -> ServiceRunStatePolicy {
        self.run_state_policy
    }

    #[must_use]
    pub const fn admin_transport(&self) -> ServiceAdminTransport {
        self.admin_transport
    }

    #[must_use]
    pub const fn admin_base_path(&self) -> ServiceAdminBasePath {
        self.admin_base_path
    }

    #[must_use]
    pub const fn admin_contract_version(&self) -> u32 {
        self.admin_contract_version
    }

    #[must_use]
    pub const fn status_surface(&self) -> ServiceStatusSurface {
        self.status_surface
    }

    #[must_use]
    pub const fn operations_surface(&self) -> ServiceOperationsSurface {
        self.operations_surface
    }

    #[must_use]
    pub const fn support_posture(&self) -> ServiceSupportPosture {
        self.support_posture
    }

    #[must_use]
    pub fn tier_1_targets(&self) -> &[ServiceTier1Target] {
        &self.tier_1_targets
    }

    fn has_exact_common_contract(&self) -> bool {
        self.instance_support == ServiceInstanceSupport::Multiple
            && self.config_format == ServiceConfigurationFormat::Toml
            && self.state_initialization == ServiceStateInitialization::Explicit
            && self.run_state_policy == ServiceRunStatePolicy::ExistingOnly
            && self.admin_transport == ServiceAdminTransport::Http11OverUnixDomainSocket
            && self.admin_base_path == ServiceAdminBasePath::V1
            && self.admin_contract_version == 1
            && self.status_surface == ServiceStatusSurface::LocalAdminServiceStatusV1
            && self.operations_surface == ServiceOperationsSurface::CachedLivezReadyzMetrics
            && self.support_posture == ServiceSupportPosture::Target
            && self.tier_1_targets == ServiceTier1Target::ALL
    }
}

impl TryFrom<HardenedServiceTargetWire> for HardenedServiceTarget {
    type Error = &'static str;

    fn try_from(wire: HardenedServiceTargetWire) -> Result<Self, Self::Error> {
        let target = Self {
            service_id: wire.service_id,
            instance_support: wire.instance_support,
            config_format: wire.config_format,
            state_initialization: wire.state_initialization,
            run_state_policy: wire.run_state_policy,
            admin_transport: wire.admin_transport,
            admin_base_path: wire.admin_base_path,
            admin_contract_version: wire.admin_contract_version,
            status_surface: wire.status_surface,
            operations_surface: wire.operations_surface,
            support_posture: wire.support_posture,
            tier_1_targets: wire.tier_1_targets,
        };
        if !matches!(target.service_id.as_str(), "myc" | "rhi")
            || !target.has_exact_common_contract()
        {
            return Err("hardened service target does not match the v1 contract");
        }
        Ok(target)
    }
}

/// Validated exact Myc/RHI service-target inventory.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(try_from = "BTreeMap<String, HardenedServiceTarget>")]
pub struct HardenedServiceTargets(BTreeMap<String, HardenedServiceTarget>);

impl HardenedServiceTargets {
    #[must_use]
    pub fn get(&self, service_id: &ServiceId) -> Option<&HardenedServiceTarget> {
        self.0.get(service_id.as_str())
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &HardenedServiceTarget)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<BTreeMap<String, HardenedServiceTarget>> for HardenedServiceTargets {
    type Error = &'static str;

    fn try_from(targets: BTreeMap<String, HardenedServiceTarget>) -> Result<Self, Self::Error> {
        const REQUIRED_SERVICES: [&str; 2] = ["myc", "rhi"];

        if targets.len() != REQUIRED_SERVICES.len() {
            return Err("hardened service target inventory must contain exactly Myc and RHI");
        }
        for service in REQUIRED_SERVICES {
            let Some(target) = targets.get(service) else {
                return Err("hardened service target inventory is incomplete");
            };
            if target.service_id.as_str() != service || !target.has_exact_common_contract() {
                return Err("hardened service target metadata does not match the v1 contract");
            }
        }
        if targets
            .iter()
            .any(|(key, target)| key != target.service_id.as_str())
        {
            return Err("hardened service target key does not match its service identifier");
        }

        Ok(Self(targets))
    }
}

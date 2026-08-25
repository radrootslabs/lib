use crate::error::RadrootsRuntimeDistributionError;
use crate::model::{
    ArtifactAdapter, RadrootsRuntimeDistributionContract, RuntimeDistributionEntry, TargetSpec,
};
use crate::service::{HardenedServiceTarget, ServiceTier1Target};
use crate::service_artifact::{HardenedServiceArtifact, ServiceArtifactSha256};
use radroots_runtime_paths::ServiceId;

pub const RUNTIME_DISTRIBUTION_SCHEMA: &str = "radroots-runtime-distribution";
pub const RUNTIME_DISTRIBUTION_SCHEMA_VERSION: u32 = 1;
pub const RUNTIME_DISTRIBUTION_CONTRACT_MAX_UTF8_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTargetRequest<'a> {
    pub service_id: &'a ServiceId,
    pub target_id: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedServiceTarget {
    service_id: ServiceId,
    target: ServiceTier1Target,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceArtifactRequest<'a> {
    pub service_id: &'a ServiceId,
    pub target_id: &'a str,
}

/// Exact native release artifact metadata for one hardened service target.
#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedServiceArtifact {
    service_id: ServiceId,
    target: ServiceTier1Target,
    version: String,
    package_name: String,
    binary_name: String,
    channel: String,
    binary_archive_name: String,
    binary_archive_format: &'static str,
    binary_archive_member: String,
    artifact_manifest_name: String,
    checksums_name: String,
    checksum_algorithm: &'static str,
    checksum_format: &'static str,
    release_contract_sha256: ServiceArtifactSha256,
    source_lock_sha256: ServiceArtifactSha256,
    output_inventory: Vec<String>,
}

impl core::fmt::Debug for ResolvedServiceArtifact {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ResolvedServiceArtifact")
            .field("service_id", &self.service_id)
            .field("target", &self.target)
            .field("version", &self.version)
            .field("package_name", &self.package_name)
            .field("binary_name", &self.binary_name)
            .field("channel", &self.channel)
            .field("binary_archive_name", &self.binary_archive_name)
            .field("binary_archive_format", &self.binary_archive_format)
            .field("binary_archive_member", &self.binary_archive_member)
            .field("artifact_manifest_name", &self.artifact_manifest_name)
            .field("checksums_name", &self.checksums_name)
            .field("checksum_algorithm", &self.checksum_algorithm)
            .field("checksum_format", &self.checksum_format)
            .field("release_contract_sha256", &"<redacted>")
            .field("source_lock_sha256", &"<redacted>")
            .field("output_inventory", &self.output_inventory)
            .finish()
    }
}

impl ResolvedServiceArtifact {
    #[must_use]
    pub fn service_id(&self) -> &ServiceId {
        &self.service_id
    }
    #[must_use]
    pub const fn target(&self) -> ServiceTier1Target {
        self.target
    }
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }
    #[must_use]
    pub fn binary_name(&self) -> &str {
        &self.binary_name
    }
    #[must_use]
    pub fn channel(&self) -> &str {
        &self.channel
    }
    #[must_use]
    pub fn binary_archive_name(&self) -> &str {
        &self.binary_archive_name
    }
    #[must_use]
    pub const fn binary_archive_format(&self) -> &'static str {
        self.binary_archive_format
    }
    #[must_use]
    pub fn binary_archive_member(&self) -> &str {
        &self.binary_archive_member
    }
    #[must_use]
    pub fn artifact_manifest_name(&self) -> &str {
        &self.artifact_manifest_name
    }
    #[must_use]
    pub fn checksums_name(&self) -> &str {
        &self.checksums_name
    }
    #[must_use]
    pub const fn checksum_algorithm(&self) -> &'static str {
        self.checksum_algorithm
    }
    #[must_use]
    pub const fn checksum_format(&self) -> &'static str {
        self.checksum_format
    }
    #[must_use]
    pub const fn release_contract_sha256(&self) -> ServiceArtifactSha256 {
        self.release_contract_sha256
    }
    #[must_use]
    pub const fn source_lock_sha256(&self) -> ServiceArtifactSha256 {
        self.source_lock_sha256
    }
    #[must_use]
    pub fn output_inventory(&self) -> &[String] {
        &self.output_inventory
    }
}

impl ResolvedServiceTarget {
    #[must_use]
    pub fn service_id(&self) -> &ServiceId {
        &self.service_id
    }

    #[must_use]
    pub const fn target(&self) -> ServiceTier1Target {
        self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeArtifactRequest<'a> {
    pub runtime_id: &'a str,
    pub os: &'a str,
    pub arch: &'a str,
    pub version: &'a str,
    pub channel: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRuntimeArtifact {
    pub runtime_id: String,
    pub release_unit: String,
    pub package_name: String,
    pub binary_name: Option<String>,
    pub artifact_adapter: String,
    pub channel: String,
    pub version: String,
    pub target_id: String,
    pub os: String,
    pub arch: String,
    pub archive_format: String,
    pub archive_extension: String,
    pub artifact_stem: String,
    pub artifact_file_name: String,
}

#[derive(Debug, Clone)]
pub struct RadrootsRuntimeDistributionResolver {
    contract: RadrootsRuntimeDistributionContract,
}

impl RadrootsRuntimeDistributionResolver {
    pub fn parse_str(raw: &str) -> Result<Self, RadrootsRuntimeDistributionError> {
        if raw.len() > RUNTIME_DISTRIBUTION_CONTRACT_MAX_UTF8_BYTES {
            return Err(RadrootsRuntimeDistributionError::ContractTooLarge);
        }
        let contract = toml::from_str::<RadrootsRuntimeDistributionContract>(raw)
            .map_err(|_| RadrootsRuntimeDistributionError::Parse)?;
        Self::new(contract)
    }

    pub fn new(
        contract: RadrootsRuntimeDistributionContract,
    ) -> Result<Self, RadrootsRuntimeDistributionError> {
        if contract.schema != RUNTIME_DISTRIBUTION_SCHEMA {
            return Err(RadrootsRuntimeDistributionError::UnexpectedSchema);
        }
        if contract.schema_version != RUNTIME_DISTRIBUTION_SCHEMA_VERSION {
            return Err(RadrootsRuntimeDistributionError::UnexpectedSchemaVersion);
        }
        if contract.runtime.iter().any(|runtime| {
            contract
                .service_targets
                .iter()
                .any(|(_, service)| runtime.id == service.service_id().as_str())
        }) {
            return Err(RadrootsRuntimeDistributionError::HardenedServiceLegacyArtifactRow);
        }
        if !contract
            .channels
            .active
            .iter()
            .any(|channel| channel == "stable")
            || !contract
                .channels
                .defined
                .iter()
                .any(|channel| channel == "stable")
        {
            return Err(RadrootsRuntimeDistributionError::InvalidServiceArtifactContract);
        }
        for (service_id, target) in contract.service_targets.iter() {
            let artifact = contract
                .service_artifacts
                .get(target.service_id())
                .ok_or(RadrootsRuntimeDistributionError::InvalidServiceArtifactContract)?;
            if service_id != artifact.service_id().as_str()
                || target.tier_1_targets() != artifact.tier_1_targets()
            {
                return Err(RadrootsRuntimeDistributionError::InvalidServiceArtifactContract);
            }
        }
        Ok(Self { contract })
    }

    pub fn contract(&self) -> &RadrootsRuntimeDistributionContract {
        &self.contract
    }

    pub fn service_target(
        &self,
        service_id: &ServiceId,
    ) -> Result<&HardenedServiceTarget, RadrootsRuntimeDistributionError> {
        self.contract
            .service_targets
            .get(service_id)
            .ok_or(RadrootsRuntimeDistributionError::UnsupportedService)
    }

    pub fn resolve_service_target(
        &self,
        request: &ServiceTargetRequest<'_>,
    ) -> Result<ResolvedServiceTarget, RadrootsRuntimeDistributionError> {
        let service = self.service_target(request.service_id)?;
        let target = ServiceTier1Target::parse(request.target_id)
            .filter(|target| service.tier_1_targets().contains(target))
            .ok_or(RadrootsRuntimeDistributionError::UnsupportedServiceTarget)?;
        Ok(ResolvedServiceTarget {
            service_id: request.service_id.clone(),
            target,
        })
    }

    pub fn service_artifact(
        &self,
        service_id: &ServiceId,
    ) -> Result<&HardenedServiceArtifact, RadrootsRuntimeDistributionError> {
        self.contract
            .service_artifacts
            .get(service_id)
            .ok_or(RadrootsRuntimeDistributionError::UnsupportedService)
    }

    pub fn resolve_service_artifact(
        &self,
        request: &ServiceArtifactRequest<'_>,
    ) -> Result<ResolvedServiceArtifact, RadrootsRuntimeDistributionError> {
        let artifact = self.service_artifact(request.service_id)?;
        let target = ServiceTier1Target::parse(request.target_id)
            .filter(|target| artifact.tier_1_targets().contains(target))
            .ok_or(RadrootsRuntimeDistributionError::UnsupportedServiceTarget)?;
        let service = artifact.service_id().as_str();
        let version = artifact.version();
        let binary = artifact.binary_name();
        Ok(ResolvedServiceArtifact {
            service_id: request.service_id.clone(),
            target,
            version: version.to_owned(),
            package_name: artifact.package_name().to_owned(),
            binary_name: binary.to_owned(),
            channel: artifact.channel().as_str().to_owned(),
            binary_archive_name: artifact.binary_archive_name().to_owned(),
            binary_archive_format: "tar.gz",
            binary_archive_member: format!("{service}-{version}-{}/{binary}", target.as_str()),
            artifact_manifest_name: artifact.artifact_manifest_name().to_owned(),
            checksums_name: artifact.checksums_name().to_owned(),
            checksum_algorithm: "sha256",
            checksum_format: "sha256_lower_hex_two_spaces_path_lf_sorted_by_path",
            release_contract_sha256: artifact.release_contract_sha256(),
            source_lock_sha256: artifact.source_lock_sha256(),
            output_inventory: artifact.output_inventory().to_vec(),
        })
    }

    pub fn resolve_artifact(
        &self,
        request: &RuntimeArtifactRequest<'_>,
    ) -> Result<ResolvedRuntimeArtifact, RadrootsRuntimeDistributionError> {
        let runtime = self
            .contract
            .runtime
            .iter()
            .find(|runtime| runtime.id == request.runtime_id)
            .ok_or(RadrootsRuntimeDistributionError::UnknownRuntime)?;

        if !runtime.human_installable {
            return Err(RadrootsRuntimeDistributionError::RuntimeNotInstallable);
        }

        let channel = request.channel.unwrap_or(runtime.default_channel.as_str());
        self.ensure_channel_is_active(channel)?;

        let target_set_id = runtime
            .target_set
            .as_ref()
            .ok_or(RadrootsRuntimeDistributionError::MissingTargetSet)?;

        let adapter = self
            .contract
            .artifact_adapters
            .get(&runtime.artifact_adapter)
            .ok_or(RadrootsRuntimeDistributionError::UnknownArtifactAdapter)?;

        let (target_id, target) =
            self.select_target(runtime, target_set_id, request.os, request.arch)?;
        let archive_format_id =
            self.resolve_archive_format_id(runtime, target_id, target, adapter)?;
        let archive_format = self
            .contract
            .archive_formats
            .get(&normalized_contract_key(archive_format_id))
            .ok_or(RadrootsRuntimeDistributionError::UnknownArchiveFormat)?;

        let artifact_stem = format!("{}-{}-{}", runtime.release_unit, request.version, target_id);
        let artifact_file_name = format!("{artifact_stem}{}", archive_format.extension);

        Ok(ResolvedRuntimeArtifact {
            runtime_id: runtime.id.clone(),
            release_unit: runtime.release_unit.clone(),
            package_name: runtime.package_name.clone(),
            binary_name: runtime.binary_name.clone(),
            artifact_adapter: runtime.artifact_adapter.clone(),
            channel: channel.to_string(),
            version: request.version.to_string(),
            target_id: target_id.to_string(),
            os: request.os.to_string(),
            arch: request.arch.to_string(),
            archive_format: archive_format_id.to_string(),
            archive_extension: archive_format.extension.clone(),
            artifact_stem,
            artifact_file_name,
        })
    }

    fn ensure_channel_is_active(
        &self,
        channel: &str,
    ) -> Result<(), RadrootsRuntimeDistributionError> {
        if !self
            .contract
            .channels
            .defined
            .iter()
            .any(|entry| entry == channel)
        {
            return Err(RadrootsRuntimeDistributionError::UnknownChannel);
        }
        if !self
            .contract
            .channels
            .active
            .iter()
            .any(|entry| entry == channel)
        {
            return Err(RadrootsRuntimeDistributionError::InactiveChannel);
        }
        Ok(())
    }

    fn select_target<'a>(
        &'a self,
        _runtime: &RuntimeDistributionEntry,
        target_set_id: &str,
        os: &str,
        arch: &str,
    ) -> Result<(&'a str, &'a TargetSpec), RadrootsRuntimeDistributionError> {
        let target_set = self
            .contract
            .target_sets
            .get(target_set_id)
            .ok_or(RadrootsRuntimeDistributionError::UnsupportedPlatform)?;

        let mut found_match = None;
        for target_id in &target_set.targets {
            let target = self
                .contract
                .targets
                .get(target_id)
                .ok_or(RadrootsRuntimeDistributionError::UnknownTarget)?;

            if target.os == os && target.arch == arch {
                found_match = Some((target_id.as_str(), target));
                break;
            }
        }

        found_match.ok_or(RadrootsRuntimeDistributionError::UnsupportedPlatform)
    }

    fn resolve_archive_format_id<'a>(
        &self,
        _runtime: &RuntimeDistributionEntry,
        _target_id: &'a str,
        target: &'a TargetSpec,
        adapter: &'a ArtifactAdapter,
    ) -> Result<&'a str, RadrootsRuntimeDistributionError> {
        if let Some(format) = target.archive_format.as_deref() {
            return Ok(format);
        }

        if adapter.supported_archive_formats.len() == 1 {
            return Ok(adapter.supported_archive_formats[0].as_str());
        }

        Err(RadrootsRuntimeDistributionError::MissingArchiveFormat)
    }
}

fn normalized_contract_key(value: &str) -> String {
    value.replace('.', "_")
}

use crate::error::RadrootsRuntimeDistributionError;
use crate::model::{
    ArtifactAdapter, RadrootsRuntimeDistributionContract, RuntimeDistributionEntry, TargetSpec,
};
use crate::service::{HardenedServiceTarget, ServiceTier1Target};
use radroots_runtime_paths::ServiceId;

pub const RUNTIME_DISTRIBUTION_SCHEMA: &str = "radroots-runtime-distribution";
pub const RUNTIME_DISTRIBUTION_SCHEMA_VERSION: u32 = 1;

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
            return Err(RadrootsRuntimeDistributionError::HardenedServiceArtifactDeferred);
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

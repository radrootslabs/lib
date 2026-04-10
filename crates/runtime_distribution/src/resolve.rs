use crate::error::RadrootsRuntimeDistributionError;
use crate::model::{
    ArtifactAdapter, RadrootsRuntimeDistributionContract, RuntimeDistributionEntry, TargetSpec,
};

pub const RUNTIME_DISTRIBUTION_SCHEMA: &str = "radroots_runtime_distribution";

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
            .map_err(|err| RadrootsRuntimeDistributionError::Parse(err.to_string()))?;
        Self::new(contract)
    }

    pub fn new(
        contract: RadrootsRuntimeDistributionContract,
    ) -> Result<Self, RadrootsRuntimeDistributionError> {
        if contract.schema != RUNTIME_DISTRIBUTION_SCHEMA {
            return Err(RadrootsRuntimeDistributionError::UnexpectedSchema {
                expected: RUNTIME_DISTRIBUTION_SCHEMA,
                found: contract.schema.clone(),
            });
        }
        Ok(Self { contract })
    }

    pub fn contract(&self) -> &RadrootsRuntimeDistributionContract {
        &self.contract
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
            .ok_or_else(|| {
                RadrootsRuntimeDistributionError::UnknownRuntime(request.runtime_id.to_string())
            })?;

        if !runtime.human_installable {
            return Err(RadrootsRuntimeDistributionError::RuntimeNotInstallable(
                runtime.id.clone(),
            ));
        }

        let channel = request.channel.unwrap_or(runtime.default_channel.as_str());
        self.ensure_channel_is_active(channel)?;

        let target_set_id = runtime.target_set.as_ref().ok_or_else(|| {
            RadrootsRuntimeDistributionError::MissingTargetSet(runtime.id.clone())
        })?;

        let adapter = self
            .contract
            .artifact_adapters
            .get(&runtime.artifact_adapter)
            .ok_or_else(
                || RadrootsRuntimeDistributionError::UnknownArtifactAdapter {
                    runtime_id: runtime.id.clone(),
                    adapter_id: runtime.artifact_adapter.clone(),
                },
            )?;

        let (target_id, target) =
            self.select_target(runtime, target_set_id, request.os, request.arch)?;
        let archive_format_id =
            self.resolve_archive_format_id(runtime, target_id, target, adapter)?;
        let archive_format = self
            .contract
            .archive_formats
            .get(&normalized_contract_key(archive_format_id))
            .ok_or_else(|| RadrootsRuntimeDistributionError::UnknownArchiveFormat {
                target_id: target_id.to_string(),
                archive_format_id: archive_format_id.to_string(),
            })?;

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
            return Err(RadrootsRuntimeDistributionError::UnknownChannel(
                channel.to_string(),
            ));
        }
        if !self
            .contract
            .channels
            .active
            .iter()
            .any(|entry| entry == channel)
        {
            return Err(RadrootsRuntimeDistributionError::InactiveChannel(
                channel.to_string(),
            ));
        }
        Ok(())
    }

    fn select_target<'a>(
        &'a self,
        runtime: &RuntimeDistributionEntry,
        target_set_id: &str,
        os: &str,
        arch: &str,
    ) -> Result<(&'a str, &'a TargetSpec), RadrootsRuntimeDistributionError> {
        let target_set = self
            .contract
            .target_sets
            .get(target_set_id)
            .ok_or_else(|| RadrootsRuntimeDistributionError::UnsupportedPlatform {
                runtime_id: runtime.id.clone(),
                os: os.to_string(),
                arch: arch.to_string(),
            })?;

        let mut found_match = None;
        for target_id in &target_set.targets {
            let target = self.contract.targets.get(target_id).ok_or_else(|| {
                RadrootsRuntimeDistributionError::UnknownTarget {
                    runtime_id: runtime.id.clone(),
                    target_set_id: target_set_id.to_string(),
                    target_id: target_id.clone(),
                }
            })?;

            if target.os == os && target.arch == arch {
                found_match = Some((target_id.as_str(), target));
                break;
            }
        }

        found_match.ok_or_else(|| RadrootsRuntimeDistributionError::UnsupportedPlatform {
            runtime_id: runtime.id.clone(),
            os: os.to_string(),
            arch: arch.to_string(),
        })
    }

    fn resolve_archive_format_id<'a>(
        &self,
        runtime: &RuntimeDistributionEntry,
        target_id: &'a str,
        target: &'a TargetSpec,
        adapter: &'a ArtifactAdapter,
    ) -> Result<&'a str, RadrootsRuntimeDistributionError> {
        if let Some(format) = target.archive_format.as_deref() {
            return Ok(format);
        }

        if adapter.supported_archive_formats.len() == 1 {
            return Ok(adapter.supported_archive_formats[0].as_str());
        }

        Err(RadrootsRuntimeDistributionError::MissingArchiveFormat {
            runtime_id: runtime.id.clone(),
            target_id: target_id.to_string(),
        })
    }
}

fn normalized_contract_key(value: &str) -> String {
    value.replace('.', "_")
}

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::service::HardenedServiceTargets;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RadrootsRuntimeDistributionContract {
    pub schema: String,
    pub schema_version: u32,
    pub owner_doc: String,
    pub runtime_registry: String,
    pub family: DistributionFamily,
    pub channels: ChannelSet,
    #[serde(default)]
    pub artifact_adapters: BTreeMap<String, ArtifactAdapter>,
    #[serde(default)]
    pub archive_formats: BTreeMap<String, ArchiveFormat>,
    #[serde(default)]
    pub target_sets: BTreeMap<String, TargetSet>,
    #[serde(default)]
    pub targets: BTreeMap<String, TargetSpec>,
    #[serde(default)]
    pub runtime: Vec<RuntimeDistributionEntry>,
    pub service_targets: HardenedServiceTargets,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DistributionFamily {
    pub id: String,
    pub canonical_installer_engine: String,
    pub human_install_facade: String,
    pub tooling_consumption: String,
    pub independent_runtime_versions: bool,
    pub version_resolution: String,
    pub artifact_verification_required: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChannelSet {
    #[serde(default)]
    pub active: Vec<String>,
    #[serde(default)]
    pub defined: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAdapter {
    pub kind: String,
    #[serde(default)]
    pub supported_archive_formats: Vec<String>,
    pub layout: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArchiveFormat {
    pub extension: String,
    #[serde(default)]
    pub platforms: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetSet {
    #[serde(default)]
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec {
    pub os: String,
    pub arch: String,
    pub archive_format: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDistributionEntry {
    pub id: String,
    pub distribution_state: String,
    pub release_unit: String,
    pub package_name: String,
    pub binary_name: Option<String>,
    pub artifact_adapter: String,
    pub target_set: Option<String>,
    pub default_channel: String,
    pub human_installable: bool,
    pub notes: Option<String>,
}

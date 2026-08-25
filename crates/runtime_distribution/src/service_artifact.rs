use std::collections::BTreeMap;

use radroots_runtime_paths::ServiceId;
use serde::{Deserialize, Deserializer};

use crate::service::ServiceTier1Target;

const OUTPUT_INVENTORY: [&str; 12] = [
    "LICENSE",
    "SHA256SUMS",
    "THIRD-PARTY-NOTICES.txt",
    "artifact-manifest.v1.json",
    "binary.tar.gz",
    "config.example.toml",
    "config.schema.json",
    "provenance-input.v1.json",
    "radroots.service.source-lock.v2.toml",
    "sbom.cdx.json",
    "service-source.tar.gz",
    "systemd.service",
];

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceArtifactChannel {
    Stable,
}

impl ServiceArtifactChannel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
        }
    }
}

/// A validated lowercase SHA-256 value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ServiceArtifactSha256([u8; 32]);

impl ServiceArtifactSha256 {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl core::fmt::Debug for ServiceArtifactSha256 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ServiceArtifactSha256(<redacted>)")
    }
}

impl<'de> Deserialize<'de> for ServiceArtifactSha256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(serde::de::Error::custom("invalid sha256"));
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_nibble(pair[0])
                .ok_or_else(|| serde::de::Error::custom("invalid sha256"))?
                << 4)
                | hex_nibble(pair[1]).ok_or_else(|| serde::de::Error::custom("invalid sha256"))?;
        }
        Ok(Self(bytes))
    }
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(try_from = "HardenedServiceArtifactWire")]
pub struct HardenedServiceArtifact {
    service_id: ServiceId,
    service_revision: String,
    release_contract: String,
    release_contract_sha256: ServiceArtifactSha256,
    source_lock_sha256: ServiceArtifactSha256,
    package_name: String,
    binary_name: String,
    version: String,
    channel: ServiceArtifactChannel,
    binary_archive_name: String,
    artifact_manifest_name: String,
    checksums_name: String,
    output_inventory: Vec<String>,
    tier_1_targets: Vec<ServiceTier1Target>,
}

impl core::fmt::Debug for HardenedServiceArtifact {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("HardenedServiceArtifact(<redacted>)")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HardenedServiceArtifactWire {
    service_id: ServiceId,
    service_revision: String,
    release_contract: String,
    release_contract_sha256: ServiceArtifactSha256,
    source_lock_sha256: ServiceArtifactSha256,
    package_name: String,
    binary_name: String,
    version: String,
    channel: ServiceArtifactChannel,
    binary_archive_name: String,
    artifact_manifest_name: String,
    checksums_name: String,
    output_inventory: Vec<String>,
    tier_1_targets: Vec<ServiceTier1Target>,
}

impl HardenedServiceArtifact {
    #[must_use]
    pub fn service_id(&self) -> &ServiceId {
        &self.service_id
    }
    #[must_use]
    pub fn service_revision(&self) -> &str {
        &self.service_revision
    }
    #[must_use]
    pub fn release_contract(&self) -> &str {
        &self.release_contract
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
    pub fn package_name(&self) -> &str {
        &self.package_name
    }
    #[must_use]
    pub fn binary_name(&self) -> &str {
        &self.binary_name
    }
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
    #[must_use]
    pub const fn channel(&self) -> ServiceArtifactChannel {
        self.channel
    }
    #[must_use]
    pub fn binary_archive_name(&self) -> &str {
        &self.binary_archive_name
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
    pub fn output_inventory(&self) -> &[String] {
        &self.output_inventory
    }
    #[must_use]
    pub fn tier_1_targets(&self) -> &[ServiceTier1Target] {
        &self.tier_1_targets
    }

    fn has_exact_contract(&self) -> bool {
        let (revision, release_contract, release_hash, source_lock_hash) =
            match self.service_id.as_str() {
                "myc" => (
                    "77b381648ed1e586efb696888beb05b9215c69cf",
                    "contracts/services_hardening/native_release.v2.json",
                    "4b3ba5789fac6aa219e84e1e5c002cf8230b72f95fd6d95a6419d2fdf2915f83",
                    "f5ebb390a480830d51d502facc623bd1b10eda27b12dad9f3dbb6a1f1f949217",
                ),
                "rhi" => (
                    "07aa6ea988da5372654bb3d1ee183ac099a77cae",
                    "contracts/services_hardening/native_release.v1.json",
                    "06a973176b4b8c11dad13000604576527df829dd0bbe2f501158662f75e70b94",
                    "3cc8bfac0d98730937754abae2ccfe20e40d0a9bbdefe02ebd94264c20f0d0ff",
                ),
                _ => return false,
            };
        self.service_revision == revision
            && self.release_contract == release_contract
            && self.release_contract_sha256 == sha256_literal(release_hash)
            && self.source_lock_sha256 == sha256_literal(source_lock_hash)
            && self.package_name == self.service_id.as_str()
            && self.binary_name == self.service_id.as_str()
            && self.version == "0.1.0"
            && self.channel == ServiceArtifactChannel::Stable
            && self.binary_archive_name == "binary.tar.gz"
            && self.artifact_manifest_name == "artifact-manifest.v1.json"
            && self.checksums_name == "SHA256SUMS"
            && self
                .output_inventory
                .iter()
                .map(String::as_str)
                .eq(OUTPUT_INVENTORY)
            && self.tier_1_targets == ServiceTier1Target::ALL
    }
}

fn sha256_literal(value: &str) -> ServiceArtifactSha256 {
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_nibble(pair[0]).expect("literal sha256") << 4)
            | hex_nibble(pair[1]).expect("literal sha256");
    }
    ServiceArtifactSha256(bytes)
}

impl TryFrom<HardenedServiceArtifactWire> for HardenedServiceArtifact {
    type Error = &'static str;

    fn try_from(wire: HardenedServiceArtifactWire) -> Result<Self, Self::Error> {
        let artifact = Self {
            service_id: wire.service_id,
            service_revision: wire.service_revision,
            release_contract: wire.release_contract,
            release_contract_sha256: wire.release_contract_sha256,
            source_lock_sha256: wire.source_lock_sha256,
            package_name: wire.package_name,
            binary_name: wire.binary_name,
            version: wire.version,
            channel: wire.channel,
            binary_archive_name: wire.binary_archive_name,
            artifact_manifest_name: wire.artifact_manifest_name,
            checksums_name: wire.checksums_name,
            output_inventory: wire.output_inventory,
            tier_1_targets: wire.tier_1_targets,
        };
        artifact
            .has_exact_contract()
            .then_some(artifact)
            .ok_or("invalid hardened service artifact")
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(try_from = "BTreeMap<String, HardenedServiceArtifact>")]
pub struct HardenedServiceArtifacts(BTreeMap<String, HardenedServiceArtifact>);

impl HardenedServiceArtifacts {
    #[must_use]
    pub fn get(&self, service_id: &ServiceId) -> Option<&HardenedServiceArtifact> {
        self.0.get(service_id.as_str())
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &HardenedServiceArtifact)> {
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

impl TryFrom<BTreeMap<String, HardenedServiceArtifact>> for HardenedServiceArtifacts {
    type Error = &'static str;

    fn try_from(artifacts: BTreeMap<String, HardenedServiceArtifact>) -> Result<Self, Self::Error> {
        if artifacts.len() != 2 {
            return Err("service artifact inventory must contain exactly Myc and RHI");
        }
        for service in ["myc", "rhi"] {
            let artifact = artifacts
                .get(service)
                .ok_or("service artifact inventory is incomplete")?;
            if artifact.service_id.as_str() != service || !artifact.has_exact_contract() {
                return Err("service artifact inventory is invalid");
            }
        }
        if artifacts
            .iter()
            .any(|(key, artifact)| key != artifact.service_id.as_str())
        {
            return Err("service artifact key mismatch");
        }
        Ok(Self(artifacts))
    }
}

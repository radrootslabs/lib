#![forbid(unsafe_code)]

pub mod error;
pub mod model;
pub mod resolve;
pub mod service;
mod service_artifact;

pub use error::RadrootsRuntimeDistributionError;
pub use model::{
    ArchiveFormat, ArtifactAdapter, ChannelSet, DistributionFamily,
    RadrootsRuntimeDistributionContract, RuntimeDistributionEntry, TargetSet, TargetSpec,
};
pub use resolve::{
    RUNTIME_DISTRIBUTION_CONTRACT_MAX_UTF8_BYTES, RUNTIME_DISTRIBUTION_SCHEMA,
    RUNTIME_DISTRIBUTION_SCHEMA_VERSION, RadrootsRuntimeDistributionResolver,
    ResolvedRuntimeArtifact, ResolvedServiceArtifact, ResolvedServiceTarget,
    RuntimeArtifactRequest, ServiceArtifactRequest, ServiceTargetRequest,
};
pub use service::{
    HardenedServiceTarget, HardenedServiceTargets, ServiceAdminBasePath, ServiceAdminTransport,
    ServiceConfigurationFormat, ServiceInstanceSupport, ServiceOperationsSurface,
    ServiceRunStatePolicy, ServiceStateInitialization, ServiceStatusSurface, ServiceSupportPosture,
    ServiceTier1Target,
};
pub use service_artifact::{
    HardenedServiceArtifact, HardenedServiceArtifacts, ServiceArtifactChannel,
    ServiceArtifactSha256,
};

#[cfg(test)]
mod tests {
    use radroots_runtime_paths::ServiceId;
    use toml::Value;

    use super::{
        HardenedServiceTarget, RUNTIME_DISTRIBUTION_CONTRACT_MAX_UTF8_BYTES,
        RUNTIME_DISTRIBUTION_SCHEMA, RadrootsRuntimeDistributionContract,
        RadrootsRuntimeDistributionError, RadrootsRuntimeDistributionResolver,
        RuntimeArtifactRequest, RuntimeDistributionEntry, ServiceAdminBasePath,
        ServiceAdminTransport, ServiceArtifactRequest, ServiceArtifactSha256,
        ServiceConfigurationFormat, ServiceInstanceSupport, ServiceOperationsSurface,
        ServiceRunStatePolicy, ServiceStateInitialization, ServiceStatusSurface,
        ServiceSupportPosture, ServiceTargetRequest, ServiceTier1Target,
    };

    const HARDENED_SERVICE_CONTRACT: &str =
        include_str!("../tests/fixtures/hardened_service_targets.v1.toml");

    const CONTRACT: &str = r#"
schema = "radroots-runtime-distribution"
schema_version = 1
owner_doc = "docs/execution/rcl/radroots-modular-runtime-management-bootstrap-rcl.md"
runtime_registry = "registry.toml"

[family]
id = "radroots_runtime-family"
canonical_installer_engine = "single_runtime_selected"
human_install_facade = "delivery_publication_only"
tooling_consumption = "shared_distribution_library"
independent_runtime_versions = true
version_resolution = "runtime_scoped_channel_latest"
artifact_verification_required = true

[channels]
active = ["stable"]
defined = ["stable", "candidate", "nightly"]

[artifact_adapters.rust_binary_archive]
kind = "binary_archive"
supported_archive_formats = ["tar.gz", "zip"]
layout = "single_binary_plus_supporting_files"

[artifact_adapters.desktop_bundle]
kind = "desktop_bundle"
supported_archive_formats = ["tar.gz", "zip", "dmg"]
layout = "host_native_bundle"

[artifact_adapters.mobile_store_package]
kind = "mobile_store_package"
supported_archive_formats = []
layout = "platform_store_managed"

[artifact_adapters.mojo_workspace_archive]
kind = "workspace_archive"
supported_archive_formats = ["tar.gz"]
layout = "workspace_tree"

[archive_formats.tar_gz]
extension = ".tar.gz"
platforms = ["linux", "macos"]

[archive_formats.zip]
extension = ".zip"
platforms = ["windows"]

[archive_formats.dmg]
extension = ".dmg"
platforms = ["macos"]

[target_sets.server_default]
targets = [
  "x86_64-unknown-linux-gnu",
  "aarch64-unknown-linux-gnu",
  "x86_64-apple-darwin",
  "aarch64-apple-darwin",
]

[target_sets.cli_default]
targets = [
  "x86_64-unknown-linux-gnu",
  "aarch64-unknown-linux-gnu",
  "x86_64-apple-darwin",
  "aarch64-apple-darwin",
]

[target_sets.desktop_default]
targets = [
  "x86_64-apple-darwin",
  "aarch64-apple-darwin",
]

[target_sets.mojo_workspace_default]
targets = [
  "osx-arm64",
  "linux-64",
]

[targets.x86_64-unknown-linux-gnu]
os = "linux"
arch = "amd64"
archive_format = "tar.gz"

[targets.aarch64-unknown-linux-gnu]
os = "linux"
arch = "arm64"
archive_format = "tar.gz"

[targets.x86_64-apple-darwin]
os = "macos"
arch = "amd64"
archive_format = "tar.gz"

[targets.aarch64-apple-darwin]
os = "macos"
arch = "arm64"
archive_format = "tar.gz"

[targets.osx-arm64]
os = "macos"
arch = "arm64"
archive_format = "tar.gz"

[targets.linux-64]
os = "linux"
arch = "amd64"
archive_format = "tar.gz"

[[runtime]]
id = "cli"
distribution_state = "active"
release_unit = "cli"
package_name = "radroots_cli"
binary_name = "radroots"
artifact_adapter = "rust_binary_archive"
target_set = "cli_default"
default_channel = "stable"
human_installable = true

[[runtime]]
id = "radrootsd"
distribution_state = "active"
release_unit = "radrootsd"
package_name = "radrootsd"
binary_name = "radrootsd"
artifact_adapter = "rust_binary_archive"
target_set = "server_default"
default_channel = "stable"
human_installable = true

[[runtime]]
id = "fixture-desktop-bundle"
distribution_state = "defined"
release_unit = "fixture-desktop-bundle"
package_name = "fixture_desktop_bundle"
binary_name = "fixture_desktop_bundle"
artifact_adapter = "desktop_bundle"
target_set = "desktop_default"
default_channel = "stable"
human_installable = true

[[runtime]]
id = "fixture-mobile-package"
distribution_state = "external_platform_managed"
release_unit = "fixture-mobile-package"
package_name = "fixture_mobile_package"
artifact_adapter = "mobile_store_package"
default_channel = "stable"
human_installable = false

[[runtime]]
id = "hyf"
distribution_state = "bootstrap_only"
release_unit = "hyf"
package_name = "hyf"
binary_name = "hyf"
artifact_adapter = "mojo_workspace_archive"
target_set = "mojo_workspace_default"
default_channel = "stable"
human_installable = false

[service_targets.myc]
service_id = "myc"
instance_support = "multiple"
config_format = "toml"
state_initialization = "explicit"
run_state_policy = "existing_only"
admin_transport = "http11_over_unix_domain_socket"
admin_base_path = "/v1"
admin_contract_version = 1
status_surface = "local_admin_service_status_v1"
operations_surface = "cached_livez_readyz_metrics"
support_posture = "target"
tier_1_targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]

[service_targets.rhi]
service_id = "rhi"
instance_support = "multiple"
config_format = "toml"
state_initialization = "explicit"
run_state_policy = "existing_only"
admin_transport = "http11_over_unix_domain_socket"
admin_base_path = "/v1"
admin_contract_version = 1
status_surface = "local_admin_service_status_v1"
operations_surface = "cached_livez_readyz_metrics"
support_posture = "target"
tier_1_targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]

[service_artifacts.myc]
service_id = "myc"
service_revision = "77b381648ed1e586efb696888beb05b9215c69cf"
release_contract = "contracts/services_hardening/native_release.v2.json"
release_contract_sha256 = "4b3ba5789fac6aa219e84e1e5c002cf8230b72f95fd6d95a6419d2fdf2915f83"
source_lock_sha256 = "f5ebb390a480830d51d502facc623bd1b10eda27b12dad9f3dbb6a1f1f949217"
package_name = "myc"
binary_name = "myc"
version = "0.1.0"
channel = "stable"
binary_archive_name = "binary.tar.gz"
artifact_manifest_name = "artifact-manifest.v1.json"
checksums_name = "SHA256SUMS"
output_inventory = ["LICENSE", "SHA256SUMS", "THIRD-PARTY-NOTICES.txt", "artifact-manifest.v1.json", "binary.tar.gz", "config.example.toml", "config.schema.json", "provenance-input.v1.json", "radroots.service.source-lock.v2.toml", "sbom.cdx.json", "service-source.tar.gz", "systemd.service"]
tier_1_targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]

[service_artifacts.rhi]
service_id = "rhi"
service_revision = "07aa6ea988da5372654bb3d1ee183ac099a77cae"
release_contract = "contracts/services_hardening/native_release.v1.json"
release_contract_sha256 = "06a973176b4b8c11dad13000604576527df829dd0bbe2f501158662f75e70b94"
source_lock_sha256 = "3cc8bfac0d98730937754abae2ccfe20e40d0a9bbdefe02ebd94264c20f0d0ff"
package_name = "rhi"
binary_name = "rhi"
version = "0.1.0"
channel = "stable"
binary_archive_name = "binary.tar.gz"
artifact_manifest_name = "artifact-manifest.v1.json"
checksums_name = "SHA256SUMS"
output_inventory = ["LICENSE", "SHA256SUMS", "THIRD-PARTY-NOTICES.txt", "artifact-manifest.v1.json", "binary.tar.gz", "config.example.toml", "config.schema.json", "provenance-input.v1.json", "radroots.service.source-lock.v2.toml", "sbom.cdx.json", "service-source.tar.gz", "systemd.service"]
tier_1_targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]
"#;

    fn contract_value() -> Value {
        toml::from_str(CONTRACT).expect("parse contract value")
    }

    fn resolver_from_value(value: Value) -> RadrootsRuntimeDistributionResolver {
        let raw = toml::to_string(&value).expect("serialize contract");
        RadrootsRuntimeDistributionResolver::parse_str(&raw).expect("parse resolver")
    }

    fn resolve_error(
        resolver: &RadrootsRuntimeDistributionResolver,
        request: RuntimeArtifactRequest<'_>,
    ) -> RadrootsRuntimeDistributionError {
        resolver
            .resolve_artifact(&request)
            .expect_err("request should fail")
    }

    #[test]
    fn parse_str_accepts_the_expected_schema() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        assert_eq!(resolver.contract().schema, RUNTIME_DISTRIBUTION_SCHEMA);
        assert_eq!(resolver.contract().runtime.len(), 5);
    }

    #[test]
    fn parse_str_rejects_invalid_toml() {
        let err = RadrootsRuntimeDistributionResolver::parse_str("schema = [")
            .expect_err("invalid toml should fail");
        assert_eq!(err, RadrootsRuntimeDistributionError::Parse);
    }

    #[test]
    fn parse_str_caps_the_complete_document_before_toml_parsing() {
        let mut exact = CONTRACT.to_owned();
        exact.push('#');
        exact.extend(std::iter::repeat_n(
            'x',
            RUNTIME_DISTRIBUTION_CONTRACT_MAX_UTF8_BYTES - exact.len(),
        ));
        assert_eq!(exact.len(), RUNTIME_DISTRIBUTION_CONTRACT_MAX_UTF8_BYTES);
        RadrootsRuntimeDistributionResolver::parse_str(&exact)
            .expect("exact maximum contract remains admissible");

        exact.push('x');
        assert_eq!(
            RadrootsRuntimeDistributionResolver::parse_str(&exact)
                .expect_err("maximum plus one must fail"),
            RadrootsRuntimeDistributionError::ContractTooLarge
        );

        let very_large = format!("{}#{}", CONTRACT, "x".repeat(4 * 1024 * 1024));
        assert_eq!(
            RadrootsRuntimeDistributionResolver::parse_str(&very_large)
                .expect_err("very large contract must fail"),
            RadrootsRuntimeDistributionError::ContractTooLarge
        );
    }

    #[test]
    fn new_rejects_unexpected_schema() {
        let mut contract = contract_value();
        contract["schema"] = Value::String("wrong-schema".to_string());

        let raw = toml::to_string(&contract).expect("serialize contract");
        let err = RadrootsRuntimeDistributionResolver::parse_str(&raw)
            .expect_err("unexpected schema should fail");

        assert_eq!(err, RadrootsRuntimeDistributionError::UnexpectedSchema);
    }

    #[test]
    fn resolves_cli_linux_artifact_with_explicit_channel() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        let artifact = resolver
            .resolve_artifact(&RuntimeArtifactRequest {
                runtime_id: "cli",
                os: "linux",
                arch: "amd64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            })
            .expect("resolve cli artifact");

        assert_eq!(artifact.binary_name.as_deref(), Some("radroots"));
        assert_eq!(artifact.target_id, "x86_64-unknown-linux-gnu");
        assert_eq!(artifact.archive_extension, ".tar.gz");
        assert_eq!(
            artifact.artifact_file_name,
            "cli-1.0.0-alpha.1-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    #[test]
    fn resolves_radrootsd_linux_arm64_using_default_channel() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        let artifact = resolver
            .resolve_artifact(&RuntimeArtifactRequest {
                runtime_id: "radrootsd",
                os: "linux",
                arch: "arm64",
                version: "1.0.0-alpha.1",
                channel: None,
            })
            .expect("resolve radrootsd artifact");

        assert_eq!(artifact.channel, "stable");
        assert_eq!(artifact.target_id, "aarch64-unknown-linux-gnu");
        assert_eq!(artifact.binary_name.as_deref(), Some("radrootsd"));
    }

    #[test]
    fn resolves_desktop_bundle_for_macos_arm64() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        let artifact = resolver
            .resolve_artifact(&RuntimeArtifactRequest {
                runtime_id: "fixture-desktop-bundle",
                os: "macos",
                arch: "arm64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            })
            .expect("resolve desktop artifact");

        assert_eq!(artifact.target_id, "aarch64-apple-darwin");
        assert_eq!(artifact.package_name, "fixture_desktop_bundle");
    }

    #[test]
    fn rejects_non_installable_mobile_runtime() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        let err = resolver
            .resolve_artifact(&RuntimeArtifactRequest {
                runtime_id: "fixture-mobile-package",
                os: "macos",
                arch: "arm64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            })
            .expect_err("mobile runtime should not be installable");

        assert_eq!(err, RadrootsRuntimeDistributionError::RuntimeNotInstallable);
    }

    #[test]
    fn rejects_bootstrap_only_runtime() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        let err = resolver
            .resolve_artifact(&RuntimeArtifactRequest {
                runtime_id: "hyf",
                os: "macos",
                arch: "arm64",
                version: "0.1.0",
                channel: Some("stable"),
            })
            .expect_err("bootstrap runtime should not be installable");

        assert_eq!(err, RadrootsRuntimeDistributionError::RuntimeNotInstallable);
    }

    #[test]
    fn rejects_inactive_channel() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        let err = resolver
            .resolve_artifact(&RuntimeArtifactRequest {
                runtime_id: "cli",
                os: "linux",
                arch: "amd64",
                version: "1.0.0-alpha.1",
                channel: Some("candidate"),
            })
            .expect_err("candidate channel should be inactive");

        assert_eq!(err, RadrootsRuntimeDistributionError::InactiveChannel);
    }

    #[test]
    fn rejects_unknown_runtime() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        let err = resolve_error(
            &resolver,
            RuntimeArtifactRequest {
                runtime_id: "missing-runtime",
                os: "linux",
                arch: "amd64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(err, RadrootsRuntimeDistributionError::UnknownRuntime);
    }

    #[test]
    fn rejects_unknown_channel() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        let err = resolve_error(
            &resolver,
            RuntimeArtifactRequest {
                runtime_id: "cli",
                os: "linux",
                arch: "amd64",
                version: "1.0.0-alpha.1",
                channel: Some("beta"),
            },
        );

        assert_eq!(err, RadrootsRuntimeDistributionError::UnknownChannel);
    }

    #[test]
    fn rejects_unsupported_platform() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        let err = resolver
            .resolve_artifact(&RuntimeArtifactRequest {
                runtime_id: "radrootsd",
                os: "windows",
                arch: "amd64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            })
            .expect_err("windows target should be unsupported");

        assert_eq!(err, RadrootsRuntimeDistributionError::UnsupportedPlatform);
    }

    #[test]
    fn rejects_runtime_with_missing_target_set() {
        let mut contract = contract_value();
        let runtime = contract["runtime"]
            .as_array_mut()
            .expect("runtime array")
            .iter_mut()
            .find(|runtime| runtime["id"].as_str() == Some("fixture-mobile-package"))
            .expect("ios runtime");
        runtime["human_installable"] = Value::Boolean(true);

        let resolver = resolver_from_value(contract);
        let err = resolve_error(
            &resolver,
            RuntimeArtifactRequest {
                runtime_id: "fixture-mobile-package",
                os: "ios",
                arch: "arm64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(err, RadrootsRuntimeDistributionError::MissingTargetSet);
    }

    #[test]
    fn rejects_unknown_artifact_adapter() {
        let mut contract = contract_value();
        let runtime = contract["runtime"]
            .as_array_mut()
            .expect("runtime array")
            .iter_mut()
            .find(|runtime| runtime["id"].as_str() == Some("cli"))
            .expect("cli runtime");
        runtime["artifact_adapter"] = Value::String("missing_adapter".to_string());

        let resolver = resolver_from_value(contract);
        let err = resolve_error(
            &resolver,
            RuntimeArtifactRequest {
                runtime_id: "cli",
                os: "linux",
                arch: "amd64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::UnknownArtifactAdapter
        );
    }

    #[test]
    fn rejects_missing_target_set_definition() {
        let mut contract = contract_value();
        let runtime = contract["runtime"]
            .as_array_mut()
            .expect("runtime array")
            .iter_mut()
            .find(|runtime| runtime["id"].as_str() == Some("cli"))
            .expect("cli runtime");
        runtime["target_set"] = Value::String("missing-target-set".to_string());

        let resolver = resolver_from_value(contract);
        let err = resolve_error(
            &resolver,
            RuntimeArtifactRequest {
                runtime_id: "cli",
                os: "linux",
                arch: "amd64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(err, RadrootsRuntimeDistributionError::UnsupportedPlatform);
    }

    #[test]
    fn rejects_target_set_with_unknown_target() {
        let mut contract = contract_value();
        contract["target_sets"]["cli_default"]["targets"] =
            Value::Array(vec![Value::String("missing-target".to_string())]);

        let resolver = resolver_from_value(contract);
        let err = resolve_error(
            &resolver,
            RuntimeArtifactRequest {
                runtime_id: "cli",
                os: "linux",
                arch: "amd64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(err, RadrootsRuntimeDistributionError::UnknownTarget);
    }

    #[test]
    fn infers_archive_format_from_single_supported_adapter_format() {
        let mut contract = contract_value();
        contract["targets"]["x86_64-unknown-linux-gnu"]
            .as_table_mut()
            .expect("target table")
            .remove("archive_format");
        contract["artifact_adapters"]["rust_binary_archive"]["supported_archive_formats"] =
            Value::Array(vec![Value::String("tar.gz".to_string())]);

        let resolver = resolver_from_value(contract);
        let artifact = resolver
            .resolve_artifact(&RuntimeArtifactRequest {
                runtime_id: "cli",
                os: "linux",
                arch: "amd64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            })
            .expect("single supported format should be inferred");

        assert_eq!(artifact.archive_format, "tar.gz");
        assert_eq!(artifact.archive_extension, ".tar.gz");
    }

    #[test]
    fn rejects_unknown_archive_format_reference() {
        let mut contract = contract_value();
        contract["targets"]["x86_64-unknown-linux-gnu"]["archive_format"] =
            Value::String("tar.xz".to_string());

        let resolver = resolver_from_value(contract);
        let err = resolve_error(
            &resolver,
            RuntimeArtifactRequest {
                runtime_id: "cli",
                os: "linux",
                arch: "amd64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(err, RadrootsRuntimeDistributionError::UnknownArchiveFormat);
    }

    #[test]
    fn rejects_missing_archive_format_when_adapter_is_ambiguous() {
        let mut contract = contract_value();
        contract["targets"]["aarch64-apple-darwin"]
            .as_table_mut()
            .expect("target table")
            .remove("archive_format");

        let resolver = resolver_from_value(contract);
        let err = resolve_error(
            &resolver,
            RuntimeArtifactRequest {
                runtime_id: "fixture-desktop-bundle",
                os: "macos",
                arch: "arm64",
                version: "1.0.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(err, RadrootsRuntimeDistributionError::MissingArchiveFormat);
    }

    #[test]
    fn durable_contract_resolves_exact_hardened_service_metadata() {
        let resolver = RadrootsRuntimeDistributionResolver::parse_str(HARDENED_SERVICE_CONTRACT)
            .expect("hardened service contract");

        for service in ["myc", "rhi"] {
            let service_id = ServiceId::new(service).expect("service id");
            let metadata = resolver
                .service_target(&service_id)
                .expect("service metadata");
            assert_eq!(metadata.service_id(), &service_id);
            assert_eq!(
                metadata.instance_support(),
                ServiceInstanceSupport::Multiple
            );
            assert_eq!(metadata.config_format(), ServiceConfigurationFormat::Toml);
            assert_eq!(metadata.config_format().as_str(), "toml");
            assert_eq!(
                metadata.state_initialization(),
                ServiceStateInitialization::Explicit
            );
            assert_eq!(
                metadata.run_state_policy(),
                ServiceRunStatePolicy::ExistingOnly
            );
            assert_eq!(
                metadata.admin_transport(),
                ServiceAdminTransport::Http11OverUnixDomainSocket
            );
            assert_eq!(metadata.admin_base_path(), ServiceAdminBasePath::V1);
            assert_eq!(metadata.admin_contract_version(), 1);
            assert_eq!(
                metadata.status_surface(),
                ServiceStatusSurface::LocalAdminServiceStatusV1
            );
            assert_eq!(
                metadata.operations_surface(),
                ServiceOperationsSurface::CachedLivezReadyzMetrics
            );
            assert_eq!(
                metadata.operations_surface().routes(),
                ["/livez", "/readyz", "/metrics"]
            );
            assert_eq!(metadata.support_posture(), ServiceSupportPosture::Target);
            assert_eq!(metadata.tier_1_targets(), ServiceTier1Target::ALL);

            for target in ServiceTier1Target::ALL {
                let resolved = resolver
                    .resolve_service_target(&ServiceTargetRequest {
                        service_id: &service_id,
                        target_id: target.as_str(),
                    })
                    .expect("eligible target");
                assert_eq!(resolved.service_id(), &service_id);
                assert_eq!(resolved.target(), target);
            }
        }

        let targets = &resolver.contract().service_targets;
        assert_eq!(targets.len(), 2);
        assert!(!targets.is_empty());
        assert_eq!(
            targets
                .iter()
                .map(|(service, _)| service)
                .collect::<Vec<_>>(),
            ["myc", "rhi"]
        );
        let artifacts = &resolver.contract().service_artifacts;
        assert_eq!(artifacts.len(), 2);
        assert!(!artifacts.is_empty());
        assert_eq!(
            artifacts
                .iter()
                .map(|(service, _)| service)
                .collect::<Vec<_>>(),
            ["myc", "rhi"]
        );

        let literal = ServiceArtifactSha256::from_bytes([0x5a; 32]);
        assert_eq!(literal.as_bytes(), &[0x5a; 32]);
        assert_eq!(format!("{literal:?}"), "ServiceArtifactSha256(<redacted>)");
    }

    #[test]
    fn hardened_services_resolve_exact_native_artifacts_and_reject_unsupported_targets() {
        let resolver = RadrootsRuntimeDistributionResolver::parse_str(HARDENED_SERVICE_CONTRACT)
            .expect("hardened service contract");
        let myc = ServiceId::new("myc").expect("myc");

        for target_id in ["aarch64-apple-darwin", "x86_64-pc-windows-msvc", "linux-64"] {
            assert_eq!(
                resolver.resolve_service_target(&ServiceTargetRequest {
                    service_id: &myc,
                    target_id,
                }),
                Err(RadrootsRuntimeDistributionError::UnsupportedServiceTarget)
            );
        }
        let artifact = resolver
            .resolve_service_artifact(&ServiceArtifactRequest {
                service_id: &myc,
                target_id: "x86_64-unknown-linux-gnu",
            })
            .expect("Myc artifact");
        assert_eq!(artifact.service_id(), &myc);
        assert_eq!(artifact.target(), ServiceTier1Target::X86_64UnknownLinuxGnu);
        assert_eq!(artifact.version(), "0.1.0");
        assert_eq!(artifact.package_name(), "myc");
        assert_eq!(artifact.binary_name(), "myc");
        assert_eq!(artifact.channel(), "stable");
        assert_eq!(artifact.binary_archive_name(), "binary.tar.gz");
        assert_eq!(artifact.binary_archive_format(), "tar.gz");
        assert_eq!(
            artifact.binary_archive_member(),
            "myc-0.1.0-x86_64-unknown-linux-gnu/myc"
        );
        assert_eq!(
            artifact.artifact_manifest_name(),
            "artifact-manifest.v1.json"
        );
        assert_eq!(artifact.checksums_name(), "SHA256SUMS");
        assert_eq!(artifact.checksum_algorithm(), "sha256");
        assert_eq!(
            artifact.checksum_format(),
            "sha256_lower_hex_two_spaces_path_lf_sorted_by_path"
        );
        assert_eq!(artifact.output_inventory().len(), 12);
        assert_eq!(artifact.output_inventory()[0], "LICENSE");
        assert_eq!(artifact.output_inventory()[11], "systemd.service");
        assert_eq!(
            artifact.release_contract_sha256(),
            resolver
                .service_artifact(&myc)
                .expect("Myc release")
                .release_contract_sha256()
        );
        assert_eq!(
            artifact.source_lock_sha256(),
            resolver
                .service_artifact(&myc)
                .expect("Myc release")
                .source_lock_sha256()
        );
        let rendered = format!("{artifact:?}");
        assert!(!rendered.contains("4b3ba578"));
        assert!(!rendered.contains("f5ebb390"));
        let rhi = ServiceId::new("rhi").expect("rhi");
        let rhi_artifact = resolver
            .resolve_service_artifact(&ServiceArtifactRequest {
                service_id: &rhi,
                target_id: "aarch64-unknown-linux-gnu",
            })
            .expect("RHI artifact");
        assert_eq!(
            rhi_artifact.binary_archive_member(),
            "rhi-0.1.0-aarch64-unknown-linux-gnu/rhi"
        );
        assert_eq!(
            resolver.resolve_artifact(&RuntimeArtifactRequest {
                runtime_id: "myc",
                os: "linux",
                arch: "amd64",
                version: "1.0.0",
                channel: None,
            }),
            Err(RadrootsRuntimeDistributionError::UnknownRuntime)
        );
    }

    #[test]
    fn hardened_services_reject_parsed_and_direct_artifact_authority() {
        let raw = format!(
            "{HARDENED_SERVICE_CONTRACT}\n\
             [[runtime]]\n\
             id = \"myc\"\n\
             distribution_state = \"defined\"\n\
             release_unit = \"myc\"\n\
             package_name = \"radroots_myc\"\n\
             artifact_adapter = \"rust_binary_archive\"\n\
             default_channel = \"stable\"\n\
             human_installable = true\n"
        );
        assert_eq!(
            RadrootsRuntimeDistributionResolver::parse_str(&raw).expect_err("parsed bypass"),
            RadrootsRuntimeDistributionError::HardenedServiceLegacyArtifactRow
        );

        let mut contract =
            toml::from_str::<RadrootsRuntimeDistributionContract>(HARDENED_SERVICE_CONTRACT)
                .expect("direct contract");
        contract.runtime.push(RuntimeDistributionEntry {
            id: "rhi".to_owned(),
            distribution_state: "defined".to_owned(),
            release_unit: "rhi".to_owned(),
            package_name: "radroots_rhi".to_owned(),
            binary_name: None,
            artifact_adapter: "rust_binary_archive".to_owned(),
            target_set: None,
            default_channel: "stable".to_owned(),
            human_installable: true,
            notes: None,
        });
        assert_eq!(
            RadrootsRuntimeDistributionResolver::new(contract).expect_err("direct bypass"),
            RadrootsRuntimeDistributionError::HardenedServiceLegacyArtifactRow
        );
    }

    #[test]
    fn hardened_service_artifacts_reject_every_identity_and_inventory_drift() {
        for (needle, replacement) in [
            ("version = \"0.1.0\"", "version = \"0.1.1\""),
            ("channel = \"stable\"", "channel = \"candidate\""),
            (
                "binary_archive_name = \"binary.tar.gz\"",
                "binary_archive_name = \"myc.tar.gz\"",
            ),
            (
                "checksums_name = \"SHA256SUMS\"",
                "checksums_name = \"checksums.txt\"",
            ),
            (
                "77b381648ed1e586efb696888beb05b9215c69cf",
                "77b381648ed1e586efb696888beb05b9215c69ce",
            ),
            (
                "4b3ba5789fac6aa219e84e1e5c002cf8230b72f95fd6d95a6419d2fdf2915f83",
                "4b3ba5789fac6aa219e84e1e5c002cf8230b72f95fd6d95a6419d2fdf2915f84",
            ),
        ] {
            let drift = HARDENED_SERVICE_CONTRACT.replacen(needle, replacement, 1);
            assert!(
                RadrootsRuntimeDistributionResolver::parse_str(&drift).is_err(),
                "drift `{needle}` must fail"
            );
        }

        let missing_member =
            HARDENED_SERVICE_CONTRACT.replacen("\"systemd.service\"", "\"unexpected\"", 1);
        assert!(RadrootsRuntimeDistributionResolver::parse_str(&missing_member).is_err());
        let uppercase_hash = HARDENED_SERVICE_CONTRACT.replacen("4b3ba578", "4B3BA578", 1);
        assert!(RadrootsRuntimeDistributionResolver::parse_str(&uppercase_hash).is_err());
        let unknown = HARDENED_SERVICE_CONTRACT.replacen(
            "[service_artifacts.myc]",
            "[service_artifacts.myc]\nunknown = true",
            1,
        );
        assert!(RadrootsRuntimeDistributionResolver::parse_str(&unknown).is_err());
    }

    #[test]
    fn hardened_service_contract_rejects_schema_drift_unknown_fields_and_inventory_drift() {
        let mut target_drift: Value =
            toml::from_str(HARDENED_SERVICE_CONTRACT).expect("contract fixture value");
        target_drift["service_targets"]["myc"]["tier_1_targets"][1] =
            Value::String("aarch64-apple-darwin".to_owned());

        for raw in [
            HARDENED_SERVICE_CONTRACT.replace("schema_version = 1", "schema_version = 2"),
            format!("{HARDENED_SERVICE_CONTRACT}\nunknown = true\n"),
            HARDENED_SERVICE_CONTRACT.replace("service_id = \"rhi\"", "service_id = \"other\""),
            toml::to_string(&target_drift).expect("target-drift contract"),
        ] {
            assert!(RadrootsRuntimeDistributionResolver::parse_str(&raw).is_err());
        }

        let mut missing_service: Value =
            toml::from_str(HARDENED_SERVICE_CONTRACT).expect("contract fixture value");
        missing_service["service_targets"]
            .as_table_mut()
            .expect("service target table")
            .remove("rhi");
        assert!(
            RadrootsRuntimeDistributionResolver::parse_str(
                &toml::to_string(&missing_service).expect("missing-service contract")
            )
            .is_err()
        );

        let mut mismatched_service: Value =
            toml::from_str(HARDENED_SERVICE_CONTRACT).expect("contract fixture value");
        let targets = mismatched_service["service_targets"]
            .as_table_mut()
            .expect("service target table");
        targets["myc"]["service_id"] = Value::String("rhi".to_owned());
        assert!(
            RadrootsRuntimeDistributionResolver::parse_str(
                &toml::to_string(&mismatched_service).expect("mismatched-service contract")
            )
            .is_err()
        );
    }

    #[test]
    fn standalone_hardened_service_target_rejects_contract_drift() {
        let contract: Value =
            toml::from_str(HARDENED_SERVICE_CONTRACT).expect("contract fixture value");
        let target =
            toml::to_string(&contract["service_targets"]["myc"]).expect("standalone target");
        let parsed = toml::from_str::<HardenedServiceTarget>(&target).expect("valid target");
        assert_eq!(parsed.service_id().as_str(), "myc");

        for raw in [
            target.replace("admin_contract_version = 1", "admin_contract_version = 99"),
            target.replace("service_id = \"myc\"", "service_id = \"unsupported\""),
            target.replace(
                "\"x86_64-unknown-linux-gnu\", \"aarch64-unknown-linux-gnu\"",
                "\"aarch64-unknown-linux-gnu\"",
            ),
        ] {
            assert!(toml::from_str::<HardenedServiceTarget>(&raw).is_err());
        }
    }

    #[test]
    fn distribution_errors_do_not_expose_contract_values_or_parser_causes() {
        use std::error::Error as _;

        for (raw, secret) in [
            (
                HARDENED_SERVICE_CONTRACT.replace(
                    "schema = \"radroots-runtime-distribution\"",
                    "schema = \"secret-contract-value\"",
                ),
                "secret-contract-value",
            ),
            (
                "credential = 'secret-value'\ninvalid = [".to_owned(),
                "secret-value",
            ),
        ] {
            let error =
                RadrootsRuntimeDistributionResolver::parse_str(&raw).expect_err("invalid contract");
            let rendered = format!("{error} {error:?}");
            assert!(!rendered.contains(secret));
            assert!(error.source().is_none());
        }
    }
}

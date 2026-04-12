#![forbid(unsafe_code)]

pub mod error;
pub mod model;
pub mod resolve;

pub use error::RadrootsRuntimeDistributionError;
pub use model::{
    ArchiveFormat, ArtifactAdapter, ChannelSet, DistributionFamily,
    RadrootsRuntimeDistributionContract, RuntimeDistributionEntry, TargetSet, TargetSpec,
};
pub use resolve::{
    RUNTIME_DISTRIBUTION_SCHEMA, RadrootsRuntimeDistributionResolver, ResolvedRuntimeArtifact,
    RuntimeArtifactRequest,
};

#[cfg(test)]
mod tests {
    use toml::Value;

    use super::{
        RUNTIME_DISTRIBUTION_SCHEMA, RadrootsRuntimeDistributionError,
        RadrootsRuntimeDistributionResolver, RuntimeArtifactRequest,
    };

    const CONTRACT: &str = r#"
schema = "radroots-runtime-distribution"
schema_version = 1
owner_doc = "docs/migration/radroots-modular-runtime-management-bootstrap-rcl.md"
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
id = "community-app-desktop"
distribution_state = "defined"
release_unit = "community-app-desktop"
package_name = "radroots-app-desktop"
binary_name = "radroots-app-desktop"
artifact_adapter = "desktop_bundle"
target_set = "desktop_default"
default_channel = "stable"
human_installable = true

[[runtime]]
id = "community-app-ios"
distribution_state = "external_platform_managed"
release_unit = "community-app-ios"
package_name = "radroots-app-ios"
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
        assert_eq!(
            std::mem::discriminant(&err),
            std::mem::discriminant(&RadrootsRuntimeDistributionError::Parse(String::new()))
        );
    }

    #[test]
    fn new_rejects_unexpected_schema() {
        let mut contract = contract_value();
        contract["schema"] = Value::String("wrong-schema".to_string());

        let raw = toml::to_string(&contract).expect("serialize contract");
        let err = RadrootsRuntimeDistributionResolver::parse_str(&raw)
            .expect_err("unexpected schema should fail");

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::UnexpectedSchema {
                expected: RUNTIME_DISTRIBUTION_SCHEMA,
                found: "wrong-schema".to_string(),
            }
        );
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
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            })
            .expect("resolve cli artifact");

        assert_eq!(artifact.binary_name.as_deref(), Some("radroots"));
        assert_eq!(artifact.target_id, "x86_64-unknown-linux-gnu");
        assert_eq!(artifact.archive_extension, ".tar.gz");
        assert_eq!(
            artifact.artifact_file_name,
            "cli-0.1.0-alpha.1-x86_64-unknown-linux-gnu.tar.gz"
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
                version: "0.1.0-alpha.1",
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
                runtime_id: "community-app-desktop",
                os: "macos",
                arch: "arm64",
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            })
            .expect("resolve desktop artifact");

        assert_eq!(artifact.target_id, "aarch64-apple-darwin");
        assert_eq!(artifact.package_name, "radroots-app-desktop");
    }

    #[test]
    fn rejects_non_installable_mobile_runtime() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        let err = resolver
            .resolve_artifact(&RuntimeArtifactRequest {
                runtime_id: "community-app-ios",
                os: "macos",
                arch: "arm64",
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            })
            .expect_err("mobile runtime should not be installable");

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::RuntimeNotInstallable(
                "community-app-ios".to_string()
            )
        );
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

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::RuntimeNotInstallable("hyf".to_string())
        );
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
                version: "0.1.0-alpha.1",
                channel: Some("candidate"),
            })
            .expect_err("candidate channel should be inactive");

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::InactiveChannel("candidate".to_string())
        );
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
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::UnknownRuntime("missing-runtime".to_string())
        );
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
                version: "0.1.0-alpha.1",
                channel: Some("beta"),
            },
        );

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::UnknownChannel("beta".to_string())
        );
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
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            })
            .expect_err("windows target should be unsupported");

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::UnsupportedPlatform {
                runtime_id: "radrootsd".to_string(),
                os: "windows".to_string(),
                arch: "amd64".to_string(),
            }
        );
    }

    #[test]
    fn rejects_runtime_with_missing_target_set() {
        let mut contract = contract_value();
        let runtime = contract["runtime"]
            .as_array_mut()
            .expect("runtime array")
            .iter_mut()
            .find(|runtime| runtime["id"].as_str() == Some("community-app-ios"))
            .expect("ios runtime");
        runtime["human_installable"] = Value::Boolean(true);

        let resolver = resolver_from_value(contract);
        let err = resolve_error(
            &resolver,
            RuntimeArtifactRequest {
                runtime_id: "community-app-ios",
                os: "ios",
                arch: "arm64",
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::MissingTargetSet("community-app-ios".to_string())
        );
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
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::UnknownArtifactAdapter {
                runtime_id: "cli".to_string(),
                adapter_id: "missing_adapter".to_string(),
            }
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
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::UnsupportedPlatform {
                runtime_id: "cli".to_string(),
                os: "linux".to_string(),
                arch: "amd64".to_string(),
            }
        );
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
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::UnknownTarget {
                runtime_id: "cli".to_string(),
                target_set_id: "cli_default".to_string(),
                target_id: "missing-target".to_string(),
            }
        );
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
                version: "0.1.0-alpha.1",
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
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::UnknownArchiveFormat {
                target_id: "x86_64-unknown-linux-gnu".to_string(),
                archive_format_id: "tar.xz".to_string(),
            }
        );
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
                runtime_id: "community-app-desktop",
                os: "macos",
                arch: "arm64",
                version: "0.1.0-alpha.1",
                channel: Some("stable"),
            },
        );

        assert_eq!(
            err,
            RadrootsRuntimeDistributionError::MissingArchiveFormat {
                runtime_id: "community-app-desktop".to_string(),
                target_id: "aarch64-apple-darwin".to_string(),
            }
        );
    }
}

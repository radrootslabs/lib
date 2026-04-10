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
package_name = "radroots-cli"
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

    #[test]
    fn parse_str_accepts_the_expected_schema() {
        let resolver =
            RadrootsRuntimeDistributionResolver::parse_str(CONTRACT).expect("parse contract");

        assert_eq!(resolver.contract().schema, RUNTIME_DISTRIBUTION_SCHEMA);
        assert_eq!(resolver.contract().runtime.len(), 5);
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
}

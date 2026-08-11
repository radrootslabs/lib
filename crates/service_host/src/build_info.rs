//! Deterministic build identity shared by hardened services.

use core::fmt;

use serde::Serialize;

/// Environment variable containing the consuming service's exact source revision.
pub const SERVICE_REVISION_ENV: &str = "RADROOTS_SERVICE_REVISION";
/// Environment variable containing the exact Radroots Lib revision.
pub const LIB_REVISION_ENV: &str = "RADROOTS_LIB_REVISION";
/// Environment variable containing the exact governed Rust version.
pub const RUST_VERSION_ENV: &str = "RADROOTS_RUST_VERSION";
/// Environment variable containing the exact Rust target triple.
pub const BUILD_TARGET_ENV: &str = "RADROOTS_BUILD_TARGET";

/// Maximum byte length of service versions, Rust versions, targets, and feature profiles.
pub const BUILD_INFO_TEXT_MAX_BYTES: usize = 128;
const DEVELOPMENT_REVISION: &str = "0000000000000000000000000000000000000000";
const DEVELOPMENT_VALUE: &str = "development";

/// Determines whether missing compile-time metadata is an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildMode {
    Development,
    Release,
}

/// Exact versions for every service-host contract family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractVersions {
    config: u32,
    state: u32,
    admin: u32,
    status: u32,
    provider: u32,
}

impl ContractVersions {
    /// Creates a contract-version cohort, rejecting zero as an unpublished identity.
    pub fn new(
        config: u32,
        state: u32,
        admin: u32,
        status: u32,
        provider: u32,
    ) -> Result<Self, BuildInfoError> {
        let versions = Self {
            config,
            state,
            admin,
            status,
            provider,
        };
        if [config, state, admin, status, provider].contains(&0) {
            return Err(BuildInfoError::InvalidValue(
                BuildInfoField::ContractVersion,
            ));
        }
        Ok(versions)
    }

    #[must_use]
    pub const fn config(self) -> u32 {
        self.config
    }

    #[must_use]
    pub const fn state(self) -> u32 {
        self.state
    }

    #[must_use]
    pub const fn admin(self) -> u32 {
        self.admin
    }

    #[must_use]
    pub const fn status(self) -> u32 {
        self.status
    }

    #[must_use]
    pub const fn provider(self) -> u32 {
        self.provider
    }
}

/// Compile-time strings captured by [`crate::compile_time_build_info!`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildInfoEnvironment<'a> {
    pub service_version: Option<&'a str>,
    pub service_commit: Option<&'a str>,
    pub lib_revision: Option<&'a str>,
    pub rust_version: Option<&'a str>,
    pub target: Option<&'a str>,
    pub feature_profile: Option<&'a str>,
    pub contract_versions: ContractVersions,
}

/// A deterministic, timestamp-free service build identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BuildInfo {
    service_version: String,
    service_commit: String,
    lib_revision: String,
    rust_version: String,
    target: String,
    feature_profile: String,
    contract_versions: ContractVersions,
}

/// The exact build-information projection frozen by the service status contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StatusBuildInfo<'a> {
    version: &'a str,
    revision: &'a str,
    toolchain: &'a str,
    contract_versions: ContractVersions,
}

impl BuildInfo {
    /// Validates captured compile-time values and constructs one stable build identity.
    pub fn from_compile_time(
        mode: BuildMode,
        environment: BuildInfoEnvironment<'_>,
    ) -> Result<Self, BuildInfoError> {
        let service_version = required_text(
            mode,
            environment.service_version,
            "CARGO_PKG_VERSION",
            BuildInfoField::ServiceVersion,
        )?;
        let service_commit = required_revision(
            mode,
            environment.service_commit,
            SERVICE_REVISION_ENV,
            BuildInfoField::ServiceCommit,
        )?;
        let lib_revision = required_revision(
            mode,
            environment.lib_revision,
            LIB_REVISION_ENV,
            BuildInfoField::LibRevision,
        )?;
        let rust_version = required_text(
            mode,
            environment.rust_version,
            RUST_VERSION_ENV,
            BuildInfoField::RustVersion,
        )?;
        let target = required_text(
            mode,
            environment.target,
            BUILD_TARGET_ENV,
            BuildInfoField::Target,
        )?;
        let feature_profile = required_text(
            mode,
            environment.feature_profile,
            "feature_profile",
            BuildInfoField::FeatureProfile,
        )?;

        Ok(Self {
            service_version,
            service_commit,
            lib_revision,
            rust_version,
            target,
            feature_profile,
            contract_versions: environment.contract_versions,
        })
    }

    #[must_use]
    pub fn service_version(&self) -> &str {
        &self.service_version
    }

    #[must_use]
    pub fn service_commit(&self) -> &str {
        &self.service_commit
    }

    #[must_use]
    pub fn lib_revision(&self) -> &str {
        &self.lib_revision
    }

    #[must_use]
    pub fn rust_version(&self) -> &str {
        &self.rust_version
    }

    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    #[must_use]
    pub fn feature_profile(&self) -> &str {
        &self.feature_profile
    }

    #[must_use]
    pub const fn contract_versions(&self) -> ContractVersions {
        self.contract_versions
    }

    /// Projects the complete identity into the frozen Myc/RHI status shape.
    ///
    /// The full Lib revision, target, and feature profile remain available on `BuildInfo` and are
    /// bound by source-lock verification; the status contract intentionally exposes only these
    /// four fields.
    #[must_use]
    pub fn status_projection(&self) -> StatusBuildInfo<'_> {
        StatusBuildInfo {
            version: &self.service_version,
            revision: &self.service_commit,
            toolchain: &self.rust_version,
            contract_versions: self.contract_versions,
        }
    }
}

/// Identifies the invalid field without retaining rejected input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildInfoField {
    ServiceVersion,
    ServiceCommit,
    LibRevision,
    RustVersion,
    Target,
    FeatureProfile,
    ContractVersion,
}

impl fmt::Display for BuildInfoField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ServiceVersion => "service_version",
            Self::ServiceCommit => "service_commit",
            Self::LibRevision => "lib_revision",
            Self::RustVersion => "rust_version",
            Self::Target => "target",
            Self::FeatureProfile => "feature_profile",
            Self::ContractVersion => "contract_version",
        })
    }
}

/// Validation failure for deterministic build metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildInfoError {
    MissingVariable(&'static str),
    InvalidValue(BuildInfoField),
}

impl fmt::Display for BuildInfoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVariable(variable) => {
                write!(formatter, "required build variable {variable} is missing")
            }
            Self::InvalidValue(field) => write!(formatter, "build field {field} is invalid"),
        }
    }
}

impl std::error::Error for BuildInfoError {}

fn required_text(
    mode: BuildMode,
    value: Option<&str>,
    variable: &'static str,
    field: BuildInfoField,
) -> Result<String, BuildInfoError> {
    match value {
        Some(value) if valid_text(value) => Ok(value.to_owned()),
        Some(_) => Err(BuildInfoError::InvalidValue(field)),
        None if mode == BuildMode::Development => Ok(DEVELOPMENT_VALUE.to_owned()),
        None => Err(BuildInfoError::MissingVariable(variable)),
    }
}

fn required_revision(
    mode: BuildMode,
    value: Option<&str>,
    variable: &'static str,
    field: BuildInfoField,
) -> Result<String, BuildInfoError> {
    match value {
        Some(value)
            if valid_revision(value)
                && (mode != BuildMode::Release || value != DEVELOPMENT_REVISION) =>
        {
            Ok(value.to_owned())
        }
        Some(_) => Err(BuildInfoError::InvalidValue(field)),
        None if mode == BuildMode::Development => Ok(DEVELOPMENT_REVISION.to_owned()),
        None => Err(BuildInfoError::MissingVariable(variable)),
    }
}

fn valid_text(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= BUILD_INFO_TEXT_MAX_BYTES
        && first.is_ascii_alphanumeric()
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn valid_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVICE_REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
    const LIB_REVISION: &str = "89abcdef0123456789abcdef0123456789abcdef";

    fn contracts() -> ContractVersions {
        ContractVersions::new(1, 2, 3, 4, 5).expect("contract versions")
    }

    fn complete_environment() -> BuildInfoEnvironment<'static> {
        BuildInfoEnvironment {
            service_version: Some("0.1.0-alpha"),
            service_commit: Some(SERVICE_REVISION),
            lib_revision: Some(LIB_REVISION),
            rust_version: Some("1.97.1"),
            target: Some("x86_64-unknown-linux-gnu"),
            feature_profile: Some("service-host"),
            contract_versions: contracts(),
        }
    }

    #[test]
    fn serialization_is_an_exact_timestamp_free_snapshot() {
        let build = BuildInfo::from_compile_time(BuildMode::Release, complete_environment())
            .expect("valid build information");
        let json = serde_json::to_string_pretty(&build).expect("serialize build information");

        assert_eq!(
            json,
            r#"{
  "service_version": "0.1.0-alpha",
  "service_commit": "0123456789abcdef0123456789abcdef01234567",
  "lib_revision": "89abcdef0123456789abcdef0123456789abcdef",
  "rust_version": "1.97.1",
  "target": "x86_64-unknown-linux-gnu",
  "feature_profile": "service-host",
  "contract_versions": {
    "config": 1,
    "state": 2,
    "admin": 3,
    "status": 4,
    "provider": 5
  }
}"#
        );
        assert!(!json.contains("time"));
        assert!(!json.contains("date"));

        let status_json = serde_json::to_string_pretty(&build.status_projection())
            .expect("serialize status build information");
        assert_eq!(
            status_json,
            r#"{
  "version": "0.1.0-alpha",
  "revision": "0123456789abcdef0123456789abcdef01234567",
  "toolchain": "1.97.1",
  "contract_versions": {
    "config": 1,
    "state": 2,
    "admin": 3,
    "status": 4,
    "provider": 5
  }
}"#
        );
    }

    #[test]
    fn release_mode_fails_closed_for_every_missing_build_variable() {
        let complete = complete_environment();
        let cases = [
            (
                BuildInfoEnvironment {
                    service_version: None,
                    ..complete
                },
                "CARGO_PKG_VERSION",
            ),
            (
                BuildInfoEnvironment {
                    service_commit: None,
                    ..complete
                },
                SERVICE_REVISION_ENV,
            ),
            (
                BuildInfoEnvironment {
                    lib_revision: None,
                    ..complete
                },
                LIB_REVISION_ENV,
            ),
            (
                BuildInfoEnvironment {
                    rust_version: None,
                    ..complete
                },
                RUST_VERSION_ENV,
            ),
            (
                BuildInfoEnvironment {
                    target: None,
                    ..complete
                },
                BUILD_TARGET_ENV,
            ),
            (
                BuildInfoEnvironment {
                    feature_profile: None,
                    ..complete
                },
                "feature_profile",
            ),
        ];

        for (environment, variable) in cases {
            assert_eq!(
                BuildInfo::from_compile_time(BuildMode::Release, environment),
                Err(BuildInfoError::MissingVariable(variable))
            );
        }
    }

    #[test]
    fn identical_inputs_are_equal_and_development_fallbacks_are_stable() {
        let left = BuildInfo::from_compile_time(BuildMode::Release, complete_environment())
            .expect("left build information");
        let right = BuildInfo::from_compile_time(BuildMode::Release, complete_environment())
            .expect("right build information");
        assert_eq!(left, right);

        let development = BuildInfo::from_compile_time(
            BuildMode::Development,
            BuildInfoEnvironment {
                service_version: None,
                service_commit: None,
                lib_revision: None,
                rust_version: None,
                target: None,
                feature_profile: None,
                contract_versions: contracts(),
            },
        )
        .expect("development fallbacks");
        assert_eq!(development.service_version(), DEVELOPMENT_VALUE);
        assert_eq!(development.service_commit(), DEVELOPMENT_REVISION);
        assert_eq!(development.lib_revision(), DEVELOPMENT_REVISION);
        assert_eq!(development.rust_version(), DEVELOPMENT_VALUE);
        assert_eq!(development.target(), DEVELOPMENT_VALUE);
        assert_eq!(development.feature_profile(), DEVELOPMENT_VALUE);
    }

    #[test]
    fn malformed_values_and_zero_contract_versions_are_rejected() {
        for revision in [
            "short",
            "ABCDEF0123456789abcdef0123456789abcdef01",
            DEVELOPMENT_REVISION,
        ] {
            let mut environment = complete_environment();
            environment.service_commit = Some(revision);
            assert_eq!(
                BuildInfo::from_compile_time(BuildMode::Release, environment),
                Err(BuildInfoError::InvalidValue(BuildInfoField::ServiceCommit))
            );

            let mut environment = complete_environment();
            environment.lib_revision = Some(revision);
            assert_eq!(
                BuildInfo::from_compile_time(BuildMode::Release, environment),
                Err(BuildInfoError::InvalidValue(BuildInfoField::LibRevision))
            );
        }
        assert_eq!(
            ContractVersions::new(1, 1, 0, 1, 1),
            Err(BuildInfoError::InvalidValue(
                BuildInfoField::ContractVersion
            ))
        );

        for invalid in ["bad version", " rustc", ".1.97.1", "rustc/+nightly"] {
            let mut environment = complete_environment();
            environment.service_version = Some(invalid);
            assert_eq!(
                BuildInfo::from_compile_time(BuildMode::Release, environment),
                Err(BuildInfoError::InvalidValue(BuildInfoField::ServiceVersion))
            );

            let mut environment = complete_environment();
            environment.rust_version = Some(invalid);
            assert_eq!(
                BuildInfo::from_compile_time(BuildMode::Release, environment),
                Err(BuildInfoError::InvalidValue(BuildInfoField::RustVersion))
            );
        }
    }

    #[test]
    fn macro_captures_the_consuming_crate_version() {
        let result = crate::compile_time_build_info!(
            feature_profile: "service-host",
            contract_versions: contracts(),
        );

        if cfg!(debug_assertions) {
            let build = result.expect("debug builds accept missing release variables");
            assert_eq!(build.service_version(), env!("CARGO_PKG_VERSION"));
            assert_eq!(build.contract_versions(), contracts());
        } else {
            assert_eq!(
                result,
                Err(BuildInfoError::MissingVariable(SERVICE_REVISION_ENV))
            );
        }
    }
}

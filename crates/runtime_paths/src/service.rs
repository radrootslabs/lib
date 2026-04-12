use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;

use crate::{
    RadrootsMigrationReport, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
    RadrootsPaths, RadrootsRuntimeNamespace, RadrootsRuntimePathsError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsRuntimePathSelection {
    pub profile: RadrootsPathProfile,
    pub profile_source: String,
    pub repo_local_root: Option<PathBuf>,
    pub repo_local_root_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RadrootsRuntimeSelectionContract {
    pub active_profile: String,
    pub allowed_profiles: Vec<String>,
    pub path_overrides: RadrootsRuntimeSelectionOverrideContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RadrootsRuntimeSelectionOverrideContract {
    pub profile_source: String,
    pub root_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_local_root: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_local_root_source: Option<String>,
    pub subordinate_path_override_source: String,
    pub subordinate_path_override_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RadrootsRuntimePathPolicyContract {
    pub canonical_root_selection: String,
    pub canonical_subordinate_path_override: String,
    pub leaf_path_env_posture: String,
    pub compatibility_leaf_path_keys: Vec<String>,
}

impl RadrootsRuntimePathPolicyContract {
    pub fn new(
        canonical_root_selection: &str,
        canonical_subordinate_path_override: &str,
        leaf_path_env_posture: &str,
        compatibility_leaf_path_keys: &[&str],
    ) -> Self {
        Self {
            canonical_root_selection: canonical_root_selection.to_owned(),
            canonical_subordinate_path_override: canonical_subordinate_path_override.to_owned(),
            leaf_path_env_posture: leaf_path_env_posture.to_owned(),
            compatibility_leaf_path_keys: compatibility_leaf_path_keys
                .iter()
                .map(|entry| (*entry).to_owned())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RadrootsRuntimeMigrationContract {
    pub posture: String,
    pub state: String,
    pub silent_startup_relocation: bool,
    pub compatibility_window: String,
    pub detected_legacy_paths: Vec<RadrootsRuntimeLegacyPathContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RadrootsRuntimeLegacyPathContract {
    pub id: String,
    pub description: String,
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<PathBuf>,
    pub import_hint: String,
}

pub fn runtime_migration_contract(
    report: RadrootsMigrationReport,
) -> RadrootsRuntimeMigrationContract {
    RadrootsRuntimeMigrationContract {
        posture: report.posture.to_owned(),
        state: report.state.to_owned(),
        silent_startup_relocation: report.silent_startup_relocation,
        compatibility_window: report.compatibility_window.to_owned(),
        detected_legacy_paths: report
            .detected_legacy_paths
            .into_iter()
            .map(|path| RadrootsRuntimeLegacyPathContract {
                id: path.id,
                description: path.description,
                path: path.path,
                destination: path.destination,
                import_hint: path.import_hint,
            })
            .collect(),
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RadrootsRuntimePathSelectionError {
    #[error("{env_var} must be valid utf-8 when set")]
    NonUnicodeEnv { env_var: String },

    #[error(
        "{env_var} must be `interactive_user`, `service_host`, or `repo_local`; found `{value}`"
    )]
    InvalidProfileEnv { env_var: String, value: String },

    #[error(
        "profile must be `interactive_user`, `service_host`, `repo_local`, or `mobile_native`; found `{value}`"
    )]
    InvalidProfileValue { value: String },

    #[error("{repo_local_root_env} must be set when {profile_env}=repo_local")]
    MissingRepoLocalRoot {
        profile_env: String,
        repo_local_root_env: String,
    },

    #[error(transparent)]
    Paths(#[from] RadrootsRuntimePathsError),
}

impl RadrootsRuntimePathSelection {
    pub fn caller(profile: RadrootsPathProfile, repo_local_root: Option<PathBuf>) -> Self {
        Self {
            profile,
            profile_source: "caller".to_owned(),
            repo_local_root_source: repo_local_root.as_ref().map(|_| "caller".to_owned()),
            repo_local_root,
        }
    }

    pub fn from_profile_value(
        profile: &str,
        repo_local_root: Option<PathBuf>,
    ) -> Result<Self, RadrootsRuntimePathSelectionError> {
        Ok(Self::caller(parse_profile_value(profile)?, repo_local_root))
    }

    pub fn from_env(
        profile_env: &'static str,
        repo_local_root_env: &'static str,
        default_profile: RadrootsPathProfile,
    ) -> Result<Self, RadrootsRuntimePathSelectionError> {
        let (profile, profile_source) = match std::env::var(profile_env) {
            Ok(value) => (
                parse_profile(profile_env, value.as_str())?,
                format!("process_env:{profile_env}"),
            ),
            Err(std::env::VarError::NotPresent) => (default_profile, "default".to_owned()),
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(RadrootsRuntimePathSelectionError::NonUnicodeEnv {
                    env_var: profile_env.to_owned(),
                });
            }
        };
        let repo_local_root_raw = std::env::var_os(repo_local_root_env);
        let repo_local_root = repo_local_root_raw.as_ref().map(PathBuf::from);
        Ok(Self {
            profile,
            profile_source,
            repo_local_root,
            repo_local_root_source: repo_local_root_raw
                .as_ref()
                .map(|_| format!("process_env:{repo_local_root_env}")),
        })
    }

    pub fn root_source(&self) -> &'static str {
        match self.profile {
            RadrootsPathProfile::InteractiveUser => "host_defaults",
            RadrootsPathProfile::ServiceHost => "service_host_defaults",
            RadrootsPathProfile::RepoLocal => "repo_local_root",
            RadrootsPathProfile::MobileNative => "mobile_native_defaults",
        }
    }

    pub fn overrides(
        &self,
        profile_env: &'static str,
        repo_local_root_env: &'static str,
    ) -> Result<RadrootsPathOverrides, RadrootsRuntimePathSelectionError> {
        self.overrides_with_labels(profile_env, repo_local_root_env)
    }

    pub fn caller_overrides(
        &self,
    ) -> Result<RadrootsPathOverrides, RadrootsRuntimePathSelectionError> {
        self.overrides_with_labels("caller_profile", "caller_repo_local_root")
    }

    pub fn contract(
        &self,
        allowed_profiles: &[&str],
        subordinate_path_override_source: &str,
        subordinate_path_override_keys: &[&str],
    ) -> RadrootsRuntimeSelectionContract {
        RadrootsRuntimeSelectionContract {
            active_profile: self.profile.to_string(),
            allowed_profiles: allowed_profiles
                .iter()
                .map(|entry| (*entry).to_owned())
                .collect(),
            path_overrides: RadrootsRuntimeSelectionOverrideContract {
                profile_source: self.profile_source.clone(),
                root_source: self.root_source().to_owned(),
                repo_local_root: self.repo_local_root.clone(),
                repo_local_root_source: self.repo_local_root_source.clone(),
                subordinate_path_override_source: subordinate_path_override_source.to_owned(),
                subordinate_path_override_keys: subordinate_path_override_keys
                    .iter()
                    .map(|entry| (*entry).to_owned())
                    .collect(),
            },
        }
    }

    fn overrides_with_labels(
        &self,
        profile_label: &str,
        repo_local_root_label: &str,
    ) -> Result<RadrootsPathOverrides, RadrootsRuntimePathSelectionError> {
        match self.profile {
            RadrootsPathProfile::RepoLocal => {
                let Some(repo_local_root) = self.repo_local_root.as_ref() else {
                    return Err(RadrootsRuntimePathSelectionError::MissingRepoLocalRoot {
                        profile_env: profile_label.to_owned(),
                        repo_local_root_env: repo_local_root_label.to_owned(),
                    });
                };
                Ok(RadrootsPathOverrides::repo_local(repo_local_root))
            }
            _ => Ok(RadrootsPathOverrides::default()),
        }
    }

    pub fn resolve_service_roots(
        &self,
        resolver: &RadrootsPathResolver,
        service_id: &str,
        profile_env: &'static str,
        repo_local_root_env: &'static str,
    ) -> Result<RadrootsPaths, RadrootsRuntimePathSelectionError> {
        let namespace = RadrootsRuntimeNamespace::service(service_id)?;
        let overrides = self.overrides(profile_env, repo_local_root_env)?;
        let roots = resolver.resolve(self.profile, &overrides)?;
        Ok(roots.namespaced(&namespace))
    }
}

fn parse_profile(
    env_var: &'static str,
    value: &str,
) -> Result<RadrootsPathProfile, RadrootsRuntimePathSelectionError> {
    match parse_profile_value(value) {
        Ok(profile) => Ok(profile),
        Err(RadrootsRuntimePathSelectionError::InvalidProfileValue { value }) => {
            Err(RadrootsRuntimePathSelectionError::InvalidProfileEnv {
                env_var: env_var.to_owned(),
                value,
            })
        }
        Err(other) => Err(other),
    }
}

fn parse_profile_value(
    value: &str,
) -> Result<RadrootsPathProfile, RadrootsRuntimePathSelectionError> {
    match value {
        "interactive_user" => Ok(RadrootsPathProfile::InteractiveUser),
        "service_host" => Ok(RadrootsPathProfile::ServiceHost),
        "repo_local" => Ok(RadrootsPathProfile::RepoLocal),
        "mobile_native" => Ok(RadrootsPathProfile::MobileNative),
        other => Err(RadrootsRuntimePathSelectionError::InvalidProfileValue {
            value: other.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver, RadrootsPlatform,
    };

    use super::{
        RadrootsRuntimePathPolicyContract, RadrootsRuntimePathSelection,
        RadrootsRuntimePathSelectionError, runtime_migration_contract,
    };
    use crate::{RadrootsLegacyPathDetection, RadrootsMigrationReport};

    #[test]
    fn caller_selection_preserves_profile_and_sources() {
        let selection =
            RadrootsRuntimePathSelection::caller(RadrootsPathProfile::InteractiveUser, None);
        assert_eq!(selection.profile, RadrootsPathProfile::InteractiveUser);
        assert_eq!(selection.profile_source, "caller");
        assert_eq!(selection.repo_local_root, None);
        assert_eq!(selection.repo_local_root_source, None);
        assert_eq!(selection.root_source(), "host_defaults");
    }

    #[test]
    fn caller_selection_marks_repo_local_source() {
        let selection = RadrootsRuntimePathSelection::caller(
            RadrootsPathProfile::RepoLocal,
            Some(PathBuf::from("/repo/.local/radroots")),
        );

        assert_eq!(selection.profile_source, "caller");
        assert_eq!(selection.repo_local_root_source.as_deref(), Some("caller"));
        assert_eq!(selection.root_source(), "repo_local_root");
    }

    #[test]
    fn resolve_service_roots_uses_repo_local_override() {
        let selection = RadrootsRuntimePathSelection::caller(
            RadrootsPathProfile::RepoLocal,
            Some(PathBuf::from("/repo/.local/radroots")),
        );
        let resolver =
            RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default());

        let roots = selection
            .resolve_service_roots(&resolver, "radrootsd", "PROFILE_ENV", "ROOT_ENV")
            .expect("service roots");

        assert_eq!(
            roots.config,
            PathBuf::from("/repo/.local/radroots/config/services/radrootsd")
        );
        assert_eq!(
            roots.data,
            PathBuf::from("/repo/.local/radroots/data/services/radrootsd")
        );
        assert_eq!(
            roots.logs,
            PathBuf::from("/repo/.local/radroots/logs/services/radrootsd")
        );
        assert_eq!(
            roots.run,
            PathBuf::from("/repo/.local/radroots/run/services/radrootsd")
        );
        assert_eq!(
            roots.secrets,
            PathBuf::from("/repo/.local/radroots/secrets/services/radrootsd")
        );
    }

    #[test]
    fn overrides_require_repo_local_root_for_repo_local_profile() {
        let selection = RadrootsRuntimePathSelection::caller(RadrootsPathProfile::RepoLocal, None);
        let err = selection
            .overrides("RADROOTS_TEST_PROFILE", "RADROOTS_TEST_ROOT")
            .expect_err("repo local root");

        assert_eq!(
            err,
            RadrootsRuntimePathSelectionError::MissingRepoLocalRoot {
                profile_env: "RADROOTS_TEST_PROFILE".to_owned(),
                repo_local_root_env: "RADROOTS_TEST_ROOT".to_owned(),
            }
        );
    }

    #[test]
    fn profile_value_selection_accepts_mobile_native() {
        let selection = RadrootsRuntimePathSelection::from_profile_value("mobile_native", None)
            .expect("mobile native profile");

        assert_eq!(selection.profile, RadrootsPathProfile::MobileNative);
        assert_eq!(selection.profile_source, "caller");
    }

    #[test]
    fn contract_captures_selection_sources() {
        let selection = RadrootsRuntimePathSelection::caller(
            RadrootsPathProfile::RepoLocal,
            Some(PathBuf::from("/repo/.local/radroots")),
        );

        let contract = selection.contract(
            &["interactive_user", "repo_local"],
            "config_artifact",
            &["config.service.logs_dir"],
        );

        assert_eq!(contract.active_profile, "repo_local");
        assert_eq!(
            contract.allowed_profiles,
            vec!["interactive_user".to_owned(), "repo_local".to_owned()]
        );
        assert_eq!(contract.path_overrides.profile_source, "caller");
        assert_eq!(
            contract.path_overrides.repo_local_root,
            Some(PathBuf::from("/repo/.local/radroots"))
        );
    }

    #[test]
    fn path_policy_contract_preserves_policy_strings() {
        let contract = RadrootsRuntimePathPolicyContract::new(
            "profile_root_env_or_repo_wrapper",
            "config_artifact",
            "compatibility_break_glass",
            &["MYC_PATHS_STATE_DIR"],
        );

        assert_eq!(
            contract.canonical_root_selection,
            "profile_root_env_or_repo_wrapper"
        );
        assert_eq!(
            contract.compatibility_leaf_path_keys,
            vec!["MYC_PATHS_STATE_DIR".to_owned()]
        );
    }

    #[test]
    fn runtime_migration_contract_maps_detected_paths() {
        let report = RadrootsMigrationReport {
            posture: "explicit_operator_import_required",
            state: "legacy_state_detected",
            silent_startup_relocation: false,
            compatibility_window: "detect_and_report_only",
            detected_legacy_paths: vec![RadrootsLegacyPathDetection {
                id: "legacy_path".to_owned(),
                description: "legacy path".to_owned(),
                path: PathBuf::from("/tmp/legacy"),
                destination: Some(PathBuf::from("/tmp/new")),
                import_hint: "copy it manually".to_owned(),
            }],
        };

        let contract = runtime_migration_contract(report);

        assert_eq!(contract.posture, "explicit_operator_import_required");
        assert_eq!(contract.detected_legacy_paths.len(), 1);
        assert_eq!(contract.detected_legacy_paths[0].id, "legacy_path");
    }
}

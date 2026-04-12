use std::path::PathBuf;

use thiserror::Error;

use crate::{
    RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver, RadrootsPaths,
    RadrootsRuntimeNamespace, RadrootsRuntimePathsError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsRuntimePathSelection {
    pub profile: RadrootsPathProfile,
    pub profile_source: String,
    pub repo_local_root: Option<PathBuf>,
    pub repo_local_root_source: Option<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RadrootsRuntimePathSelectionError {
    #[error("{env_var} must be valid utf-8 when set")]
    NonUnicodeEnv { env_var: String },

    #[error(
        "{env_var} must be `interactive_user`, `service_host`, or `repo_local`; found `{value}`"
    )]
    InvalidProfileEnv { env_var: String, value: String },

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
        match self.profile {
            RadrootsPathProfile::RepoLocal => {
                let Some(repo_local_root) = self.repo_local_root.as_ref() else {
                    return Err(RadrootsRuntimePathSelectionError::MissingRepoLocalRoot {
                        profile_env: profile_env.to_owned(),
                        repo_local_root_env: repo_local_root_env.to_owned(),
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
    match value {
        "interactive_user" => Ok(RadrootsPathProfile::InteractiveUser),
        "service_host" => Ok(RadrootsPathProfile::ServiceHost),
        "repo_local" => Ok(RadrootsPathProfile::RepoLocal),
        other => Err(RadrootsRuntimePathSelectionError::InvalidProfileEnv {
            env_var: env_var.to_owned(),
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

    use super::{RadrootsRuntimePathSelection, RadrootsRuntimePathSelectionError};

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
}

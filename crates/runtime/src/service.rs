use serde::{Deserialize, Serialize};
#[cfg(any(feature = "cli", test))]
use std::path::PathBuf;

#[cfg(feature = "cli")]
use clap::{ArgAction, Args, ValueHint};
use radroots_runtime_paths::{
    DEFAULT_SERVICE_IDENTITY_FILE_NAME, RadrootsBootstrapPaths, RadrootsPathOverrides,
    RadrootsPathProfile, RadrootsPathResolver, RadrootsRuntimeNamespace, RadrootsRuntimePathsError,
    default_namespaced_bootstrap_paths,
};

pub const DEFAULT_SERVICE_IDENTITY_PATH: &str = DEFAULT_SERVICE_IDENTITY_FILE_NAME;

pub fn service_bootstrap_paths_for(
    resolver: &RadrootsPathResolver,
    profile: RadrootsPathProfile,
    overrides: &RadrootsPathOverrides,
    runtime_id: &str,
) -> Result<RadrootsBootstrapPaths, RadrootsRuntimePathsError> {
    let namespace = RadrootsRuntimeNamespace::service(runtime_id)?;
    default_namespaced_bootstrap_paths(
        resolver,
        profile,
        overrides,
        &namespace,
        DEFAULT_SERVICE_IDENTITY_PATH,
    )
}

#[cfg(feature = "cli")]
#[derive(Args, Debug, Clone)]
pub struct RadrootsServiceCliArgs {
    #[arg(
        long,
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        help = "Path to the daemon configuration file; no implicit cwd-rooted default is used"
    )]
    pub config: Option<PathBuf>,

    #[arg(
        long,
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        help = "Path to the daemon encrypted identity envelope; callers may resolve a canonical namespaced default ending in identity.secret.json with a sibling .key wrapping key file"
    )]
    pub identity: Option<PathBuf>,

    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Allow generating a new encrypted identity envelope when the configured path is missing; if not set and the identity is absent, the daemon will fail"
    )]
    pub allow_generate_identity: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RadrootsNostrServiceConfig {
    pub logs_dir: String,
    #[serde(default)]
    pub relays: Vec<String>,
    #[serde(default)]
    pub nip89_identifier: Option<String>,
    #[serde(default)]
    pub nip89_extra_tags: Vec<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use radroots_runtime_paths::{
        RadrootsHostEnvironment, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform,
    };

    use super::{RadrootsNostrServiceConfig, service_bootstrap_paths_for};

    #[test]
    fn service_config_defaults_optional_fields() {
        let cfg: RadrootsNostrServiceConfig = toml::from_str(
            r#"
logs_dir = "logs"
"#,
        )
        .expect("service config should parse");

        assert_eq!(cfg.logs_dir, "logs");
        assert!(cfg.relays.is_empty());
        assert_eq!(cfg.nip89_identifier, None);
        assert!(cfg.nip89_extra_tags.is_empty());
    }

    #[test]
    fn service_bootstrap_paths_follow_runtime_paths_contract() {
        let resolver = RadrootsPathResolver::new(
            RadrootsPlatform::Linux,
            RadrootsHostEnvironment {
                home_dir: Some(PathBuf::from("/home/treesap")),
                ..RadrootsHostEnvironment::default()
            },
        );

        let paths = service_bootstrap_paths_for(
            &resolver,
            RadrootsPathProfile::InteractiveUser,
            &RadrootsPathOverrides::default(),
            "radrootsd",
        )
        .expect("service bootstrap paths should resolve");

        assert_eq!(
            paths.config_path,
            PathBuf::from("/home/treesap/.radroots/config/services/radrootsd/config.toml")
        );
        assert_eq!(
            paths.logs_dir,
            PathBuf::from("/home/treesap/.radroots/logs/services/radrootsd")
        );
        assert_eq!(
            paths.identity_path,
            PathBuf::from(
                "/home/treesap/.radroots/secrets/services/radrootsd/identity.secret.json"
            )
        );
    }
}

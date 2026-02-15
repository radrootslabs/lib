use serde::{Deserialize, Serialize};

#[cfg(feature = "cli")]
use clap::{ArgAction, Args, ValueHint};
#[cfg(feature = "cli")]
use std::path::PathBuf;

#[cfg(feature = "cli")]
#[derive(Args, Debug, Clone)]
pub struct RadrootsServiceCliArgs {
    #[arg(
        long,
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        default_value = "config.toml",
        help = "Path to the daemon configuration file (defaults to config.toml)"
    )]
    pub config: PathBuf,

    #[arg(
        long,
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        help = "Path to the daemon identity file (json, txt, or raw 32-byte key; defaults to identity.json)"
    )]
    pub identity: Option<PathBuf>,

    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Allow generating a new identity file if missing; if not set and identity file is absent, the daemon will fail"
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
    use super::RadrootsNostrServiceConfig;

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
}

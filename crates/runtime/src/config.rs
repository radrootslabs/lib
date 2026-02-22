use config::{Config, Environment, File, Map, Value};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

use crate::error::RuntimeConfigError;

pub fn load_required_file<T>(path: impl AsRef<Path>) -> Result<T, RuntimeConfigError>
where
    T: DeserializeOwned,
{
    let p: &Path = path.as_ref();

    let cfg = Config::builder()
        .add_source(File::from(p).required(true))
        .build()
        .map_err(|source| RuntimeConfigError::Load {
            path: p.to_path_buf(),
            source,
        })?;

    try_deser::<T>(cfg, p)
}

pub fn load_required_file_with_env<T>(
    path: impl AsRef<Path>,
    env_prefix: &str,
) -> Result<T, RuntimeConfigError>
where
    T: DeserializeOwned,
{
    let p: &Path = path.as_ref();

    let cfg = Config::builder()
        .add_source(File::from(p).required(true))
        .add_source(Environment::with_prefix(env_prefix).separator("__"))
        .build()
        .map_err(|source| RuntimeConfigError::Load {
            path: p.to_path_buf(),
            source,
        })?;

    try_deser::<T>(cfg, p)
}

pub fn load_required_file_with_env_and_overrides<T>(
    path: impl AsRef<Path>,
    env_prefix: Option<&str>,
    overrides: Option<Map<String, Value>>,
) -> Result<T, RuntimeConfigError>
where
    T: DeserializeOwned,
{
    let p: &Path = path.as_ref();
    let mut builder = Config::builder().add_source(File::from(p).required(true));

    if let Some(prefix) = env_prefix {
        builder = builder.add_source(Environment::with_prefix(prefix).separator("__"));
    }

    if let Some(ovr) = overrides {
        for (k, v) in ovr {
            builder = builder
                .set_override(k, v)
                .map_err(|source| RuntimeConfigError::Load {
                    path: p.to_path_buf(),
                    source,
                })?;
        }
    }

    let cfg = builder.build().map_err(|source| RuntimeConfigError::Load {
        path: p.to_path_buf(),
        source,
    })?;

    try_deser::<T>(cfg, p)
}

fn try_deser<T>(cfg: Config, p: &Path) -> Result<T, RuntimeConfigError>
where
    T: DeserializeOwned,
{
    cfg.try_deserialize::<T>()
        .map_err(|source| RuntimeConfigError::Load {
            path: PathBuf::from(p),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::{
        load_required_file, load_required_file_with_env, load_required_file_with_env_and_overrides,
    };
    use config::{Map, Value};
    use serde::Deserialize;
    use tempfile::tempdir;

    use crate::error::RuntimeConfigError;

    #[derive(Debug, Deserialize, PartialEq)]
    struct RuntimeCfg {
        logs_dir: String,
        enabled: bool,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct NumberCfg {
        count: u32,
    }

    fn write_config(contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("runtime.toml");
        std::fs::write(&path, contents).expect("write config");
        (dir, path)
    }

    #[test]
    fn load_required_file_reads_toml() {
        let (_dir, path) = write_config(
            r#"
logs_dir = "logs"
enabled = false
"#,
        );

        let cfg: RuntimeCfg = load_required_file(&path).expect("load config");
        assert_eq!(
            cfg,
            RuntimeCfg {
                logs_dir: "logs".to_string(),
                enabled: false,
            }
        );
    }

    #[test]
    fn load_required_file_reports_missing_path() {
        let path = std::path::PathBuf::from("/tmp/radroots-runtime-config-does-not-exist.toml");
        let err = load_required_file::<RuntimeCfg>(&path).expect_err("missing config should fail");
        match err {
            RuntimeConfigError::Load { path: p, .. } => assert_eq!(p, path),
        }
    }

    #[test]
    fn load_required_file_reports_missing_path_for_number_cfg_owned_path() {
        let path = std::path::PathBuf::from("/tmp/radroots-runtime-config-missing-number.toml");
        let err =
            load_required_file::<NumberCfg>(path.clone()).expect_err("missing config should fail");
        match err {
            RuntimeConfigError::Load { path: p, .. } => assert_eq!(p, path),
        }
    }

    #[test]
    fn load_required_file_reports_deserialize_failure() {
        let (_dir, path) = write_config(
            r#"
count = "not-a-number"
"#,
        );

        let err =
            load_required_file::<NumberCfg>(path.clone()).expect_err("invalid value should fail");
        match err {
            RuntimeConfigError::Load { path: p, .. } => assert_eq!(p, path),
        }
    }

    #[test]
    fn load_required_file_with_env_path_executes_env_source() {
        let (_dir, path) = write_config(
            r#"
logs_dir = "logs"
enabled = true
"#,
        );

        let cfg: RuntimeCfg = load_required_file_with_env(path.clone(), "RADROOTS_RUNTIME_TEST")
            .expect("load config with env source");
        assert_eq!(cfg.logs_dir, "logs");
        assert!(cfg.enabled);
    }

    #[test]
    fn load_required_file_with_env_reports_missing_path() {
        let path =
            std::path::PathBuf::from("/tmp/radroots-runtime-config-does-not-exist-with-env.toml");
        let err = load_required_file_with_env::<RuntimeCfg>(path.clone(), "RADROOTS_RUNTIME_TEST")
            .expect_err("missing config should fail");
        match err {
            RuntimeConfigError::Load { path: p, .. } => assert_eq!(p, path),
        }
    }

    #[test]
    fn load_required_file_with_env_and_overrides_applies_overrides() {
        let (_dir, path) = write_config(
            r#"
logs_dir = "logs"
enabled = false
"#,
        );

        let mut overrides = Map::new();
        overrides.insert("enabled".to_string(), Value::from(true));
        let cfg: RuntimeCfg = load_required_file_with_env_and_overrides(
            path.clone(),
            Some("RADROOTS_RUNTIME_TEST"),
            Some(overrides),
        )
        .expect("load config with overrides");

        assert!(cfg.enabled);
        assert_eq!(cfg.logs_dir, "logs");
    }

    #[test]
    fn load_required_file_with_env_and_overrides_handles_none_overrides() {
        let (_dir, path) = write_config(
            r#"
logs_dir = "logs"
enabled = true
"#,
        );

        let cfg: RuntimeCfg = load_required_file_with_env_and_overrides(path.clone(), None, None)
            .expect("load config without overrides");
        assert_eq!(cfg.logs_dir, "logs");
        assert!(cfg.enabled);
    }

    #[test]
    fn load_required_file_with_env_and_overrides_reports_override_error() {
        let (_dir, path) = write_config(
            r#"
logs_dir = "logs"
enabled = false
"#,
        );

        let mut overrides = Map::new();
        overrides.insert(String::new(), Value::from(true));
        let err = load_required_file_with_env_and_overrides::<RuntimeCfg>(
            path.clone(),
            None,
            Some(overrides),
        )
        .expect_err("invalid override should fail");

        match err {
            RuntimeConfigError::Load { path: p, .. } => assert_eq!(p, path),
        }
    }

    #[test]
    fn load_required_file_with_env_and_overrides_reports_build_error() {
        let path = std::path::PathBuf::from(
            "/tmp/radroots-runtime-config-does-not-exist-with-overrides.toml",
        );
        let err = load_required_file_with_env_and_overrides::<RuntimeCfg>(path.clone(), None, None)
            .expect_err("missing config should fail");

        match err {
            RuntimeConfigError::Load { path: p, .. } => assert_eq!(p, path),
        }
    }

    #[test]
    fn load_required_file_with_env_and_overrides_reports_runtime_cfg_deserialize_error() {
        let (_dir, path) = write_config(
            r#"
logs_dir = "logs"
enabled = "invalid"
"#,
        );

        let err = load_required_file_with_env_and_overrides::<RuntimeCfg>(path.clone(), None, None)
            .expect_err("deserialize should fail");
        match err {
            RuntimeConfigError::Load { path: p, .. } => assert_eq!(p, path),
        }
    }
}

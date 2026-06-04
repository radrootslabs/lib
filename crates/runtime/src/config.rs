use config::{Config, Environment, File, Map, Value};
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::error::RuntimeConfigError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSourceKind {
    ProcessEnv,
    EnvFile,
    Toml,
    Caller,
    Default,
}

impl ConfigSourceKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProcessEnv => "process_env",
            Self::EnvFile => "env_file",
            Self::Toml => "toml",
            Self::Caller => "caller",
            Self::Default => "default",
        }
    }

    #[must_use]
    pub fn key_label(self, key: &str) -> String {
        format!("{}:{key}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigKeySpec {
    pub name: &'static str,
}

impl ConfigKeySpec {
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrictEnvFileValues {
    values: BTreeMap<String, String>,
}

impl StrictEnvFileValues {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    #[must_use]
    pub fn into_inner(self) -> BTreeMap<String, String> {
        self.values
    }
}

#[derive(Debug, Error)]
pub enum RuntimeEnvFileError {
    #[error("failed to read env file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid env file {path} line {line}: expected KEY=VALUE")]
    InvalidLine { path: PathBuf, line: usize },

    #[error("invalid env file {path} line {line}: empty key")]
    EmptyKey { path: PathBuf, line: usize },

    #[error("invalid env file {path} line {line}: unknown environment variable `{key}`")]
    UnknownKey {
        path: PathBuf,
        line: usize,
        key: String,
    },

    #[error(
        "invalid env file {path} line {line}: duplicate environment variable `{key}` first set on line {first_line}"
    )]
    DuplicateKey {
        path: PathBuf,
        line: usize,
        key: String,
        first_line: usize,
    },

    #[error("invalid env file {path} line {line}: unterminated quoted environment value")]
    UnterminatedQuotedValue { path: PathBuf, line: usize },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RuntimeConfigValueError {
    #[error("{key} must be a boolean value, got `{value}`")]
    Bool { key: String, value: String },

    #[error("{key} must be an unsigned integer, got `{value}`")]
    U64 { key: String, value: String },

    #[error("{key} must be a non-negative integer, got `{value}`")]
    Usize { key: String, value: String },
}

pub fn load_strict_env_file(
    path: impl AsRef<Path>,
    supported_keys: &[&str],
) -> Result<StrictEnvFileValues, RuntimeEnvFileError> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| RuntimeEnvFileError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_strict_env_file(raw.as_str(), path, supported_keys)
}

pub fn load_strict_env_file_with_specs(
    path: impl AsRef<Path>,
    supported_keys: &[ConfigKeySpec],
) -> Result<StrictEnvFileValues, RuntimeEnvFileError> {
    let keys: Vec<&str> = supported_keys.iter().map(|spec| spec.name).collect();
    load_strict_env_file(path, keys.as_slice())
}

pub fn parse_strict_env_file(
    raw: &str,
    path: impl AsRef<Path>,
    supported_keys: &[&str],
) -> Result<StrictEnvFileValues, RuntimeEnvFileError> {
    let path = path.as_ref();
    let supported_keys: BTreeSet<&str> = supported_keys.iter().copied().collect();
    let mut values = BTreeMap::new();
    let mut first_lines = BTreeMap::new();

    for (index, line) in raw.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            return Err(RuntimeEnvFileError::InvalidLine {
                path: path.to_path_buf(),
                line: line_number,
            });
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(RuntimeEnvFileError::EmptyKey {
                path: path.to_path_buf(),
                line: line_number,
            });
        }
        if !supported_keys.contains(key) {
            return Err(RuntimeEnvFileError::UnknownKey {
                path: path.to_path_buf(),
                line: line_number,
                key: key.to_owned(),
            });
        }
        if let Some(first_line) = first_lines.get(key) {
            return Err(RuntimeEnvFileError::DuplicateKey {
                path: path.to_path_buf(),
                line: line_number,
                key: key.to_owned(),
                first_line: *first_line,
            });
        }
        let value = normalize_env_value(value.trim(), path, line_number)?;
        first_lines.insert(key.to_owned(), line_number);
        values.insert(key.to_owned(), value);
    }

    Ok(StrictEnvFileValues { values })
}

pub fn parse_strict_env_file_with_specs(
    raw: &str,
    path: impl AsRef<Path>,
    supported_keys: &[ConfigKeySpec],
) -> Result<StrictEnvFileValues, RuntimeEnvFileError> {
    let keys: Vec<&str> = supported_keys.iter().map(|spec| spec.name).collect();
    parse_strict_env_file(raw, path, keys.as_slice())
}

pub fn parse_bool_value(key: &str, value: &str) -> Result<bool, RuntimeConfigValueError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(RuntimeConfigValueError::Bool {
            key: key.to_owned(),
            value: other.to_owned(),
        }),
    }
}

pub fn parse_u64_value(key: &str, value: &str) -> Result<u64, RuntimeConfigValueError> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| RuntimeConfigValueError::U64 {
            key: key.to_owned(),
            value: value.trim().to_owned(),
        })
}

pub fn parse_usize_value(key: &str, value: &str) -> Result<usize, RuntimeConfigValueError> {
    value
        .trim()
        .parse::<usize>()
        .map_err(|_| RuntimeConfigValueError::Usize {
            key: key.to_owned(),
            value: value.trim().to_owned(),
        })
}

#[must_use]
pub fn parse_optional_string_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[must_use]
pub fn parse_string_list_value(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

#[must_use]
pub fn parse_optional_path_value(value: &str) -> Option<PathBuf> {
    parse_optional_string_value(value).map(PathBuf::from)
}

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

fn normalize_env_value(
    value: &str,
    path: &Path,
    line_number: usize,
) -> Result<String, RuntimeEnvFileError> {
    if value.starts_with('"') || value.starts_with('\'') {
        let quote = value.chars().next().expect("quoted env value prefix");
        if !value.ends_with(quote) || value.len() < 2 {
            return Err(RuntimeEnvFileError::UnterminatedQuotedValue {
                path: path.to_path_buf(),
                line: line_number,
            });
        }
        return Ok(value[1..value.len() - 1].to_owned());
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        ConfigKeySpec, ConfigSourceKind, RuntimeConfigValueError, RuntimeEnvFileError,
        load_required_file, load_required_file_with_env, load_required_file_with_env_and_overrides,
        load_strict_env_file, load_strict_env_file_with_specs, parse_bool_value,
        parse_optional_path_value, parse_optional_string_value, parse_strict_env_file,
        parse_strict_env_file_with_specs, parse_string_list_value, parse_u64_value,
        parse_usize_value,
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
    fn config_source_kind_formats_labels() {
        assert_eq!(ConfigSourceKind::ProcessEnv.as_str(), "process_env");
        assert_eq!(
            ConfigSourceKind::EnvFile.key_label("RADROOTS_CLI_OUTPUT_FORMAT"),
            "env_file:RADROOTS_CLI_OUTPUT_FORMAT"
        );
    }

    #[test]
    fn strict_env_file_parses_supported_keys() {
        let values = parse_strict_env_file(
            r#"
# ignored
RADROOTS_CLI_OUTPUT_FORMAT = "json"
RADROOTS_CLI_HYF_ENABLED='true'
"#,
            "runtime.env",
            &["RADROOTS_CLI_OUTPUT_FORMAT", "RADROOTS_CLI_HYF_ENABLED"],
        )
        .expect("parse env file");

        assert_eq!(values.get("RADROOTS_CLI_OUTPUT_FORMAT"), Some("json"));
        assert_eq!(values.get("RADROOTS_CLI_HYF_ENABLED"), Some("true"));
        assert_eq!(
            values.iter().collect::<Vec<_>>(),
            vec![
                ("RADROOTS_CLI_HYF_ENABLED", "true"),
                ("RADROOTS_CLI_OUTPUT_FORMAT", "json")
            ]
        );
    }

    #[test]
    fn strict_env_file_rejects_unknown_keys() {
        let err = parse_strict_env_file("RADROOTS_OUTPUT=json", "runtime.env", &[])
            .expect_err("unknown key should fail");

        match err {
            RuntimeEnvFileError::UnknownKey { line, key, .. } => {
                assert_eq!(line, 1);
                assert_eq!(key, "RADROOTS_OUTPUT");
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn strict_env_file_rejects_duplicate_keys() {
        let err = parse_strict_env_file(
            r#"
RADROOTS_CLI_OUTPUT_FORMAT=json
RADROOTS_CLI_OUTPUT_FORMAT=ndjson
"#,
            "runtime.env",
            &["RADROOTS_CLI_OUTPUT_FORMAT"],
        )
        .expect_err("duplicate key should fail");

        match err {
            RuntimeEnvFileError::DuplicateKey {
                line, first_line, ..
            } => {
                assert_eq!(line, 3);
                assert_eq!(first_line, 2);
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn strict_env_file_rejects_unterminated_quotes() {
        let err = parse_strict_env_file(
            "RADROOTS_CLI_OUTPUT_FORMAT=\"json",
            "runtime.env",
            &["RADROOTS_CLI_OUTPUT_FORMAT"],
        )
        .expect_err("unterminated quote should fail");

        assert!(matches!(
            err,
            RuntimeEnvFileError::UnterminatedQuotedValue { line: 1, .. }
        ));
    }

    #[test]
    fn strict_env_file_supports_key_specs_and_file_loading() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("runtime.env");
        std::fs::write(&path, "RHI_PATHS_PROFILE=repo_local").expect("write env file");

        let values =
            load_strict_env_file_with_specs(&path, &[ConfigKeySpec::new("RHI_PATHS_PROFILE")])
                .expect("load env file with specs");

        assert_eq!(values.get("RHI_PATHS_PROFILE"), Some("repo_local"));

        let values =
            load_strict_env_file(&path, &["RHI_PATHS_PROFILE"]).expect("load env file with keys");
        assert_eq!(values.into_inner().len(), 1);

        let values = parse_strict_env_file_with_specs(
            "RHI_PATHS_PROFILE=service_host",
            "runtime.env",
            &[ConfigKeySpec::new("RHI_PATHS_PROFILE")],
        )
        .expect("parse env file with specs");
        assert_eq!(values.get("RHI_PATHS_PROFILE"), Some("service_host"));
    }

    #[test]
    fn config_value_parsers_handle_shared_scalars() {
        assert_eq!(parse_bool_value("KEY", "yes"), Ok(true));
        assert_eq!(parse_bool_value("KEY", "off"), Ok(false));
        assert_eq!(parse_u64_value("KEY_MS", "250"), Ok(250));
        assert_eq!(parse_usize_value("KEY_COUNT", "8"), Ok(8));
        assert_eq!(parse_optional_string_value("  "), None);
        assert_eq!(
            parse_optional_path_value(" state ").unwrap(),
            std::path::PathBuf::from("state")
        );
        assert_eq!(
            parse_string_list_value("a, b,,c"),
            vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
        );
    }

    #[test]
    fn config_value_parsers_report_keyed_errors() {
        assert_eq!(
            parse_bool_value("KEY", "maybe"),
            Err(RuntimeConfigValueError::Bool {
                key: "KEY".to_owned(),
                value: "maybe".to_owned(),
            })
        );
        assert_eq!(
            parse_u64_value("KEY_MS", "soon"),
            Err(RuntimeConfigValueError::U64 {
                key: "KEY_MS".to_owned(),
                value: "soon".to_owned(),
            })
        );
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
        let path = std::path::PathBuf::from("/tmp/radroots_runtime-config-does-not-exist.toml");
        let err = load_required_file::<RuntimeCfg>(&path).expect_err("missing config should fail");
        match err {
            RuntimeConfigError::Load { path: p, .. } => assert_eq!(p, path),
        }
    }

    #[test]
    fn load_required_file_reports_missing_path_for_number_cfg_owned_path() {
        let path = std::path::PathBuf::from("/tmp/radroots_runtime-config-missing-number.toml");
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
            std::path::PathBuf::from("/tmp/radroots_runtime-config-does-not-exist-with-env.toml");
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
            "/tmp/radroots_runtime-config-does-not-exist-with-overrides.toml",
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

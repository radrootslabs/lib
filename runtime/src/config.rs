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

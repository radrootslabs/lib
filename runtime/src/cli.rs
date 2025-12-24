#![cfg(feature = "cli")]

use clap::Parser;
use config::{Map, Value};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

use crate::error::{RuntimeCliError, RuntimeError};

pub fn parse_and_load_path<Args, C, FP>(path_of: FP) -> Result<(Args, C), RuntimeError>
where
    Args: Parser,
    C: DeserializeOwned,
    FP: Fn(&Args) -> Option<&Path>,
{
    let args = Args::try_parse().map_err(RuntimeCliError::from)?;
    let path = resolve_path(path_of(&args));
    let cfg = crate::config::load_required_file::<C>(&path)?;
    Ok((args, cfg))
}

pub fn parse_and_load_path_with_env_overrides<Args, C, FP, FO>(
    path_of: FP,
    env_prefix: Option<&str>,
    overrides_of: FO,
) -> Result<(Args, C), RuntimeError>
where
    Args: Parser,
    C: DeserializeOwned,
    FP: Fn(&Args) -> Option<&Path>,
    FO: Fn(&Args) -> Option<Map<String, Value>>,
{
    let args = Args::try_parse().map_err(RuntimeCliError::from)?;
    let path = resolve_path(path_of(&args));
    let cfg = crate::config::load_required_file_with_env_and_overrides::<C>(
        &path,
        env_prefix,
        overrides_of(&args),
    )?;
    Ok((args, cfg))
}

pub fn parse_and_load_path_with_init<Args, C, FP, FL>(
    path_of: FP,
    logs_dir_of: FL,
    default_level: Option<&str>,
) -> Result<(Args, C), RuntimeError>
where
    Args: Parser,
    C: DeserializeOwned,
    FP: Fn(&Args) -> Option<&Path>,
    FL: Fn(&C) -> &str,
{
    let (args, cfg) = parse_and_load_path::<Args, C, FP>(path_of)?;
    crate::tracing::init_with(logs_dir_of(&cfg), default_level)?;
    Ok((args, cfg))
}

pub fn parse_and_load_path_with_env_overrides_and_init<Args, C, FP, FO, FL>(
    path_of: FP,
    env_prefix: Option<&str>,
    overrides_of: FO,
    logs_dir_of: FL,
    default_level: Option<&str>,
) -> Result<(Args, C), RuntimeError>
where
    Args: Parser,
    C: DeserializeOwned,
    FP: Fn(&Args) -> Option<&Path>,
    FO: Fn(&Args) -> Option<Map<String, Value>>,
    FL: Fn(&C) -> &str,
{
    let (args, cfg) =
        parse_and_load_path_with_env_overrides::<Args, C, FP, FO>(path_of, env_prefix, overrides_of)?;
    crate::tracing::init_with(logs_dir_of(&cfg), default_level)?;
    Ok((args, cfg))
}

#[inline]
fn resolve_path(p: Option<&Path>) -> PathBuf {
    p.unwrap_or_else(|| Path::new("config.toml")).to_path_buf()
}

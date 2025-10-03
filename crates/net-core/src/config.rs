use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetConfig {}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum KeyFormat {
    Json,
    Nsec,
    Hex,
    Bin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPersistenceConfig {
    pub path: Option<PathBuf>,
    pub format: KeyFormat,
    pub no_overwrite: bool,
}

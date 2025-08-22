use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Error)]
pub enum RuntimeJsonError {
    #[error("JSON file does not exist at {0}")]
    NotFound(PathBuf),

    #[error("Failed to open JSON file at {0}: {1}")]
    FileOpen(PathBuf, #[source] io::Error),

    #[error("Failed to parse JSON at {0}: {1}")]
    FileParse(PathBuf, #[source] serde_json::Error),

    #[error("Failed to serialize JSON: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("I/O error during JSON write: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to persist JSON file to disk: {0}")]
    Persist(#[from] tempfile::PersistError),
}

#[derive(Debug, Clone)]
pub struct JsonWriteOptions {
    pub pretty: bool,
    pub mode_unix: Option<u32>,
}

impl Default for JsonWriteOptions {
    fn default() -> Self {
        Self {
            pretty: false,
            mode_unix: Some(0o600),
        }
    }
}

#[derive(Debug, Clone)]
pub struct JsonFile<T> {
    pub value: T,
    path: PathBuf,
    options: JsonWriteOptions,
}

impl<T> JsonFile<T> {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn set_options(&mut self, options: JsonWriteOptions) {
        self.options = options;
    }
}

impl<T> JsonFile<T>
where
    T: Serialize + DeserializeOwned,
{
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RuntimeJsonError> {
        let p = path.as_ref().to_path_buf();
        if !p.exists() {
            return Err(RuntimeJsonError::NotFound(p));
        }
        let file = std::fs::File::open(&p).map_err(|e| RuntimeJsonError::FileOpen(p.clone(), e))?;
        let reader = std::io::BufReader::new(file);
        let value = serde_json::from_reader(reader)
            .map_err(|e| RuntimeJsonError::FileParse(p.clone(), e))?;
        Ok(Self {
            value,
            path: p,
            options: JsonWriteOptions::default(),
        })
    }

    pub fn load_or_create_with<F>(path: impl AsRef<Path>, init: F) -> Result<Self, RuntimeJsonError>
    where
        F: FnOnce() -> T,
    {
        let p = path.as_ref().to_path_buf();
        if p.exists() {
            return Self::load(p);
        }
        let s = Self {
            value: init(),
            path: p,
            options: JsonWriteOptions::default(),
        };
        s.save()?;
        Ok(s)
    }

    pub fn save(&self) -> Result<(), RuntimeJsonError> {
        self.save_as(&self.path)
    }

    pub fn save_as(&self, new_path: impl AsRef<Path>) -> Result<(), RuntimeJsonError> {
        let json = if self.options.pretty {
            serde_json::to_string_pretty(&self.value)?
        } else {
            serde_json::to_string(&self.value)?
        };
        atomic_write_json(new_path.as_ref(), json.as_bytes(), self.options.mode_unix)?;
        Ok(())
    }

    pub fn modify<F>(&mut self, f: F) -> Result<(), RuntimeJsonError>
    where
        F: FnOnce(&mut T),
    {
        f(&mut self.value);
        self.save()
    }
}

fn atomic_write_json(
    path: &Path,
    bytes: &[u8],
    mode_unix: Option<u32>,
) -> Result<(), RuntimeJsonError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir).ok();

    let mut tmp = NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file_mut().sync_all()?;

    #[cfg(unix)]
    if let Some(mode) = mode_unix {
        fs::set_permissions(tmp.path(), fs::Permissions::from_mode(mode))?;
    }

    tmp.persist(path)?;
    Ok(())
}

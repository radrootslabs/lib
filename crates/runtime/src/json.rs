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

#[cfg(test)]
mod tests {
    use super::{JsonFile, JsonWriteOptions, RuntimeJsonError, atomic_write_json};
    use serde::{Deserialize, Serialize, Serializer};
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct Payload {
        id: String,
        count: u32,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct AlwaysSerializeError;

    impl Serialize for AlwaysSerializeError {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(serde::ser::Error::custom(format!(
                "serialize error: {}",
                core::any::type_name::<S>()
            )))
        }
    }

    fn payload_path(name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(name);
        (dir, path)
    }

    fn should_not_create_payload() -> Payload {
        Payload {
            id: "should-not-create".to_string(),
            count: 9,
        }
    }

    #[test]
    fn load_reports_not_found_for_missing_path() {
        let (_dir, path) = payload_path("missing.json");
        let err = JsonFile::<Payload>::load(&path).expect_err("missing path should fail");
        assert!(err.to_string().contains(path.to_string_lossy().as_ref()));
    }

    #[test]
    fn load_reports_file_open_error_for_directory() {
        let dir = tempdir().expect("tempdir");
        let err = JsonFile::<Payload>::load(dir.path()).expect_err("directory path should fail");
        assert!(err.to_string().contains("Failed to parse JSON"));
        assert!(
            err.to_string()
                .contains(dir.path().to_string_lossy().as_ref())
        );
    }

    #[cfg(unix)]
    #[test]
    fn load_reports_file_open_error_for_unreadable_file_path_variants() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("unreadable.json");
        std::fs::write(&path, "{}").expect("write json");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
            .expect("set unreadable permission");

        let err_owned =
            JsonFile::<Payload>::load(path.clone()).expect_err("owned path should fail");
        assert!(matches!(err_owned, RuntimeJsonError::FileOpen(_, _)));

        let err_ref_buf = JsonFile::<Payload>::load(&path).expect_err("pathbuf ref should fail");
        assert!(matches!(err_ref_buf, RuntimeJsonError::FileOpen(_, _)));

        let err_ref_path =
            JsonFile::<Payload>::load(path.as_path()).expect_err("path ref should fail");
        assert!(matches!(err_ref_path, RuntimeJsonError::FileOpen(_, _)));

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("restore permission");
    }

    #[test]
    fn load_reports_file_parse_error_for_invalid_json() {
        let (_dir, path) = payload_path("invalid.json");
        std::fs::write(&path, "{invalid json").expect("write invalid json");
        let err_ref = JsonFile::<Payload>::load(&path).expect_err("invalid json should fail");
        assert!(matches!(err_ref, RuntimeJsonError::FileParse(_, _)));
        let err_owned =
            JsonFile::<Payload>::load(path.clone()).expect_err("invalid json should fail");
        assert!(matches!(err_owned, RuntimeJsonError::FileParse(_, _)));
    }

    #[test]
    fn load_or_create_save_modify_and_load_round_trip() {
        let (_dir, path) = payload_path("payload.json");
        let mut json = JsonFile::load_or_create_with(&path, || Payload {
            id: "item-1".to_string(),
            count: 1,
        })
        .expect("create json");

        assert_eq!(json.path(), path);
        assert_eq!(
            json.value,
            Payload {
                id: "item-1".to_string(),
                count: 1,
            }
        );

        json.set_options(JsonWriteOptions {
            pretty: true,
            mode_unix: None,
        });
        json.modify(|value| {
            value.count = 2;
        })
        .expect("modify json");

        let raw = std::fs::read_to_string(&path).expect("read json");
        assert!(raw.contains('\n'));
        assert_eq!(should_not_create_payload().count, 9);

        let loaded = JsonFile::<Payload>::load_or_create_with(&path, should_not_create_payload)
            .expect("load existing json");
        assert_eq!(
            loaded.value,
            Payload {
                id: "item-1".to_string(),
                count: 2,
            }
        );
    }

    #[test]
    fn save_as_writes_to_new_path() {
        let (_src_dir, source) = payload_path("source.json");
        let (_dst_dir, destination) = payload_path("dest.json");
        let json = JsonFile::load_or_create_with(&source, || Payload {
            id: "item-2".to_string(),
            count: 3,
        })
        .expect("create source json");

        json.save_as(&destination).expect("save as");
        let loaded = JsonFile::<Payload>::load(&destination).expect("load destination json");
        assert_eq!(
            loaded.value,
            Payload {
                id: "item-2".to_string(),
                count: 3,
            }
        );
    }

    #[test]
    fn save_reports_io_error_when_parent_is_not_directory() {
        let dir = tempdir().expect("tempdir");
        let parent_file = dir.path().join("not-a-dir");
        std::fs::write(&parent_file, "file").expect("write parent file");
        let target = parent_file.join("payload.json");

        let json = JsonFile::load_or_create_with(dir.path().join("valid.json"), || Payload {
            id: "item-3".to_string(),
            count: 4,
        })
        .expect("create json");

        let err = json.save_as(&target).expect_err("io error should surface");
        assert!(matches!(err, RuntimeJsonError::Io(_)));
    }

    #[test]
    fn save_reports_persist_error_when_target_is_directory() {
        let dir = tempdir().expect("tempdir");
        let target_dir = dir.path().join("target");
        std::fs::create_dir_all(&target_dir).expect("create target dir");

        let json = JsonFile::load_or_create_with(dir.path().join("value.json"), || Payload {
            id: "item-4".to_string(),
            count: 5,
        })
        .expect("create json");

        let err = json
            .save_as(&target_dir)
            .expect_err("persist error should surface");
        assert!(matches!(err, RuntimeJsonError::Persist(_)));
    }

    #[test]
    fn save_reports_serialization_error() {
        let (_dir, path) = payload_path("serialize-error.json");
        let mut json = JsonFile {
            value: AlwaysSerializeError,
            path,
            options: JsonWriteOptions::default(),
        };
        json.set_options(JsonWriteOptions {
            pretty: true,
            mode_unix: Some(0o600),
        });

        let err = json.save().expect_err("serialization error should surface");
        assert!(matches!(err, RuntimeJsonError::Serialization(_)));
    }

    #[test]
    fn atomic_write_json_honors_mode_none_and_some() {
        let (_none_dir, path_none) = payload_path("mode-none.json");
        atomic_write_json(&path_none, br#"{"id":"x","count":1}"#, None)
            .expect("write without mode");
        let (_some_dir, path_some) = payload_path("mode-some.json");
        atomic_write_json(&path_some, br#"{"id":"y","count":2}"#, Some(0o600))
            .expect("write with mode");

        let err =
            atomic_write_json(Path::new("/"), br#"{}"#, None).expect_err("root write should fail");
        assert!(matches!(
            err,
            RuntimeJsonError::Persist(_) | RuntimeJsonError::Io(_)
        ));
    }
}

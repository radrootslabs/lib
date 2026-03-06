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
        self.save_as(self.path.clone())
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

#[cfg(test)]
mod test_hooks {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::thread::{self, ThreadId};

    const FAIL_WRITE: u8 = 1;
    const FAIL_SYNC: u8 = 2;
    const FAIL_PERMS: u8 = 3;

    static FAIL_POINTS: OnceLock<Mutex<HashMap<ThreadId, u8>>> = OnceLock::new();

    pub struct FailGuard {
        thread_id: ThreadId,
    }

    impl Drop for FailGuard {
        fn drop(&mut self) {
            clear(self.thread_id);
        }
    }

    pub fn fail_write() -> FailGuard {
        set(FAIL_WRITE)
    }

    pub fn fail_sync() -> FailGuard {
        set(FAIL_SYNC)
    }

    pub fn fail_perms() -> FailGuard {
        set(FAIL_PERMS)
    }

    pub fn take_write() -> bool {
        take(FAIL_WRITE)
    }

    pub fn take_sync() -> bool {
        take(FAIL_SYNC)
    }

    pub fn take_perms() -> bool {
        take(FAIL_PERMS)
    }

    fn set(point: u8) -> FailGuard {
        let thread_id = thread::current().id();
        fail_map()
            .lock()
            .expect("lock fail hooks")
            .insert(thread_id, point);
        FailGuard { thread_id }
    }

    fn clear(thread_id: ThreadId) {
        fail_map()
            .lock()
            .expect("lock clear hooks")
            .remove(&thread_id);
    }

    fn take(point: u8) -> bool {
        let thread_id = thread::current().id();
        let mut map = fail_map().lock().expect("lock take hooks");
        match map.get(&thread_id).copied() {
            Some(current_point) if current_point == point => {
                map.remove(&thread_id);
                true
            }
            _ => false,
        }
    }

    fn fail_map() -> &'static Mutex<HashMap<ThreadId, u8>> {
        FAIL_POINTS.get_or_init(|| Mutex::new(HashMap::new()))
    }
}

fn write_temp_file(tmp: &mut NamedTempFile, bytes: &[u8]) -> io::Result<()> {
    #[cfg(test)]
    if test_hooks::take_write() {
        return Err(io::Error::new(io::ErrorKind::Other, "forced write failure"));
    }
    tmp.write_all(bytes)
}

fn sync_temp_file(tmp: &mut NamedTempFile) -> io::Result<()> {
    #[cfg(test)]
    if test_hooks::take_sync() {
        return Err(io::Error::new(io::ErrorKind::Other, "forced sync failure"));
    }
    tmp.as_file_mut().sync_all()
}

#[cfg(unix)]
fn set_temp_permissions(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(test)]
    if test_hooks::take_perms() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "forced permissions failure",
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

fn atomic_write_json(
    path: &Path,
    bytes: &[u8],
    mode_unix: Option<u32>,
) -> Result<(), RuntimeJsonError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir).ok();

    let mut tmp = NamedTempFile::new_in(dir)?;
    write_temp_file(&mut tmp, bytes)?;
    sync_temp_file(&mut tmp)?;

    #[cfg(unix)]
    if let Some(mode) = mode_unix {
        set_temp_permissions(tmp.path(), mode)?;
    }

    tmp.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{JsonFile, JsonWriteOptions, atomic_write_json, test_hooks};
    use serde::{Deserialize, Serialize, Serializer};
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct Payload {
        id: String,
        count: u32,
    }

    #[derive(Debug, Clone, Deserialize, PartialEq)]
    struct SerializeToggle {
        fail: bool,
        label: String,
    }

    #[derive(Serialize)]
    struct SerializeToggleData<'a> {
        fail: bool,
        label: &'a str,
    }

    impl Serialize for SerializeToggle {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            if self.fail {
                Err(serde::ser::Error::custom(format!(
                    "serialize error: {}",
                    core::any::type_name::<S>()
                )))
            } else {
                SerializeToggleData {
                    fail: self.fail,
                    label: &self.label,
                }
                .serialize(_serializer)
            }
        }
    }

    fn payload_path(name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(name);
        (dir, path)
    }

    fn toggle_default() -> SerializeToggle {
        SerializeToggle {
            fail: false,
            label: "item-1".to_string(),
        }
    }

    fn toggle_should_not_create() -> SerializeToggle {
        SerializeToggle {
            fail: false,
            label: "should-not-create".to_string(),
        }
    }

    #[test]
    fn toggle_should_not_create_builds_expected_value() {
        let value = toggle_should_not_create();
        assert_eq!(value.label, "should-not-create");
        assert!(!value.fail);
    }

    #[test]
    fn load_reports_not_found_for_missing_path() {
        let (_dir, path) = payload_path("missing.json");
        let err = JsonFile::<Payload>::load(path.clone()).expect_err("missing path should fail");
        assert!(err.to_string().contains(path.to_string_lossy().as_ref()));
    }

    #[test]
    fn load_reports_file_open_error_for_directory() {
        let dir = tempdir().expect("tempdir");
        let err = JsonFile::<Payload>::load(dir.path().to_path_buf())
            .expect_err("directory path should fail");
        assert!(err.to_string().contains("Failed to parse JSON"));
        assert!(
            err.to_string()
                .contains(dir.path().to_string_lossy().as_ref())
        );
    }

    #[cfg(unix)]
    #[test]
    fn load_reports_file_open_error_for_unreadable_file_path() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("unreadable.json");
        std::fs::write(&path, "{}").expect("write json");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
            .expect("set unreadable permission");

        let err = JsonFile::<Payload>::load(path.clone()).expect_err("owned path should fail");
        assert!(err.to_string().contains("Failed to open JSON file"));

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("restore permission");
    }

    #[test]
    fn load_reports_file_parse_error_for_invalid_json() {
        let (_dir, path) = payload_path("invalid.json");
        std::fs::write(&path, "{invalid json").expect("write invalid json");
        let err = JsonFile::<Payload>::load(path.clone()).expect_err("invalid json should fail");
        assert!(err.to_string().contains("Failed to parse JSON"));
    }

    #[test]
    fn load_reads_valid_json_payload() {
        let (_dir, path) = payload_path("valid.json");
        let payload = Payload {
            id: "item-1".to_string(),
            count: 2,
        };
        let encoded = serde_json::to_string(&payload).expect("serialize payload");
        std::fs::write(&path, encoded).expect("write json");
        let loaded = JsonFile::<Payload>::load(path.clone()).expect("load json");
        assert_eq!(loaded.value, payload);
    }

    #[test]
    fn load_or_create_save_modify_and_load_round_trip() {
        let (_dir, path) = payload_path("payload.json");
        let builder: fn() -> SerializeToggle = toggle_default;
        let mut json = JsonFile::load_or_create_with(path.clone(), builder).expect("create json");

        assert_eq!(json.path(), path);
        assert_eq!(json.value, toggle_default());

        json.set_options(JsonWriteOptions {
            pretty: true,
            mode_unix: None,
        });
        json.modify(|value| {
            value.label = "item-2".to_string();
        })
        .expect("modify json");

        let raw = std::fs::read_to_string(&path).expect("read json");
        assert!(raw.contains('\n'));

        let skip_builder: fn() -> SerializeToggle = toggle_should_not_create;
        let loaded = JsonFile::<SerializeToggle>::load_or_create_with(path.clone(), skip_builder)
            .expect("load existing json");
        assert_eq!(
            loaded.value,
            SerializeToggle {
                fail: false,
                label: "item-2".to_string(),
            }
        );
    }

    #[test]
    fn load_or_create_reports_save_error() {
        let (_dir, path) = payload_path("create-error.json");
        let builder: fn() -> SerializeToggle = toggle_default;
        let _guard = test_hooks::fail_write();
        let err = JsonFile::load_or_create_with(path.clone(), builder)
            .expect_err("save failure should surface");
        assert!(err.to_string().contains("I/O error during JSON write"));
    }

    #[test]
    fn save_as_writes_to_new_path() {
        let (_src_dir, source) = payload_path("source.json");
        let (_dst_dir, destination) = payload_path("dest.json");
        let builder: fn() -> SerializeToggle = toggle_default;
        let json =
            JsonFile::load_or_create_with(source.clone(), builder).expect("create source json");

        json.save_as(destination.clone()).expect("save as");
        let loaded =
            JsonFile::<SerializeToggle>::load(destination.clone()).expect("load destination json");
        assert_eq!(loaded.value, toggle_default());
    }

    #[test]
    fn save_reports_io_error_when_parent_is_not_directory() {
        let dir = tempdir().expect("tempdir");
        let parent_file = dir.path().join("not-a-dir");
        std::fs::write(&parent_file, "file").expect("write parent file");
        let target = parent_file.join("payload.json");

        let builder: fn() -> SerializeToggle = toggle_default;
        let json = JsonFile::load_or_create_with(dir.path().join("valid.json"), builder)
            .expect("create json");

        let err = json
            .save_as(target.clone())
            .expect_err("io error should surface");
        assert!(err.to_string().contains("I/O error during JSON write"));
    }

    #[test]
    fn save_reports_persist_error_when_target_is_directory() {
        let dir = tempdir().expect("tempdir");
        let target_dir = dir.path().join("target");
        std::fs::create_dir_all(&target_dir).expect("create target dir");

        let builder: fn() -> SerializeToggle = toggle_default;
        let json = JsonFile::load_or_create_with(dir.path().join("value.json"), builder)
            .expect("create json");

        let err = json
            .save_as(target_dir.clone())
            .expect_err("persist error should surface");
        assert!(
            err.to_string()
                .contains("Failed to persist JSON file to disk")
        );
    }

    #[test]
    fn save_reports_serialization_error() {
        let (_dir, path) = payload_path("serialize-error.json");
        let mut json = JsonFile {
            value: SerializeToggle {
                fail: true,
                label: "error".to_string(),
            },
            path,
            options: JsonWriteOptions::default(),
        };
        json.set_options(JsonWriteOptions {
            pretty: true,
            mode_unix: Some(0o600),
        });

        let err = json.save().expect_err("serialization error should surface");
        assert!(err.to_string().contains("Failed to serialize JSON"));
    }

    #[test]
    fn save_reports_serialization_error_non_pretty() {
        let (_dir, path) = payload_path("serialize-error-plain.json");
        let json = JsonFile {
            value: SerializeToggle {
                fail: true,
                label: "error".to_string(),
            },
            path,
            options: JsonWriteOptions::default(),
        };
        let err = json.save().expect_err("serialization error should surface");
        assert!(err.to_string().contains("Failed to serialize JSON"));
    }

    #[test]
    fn save_writes_when_serialize_toggle_allows() {
        let (_dir, path) = payload_path("serialize-ok.json");
        let json = JsonFile {
            value: SerializeToggle {
                fail: false,
                label: "ok".to_string(),
            },
            path,
            options: JsonWriteOptions::default(),
        };
        json.save().expect("save should succeed");
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
        let message = err.to_string();
        let is_persist = message.contains("Failed to persist JSON file to disk");
        let is_io = message.contains("I/O error during JSON write");
        assert!(is_persist | is_io);
    }

    #[test]
    fn atomic_write_json_reports_write_error() {
        let (_dir, path) = payload_path("write-error.json");
        let _guard = test_hooks::fail_write();
        let err = atomic_write_json(&path, br#"{"id":"x","count":1}"#, None)
            .expect_err("write error should surface");
        assert!(err.to_string().contains("I/O error during JSON write"));
    }

    #[test]
    fn atomic_write_json_reports_sync_error() {
        let (_dir, path) = payload_path("sync-error.json");
        let _guard = test_hooks::fail_sync();
        let err = atomic_write_json(&path, br#"{"id":"x","count":1}"#, None)
            .expect_err("sync error should surface");
        assert!(err.to_string().contains("I/O error during JSON write"));
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_json_reports_permissions_error() {
        let (_dir, path) = payload_path("perms-error.json");
        let _guard = test_hooks::fail_perms();
        let err = atomic_write_json(&path, br#"{"id":"x","count":1}"#, Some(0o600))
            .expect_err("permissions error should surface");
        assert!(err.to_string().contains("I/O error during JSON write"));
    }

    #[test]
    fn fail_hook_ignores_other_points() {
        let (_dir, path) = payload_path("ignore-other.json");
        let _guard = test_hooks::fail_write();
        assert!(!test_hooks::take_sync());
        let err = atomic_write_json(&path, br#"{"id":"x","count":1}"#, None)
            .expect_err("write error should surface");
        assert!(err.to_string().contains("I/O error during JSON write"));
    }

    #[test]
    fn fail_hook_is_thread_local() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("thread-local.json");
        let other_path = dir.path().join("thread-ok.json");
        let _guard = test_hooks::fail_write();
        let handle = std::thread::spawn(move || {
            atomic_write_json(&other_path, br#"{"id":"x","count":1}"#, None)
                .expect("other thread write");
        });
        handle.join().expect("join thread");
        let err = atomic_write_json(&path, br#"{"id":"x","count":1}"#, None)
            .expect_err("write error should surface");
        assert!(err.to_string().contains("I/O error during JSON write"));
    }
}

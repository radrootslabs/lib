use std::{fs, path::Path};

use crate::build_control::Mode;

pub fn write_if_changed(path: &Path, contents: &str) -> Result<bool, String> {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == contents
    {
        return Ok(false);
    }
    crate::build_control::atomic_write(path, contents.as_bytes())?;
    Ok(true)
}

pub fn write_or_check(path: &Path, contents: &str, mode: Mode) -> Result<(), String> {
    match mode {
        Mode::Write => write_if_changed(path, contents).map(|_| ()),
        Mode::Check => {
            let current = fs::read_to_string(path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            if current == contents {
                Ok(())
            } else {
                Err(format!("stale generated SDK output: {}", path.display()))
            }
        }
    }
}

pub fn write_bytes_if_changed(path: &Path, contents: &[u8]) -> Result<bool, String> {
    if let Ok(existing) = fs::read(path)
        && existing == contents
    {
        return Ok(false);
    }
    crate::build_control::atomic_write_bytes(path, contents)?;
    Ok(true)
}

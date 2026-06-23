use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "cannot resolve workspace root from {}",
                manifest_dir.display()
            )
        })
}

#[allow(dead_code)]
pub fn write_if_changed(path: &Path, contents: &str) -> Result<bool, String> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == contents {
            return Ok(false);
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::workspace_root;

    #[test]
    fn resolves_workspace_root() {
        let root = workspace_root().expect("workspace root resolves");
        assert!(root.join("Cargo.toml").is_file());
        assert!(root.join("packages").is_dir());
    }
}

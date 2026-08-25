use std::path::Path;

pub(super) fn short_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("rsh")
        .tempdir_in(Path::new("/tmp"))
        .expect("short temporary directory")
}

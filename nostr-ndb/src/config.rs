use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RadrootsNostrNdbConfig {
    db_dir: PathBuf,
    mapsize_bytes: Option<usize>,
    ingester_threads: Option<i32>,
    skip_validation: bool,
}

impl RadrootsNostrNdbConfig {
    pub fn new(db_dir: impl Into<PathBuf>) -> Self {
        Self {
            db_dir: db_dir.into(),
            mapsize_bytes: None,
            ingester_threads: None,
            skip_validation: false,
        }
    }

    pub fn db_dir(&self) -> &Path {
        &self.db_dir
    }

    pub fn mapsize_bytes(&self) -> Option<usize> {
        self.mapsize_bytes
    }

    pub fn ingester_threads(&self) -> Option<i32> {
        self.ingester_threads
    }

    pub fn skip_validation(&self) -> bool {
        self.skip_validation
    }

    pub fn with_mapsize_bytes(mut self, bytes: usize) -> Self {
        self.mapsize_bytes = Some(bytes);
        self
    }

    pub fn with_ingester_threads(mut self, threads: i32) -> Self {
        self.ingester_threads = Some(threads);
        self
    }

    pub fn with_skip_validation(mut self, skip_validation: bool) -> Self {
        self.skip_validation = skip_validation;
        self
    }
}

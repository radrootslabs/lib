use std::path::PathBuf;

pub const RADROOTS_MIGRATION_POSTURE: &str = "explicit_operator_import_required";
pub const RADROOTS_MIGRATION_COMPATIBILITY_WINDOW: &str = "detect_and_report_only";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsLegacyPathCandidate {
    pub id: String,
    pub description: String,
    pub path: PathBuf,
    pub destination: Option<PathBuf>,
    pub import_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsLegacyPathDetection {
    pub id: String,
    pub description: String,
    pub path: PathBuf,
    pub destination: Option<PathBuf>,
    pub import_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsMigrationReport {
    pub posture: &'static str,
    pub state: &'static str,
    pub silent_startup_relocation: bool,
    pub compatibility_window: &'static str,
    pub detected_legacy_paths: Vec<RadrootsLegacyPathDetection>,
}

impl RadrootsLegacyPathCandidate {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        path: impl Into<PathBuf>,
        destination: Option<PathBuf>,
        import_hint: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            path: path.into(),
            destination,
            import_hint: import_hint.into(),
        }
    }

    fn into_detection(self) -> RadrootsLegacyPathDetection {
        RadrootsLegacyPathDetection {
            id: self.id,
            description: self.description,
            path: self.path,
            destination: self.destination,
            import_hint: self.import_hint,
        }
    }
}

impl RadrootsMigrationReport {
    #[must_use]
    pub fn empty() -> Self {
        Self::from_detected_legacy_paths(Vec::new())
    }

    #[must_use]
    pub fn from_detected_legacy_paths(
        detected_legacy_paths: Vec<RadrootsLegacyPathDetection>,
    ) -> Self {
        let state = if detected_legacy_paths.is_empty() {
            "ready"
        } else {
            "legacy_state_detected"
        };
        Self {
            posture: RADROOTS_MIGRATION_POSTURE,
            state,
            silent_startup_relocation: false,
            compatibility_window: RADROOTS_MIGRATION_COMPATIBILITY_WINDOW,
            detected_legacy_paths,
        }
    }
}

#[must_use]
pub fn inspect_legacy_paths(
    candidates: impl IntoIterator<Item = RadrootsLegacyPathCandidate>,
) -> RadrootsMigrationReport {
    let detected = candidates
        .into_iter()
        .filter(|candidate| candidate.path.exists())
        .map(RadrootsLegacyPathCandidate::into_detection)
        .collect();
    RadrootsMigrationReport::from_detected_legacy_paths(detected)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        RADROOTS_MIGRATION_COMPATIBILITY_WINDOW, RADROOTS_MIGRATION_POSTURE,
        RadrootsLegacyPathCandidate, inspect_legacy_paths,
    };

    fn unique_test_dir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("radroots-runtime-paths-test-{nanos}"));
        std::fs::create_dir_all(&path).expect("create temp test dir");
        path
    }

    #[test]
    fn inspect_legacy_paths_reports_only_paths_that_exist() {
        let temp = unique_test_dir();
        let existing = temp.join("old-state");
        let missing = temp.join("missing-state");
        std::fs::write(&existing, "legacy").expect("write legacy marker");

        let report = inspect_legacy_paths([
            RadrootsLegacyPathCandidate::new(
                "old-state",
                "old state",
                &existing,
                Some(temp.join("new-state")),
                "run the explicit importer",
            ),
            RadrootsLegacyPathCandidate::new(
                "missing-state",
                "missing state",
                &missing,
                None,
                "nothing to do",
            ),
        ]);

        assert_eq!(report.posture, RADROOTS_MIGRATION_POSTURE);
        assert_eq!(report.state, "legacy_state_detected");
        assert!(!report.silent_startup_relocation);
        assert_eq!(
            report.compatibility_window,
            RADROOTS_MIGRATION_COMPATIBILITY_WINDOW
        );
        assert_eq!(report.detected_legacy_paths.len(), 1);
        assert_eq!(report.detected_legacy_paths[0].id, "old-state");
        assert_eq!(report.detected_legacy_paths[0].path, existing);
        std::fs::remove_dir_all(temp).expect("remove temp test dir");
    }

    #[test]
    fn inspect_legacy_paths_is_ready_when_no_candidate_exists() {
        let temp = unique_test_dir();

        let report = inspect_legacy_paths([RadrootsLegacyPathCandidate::new(
            "missing-state",
            "missing state",
            temp.join("missing-state"),
            None,
            "nothing to do",
        )]);

        assert_eq!(report.state, "ready");
        assert!(report.detected_legacy_paths.is_empty());
        std::fs::remove_dir_all(temp).expect("remove temp test dir");
    }
}

use chrono::Utc;
use std::path::PathBuf;

pub const DEFAULT_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
pub const DEFAULT_RETAINED_FILES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFileLayout {
    PrefixedDate,
    DatedFileName,
    StableFileName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Compact,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogIdentity {
    pub service: String,
    pub run_id: String,
    pub environment: String,
}

impl LogIdentity {
    pub fn new(
        service: impl Into<String>,
        run_id: impl Into<String>,
        environment: impl Into<String>,
    ) -> Self {
        Self {
            service: service.into(),
            run_id: run_id.into(),
            environment: environment.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogRotation {
    pub max_file_bytes: u64,
    pub retained_files: usize,
}

impl Default for LogRotation {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            retained_files: DEFAULT_RETAINED_FILES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingOptions {
    pub dir: Option<PathBuf>,
    pub file_name: String,
    pub stdout: bool,
    pub stderr: bool,
    pub default_level: Option<String>,
    pub file_layout: LogFileLayout,
    pub format: LogFormat,
    pub identity: Option<LogIdentity>,
    pub rotation: LogRotation,
}

impl LoggingOptions {
    pub fn also_stdout(&self) -> bool {
        self.stdout
    }

    pub fn json_service(
        service: impl Into<String>,
        run_id: impl Into<String>,
        environment: impl Into<String>,
    ) -> Self {
        let service = service.into();
        Self {
            file_name: format!("{service}.jsonl"),
            format: LogFormat::Json,
            identity: Some(LogIdentity::new(service, run_id, environment)),
            file_layout: LogFileLayout::StableFileName,
            ..Self::default()
        }
    }

    pub fn resolved_log_file_name_for_date(&self, date: &str) -> String {
        match self.file_layout {
            LogFileLayout::PrefixedDate => format!("{}.{}", self.file_name, date),
            LogFileLayout::DatedFileName => format!("{}.{}", date, self.file_name),
            LogFileLayout::StableFileName => self.file_name.clone(),
        }
    }

    pub fn resolved_current_log_file_path(&self) -> Option<PathBuf> {
        let dir = self.dir.as_ref()?;
        let date = Utc::now().format("%Y-%m-%d").to_string();
        Some(dir.join(self.resolved_log_file_name_for_date(date.as_str())))
    }
}

impl Default for LoggingOptions {
    fn default() -> Self {
        Self {
            dir: None,
            file_name: "radroots.log".into(),
            stdout: true,
            stderr: false,
            default_level: None,
            file_layout: LogFileLayout::PrefixedDate,
            format: LogFormat::Compact,
            identity: None,
            rotation: LogRotation::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LogFileLayout, LogFormat, LoggingOptions};
    use std::path::PathBuf;

    #[test]
    fn layouts_resolve_expected_file_names() {
        let mut options = LoggingOptions {
            dir: Some(PathBuf::from("/tmp/logs")),
            file_name: "service.log".to_owned(),
            ..LoggingOptions::default()
        };
        assert_eq!(
            options.resolved_log_file_name_for_date("2026-03-23"),
            "service.log.2026-03-23"
        );

        options.file_layout = LogFileLayout::DatedFileName;
        assert_eq!(
            options.resolved_log_file_name_for_date("2026-03-23"),
            "2026-03-23.service.log"
        );

        options.file_layout = LogFileLayout::StableFileName;
        assert_eq!(
            options.resolved_log_file_name_for_date("2026-03-23"),
            "service.log"
        );
    }

    #[test]
    fn json_service_has_bounded_production_defaults() {
        let options = LoggingOptions::json_service("global_relay", "run-1", "localhost");
        assert_eq!(options.file_name, "global_relay.jsonl");
        assert_eq!(options.file_layout, LogFileLayout::StableFileName);
        assert_eq!(options.format, LogFormat::Json);
        assert_eq!(
            options
                .identity
                .as_ref()
                .map(|identity| identity.service.as_str()),
            Some("global_relay")
        );
        assert!(options.rotation.max_file_bytes > 0);
        assert!(options.rotation.retained_files > 0);
    }

    #[test]
    fn current_log_file_path_joins_dir_and_layout_shape() {
        let options = LoggingOptions {
            dir: Some(PathBuf::from("/tmp/logs")),
            file_name: "service.jsonl".to_owned(),
            file_layout: LogFileLayout::StableFileName,
            ..LoggingOptions::default()
        };
        assert_eq!(
            options.resolved_current_log_file_path(),
            Some(PathBuf::from("/tmp/logs/service.jsonl"))
        );
    }
}

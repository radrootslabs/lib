use chrono::Local;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFileLayout {
    PrefixedDate,
    DatedFileName,
}

#[derive(Debug, Clone)]
pub struct LoggingOptions {
    pub dir: Option<PathBuf>,
    pub file_name: String,
    pub stdout: bool,
    pub default_level: Option<String>,
    pub file_layout: LogFileLayout,
}

impl LoggingOptions {
    pub fn also_stdout(&self) -> bool {
        self.stdout
    }

    pub fn resolved_log_file_name_for_date(&self, date: &str) -> String {
        match self.file_layout {
            LogFileLayout::PrefixedDate => format!("{}.{}", self.file_name, date),
            LogFileLayout::DatedFileName => format!("{}.{}", date, self.file_name),
        }
    }

    pub fn resolved_current_log_file_path(&self) -> Option<PathBuf> {
        let dir = self.dir.as_ref()?;
        let date = Local::now().format("%Y-%m-%d").to_string();
        Some(dir.join(self.resolved_log_file_name_for_date(date.as_str())))
    }
}

impl Default for LoggingOptions {
    fn default() -> Self {
        Self {
            dir: None,
            file_name: "radroots.log".into(),
            stdout: true,
            default_level: None,
            file_layout: LogFileLayout::PrefixedDate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LogFileLayout, LoggingOptions};
    use std::path::PathBuf;

    #[test]
    fn prefixed_date_layout_resolves_expected_file_name() {
        let options = LoggingOptions {
            dir: Some(PathBuf::from("/tmp/logs")),
            file_name: "myc.log".to_owned(),
            stdout: false,
            default_level: None,
            file_layout: LogFileLayout::PrefixedDate,
        };

        assert_eq!(
            options.resolved_log_file_name_for_date("2026-03-23"),
            "myc.log.2026-03-23"
        );
    }

    #[test]
    fn dated_file_name_layout_resolves_expected_file_name() {
        let options = LoggingOptions {
            dir: Some(PathBuf::from("/tmp/logs")),
            file_name: "log".to_owned(),
            stdout: false,
            default_level: None,
            file_layout: LogFileLayout::DatedFileName,
        };

        assert_eq!(
            options.resolved_log_file_name_for_date("2026-03-23"),
            "2026-03-23.log"
        );
    }

    #[test]
    fn current_log_file_path_joins_dir_and_layout_shape() {
        let options = LoggingOptions {
            dir: Some(PathBuf::from("/tmp/logs")),
            file_name: "log".to_owned(),
            stdout: false,
            default_level: None,
            file_layout: LogFileLayout::DatedFileName,
        };

        let path = options
            .resolved_current_log_file_path()
            .expect("resolved path");

        assert_eq!(path.parent(), Some(PathBuf::from("/tmp/logs").as_path()));
        assert!(
            path.file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.ends_with(".log"))
        );
    }
}

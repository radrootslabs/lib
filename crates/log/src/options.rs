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

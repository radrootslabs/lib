use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LoggingOptions {
    pub dir: Option<PathBuf>,
    pub file_name: String,
    pub stdout: bool,
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
        }
    }
}

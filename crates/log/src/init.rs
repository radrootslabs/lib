use std::fs;
use std::sync::OnceLock;

use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::Result;
use crate::options::{LogFileLayout, LoggingOptions};

static GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
static INIT: OnceLock<()> = OnceLock::new();

pub fn init_logging(opts: LoggingOptions) -> Result<()> {
    if INIT.get().is_some() {
        return Ok(());
    }

    let writer = if let Some(dir) = &opts.dir {
        fs::create_dir_all(dir)?;
        let file_appender = build_file_appender(dir, &opts)?;
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let _ = GUARD.set(guard);
        Some(non_blocking)
    } else {
        None
    };

    let env = resolve_env_filter(opts.default_level.as_deref());
    let fmt_layer_file = writer.as_ref().map(|w| {
        fmt::layer()
            .with_writer(w.clone())
            .with_ansi(false)
            .with_target(false)
    });
    let fmt_layer_stdout = if opts.also_stdout() {
        Some(fmt::layer().with_writer(std::io::stdout).with_target(false))
    } else {
        None
    };

    let subscriber = tracing_subscriber::registry()
        .with(env)
        .with(fmt_layer_file)
        .with(fmt_layer_stdout);

    subscriber.try_init()?;
    let _ = INIT.set(());
    info!(
        "logging initialized (file: {}, stdout: {})",
        opts.resolved_current_log_file_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<disabled>".into()),
        opts.also_stdout()
    );
    Ok(())
}

fn resolve_env_filter(default_level: Option<&str>) -> EnvFilter {
    match default_level {
        Some(level) => EnvFilter::new(level),
        None => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
    }
}

pub fn init_stdout() -> Result<()> {
    init_logging(LoggingOptions {
        dir: None,
        file_name: "radroots.log".into(),
        stdout: true,
        default_level: None,
        file_layout: LogFileLayout::PrefixedDate,
    })
}

fn build_file_appender(
    dir: &std::path::Path,
    opts: &LoggingOptions,
) -> Result<RollingFileAppender> {
    let builder = RollingFileAppender::builder().rotation(Rotation::DAILY);
    let builder = match opts.file_layout {
        LogFileLayout::PrefixedDate => builder.filename_prefix(opts.file_name.as_str()),
        LogFileLayout::DatedFileName => builder.filename_suffix(opts.file_name.as_str()),
    };
    Ok(builder.build(dir)?)
}

#[cfg(test)]
mod tests {
    use super::{build_file_appender, init_logging, resolve_env_filter};
    use crate::{LogFileLayout, LoggingOptions};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tracing_subscriber::fmt::MakeWriter;

    fn temp_log_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("radroots_log-{name}-{nanos}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn prefixed_date_layout_keeps_existing_filename_shape() {
        let dir = temp_log_dir("prefixed-date");
        let appender = build_file_appender(
            dir.as_path(),
            &LoggingOptions {
                dir: Some(dir.clone()),
                file_name: "myc.log".to_owned(),
                stdout: false,
                default_level: Some("info".to_owned()),
                file_layout: LogFileLayout::PrefixedDate,
            },
        )
        .expect("appender");

        let writer = appender.make_writer();
        drop(writer);

        let names: Vec<String> = std::fs::read_dir(&dir)
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert_eq!(names.len(), 1);
        assert!(names[0].starts_with("myc.log."));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dated_file_name_layout_writes_date_named_log_files() {
        let dir = temp_log_dir("dated-file-name");
        let appender = build_file_appender(
            dir.as_path(),
            &LoggingOptions {
                dir: Some(dir.clone()),
                file_name: "log".to_owned(),
                stdout: false,
                default_level: Some("info".to_owned()),
                file_layout: LogFileLayout::DatedFileName,
            },
        )
        .expect("appender");

        let writer = appender.make_writer();
        drop(writer);

        let names: Vec<String> = std::fs::read_dir(&dir)
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert_eq!(names.len(), 1);
        assert!(names[0].ends_with(".log"));
        assert_eq!(names[0].matches('.').count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn init_paths_cover_layout_options() {
        let dir = temp_log_dir("init-paths");
        std::fs::create_dir_all(&dir).expect("create dir");
        let invalid = dir.join("not-a-dir");
        std::fs::write(&invalid, "file").expect("write invalid path");
        let err_path = init_logging(LoggingOptions {
            dir: Some(invalid),
            file_name: "x.log".to_string(),
            stdout: false,
            default_level: None,
            file_layout: LogFileLayout::PrefixedDate,
        });
        assert!(err_path.is_err());

        let first = build_file_appender(
            &dir,
            &LoggingOptions {
                dir: Some(dir.clone()),
                file_name: "service".to_string(),
                stdout: false,
                default_level: Some("info".to_string()),
                file_layout: LogFileLayout::DatedFileName,
            },
        );
        assert!(first.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn explicit_default_level_wins_over_ambient_rust_log() {
        // Callers that pass an explicit service filter should not inherit the shell's RUST_LOG.
        let env = resolve_env_filter(Some("info,myc=info"));
        let rendered = env.to_string();
        assert!(rendered.contains("info"));
        assert!(rendered.contains("myc=info"));
    }
}

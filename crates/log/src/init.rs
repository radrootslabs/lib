use std::fs;
use std::path::{Component, Path};
use std::sync::Mutex;

use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::format::JsonEventFormatter;
use crate::options::{LogFileLayout, LogFormat, LoggingOptions};
use crate::writer::SizeRotatingWriter;
use crate::{Error, Result};

struct InitializedLogging {
    options: LoggingOptions,
    _file_guard: Option<WorkerGuard>,
}

enum LoggingState {
    Uninitialized,
    Active(Box<InitializedLogging>),
    Shutdown,
}

static LOGGING_STATE: Mutex<LoggingState> = Mutex::new(LoggingState::Uninitialized);

pub fn init_logging(opts: LoggingOptions) -> Result<()> {
    validate_options(&opts)?;
    let mut state = LOGGING_STATE
        .lock()
        .map_err(|_| Error::Msg("logging initialization state is poisoned".to_owned()))?;
    match &*state {
        LoggingState::Active(active) => {
            return initialization_decision(Some(&active.options), &opts).map(|_| ());
        }
        LoggingState::Shutdown => return Err(Error::InitializationAfterShutdown),
        LoggingState::Uninitialized => {}
    }

    let (file_writer, file_guard) = build_file_writer(&opts)?;
    let filter = resolve_env_filter(opts.default_level.as_deref());
    match opts.format {
        LogFormat::Compact => {
            let file_layer = file_writer.as_ref().map(|writer| {
                fmt::layer()
                    .with_writer(writer.clone())
                    .with_ansi(false)
                    .with_target(false)
            });
            let stdout_layer = opts
                .also_stdout()
                .then(|| fmt::layer().with_writer(std::io::stdout).with_target(false));
            let stderr_layer = opts
                .stderr
                .then(|| fmt::layer().with_writer(std::io::stderr).with_target(false));
            tracing_subscriber::registry()
                .with(filter)
                .with(file_layer)
                .with(stdout_layer)
                .with(stderr_layer)
                .try_init()?;
        }
        LogFormat::Json => {
            let identity = opts
                .identity
                .as_ref()
                .cloned()
                .ok_or_else(|| Error::Msg("validated JSON identity is missing".to_owned()))?;
            let file_layer = file_writer.as_ref().map(|writer| {
                fmt::layer()
                    .event_format(JsonEventFormatter::new(identity.clone()))
                    .with_writer(writer.clone())
                    .with_ansi(false)
            });
            let stdout_layer = opts.also_stdout().then(|| {
                fmt::layer()
                    .event_format(JsonEventFormatter::new(identity.clone()))
                    .with_writer(std::io::stdout)
                    .with_ansi(false)
            });
            let stderr_layer = opts.stderr.then(|| {
                fmt::layer()
                    .event_format(JsonEventFormatter::new(identity))
                    .with_writer(std::io::stderr)
                    .with_ansi(false)
            });
            tracing_subscriber::registry()
                .with(filter)
                .with(file_layer)
                .with(stdout_layer)
                .with(stderr_layer)
                .try_init()?;
        }
    }
    let file_path = opts.resolved_current_log_file_path();
    *state = LoggingState::Active(Box::new(InitializedLogging {
        options: opts.clone(),
        _file_guard: file_guard,
    }));
    info!(
        file_enabled = file_path.is_some(),
        stdout_enabled = opts.also_stdout(),
        stderr_enabled = opts.stderr,
        "logging initialized"
    );
    Ok(())
}

pub fn flush_and_shutdown() -> Result<()> {
    let active = {
        let mut state = LOGGING_STATE
            .lock()
            .map_err(|_| Error::Msg("logging initialization state is poisoned".to_owned()))?;
        match std::mem::replace(&mut *state, LoggingState::Shutdown) {
            LoggingState::Active(active) => Some(active),
            LoggingState::Shutdown => None,
            LoggingState::Uninitialized => {
                *state = LoggingState::Uninitialized;
                return Err(Error::ShutdownBeforeInitialization);
            }
        }
    };
    drop(active);
    Ok(())
}

fn initialization_decision(
    active: Option<&LoggingOptions>,
    requested: &LoggingOptions,
) -> Result<bool> {
    match active {
        Some(active) if active == requested => Ok(true),
        Some(_) => Err(Error::ConflictingInitialization),
        None => Ok(false),
    }
}

fn build_file_writer(
    opts: &LoggingOptions,
) -> Result<(
    Option<tracing_appender::non_blocking::NonBlocking>,
    Option<WorkerGuard>,
)> {
    let Some(dir) = &opts.dir else {
        return Ok((None, None));
    };
    fs::create_dir_all(dir)?;
    let metadata = fs::symlink_metadata(dir)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::Msg(
            "log directory is not a safe directory".to_owned(),
        ));
    }
    let path = opts
        .resolved_current_log_file_path()
        .ok_or_else(|| Error::Msg("log file path could not be resolved".to_owned()))?;
    let writer = SizeRotatingWriter::new(path, opts.rotation)?;
    let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(8192)
        .lossy(false)
        .thread_name("radroots-log-writer")
        .finish(writer);
    Ok((Some(writer), Some(guard)))
}

fn validate_options(opts: &LoggingOptions) -> Result<()> {
    if opts.dir.is_none() && !opts.stdout && !opts.stderr {
        return Err(Error::Msg(
            "logging requires at least one configured output".to_owned(),
        ));
    }
    if opts.file_name.is_empty()
        || Path::new(&opts.file_name).components().count() != 1
        || !matches!(
            Path::new(&opts.file_name).components().next(),
            Some(Component::Normal(_))
        )
    {
        return Err(Error::Msg(
            "log file_name must be a safe basename".to_owned(),
        ));
    }
    if opts.dir.is_some() && opts.file_layout != LogFileLayout::StableFileName {
        return Err(Error::Msg(
            "bounded file logging requires StableFileName layout".to_owned(),
        ));
    }
    if opts.rotation.max_file_bytes == 0 || opts.rotation.retained_files == 0 {
        return Err(Error::Msg(
            "log rotation limits must be greater than zero".to_owned(),
        ));
    }
    match (&opts.format, &opts.identity) {
        (LogFormat::Json, Some(identity)) => {
            validate_identity_value("service", &identity.service)?;
            validate_identity_value("run_id", &identity.run_id)?;
            validate_identity_value("environment", &identity.environment)?;
        }
        (LogFormat::Json, None) => {
            return Err(Error::Msg(
                "JSON logging requires service, run, and environment identity".to_owned(),
            ));
        }
        (LogFormat::Compact, _) => {}
    }
    Ok(())
}

fn validate_identity_value(label: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(Error::Msg(format!(
            "log {label} identity must use 1-128 ASCII identifier characters"
        )));
    }
    Ok(())
}

fn resolve_env_filter(default_level: Option<&str>) -> EnvFilter {
    match default_level {
        Some(level) => EnvFilter::new(level),
        None => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
    }
}

pub fn init_stdout() -> Result<()> {
    init_logging(LoggingOptions::default())
}

#[cfg(test)]
mod tests {
    use super::{initialization_decision, resolve_env_filter, validate_options};
    use crate::{LogFileLayout, LogFormat, LoggingOptions};
    use std::path::PathBuf;

    #[test]
    fn rejects_unbounded_or_identity_free_json_options() {
        let mut options = LoggingOptions::json_service("service", "run-1", "test");
        options.identity = None;
        assert!(validate_options(&options).is_err());

        options = LoggingOptions::json_service("service", "run-1", "test");
        options.rotation.retained_files = 0;
        assert!(validate_options(&options).is_err());

        options.rotation.retained_files = 1;
        options.dir = Some(PathBuf::from("/tmp/logs"));
        options.file_layout = LogFileLayout::PrefixedDate;
        assert!(validate_options(&options).is_err());
    }

    #[test]
    fn rejects_unsafe_file_and_identity_values() {
        let mut options = LoggingOptions::json_service("service", "run-1", "test");
        options.file_name = "../service.jsonl".to_owned();
        assert!(validate_options(&options).is_err());

        options.file_name = "service.jsonl".to_owned();
        options.identity.as_mut().expect("identity").run_id = "contains space".to_owned();
        assert!(validate_options(&options).is_err());
    }

    #[test]
    fn explicit_default_level_wins_over_ambient_rust_log() {
        let env = resolve_env_filter(Some("info,myc=info"));
        let rendered = env.to_string();
        assert!(rendered.contains("info"));
        assert!(rendered.contains("myc=info"));
    }

    #[test]
    fn compact_stdout_defaults_remain_valid() {
        let options = LoggingOptions::default();
        assert_eq!(options.format, LogFormat::Compact);
        assert!(validate_options(&options).is_ok());
    }

    #[test]
    fn stderr_is_a_valid_standalone_sink() {
        let mut options = LoggingOptions::json_service("service", "run-1", "production");
        options.stdout = false;
        options.stderr = true;
        assert!(validate_options(&options).is_ok());
    }

    #[test]
    fn repeated_initialization_reuses_only_an_identical_configuration() {
        let active = LoggingOptions::json_service("service", "run-1", "test");
        assert!(initialization_decision(Some(&active), &active).expect("reuse"));
        let conflicting = LoggingOptions::json_service("service", "run-2", "test");
        assert!(initialization_decision(Some(&active), &conflicting).is_err());
        assert!(!initialization_decision(None, &active).expect("initialize"));
    }
}

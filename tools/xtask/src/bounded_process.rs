//! Hermetic, resource-bounded subprocess execution for xtask.
//!
//! This module owns command construction so callers cannot bypass replacement
//! environment, closed-stdin, process-group, deadline, or stream-cap policy.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;

const DEFAULT_DEADLINE: Duration = Duration::from_secs(60);
const DEFAULT_STREAM_LIMIT: usize = 1024 * 1024;
const DEFAULT_TERMINATION_GRACE: Duration = Duration::from_millis(250);
const MAX_DEADLINE: Duration = Duration::from_secs(86_400);
const MAX_TERMINATION_GRACE: Duration = Duration::from_secs(5);
const MAX_STREAM_LIMIT: usize = 67_108_864;
const MAX_ENVIRONMENT_ENTRIES: usize = 64;
const MAX_ENVIRONMENT_NAME_BYTES: usize = 128;
const MAX_ENVIRONMENT_VALUE_BYTES: usize = 65_536;

const FORBIDDEN_ENVIRONMENT_NAMES: [&str; 10] = [
    "CARGO_ENCODED_RUSTFLAGS",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "LD_LIBRARY_PATH",
    "LD_PRELOAD",
    "NIX_CONFIG",
    "NIX_PATH",
    "NIXPKGS_ALLOW_BROKEN",
    "NIXPKGS_ALLOW_UNFREE",
    "RUSTFLAGS",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EnvironmentRejection {
    DuplicateName,
    ForbiddenControl,
    InvalidNameOrNul,
    SensitiveName,
    TooManyEntries,
    ValueTooLarge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputLimitBreach {
    Stdout,
    Stderr,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessFailureKind {
    CleanupIncomplete,
    DeadlineExceeded,
    EnvironmentRejected(EnvironmentRejection),
    InvalidConfiguration,
    OrphanedDescendants,
    OutputLimitExceeded,
    PipeSetup,
    Poll,
    Read,
    Signal,
    Spawn,
    #[cfg_attr(unix, allow(dead_code))]
    UnsupportedPlatform,
    Wait,
}

/// A deliberately redacted process failure.
pub(crate) struct ProcessError {
    kind: ProcessFailureKind,
    io_kind: Option<io::ErrorKind>,
    output_limit: Option<OutputLimitBreach>,
}

impl ProcessError {
    fn new(kind: ProcessFailureKind) -> Self {
        Self {
            kind,
            io_kind: None,
            output_limit: None,
        }
    }

    fn from_io(kind: ProcessFailureKind, error: &io::Error) -> Self {
        Self {
            kind,
            io_kind: Some(error.kind()),
            output_limit: None,
        }
    }

    #[cfg(unix)]
    fn from_errno(kind: ProcessFailureKind, error: rustix::io::Errno) -> Self {
        Self {
            kind,
            io_kind: Some(error.kind()),
            output_limit: None,
        }
    }

    fn output_limit(breach: OutputLimitBreach) -> Self {
        Self {
            kind: ProcessFailureKind::OutputLimitExceeded,
            io_kind: None,
            output_limit: Some(breach),
        }
    }

    pub(crate) fn kind(&self) -> ProcessFailureKind {
        self.kind
    }

    pub(crate) fn output_limit_breach(&self) -> Option<OutputLimitBreach> {
        self.output_limit
    }
}

impl fmt::Debug for ProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessError")
            .field("kind", &self.kind)
            .field("io_kind", &self.io_kind)
            .field("output_limit", &self.output_limit)
            .finish()
    }
}

impl fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            ProcessFailureKind::CleanupIncomplete => {
                "bounded process cleanup did not reach a verified terminal state"
            }
            ProcessFailureKind::DeadlineExceeded => "bounded process deadline exceeded",
            ProcessFailureKind::EnvironmentRejected(_) => {
                "bounded process replacement environment was rejected"
            }
            ProcessFailureKind::InvalidConfiguration => {
                "bounded process configuration was rejected"
            }
            ProcessFailureKind::OrphanedDescendants => {
                "bounded process leader exited while descendants remained"
            }
            ProcessFailureKind::OutputLimitExceeded => "bounded process output limit exceeded",
            ProcessFailureKind::PipeSetup => "bounded process pipe setup failed",
            ProcessFailureKind::Poll => "bounded process polling failed",
            ProcessFailureKind::Read => "bounded process output read failed",
            ProcessFailureKind::Signal => "bounded process group signaling failed",
            ProcessFailureKind::Spawn => "bounded process spawn failed",
            ProcessFailureKind::UnsupportedPlatform => {
                "bounded process groups are unsupported on this platform"
            }
            ProcessFailureKind::Wait => "bounded process wait failed",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ProcessError {}

/// An explicit replacement environment. No ambient snapshot is ever taken.
#[derive(Clone, Default)]
pub(crate) struct ReplacementEnvironment {
    entries: BTreeMap<OsString, OsString>,
}

impl ReplacementEnvironment {
    pub(crate) fn insert(
        &mut self,
        name: impl Into<OsString>,
        value: impl Into<OsString>,
    ) -> Result<(), ProcessError> {
        let name = name.into();
        let value = value.into();
        let name_text = name
            .to_str()
            .ok_or_else(|| environment_error(EnvironmentRejection::InvalidNameOrNul))?;
        if let Some(rejection) = environment_rejection(name_text) {
            return Err(environment_error(rejection));
        }
        if os_value_contains_nul(&value) {
            return Err(environment_error(EnvironmentRejection::InvalidNameOrNul));
        }
        if os_value_byte_len(&value) > MAX_ENVIRONMENT_VALUE_BYTES {
            return Err(environment_error(EnvironmentRejection::ValueTooLarge));
        }
        if self.entries.contains_key(&name) {
            return Err(environment_error(EnvironmentRejection::DuplicateName));
        }
        if self.entries.len() == MAX_ENVIRONMENT_ENTRIES {
            return Err(environment_error(EnvironmentRejection::TooManyEntries));
        }
        self.entries.insert(name, value);
        Ok(())
    }
}

impl fmt::Debug for ReplacementEnvironment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplacementEnvironment")
            .field("entry_count", &self.entries.len())
            .field("values", &"<redacted>")
            .finish()
    }
}

fn environment_error(rejection: EnvironmentRejection) -> ProcessError {
    ProcessError::new(ProcessFailureKind::EnvironmentRejected(rejection))
}

fn environment_rejection(name: &str) -> Option<EnvironmentRejection> {
    if name.is_empty()
        || name.len() > MAX_ENVIRONMENT_NAME_BYTES
        || name.as_bytes().contains(&0)
        || !name.is_ascii()
        || !name
            .bytes()
            .next()
            .is_some_and(|byte| byte == b'_' || byte.is_ascii_uppercase())
        || !name
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Some(EnvironmentRejection::InvalidNameOrNul);
    }
    if ["CREDENTIAL", "KEY", "PASSWORD", "SECRET", "TOKEN"]
        .iter()
        .any(|pattern| name.contains(pattern))
    {
        return Some(EnvironmentRejection::SensitiveName);
    }
    if FORBIDDEN_ENVIRONMENT_NAMES.contains(&name)
        || name == "CARGO_BUILD_RUSTFLAGS"
        || (name.starts_with("CARGO_TARGET_") && name.ends_with("_RUSTFLAGS"))
        || name.starts_with("DYLD_")
        || matches!(
            name,
            "LD_AUDIT"
                | "LD_DEBUG"
                | "LD_PROFILE"
                | "RUSTC_WRAPPER"
                | "RUSTC_WORKSPACE_WRAPPER"
                | "RUSTDOCFLAGS"
        )
    {
        return Some(EnvironmentRejection::ForbiddenControl);
    }
    None
}

#[cfg(unix)]
fn os_value_byte_len(value: &OsStr) -> usize {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().len()
}

#[cfg(unix)]
fn os_value_contains_nul(value: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().contains(&0)
}

#[cfg(windows)]
fn os_value_byte_len(value: &OsStr) -> usize {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().count().saturating_mul(2)
}

#[cfg(windows)]
fn os_value_contains_nul(value: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().any(|unit| unit == 0)
}

#[cfg(not(any(unix, windows)))]
fn os_value_byte_len(value: &OsStr) -> usize {
    value.to_string_lossy().len()
}

#[cfg(not(any(unix, windows)))]
fn os_value_contains_nul(value: &OsStr) -> bool {
    value.to_string_lossy().contains('\0')
}

pub(crate) struct ProcessRequest {
    program: OsString,
    arguments: Vec<OsString>,
    current_dir: Option<PathBuf>,
    environment: ReplacementEnvironment,
    deadline: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    termination_grace: Duration,
}

impl ProcessRequest {
    pub(crate) fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
            current_dir: None,
            environment: ReplacementEnvironment::default(),
            deadline: DEFAULT_DEADLINE,
            stdout_limit: DEFAULT_STREAM_LIMIT,
            stderr_limit: DEFAULT_STREAM_LIMIT,
            termination_grace: DEFAULT_TERMINATION_GRACE,
        }
    }

    pub(crate) fn arg(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    pub(crate) fn current_dir(mut self, current_dir: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(current_dir.into());
        self
    }

    pub(crate) fn environment(mut self, environment: ReplacementEnvironment) -> Self {
        self.environment = environment;
        self
    }

    pub(crate) fn deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }

    pub(crate) fn output_limits(mut self, stdout_limit: usize, stderr_limit: usize) -> Self {
        self.stdout_limit = stdout_limit;
        self.stderr_limit = stderr_limit;
        self
    }

    pub(crate) fn termination_grace(mut self, termination_grace: Duration) -> Self {
        self.termination_grace = termination_grace;
        self
    }

    fn validate(&self) -> Result<(), ProcessError> {
        if self.program.is_empty()
            || self.deadline.is_zero()
            || self.deadline > MAX_DEADLINE
            || self.stdout_limit > MAX_STREAM_LIMIT
            || self.stderr_limit > MAX_STREAM_LIMIT
            || self.termination_grace > MAX_TERMINATION_GRACE
        {
            return Err(ProcessError::new(ProcessFailureKind::InvalidConfiguration));
        }
        Ok(())
    }
}

impl fmt::Debug for ProcessRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessRequest")
            .field("program", &"<redacted>")
            .field("argument_count", &self.arguments.len())
            .field(
                "current_dir",
                &self.current_dir.as_ref().map(|_| "<redacted>"),
            )
            .field("environment", &self.environment)
            .field("deadline", &self.deadline)
            .field("stdout_limit", &self.stdout_limit)
            .field("stderr_limit", &self.stderr_limit)
            .field("termination_grace", &self.termination_grace)
            .finish()
    }
}

pub(crate) struct ProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl ProcessOutput {
    pub(crate) fn status(&self) -> ExitStatus {
        self.status
    }

    pub(crate) fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub(crate) fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

impl fmt::Debug for ProcessOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessOutput")
            .field("success", &self.status.success())
            .field("code", &self.status.code())
            .field("stdout_bytes", &self.stdout.len())
            .field("stderr_bytes", &self.stderr.len())
            .field("contents", &"<redacted>")
            .finish()
    }
}

pub(crate) fn run(request: &ProcessRequest) -> Result<ProcessOutput, ProcessError> {
    request.validate()?;
    #[cfg(unix)]
    {
        unix::run(request)
    }
    #[cfg(not(unix))]
    {
        let _ = request;
        Err(ProcessError::new(ProcessFailureKind::UnsupportedPlatform))
    }
}

#[cfg(unix)]
mod unix {
    use super::{
        OutputLimitBreach, ProcessError, ProcessFailureKind, ProcessOutput, ProcessRequest,
    };
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
    use rustix::io::Errno;
    use rustix::process::{Pid, Signal, kill_process_group, test_kill_process_group};
    use std::io::{self, Read};
    use std::os::unix::process::CommandExt;
    use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    const POLL_QUANTUM: Duration = Duration::from_millis(10);
    const FORCE_KILL_GRACE: Duration = Duration::from_secs(1);
    const MAX_DRAIN_BYTES_PER_PASS: usize = 64 * 1024;

    struct Capture {
        bytes: Vec<u8>,
        limit: usize,
        exceeded: bool,
    }

    impl Capture {
        fn new(limit: usize) -> Self {
            Self {
                bytes: Vec::new(),
                limit,
                exceeded: false,
            }
        }

        fn record(&mut self, chunk: &[u8]) -> bool {
            let was_exceeded = self.exceeded;
            let remaining = self.limit.saturating_sub(self.bytes.len());
            let retained = remaining.min(chunk.len());
            self.bytes.extend_from_slice(&chunk[..retained]);
            if retained < chunk.len() {
                self.exceeded = true;
            }
            !was_exceeded && self.exceeded
        }
    }

    enum StopCause {
        Deadline,
        Error(ProcessError),
        OrphanedDescendants,
        OutputLimit,
        RetainedPipes,
    }

    pub(super) fn run(request: &ProcessRequest) -> Result<ProcessOutput, ProcessError> {
        let deadline = Instant::now()
            .checked_add(request.deadline)
            .ok_or_else(|| ProcessError::new(ProcessFailureKind::InvalidConfiguration))?;

        let mut command = Command::new(&request.program);
        command
            .args(&request.arguments)
            .env_clear()
            .envs(&request.environment.entries)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        if let Some(current_dir) = &request.current_dir {
            command.current_dir(current_dir);
        }

        let mut child = command
            .spawn()
            .map_err(|error| ProcessError::from_io(ProcessFailureKind::Spawn, &error))?;
        let process_group = Pid::from_child(&child);
        let mut stdout = child.stdout.take();
        let mut stderr = child.stderr.take();
        let mut stdout_capture = Capture::new(request.stdout_limit);
        let mut stderr_capture = Capture::new(request.stderr_limit);
        let mut status = None;

        if stdout.is_none() || stderr.is_none() {
            let original = ProcessError::new(ProcessFailureKind::PipeSetup);
            return Err(cleanup_or_original(
                original,
                &mut child,
                process_group,
                &mut status,
                &mut stdout,
                &mut stderr,
                &mut stdout_capture,
                &mut stderr_capture,
                request.termination_grace,
            ));
        }
        if let Err(original) = configure_nonblocking(stdout.as_ref(), stderr.as_ref()) {
            return Err(cleanup_or_original(
                original,
                &mut child,
                process_group,
                &mut status,
                &mut stdout,
                &mut stderr,
                &mut stdout_capture,
                &mut stderr_capture,
                request.termination_grace,
            ));
        }

        let stop = loop {
            if let Err(error) = drain_streams(
                &mut stdout,
                &mut stderr,
                &mut stdout_capture,
                &mut stderr_capture,
            ) {
                break StopCause::Error(error);
            }
            if stdout_capture.exceeded || stderr_capture.exceeded {
                break StopCause::OutputLimit;
            }

            match child.try_wait() {
                Ok(Some(exit_status)) => status = Some(exit_status),
                Ok(None) => {}
                Err(error) => {
                    break StopCause::Error(ProcessError::from_io(
                        ProcessFailureKind::Wait,
                        &error,
                    ));
                }
            }

            if status.is_some() {
                match process_group_is_alive(process_group) {
                    Ok(true) => break StopCause::OrphanedDescendants,
                    Ok(false) if stdout.is_none() && stderr.is_none() => {
                        let Some(exit_status) = status else {
                            unreachable!("status was checked above");
                        };
                        return Ok(ProcessOutput {
                            status: exit_status,
                            stdout: stdout_capture.bytes,
                            stderr: stderr_capture.bytes,
                        });
                    }
                    Ok(false) => {}
                    Err(error) => break StopCause::Error(error),
                }
            }

            let now = Instant::now();
            if now >= deadline {
                if status.is_some() {
                    break StopCause::RetainedPipes;
                }
                break StopCause::Deadline;
            }
            let wait = deadline.saturating_duration_since(now).min(POLL_QUANTUM);
            if let Err(error) = poll_streams(stdout.as_ref(), stderr.as_ref(), wait) {
                break StopCause::Error(error);
            }
        };

        let original = match stop {
            StopCause::Deadline => ProcessError::new(ProcessFailureKind::DeadlineExceeded),
            StopCause::Error(error) => error,
            StopCause::OrphanedDescendants => {
                ProcessError::new(ProcessFailureKind::OrphanedDescendants)
            }
            StopCause::OutputLimit => ProcessError::output_limit(limit_breach(
                stdout_capture.exceeded,
                stderr_capture.exceeded,
            )),
            StopCause::RetainedPipes => ProcessError::new(ProcessFailureKind::CleanupIncomplete),
        };
        let mut error = cleanup_or_original(
            original,
            &mut child,
            process_group,
            &mut status,
            &mut stdout,
            &mut stderr,
            &mut stdout_capture,
            &mut stderr_capture,
            request.termination_grace,
        );
        if error.kind == ProcessFailureKind::OutputLimitExceeded {
            error.output_limit = Some(limit_breach(
                stdout_capture.exceeded,
                stderr_capture.exceeded,
            ));
        }
        Err(error)
    }

    fn configure_nonblocking(
        stdout: Option<&ChildStdout>,
        stderr: Option<&ChildStderr>,
    ) -> Result<(), ProcessError> {
        let stdout = stdout.ok_or_else(|| ProcessError::new(ProcessFailureKind::PipeSetup))?;
        let stderr = stderr.ok_or_else(|| ProcessError::new(ProcessFailureKind::PipeSetup))?;
        set_nonblocking(stdout)?;
        set_nonblocking(stderr)
    }

    fn set_nonblocking(fd: impl rustix::fd::AsFd) -> Result<(), ProcessError> {
        let flags = fcntl_getfl(&fd)
            .map_err(|error| ProcessError::from_errno(ProcessFailureKind::PipeSetup, error))?;
        fcntl_setfl(fd, flags | OFlags::NONBLOCK)
            .map_err(|error| ProcessError::from_errno(ProcessFailureKind::PipeSetup, error))
    }

    fn drain_streams(
        stdout: &mut Option<ChildStdout>,
        stderr: &mut Option<ChildStderr>,
        stdout_capture: &mut Capture,
        stderr_capture: &mut Capture,
    ) -> Result<(), ProcessError> {
        drain_stream(stdout, stdout_capture)?;
        drain_stream(stderr, stderr_capture)
    }

    fn drain_stream<Reader: Read>(
        stream: &mut Option<Reader>,
        capture: &mut Capture,
    ) -> Result<(), ProcessError> {
        let Some(reader) = stream.as_mut() else {
            return Ok(());
        };
        let mut buffer = [0_u8; 8192];
        let mut reached_eof = false;
        let mut failure = None;
        let mut bytes_read = 0_usize;
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    reached_eof = true;
                    break;
                }
                Ok(count) => {
                    bytes_read = bytes_read.saturating_add(count);
                    if capture.record(&buffer[..count]) || bytes_read >= MAX_DRAIN_BYTES_PER_PASS {
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    failure = Some(ProcessError::from_io(ProcessFailureKind::Read, &error));
                    break;
                }
            }
        }
        if reached_eof || failure.is_some() {
            *stream = None;
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn poll_streams(
        stdout: Option<&ChildStdout>,
        stderr: Option<&ChildStderr>,
        timeout: Duration,
    ) -> Result<(), ProcessError> {
        let timeout = Timespec::try_from(timeout)
            .map_err(|_| ProcessError::new(ProcessFailureKind::InvalidConfiguration))?;
        let result = match (stdout, stderr) {
            (Some(stdout), Some(stderr)) => {
                let mut poll_fds = [
                    PollFd::new(stdout, PollFlags::IN),
                    PollFd::new(stderr, PollFlags::IN),
                ];
                poll(&mut poll_fds, Some(&timeout))
            }
            (Some(stdout), None) => {
                let mut poll_fds = [PollFd::new(stdout, PollFlags::IN)];
                poll(&mut poll_fds, Some(&timeout))
            }
            (None, Some(stderr)) => {
                let mut poll_fds = [PollFd::new(stderr, PollFlags::IN)];
                poll(&mut poll_fds, Some(&timeout))
            }
            (None, None) => {
                let mut poll_fds = [];
                poll(&mut poll_fds, Some(&timeout))
            }
        };
        match result {
            Ok(_) | Err(Errno::INTR) => Ok(()),
            Err(error) => Err(ProcessError::from_errno(ProcessFailureKind::Poll, error)),
        }
    }

    fn process_group_is_alive(process_group: Pid) -> Result<bool, ProcessError> {
        match test_kill_process_group(process_group) {
            Ok(()) | Err(Errno::PERM) => Ok(true),
            Err(Errno::SRCH) => Ok(false),
            Err(error) => Err(ProcessError::from_errno(ProcessFailureKind::Signal, error)),
        }
    }

    fn signal_process_group(process_group: Pid, signal: Signal) -> Result<(), ProcessError> {
        match kill_process_group(process_group, signal) {
            // macOS may report EPERM while the just-exited leader is being
            // reaped. Cleanup still proves the group absent below; a group
            // that remains present is a hard CleanupIncomplete failure.
            Ok(()) | Err(Errno::SRCH | Errno::PERM) => Ok(()),
            Err(error) => Err(ProcessError::from_errno(ProcessFailureKind::Signal, error)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn cleanup_or_original(
        original: ProcessError,
        child: &mut Child,
        process_group: Pid,
        status: &mut Option<ExitStatus>,
        stdout: &mut Option<ChildStdout>,
        stderr: &mut Option<ChildStderr>,
        stdout_capture: &mut Capture,
        stderr_capture: &mut Capture,
        termination_grace: Duration,
    ) -> ProcessError {
        match cleanup(
            child,
            process_group,
            status,
            stdout,
            stderr,
            stdout_capture,
            stderr_capture,
            termination_grace,
        ) {
            Ok(()) => original,
            Err(error) => error,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn cleanup(
        child: &mut Child,
        process_group: Pid,
        status: &mut Option<ExitStatus>,
        stdout: &mut Option<ChildStdout>,
        stderr: &mut Option<ChildStderr>,
        stdout_capture: &mut Capture,
        stderr_capture: &mut Capture,
        termination_grace: Duration,
    ) -> Result<(), ProcessError> {
        let mut first_error = signal_process_group(process_group, Signal::TERM).err();
        let term_deadline = Instant::now()
            .checked_add(termination_grace)
            .unwrap_or_else(Instant::now);
        pump_cleanup_until(
            term_deadline,
            child,
            process_group,
            status,
            stdout,
            stderr,
            stdout_capture,
            stderr_capture,
            &mut first_error,
        );

        let group_alive = process_group_is_alive(process_group).unwrap_or(true);
        if group_alive || status.is_none() {
            remember_first(
                &mut first_error,
                signal_process_group(process_group, Signal::KILL).err(),
            );
            if status.is_none() {
                let _ = child.kill();
            }
        }
        let kill_deadline = Instant::now()
            .checked_add(FORCE_KILL_GRACE)
            .unwrap_or_else(Instant::now);
        pump_cleanup_until(
            kill_deadline,
            child,
            process_group,
            status,
            stdout,
            stderr,
            stdout_capture,
            stderr_capture,
            &mut first_error,
        );

        let final_group_alive = match process_group_is_alive(process_group) {
            Ok(alive) => alive,
            Err(error) => {
                remember_first(&mut first_error, Some(error));
                true
            }
        };
        if status.is_none() || final_group_alive || stdout.is_some() || stderr.is_some() {
            *stdout = None;
            *stderr = None;
            return Err(ProcessError::new(ProcessFailureKind::CleanupIncomplete));
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn pump_cleanup_until(
        deadline: Instant,
        child: &mut Child,
        process_group: Pid,
        status: &mut Option<ExitStatus>,
        stdout: &mut Option<ChildStdout>,
        stderr: &mut Option<ChildStderr>,
        stdout_capture: &mut Capture,
        stderr_capture: &mut Capture,
        first_error: &mut Option<ProcessError>,
    ) {
        loop {
            if let Err(error) = drain_streams(stdout, stderr, stdout_capture, stderr_capture) {
                remember_first(first_error, Some(error));
            }
            if status.is_none() {
                match child.try_wait() {
                    Ok(Some(exit_status)) => *status = Some(exit_status),
                    Ok(None) => {}
                    Err(error) => remember_first(
                        first_error,
                        Some(ProcessError::from_io(ProcessFailureKind::Wait, &error)),
                    ),
                }
            }
            let group_alive = match process_group_is_alive(process_group) {
                Ok(alive) => alive,
                Err(error) => {
                    remember_first(first_error, Some(error));
                    true
                }
            };
            if status.is_some() && !group_alive && stdout.is_none() && stderr.is_none() {
                return;
            }

            let now = Instant::now();
            if now >= deadline {
                return;
            }
            let wait = deadline.saturating_duration_since(now).min(POLL_QUANTUM);
            if let Err(error) = poll_streams(stdout.as_ref(), stderr.as_ref(), wait) {
                remember_first(first_error, Some(error));
                thread::sleep(wait);
            }
        }
    }

    fn remember_first(slot: &mut Option<ProcessError>, candidate: Option<ProcessError>) {
        if slot.is_none() {
            *slot = candidate;
        }
    }

    fn limit_breach(stdout_exceeded: bool, stderr_exceeded: bool) -> OutputLimitBreach {
        match (stdout_exceeded, stderr_exceeded) {
            (true, true) => OutputLimitBreach::Both,
            (true, false) => OutputLimitBreach::Stdout,
            (false, true) => OutputLimitBreach::Stderr,
            (false, false) => unreachable!("output stop requires an exceeded stream"),
        }
    }
}

pub(crate) fn self_test() -> Result<(), String> {
    #[cfg(unix)]
    {
        self_test_suite()?;
        println!("bounded process self-test: ok");
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Err(ProcessError::new(ProcessFailureKind::UnsupportedPlatform).to_string())
    }
}

#[cfg(unix)]
fn self_test_suite() -> Result<(), String> {
    verify_replacement_environment()?;
    verify_closed_stdin()?;
    verify_environment_rejections()?;
    verify_deadline_tree_kill()?;
    verify_orphan_kill()?;
    verify_dual_output_caps()?;
    verify_continuous_output_is_bounded()?;
    verify_redaction()?;
    Ok(())
}

#[cfg(unix)]
fn verify_replacement_environment() -> Result<(), String> {
    let mut environment = ReplacementEnvironment::default();
    environment
        .insert("RSHR_VISIBLE", "replacement-only")
        .map_err(|error| error.to_string())?;
    let output = run(&ProcessRequest::new("/usr/bin/env")
        .environment(environment)
        .deadline(Duration::from_secs(2))
        .output_limits(4096, 4096))
    .map_err(|error| error.to_string())?;
    if !output.status().success()
        || output.stdout() != b"RSHR_VISIBLE=replacement-only\n"
        || !output.stderr().is_empty()
    {
        return Err("replacement environment self-test failed".to_owned());
    }
    Ok(())
}

#[cfg(unix)]
fn verify_closed_stdin() -> Result<(), String> {
    let working_directory =
        tempfile::tempdir().map_err(|_| "create self-test directory".to_owned())?;
    let output = run(&ProcessRequest::new("/bin/sh")
        .arg("-c")
        .arg("if IFS= read -r value; then exit 92; fi; printf 'stdin-closed'")
        .current_dir(working_directory.path())
        .deadline(Duration::from_secs(2)))
    .map_err(|error| error.to_string())?;
    if !output.status().success() || output.stdout() != b"stdin-closed" {
        return Err("closed stdin self-test failed".to_owned());
    }
    Ok(())
}

#[cfg(unix)]
fn verify_environment_rejections() -> Result<(), String> {
    for name in FORBIDDEN_ENVIRONMENT_NAMES {
        expect_environment_rejection(name, EnvironmentRejection::ForbiddenControl)?;
    }
    for name in [
        "CARGO_BUILD_RUSTFLAGS",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
        "DYLD_FRAMEWORK_PATH",
        "LD_AUDIT",
        "LD_DEBUG",
        "LD_PROFILE",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTDOCFLAGS",
    ] {
        expect_environment_rejection(name, EnvironmentRejection::ForbiddenControl)?;
    }
    for name in [
        "CREDENTIAL",
        "APIKEY",
        "PASSWORD",
        "SECRET",
        "TOKEN",
        "RSHR_CREDENTIAL_FILE",
        "RSHR_KEY_ID",
        "RSHR_PASSWORD_FILE",
        "RSHR_SECRET_VALUE",
        "RSHR_TOKEN_FILE",
    ] {
        expect_environment_rejection(name, EnvironmentRejection::SensitiveName)?;
    }
    Ok(())
}

#[cfg(unix)]
fn expect_environment_rejection(name: &str, expected: EnvironmentRejection) -> Result<(), String> {
    let mut environment = ReplacementEnvironment::default();
    let error = environment
        .insert(name, "redacted-value")
        .expect_err("forbidden environment entry must fail closed");
    if error.kind() != ProcessFailureKind::EnvironmentRejected(expected) {
        return Err("environment rejection self-test failed".to_owned());
    }
    Ok(())
}

#[cfg(unix)]
fn verify_deadline_tree_kill() -> Result<(), String> {
    let pid_file =
        tempfile::NamedTempFile::new().map_err(|_| "create self-test pid file".to_owned())?;
    let request = ProcessRequest::new("/bin/sh")
        .arg("-c")
        .arg(
            "trap 'kill \"$child\" 2>/dev/null; wait \"$child\" 2>/dev/null; exit 143' TERM; \
             /bin/sleep 30 & child=$!; printf '%s\n' \"$child\" > \"$1\"; wait \"$child\"",
        )
        .arg("rshr-bounded-process")
        .arg(pid_file.path())
        .deadline(Duration::from_millis(750))
        .termination_grace(Duration::from_millis(100));
    let error = run(&request).expect_err("deadline self-test must time out");
    if error.kind() != ProcessFailureKind::DeadlineExceeded {
        return Err("deadline classification self-test failed".to_owned());
    }
    verify_recorded_process_gone(pid_file.path())
}

#[cfg(unix)]
fn verify_orphan_kill() -> Result<(), String> {
    let pid_file =
        tempfile::NamedTempFile::new().map_err(|_| "create self-test pid file".to_owned())?;
    let request = ProcessRequest::new("/bin/sh")
        .arg("-c")
        .arg("/bin/sleep 30 & child=$!; printf '%s\n' \"$child\" > \"$1\"; exit 0")
        .arg("rshr-bounded-process")
        .arg(pid_file.path())
        .deadline(Duration::from_secs(2))
        .termination_grace(Duration::from_millis(100));
    let error = run(&request).expect_err("orphan self-test must fail closed");
    if error.kind() != ProcessFailureKind::OrphanedDescendants {
        return Err("orphan classification self-test failed".to_owned());
    }
    verify_recorded_process_gone(pid_file.path())
}

#[cfg(unix)]
fn verify_recorded_process_gone(path: &std::path::Path) -> Result<(), String> {
    use rustix::io::Errno;
    use rustix::process::{Pid, test_kill_process};
    use std::fs;
    use std::thread;
    use std::time::Instant;

    let raw = fs::read_to_string(path).map_err(|_| "read self-test pid file".to_owned())?;
    let pid_number = raw
        .trim()
        .parse::<i32>()
        .map_err(|_| "parse self-test pid".to_owned())?;
    let pid =
        Pid::from_raw(pid_number).ok_or_else(|| "self-test pid must be positive".to_owned())?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(2))
        .ok_or_else(|| "self-test pid deadline overflow".to_owned())?;
    loop {
        match test_kill_process(pid) {
            Err(Errno::SRCH) => return Ok(()),
            Ok(()) | Err(Errno::PERM) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            _ => return Err("bounded process descendant survived cleanup".to_owned()),
        }
    }
}

#[cfg(unix)]
fn verify_dual_output_caps() -> Result<(), String> {
    let stdout_error = run(&ProcessRequest::new("/bin/sh")
        .arg("-c")
        .arg("printf 'stdout-over-cap'")
        .deadline(Duration::from_secs(2))
        .output_limits(4, 4096))
    .expect_err("stdout cap self-test must fail closed");
    if stdout_error.output_limit_breach() != Some(OutputLimitBreach::Stdout) {
        return Err(format!("stdout cap self-test failed: {stdout_error:?}"));
    }

    let stderr_error = run(&ProcessRequest::new("/bin/sh")
        .arg("-c")
        .arg("printf 'stderr-over-cap' >&2")
        .deadline(Duration::from_secs(2))
        .output_limits(4096, 4))
    .expect_err("stderr cap self-test must fail closed");
    if stderr_error.output_limit_breach() != Some(OutputLimitBreach::Stderr) {
        return Err(format!("stderr cap self-test failed: {stderr_error:?}"));
    }
    Ok(())
}

#[cfg(unix)]
fn verify_continuous_output_is_bounded() -> Result<(), String> {
    let error = run(&ProcessRequest::new("/bin/sh")
        .arg("-c")
        .arg("while :; do printf 'continuous-output'; done")
        .deadline(Duration::from_secs(2))
        .output_limits(4096, 4096)
        .termination_grace(Duration::from_millis(100)))
    .expect_err("continuous output must hit its live cap");
    if error.output_limit_breach() != Some(OutputLimitBreach::Stdout) {
        return Err("continuous output cap self-test failed".to_owned());
    }
    Ok(())
}

#[cfg(unix)]
fn verify_redaction() -> Result<(), String> {
    const ARGUMENT_MARKER: &str = "argument-marker-must-not-leak";
    const CURRENT_DIR_MARKER: &str = "/cwd-marker-must-not-leak";
    const ENVIRONMENT_MARKER: &str = "environment-marker-must-not-leak";
    const PROGRAM_MARKER: &str = "/program-marker-must-not-leak";

    let mut environment = ReplacementEnvironment::default();
    environment
        .insert("RSHR_VISIBLE", ENVIRONMENT_MARKER)
        .map_err(|error| error.to_string())?;
    let diagnostics_request = ProcessRequest::new(PROGRAM_MARKER)
        .arg(ARGUMENT_MARKER)
        .current_dir(CURRENT_DIR_MARKER)
        .environment(environment.clone());
    let request_debug = format!("{diagnostics_request:?}");
    if [
        ARGUMENT_MARKER,
        CURRENT_DIR_MARKER,
        ENVIRONMENT_MARKER,
        PROGRAM_MARKER,
    ]
    .iter()
    .any(|marker| request_debug.contains(marker))
    {
        return Err("request redaction self-test failed".to_owned());
    }
    let spawn_error = run(&diagnostics_request)
        .expect_err("missing redaction fixture program must fail to spawn");
    let spawn_debug = format!("{spawn_error:?}");
    let spawn_display = spawn_error.to_string();
    if [
        ARGUMENT_MARKER,
        CURRENT_DIR_MARKER,
        ENVIRONMENT_MARKER,
        PROGRAM_MARKER,
    ]
    .iter()
    .any(|marker| spawn_debug.contains(marker) || spawn_display.contains(marker))
    {
        return Err("spawn error redaction self-test failed".to_owned());
    }

    let request = ProcessRequest::new("/bin/sh")
        .arg("-c")
        .arg("printf '%s' \"$RSHR_VISIBLE\"; printf '%s' \"$1\" >&2")
        .arg("rshr-bounded-process")
        .arg(ARGUMENT_MARKER)
        .environment(environment)
        .deadline(Duration::from_secs(2));

    let output = run(&request).map_err(|error| error.to_string())?;
    if output.stdout() != ENVIRONMENT_MARKER.as_bytes()
        || output.stderr() != ARGUMENT_MARKER.as_bytes()
    {
        return Err("redaction fixture output self-test failed".to_owned());
    }
    let output_debug = format!("{output:?}");
    if output_debug.contains(ARGUMENT_MARKER) || output_debug.contains(ENVIRONMENT_MARKER) {
        return Err("output redaction self-test failed".to_owned());
    }

    let capped = run(&ProcessRequest::new("/bin/sh")
        .arg("-c")
        .arg(format!("printf '{ARGUMENT_MARKER}'"))
        .output_limits(1, 1))
    .expect_err("redaction cap fixture must fail closed");
    let error_debug = format!("{capped:?}");
    let error_display = capped.to_string();
    if error_debug.contains(ARGUMENT_MARKER) || error_display.contains(ARGUMENT_MARKER) {
        return Err("error redaction self-test failed".to_owned());
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn replacement_environment_is_exact_and_stdin_is_closed() {
        verify_replacement_environment().expect("replacement environment must be exact");
        verify_closed_stdin().expect("stdin must be closed");
    }

    #[test]
    fn build_loader_and_sensitive_environment_controls_are_rejected() {
        verify_environment_rejections().expect("environment controls must fail closed");
    }

    #[test]
    fn deadline_kills_the_process_tree() {
        verify_deadline_tree_kill().expect("deadline must kill the process group");
    }

    #[test]
    fn leader_exit_kills_orphaned_descendants() {
        verify_orphan_kill().expect("orphaned process group must be killed");
    }

    #[test]
    fn stdout_and_stderr_have_independent_caps() {
        verify_dual_output_caps().expect("both output streams must be capped");
        verify_continuous_output_is_bounded().expect("continuous output must remain bounded");
    }

    #[test]
    fn request_output_and_error_diagnostics_are_redacted() {
        verify_redaction().expect("bounded process diagnostics must be redacted");
    }

    #[test]
    fn environment_boundaries_fail_closed() {
        use std::os::unix::ffi::OsStringExt;

        let mut environment = ReplacementEnvironment::default();
        environment
            .insert("RSHR_VISIBLE", "first")
            .expect("first entry must be accepted");
        let duplicate = environment
            .insert("RSHR_VISIBLE", "second")
            .expect_err("duplicate must fail");
        assert_eq!(
            duplicate.kind(),
            ProcessFailureKind::EnvironmentRejected(EnvironmentRejection::DuplicateName)
        );

        let mut nul_environment = ReplacementEnvironment::default();
        let nul = nul_environment
            .insert("RSHR_VISIBLE", OsString::from_vec(b"value\0tail".to_vec()))
            .expect_err("NUL value must fail");
        assert_eq!(
            nul.kind(),
            ProcessFailureKind::EnvironmentRejected(EnvironmentRejection::InvalidNameOrNul)
        );

        let mut full_environment = ReplacementEnvironment::default();
        for index in 0..MAX_ENVIRONMENT_ENTRIES {
            full_environment
                .insert(format!("RSHR_ENTRY_{index}"), "value")
                .expect("entry within cardinality bound must pass");
        }
        let too_many = full_environment
            .insert("RSHR_ENTRY_OVER_LIMIT", "value")
            .expect_err("entry above cardinality bound must fail");
        assert_eq!(
            too_many.kind(),
            ProcessFailureKind::EnvironmentRejected(EnvironmentRejection::TooManyEntries)
        );

        let mut large_environment = ReplacementEnvironment::default();
        let too_large = large_environment
            .insert(
                "RSHR_VISIBLE",
                OsString::from_vec(vec![b'x'; MAX_ENVIRONMENT_VALUE_BYTES + 1]),
            )
            .expect_err("value above byte bound must fail");
        assert_eq!(
            too_large.kind(),
            ProcessFailureKind::EnvironmentRejected(EnvironmentRejection::ValueTooLarge)
        );
    }

    #[test]
    fn hard_configuration_maximums_fail_before_spawn() {
        for request in [
            ProcessRequest::new("/usr/bin/true").deadline(MAX_DEADLINE + Duration::from_secs(1)),
            ProcessRequest::new("/usr/bin/true")
                .output_limits(MAX_STREAM_LIMIT + 1, MAX_STREAM_LIMIT),
            ProcessRequest::new("/usr/bin/true")
                .output_limits(MAX_STREAM_LIMIT, MAX_STREAM_LIMIT + 1),
            ProcessRequest::new("/usr/bin/true")
                .termination_grace(MAX_TERMINATION_GRACE + Duration::from_millis(1)),
        ] {
            let error = run(&request).expect_err("configuration above hard maximum must fail");
            assert_eq!(error.kind(), ProcessFailureKind::InvalidConfiguration);
        }
    }

    #[test]
    fn exact_and_zero_stream_caps_have_byte_precise_semantics() {
        let exact = run(&ProcessRequest::new("/bin/sh")
            .arg("-c")
            .arg("printf '1234'; printf 'abcd' >&2")
            .output_limits(4, 4))
        .expect("exact cap must pass");
        assert_eq!(exact.stdout(), b"1234");
        assert_eq!(exact.stderr(), b"abcd");

        let empty = run(&ProcessRequest::new("/usr/bin/true")
            .output_limits(0, 0)
            .deadline(Duration::from_secs(2)))
        .expect("empty streams must fit zero caps");
        assert!(empty.stdout().is_empty());
        assert!(empty.stderr().is_empty());

        let over_zero = run(&ProcessRequest::new("/bin/sh")
            .arg("-c")
            .arg("printf x")
            .output_limits(0, 0))
        .expect_err("first byte above zero cap must fail");
        assert_eq!(
            over_zero.output_limit_breach(),
            Some(OutputLimitBreach::Stdout)
        );
    }
}

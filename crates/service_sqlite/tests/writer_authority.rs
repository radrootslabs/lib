#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::{
    env,
    error::Error,
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        process::ExitStatusExt,
    },
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use radroots_runtime_paths::{
    InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
    RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
};
use radroots_service_sqlite::{
    OpenMode, ServiceSqliteErrorKind, ServiceSqlitePaths, WriterAuthority,
};

const CHILD_ROOT: &str = "RADROOTS_SERVICE_SQLITE_WRITER_CHILD_ROOT";
const PROCESS_READY: &str = "RSHR_STEP073_WRITER_READY";
const MAX_CHILD_INPUT_BYTES: u64 = 4_096;

fn paths(root: PathBuf) -> ServiceSqlitePaths {
    let context = RuntimeContext::resolve(
        &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
        RuntimeContextBootstrap::new(
            RadrootsPathProfile::RepoLocal,
            Some(root),
            RuntimeContextSource::BootstrapCli,
            RuntimeContextSource::BootstrapCli,
        )
        .expect("bootstrap"),
        ServiceId::new("myc").expect("service"),
        InstanceId::new("process-contention").expect("instance"),
    )
    .expect("runtime context");
    ServiceSqlitePaths::from_runtime_context(&context).expect("SQLite paths")
}

#[test]
fn writer_authority_is_exclusive_across_processes() {
    let root = tempfile::tempdir().expect("root");
    let paths = paths(root.path().to_path_buf());
    fs::create_dir_all(paths.state_lock().parent().expect("state directory"))
        .expect("create state directory");
    let _authority = WriterAuthority::acquire(&paths, OpenMode::Initialize)
        .expect("parent authority")
        .expect("writer capability");

    let status = Command::new(env::current_exe().expect("test executable"))
        .arg("--exact")
        .arg("writer_authority_child_probe")
        .arg("--nocapture")
        .env(CHILD_ROOT, root.path())
        .status()
        .expect("child probe");
    assert!(status.success(), "child must observe writer contention");
}

#[test]
fn writer_authority_child_probe() {
    let Some(root) = env::var_os(CHILD_ROOT) else {
        return;
    };
    let error = WriterAuthority::acquire(&paths(PathBuf::from(root)), OpenMode::Initialize)
        .expect_err("parent writer must remain authoritative");
    assert_eq!(error.kind(), ServiceSqliteErrorKind::Authority);
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("another SQLite writer is active")
    );
}

#[test]
#[ignore = "private process child invoked by the parent integration test"]
fn writer_authority_holder_child_probe() {
    let root = child_root_from_stdin();
    rustix::process::umask(rustix::fs::Mode::empty());
    let _authority = WriterAuthority::acquire(&paths(root), OpenMode::Initialize)
        .expect("child authority")
        .expect("writer capability");
    println!("\n{PROCESS_READY}");
    std::io::stdout().flush().expect("flush readiness");
    loop {
        thread::park();
    }
}

#[test]
fn sigkill_releases_process_writer_lock_and_preserves_lock_permissions() {
    let root = tempfile::tempdir().expect("root");
    let paths = paths(root.path().to_path_buf());
    let state_directory = paths.state_lock().parent().expect("state directory");
    fs::create_dir_all(state_directory).expect("create state directory");
    fs::set_permissions(state_directory, fs::Permissions::from_mode(0o700))
        .expect("restrict state directory");
    let mut child = Command::new(env::current_exe().expect("test executable"))
        .arg("--ignored")
        .arg("--exact")
        .arg("writer_authority_holder_child_probe")
        .arg("--nocapture")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn authority holder");
    writeln!(
        child.stdin.take().expect("child stdin"),
        "{}",
        root.path().display()
    )
    .expect("send child root");
    let ready = readiness_receiver(child.stdout.take().expect("child stdout"));
    let mut child = KillOnDrop::new(child);
    wait_for_ready(&mut child, &ready);
    let contended = WriterAuthority::acquire(&paths, OpenMode::Initialize)
        .expect_err("live child retains writer authority");
    assert_eq!(contended.kind(), ServiceSqliteErrorKind::Authority);
    assert_lock_permissions(&paths);

    rustix::process::kill_process(
        rustix::process::Pid::from_child(child.child()),
        rustix::process::Signal::KILL,
    )
    .expect("SIGKILL authority holder");
    let status = child.wait().expect("wait for killed authority holder");
    assert_eq!(status.signal(), Some(9));

    let mut recovered = WriterAuthority::acquire(&paths, OpenMode::Initialize)
        .expect("authority after process death")
        .expect("writer capability");
    assert_lock_permissions(&paths);
    recovered.release().expect("release recovered authority");
}

fn child_root_from_stdin() -> PathBuf {
    let mut input = String::new();
    std::io::stdin()
        .take(MAX_CHILD_INPUT_BYTES + 1)
        .read_to_string(&mut input)
        .expect("read child root");
    assert!(
        input.len() <= MAX_CHILD_INPUT_BYTES as usize,
        "bounded child input"
    );
    let root = input
        .strip_suffix('\n')
        .expect("newline-terminated child root");
    assert!(
        !root.is_empty() && !root.contains('\n'),
        "single child root"
    );
    PathBuf::from(root)
}

fn readiness_receiver(stdout: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line.unwrap_or_default()).is_err() {
                break;
            }
        }
    });
    receiver
}

fn wait_for_ready(child: &mut KillOnDrop, ready: &mpsc::Receiver<String>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match ready.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) if line == PROCESS_READY => return,
            Ok(_) => {}
            Err(error) => {
                let status = child.child_mut().try_wait().expect("query child status");
                panic!("writer child readiness failed ({error}); status: {status:?}");
            }
        }
    }
}

fn assert_lock_permissions(paths: &ServiceSqlitePaths) {
    let metadata = fs::metadata(paths.state_lock()).expect("lock metadata");
    assert!(metadata.file_type().is_file());
    assert_eq!(metadata.mode() & 0o777, 0o600);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
}

struct KillOnDrop {
    child: Child,
    finished: bool,
}

impl KillOnDrop {
    fn new(child: Child) -> Self {
        Self {
            child,
            finished: false,
        }
    }

    fn child(&self) -> &Child {
        &self.child
    }

    fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        let status = self.child.wait()?;
        self.finished = true;
        Ok(status)
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

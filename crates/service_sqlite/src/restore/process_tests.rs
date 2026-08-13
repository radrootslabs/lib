use core::num::{NonZeroU32, NonZeroU64};
use std::{
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        process::ExitStatusExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use radroots_runtime_paths::{
    InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
    RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
};
use radroots_storage::event::SourceGeneration;
use sha2::{Digest, Sha256};
use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};

use super::{
    BACKUP_FILE_NAME, MARKER_FILE_NAME, MARKER_NEXT_FILE_NAME, STAGED_FILE_NAME,
    finalize::test_finalize_with_failpoint,
};
use crate::failpoint::{DurabilityFailpoint, DurabilityFailpoints};
use crate::{
    BackupCreatedAtUnixMs, BackupMemberSha256, MigrationAppliedAtUnixSeconds,
    MigrationBuildIdentity, MigrationCatalog, OpenMode, SchemaCatalog, SchemaVersionCatalog,
    ServiceBackupManifest, ServiceDatabaseIdentity, ServiceDatabaseMetadata,
    ServiceSqliteApplicationId, ServiceSqliteConnectionOptions, ServiceSqliteErrorKind,
    ServiceSqliteHost, ServiceSqlitePaths, initialize_database, stage_verified_restore,
    verify_backup_bundle,
};

const PROCESS_READY: &str = "RSHR_STEP073_READY";
const MANIFEST_FILE_NAME: &str = "process-restore-manifest.v1.json";
const BUNDLE_DIRECTORY_NAME: &str = "process-restore-bundle";
const MAX_CHILD_INPUT_BYTES: u64 = 4_096;
const REPLACEMENT_USER_VERSION: i64 = 73;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);

struct Fixture {
    root: tempfile::TempDir,
    paths: ServiceSqlitePaths,
    identity: ServiceDatabaseIdentity,
    migrations: MigrationCatalog,
    schema: SchemaCatalog,
}

impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().expect("process fixture root");
        let paths = paths(root.path());
        let state_directory = paths.state_database().parent().expect("state directory");
        fs::create_dir_all(state_directory).expect("create state directory");
        fs::set_permissions(state_directory, fs::Permissions::from_mode(0o700))
            .expect("restrict state directory");
        let metadata = metadata(&paths);
        let (migrations, schema) = catalogs();
        let mut authority = initialize_database(
            &paths,
            OpenMode::Initialize,
            &metadata,
            &schema,
            |path| async move {
                let options = SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(false)
                    .disable_statement_logging();
                let connection = sqlx::SqliteConnection::connect_with(&options).await?;
                connection.close().await
            },
        )
        .await
        .expect("initialize process database");
        authority
            .release()
            .expect("release initialization authority");
        {
            let connection = rusqlite::Connection::open(paths.state_database())
                .expect("open live database for WAL posture");
            connection
                .pragma_update(None, "journal_mode", "WAL")
                .expect("set WAL posture");
            connection
                .pragma_update(None, "wal_checkpoint", "TRUNCATE")
                .expect("checkpoint WAL posture");
        }

        let bundle = root.path().join(BUNDLE_DIRECTORY_NAME);
        fs::create_dir(&bundle).expect("create process bundle");
        fs::set_permissions(&bundle, fs::Permissions::from_mode(0o700))
            .expect("restrict process bundle");
        let member = bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
        fs::copy(paths.state_database(), &member).expect("copy process member");
        fs::set_permissions(&member, fs::Permissions::from_mode(0o600))
            .expect("restrict process member");
        {
            let connection = rusqlite::Connection::open(&member).expect("open process member");
            connection
                .pragma_update(None, "user_version", REPLACEMENT_USER_VERSION)
                .expect("set replacement probe");
            connection
                .pragma_update(None, "wal_checkpoint", "TRUNCATE")
                .expect("checkpoint replacement probe");
        }
        let bytes = fs::read(&member).expect("read process member");
        let manifest = ServiceBackupManifest::from_capture(
            &metadata,
            BackupCreatedAtUnixMs::new(1_700_000_073_000).expect("capture time"),
            u64::try_from(bytes.len()).expect("member length"),
            BackupMemberSha256::from_bytes(Sha256::digest(&bytes).into()),
        )
        .expect("process manifest");
        fs::write(
            root.path().join(MANIFEST_FILE_NAME),
            manifest.canonical_bytes(),
        )
        .expect("write process manifest");

        let identity = metadata.identity();
        Self {
            root,
            paths,
            identity,
            migrations,
            schema,
        }
    }

    fn root(&self) -> &Path {
        self.root.path()
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "private process child invoked by the parent crash test"]
async fn child_before_prepared_marker() {
    run_child(DurabilityFailpoint::MarkerBeforeCreate, 1).await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "private process child invoked by the parent crash test"]
async fn child_after_prepared_marker() {
    run_child(DurabilityFailpoint::MarkerAfterDirectorySync, 1).await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "private process child invoked by the parent crash test"]
async fn child_after_live_retained_scratch_sync() {
    run_child(DurabilityFailpoint::MarkerAdvanceAfterWriteAndFileSync, 1).await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "private process child invoked by the parent crash test"]
async fn child_after_replacement_install_sync() {
    run_child(DurabilityFailpoint::RestoreAfterInstallStageSync, 1).await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "private process child invoked by the parent crash test"]
async fn child_after_terminal_marker_sync() {
    run_child(DurabilityFailpoint::MarkerAdvanceAfterDirectorySync, 2).await;
}

async fn run_child(point: DurabilityFailpoint, occurrence: u8) {
    let root = child_root_from_stdin();
    rustix::process::umask(rustix::fs::Mode::empty());
    let paths = paths(&root);
    let metadata = metadata(&paths);
    let identity = metadata.identity();
    let (migrations, schema) = catalogs();
    let manifest = ServiceBackupManifest::from_canonical_bytes(
        &fs::read(root.join(MANIFEST_FILE_NAME)).expect("read child manifest"),
    )
    .expect("parse child manifest");
    let verified = verify_backup_bundle(
        manifest.canonical_bytes(),
        manifest.digest(),
        &root.join(BUNDLE_DIRECTORY_NAME),
        &identity,
        NonZeroU64::new(16 * 1_024 * 1_024).expect("member limit"),
    )
    .expect("verify child bundle");
    let staged = stage_verified_restore(&paths, &identity, &migrations, &schema, verified)
        .await
        .expect("stage child restore");
    let failpoints = DurabilityFailpoints::process_barrier(point, occurrence);
    let result = test_finalize_with_failpoint(staged, failpoints).await;
    panic!("process durability barrier returned before SIGKILL: {result:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn sigkill_restore_boundaries_recover_exact_topologies_and_preserve_permissions() {
    for scenario in [
        Scenario::unresolved("child_before_prepared_marker"),
        Scenario::recovered("child_after_prepared_marker", 0),
        Scenario::recovered(
            "child_after_live_retained_scratch_sync",
            REPLACEMENT_USER_VERSION,
        ),
        Scenario::recovered(
            "child_after_replacement_install_sync",
            REPLACEMENT_USER_VERSION,
        ),
        Scenario::recovered("child_after_terminal_marker_sync", REPLACEMENT_USER_VERSION),
    ] {
        run_parent_scenario(scenario).await;
    }
}

struct Scenario {
    child_name: &'static str,
    expected_user_version: Option<i64>,
}

impl Scenario {
    const fn unresolved(child_name: &'static str) -> Self {
        Self {
            child_name,
            expected_user_version: None,
        }
    }

    const fn recovered(child_name: &'static str, expected_user_version: i64) -> Self {
        Self {
            child_name,
            expected_user_version: Some(expected_user_version),
        }
    }
}

async fn run_parent_scenario(scenario: Scenario) {
    let fixture = Fixture::new().await;
    let original_live = file_snapshot(fixture.paths.state_database());
    let mut child = spawn_restore_child(scenario.child_name, fixture.root());
    wait_for_ready(&mut child);
    assert_recovery_permissions(&fixture.paths);
    rustix::process::kill_process(
        rustix::process::Pid::from_child(child.child()),
        rustix::process::Signal::KILL,
    )
    .expect("SIGKILL restore child");
    let status = child.wait().expect("wait for restore child");
    assert_eq!(status.signal(), Some(9), "scenario {}", scenario.child_name);
    assert_recovery_permissions(&fixture.paths);

    if let Some(expected_user_version) = scenario.expected_user_version {
        let (host, outcome) = ServiceSqliteHost::open_read_write_existing(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            ServiceSqliteConnectionOptions::reviewed(),
            MigrationAppliedAtUnixSeconds::new(1_700_000_073).expect("migration time"),
            &build_identity(),
            &[],
        )
        .await
        .expect("reopen and reconcile interrupted restore");
        assert_eq!(outcome.applied_count(), 0);
        host.close().await.expect("close recovered host");
        assert_no_recovery_evidence(&fixture.paths);
        assert_eq!(database_user_version(&fixture.paths), expected_user_version);
        assert_live_permissions(&fixture.paths);
    } else {
        let staged = recovery_path(&fixture.paths, STAGED_FILE_NAME);
        let staged_before = file_snapshot(&staged);
        let error = ServiceSqliteHost::open_read_write_existing(
            &fixture.paths,
            &fixture.identity,
            &fixture.migrations,
            &fixture.schema,
            ServiceSqliteConnectionOptions::reviewed(),
            MigrationAppliedAtUnixSeconds::new(1_700_000_073).expect("migration time"),
            &build_identity(),
            &[],
        )
        .await
        .expect_err("orphan stage without a marker must fail closed");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Recovery);
        assert_eq!(file_snapshot(fixture.paths.state_database()), original_live);
        assert_eq!(file_snapshot(&staged), staged_before);
        assert!(!recovery_path(&fixture.paths, MARKER_FILE_NAME).exists());
    }
}

fn spawn_restore_child(child_name: &str, root: &Path) -> KillOnDrop {
    let exact_name = format!("restore::process_tests::{child_name}");
    let mut child = Command::new(env::current_exe().expect("test executable"))
        .arg("--ignored")
        .arg("--exact")
        .arg(exact_name)
        .arg("--nocapture")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn restore child");
    writeln!(
        child.stdin.take().expect("child stdin"),
        "{}",
        root.display()
    )
    .expect("send child root");
    let ready = readiness_receiver(child.stdout.take().expect("child stdout"));
    KillOnDrop::new(child, ready)
}

fn wait_for_ready(child: &mut KillOnDrop) {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match child.ready.recv_timeout(remaining) {
            Ok(line) if line == PROCESS_READY => return,
            Ok(_) => {}
            Err(error) => {
                let status = child.child.try_wait().expect("query child status");
                panic!("restore child readiness failed ({error}); status: {status:?}");
            }
        }
    }
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

fn paths(root: &Path) -> ServiceSqlitePaths {
    let context = RuntimeContext::resolve(
        &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
        RuntimeContextBootstrap::new(
            RadrootsPathProfile::RepoLocal,
            Some(root.to_path_buf()),
            RuntimeContextSource::BootstrapCli,
            RuntimeContextSource::BootstrapCli,
        )
        .expect("bootstrap"),
        ServiceId::new("myc").expect("service"),
        InstanceId::new("process-recovery").expect("instance"),
    )
    .expect("runtime context");
    ServiceSqlitePaths::from_runtime_context(&context).expect("SQLite paths")
}

fn metadata(paths: &ServiceSqlitePaths) -> ServiceDatabaseMetadata {
    ServiceDatabaseMetadata::new(
        paths,
        SourceGeneration::new([7; 32]).expect("generation"),
        NonZeroU32::new(1).expect("schema"),
        1_700_000_000_000,
        ServiceSqliteApplicationId::new(0x5244_5351).expect("application ID"),
    )
    .expect("metadata")
}

fn catalogs() -> (MigrationCatalog, SchemaCatalog) {
    let migrations = MigrationCatalog::new([]).expect("migration catalog");
    let digest = SchemaVersionCatalog::computed_digest(1, []).expect("schema digest");
    let version = SchemaVersionCatalog::new(1, [], digest).expect("schema version");
    let schema = SchemaCatalog::new(&migrations, [version]).expect("schema catalog");
    (migrations, schema)
}

fn build_identity() -> MigrationBuildIdentity {
    MigrationBuildIdentity::new(
        "1.0.0",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "rustc-1.97.0",
        "process-test",
        "test",
        1,
        1,
        1,
        1,
        1,
    )
    .expect("build identity")
}

fn recovery_path(paths: &ServiceSqlitePaths, name: &str) -> PathBuf {
    paths
        .state_database()
        .parent()
        .expect("state directory")
        .join(name)
}

fn assert_recovery_permissions(paths: &ServiceSqlitePaths) {
    let state_directory = paths.state_database().parent().expect("state directory");
    let directory = fs::metadata(state_directory).expect("state directory metadata");
    assert!(directory.is_dir());
    assert_eq!(directory.mode() & 0o777, 0o700);
    assert_eq!(directory.uid(), rustix::process::geteuid().as_raw());
    for path in [
        paths.state_database().to_path_buf(),
        paths.state_lock().to_path_buf(),
        recovery_path(paths, STAGED_FILE_NAME),
        recovery_path(paths, BACKUP_FILE_NAME),
        recovery_path(paths, MARKER_FILE_NAME),
        recovery_path(paths, MARKER_NEXT_FILE_NAME),
    ] {
        if path.exists() {
            assert_file_permissions(&path);
        }
    }
}

fn assert_live_permissions(paths: &ServiceSqlitePaths) {
    assert_file_permissions(paths.state_database());
    assert_file_permissions(paths.state_lock());
}

fn assert_file_permissions(path: &Path) {
    let metadata = fs::symlink_metadata(path).expect("artifact metadata");
    assert!(metadata.file_type().is_file());
    assert_eq!(metadata.mode() & 0o777, 0o600);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
}

fn assert_no_recovery_evidence(paths: &ServiceSqlitePaths) {
    for name in [
        STAGED_FILE_NAME,
        BACKUP_FILE_NAME,
        MARKER_FILE_NAME,
        MARKER_NEXT_FILE_NAME,
    ] {
        assert!(!recovery_path(paths, name).exists(), "retained {name}");
    }
}

fn database_user_version(paths: &ServiceSqlitePaths) -> i64 {
    let connection = rusqlite::Connection::open_with_flags(
        paths.state_database(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("open recovered database");
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read recovery probe")
}

#[derive(Debug, PartialEq, Eq)]
struct FileSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    bytes: Vec<u8>,
}

fn file_snapshot(path: &Path) -> FileSnapshot {
    let metadata = fs::metadata(path).expect("snapshot metadata");
    FileSnapshot {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        bytes: fs::read(path).expect("snapshot bytes"),
    }
}

struct KillOnDrop {
    child: Child,
    ready: mpsc::Receiver<String>,
    finished: bool,
}

impl KillOnDrop {
    fn new(child: Child, ready: mpsc::Receiver<String>) -> Self {
        Self {
            child,
            ready,
            finished: false,
        }
    }

    fn child(&self) -> &Child {
        &self.child
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

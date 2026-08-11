#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::{env, error::Error, fs, path::PathBuf, process::Command};

use radroots_runtime_paths::{
    InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
    RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
};
use radroots_service_sqlite::{
    OpenMode, ServiceSqliteErrorKind, ServiceSqlitePaths, WriterAuthority,
};

const CHILD_ROOT: &str = "RADROOTS_SERVICE_SQLITE_WRITER_CHILD_ROOT";

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

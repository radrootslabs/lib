#![forbid(unsafe_code)]

use std::process::{Command, Stdio};

fn run_self_test(command: &str, expected_stdout: &[u8]) {
    let directory = tempfile::tempdir().expect("isolated command working directory");
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg(command)
        .current_dir(directory.path())
        .stdin(Stdio::null())
        .output()
        .expect("start the actual xtask binary");
    assert!(
        output.status.success(),
        "{command} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected_stdout);
    assert!(output.stderr.is_empty());
}

#[test]
fn advisory_command_executes_the_complete_offline_snapshot_and_process_suite() {
    run_self_test("advisory-snapshot-self-test", b"");
}

#[test]
fn bounded_process_command_executes_deadline_tree_output_and_environment_faults() {
    run_self_test(
        "bounded-process-self-test",
        b"bounded process self-test: ok\n",
    );
}

#[test]
fn artifact_command_executes_filesystem_archive_and_preflight_faults() {
    run_self_test(
        "safe-artifact-io-self-test",
        b"safe artifact I/O self-test: ok\n",
    );
}

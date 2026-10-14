use super::*;
use crate::bounded_process::{ProcessRequest, ReplacementEnvironment};
use std::os::unix::fs::{FileTypeExt, symlink};
use std::os::unix::net::UnixListener;
use std::time::Duration;
use tempfile::TempDir;

const CHILD_ROOT: &str = "RADROOTS_HYGIENE_FIXTURE_ROOT";
const FIFO_CASE: &str =
    "hygiene::prototype_input_tests::git_inventory_fifo_is_rejected_without_blocking";
const CONFIG: &str = r#"schema = "radroots.prototype-contract-source-guard.v1"
mode = "report_only"
[scan]
roots = ["src"]
path_roots = ["src"]
path_excludes = []
extensions = ["rs"]
extensionless_names = []
[limits]
max_scan_entries = 16
max_inventory_bytes = 4096
max_file_bytes = 1024
max_matches = 16
max_reported_findings = 4
max_reported_allowlisted = 4
[[pattern]]
id = "fixture-marker"
needle = "fixture-marker"
match_kind = "substring"
description = "Disposable scanner admission fixture"
"#;

struct Fixture {
    _directory: TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new(git_inventory: bool) -> Self {
        let directory = TempDir::new().expect("disposable scanner root");
        let root = directory
            .path()
            .canonicalize()
            .expect("physical fixture root");
        fs::create_dir(root.join("src")).expect("source directory");
        fs::create_dir(root.join("contracts")).expect("authority directory");
        assert!(CONFIG.len() <= 1024);
        fs::write(root.join("contracts/guard.toml"), CONFIG).expect("fixture authority");
        fs::write(root.join("src/input.rs"), "fn current_contract() {}\n").expect("source");
        if git_inventory {
            for args in [vec!["init", "-q"], vec!["add", "--", "src/input.rs"]] {
                let mut request = ProcessRequest::new("/usr/bin/git")
                    .arg("-C")
                    .arg(root.as_os_str())
                    .deadline(Duration::from_secs(5))
                    .output_limits(1024, 1024);
                for argument in args {
                    request = request.arg(argument);
                }
                let result = crate::bounded_process::run(&request).expect("bounded fixture Git");
                assert!(result.status().success(), "fixture Git: {result:?}");
            }
        }
        let fixture = Self {
            _directory: directory,
            root,
        };
        fixture.assert_bounds();
        fixture
    }

    fn path(&self) -> PathBuf {
        self.root.join("src/input.rs")
    }

    fn scan(&self, strict: bool) -> Result<(), String> {
        scan_fixture(&self.root, strict)
    }

    fn replace_with_fifo(&self) {
        fs::remove_file(self.path()).expect("replace regular fixture");
        let request = ProcessRequest::new("/usr/bin/mkfifo")
            .arg(self.path().as_os_str())
            .deadline(Duration::from_secs(5))
            .output_limits(1024, 1024);
        let output = crate::bounded_process::run(&request).expect("bounded fixture FIFO creation");
        assert!(output.status().success());
        assert!(
            fs::symlink_metadata(self.path())
                .expect("FIFO type")
                .file_type()
                .is_fifo()
        );
        self.assert_bounds();
    }

    fn replace_with_socket(&self) -> UnixListener {
        fs::remove_file(self.path()).expect("replace regular fixture");
        let short_path = self.root.join("s");
        let socket = UnixListener::bind(&short_path).expect("short owned fixture socket");
        fs::rename(short_path, self.path()).expect("move owned socket into source");
        assert!(
            fs::symlink_metadata(self.path())
                .expect("socket type")
                .file_type()
                .is_socket()
        );
        self.assert_bounds();
        socket
    }

    fn assert_bounds(&self) {
        let mut directories = 0;
        let mut files = 0;
        for entry in walkdir::WalkDir::new(&self.root).follow_links(false) {
            let entry = entry.expect("fixture inventory");
            if entry.file_type().is_dir() {
                directories += 1;
            } else {
                files += 1;
            }
            assert!(directories <= 32 && files <= 64, "fixture scaffold bounds");
        }
    }
}

fn scan_fixture(root: &Path, strict: bool) -> Result<(), String> {
    let mut args = vec![
        "prototype-contracts".to_owned(),
        "--config".to_owned(),
        "contracts/guard.toml".to_owned(),
    ];
    if strict {
        args.push("--strict".to_owned());
    }
    run(&args, root)
}

fn assert_regular_admission_error(result: Result<(), String>) {
    let error = result.expect_err("selected nonregular text input must not pass or be omitted");
    assert!(
        error.contains("regular"),
        "must reject object before reading: {error}"
    );
}

#[test]
fn regular_report_and_strict_findings_are_preserved() {
    for git_inventory in [false, true] {
        let fixture = Fixture::new(git_inventory);
        fs::write(fixture.path(), "const MARKER: &str = \"fixture-marker\";\n")
            .expect("fixture source");
        fixture
            .scan(false)
            .expect("report-only mode retains finding");
        let strict = fixture.scan(true).expect_err("strict mode retains failure");
        assert!(strict.contains("1 non-allowlisted match(es)"));
        assert_eq!(
            fs::read_to_string(fixture.path()).expect("unchanged source"),
            "const MARKER: &str = \"fixture-marker\";\n"
        );
        fixture.assert_bounds();
    }
}

#[test]
fn regular_byte_ceiling_and_utf8_are_preserved() {
    let fixture = Fixture::new(false);
    fs::write(fixture.path(), "x".repeat(1024)).expect("exact byte ceiling");
    assert_eq!(
        read_bounded_prototype_input(&fixture.path(), "src/input.rs", 1024)
            .expect("exact bounded bytes"),
        "x".repeat(1024)
    );
    fs::write(fixture.path(), "x".repeat(1025)).expect("one excess byte");
    assert!(
        read_bounded_prototype_input(&fixture.path(), "src/input.rs", 1024)
            .expect_err("reject excess byte")
            .contains("exceeds 1024 bytes")
    );
    fs::write(fixture.path(), [0xff]).expect("invalid UTF-8 fixture");
    assert!(
        read_bounded_prototype_input(&fixture.path(), "src/input.rs", 1024)
            .expect_err("reject invalid UTF-8")
            .contains("not UTF-8")
    );
}

#[test]
fn git_inventory_fifo_is_rejected_without_blocking() {
    if std::env::args().any(|argument| argument == "--exact")
        && let Some(root) = std::env::var_os(CHILD_ROOT)
    {
        let root = PathBuf::from(root);
        assert!(
            fs::symlink_metadata(root.join("src/input.rs"))
                .expect("child FIFO type")
                .file_type()
                .is_fifo()
        );
        assert_regular_admission_error(scan_fixture(&root, false));
        println!("owned_fifo_admission_probe_completed");
        return;
    }
    let fixture = Fixture::new(true);
    fixture.replace_with_fifo();
    let mut environment = ReplacementEnvironment::default();
    environment
        .insert(CHILD_ROOT, fixture.root.as_os_str())
        .expect("fixture-only child environment");
    let request =
        ProcessRequest::new(std::env::current_exe().expect("compiled scanner test binary"))
            .arg("--exact")
            .arg(FIFO_CASE)
            .arg("--nocapture")
            .arg("--test-threads=1")
            .environment(environment)
            .deadline(Duration::from_secs(2))
            .output_limits(8192, 8192);
    let output = crate::bounded_process::run(&request)
        .expect("nonregular source must be rejected within the child bound");
    assert!(
        output.status().success(),
        "child failed: {}{}",
        String::from_utf8_lossy(output.stdout()),
        String::from_utf8_lossy(output.stderr())
    );
    assert!(
        String::from_utf8_lossy(output.stdout()).contains("owned_fifo_admission_probe_completed")
    );
    assert!(
        fs::symlink_metadata(fixture.path())
            .expect("retained FIFO type")
            .file_type()
            .is_fifo()
    );
    fixture.assert_bounds();
}

#[test]
fn fallback_fifo_is_rejected_instead_of_omitted() {
    let fixture = Fixture::new(false);
    fixture.replace_with_fifo();
    assert_regular_admission_error(fixture.scan(false));
    assert!(
        fs::symlink_metadata(fixture.path())
            .expect("retained FIFO type")
            .file_type()
            .is_fifo()
    );
}

#[test]
fn git_socket_is_rejected_before_text_read() {
    let fixture = Fixture::new(true);
    let _socket = fixture.replace_with_socket();
    assert_regular_admission_error(fixture.scan(false));
    assert!(
        fs::symlink_metadata(fixture.path())
            .expect("retained socket type")
            .file_type()
            .is_socket()
    );
}

#[test]
fn fallback_socket_is_rejected_instead_of_omitted() {
    let fixture = Fixture::new(false);
    let _socket = fixture.replace_with_socket();
    assert_regular_admission_error(fixture.scan(false));
    assert!(
        fs::symlink_metadata(fixture.path())
            .expect("retained socket type")
            .file_type()
            .is_socket()
    );
}

#[test]
fn linked_source_rejection_preserves_target_bytes() {
    let fixture = Fixture::new(false);
    let target = fixture.root.join("src/target.txt");
    fs::write(&target, b"unchanged target\n").expect("owned link target");
    fs::remove_file(fixture.path()).expect("replace fixture source");
    symlink(&target, fixture.path()).expect("fixture link");
    assert!(
        fixture
            .scan(false)
            .expect_err("linked source remains rejected")
            .contains("symlink")
    );
    assert_eq!(
        fs::read(&target).expect("retained target"),
        b"unchanged target\n"
    );
    assert!(
        fs::symlink_metadata(fixture.path())
            .expect("retained link type")
            .file_type()
            .is_symlink()
    );
    fixture.assert_bounds();
}

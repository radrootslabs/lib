use super::*;
use std::{
    fs,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const CHILD_TEST: &str = "authored_draft::durability_tests::crash::authored_durability_child";
const CHILD_ROOT: &str = "RADROOTS_AUTHORED_CRASH_FIXTURE_ROOT";
const CHILD_PHASE: &str = "RADROOTS_AUTHORED_CRASH_FIXTURE_PHASE";

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn authored_durability_child() {
    let Some(directory) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let directory = Path::new(&directory);
    assert!(directory.is_absolute() && directory.is_dir());
    let phase = std::env::var(CHILD_PHASE).unwrap();
    assert!(matches!(
        phase.as_str(),
        "after_acknowledgment" | "during_uncommitted_insert"
    ));
    let store = open(directory, OpenMode::ReadWriteExisting).await;
    let baseline = first();
    let pending = next(&baseline, b"complete child revision".to_vec());
    if phase == "after_acknowledgment" {
        let receipt = store
            .append_authored_draft(pending.clone(), Some(baseline.revision()))
            .await
            .unwrap();
        assert_eq!(receipt.disposition(), DraftAppendDisposition::Inserted);
        assert_head(&store, &pending).await;
        fs::write(directory.join("child-ready"), phase.as_bytes()).unwrap();
        std::future::pending::<()>().await;
    } else {
        let mut transaction = store.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
        insert_draft_tx(&mut transaction, &pending).await.unwrap();
        assert_eq!(
            load_head_tx(&mut transaction, baseline.draft_id())
                .await
                .unwrap(),
            Some(pending)
        );
        fs::write(directory.join("child-ready"), phase.as_bytes()).unwrap();
        std::future::pending::<()>().await;
        // Keep the actual uncommitted transaction alive until process termination.
        transaction.rollback().await.unwrap();
    }
}

async fn kill_and_reopen(phase: &str) {
    let temp = TempDir::new().unwrap();
    let store = open(temp.path(), OpenMode::Create).await;
    let baseline = first();
    store
        .append_authored_draft(baseline.clone(), None)
        .await
        .unwrap();
    store.close().await.unwrap();
    let output = fs::File::create(temp.path().join("child-output")).unwrap();
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
            .env(CHILD_ROOT, temp.path())
            .env(CHILD_PHASE, phase)
            .stdin(Stdio::null())
            .stdout(output.try_clone().unwrap())
            .stderr(output)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "child exited before ready: {}",
            fs::read_to_string(temp.path().join("child-output")).unwrap()
        );
        if fs::read(temp.path().join("child-ready")).ok().as_deref() == Some(phase.as_bytes()) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "child did not reach exact write boundary"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(temp.path().join("runtime.sqlite-wal").is_file());
    child.0.kill().unwrap();
    assert!(!child.0.wait().unwrap().success());
    let reopened = open(temp.path(), OpenMode::ReadWriteExisting).await;
    let pending = next(&baseline, b"complete child revision".to_vec());
    let expected = if phase == "after_acknowledgment" {
        &pending
    } else {
        &baseline
    };
    assert_head(&reopened, expected).await;
    assert_eq!(
        reopened
            .authored_draft_revision(baseline.draft_id(), baseline.revision())
            .await
            .unwrap(),
        Some(baseline.clone())
    );
    if phase == "during_uncommitted_insert" {
        assert!(
            reopened
                .authored_draft_revision(pending.draft_id(), pending.revision())
                .await
                .unwrap()
                .is_none()
        );
    }
    let receipt = reopened
        .append_authored_draft(pending.clone(), Some(baseline.revision()))
        .await
        .unwrap();
    assert_eq!(
        receipt.disposition(),
        if phase == "after_acknowledgment" {
            DraftAppendDisposition::Replay
        } else {
            DraftAppendDisposition::Inserted
        }
    );
    assert_head(&reopened, &pending).await;
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn authored_durability_acknowledged_revision_survives_process_kill() {
    kill_and_reopen("after_acknowledgment").await;
}

#[tokio::test]
async fn authored_durability_uncommitted_revision_is_absent_after_process_kill() {
    kill_and_reopen("during_uncommitted_insert").await;
}

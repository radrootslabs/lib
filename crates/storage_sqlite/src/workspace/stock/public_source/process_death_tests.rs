//! Private process-death qualification of S, T2 and proven-unissued abort.
use super::*;
use radroots_domain::{CommandDigest, OperationId};
use radroots_secrets::{
    file::{FileOpenMode, FileProvider},
    wrapping::SecretMaterial,
};
use radroots_storage::workspace::AttachmentInput;
use std::{
    io::{BufRead, Read, Write},
    os::unix::fs::OpenOptionsExt,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

const BARRIER: &str = "RADROOTS_STOCK_PROCESS_BOUNDARY_V1";
const CHILD: &str = "workspace::stock::public_source::tests::provider::history::acceptance::signing::claims::evidence::installations::process_death::stock_process_child";

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
enum Edge {
    Capture,
    Install,
    Abort,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    input: AttachmentInput,
    proof: Vec<u8>,
    operation: OperationId,
    digest: CommandDigest,
    recipient: PublicKey,
    raw: String,
    edge: Edge,
    before: bool,
}
struct KillOnDrop(Child);
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn stop_at_boundary(root: &std::path::Path) {
    let mut child = KillOnDrop(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                CHILD,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let mut input = child.0.stdin.take().unwrap();
    writeln!(input, "{}", root.to_str().unwrap()).unwrap();
    drop(input);
    let stdout = child.0.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines().take(12) {
            let line = line.unwrap();
            if line.ends_with(BARRIER) {
                let _ = send.send(());
                return;
            }
        }
    });
    receive.recv_timeout(Duration::from_secs(60)).unwrap();
    assert!(child.0.try_wait().unwrap().is_none());
    child.0.kill().unwrap();
    let status = child.0.wait().unwrap();
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(status.signal(), Some(9));
    reader.join().unwrap();
}

#[tokio::test]
#[ignore = "private child invoked only by the bounded process-death harness"]
async fn stock_process_child() {
    let mut input = String::new();
    std::io::stdin()
        .take(4097)
        .read_to_string(&mut input)
        .unwrap();
    assert!(input.len() <= 4096 && input.ends_with('\n'));
    let root = std::path::PathBuf::from(input.trim_end_matches('\n'));
    assert!(root.is_absolute() && root.canonicalize().unwrap() == root);
    let mut bytes = Vec::new();
    std::fs::File::open(root.join("stock-process-control.json"))
        .unwrap()
        .take(73729)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= 73728);
    let control: Control = serde_json::from_slice(&bytes).unwrap();
    let file = Arc::new(
        FileProvider::open(
            root.join("vault"),
            FileOpenMode::OpenExisting,
            SecretMaterial::from_slice(&[0xa5; 32]).unwrap(),
        )
        .unwrap(),
    );
    let provider = Arc::new(Provider {
        inner: file,
        gate: Gate::new(),
    });
    let crypto = crate::workspace::WorkspaceCrypto::new(
        provider.clone(),
        radroots_secrets::SecretRef::new(
            radroots_secrets::SecretId::parse("grant-key").unwrap(),
            radroots_secrets::id::BackendKind::File,
            radroots_secrets::id::KeyVersion::new(1).unwrap(),
        ),
        SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
        radroots_secrets::provider::AccessPolicy::standard(),
    )
    .unwrap();
    let store = NativeWorkspace::open_seller_stock(
        &root.join("public-sources"),
        control.input.clone(),
        crypto,
        Arc::new(Clock(AtomicU64::new(150))),
    )
    .await
    .unwrap();
    let proof = AttachmentProof::from_protected_input(&control.proof).unwrap();
    let binding = store
        .authenticate(control.input.clone(), &proof)
        .await
        .unwrap();
    let minimum = store
        .stock_authority(control.input, &proof)
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
    let capture = if matches!(control.edge, Edge::Abort) {
        None
    } else {
        let report = store
            .stock_signing_attempt(
                StockClaimRequest::new(
                    binding.clone(),
                    minimum,
                    control.operation,
                    control.digest,
                    control.recipient,
                ),
                &proof,
            )
            .await
            .unwrap()
            .unwrap();
        Some(StockEvidenceCapture::verify(binding.clone(), &report, &control.raw, None).unwrap())
    };
    if control.before {
        provider
            .gate
            .arm(Direction::Wrap, "radroots.seller_stock.authority.v13", 0);
    }
    let work = async {
        match control.edge {
            Edge::Capture => {
                committed(
                    store
                        .capture_stock_signing_evidence(capture.as_ref().unwrap(), &proof)
                        .await
                        .unwrap(),
                );
            }
            Edge::Install => {
                installed(
                    store
                        .install_stock_agreement(
                            StockInstallationRequest::new(
                                binding,
                                minimum,
                                control.operation,
                                control.digest,
                            ),
                            &proof,
                        )
                        .await
                        .unwrap(),
                );
            }
            Edge::Abort => {
                aborts::released(
                    store
                        .abort_proven_unissued(
                            radroots_storage::inventory::StockAbortRequest::new(
                                binding,
                                minimum,
                                control.operation,
                                control.digest,
                            ),
                            &proof,
                        )
                        .await
                        .unwrap(),
                );
            }
        }
    };
    let mut work = Box::pin(work);
    if control.before {
        assert!(futures::poll!(work.as_mut()).is_pending());
        provider.gate.entered();
    } else {
        work.as_mut().await;
    }
    println!("{BARRIER}");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}

#[tokio::test]
async fn process_death_before_and_after_capture_install_or_abort_preserves_original_stock_and_evidence()
 {
    for edge in [Edge::Capture, Edge::Install, Edge::Abort] {
        for before in [true, false] {
            let mut r = Rig::new().await;
            let receipt = if matches!(edge, Edge::Abort) {
                Box::pin(prepared(&r)).await.1
            } else {
                Box::pin(ready(&r)).await
            };
            let recipient = peer(&r, &receipt);
            let (report, raw) = if matches!(edge, Edge::Abort) {
                (None, String::new())
            } else {
                let (report, raw) = Box::pin(obtained(&r, &receipt, recipient)).await;
                (Some(report), raw)
            };
            if matches!(edge, Edge::Install) {
                let capture = StockEvidenceCapture::verify(
                    r.binding.clone(),
                    report.as_ref().unwrap(),
                    &raw,
                    None,
                )
                .unwrap();
                committed(
                    r.store
                        .capture_stock_signing_evidence(&capture, r.enrollment.proof())
                        .await
                        .unwrap(),
                );
            }
            let minimum = inventory_minimum(&r).await;
            let control = Control {
                input: r.enrollment.input().clone(),
                proof: r.enrollment.proof().with_secret(|bytes| bytes.to_vec()),
                operation: receipt.attempt().operation_id(),
                digest: receipt.attempt().command_digest(),
                recipient,
                raw,
                edge,
                before,
            };
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(r.fixture.root().join("stock-process-control.json"))
                .unwrap();
            file.write_all(&serde_json::to_vec(&control).unwrap())
                .unwrap();
            file.sync_all().unwrap();
            r.store.close().await.unwrap();
            stop_at_boundary(&r.fixture.root());
            reopen_plain(&mut r).await;
            let current = inventory_minimum(&r).await;
            assert_eq!(
                current.revision().get(),
                minimum.revision().get() + u64::from(!before)
            );
            if matches!(edge, Edge::Abort) {
                assert!(claim(&r, &receipt, recipient).await.is_err());
            } else {
                assert!(matches!(
                    claim(&r, &receipt, recipient).await.unwrap(),
                    StockClaimOutcome::Existing(_)
                ));
            }
            let snapshot = r
                .store
                .stock_operation_snapshot(
                    StockSnapshotRequest::new(
                        r.binding.clone(),
                        current,
                        control.operation,
                        control.digest,
                    ),
                    r.enrollment.proof(),
                )
                .await
                .unwrap();
            let peer = snapshot
                .recipients()
                .iter()
                .find(|value| value.recipient() == recipient)
                .unwrap();
            let has_evidence =
                matches!(edge, Edge::Install) || (matches!(edge, Edge::Capture) && !before);
            assert_eq!(peer.first_evidence().is_some(), has_evidence);
            let has_installation = matches!(edge, Edge::Install) && !before;
            assert_eq!(snapshot.installation().is_some(), has_installation);
            let has_abort = matches!(edge, Edge::Abort) && !before;
            assert_eq!(snapshot.aborted().is_some(), has_abort);
            assert_eq!(peer.claim().is_some(), !matches!(edge, Edge::Abort));
            assert_eq!(
                history(&r).await.analysis().seller_head(),
                has_evidence.then_some(receipt.rumor().record_id())
            );
            if has_abort {
                balances(&r, 80, 10, 0, 0, 3).await;
                balances(&r, 81, 10, 0, 0, 3).await;
            } else if has_installation {
                balances(&r, 80, 9, 0, 1, 3).await;
                balances(&r, 81, 8, 0, 2, 3).await;
                let replay = installed(install(&r, &receipt).await.unwrap());
                assert_eq!(replay.attempt(), snapshot.installation().unwrap().attempt());
            } else {
                balances(&r, 80, 9, 1, 0, 2).await;
                balances(&r, 81, 8, 2, 0, 2).await;
            }
            assert_eq!(inventory_minimum(&r).await, current);
            if matches!(edge, Edge::Abort) {
                let first = aborts::released(aborts::abort(&r, &receipt).await.unwrap());
                if let Some(original) = snapshot.aborted() {
                    assert_eq!(first.attempt(), original.attempt());
                }
                balances(&r, 80, 10, 0, 0, 3).await;
                balances(&r, 81, 10, 0, 0, 3).await;
                let complete = inventory_minimum(&r).await;
                assert_eq!(
                    aborts::released(aborts::abort(&r, &receipt).await.unwrap()).attempt(),
                    first.attempt()
                );
                assert_eq!(inventory_minimum(&r).await, complete);
                r.store.close().await.unwrap();
                continue;
            }
            if !has_evidence {
                let capture = StockEvidenceCapture::verify(
                    r.binding.clone(),
                    report.as_ref().unwrap(),
                    &control.raw,
                    None,
                )
                .unwrap();
                committed(
                    r.store
                        .capture_stock_signing_evidence(&capture, r.enrollment.proof())
                        .await
                        .unwrap(),
                );
            }
            let final_installation = installed(install(&r, &receipt).await.unwrap());
            assert_eq!(final_installation.record_id(), receipt.rumor().record_id());
            balances(&r, 80, 9, 0, 1, 3).await;
            balances(&r, 81, 8, 0, 2, 3).await;
            let complete = inventory_minimum(&r).await;
            let replay = installed(install(&r, &receipt).await.unwrap());
            assert_eq!(replay.attempt(), final_installation.attempt());
            assert_eq!(inventory_minimum(&r).await, complete);
            r.store.close().await.unwrap();
        }
    }
}

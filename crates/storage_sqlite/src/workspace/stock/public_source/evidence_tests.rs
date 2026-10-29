//! Obtained-byte capture executes on the same real native stock owner.
use super::*;
#[path = "installation_tests.rs"]
mod installations;
#[path = "evidence_reconciliation_tests.rs"]
mod reconciliation;
#[path = "snapshot_tests.rs"]
mod snapshots;
use crate::workspace::StockEvidenceCapture;
use radroots_storage::inventory::{StockEvidenceOutcome, StockEvidenceReceipt};
use sha2::{Digest, Sha256};
async fn obtained(
    r: &Rig,
    receipt: &StockAcceptanceReceipt,
    recipient: PublicKey,
) -> (StockSigningAttempt, String) {
    let StockClaimOutcome::Fresh(permit) = claim(r, receipt, recipient).await.unwrap() else {
        panic!("first claim");
    };
    let signer = LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 3)).unwrap(), || {
        Ok(150000)
    })
    .unwrap();
    let crate::workspace::StockDispatchOutcome::Obtained {
        attempt: report,
        artifact: signed,
    } = permit
        .dispatch(&signer, CancellationSignal::new())
        .await
        .unwrap()
    else {
        panic!("real signature");
    };
    (report, signed.with_json(str::to_owned))
}
fn committed(value: StockEvidenceOutcome) -> StockEvidenceReceipt {
    let StockEvidenceOutcome::Committed(value) = value else {
        panic!("capture must confirm");
    };
    value
}
#[tokio::test]
async fn shared_capture_authenticates_actual_claim_and_rejects_substituted_descriptors_or_wire() {
    use radroots_storage::inventory::{InventoryStore, StockEvidenceRequest};
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let (report, raw) = obtained(&r, &receipt, recipient).await;
    let before = inventory_minimum(&r).await;
    let port: &dyn InventoryStore = &r.store;
    let tampered = raw.replace("\"sig\":\"", "\"sig\":\"0");
    for mode in 0..7 {
        let request = StockEvidenceRequest::from_obtained_wire(
            r.binding.clone(),
            if mode == 0 {
                OperationId::from_bytes([91; 32])
            } else {
                report.attempt().operation_id()
            },
            if mode == 1 {
                radroots_domain::CommandDigest::from_bytes([92; 32])
            } else {
                report.attempt().command_digest()
            },
            if mode == 2 {
                radroots_domain::SigningAttemptId::from_bytes([93; 32])
            } else {
                report.attempt_id()
            },
            if mode == 3 {
                receipt.rumor().recipients()[1]
            } else {
                report.recipient()
            },
            if mode == 4 {
                [94; 32]
            } else {
                *report.binding().digest()
            },
            if mode == 5 {
                "synthetic invalid original wire"
            } else if mode == 6 {
                &tampered
            } else {
                &raw
            },
            None,
        )
        .unwrap();
        assert!(!format!("{request:?}").contains("synthetic invalid"));
        assert!(
            port.capture_stock_evidence(&request, r.enrollment.proof())
                .await
                .is_err(),
            "mode {mode}"
        );
        request.with_json(|retained| assert!(!retained.is_empty()));
        assert_eq!(inventory_minimum(&r).await, before);
    }
    let request = StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None)
        .unwrap()
        .into_request();
    let retained = request.retained_copy();
    assert_eq!(retained.attempt(), request.attempt());
    retained.with_json(|bytes| assert_eq!(bytes, raw));
    let first = committed(
        port.capture_stock_evidence(&request, r.enrollment.proof())
            .await
            .unwrap(),
    );
    assert_eq!(first.signing_attempt_id(), report.attempt_id());
    assert_eq!(
        committed(
            port.capture_stock_evidence(&request, r.enrollment.proof())
                .await
                .unwrap()
        ),
        first
    );
    reopen_plain(&mut r).await;
    let stored = r
        .store
        .stock_signing_evidence(
            selector(&r, &receipt, recipient).await,
            *first.digest(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.receipt(), first);
    stored.with_json(|bytes| assert_eq!(bytes, raw));
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn original_peer_self_bytes_are_captured_once_reopened_and_preserve_first_optional_time() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let mut values = Vec::new();
    for recipient in receipt.rumor().recipients() {
        let (report, raw) = obtained(&r, &receipt, *recipient).await;
        let evidence =
            StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
        let before = inventory_minimum(&r).await;
        let first = committed(
            r.store
                .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
                .await
                .unwrap(),
        );
        assert_eq!(
            first.digest(),
            &<[u8; 32]>::from(Sha256::digest(raw.as_bytes()))
        );
        assert_eq!(first.signing_attempt_id(), report.attempt_id());
        assert_eq!(first.observed_at_unix_ms(), None);
        assert_eq!(
            inventory_minimum(&r).await.revision().get(),
            before.revision().get() + 1
        );
        let later =
            StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, NonZeroU64::new(150000))
                .unwrap();
        assert_eq!(
            committed(
                r.store
                    .capture_stock_signing_evidence(&later, r.enrollment.proof())
                    .await
                    .unwrap()
            ),
            first
        );
        values.push((report, raw, first));
    }
    reopen_plain(&mut r).await;
    for (report, raw, first) in values {
        let stored = r
            .store
            .stock_signing_evidence(
                selector(&r, &receipt, report.recipient()).await,
                *first.digest(),
                r.enrollment.proof(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.receipt(), first);
        stored.with_json(|value| assert_eq!(value, raw));
        assert!(matches!(
            claim(&r, &receipt, report.recipient()).await.unwrap(),
            StockClaimOutcome::Existing(_)
        ));
    }
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn already_obtained_bytes_survive_expiry_revocation_cancellation_and_missing_clock() {
    for mode in ["expired", "clock_unavailable", "revoked", "damaged_pool"] {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        let (report, raw) = obtained(&r, &receipt, recipient).await;
        let evidence =
            StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
        match mode {
            "damaged_pool" => {
                let mut rawdb = database(&r).await;
                sqlx::raw_sql("UPDATE stock_pools SET protected=randomblob(length(protected))")
                    .execute(&mut rawdb)
                    .await
                    .unwrap();
                rawdb.close().await.unwrap();
            }
            "expired" => r.clock.0.store(350, Ordering::SeqCst),
            "clock_unavailable" => {
                r.clock.0.store(u64::MAX, Ordering::SeqCst);
                assert!(r.clock.now().is_err());
            }
            "revoked" => {
                r.store
                    .revoke_stock_grant(
                        r.binding.clone(),
                        inventory_minimum(&r).await,
                        r.enrollment.proof(),
                        radroots_domain::TransactionId::from_bytes([94; 32]),
                        r.enrollment.input().grant_id(),
                    )
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let first = committed(
            r.store
                .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
                .await
                .unwrap(),
        );
        assert_eq!(first.observed_at_unix_ms(), None);
        evidence.with_json(|value| assert_eq!(value, raw));
        assert_eq!(
            committed(
                r.store
                    .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
                    .await
                    .unwrap()
            ),
            first
        );
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn distinct_valid_original_encodings_append_without_replacing_the_first_evidence() {
    let r = Rig::new().await;
    let receipt = ready(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let (report, raw) = obtained(&r, &receipt, recipient).await;
    let mut receipts = Vec::new();
    for (text, at) in [
        (&raw, None),
        (&format!("\n{raw}\n"), NonZeroU64::new(160000)),
    ] {
        let evidence = StockEvidenceCapture::verify(r.binding.clone(), &report, text, at).unwrap();
        let result = committed(
            r.store
                .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
                .await
                .unwrap(),
        );
        let retained = r
            .store
            .stock_signing_evidence(
                selector(&r, &receipt, recipient).await,
                *result.digest(),
                r.enrollment.proof(),
            )
            .await
            .unwrap()
            .unwrap();
        retained.with_json(|value| assert_eq!(value, text));
        assert_eq!(retained.receipt().observed_at_unix_ms(), at);
        receipts.push(result);
    }
    assert_ne!(receipts[0].digest(), receipts[1].digest());
    assert_ne!(
        receipts[0].attempt().transaction_id(),
        receipts[1].attempt().transaction_id()
    );
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn capture_refuses_wrong_recipient_bad_signature_and_wrong_original_credential() {
    let r = Rig::new().await;
    let receipt = ready(&r).await;
    let (peer, raw) = obtained(&r, &receipt, receipt.rumor().recipients()[0]).await;
    let (other, _) = obtained(&r, &receipt, receipt.rumor().recipients()[1]).await;
    assert!(StockEvidenceCapture::verify(r.binding.clone(), &other, &raw, None).is_err());
    let mut altered: serde_json::Value = serde_json::from_str(&raw).unwrap();
    altered["sig"] = serde_json::json!("00".repeat(64));
    assert!(
        StockEvidenceCapture::verify(
            r.binding.clone(),
            &peer,
            &serde_json::to_string(&altered).unwrap(),
            None
        )
        .is_err()
    );
    let evidence = StockEvidenceCapture::verify(r.binding.clone(), &peer, &raw, None).unwrap();
    let wrong = AttachmentProof::from_protected_input(&[99; 32]).unwrap();
    assert!(
        r.store
            .capture_stock_signing_evidence(&evidence, &wrong)
            .await
            .is_err()
    );
    evidence.with_json(|value| assert_eq!(value, raw));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_signing_evidence")
            .fetch_one(&mut database(&r).await)
            .await
            .unwrap(),
        0
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn every_capture_write_failure_preserves_caller_bytes_and_original_holds() {
    for statement in [
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operations WHEN NEW.family=5 BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_signing_evidence BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
    ] {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        let (report, raw) = obtained(&r, &receipt, recipient).await;
        let evidence =
            StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
        let before = inventory_minimum(&r).await;
        let mut rawdb = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut rawdb).await.unwrap();
        assert_eq!(
            r.store
                .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
                .await
                .unwrap_err()
                .code(),
            "storage_failure"
        );
        evidence.with_json(|value| assert_eq!(value, raw));
        assert_eq!(inventory_minimum(&r).await, before);
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut rawdb)
            .await
            .unwrap();
        committed(
            r.store
                .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
                .await
                .unwrap(),
        );
        assert!(matches!(
            claim(&r, &receipt, recipient).await.unwrap(),
            StockClaimOutcome::Existing(_)
        ));
        assert_pool(&r, 80, 9, 1, 2).await;
        rawdb.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn uncertain_capture_keeps_input_and_reopens_for_capture_without_another_signer_claim() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let (report, raw) = obtained(&r, &receipt, recipient).await;
    let evidence = StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
    let before = inventory_minimum(&r).await;
    let mut rawdb = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_signing_evidence BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut rawdb).await.unwrap();
    let StockEvidenceOutcome::Unknown(anchor) = r
        .store
        .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
        .await
        .unwrap()
    else {
        panic!("COMMIT must be unknown");
    };
    assert!(anchor.transaction_id().is_some());
    evidence.with_json(|value| assert_eq!(value, raw));
    assert_eq!(
        r.store
            .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
            .await
            .unwrap_err()
            .code(),
        "recovery_required"
    );
    r.store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fault; DROP TABLE deferred_fault")
        .execute(&mut rawdb)
        .await
        .unwrap();
    rawdb.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(inventory_minimum(&r).await, before);
    committed(
        r.store
            .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
            .await
            .unwrap(),
    );
    let StockClaimOutcome::Existing(again) = claim(&r, &receipt, recipient).await.unwrap() else {
        panic!("capture reconciliation cannot reissue");
    };
    assert_eq!(again.attempt_id(), report.attempt_id());
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn lost_capture_observer_drains_owned_input_and_reopens_the_exact_original_bytes() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let (report, raw) = obtained(&r, &receipt, recipient).await;
    let evidence = StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
    let provider = reopen(&mut r).await;
    provider.gate.arm(Direction::Wrap, r.authority_schema(), 0);
    let mut pending = Box::pin(
        r.store
            .capture_stock_signing_evidence(&evidence, r.enrollment.proof()),
    );
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    drop(pending);
    evidence.with_json(|value| assert_eq!(value, raw));
    provider.gate.release();
    reopen_plain(&mut r).await;
    let digest: [u8; 32] = Sha256::digest(raw.as_bytes()).into();
    let retained = r
        .store
        .stock_signing_evidence(
            selector(&r, &receipt, recipient).await,
            digest,
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    retained.with_json(|value| assert_eq!(value, raw));
    assert_pool(&r, 80, 9, 1, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn evidence_projection_or_protected_byte_damage_refuses_read_and_replacement() {
    for statement in [
        "DELETE FROM stock_signing_evidence",
        "UPDATE stock_signing_evidence SET evidence_digest=randomblob(32)",
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=5",
        "DELETE FROM stock_operation_parts WHERE operation_id IN (SELECT operation_id FROM stock_operations WHERE family=5)",
    ] {
        let r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        let (report, raw) = obtained(&r, &receipt, recipient).await;
        let evidence =
            StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
        let first = committed(
            r.store
                .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
                .await
                .unwrap(),
        );
        let request = selector(&r, &receipt, recipient).await;
        let mut rawdb = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut rawdb).await.unwrap();
        assert!(
            r.store
                .stock_signing_evidence(request, *first.digest(), r.enrollment.proof())
                .await
                .is_err(),
            "{statement}"
        );
        assert!(
            r.store
                .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
                .await
                .is_err(),
            "{statement}"
        );
        evidence.with_json(|value| assert_eq!(value, raw));
        rawdb.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn capture_rechecks_original_claim_plan_root_and_grant_after_provider_io() {
    for statement in [
        "UPDATE stock_grants SET protected=randomblob(length(protected))",
        "UPDATE stock_signing_claims SET recipient=randomblob(32)",
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=3",
        "UPDATE stock_authority SET protected=randomblob(length(protected))",
    ] {
        let mut r = Rig::new().await;
        let receipt = ready(&r).await;
        let recipient = receipt.rumor().recipients()[0];
        let (report, raw) = obtained(&r, &receipt, recipient).await;
        let evidence =
            StockEvidenceCapture::verify(r.binding.clone(), &report, &raw, None).unwrap();
        let provider = reopen(&mut r).await;
        let mut rawdb = database(&r).await;
        provider.gate.arm(Direction::Wrap, r.authority_schema(), 0);
        let mut pending = Box::pin(
            r.store
                .capture_stock_signing_evidence(&evidence, r.enrollment.proof()),
        );
        assert!(futures::poll!(pending.as_mut()).is_pending());
        provider.gate.entered();
        sqlx::raw_sql(statement).execute(&mut rawdb).await.unwrap();
        provider.gate.release();
        assert!(pending.await.is_err(), "{statement}");
        evidence.with_json(|value| assert_eq!(value, raw));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations WHERE family=5")
                .fetch_one(&mut rawdb)
                .await
                .unwrap(),
            0
        );
        rawdb.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn bounded_evidence_pages_recover_all_original_outputs_by_operation_after_reopen() {
    let mut r = Rig::new().await;
    let receipt = ready(&r).await;
    let recipient = receipt.rumor().recipients()[0];
    let (report, raw) = obtained(&r, &receipt, recipient).await;
    let empty = r
        .store
        .stock_signing_evidence_page(
            selector(&r, &receipt, recipient).await,
            None,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    assert!(empty.receipts().is_empty());
    assert_eq!(empty.next(), None);
    let base_revision = inventory_minimum(&r).await.revision().get();
    let mut expected = Vec::new();
    for count in 0..33 {
        let text = format!("{}{raw}", " ".repeat(count));
        let evidence =
            StockEvidenceCapture::verify(r.binding.clone(), &report, &text, None).unwrap();
        expected.push(committed(
            r.store
                .capture_stock_signing_evidence(&evidence, r.enrollment.proof())
                .await
                .unwrap(),
        ));
    }
    drop(raw);
    drop(report);
    reopen_plain(&mut r).await;
    let first = r
        .store
        .stock_signing_evidence_page(
            selector(&r, &receipt, recipient).await,
            None,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    assert_eq!(first.receipts(), &expected[..32]);
    assert_eq!(
        first.next(),
        Some(radroots_domain::Sequence::new(base_revision + 32).unwrap())
    );
    let recovered = r
        .store
        .stock_signing_evidence(
            selector(&r, &receipt, recipient).await,
            *expected[0].digest(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    let report = read(&r, &receipt, recipient).await.unwrap().unwrap();
    let mut digests: Vec<_> = expected.iter().map(|value| *value.digest()).collect();
    digests.sort();
    let text = (33..4096)
        .map(|count| recovered.with_json(|raw| format!("{}{raw}", " ".repeat(count))))
        .find(|text| <[u8; 32]>::from(Sha256::digest(text.as_bytes())) < digests[31])
        .unwrap();
    let late = StockEvidenceCapture::verify(r.binding.clone(), &report, &text, None).unwrap();
    expected.push(committed(
        r.store
            .capture_stock_signing_evidence(&late, r.enrollment.proof())
            .await
            .unwrap(),
    ));
    let last = r
        .store
        .stock_signing_evidence_page(
            selector(&r, &receipt, recipient).await,
            first.next(),
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    assert_eq!(last.receipts(), &expected[32..]);
    assert_eq!(last.next(), None);
    for value in first.receipts().iter().chain(last.receipts()) {
        let retained = r
            .store
            .stock_signing_evidence(
                selector(&r, &receipt, recipient).await,
                *value.digest(),
                r.enrollment.proof(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.receipt(), *value);
    }
    let missing = r
        .store
        .stock_signing_evidence_page(
            selector(&r, &receipt, receipt.rumor().recipients()[1]).await,
            None,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    assert!(missing.receipts().is_empty());
    let mut rawdb = database(&r).await;
    sqlx::raw_sql("DELETE FROM stock_signing_evidence WHERE evidence_digest=(SELECT min(evidence_digest) FROM stock_signing_evidence)").execute(&mut rawdb).await.unwrap();
    let request = StockClaimRequest::new(
        r.binding.clone(),
        receipt.checkpoint(),
        receipt.attempt().operation_id(),
        receipt.attempt().command_digest(),
        recipient,
    );
    assert!(
        r.store
            .stock_signing_evidence_page(request, None, r.enrollment.proof())
            .await
            .is_err()
    );
    rawdb.close().await.unwrap();
    r.store.close().await.unwrap();
}

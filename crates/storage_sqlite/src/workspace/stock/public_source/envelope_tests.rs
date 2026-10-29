//! Actual protected capture, original containment, reopen and transaction faults.
use super::*;
use nostr::JsonUtil;
use radroots_nostr::signing::{OsNonceSource, gift_wrap::wrap_retained_seal};
use radroots_storage::{
    command::PrivateEnvelopeCandidate,
    inventory::{InventoryStore, StockEnvelopeOutcome, StockEnvelopeReceipt, StockEnvelopeRequest},
};
use std::num::NonZeroU64;
#[path = "delivery_tests.rs"]
mod delivery_tests;
#[path = "rewrap_tests.rs"]
mod rewrapping;
fn proof(r: &Rig) -> AttachmentProof {
    r.enrollment
        .proof()
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
        .unwrap()
}
async fn envelope_request(
    r: &Rig,
    original: &StockAcceptanceReceipt,
    recipient: PublicKey,
    observed: Option<NonZeroU64>,
) -> StockEnvelopeRequest {
    StockEnvelopeRequest::new(
        r.binding.clone(),
        inventory_minimum(r).await,
        original.attempt().operation_id(),
        original.attempt().command_digest(),
        recipient,
        observed,
    )
    .unwrap()
}
fn wrapped(raw: &str, recipient: PublicKey) -> PrivateEnvelopeCandidate {
    let pending = wrap_retained_seal(
        raw,
        recipient,
        NonZeroU64::new(150).unwrap(),
        &OsNonceSource,
    )
    .unwrap();
    pending.with_wrapping_key(|key| {
        PrivateEnvelopeCandidate::from_obtained(recipient, pending.signed_json(), key).unwrap()
    })
}
async fn setup(
    catalog: i64,
) -> (
    Rig,
    StockAcceptanceReceipt,
    StockInstallationReceipt,
    PrivateEnvelopeCandidate,
) {
    let r = Rig::with_catalog(catalog).await;
    let original = ready(&r).await;
    let recipient = peer(&r, &original);
    let (claim, raw) = obtained(&r, &original, recipient).await;
    let capture = StockEvidenceCapture::verify(r.binding.clone(), &claim, &raw, None).unwrap();
    r.store
        .capture_stock_signing_evidence(&capture, r.enrollment.proof())
        .await
        .unwrap();
    let installed = installed(install(&r, &original).await.unwrap());
    let candidate = wrapped(&raw, recipient);
    (r, original, installed, candidate)
}
fn committed_envelope(value: StockEnvelopeOutcome) -> StockEnvelopeReceipt {
    let StockEnvelopeOutcome::Committed(value) = value else {
        panic!("confirmed original outer required")
    };
    *value
}
async fn capture(
    r: &Rig,
    original: &StockAcceptanceReceipt,
    candidate: &PrivateEnvelopeCandidate,
    at: Option<NonZeroU64>,
) -> Result<StockEnvelopeOutcome, OperationError> {
    r.store
        .capture_stock_envelope(
            envelope_request(r, original, candidate.recipient(), at).await,
            proof(r),
            candidate,
        )
        .await
}
#[tokio::test]
async fn original_peer_and_self_outers_retain_exact_bytes_once_and_reopen_without_new_stock_effects()
 {
    let (mut r, original, installed, peer_outer) = setup(17).await;
    let before = inventory_minimum(&r).await;
    let first = committed_envelope(capture(&r, &original, &peer_outer, None).await.unwrap());
    assert_eq!(
        first.installation_transaction_id(),
        installed.attempt().transaction_id().unwrap()
    );
    assert_eq!(first.event_id(), peer_outer.event_id());
    assert!(first.observed_at_unix_ms().is_none());
    assert_eq!(
        first.checkpoint().revision().get(),
        before.revision().get() + 1
    );
    assert_eq!(
        committed_envelope(
            capture(&r, &original, &peer_outer, NonZeroU64::new(151000))
                .await
                .unwrap()
        ),
        first
    );
    let self_recipient = r.enrollment.input().participant();
    let (claim, raw) = obtained(&r, &original, self_recipient).await;
    let signed = StockEvidenceCapture::verify(r.binding.clone(), &claim, &raw, None).unwrap();
    r.store
        .capture_stock_signing_evidence(&signed, r.enrollment.proof())
        .await
        .unwrap();
    let self_outer = wrapped(&raw, self_recipient);
    let second = committed_envelope(
        capture(&r, &original, &self_outer, NonZeroU64::new(151000))
            .await
            .unwrap(),
    );
    assert_eq!(second.observed_at_unix_ms(), NonZeroU64::new(151000));
    let checkpoint = inventory_minimum(&r).await;
    reopen_plain(&mut r).await;
    for (outer, receipt) in [(&peer_outer, first), (&self_outer, second)] {
        let source = r
            .store
            .stock_envelope(
                envelope_request(&r, &original, outer.recipient(), None).await,
                proof(&r),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(source.receipt(), receipt);
        source.with_json(|raw| assert_eq!(raw, outer.raw_json()));
    }
    assert_eq!(inventory_minimum(&r).await, checkpoint);
    balances(&r, 80, 9, 0, 1, 3).await;
    balances(&r, 81, 8, 0, 2, 3).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn invalid_key_wrong_original_seal_and_rewrapping_cannot_replace_the_original_outer() {
    let (r, original, _, outer) = setup(17).await;
    let before = inventory_minimum(&r).await;
    let wrong =
        PrivateEnvelopeCandidate::from_obtained(outer.recipient(), outer.raw_json(), &[9; 32])
            .unwrap();
    assert!(matches!(
        capture(&r, &original, &wrong, None).await,
        Err(OperationError::InvalidEvidence)
    ));
    let self_recipient = r.enrollment.input().participant();
    let (claim, raw) = obtained(&r, &original, self_recipient).await;
    let signed = StockEvidenceCapture::verify(r.binding.clone(), &claim, &raw, None).unwrap();
    r.store
        .capture_stock_signing_evidence(&signed, r.enrollment.proof())
        .await
        .unwrap();
    // A valid peer carrier containing the other recipient's actual seal.
    let keys = nostr::Keys::new(nostr::SecretKey::from_slice(&[7; 32]).unwrap());
    let recipient = nostr::PublicKey::from_slice(outer.recipient().as_bytes()).unwrap();
    let event = nostr::UnsignedEvent {
        id: None,
        pubkey: keys.public_key(),
        created_at: nostr::Timestamp::from_secs(150),
        kind: nostr::Kind::Custom(1059),
        tags: nostr::Tags::from_list(vec![nostr::Tag::public_key(recipient)]),
        content: nostr::nips::nip44::encrypt(
            keys.secret_key(),
            &recipient,
            &raw,
            nostr::nips::nip44::Version::V2,
        )
        .unwrap(),
    }
    .sign_with_keys(&keys)
    .unwrap();
    let foreign =
        PrivateEnvelopeCandidate::from_obtained(outer.recipient(), &event.as_json(), &[7; 32])
            .unwrap();
    assert!(matches!(
        capture(&r, &original, &foreign, None).await,
        Err(OperationError::InvalidEvidence)
    ));
    assert!(inventory_minimum(&r).await.revision() > before.revision());
    let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let evidence = r
        .store
        .stock_signing_evidence_page(
            crate::workspace::StockClaimRequest::new(
                r.binding.clone(),
                inventory_minimum(&r).await,
                original.attempt().operation_id(),
                original.attempt().command_digest(),
                outer.recipient(),
            ),
            None,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    let source = r
        .store
        .stock_signing_evidence(
            crate::workspace::StockClaimRequest::new(
                r.binding.clone(),
                inventory_minimum(&r).await,
                original.attempt().operation_id(),
                original.attempt().command_digest(),
                outer.recipient(),
            ),
            *evidence.receipts()[0].digest(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    let replacement = source.with_json(|raw| wrapped(raw, outer.recipient()));
    assert!(matches!(
        capture(&r, &original, &replacement, None).await,
        Err(OperationError::IdempotencyConflict)
    ));
    assert_eq!(
        committed_envelope(capture(&r, &original, &outer, None).await.unwrap()),
        first
    );
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn original_credential_captures_after_clock_loss_expiry_or_revocation_but_never_grants_read_access()
 {
    for mode in ["clock_unavailable", "expired", "revoked"] {
        let (r, original, _, outer) = setup(17).await;
        let selected = envelope_request(&r, &original, outer.recipient(), None).await;
        let read = envelope_request(&r, &original, outer.recipient(), None).await;
        match mode {
            "clock_unavailable" => r.clock.0.store(u64::MAX, Ordering::SeqCst),
            "expired" => r.clock.0.store(350, Ordering::SeqCst),
            "revoked" => {
                r.store
                    .revoke_stock_grant(
                        r.binding.clone(),
                        inventory_minimum(&r).await,
                        r.enrollment.proof(),
                        radroots_domain::TransactionId::from_bytes([99; 32]),
                        r.enrollment.input().grant_id(),
                    )
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        };
        let saved = committed_envelope(
            r.store
                .capture_stock_envelope(selected, proof(&r), &outer)
                .await
                .unwrap(),
        );
        assert!(saved.observed_at_unix_ms().is_none());
        assert!(r.store.stock_envelope(read, proof(&r)).await.is_err());
        r.clock.0.store(150, Ordering::SeqCst);
        if mode != "revoked" {
            let found = r
                .store
                .stock_envelope(
                    envelope_request(&r, &original, outer.recipient(), None).await,
                    proof(&r),
                )
                .await
                .unwrap()
                .unwrap();
            assert_eq!(found.receipt(), saved);
        }
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn late_original_or_projection_damage_cannot_commit_a_partially_authenticated_outer() {
    for statement in [
        "UPDATE stock_grants SET protected=randomblob(length(protected))",
        "UPDATE stock_signing_evidence SET evidence_digest=randomblob(32)",
        "UPDATE stock_publication_work SET recipient=randomblob(32)",
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=3",
    ] {
        let (mut r, original, _, outer) = setup(17).await;
        let selected = envelope_request(&r, &original, outer.recipient(), None).await;
        let provider = reopen(&mut r).await;
        let mut db = database(&r).await;
        provider.gate.arm(Direction::Wrap, r.authority_schema(), 0);
        let mut pending = Box::pin(r.store.capture_stock_envelope(selected, proof(&r), &outer));
        assert!(futures::poll!(pending.as_mut()).is_pending());
        provider.gate.entered();
        sqlx::raw_sql(statement).execute(&mut db).await.unwrap();
        provider.gate.release();
        assert!(pending.await.is_err(), "{statement}");
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_envelopes")
            .fetch_one(&mut db)
            .await
            .unwrap();
        assert_eq!(rows, 0);
        db.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn absent_installation_is_not_outer_capture_authority_and_retained15_never_upgrades() {
    let r = Rig::new().await;
    let original = ready(&r).await;
    let recipient = peer(&r, &original);
    let (claim, raw) = obtained(&r, &original, recipient).await;
    let signed = StockEvidenceCapture::verify(r.binding.clone(), &claim, &raw, None).unwrap();
    r.store
        .capture_stock_signing_evidence(&signed, r.enrollment.proof())
        .await
        .unwrap();
    let outer = wrapped(&raw, recipient);
    assert!(matches!(
        capture(&r, &original, &outer, None).await,
        Err(OperationError::MissingHistory)
    ));
    r.store.close().await.unwrap();
    let (mut r, original, _, outer) = setup(15).await;
    assert!(matches!(
        capture(&r, &original, &outer, None).await,
        Err(OperationError::UnsupportedOperation)
    ));
    assert!(matches!(
        r.store
            .stock_envelope(
                envelope_request(&r, &original, outer.recipient(), None).await,
                proof(&r)
            )
            .await,
        Err(OperationError::UnsupportedOperation)
    ));
    reopen_plain(&mut r).await;
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut database(&r).await)
        .await
        .unwrap();
    assert_eq!(catalog, 15);
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn every_outer_write_failure_retains_caller_bytes_and_no_partial_row_or_stock_effect() {
    for statement in [
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operations WHEN NEW.family=10 BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_envelopes BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
    ] {
        let (r, original, _, outer) = setup(17).await;
        let before = inventory_minimum(&r).await;
        let raw = outer.raw_json().to_owned();
        let mut db = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut db).await.unwrap();
        assert_eq!(
            capture(&r, &original, &outer, None)
                .await
                .unwrap_err()
                .code(),
            "storage_failure"
        );
        assert_eq!(outer.raw_json(), raw);
        assert_eq!(inventory_minimum(&r).await, before);
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_envelopes")
            .fetch_one(&mut db)
            .await
            .unwrap();
        assert_eq!(rows, 0);
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut db)
            .await
            .unwrap();
        committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
        balances(&r, 80, 9, 0, 1, 3).await;
        db.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn uncertain_outer_commit_fences_owner_and_original_bytes_can_be_retained_after_reopen() {
    let (mut r, original, _, outer) = setup(17).await;
    let before = inventory_minimum(&r).await;
    let mut db = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_envelopes BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut db).await.unwrap();
    let StockEnvelopeOutcome::Unknown(anchor) = capture(&r, &original, &outer, None).await.unwrap()
    else {
        panic!("unknown COMMIT")
    };
    assert!(anchor.transaction_id().is_some());
    let selected = StockEnvelopeRequest::new(
        r.binding.clone(),
        before,
        original.attempt().operation_id(),
        original.attempt().command_digest(),
        outer.recipient(),
        None,
    )
    .unwrap();
    assert_eq!(
        r.store
            .stock_envelope(selected, proof(&r))
            .await
            .unwrap_err()
            .code(),
        "recovery_required"
    );
    r.store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fault;DROP TABLE deferred_fault")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    reopen_plain(&mut r).await;
    assert_eq!(inventory_minimum(&r).await, before);
    committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn lost_outer_observer_is_drained_and_one_exact_record_survives_reopen() {
    let (mut r, original, _, outer) = setup(17).await;
    let selected = envelope_request(&r, &original, outer.recipient(), None).await;
    let provider = reopen(&mut r).await;
    provider.gate.arm(Direction::Wrap, r.authority_schema(), 0);
    let mut pending = Box::pin(r.store.capture_stock_envelope(selected, proof(&r), &outer));
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    drop(pending);
    provider.gate.release();
    reopen_plain(&mut r).await;
    let found = r
        .store
        .stock_envelope(
            envelope_request(&r, &original, outer.recipient(), None).await,
            proof(&r),
        )
        .await
        .unwrap()
        .unwrap();
    found.with_json(|raw| assert_eq!(raw, outer.raw_json()));
    assert_eq!(
        committed_envelope(capture(&r, &original, &outer, None).await.unwrap()),
        found.receipt()
    );
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn protected_outer_or_cache_damage_is_recovery_evidence_not_missing_work() {
    for statement in [
        "DELETE FROM stock_envelopes",
        "UPDATE stock_envelopes SET recipient=randomblob(32)",
        "UPDATE stock_operation_parts SET protected=randomblob(length(protected)) WHERE operation_id IN(SELECT operation_id FROM stock_operations WHERE family=10)",
        "UPDATE stock_operations SET protected=randomblob(length(protected)) WHERE family=10",
        "DELETE FROM stock_publication_work",
        "UPDATE stock_signing_evidence SET evidence_digest=randomblob(32)",
    ] {
        let (r, original, _, outer) = setup(17).await;
        committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
        let selected = envelope_request(&r, &original, outer.recipient(), None).await;
        sqlx::raw_sql(statement)
            .execute(&mut database(&r).await)
            .await
            .unwrap();
        assert_eq!(
            r.store
                .stock_envelope(selected, proof(&r))
                .await
                .unwrap_err()
                .code(),
            "recovery_required"
        );
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn retained16_keeps_exact_outer_capture_and_query_without_a_catalog_upgrade() {
    let (r, original, _, outer) = setup(16).await;
    let receipt = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let source = r
        .store
        .stock_envelope(
            envelope_request(&r, &original, outer.recipient(), None).await,
            proof(&r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(source.receipt(), receipt);
    source.with_json(|raw| assert_eq!(raw, outer.raw_json()));
    let mut db = database(&r).await;
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(catalog, 16);
    db.close().await.unwrap();
    r.store.close().await.unwrap();
}

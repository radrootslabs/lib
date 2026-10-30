//! Actual additional-envelope transactions preserve original stock commitments.
use super::delivery_tests::{claim as delivery_claim, observed, request as delivery_request, save};
use super::*;
use radroots_storage::inventory::{
    StockDeliveryAttemptOutcome, StockRewrapAdmission, StockRewrapOutcome, StockRewrapRequest,
    StockRewrapState,
};
struct Additional {
    outer: PrivateEnvelopeCandidate,
    admission: Option<StockRewrapAdmission>,
    id: OperationId,
}
impl core::ops::Deref for Additional {
    type Target = PrivateEnvelopeCandidate;
    fn deref(&self) -> &Self::Target {
        &self.outer
    }
}
use radroots_transport::outcome::DeliveryOutcome;

async fn additional(
    r: &Rig,
    original: &StockAcceptanceReceipt,
    first: StockEnvelopeReceipt,
) -> Additional {
    let id = OperationId::from_bytes([28; 32]);
    let admitted = r
        .store
        .prepare_stock_rewrap(
            StockRewrapRequest::new(
                envelope_request(r, original, first.recipient(), None).await,
                id,
            ),
            proof(r),
        )
        .await;
    let admission = if matches!(r.catalog, 18..=20) {
        let StockRewrapOutcome::Claimed(admission) = admitted.unwrap() else {
            panic!("fresh confirmed admission required before wrapping")
        };
        Some(*admission)
    } else {
        assert!(matches!(
            admitted,
            Err(OperationError::UnsupportedOperation)
        ));
        None
    };
    let source = r
        .store
        .stock_signing_evidence(
            crate::workspace::StockClaimRequest::new(
                r.binding.clone(),
                inventory_minimum(r).await,
                original.attempt().operation_id(),
                original.attempt().command_digest(),
                first.recipient(),
            ),
            *first.evidence().digest(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    Additional {
        outer: source.with_json(|raw| wrapped(raw, first.recipient())),
        admission,
        id,
    }
}
async fn selected(
    r: &Rig,
    original: &StockAcceptanceReceipt,
    outer: &PrivateEnvelopeCandidate,
) -> StockEnvelopeRequest {
    envelope_request(r, original, outer.recipient(), None)
        .await
        .with_envelope(outer.event_id())
}
async fn capture_request(
    r: &Rig,
    original: &StockAcceptanceReceipt,
    new: &Additional,
) -> StockRewrapRequest {
    StockRewrapRequest::new(selected(r, original, new).await, new.id)
}
#[tokio::test]
async fn rewrapping_preserves_original_bytes_and_independent_transactions_across_reopen() {
    let (mut r, original, _, outer) = setup(18).await;
    let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let new = additional(&r, &original, first).await;
    assert_ne!(new.event_id(), first.event_id());
    let before = inventory_minimum(&r).await;
    let saved = committed_envelope(
        r.store
            .capture_stock_rewrap(capture_request(&r, &original, &new).await, proof(&r), &new)
            .await
            .unwrap(),
    );
    assert_eq!(saved.evidence(), first.evidence());
    assert_eq!(
        saved.installation_transaction_id(),
        first.installation_transaction_id()
    );
    assert_ne!(
        saved.attempt().transaction_id(),
        first.attempt().transaction_id()
    );
    assert_eq!(
        saved.checkpoint().revision().get(),
        before.revision().get() + 1
    );
    let minimum = inventory_minimum(&r).await;
    assert_eq!(
        committed_envelope(
            r.store
                .capture_stock_rewrap(capture_request(&r, &original, &new).await, proof(&r), &new)
                .await
                .unwrap()
        ),
        saved
    );
    assert_eq!(inventory_minimum(&r).await, minimum);
    assert!(matches!(
        r.store
            .capture_stock_envelope(selected(&r, &original, &new).await, proof(&r), &new)
            .await,
        Err(OperationError::InvalidCommand)
    ));
    reopen_plain(&mut r).await;
    let old = r
        .store
        .stock_envelope(
            envelope_request(&r, &original, outer.recipient(), None).await,
            proof(&r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old.receipt(), first);
    old.with_json(|raw| assert_eq!(raw, outer.raw_json()));
    let loaded = r
        .store
        .stock_envelope(selected(&r, &original, &new).await, proof(&r))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.receipt(), saved);
    loaded.with_json(|raw| assert_eq!(raw, new.raw_json()));
    let page = r
        .store
        .stock_rewraps(
            envelope_request(&r, &original, outer.recipient(), None).await,
            None,
            proof(&r),
        )
        .await
        .unwrap();
    assert_eq!(
        page.entries(),
        &[StockRewrapState::from_store_report(new.admission.unwrap(), Some(saved)).unwrap()]
    );
    assert!(page.next_after().is_none());
    balances(&r, 80, 9, 0, 1, 3).await;
    balances(&r, 81, 8, 0, 2, 3).await;
    assert_eq!(inventory_minimum(&r).await, minimum);
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn rewrapping_delivery_preserves_global_policy_and_counters_but_scopes_acceptance_to_each_outer()
 {
    let (mut r, original, _, outer) = setup(18).await;
    let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let initial = delivery_claim(&r, &original, &outer, 1).await;
    let initial_observation =
        save(&r, &observed(&initial, DeliveryOutcome::accepted(), None)).await;
    let new = additional(&r, &original, first).await;
    let saved = committed_envelope(
        r.store
            .capture_stock_rewrap(capture_request(&r, &original, &new).await, proof(&r), &new)
            .await
            .unwrap(),
    );
    let request = delivery_request(&r, &original, &new, 2)
        .await
        .with_envelope(new.event_id())
        .unwrap()
        .with_expected_attempt_count(1)
        .unwrap();
    let StockDeliveryAttemptOutcome::Claimed(attempt) = r
        .store
        .prepare_stock_delivery_attempt(request, proof(&r))
        .await
        .unwrap()
    else {
        panic!("fresh confirmed selected D required")
    };
    assert_eq!(attempt.target_ordinals(), &[2]);
    attempt
        .source()
        .with_json(|raw| assert_eq!(raw, new.raw_json()));
    assert_eq!(attempt.source().receipt(), saved);
    let rejected = save(&r, &observed(&attempt, DeliveryOutcome::rejected(), None)).await;
    let original_summary = r
        .store
        .stock_delivery_summary(
            envelope_request(&r, &original, outer.recipient(), None).await,
            proof(&r),
        )
        .await
        .unwrap();
    let selected_summary = r
        .store
        .stock_delivery_summary(selected(&r, &original, &new).await, proof(&r))
        .await
        .unwrap();
    assert!(original_summary.acceptance_satisfied().unwrap());
    assert!(!selected_summary.acceptance_satisfied().unwrap());
    assert_eq!(selected_summary.source(), Some(saved));
    assert_eq!(selected_summary.publication_policy_source(), Some(first));
    assert_eq!(selected_summary.original_intent(), Some(initial.intent()));
    assert_eq!(original_summary.attempt_count(), 2);
    assert_eq!(selected_summary.attempt_count(), 2);
    assert_eq!(selected_summary.selected_attempt_count(), 1);
    assert_eq!(original_summary.selected_attempt_count(), 1);
    assert_eq!(selected_summary.targets()[0].potential_attempts(), 2);
    let accepted = save(&r, &observed(&attempt, DeliveryOutcome::accepted(), None)).await;
    let page = r
        .store
        .stock_delivery_observations(selected(&r, &original, &new).await, None, proof(&r))
        .await
        .unwrap();
    assert_eq!(page.entries(), &[rejected, accepted]);
    let original_page = r
        .store
        .stock_delivery_observations(
            envelope_request(&r, &original, outer.recipient(), None).await,
            None,
            proof(&r),
        )
        .await
        .unwrap();
    assert_eq!(original_page.entries(), &[initial_observation]);
    let transaction = attempt.transaction_id();
    reopen_plain(&mut r).await;
    let request = delivery_request(&r, &original, &new, 2)
        .await
        .with_envelope(new.event_id())
        .unwrap();
    let StockDeliveryAttemptOutcome::Retained(existing) = r
        .store
        .prepare_stock_delivery_attempt(request, proof(&r))
        .await
        .unwrap()
    else {
        panic!("existing selected D cannot authorize another invocation")
    };
    assert_eq!(existing.transaction_id(), transaction);
    existing
        .source()
        .with_json(|raw| assert_eq!(raw, new.raw_json()));
    let request = delivery_request(&r, &original, &new, 3)
        .await
        .with_envelope(new.event_id())
        .unwrap()
        .with_expected_attempt_count(0)
        .unwrap();
    assert!(matches!(
        r.store
            .prepare_stock_delivery_attempt(request, proof(&r))
            .await,
        Err(OperationError::StaleState)
    ));
    for index in 3..=12 {
        let request = delivery_request(&r, &original, &new, index)
            .await
            .with_envelope(new.event_id())
            .unwrap();
        let StockDeliveryAttemptOutcome::Claimed(value) = r
            .store
            .prepare_stock_delivery_attempt(request, proof(&r))
            .await
            .unwrap()
        else {
            panic!("new confirmed bounded D required")
        };
        assert_eq!(value.target_ordinals(), &[index]);
    }
    let minimum = inventory_minimum(&r).await;
    let request = delivery_request(&r, &original, &new, 13)
        .await
        .with_envelope(new.event_id())
        .unwrap();
    assert!(matches!(
        r.store
            .prepare_stock_delivery_attempt(request, proof(&r))
            .await,
        Err(OperationError::DeliveryAttemptLimit)
    ));
    assert_eq!(inventory_minimum(&r).await, minimum);
    balances(&r, 80, 9, 0, 1, 3).await;
    balances(&r, 81, 8, 0, 2, 3).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn rewrapping_first_delivery_does_not_fabricate_original_envelope_acceptance() {
    let (r, original, _, outer) = setup(18).await;
    let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let new = additional(&r, &original, first).await;
    let saved = committed_envelope(
        r.store
            .capture_stock_rewrap(capture_request(&r, &original, &new).await, proof(&r), &new)
            .await
            .unwrap(),
    );
    let request = delivery_request(&r, &original, &new, 1)
        .await
        .with_envelope(new.event_id())
        .unwrap();
    let StockDeliveryAttemptOutcome::Claimed(attempt) = r
        .store
        .prepare_stock_delivery_attempt(request, proof(&r))
        .await
        .unwrap()
    else {
        panic!("new confirmed selected D required")
    };
    save(&r, &observed(&attempt, DeliveryOutcome::accepted(), None)).await;
    let old = r
        .store
        .stock_delivery_summary(
            envelope_request(&r, &original, outer.recipient(), None).await,
            proof(&r),
        )
        .await
        .unwrap();
    let new_summary = r
        .store
        .stock_delivery_summary(selected(&r, &original, &new).await, proof(&r))
        .await
        .unwrap();
    assert!(!old.acceptance_satisfied().unwrap());
    assert!(new_summary.acceptance_satisfied().unwrap());
    assert_eq!(old.source(), Some(first));
    assert_eq!(old.publication_policy_source(), Some(saved));
    assert_eq!(old.attempt_count(), 1);
    assert_eq!(old.selected_attempt_count(), 0);
    assert_eq!(new_summary.selected_attempt_count(), 1);
    let page = r
        .store
        .stock_delivery_observations(
            envelope_request(&r, &original, outer.recipient(), None).await,
            None,
            proof(&r),
        )
        .await
        .unwrap();
    assert!(page.entries().is_empty());
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn rewrapping_late_unprotected_index_cannot_cross_final_transaction_witnesses() {
    use sqlx::Connection;
    let (mut r, original, _, outer) = setup(18).await;
    let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let new = additional(&r, &original, first).await;
    let request = capture_request(&r, &original, &new).await;
    let provider = reopen(&mut r).await;
    let mut db = database(&r).await;
    provider.gate.arm(Direction::Wrap, r.authority_schema(), 0);
    let mut pending = Box::pin(r.store.capture_stock_rewrap(request, proof(&r), &new));
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    sqlx::query("INSERT INTO stock_rewraps(actor,operation_id,recipient,admission_entry,event_id,entry_id) SELECT a.actor,a.operation_id,a.recipient,a.entry_id,?,e.entry_id FROM stock_rewrap_admissions a JOIN stock_envelopes e ON e.actor=a.actor AND e.operation_id=a.operation_id AND e.recipient=a.recipient LIMIT 1")
        .bind(new.event_id().as_bytes().as_slice()).execute(&mut db).await.unwrap();
    provider.gate.release();
    assert!(matches!(
        pending.await,
        Err(OperationError::RecoveryRequired)
    ));
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE family=14")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    sqlx::query("DELETE FROM stock_rewraps WHERE event_id=?")
        .bind(new.event_id().as_bytes().as_slice())
        .execute(&mut db)
        .await
        .unwrap();
    let request = capture_request(&r, &original, &new).await;
    let saved = committed_envelope(
        r.store
            .reconcile_stock_rewrap(request, proof(&r), &new)
            .await
            .unwrap(),
    );
    assert_eq!(saved.event_id(), new.event_id());
    balances(&r, 80, 9, 0, 1, 3).await;
    db.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn rewrapping_refuses_retained_catalog_seventeen_without_upgrading_it() {
    use sqlx::Connection;
    let (r, original, _, outer) = setup(17).await;
    let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let new = additional(&r, &original, first).await;
    assert!(matches!(
        r.store
            .capture_stock_rewrap(capture_request(&r, &original, &new).await, proof(&r), &new)
            .await,
        Err(OperationError::UnsupportedOperation)
    ));
    let mut db = database(&r).await;
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(catalog, 17);
    let table: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sqlite_schema WHERE name='stock_rewraps'")
            .fetch_one(&mut db)
            .await
            .unwrap();
    assert_eq!(table, 0);
    assert_eq!(
        committed_envelope(capture(&r, &original, &outer, None).await.unwrap()),
        first
    );
    db.close().await.unwrap();
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn rewrapping_retains_borrowed_original_through_statement_and_commit_failure_and_clock_loss()
{
    use sqlx::Connection;
    for uncertain in [false, true] {
        let (r, original, _, outer) = setup(18).await;
        let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
        let new = additional(&r, &original, first).await;
        let request = capture_request(&r, &original, &new).await;
        let mut fault = database(&r).await;
        if uncertain {
            sqlx::raw_sql("CREATE TABLE rewrap_deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER rewrap_fault AFTER INSERT ON stock_rewraps BEGIN INSERT INTO rewrap_deferred_fault VALUES(2); END").execute(&mut fault).await.unwrap();
            assert!(matches!(
                r.store
                    .capture_stock_rewrap(request, proof(&r), &new)
                    .await
                    .unwrap(),
                StockEnvelopeOutcome::Unknown(_)
            ));
        } else {
            sqlx::raw_sql("CREATE TRIGGER rewrap_fault BEFORE INSERT ON stock_rewraps BEGIN SELECT RAISE(ABORT, 'rewrap_fault'); END").execute(&mut fault).await.unwrap();
            assert!(matches!(
                r.store.capture_stock_rewrap(request, proof(&r), &new).await,
                Err(OperationError::StorageFailure)
            ));
        }
        r.clock.0.store(0, Ordering::SeqCst);
        sqlx::raw_sql("DROP TRIGGER rewrap_fault")
            .execute(&mut fault)
            .await
            .unwrap();
        if uncertain {
            sqlx::raw_sql("DROP TABLE rewrap_deferred_fault")
                .execute(&mut fault)
                .await
                .unwrap();
        }
        // Capture retries use only the retained original credential and bytes.
        let request = StockRewrapRequest::new(
            StockEnvelopeRequest::new(
                r.binding.clone(),
                first.checkpoint(),
                original.attempt().operation_id(),
                original.attempt().command_digest(),
                new.recipient(),
                None,
            )
            .unwrap()
            .with_envelope(new.event_id()),
            new.id,
        );
        let saved = committed_envelope(
            r.store
                .reconcile_stock_rewrap(request, proof(&r), &new)
                .await
                .unwrap(),
        );
        assert_eq!(saved.event_id(), new.event_id());
        assert_eq!(saved.evidence(), first.evidence());
        r.clock.0.store(150, Ordering::SeqCst);
        let loaded = r
            .store
            .stock_envelope(selected(&r, &original, &new).await, proof(&r))
            .await
            .unwrap()
            .unwrap();
        loaded.with_json(|raw| assert_eq!(raw, new.raw_json()));
        assert_eq!(loaded.receipt(), saved);
        assert_eq!(
            committed_envelope(capture(&r, &original, &outer, None).await.unwrap()),
            first
        );
        balances(&r, 80, 9, 0, 1, 3).await;
        balances(&r, 81, 8, 0, 2, 3).await;
        fault.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn rewrapping_same_request_has_one_confirmed_admission_and_unknown_wrap_is_not_reissued() {
    let (mut r, original, _, outer) = setup(18).await;
    let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let id = OperationId::from_bytes([29; 32]);
    let minimum = inventory_minimum(&r).await;
    let request = || {
        StockRewrapRequest::new(
            StockEnvelopeRequest::new(
                r.binding.clone(),
                minimum,
                original.attempt().operation_id(),
                original.attempt().command_digest(),
                first.recipient(),
                None,
            )
            .unwrap(),
            id,
        )
    };
    let (one, two) = futures::join!(
        r.store.prepare_stock_rewrap(request(), proof(&r)),
        r.store.prepare_stock_rewrap(request(), proof(&r))
    );
    let (StockRewrapOutcome::Claimed(one), StockRewrapOutcome::Retained(two)) =
        (one.unwrap(), two.unwrap())
    else {
        panic!("only first confirmed admission may qualify wrapping")
    };
    assert_eq!(one, two);
    let state = r
        .store
        .stock_rewrap_attempt(request(), proof(&r))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.admission(), *one);
    assert!(state.envelope().is_none());
    let checkpoint = inventory_minimum(&r).await;
    reopen_plain(&mut r).await;
    let request = StockRewrapRequest::new(
        envelope_request(&r, &original, first.recipient(), None).await,
        id,
    );
    let StockRewrapOutcome::Retained(retained) = r
        .store
        .prepare_stock_rewrap(request, proof(&r))
        .await
        .unwrap()
    else {
        panic!("reopened unknown wrapping outcome must not be reissued")
    };
    assert_eq!(*retained, *one);
    assert_eq!(inventory_minimum(&r).await, checkpoint);
    let page = r
        .store
        .stock_rewraps(
            envelope_request(&r, &original, first.recipient(), None).await,
            None,
            proof(&r),
        )
        .await
        .unwrap();
    assert_eq!(page.entries(), &[state]);
    assert!(page.next_after().is_none());
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn rewrapping_dropped_admission_waiter_cannot_reclaim_the_same_request() {
    let (mut r, original, _, outer) = setup(18).await;
    let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let provider = reopen(&mut r).await;
    let id = OperationId::from_bytes([30; 32]);
    let request = StockRewrapRequest::new(
        envelope_request(&r, &original, first.recipient(), None).await,
        id,
    );
    provider.gate.arm(Direction::Wrap, r.authority_schema(), 0);
    let mut pending = Box::pin(r.store.prepare_stock_rewrap(request, proof(&r)));
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    drop(pending);
    provider.gate.release();
    let request = StockRewrapRequest::new(
        envelope_request(&r, &original, first.recipient(), None).await,
        id,
    );
    let StockRewrapOutcome::Retained(retained) = r
        .store
        .prepare_stock_rewrap(request, proof(&r))
        .await
        .unwrap()
    else {
        panic!("lost waiter must not regain an invocation")
    };
    let state = r
        .store
        .stock_rewrap_attempt(
            StockRewrapRequest::new(
                envelope_request(&r, &original, first.recipient(), None).await,
                id,
            ),
            proof(&r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*retained, state.admission());
    assert!(state.envelope().is_none());
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn rewrapping_admission_statement_and_commit_failures_repair_without_clock_or_new_permit() {
    use sqlx::Connection;
    for uncertain in [false, true] {
        let (r, original, _, outer) = setup(18).await;
        let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
        let mut db = database(&r).await;
        let id = OperationId::from_bytes([31; 32]);
        let minimum = inventory_minimum(&r).await;
        let request = || {
            StockRewrapRequest::new(
                StockEnvelopeRequest::new(
                    r.binding.clone(),
                    minimum,
                    original.attempt().operation_id(),
                    original.attempt().command_digest(),
                    first.recipient(),
                    None,
                )
                .unwrap(),
                id,
            )
        };
        if uncertain {
            sqlx::raw_sql("CREATE TABLE admission_deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER admission_fault AFTER INSERT ON stock_rewrap_admissions BEGIN INSERT INTO admission_deferred_fault VALUES(2); END").execute(&mut db).await.unwrap();
            assert!(matches!(
                r.store
                    .prepare_stock_rewrap(request(), proof(&r))
                    .await
                    .unwrap(),
                StockRewrapOutcome::Unknown(_)
            ));
        } else {
            sqlx::raw_sql("CREATE TRIGGER admission_fault BEFORE INSERT ON stock_rewrap_admissions BEGIN SELECT RAISE(ABORT, 'admission_fault'); END").execute(&mut db).await.unwrap();
            assert!(matches!(
                r.store.prepare_stock_rewrap(request(), proof(&r)).await,
                Err(OperationError::StorageFailure)
            ));
        }
        r.clock.0.store(0, Ordering::SeqCst);
        sqlx::raw_sql("DROP TRIGGER admission_fault")
            .execute(&mut db)
            .await
            .unwrap();
        if uncertain {
            sqlx::raw_sql("DROP TABLE admission_deferred_fault")
                .execute(&mut db)
                .await
                .unwrap();
        }
        r.store
            .reconcile_stock_rewrap_admission(request(), proof(&r))
            .await
            .unwrap();
        assert!(
            r.store
                .stock_rewrap_attempt(request(), proof(&r))
                .await
                .is_err()
        );
        r.clock.0.store(150, Ordering::SeqCst);
        assert!(
            r.store
                .stock_rewrap_attempt(request(), proof(&r))
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            committed_envelope(capture(&r, &original, &outer, None).await.unwrap()),
            first
        );
        balances(&r, 80, 9, 0, 1, 3).await;
        db.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn rewrapping_pages_include_uncaptured_admissions_at_sixteen_with_stable_cursors() {
    let (mut r, original, _, outer) = setup(18).await;
    let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let mut expected = Vec::new();
    for index in 0..17 {
        let request = StockRewrapRequest::new(
            envelope_request(&r, &original, first.recipient(), None).await,
            OperationId::from_bytes([index; 32]),
        );
        let StockRewrapOutcome::Claimed(admitted) = r
            .store
            .prepare_stock_rewrap(request, proof(&r))
            .await
            .unwrap()
        else {
            panic!("distinct request requires a confirmed admission")
        };
        expected.push(StockRewrapState::from_store_report(*admitted, None).unwrap());
    }
    let checkpoint = inventory_minimum(&r).await;
    reopen_plain(&mut r).await;
    let page = r
        .store
        .stock_rewraps(
            envelope_request(&r, &original, first.recipient(), None).await,
            None,
            proof(&r),
        )
        .await
        .unwrap();
    assert_eq!(page.entries(), &expected[..16]);
    assert_eq!(
        page.next_after(),
        Some(expected[15].admission().checkpoint().revision())
    );
    assert_eq!(page.checkpoint(), checkpoint);
    let last = r
        .store
        .stock_rewraps(
            envelope_request(&r, &original, first.recipient(), None).await,
            page.next_after(),
            proof(&r),
        )
        .await
        .unwrap();
    assert_eq!(last.entries(), &expected[16..]);
    assert!(last.next_after().is_none());
    let empty = r
        .store
        .stock_rewraps(
            envelope_request(&r, &original, first.recipient(), None).await,
            Some(expected[16].admission().checkpoint().revision()),
            proof(&r),
        )
        .await
        .unwrap();
    assert!(empty.entries().is_empty());
    assert!(empty.next_after().is_none());
    assert_eq!(inventory_minimum(&r).await, checkpoint);
    assert_eq!(
        committed_envelope(capture(&r, &original, &outer, None).await.unwrap()),
        first
    );
    balances(&r, 80, 9, 0, 1, 3).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn rewrapping_oversized_unprotected_index_is_refused_without_changing_originals() {
    use sqlx::Connection;
    let (r, original, _, outer) = setup(18).await;
    let first = committed_envelope(capture(&r, &original, &outer, None).await.unwrap());
    let new = additional(&r, &original, first).await;
    let before = inventory_minimum(&r).await;
    let mut db = database(&r).await;
    sqlx::raw_sql("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON")
        .execute(&mut db)
        .await
        .unwrap();
    let phantom = OperationId::from_bytes([199; 32]);
    sqlx::query("INSERT INTO stock_rewrap_admissions(actor,operation_id,recipient,request_id,entry_id) SELECT actor,operation_id,recipient,?,zeroblob(1048576) FROM stock_envelopes LIMIT 1")
        .bind(phantom.as_bytes().as_slice()).execute(&mut db).await.unwrap();
    assert!(matches!(
        r.store
            .stock_rewraps(
                envelope_request(&r, &original, first.recipient(), None).await,
                None,
                proof(&r)
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    assert!(matches!(
        r.store
            .stock_rewrap_attempt(
                StockRewrapRequest::new(
                    envelope_request(&r, &original, first.recipient(), None).await,
                    phantom
                ),
                proof(&r)
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    assert!(matches!(
        r.store
            .capture_stock_rewrap(capture_request(&r, &original, &new).await, proof(&r), &new)
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    assert_eq!(inventory_minimum(&r).await, before);
    sqlx::query("DELETE FROM stock_rewrap_admissions WHERE request_id=?")
        .bind(phantom.as_bytes().as_slice())
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::raw_sql("PRAGMA ignore_check_constraints=OFF; PRAGMA foreign_keys=ON")
        .execute(&mut db)
        .await
        .unwrap();
    let saved = committed_envelope(
        r.store
            .capture_stock_rewrap(capture_request(&r, &original, &new).await, proof(&r), &new)
            .await
            .unwrap(),
    );
    assert_eq!(
        saved.rewrap_admission_transaction_id(),
        Some(new.admission.unwrap().transaction_id())
    );
    let loaded = r
        .store
        .stock_envelope(
            envelope_request(&r, &original, first.recipient(), None).await,
            proof(&r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.receipt(), first);
    loaded.with_json(|raw| assert_eq!(raw, outer.raw_json()));
    balances(&r, 80, 9, 0, 1, 3).await;
    db.close().await.unwrap();
    r.store.close().await.unwrap();
}

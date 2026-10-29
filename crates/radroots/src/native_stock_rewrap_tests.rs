//! Ordinary explicit wrapping and exact recovery use the real protected owner.
use super::*;
fn peer_index(r: &Rig, status: &crate::orders::AcceptanceStatus) -> usize {
    status
        .recipients()
        .iter()
        .position(|entry| entry.recipient != r.enrollment.input().participant())
        .unwrap()
}
#[tokio::test]
async fn ordinary_rewrap_is_explicit_preserves_originals_and_resumes_exact_bytes_without_signing() {
    let r = Rig::new().await;
    let (command, initial) = r.accept().await;
    let index = peer_index(&r, &initial);
    let recipient = initial.recipients()[index].recipient;
    let original = initial.delivery().unwrap()[index].source().unwrap();
    let id = OperationId::from_bytes([71; 32]);
    let (_, _, minimum) = r.minima().await;
    r.io.missing_ack.store(true, Ordering::SeqCst);
    let first = r
        .client
        .orders(&r.scope)
        .rewrap_acceptance(
            minimum,
            command.context().operation_id(),
            command.digest(),
            recipient,
            id,
        )
        .await
        .unwrap();
    let state = first.state().unwrap();
    let outer = state.envelope().unwrap();
    assert_eq!(state.admission().original(), original);
    assert_ne!(outer.event_id(), original.event_id());
    assert_eq!(outer.evidence(), original.evidence());
    assert_eq!(
        outer.installation_transaction_id(),
        original.installation_transaction_id()
    );
    let summary = first.delivery().unwrap();
    assert_eq!(summary.attempt_count(), 2);
    assert_eq!(summary.selected_attempt_count(), 1);
    assert!(!summary.acceptance_satisfied().unwrap());
    assert_eq!(summary.publication_policy_source(), Some(original));
    assert_eq!(r.io.wraps.load(Ordering::SeqCst), 3);
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    let sent = r.io.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 3);
    let raw = sent[2].clone();
    let repeated = r
        .client
        .orders(&r.scope)
        .rewrap_acceptance(
            minimum,
            command.context().operation_id(),
            command.digest(),
            recipient,
            id,
        )
        .await
        .unwrap();
    assert_eq!(repeated.state(), Some(state));
    assert_eq!(r.io.sent.lock().unwrap().len(), 3);
    assert_eq!(r.io.wraps.load(Ordering::SeqCst), 3);
    let observed = r
        .client
        .orders(&r.scope)
        .acceptance_status(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(observed.installation(), initial.installation());
    let old = &observed.delivery().unwrap()[index];
    assert_eq!(old.source(), Some(original));
    assert_eq!(old.attempt_count(), 2);
    assert_eq!(old.selected_attempt_count(), 1);
    assert!(old.acceptance_satisfied().unwrap());
    let read = r
        .client
        .orders(&r.scope)
        .acceptance_rewrap_status(
            minimum,
            command.context().operation_id(),
            command.digest(),
            recipient,
            id,
        )
        .await
        .unwrap();
    assert_eq!(read.state(), Some(state));
    assert_eq!(r.io.sent.lock().unwrap().len(), 3);
    r.io.missing_ack.store(false, Ordering::SeqCst);
    let resumed = r
        .client
        .orders(&r.scope)
        .resume_acceptance_rewrap(
            minimum,
            command.context().operation_id(),
            command.digest(),
            recipient,
            id,
        )
        .await
        .unwrap();
    assert_eq!(resumed.state(), Some(state));
    assert_eq!(resumed.delivery().unwrap().attempt_count(), 3);
    assert_eq!(resumed.delivery().unwrap().selected_attempt_count(), 2);
    assert!(resumed.delivery().unwrap().acceptance_satisfied().unwrap());
    assert_eq!(r.io.sent.lock().unwrap()[3], raw);
    assert_eq!(r.io.wraps.load(Ordering::SeqCst), 3);
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    r.client.close().await.unwrap();
}

async fn fault_connection(r: &Rig) -> sqlx::SqliteConnection {
    use sqlx::Connection;
    let path = r._temp.path().join("stock/stock_execution_v1.sqlite");
    sqlx::SqliteConnection::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap()
}
async fn original_entries(c: &mut sqlx::SqliteConnection) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let outers = sqlx::query_scalar("SELECT entry_id FROM stock_envelopes ORDER BY entry_id")
        .fetch_all(&mut *c)
        .await
        .unwrap();
    let seals = sqlx::query_scalar("SELECT entry_id FROM stock_signing_evidence ORDER BY entry_id")
        .fetch_all(c)
        .await
        .unwrap();
    (outers, seals)
}

#[tokio::test]
async fn obtained_rewrap_survives_statement_and_commit_failure_and_clockless_failed_close() {
    use sqlx::Connection;
    for uncertain in [false, true] {
        let r = Rig::new().await;
        let (command, initial) = r.accept().await;
        let recipient = initial.recipients()[peer_index(&r, &initial)].recipient;
        let (_, _, minimum) = r.minima().await;
        let mut fault = fault_connection(&r).await;
        let originals = original_entries(&mut fault).await;
        if uncertain {
            sqlx::raw_sql("CREATE TABLE rewrap_deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER controlled_rewrap_failure AFTER INSERT ON stock_rewraps BEGIN INSERT INTO rewrap_deferred_fault VALUES(2); END").execute(&mut fault).await.unwrap();
        } else {
            sqlx::query("CREATE TRIGGER controlled_rewrap_failure BEFORE INSERT ON stock_rewraps BEGIN SELECT RAISE(ABORT, 'controlled_rewrap_failure'); END").execute(&mut fault).await.unwrap();
        }
        let error = r
            .client
            .orders(&r.scope)
            .rewrap_acceptance(
                minimum,
                command.context().operation_id(),
                command.digest(),
                recipient,
                OperationId::from_bytes([72; 32]),
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.code(),
            if uncertain {
                crate::Code::RecoveryRequired
            } else {
                crate::Code::StorageFailure
            }
        );
        assert_eq!(r.io.wraps.load(Ordering::SeqCst), 3);
        assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
        assert_eq!(r.io.sent.lock().unwrap().len(), 2);
        r.clock.0.store(0, Ordering::SeqCst);
        assert!(r.client.close().await.is_err());
        assert!(r.provider.upgrade().is_some());
        sqlx::raw_sql(
            "DROP TRIGGER controlled_rewrap_failure; DROP TABLE IF EXISTS rewrap_deferred_fault",
        )
        .execute(&mut fault)
        .await
        .unwrap();
        r.client.close().await.unwrap();
        assert!(r.provider.upgrade().is_none());
        assert_eq!(original_entries(&mut fault).await, originals);
        let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM stock_rewrap_admissions),(SELECT count(*) FROM stock_rewraps)").fetch_one(&mut fault).await.unwrap();
        assert_eq!(counts, (1, 1));
        assert_eq!(r.io.wraps.load(Ordering::SeqCst), 3);
        assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
        assert_eq!(r.io.sent.lock().unwrap().len(), 2);
        fault.close().await.unwrap();
    }
}

#[tokio::test]
async fn failed_rewrap_admission_repairs_without_a_key_or_renewed_invocation() {
    use sqlx::Connection;
    for uncertain in [false, true] {
        let r = Rig::new().await;
        let (command, initial) = r.accept().await;
        let recipient = initial.recipients()[peer_index(&r, &initial)].recipient;
        let (_, _, minimum) = r.minima().await;
        let id = OperationId::from_bytes([73; 32]);
        let mut fault = fault_connection(&r).await;
        let originals = original_entries(&mut fault).await;
        if uncertain {
            sqlx::raw_sql("CREATE TABLE admission_deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER controlled_admission_failure AFTER INSERT ON stock_rewrap_admissions BEGIN INSERT INTO admission_deferred_fault VALUES(2); END").execute(&mut fault).await.unwrap();
        } else {
            sqlx::query("CREATE TRIGGER controlled_admission_failure BEFORE INSERT ON stock_rewrap_admissions BEGIN SELECT RAISE(ABORT, 'controlled_admission_failure'); END").execute(&mut fault).await.unwrap();
        }
        let error = r
            .client
            .orders(&r.scope)
            .rewrap_acceptance(
                minimum,
                command.context().operation_id(),
                command.digest(),
                recipient,
                id,
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.code(),
            if uncertain {
                crate::Code::RecoveryRequired
            } else {
                crate::Code::StorageFailure
            }
        );
        r.clock.0.store(0, Ordering::SeqCst);
        sqlx::raw_sql("DROP TRIGGER controlled_admission_failure; DROP TABLE IF EXISTS admission_deferred_fault").execute(&mut fault).await.unwrap();
        // Restore the exact catalog before uncertain-connection reopen. Repair
        // then resolves only the original writer fence, without a current clock,
        // a fresh admission or an invocation right.
        r.client.stock(&r.scope).drain_evidence().await.unwrap();
        r.clock.0.store(150, Ordering::SeqCst);
        let view = r
            .client
            .orders(&r.scope)
            .resume_acceptance_rewrap(
                minimum,
                command.context().operation_id(),
                command.digest(),
                recipient,
                id,
            )
            .await
            .unwrap();
        assert!(view.state().is_none());
        assert!(view.delivery().is_none());
        assert_eq!(original_entries(&mut fault).await, originals);
        assert_eq!(r.io.wraps.load(Ordering::SeqCst), 2);
        assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
        assert_eq!(r.io.sent.lock().unwrap().len(), 2);
        r.client.close().await.unwrap();
        fault.close().await.unwrap();
    }
}

#[tokio::test]
async fn obtained_rewrap_delivery_survives_lost_waiter_commit_failure_and_clock_loss() {
    use sqlx::Connection;
    for uncertain in [false, true] {
        let r = Rig::new().await;
        let (command, initial) = r.accept().await;
        let recipient = initial.recipients()[peer_index(&r, &initial)].recipient;
        let (_, _, minimum) = r.minima().await;
        r.io.pause_delivery.store(true, Ordering::SeqCst);
        let client = r.client.clone();
        let scope = r.scope.clone();
        let waiter = tokio::spawn(async move {
            client
                .orders(&scope)
                .rewrap_acceptance(
                    minimum,
                    command.context().operation_id(),
                    command.digest(),
                    recipient,
                    OperationId::from_bytes([74; 32]),
                )
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(120), r.io.entered.notified())
            .await
            .unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        let mut fault = fault_connection(&r).await;
        let originals = original_entries(&mut fault).await;
        let attempts: Vec<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
            "SELECT entry_id,request_key FROM stock_delivery_attempts ORDER BY entry_id",
        )
        .fetch_all(&mut fault)
        .await
        .unwrap();
        let observations: i64 =
            sqlx::query_scalar("SELECT count(*) FROM stock_delivery_observations")
                .fetch_one(&mut fault)
                .await
                .unwrap();
        if uncertain {
            sqlx::raw_sql("CREATE TABLE rewrap_observation_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER controlled_rewrap_observation AFTER INSERT ON stock_delivery_observations BEGIN INSERT INTO rewrap_observation_fault VALUES(2); END").execute(&mut fault).await.unwrap();
        } else {
            sqlx::query("CREATE TRIGGER controlled_rewrap_observation BEFORE INSERT ON stock_delivery_observations BEGIN SELECT RAISE(ABORT, 'controlled_rewrap_observation'); END").execute(&mut fault).await.unwrap();
        }
        r.clock.0.store(0, Ordering::SeqCst);
        r.io.release.notify_one();
        let error = r.client.close().await.unwrap_err();
        assert_eq!(
            error.code(),
            if uncertain {
                crate::Code::RecoveryRequired
            } else {
                crate::Code::StorageFailure
            }
        );
        assert!(r.provider.upgrade().is_some());
        sqlx::raw_sql("DROP TRIGGER controlled_rewrap_observation; DROP TABLE IF EXISTS rewrap_observation_fault").execute(&mut fault).await.unwrap();
        r.client.close().await.unwrap();
        assert!(r.provider.upgrade().is_none());
        assert_eq!(original_entries(&mut fault).await, originals);
        let actual: Vec<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
            "SELECT entry_id,request_key FROM stock_delivery_attempts ORDER BY entry_id",
        )
        .fetch_all(&mut fault)
        .await
        .unwrap();
        assert_eq!(actual, attempts);
        let actual: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_delivery_observations")
            .fetch_one(&mut fault)
            .await
            .unwrap();
        assert_eq!(actual, observations + 1);
        assert_eq!(r.io.wraps.load(Ordering::SeqCst), 3);
        assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
        assert_eq!(r.io.sent.lock().unwrap().len(), 3);
        fault.close().await.unwrap();
    }
}

#[tokio::test]
async fn rewrap_loopback_scopes_partial_acknowledgement_and_retries_exact_bytes_without_signing() {
    use futures::{SinkExt, StreamExt};
    use radroots_transport_nostr::{
        Config, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
    };
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tokio_tungstenite::{accept_async, tungstenite::Message};
    async fn relay(listener: tokio::net::TcpListener, partial: bool) -> Vec<String> {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(tcp).await.unwrap();
        let mut frames = Vec::new();
        for index in 0..if partial { 2 } else { 1 } {
            let frame = tokio::time::timeout(Duration::from_secs(300), async {
                loop {
                    if let Message::Text(raw) = socket.next().await.unwrap().unwrap() {
                        let event: serde_json::Value = serde_json::from_str(&raw).unwrap();
                        if event[0] == "EVENT" {
                            break (raw.to_string(), event[1]["id"].clone());
                        }
                    }
                }
            })
            .await
            .unwrap();
            socket
                .send(Message::Text(
                    serde_json::json!(["OK", "ff".repeat(32), true, ""])
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            socket
                .send(Message::Text(
                    serde_json::json!(["OK", frame.1, !partial || index > 0, ""])
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            frames.push(frame.0);
        }
        // Keep the accepted relay connected through the second invocation, so
        // skipping it is actual target selection rather than disconnect luck.
        if !partial {
            while let Some(message) = socket.next().await {
                match message.unwrap() {
                    Message::Text(raw) => {
                        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
                        assert_ne!(value[0], "EVENT", "accepted target must not be retried");
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        }
        frames
    }
    let first = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let second = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let uris = vec![
        format!("ws://{}", first.local_addr().unwrap()),
        format!("ws://{}", second.local_addr().unwrap()),
    ];
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let r = Rig::configured(Some(uris.clone()), now + 1800).await;
    r.io.missing_ack.store(true, Ordering::SeqCst);
    let (command, initial) = r.accept().await;
    let index = peer_index(&r, &initial);
    let recipient = initial.recipients()[index].recipient;
    let original = initial.delivery().unwrap()[index].source().unwrap();
    let originals = r.io.sent.lock().unwrap().clone();
    let accepted = tokio::spawn(relay(first, false));
    let partial = tokio::spawn(relay(second, true));
    let config = Config::from_profile(
        RelayProfile::explicit(
            RelayProfileKind::Simulator,
            uris.iter().map(|uri| {
                RelayEndpoint::new(uri, RelayUrlPolicy::Local, RelayAccess::ReadWrite).unwrap()
            }),
        )
        .unwrap(),
    )
    .with_timeouts(1000, 1000, 500)
    .unwrap();
    let transport = Arc::new(radroots_transport_nostr_native::create_transport(config));
    *r.io.transport.lock().unwrap() = Some(transport.clone());
    r.clock.enable_realtime();
    let (_, _, minimum) = r.minima().await;
    let grant = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        "explicit wrapping recovery".into(),
        UnixSeconds::new(now + 1500).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    r.native
        .enroll_stock_grant(
            r.scope.binding().clone(),
            minimum,
            r.enrollment.proof(),
            &grant,
        )
        .await
        .unwrap();
    let (_, _, minimum) = r.minima().await;
    let capture = StockConsentCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        grant.input().grant_id(),
        StockConsentMode::RecoverOnly,
        StockResources::all_pools(),
        UnixSeconds::new(now + 1200).unwrap(),
        CapturedOrderAcceptance::parse(command.protected_wire()).unwrap(),
    )
    .unwrap();
    r.native
        .issue_stock_consent(
            r.scope.binding().clone(),
            minimum,
            &capture,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    let (_, _, minimum) = r.minima().await;
    let recovery = r
        .client
        .attach(grant.input().clone(), proof(grant.proof()))
        .await
        .unwrap();
    let id = OperationId::from_bytes([75; 32]);
    let first = r
        .client
        .orders(&recovery)
        .rewrap_acceptance(
            minimum,
            command.context().operation_id(),
            command.digest(),
            recipient,
            id,
        )
        .await
        .unwrap();
    let state = first.state().unwrap();
    let summary = first.delivery().unwrap();
    assert_eq!(state.admission().original(), original);
    assert_eq!(summary.attempt_count(), 2);
    assert_eq!(summary.selected_attempt_count(), 1);
    assert!(!summary.acceptance_satisfied().unwrap());
    assert_eq!(summary.targets().iter().filter(|v| v.accepted()).count(), 1);
    let repeated = r
        .client
        .orders(&recovery)
        .rewrap_acceptance(
            minimum,
            command.context().operation_id(),
            command.digest(),
            recipient,
            id,
        )
        .await
        .unwrap();
    assert_eq!(repeated.state(), Some(state));
    assert_eq!(r.io.sent.lock().unwrap().len(), 3);
    let resumed = r
        .client
        .orders(&recovery)
        .resume_acceptance_rewrap(
            minimum,
            command.context().operation_id(),
            command.digest(),
            recipient,
            id,
        )
        .await
        .unwrap();
    assert_eq!(resumed.state(), Some(state));
    let summary = resumed.delivery().unwrap();
    assert_eq!(summary.attempt_count(), 3);
    assert_eq!(summary.selected_attempt_count(), 2);
    assert!(summary.acceptance_satisfied().unwrap());
    let mut counts: Vec<_> = summary
        .targets()
        .iter()
        .map(|v| v.potential_attempts())
        .collect();
    counts.sort();
    assert_eq!(counts, [2, 3]);
    let observed = r
        .client
        .orders(&recovery)
        .acceptance_status(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(observed.installation(), initial.installation());
    let old = &observed.delivery().unwrap()[index];
    assert_eq!(old.source(), Some(original));
    assert_eq!(old.selected_attempt_count(), 1);
    assert!(!old.acceptance_satisfied().unwrap());
    let sent = r.io.sent.lock().unwrap().clone();
    assert_eq!(&sent[..2], originals.as_slice());
    assert_eq!(sent[2], sent[3]);
    assert!(!originals.contains(&sent[2]));
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.io.wraps.load(Ordering::SeqCst), 3);
    transport.shutdown().await;
    let accepted = tokio::time::timeout(Duration::from_secs(10), accepted)
        .await
        .unwrap()
        .unwrap();
    let partial = tokio::time::timeout(Duration::from_secs(10), partial)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(accepted.len(), 1);
    assert_eq!(partial.len(), 2);
    assert_eq!(accepted[0], partial[0]);
    assert_eq!(partial[0], partial[1]);
    assert_eq!(accepted[0], format!("[\"EVENT\",{}]", sent[2]));
    r.client.close().await.unwrap();
}

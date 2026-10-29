//! Ordinary stock delivery and original-material recovery against real SQLx.
use super::*;

#[tokio::test]
async fn obtained_outer_is_retained_through_failed_sql_capture_and_clockless_drain() {
    use sqlx::Connection;
    let r = Rig::new().await;
    r.control.pause_peer.store(true, Ordering::SeqCst);
    let (command, context) = r.captured().await;
    let (private, public, minimum) = r.minima().await;
    let input = AcceptOrderInput::new(
        command.input().request(),
        command.input().terms_digest(),
        AllocationSelection::automatic(),
    );
    let client = r.client.clone();
    let scope = r.scope.clone();
    let waiter = tokio::spawn(async move {
        client
            .orders(&scope)
            .accept(context, minimum, private, public, input)
            .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        r.control.entered.notified(),
    )
    .await
    .unwrap();
    let path = r._temp.path().join("stock/stock_execution_v1.sqlite");
    let mut fault = sqlx::SqliteConnection::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER controlled_outer_failure BEFORE INSERT ON stock_envelopes BEGIN SELECT RAISE(ABORT, 'controlled_outer_failure'); END").execute(&mut fault).await.unwrap();
    r.control.release.notify_one();
    let error = waiter.await.unwrap().unwrap_err();
    assert_eq!(error.code(), crate::Code::StorageFailure);
    assert_eq!(r.io.wraps.load(Ordering::SeqCst), 1);
    assert!(r.io.sent.lock().unwrap().is_empty());
    sqlx::query("DROP TRIGGER controlled_outer_failure")
        .execute(&mut fault)
        .await
        .unwrap();
    fault.close().await.unwrap();
    r.clock.0.store(0, Ordering::SeqCst);
    r.client.stock(&r.scope).drain_evidence().await.unwrap();
    assert_eq!(r.io.wraps.load(Ordering::SeqCst), 1);
    r.clock.0.store(150, Ordering::SeqCst);
    let resumed = r
        .client
        .orders(&r.scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert!(
        resumed
            .delivery()
            .unwrap()
            .iter()
            .all(|v| v.acceptance_satisfied().unwrap())
    );
    assert_eq!(r.io.wraps.load(Ordering::SeqCst), 2);
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    r.client.close().await.unwrap();
}

#[tokio::test]
async fn recovery_delivers_original_stock_outers_through_actual_loopback_transport() {
    use futures::{SinkExt, StreamExt};
    use radroots_transport_nostr::{
        Config, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
    };
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tokio_tungstenite::{accept_async, tungstenite::Message};
    async fn relay(listener: tokio::net::TcpListener, originals: Vec<String>, misses: usize) {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(tcp).await.unwrap();
        for index in 0..(2 + misses) {
            let raw = tokio::time::timeout(Duration::from_secs(300), async {
                loop {
                    if let Message::Text(raw) = socket.next().await.unwrap().unwrap() {
                        break raw;
                    }
                }
            })
            .await
            .unwrap();
            let original = &originals[index % 2];
            assert_eq!(raw.as_str(), format!("[\"EVENT\",{original}]"));
            let event: serde_json::Value = serde_json::from_str(original).unwrap();
            socket
                .send(Message::Text(
                    serde_json::json!(["OK", "ff".repeat(32), true, ""])
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            if index >= misses {
                socket
                    .send(Message::Text(
                        serde_json::json!(["OK", event["id"], true, ""])
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
            }
        }
    }
    let first_relay = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let second_relay = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let uris = vec![
        format!("ws://{}", first_relay.local_addr().unwrap()),
        format!("ws://{}", second_relay.local_addr().unwrap()),
    ];
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let r = Rig::configured(Some(uris.clone()), now + 1800).await;
    r.io.missing_ack.store(true, Ordering::SeqCst);
    let (command, first) = r.accept().await;
    let originals = r.io.sent.lock().unwrap().clone();
    assert_eq!(originals.len(), 2);
    let accepted = tokio::spawn(relay(first_relay, originals.clone(), 0));
    let partial = tokio::spawn(relay(second_relay, originals.clone(), 2));
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
    // Real transport uses advancing UTC; only this injected test clock follows
    // it. Retained source events, S and original outers remain unchanged.
    r.clock.enable_realtime();
    let (_, _, minimum) = r.minima().await;
    let grant = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        "loopback recovery".into(),
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
    let incomplete = r
        .client
        .orders(&recovery)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    for state in incomplete.delivery().unwrap() {
        assert_eq!(state.attempt_count(), 2);
        assert!(!state.acceptance_satisfied().unwrap());
        assert_eq!(
            state
                .targets()
                .iter()
                .filter(|target| target.accepted())
                .count(),
            1
        );
    }
    let resumed = r
        .client
        .orders(&recovery)
        .resume_acceptance(
            incomplete.checkpoint(),
            command.context().operation_id(),
            command.digest(),
        )
        .await
        .unwrap();
    assert_eq!(resumed.installation(), first.installation());
    for state in resumed.delivery().unwrap() {
        assert_eq!(state.attempt_count(), 3);
        assert!(state.acceptance_satisfied().unwrap());
        let mut counts: Vec<_> = state
            .targets()
            .iter()
            .map(|v| v.potential_attempts())
            .collect();
        counts.sort();
        assert_eq!(counts, [2, 3]);
    }
    let sent = r.io.sent.lock().unwrap().clone();
    assert_eq!(&sent[2..4], originals.as_slice());
    assert_eq!(&sent[4..6], originals.as_slice());
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    tokio::time::timeout(Duration::from_secs(10), accepted)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), partial)
        .await
        .unwrap()
        .unwrap();
    transport.shutdown().await;
    r.client.close().await.unwrap();
}

#[tokio::test]
async fn repeated_command_does_not_resend_and_signerless_resume_uses_exact_originals() {
    let r = Rig::new().await;
    r.io.missing_ack.store(true, Ordering::SeqCst);
    let (command, first) = r.accept().await;
    assert!(
        first
            .delivery()
            .unwrap()
            .iter()
            .all(|v| v.attempt_count() == 1 && !v.acceptance_satisfied().unwrap())
    );
    let original = r.io.sent.lock().unwrap().clone();
    assert_eq!(original.len(), 2);
    let (private, public, minimum) = r.minima().await;
    let context = r
        .scope
        .command_context(
            command.context().operation_id(),
            command.context().expected().cloned(),
            command.context().approval().cloned(),
            command.context().not_after(),
        )
        .unwrap();
    let repeated = r
        .client
        .orders(&r.scope)
        .accept(
            context,
            minimum,
            private,
            public,
            AcceptOrderInput::new(
                command.input().request(),
                command.input().terms_digest(),
                AllocationSelection::automatic(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(repeated.installation(), first.installation());
    assert_eq!(r.io.sent.lock().unwrap().len(), 2);

    let path = r._temp.path().join("recovery_cache");
    std::fs::create_dir(&path).unwrap();
    let options = OpenOptions::new(Paths::from_directory(&path).unwrap(), OpenMode::Create)
        .with_source_generation(SourceGeneration::new([99; 32]).unwrap(), 1)
        .unwrap();
    let mut recovery =
        Client::open(NativeConfig::public_read(options).with_workspace(r.native.clone()))
            .await
            .unwrap();
    recovery.session = recovery
        .session
        .clone()
        .with_command_runtime(Arc::new(CommandRuntime::new(r.io.clone())));
    let scope = recovery
        .attach(r.enrollment.input().clone(), proof(r.enrollment.proof()))
        .await
        .unwrap();
    recovery.start(StartMode::LocalOnly).await.unwrap();
    assert!(recovery.signer_session().is_none());
    r.io.missing_ack.store(false, Ordering::SeqCst);
    let resumed = recovery
        .orders(&scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(resumed.installation(), first.installation());
    assert!(
        resumed
            .delivery()
            .unwrap()
            .iter()
            .all(|v| v.attempt_count() == 2 && v.acceptance_satisfied().unwrap())
    );
    assert_eq!(&r.io.sent.lock().unwrap()[2..], original.as_slice());
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    recovery.close().await.unwrap();
    r.client.close().await.unwrap();
}

#[tokio::test]
async fn obtained_delivery_survives_failed_append_lost_waiter_and_clock_loss() {
    use sqlx::Connection;
    let r = Rig::new().await;
    r.io.pause_delivery.store(true, Ordering::SeqCst);
    let (command, context) = r.captured().await;
    let (private, public, minimum) = r.minima().await;
    let input = AcceptOrderInput::new(
        command.input().request(),
        command.input().terms_digest(),
        AllocationSelection::automatic(),
    );
    let client = r.client.clone();
    let scope = r.scope.clone();
    let waiter = tokio::spawn(async move {
        client
            .orders(&scope)
            .accept(context, minimum, private, public, input)
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(30), r.io.entered.notified())
        .await
        .unwrap();
    assert_eq!(r.io.sent.lock().unwrap().len(), 1);
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 1);
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    let path = r._temp.path().join("stock/stock_execution_v1.sqlite");
    let mut fault = sqlx::SqliteConnection::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER controlled_d_failure BEFORE INSERT ON stock_delivery_observations BEGIN SELECT RAISE(ABORT, 'controlled_d_failure'); END").execute(&mut fault).await.unwrap();
    r.clock.0.store(0, Ordering::SeqCst);
    r.io.release.notify_one();
    // Close drains the admitted task and then its retained raw result, keeping
    // the original credential and resource lease through the actual SQL fault.
    let error = r.client.close().await.unwrap_err();
    assert_eq!(error.code(), crate::Code::StorageFailure);
    assert!(r.provider.upgrade().is_some());
    sqlx::query("DROP TRIGGER controlled_d_failure")
        .execute(&mut fault)
        .await
        .unwrap();
    r.client.close().await.unwrap();
    assert!(r.provider.upgrade().is_none());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_delivery_observations")
        .fetch_one(&mut fault)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(r.io.sent.lock().unwrap().len(), 1);
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 1);
    fault.close().await.unwrap();
}

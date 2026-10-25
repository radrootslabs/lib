//! Actual original request/source checks, not acceptance or physical reservation.
use super::*;

fn selected(name: &str) -> RecordId {
    data()["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == name)
        .unwrap()["record_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}
async fn inspect(r: &Rig, name: &str) -> Result<StockRequestSnapshot, OperationError> {
    let (private, public) = minima(r).await;
    r.store
        .stock_order_request(
            r.binding.clone(),
            private,
            public,
            selected(name),
            r.enrollment.proof(),
        )
        .await
}
async fn seed(r: &Rig, name: &str, quote: bool) {
    r.retain(&r.capture("listing_fixed")).await;
    if quote {
        retain(r, "chat").await;
        retain(r, "quote").await;
    }
    retain(r, name).await;
}

#[tokio::test]
async fn original_request_inspection_returns_exact_private_terms_without_stock_or_order_effects_and_reopens()
 {
    let mut r = Rig::new().await;
    base(&r).await;
    let before = minima(&r).await;
    let snapshot = inspect(&r, "request").await.unwrap();
    assert_eq!(snapshot.request(), identity("request"));
    assert_eq!(snapshot.scope(), scope());
    assert_eq!(snapshot.quote(), Some(identity("quote")));
    assert_eq!(snapshot.expires_at().get(), 240);
    assert_eq!(snapshot.observed_at().get(), 150);
    assert_eq!(snapshot.source_checkpoint(), before.0);
    assert_eq!(snapshot.listing_checkpoint(), before.1);
    assert_eq!(
        snapshot.checkpoint(),
        history(&r).await.checkpoint().unwrap()
    );
    let encoded = radroots_protocol::terms::EncodedTerms::capture(snapshot.terms()).unwrap();
    assert_eq!(snapshot.terms_digest(), encoded.digest());
    assert_eq!(snapshot.terms().lines().len(), 1);
    let fixture = data();
    let original = fixture["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "request")
        .unwrap();
    let rumor: serde_json::Value =
        serde_json::from_str(original["raw_rumor"].as_str().unwrap()).unwrap();
    let original =
        radroots_protocol::commerce::decode(rumor["content"].as_str().unwrap().as_bytes()).unwrap();
    let radroots_domain::CommercePayload::Request(original) = original.content().payload() else {
        panic!()
    };
    assert_eq!(snapshot.terms(), original.terms());
    assert_eq!(
        format!("{snapshot:?}"),
        "StockRequestSnapshot(<private request>)"
    );
    let mut raw = database(&r).await;
    for statement in [
        "SELECT count(*) FROM stock_operations",
        "SELECT count(*) FROM stock_movements",
        "SELECT count(*) FROM stock_orders",
        "SELECT count(*) FROM stock_pools",
    ] {
        let count: i64 = sqlx::query_scalar(statement)
            .fetch_one(&mut raw)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    raw.close().await.unwrap();
    assert_eq!(minima(&r).await, before);
    r.store.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let recovered = inspect(&r, "request").await.unwrap();
    assert_eq!(recovered.terms(), snapshot.terms());
    assert_eq!(recovered.checkpoint(), snapshot.checkpoint());
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn historical_existence_without_quote_cannot_bypass_paused_unsupported_or_future_observed_heads()
 {
    for name in [
        "listing_paused",
        "listing_withdrawn",
        "listing_unsupported",
        "listing_future",
    ] {
        let r = Rig::new().await;
        seed(&r, "request_no_quote", false).await;
        assert!(
            inspect(&r, "request_no_quote")
                .await
                .unwrap()
                .quote()
                .is_none()
        );
        r.retain(&r.capture(name)).await;
        assert_eq!(
            inspect(&r, "request_no_quote").await.unwrap_err().code(),
            "stale_state",
            "{name}"
        );
        assert_eq!(
            history(&r).await.analysis().readiness(),
            CausalReadiness::Complete
        );
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn still_valid_exact_explicit_quote_permits_its_historical_snapshot_without_public_offer_fallback()
 {
    for name in ["listing_paused", "listing_unsupported", "listing_future"] {
        let r = Rig::new().await;
        base(&r).await;
        r.retain(&r.capture(name)).await;
        let snapshot = inspect(&r, "request").await.unwrap();
        assert_eq!(snapshot.quote(), Some(identity("quote")));
        assert_eq!(
            snapshot.terms().lines()[0].source().event_id().into_bytes(),
            identity("listing_id").into_bytes()
        );
        let old = r
            .store
            .stock_listing_source(
                r.binding.clone(),
                r.minimum().await,
                r.capture("listing_fixed").listing_revision(),
                r.enrollment.proof(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(!old.is_current_observed());
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn authenticated_but_different_snapshots_quotes_withdrawals_and_forks_do_not_become_ready() {
    for (name, quote, expected) in [
        ("request_wrong_snapshot", false, "invalid_evidence"),
        ("request_wrong_quote_terms", true, "invalid_evidence"),
    ] {
        let r = Rig::new().await;
        seed(&r, name, quote).await;
        assert_eq!(inspect(&r, name).await.unwrap_err().code(), expected);
        r.store.close().await.unwrap();
    }
    for name in ["withdrawal", "decline", "future_commerce", "buyer_fork"] {
        let r = Rig::new().await;
        base(&r).await;
        if name == "buyer_fork" {
            retain(&r, "withdrawal").await;
        }
        retain(&r, name).await;
        let expected = if name == "future_commerce" {
            "capability_unavailable"
        } else {
            "stale_state"
        };
        assert_eq!(inspect(&r, "request").await.unwrap_err().code(), expected);
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn request_deadline_and_current_access_are_rechecked_after_all_actual_provider_io() {
    for at in [160, 240, 300] {
        let mut r = Rig::new().await;
        base(&r).await;
        let provider = reopen(&mut r).await;
        let (private, public) = minima(&r).await;
        provider.gate.arm(
            Direction::Unwrap,
            "radroots.seller_stock.public_source_part.v1",
            1,
        );
        let mut future = Box::pin(r.store.stock_order_request(
            r.binding.clone(),
            private,
            public,
            identity("request"),
            r.enrollment.proof(),
        ));
        assert!(futures::poll!(future.as_mut()).is_pending());
        provider.gate.entered();
        r.clock.0.store(at, Ordering::SeqCst);
        provider.gate.release();
        let result = future.await;
        match at {
            160 => assert_eq!(result.unwrap().observed_at().get(), 160),
            240 => assert_eq!(result.unwrap_err().code(), "deadline_exceeded"),
            _ => assert_eq!(result.unwrap_err().code(), "grant_expired"),
        }
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn protected_current_head_changes_after_opening_refuse_even_a_valid_quote_snapshot() {
    let mut r = Rig::new().await;
    base(&r).await;
    let provider = reopen(&mut r).await;
    let (private, public) = minima(&r).await;
    let mut raw = database(&r).await;
    provider.gate.arm(
        Direction::Unwrap,
        "radroots.seller_stock.public_source_head.v1",
        0,
    );
    let mut future = Box::pin(r.store.stock_order_request(
        r.binding.clone(),
        private,
        public,
        identity("request"),
        r.enrollment.proof(),
    ));
    assert!(futures::poll!(future.as_mut()).is_pending());
    provider.gate.entered();
    sqlx::query("UPDATE stock_public_heads SET protected=randomblob(length(protected))")
        .execute(&mut raw)
        .await
        .unwrap();
    provider.gate.release();
    assert_eq!(future.await.unwrap_err().code(), "recovery_required");
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}

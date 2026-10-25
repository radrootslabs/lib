//! Real NIP59/NIP44 source admission and actual native causal coverage. The
//! independent producer does not call a Radroots record/envelope constructor.
use super::*;
#[path = "acceptance_tests.rs"]
mod acceptance;
#[path = "request_tests.rs"]
mod requests;
use radroots_domain::{
    RecordId, SourceId,
    causal::{CausalReadiness, CausalScope},
};
use radroots_nostr::{commerce, key::SecretKey, signing::LocalSigner};
use radroots_signing::{provider::ProviderCall, request::CancellationSignal};
use radroots_storage::workspace::seller_stock::StockSourceCheckpointRef;
use std::num::NonZeroU64;

fn data() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/stock_history.v1.json"
    ))
    .unwrap()
}
fn scope() -> CausalScope {
    let value = data();
    let value = &value["scope"];
    CausalScope::new(
        value["thread"].as_str().unwrap().parse().unwrap(),
        value["order"].as_str().unwrap().parse().unwrap(),
        value["buyer"].as_str().unwrap().parse().unwrap(),
        value["seller"].as_str().unwrap().parse().unwrap(),
    )
    .unwrap()
}
fn identity(name: &str) -> RecordId {
    data()[name].as_str().unwrap().parse().unwrap()
}
fn source(name: &str) -> SourceId {
    SourceId::from_bytes(identity(name).into_bytes())
}
async fn retain(r: &Rig, name: &str) {
    let fixture = data();
    let row = fixture["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == name)
        .unwrap();
    let signer = LocalSigner::with_clock(
        SecretKey::parse(&format!(
            "{:064x}",
            row["recipient_scalar"].as_u64().unwrap()
        ))
        .unwrap(),
        || Ok(1000),
    )
    .unwrap();
    assert_eq!(signer.public_key(), r.enrollment.input().participant());
    let call = ProviderCall::new(
        signer.public_key(),
        NonZeroU64::new(2000).unwrap(),
        CancellationSignal::new(),
    );
    let opened = commerce::open(&signer, call, row["raw_wrap"].as_str().unwrap())
        .await
        .unwrap();
    let capture = StockSourceCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        &opened,
    )
    .unwrap();
    let expected = if name == "chat" || name == "future_commerce" {
        StockSourceProfile::Unsupported
    } else {
        StockSourceProfile::Known
    };
    assert_eq!(capture.profile(), expected, "{name}");
    let (minimum, _) = minima(r).await;
    assert!(matches!(
        r.store
            .retain_stock_source(r.binding.clone(), minimum, &capture, r.enrollment.proof())
            .await
            .unwrap(),
        StockSourceOutcome::Committed(_)
    ));
}
async fn minima(r: &Rig) -> (StockSourceCheckpointRef, StockListingCheckpointRef) {
    let current = r
        .store
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    (
        current.source_checkpoint().unwrap(),
        current.listing_checkpoint().unwrap(),
    )
}
async fn history(r: &Rig) -> StockHistorySnapshot {
    let (private, public) = minima(r).await;
    r.store
        .stock_order_history(
            r.binding.clone(),
            private,
            public,
            scope(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
}
async fn base(r: &Rig) {
    assert_eq!(
        data()["listing_raw"].as_str().unwrap(),
        vector("listing_fixed")
    );
    r.retain(&r.capture("listing_fixed")).await;
    for name in ["request", "quote", "chat"] {
        retain(r, name).await;
    }
}

#[tokio::test]
async fn all_independent_sources_have_their_exact_original_authenticated_profile_disposition() {
    let r = Rig::new().await;
    let fixture = data();
    for row in fixture["sources"].as_array().unwrap() {
        retain(&r, row["name"].as_str().unwrap()).await;
    }
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn seller_order_chain_begins_separately_from_the_original_null_order_quote() {
    let r = Rig::new().await;
    base(&r).await;
    retain(&r, "decline").await;
    let result = history(&r).await;
    assert_eq!(result.analysis().readiness(), CausalReadiness::Complete);
    assert_eq!(result.analysis().quote_head(), Some(identity("quote")));
    assert_eq!(result.analysis().seller_head(), Some(identity("decline")));
    assert_eq!(result.analysis().buyer_head(), Some(identity("request")));
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn real_original_missing_sources_become_complete_only_after_exact_history_arrives_and_reopen_retains_it()
 {
    let mut r = Rig::new().await;
    retain(&r, "request").await;
    let missing = history(&r).await;
    assert!(missing.checkpoint().is_none());
    assert_eq!(
        missing.analysis().readiness(),
        CausalReadiness::MissingHistory
    );
    let mut expected = vec![source("listing_id"), source("quote")];
    expected.sort();
    assert_eq!(missing.analysis().missing(), expected);
    retain(&r, "quote").await;
    retain(&r, "chat").await;
    assert_eq!(
        history(&r).await.analysis().missing(),
        [source("listing_id")]
    );
    r.retain(&r.capture("listing_fixed")).await;
    let before = minima(&r).await;
    let complete = history(&r).await;
    let coverage = complete.checkpoint().unwrap().clone();
    assert_eq!(coverage.sequence().get(), 4);
    assert_eq!(complete.analysis().readiness(), CausalReadiness::Complete);
    assert_eq!(complete.analysis().buyer_head(), Some(identity("request")));
    assert_eq!(complete.analysis().quote_head(), Some(identity("quote")));
    assert_eq!(complete.analysis().seller_head(), None);
    assert_eq!(complete.known_records(), 2);
    assert_eq!(complete.source_checkpoint(), before.0);
    assert_eq!(complete.listing_checkpoint(), before.1);
    assert_eq!(complete.observed_at().get(), 150);
    assert_eq!(minima(&r).await, before);
    retain(&r, "request_rewrapped").await;
    retain(&r, "unrelated").await;
    assert_eq!(history(&r).await.checkpoint(), Some(&coverage));
    assert_eq!(history(&r).await.analysis(), complete.analysis());
    assert_eq!(history(&r).await.known_records(), 2);
    r.store.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(history(&r).await.analysis(), complete.analysis());
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn actual_withdrawal_and_authenticated_author_fork_are_exposed_without_timestamp_winners_or_inventory_effects()
 {
    let r = Rig::new().await;
    base(&r).await;
    let stock = r
        .store
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap()
        .inventory_checkpoint();
    let original_coverage = history(&r).await.checkpoint().unwrap().clone();
    retain(&r, "withdrawal").await;
    let withdrawal = history(&r).await;
    assert_ne!(
        withdrawal.checkpoint().unwrap().digest(),
        original_coverage.digest()
    );
    assert_eq!(withdrawal.analysis().readiness(), CausalReadiness::Complete);
    assert_eq!(
        withdrawal.analysis().buyer_head(),
        Some(identity("withdrawal"))
    );
    retain(&r, "buyer_fork").await;
    let fork = history(&r).await;
    assert!(fork.checkpoint().is_none());
    assert_eq!(fork.analysis().readiness(), CausalReadiness::Conflicted);
    assert_eq!(fork.analysis().buyer_head(), Some(identity("request")));
    let mut expected = vec![identity("withdrawal"), identity("buyer_fork")];
    expected.sort();
    assert_eq!(fork.analysis().conflicts(), expected);
    assert_eq!(
        r.store
            .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
            .await
            .unwrap()
            .inventory_checkpoint(),
        stock
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn actual_missing_and_wrong_predecessors_do_not_manufacture_genesis_or_cross_quote_author_scope()
 {
    for name in ["missing_predecessor", "wrong_predecessor"] {
        let r = Rig::new().await;
        base(&r).await;
        retain(&r, name).await;
        let result = history(&r).await;
        assert_eq!(result.analysis().buyer_head(), Some(identity("request")));
        if name == "missing_predecessor" {
            assert_eq!(
                result.analysis().readiness(),
                CausalReadiness::MissingHistory
            );
            assert_eq!(result.analysis().missing(), [source("missing")]);
        } else {
            assert_eq!(result.analysis().readiness(), CausalReadiness::Conflicted);
            assert_eq!(result.analysis().conflicts().len(), 1);
        }
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn ordinary_chat_is_a_source_but_uninterpretable_authenticated_commerce_blocks_the_pair() {
    let r = Rig::new().await;
    base(&r).await;
    assert_eq!(
        history(&r).await.analysis().readiness(),
        CausalReadiness::Complete
    );
    retain(&r, "future_commerce").await;
    let result = history(&r).await;
    assert_eq!(
        result.analysis().readiness(),
        CausalReadiness::Uninterpreted
    );
    assert_eq!(result.analysis().uninterpreted().len(), 1);
    assert!(result.checkpoint().is_none());
    assert!(result.analysis().missing().is_empty());
    assert_eq!(result.analysis().buyer_head(), Some(identity("request")));
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn physical_grants_and_forked_source_acknowledgments_cannot_read_private_history() {
    let r = Rig::new().await;
    base(&r).await;
    let old = minima(&r).await;
    let inventory = r
        .store
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
    let child = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        "physical".into(),
        UnixSeconds::new(290).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    assert!(matches!(
        r.store
            .enroll_stock_grant(r.binding.clone(), inventory, r.enrollment.proof(), &child)
            .await
            .unwrap(),
        StockGrantOutcome::Committed(_)
    ));
    let child_binding = r
        .store
        .authenticate(child.input().clone(), child.proof())
        .await
        .unwrap();
    let current = minima(&r).await;
    assert_eq!(
        r.store
            .stock_order_history(child_binding, current.0, current.1, scope(), child.proof())
            .await
            .unwrap_err()
            .code(),
        "unauthorized"
    );
    assert_eq!(
        r.store
            .stock_order_history(
                r.binding.clone(),
                old.0,
                old.1,
                scope(),
                r.enrollment.proof()
            )
            .await
            .unwrap_err()
            .code(),
        "stale_checkpoint"
    );
    let fork = StockSourceCheckpointRef::from_host_metadata(
        r.enrollment.input().authority(),
        current.0.revision(),
        Some(radroots_domain::CheckpointDigest::from_bytes([0x44; 32])),
    )
    .unwrap()
    .with_optional_authorization(current.0.authorization());
    assert_eq!(
        r.store
            .stock_order_history(
                r.binding.clone(),
                fork,
                current.1,
                scope(),
                r.enrollment.proof()
            )
            .await
            .unwrap_err()
            .code(),
        "stale_checkpoint"
    );
    assert_eq!(
        history(&r).await.analysis().readiness(),
        CausalReadiness::Complete
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn causal_reads_recheck_real_canonical_rumor_and_listing_rows_after_successful_provider_opening()
 {
    for (schema, statement) in [
        (
            "radroots.seller_stock.source_record.v1",
            "UPDATE stock_source_records SET protected=randomblob(length(protected)) WHERE record_id=?",
        ),
        (
            "radroots.seller_stock.source_part.v1",
            "UPDATE stock_source_parts SET protected=randomblob(length(protected)) WHERE role=2 AND envelope_id=?",
        ),
        (
            "radroots.seller_stock.public_source_part.v1",
            "UPDATE stock_public_parts SET protected=randomblob(length(protected)) WHERE event_id=?",
        ),
    ] {
        let mut r = Rig::new().await;
        base(&r).await;
        let provider = reopen(&mut r).await;
        let (private, public) = minima(&r).await;
        let mut raw = database(&r).await;
        provider.gate.arm(Direction::Unwrap, schema, 0);
        let mut future = Box::pin(r.store.stock_order_history(
            r.binding.clone(),
            private,
            public,
            scope(),
            r.enrollment.proof(),
        ));
        assert!(futures::poll!(future.as_mut()).is_pending());
        provider.gate.entered();
        let key = match schema {
            "radroots.seller_stock.source_record.v1" => identity("request").into_bytes().to_vec(),
            "radroots.seller_stock.source_part.v1" => {
                let fixture = data();
                let row = fixture["sources"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["name"] == "chat")
                    .unwrap();
                let outer: serde_json::Value =
                    serde_json::from_str(row["raw_wrap"].as_str().unwrap()).unwrap();
                outer["id"]
                    .as_str()
                    .unwrap()
                    .parse::<EnvelopeEventId>()
                    .unwrap()
                    .into_bytes()
                    .to_vec()
            }
            _ => identity("listing_id").into_bytes().to_vec(),
        };
        assert_eq!(
            sqlx::query(statement)
                .bind(key)
                .execute(&mut raw)
                .await
                .unwrap()
                .rows_affected(),
            1
        );
        provider.gate.release();
        assert_eq!(future.await.unwrap_err().code(), "recovery_required");
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn causal_snapshot_reports_the_actual_final_checked_time_and_rejects_expiry_after_provider_io()
 {
    for at in [299, 300] {
        let mut r = Rig::new().await;
        base(&r).await;
        let provider = reopen(&mut r).await;
        let (private, public) = minima(&r).await;
        provider.gate.arm(
            Direction::Unwrap,
            "radroots.seller_stock.public_source_part.v1",
            0,
        );
        let mut future = Box::pin(r.store.stock_order_history(
            r.binding.clone(),
            private,
            public,
            scope(),
            r.enrollment.proof(),
        ));
        assert!(futures::poll!(future.as_mut()).is_pending());
        provider.gate.entered();
        r.clock.0.store(at, Ordering::SeqCst);
        provider.gate.release();
        let result = future.await;
        if at == 299 {
            assert_eq!(result.unwrap().observed_at().get(), 299);
        } else {
            assert_eq!(result.unwrap_err().code(), "grant_expired");
        }
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn observer_loss_does_not_abandon_admitted_history_work_or_release_the_owner_early() {
    let mut r = Rig::new().await;
    base(&r).await;
    let provider = reopen(&mut r).await;
    let (private, public) = minima(&r).await;
    provider.gate.arm(
        Direction::Unwrap,
        "radroots.seller_stock.public_source_part.v1",
        0,
    );
    let mut future = Box::pin(r.store.stock_order_history(
        r.binding.clone(),
        private,
        public,
        scope(),
        r.enrollment.proof(),
    ));
    assert!(futures::poll!(future.as_mut()).is_pending());
    provider.gate.entered();
    drop(future);
    assert!(matches!(
        NativeWorkspace::open_seller_stock(
            &r.path,
            r.enrollment.input().clone(),
            r.fixture.crypto(),
            r.clock.clone()
        )
        .await,
        Err(WorkspaceError::WriterActive)
    ));
    provider.gate.release();
    r.store.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        history(&r).await.analysis().readiness(),
        CausalReadiness::Complete
    );
    assert_eq!(minima(&r).await, (private, public));
    r.store.close().await.unwrap();
}

//! Real native observation persistence over independent signed inputs. A
//! selected head remains observation evidence, never business permission.
use super::*;
use crate::workspace::tests::{CryptoFixture, participant};
use radroots_domain::{PublicKey, UnixSeconds};
use radroots_storage::{
    command::OperationError, workspace::seller_stock::StockListingCheckpointRef,
};
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "test_provider.rs"]
mod provider;

struct Clock(AtomicU64);
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        UnixSeconds::new(self.0.load(Ordering::SeqCst)).map_err(|_| WorkspaceError::Unavailable)
    }
}
fn vectors() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/stock_public_sources.v1.json"
    ))
    .unwrap()
}
fn vector(name: &str) -> String {
    vectors()["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == name)
        .unwrap()["raw"]
        .as_str()
        .unwrap()
        .into()
}
struct Rig {
    fixture: CryptoFixture,
    path: std::path::PathBuf,
    store: NativeWorkspace,
    enrollment: StockEnrollment,
    binding: WorkspaceBinding,
    clock: Arc<Clock>,
}
impl Rig {
    async fn new() -> Self {
        let fixture = CryptoFixture::new();
        let path = fixture.root().join("public-sources");
        let owner = PublicKey::from_hex(vectors()["author"].as_str().unwrap()).unwrap();
        let clock = Arc::new(Clock(AtomicU64::new(150)));
        let enrollment = StockEnrollment::prepare(
            SellerStockConfig::new(
                owner,
                participant(),
                "public-source-owner",
                UnixSeconds::new(300).unwrap(),
            )
            .unwrap(),
            clock.as_ref(),
        )
        .unwrap();
        let store = NativeWorkspace::initialize_seller_stock(
            &path,
            &enrollment,
            fixture.crypto(),
            clock.clone(),
        )
        .await
        .unwrap();
        let binding = store
            .authenticate(enrollment.input().clone(), enrollment.proof())
            .await
            .unwrap();
        Self {
            fixture,
            path,
            store,
            enrollment,
            binding,
            clock,
        }
    }
    fn capture_raw(&self, raw: &str) -> StockListingCapture {
        let mut observed = ObservedHead::listing(
            self.enrollment.input().participant(),
            vectors()["d"].as_str().unwrap(),
        )
        .unwrap();
        observed.observe(raw).unwrap();
        StockListingCapture::prepare(
            SellerAuthoritySelection::from_attachment(self.enrollment.input()),
            &observed,
        )
        .unwrap()
    }
    fn capture(&self, name: &str) -> StockListingCapture {
        self.capture_raw(&vector(name))
    }
    async fn minimum(&self) -> StockListingCheckpointRef {
        self.store
            .stock_authority(self.enrollment.input().clone(), self.enrollment.proof())
            .await
            .unwrap()
            .listing_checkpoint()
            .unwrap()
    }
    async fn retain(&self, capture: &StockListingCapture) -> StockListingReceipt {
        committed(
            self.store
                .retain_stock_listing(
                    self.binding.clone(),
                    self.minimum().await,
                    capture,
                    self.enrollment.proof(),
                )
                .await
                .unwrap(),
        )
    }
}
fn committed(value: StockListingOutcome) -> StockListingReceipt {
    match value {
        StockListingOutcome::Committed(value) => *value,
        _ => panic!("retention must confirm"),
    }
}

#[tokio::test]
async fn native_original_observation_reopens_and_deduplicates_transport_encodings_without_replacement()
 {
    let r = Rig::new().await;
    let capture = r.capture("listing_fixed");
    let before = r
        .store
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    let receipt = r.retain(&capture).await;
    assert_eq!(receipt.transaction_id(), capture.transaction_id());
    assert_eq!(receipt.checkpoint().revision().get(), 1);
    assert!(receipt.selected_at_retention());
    let alias = serde_json::to_string_pretty(
        &serde_json::from_str::<serde_json::Value>(&vector("listing_fixed")).unwrap(),
    )
    .unwrap();
    assert_ne!(alias, vector("listing_fixed"));
    let alias = r.capture_raw(&alias);
    assert_ne!(alias.transaction_id(), capture.transaction_id());
    assert_eq!(r.retain(&alias).await, receipt);
    let after = r
        .store
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    assert_eq!(before.inventory_checkpoint(), after.inventory_checkpoint());
    assert_eq!(before.source_checkpoint(), after.source_checkpoint());
    let source = r
        .store
        .stock_listing_source(
            r.binding.clone(),
            receipt.checkpoint(),
            capture.listing_revision(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(source.raw_json(), vector("listing_fixed"));
    assert!(source.is_current_observed());
    assert!(matches!(
        source.interpret_at_observation().unwrap(),
        radroots_nostr::public_ingress::Interpretation::Listing(_)
    ));
    r.store.close().await.unwrap();
    let store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let source = store
        .stock_listing_head(
            r.binding.clone(),
            receipt.checkpoint(),
            capture.listing_revision().coordinate(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(source.receipt(), &receipt);
    assert_eq!(source.raw_json(), vector("listing_fixed"));
    store.close().await.unwrap();
}

#[tokio::test]
async fn protected_heads_match_independent_timestamp_and_id_oracles_without_profile_fallback() {
    for steps in vectors()["ranking_oracles"].as_array().unwrap() {
        let r = Rig::new().await;
        for step in steps.as_array().unwrap() {
            let capture = r.capture(step["input"].as_str().unwrap());
            r.retain(&capture).await;
            let head = r
                .store
                .stock_listing_head(
                    r.binding.clone(),
                    r.minimum().await,
                    capture.listing_revision().coordinate(),
                    r.enrollment.proof(),
                )
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                head.receipt().listing_revision().event_id().to_string(),
                step["head"].as_str().unwrap()
            );
        }
        let old = r.capture("listing_fixed");
        let original = r
            .store
            .stock_listing_source(
                r.binding.clone(),
                r.minimum().await,
                old.listing_revision(),
                r.enrollment.proof(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(original.raw_json(), vector("listing_fixed"));
        r.store.close().await.unwrap();
    }
    for (name, expected) in [
        ("listing_unsupported", "unsupported"),
        ("listing_unknown_field", "invalid"),
        ("listing_future", "clock_ahead"),
    ] {
        let r = Rig::new().await;
        let old = r.capture("listing_fixed");
        r.retain(&old).await;
        let next = r.capture(name);
        r.retain(&next).await;
        let head = r
            .store
            .stock_listing_head(
                r.binding.clone(),
                r.minimum().await,
                old.listing_revision().coordinate(),
                r.enrollment.proof(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(head.receipt().listing_revision(), next.listing_revision());
        let classification = match head.interpret_at_observation().unwrap() {
            radroots_nostr::public_ingress::Interpretation::Unsupported => "unsupported",
            radroots_nostr::public_ingress::Interpretation::Invalid => "invalid",
            radroots_nostr::public_ingress::Interpretation::ClockAhead => "clock_ahead",
            _ => panic!("must not fall back"),
        };
        assert_eq!(classification, expected);
        let old = r
            .store
            .stock_listing_source(
                r.binding.clone(),
                r.minimum().await,
                old.listing_revision(),
                r.enrollment.proof(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(!old.is_current_observed());
        assert!(old.receipt().selected_at_retention());
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn full_event_bounds_and_invalid_profile_bytes_are_retained_after_signature_admission() {
    for name in [
        "listing_exact_event_bound",
        "listing_large_profile",
        "listing_duplicate_d",
        "listing_ambiguous_d",
        "listing_contradictory_price",
        "listing_recurring_price",
        "listing_duplicate_profile",
    ] {
        let r = Rig::new().await;
        let capture = r.capture(name);
        let receipt = r.retain(&capture).await;
        let source = r
            .store
            .stock_listing_source(
                r.binding.clone(),
                receipt.checkpoint(),
                capture.listing_revision(),
                r.enrollment.proof(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(source.raw_json(), vector(name));
        if name == "listing_exact_event_bound" {
            assert_eq!(source.raw_json().len(), 65536);
        }
        r.store.close().await.unwrap();
    }
    let r = Rig::new().await;
    for name in ["bad_id", "bad_signature", "capability"] {
        let mut observed = ObservedHead::listing(
            r.enrollment.input().participant(),
            vectors()["d"].as_str().unwrap(),
        )
        .unwrap();
        assert!(observed.observe(&vector(name)).is_err());
        assert!(
            StockListingCapture::prepare(
                SellerAuthoritySelection::from_attachment(r.enrollment.input()),
                &observed
            )
            .is_err()
        );
    }
    let capture = r.capture("listing_fixed");
    let wrong = SellerStockConfig::new(
        participant(),
        participant(),
        "wrong",
        UnixSeconds::new(300).unwrap(),
    )
    .unwrap()
    .enroll(
        UnixSeconds::new(150).unwrap(),
        WorkspaceId::from_bytes([1; 32]),
        ExecutorId::from_bytes([2; 32]),
        GrantId::from_bytes([3; 32]),
        AttachmentProof::from_protected_input(&[4; 32]).unwrap(),
    )
    .unwrap()
    .0;
    let mut observed = ObservedHead::listing(
        r.enrollment.input().participant(),
        vectors()["d"].as_str().unwrap(),
    )
    .unwrap();
    observed.observe(&vector("listing_fixed")).unwrap();
    assert!(matches!(
        StockListingCapture::prepare(SellerAuthoritySelection::from_attachment(&wrong), &observed),
        Err(WorkspaceError::WrongNamespace)
    ));
    let wrong_proof = AttachmentProof::from_protected_input(&[9; 32]).unwrap();
    assert!(
        r.store
            .retain_stock_listing(r.binding.clone(), r.minimum().await, &capture, &wrong_proof)
            .await
            .is_err()
    );
    r.clock.0.store(300, Ordering::SeqCst);
    assert!(matches!(
        r.store
            .retain_stock_listing(
                r.binding.clone(),
                StockListingCheckpointRef::from_host_metadata(
                    r.enrollment.input().authority(),
                    radroots_domain::Sequence::new(0).unwrap(),
                    None
                )
                .unwrap(),
                &capture,
                r.enrollment.proof()
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantExpired))
    ));
    r.store.close().await.unwrap();
}

async fn database(r: &Rig) -> SqliteConnection {
    SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(r.path.join("stock_execution_v1.sqlite"))
            .foreign_keys(false),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn every_public_write_window_rolls_back_exact_progress_and_retained_originals() {
    for (initial, trigger) in [
        (
            false,
            "CREATE TRIGGER fault BEFORE INSERT ON stock_public_events BEGIN SELECT RAISE(ABORT,'fixture'); END",
        ),
        (
            false,
            "CREATE TRIGGER fault BEFORE INSERT ON stock_public_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        ),
        (
            false,
            "CREATE TRIGGER fault BEFORE INSERT ON stock_public_heads BEGIN SELECT RAISE(ABORT,'fixture'); END",
        ),
        (
            true,
            "CREATE TRIGGER fault BEFORE UPDATE ON stock_public_heads BEGIN SELECT RAISE(ABORT,'fixture'); END",
        ),
        (
            false,
            "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
        ),
    ] {
        let r = Rig::new().await;
        if initial {
            r.retain(&r.capture("listing_fixed")).await;
        }
        let minimum = r.minimum().await;
        let capture = r.capture("listing_unsupported");
        let mut raw = database(&r).await;
        sqlx::raw_sql(trigger).execute(&mut raw).await.unwrap();
        assert_eq!(
            r.store
                .retain_stock_listing(r.binding.clone(), minimum, &capture, r.enrollment.proof())
                .await,
            Err(OperationError::StorageFailure)
        );
        assert_eq!(r.minimum().await, minimum);
        let events: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_public_events")
            .fetch_one(&mut raw)
            .await
            .unwrap();
        assert_eq!(events, i64::from(initial));
        let parts: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_public_parts")
            .fetch_one(&mut raw)
            .await
            .unwrap();
        assert_eq!(parts, i64::from(initial));
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut raw)
            .await
            .unwrap();
        raw.close().await.unwrap();
        assert_eq!(
            r.retain(&capture).await.transaction_id(),
            capture.transaction_id()
        );
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn failed_public_commit_keeps_original_identity_and_requires_drain_and_reopen() {
    let r = Rig::new().await;
    let capture = r.capture("listing_fixed");
    let minimum = r.minimum().await;
    let mut raw = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_public_events BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut raw).await.unwrap();
    assert_eq!(
        r.store
            .retain_stock_listing(r.binding.clone(), minimum, &capture, r.enrollment.proof())
            .await
            .unwrap(),
        StockListingOutcome::Unknown {
            transaction: capture.transaction_id(),
            listing: capture.listing_revision()
        }
    );
    assert!(matches!(
        r.store
            .stock_listing_head(
                r.binding.clone(),
                minimum,
                capture.listing_revision().coordinate(),
                r.enrollment.proof()
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    r.store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fault; DROP TABLE deferred_fault")
        .execute(&mut raw)
        .await
        .unwrap();
    raw.close().await.unwrap();
    let store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert!(
        store
            .stock_listing_head(
                r.binding.clone(),
                minimum,
                capture.listing_revision().coordinate(),
                r.enrollment.proof()
            )
            .await
            .unwrap()
            .is_none()
    );
    let receipt = committed(
        store
            .retain_stock_listing(r.binding.clone(), minimum, &capture, r.enrollment.proof())
            .await
            .unwrap(),
    );
    assert_eq!(receipt.transaction_id(), capture.transaction_id());
    store.close().await.unwrap();
}

#[tokio::test]
async fn changed_projection_and_hidden_original_identity_refuse_without_repair() {
    for projection in [true, false] {
        let r = Rig::new().await;
        let original = r.capture("listing_fixed");
        r.retain(&original).await;
        r.retain(&r.capture("listing_unsupported")).await;
        let minimum = r.minimum().await;
        let mut raw = database(&r).await;
        if projection {
            sqlx::query("UPDATE stock_public_heads SET listing_d=?")
                .bind([0x77_u8; 16].as_slice())
                .execute(&mut raw)
                .await
                .unwrap();
        } else {
            sqlx::query("UPDATE stock_public_events SET event_id=? WHERE event_id=?")
                .bind([0x78_u8; 32].as_slice())
                .bind(original.listing_revision().event_id().as_bytes().as_slice())
                .execute(&mut raw)
                .await
                .unwrap();
        }
        assert!(
            r.store
                .stock_listing_source(
                    r.binding.clone(),
                    minimum,
                    original.listing_revision(),
                    r.enrollment.proof()
                )
                .await
                .is_err()
        );
        if !projection {
            assert!(
                r.store
                    .retain_stock_listing(
                        r.binding.clone(),
                        minimum,
                        &original,
                        r.enrollment.proof()
                    )
                    .await
                    .is_err()
            );
            sqlx::query("UPDATE stock_public_events SET event_id=? WHERE event_id=?")
                .bind(original.listing_revision().event_id().as_bytes().as_slice())
                .bind([0x78_u8; 32].as_slice())
                .execute(&mut raw)
                .await
                .unwrap();
        } else {
            sqlx::query("UPDATE stock_public_heads SET listing_d=?")
                .bind(
                    original
                        .listing_revision()
                        .coordinate()
                        .d()
                        .as_bytes()
                        .as_slice(),
                )
                .execute(&mut raw)
                .await
                .unwrap();
        }
        assert_eq!(
            r.store
                .stock_listing_source(
                    r.binding.clone(),
                    minimum,
                    original.listing_revision(),
                    r.enrollment.proof()
                )
                .await
                .unwrap()
                .unwrap()
                .raw_json(),
            vector("listing_fixed")
        );
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn physical_grants_have_no_public_source_access_and_exact_current_generation_is_required() {
    let r = Rig::new().await;
    let capture = r.capture("listing_fixed");
    let original = r.retain(&capture).await;
    let stale = r.minimum().await;
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
    assert_eq!(
        r.store
            .retain_stock_listing(r.binding.clone(), stale, &capture, r.enrollment.proof())
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    );
    let minimum = r.minimum().await;
    let binding = r
        .store
        .authenticate(child.input().clone(), child.proof())
        .await
        .unwrap();
    assert_eq!(
        r.store
            .retain_stock_listing(binding.clone(), minimum, &capture, child.proof())
            .await,
        Err(OperationError::Unauthorized)
    );
    assert!(matches!(
        r.store
            .stock_listing_head(
                binding,
                minimum,
                capture.listing_revision().coordinate(),
                child.proof()
            )
            .await,
        Err(OperationError::Unauthorized)
    ));
    let receipt = r.retain(&capture).await;
    assert_eq!(receipt, original);
    assert_eq!(minimum.revision(), stale.revision());
    assert_eq!(minimum.head(), stale.head());
    assert_ne!(minimum.authorization(), stale.authorization());
    let read = r
        .store
        .stock_listing_head(
            r.binding.clone(),
            minimum,
            capture.listing_revision().coordinate(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.selection_checkpoint(), minimum);
    assert_eq!(read.receipt(), &original);
    let fork = StockListingCheckpointRef::from_host_metadata(
        r.enrollment.input().authority(),
        minimum.revision(),
        Some(radroots_domain::CheckpointDigest::from_bytes([0x44; 32])),
    )
    .unwrap()
    .with_optional_authorization(minimum.authorization());
    assert!(
        r.store
            .stock_listing_head(
                r.binding.clone(),
                fork,
                capture.listing_revision().coordinate(),
                r.enrollment.proof()
            )
            .await
            .is_err()
    );
    r.clock.0.store(149, Ordering::SeqCst);
    assert!(
        r.store
            .stock_listing_source(
                r.binding.clone(),
                minimum,
                capture.listing_revision(),
                r.enrollment.proof()
            )
            .await
            .is_err()
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn acknowledged_public_progress_detects_whole_database_restore() {
    let r = Rig::new().await;
    let first = r.capture("listing_fixed");
    r.retain(&first).await;
    r.store.close().await.unwrap();
    let snapshot = r.fixture.root().join("old-public.sqlite");
    std::fs::copy(r.path.join("stock_execution_v1.sqlite"), &snapshot).unwrap();
    let store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let minimum = store
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap()
        .listing_checkpoint()
        .unwrap();
    let next = r.capture("listing_unsupported");
    let latest = committed(
        store
            .retain_stock_listing(r.binding.clone(), minimum, &next, r.enrollment.proof())
            .await
            .unwrap(),
    );
    store.close().await.unwrap();
    std::fs::copy(snapshot, r.path.join("stock_execution_v1.sqlite")).unwrap();
    let store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert!(
        store
            .stock_listing_head(
                r.binding.clone(),
                latest.checkpoint(),
                first.listing_revision().coordinate(),
                r.enrollment.proof()
            )
            .await
            .is_err()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn actual_inventory_private_sources_consent_and_grant_changes_preserve_public_progress() {
    use nostr::{EventBuilder, JsonUtil, Keys, Kind, Timestamp};
    use radroots_domain::{CommandContextData, OperationId, PoolId};
    use radroots_protocol::command::CapturedOrderAcceptance;
    use radroots_signing::{provider::ProviderCall, request::CancellationSignal};
    use radroots_storage::inventory::{InventoryOutcome, InventoryRequest, InventoryStore};
    use radroots_storage::workspace::seller_stock::StockResources;
    let r = Rig::new().await;
    let public = r.capture("listing_fixed");
    let first = r.retain(&public).await;
    let inventory = r
        .store
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
    let context = CommandContextData::new(
        OperationId::from_bytes([0x61; 32]),
        r.binding.actor(),
        r.enrollment.input().workspace_id(),
        r.enrollment.input().authority(),
        r.enrollment.input().route_digest(),
        None,
        None,
        UnixSeconds::new(250).unwrap(),
        None,
    )
    .unwrap();
    let wire = serde_json::json!({"type":"stock.create_pool","context":context,"input":{"id":PoolId::from_bytes([0x62;32]),"label":"source coexistence","unit":"each","tick_scale":"0","initial_quantity":"1"}});
    let request = InventoryRequest::from_protected_wire(
        r.binding.clone(),
        inventory,
        &serde_json::to_vec(&wire).unwrap(),
    )
    .unwrap();
    let created = match r
        .store
        .execute_inventory(
            request,
            r.enrollment
                .proof()
                .with_secret(|secret| AttachmentProof::from_protected_input(secret))
                .unwrap(),
        )
        .await
        .unwrap()
    {
        InventoryOutcome::Committed(receipt) => receipt,
        _ => panic!("pool must confirm"),
    };
    let sender = Keys::parse(&format!("{:064x}", 1)).unwrap();
    let recipient =
        nostr::PublicKey::from_hex(&r.enrollment.input().participant().to_hex()).unwrap();
    let rumor = EventBuilder::new(Kind::from(14), "synthetic source coexistence")
        .custom_created_at(Timestamp::from(150))
        .build(sender.public_key());
    let wrap = EventBuilder::gift_wrap(&sender, &recipient, rumor, [])
        .await
        .unwrap();
    let signer = radroots_nostr::signing::LocalSigner::with_clock(
        radroots_nostr::key::SecretKey::parse(&format!("{:064x}", 3)).unwrap(),
        || Ok(150),
    )
    .unwrap();
    let opened = radroots_nostr::commerce::open(
        &signer,
        ProviderCall::new(
            r.enrollment.input().participant(),
            std::num::NonZeroU64::new(2000).unwrap(),
            CancellationSignal::new(),
        ),
        &wrap.as_json(),
    )
    .await
    .unwrap();
    let private = StockSourceCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        &opened,
    )
    .unwrap();
    let state = r
        .store
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    let private_receipt = match r
        .store
        .retain_stock_source(
            r.binding.clone(),
            state.source_checkpoint().unwrap(),
            &private,
            r.enrollment.proof(),
        )
        .await
        .unwrap()
    {
        StockSourceOutcome::Committed(receipt) => receipt,
        _ => panic!("private evidence must confirm"),
    };
    let mut wire = serde_json::json!({"type":"orders.accept","context":context,"input":{"request":"21".repeat(32),"terms_digest":"24".repeat(32),"allocation":{"type":"automatic"}}});
    wire["context"]["operation_id"] = serde_json::json!(OperationId::from_bytes([0x63; 32]));
    wire["context"]["expected"] = serde_json::json!({"agreement":null,"buyer_head":"21".repeat(32),"checkpoint":{"coverage":"scope_satisfied","digest":"22".repeat(32),"missing":[],"sequence":"1"},"order_id":"23".repeat(32),"seller_sequence":"0"});
    let unapproved = CapturedOrderAcceptance::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    wire["context"]["approval"] = serde_json::json!({"intent_digest":unapproved.approval_intent_digest(),"scope":"orders.accept","expires_at":"250"});
    let consent = StockConsentCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        r.enrollment.input().grant_id(),
        StockConsentMode::ExecuteAndRecover,
        StockResources::all_pools(),
        UnixSeconds::new(240).unwrap(),
        CapturedOrderAcceptance::parse(&serde_json::to_vec(&wire).unwrap()).unwrap(),
    )
    .unwrap();
    let consent = match r
        .store
        .issue_stock_consent(
            r.binding.clone(),
            created.checkpoint(),
            &consent,
            r.enrollment.proof(),
        )
        .await
        .unwrap()
    {
        StockConsentOutcome::Committed(receipt) => receipt,
        _ => panic!("consent must confirm"),
    };
    let child = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        "coexistence".into(),
        UnixSeconds::new(290).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    assert!(matches!(
        r.store
            .enroll_stock_grant(
                r.binding.clone(),
                consent.checkpoint(),
                r.enrollment.proof(),
                &child
            )
            .await
            .unwrap(),
        StockGrantOutcome::Committed(_)
    ));
    let state = r
        .store
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    let current = state.listing_checkpoint().unwrap();
    assert_eq!(current.revision(), first.checkpoint().revision());
    assert_eq!(current.head(), first.checkpoint().head());
    let current_private = state.source_checkpoint().unwrap();
    assert_eq!(
        current_private.revision(),
        private_receipt.checkpoint().revision()
    );
    assert_eq!(current_private.head(), private_receipt.checkpoint().head());
    assert_eq!(state.inventory_checkpoint().unwrap().revision().get(), 2);
    let source = r
        .store
        .stock_listing_head(
            r.binding.clone(),
            current,
            public.listing_revision().coordinate(),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(source.raw_json(), vector("listing_fixed"));
    assert!(
        r.store
            .stock_source(
                r.binding.clone(),
                current_private,
                private.envelope_id(),
                r.enrollment.proof()
            )
            .await
            .unwrap()
            .is_some()
    );
    let pool = r
        .store
        .pool(
            r.binding.clone(),
            state.inventory_checkpoint().unwrap(),
            created.pool().id(),
            r.enrollment
                .proof()
                .with_secret(|secret| AttachmentProof::from_protected_input(secret))
                .unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pool.snapshot().balances().physical().available.get(), 1);
    r.store.close().await.unwrap();
}

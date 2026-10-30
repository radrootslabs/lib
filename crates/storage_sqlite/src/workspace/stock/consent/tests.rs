//! Actual native issuance tests. The captured request is deliberately only
//! command data; these tests do not assert causal admission or stock holds.
use super::*;
use crate::workspace::{tests::CryptoFixture, tests::participant};
use radroots_domain::{CommandContextData, PoolId};
use radroots_protocol::command::CapturedOrderAcceptance;
use radroots_storage::inventory::{InventoryOutcome, InventoryRequest, InventoryStore};
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "test_provider.rs"]
mod provider;

struct Clock(AtomicU64);
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        UnixSeconds::new(self.0.load(Ordering::SeqCst)).map_err(|_| WorkspaceError::Unavailable)
    }
}
struct Rig {
    fixture: CryptoFixture,
    path: std::path::PathBuf,
    store: NativeWorkspace,
    input: AttachmentInput,
    proof: AttachmentProof,
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    clock: Arc<Clock>,
}
impl Rig {
    async fn new() -> Self {
        Self::with_catalog(None).await
    }
    async fn with_catalog(catalog: Option<i64>) -> Self {
        let fixture = CryptoFixture::new();
        let path = fixture.root().join("consent");
        let clock = Arc::new(Clock(AtomicU64::new(100)));
        let (input, proof, store) = if let Some(catalog) = catalog {
            let (input, proof) =
                crate::workspace::stock::tests::literal_catalog(&fixture, &path, catalog).await;
            let store = NativeWorkspace::open_seller_stock(
                &path,
                input.clone(),
                fixture.crypto(),
                clock.clone(),
            )
            .await
            .unwrap();
            (input, proof, store)
        } else {
            let enrollment = StockEnrollment::prepare(
                SellerStockConfig::new(
                    participant(),
                    participant(),
                    "consent-owner",
                    UnixSeconds::new(200).unwrap(),
                )
                .unwrap(),
                clock.as_ref(),
            )
            .unwrap();
            let input = enrollment.input().clone();
            let proof = enrollment
                .proof()
                .with_secret(|v| AttachmentProof::from_protected_input(v))
                .unwrap();
            let store = NativeWorkspace::initialize_seller_stock(
                &path,
                &enrollment,
                fixture.crypto(),
                clock.clone(),
            )
            .await
            .unwrap();
            (input, proof, store)
        };
        let binding = store.authenticate(input.clone(), &proof).await.unwrap();
        let minimum = store
            .stock_authority(input.clone(), &proof)
            .await
            .unwrap()
            .inventory_checkpoint()
            .unwrap();
        Self {
            fixture,
            path,
            store,
            input,
            proof,
            binding,
            minimum,
            clock,
        }
    }
    fn command(&self, binding: &WorkspaceBinding, id: u8) -> CapturedOrderAcceptance {
        let context = CommandContextData::new(
            OperationId::from_bytes([id; 32]),
            binding.actor(),
            self.input.workspace_id(),
            self.input.authority(),
            self.input.route_digest(),
            None,
            None,
            UnixSeconds::new(150).unwrap(),
            None,
        )
        .unwrap();
        let mut wire = serde_json::json!({"type":"orders.accept","context":context,"input":{"request":"21".repeat(32),"terms_digest":"24".repeat(32),"allocation":{"type":"automatic"}}});
        wire["context"]["expected"] = serde_json::json!({"agreement":null,"buyer_head":"21".repeat(32),"checkpoint":{"coverage":"scope_satisfied","digest":"22".repeat(32),"missing":[],"sequence":"1"},"order_id":"23".repeat(32),"seller_sequence":"0"});
        let captured = CapturedOrderAcceptance::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
        wire["context"]["approval"] = serde_json::json!({"intent_digest":captured.approval_intent_digest(),"scope":"orders.accept","expires_at":"150"});
        CapturedOrderAcceptance::parse(&serde_json::to_vec(&wire).unwrap()).unwrap()
    }
    fn capture(&self, binding: &WorkspaceBinding, id: u8) -> StockConsentCapture {
        StockConsentCapture::prepare(
            SellerAuthoritySelection::from_attachment(&self.input),
            binding.input().grant_id(),
            StockConsentMode::ExecuteAndRecover,
            StockResources::all_pools(),
            UnixSeconds::new(150).unwrap(),
            self.command(binding, id),
        )
        .unwrap()
    }
    async fn physical(&self, id: u8, minimum: InventoryCheckpointRef) -> InventoryCheckpointRef {
        let context = CommandContextData::new(
            OperationId::from_bytes([id; 32]),
            self.binding.actor(),
            self.input.workspace_id(),
            self.input.authority(),
            self.input.route_digest(),
            None,
            None,
            UnixSeconds::new(190).unwrap(),
            None,
        )
        .unwrap();
        let wire = serde_json::json!({"type":"stock.create_pool","context":context,"input":{"id":PoolId::from_bytes([id;32]),"label":"consent-boundary","unit":"each","tick_scale":"0","initial_quantity":"1"}});
        let request = InventoryRequest::from_protected_wire(
            self.binding.clone(),
            minimum,
            &serde_json::to_vec(&wire).unwrap(),
        )
        .unwrap();
        match self
            .store
            .execute_inventory(
                request,
                self.proof
                    .with_secret(|v| AttachmentProof::from_protected_input(v))
                    .unwrap(),
            )
            .await
            .unwrap()
        {
            InventoryOutcome::Committed(receipt) => receipt.checkpoint(),
            _ => panic!("physical operation must confirm"),
        }
    }
}
fn committed(outcome: StockConsentOutcome) -> StockConsentReceipt {
    match outcome {
        StockConsentOutcome::Committed(value) => *value,
        _ => panic!("issuance must confirm"),
    }
}

#[tokio::test]
async fn retained_catalog_cannot_mint_initial_consent_for_captured_replacement_terms() {
    for catalog in [9, 19] {
        let r = Rig::with_catalog(Some(catalog)).await;
        let initial = r.command(&r.binding, 0x31);
        let mut wire: serde_json::Value = serde_json::from_slice(initial.protected_wire()).unwrap();
        wire["type"] = serde_json::json!("orders.install_terms");
        wire["input"] = serde_json::json!({"proposal":"61".repeat(32),"authorization":"62".repeat(32),"terms_digest":"24".repeat(32),"allocation":{"type":"automatic"}});
        wire["context"]["expected"]["agreement"] = serde_json::json!("63".repeat(32));
        wire["context"]["expected"]["seller_sequence"] = serde_json::json!("1");
        wire["context"]["approval"] = serde_json::Value::Null;
        let command = radroots_protocol::command::CapturedTermsInstallation::parse(
            &serde_json::to_vec(&wire).unwrap(),
        )
        .unwrap();
        wire["context"]["approval"] = serde_json::json!({"intent_digest":command.approval_intent_digest(),"scope":"orders.install_terms","expires_at":"150"});
        let command = radroots_protocol::command::CapturedTermsInstallation::parse(
            &serde_json::to_vec(&wire).unwrap(),
        )
        .unwrap();
        let capture = StockConsentCapture::prepare(
            SellerAuthoritySelection::from_attachment(&r.input),
            r.input.grant_id(),
            StockConsentMode::ExecuteAndRecover,
            StockResources::all_pools(),
            UnixSeconds::new(150).unwrap(),
            command,
        )
        .unwrap();
        assert_eq!(
            r.store
                .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof)
                .await
                .unwrap_err(),
            OperationError::UnsupportedOperation
        );
        let mut raw = SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(r.path.join("stock_execution_v1.sqlite")),
        )
        .await
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations")
                .fetch_one(&mut raw)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA user_version")
                .fetch_one(&mut raw)
                .await
                .unwrap(),
            catalog
        );
        raw.close().await.unwrap();
        let receipt = committed(
            r.store
                .issue_stock_consent(
                    r.binding.clone(),
                    r.minimum,
                    &r.capture(&r.binding, 0x32),
                    &r.proof,
                )
                .await
                .unwrap(),
        );
        assert_eq!(receipt.checkpoint().revision().get(), 1);
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn original_consent_replays_after_expiry_and_survives_physical_progress_and_reopen() {
    let r = Rig::new().await;
    let capture = r.capture(&r.binding, 0x31);
    let receipt = committed(
        r.store
            .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof)
            .await
            .unwrap(),
    );
    assert_eq!(
        receipt.control_operation_id(),
        capture.control_operation_id()
    );
    assert_eq!(receipt.transaction_id(), capture.transaction_id());
    assert_eq!(receipt.command_digest(), capture.command().digest());
    assert_eq!(receipt.checkpoint().revision().get(), 1);
    let minimum = r.physical(0x41, receipt.checkpoint()).await;
    assert_eq!(minimum.revision().get(), 2);
    let replay = r.capture(&r.binding, 0x31);
    assert_ne!(
        replay.control_operation_id(),
        capture.control_operation_id()
    );
    r.clock.0.store(160, Ordering::SeqCst);
    assert_eq!(
        committed(
            r.store
                .issue_stock_consent(r.binding.clone(), minimum, &replay, &r.proof)
                .await
                .unwrap()
        ),
        receipt
    );
    let fresh = r.capture(&r.binding, 0x32);
    assert_eq!(
        r.store
            .issue_stock_consent(r.binding.clone(), minimum, &fresh, &r.proof)
            .await,
        Err(OperationError::DeadlineExceeded)
    );
    assert_eq!(
        r.store
            .stock_consent(
                r.binding.clone(),
                minimum,
                capture.command().context().operation_id(),
                r.input.grant_id(),
                &r.proof
            )
            .await
            .unwrap(),
        Some(receipt.clone())
    );
    assert_eq!(
        r.store
            .inventory_operation(
                r.binding.clone(),
                minimum,
                capture.control_operation_id(),
                None,
                r.proof
                    .with_secret(|v| AttachmentProof::from_protected_input(v))
                    .unwrap()
            )
            .await,
        Err(OperationError::IdempotencyConflict)
    );
    r.store.close().await.unwrap();
    let store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.input.clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let binding = store.authenticate(r.input.clone(), &r.proof).await.unwrap();
    assert_eq!(
        store
            .stock_consent(
                binding,
                minimum,
                capture.command().context().operation_id(),
                r.input.grant_id(),
                &r.proof
            )
            .await
            .unwrap(),
        Some(receipt)
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn physical_grant_cannot_issue_business_consent_but_can_read_its_own_issuance() {
    let r = Rig::new().await;
    let subject = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(&r.input),
        "physical-device".into(),
        UnixSeconds::new(190).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    assert!(matches!(
        r.store
            .enroll_stock_grant(r.binding.clone(), r.minimum, &r.proof, &subject)
            .await
            .unwrap(),
        StockGrantOutcome::Committed(_)
    ));
    let minimum = r
        .store
        .stock_authority(r.input.clone(), &r.proof)
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
    let child = r
        .store
        .authenticate(subject.input().clone(), subject.proof())
        .await
        .unwrap();
    let capture = r.capture(&child, 0x31);
    assert_eq!(
        r.store
            .issue_stock_consent(child.clone(), minimum, &capture, subject.proof())
            .await,
        Err(OperationError::Unauthorized)
    );
    let admin = r
        .store
        .authenticate(r.input.clone(), &r.proof)
        .await
        .unwrap();
    let receipt = committed(
        r.store
            .issue_stock_consent(admin, minimum, &capture, &r.proof)
            .await
            .unwrap(),
    );
    assert_eq!(
        r.store
            .stock_consent(
                child,
                receipt.checkpoint(),
                capture.command().context().operation_id(),
                subject.input().grant_id(),
                subject.proof()
            )
            .await
            .unwrap(),
        Some(receipt.clone())
    );
    assert_eq!(
        r.store
            .stock_consent(
                r.binding.clone(),
                receipt.checkpoint(),
                OperationId::from_bytes([0x55; 32]),
                r.input.grant_id(),
                &r.proof
            )
            .await
            .unwrap(),
        None
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn changed_natural_index_cannot_turn_retained_consent_into_absence() {
    let r = Rig::new().await;
    let capture = r.capture(&r.binding, 0x31);
    let receipt = committed(
        r.store
            .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof)
            .await
            .unwrap(),
    );
    let minimum = r.physical(0x41, receipt.checkpoint()).await;
    let mut raw = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(r.path.join("stock_execution_v1.sqlite")),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE stock_operations SET related_operation=? WHERE operation_id=?")
        .bind([0x77_u8; 32].as_slice())
        .bind(receipt.control_operation_id().as_bytes().as_slice())
        .execute(&mut raw)
        .await
        .unwrap();
    assert_eq!(
        r.store
            .stock_consent(
                r.binding.clone(),
                minimum,
                capture.command().context().operation_id(),
                r.input.grant_id(),
                &r.proof
            )
            .await,
        Err(OperationError::RecoveryRequired)
    );
    assert_eq!(
        r.store
            .issue_stock_consent(r.binding.clone(), minimum, &capture, &r.proof)
            .await,
        Err(OperationError::RecoveryRequired)
    );
    sqlx::query("UPDATE stock_operations SET related_operation=? WHERE operation_id=?")
        .bind(
            capture
                .command()
                .context()
                .operation_id()
                .as_bytes()
                .as_slice(),
        )
        .bind(receipt.control_operation_id().as_bytes().as_slice())
        .execute(&mut raw)
        .await
        .unwrap();
    assert_eq!(
        r.store
            .stock_consent(
                r.binding.clone(),
                minimum,
                capture.command().context().operation_id(),
                r.input.grant_id(),
                &r.proof
            )
            .await
            .unwrap(),
        Some(receipt)
    );
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn every_issuance_write_window_rolls_back_without_advancing_the_root() {
    for trigger in [
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operations BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
    ] {
        let r = Rig::new().await;
        let capture = r.capture(&r.binding, 0x31);
        let mut raw = SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(r.path.join("stock_execution_v1.sqlite")),
        )
        .await
        .unwrap();
        sqlx::raw_sql(trigger).execute(&mut raw).await.unwrap();
        assert_eq!(
            r.store
                .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof)
                .await,
            Err(OperationError::StorageFailure)
        );
        assert_eq!(
            r.store
                .stock_authority(r.input.clone(), &r.proof)
                .await
                .unwrap()
                .inventory_checkpoint()
                .unwrap(),
            r.minimum
        );
        let count:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM stock_operations)+(SELECT count(*) FROM stock_operation_parts)").fetch_one(&mut raw).await.unwrap();
        assert_eq!(count, 0);
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut raw)
            .await
            .unwrap();
        assert_eq!(
            committed(
                r.store
                    .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof)
                    .await
                    .unwrap()
            )
            .transaction_id(),
            capture.transaction_id()
        );
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn failed_commit_keeps_the_original_anchor_and_fences_until_close_and_reopen() {
    let r = Rig::new().await;
    let capture = r.capture(&r.binding, 0x31);
    let mut raw = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(r.path.join("stock_execution_v1.sqlite")),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_operations BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut raw).await.unwrap();
    assert_eq!(
        r.store
            .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof)
            .await
            .unwrap(),
        StockConsentOutcome::Unknown {
            control_operation: capture.control_operation_id(),
            transaction: capture.transaction_id(),
            command_operation: capture.command().context().operation_id(),
            subject: r.input.grant_id()
        }
    );
    assert_eq!(
        r.store
            .stock_consent(
                r.binding.clone(),
                r.minimum,
                capture.command().context().operation_id(),
                r.input.grant_id(),
                &r.proof
            )
            .await,
        Err(OperationError::RecoveryRequired)
    );
    r.store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fault; DROP TABLE deferred_fault")
        .execute(&mut raw)
        .await
        .unwrap();
    raw.close().await.unwrap();
    let store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.input.clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        store
            .stock_consent(
                r.binding.clone(),
                r.minimum,
                capture.command().context().operation_id(),
                r.input.grant_id(),
                &r.proof
            )
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        committed(
            store
                .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof)
                .await
                .unwrap()
        )
        .transaction_id(),
        capture.transaction_id()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn exact_resources_are_immutable_and_current_access_precedes_replay() {
    let r = Rig::new().await;
    let capture = r.capture(&r.binding, 0x31);
    let receipt = committed(
        r.store
            .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof)
            .await
            .unwrap(),
    );
    let changed = StockConsentCapture::prepare(
        SellerAuthoritySelection::from_attachment(&r.input),
        r.input.grant_id(),
        StockConsentMode::ExecuteAndRecover,
        StockResources::pools(vec![PoolId::from_bytes([0x41; 32])]).unwrap(),
        UnixSeconds::new(150).unwrap(),
        r.command(&r.binding, 0x31),
    )
    .unwrap();
    assert_eq!(
        r.store
            .issue_stock_consent(r.binding.clone(), receipt.checkpoint(), &changed, &r.proof)
            .await,
        Err(OperationError::IdempotencyConflict)
    );
    let wrong = AttachmentProof::from_protected_input(&[0xA7; 32]).unwrap();
    assert_eq!(
        r.store
            .issue_stock_consent(r.binding.clone(), receipt.checkpoint(), &capture, &wrong)
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unauthorized))
    );
    r.clock.0.store(99, Ordering::SeqCst);
    assert!(
        r.store
            .issue_stock_consent(r.binding.clone(), receipt.checkpoint(), &capture, &r.proof)
            .await
            .is_err()
    );
    r.clock.0.store(200, Ordering::SeqCst);
    assert_eq!(
        r.store
            .stock_consent(
                r.binding.clone(),
                receipt.checkpoint(),
                capture.command().context().operation_id(),
                r.input.grant_id(),
                &r.proof
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantExpired))
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn recovery_only_can_bind_an_expired_original_command_to_an_explicit_replacement() {
    let r = Rig::new().await;
    let replacement = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(&r.input),
        "replacement".into(),
        UnixSeconds::new(190).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    assert!(matches!(
        r.store
            .enroll_stock_grant(r.binding.clone(), r.minimum, &r.proof, &replacement)
            .await
            .unwrap(),
        StockGrantOutcome::Committed(_)
    ));
    let minimum = r
        .store
        .stock_authority(r.input.clone(), &r.proof)
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
    let command = r.command(&r.binding, 0x31);
    assert!(
        StockConsentCapture::prepare(
            SellerAuthoritySelection::from_attachment(&r.input),
            replacement.input().grant_id(),
            StockConsentMode::ExecuteAndRecover,
            StockResources::all_pools(),
            UnixSeconds::new(150).unwrap(),
            r.command(&r.binding, 0x31)
        )
        .is_err()
    );
    let capture = StockConsentCapture::prepare(
        SellerAuthoritySelection::from_attachment(&r.input),
        replacement.input().grant_id(),
        StockConsentMode::RecoverOnly,
        StockResources::all_pools(),
        UnixSeconds::new(180).unwrap(),
        command,
    )
    .unwrap();
    r.clock.0.store(160, Ordering::SeqCst);
    let admin = r
        .store
        .authenticate(r.input.clone(), &r.proof)
        .await
        .unwrap();
    let receipt = committed(
        r.store
            .issue_stock_consent(admin, minimum, &capture, &r.proof)
            .await
            .unwrap(),
    );
    assert_eq!(receipt.mode(), StockConsentMode::RecoverOnly);
    assert_eq!(receipt.command_digest(), capture.command().digest());
    let subject = r
        .store
        .authenticate(replacement.input().clone(), replacement.proof())
        .await
        .unwrap();
    assert_eq!(
        r.store
            .stock_consent(
                subject,
                receipt.checkpoint(),
                capture.command().context().operation_id(),
                replacement.input().grant_id(),
                replacement.proof()
            )
            .await
            .unwrap(),
        Some(receipt)
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn retained_catalog7_issues_original_consent_without_gaining_public_source_tables() {
    use radroots_domain::listing::ListingCoordinate;
    use radroots_storage::workspace::seller_stock::StockListingCheckpointRef;
    let r = Rig::with_catalog(Some(7)).await;
    let capture = r.capture(&r.binding, 0x31);
    let receipt = committed(
        r.store
            .issue_stock_consent(r.binding.clone(), r.minimum, &capture, &r.proof)
            .await
            .unwrap(),
    );
    assert!(
        r.store
            .stock_authority(r.input.clone(), &r.proof)
            .await
            .unwrap()
            .listing_checkpoint()
            .is_none()
    );
    let public = StockListingCheckpointRef::from_host_metadata(
        r.input.authority(),
        Sequence::new(0).unwrap(),
        None,
    )
    .unwrap()
    .with_optional_authorization(r.minimum.authorization());
    assert!(matches!(
        r.store
            .stock_listing_head(
                r.binding.clone(),
                public,
                ListingCoordinate::new(r.input.participant(), "04".repeat(16).parse().unwrap()),
                &r.proof
            )
            .await,
        Err(OperationError::UnsupportedOperation)
    ));
    let minimum = r.physical(0x41, receipt.checkpoint()).await;
    r.store.close().await.unwrap();
    let store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.input.clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        store
            .stock_consent(
                r.binding.clone(),
                minimum,
                capture.command().context().operation_id(),
                r.input.grant_id(),
                &r.proof
            )
            .await
            .unwrap(),
        Some(receipt)
    );
    store.close().await.unwrap();
}

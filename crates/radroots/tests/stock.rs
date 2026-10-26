#![cfg(all(feature = "native", unix))]
//! Actual ordinary-client private inventory, not a model of SQL effects.
use radroots::stock::{
    CreatePoolInput, InventoryCheckpointRef, InventoryOutcome, InventoryReceipt, MapOptionInput,
    PoolCorrectionInput,
};
use radroots::{
    Client, Code, CommandContext, NativeConfig, OperationId, StartMode, WorkspaceSession,
};
use radroots_domain::{
    CommandDigest, ListingOptionId, PoolId, PublicKey, Quantity, Sequence, UnixSeconds,
    listing::{ListingCoordinate, ListingId, MeasurementUnit},
    stock::{AvailableStockCorrection, OptionMappingKey, PoolUnit},
};
use radroots_secrets::{
    SecretId, SecretRef,
    envelope::Nonce,
    file::{FileOpenMode, FileProvider},
    id::{BackendKind, KeyVersion},
    provider::AccessPolicy,
    wrapping::SecretMaterial,
};
use radroots_storage::{
    event::SourceGeneration,
    workspace::{AttachmentProof, WorkspaceClock, WorkspaceError},
};
use radroots_storage_sqlite::{
    OpenMode, OpenOptions, Paths,
    workspace::{
        LocalOutboxConfig, NativeWorkspace, SellerStockConfig, StockEnrollment, WorkspaceCrypto,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

struct Clock(AtomicU64);
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        UnixSeconds::new(self.0.load(Ordering::SeqCst)).map_err(|_| WorkspaceError::Unavailable)
    }
}
fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn reference() -> SecretRef {
    SecretRef::new(
        SecretId::parse("workspace-key").unwrap(),
        BackendKind::File,
        KeyVersion::new(1).unwrap(),
    )
}
fn proof(value: &AttachmentProof) -> AttachmentProof {
    value
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
        .unwrap()
}
struct Fixture {
    _temp: tempfile::TempDir,
    root: std::path::PathBuf,
    provider: Arc<FileProvider>,
    clock: Arc<Clock>,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let provider = Arc::new(
            FileProvider::open(
                root.join("vault"),
                FileOpenMode::CreateNew,
                SecretMaterial::from_slice(&[0xA5; 32]).unwrap(),
            )
            .unwrap(),
        );
        provider
            .provision(
                &reference(),
                &SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
                Nonce::new([0x11; 24]),
            )
            .unwrap();
        Self {
            _temp: temp,
            root,
            provider,
            clock: Arc::new(Clock(AtomicU64::new(100))),
        }
    }
    fn crypto(&self) -> WorkspaceCrypto {
        WorkspaceCrypto::new(
            self.provider.clone(),
            reference(),
            SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
            AccessPolicy::standard(),
        )
        .unwrap()
    }
    fn options(&self) -> OpenOptions {
        let path = self.root.join("cache");
        std::fs::create_dir_all(&path).unwrap();
        OpenOptions::new(Paths::from_directory(path).unwrap(), OpenMode::Create)
            .with_source_generation(SourceGeneration::new([17; 32]).unwrap(), 1)
            .unwrap()
    }
    async fn client(&self, workspace: NativeWorkspace) -> Client {
        Client::open(NativeConfig::public_read(self.options()).with_workspace(workspace))
            .await
            .unwrap()
    }
    async fn owner(&self) -> (Client, NativeWorkspace, StockEnrollment) {
        let enrollment = StockEnrollment::prepare(
            SellerStockConfig::new(
                key(),
                key(),
                "synthetic-stock-owner",
                UnixSeconds::new(300).unwrap(),
            )
            .unwrap(),
            self.clock.as_ref(),
        )
        .unwrap();
        let store = NativeWorkspace::initialize_seller_stock(
            &self.root.join("stock"),
            &enrollment,
            self.crypto(),
            self.clock.clone(),
        )
        .await
        .unwrap();
        (self.client(store.clone()).await, store, enrollment)
    }
}
fn context(scope: &WorkspaceSession, op: u8) -> CommandContext {
    scope
        .command_context(
            OperationId::from_bytes([op; 32]),
            None,
            None,
            UnixSeconds::new(150).unwrap(),
        )
        .unwrap()
}
fn pool() -> PoolId {
    PoolId::from_bytes([70; 32])
}
fn quantity(value: &str) -> Quantity {
    Quantity::new(value.parse().unwrap(), MeasurementUnit::Each).unwrap()
}
fn creation() -> CreatePoolInput {
    CreatePoolInput::new(
        pool(),
        "private test vegetables",
        PoolUnit::new(MeasurementUnit::Each, 0).unwrap(),
        quantity("10"),
    )
    .unwrap()
}
fn correction(version: u64, kind: AvailableStockCorrection, amount: &str) -> PoolCorrectionInput {
    PoolCorrectionInput::new(
        pool(),
        Sequence::new(version).unwrap(),
        kind,
        amount.parse().unwrap(),
        "synthetic audited correction",
    )
    .unwrap()
}
fn committed(value: InventoryOutcome) -> InventoryReceipt {
    let InventoryOutcome::Committed(value) = value else {
        panic!("effect must confirm")
    };
    *value
}
fn native_future<T: Send>(future: T) -> T {
    future
}

#[tokio::test]
async fn ordinary_stock_effects_queries_original_replay_and_reopen_use_the_selected_native_owner() {
    let f = Fixture::new();
    let (client, native, enrollment) = f.owner().await;
    let scope = client
        .attach(enrollment.input().clone(), proof(enrollment.proof()))
        .await
        .unwrap();
    let first_minimum = native
        .stock_authority(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
    client.start(StartMode::LocalOnly).await.unwrap();
    let stock = client.stock(&scope);
    drop(stock.create_pool(context(&scope, 80), first_minimum, creation()));
    assert!(stock.pool(first_minimum, pool()).await.unwrap().is_none());
    let original = committed(
        native_future(stock.create_pool(context(&scope, 80), first_minimum, creation()))
            .await
            .unwrap(),
    );
    assert_eq!(original.pool().version().get(), 1);
    assert_eq!(original.pool().balances().physical().available.get(), 10);
    let added = committed(
        stock
            .adjust(
                context(&scope, 81),
                original.checkpoint(),
                correction(1, AvailableStockCorrection::Add, "3"),
            )
            .await
            .unwrap(),
    );
    let removed = committed(
        stock
            .adjust(
                context(&scope, 82),
                added.checkpoint(),
                correction(2, AvailableStockCorrection::Remove, "2"),
            )
            .await
            .unwrap(),
    );
    let spoiled = committed(
        stock
            .spoil(
                context(&scope, 83),
                removed.checkpoint(),
                correction(3, AvailableStockCorrection::Spoil, "1"),
            )
            .await
            .unwrap(),
    );
    assert_eq!(spoiled.pool().version().get(), 4);
    assert_eq!(spoiled.pool().balances().introduced().get(), 13);
    assert_eq!(spoiled.pool().balances().physical().available.get(), 10);
    assert_eq!(spoiled.pool().balances().physical().removed.get(), 2);
    assert_eq!(spoiled.pool().balances().physical().waste.get(), 1);
    let key = OptionMappingKey::new(
        ListingCoordinate::new(key(), ListingId::from_bytes([89; 16])),
        ListingOptionId::from_bytes([90; 32]),
    );
    let mapped = committed(
        stock
            .map_option(
                context(&scope, 84),
                spoiled.checkpoint(),
                MapOptionInput::new(key, pool(), quantity("2"), Sequence::new(0).unwrap()).unwrap(),
            )
            .await
            .unwrap(),
    );
    assert_eq!(mapped.pool(), spoiled.pool());
    assert_eq!(
        stock
            .option_mapping(mapped.checkpoint(), key)
            .await
            .unwrap(),
        mapped.mapping()
    );
    let current = mapped.checkpoint();
    assert_eq!(
        stock
            .pool(current, pool())
            .await
            .unwrap()
            .unwrap()
            .snapshot(),
        spoiled.pool()
    );
    assert_eq!(
        stock
            .operation(
                current,
                original.attempt().operation_id(),
                Some(original.attempt().command_digest())
            )
            .await
            .unwrap()
            .unwrap(),
        original
    );
    assert_eq!(
        stock
            .adjust(
                context(&scope, 85),
                current,
                correction(1, AvailableStockCorrection::Add, "1")
            )
            .await
            .unwrap_err()
            .code(),
        Code::StaleState
    );
    assert_eq!(
        stock
            .adjust(
                context(&scope, 85),
                current,
                correction(4, AvailableStockCorrection::Spoil, "1")
            )
            .await
            .unwrap_err()
            .code(),
        Code::InvalidInput
    );
    assert_eq!(
        stock
            .spoil(
                context(&scope, 85),
                current,
                correction(4, AvailableStockCorrection::Add, "1")
            )
            .await
            .unwrap_err()
            .code(),
        Code::InvalidInput
    );
    let missing = OperationId::from_bytes([99; 32]);
    let digest = CommandDigest::from_bytes([98; 32]);
    // The native original-preparation loader reports a missing selected T0 as
    // StaleState, before any consent lookup or cleanup. Preserve that refusal.
    assert_eq!(
        client
            .operations(&scope)
            .abort_proven_unissued(current, missing, digest)
            .await
            .unwrap_err()
            .code(),
        Code::StaleState
    );
    assert_eq!(
        client
            .operations(&scope)
            .preparation_abort(current, missing, digest)
            .await
            .unwrap_err()
            .code(),
        Code::StaleState
    );
    f.clock.0.store(160, Ordering::SeqCst);
    let replay = committed(
        stock
            .create_pool(context(&scope, 80), first_minimum, creation())
            .await
            .unwrap(),
    );
    assert_eq!(replay, original);
    assert_eq!(
        stock
            .pool(current, pool())
            .await
            .unwrap()
            .unwrap()
            .snapshot(),
        spoiled.pool()
    );
    client.close().await.unwrap();
    assert!(
        native
            .authenticate(enrollment.input().clone(), enrollment.proof())
            .await
            .is_err()
    );
    let native = NativeWorkspace::open_seller_stock(
        &f.root.join("stock"),
        enrollment.input().clone(),
        f.crypto(),
        f.clock.clone(),
    )
    .await
    .unwrap();
    let client = f.client(native.clone()).await;
    let scope = client
        .attach(enrollment.input().clone(), proof(enrollment.proof()))
        .await
        .unwrap();
    client.start(StartMode::LocalOnly).await.unwrap();
    assert_eq!(
        client
            .stock(&scope)
            .pool(current, pool())
            .await
            .unwrap()
            .unwrap()
            .snapshot(),
        spoiled.pool()
    );
    assert_eq!(
        client
            .stock(&scope)
            .operation(
                current,
                original.attempt().operation_id(),
                Some(original.attempt().command_digest())
            )
            .await
            .unwrap()
            .unwrap(),
        original
    );
    client.close().await.unwrap();
}

#[tokio::test]
async fn identical_but_foreign_and_closed_scopes_cannot_reach_the_selected_owner() {
    let f = Fixture::new();
    let (client, native, enrollment) = f.owner().await;
    let scope = client
        .attach(enrollment.input().clone(), proof(enrollment.proof()))
        .await
        .unwrap();
    let foreign = WorkspaceSession::attach(
        Arc::new(native.clone()),
        enrollment.input().clone(),
        proof(enrollment.proof()),
    )
    .await
    .unwrap();
    let minimum = native
        .stock_authority(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
    client.start(StartMode::LocalOnly).await.unwrap();
    assert_eq!(
        client
            .stock(&foreign)
            .pool(minimum, pool())
            .await
            .unwrap_err()
            .code(),
        Code::Unauthorized
    );
    assert_eq!(
        client
            .stock(&scope)
            .create_pool(context(&foreign, 80), minimum, creation())
            .await
            .unwrap_err()
            .code(),
        Code::Unauthorized
    );
    scope.close();
    assert_eq!(
        client
            .stock(&scope)
            .pool(minimum, pool())
            .await
            .unwrap_err()
            .code(),
        Code::CapabilityUnavailable
    );
    foreign.close();
    client.close().await.unwrap();
    assert_eq!(
        client
            .stock(&scope)
            .pool(minimum, pool())
            .await
            .unwrap_err()
            .code(),
        Code::CapabilityUnavailable
    );
}

#[tokio::test]
async fn public_cache_and_preview_never_adopt_a_foreign_stock_workspace() {
    let f = Fixture::new();
    let (owner, native, enrollment) = f.owner().await;
    let scope = owner
        .attach(enrollment.input().clone(), proof(enrollment.proof()))
        .await
        .unwrap();
    let minimum = native
        .stock_authority(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
    let public = tempfile::tempdir().unwrap();
    let options = OpenOptions::new(
        Paths::from_directory(public.path()).unwrap(),
        OpenMode::Create,
    )
    .with_source_generation(SourceGeneration::new([18; 32]).unwrap(), 1)
    .unwrap();
    let cache = Client::open(NativeConfig::public_read(options))
        .await
        .unwrap();
    let preview = Client::preview();
    for client in [&cache, &preview] {
        client.start(StartMode::LocalOnly).await.unwrap();
        assert_eq!(
            client
                .stock(&scope)
                .create_pool(context(&scope, 80), minimum, creation())
                .await
                .unwrap_err()
                .code(),
            Code::MissingAuthority
        );
        assert_eq!(
            client
                .operations(&scope)
                .abort_proven_unissued(
                    minimum,
                    OperationId::from_bytes([99; 32]),
                    CommandDigest::from_bytes([98; 32])
                )
                .await
                .unwrap_err()
                .code(),
            Code::MissingAuthority
        );
        client.close().await.unwrap();
    }
    native
        .authenticate(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap();
    assert_eq!(
        native
            .stock_authority(enrollment.input().clone(), enrollment.proof())
            .await
            .unwrap()
            .inventory_checkpoint()
            .unwrap(),
        minimum
    );
    owner.close().await.unwrap();
}

#[tokio::test]
async fn ordinary_outbox_refuses_cleanup_and_never_creates_stock_authority() {
    let f = Fixture::new();
    let path = f.root.join("outbox");
    let (native, input, credential) = NativeWorkspace::initialize_local_outbox(
        &path,
        LocalOutboxConfig::new(
            key(),
            key(),
            "synthetic-outbox",
            UnixSeconds::new(300).unwrap(),
        )
        .unwrap(),
        f.crypto(),
        f.clock.clone(),
    )
    .await
    .unwrap();
    let client = f.client(native.clone()).await;
    let scope = client
        .attach(input.clone(), proof(&credential))
        .await
        .unwrap();
    let minimum = InventoryCheckpointRef::from_host_metadata(
        input.authority(),
        Sequence::new(0).unwrap(),
        None,
    )
    .unwrap();
    client.start(StartMode::LocalOnly).await.unwrap();
    let operation = OperationId::from_bytes([99; 32]);
    let digest = CommandDigest::from_bytes([98; 32]);
    assert_eq!(
        client
            .operations(&scope)
            .abort_proven_unissued(minimum, operation, digest)
            .await
            .unwrap_err()
            .code(),
        Code::CapabilityUnavailable
    );
    assert_eq!(
        client
            .operations(&scope)
            .preparation_abort(minimum, operation, digest)
            .await
            .unwrap_err()
            .code(),
        Code::CapabilityUnavailable
    );
    assert_eq!(
        client
            .stock(&scope)
            .create_pool(context(&scope, 80), minimum, creation())
            .await
            .unwrap_err()
            .code(),
        Code::Unauthorized
    );
    assert!(!path.join("stock_execution_v1.sqlite").exists());
    native.authenticate(input, &credential).await.unwrap();
    client.close().await.unwrap();
}

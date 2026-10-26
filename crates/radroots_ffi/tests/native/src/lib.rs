//! Synthetic native host for actual generated-language qualification.
#![forbid(unsafe_code)]
mod relay;
mod signer;

use radroots::{Client, NativeConfig};
use radroots_domain::{PublicKey, UnixSeconds};
use radroots_ffi::{NativeClient, SessionState, Workspace};
use radroots_secrets::{
    SecretId, SecretRef,
    envelope::Nonce,
    file::{FileOpenMode, FileProvider},
    id::{BackendKind, KeyVersion},
    provider::AccessPolicy,
    wrapping::SecretMaterial,
};
use radroots_signing::{
    provider::{ProviderError, SignerCallbacks, SignerHandle},
    signer::BoxFuture,
};
use radroots_storage::{
    event::SourceGeneration,
    workspace::{AttachmentInput, AttachmentProof, WorkspaceClock, WorkspaceError},
};
use radroots_storage_sqlite::{
    OpenMode, OpenOptions, Paths,
    workspace::{LocalOutboxConfig, NativeWorkspace, WorkspaceCrypto},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

uniffi::setup_scaffolding!();
radroots_ffi::uniffi_reexport_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FixtureError {
    #[error("native fixture unavailable")]
    Unavailable,
    #[error("native fixture still owns an open client")]
    Busy,
}
pub(crate) fn now() -> Result<u64, FixtureError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|_| FixtureError::Unavailable)
}
struct Clock;
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        UnixSeconds::new(now().map_err(|_| WorkspaceError::Unavailable)?)
            .map_err(|_| WorkspaceError::Unavailable)
    }
}
struct Callbacks {
    closed: Arc<AtomicU64>,
}
impl SignerCallbacks for Callbacks {
    fn quiesce(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async { Ok(()) })
    }
    fn close(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        self.closed.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone, uniffi::Record)]
pub struct Session {
    pub client: Arc<NativeClient>,
    pub workspace: Arc<Workspace>,
}
#[derive(Clone, uniffi::Record)]
pub struct Measurements {
    pub signing_calls: String,
    pub legacy_signing_calls: String,
    pub relay_reads: String,
    pub closed_callbacks: String,
    pub original_signed_events: Vec<String>,
}

/// Explicit test-host assembly choices; evaluation remains in shared Rust.
#[derive(Clone, Copy, uniffi::Enum)]
pub enum FixturePolicy {
    All,
    Any,
    Quorum,
    Required,
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum FixtureRejection {
    Malformed,
    Oversized,
    Foreign,
}

/// Test resources and counters. The shared client owns every command effect.
#[derive(uniffi::Object)]
pub struct Host {
    root: PathBuf,
    provider: Arc<FileProvider>,
    clock: Arc<Clock>,
    selected: AttachmentInput,
    proof: AttachmentProof,
    signer: Arc<signer::MeasuredSigner>,
    closed: Arc<AtomicU64>,
    relay: relay::Relay,
    initial: Session,
    clients: Mutex<Vec<Arc<NativeClient>>>,
    lifecycle: tokio::sync::Mutex<()>,
}
#[uniffi::export(async_runtime = "tokio")]
impl Host {
    /// Explicit test setup, with real protected grant creation and no commands.
    #[uniffi::constructor]
    pub async fn open(
        seller: bool,
        unknown_signing: bool,
        pause_after_signing: bool,
    ) -> Result<Arc<Self>, FixtureError> {
        Self::open_with_policy(
            seller,
            unknown_signing,
            pause_after_signing,
            FixturePolicy::All,
        )
        .await
    }
    #[uniffi::constructor]
    pub async fn open_with_policy(
        seller: bool,
        unknown_signing: bool,
        pause_after_signing: bool,
        policy: FixturePolicy,
    ) -> Result<Arc<Self>, FixtureError> {
        let directory = tempfile::Builder::new()
            .prefix("radroots_native_binding_fixture_")
            .tempdir()
            .map_err(|_| FixtureError::Unavailable)?;
        // Keep failed or observer-lost fixtures recoverable. Only explicit
        // cleanup after confirmed client closure removes this owned namespace.
        let root = directory
            .keep()
            .canonicalize()
            .map_err(|_| FixtureError::Unavailable)?;
        let provider = Arc::new(
            FileProvider::open(root.join("vault"), FileOpenMode::CreateNew, material(0xa5)?)
                .map_err(|_| FixtureError::Unavailable)?,
        );
        provider
            .provision(&reference()?, &material(0x41)?, Nonce::new([0x11; 24]))
            .map_err(|_| FixtureError::Unavailable)?;
        let clock = Arc::new(Clock);
        let expiry = UnixSeconds::new(now()? + 600).map_err(|_| FixtureError::Unavailable)?;
        let config = if seller {
            LocalOutboxConfig::seller(key()?, key()?, "native-binding-fixture", expiry)
        } else {
            LocalOutboxConfig::new(key()?, key()?, "native-binding-fixture", expiry)
        }
        .map_err(|_| FixtureError::Unavailable)?;
        let (store, selected, proof) = NativeWorkspace::initialize_local_outbox(
            &root.join("outbox"),
            config,
            crypto(provider.clone())?,
            clock.clone(),
        )
        .await
        .map_err(|_| FixtureError::Unavailable)?;
        let relay = relay::Relay::new(policy).await?;
        let signer = signer::MeasuredSigner::new(unknown_signing, pause_after_signing);
        let closed = Arc::new(AtomicU64::new(0));
        let initial = open_client(
            &root.join("cache0"),
            store,
            &selected,
            &proof,
            signer.clone(),
            closed.clone(),
            &relay,
        )
        .await?;
        Ok(Arc::new(Self {
            root,
            provider,
            clock,
            selected,
            proof,
            signer,
            closed,
            relay,
            clients: Mutex::new(vec![initial.client.clone()]),
            initial,
            lifecycle: tokio::sync::Mutex::new(()),
        }))
    }
    pub fn session(&self) -> Session {
        self.initial.clone()
    }
    pub fn unix_seconds(&self) -> Result<String, FixtureError> {
        now().map(|v| v.to_string())
    }
    pub fn set_accepting(&self, accepting: bool) {
        self.relay.accepting.store(accepting, Ordering::SeqCst);
    }
    pub fn set_unknown_signing(&self, unknown: bool) {
        self.signer.unknown.store(unknown, Ordering::SeqCst);
    }
    pub fn set_rejected_signing(&self, mode: FixtureRejection) {
        self.signer.rejection.store(
            match mode {
                FixtureRejection::Malformed => 1,
                FixtureRejection::Oversized => 2,
                FixtureRejection::Foreign => 3,
            },
            Ordering::SeqCst,
        );
    }
    pub fn release_signer(&self) {
        self.signer.release();
    }
    pub async fn wait_until_signer_paused(&self) -> Result<(), FixtureError> {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !self.signer.paused.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .map_err(|_| FixtureError::Unavailable)
    }
    /// Opens the same real namespace only after every previous client closed.
    /// No start, query, signature or outbox continuation is performed here.
    pub async fn reopen(&self) -> Result<Session, FixtureError> {
        let _lifecycle = self.lifecycle.lock().await;
        let index = {
            let clients = self.clients.lock().map_err(|_| FixtureError::Unavailable)?;
            if clients.len() >= 16 || clients.iter().any(|v| v.state() != SessionState::Closed) {
                return Err(FixtureError::Busy);
            }
            clients.len()
        };
        let store = NativeWorkspace::open_existing(
            &self.root.join("outbox"),
            self.selected.clone(),
            crypto(self.provider.clone())?,
            self.clock.clone(),
        )
        .await
        .map_err(|_| FixtureError::Unavailable)?;
        let session = open_client(
            &self.root.join(format!("cache{index}")),
            store,
            &self.selected,
            &self.proof,
            self.signer.clone(),
            self.closed.clone(),
            &self.relay,
        )
        .await?;
        self.clients
            .lock()
            .map_err(|_| FixtureError::Unavailable)?
            .push(session.client.clone());
        Ok(session)
    }
    pub fn measurements(&self) -> Result<Measurements, FixtureError> {
        Ok(Measurements {
            signing_calls: self.signer.calls.load(Ordering::SeqCst).to_string(),
            legacy_signing_calls: self.signer.legacy_calls.load(Ordering::SeqCst).to_string(),
            relay_reads: self.relay.reads.load(Ordering::SeqCst).to_string(),
            closed_callbacks: self.closed.load(Ordering::SeqCst).to_string(),
            original_signed_events: self
                .relay
                .events
                .lock()
                .map_err(|_| FixtureError::Unavailable)?
                .clone(),
        })
    }
    pub async fn cleanup(&self) -> Result<(), FixtureError> {
        let _lifecycle = self.lifecycle.lock().await;
        if self
            .clients
            .lock()
            .map_err(|_| FixtureError::Unavailable)?
            .iter()
            .any(|v| v.state() != SessionState::Closed)
        {
            return Err(FixtureError::Busy);
        }
        self.relay.stop().await?;
        std::fs::remove_dir_all(&self.root).map_err(|_| FixtureError::Unavailable)
    }
}

async fn open_client(
    cache: &std::path::Path,
    store: NativeWorkspace,
    selected: &AttachmentInput,
    proof: &AttachmentProof,
    signer: Arc<signer::MeasuredSigner>,
    closed: Arc<AtomicU64>,
    relay: &relay::Relay,
) -> Result<Session, FixtureError> {
    std::fs::create_dir(cache).map_err(|_| FixtureError::Unavailable)?;
    let options = OpenOptions::new(
        Paths::from_directory(cache).map_err(|_| FixtureError::Unavailable)?,
        OpenMode::Create,
    )
    .with_source_generation(
        SourceGeneration::new([43; 32]).map_err(|_| FixtureError::Unavailable)?,
        1,
    )
    .map_err(|_| FixtureError::Unavailable)?;
    let config = NativeConfig::public_read(options)
        .with_workspace(store)
        .with_nostr(relay.config()?, relay.policy.clone())
        .map_err(|_| FixtureError::Unavailable)?;
    let client = Client::open(config)
        .await
        .map_err(|_| FixtureError::Unavailable)?;
    let scope = client
        .attach(
            selected.clone(),
            proof
                .with_secret(|v| AttachmentProof::from_protected_input(v))
                .map_err(|_| FixtureError::Unavailable)?,
        )
        .await
        .map_err(|_| FixtureError::Unavailable)?;
    client
        .attach_signer(SignerHandle::new(signer, Arc::new(Callbacks { closed })))
        .await
        .map_err(|_| FixtureError::Unavailable)?;
    Ok(Session {
        client: NativeClient::from_native(client),
        workspace: Workspace::from_session(scope),
    })
}
fn reference() -> Result<SecretRef, FixtureError> {
    Ok(SecretRef::new(
        SecretId::parse("workspace-key").map_err(|_| FixtureError::Unavailable)?,
        BackendKind::File,
        KeyVersion::new(1).map_err(|_| FixtureError::Unavailable)?,
    ))
}
fn key() -> Result<PublicKey, FixtureError> {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
        .map_err(|_| FixtureError::Unavailable)
}
fn material(byte: u8) -> Result<SecretMaterial, FixtureError> {
    SecretMaterial::from_slice(&[byte; 32]).map_err(|_| FixtureError::Unavailable)
}
fn crypto(provider: Arc<FileProvider>) -> Result<WorkspaceCrypto, FixtureError> {
    WorkspaceCrypto::new(
        provider,
        reference()?,
        material(0x41)?,
        AccessPolicy::standard(),
    )
    .map_err(|_| FixtureError::Unavailable)
}

/// This probe belongs only to this test library, never to product bindings.
#[uniffi::export]
pub fn fixture_panic_probe() -> String {
    panic!("synthetic native binding panic probe")
}

/// Exercises unexpected panic conversion on the same Result-shaped boundary
/// used by the production command methods. This is fixture-only code.
#[uniffi::export]
pub fn fixture_result_panic_probe() -> Result<String, FixtureError> {
    panic!("synthetic native binding Result panic probe")
}

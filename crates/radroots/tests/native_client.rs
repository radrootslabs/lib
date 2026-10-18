#![cfg(feature = "native")]

use futures::{future::poll_fn, task::AtomicWaker};
use radroots::{Client, NativeConfig, StartMode};
use radroots_execution::session::SessionState;
use radroots_nostr::{key::parse_secret_key, signing::LocalSigner};
use radroots_signing::{
    provider::{ProviderError, SignerCallbacks, SignerHandle},
    signer::BoxFuture,
};
use radroots_storage::{event::SourceGeneration, status::ShutdownState};
use radroots_storage_sqlite::{OpenMode, OpenOptions, Paths, SqliteStorage};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

fn options(root: &std::path::Path) -> OpenOptions {
    OpenOptions::new(Paths::from_directory(root).unwrap(), OpenMode::Create)
        .with_source_generation(SourceGeneration::new([17; 32]).unwrap(), 1)
        .unwrap()
}

#[tokio::test]
async fn native_open_start_and_close_own_the_actual_store_without_creating_authority() {
    let root = tempfile::tempdir().unwrap();
    let options = options(root.path());
    let client = Client::open(NativeConfig::public_read(options.clone()))
        .await
        .unwrap();
    let copy = client.clone();
    assert_eq!(client.state(), SessionState::Ready);
    let blocked = match Client::open(NativeConfig::public_read(options.clone())).await {
        Err(error) => error,
        Ok(_) => panic!("second writer accepted"),
    };
    assert_eq!(blocked.code(), radroots_execution::Code::StorageFailure);
    assert!(!format!("{blocked:?}").contains(root.path().to_str().unwrap()));
    assert_eq!(
        client.storage_status().await.unwrap().shutdown(),
        ShutdownState::Open
    );
    assert!(matches!(
        SqliteStorage::open(options.clone()).await,
        Err(radroots_storage_sqlite::Error::WriterAlreadyActive { .. })
    ));
    drop(client.start(StartMode::LocalOnly));
    assert_eq!(client.state(), SessionState::Ready);
    client.start(StartMode::LocalOnly).await.unwrap();
    assert_eq!(copy.state(), SessionState::Running);
    drop(client.close());
    assert_eq!(copy.state(), SessionState::Running);
    let receipt = copy.close().await.unwrap();
    assert_eq!(receipt.storage().shutdown(), ShutdownState::Closed);
    assert_eq!(client.state(), SessionState::Closed);
    assert_eq!(client.close().await.unwrap(), receipt);
    assert!(client.start(StartMode::LocalOnly).await.is_err());
    let reopened = SqliteStorage::open(options).await.unwrap();
    reopened.close().await.unwrap();
}

struct Callbacks {
    options: OpenOptions,
    quiesces: AtomicUsize,
    closes: AtomicUsize,
    ready: AtomicBool,
    wake: AtomicWaker,
}
impl SignerCallbacks for Callbacks {
    fn quiesce(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async move {
            self.quiesces.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
    fn close(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async move {
            self.closes.fetch_add(1, Ordering::SeqCst);
            // The real database writer remains owned while callbacks drain.
            assert!(matches!(
                SqliteStorage::open(self.options.clone()).await,
                Err(radroots_storage_sqlite::Error::WriterAlreadyActive { .. })
            ));
            poll_fn(|cx| {
                self.wake.register(cx.waker());
                if self.ready.load(Ordering::SeqCst) {
                    std::task::Poll::Ready(())
                } else {
                    std::task::Poll::Pending
                }
            })
            .await;
            Ok(())
        })
    }
}
fn callbacks(options: OpenOptions, ready: bool) -> Arc<Callbacks> {
    Arc::new(Callbacks {
        options,
        quiesces: AtomicUsize::new(0),
        closes: AtomicUsize::new(0),
        ready: AtomicBool::new(ready),
        wake: AtomicWaker::new(),
    })
}
fn local() -> Arc<LocalSigner> {
    // Published synthetic interoperability identity, never custody material.
    Arc::new(
        LocalSigner::new(
            parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn actual_native_signer_attaches_before_start_and_callbacks_close_before_the_store() {
    let root = tempfile::tempdir().unwrap();
    let options = options(root.path());
    let client = Client::open(NativeConfig::public_read(options.clone()))
        .await
        .unwrap();
    let provider = local();
    let weak = Arc::downgrade(&provider);
    let expected = provider.public_key();
    let callbacks = callbacks(options.clone(), true);
    let signer = client
        .attach_signer(SignerHandle::new(provider, callbacks.clone()))
        .await
        .unwrap();
    assert_eq!(signer.observed().unwrap().identity(), expected);
    assert_eq!(client.state(), SessionState::Ready);
    assert_eq!(callbacks.quiesces.load(Ordering::SeqCst), 0);
    client.close().await.unwrap();
    assert_eq!(callbacks.quiesces.load(Ordering::SeqCst), 1);
    assert_eq!(callbacks.closes.load(Ordering::SeqCst), 1);
    assert!(weak.upgrade().is_none());
    assert_eq!(signer.state(), SessionState::Closed);
    signer.quiesce().await.unwrap();
    assert_eq!(callbacks.quiesces.load(Ordering::SeqCst), 1);
    let reopened = SqliteStorage::open(options).await.unwrap();
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn cancelled_client_close_retains_actual_provider_and_writer_until_callback_completion() {
    use std::{
        future::Future,
        task::{Context, Poll},
    };
    let root = tempfile::tempdir().unwrap();
    let options = options(root.path());
    let client = Client::open(NativeConfig::public_read(options.clone()))
        .await
        .unwrap();
    let provider = local();
    let weak = Arc::downgrade(&provider);
    let callbacks = callbacks(options.clone(), false);
    client
        .attach_signer(SignerHandle::new(provider, callbacks.clone()))
        .await
        .unwrap();
    let mut close = Box::pin(client.close());
    assert!(matches!(
        close
            .as_mut()
            .poll(&mut Context::from_waker(futures::task::noop_waker_ref())),
        Poll::Pending
    ));
    drop(close);
    assert_eq!(client.state(), SessionState::Closing);
    assert!(weak.upgrade().is_some());
    assert!(matches!(
        SqliteStorage::open(options.clone()).await,
        Err(radroots_storage_sqlite::Error::WriterAlreadyActive { .. })
    ));
    callbacks.ready.store(true, Ordering::SeqCst);
    callbacks.wake.wake();
    client.close().await.unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(callbacks.quiesces.load(Ordering::SeqCst), 1);
    let reopened = SqliteStorage::open(options).await.unwrap();
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn ordinary_native_signer_scope_returns_real_full_identity_signature_facts() {
    use radroots::signing::{ArtifactRole, OutcomeKind, SigningIntent};
    use radroots_event_codec::authoring::AuthoredEventPlan;
    use radroots_signing::{provider::ProviderCall, request::CancellationSignal};
    fn checked_clock() -> Result<u64, radroots_signing::Error> {
        Ok(1_700_000_000_001)
    }
    let root = tempfile::tempdir().unwrap();
    let options = options(root.path());
    let client = Client::open(NativeConfig::public_read(options.clone()))
        .await
        .unwrap();
    let provider = Arc::new(
        LocalSigner::with_clock(
            parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
            checked_clock,
        )
        .unwrap(),
    );
    let author = provider.public_key();
    let weak = Arc::downgrade(&provider);
    let callbacks = callbacks(options, true);
    let scope = client
        .attach_signer(SignerHandle::new(provider, callbacks))
        .await
        .unwrap();
    let plan = AuthoredEventPlan::from_generic(
        radroots::event::GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            1_700_000_000,
            vec![],
            "synthetic native retained preimage",
            author.to_hex(),
        )
        .unwrap(),
    )
    .unwrap();
    let operation = radroots::OperationId::from_bytes([31; 32]);
    let intent = SigningIntent::new(
        operation,
        [32; 32],
        plan,
        ArtifactRole::PublicEvent,
        None,
        ProviderCall::new(
            author,
            std::num::NonZeroU64::new(1_700_000_010_000).unwrap(),
            CancellationSignal::new(),
        ),
    )
    .unwrap();
    let outcome = scope.sign_exact(intent).await;
    assert_eq!(outcome.binding().operation_id(), operation);
    let OutcomeKind::Obtained(artifact) = outcome.kind() else {
        panic!("native signature unavailable: {outcome:?}");
    };
    artifact.with_json(|raw| {
        let value = radroots_event::wire::Nip01EventWire::parse_json(raw).unwrap();
        assert_eq!(value.pubkey, author.to_hex());
        assert_eq!(value.content, "synthetic native retained preimage");
    });
    assert!(artifact.observed_at_unix_ms().is_some());
    client.close().await.unwrap();
    assert!(weak.upgrade().is_none());
    // Owning authenticated bytes outlive released key/callback roots. This
    // primitive fact alone does not claim a durable command or publication.
    artifact.with_json(|raw| assert!(!raw.is_empty()));
}

#[tokio::test]
async fn ordinary_scoped_private_crypto_uses_actual_custody_and_nip44() {
    use radroots_nostr::signing::OsNonceSource;
    use radroots_signing::provider::{
        ParticipantProvider, PrivateText, ProviderCall, ProviderError,
    };
    use radroots_signing::request::CancellationSignal;
    fn checked_clock() -> Result<u64, radroots_signing::Error> {
        Ok(1_700_000_000_001)
    }
    let root = tempfile::tempdir().unwrap();
    let options = options(root.path());
    let client = Client::open(NativeConfig::public_read(options.clone()))
        .await
        .unwrap();
    let owner = Arc::new(
        LocalSigner::with_clock(
            parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
            checked_clock,
        )
        .unwrap()
        .with_nonce_source(Arc::new(OsNonceSource)),
    );
    let peer = LocalSigner::with_clock(
        parse_secret_key("0000000000000000000000000000000000000000000000000000000000000002")
            .unwrap(),
        checked_clock,
    )
    .unwrap()
    .with_nonce_source(Arc::new(OsNonceSource));
    let call = |identity| {
        ProviderCall::new(
            identity,
            std::num::NonZeroU64::new(1_700_000_010_000).unwrap(),
            CancellationSignal::new(),
        )
    };
    let scope = client
        .attach_signer(SignerHandle::new(owner.clone(), callbacks(options, true)))
        .await
        .unwrap();
    let text = "scoped private inquiry: apples 🍎 and exact original text";
    let ciphertext = scope
        .encrypt_private(
            call(owner.public_key()),
            peer.public_key(),
            PrivateText::new(text).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        peer.decrypt_private(call(peer.public_key()), owner.public_key(), ciphertext)
            .await
            .unwrap()
            .expose(),
        text
    );
    let ciphertext = peer
        .encrypt_private(
            call(peer.public_key()),
            owner.public_key(),
            PrivateText::new("independent peer response").unwrap(),
        )
        .await
        .unwrap();
    let plaintext = scope
        .decrypt_private(call(owner.public_key()), peer.public_key(), ciphertext)
        .await
        .unwrap();
    assert_eq!(plaintext.expose(), "independent peer response");
    assert!(!format!("{plaintext:?}").contains("independent peer response"));
    let cancelled = CancellationSignal::new();
    cancelled.cancel();
    let denied = scope
        .encrypt_private(
            ProviderCall::new(
                owner.public_key(),
                std::num::NonZeroU64::new(1_700_000_010_000).unwrap(),
                cancelled,
            ),
            peer.public_key(),
            PrivateText::new(text).unwrap(),
        )
        .await
        .unwrap_err();
    assert_eq!(denied, ProviderError::Cancelled);
    client.close().await.unwrap();
    assert_eq!(
        scope
            .encrypt_private(
                call(owner.public_key()),
                peer.public_key(),
                PrivateText::new(text).unwrap()
            )
            .await
            .unwrap_err(),
        ProviderError::CapabilityUnavailable
    );
}

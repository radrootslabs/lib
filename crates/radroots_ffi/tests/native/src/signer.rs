use radroots_nostr::{
    key::parse_secret_key,
    signing::{LocalSigner, OsNonceSource},
};
use radroots_signing::{
    Signer,
    issuance::{SigningIntent, SigningOutcome},
    provider::{
        ParticipantProvider, PrivateCiphertext, PrivateText, ProviderCall, ProviderError,
        ProviderState,
    },
    signer::BoxFuture,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use tokio::sync::Notify;

pub(crate) struct MeasuredSigner {
    local: LocalSigner,
    pub calls: AtomicU64,
    pub legacy_calls: AtomicU64,
    pub unknown: AtomicBool,
    pub rejection: AtomicUsize,
    pub pause_next: AtomicBool,
    pub paused: AtomicBool,
    release: Notify,
}
impl MeasuredSigner {
    pub fn new(unknown: bool, pause: bool) -> Arc<Self> {
        Arc::new(Self {
            local: LocalSigner::new(
                parse_secret_key(
                    "0000000000000000000000000000000000000000000000000000000000000001",
                )
                .expect("synthetic scalar"),
            )
            .expect("synthetic signer")
            .with_nonce_source(Arc::new(OsNonceSource)),
            calls: AtomicU64::new(0),
            legacy_calls: AtomicU64::new(0),
            unknown: AtomicBool::new(unknown),
            rejection: AtomicUsize::new(0),
            pause_next: AtomicBool::new(pause),
            paused: AtomicBool::new(false),
            release: Notify::new(),
        })
    }
    pub fn release(&self) {
        self.release.notify_one();
    }
}
impl Signer for MeasuredSigner {
    fn status(
        &self,
    ) -> BoxFuture<'_, Result<radroots_signing::SignerStatus, radroots_signing::Error>> {
        self.local.status()
    }
    fn sign(
        &self,
        request: radroots_signing::SignRequest,
    ) -> BoxFuture<'_, Result<radroots_signing::SignReceipt, radroots_signing::Error>> {
        self.legacy_calls.fetch_add(1, Ordering::SeqCst);
        self.local.sign(request)
    }
}
impl ParticipantProvider for MeasuredSigner {
    fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>> {
        self.local.provider_state()
    }
    fn sign_exact(&self, intent: SigningIntent) -> BoxFuture<'_, SigningOutcome> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.rejection.load(Ordering::SeqCst) {
                1 => {
                    return SigningOutcome::capture(
                        &intent,
                        "synthetic private rejection canary".into(),
                        None,
                    );
                }
                2 => return SigningOutcome::capture(&intent, "X".repeat(65537), None),
                3 => {
                    let foreign = radroots_signing::issuance::IntentBinding::from_components(
                        radroots_domain::OperationId::from_bytes([0xf1; 32]),
                        [0xf2; 32],
                        [0xf3; 32],
                        radroots_signing::issuance::ArtifactRole::PublicEvent,
                        None,
                        std::num::NonZeroU64::new(1).unwrap(),
                    )
                    .unwrap();
                    return SigningOutcome::not_dispatched(foreign, ProviderError::Cancelled);
                }
                _ => {}
            }
            if self.unknown.load(Ordering::SeqCst) {
                return SigningOutcome::unknown(intent.binding(), ProviderError::ProviderFailure);
            }
            let outcome = self.local.sign_exact(intent).await;
            if self.pause_next.swap(false, Ordering::SeqCst) {
                self.paused.store(true, Ordering::SeqCst);
                self.release.notified().await;
                self.paused.store(false, Ordering::SeqCst);
            }
            outcome
        })
    }
    fn encrypt_private(
        &self,
        call: ProviderCall,
        recipient: radroots_domain::PublicKey,
        plaintext: PrivateText,
    ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>> {
        self.local.encrypt_private(call, recipient, plaintext)
    }
    fn decrypt_private(
        &self,
        call: ProviderCall,
        sender: radroots_domain::PublicKey,
        ciphertext: PrivateCiphertext,
    ) -> BoxFuture<'_, Result<PrivateText, ProviderError>> {
        self.local.decrypt_private(call, sender, ciphertext)
    }
}

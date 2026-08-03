//! Generic signer composition without protocol or relay ownership.

use std::sync::Arc;
#[cfg(feature = "local-signing")]
use std::sync::RwLock;

use radroots_signing::{SignReceipt, SignRequest, Signer, SignerStatus};

/// Host-visible signer composition mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Mode {
    /// A concrete local Nostr adapter owns opaque key material.
    Local,
    /// A host-provided signer drives NIP-46 protocol and relay execution.
    Nip46,
    /// Another host-provided implementation of the generic signer SPI.
    Host,
}

/// Cloneable SDK composition wrapper around the canonical signer SPI.
#[derive(Clone)]
pub struct Provider {
    mode: Mode,
    signer: Arc<dyn Signer>,
    #[cfg(feature = "local-signing")]
    slot: Option<Slot>,
}

impl Provider {
    /// Wraps any host-provided canonical signer implementation.
    #[must_use]
    pub fn host(signer: Arc<dyn Signer>) -> Self {
        Self {
            mode: Mode::Host,
            signer,
            #[cfg(feature = "local-signing")]
            slot: None,
        }
    }

    /// Wraps the concrete local Nostr adapter without exposing its key.
    #[cfg(feature = "local-signing")]
    #[must_use]
    pub fn local(signer: radroots_nostr::signing::LocalSigner) -> Self {
        Self {
            mode: Mode::Local,
            signer: Arc::new(signer),
            slot: None,
        }
    }

    /// Wraps a host-controlled local signer slot.
    ///
    /// The slot starts inert and can be populated or cleared without rebuilding
    /// the client. Secret persistence remains entirely host-owned.
    #[cfg(feature = "local-signing")]
    #[must_use]
    pub fn slot(slot: Slot) -> Self {
        Self {
            mode: Mode::Local,
            signer: Arc::new(slot.clone()),
            slot: Some(slot),
        }
    }

    /// Marks a host-provided canonical signer as a NIP-46 composition.
    ///
    /// `radroots_nostr_connect` owns protocol state and its transport SPI. The
    /// injected implementation owns that client plus explicit relay execution;
    /// this wrapper does not contact a relay, persist a session, or start work.
    #[cfg(feature = "nip46")]
    #[must_use]
    pub fn nip46(signer: Arc<dyn Signer>) -> Self {
        Self {
            mode: Mode::Nip46,
            signer,
            #[cfg(feature = "local-signing")]
            slot: None,
        }
    }

    /// Returns the explicit composition mode.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// Borrows the canonical signer SPI.
    #[must_use]
    pub fn as_signer(&self) -> &dyn Signer {
        self.signer.as_ref()
    }

    /// Reports canonical status without initiating a signing request.
    pub async fn status(&self) -> Result<SignerStatus, radroots_signing::Error> {
        self.signer.status().await
    }

    /// Delegates one already-authorized request to the canonical SPI.
    pub async fn sign(&self, request: SignRequest) -> Result<SignReceipt, radroots_signing::Error> {
        self.signer.sign(request).await
    }

    #[cfg(feature = "local-signing")]
    pub(crate) fn into_parts(self) -> (Arc<dyn Signer>, Option<Slot>) {
        (self.signer, self.slot)
    }

    #[cfg(not(feature = "local-signing"))]
    pub(crate) fn into_signer(self) -> Arc<dyn Signer> {
        self.signer
    }
}

/// Public identity controlled by an installed local signer.
#[cfg(feature = "local-signing")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalIdentity {
    public_key: radroots_identity::PublicKey,
    npub: String,
}

#[cfg(feature = "local-signing")]
impl LocalIdentity {
    fn from_public_key(
        public_key: radroots_identity::PublicKey,
    ) -> Result<Self, radroots_nostr::Error> {
        Ok(Self {
            public_key,
            npub: radroots_nostr::key::public_key_to_npub(public_key)?,
        })
    }

    /// Returns the canonical lowercase public-key hexadecimal form.
    #[must_use]
    pub fn public_key_hex(&self) -> String {
        self.public_key.to_hex()
    }

    /// Returns the canonical NIP-19 public identity.
    #[must_use]
    pub fn npub(&self) -> &str {
        self.npub.as_str()
    }

    pub(crate) const fn public_key(&self) -> radroots_identity::PublicKey {
        self.public_key
    }
}

/// Mutable, client-shareable local signer selected by the host.
///
/// The slot never persists key material and its debug output never observes
/// the installed signer. Installing and clearing are explicit host actions.
#[cfg(feature = "local-signing")]
#[derive(Clone, Default)]
pub struct Slot {
    state: Arc<RwLock<Option<SlotState>>>,
}

#[cfg(feature = "local-signing")]
struct SlotState {
    signer: Arc<dyn Signer>,
    identity: LocalIdentity,
}

#[cfg(feature = "local-signing")]
impl Slot {
    /// Creates an empty signer slot.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Validates and installs one host-supplied hexadecimal or `nsec` secret.
    pub fn install(&self, encoded: &str) -> Result<LocalIdentity, radroots_nostr::Error> {
        let secret = radroots_nostr::key::SecretKey::parse(encoded)?;
        self.install_secret(secret)
    }

    /// Generates, installs, and returns one secret for immediate host custody.
    ///
    /// The SDK retains only the opaque signer. The returned `nsec` is the sole
    /// persistence handoff and must be moved into host secure storage.
    pub fn generate(&self) -> Result<(String, LocalIdentity), radroots_nostr::Error> {
        let secret = radroots_nostr::key::SecretKey::generate();
        let encoded = radroots_nostr::key::secret_key_to_nsec(&secret);
        let identity = self.install_secret(secret)?;
        Ok((encoded, identity))
    }

    /// Removes the active signer from this process.
    pub fn clear(&self) {
        if let Ok(mut state) = self.state.write() {
            *state = None;
        }
    }

    /// Returns the currently installed public identity.
    #[must_use]
    pub fn identity(&self) -> Option<LocalIdentity> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.as_ref().map(|state| state.identity.clone()))
    }

    fn install_secret(
        &self,
        secret: radroots_nostr::key::SecretKey,
    ) -> Result<LocalIdentity, radroots_nostr::Error> {
        let public_key = secret.public_key()?;
        let identity = LocalIdentity::from_public_key(public_key)?;
        let signer: Arc<dyn Signer> = Arc::new(radroots_nostr::signing::LocalSigner::new(secret)?);
        if let Ok(mut state) = self.state.write() {
            *state = Some(SlotState {
                signer,
                identity: identity.clone(),
            });
        }
        Ok(identity)
    }

    fn signer(&self) -> Result<Arc<dyn Signer>, radroots_signing::Error> {
        self.state
            .read()
            .map_err(|source| {
                radroots_signing::Error::with_source(
                    radroots_signing::error::Kind::InternalError,
                    LockFailure(source.to_string()),
                )
            })?
            .as_ref()
            .map(|state| Arc::clone(&state.signer))
            .ok_or_else(|| {
                radroots_signing::Error::new(radroots_signing::error::Kind::SignerUnavailable)
            })
    }
}

#[cfg(feature = "local-signing")]
impl Signer for Slot {
    fn status(
        &self,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignerStatus, radroots_signing::Error>>
    {
        Box::pin(async move {
            match self.signer() {
                Ok(signer) => signer.status().await,
                Err(error) if error.kind() == radroots_signing::error::Kind::SignerUnavailable => {
                    Ok(SignerStatus::unavailable())
                }
                Err(error) => Err(error),
            }
        })
    }

    fn sign(
        &self,
        request: SignRequest,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignReceipt, radroots_signing::Error>> {
        Box::pin(async move { self.signer()?.sign(request).await })
    }
}

#[cfg(feature = "local-signing")]
impl std::fmt::Debug for Slot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Slot")
            .field("installed", &self.identity().is_some())
            .finish()
    }
}

#[cfg(feature = "local-signing")]
#[derive(Debug)]
struct LockFailure(String);

#[cfg(feature = "local-signing")]
impl std::fmt::Display for LockFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

#[cfg(feature = "local-signing")]
impl std::error::Error for LockFailure {}

impl std::fmt::Debug for Provider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Provider")
            .field("mode", &self.mode)
            .field("signer", &"<opaque>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use radroots_event::{EventDraft, contract::AuthorRole};
    use radroots_identity::PublicKey;
    use radroots_protocol::runtime::v1::OperationId;
    use radroots_signing::{
        Actor, Error, SignReceipt, SignRequest, SignerStatus,
        actor::ActorSource,
        error::Kind,
        request::{CancellationPolicy, SignPolicy},
        signer::BoxFuture,
    };
    #[cfg(any(feature = "local-signing", feature = "nip46"))]
    use radroots_signing::{capability::SignerKind, status::SignerAvailability};
    #[cfg(feature = "nip46")]
    use radroots_signing::{
        capability::{CancellationSupport, SignerCapability},
        status::{AuthChallenge, SignProgress},
    };

    use super::*;

    const PUBLIC_KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    struct ScriptedSigner {
        status: SignerStatus,
        result: Kind,
        polls: Arc<AtomicUsize>,
    }

    impl Signer for ScriptedSigner {
        fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
            let status = self.status.clone();
            Box::pin(async move { Ok(status) })
        }

        fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
            let result = self.result;
            let polls = Arc::clone(&self.polls);
            Box::pin(async move {
                polls.fetch_add(1, Ordering::Relaxed);
                Err(Error::new(result))
            })
        }
    }

    fn request() -> SignRequest {
        let actor = Actor::new(
            PublicKey::from_hex(PUBLIC_KEY).expect("public key"),
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .expect("actor");
        let draft = EventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            1_700_000_000,
            Vec::new(),
            "frozen-content",
            PUBLIC_KEY,
        )
        .expect("draft");
        SignRequest::new(
            OperationId::SyncPush,
            actor,
            draft,
            SignPolicy::new(1_700_000_100, CancellationPolicy::PreservePublishedRequest)
                .expect("policy"),
        )
        .expect("request")
    }

    #[cfg(feature = "nip46")]
    fn remote_status(progress: Option<SignProgress>) -> SignerStatus {
        SignerStatus::new(
            SignerAvailability::AwaitingAuthentication,
            vec![SignerCapability::new(
                SignerKind::Remote,
                CancellationSupport::BeforeAndAfterPublication,
                true,
                true,
            )],
            progress,
        )
    }

    #[cfg(feature = "local-signing")]
    #[tokio::test]
    async fn local_provider_uses_the_concrete_lower_adapter() {
        let local = radroots_nostr::signing::LocalSigner::generate().expect("local signer");
        let provider = Provider::local(local);
        let status = provider.status().await.expect("status");
        assert_eq!(provider.mode(), Mode::Local);
        assert_eq!(status.availability(), SignerAvailability::Ready);
        assert_eq!(status.capabilities()[0].kind(), SignerKind::Local);
    }

    #[cfg(feature = "local-signing")]
    #[tokio::test]
    async fn local_slot_hands_secret_to_host_and_supports_lock_restore() {
        let slot = Slot::new();
        assert!(slot.identity().is_none());
        assert_eq!(
            slot.status().await.expect("empty status").availability(),
            SignerAvailability::Unavailable
        );

        let (secret, generated) = slot.generate().expect("generated identity");
        assert!(secret.starts_with("nsec1"));
        assert_eq!(slot.identity().expect("installed"), generated);
        assert!(!format!("{slot:?}").contains(secret.as_str()));

        slot.clear();
        assert!(slot.identity().is_none());
        let restored = slot.install(secret.as_str()).expect("restored identity");
        assert_eq!(restored, generated);
    }

    #[cfg(feature = "nip46")]
    #[tokio::test]
    async fn nip46_provider_preserves_auth_challenge_and_capabilities() {
        let challenge = AuthChallenge::new(
            "https://signer.example/approve",
            1_700_000_000,
            Some(1_700_000_100),
        )
        .expect("challenge");
        let signer = ScriptedSigner {
            status: remote_status(Some(SignProgress::authentication(challenge))),
            result: Kind::SignerRejected,
            polls: Arc::new(AtomicUsize::new(0)),
        };
        let provider = Provider::nip46(Arc::new(signer));
        let status = provider.status().await.expect("status");
        assert_eq!(provider.mode(), Mode::Nip46);
        assert_eq!(
            status.availability(),
            SignerAvailability::AwaitingAuthentication
        );
        assert!(status.progress().expect("progress").challenge().is_some());
        assert!(status.capabilities()[0].may_require_authentication());
    }

    #[tokio::test]
    async fn canonical_errors_preserve_timeout_drift_and_cancellation() {
        for expected in [
            Kind::SignerTimeout,
            Kind::SignerOutputInvalid,
            Kind::SignerCancelled,
        ] {
            let provider = Provider::host(Arc::new(ScriptedSigner {
                status: SignerStatus::unavailable(),
                result: expected,
                polls: Arc::new(AtomicUsize::new(0)),
            }));
            assert_eq!(
                provider.sign(request()).await.expect_err("failure").kind(),
                expected
            );
        }
    }

    #[test]
    fn dropping_unpolled_signing_future_has_no_effect() {
        let polls = Arc::new(AtomicUsize::new(0));
        let provider = Provider::host(Arc::new(ScriptedSigner {
            status: SignerStatus::unavailable(),
            result: Kind::SignerCancelled,
            polls: Arc::clone(&polls),
        }));
        drop(provider.sign(request()));
        assert_eq!(polls.load(Ordering::Relaxed), 0);
    }
}

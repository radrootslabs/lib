//! Generic signer composition without protocol or relay ownership.

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "blossom")]
use radroots_signing::SigningPurpose;
use radroots_signing::{
    Actor, SignReceipt, SignRequest, Signer, SignerStatus, SigningIntentId, request::SignPolicy,
};

pub use radroots_event_codec::authoring::BlossomAuthorizationPlan;
#[cfg(feature = "blossom")]
pub use radroots_nostr::blossom::AuthorizationHeader;

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
}

/// Borrowed high-level signing operations over one configured opaque signer.
#[derive(Clone, Copy)]
pub struct Operations<'a> {
    signer: &'a dyn Signer,
}

impl<'a> Operations<'a> {
    pub(crate) const fn new(signer: &'a dyn Signer) -> Self {
        Self { signer }
    }

    /// Delegates an already authorized exact request to the opaque signer.
    pub async fn sign(&self, request: SignRequest) -> Result<SignReceipt, radroots_signing::Error> {
        sign_checked(self.signer, request).await
    }

    /// Signs one HTTP-only BUD-11 upload plan and returns its canonical header.
    #[cfg(feature = "blossom")]
    pub async fn authorize_blossom_upload(
        &self,
        request: SignRequest,
    ) -> Result<radroots_nostr::blossom::AuthorizationHeader, BlossomSigningError> {
        if request.purpose() != SigningPurpose::BlossomUploadAuthorization {
            return Err(BlossomSigningError::WrongPurpose);
        }
        let receipt = self
            .sign(request)
            .await
            .map_err(BlossomSigningError::Signing)?;
        radroots_nostr::blossom::encode_signed_event_authorization_header(receipt.signed_event())
            .map_err(BlossomSigningError::Encoding)
    }
}

impl std::fmt::Debug for Operations<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Operations")
            .field("signer", &"<borrowed opaque signer>")
            .finish()
    }
}

/// Creates one domain-separated BUD-11 request from an exact HTTP-only plan.
pub fn blossom_upload_request(
    operation_kind: radroots_protocol::runtime::v1::OperationId,
    intent_id: SigningIntentId,
    actor: Actor,
    plan: BlossomAuthorizationPlan,
    policy: SignPolicy,
) -> Result<SignRequest, radroots_signing::Error> {
    SignRequest::blossom_upload(operation_kind, intent_id, actor, plan, policy)
}

/// Failure while producing a BUD-11 HTTP authorization header.
#[cfg(feature = "blossom")]
#[derive(Debug)]
#[non_exhaustive]
pub enum BlossomSigningError {
    /// The caller supplied a relay-authoring request to the HTTP-only method.
    WrongPurpose,
    /// The opaque signer rejected or failed the exact request.
    Signing(radroots_signing::Error),
    /// The verified signer result could not be encoded as BUD-11.
    Encoding(radroots_nostr::blossom::AuthorizationError),
}

#[cfg(feature = "blossom")]
impl std::fmt::Display for BlossomSigningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongPurpose => {
                formatter.write_str("signing request is not BUD-11 HTTP authorization")
            }
            Self::Signing(error) => error.fmt(formatter),
            Self::Encoding(error) => error.fmt(formatter),
        }
    }
}

#[cfg(feature = "blossom")]
impl std::error::Error for BlossomSigningError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WrongPurpose => None,
            Self::Signing(error) => Some(error),
            Self::Encoding(error) => Some(error),
        }
    }
}

impl Provider {
    /// Wraps any host-provided canonical signer implementation.
    #[must_use]
    pub fn host(signer: Arc<dyn Signer>) -> Self {
        Self {
            mode: Mode::Host,
            signer,
        }
    }

    /// Wraps the concrete local Nostr adapter without exposing its key.
    #[cfg(feature = "local-signing")]
    #[must_use]
    pub fn local(signer: radroots_nostr::signing::LocalSigner) -> Self {
        Self {
            mode: Mode::Local,
            signer: Arc::new(signer),
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
        sign_checked(self.signer.as_ref(), request).await
    }

    pub(crate) fn into_signer(self) -> Arc<dyn Signer> {
        self.signer
    }
}

async fn sign_checked(
    signer: &dyn Signer,
    request: SignRequest,
) -> Result<SignReceipt, radroots_signing::Error> {
    let expected = request.clone();
    let returned = signer.sign(request).await?;
    SignReceipt::from_signed_event(
        &expected,
        returned.signed_event().clone(),
        system_time_unix_ms()?,
    )
}

fn system_time_unix_ms() -> Result<u64, radroots_signing::Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| radroots_signing::Error::new(radroots_signing::error::Kind::InternalError))
        .and_then(|duration| {
            u64::try_from(duration.as_millis()).map_err(|_| {
                radroots_signing::Error::new(radroots_signing::error::Kind::InternalError)
            })
        })
}

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

    use radroots_event::{GenericEventDraft, contract::AuthorRole};
    use radroots_event_codec::authoring::AuthoredEventPlan;
    use radroots_identity::PublicKey;
    use radroots_protocol::runtime::v1::OperationId;
    use radroots_signing::{
        Actor, AuthoredArtifactId, Error, SignReceipt, SignRequest, SignerStatus, SigningIntentId,
        SigningOperationId,
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
        recovery::ReplayCapability,
        status::{AuthChallenge, SignProgress},
    };

    use super::*;

    const PUBLIC_KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[cfg(feature = "local-signing")]
    const SECRET_KEY: &str = "7e0112ad58b2d2d13fb80532625195dc169b86d72b0e1db48347837a785cae90";

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

    fn request_for(public_key: PublicKey, artifact: u8, deadline_unix_ms: u64) -> SignRequest {
        let actor = Actor::new(
            public_key,
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .expect("actor");
        let plan = AuthoredEventPlan::from_generic(
            GenericEventDraft::new(
                "radroots.social.geochat.v1",
                20_000,
                1_700_000_000,
                Vec::new(),
                "frozen-content",
                public_key.to_hex(),
            )
            .expect("draft"),
        )
        .expect("authored plan");
        SignRequest::new(
            OperationId::SyncPush,
            SigningIntentId::new(
                SigningOperationId::new([1; 16]).expect("operation id"),
                AuthoredArtifactId::new([artifact; 16]).expect("artifact id"),
            ),
            actor,
            plan,
            SignPolicy::new(
                deadline_unix_ms,
                CancellationPolicy::PreservePublishedRequest,
            )
            .expect("policy"),
        )
        .expect("request")
    }

    fn request() -> SignRequest {
        request_for(
            PublicKey::from_hex(PUBLIC_KEY).expect("public key"),
            2,
            1_700_000_100,
        )
    }

    #[cfg(feature = "local-signing")]
    fn local_signer() -> radroots_nostr::signing::LocalSigner {
        radroots_nostr::signing::LocalSigner::new(
            radroots_nostr::key::SecretKey::parse(SECRET_KEY).expect("secret fixture"),
        )
        .expect("local signer")
    }

    #[cfg(all(feature = "local-signing", feature = "blossom"))]
    fn blossom_request_for(public_key: PublicKey) -> SignRequest {
        use radroots_blossom::{
            Sha256,
            authorization::{AuthoredUploadClaim, AuthorizationContent, ServerDomain},
        };

        let claim = AuthoredUploadClaim::new(
            AuthorizationContent::parse("Upload exact SDK image").expect("content"),
            ServerDomain::parse("media.example").expect("server"),
            Sha256::digest(b"exact-sdk-image"),
            1_700_000_000,
            60,
        )
        .expect("claim");
        let actor = Actor::new(
            public_key,
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .expect("actor");
        blossom_upload_request(
            OperationId::SyncPush,
            SigningIntentId::new(
                SigningOperationId::new([9; 16]).expect("operation id"),
                AuthoredArtifactId::new([10; 16]).expect("artifact id"),
            ),
            actor,
            BlossomAuthorizationPlan::for_upload(&claim, public_key).expect("plan"),
            SignPolicy::new(u64::MAX, CancellationPolicy::LocalCooperative).expect("policy"),
        )
        .expect("Blossom request")
    }

    #[cfg(feature = "nip46")]
    fn remote_status(progress: Option<SignProgress>) -> SignerStatus {
        SignerStatus::new(
            SignerAvailability::AwaitingAuthentication,
            vec![SignerCapability::new(
                SignerKind::Remote,
                ReplayCapability::ExactReplayByRequestId,
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

    #[cfg(all(feature = "local-signing", feature = "blossom"))]
    #[tokio::test]
    async fn focused_operations_sign_exact_events_and_bud11_without_secret_surface() {
        use radroots_blossom::{
            Sha256,
            authorization::{AuthorizationTarget, AuthorizationValidation, ServerDomain},
        };

        let signer = local_signer();
        let public_key = signer.public_key();
        let provider = Provider::local(signer);
        let operations = Operations::new(provider.as_signer());
        assert!(format!("{provider:?}").contains("signer: \"<opaque>\""));
        assert!(format!("{operations:?}").contains("borrowed opaque signer"));

        let receipt = operations
            .sign(request_for(public_key, 3, u64::MAX))
            .await
            .expect("focused sign");
        assert_eq!(receipt.signed_event().pubkey(), &public_key);
        let provider_receipt = provider
            .sign(request_for(public_key, 4, u64::MAX))
            .await
            .expect("provider sign");
        assert_eq!(provider_receipt.signed_event().pubkey(), &public_key);

        let wrong_purpose = operations
            .authorize_blossom_upload(request_for(public_key, 5, u64::MAX))
            .await
            .expect_err("relay event cannot become an HTTP credential");
        assert!(matches!(wrong_purpose, BlossomSigningError::WrongPurpose));

        let header = operations
            .authorize_blossom_upload(blossom_request_for(public_key))
            .await
            .expect("BUD-11 authorization");
        let hash = Sha256::digest(b"exact-sdk-image");
        let verified = radroots_nostr::blossom::decode_verify_authorization_header(
            header.as_str(),
            &AuthorizationValidation::bud11(
                AuthorizationTarget::Upload(hash),
                ServerDomain::parse("media.example").expect("server"),
                1_700_000_001,
            ),
        )
        .expect("verify BUD-11 header");
        assert_eq!(verified.author().to_hex(), public_key.to_hex());
    }

    #[cfg(feature = "blossom")]
    #[test]
    fn blossom_errors_are_typed_and_preserve_only_explicit_sources() {
        use std::error::Error as _;

        let wrong = BlossomSigningError::WrongPurpose;
        assert!(wrong.source().is_none());
        assert!(!wrong.to_string().is_empty());

        let signing = BlossomSigningError::Signing(Error::new(Kind::SignerRejected));
        assert!(signing.source().is_some());
        assert_eq!(signing.to_string(), "signer rejected the request");

        let encoding = BlossomSigningError::Encoding(
            radroots_nostr::blossom::AuthorizationError::InvalidEventSignature,
        );
        assert!(encoding.source().is_some());
        assert_eq!(
            encoding.to_string(),
            "invalid Blossom authorization event signature"
        );
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
            assert_eq!(
                provider.as_signer().status().await.expect("status"),
                SignerStatus::unavailable()
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

//! Concrete local Nostr implementation of the generic signing SPI.
//!
//! Signing is local and in-memory. Success returns a cryptographically verified
//! exact-plan receipt without persistence or publication.

use core::fmt;
use std::{
    time::{SystemTime, UNIX_EPOCH},
    vec,
};

use radroots_identity::PublicKey;
use radroots_signing::{
    Error as SigningError, SignReceipt, SignRequest, Signer, SignerStatus,
    capability::{CancellationSupport, SignerCapability, SignerKind},
    error::Kind,
    recovery::ReplayCapability,
    signer::BoxFuture,
    status::{SignProgress, SignProgressStage, SignerAvailability},
};

use crate::{Error as NostrError, key::SecretKey};

pub use crate::draft_signing::sign_frozen_draft;

type Clock = fn() -> Result<u64, SigningError>;

/// A local Nostr key-backed signer adapter.
pub struct LocalSigner {
    keys: nostr::Keys,
    public_key: PublicKey,
    clock: Clock,
}

impl LocalSigner {
    pub fn new(secret_key: SecretKey) -> Result<Self, crate::Error> {
        Self::with_clock(secret_key, system_time_unix_ms)
    }

    pub fn generate() -> Result<Self, crate::Error> {
        Self::new(SecretKey::generate())
    }

    #[must_use]
    pub const fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn with_clock(secret_key: SecretKey, clock: Clock) -> Result<Self, crate::Error> {
        let public_key = secret_key.public_key()?;
        Ok(Self {
            keys: secret_key.into_keys(),
            public_key,
            clock,
        })
    }
}

impl fmt::Debug for LocalSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalSigner")
            .field("public_key", &self.public_key)
            .field("secret_key", &"[redacted]")
            .finish()
    }
}

impl Signer for LocalSigner {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, SigningError>> {
        Box::pin(async {
            Ok(SignerStatus::new(
                SignerAvailability::Ready,
                vec![SignerCapability::new(
                    SignerKind::Local,
                    ReplayCapability::LocalReplaySafe,
                    CancellationSupport::BeforePublication,
                    true,
                    false,
                )],
                None,
            ))
        })
    }

    fn sign(&self, request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, SigningError>> {
        Box::pin(async move {
            request.ensure_active((self.clock)()?)?;
            request.report_progress(
                &SignProgress::stage(SignProgressStage::Validating)
                    .expect("validating has no challenge"),
            );
            if request.plan().author() != &self.public_key {
                return Err(SigningError::new(Kind::AuthorizationDenied));
            }
            let signed_event = crate::plan_signing::sign_authored_plan(&self.keys, request.plan())
                .map_err(normalize_nostr_error)?;
            request.report_progress(
                &SignProgress::stage(SignProgressStage::VerifyingOutput)
                    .expect("verification has no challenge"),
            );
            let completed_at_unix_ms = (self.clock)()?;
            request.ensure_active(completed_at_unix_ms)?;
            let receipt =
                SignReceipt::from_signed_event(&request, signed_event, completed_at_unix_ms)?;
            request.report_progress(
                &SignProgress::stage(SignProgressStage::Complete)
                    .expect("completion has no challenge"),
            );
            Ok(receipt)
        })
    }
}

fn system_time_unix_ms() -> Result<u64, SigningError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| SigningError::with_source(Kind::InternalError, source))
        .and_then(|duration| {
            u64::try_from(duration.as_millis())
                .map_err(|source| SigningError::with_source(Kind::InternalError, source))
        })
}

fn normalize_nostr_error(source: NostrError) -> SigningError {
    let kind = match &source {
        NostrError::FrozenDraftPubkeyMismatch { .. }
        | NostrError::ExternalSigningAuthorMismatch { .. } => Kind::AuthorizationDenied,
        NostrError::FrozenDraftEventIdMismatch { .. }
        | NostrError::ExternalSigningEventIdMismatch { .. }
        | NostrError::ExternalSigningEventInvalid(_)
        | NostrError::ExternalSigningPlanMismatch { .. } => Kind::SignerOutputInvalid,
        _ => Kind::InternalError,
    };
    SigningError::with_source(kind, source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::{GenericEventDraft, contract::AuthorRole};
    use radroots_event_codec::authoring::AuthoredEventPlan;
    use radroots_protocol::runtime::v1::OperationId;
    use radroots_signing::{
        Actor, AuthoredArtifactId, SigningIntentId, SigningOperationId,
        actor::ActorSource,
        request::{CancellationPolicy, CancellationSignal, SignPolicy},
    };

    use crate::{
        key::parse_secret_key,
        test_fixtures::{FIXTURE_ALICE, FIXTURE_BOB},
    };

    const CREATED_AT: u64 = 1_700_000_000;
    const DEADLINE_MS: u64 = 1_700_000_100_000;

    fn before_deadline() -> Result<u64, SigningError> {
        Ok(DEADLINE_MS - 1)
    }

    fn at_deadline() -> Result<u64, SigningError> {
        Ok(DEADLINE_MS)
    }

    fn fixture_secret(value: &str) -> SecretKey {
        parse_secret_key(value).expect("secret fixture")
    }

    fn request() -> SignRequest {
        let actor = Actor::from_public_key_hex(
            FIXTURE_ALICE.public_key_hex,
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .unwrap();
        let draft = GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            CREATED_AT,
            Vec::new(),
            "private-fixture-content",
            FIXTURE_ALICE.public_key_hex,
        )
        .unwrap();
        SignRequest::new(
            OperationId::SyncPush,
            SigningIntentId::new(
                SigningOperationId::new([1; 16]).unwrap(),
                AuthoredArtifactId::new([2; 16]).unwrap(),
            ),
            actor,
            AuthoredEventPlan::from_generic(draft).unwrap(),
            SignPolicy::new(DEADLINE_MS, CancellationPolicy::LocalCooperative).unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn local_signer_reports_safe_replay_and_returns_verified_exact_plan() {
        let signer = LocalSigner::with_clock(
            fixture_secret(FIXTURE_ALICE.secret_key_hex),
            before_deadline,
        )
        .unwrap();
        let status = signer.status().await.unwrap();
        assert_eq!(
            status.capabilities()[0].replay(),
            ReplayCapability::LocalReplaySafe
        );
        let request = request();
        let expected_id = request.plan().expected_event_id().to_hex();
        let receipt = signer.sign(request).await.unwrap();
        assert_eq!(receipt.signed_event().id_str(), expected_id);
        assert_eq!(receipt.completed_at_unix_ms(), DEADLINE_MS - 1);
    }

    #[tokio::test]
    async fn wrong_key_deadline_and_cancellation_fail_closed() {
        let wrong =
            LocalSigner::with_clock(fixture_secret(FIXTURE_BOB.secret_key_hex), before_deadline)
                .unwrap();
        assert_eq!(
            wrong.sign(request()).await.unwrap_err().kind(),
            Kind::AuthorizationDenied
        );
        let expired =
            LocalSigner::with_clock(fixture_secret(FIXTURE_ALICE.secret_key_hex), at_deadline)
                .unwrap();
        assert_eq!(
            expired.sign(request()).await.unwrap_err().kind(),
            Kind::DeadlineExceeded
        );
        let signal = CancellationSignal::new();
        let cancelled = request().with_cancellation_signal(signal.clone());
        signal.cancel();
        assert_eq!(
            LocalSigner::with_clock(
                fixture_secret(FIXTURE_ALICE.secret_key_hex),
                before_deadline,
            )
            .unwrap()
            .sign(cancelled)
            .await
            .unwrap_err()
            .kind(),
            Kind::SignerCancelled
        );
    }

    #[test]
    fn debug_never_exposes_local_secret_material() {
        let signer = LocalSigner::with_clock(
            fixture_secret(FIXTURE_ALICE.secret_key_hex),
            before_deadline,
        )
        .unwrap();
        let debug = format!("{signer:?}");
        assert!(!debug.contains(FIXTURE_ALICE.secret_key_hex));
        assert!(!debug.contains(FIXTURE_ALICE.nsec));
    }
}

//! Concrete local Nostr implementation of the generic signing SPI.
//!
//! Signing is local and in-memory. Success returns a verified receipt without
//! persisting or publishing it; dropping the future creates no durable effect.

use core::fmt;
use std::{
    time::{SystemTime, UNIX_EPOCH},
    vec,
};

use radroots_signing::{
    Error as SigningError, SignReceipt, SignRequest, Signer, SignerStatus,
    capability::{CancellationSupport, SignerCapability, SignerKind},
    error::Kind,
    signer::BoxFuture,
    status::{SignProgress, SignProgressStage, SignerAvailability},
};

use crate::{
    draft_signing::radroots_nostr_sign_frozen_draft, error::RadrootsNostrError, key::SecretKey,
};
use radroots_identity::PublicKey;

type Clock = fn() -> Result<u64, SigningError>;

/// A local Nostr key-backed signer adapter.
///
/// Key material remains private to this adapter. Debug output reports only the
/// public key and never delegates to the upstream key container.
pub struct LocalSigner {
    keys: nostr::Keys,
    public_key: PublicKey,
    clock: Clock,
}

impl LocalSigner {
    /// Consumes one opaque local secret and creates its signer adapter.
    pub fn new(secret_key: SecretKey) -> Result<Self, crate::Error> {
        Self::with_clock(secret_key, system_time_unix)
    }

    /// Generates a fresh opaque secret and creates its signer adapter.
    pub fn generate() -> Result<Self, crate::Error> {
        Self::new(SecretKey::generate())
    }

    /// Returns the canonical public identity controlled by this signer.
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
            let started_at_unix = (self.clock)()?;
            if started_at_unix >= request.policy().deadline_unix() {
                return Err(SigningError::new(Kind::DeadlineExceeded));
            }
            request.report_progress(
                &SignProgress::stage(SignProgressStage::Validating)
                    .expect("validating progress never requires a challenge"),
            );
            let signed_event = radroots_nostr_sign_frozen_draft(&self.keys, request.draft())
                .map_err(normalize_nostr_error)?;
            request.report_progress(
                &SignProgress::stage(SignProgressStage::VerifyingOutput)
                    .expect("verification progress never requires a challenge"),
            );
            let completed_at_unix = (self.clock)()?;
            if completed_at_unix >= request.policy().deadline_unix() {
                return Err(SigningError::new(Kind::DeadlineExceeded));
            }
            let receipt =
                SignReceipt::from_signed_event(&request, signed_event, completed_at_unix)?;
            request.report_progress(
                &SignProgress::stage(SignProgressStage::Complete)
                    .expect("completion progress never requires a challenge"),
            );
            Ok(receipt)
        })
    }
}

fn system_time_unix() -> Result<u64, SigningError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|source| SigningError::with_source(Kind::InternalError, source))
}

fn normalize_nostr_error(source: RadrootsNostrError) -> SigningError {
    let kind = match &source {
        RadrootsNostrError::FrozenDraftPubkeyMismatch { .. } => Kind::AuthorizationDenied,
        RadrootsNostrError::FrozenDraftEventIdMismatch { .. } => Kind::SignerOutputInvalid,
        _ => Kind::InternalError,
    };
    SigningError::with_source(kind, source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::{EventDraft, contract::AuthorRole, envelope::kind::KIND_GEOCHAT};
    use radroots_protocol::runtime::v1::OperationId;
    use radroots_signing::{
        Actor,
        actor::ActorSource,
        request::{CancellationPolicy, ProgressObserver, SignPolicy},
    };
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use crate::{
        key::parse_secret_key,
        test_fixtures::{FIXTURE_ALICE, FIXTURE_BOB},
    };

    const DEADLINE: u64 = 1_700_000_100;
    static CROSSING_DEADLINE_CALLS: AtomicUsize = AtomicUsize::new(0);

    struct RecordingObserver(Mutex<Vec<SignProgressStage>>);

    impl ProgressObserver for RecordingObserver {
        fn on_progress(&self, progress: &SignProgress) {
            self.0
                .lock()
                .expect("progress lock")
                .push(progress.stage_value());
        }
    }

    fn before_deadline() -> Result<u64, SigningError> {
        Ok(1_700_000_050)
    }

    fn at_deadline() -> Result<u64, SigningError> {
        Ok(DEADLINE)
    }

    fn crossing_deadline() -> Result<u64, SigningError> {
        if CROSSING_DEADLINE_CALLS.fetch_add(1, Ordering::SeqCst) == 0 {
            before_deadline()
        } else {
            at_deadline()
        }
    }

    fn fixture_secret(secret_key_hex: &str) -> SecretKey {
        parse_secret_key(secret_key_hex).expect("secret key fixture")
    }

    fn request() -> SignRequest {
        let actor = Actor::from_public_key_hex(
            FIXTURE_ALICE.public_key_hex,
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .expect("actor");
        let draft = EventDraft::new(
            "radroots.social.geochat.v1",
            KIND_GEOCHAT,
            1_700_000_000,
            Vec::new(),
            "private-fixture-content",
            FIXTURE_ALICE.public_key_hex,
        )
        .expect("draft");
        SignRequest::new(
            OperationId::SyncPush,
            actor,
            draft,
            SignPolicy::new(DEADLINE, CancellationPolicy::LocalCooperative).expect("policy"),
        )
        .expect("request")
    }

    #[tokio::test]
    async fn local_adapter_reports_capability_and_signs_the_exact_draft() {
        let signer = LocalSigner::with_clock(
            fixture_secret(FIXTURE_ALICE.secret_key_hex),
            before_deadline,
        )
        .expect("local signer");
        let status = signer.status().await.expect("status");
        assert_eq!(status.availability(), SignerAvailability::Ready);
        assert_eq!(status.capabilities().len(), 1);
        let capability = status.capabilities()[0];
        assert_eq!(capability.kind(), SignerKind::Local);
        assert_eq!(
            capability.cancellation(),
            CancellationSupport::BeforePublication
        );
        assert!(capability.reports_progress());
        assert!(!capability.may_require_authentication());
        assert_eq!(signer.public_key().to_hex(), FIXTURE_ALICE.public_key_hex);

        let observer = Arc::new(RecordingObserver(Mutex::new(Vec::new())));
        let request = request().with_progress_observer(observer.clone());
        let expected_id = request.draft().expected_event_id_hex();
        let receipt = signer.sign(request).await.expect("receipt");

        assert_eq!(receipt.operation_id(), OperationId::SyncPush);
        assert_eq!(receipt.completed_at_unix(), 1_700_000_050);
        assert_eq!(receipt.signed_event().id_str(), expected_id);
        assert_eq!(
            receipt.signed_event().pubkey().to_hex(),
            FIXTURE_ALICE.public_key_hex
        );
        assert_eq!(
            observer.0.lock().expect("progress lock").as_slice(),
            &[
                SignProgressStage::Validating,
                SignProgressStage::VerifyingOutput,
                SignProgressStage::Complete,
            ]
        );
    }

    #[tokio::test]
    async fn wrong_local_key_is_normalized_without_leaking_secret_material() {
        let signer =
            LocalSigner::with_clock(fixture_secret(FIXTURE_BOB.secret_key_hex), before_deadline)
                .expect("local signer");
        let error = signer
            .sign(request())
            .await
            .expect_err("wrong key must fail");

        assert_eq!(error.kind(), Kind::AuthorizationDenied);
        assert!(!error.to_string().contains(FIXTURE_BOB.secret_key_hex));
        assert!(!format!("{error:?}").contains(FIXTURE_BOB.secret_key_hex));
        assert!(!format!("{signer:?}").contains(FIXTURE_BOB.secret_key_hex));
        assert!(!format!("{signer:?}").contains(FIXTURE_BOB.nsec));
    }

    #[test]
    fn generated_local_signer_exposes_only_its_public_identity() {
        let signer = LocalSigner::generate().expect("generated local signer");
        let rendered = format!("{signer:?}");

        assert_eq!(signer.public_key().to_hex().len(), 64);
        assert!(rendered.contains("[redacted]"));
        assert!(rendered.contains(&signer.public_key().to_hex()));
    }

    #[tokio::test]
    async fn expired_deadline_fails_before_local_signing() {
        let signer =
            LocalSigner::with_clock(fixture_secret(FIXTURE_ALICE.secret_key_hex), at_deadline)
                .expect("local signer");
        let observer = Arc::new(RecordingObserver(Mutex::new(Vec::new())));
        let error = signer
            .sign(request().with_progress_observer(observer.clone()))
            .await
            .expect_err("deadline must fail");

        assert_eq!(error.kind(), Kind::DeadlineExceeded);
        assert!(observer.0.lock().expect("progress lock").is_empty());
    }

    #[tokio::test]
    async fn deadline_crossing_discards_output_before_receipt_completion() {
        CROSSING_DEADLINE_CALLS.store(0, Ordering::SeqCst);
        let signer = LocalSigner::with_clock(
            fixture_secret(FIXTURE_ALICE.secret_key_hex),
            crossing_deadline,
        )
        .expect("local signer");
        let observer = Arc::new(RecordingObserver(Mutex::new(Vec::new())));
        let error = signer
            .sign(request().with_progress_observer(observer.clone()))
            .await
            .expect_err("crossed deadline must fail");

        assert_eq!(error.kind(), Kind::DeadlineExceeded);
        assert_eq!(
            observer.0.lock().expect("progress lock").as_slice(),
            &[
                SignProgressStage::Validating,
                SignProgressStage::VerifyingOutput,
            ]
        );
    }
}

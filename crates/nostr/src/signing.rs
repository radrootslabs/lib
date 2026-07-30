//! Concrete local Nostr implementation of the generic signing SPI.

use core::fmt;
use std::{
    time::{SystemTime, UNIX_EPOCH},
    vec,
};

use radroots_signing::{
    Error, SignReceipt, SignRequest, Signer, SignerStatus,
    capability::{CancellationSupport, SignerCapability, SignerKind},
    error::Kind,
    signer::BoxFuture,
    status::{SignProgress, SignProgressStage, SignerAvailability},
};

use crate::{
    draft_signing::radroots_nostr_sign_frozen_draft, error::RadrootsNostrError,
    types::RadrootsNostrKeys,
};

type Clock = fn() -> Result<u64, Error>;

/// A local Nostr key-backed signer adapter.
///
/// Key material remains private to this adapter. Debug output reports only the
/// public key and never delegates to the upstream key container.
pub struct LocalSigner {
    keys: RadrootsNostrKeys,
    clock: Clock,
}

impl LocalSigner {
    /// Creates a local signer over one Nostr keypair.
    #[must_use]
    pub const fn new(keys: RadrootsNostrKeys) -> Self {
        Self {
            keys,
            clock: system_time_unix,
        }
    }

    #[cfg(test)]
    const fn with_clock(keys: RadrootsNostrKeys, clock: Clock) -> Self {
        Self { keys, clock }
    }
}

impl fmt::Debug for LocalSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalSigner")
            .field("public_key", &self.keys.public_key().to_hex())
            .field("secret_key", &"[redacted]")
            .finish()
    }
}

impl Signer for LocalSigner {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
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

    fn sign(&self, request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
        Box::pin(async move {
            let started_at_unix = (self.clock)()?;
            if started_at_unix >= request.policy().deadline_unix() {
                return Err(Error::new(Kind::DeadlineExceeded));
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
                return Err(Error::new(Kind::DeadlineExceeded));
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

fn system_time_unix() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|source| Error::with_source(Kind::InternalError, source))
}

fn normalize_nostr_error(source: RadrootsNostrError) -> Error {
    let kind = match &source {
        RadrootsNostrError::FrozenDraftPubkeyMismatch { .. } => Kind::AuthorizationDenied,
        RadrootsNostrError::FrozenDraftEventIdMismatch { .. } => Kind::SignerOutputInvalid,
        _ => Kind::InternalError,
    };
    Error::with_source(kind, source)
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
    use std::sync::{Arc, Mutex};

    use crate::{
        test_fixtures::{FIXTURE_ALICE, FIXTURE_BOB},
        types::RadrootsNostrSecretKey,
    };

    const DEADLINE: u64 = 1_700_000_100;

    struct RecordingObserver(Mutex<Vec<SignProgressStage>>);

    impl ProgressObserver for RecordingObserver {
        fn on_progress(&self, progress: &SignProgress) {
            self.0
                .lock()
                .expect("progress lock")
                .push(progress.stage_value());
        }
    }

    fn before_deadline() -> Result<u64, Error> {
        Ok(1_700_000_050)
    }

    fn at_deadline() -> Result<u64, Error> {
        Ok(DEADLINE)
    }

    fn fixture_keys(secret_key_hex: &str) -> RadrootsNostrKeys {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key fixture");
        RadrootsNostrKeys::new(secret_key)
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
        let signer =
            LocalSigner::with_clock(fixture_keys(FIXTURE_ALICE.secret_key_hex), before_deadline);
        let status = signer.status().await.expect("status");
        assert_eq!(status.availability(), SignerAvailability::Ready);
        assert_eq!(status.capabilities().len(), 1);
        assert_eq!(status.capabilities()[0].kind(), SignerKind::Local);

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
            LocalSigner::with_clock(fixture_keys(FIXTURE_BOB.secret_key_hex), before_deadline);
        let error = signer
            .sign(request())
            .await
            .expect_err("wrong key must fail");

        assert_eq!(error.kind(), Kind::AuthorizationDenied);
        assert!(!error.to_string().contains(FIXTURE_BOB.secret_key_hex));
        assert!(!format!("{error:?}").contains(FIXTURE_BOB.secret_key_hex));
        assert!(!format!("{signer:?}").contains(FIXTURE_BOB.secret_key_hex));
    }

    #[tokio::test]
    async fn expired_deadline_fails_before_local_signing() {
        let signer =
            LocalSigner::with_clock(fixture_keys(FIXTURE_ALICE.secret_key_hex), at_deadline);
        let error = signer
            .sign(request())
            .await
            .expect_err("deadline must fail");

        assert_eq!(error.kind(), Kind::DeadlineExceeded);
    }
}

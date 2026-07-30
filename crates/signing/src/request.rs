//! Validated signing requests.

use core::fmt;
use radroots_event::{EventDraft, contract::event_contract};
use radroots_protocol::runtime::v1::OperationId;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::sync::Arc;

use crate::{Actor, Error, status::SignProgress};

/// How a signer must interpret cancellation around remote publication.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellationPolicy {
    /// Stop if cancellation is observed before publication; report the final
    /// remote state explicitly when observed after publication.
    PreservePublishedRequest,
    /// A local-only operation may stop whenever cancellation is observed.
    LocalCooperative,
}

/// Explicit deadline and cancellation policy for one signing operation.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignPolicy {
    deadline_unix: u64,
    cancellation: CancellationPolicy,
}

impl SignPolicy {
    /// Creates a bounded policy. Unix timestamp zero is never a valid deadline.
    pub const fn new(
        deadline_unix: u64,
        cancellation: CancellationPolicy,
    ) -> Result<Self, SignPolicyError> {
        if deadline_unix == 0 {
            return Err(SignPolicyError::InvalidDeadline);
        }
        Ok(Self {
            deadline_unix,
            cancellation,
        })
    }

    /// Returns the absolute Unix deadline.
    #[must_use]
    pub const fn deadline_unix(self) -> u64 {
        self.deadline_unix
    }

    /// Returns the explicit cancellation contract.
    #[must_use]
    pub const fn cancellation(self) -> CancellationPolicy {
        self.cancellation
    }
}

/// Invalid signing policy input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignPolicyError {
    /// The deadline was the Unix epoch sentinel rather than a real bound.
    InvalidDeadline,
}

impl fmt::Display for SignPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("signing deadline must be greater than zero")
    }
}

impl core::error::Error for SignPolicyError {}

/// Runtime-local observer for signing progress.
///
/// Observers are not serialized, persisted, or invoked by hidden workers.
/// Implementations call them synchronously from the active signing future.
pub trait ProgressObserver: Send + Sync {
    /// Observes one immutable progress value.
    fn on_progress(&self, progress: &SignProgress);
}

/// One authorized actor, frozen draft, and bounded signer invocation.
#[derive(Clone)]
pub struct SignRequest {
    operation_id: OperationId,
    actor: Actor,
    draft: EventDraft,
    policy: SignPolicy,
    progress_observer: Option<Arc<dyn ProgressObserver>>,
}

impl SignRequest {
    /// Validates the current draft, then the actor role, then the expected
    /// public key, and creates a request without a progress observer.
    pub fn new(
        operation_id: OperationId,
        actor: Actor,
        draft: EventDraft,
        policy: SignPolicy,
    ) -> Result<Self, Error> {
        authorize_actor_for_draft(&actor, &draft).map_err(|_| Error)?;
        Ok(Self {
            operation_id,
            actor,
            draft,
            policy,
            progress_observer: None,
        })
    }

    /// Installs a runtime-local progress observer.
    #[must_use]
    pub fn with_progress_observer(mut self, observer: Arc<dyn ProgressObserver>) -> Self {
        self.progress_observer = Some(observer);
        self
    }

    /// Returns the versioned runtime operation identity.
    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    /// Borrows the actor provenance and role claim.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// Borrows the exact canonical draft to sign.
    #[must_use]
    pub const fn draft(&self) -> &EventDraft {
        &self.draft
    }

    /// Returns the deadline and cancellation policy.
    #[must_use]
    pub const fn policy(&self) -> SignPolicy {
        self.policy
    }

    /// Reports progress to the request-local observer, when present.
    pub fn report_progress(&self, progress: &SignProgress) {
        if let Some(observer) = &self.progress_observer {
            observer.on_progress(progress);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthorizationFailure {
    InvalidDraft,
    ActorRoleUnsatisfied,
    ActorPublicKeyMismatch,
}

fn authorize_actor_for_draft(
    actor: &Actor,
    draft: &EventDraft,
) -> Result<(), AuthorizationFailure> {
    // This order is part of the authorization contract: no actor decision is
    // made for an invalid/stale draft, and role rejection precedes key drift.
    draft
        .validate_for_signing()
        .map_err(|_| AuthorizationFailure::InvalidDraft)?;
    let contract = event_contract(draft.contract_id()).ok_or(AuthorizationFailure::InvalidDraft)?;
    if !actor.satisfies(contract.required_author_role()) {
        return Err(AuthorizationFailure::ActorRoleUnsatisfied);
    }
    if actor.public_key() != *draft.expected_pubkey() {
        return Err(AuthorizationFailure::ActorPublicKeyMismatch);
    }
    Ok(())
}

impl fmt::Debug for SignRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignRequest")
            .field("operation_id", &self.operation_id)
            .field("actor", &self.actor)
            .field("draft", &"[redacted frozen event draft]")
            .field("policy", &self.policy)
            .field(
                "progress_observer",
                &self.progress_observer.as_ref().map(|_| "[installed]"),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actor::ActorSource,
        status::{SignProgress, SignProgressStage},
    };
    use core::sync::atomic::{AtomicUsize, Ordering};
    use radroots_event::contract::AuthorRole;
    use radroots_event::envelope::kind::KIND_TRADE_PROPOSAL;
    use radroots_identity::PublicKey;

    #[cfg(not(feature = "std"))]
    use alloc::{string::String, sync::Arc, vec, vec::Vec};
    #[cfg(feature = "std")]
    use std::{string::String, sync::Arc, vec, vec::Vec};

    const PUBLIC_KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER_PUBLIC_KEY: &str =
        "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    struct CountingObserver(AtomicUsize);

    impl ProgressObserver for CountingObserver {
        fn on_progress(&self, _progress: &SignProgress) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn request() -> SignRequest {
        let public_key = PublicKey::from_hex(PUBLIC_KEY).expect("public key");
        let actor = Actor::new(
            public_key,
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .expect("actor");
        let draft = EventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            1_700_000_000,
            Vec::new(),
            "private-draft-content",
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
        .expect("authorized request")
    }

    #[test]
    fn policy_requires_a_real_deadline() {
        assert_eq!(
            SignPolicy::new(0, CancellationPolicy::LocalCooperative),
            Err(SignPolicyError::InvalidDeadline)
        );
    }

    #[test]
    fn request_preserves_inputs_reports_progress_and_redacts_draft_debug() {
        let observer = Arc::new(CountingObserver(AtomicUsize::new(0)));
        let request = request().with_progress_observer(observer.clone());
        let progress = SignProgress::stage(SignProgressStage::Queued).expect("progress");

        request.report_progress(&progress);

        assert_eq!(request.operation_id(), OperationId::SyncPush);
        assert_eq!(request.policy().deadline_unix(), 1_700_000_100);
        assert_eq!(request.draft().content(), "private-draft-content");
        assert_eq!(observer.0.load(Ordering::Relaxed), 1);
        let debug = alloc_or_std_format(&request);
        assert!(!debug.contains("private-draft-content"));
        assert!(debug.contains("redacted frozen event draft"));
    }

    #[test]
    fn authorization_rejects_role_before_public_key_drift() {
        let draft = EventDraft::new(
            "radroots.trade.proposal.v1",
            KIND_TRADE_PROPOSAL,
            1_700_000_000,
            vec![
                vec![
                    "contract".to_owned(),
                    "radroots.trade.proposal.v1".to_owned(),
                ],
                vec![
                    "d".to_owned(),
                    "11111111111111111111111111111111".to_owned(),
                ],
                vec!["p".to_owned(), PUBLIC_KEY.to_owned()],
            ],
            r#"{"contract_id":"radroots.trade.proposal.v1"}"#,
            PUBLIC_KEY,
        )
        .expect("draft");
        let wrong_key = PublicKey::from_hex(OTHER_PUBLIC_KEY).expect("public key");
        let actor = Actor::new(
            wrong_key,
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Seller],
        )
        .expect("actor");

        assert_eq!(
            authorize_actor_for_draft(&actor, &draft),
            Err(AuthorizationFailure::ActorRoleUnsatisfied)
        );
    }

    #[test]
    fn authorization_rejects_actor_public_key_drift() {
        let draft = request().draft().clone();
        let wrong_key = PublicKey::from_hex(OTHER_PUBLIC_KEY).expect("public key");
        let actor = Actor::new(wrong_key, ActorSource::ExplicitPublicKey, [AuthorRole::Any])
            .expect("actor");

        assert_eq!(
            authorize_actor_for_draft(&actor, &draft),
            Err(AuthorizationFailure::ActorPublicKeyMismatch)
        );
    }

    fn alloc_or_std_format(value: &SignRequest) -> String {
        value_to_string(format_args!("{value:?}"))
    }

    fn value_to_string(arguments: fmt::Arguments<'_>) -> String {
        use core::fmt::Write as _;
        let mut output = String::new();
        output.write_fmt(arguments).expect("string formatting");
        output
    }
}

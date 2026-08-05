//! Validated authored-plan signing requests.

use core::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};
use radroots_event::contract::event_contract;
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_protocol::runtime::v1::OperationId;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::sync::Arc;

use crate::{
    Actor, Error, SignerRequestId, SigningIntentId,
    authorization::{
        CurrentAuthoringAuthority, CurrentAuthoringDecision, CurrentRegistryAuthority,
        DeprecatedPlanPolicy, ManagedSigningPolicy,
    },
    error::Kind,
    status::SignProgress,
};

/// How a signer must interpret cancellation around remote publication.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellationPolicy {
    PreservePublishedRequest,
    LocalCooperative,
}

/// Runtime-local cooperative cancellation shared by caller and signer.
#[derive(Clone, Debug, Default)]
pub struct CancellationSignal(Arc<AtomicBool>);

impl CancellationSignal {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Explicit millisecond deadline and authorization/cancellation policy.
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignPolicy {
    deadline_unix_ms: u64,
    cancellation: CancellationPolicy,
    deprecated_plan: DeprecatedPlanPolicy,
    managed_signing: ManagedSigningPolicy,
}

impl SignPolicy {
    pub const fn new(
        deadline_unix_ms: u64,
        cancellation: CancellationPolicy,
    ) -> Result<Self, Error> {
        if deadline_unix_ms == 0 {
            return Err(Error::new(Kind::InvalidArgument));
        }
        Ok(Self {
            deadline_unix_ms,
            cancellation,
            deprecated_plan: DeprecatedPlanPolicy::Deny,
            managed_signing: ManagedSigningPolicy::AnyValidatedSource,
        })
    }

    #[must_use]
    pub const fn allowing_deprecated(mut self) -> Self {
        self.deprecated_plan = DeprecatedPlanPolicy::Allow;
        self
    }

    #[must_use]
    pub const fn with_managed_signing_policy(mut self, policy: ManagedSigningPolicy) -> Self {
        self.managed_signing = policy;
        self
    }

    #[must_use]
    pub const fn deadline_unix_ms(self) -> u64 {
        self.deadline_unix_ms
    }

    #[must_use]
    pub const fn cancellation(self) -> CancellationPolicy {
        self.cancellation
    }

    #[must_use]
    pub const fn deprecated_plan(self) -> DeprecatedPlanPolicy {
        self.deprecated_plan
    }

    #[must_use]
    pub const fn managed_signing(self) -> ManagedSigningPolicy {
        self.managed_signing
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SignPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            deadline_unix_ms: u64,
            cancellation: CancellationPolicy,
            deprecated_plan: DeprecatedPlanPolicy,
            managed_signing: ManagedSigningPolicy,
        }

        let value = Repr::deserialize(deserializer)?;
        let mut policy = Self::new(value.deadline_unix_ms, value.cancellation)
            .map_err(serde::de::Error::custom)?;
        policy.deprecated_plan = value.deprecated_plan;
        policy.managed_signing = value.managed_signing;
        Ok(policy)
    }
}

/// Runtime-local observer for signing progress.
pub trait ProgressObserver: Send + Sync {
    fn on_progress(&self, progress: &SignProgress);
}

/// One currently authorized exact plan and bounded signer invocation.
#[derive(Clone)]
pub struct SignRequest {
    operation_kind: OperationId,
    intent_id: SigningIntentId,
    signer_request_id: SignerRequestId,
    actor: Actor,
    plan: AuthoredEventPlan,
    authorization: CurrentAuthoringDecision,
    policy: SignPolicy,
    cancellation_signal: CancellationSignal,
    progress_observer: Option<Arc<dyn ProgressObserver>>,
}

impl SignRequest {
    pub fn new(
        operation_kind: OperationId,
        intent_id: SigningIntentId,
        actor: Actor,
        plan: AuthoredEventPlan,
        policy: SignPolicy,
    ) -> Result<Self, Error> {
        Self::new_with_authority(
            operation_kind,
            intent_id,
            actor,
            plan,
            policy,
            &CurrentRegistryAuthority,
        )
    }

    pub fn new_with_authority(
        operation_kind: OperationId,
        intent_id: SigningIntentId,
        actor: Actor,
        plan: AuthoredEventPlan,
        policy: SignPolicy,
        authority: &dyn CurrentAuthoringAuthority,
    ) -> Result<Self, Error> {
        let authorization = authority.evaluate(&plan);
        authorize(&actor, &plan, policy, authorization)?;
        let signer_request_id = SignerRequestId::derive(intent_id.artifact_id(), plan.digest());
        Ok(Self {
            operation_kind,
            intent_id,
            signer_request_id,
            actor,
            plan,
            authorization,
            policy,
            cancellation_signal: CancellationSignal::new(),
            progress_observer: None,
        })
    }

    #[must_use]
    pub fn with_cancellation_signal(mut self, signal: CancellationSignal) -> Self {
        self.cancellation_signal = signal;
        self
    }

    #[must_use]
    pub fn with_progress_observer(mut self, observer: Arc<dyn ProgressObserver>) -> Self {
        self.progress_observer = Some(observer);
        self
    }

    #[must_use]
    pub const fn operation_kind(&self) -> OperationId {
        self.operation_kind
    }

    #[must_use]
    pub const fn intent_id(&self) -> SigningIntentId {
        self.intent_id
    }

    #[must_use]
    pub const fn signer_request_id(&self) -> SignerRequestId {
        self.signer_request_id
    }

    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    #[must_use]
    pub const fn plan(&self) -> &AuthoredEventPlan {
        &self.plan
    }

    #[must_use]
    pub const fn authorization(&self) -> CurrentAuthoringDecision {
        self.authorization
    }

    #[must_use]
    pub const fn policy(&self) -> SignPolicy {
        self.policy
    }

    #[must_use]
    pub const fn cancellation_signal(&self) -> &CancellationSignal {
        &self.cancellation_signal
    }

    pub fn ensure_active(&self, now_unix_ms: u64) -> Result<(), Error> {
        if self.cancellation_signal.is_cancelled() {
            return Err(Error::new(Kind::SignerCancelled));
        }
        if now_unix_ms >= self.policy.deadline_unix_ms {
            return Err(Error::new(Kind::DeadlineExceeded));
        }
        Ok(())
    }

    pub fn report_progress(&self, progress: &SignProgress) {
        if let Some(observer) = &self.progress_observer {
            observer.on_progress(progress);
        }
    }
}

fn authorize(
    actor: &Actor,
    plan: &AuthoredEventPlan,
    policy: SignPolicy,
    decision: CurrentAuthoringDecision,
) -> Result<(), Error> {
    match decision {
        CurrentAuthoringDecision::Allowed => {}
        CurrentAuthoringDecision::AllowedDeprecated { .. }
            if policy.deprecated_plan() == DeprecatedPlanPolicy::Allow => {}
        CurrentAuthoringDecision::AllowedDeprecated { .. }
        | CurrentAuthoringDecision::Blocked { .. }
        | CurrentAuthoringDecision::Revoked { .. } => {
            return Err(Error::new(Kind::AuthorizationDenied));
        }
    }
    let contract = event_contract(plan.body().contract().contract_id().as_str())
        .ok_or_else(|| Error::new(Kind::AuthorizationDenied))?;
    if !actor.satisfies(contract.required_author_role())
        || actor.public_key() != *plan.author()
        || !policy.managed_signing().permits(actor)
    {
        return Err(Error::new(Kind::AuthorizationDenied));
    }
    Ok(())
}

impl fmt::Debug for SignRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignRequest")
            .field("operation_kind", &self.operation_kind)
            .field("intent_id", &self.intent_id)
            .field("signer_request_id", &self.signer_request_id)
            .field("actor", &self.actor)
            .field("plan", &"[redacted authored event plan]")
            .field("authorization", &self.authorization)
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

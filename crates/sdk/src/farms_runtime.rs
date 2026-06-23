#[cfg(feature = "signer-adapters")]
use crate::workflow_runtime::enqueue_configured_signed_workflow;
#[cfg(feature = "runtime")]
use crate::{
    FarmsClient, RadrootsSdkError, RadrootsSdkTimestamp, SdkIdempotencyKey, SdkMutationState,
    SdkRelayTargetPolicy, SdkRelayUrlPolicy, farm,
    workflow_runtime::{SdkWorkflowEnqueueRequest, enqueue_signed_workflow},
};
#[cfg(feature = "runtime")]
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner};
#[cfg(feature = "runtime")]
use radroots_events::{
    contract::RadrootsActorRole,
    draft::RadrootsFrozenEventDraft,
    farm::RadrootsFarm,
    ids::{RadrootsAddressableCoordinate, RadrootsEventId},
    kinds::KIND_FARM,
};
#[cfg(feature = "runtime")]
use radroots_events_codec::wire::to_frozen_draft;
#[cfg(feature = "runtime")]
pub const FARM_PUBLISH_OPERATION_KIND: &str = "farm.publish.v1";

#[cfg(feature = "runtime")]
const FARM_PROFILE_CONTRACT_ID: &str = "radroots.farm.profile.v1";

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct FarmPreparePublishRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub farm: RadrootsFarm,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl FarmPreparePublishRequest {
    pub fn new(actor: RadrootsActorContext, farm: RadrootsFarm) -> Self {
        Self {
            actor,
            farm,
            created_at: None,
        }
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct FarmEnqueuePublishRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub farm: RadrootsFarm,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl FarmEnqueuePublishRequest {
    pub fn new(
        actor: RadrootsActorContext,
        farm: RadrootsFarm,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self {
            actor,
            farm,
            target_relays,
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn try_with_target_relays<I, S>(
        mut self,
        target_relays: I,
        policy: SdkRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.target_relays = SdkRelayTargetPolicy::try_explicit(target_relays, policy)?;
        Ok(self)
    }

    pub fn with_idempotency_key(mut self, idempotency_key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn try_with_idempotency_key(
        mut self,
        idempotency_key: impl AsRef<str>,
    ) -> Result<Self, RadrootsSdkError> {
        self.idempotency_key = Some(SdkIdempotencyKey::new(idempotency_key)?);
        Ok(self)
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct FarmPublishPlan {
    pub farm_addr: RadrootsAddressableCoordinate,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsFrozenEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct FarmEnqueueReceipt {
    pub farm_addr: RadrootsAddressableCoordinate,
    pub expected_event_id: RadrootsEventId,
    pub signed_event_id: RadrootsEventId,
    pub local_event_seq: i64,
    pub outbox_operation_id: i64,
    pub outbox_event_id: i64,
    pub state: SdkMutationState,
    pub idempotency_digest_prefix: Option<String>,
}

#[cfg(feature = "runtime")]
impl<'sdk> FarmsClient<'sdk> {
    pub fn prepare_publish(
        &self,
        request: FarmPreparePublishRequest,
    ) -> Result<FarmPublishPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        farm_publish_plan(&request.actor, request.farm, created_at)
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_publish(
        &self,
        request: FarmEnqueuePublishRequest,
    ) -> Result<FarmEnqueueReceipt, RadrootsSdkError> {
        let FarmEnqueuePublishRequest {
            actor,
            farm,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = FarmPreparePublishRequest {
            actor: actor.clone(),
            farm,
            created_at,
        };
        let plan = self.prepare_publish(prepare_request)?;
        self.enqueue_prepared_publish(&actor, plan, target_relays, idempotency_key)
            .await
    }

    pub async fn enqueue_publish_with_explicit_signer(
        &self,
        request: FarmEnqueuePublishRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<FarmEnqueueReceipt, RadrootsSdkError> {
        let FarmEnqueuePublishRequest {
            actor,
            farm,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = FarmPreparePublishRequest {
            actor: actor.clone(),
            farm,
            created_at,
        };
        let plan = self.prepare_publish(prepare_request)?;
        self.enqueue_prepared_publish_with_explicit_signer(
            &actor,
            plan,
            target_relays,
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_prepared_publish(
        &self,
        actor: &RadrootsActorContext,
        plan: FarmPublishPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<FarmEnqueueReceipt, RadrootsSdkError> {
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: FARM_PUBLISH_OPERATION_KIND,
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
        )
        .await?;
        Ok(farm_enqueue_receipt(plan, enqueue))
    }

    pub async fn enqueue_prepared_publish_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: FarmPublishPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<FarmEnqueueReceipt, RadrootsSdkError> {
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: FARM_PUBLISH_OPERATION_KIND,
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(farm_enqueue_receipt(plan, enqueue))
    }

    fn resolved_created_at(
        &self,
        created_at: Option<RadrootsSdkTimestamp>,
    ) -> Result<RadrootsSdkTimestamp, RadrootsSdkError> {
        match created_at {
            Some(created_at) => Ok(created_at),
            None => self.sdk.now(),
        }
    }
}

#[cfg(feature = "runtime")]
fn farm_enqueue_receipt(
    plan: FarmPublishPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> FarmEnqueueReceipt {
    FarmEnqueueReceipt {
        farm_addr: plan.farm_addr,
        expected_event_id: plan.expected_event_id,
        signed_event_id: enqueue.signed_event_id,
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        state: enqueue.state.into(),
        idempotency_digest_prefix: Some(enqueue.idempotency_digest_prefix),
    }
}

#[cfg(feature = "runtime")]
fn farm_publish_plan(
    actor: &RadrootsActorContext,
    farm_value: RadrootsFarm,
    created_at: RadrootsSdkTimestamp,
) -> Result<FarmPublishPlan, RadrootsSdkError> {
    require_farmer_actor(actor, "farm.prepare_publish")?;
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let parts =
        farm::build_draft(&farm_value).map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("farm publish draft encode failed: {error}"),
        })?;
    let farm_addr = farm_addr(actor, farm_value.d_tag.as_str())
        .expect("validated farm d tag forms a farm address");
    let frozen_draft = to_frozen_draft(
        parts,
        FARM_PROFILE_CONTRACT_ID,
        actor.pubkey().as_str(),
        created_at_nostr,
    )
    .expect("validated farm publish draft freezes");
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .expect("frozen farm draft produces a valid event id");
    Ok(FarmPublishPlan {
        farm_addr,
        expected_event_id,
        frozen_draft,
        created_at,
    })
}

#[cfg(feature = "runtime")]
fn require_farmer_actor(
    actor: &RadrootsActorContext,
    operation: &'static str,
) -> Result<(), RadrootsSdkError> {
    if actor.satisfies(RadrootsActorRole::Farmer) {
        Ok(())
    } else {
        Err(RadrootsSdkError::UnauthorizedActor {
            operation: operation.to_owned(),
            reason: "missing role Farmer".to_owned(),
        })
    }
}

#[cfg(feature = "runtime")]
fn farm_addr(
    actor: &RadrootsActorContext,
    d_tag: &str,
) -> Result<RadrootsAddressableCoordinate, RadrootsSdkError> {
    RadrootsAddressableCoordinate::parse(format!("{KIND_FARM}:{}:{d_tag}", actor.pubkey())).map_err(
        |error| RadrootsSdkError::InvalidRequest {
            message: format!("farm address is invalid: {error}"),
        },
    )
}

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/farms_runtime_tests.rs"]
mod tests;

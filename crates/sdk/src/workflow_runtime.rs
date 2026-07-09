#[cfg(feature = "signer-adapters")]
use crate::RadrootsSdkSignRequest;
use crate::{
    RadrootsClient, RadrootsSdkError, ReticulumPreviewBehavior, SatisfactionPolicy,
    SdkIdempotencyKey, TargetPolicy, TargetSet, TransportProfile, runtime::sdk_now_ms,
};
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner, sign_authorized_draft};
use radroots_event_store::{
    RadrootsEventIngest, RadrootsTransportObservation, RadrootsTransportObservationType,
};
use radroots_events::{
    RadrootsNostrEvent,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent},
    ids::RadrootsEventId,
};
use radroots_outbox::{
    RadrootsOutboxDeliveryPlanInput, RadrootsOutboxEnqueueStatus,
    RadrootsOutboxReticulumPreviewBehavior, RadrootsOutboxSignedOperationInput,
};
use radroots_transport::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RadrootsTransportKind, RadrootsTransportTarget,
};

const SDK_LOCAL_EVENT_ENDPOINT_URI: &str = "local:sdk";

pub(crate) struct SdkWorkflowEnqueueRequest<'a> {
    pub(crate) operation_kind: &'static str,
    pub(crate) actor: &'a RadrootsActorContext,
    pub(crate) frozen_draft: &'a RadrootsFrozenEventDraft,
    pub(crate) target_policy: TargetPolicy,
    pub(crate) satisfaction_policy: SatisfactionPolicy,
    pub(crate) idempotency_key: Option<SdkIdempotencyKey>,
}

pub(crate) struct SdkWorkflowEnqueueReceipt {
    pub(crate) signed_event_id: RadrootsEventId,
    pub(crate) local_event_seq: i64,
    pub(crate) outbox_operation_id: i64,
    pub(crate) outbox_event_id: i64,
    pub(crate) state: RadrootsOutboxEnqueueStatus,
    pub(crate) idempotency_digest_prefix: String,
}

pub(crate) async fn enqueue_signed_workflow(
    sdk: &RadrootsClient,
    request: SdkWorkflowEnqueueRequest<'_>,
    signer: &dyn RadrootsEventSigner,
) -> Result<SdkWorkflowEnqueueReceipt, RadrootsSdkError> {
    let delivery_plan =
        resolved_delivery_plan(sdk, &request.target_policy, &request.satisfaction_policy)?;
    let signed_event = sign_authorized_draft(request.actor, signer, request.frozen_draft)?;
    enqueue_signed_workflow_event(sdk, request, signed_event, delivery_plan).await
}

#[cfg(feature = "signer-adapters")]
pub(crate) async fn enqueue_configured_signed_workflow(
    sdk: &RadrootsClient,
    request: SdkWorkflowEnqueueRequest<'_>,
) -> Result<SdkWorkflowEnqueueReceipt, RadrootsSdkError> {
    let delivery_plan =
        resolved_delivery_plan(sdk, &request.target_policy, &request.satisfaction_policy)?;
    let signed_event = sdk
        .sign_with_configured_signer(RadrootsSdkSignRequest::new(
            request.operation_kind,
            request.actor,
            request.frozen_draft,
        ))
        .await?
        .signed_event;
    enqueue_signed_workflow_event(sdk, request, signed_event, delivery_plan).await
}

async fn enqueue_signed_workflow_event(
    sdk: &RadrootsClient,
    request: SdkWorkflowEnqueueRequest<'_>,
    signed_event: RadrootsSignedNostrEvent,
    delivery_plan: SdkResolvedDeliveryPlan,
) -> Result<SdkWorkflowEnqueueReceipt, RadrootsSdkError> {
    let idempotency_key = match request.idempotency_key {
        Some(idempotency_key) => idempotency_key,
        None => SdkIdempotencyKey::derive(
            request.operation_kind,
            request.frozen_draft.expected_event_id.as_str(),
            request.frozen_draft.expected_pubkey.as_str(),
        ),
    };
    let observed_at_ms = sdk_now_ms(sdk)?;
    let signed_event_id = RadrootsEventId::parse(request.frozen_draft.expected_event_id.as_str())
        .expect("frozen workflow draft has a valid expected event id");
    let delivery_plan_value = delivery_plan.delivery_plan;
    let idempotency_key_for_enqueue = idempotency_key.clone();
    let preflight_input = signed_outbox_input(
        request.operation_kind,
        request.frozen_draft,
        signed_event.clone(),
        delivery_plan_value.clone(),
        idempotency_key,
        false,
        observed_at_ms,
    );
    let preflight = sdk
        ._outbox
        .preflight_signed_operation_idempotency(&preflight_input)
        .await?;
    let partial_failure_digest_prefix =
        digest_prefix(preflight.operation_idempotency_digest.as_str());
    let event = event_from_signed(&signed_event);
    let local_import_observation = RadrootsTransportObservation::new(
        RadrootsTransportKind::Local,
        SDK_LOCAL_EVENT_ENDPOINT_URI,
        RadrootsTransportObservationType::LocalImport,
        observed_at_ms,
    )?;
    let ingest = RadrootsEventIngest::new(event, observed_at_ms)
        .with_raw_json(signed_event.raw_json.clone())
        .with_observation(local_import_observation);
    let ingest_receipt = sdk._event_store.ingest_event(ingest).await?;
    let outbox_input = signed_outbox_input(
        request.operation_kind,
        request.frozen_draft,
        signed_event,
        delivery_plan_value,
        idempotency_key_for_enqueue,
        ingest_receipt.inserted,
        observed_at_ms,
    );
    let outbox_receipt = sdk
        ._outbox
        .enqueue_signed_operation(outbox_input)
        .await
        .map_err(|error| {
            if matches!(
                error,
                radroots_outbox::RadrootsOutboxError::IdempotencyConflict { .. }
            ) {
                RadrootsSdkError::partial_outbox_idempotency_conflict_mutation(
                    signed_event_id.as_str(),
                    request.operation_kind,
                    partial_failure_digest_prefix.as_str(),
                )
            } else {
                RadrootsSdkError::partial_outbox_enqueue_mutation(
                    signed_event_id.as_str(),
                    request.operation_kind,
                    partial_failure_digest_prefix.as_str(),
                )
            }
        })?;
    let idempotency_digest_prefix =
        digest_prefix(outbox_receipt.operation_idempotency_digest.as_str());
    Ok(SdkWorkflowEnqueueReceipt {
        signed_event_id,
        local_event_seq: ingest_receipt.seq,
        outbox_operation_id: outbox_receipt.operation_id,
        outbox_event_id: outbox_receipt.outbox_event_id,
        state: outbox_receipt.status,
        idempotency_digest_prefix,
    })
}

struct SdkResolvedDeliveryPlan {
    delivery_plan: RadrootsOutboxDeliveryPlanInput,
}

fn resolved_delivery_plan(
    sdk: &RadrootsClient,
    target_policy: &TargetPolicy,
    satisfaction_policy: &SatisfactionPolicy,
) -> Result<SdkResolvedDeliveryPlan, RadrootsSdkError> {
    match target_policy {
        TargetPolicy::Explicit(target_policy) => {
            let targets = target_policy.clone().into_targets();
            let reticulum_preview_behavior =
                reticulum_preview_behavior_for_targets(sdk.transport_profile(), &targets);
            delivery_plan_from_targets(
                "explicit",
                targets,
                satisfaction_policy,
                reticulum_preview_behavior,
            )
        }
        TargetPolicy::DefaultProfile => {
            let transport_profile = sdk.transport_profile();
            let targets = transport_profile
                .target_set()?
                .map(TargetSet::into_targets)
                .unwrap_or_default();
            if targets.is_empty() && !satisfaction_policy.is_no_wait() {
                return Err(RadrootsSdkError::empty_transport_targets(
                    "publish transport profile",
                ));
            }
            delivery_plan_from_targets(
                transport_profile.transport_profile_id(),
                targets,
                satisfaction_policy,
                outbox_reticulum_preview_behavior(transport_profile),
            )
        }
        TargetPolicy::LocalOnly => {
            if !satisfaction_policy.is_no_wait() {
                return Err(RadrootsSdkError::InvalidRequest {
                    message: "local-only target policy requires no_wait satisfaction policy"
                        .to_owned(),
                });
            }
            delivery_plan_from_targets(
                "local_only",
                Vec::new(),
                satisfaction_policy,
                RadrootsOutboxReticulumPreviewBehavior::RejectDeliveryAttempts,
            )
        }
        TargetPolicy::MeshScope(scope) => {
            let target_set =
                TargetSet::transport_targets(vec![RadrootsTransportTarget::new_with_metadata(
                    RadrootsTransportKind::Reticulum,
                    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI,
                    Some(scope.transport_scope()),
                    None,
                )?])?;
            delivery_plan_from_targets(
                "mesh_scope",
                target_set.into_targets(),
                satisfaction_policy,
                outbox_reticulum_preview_behavior(sdk.transport_profile()),
            )
        }
    }
}

fn delivery_plan_from_targets(
    transport_profile_id: impl Into<String>,
    targets: Vec<RadrootsTransportTarget>,
    satisfaction_policy: &SatisfactionPolicy,
    reticulum_preview_behavior: RadrootsOutboxReticulumPreviewBehavior,
) -> Result<SdkResolvedDeliveryPlan, RadrootsSdkError> {
    let delivery_plan = RadrootsOutboxDeliveryPlanInput::new(
        transport_profile_id,
        1,
        satisfaction_policy.transport_satisfaction_policy()?,
        targets,
    )
    .with_reticulum_preview_behavior(reticulum_preview_behavior);
    Ok(SdkResolvedDeliveryPlan { delivery_plan })
}

fn reticulum_preview_behavior_for_targets(
    transport_profile: &TransportProfile,
    targets: &[RadrootsTransportTarget],
) -> RadrootsOutboxReticulumPreviewBehavior {
    if targets
        .iter()
        .any(|target| target.kind == RadrootsTransportKind::Reticulum)
    {
        outbox_reticulum_preview_behavior(transport_profile)
    } else {
        RadrootsOutboxReticulumPreviewBehavior::RejectDeliveryAttempts
    }
}

fn outbox_reticulum_preview_behavior(
    transport_profile: &TransportProfile,
) -> RadrootsOutboxReticulumPreviewBehavior {
    match transport_profile {
        TransportProfile::ReticulumPreview { profile } => {
            reticulum_preview_behavior(profile.behavior())
        }
        TransportProfile::Hybrid { profile } => {
            reticulum_preview_behavior(profile.reticulum_preview().behavior())
        }
        TransportProfile::LocalOnly
        | TransportProfile::Nostr { .. }
        | TransportProfile::Proxy { .. } => {
            RadrootsOutboxReticulumPreviewBehavior::RejectDeliveryAttempts
        }
    }
}

fn reticulum_preview_behavior(
    behavior: ReticulumPreviewBehavior,
) -> RadrootsOutboxReticulumPreviewBehavior {
    match behavior {
        ReticulumPreviewBehavior::RejectDeliveryAttempts => {
            RadrootsOutboxReticulumPreviewBehavior::RejectDeliveryAttempts
        }
        ReticulumPreviewBehavior::DeferDeliveryPlans => {
            RadrootsOutboxReticulumPreviewBehavior::DeferDeliveryPlans
        }
    }
}

fn digest_prefix(digest: &str) -> String {
    digest.chars().take(12).collect()
}

#[cfg(test)]
fn parse_event_id(value: &str, field: &str) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(value).map_err(|error| RadrootsSdkError::InvalidRequest {
        message: format!("{field} is invalid: {error}"),
    })
}

fn signed_outbox_input(
    operation_kind: &'static str,
    frozen_draft: &RadrootsFrozenEventDraft,
    signed_event: RadrootsSignedNostrEvent,
    delivery_plan: RadrootsOutboxDeliveryPlanInput,
    idempotency_key: SdkIdempotencyKey,
    event_store_inserted: bool,
    observed_at_ms: i64,
) -> RadrootsOutboxSignedOperationInput {
    RadrootsOutboxSignedOperationInput::new(
        operation_kind,
        frozen_draft.clone(),
        signed_event,
        delivery_plan,
        event_store_inserted,
        observed_at_ms,
        observed_at_ms,
    )
    .with_idempotency_key(idempotency_key.into_string())
}

fn event_from_signed(signed_event: &RadrootsSignedNostrEvent) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: signed_event.id.clone(),
        author: signed_event.pubkey.clone(),
        created_at: signed_event.created_at,
        kind: signed_event.kind,
        tags: signed_event.tags.clone(),
        content: signed_event.content.clone(),
        sig: signed_event.sig.clone(),
    }
}

#[cfg(test)]
#[path = "../tests/unit/workflow_runtime_tests.rs"]
mod tests;

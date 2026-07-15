#[cfg(feature = "signer-adapters")]
use crate::RadrootsSdkSignRequest;
use crate::{
    RadrootsClient, RadrootsSdkError, ReticulumPreviewBehavior, SatisfactionPolicy,
    SdkIdempotencyKey, TargetPolicy, TargetSet, TransportProfile, runtime::sdk_now_ms,
};
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner, sign_authorized_draft};
use radroots_event::{
    draft::{RadrootsEventDraft, RadrootsSignedEvent},
    ids::RadrootsEventId,
};
use radroots_event_store::{
    RadrootsEventIngest, RadrootsTransportObservation, RadrootsTransportObservationType,
};
use radroots_outbox::{
    RadrootsOutboxDeliveryPlanInput, RadrootsOutboxEnqueueStatus,
    RadrootsOutboxReticulumPreviewBehavior, RadrootsOutboxSignedOperationInput,
};
use radroots_transport::{RadrootsTransportKind, RadrootsTransportTarget};
use sha2::{Digest, Sha256};
use sqlx::Row;

const SDK_LOCAL_EVENT_ENDPOINT_URI: &str = "local:sdk";

pub(crate) struct SdkWorkflowEnqueueRequest<'a> {
    pub(crate) operation_kind: &'static str,
    pub(crate) actor: &'a RadrootsActorContext,
    pub(crate) frozen_draft: &'a RadrootsEventDraft,
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
    signed_event: RadrootsSignedEvent,
    delivery_plan: SdkResolvedDeliveryPlan,
) -> Result<SdkWorkflowEnqueueReceipt, RadrootsSdkError> {
    let idempotency_key =
        request
            .idempotency_key
            .ok_or_else(|| RadrootsSdkError::InvalidRequest {
                message: format!(
                    "{} requires an explicit UUIDv7 idempotency key",
                    request.operation_kind
                ),
            })?;
    let observed_at_ms = sdk_now_ms(sdk)?;
    let signed_event_id = RadrootsEventId::parse(request.frozen_draft.expected_event_id_str())
        .expect("frozen workflow draft has a valid expected event id");
    let delivery_plan_value = delivery_plan.delivery_plan;
    let idempotency_key_for_enqueue = idempotency_key.clone();
    let mut tx =
        sdk._event_store
            .pool()
            .begin()
            .await
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
    record_runtime_operation_journal(
        &mut tx,
        request.operation_kind,
        request.actor,
        request.frozen_draft,
        &idempotency_key_for_enqueue,
        observed_at_ms,
    )
    .await?;
    let local_import_observation = RadrootsTransportObservation::new(
        RadrootsTransportKind::Local,
        SDK_LOCAL_EVENT_ENDPOINT_URI,
        RadrootsTransportObservationType::LocalImport,
        observed_at_ms,
    )?;
    let ingest = RadrootsEventIngest::new(signed_event.clone(), observed_at_ms)
        .with_observation(local_import_observation);
    let ingest_receipt = sdk
        ._event_store
        .ingest_event_in_transaction(&mut tx, ingest)
        .await?;
    let outbox_input = signed_outbox_input(
        request.operation_kind,
        request.frozen_draft,
        signed_event,
        delivery_plan_value,
        idempotency_key_for_enqueue.clone(),
        ingest_receipt.inserted,
        observed_at_ms,
    );
    let outbox_receipt = sdk
        ._outbox
        .enqueue_signed_operation_in_transaction(&mut tx, outbox_input)
        .await?;
    complete_runtime_operation_journal(
        &mut tx,
        request.operation_kind,
        request.actor,
        &idempotency_key_for_enqueue,
        observed_at_ms,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
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
            let target_set = TargetSet::transport_targets(vec![
                RadrootsTransportTarget::reticulum_preview_with_metadata(
                    Some(scope.transport_scope()),
                    None,
                )?,
            ])?;
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
    frozen_draft: &RadrootsEventDraft,
    signed_event: RadrootsSignedEvent,
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

async fn record_runtime_operation_journal(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    operation_kind: &'static str,
    actor: &RadrootsActorContext,
    frozen_draft: &RadrootsEventDraft,
    idempotency_key: &SdkIdempotencyKey,
    observed_at_ms: i64,
) -> Result<(), RadrootsSdkError> {
    let request_digest = runtime_request_digest(operation_kind, actor, frozen_draft);
    if let Some(row) = sqlx::query(
        "SELECT request_digest_sha256_hex FROM sdk_runtime_operation_journal WHERE contract_version = ? AND operation_id = ? AND actor_pubkey = ? AND idempotency_key = ?",
    )
    .bind("1")
    .bind(operation_kind)
    .bind(actor.pubkey().as_str())
    .bind(idempotency_key.as_str())
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| RadrootsSdkError::EventStore {
        message: error.to_string(),
    })? {
        let existing_digest: String = row
            .try_get("request_digest_sha256_hex")
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
        if existing_digest != request_digest {
            return Err(RadrootsSdkError::IdempotencyConflict {
                operation_kind: operation_kind.to_owned(),
                expected_pubkey_prefix: actor.pubkey().as_str().chars().take(12).collect(),
                existing_digest_prefix: digest_prefix(existing_digest.as_str()),
                new_digest_prefix: digest_prefix(request_digest.as_str()),
            });
        }
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO sdk_runtime_operation_journal(contract_version, operation_id, actor_pubkey, idempotency_key, request_digest_sha256_hex, created_at_ms) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("1")
    .bind(operation_kind)
    .bind(actor.pubkey().as_str())
    .bind(idempotency_key.as_str())
    .bind(request_digest.as_str())
    .bind(observed_at_ms)
    .execute(&mut **tx)
    .await
    .map(|_| ())
    .map_err(|error| RadrootsSdkError::EventStore {
        message: error.to_string(),
    })
}

async fn complete_runtime_operation_journal(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    operation_kind: &'static str,
    actor: &RadrootsActorContext,
    idempotency_key: &SdkIdempotencyKey,
    observed_at_ms: i64,
) -> Result<(), RadrootsSdkError> {
    sqlx::query(
        "UPDATE sdk_runtime_operation_journal SET completed_at_ms = ? WHERE contract_version = ? AND operation_id = ? AND actor_pubkey = ? AND idempotency_key = ?",
    )
    .bind(observed_at_ms)
    .bind("1")
    .bind(operation_kind)
    .bind(actor.pubkey().as_str())
    .bind(idempotency_key.as_str())
    .execute(&mut **tx)
    .await
    .map(|_| ())
    .map_err(|error| RadrootsSdkError::EventStore {
        message: error.to_string(),
    })
}

fn runtime_request_digest(
    operation_kind: &'static str,
    actor: &RadrootsActorContext,
    frozen_draft: &RadrootsEventDraft,
) -> String {
    let digest_document = serde_json::json!({
        "contract_version": "1",
        "operation_id": operation_kind,
        "actor_pubkey": actor.pubkey().as_str(),
        "expected_event_id": frozen_draft.expected_event_id_str(),
        "expected_pubkey": frozen_draft.expected_pubkey_str(),
    });
    let bytes =
        serde_json::to_vec(&digest_document).expect("runtime journal digest document serializes");
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
#[path = "../tests/unit/workflow_runtime_tests.rs"]
mod tests;

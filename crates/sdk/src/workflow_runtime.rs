#[cfg(feature = "signer-adapters")]
use crate::RadrootsSdkSignRequest;
use crate::{
    RadrootsClient, RadrootsSdkError, ReticulumBehavior, SatisfactionPolicy, SdkIdempotencyKey,
    TargetPolicy, TargetSet, TransportProfile,
    runtime::{RuntimeRecoveryReceiptWrite, record_runtime_recovery_receipt, sdk_now_ms},
};
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner, sign_authorized_draft};
use radroots_event::{
    RadrootsEventKind, RadrootsEventKindClass,
    draft::{RadrootsEventDraft, RadrootsSignedEvent},
    ids::RadrootsEventId,
};
use radroots_event_store::{
    RadrootsEventIngest, RadrootsEventPersistence, RadrootsEventStoreError,
    RadrootsTransportObservation, RadrootsTransportObservationType,
};
use radroots_outbox::{
    RadrootsOutboxDeliveryPlanInput, RadrootsOutboxEnqueueStatus, RadrootsOutboxReticulumBehavior,
    RadrootsOutboxSignedOperationInput, RadrootsOutboxSignedTradeMutationInput,
};
use radroots_transport::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RadrootsTransportKind, RadrootsTransportTarget,
};
use sha2::{Digest, Sha256};
use sqlx::Row;

const SDK_LOCAL_EVENT_ENDPOINT_URI: &str = "local:sdk";
const SDK_RUNTIME_CONTRACT_VERSION: &str = "1";

pub(crate) struct SdkWorkflowEnqueueRequest<'a> {
    pub(crate) operation_kind: &'static str,
    pub(crate) actor: &'a RadrootsActorContext,
    pub(crate) frozen_draft: &'a RadrootsEventDraft,
    pub(crate) target_policy: TargetPolicy,
    pub(crate) satisfaction_policy: SatisfactionPolicy,
    pub(crate) idempotency_key: Option<SdkIdempotencyKey>,
}

#[derive(Debug)]
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
    ensure_durable_workflow_kind(&request)?;
    let delivery_plan =
        resolved_delivery_plan(sdk, &request.target_policy, &request.satisfaction_policy)?;
    let prepared = prepare_runtime_operation_journal(sdk, &request, &delivery_plan).await?;
    if let Some(receipt) = prepared.committed_receipt {
        return Ok(receipt);
    }
    mark_runtime_operation_state(
        sdk,
        &request,
        &prepared.idempotency_key,
        SdkRuntimeOperationState::SignaturePending,
        None,
    )
    .await?;
    let signed_event = match sign_authorized_draft(request.actor, signer, request.frozen_draft) {
        Ok(signed_event) => signed_event,
        Err(error) => {
            let sdk_error: RadrootsSdkError = error.into();
            record_runtime_operation_failure(sdk, &request, &prepared.idempotency_key, &sdk_error)
                .await?;
            return Err(sdk_error);
        }
    };
    match enqueue_signed_workflow_event(sdk, &request, signed_event, delivery_plan).await {
        Ok(receipt) => Ok(receipt),
        Err(error) => {
            record_runtime_operation_failure(sdk, &request, &prepared.idempotency_key, &error)
                .await?;
            Err(error)
        }
    }
}

#[cfg(feature = "signer-adapters")]
pub(crate) async fn enqueue_configured_signed_workflow(
    sdk: &RadrootsClient,
    request: SdkWorkflowEnqueueRequest<'_>,
) -> Result<SdkWorkflowEnqueueReceipt, RadrootsSdkError> {
    ensure_durable_workflow_kind(&request)?;
    let delivery_plan =
        resolved_delivery_plan(sdk, &request.target_policy, &request.satisfaction_policy)?;
    let prepared = prepare_runtime_operation_journal(sdk, &request, &delivery_plan).await?;
    if let Some(receipt) = prepared.committed_receipt {
        return Ok(receipt);
    }
    mark_runtime_operation_state(
        sdk,
        &request,
        &prepared.idempotency_key,
        SdkRuntimeOperationState::SignaturePending,
        None,
    )
    .await?;
    let signed_event = match sdk
        .sign_with_configured_signer(RadrootsSdkSignRequest::new(
            request.operation_kind,
            request.actor,
            request.frozen_draft,
        ))
        .await
    {
        Ok(receipt) => receipt.signed_event,
        Err(error) => {
            record_runtime_operation_failure(sdk, &request, &prepared.idempotency_key, &error)
                .await?;
            return Err(error);
        }
    };
    match enqueue_signed_workflow_event(sdk, &request, signed_event, delivery_plan).await {
        Ok(receipt) => Ok(receipt),
        Err(error) => {
            record_runtime_operation_failure(sdk, &request, &prepared.idempotency_key, &error)
                .await?;
            Err(error)
        }
    }
}

async fn enqueue_signed_workflow_event(
    sdk: &RadrootsClient,
    request: &SdkWorkflowEnqueueRequest<'_>,
    signed_event: RadrootsSignedEvent,
    delivery_plan: SdkResolvedDeliveryPlan,
) -> Result<SdkWorkflowEnqueueReceipt, RadrootsSdkError> {
    if radroots_event::kinds::TRADE_MUTATION_EVENT_KINDS.contains(&request.frozen_draft.kind_u32())
    {
        return enqueue_signed_trade_workflow_event(sdk, request, signed_event, delivery_plan)
            .await;
    }
    let idempotency_key =
        request
            .idempotency_key
            .clone()
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
    let mut tx =
        sdk._event_store
            .pool()
            .begin()
            .await
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
    ensure_runtime_operation_can_commit(&mut tx, request, &idempotency_key).await?;
    let local_import_observation = RadrootsTransportObservation::new(
        RadrootsTransportKind::Local,
        SDK_LOCAL_EVENT_ENDPOINT_URI,
        RadrootsTransportObservationType::LocalImport,
        observed_at_ms,
    )?;
    let ingest =
        workflow_event_ingest(request.operation_kind, signed_event.clone(), observed_at_ms)?
            .with_observation(local_import_observation);
    let ingest_receipt = sdk
        ._event_store
        .ingest_event_in_transaction(&mut tx, ingest)
        .await?;
    let (event_store_inserted, local_event_seq) = durable_event_persistence(
        ingest_receipt.event_id.as_str(),
        &ingest_receipt.persistence,
    )?;
    let outbox_input = signed_outbox_input(
        request.operation_kind,
        request.frozen_draft,
        signed_event,
        delivery_plan_value,
        idempotency_key.clone(),
        event_store_inserted,
        observed_at_ms,
    );
    let outbox_receipt = sdk
        ._outbox
        .enqueue_signed_operation_in_transaction(&mut tx, outbox_input)
        .await?;
    let idempotency_digest_prefix =
        digest_prefix(outbox_receipt.operation_idempotency_digest.as_str());
    let receipt = SdkWorkflowEnqueueReceipt {
        signed_event_id,
        local_event_seq,
        outbox_operation_id: outbox_receipt.operation_id,
        outbox_event_id: outbox_receipt.outbox_event_id,
        state: outbox_receipt.status,
        idempotency_digest_prefix,
    };
    commit_runtime_operation_journal(&mut tx, request, &idempotency_key, &receipt, observed_at_ms)
        .await?;
    tx.commit()
        .await
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?;
    Ok(receipt)
}

async fn enqueue_signed_trade_workflow_event(
    sdk: &RadrootsClient,
    request: &SdkWorkflowEnqueueRequest<'_>,
    signed_event: RadrootsSignedEvent,
    delivery_plan: SdkResolvedDeliveryPlan,
) -> Result<SdkWorkflowEnqueueReceipt, RadrootsSdkError> {
    let idempotency_key =
        request
            .idempotency_key
            .clone()
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
    let mut tx =
        sdk._event_store
            .pool()
            .begin()
            .await
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
    ensure_runtime_operation_can_commit(&mut tx, request, &idempotency_key).await?;
    let local_import_observation = RadrootsTransportObservation::new(
        RadrootsTransportKind::Local,
        SDK_LOCAL_EVENT_ENDPOINT_URI,
        RadrootsTransportObservationType::LocalImport,
        observed_at_ms,
    )?;
    let ingest =
        workflow_event_ingest(request.operation_kind, signed_event.clone(), observed_at_ms)?
            .with_observation(local_import_observation);
    let ingest_receipt = sdk
        ._event_store
        .ingest_event_in_transaction(&mut tx, ingest)
        .await?;
    let (event_store_inserted, local_event_seq) = durable_event_persistence(
        ingest_receipt.event_id.as_str(),
        &ingest_receipt.persistence,
    )?;
    tx.commit()
        .await
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?;
    let outbox_input = signed_trade_outbox_input(
        request.operation_kind,
        request.frozen_draft,
        signed_event,
        delivery_plan_value,
        idempotency_key.clone(),
        event_store_inserted,
        observed_at_ms,
    )?;
    let outbox_receipt = sdk
        ._outbox
        .enqueue_signed_trade_mutation_operation(outbox_input)
        .await?;
    let idempotency_digest_prefix =
        digest_prefix(outbox_receipt.operation_idempotency_digest.as_str());
    let receipt = SdkWorkflowEnqueueReceipt {
        signed_event_id,
        local_event_seq,
        outbox_operation_id: outbox_receipt.operation_id,
        outbox_event_id: outbox_receipt.outbox_event_id,
        state: outbox_receipt.status,
        idempotency_digest_prefix,
    };
    let mut tx =
        sdk._event_store
            .pool()
            .begin()
            .await
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
    commit_runtime_operation_journal(&mut tx, request, &idempotency_key, &receipt, observed_at_ms)
        .await?;
    tx.commit()
        .await
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?;
    Ok(receipt)
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
            let reticulum_behavior =
                reticulum_behavior_for_targets(sdk.transport_profile(), &targets);
            delivery_plan_from_targets("explicit", targets, satisfaction_policy, reticulum_behavior)
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
                outbox_reticulum_behavior(transport_profile),
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
                RadrootsOutboxReticulumBehavior::RejectDeliveryAttempts,
            )
        }
        TargetPolicy::MeshScope(scope) => {
            let target_set = TargetSet::transport_targets(vec![
                RadrootsTransportTarget::reticulum_with_metadata(
                    RADROOTS_RETICULUM_ENDPOINT_URI,
                    Some(scope.transport_scope()),
                    None,
                )?,
            ])?;
            delivery_plan_from_targets(
                "mesh_scope",
                target_set.into_targets(),
                satisfaction_policy,
                outbox_reticulum_behavior(sdk.transport_profile()),
            )
        }
    }
}

fn delivery_plan_from_targets(
    transport_profile_id: impl Into<String>,
    targets: Vec<RadrootsTransportTarget>,
    satisfaction_policy: &SatisfactionPolicy,
    reticulum_behavior: RadrootsOutboxReticulumBehavior,
) -> Result<SdkResolvedDeliveryPlan, RadrootsSdkError> {
    let delivery_plan = RadrootsOutboxDeliveryPlanInput::new(
        transport_profile_id,
        1,
        satisfaction_policy.transport_satisfaction_policy()?,
        targets,
    )
    .with_reticulum_behavior(reticulum_behavior);
    Ok(SdkResolvedDeliveryPlan { delivery_plan })
}

fn reticulum_behavior_for_targets(
    transport_profile: &TransportProfile,
    targets: &[RadrootsTransportTarget],
) -> RadrootsOutboxReticulumBehavior {
    if targets
        .iter()
        .any(|target| target.kind == RadrootsTransportKind::Reticulum)
    {
        outbox_reticulum_behavior(transport_profile)
    } else {
        RadrootsOutboxReticulumBehavior::RejectDeliveryAttempts
    }
}

fn outbox_reticulum_behavior(
    transport_profile: &TransportProfile,
) -> RadrootsOutboxReticulumBehavior {
    match transport_profile {
        TransportProfile::Reticulum { profile } => reticulum_behavior(profile.behavior()),
        TransportProfile::MultiTarget { profile } => {
            reticulum_behavior(profile.reticulum().behavior())
        }
        TransportProfile::LocalOnly | TransportProfile::Nostr { .. } => {
            RadrootsOutboxReticulumBehavior::RejectDeliveryAttempts
        }
    }
}

fn reticulum_behavior(behavior: ReticulumBehavior) -> RadrootsOutboxReticulumBehavior {
    match behavior {
        ReticulumBehavior::RejectDeliveryAttempts => {
            RadrootsOutboxReticulumBehavior::RejectDeliveryAttempts
        }
        ReticulumBehavior::DeferDeliveryPlans => {
            RadrootsOutboxReticulumBehavior::DeferDeliveryPlans
        }
    }
}

fn digest_prefix(digest: &str) -> String {
    digest.chars().take(12).collect()
}

fn ensure_durable_workflow_kind(
    request: &SdkWorkflowEnqueueRequest<'_>,
) -> Result<(), RadrootsSdkError> {
    if RadrootsEventKind::new(request.frozen_draft.kind_u32()).class()
        == RadrootsEventKindClass::Ephemeral
    {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "{} cannot enqueue ephemeral event kind {} into a durable workflow",
                request.operation_kind,
                request.frozen_draft.kind_u32()
            ),
        });
    }
    Ok(())
}

fn workflow_event_ingest(
    operation_kind: &str,
    signed_event: RadrootsSignedEvent,
    observed_at_ms: i64,
) -> Result<RadrootsEventIngest, RadrootsSdkError> {
    RadrootsEventIngest::from_signed_event(signed_event, observed_at_ms).map_err(
        |error| match error {
            RadrootsEventStoreError::Nip01Verification(error) => {
                RadrootsSdkError::SignerReturnedEventDrift {
                    operation: operation_kind.to_owned(),
                    reason: format!(
                        "signer returned an event that failed NIP-01 verification: {error}"
                    ),
                }
            }
            error => error.into(),
        },
    )
}

fn durable_event_persistence(
    event_id: &str,
    persistence: &RadrootsEventPersistence,
) -> Result<(bool, i64), RadrootsSdkError> {
    match persistence {
        RadrootsEventPersistence::Inserted { seq } => Ok((true, *seq)),
        RadrootsEventPersistence::Duplicate { seq } => Ok((false, *seq)),
        RadrootsEventPersistence::NotPersisted => Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "workflow event `{event_id}` requires durable local event-store persistence"
            ),
        }),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SdkRuntimeOperationState {
    Prepared,
    SignaturePending,
    Committed,
    Rejected,
    FailedRecoverable,
}

impl SdkRuntimeOperationState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::SignaturePending => "signature_pending",
            Self::Committed => "committed",
            Self::Rejected => "rejected",
            Self::FailedRecoverable => "failed_recoverable",
        }
    }

    fn from_str(value: &str) -> Result<Self, RadrootsSdkError> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "signature_pending" => Ok(Self::SignaturePending),
            "committed" => Ok(Self::Committed),
            "rejected" => Ok(Self::Rejected),
            "failed_recoverable" => Ok(Self::FailedRecoverable),
            _ => Err(RadrootsSdkError::EventStore {
                message: format!("unknown SDK runtime operation state `{value}`"),
            }),
        }
    }
}

struct PreparedRuntimeOperation {
    idempotency_key: SdkIdempotencyKey,
    committed_receipt: Option<SdkWorkflowEnqueueReceipt>,
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

fn signed_trade_outbox_input(
    operation_kind: &'static str,
    frozen_draft: &RadrootsEventDraft,
    signed_event: RadrootsSignedEvent,
    delivery_plan: RadrootsOutboxDeliveryPlanInput,
    idempotency_key: SdkIdempotencyKey,
    event_store_inserted: bool,
    observed_at_ms: i64,
) -> Result<RadrootsOutboxSignedTradeMutationInput, RadrootsSdkError> {
    let envelope =
        radroots_event::trade::trade_mutation_from_canonical_content(frozen_draft.content())
            .map_err(|error| RadrootsSdkError::InvalidRequest {
                message: format!("trade mutation draft content is invalid: {error}"),
            })?;
    let mutation_id = envelope
        .mutation_id
        .ok_or_else(|| RadrootsSdkError::InvalidRequest {
            message: "trade mutation draft content is missing mutation id".to_owned(),
        })?;
    Ok(RadrootsOutboxSignedTradeMutationInput::new(
        operation_kind,
        envelope.trade_id,
        mutation_id,
        hex::encode(Sha256::digest(frozen_draft.content().as_bytes())),
        frozen_draft.clone(),
        signed_event,
        delivery_plan,
        event_store_inserted,
        observed_at_ms,
        observed_at_ms,
    )
    .with_idempotency_key(idempotency_key.into_string()))
}

async fn prepare_runtime_operation_journal(
    sdk: &RadrootsClient,
    request: &SdkWorkflowEnqueueRequest<'_>,
    delivery_plan: &SdkResolvedDeliveryPlan,
) -> Result<PreparedRuntimeOperation, RadrootsSdkError> {
    let idempotency_key =
        request
            .idempotency_key
            .clone()
            .ok_or_else(|| RadrootsSdkError::InvalidRequest {
                message: format!(
                    "{} requires an explicit UUIDv7 idempotency key",
                    request.operation_kind
                ),
            })?;
    let observed_at_ms = sdk_now_ms(sdk)?;
    let command_hash = runtime_request_digest(request, &delivery_plan.delivery_plan);
    let frozen_draft_json = frozen_draft_json(request.frozen_draft)?;
    let expected_transport_id = request.frozen_draft.expected_event_id_str();
    let mut tx =
        sdk._event_store
            .pool()
            .begin()
            .await
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
    let committed_receipt = if let Some(row) = sqlx::query(
        "SELECT command_payload_hash, state, result_json FROM sdk_runtime_operation_journal WHERE contract_version = ? AND operation_kind = ? AND actor_pubkey = ? AND idempotency_key = ?",
    )
    .bind(SDK_RUNTIME_CONTRACT_VERSION)
    .bind(request.operation_kind)
    .bind(request.actor.pubkey().as_str())
    .bind(idempotency_key.as_str())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| RadrootsSdkError::EventStore {
        message: error.to_string(),
    })? {
        let existing_digest: String = row
            .try_get("command_payload_hash")
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
        if existing_digest != command_hash {
            let existing_digest_prefix = digest_prefix(existing_digest.as_str());
            let new_digest_prefix = digest_prefix(command_hash.as_str());
            let error = RadrootsSdkError::IdempotencyConflict {
                operation_kind: request.operation_kind.to_owned(),
                expected_pubkey_prefix: request.actor.pubkey().as_str().chars().take(12).collect(),
                existing_digest_prefix: existing_digest_prefix.clone(),
                new_digest_prefix: new_digest_prefix.clone(),
            };
            sqlx::query(
                "INSERT INTO sdk_runtime_recovery_receipt(recovery_code, operation_kind, actor_pubkey, idempotency_key, recovery_action, detail_json, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind("idempotency_conflict")
            .bind(request.operation_kind)
            .bind(request.actor.pubkey().as_str())
            .bind(idempotency_key.as_str())
            .bind("retry_operation_with_same_idempotency_key")
            .bind(
                serde_json::json!({
                    "existing_digest_prefix": existing_digest_prefix,
                    "new_digest_prefix": new_digest_prefix
                })
                .to_string(),
            )
            .bind(observed_at_ms)
            .execute(&mut *tx)
            .await
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
            tx.commit()
                .await
                .map_err(|error| RadrootsSdkError::EventStore {
                    message: error.to_string(),
                })?;
            return Err(error);
        }
        let state: String = row.try_get("state").map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?;
        if SdkRuntimeOperationState::from_str(state.as_str())?
            == SdkRuntimeOperationState::Committed
        {
            let result_json: String =
                row.try_get("result_json")
                    .map_err(|error| RadrootsSdkError::EventStore {
                        message: error.to_string(),
                    })?;
            Some(workflow_receipt_from_result_json(result_json.as_str())?)
        } else {
            sqlx::query(
                "UPDATE sdk_runtime_operation_journal SET frozen_draft_json = ?, expected_transport_id = ?, state = ?, last_error_code = NULL, last_error_detail = NULL, updated_at_ms = ? WHERE contract_version = ? AND operation_kind = ? AND actor_pubkey = ? AND idempotency_key = ?",
            )
            .bind(frozen_draft_json.as_str())
            .bind(expected_transport_id)
            .bind(SdkRuntimeOperationState::Prepared.as_str())
            .bind(observed_at_ms)
            .bind(SDK_RUNTIME_CONTRACT_VERSION)
            .bind(request.operation_kind)
            .bind(request.actor.pubkey().as_str())
            .bind(idempotency_key.as_str())
            .execute(&mut *tx)
            .await
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
            None
        }
    } else {
        sqlx::query(
            "INSERT INTO sdk_runtime_operation_journal(contract_version, operation_kind, actor_pubkey, idempotency_key, command_payload_hash, frozen_draft_json, expected_transport_id, mutation_id, state, result_json, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(SDK_RUNTIME_CONTRACT_VERSION)
        .bind(request.operation_kind)
        .bind(request.actor.pubkey().as_str())
        .bind(idempotency_key.as_str())
        .bind(command_hash.as_str())
        .bind(frozen_draft_json.as_str())
        .bind(expected_transport_id)
        .bind(mutation_id_from_draft(request.frozen_draft))
        .bind(SdkRuntimeOperationState::Prepared.as_str())
        .bind(Option::<String>::None)
        .bind(observed_at_ms)
        .bind(observed_at_ms)
        .execute(&mut *tx)
        .await
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?;
        None
    };
    tx.commit()
        .await
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?;
    Ok(PreparedRuntimeOperation {
        idempotency_key,
        committed_receipt,
    })
}

async fn mark_runtime_operation_state(
    sdk: &RadrootsClient,
    request: &SdkWorkflowEnqueueRequest<'_>,
    idempotency_key: &SdkIdempotencyKey,
    state: SdkRuntimeOperationState,
    error: Option<&RadrootsSdkError>,
) -> Result<(), RadrootsSdkError> {
    let observed_at_ms = sdk_now_ms(sdk)?;
    let (last_error_code, last_error_detail) = match error {
        Some(error) => (
            Some(error.code().to_owned()),
            Some(error.detail_json().to_string()),
        ),
        None => (None, None),
    };
    sqlx::query(
        "UPDATE sdk_runtime_operation_journal SET state = ?, last_error_code = ?, last_error_detail = ?, updated_at_ms = ? WHERE contract_version = ? AND operation_kind = ? AND actor_pubkey = ? AND idempotency_key = ?",
    )
    .bind(state.as_str())
    .bind(last_error_code)
    .bind(last_error_detail)
    .bind(observed_at_ms)
    .bind(SDK_RUNTIME_CONTRACT_VERSION)
    .bind(request.operation_kind)
    .bind(request.actor.pubkey().as_str())
    .bind(idempotency_key.as_str())
    .execute(sdk._event_store.pool())
    .await
    .map(|_| ())
    .map_err(|error| RadrootsSdkError::EventStore {
        message: error.to_string(),
    })
}

async fn record_runtime_operation_failure(
    sdk: &RadrootsClient,
    request: &SdkWorkflowEnqueueRequest<'_>,
    idempotency_key: &SdkIdempotencyKey,
    error: &RadrootsSdkError,
) -> Result<(), RadrootsSdkError> {
    let state = match error {
        RadrootsSdkError::SignerRequestRejected { .. }
        | RadrootsSdkError::SignerReturnedEventDrift { .. }
        | RadrootsSdkError::SignerPubkeyMismatch { .. }
        | RadrootsSdkError::UnauthorizedActor { .. } => SdkRuntimeOperationState::Rejected,
        _ => SdkRuntimeOperationState::FailedRecoverable,
    };
    mark_runtime_operation_state(sdk, request, idempotency_key, state, Some(error)).await?;
    let recovery = match error {
        RadrootsSdkError::SignerRequestTimedOut { .. } => Some((
            "signer_timeout",
            "retry_operation_with_same_idempotency_key",
        )),
        RadrootsSdkError::IdempotencyConflict { .. } => Some((
            "idempotency_conflict",
            "retry_operation_with_same_idempotency_key",
        )),
        _ => None,
    };
    if let Some((recovery_code, recovery_action)) = recovery {
        record_runtime_recovery_receipt(
            sdk._event_store.pool(),
            RuntimeRecoveryReceiptWrite {
                recovery_code,
                operation_kind: Some(request.operation_kind),
                actor_pubkey: Some(request.actor.pubkey().as_str()),
                idempotency_key: Some(idempotency_key.as_str()),
                recovery_action,
                detail_json: error.detail_json(),
                created_at_ms: sdk_now_ms(sdk)?,
            },
        )
        .await?;
    }
    Ok(())
}

async fn ensure_runtime_operation_can_commit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request: &SdkWorkflowEnqueueRequest<'_>,
    idempotency_key: &SdkIdempotencyKey,
) -> Result<(), RadrootsSdkError> {
    let row = sqlx::query(
        "SELECT state FROM sdk_runtime_operation_journal WHERE contract_version = ? AND operation_kind = ? AND actor_pubkey = ? AND idempotency_key = ?",
    )
    .bind(SDK_RUNTIME_CONTRACT_VERSION)
    .bind(request.operation_kind)
    .bind(request.actor.pubkey().as_str())
    .bind(idempotency_key.as_str())
    .fetch_one(tx.as_mut())
    .await
    .map_err(|error| RadrootsSdkError::EventStore {
        message: error.to_string(),
    })?;
    let state: String = row
        .try_get("state")
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?;
    match SdkRuntimeOperationState::from_str(state.as_str())? {
        SdkRuntimeOperationState::Prepared
        | SdkRuntimeOperationState::SignaturePending
        | SdkRuntimeOperationState::FailedRecoverable => Ok(()),
        SdkRuntimeOperationState::Committed => Err(RadrootsSdkError::InvalidRequest {
            message: "SDK runtime operation is already committed".to_owned(),
        }),
        SdkRuntimeOperationState::Rejected => Err(RadrootsSdkError::InvalidRequest {
            message: "SDK runtime operation is rejected".to_owned(),
        }),
    }
}

async fn commit_runtime_operation_journal(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request: &SdkWorkflowEnqueueRequest<'_>,
    idempotency_key: &SdkIdempotencyKey,
    receipt: &SdkWorkflowEnqueueReceipt,
    observed_at_ms: i64,
) -> Result<(), RadrootsSdkError> {
    let result_json = workflow_receipt_result_json(receipt);
    sqlx::query(
        "UPDATE sdk_runtime_operation_journal SET state = ?, result_json = ?, last_error_code = NULL, last_error_detail = NULL, updated_at_ms = ? WHERE contract_version = ? AND operation_kind = ? AND actor_pubkey = ? AND idempotency_key = ?",
    )
    .bind(SdkRuntimeOperationState::Committed.as_str())
    .bind(result_json.to_string())
    .bind(observed_at_ms)
    .bind(SDK_RUNTIME_CONTRACT_VERSION)
    .bind(request.operation_kind)
    .bind(request.actor.pubkey().as_str())
    .bind(idempotency_key.as_str())
    .execute(tx.as_mut())
    .await
    .map(|_| ())
    .map_err(|error| RadrootsSdkError::EventStore {
        message: error.to_string(),
    })
}

fn workflow_receipt_result_json(receipt: &SdkWorkflowEnqueueReceipt) -> serde_json::Value {
    serde_json::json!({
        "api_version": 1,
        "state": "committed",
        "signed_event_id": receipt.signed_event_id.as_str(),
        "local_event_seq": receipt.local_event_seq,
        "outbox_operation_id": receipt.outbox_operation_id,
        "outbox_event_id": receipt.outbox_event_id,
        "outbox_state": outbox_enqueue_status_str(receipt.state),
        "idempotency_digest_prefix": receipt.idempotency_digest_prefix
    })
}

fn workflow_receipt_from_result_json(
    result_json: &str,
) -> Result<SdkWorkflowEnqueueReceipt, RadrootsSdkError> {
    let value: serde_json::Value =
        serde_json::from_str(result_json).map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?;
    let signed_event_id = value
        .get("signed_event_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| RadrootsSdkError::EventStore {
            message: "committed SDK operation receipt is missing signed_event_id".to_owned(),
        })?;
    let local_event_seq = value
        .get("local_event_seq")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| RadrootsSdkError::EventStore {
            message: "committed SDK operation receipt is missing local_event_seq".to_owned(),
        })?;
    let outbox_operation_id = value
        .get("outbox_operation_id")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| RadrootsSdkError::EventStore {
            message: "committed SDK operation receipt is missing outbox_operation_id".to_owned(),
        })?;
    let outbox_event_id = value
        .get("outbox_event_id")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| RadrootsSdkError::EventStore {
            message: "committed SDK operation receipt is missing outbox_event_id".to_owned(),
        })?;
    let outbox_state = value
        .get("outbox_state")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| RadrootsSdkError::EventStore {
            message: "committed SDK operation receipt is missing outbox_state".to_owned(),
        })?;
    let idempotency_digest_prefix = value
        .get("idempotency_digest_prefix")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| RadrootsSdkError::EventStore {
            message: "committed SDK operation receipt is missing idempotency_digest_prefix"
                .to_owned(),
        })?
        .to_owned();
    Ok(SdkWorkflowEnqueueReceipt {
        signed_event_id: RadrootsEventId::parse(signed_event_id).map_err(|error| {
            RadrootsSdkError::EventStore {
                message: error.to_string(),
            }
        })?,
        local_event_seq,
        outbox_operation_id,
        outbox_event_id,
        state: outbox_enqueue_status_from_str(outbox_state)?,
        idempotency_digest_prefix,
    })
}

fn outbox_enqueue_status_str(status: RadrootsOutboxEnqueueStatus) -> &'static str {
    match status {
        RadrootsOutboxEnqueueStatus::Inserted => "inserted",
        RadrootsOutboxEnqueueStatus::Existing => "existing",
    }
}

fn outbox_enqueue_status_from_str(
    status: &str,
) -> Result<RadrootsOutboxEnqueueStatus, RadrootsSdkError> {
    match status {
        "inserted" => Ok(RadrootsOutboxEnqueueStatus::Inserted),
        "existing" => Ok(RadrootsOutboxEnqueueStatus::Existing),
        _ => Err(RadrootsSdkError::EventStore {
            message: format!("unknown outbox enqueue status `{status}`"),
        }),
    }
}

fn frozen_draft_json(frozen_draft: &RadrootsEventDraft) -> Result<String, RadrootsSdkError> {
    serde_json::to_string(&serde_json::json!({
        "contract_id": frozen_draft.contract_id(),
        "contract_registry_version": frozen_draft.contract_registry_version(),
        "kind": frozen_draft.kind_u32(),
        "created_at": frozen_draft.created_at_u64(),
        "tags": frozen_draft.tags_as_vec(),
        "content": frozen_draft.content(),
        "expected_pubkey": frozen_draft.expected_pubkey_str(),
        "expected_event_id": frozen_draft.expected_event_id_str()
    }))
    .map_err(|error| RadrootsSdkError::EventStore {
        message: error.to_string(),
    })
}

fn mutation_id_from_draft(frozen_draft: &RadrootsEventDraft) -> Option<String> {
    if !radroots_event::kinds::TRADE_MUTATION_EVENT_KINDS.contains(&frozen_draft.kind_u32()) {
        return None;
    }
    radroots_event::trade::trade_mutation_from_canonical_content(frozen_draft.content())
        .ok()
        .and_then(|envelope| {
            envelope
                .mutation_id
                .map(|mutation_id| mutation_id.to_string())
        })
}

fn runtime_request_digest(
    request: &SdkWorkflowEnqueueRequest<'_>,
    delivery_plan: &RadrootsOutboxDeliveryPlanInput,
) -> String {
    let mut targets = delivery_plan
        .targets
        .iter()
        .map(|target| {
            serde_json::json!({
                "kind": target.kind.canonical_label(),
                "uri": target.uri.as_str(),
                "scope": target.scope.as_ref().map(|scope| scope.as_str()),
                "label": target.label.as_ref().map(|label| label.as_str()),
                "fingerprint": target.fingerprint.as_str()
            })
        })
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| {
        left.get("fingerprint")
            .and_then(serde_json::Value::as_str)
            .cmp(&right.get("fingerprint").and_then(serde_json::Value::as_str))
    });
    let digest_document = serde_json::json!({
        "contract_version": SDK_RUNTIME_CONTRACT_VERSION,
        "operation_kind": request.operation_kind,
        "actor_pubkey": request.actor.pubkey().as_str(),
        "draft": {
            "contract_id": request.frozen_draft.contract_id(),
            "contract_registry_version": request.frozen_draft.contract_registry_version(),
            "kind": request.frozen_draft.kind_u32(),
            "created_at": request.frozen_draft.created_at_u64(),
            "tags": request.frozen_draft.tags_as_vec(),
            "content_sha256": hex::encode(Sha256::digest(request.frozen_draft.content().as_bytes())),
            "expected_pubkey": request.frozen_draft.expected_pubkey_str(),
            "expected_event_id": request.frozen_draft.expected_event_id_str()
        },
        "delivery_plan": {
            "transport_profile_id": delivery_plan.transport_profile_id.as_str(),
            "target_policy_version": delivery_plan.target_policy_version,
            "satisfaction_policy": &delivery_plan.satisfaction_policy,
            "reticulum_behavior": delivery_plan.reticulum_behavior.as_str(),
            "targets": targets
        }
    });
    let bytes =
        serde_json::to_vec(&digest_document).expect("runtime journal digest document serializes");
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
#[path = "../tests/unit/workflow_runtime_tests.rs"]
mod tests;

#[cfg(feature = "signer-adapters")]
use crate::RadrootsSdkSignRequest;
use crate::{
    RadrootsClient, RadrootsSdkError, SdkIdempotencyKey, SdkRelayTargetPolicy, SdkRelayTargetSet,
    runtime::sdk_now_ms,
};
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner, sign_authorized_draft};
use radroots_event_store::RadrootsEventIngest;
use radroots_events::{
    RadrootsNostrEvent,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent},
    ids::RadrootsEventId,
};
use radroots_outbox::{RadrootsOutboxEnqueueStatus, RadrootsOutboxSignedOperationInput};
use sha2::{Digest, Sha256};

pub(crate) struct SdkWorkflowEnqueueRequest<'a> {
    pub(crate) operation_kind: &'static str,
    pub(crate) actor: &'a RadrootsActorContext,
    pub(crate) frozen_draft: &'a RadrootsFrozenEventDraft,
    pub(crate) target_relays: SdkRelayTargetPolicy,
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
    let target_relays = resolved_target_relays(sdk, &request.target_relays)?;
    let signed_event = sign_authorized_draft(request.actor, signer, request.frozen_draft)?;
    enqueue_signed_workflow_event(sdk, request, signed_event, target_relays).await
}

#[cfg(feature = "signer-adapters")]
pub(crate) async fn enqueue_configured_signed_workflow(
    sdk: &RadrootsClient,
    request: SdkWorkflowEnqueueRequest<'_>,
) -> Result<SdkWorkflowEnqueueReceipt, RadrootsSdkError> {
    let target_relays = resolved_target_relays(sdk, &request.target_relays)?;
    let signed_event = sdk
        .sign_with_configured_signer(RadrootsSdkSignRequest::new(
            request.operation_kind,
            request.actor,
            request.frozen_draft,
        ))
        .await?
        .signed_event;
    enqueue_signed_workflow_event(sdk, request, signed_event, target_relays).await
}

async fn enqueue_signed_workflow_event(
    sdk: &RadrootsClient,
    request: SdkWorkflowEnqueueRequest<'_>,
    signed_event: RadrootsSignedNostrEvent,
    target_relays: SdkResolvedRelayTargets,
) -> Result<SdkWorkflowEnqueueReceipt, RadrootsSdkError> {
    let idempotency_key = match request.idempotency_key {
        Some(idempotency_key) => idempotency_key,
        None => SdkIdempotencyKey::derive(
            request.operation_kind,
            request.frozen_draft.expected_event_id.as_str(),
            request.frozen_draft.expected_pubkey.as_str(),
            target_relays.canonical_relays.as_slice(),
        ),
    };
    let observed_at_ms = sdk_now_ms(sdk)?;
    let signed_event_id = RadrootsEventId::parse(request.frozen_draft.expected_event_id.as_str())
        .expect("frozen workflow draft has a valid expected event id");
    let event = event_from_signed(&signed_event);
    let ingest = RadrootsEventIngest::new(event, observed_at_ms)
        .with_raw_json(signed_event.raw_json.clone());
    let ingest_receipt = sdk._event_store.ingest_event(ingest).await?;
    let canonical_target_relays = target_relays.canonical_relays.clone();
    let target_relay_values = target_relays.relays;
    let partial_failure_digest_prefix = outbox_idempotency_digest_prefix(
        request.operation_kind,
        request.frozen_draft,
        canonical_target_relays.as_slice(),
    );
    let outbox_input = signed_outbox_input(
        request.operation_kind,
        request.frozen_draft,
        signed_event,
        target_relay_values,
        idempotency_key,
        target_relays.allow_empty_target_relays,
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
    let idempotency_digest_prefix = digest_prefix(outbox_receipt.idempotency_digest.as_str());
    Ok(SdkWorkflowEnqueueReceipt {
        signed_event_id,
        local_event_seq: ingest_receipt.seq,
        outbox_operation_id: outbox_receipt.operation_id,
        outbox_event_id: outbox_receipt.outbox_event_id,
        state: outbox_receipt.status,
        idempotency_digest_prefix,
    })
}

struct SdkResolvedRelayTargets {
    relays: Vec<String>,
    canonical_relays: Vec<String>,
    allow_empty_target_relays: bool,
}

fn resolved_target_relays(
    sdk: &RadrootsClient,
    target_relays: &SdkRelayTargetPolicy,
) -> Result<SdkResolvedRelayTargets, RadrootsSdkError> {
    match target_relays {
        SdkRelayTargetPolicy::Explicit(target_relays) => Ok(SdkResolvedRelayTargets {
            relays: target_relays.relays().to_vec(),
            canonical_relays: target_relays.canonical_relays().to_vec(),
            allow_empty_target_relays: false,
        }),
        SdkRelayTargetPolicy::UseConfiguredRelays => {
            let target_relays =
                SdkRelayTargetSet::from_normalized_relays(sdk.relay_urls().to_vec())?;
            Ok(SdkResolvedRelayTargets {
                relays: target_relays.relays().to_vec(),
                canonical_relays: target_relays.canonical_relays().to_vec(),
                allow_empty_target_relays: false,
            })
        }
        SdkRelayTargetPolicy::UsePublishTransport => {
            if sdk
                .publish_transport()
                .supports_delegated_relay_resolution()
            {
                Ok(SdkResolvedRelayTargets {
                    relays: Vec::new(),
                    canonical_relays: Vec::new(),
                    allow_empty_target_relays: true,
                })
            } else {
                Err(RadrootsSdkError::empty_target_relays(
                    "publish transport relay resolution",
                ))
            }
        }
    }
}

#[derive(serde::Serialize)]
struct SdkWorkflowOutboxDigestInput<'a> {
    operation_kind: &'static str,
    expected_pubkey: &'a str,
    draft: &'a RadrootsFrozenEventDraft,
    target_relays: &'a [String],
}

fn outbox_idempotency_digest_prefix(
    operation_kind: &'static str,
    frozen_draft: &RadrootsFrozenEventDraft,
    target_relays: &[String],
) -> String {
    let input = SdkWorkflowOutboxDigestInput {
        operation_kind,
        expected_pubkey: frozen_draft.expected_pubkey.as_str(),
        draft: frozen_draft,
        target_relays,
    };
    let bytes = serde_json::to_vec(&input).expect("workflow digest input serializes");
    digest_prefix(hex::encode(Sha256::digest(bytes)).as_str())
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
    target_relays: Vec<String>,
    idempotency_key: SdkIdempotencyKey,
    allow_empty_target_relays: bool,
    event_store_inserted: bool,
    observed_at_ms: i64,
) -> RadrootsOutboxSignedOperationInput {
    let input = RadrootsOutboxSignedOperationInput::new(
        operation_kind,
        frozen_draft.clone(),
        signed_event,
        target_relays,
        event_store_inserted,
        observed_at_ms,
        observed_at_ms,
    )
    .with_idempotency_key(idempotency_key.into_string());
    if allow_empty_target_relays {
        input.allow_empty_target_relays()
    } else {
        input
    }
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

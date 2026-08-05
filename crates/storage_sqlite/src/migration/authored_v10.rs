use crate::{Error, authored, journal, outbox};
use core::num::{NonZeroU32, NonZeroU64};
use radroots_event::SignedEvent;
use radroots_event_codec::{Codec, verify};
use radroots_storage::{
    authored::{
        AdmissionState, AuthoredArtifact, AuthoredArtifactId, AuthoredOperation, FailureClass,
        RetrySchedule, WorkClaim, WorkFailure, WorkPhase,
    },
    authored_delivery::{AuthoredDeliveryPlan, AuthoredDeliveryPlanId, AuthoredDeliveryState},
    journal::{JournalState, OperationRecord},
    outbox::{OutboxRecord, OutboxStage},
};
use radroots_transport::{
    DeliveryReceipt,
    policy::{SatisfactionState, evaluate_satisfaction},
    sink::DeliveryTargetReceipt,
};
use sha2::{Digest, Sha256};
use sqlx::{Row, Sqlite, SqliteConnection};
use std::collections::BTreeSet;

const BLOCKED_IDENTIFIERS_MAX: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredV10Preflight {
    operation_count: u64,
    event_count: u64,
    outbox_count: u64,
    target_count: u64,
    attempt_count: u64,
    importable_count: u64,
    prepared_or_recoverable: u64,
    signed_without_complete_event: u64,
    invalid_or_unsupported: u64,
    blocked_operation_ids: Vec<[u8; 16]>,
    source_digest: [u8; 32],
}

impl AuthoredV10Preflight {
    pub const fn operation_count(&self) -> u64 {
        self.operation_count
    }
    pub const fn event_count(&self) -> u64 {
        self.event_count
    }
    pub const fn outbox_count(&self) -> u64 {
        self.outbox_count
    }
    pub const fn target_count(&self) -> u64 {
        self.target_count
    }
    pub const fn attempt_count(&self) -> u64 {
        self.attempt_count
    }
    pub const fn importable_count(&self) -> u64 {
        self.importable_count
    }
    pub const fn prepared_or_recoverable(&self) -> u64 {
        self.prepared_or_recoverable
    }
    pub const fn signed_without_complete_event(&self) -> u64 {
        self.signed_without_complete_event
    }
    pub const fn invalid_or_unsupported(&self) -> u64 {
        self.invalid_or_unsupported
    }
    pub fn blocked_operation_ids(&self) -> &[[u8; 16]] {
        self.blocked_operation_ids.as_slice()
    }
    pub const fn source_digest(&self) -> &[u8; 32] {
        &self.source_digest
    }
    pub const fn is_eligible(&self) -> bool {
        self.prepared_or_recoverable == 0
            && self.signed_without_complete_event == 0
            && self.invalid_or_unsupported == 0
    }

    pub(crate) fn blocked_error(&self) -> Error {
        Error::AuthoredMigrationBlocked {
            prepared_or_recoverable: self.prepared_or_recoverable,
            signed_without_complete_event: self.signed_without_complete_event,
            invalid_or_unsupported: self.invalid_or_unsupported,
        }
    }
}

struct Candidate {
    journal: OperationRecord,
    outbox: OutboxRecord,
    event_admitted_at_unix_ms: u64,
}

struct EventMetadata {
    event_id: [u8; 32],
    contract_id: &'static str,
    registry_version: u32,
}

pub(crate) struct InspectedV10 {
    pub(crate) report: AuthoredV10Preflight,
    candidates: Vec<Candidate>,
    event_metadata: Vec<EventMetadata>,
}

pub(crate) async fn inspect(connection: &mut SqliteConnection) -> Result<InspectedV10, Error> {
    let operation_count = count(connection, "radroots_runtime_journal_operations").await?;
    let event_count = count(connection, "radroots_runtime_events").await?;
    let outbox_count = count(connection, "radroots_runtime_outbox_items").await?;
    let target_count = count(connection, "radroots_runtime_outbox_targets").await?;
    let attempt_count = count(connection, "radroots_runtime_delivery_evidence").await?;

    let mut invalid_or_unsupported = orphan_count(connection).await?;
    let mut source_hasher = Sha256::new();
    source_hasher.update(b"radroots.authored.v10.preflight.v1");
    let event_rows =
        sqlx::query("SELECT event_id, signed_event FROM radroots_runtime_events ORDER BY event_id")
            .fetch_all(&mut *connection)
            .await
            .map_err(|_| metadata_error())?;
    let mut event_metadata = Vec::with_capacity(event_rows.len());
    let mut invalid_event_ids = BTreeSet::new();
    for row in event_rows {
        let event_id = array::<32>(row.try_get("event_id").map_err(|_| metadata_error())?)?;
        let raw_bytes = row
            .try_get::<Vec<u8>, _>("signed_event")
            .map_err(|_| metadata_error())?;
        source_hasher.update(event_id);
        source_hasher.update((raw_bytes.len() as u64).to_be_bytes());
        source_hasher.update(raw_bytes.as_slice());
        let Ok(raw) = String::from_utf8(raw_bytes) else {
            invalid_event_ids.insert(event_id);
            continue;
        };
        let Ok(event) = Codec::decode_signed_event(raw.as_str()) else {
            invalid_event_ids.insert(event_id);
            continue;
        };
        let Ok(contract) =
            radroots_event::contract::registry_v7::validate_event_contract_registry_v7(
                event.envelope(),
            )
        else {
            invalid_event_ids.insert(event_id);
            continue;
        };
        if event.id().as_bytes() != &event_id {
            invalid_event_ids.insert(event_id);
            continue;
        }
        event_metadata.push(EventMetadata {
            event_id,
            contract_id: contract.id,
            registry_version: radroots_event::contract::RegistryVersion::CURRENT.get(),
        });
    }

    let mut outboxes = Vec::new();
    let outbox_ids = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT item_id FROM radroots_runtime_outbox_items ORDER BY item_id",
    )
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| metadata_error())?;
    for bytes in outbox_ids {
        source_hasher.update((bytes.len() as u64).to_be_bytes());
        source_hasher.update(bytes.as_slice());
        let item_id = match radroots_storage::outbox::OutboxItemId::new(array(bytes)?) {
            Ok(value) => value,
            Err(_) => {
                invalid_or_unsupported = invalid_or_unsupported.saturating_add(1);
                continue;
            }
        };
        match outbox::load_record(connection, item_id).await {
            Ok(Some(record)) => outboxes.push(record),
            Ok(None) | Err(_) => {
                invalid_or_unsupported = invalid_or_unsupported.saturating_add(1);
            }
        }
    }

    let journal_rows =
        sqlx::query("SELECT * FROM radroots_runtime_journal_operations ORDER BY instance_id")
            .fetch_all(&mut *connection)
            .await
            .map_err(|_| metadata_error())?;
    let mut candidates = Vec::new();
    let mut matched_outboxes = BTreeSet::new();
    let mut prepared_or_recoverable = 0_u64;
    let mut signed_without_complete_event = 0_u64;
    let mut blocked_operation_ids = Vec::new();
    let mut referenced_event_ids = BTreeSet::new();

    for row in journal_rows {
        let raw_id = row
            .try_get::<Vec<u8>, _>("instance_id")
            .map_err(|_| metadata_error())?;
        source_hasher.update((raw_id.len() as u64).to_be_bytes());
        source_hasher.update(raw_id.as_slice());
        let Ok(record) = journal::decode_record(&row) else {
            invalid_or_unsupported = invalid_or_unsupported.saturating_add(1);
            push_blocked(&mut blocked_operation_ids, raw_id.as_slice());
            continue;
        };
        let operation_id = *record.instance_id().as_bytes();
        match record.state() {
            JournalState::Prepared | JournalState::Recoverable(_) => {
                prepared_or_recoverable = prepared_or_recoverable.saturating_add(1);
                push_blocked(&mut blocked_operation_ids, &operation_id);
            }
            JournalState::Signed { .. } => {
                signed_without_complete_event = signed_without_complete_event.saturating_add(1);
                push_blocked(&mut blocked_operation_ids, &operation_id);
            }
            JournalState::Committed { event_id, .. } => {
                referenced_event_ids.insert(*event_id.as_bytes());
                let Some(record_outbox) = outboxes
                    .iter()
                    .find(|outbox| outbox.operation_instance_id() == record.instance_id())
                    .cloned()
                else {
                    signed_without_complete_event = signed_without_complete_event.saturating_add(1);
                    push_blocked(&mut blocked_operation_ids, &operation_id);
                    continue;
                };
                matched_outboxes.insert(*record_outbox.item_id().as_bytes());
                if validate_candidate(connection, &record_outbox, event_id)
                    .await
                    .is_err()
                {
                    invalid_or_unsupported = invalid_or_unsupported.saturating_add(1);
                    push_blocked(&mut blocked_operation_ids, &operation_id);
                    continue;
                }
                let admitted_at = event_admitted_at(connection, event_id.as_bytes()).await?;
                source_hasher.update(record.input_digest().as_bytes());
                source_hasher.update(record_outbox.item_id().as_bytes());
                source_hasher.update(
                    record_outbox
                        .request()
                        .payload()
                        .event()
                        .raw_json()
                        .as_bytes(),
                );
                candidates.push(Candidate {
                    journal: record,
                    outbox: record_outbox,
                    event_admitted_at_unix_ms: admitted_at,
                });
            }
        }
    }
    let unmatched = outboxes
        .iter()
        .filter(|record| !matched_outboxes.contains(record.item_id().as_bytes()))
        .count();
    let unreferenced_invalid_events = invalid_event_ids.difference(&referenced_event_ids).count();
    invalid_or_unsupported = invalid_or_unsupported
        .saturating_add(u64::try_from(unmatched).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(unreferenced_invalid_events).unwrap_or(u64::MAX));

    let report = AuthoredV10Preflight {
        operation_count,
        event_count,
        outbox_count,
        target_count,
        attempt_count,
        importable_count: u64::try_from(candidates.len()).unwrap_or(u64::MAX),
        prepared_or_recoverable,
        signed_without_complete_event,
        invalid_or_unsupported,
        blocked_operation_ids,
        source_digest: source_hasher.finalize().into(),
    };
    Ok(InspectedV10 {
        report,
        candidates,
        event_metadata,
    })
}

pub(crate) async fn apply(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    inspected: &InspectedV10,
) -> Result<(), Error> {
    if !inspected.report.is_eligible() {
        return Err(inspected.report.blocked_error());
    }
    for metadata in &inspected.event_metadata {
        let result = sqlx::query(
            "UPDATE radroots_runtime_events
             SET admitted_contract_id = ?, admitted_registry_version = ?
             WHERE event_id = ? AND admitted_contract_id IS NULL
               AND admitted_registry_version IS NULL",
        )
        .bind(metadata.contract_id)
        .bind(i64::from(metadata.registry_version))
        .bind(metadata.event_id.as_slice())
        .execute(&mut **transaction)
        .await
        .map_err(|_| metadata_error())?;
        if result.rows_affected() != 1 {
            return Err(metadata_error());
        }
    }
    for candidate in &inspected.candidates {
        let (operation, artifact, plan) = convert_candidate(candidate)?;
        authored::persist_operation(transaction, &operation)
            .await
            .map_err(|_| metadata_error())?;
        authored::persist_artifact(transaction, &artifact)
            .await
            .map_err(|_| metadata_error())?;
        authored::persist_plan(transaction, &plan)
            .await
            .map_err(|_| metadata_error())?;
    }
    let operation_count = count_transaction(
        transaction,
        "SELECT COUNT(*) FROM radroots_runtime_authored_operations",
    )
    .await?;
    let artifact_count = count_transaction(
        transaction,
        "SELECT COUNT(*) FROM radroots_runtime_authored_artifacts",
    )
    .await?;
    let plan_count = count_transaction(
        transaction,
        "SELECT COUNT(*) FROM radroots_runtime_authored_delivery_plans",
    )
    .await?;
    if [operation_count, artifact_count, plan_count]
        .iter()
        .any(|count| *count != inspected.report.importable_count)
    {
        return Err(metadata_error());
    }
    let foreign_keys = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut **transaction)
        .await
        .map_err(|_| metadata_error())?;
    if !foreign_keys.is_empty() {
        return Err(metadata_error());
    }
    let integrity = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
        .fetch_all(&mut **transaction)
        .await
        .map_err(|_| metadata_error())?;
    if integrity.as_slice() != ["ok"] {
        return Err(metadata_error());
    }
    sqlx::query(
        "INSERT INTO radroots_runtime_authored_migration_evidence (
           source_version, operation_count, event_count, outbox_count,
           target_count, attempt_count, imported_count, source_digest,
           completed_at_unix_ms
         ) VALUES (10, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(i64_from_u64(inspected.report.operation_count)?)
    .bind(i64_from_u64(inspected.report.event_count)?)
    .bind(i64_from_u64(inspected.report.outbox_count)?)
    .bind(i64_from_u64(inspected.report.target_count)?)
    .bind(i64_from_u64(inspected.report.attempt_count)?)
    .bind(i64_from_u64(inspected.report.importable_count)?)
    .bind(inspected.report.source_digest.as_slice())
    .bind(migration_timestamp(inspected))
    .execute(&mut **transaction)
    .await
    .map_err(|_| metadata_error())?;
    Ok(())
}

fn convert_candidate(
    candidate: &Candidate,
) -> Result<(AuthoredOperation, AuthoredArtifact, AuthoredDeliveryPlan), Error> {
    let operation_id = candidate.journal.instance_id();
    let artifact_id = AuthoredArtifactId::new(derive_id(
        b"radroots.authored.v10.artifact.v1",
        operation_id.as_bytes(),
    ))
    .map_err(|_| metadata_error())?;
    let plan_id = AuthoredDeliveryPlanId::new(*candidate.outbox.item_id().as_bytes())
        .map_err(|_| metadata_error())?;
    let operation = AuthoredOperation::new(
        operation_id,
        vec![artifact_id],
        candidate.journal.prepared_at_unix_ms(),
    )
    .map_err(|_| metadata_error())?;
    let mut artifact = AuthoredArtifact::imported_signed(
        artifact_id,
        operation_id,
        0,
        candidate.outbox.request().payload().event().clone(),
        candidate.journal.prepared_at_unix_ms(),
    )
    .map_err(|_| metadata_error())?;
    let admitted_at = candidate
        .event_admitted_at_unix_ms
        .max(candidate.journal.prepared_at_unix_ms());
    let admission_claim = WorkClaim::new(
        derive_id(
            b"radroots.authored.v10.admission-claim.v1",
            artifact_id.as_bytes(),
        ),
        "migration-v10",
        NonZeroU64::MIN,
        admitted_at,
        admitted_at.checked_add(1).ok_or_else(metadata_error)?,
        artifact.revision(),
    )
    .map_err(|_| metadata_error())?;
    artifact
        .set_admission_claim(admission_claim, admitted_at)
        .map_err(|_| metadata_error())?;
    artifact
        .record_admission(AdmissionState::Inserted, None, None, admitted_at)
        .map_err(|_| metadata_error())?;

    let mut plan = AuthoredDeliveryPlan::new_bound(
        plan_id,
        artifact_id,
        candidate.outbox.request().clone(),
        candidate.outbox.created_at_unix_ms(),
    )
    .map_err(|_| metadata_error())?;
    replay_evidence(&mut plan, &candidate.outbox)?;
    Ok((operation, artifact, plan))
}

fn replay_evidence(plan: &mut AuthoredDeliveryPlan, legacy: &OutboxRecord) -> Result<(), Error> {
    let Some(last_attempt) = legacy.last_attempt() else {
        if !legacy.evidence().is_empty()
            || !matches!(legacy.stage(), OutboxStage::Pending | OutboxStage::Leased)
        {
            return Err(metadata_error());
        }
        return Ok(());
    };
    let mut cumulative = Vec::new();
    for attempt_number in 1..=last_attempt.get() {
        let evidence = legacy
            .evidence()
            .iter()
            .filter(|entry| entry.attempt().get() == attempt_number)
            .collect::<Vec<_>>();
        if evidence.len() != legacy.request().target_set().len() {
            return Err(metadata_error());
        }
        let recorded_at = evidence
            .first()
            .map(|entry| entry.recorded_at_unix_ms())
            .ok_or_else(metadata_error)?;
        if evidence
            .iter()
            .any(|entry| entry.recorded_at_unix_ms() != recorded_at)
        {
            return Err(metadata_error());
        }
        let mut receipts = Vec::with_capacity(evidence.len());
        for target in legacy.request().target_set().targets() {
            let entry = evidence
                .iter()
                .find(|entry| entry.target() == target.fingerprint())
                .ok_or_else(metadata_error)?;
            let receipt = if entry.was_attempted() {
                DeliveryTargetReceipt::attempted(target.clone(), entry.outcome().clone())
            } else {
                DeliveryTargetReceipt::skipped(target.clone(), entry.outcome().clone())
                    .map_err(|_| metadata_error())?
            };
            receipts.push(receipt);
        }
        cumulative.extend(receipts.iter().cloned());
        let receipt = DeliveryReceipt::for_request(legacy.request(), receipts)
            .map_err(|_| metadata_error())?;
        let satisfaction = evaluate_satisfaction(
            legacy.request().satisfaction(),
            legacy.request().target_set(),
            cumulative
                .iter()
                .map(|entry| (entry.target().fingerprint(), entry.outcome())),
        )
        .map_err(|_| metadata_error())?;
        let retry = if satisfaction == SatisfactionState::Pending {
            let next_at = legacy
                .evidence()
                .iter()
                .find(|entry| entry.attempt().get() == attempt_number.saturating_add(1))
                .map(|entry| entry.recorded_at_unix_ms())
                .or(legacy.retry_not_before_unix_ms())
                .unwrap_or_else(|| legacy.updated_at_unix_ms().max(recorded_at));
            let failure = WorkFailure::new(
                "migrated_delivery_pending",
                WorkPhase::Delivery,
                FailureClass::Retryable,
                Some(next_at),
                None,
            )
            .map_err(|_| metadata_error())?;
            Some(
                RetrySchedule::new(
                    NonZeroU32::new(attempt_number).ok_or_else(metadata_error)?,
                    next_at,
                    failure,
                )
                .map_err(|_| metadata_error())?,
            )
        } else {
            None
        };
        let claim = WorkClaim::new(
            derive_attempt_token(plan.plan_id(), attempt_number),
            "migration-v10",
            NonZeroU64::new(u64::from(attempt_number)).ok_or_else(metadata_error)?,
            recorded_at,
            recorded_at.checked_add(1).ok_or_else(metadata_error)?,
            plan.revision(),
        )
        .map_err(|_| metadata_error())?;
        let token = *claim.token();
        let generation = claim.generation();
        let revision = claim.row_revision();
        plan.claim(claim, recorded_at)
            .map_err(|_| metadata_error())?;
        plan.apply_receipt(&token, generation, revision, receipt, retry, recorded_at)
            .map_err(|_| metadata_error())?;
    }
    let state_matches = matches!(
        (legacy.stage(), plan.state()),
        (OutboxStage::Satisfied, AuthoredDeliveryState::Satisfied)
            | (OutboxStage::Exhausted, AuthoredDeliveryState::Exhausted)
            | (OutboxStage::Retryable, AuthoredDeliveryState::Retryable)
            | (OutboxStage::Leased, AuthoredDeliveryState::Retryable)
    );
    if !state_matches {
        return Err(metadata_error());
    }
    Ok(())
}

async fn validate_candidate(
    connection: &mut SqliteConnection,
    outbox: &OutboxRecord,
    event_id: &radroots_event::EventId,
) -> Result<(), Error> {
    let event = outbox.request().payload().event();
    if event.id() != event_id {
        return Err(metadata_error());
    }
    let stored = sqlx::query("SELECT signed_event FROM radroots_runtime_events WHERE event_id = ?")
        .bind(event_id.as_bytes().as_slice())
        .fetch_optional(&mut *connection)
        .await
        .map_err(|_| metadata_error())?
        .ok_or_else(metadata_error)?;
    if stored
        .try_get::<Vec<u8>, _>("signed_event")
        .map_err(|_| metadata_error())?
        .as_slice()
        != event.raw_json().as_bytes()
    {
        return Err(metadata_error());
    }
    verify_exact_event(event)?;
    Ok(())
}

fn verify_exact_event(event: &SignedEvent) -> Result<(), Error> {
    let raw = verify::RawEvent::new(event.envelope().clone());
    let id = verify::id(raw).map_err(|_| metadata_error())?;
    let signature =
        verify::signature(id, &verify::Nip01SignatureVerifier).map_err(|_| metadata_error())?;
    verify::contract(signature).map_err(|_| metadata_error())?;
    Ok(())
}

async fn event_admitted_at(
    connection: &mut SqliteConnection,
    event_id: &[u8; 32],
) -> Result<u64, Error> {
    let value = sqlx::query_scalar::<_, i64>(
        "SELECT admitted_at_unix_ms FROM radroots_runtime_events WHERE event_id = ?",
    )
    .bind(event_id.as_slice())
    .fetch_one(&mut *connection)
    .await
    .map_err(|_| metadata_error())?;
    u64_from_i64(value)
}

async fn orphan_count(connection: &mut SqliteConnection) -> Result<u64, Error> {
    let value = sqlx::query_scalar::<_, i64>(
        "SELECT
           (SELECT COUNT(*) FROM radroots_runtime_outbox_targets AS target
             LEFT JOIN radroots_runtime_outbox_items AS item ON item.item_id = target.item_id
             WHERE item.item_id IS NULL)
           +
           (SELECT COUNT(*) FROM radroots_runtime_delivery_evidence AS evidence
             LEFT JOIN radroots_runtime_outbox_targets AS target
               ON target.item_id = evidence.item_id
              AND target.target_fingerprint = evidence.target_fingerprint
             WHERE target.item_id IS NULL)",
    )
    .fetch_one(&mut *connection)
    .await
    .map_err(|_| metadata_error())?;
    u64_from_i64(value)
}

async fn count(connection: &mut SqliteConnection, table: &'static str) -> Result<u64, Error> {
    let query = match table {
        "radroots_runtime_journal_operations" => {
            "SELECT COUNT(*) FROM radroots_runtime_journal_operations"
        }
        "radroots_runtime_events" => "SELECT COUNT(*) FROM radroots_runtime_events",
        "radroots_runtime_outbox_items" => "SELECT COUNT(*) FROM radroots_runtime_outbox_items",
        "radroots_runtime_outbox_targets" => "SELECT COUNT(*) FROM radroots_runtime_outbox_targets",
        "radroots_runtime_delivery_evidence" => {
            "SELECT COUNT(*) FROM radroots_runtime_delivery_evidence"
        }
        _ => return Err(metadata_error()),
    };
    u64_from_i64(
        sqlx::query_scalar::<_, i64>(query)
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| metadata_error())?,
    )
}

async fn count_transaction(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    query: &'static str,
) -> Result<u64, Error> {
    u64_from_i64(
        sqlx::query_scalar::<_, i64>(query)
            .fetch_one(&mut **transaction)
            .await
            .map_err(|_| metadata_error())?,
    )
}

fn migration_timestamp(inspected: &InspectedV10) -> i64 {
    inspected
        .candidates
        .iter()
        .map(|candidate| candidate.outbox.updated_at_unix_ms())
        .max()
        .unwrap_or(1)
        .try_into()
        .unwrap_or(i64::MAX)
}

fn derive_attempt_token(plan_id: AuthoredDeliveryPlanId, attempt: u32) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"radroots.authored.v10.delivery-claim.v1");
    hasher.update(plan_id.as_bytes());
    hasher.update(attempt.to_be_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let mut result = [0; 16];
    result.copy_from_slice(&digest[..16]);
    result
}

fn derive_id(domain: &[u8], input: &[u8; 16]) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(input);
    let digest: [u8; 32] = hasher.finalize().into();
    let mut result = [0; 16];
    result.copy_from_slice(&digest[..16]);
    result
}

fn push_blocked(target: &mut Vec<[u8; 16]>, bytes: &[u8]) {
    if target.len() < BLOCKED_IDENTIFIERS_MAX
        && let Ok(id) = <[u8; 16]>::try_from(bytes)
    {
        target.push(id);
    }
}

fn array<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N], Error> {
    bytes.try_into().map_err(|_| metadata_error())
}

fn i64_from_u64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| metadata_error())
}

fn u64_from_i64(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| metadata_error())
}

const fn metadata_error() -> Error {
    Error::SchemaMigrationFailed {
        database: "runtime.sqlite",
        target_version: 11,
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::{SqliteStorage, migration::runtime};
    use radroots_storage::{
        Journal, Outbox,
        event::SourceGeneration,
        journal::{
            IdempotencyDigest, IdempotencyKey, JournalTransition, OperationId, OperationInstanceId,
            PrepareOperation,
        },
        outbox::{DeliveryPlanDigest, EnqueueOutboxItem, OutboxItemId},
        status::EventStoreMode,
    };
    use radroots_transport::{
        DeliveryRequest, Target, TargetSet,
        policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
        sink::DeliveryPayload,
    };
    use sqlx::{Connection, sqlite::SqlitePoolOptions};

    const VALID_EVENT: &str = r#"{"id":"762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1800000100,"kind":0,"tags":[],"content":"{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}","sig":"4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109"}"#;

    async fn v10_store() -> SqliteStorage {
        let generation = SourceGeneration::new([61; 32]).expect("generation");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory SQLite");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign keys");
        for migration in runtime::MIGRATIONS.iter().take(10) {
            sqlx::raw_sql(runtime::migration_sql(migration.version()).expect("migration SQL"))
                .execute(&pool)
                .await
                .expect("runtime migration");
        }
        sqlx::raw_sql("PRAGMA application_id = 1380209236; PRAGMA user_version = 10")
            .execute(&pool)
            .await
            .expect("schema metadata");
        sqlx::query(
            "INSERT INTO radroots_runtime_source_generations (
               generation, sequence_head, state, created_at_unix_ms, retired_at_unix_ms
             ) VALUES (?, 0, 'active', 1, NULL)",
        )
        .bind(generation.as_bytes().as_slice())
        .execute(&pool)
        .await
        .expect("source generation");
        SqliteStorage::new(pool, generation, EventStoreMode::ReadWrite)
    }

    fn event() -> SignedEvent {
        Codec::decode_signed_event(VALID_EVENT).expect("valid signed event")
    }

    fn operation_id(byte: u8) -> OperationInstanceId {
        OperationInstanceId::new([byte; 16]).expect("operation")
    }

    async fn prepare(store: &SqliteStorage, byte: u8) -> OperationRecord {
        store
            .prepare(
                PrepareOperation::new(
                    operation_id(byte),
                    OperationId::SyncPush,
                    IdempotencyKey::parse(format!("authored-v10-{byte}")).expect("idempotency key"),
                    IdempotencyDigest::new([byte; 32]),
                    100,
                )
                .expect("prepare operation"),
            )
            .await
            .expect("prepare")
            .record()
            .clone()
    }

    fn request(event: SignedEvent) -> DeliveryRequest {
        DeliveryRequest::new(
            "migration-delivery",
            DeliveryPayload::new(event),
            TargetSet::new(vec![
                Target::nostr_relay("wss://migration.example").expect("target"),
            ])
            .expect("target set"),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
            10_000,
        )
        .expect("delivery request")
    }

    async fn seed_complete(store: &SqliteStorage, byte: u8, event: SignedEvent) {
        let prepared = prepare(store, byte).await;
        let signed = store
            .transition(JournalTransition::signed(
                prepared.instance_id(),
                prepared.revision(),
                *event.id(),
            ))
            .await
            .expect("signed transition");
        let enqueue = EnqueueOutboxItem::new(
            OutboxItemId::new([byte.saturating_add(20); 16]).expect("outbox item"),
            prepared.instance_id(),
            DeliveryPlanDigest::new([byte.saturating_add(30); 32]),
            request(event.clone()),
            102,
        )
        .expect("enqueue");
        sqlx::query(
            "UPDATE radroots_runtime_source_generations
             SET sequence_head = sequence_head + 1
             WHERE generation = ?",
        )
        .bind(store.generation.as_bytes().as_slice())
        .execute(&store.pool)
        .await
        .expect("advance event sequence");
        sqlx::query(
            "INSERT INTO radroots_runtime_events (
               source_generation, source_sequence, event_id, admission_stage,
               signed_event, admitted_at_unix_ms, updated_at_unix_ms
             ) VALUES (?, 1, ?, 'raw', ?, 102, 102)",
        )
        .bind(store.generation.as_bytes().as_slice())
        .bind(event.id().as_bytes().as_slice())
        .bind(event.raw_json().as_bytes())
        .execute(&store.pool)
        .await
        .expect("legacy event admission");
        store.enqueue(enqueue).await.expect("outbox enqueue");
        store
            .transition(JournalTransition::committed(
                prepared.instance_id(),
                signed.revision(),
                *event.id(),
                102,
            ))
            .await
            .expect("committed transition");
    }

    #[tokio::test]
    async fn complete_rows_preflight_and_migrate_with_exact_signed_bytes_and_evidence() {
        let store = v10_store().await;
        let event = event();
        seed_complete(&store, 1, event.clone()).await;
        let mut connection = store.pool.acquire().await.expect("connection");
        let inspected = inspect(&mut connection).await.expect("preflight");
        assert!(inspected.report.is_eligible());
        assert_eq!(inspected.report.importable_count(), 1);
        assert_eq!(inspected.report.event_count(), 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA user_version")
                .fetch_one(&mut *connection)
                .await
                .expect("version after preflight"),
            10
        );
        crate::migration::migrate_runtime(&mut connection, crate::OpenMode::ReadWriteExisting)
            .await
            .expect("migration");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA user_version")
                .fetch_one(&mut *connection)
                .await
                .expect("version after migration"),
            11
        );
        let raw = sqlx::query_scalar::<_, Vec<u8>>(
            "SELECT signed_raw_json FROM radroots_runtime_authored_artifacts",
        )
        .fetch_one(&mut *connection)
        .await
        .expect("imported signed bytes");
        assert_eq!(raw.as_slice(), event.raw_json().as_bytes());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT imported_count FROM radroots_runtime_authored_migration_evidence",
            )
            .fetch_one(&mut *connection)
            .await
            .expect("migration evidence"),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT admitted_contract_id FROM radroots_runtime_events",
            )
            .fetch_one(&mut *connection)
            .await
            .expect("contract metadata"),
            "radroots.profile.metadata.v1"
        );
    }

    #[tokio::test]
    async fn prepared_and_event_id_only_rows_block_without_schema_mutation() {
        let store = v10_store().await;
        let prepared = prepare(&store, 2).await;
        let mut connection = store.pool.acquire().await.expect("connection");
        let report = inspect(&mut connection)
            .await
            .expect("prepared preflight")
            .report;
        assert!(!report.is_eligible());
        assert_eq!(report.prepared_or_recoverable(), 1);
        assert_eq!(
            report.blocked_operation_ids(),
            &[prepared.instance_id().as_bytes().to_owned()]
        );
        assert!(matches!(
            crate::migration::migrate_runtime(&mut connection, crate::OpenMode::ReadWriteExisting)
                .await,
            Err(Error::AuthoredMigrationBlocked {
                prepared_or_recoverable: 1,
                ..
            })
        ));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA user_version")
                .fetch_one(&mut *connection)
                .await
                .expect("preserved version"),
            10
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE name = 'radroots_runtime_authored_operations'",
            )
            .fetch_one(&mut *connection)
            .await
            .expect("no successor table"),
            0
        );

        drop(connection);
        let store = v10_store().await;
        let prepared = prepare(&store, 3).await;
        store
            .transition(JournalTransition::signed(
                prepared.instance_id(),
                prepared.revision(),
                *event().id(),
            ))
            .await
            .expect("signed transition");
        let mut connection = store.pool.acquire().await.expect("connection");
        let report = inspect(&mut connection)
            .await
            .expect("signed preflight")
            .report;
        assert!(!report.is_eligible());
        assert_eq!(report.signed_without_complete_event(), 1);
    }

    #[tokio::test]
    async fn invalid_signature_blocks_complete_rows_and_retains_v10_authority() {
        let store = v10_store().await;
        let valid = event();
        let mut wire = valid.wire().clone();
        wire.sig = "dd".repeat(64);
        let raw = serde_json::to_string(&wire).expect("invalid raw");
        let invalid = SignedEvent::from_wire_verified_id(wire, raw).expect("id-valid event");
        seed_complete(&store, 4, invalid).await;
        let mut connection = store.pool.acquire().await.expect("connection");
        let report = inspect(&mut connection).await.expect("preflight").report;
        assert!(!report.is_eligible());
        assert_eq!(report.invalid_or_unsupported(), 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA user_version")
                .fetch_one(&mut *connection)
                .await
                .expect("preserved version"),
            10
        );
    }

    #[tokio::test]
    async fn interrupted_conversion_rolls_back_schema_and_imported_rows() {
        let store = v10_store().await;
        seed_complete(&store, 5, event()).await;
        let mut connection = store.pool.acquire().await.expect("connection");
        let inspected = inspect(&mut connection).await.expect("preflight");
        let mut transaction = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("migration transaction");
        sqlx::raw_sql(runtime::migration_sql(11).expect("migration SQL"))
            .execute(&mut *transaction)
            .await
            .expect("successor schema");
        sqlx::raw_sql(
            "CREATE TEMP TRIGGER radroots_test_interrupt_authored_import
             BEFORE INSERT ON radroots_runtime_authored_artifacts
             BEGIN SELECT RAISE(ABORT, 'injected migration interruption'); END",
        )
        .execute(&mut *transaction)
        .await
        .expect("fault trigger");
        assert!(apply(&mut transaction, &inspected).await.is_err());
        transaction.rollback().await.expect("rollback");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA user_version")
                .fetch_one(&mut *connection)
                .await
                .expect("preserved version"),
            10
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE name = 'radroots_runtime_authored_operations'",
            )
            .fetch_one(&mut *connection)
            .await
            .expect("rolled-back schema"),
            0
        );
    }
}

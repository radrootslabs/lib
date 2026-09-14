//! Indexed issued claims and immutable fact-to-attempt provenance.

use super::{
    column, decode_receipt_row, load_artifact_tx, load_optional_plan_tx, map_backend, persist_plan,
};
use radroots_storage::{
    Error,
    atomic::AtomicCommitId,
    authored_atomic::{
        AuthoredAtomicCommand, AuthoredAtomicReceipt, ClaimAuthoredTarget, ClaimAuthoredWork,
        ReconcileDeliveryFacts,
    },
    authored_delivery::{
        AuthoredDeliveryHistory, AuthoredDeliveryPlan, AuthoredDeliveryPlanId,
        DELIVERY_PLAN_ATTEMPTS_MAX,
    },
};
use sqlx::{Sqlite, sqlite::SqliteRow};

const MARKERS: &str = "SELECT attempt, claim_id FROM radroots_runtime_authored_delivery_reconciliations WHERE plan_id = ? ORDER BY attempt LIMIT 1025";

async fn receipt(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    id: &[u8],
) -> Result<AuthoredAtomicReceipt, Error> {
    let row = sqlx::query("SELECT commit_id, commit_digest, requested_at_unix_ms, committed_at_unix_ms, receipt FROM radroots_runtime_authored_atomic_commits WHERE commit_id = ?")
        .bind(id).fetch_one(&mut **transaction).await.map_err(map_backend)?;
    decode_receipt_row(&row)
}

pub(super) async fn history(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    plan_id: AuthoredDeliveryPlanId,
) -> Result<Option<AuthoredDeliveryHistory>, Error> {
    let Some(plan) = load_optional_plan_tx(transaction, plan_id).await? else {
        return Ok(None);
    };
    let operation_id = load_artifact_tx(transaction, plan.artifact_id())
        .await?
        .operation_id();
    let preparations: Vec<Vec<u8>> = sqlx::query_scalar("SELECT commit_id FROM radroots_runtime_authored_atomic_commits WHERE target_id = ? AND phase = 'prepare' AND json_type(CAST(receipt AS TEXT), '$.outcome.prepared') = 'object' ORDER BY commit_id LIMIT 2")
        .bind(operation_id.as_bytes().as_slice()).fetch_all(&mut **transaction).await.map_err(map_backend)?;
    if preparations.len() > 1 {
        return Err(Error::AtomicWorkflowMismatch);
    }
    let original = match preparations.first() {
        Some(id) => Some(receipt(transaction, id).await?),
        None => None,
    };
    let mut history = AuthoredDeliveryHistory::new(plan, original.as_ref())?;
    drop(original);
    // Fetch only bounded IDs, then decode one bounded original receipt at a time.
    let ids: Vec<Vec<u8>> = sqlx::query_scalar("SELECT claim_id FROM radroots_runtime_authored_delivery_claims WHERE plan_id = ? ORDER BY claim_id LIMIT 1025")
        .bind(plan_id.as_bytes().as_slice()).fetch_all(&mut **transaction).await.map_err(map_backend)?;
    if ids.len() > DELIVERY_PLAN_ATTEMPTS_MAX as usize {
        history.mark_truncated();
    }
    for id in ids.iter().take(DELIVERY_PLAN_ATTEMPTS_MAX as usize) {
        history.push_claim(&receipt(transaction, id).await?)?;
    }
    history.validate()?;
    Ok(Some(history))
}

pub(super) async fn record_claim(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    command: &AuthoredAtomicCommand,
) -> Result<(), Error> {
    let AuthoredAtomicCommand::Claim(value) = command else {
        return Ok(());
    };
    let ClaimAuthoredTarget::DeliveryPlan(plan_id) = value.target() else {
        return Ok(());
    };
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM (SELECT 1 FROM radroots_runtime_authored_delivery_claims WHERE plan_id = ? LIMIT 1024)")
        .bind(plan_id.as_bytes().as_slice()).fetch_one(&mut **transaction).await.map_err(map_backend)?;
    if count >= i64::from(DELIVERY_PLAN_ATTEMPTS_MAX) {
        return Err(Error::DeliveryAttemptOverflow);
    }
    sqlx::query(
        "INSERT INTO radroots_runtime_authored_delivery_claims (plan_id, claim_id) VALUES (?, ?)",
    )
    .bind(plan_id.as_bytes().as_slice())
    .bind(command.commit_id().as_bytes().as_slice())
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    Ok(())
}

pub(super) async fn reconcile(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    command: &ReconcileDeliveryFacts,
) -> Result<AuthoredDeliveryPlan, Error> {
    let history = history(transaction, command.plan_id())
        .await?
        .ok_or(Error::InvalidAuthoredDeliveryPlan)?;
    let plan = command.apply_to(&history)?;
    persist_plan(transaction, &plan).await?;
    Ok(plan)
}

fn marker_id(
    plan_id: AuthoredDeliveryPlanId,
    claim: &radroots_storage::authored::WorkClaim,
) -> AtomicCommitId {
    AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
        ClaimAuthoredTarget::DeliveryPlan(plan_id),
        claim.clone(),
    ))
    .commit_id()
}

fn validate_existing(
    plan: &AuthoredDeliveryPlan,
    rows: &[SqliteRow],
) -> Result<std::collections::BTreeSet<u32>, Error> {
    let mut existing = std::collections::BTreeSet::new();
    for row in rows {
        let ordinal = u32::try_from(column::<i64>(row, "attempt")?)
            .map_err(|_| Error::InvalidAuthoredDeliveryPlan)?;
        let index = ordinal
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .ok_or(Error::InvalidAuthoredDeliveryPlan)?;
        let claim = plan
            .attempts()
            .get(index)
            .and_then(|attempt| attempt.claim_evidence())
            .ok_or(Error::InvalidAuthoredDeliveryPlan)?;
        if !existing.insert(ordinal)
            || column::<Vec<u8>>(row, "claim_id")?.as_slice()
                != marker_id(plan.plan_id(), claim).as_bytes()
        {
            return Err(Error::InvalidAuthoredDeliveryPlan);
        }
    }
    Ok(existing)
}

pub(super) async fn persist(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    plan: &AuthoredDeliveryPlan,
) -> Result<(), Error> {
    let existing = sqlx::query(MARKERS)
        .bind(plan.plan_id().as_bytes().as_slice())
        .fetch_all(&mut **transaction)
        .await
        .map_err(map_backend)?;
    let existing = validate_existing(plan, &existing)?;
    for (attempt, claim) in plan
        .attempts()
        .iter()
        .filter_map(|attempt| {
            attempt
                .claim_evidence()
                .map(|claim| (attempt.attempt(), claim))
        })
        .filter(|(attempt, _)| !existing.contains(&attempt.get()))
    {
        sqlx::query("INSERT INTO radroots_runtime_authored_delivery_reconciliations (plan_id, attempt, claim_id) VALUES (?, ?, ?)")
            .bind(plan.plan_id().as_bytes().as_slice()).bind(i64::from(attempt.get()))
            .bind(marker_id(plan.plan_id(), claim).as_bytes().as_slice())
            .execute(&mut **transaction).await.map_err(map_backend)?;
    }
    Ok(())
}

pub(super) async fn validate(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    plan: &AuthoredDeliveryPlan,
) -> Result<(), Error> {
    let rows = sqlx::query(MARKERS)
        .bind(plan.plan_id().as_bytes().as_slice())
        .fetch_all(&mut **transaction)
        .await
        .map_err(map_backend)?;
    let count = plan
        .attempts()
        .iter()
        .filter(|attempt| attempt.claim_evidence().is_some())
        .count();
    if rows.len() != count {
        return Err(Error::InvalidAuthoredDeliveryPlan);
    }
    validate_existing(plan, &rows).map(|_| ())
}

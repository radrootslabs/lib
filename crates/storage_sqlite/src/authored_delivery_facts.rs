//! Normalized, append-only delivery facts in the authored transaction.

use crate::authored::{column, decode_snapshot, encode_snapshot, i64_from_u64, map_backend};
use radroots_storage::{
    Error,
    authored_atomic::{AuthoredAtomicCommand, ClaimAuthoredTarget, ClaimAuthoredWork},
    authored_delivery::{AuthoredDeliveryFact, AuthoredDeliveryPlan},
};
use sqlx::{Sqlite, sqlite::SqliteRow};

pub(crate) const SELECT: &str = "SELECT ordinal, claim_id, observed_at_unix_ms, fact_snapshot FROM radroots_runtime_authored_delivery_facts WHERE plan_id = ? ORDER BY ordinal";

fn claim_id(
    plan: &AuthoredDeliveryPlan,
    fact: &AuthoredDeliveryFact,
) -> radroots_storage::atomic::AtomicCommitId {
    AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
        ClaimAuthoredTarget::DeliveryPlan(plan.plan_id()),
        fact.claim().clone(),
    ))
    .commit_id()
}

pub(crate) async fn persist(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    plan: &AuthoredDeliveryPlan,
) -> Result<(), Error> {
    sqlx::query("UPDATE radroots_runtime_authored_delivery_plans SET stop_requested_at_unix_ms = ? WHERE plan_id = ?")
        .bind(plan.stop_requested_at_unix_ms().map(i64_from_u64).transpose()?)
        .bind(plan.plan_id().as_bytes().as_slice())
        .execute(&mut **transaction).await.map_err(map_backend)?;
    let existing = sqlx::query(SELECT)
        .bind(plan.plan_id().as_bytes().as_slice())
        .fetch_all(&mut **transaction)
        .await
        .map_err(map_backend)?;
    if existing.len() > plan.delivery_facts().len() {
        return Err(Error::InvalidAuthoredDeliveryPlan);
    }
    validate_prefix(plan, &existing)?;
    for (ordinal, fact) in plan
        .delivery_facts()
        .iter()
        .enumerate()
        .skip(existing.len())
    {
        sqlx::query("INSERT INTO radroots_runtime_authored_delivery_facts (plan_id, ordinal, claim_id, observed_at_unix_ms, fact_snapshot) VALUES (?, ?, ?, ?, ?)")
            .bind(plan.plan_id().as_bytes().as_slice())
            .bind(i64::try_from(ordinal).map_err(|_| Error::InvalidAuthoredDeliveryPlan)?)
            .bind(claim_id(plan, fact).as_bytes().as_slice())
            .bind(i64_from_u64(fact.observed_at_unix_ms())?)
            .bind(encode_snapshot(fact)?)
            .execute(&mut **transaction).await.map_err(map_backend)?;
    }
    Ok(())
}

pub(crate) fn validate(plan: &AuthoredDeliveryPlan, rows: &[SqliteRow]) -> Result<(), Error> {
    if rows.len() != plan.delivery_facts().len() {
        return Err(Error::InvalidAuthoredDeliveryPlan);
    }
    validate_prefix(plan, rows)
}

fn validate_prefix(plan: &AuthoredDeliveryPlan, rows: &[SqliteRow]) -> Result<(), Error> {
    for (ordinal, (row, expected)) in rows.iter().zip(plan.delivery_facts()).enumerate() {
        let decoded = decode_snapshot::<AuthoredDeliveryFact>(column(row, "fact_snapshot")?)?;
        if column::<i64>(row, "ordinal")?
            != i64::try_from(ordinal).map_err(|_| Error::InvalidAuthoredDeliveryPlan)?
            || column::<Vec<u8>>(row, "claim_id")?.as_slice() != claim_id(plan, expected).as_bytes()
            || column::<i64>(row, "observed_at_unix_ms")?
                != i64_from_u64(expected.observed_at_unix_ms())?
            || decoded != *expected
        {
            return Err(Error::InvalidAuthoredDeliveryPlan);
        }
    }
    Ok(())
}

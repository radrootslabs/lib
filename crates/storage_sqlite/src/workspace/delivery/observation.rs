use super::super::crypto::{DeliveryPayload, DeliveryScope, WorkspaceCrypto};
use super::{
    codec::{self, WorkRef},
    graph::Graph,
};
use radroots_domain::EnvelopeEventId;
use radroots_storage::command::{DeliveryAttempt, DeliveryObservation, OperationError};
use radroots_transport::sink::DeliveryTargetReceipt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqliteConnection};
use zeroize::Zeroizing;

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;

pub(super) const TABLE: &str = "CREATE TABLE chat_delivery_observations (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL, request_key BLOB NOT NULL, target_key BLOB NOT NULL CHECK(length(target_key)=32), kind INTEGER NOT NULL CHECK(kind BETWEEN 1 AND 6), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient,request_key,target_key,kind), FOREIGN KEY(workspace,operation_id,recipient,request_key) REFERENCES chat_delivery_attempts(workspace,operation_id,recipient,request_key)) STRICT, WITHOUT ROWID";
const MAX_OBSERVATIONS: usize = 2 * 16 * 12 * 6;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u8,
    work: WorkRef,
    request: String,
    event: EnvelopeEventId,
    envelope_digest: [u8; 32],
    target: DeliveryTargetReceipt,
    kind: u8,
    observed_at: Option<core::num::NonZeroU64>,
}
fn bad<E>(_: E) -> OperationError {
    OperationError::RecoveryRequired
}
fn target_key(value: &DeliveryObservation) -> [u8; 32] {
    Sha256::digest(
        value
            .target_receipt()
            .target()
            .fingerprint()
            .as_str()
            .as_bytes(),
    )
    .into()
}
fn context(scope: DeliveryScope, target: [u8; 32], kind: u8) -> DeliveryScope {
    let mut hash = Sha256::new();
    hash.update(b"radroots.delivery.observation.key.v1\0");
    hash.update(scope.request);
    hash.update(target);
    hash.update([kind]);
    DeliveryScope {
        request: hash.finalize().into(),
        ..scope
    }
}
pub(super) struct Fact {
    scope: DeliveryScope,
    target: [u8; 32],
    kind: u8,
    digest: [u8; 32],
}
pub(in crate::workspace) struct Observations {
    pub reports: Vec<DeliveryObservation>,
    facts: Vec<Fact>,
}
pub(super) struct Protected {
    pub report: DeliveryObservation,
    scope: DeliveryScope,
    target: [u8; 32],
    protected: Vec<u8>,
}
pub(super) async fn protect(
    report: DeliveryObservation,
    crypto: &WorkspaceCrypto,
) -> Result<Protected, OperationError> {
    let wire = Wire {
        version: 1,
        work: WorkRef::new(report.work()),
        request: report.request_id().as_str().into(),
        event: report.event_id(),
        envelope_digest: *report.envelope_digest(),
        target: report.target_receipt().clone(),
        kind: report.kind().code(),
        observed_at: report.observed_at_unix_ms(),
    };
    let plain = Zeroizing::new(serde_json::to_vec(&wire).map_err(bad)?);
    if plain.len() > 16 * 1024 {
        return Err(OperationError::InvalidEvidence);
    }
    let scope = codec::scope(
        report.work(),
        codec::request_key(report.request_id().as_str()),
    );
    let target = target_key(&report);
    let protected = crypto
        .seal_delivery(
            context(scope, target, report.kind().code()),
            DeliveryPayload::Observation,
            0,
            &plain,
        )
        .await?;
    Ok(Protected {
        report,
        scope,
        target,
        protected,
    })
}
impl Protected {
    pub(super) async fn insert(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        let s = self.scope;
        sqlx::query("INSERT INTO chat_delivery_observations(workspace,operation_id,recipient,request_key,target_key,kind,protected) VALUES(?,?,?,?,?,?,?)")
            .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).bind(self.target.as_slice()).bind(self.report.kind().code() as i64).bind(&self.protected).execute(connection).await.map_err(|_|OperationError::StorageFailure)?;
        Ok(())
    }
}
impl Observations {
    pub(in crate::workspace) async fn read(
        connection: &mut SqliteConnection,
        graph: &Graph,
        selected: &super::super::AttachmentInput,
        operation: radroots_domain::OperationId,
        crypto: &WorkspaceCrypto,
    ) -> Result<Self, OperationError> {
        // Bound keys before fetching or opening any protected observation body.
        let rows=sqlx::query("SELECT CASE WHEN length(recipient)=32 THEN recipient END,CASE WHEN length(request_key)=32 THEN request_key END,CASE WHEN length(target_key)=32 THEN target_key END,kind FROM chat_delivery_observations WHERE workspace=? AND operation_id=? ORDER BY recipient,request_key,target_key,kind LIMIT 2305")
            .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_all(&mut *connection).await.map_err(bad)?;
        if rows.len() > MAX_OBSERVATIONS {
            return Err(OperationError::RecoveryRequired);
        }
        let mut result = Self {
            reports: Vec::with_capacity(rows.len()),
            facts: Vec::with_capacity(rows.len()),
        };
        for row in rows {
            let key = |index| -> Result<[u8; 32], OperationError> {
                row.try_get::<Option<Vec<u8>>, _>(index)
                    .map_err(bad)?
                    .ok_or(OperationError::RecoveryRequired)?
                    .try_into()
                    .map_err(bad)
            };
            let recipient = key(0)?;
            let request = key(1)?;
            let target = key(2)?;
            let kind: i64 = row.try_get(3).map_err(bad)?;
            if !(1..=6).contains(&kind) {
                return Err(OperationError::RecoveryRequired);
            }
            let attempt = graph
                .attempts
                .iter()
                .find(|attempt| {
                    attempt.work().recipient().as_bytes() == &recipient
                        && codec::request_key(attempt.intent().request_id().as_str()) == request
                })
                .ok_or(OperationError::RecoveryRequired)?;
            let scope = codec::scope(attempt.work(), request);
            let protected = read_body(connection, scope, target, kind as u8).await?;
            let plain = crypto
                .open_delivery(
                    context(scope, target, kind as u8),
                    DeliveryPayload::Observation,
                    0,
                    &protected,
                )
                .await?;
            let wire: Wire = plain.expose_secret(|raw| {
                if !super::json::bounded(raw, 32) {
                    return Err(OperationError::RecoveryRequired);
                }
                serde_json::from_slice(raw).map_err(bad)
            })?;
            let report = decode(wire, attempt, target, kind as u8)?;
            result.reports.push(report);
            result.facts.push(Fact {
                scope,
                target,
                kind: kind as u8,
                digest: Sha256::digest(&protected).into(),
            });
        }
        Ok(result)
    }
    pub(in crate::workspace) async fn unchanged(
        &self,
        connection: &mut SqliteConnection,
        selected: &super::super::AttachmentInput,
        operation: radroots_domain::OperationId,
    ) -> Result<(), OperationError> {
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM (SELECT 1 FROM chat_delivery_observations WHERE workspace=? AND operation_id=? LIMIT 2305)")
            .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_one(&mut *connection).await.map_err(bad)?;
        if count != self.facts.len() as i64 {
            return Err(OperationError::RecoveryRequired);
        }
        for fact in &self.facts {
            let protected = read_body(connection, fact.scope, fact.target, fact.kind).await?;
            if <[u8; 32]>::from(Sha256::digest(&protected)) != fact.digest {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(())
    }
}
fn decode(
    wire: Wire,
    attempt: &DeliveryAttempt,
    target: [u8; 32],
    kind: u8,
) -> Result<DeliveryObservation, OperationError> {
    if wire.version != 1
        || wire.work != WorkRef::new(attempt.work())
        || wire.request != attempt.intent().request_id().as_str()
        || wire.event != attempt.intent().event_id()
        || wire.envelope_digest != *attempt.envelope_digest()
        || wire.kind != kind
    {
        return Err(OperationError::RecoveryRequired);
    }
    let report = DeliveryObservation::from_store_report(attempt, &wire.target, wire.observed_at)
        .map_err(bad)?;
    if report.kind().code() != kind || target_key(&report) != target {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(report)
}
async fn read_body(
    connection: &mut SqliteConnection,
    s: DeliveryScope,
    target: [u8; 32],
    kind: u8,
) -> Result<Vec<u8>, OperationError> {
    sqlx::query_scalar::<_,Option<Vec<u8>>>("SELECT CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM chat_delivery_observations WHERE workspace=? AND operation_id=? AND recipient=? AND request_key=? AND target_key=? AND kind=?")
        .bind(s.workspace.as_bytes().as_slice()).bind(s.operation.as_bytes().as_slice()).bind(s.recipient.as_bytes().as_slice()).bind(s.request.as_slice()).bind(target.as_slice()).bind(kind as i64).fetch_optional(connection).await.map_err(bad)?.flatten().ok_or(OperationError::RecoveryRequired)
}

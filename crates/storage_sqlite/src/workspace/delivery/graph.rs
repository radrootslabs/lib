use super::super::{
    AttachmentInput, crypto::WorkspaceCrypto, installation::Records, snapshot::RetainedGraph,
};
use super::{
    MAX_ATTEMPTS,
    codec::{self, Fact},
};
use radroots_domain::{OperationId, PublicKey};
use radroots_storage::command::{
    BoundSigningPlan, DeliveryAttempt, DeliveryIntent, OperationError, PublicationArtifact,
    SignedDmRelaySource, SigningTarget,
};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;

pub(in crate::workspace) struct Graph {
    pub attempts: Vec<DeliveryAttempt>,
    facts: Vec<Fact>,
    parts: usize,
    counters: BTreeMap<(Option<PublicKey>, String), BTreeSet<u8>>,
    routes: BTreeMap<Option<PublicKey>, BTreeSet<String>>,
}
fn bad<E>(_: E) -> OperationError {
    OperationError::RecoveryRequired
}
fn keys(row: &sqlx::sqlite::SqliteRow, index: usize) -> Result<[u8; 32], OperationError> {
    row.try_get::<Option<Vec<u8>>, _>(index)
        .map_err(bad)?
        .ok_or(OperationError::RecoveryRequired)?
        .try_into()
        .map_err(bad)
}

fn allowed_routes(
    bound: &BoundSigningPlan,
    target: SigningTarget,
) -> Result<BTreeMap<Option<PublicKey>, BTreeSet<String>>, OperationError> {
    let recipient = target.recipient();
    if target == SigningTarget::PublicEvent {
        let plan = bound
            .plan()
            .listing()
            .ok_or(OperationError::UnverifiedApproval)?;
        return Ok(BTreeMap::from([(
            None,
            plan.selected_targets()
                .iter()
                .map(|v| v.as_ref().to_owned())
                .collect(),
        )]));
    }
    let mut result = BTreeMap::new();
    for route in bound
        .chat_plan()?
        .routes()
        .iter()
        .filter(|route| Some(route.participant()) == recipient)
    {
        let source = SignedDmRelaySource::verify(route.raw_json())?;
        source.validate_capture(route)?;
        let selected = route
            .selected_relays()
            .iter()
            .map(|target| target.as_ref().to_owned())
            .collect::<BTreeSet<_>>();
        if selected.is_empty() || result.insert(Some(route.participant()), selected).is_some() {
            return Err(OperationError::InvalidCommand);
        }
    }
    Ok(result)
}
impl Graph {
    pub(in crate::workspace) async fn read(
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        operation: OperationId,
        original: &RetainedGraph,
        installed: &Records,
        public: &super::super::listing_installation::Records,
        crypto: &WorkspaceCrypto,
    ) -> Result<Self, OperationError> {
        let rows=sqlx::query(super::sql(connection,"SELECT CASE WHEN length(recipient)=32 OR (length(recipient)=1 AND hex(recipient)='00') THEN recipient END,CASE WHEN length(request_key)=32 THEN request_key END FROM chat_delivery_attempts WHERE workspace=? AND operation_id=? ORDER BY recipient,request_key LIMIT 385").await?)
            .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_all(&mut *connection).await.map_err(bad)?;
        if rows.len() > MAX_ATTEMPTS {
            return Err(OperationError::RecoveryRequired);
        }
        let routes = BTreeMap::new();
        let mut result = Self {
            attempts: Vec::with_capacity(rows.len()),
            facts: Vec::with_capacity(rows.len()),
            parts: 0,
            counters: BTreeMap::new(),
            routes,
        };
        let mut target_count = 0usize;
        let mut representations = BTreeMap::new();
        let mut policies = BTreeMap::new();
        for row in rows {
            let key_bytes = row
                .try_get::<Option<Vec<u8>>, _>(0)
                .map_err(bad)?
                .ok_or(OperationError::RecoveryRequired)?;
            let target_scope = SigningTarget::from_key(&key_bytes).map_err(bad)?;
            let recipient = target_scope.recipient();
            let key = keys(&row, 1)?;
            if !result.routes.contains_key(&recipient) {
                result
                    .authorize_routes(
                        original
                            .bound_plan()
                            .ok_or(OperationError::RecoveryRequired)?,
                        target_scope,
                    )
                    .map_err(|_| OperationError::RecoveryRequired)?;
            }
            let work = match target_scope {
                SigningTarget::BusinessSeal(peer) => PublicationArtifact::Private(
                    installed
                        .work
                        .iter()
                        .find(|v| v.report.recipient() == peer)
                        .ok_or(OperationError::RecoveryRequired)?
                        .report,
                ),
                SigningTarget::PublicEvent => PublicationArtifact::Public(
                    public.work().ok_or(OperationError::RecoveryRequired)?,
                ),
            };
            let value = codec::read(connection, work, key, crypto)
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
            if target_scope == SigningTarget::PublicEvent
                && original
                    .obtained_artifact(target_scope)
                    .is_none_or(|source| {
                        source.raw_json() != value.report.raw_json()
                            || source.digest() != work.evidence().digest()
                    })
            {
                return Err(OperationError::RecoveryRequired);
            }

            if policies
                .insert(
                    recipient,
                    *value.report.intent().publication_policy_digest(),
                )
                .is_some_and(|original| {
                    original != *value.report.intent().publication_policy_digest()
                })
            {
                return Err(OperationError::RecoveryRequired);
            }
            if representations
                .insert(
                    value.report.intent().event_id(),
                    *value.report.envelope_digest(),
                )
                .is_some_and(|digest| digest != *value.report.envelope_digest())
            {
                return Err(OperationError::RecoveryRequired);
            }
            target_count = target_count
                .checked_add(value.report.intent().selection().len())
                .filter(|n| *n <= MAX_ATTEMPTS)
                .ok_or(OperationError::RecoveryRequired)?;
            result
                .validate_routes(target_scope, value.report.intent())
                .map_err(|_| OperationError::RecoveryRequired)?;
            for (target, ordinal) in value
                .report
                .intent()
                .selected_targets()
                .zip(value.report.target_ordinals())
            {
                let slots = result
                    .counters
                    .entry((recipient, target.uri().as_str().to_owned()))
                    .or_default();
                if !slots.insert(*ordinal) {
                    return Err(OperationError::RecoveryRequired);
                }
            }
            result.parts += value.parts.len();
            result.facts.push(value.fact());
            result.attempts.push(value.report);
        }
        for slots in result.counters.values() {
            if slots.iter().copied().ne(1..=slots.len() as u8) {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(result)
    }
    pub(super) fn validate_routes(
        &self,
        target: SigningTarget,
        intent: &DeliveryIntent,
    ) -> Result<(), OperationError> {
        let routes = self
            .routes
            .get(&target.recipient())
            .ok_or(OperationError::UnverifiedApproval)?;
        if intent.targets().targets().iter().any(|target| {
            !routes
                .iter()
                .any(|raw| matches!(target.matches_nostr_relay(raw), Ok(true)))
        }) {
            return Err(OperationError::UnverifiedApproval);
        }
        Ok(())
    }
    pub(super) fn authorize_routes(
        &mut self,
        bound: &BoundSigningPlan,
        target: SigningTarget,
    ) -> Result<(), OperationError> {
        let routes = allowed_routes(bound, target)?;
        if !routes.contains_key(&target.recipient()) {
            return Err(OperationError::UnverifiedApproval);
        }
        self.routes.extend(routes);
        Ok(())
    }
    pub(super) fn ordinals(
        &self,
        target: SigningTarget,
        intent: &DeliveryIntent,
    ) -> Result<Vec<u8>, OperationError> {
        self.validate_routes(target, intent)?;
        let target_scope = target.recipient();
        intent
            .selected_targets()
            .map(|target| {
                let count = self
                    .counters
                    .get(&(target_scope, target.uri().as_str().to_owned()))
                    .map_or(0, BTreeSet::len);
                if count >= 12 {
                    return Err(OperationError::DeliveryAttemptLimit);
                }
                Ok(count as u8 + 1)
            })
            .collect()
    }
    pub(in crate::workspace) async fn unchanged(
        &self,
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        operation: OperationId,
    ) -> Result<(), OperationError> {
        let counts:(i64,i64)=sqlx::query_as(super::sql(connection,"SELECT (SELECT count(*) FROM (SELECT 1 FROM chat_delivery_attempts WHERE workspace=?1 AND operation_id=?2 LIMIT 385)),(SELECT count(*) FROM (SELECT 1 FROM chat_delivery_parts WHERE workspace=?1 AND operation_id=?2 LIMIT 9217))").await?)
            .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_one(&mut *connection).await.map_err(bad)?;
        if counts != (self.facts.len() as i64, self.parts as i64) {
            return Err(OperationError::RecoveryRequired);
        }
        for fact in &self.facts {
            fact.unchanged(connection).await?;
        }
        Ok(())
    }
}

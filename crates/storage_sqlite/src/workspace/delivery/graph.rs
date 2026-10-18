use super::super::{
    AttachmentInput, crypto::WorkspaceCrypto, installation::Records, snapshot::RetainedGraph,
};
use super::{
    MAX_ATTEMPTS,
    codec::{self, Fact},
};
use radroots_domain::{OperationId, PublicKey};
use radroots_event::{
    SignedEvent,
    wire::{EventWireLimits, Nip01EventWire},
};
use radroots_event_codec::verify::{self, Nip01SignatureVerifier, RawEvent};
use radroots_storage::command::{
    BoundSigningPlan, DeliveryAttempt, DeliveryIntent, OperationError,
};
use radroots_transport::target::EndpointUri;
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;

pub(in crate::workspace) struct Graph {
    pub attempts: Vec<DeliveryAttempt>,
    facts: Vec<Fact>,
    parts: usize,
    counters: BTreeMap<(PublicKey, String), BTreeSet<u8>>,
    routes: BTreeMap<PublicKey, BTreeSet<String>>,
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
    recipient: PublicKey,
) -> Result<BTreeMap<PublicKey, BTreeSet<String>>, OperationError> {
    let mut result = BTreeMap::new();
    for route in bound
        .plan()
        .routes()
        .iter()
        .filter(|route| route.participant() == recipient)
    {
        let limits = EventWireLimits {
            max_raw_json_bytes: 65536,
            max_tag_element_bytes: 32768,
            max_total_tag_bytes: 65536,
            max_extra_fields: 0,
            max_total_extra_json_bytes: 0,
            ..Default::default()
        };
        let wire = Nip01EventWire::parse_json_exact_with_limits(route.raw_json(), limits)
            .map_err(|_| OperationError::InvalidEvidence)?;
        if wire.kind != 10050
            || wire.pubkey != route.participant().to_hex()
            || wire.id != route.source_id().to_string()
            || wire.created_at == 0
            || wire.created_at > 253_402_300_799
        {
            return Err(OperationError::InvalidEvidence);
        }
        for tag in &wire.tags {
            if tag.first().is_some_and(|v| v == "relay") && (tag.len() != 2 || tag[1].len() > 2048)
            {
                return Err(OperationError::InvalidEvidence);
            }
        }
        let mut selected = BTreeSet::new();
        for target in route.selected_relays() {
            if !wire
                .tags
                .iter()
                .any(|tag| tag.len() == 2 && tag[0] == "relay" && tag[1] == target.as_ref())
            {
                return Err(OperationError::UnverifiedApproval);
            }
            let endpoint =
                EndpointUri::parse(target.as_ref()).map_err(|_| OperationError::InvalidCommand)?;
            if endpoint.as_str().len() > 2048 {
                return Err(OperationError::InvalidCommand);
            }
            selected.insert(target.as_ref().to_owned());
        }
        let event = SignedEvent::from_wire_verified_id(wire, route.raw_json())
            .map_err(|_| OperationError::InvalidEvidence)?;
        let checked = verify::id(RawEvent::new(event.envelope().clone()))
            .map_err(|_| OperationError::InvalidEvidence)?;
        verify::signature(checked, &Nip01SignatureVerifier)
            .map_err(|_| OperationError::InvalidEvidence)?;
        if selected.is_empty() || result.insert(route.participant(), selected).is_some() {
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
        crypto: &WorkspaceCrypto,
    ) -> Result<Self, OperationError> {
        let rows=sqlx::query("SELECT CASE WHEN length(recipient)=32 THEN recipient END,CASE WHEN length(request_key)=32 THEN request_key END FROM chat_delivery_attempts WHERE workspace=? AND operation_id=? ORDER BY recipient,request_key LIMIT 385")
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
            let recipient = PublicKey::from_bytes(keys(&row, 0)?).map_err(bad)?;
            let key = keys(&row, 1)?;
            if !result.routes.contains_key(&recipient) {
                result
                    .authorize_routes(
                        original
                            .bound_plan()
                            .ok_or(OperationError::RecoveryRequired)?,
                        recipient,
                    )
                    .map_err(|_| OperationError::RecoveryRequired)?;
            }
            let work = installed
                .work
                .iter()
                .find(|work| work.report.evidence().recipient() == recipient)
                .ok_or(OperationError::RecoveryRequired)?
                .report;
            let value = codec::read(connection, work, key, crypto)
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
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
                .validate_routes(recipient, value.report.intent())
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
        recipient: PublicKey,
        intent: &DeliveryIntent,
    ) -> Result<(), OperationError> {
        let routes = self
            .routes
            .get(&recipient)
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
        recipient: PublicKey,
    ) -> Result<(), OperationError> {
        let routes = allowed_routes(bound, recipient)?;
        if !routes.contains_key(&recipient) {
            return Err(OperationError::UnverifiedApproval);
        }
        self.routes.extend(routes);
        Ok(())
    }
    pub(super) fn ordinals(
        &self,
        recipient: PublicKey,
        intent: &DeliveryIntent,
    ) -> Result<Vec<u8>, OperationError> {
        self.validate_routes(recipient, intent)?;
        intent
            .selected_targets()
            .map(|target| {
                let count = self
                    .counters
                    .get(&(recipient, target.uri().as_str().to_owned()))
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
        let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM (SELECT 1 FROM chat_delivery_attempts WHERE workspace=?1 AND operation_id=?2 LIMIT 385)),(SELECT count(*) FROM (SELECT 1 FROM chat_delivery_parts WHERE workspace=?1 AND operation_id=?2 LIMIT 9217))")
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

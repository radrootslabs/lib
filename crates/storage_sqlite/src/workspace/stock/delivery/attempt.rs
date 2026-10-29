//! Original potential transport invocation, independently bound to stock sources.
use super::*;
use serde::{Deserialize, Serialize};
use wire::Reference;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Wire {
    version: u8,
    envelope: Reference,
    plan: Reference,
    consent: Reference,
    request: String,
    event: EnvelopeEventId,
    intent: [u8; 32],
    #[serde(deserialize_with = "wire::targets")]
    targets: TargetSet,
    #[serde(deserialize_with = "wire::small_vec")]
    selection: Vec<TargetFingerprint>,
    #[serde(deserialize_with = "wire::policy")]
    policy: SatisfactionPolicy,
    deadline: u64,
    at: UnixSeconds,
    #[serde(deserialize_with = "wire::small_vec")]
    ordinals: Vec<u8>,
}
impl Wire {
    pub(super) fn new(
        envelope: &envelope::Retained,
        auth: &signing_plan::Authorized,
        intent: &DeliveryIntent,
        at: UnixSeconds,
        ordinals: Vec<u8>,
    ) -> Result<Self, OperationError> {
        let plan = auth.plan.as_ref().ok_or(OperationError::MissingHistory)?;
        validate_route(
            plan.bound.plan(),
            envelope.source.receipt().recipient(),
            intent,
        )?;
        Ok(Self {
            version: 1,
            envelope: Reference::new(&envelope.frame.header),
            plan: Reference::new(&plan.frame.header),
            consent: Reference::new(&auth.consent.frame.header),
            request: intent.request_id().as_str().into(),
            event: intent.event_id(),
            intent: *intent.digest(),
            targets: intent.targets().clone(),
            selection: intent.selection().to_vec(),
            policy: intent.policy().clone(),
            deadline: intent.deadline_unix_ms().get(),
            at,
            ordinals,
        })
    }
    fn intent(&self) -> Result<DeliveryIntent, OperationError> {
        let selected = TargetSet::new(
            self.targets
                .targets()
                .iter()
                .filter(|target| self.selection.contains(target.fingerprint()))
                .cloned()
                .collect(),
        )
        .map_err(|_| OperationError::RecoveryRequired)?;
        let value = DeliveryIntent::new(
            &self.request,
            self.event,
            &self.targets,
            &self.policy,
            NonZeroU64::new(self.deadline).ok_or(OperationError::RecoveryRequired)?,
        )
        .and_then(|intent| intent.with_selection(&selected))
        .map_err(|_| OperationError::RecoveryRequired)?;
        if value.selection() != self.selection || value.digest() != &self.intent {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(value)
    }
}
pub(super) fn validate_route(
    plan: &radroots_protocol::commerce::CommerceSigningPlan,
    recipient: PublicKey,
    intent: &DeliveryIntent,
) -> Result<(), OperationError> {
    let route = plan
        .routes()
        .iter()
        .find(|route| route.participant() == recipient)
        .ok_or(OperationError::RecoveryRequired)?;
    SignedDmRelaySource::verify(route.raw_json())?.validate_capture(route)?;
    if intent.targets().targets().iter().any(|target| {
        !route
            .selected_relays()
            .iter()
            .any(|uri| target.matches_nostr_relay(uri).is_ok_and(|same| same))
    }) {
        return Err(OperationError::UnverifiedApproval);
    }
    Ok(())
}
pub(in crate::workspace::stock) struct Retained {
    pub frame: ledger::Retained,
    pub report: StockDeliveryAttempt,
    pub projection: Projection,
}
impl Retained {
    pub(super) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        confirm_record(connection, &self.frame, self.projection).await
    }
    pub(in crate::workspace::stock) fn into_history(self) -> ledger::HistoryEntry {
        history(self.frame, self.projection)
    }
}
pub(in crate::workspace::stock) async fn read_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    administrator: GrantId,
    entry: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let Some(frame) = Box::pin(ledger::read(connection, input, crypto, entry)).await? else {
        return Ok(None);
    };
    if frame.header.family != ledger::Family::DeliveryAttempt {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(
        Box::pin(decode_entry(
            connection,
            input,
            crypto,
            binding,
            administrator,
            frame,
        ))
        .await?,
    ))
}
async fn decode_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    administrator: GrantId,
    frame: ledger::Retained,
) -> Result<Retained, OperationError> {
    let value: Wire = wire::decode(&frame.body, ATTEMPT_MAX)?;
    if value.version != 1 || wire::head(&frame.header, &frame.body) != frame.header.head {
        return Err(OperationError::RecoveryRequired);
    }
    let outer = Box::pin(envelope::read_entry(
        connection,
        input,
        crypto,
        binding,
        value.envelope.entry,
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    value.envelope.confirm(&outer.frame.header)?;
    let plan = Box::pin(signing_plan::read_entry(
        connection,
        input,
        crypto,
        value.plan.entry,
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    value.plan.confirm(&plan.frame.header)?;
    let consent = Box::pin(super::super::consent::read(
        connection,
        input,
        crypto,
        administrator,
        value.consent.entry,
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    value.consent.confirm(&consent.frame.header)?;
    let installed = Box::pin(inventory::installation::read_entry(
        connection,
        input,
        crypto,
        outer.installation_entry(),
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let source = outer.source.receipt();
    let original = source.attempt();
    let h = &frame.header;
    let command = consent.command.context();
    if h.related_operation != Some(original.operation_id())
        || h.digest != original.command_digest()
        || h.actor.participant() != outer.frame.header.actor.participant()
        || h.actor.grant_id() != Some(consent.subject)
        || h.order != outer.frame.header.order
        || h.order != Some(installed.receipt.order_id())
        || h.revision <= outer.frame.header.revision
        || h.revision <= plan.frame.header.revision
        || h.revision <= consent.frame.header.revision
        || plan.bound.attempt().operation_id() != original.operation_id()
        || plan.bound.attempt().command_digest() != h.digest
        || (outer.frame.header.family == ledger::Family::Envelope
            && plan.frame.header.actor != outer.frame.header.actor)
        || plan.frame.header.actor.participant() != outer.frame.header.actor.participant()
        || command.operation_id() != original.operation_id()
        || consent.command.digest() != h.digest
        || value.at >= consent.expires
        || value.at < consent.observed_at
        || value.at < installed.receipt.installed_at()
    {
        return Err(OperationError::RecoveryRequired);
    }
    let intent = value.intent()?;
    validate_route(plan.bound.plan(), source.recipient(), &intent)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let projection = Projection::Attempt {
        operation: original.operation_id(),
        recipient: source.recipient(),
        request: request_key(intent.request_id()),
    };
    let report = StockDeliveryAttempt::from_store_report(
        Arc::new(outer.source),
        intent,
        h.transaction,
        value.at,
        InventoryCheckpointRef::from_host_metadata(input.authority(), h.revision, Some(h.head))?,
        value.ordinals,
    )?;
    let retained = Retained {
        frame,
        report,
        projection,
    };
    retained.confirm(connection).await?;
    Ok(retained)
}

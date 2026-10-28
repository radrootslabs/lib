//! Original per-target classes, tied to one actual protected potential invocation.
use super::*;
use serde::{Deserialize, Serialize};
use wire::Reference;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Wire {
    version: u8,
    attempt: Reference,
    target: DeliveryTargetReceipt,
    kind: u8,
    #[serde(deserialize_with = "super::super::required_nullable")]
    observed: Option<NonZeroU64>,
}
impl Wire {
    pub(super) fn new(attempt: &attempt::Retained, observation: &StockDeliveryObservation) -> Self {
        Self {
            version: 1,
            attempt: Reference::new(&attempt.frame.header),
            target: observation.target_receipt().clone(),
            kind: observation.kind().code(),
            observed: observation.observed_at_unix_ms(),
        }
    }
}
pub(in crate::workspace::stock) struct Retained {
    pub frame: ledger::Retained,
    pub receipt: StockDeliveryObservationReceipt,
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
    if frame.header.family != ledger::Family::DeliveryObservation {
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
    let value: Wire = wire::decode(&frame.body, OBSERVATION_MAX)?;
    if value.version != 1 || wire::head(&frame.header, &frame.body) != frame.header.head {
        return Err(OperationError::RecoveryRequired);
    }
    let attempt = Box::pin(attempt::read_entry(
        connection,
        input,
        crypto,
        binding,
        administrator,
        value.attempt.entry,
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    value.attempt.confirm(&attempt.frame.header)?;
    let h = &frame.header;
    let original = &attempt.frame.header;
    if h.actor != original.actor
        || h.digest != original.digest
        || h.order != original.order
        || h.related_operation != original.related_operation
        || h.revision <= original.revision
    {
        return Err(OperationError::RecoveryRequired);
    }
    let observation =
        StockDeliveryObservation::from_store_report(&attempt.report, &value.target, value.observed)
            .map_err(|_| OperationError::RecoveryRequired)?;
    if observation.kind().code() != value.kind {
        return Err(OperationError::RecoveryRequired);
    }
    let source = observation.source();
    let projection = Projection::Observation {
        operation: source.attempt().operation_id(),
        recipient: source.recipient(),
        request: request_key(observation.request_id()),
        target: target_key(observation.target_receipt().target().fingerprint()),
        kind: observation.kind().code(),
    };
    let receipt = StockDeliveryObservationReceipt::from_store_report(
        observation,
        h.transaction,
        InventoryCheckpointRef::from_host_metadata(input.authority(), h.revision, Some(h.head))?,
    )?;
    let retained = Retained {
        frame,
        receipt,
        projection,
    };
    retained.confirm(connection).await?;
    Ok(retained)
}

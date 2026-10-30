//! Stock-owned protected dispatch history. Transport stays outside transactions.
use super::*;
use inventory::{Turn, checkpoint};
use radroots_domain::{
    ActorRef, CheckpointDigest, CommandDigest, EnvelopeEventId, OperationId, OrderId, PublicKey,
    UnixSeconds,
};
use radroots_storage::{
    command::{DeliveryIntent, DeliveryObservationKind, OperationError, SignedDmRelaySource},
    inventory::{
        InventoryCheckpointRef, StockDeliveryAttempt, StockDeliveryAttemptOutcome,
        StockDeliveryObservation, StockDeliveryObservationOutcome, StockDeliveryObservationPage,
        StockDeliveryObservationReceipt, StockDeliveryRequest, StockDeliverySummary,
        StockDeliveryTargetProgress, StockEnvelopeRequest,
    },
};
use radroots_transport::{
    TargetSet,
    policy::SatisfactionPolicy,
    sink::{DeliveryRequestId, DeliveryTargetReceipt},
    target::TargetFingerprint,
};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{num::NonZeroU64, sync::Arc};

pub(in crate::workspace::stock) mod attempt;
mod graph;
pub(in crate::workspace::stock) mod observation;
mod observation_operations;
mod operations;
mod projection;
mod wire;
pub(in crate::workspace) use observation_operations::{capture_observation, page};
pub(in crate::workspace) use operations::{prepare, query_attempt, summary};
pub(in crate::workspace::stock) use projection::{Projection, raw_projection};

const ATTEMPT_MAX: usize = 65536;
const OBSERVATION_MAX: usize = 16384;
const ATTEMPT_COUNT_MAX: usize = 384;
const OBSERVATION_COUNT_MAX: usize = 2304;
const PAGE_MAX: usize = 16;

pub(in crate::workspace::stock) async fn snapshot(
    turn: &mut Turn<'_>,
    request: StockEnvelopeRequest,
    authority: &Authority,
) -> Result<[StockDeliverySummary; 2], OperationError> {
    let graph = Box::pin(graph::Graph::read(turn, &request, authority, None, None)).await?;
    let original = Box::pin(inventory::acceptance::codec::read(
        turn.connection,
        turn.selected,
        turn.crypto,
        request.operation_id(),
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let checkpoint = authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let checkpoint = InventoryCheckpointRef::from_host_metadata(
        turn.selected.authority(),
        checkpoint.revision,
        checkpoint.head,
    )?
    .with_optional_authorization(authority.authorization);
    let reports = original
        .receipt
        .rumor()
        .recipients()
        .each_ref()
        .map(|recipient| {
            let source = StockEnvelopeRequest::new(
                request.binding().clone(),
                request.minimum_checkpoint(),
                request.operation_id(),
                request.command_digest(),
                *recipient,
                None,
            )?;
            graph.summary(&source, checkpoint)
        });
    let [first, second] = reports;
    Ok([first?, second?])
}

fn request_key(request: &DeliveryRequestId) -> [u8; 32] {
    Sha256::digest(request.as_str().as_bytes()).into()
}
fn target_key(target: &TargetFingerprint) -> [u8; 32] {
    Sha256::digest(target.as_str().as_bytes()).into()
}
fn validate_request(request: &DeliveryRequestId) -> Result<(), OperationError> {
    let raw = request.as_str();
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(OperationError::InvalidCommand);
    }
    Ok(())
}
async fn confirm_record(
    connection: &mut SqliteConnection,
    frame: &ledger::Retained,
    projection: Projection,
) -> Result<(), OperationError> {
    if !matches!(
        (frame.header.family, projection),
        (ledger::Family::DeliveryAttempt, Projection::Attempt { .. })
            | (
                ledger::Family::DeliveryObservation,
                Projection::Observation { .. }
            )
    ) {
        return Err(OperationError::RecoveryRequired);
    }
    ledger::confirm_header(connection, &frame.header, &frame.protected).await?;
    ledger::confirm_parts(
        connection,
        frame.header.actor,
        frame.header.operation,
        frame.parts_witness,
        frame.header.parts,
    )
    .await?;
    projection
        .confirm(
            connection,
            frame.header.actor.participant(),
            frame.header.operation,
        )
        .await?;
    let movements: i64 =
        sqlx::query_scalar("SELECT count(*) FROM stock_movements WHERE actor=? AND operation_id=?")
            .bind(frame.header.actor.participant().as_bytes().as_slice())
            .bind(frame.header.operation.as_bytes().as_slice())
            .fetch_one(connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
    if movements != 0 {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
fn history(frame: ledger::Retained, projection: Projection) -> ledger::HistoryEntry {
    ledger::HistoryEntry {
        deficit: None,
        delivery: Some(projection),
        envelope: None,
        aborted: None,
        preparation: None,
        header: frame.header,
        protected: frame.protected,
        parts_witness: frame.parts_witness,
        pools: vec![],
        mapping: None,
        movements: vec![],
        signing: None,
        installation: None,
        local_record: None,
    }
}
struct Append {
    actor: ActorRef,
    original: OperationId,
    digest: CommandDigest,
    order: OrderId,
    family: ledger::Family,
    projection: Projection,
}
struct Prepared {
    frame: ledger::Prepared,
    projection: Projection,
    root: Vec<u8>,
}
impl Prepared {
    async fn new(
        turn: &Turn<'_>,
        authority: &Authority,
        append: Append,
        body: &[u8],
    ) -> Result<Self, OperationError> {
        if !matches!(authority.catalog, 17..=20) {
            return Err(OperationError::UnsupportedOperation);
        }
        let before = authority
            .inventory
            .ok_or(OperationError::RecoveryRequired)?;
        let revision = Sequence::new(
            before
                .revision
                .get()
                .checked_add(1)
                .ok_or(WorkspaceError::Capacity)?,
        )
        .map_err(|_| WorkspaceError::Capacity)?;
        let mut header = ledger::Header {
            version: 5,
            family: append.family,
            namespace: SellerAuthoritySelection::from_attachment(turn.selected),
            actor: append.actor,
            operation: OperationId::from_bytes(super::super::random()?),
            digest: append.digest,
            transaction: TransactionId::from_bytes(super::super::random()?),
            revision,
            previous: before.head,
            head: CheckpointDigest::from_bytes([0; 32]),
            pool: None,
            order: Some(append.order),
            related_operation: Some(append.original),
            subject_grant: None,
            bytes: 0,
            parts: 0,
            body_digest: [0; 32],
        };
        header.head = wire::head(&header, body);
        let frame = Box::pin(ledger::prepare(turn.crypto, turn.selected, header, body)).await?;
        let mut next = authority.clone();
        next.inventory = Some(InventoryCheckpoint {
            revision,
            head: Some(frame.header.head),
            ..before
        });
        let root = Box::pin(turn.crypto.seal_stock_root(
            turn.selected,
            next.catalog,
            &next.encode()?,
        ))
        .await?;
        Ok(Self {
            frame,
            projection: append.projection,
            root,
        })
    }
    fn checkpoint(&self, authority: &Authority) -> Result<InventoryCheckpointRef, OperationError> {
        Ok(InventoryCheckpointRef::from_host_metadata(
            authority.input.authority(),
            self.frame.header.revision,
            Some(self.frame.header.head),
        )?)
    }
    async fn insert(
        &self,
        connection: &mut SqliteConnection,
        original_root: &[u8],
    ) -> Result<(), OperationError> {
        self.frame.insert_record(connection).await?;
        self.projection
            .insert(
                connection,
                self.frame.header.actor.participant(),
                self.frame.header.operation,
            )
            .await?;
        let result =
            sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?")
                .bind(&self.root)
                .bind(original_root)
                .execute(connection)
                .await
                .map_err(|_| OperationError::StorageFailure)?;
        if result.rows_affected() != 1 {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}

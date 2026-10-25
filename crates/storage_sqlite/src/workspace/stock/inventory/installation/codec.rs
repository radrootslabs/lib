//! Independently reconstruct every installed effect from original T0/T1/S.
use super::*;
#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Movement {
    pub version: u8,
    pub operation: OperationId,
    pub digest: CommandDigest,
    pub record: RecordId,
    pub effect: Effect,
}
pub(in crate::workspace::stock) struct Retained {
    pub frame: ledger::Retained,
    pub receipt: StockInstallationReceipt,
    projection: Projection,
    movements: Vec<ledger::MovementWitness>,
}
pub(super) fn head(header: &ledger::Header, body: &[u8]) -> CheckpointDigest {
    let mut h = Sha256::new();
    h.update(b"radroots.seller_stock.installation_chain.v1\0");
    h.update(header.namespace.workspace_id().as_bytes());
    h.update(header.namespace.authority().executor_id().as_bytes());
    h.update(
        header
            .namespace
            .authority()
            .generation()
            .get()
            .to_be_bytes(),
    );
    h.update(header.namespace.route_digest().as_bytes());
    h.update(header.actor.participant().as_bytes());
    h.update(
        header
            .actor
            .grant_id()
            .map(|v| *v.as_bytes())
            .unwrap_or([0; 32]),
    );
    h.update(header.operation.as_bytes());
    h.update(header.digest.as_bytes());
    h.update(header.transaction.as_bytes());
    h.update(header.revision.get().to_be_bytes());
    h.update([u8::from(header.previous.is_some())]);
    h.update(header.previous.map(|v| *v.as_bytes()).unwrap_or([0; 32]));
    h.update(Sha256::digest(body));
    CheckpointDigest::from_bytes(h.finalize().into())
}
pub(super) fn receipt(
    input: &AttachmentInput,
    header: &ledger::Header,
    body: &Body,
    protected: &[u8],
) -> Result<StockInstallationReceipt, OperationError> {
    Ok(StockInstallationReceipt {
        attempt: CommitAttempt::new(
            input.workspace_id(),
            body.operation,
            header.digest,
            Some(header.transaction),
        ),
        entry: EntryIdentity {
            operation: header.operation,
            revision: header.revision,
            head: header.head,
            protected_digest: Sha256::digest(protected).into(),
        },
        order: header.order.ok_or(OperationError::RecoveryRequired)?,
        record: body.record,
        at: body.installed_at,
        checkpoint: radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
            input.authority(),
            header.revision,
            Some(header.head),
        )?
        .with_authorization(body.authorization),
        pools: body.effects.iter().map(|value| value.after).collect(),
        work: body.work.map(|value| StockPublicationWork {
            recipient: value.recipient,
            seal: value.seal,
            evidence: value.evidence.map(|v| v.digest),
        }),
    })
}
pub(in crate::workspace::stock) async fn read_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    operation: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let Some(frame) = ledger::read(connection, input, crypto, operation).await? else {
        return Ok(None);
    };
    if frame.header.family != ledger::Family::Installation {
        return Err(OperationError::RecoveryRequired);
    }
    let body: Body = super::super::super::closed_record(&frame.body)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if body.version != 1
        || body.effects.len() > 64
        || body
            .effects
            .windows(2)
            .any(|p| p[0].before.id() >= p[1].before.id())
        || head(&frame.header, &frame.body) != frame.header.head
    {
        return Err(OperationError::RecoveryRequired);
    }
    let plan = Box::pin(signing_plan::read_entry(
        connection,
        input,
        crypto,
        body.plan_entry,
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let original = Box::pin(super::super::acceptance::codec::read(
        connection,
        input,
        crypto,
        body.operation,
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let header = &frame.header;
    if header.related_operation != Some(body.operation)
        || header.order != Some(original.receipt.order_id())
        || header.actor.participant() != original.frame.header.actor.participant()
        || header.digest != original.frame.header.digest
        || header.revision <= plan.frame.header.revision
        || plan.frame.header.head != body.plan_head
        || plan.bound.plan().operation_id() != body.operation
        || plan.bound.plan().command_digest() != header.digest
        || plan.bound.preparation().rumor().record_id() != body.record
        || body.installed_at < plan.bound.bound_at()
    {
        return Err(OperationError::RecoveryRequired);
    }
    let allocations = original.held_allocations()?;
    if allocations.len() != body.effects.len() {
        return Err(OperationError::RecoveryRequired);
    }
    for ((pool, quantity), effect) in allocations.into_iter().zip(&body.effects) {
        if pool != effect.before.id()
            || quantity != effect.quantity
            || effect.before.version().get() == 0
            || after(effect.before, quantity)? != effect.after
        {
            return Err(OperationError::RecoveryRequired);
        }
    }
    let recipients = plan.bound.preparation().rumor().recipients();
    let mut peer = false;
    for (index, work) in body.work.iter().enumerate() {
        if work.recipient != recipients[index]
            || work.intent != plan.bound.intent_digests()[index]
            || work.seal != plan.bound.plan().seals()[index].event_id()
        {
            return Err(OperationError::RecoveryRequired);
        }
        if let Some(selected) = work.evidence {
            let obtained = Box::pin(selection::open(
                connection,
                input,
                crypto,
                body.operation,
                work.recipient,
                selection::Selected {
                    entry: selected.entry,
                    revision: selected.revision,
                    digest: selected.digest,
                },
            ))
            .await?;
            if obtained.frame.header.revision >= header.revision {
                return Err(OperationError::RecoveryRequired);
            }
            if work.recipient != input.participant() {
                peer = true;
            }
        }
    }
    if !peer {
        return Err(OperationError::RecoveryRequired);
    }
    let mut movements = Vec::with_capacity(body.effects.len());
    for effect in &body.effects {
        let row=sqlx::query("SELECT actor,grant_id,operation_id,pool_id,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_movements WHERE movement_id=?").bind(effect.movement.as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let expected = [
            *header.actor.participant().as_bytes(),
            *header
                .actor
                .grant_id()
                .ok_or(OperationError::RecoveryRequired)?
                .as_bytes(),
            *header.operation.as_bytes(),
            *effect.before.id().as_bytes(),
        ];
        for (index, wanted) in expected.into_iter().enumerate() {
            let actual: Vec<u8> = row
                .try_get(index)
                .map_err(|_| OperationError::RecoveryRequired)?;
            if actual != wanted {
                return Err(OperationError::RecoveryRequired);
            }
        }
        let protected: Vec<u8> = row
            .try_get::<Option<Vec<u8>>, _>(4)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
        let clear = crypto
            .open_stock_record(input, "installation_movement", &effect.movement, &protected)
            .await?;
        let movement: Movement = clear
            .expose_secret(|bytes| super::super::super::closed_record(bytes))
            .map_err(|_| OperationError::RecoveryRequired)?;
        if movement
            != (Movement {
                version: 1,
                operation: header.operation,
                digest: header.digest,
                record: body.record,
                effect: *effect,
            })
        {
            return Err(OperationError::RecoveryRequired);
        }
        movements.push(ledger::MovementWitness {
            id: effect.movement,
            pool: effect.before.id(),
            grant: header
                .actor
                .grant_id()
                .ok_or(OperationError::RecoveryRequired)?,
            protected,
        });
    }
    movements.sort_by_key(|value| value.id);
    if movements.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(OperationError::RecoveryRequired);
    }
    let projection = Projection::new(header, &body)?;
    let receipt = receipt(input, header, &body, &frame.protected)?;
    let retained = Retained {
        frame,
        receipt,
        projection,
        movements,
    };
    retained.confirm(connection).await?;
    Ok(Some(retained))
}
impl Retained {
    pub(in crate::workspace::stock) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        ledger::confirm_header(connection, &self.frame.header, &self.frame.protected).await?;
        ledger::confirm_parts(
            connection,
            self.frame.header.actor,
            self.frame.header.operation,
            self.frame.parts_witness,
            self.frame.header.parts,
        )
        .await?;
        self.projection
            .confirm(
                connection,
                self.frame.header.actor.participant(),
                self.frame.header.operation,
            )
            .await?;
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM stock_movements WHERE actor=? AND operation_id=?",
        )
        .bind(self.frame.header.actor.participant().as_bytes().as_slice())
        .bind(self.frame.header.operation.as_bytes().as_slice())
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
        if count != self.movements.len() as i64 {
            return Err(OperationError::RecoveryRequired);
        }
        for movement in &self.movements {
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_movements WHERE movement_id=? AND actor=? AND grant_id=? AND operation_id=? AND pool_id=? AND protected=?").bind(movement.id.as_slice()).bind(self.frame.header.actor.participant().as_bytes().as_slice()).bind(movement.grant.as_bytes().as_slice()).bind(self.frame.header.operation.as_bytes().as_slice()).bind(movement.pool.as_bytes().as_slice()).bind(&movement.protected).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
            if count != 1 {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(())
    }
    pub(in crate::workspace::stock) fn into_history(self) -> ledger::HistoryEntry {
        ledger::HistoryEntry {
            local_record: None,
            header: self.frame.header,
            protected: self.frame.protected,
            parts_witness: self.frame.parts_witness,
            pools: self.receipt.pools,
            mapping: None,
            movements: self.movements,
            signing: None,
            installation: Some(self.projection),
        }
    }
}

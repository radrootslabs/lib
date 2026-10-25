use super::*;
pub(in crate::workspace::stock) struct Retained {
    pub frame: ledger::Retained,
    pub report: StockSigningAttempt,
    projection: Projection,
}
pub(super) fn head(header: &ledger::Header, body: &[u8]) -> radroots_domain::CheckpointDigest {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.signing_claim_chain.v1\0");
    hash.update(header.namespace.workspace_id().as_bytes());
    hash.update(header.namespace.authority().executor_id().as_bytes());
    hash.update(
        header
            .namespace
            .authority()
            .generation()
            .get()
            .to_be_bytes(),
    );
    hash.update(header.namespace.route_digest().as_bytes());
    hash.update(header.actor.participant().as_bytes());
    hash.update(
        header
            .actor
            .grant_id()
            .map(|value| *value.as_bytes())
            .unwrap_or([0; 32]),
    );
    hash.update(header.operation.as_bytes());
    hash.update(header.digest.as_bytes());
    hash.update(header.transaction.as_bytes());
    hash.update(header.revision.get().to_be_bytes());
    hash.update([u8::from(header.previous.is_some())]);
    hash.update(
        header
            .previous
            .map(|value| *value.as_bytes())
            .unwrap_or([0; 32]),
    );
    hash.update(Sha256::digest(body));
    radroots_domain::CheckpointDigest::from_bytes(hash.finalize().into())
}
pub(in crate::workspace::stock) async fn read_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    entry: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let Some(frame) = ledger::read(connection, input, crypto, entry).await? else {
        return Ok(None);
    };
    if frame.header.family != ledger::Family::SigningClaim || frame.body.len() > 16 * 1024 {
        return Err(OperationError::RecoveryRequired);
    }
    let body: Body = super::super::super::closed_record(&frame.body)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let plan = signing_plan::read_entry(connection, input, crypto, body.plan_entry)
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
    let selected = intent(&plan.bound, body.recipient, CancellationSignal::new())?;
    let header = &frame.header;
    if body.version != 1
        || header.related_operation != Some(body.operation)
        || header.digest != plan.bound.plan().command_digest()
        || body.operation != plan.bound.plan().operation_id()
        || header.actor != plan.frame.header.actor
        || header.order != plan.frame.header.order
        || header.revision <= plan.frame.header.revision
        || body.plan_transaction != plan.frame.header.transaction
        || body.plan_head != plan.frame.header.head
        || body.plan_digest != *plan.bound.plan().digest()
        || body.intent != *selected.binding().digest()
        || body.deadline != plan.bound.preparation().first_issue_deadline()
        || body.claimed_at < plan.bound.bound_at()
        || body.claimed_at >= body.deadline
        || head(header, &frame.body) != header.head
    {
        return Err(OperationError::RecoveryRequired);
    }
    let projection = Projection {
        operation: body.operation,
        recipient: body.recipient,
        evidence: None,
    };
    let report = StockSigningAttempt {
        bound: Arc::new(plan.bound),
        recipient: body.recipient,
        entry: EntryIdentity {
            operation: header.operation,
            revision: header.revision,
            head: header.head,
            protected_digest: Sha256::digest(&frame.protected).into(),
        },
        transaction: header.transaction,
        claimed_at: body.claimed_at,
        binding: selected.binding(),
    };
    let retained = Retained {
        frame,
        report,
        projection,
    };
    retained.confirm(connection).await?;
    Ok(Some(retained))
}
impl Retained {
    pub(in crate::workspace::stock) fn identity(&self) -> EntryIdentity {
        self.report.entry
    }
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
        .fetch_one(connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
        if count != 0 {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
    pub(in crate::workspace::stock) fn into_history(self) -> ledger::HistoryEntry {
        ledger::HistoryEntry {
            header: self.frame.header,
            protected: self.frame.protected,
            parts_witness: self.frame.parts_witness,
            pools: vec![],
            mapping: None,
            movements: vec![],
            signing: Some(self.projection),
        }
    }
}

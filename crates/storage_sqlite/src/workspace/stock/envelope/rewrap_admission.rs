//! Confirmed one-invocation wrapping admission, separate from obtained bytes.
use super::*;
use radroots_domain::UnixSeconds;
use radroots_storage::inventory::{StockRewrapAdmission, StockRewrapOutcome, StockRewrapRequest};
const MAGIC: &[u8; 8] = b"RDSTRP1\0";
const BYTES: usize = 248;
pub(super) struct Admitted {
    pub frame: ledger::Retained,
    pub report: StockRewrapAdmission,
    pub original_entry: OperationId,
    projection: Projection,
}
fn head(header: &ledger::Header, body: &[u8]) -> CheckpointDigest {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.rewrap_admission_chain.v1\0");
    hash.update([13]);
    hash.update(codec::head(header, body).as_bytes());
    CheckpointDigest::from_bytes(hash.finalize().into())
}
fn reference(body: &mut Vec<u8>, header: &ledger::Header) {
    body.extend_from_slice(header.operation.as_bytes());
    body.extend_from_slice(header.transaction.as_bytes());
    body.extend_from_slice(header.head.as_bytes());
}
fn encode(
    id: OperationId,
    original: &Retained,
    consent: &super::super::consent::Retained,
    at: UnixSeconds,
    expires: UnixSeconds,
) -> Zeroizing<Vec<u8>> {
    let mut body = Zeroizing::new(Vec::with_capacity(BYTES));
    body.extend_from_slice(MAGIC);
    body.extend_from_slice(id.as_bytes());
    reference(&mut body, &original.frame.header);
    reference(&mut body, &consent.frame.header);
    body.extend_from_slice(&at.get().to_be_bytes());
    body.extend_from_slice(&expires.get().to_be_bytes());
    body
}
pub(super) async fn lookup(
    turn: &mut Turn<'_>,
    request: &StockRewrapRequest,
) -> Result<Option<Admitted>, OperationError> {
    let source = request.source();
    let entry: Option<Vec<u8>> = sqlx::query_scalar("SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_rewrap_admissions WHERE actor=? AND operation_id=? AND recipient=? AND request_id=?")
        .bind(turn.selected.participant().as_bytes().as_slice()).bind(source.operation_id().as_bytes().as_slice())
        .bind(source.recipient().as_bytes().as_slice()).bind(request.rewrap_id().as_bytes().as_slice())
        .fetch_optional(&mut *turn.connection).await.map_err(|_| OperationError::RecoveryRequired)?;
    let Some(entry) = entry else { return Ok(None) };
    let retained = Box::pin(read_entry(
        turn.connection,
        turn.selected,
        turn.crypto,
        source.binding(),
        OperationId::from_bytes(
            entry
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        ),
    ))
    .await?;
    let original = retained.report.original();
    if retained.report.rewrap_id() != request.rewrap_id()
        || original.attempt().operation_id() != source.operation_id()
        || original.attempt().command_digest() != source.command_digest()
        || original.recipient() != source.recipient()
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(retained))
}
pub(super) async fn read_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    entry: OperationId,
) -> Result<Admitted, OperationError> {
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    let frame = Box::pin(ledger::read(connection, input, crypto, entry))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
    let (authority, _, _, _) = super::super::read(connection, input, crypto).await?;
    let body = frame.body.as_slice();
    if catalog != 18
        || frame.header.family != ledger::Family::RewrapAdmission
        || body.len() != BYTES
        || &body[..8] != MAGIC
    {
        return Err(OperationError::RecoveryRequired);
    }
    let array = |start, end| -> Result<[u8; 32], OperationError> {
        body[start..end]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)
    };
    let id = OperationId::from_bytes(array(8, 40)?);
    let original = Box::pin(codec::read_original_entry(
        connection,
        input,
        crypto,
        binding,
        OperationId::from_bytes(array(40, 72)?),
    ))
    .await?;
    let consent = Box::pin(super::super::consent::read(
        connection,
        input,
        crypto,
        authority.input.grant_id(),
        OperationId::from_bytes(array(136, 168)?),
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let at = UnixSeconds::new(u64::from_be_bytes(
        body[232..240]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    ))
    .map_err(|_| OperationError::RecoveryRequired)?;
    let expires = UnixSeconds::new(u64::from_be_bytes(
        body[240..248]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    ))
    .map_err(|_| OperationError::RecoveryRequired)?;
    let h = &frame.header;
    let first = &original.frame.header;
    if first.transaction != TransactionId::from_bytes(array(72, 104)?)
        || first.head != CheckpointDigest::from_bytes(array(104, 136)?)
        || consent.frame.header.transaction != TransactionId::from_bytes(array(168, 200)?)
        || consent.frame.header.head != CheckpointDigest::from_bytes(array(200, 232)?)
        || h.actor.participant() != first.actor.participant()
        || h.actor.grant_id() != Some(consent.subject)
        || h.digest != first.digest
        || h.related_operation != first.related_operation
        || h.order != first.order
        || h.revision <= first.revision
        || h.revision <= consent.frame.header.revision
        || consent.command.context().operation_id()
            != first
                .related_operation
                .ok_or(OperationError::RecoveryRequired)?
        || consent.command.digest() != h.digest
        || at < consent.observed_at
        || expires > consent.expires
        || at >= expires
        || expires.get() - at.get() > 30
        || head(h, body) != h.head
    {
        return Err(OperationError::RecoveryRequired);
    }
    let source = original.source.receipt();
    let request = StockRewrapRequest::new(
        StockEnvelopeRequest::new(
            binding.clone(),
            source.checkpoint(),
            source.attempt().operation_id(),
            source.attempt().command_digest(),
            source.recipient(),
            None,
        )?,
        id,
    );
    let report = StockRewrapAdmission::from_store_report(
        &request,
        source,
        h.transaction,
        at,
        expires,
        InventoryCheckpointRef::from_host_metadata(input.authority(), h.revision, Some(h.head))?,
    )?;
    let retained = Admitted {
        original_entry: first.operation,
        projection: Projection::Admission {
            operation: source.attempt().operation_id(),
            recipient: source.recipient(),
            request: id,
        },
        frame,
        report,
    };
    retained.confirm(connection).await?;
    Ok(retained)
}
impl Admitted {
    pub(super) async fn confirm(
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
    pub(super) fn into_history(self) -> ledger::HistoryEntry {
        ledger::HistoryEntry {
            envelope: Some(self.projection),
            delivery: None,
            aborted: None,
            preparation: None,
            header: self.frame.header,
            protected: self.frame.protected,
            parts_witness: self.frame.parts_witness,
            pools: vec![],
            mapping: None,
            movements: vec![],
            signing: None,
            installation: None,
            local_record: None,
        }
    }
}
pub(in crate::workspace) async fn prepare(
    turn: &mut Turn<'_>,
    request: StockRewrapRequest,
    proof: AttachmentProof,
) -> Result<StockRewrapOutcome, OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    let source = request.source();
    if source.envelope_event_id().is_some() {
        return Err(OperationError::InvalidCommand);
    }
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *turn.connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    if catalog != 18 {
        return Err(OperationError::UnsupportedOperation);
    }
    let auth = Box::pin(super::super::signing_plan::load(
        turn,
        source.binding(),
        source.minimum_checkpoint(),
        source.operation_id(),
        source.command_digest(),
        &proof,
    ))
    .await?;
    if auth.access.authority.catalog != 18
        || !auth
            .original
            .receipt
            .rumor()
            .recipients()
            .contains(&source.recipient())
    {
        return Err(OperationError::InvalidEvidence);
    }
    let witness = Box::pin(full_proof(turn, source, &auth.access.authority)).await?;
    let selection =
        super::rewrap::selection::SelectionWitness::read(turn.connection, source).await?;
    let original = Box::pin(lookup_original(turn, source))
        .await?
        .ok_or(OperationError::MissingHistory)?;
    let existing = Box::pin(lookup(turn, &request)).await?;
    let before = auth
        .access
        .authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let (frame, root, report) = if let Some(existing) = &existing {
        if existing.report.original() != original.source.receipt() {
            return Err(OperationError::RecoveryRequired);
        }
        (None, None, existing.report)
    } else {
        if selection.count >= 192 {
            return Err(OperationError::DeliveryAttemptLimit);
        }
        let at = auth.access.current.observed_at();
        let expires = UnixSeconds::new(
            at.get()
                .checked_add(30)
                .ok_or(OperationError::DeadlineExceeded)?,
        )
        .map_err(|_| OperationError::DeadlineExceeded)?
        .min(auth.access.current.expires_at())
        .min(auth.consent.expires);
        if expires <= at {
            return Err(OperationError::DeadlineExceeded);
        }
        let revision = Sequence::new(
            before
                .revision
                .get()
                .checked_add(1)
                .ok_or(WorkspaceError::Capacity)?,
        )
        .map_err(|_| WorkspaceError::Capacity)?;
        let body = encode(request.rewrap_id(), &original, &auth.consent, at, expires);
        let mut header = ledger::Header {
            version: 5,
            family: ledger::Family::RewrapAdmission,
            namespace: SellerAuthoritySelection::from_attachment(turn.selected),
            actor: auth.access.current.actor(),
            operation: OperationId::from_bytes(super::super::super::random()?),
            digest: source.command_digest(),
            transaction: TransactionId::from_bytes(super::super::super::random()?),
            revision,
            previous: before.head,
            head: CheckpointDigest::from_bytes([0; 32]),
            pool: None,
            order: original.frame.header.order,
            related_operation: Some(source.operation_id()),
            subject_grant: None,
            bytes: 0,
            parts: 0,
            body_digest: [0; 32],
        };
        header.head = head(&header, &body);
        let report = StockRewrapAdmission::from_store_report(
            &request,
            original.source.receipt(),
            header.transaction,
            at,
            expires,
            InventoryCheckpointRef::from_host_metadata(
                turn.selected.authority(),
                revision,
                Some(header.head),
            )?,
        )?;
        let frame = ledger::prepare(turn.crypto, turn.selected, header, &body).await?;
        let mut next = auth.access.authority.clone();
        next.inventory = Some(InventoryCheckpoint {
            revision,
            head: Some(frame.header.head),
            ..before
        });
        let root = turn
            .crypto
            .seal_stock_root(turn.selected, next.catalog, &next.encode()?)
            .await?;
        (Some(frame), Some(root), report)
    };
    let unknown = || {
        StockRewrapOutcome::Unknown(CommitAttempt::new(
            turn.selected.workspace_id(),
            source.operation_id(),
            source.command_digest(),
            Some(report.transaction_id()),
        ))
    };
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        let current = auth
            .confirm(&mut tx, turn.namespace, turn.clock, &proof, false)
            .await?;
        if frame.is_some() && current.observed_at() >= report.expires_at() {
            return Err(OperationError::DeadlineExceeded);
        }
        inventory::check_head(&mut tx, before).await?;
        witness.confirm(&mut tx).await?;
        selection.confirm(&mut tx, source).await?;
        original.confirm(&mut tx).await?;
        if let Some(existing) = &existing {
            existing.confirm(&mut tx).await?;
        }
        if let Some(frame) = &frame {
            frame.insert_record(&mut tx).await?;
            Projection::Admission {
                operation: source.operation_id(),
                recipient: source.recipient(),
                request: request.rewrap_id(),
            }
            .insert(&mut tx, turn.selected.participant(), frame.header.operation)
            .await?;
            let changed = sqlx::query(
                "UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?",
            )
            .bind(root.as_ref().ok_or(OperationError::RecoveryRequired)?)
            .bind(&auth.access.root)
            .execute(&mut *tx)
            .await
            .map_err(|_| OperationError::StorageFailure)?;
            if changed.rows_affected() != 1 {
                return Err(OperationError::RecoveryRequired);
            }
        }
        turn.namespace.validate()?;
        Ok(())
    }
    .await;
    if let Err(error) = checked {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(unknown());
        }
        return Err(error);
    }
    let done = if frame.is_some() {
        tx.commit().await
    } else {
        tx.rollback().await
    };
    if done.is_err() {
        *turn.uncertain = true;
        return Ok(unknown());
    }
    Ok(if frame.is_some() {
        StockRewrapOutcome::Claimed(Box::new(report))
    } else {
        StockRewrapOutcome::Retained(Box::new(report))
    })
}

/// Repair the original writer fence without returning history or a new permit.
pub(in crate::workspace) async fn reconcile(
    turn: &mut Turn<'_>,
    request: StockRewrapRequest,
    proof: AttachmentProof,
) -> Result<(), OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    let source = request.source();
    if source.envelope_event_id().is_some() {
        return Err(OperationError::InvalidCommand);
    }
    let input = source.binding().input();
    if !SellerAuthoritySelection::from_attachment(turn.selected).matches_attachment(input) {
        return Err(OperationError::Unauthorized);
    }
    let (authority, record, root, grant) =
        super::super::read(turn.connection, input, turn.crypto).await?;
    if authority.catalog != 18 {
        return Err(OperationError::UnsupportedOperation);
    }
    record.authenticate_retained_capture(input, &proof)?;
    let witness = Box::pin(full_proof(turn, source, &authority)).await?;
    let selection =
        super::rewrap::selection::SelectionWitness::read(turn.connection, source).await?;
    let existing = Box::pin(lookup(turn, &request)).await?;
    if existing
        .as_ref()
        .is_some_and(|value| value.frame.header.actor != source.binding().actor())
    {
        return Err(OperationError::Unauthorized);
    }
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        turn.namespace.validate()?;
        super::super::unchanged(&mut tx, input, authority.catalog, &root, &grant).await?;
        record.authenticate_retained_capture(input, &proof)?;
        witness.confirm(&mut tx).await?;
        selection.confirm(&mut tx, source).await?;
        if let Some(existing) = &existing {
            existing.confirm(&mut tx).await?;
        }
        turn.namespace.validate()?;
        Ok(())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    checked
}

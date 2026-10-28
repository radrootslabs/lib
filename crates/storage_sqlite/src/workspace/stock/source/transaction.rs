use super::super::inventory::Turn;
use super::*;
use checkpoint::{Proof, Target};
use radroots_storage::command::protected::same_binding;
use record::{Header, RetainedHeader};

struct Access {
    authority: Authority,
    record: StockRecord,
    root: Vec<u8>,
    grant: Vec<u8>,
    current: WorkspaceBinding,
    minimum: Proof,
}
async fn access(
    turn: &mut Turn<'_>,
    binding: &WorkspaceBinding,
    minimum: StockSourceCheckpointRef,
    proof: &AttachmentProof,
) -> Result<Access, OperationError> {
    if *turn.uncertain {
        return Err(OperationError::RecoveryRequired);
    }
    turn.namespace.validate()?;
    if !SellerAuthoritySelection::from_attachment(turn.selected).matches_attachment(binding.input())
    {
        return Err(OperationError::Unauthorized);
    }
    let (authority, record, root, grant) =
        super::super::read(turn.connection, binding.input(), turn.crypto).await?;
    if !(6..=16).contains(&authority.catalog) {
        return Err(OperationError::UnsupportedOperation);
    }
    let current = record.authenticate(binding.input(), proof, turn.clock.now()?)?;
    same_binding(binding, &current)?;
    if current.observed_at() < binding.observed_at() {
        return Err(WorkspaceError::Unavailable.into());
    }
    if current.scope() != WorkspaceScope::SellerStockOwner || !record.administrator() {
        return Err(OperationError::Unauthorized);
    }
    if authority.authorization != minimum.authorization() {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    let witness = checkpoint::authenticate(
        turn.connection,
        turn.selected,
        turn.crypto,
        authority.sources.ok_or(OperationError::RecoveryRequired)?,
        Target::Minimum(minimum),
    )
    .await?;
    Ok(Access {
        authority,
        record,
        root,
        grant,
        current,
        minimum: witness,
    })
}
impl Access {
    async fn confirm(
        &self,
        connection: &mut SqliteConnection,
        namespace: &Namespace,
        clock: &dyn WorkspaceClock,
        proof: &AttachmentProof,
    ) -> Result<WorkspaceBinding, OperationError> {
        namespace.validate()?;
        super::super::unchanged(
            connection,
            self.current.input(),
            self.authority.catalog,
            &self.root,
            &self.grant,
        )
        .await?;
        grants::check_state(
            connection,
            self.authority
                .authorization
                .ok_or(OperationError::RecoveryRequired)?,
        )
        .await?;
        super::super::check_source_counts(
            connection,
            self.authority
                .sources
                .ok_or(OperationError::RecoveryRequired)?,
        )
        .await?;
        self.minimum.confirm(connection).await?;
        let current = self
            .record
            .authenticate(self.current.input(), proof, clock.now()?)?;
        same_binding(&self.current, &current)?;
        if current.observed_at() < self.current.observed_at() {
            return Err(WorkspaceError::Unavailable.into());
        }
        if !self.record.administrator() {
            return Err(OperationError::Unauthorized);
        }
        Ok(current)
    }
}
pub(in crate::workspace) async fn history(
    turn: &mut Turn<'_>,
    binding: WorkspaceBinding,
    inventory_minimum: radroots_storage::inventory::InventoryCheckpointRef,
    minimum: StockSourceCheckpointRef,
    listing_minimum: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
    scope: radroots_domain::causal::CausalScope,
    proof: AttachmentProof,
) -> Result<StockHistorySnapshot, OperationError> {
    let access = access(turn, &binding, minimum, &proof).await?;
    if !matches!(access.authority.catalog, 8..=16) {
        return Err(OperationError::UnsupportedOperation);
    }
    let prepared = Box::pin(super::history::prepare(
        turn.connection,
        turn.selected,
        turn.crypto,
        super::history::InventoryRead {
            binding: &access.current,
            authority: &access.authority,
            minimum: inventory_minimum,
        },
        scope,
        listing_minimum,
    ))
    .await?;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<_, OperationError> = async {
        // The complete cryptographic/row witness may span a large local
        // history. Check usable current permission/time after that work.
        prepared.confirm(&mut tx).await?;
        let current = access
            .confirm(&mut tx, turn.namespace, turn.clock, &proof)
            .await?;
        Ok(current.observed_at())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    let observed_at = checked?;
    let private = access
        .authority
        .sources
        .ok_or(OperationError::RecoveryRequired)?;
    let public = access
        .authority
        .public_sources
        .ok_or(OperationError::RecoveryRequired)?;
    Ok(StockHistorySnapshot {
        inventory: prepared.inventory,
        scope,records:prepared.records.len(),analysis:prepared.analysis,checkpoint:prepared.checkpoint,observed_at,
        sources:StockSourceCheckpointRef::from_host_metadata(turn.selected.authority(),private.revision,private.head)?.with_optional_authorization(access.authority.authorization),
        listings:radroots_storage::workspace::seller_stock::StockListingCheckpointRef::from_host_metadata(turn.selected.authority(),public.revision,public.head)?.with_optional_authorization(access.authority.authorization),
    })
}
pub(in crate::workspace) async fn inspect_request(
    turn: &mut Turn<'_>,
    binding: WorkspaceBinding,
    inventory_minimum: radroots_storage::inventory::InventoryCheckpointRef,
    minimum: StockSourceCheckpointRef,
    listing_minimum: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
    selected: RecordId,
    proof: AttachmentProof,
) -> Result<StockRequestSnapshot, OperationError> {
    let access = access(turn, &binding, minimum, &proof).await?;
    if !matches!(access.authority.catalog, 8..=16) {
        return Err(OperationError::UnsupportedOperation);
    }
    let prepared = Box::pin(super::request::prepare(
        turn.connection,
        turn.selected,
        turn.crypto,
        super::history::InventoryRead {
            binding: &access.current,
            authority: &access.authority,
            minimum: inventory_minimum,
        },
        selected,
        listing_minimum,
        access.current.observed_at(),
    ))
    .await;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<_, OperationError> = async {
        let witnessed = match &prepared {
            Ok(value) => value.confirm(&mut tx).await,
            Err(_) => Ok(()),
        };
        // Do not disclose even a detailed source denial under expired/revoked
        // access. All provider work and row witnesses precede this final check.
        let current = access
            .confirm(&mut tx, turn.namespace, turn.clock, &proof)
            .await?;
        witnessed?;
        if let Ok(value) = &prepared {
            value.validate_at(current.observed_at())?;
        }
        Ok(current.observed_at())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    let observed_at = checked?;
    let prepared = prepared?;
    let private = access
        .authority
        .sources
        .ok_or(OperationError::RecoveryRequired)?;
    let public = access
        .authority
        .public_sources
        .ok_or(OperationError::RecoveryRequired)?;
    Ok(StockRequestSnapshot {
        inventory: prepared.history.inventory,
        scope:prepared.scope,request:prepared.request,terms:Box::new(prepared.terms()?.clone()),digest:prepared.digest,
        expires:prepared.expires,quote:prepared.quote,checkpoint:prepared.history.checkpoint.clone().ok_or(OperationError::RecoveryRequired)?,observed_at,
        sources:StockSourceCheckpointRef::from_host_metadata(turn.selected.authority(),private.revision,private.head)?.with_optional_authorization(access.authority.authorization),
        listings:radroots_storage::workspace::seller_stock::StockListingCheckpointRef::from_host_metadata(turn.selected.authority(),public.revision,public.head)?.with_optional_authorization(access.authority.authorization),
    })
}
struct Observation {
    ancestry: Proof,
    header: Option<RetainedHeader>,
    canonical: Option<(RecordId, Option<Vec<u8>>)>,
    parts: Option<[u8; 32]>,
    record_ancestry: Option<Proof>,
}
impl Observation {
    async fn confirm(
        &self,
        connection: &mut SqliteConnection,
        envelope: EnvelopeEventId,
    ) -> Result<(), OperationError> {
        self.ancestry.confirm(connection).await?;
        if let Some(proof) = &self.record_ancestry {
            proof.confirm(connection).await?;
        }
        let count: i64 = if let Some(header) = &self.header {
            sqlx::query_scalar(
                "SELECT count(*) FROM stock_source_evidence WHERE envelope_id=? AND protected=?",
            )
            .bind(envelope.as_bytes().as_slice())
            .bind(&header.protected)
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?
        } else {
            sqlx::query_scalar("SELECT count(*) FROM stock_source_evidence WHERE envelope_id=?")
                .bind(envelope.as_bytes().as_slice())
                .fetch_one(&mut *connection)
                .await
                .map_err(|_| OperationError::RecoveryRequired)?
        };
        if count != i64::from(self.header.is_some()) {
            return Err(OperationError::RecoveryRequired);
        }
        if let Some((id, protected)) = &self.canonical {
            let count: i64 = if let Some(protected) = protected {
                sqlx::query_scalar(
                    "SELECT count(*) FROM stock_source_records WHERE record_id=? AND protected=?",
                )
                .bind(id.as_bytes().as_slice())
                .bind(protected)
                .fetch_one(&mut *connection)
                .await
                .map_err(|_| OperationError::RecoveryRequired)?
            } else {
                sqlx::query_scalar("SELECT count(*) FROM stock_source_records WHERE record_id=?")
                    .bind(id.as_bytes().as_slice())
                    .fetch_one(&mut *connection)
                    .await
                    .map_err(|_| OperationError::RecoveryRequired)?
            };
            if count != i64::from(protected.is_some()) {
                return Err(OperationError::RecoveryRequired);
            }
        }
        if let (Some(header), Some(expected)) = (&self.header, self.parts) {
            record::confirm_parts(connection, &header.header, expected).await?;
        }
        Ok(())
    }
}
async fn observed(
    turn: &mut Turn<'_>,
    authority: &Authority,
    envelope: EnvelopeEventId,
) -> Result<Observation, OperationError> {
    let retained = record::read(turn.connection, turn.selected, turn.crypto, envelope).await?;
    let expected = retained.as_ref().map(|value| {
        (
            value.header.revision,
            value.header.head,
            Sha256::digest(&value.protected).into(),
        )
    });
    let ancestry = checkpoint::authenticate(
        turn.connection,
        turn.selected,
        turn.crypto,
        authority.sources.ok_or(OperationError::RecoveryRequired)?,
        Target::Envelope(envelope, expected),
    )
    .await?;
    Ok(Observation {
        ancestry,
        header: retained,
        canonical: None,
        parts: None,
        record_ancestry: None,
    })
}
async fn load(
    turn: &mut Turn<'_>,
    observation: &mut Observation,
) -> Result<StockSourceSnapshot, OperationError> {
    let retained = observation
        .header
        .as_ref()
        .ok_or(OperationError::RecoveryRequired)?;
    let (pieces, witness) = record::pieces(
        turn.connection,
        turn.selected,
        turn.crypto,
        &retained.header,
    )
    .await?;
    observation.parts = Some(witness);
    let canonical = if let Some(id) = retained.header.manifest.record {
        let (clear, protected) = record::canonical(turn.connection, turn.selected, turn.crypto, id)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let digest: [u8; 32] = Sha256::digest(clear.as_slice()).into();
        if Some(digest) != retained.header.manifest.canonical_digest {
            return Err(OperationError::RecoveryRequired);
        }
        observation.canonical = Some((id, Some(protected)));
        Some(clear)
    } else {
        None
    };
    Ok(StockSourceSnapshot {
        receipt: retained.header.receipt(turn.selected)?,
        pieces,
        canonical,
    })
}
async fn confirm_read(
    turn: &mut Turn<'_>,
    access: &Access,
    proof: &AttachmentProof,
    envelope: EnvelopeEventId,
    observed: &Observation,
) -> Result<(), OperationError> {
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<(), OperationError> = async {
        access
            .confirm(&mut tx, turn.namespace, turn.clock, proof)
            .await?;
        observed.confirm(&mut tx, envelope).await?;
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
pub(in crate::workspace) async fn query(
    turn: &mut Turn<'_>,
    binding: WorkspaceBinding,
    minimum: StockSourceCheckpointRef,
    envelope: EnvelopeEventId,
    proof: AttachmentProof,
) -> Result<Option<StockSourceSnapshot>, OperationError> {
    let access = access(turn, &binding, minimum, &proof).await?;
    let mut observation = observed(turn, &access.authority, envelope).await?;
    let result = if observation.header.is_some() {
        Some(load(turn, &mut observation).await?)
    } else {
        None
    };
    confirm_read(turn, &access, &proof, envelope, &observation).await?;
    Ok(result)
}
pub(in crate::workspace) async fn retain(
    turn: &mut Turn<'_>,
    binding: WorkspaceBinding,
    minimum: StockSourceCheckpointRef,
    capture: StockSourceCapture,
    proof: AttachmentProof,
) -> Result<StockSourceOutcome, OperationError> {
    let access = access(turn, &binding, minimum, &proof).await?;
    if capture.namespace != SellerAuthoritySelection::from_attachment(turn.selected) {
        return Err(OperationError::Unauthorized);
    }
    let mut observation = observed(turn, &access.authority, capture.envelope).await?;
    if let Some(prior) = &observation.header {
        if prior.header.manifest != capture.manifest {
            return Err(OperationError::IdempotencyConflict);
        }
        let snapshot = load(turn, &mut observation).await?;
        confirm_read(turn, &access, &proof, capture.envelope, &observation).await?;
        return Ok(StockSourceOutcome::Committed(snapshot.receipt));
    }
    let before = access
        .authority
        .sources
        .ok_or(OperationError::RecoveryRequired)?;
    let mut protected_record = None;
    let mut new_record = false;
    if let Some((id, canonical)) = &capture.canonical {
        let prior = record::canonical(turn.connection, turn.selected, turn.crypto, *id).await?;
        let digest = prior
            .as_ref()
            .map(|value| Sha256::digest(value.0.as_slice()).into());
        observation.record_ancestry = Some(
            checkpoint::authenticate(
                turn.connection,
                turn.selected,
                turn.crypto,
                before,
                Target::Record(*id, digest),
            )
            .await?,
        );
        if let Some((clear, _)) = &prior
            && clear.as_slice() != canonical.as_slice()
        {
            return Err(OperationError::InvalidEvidence);
        }
        new_record = prior.is_none();
        if new_record {
            protected_record = Some((
                *id,
                turn.crypto
                    .seal_stock_record(turn.selected, "source_record", id.as_bytes(), canonical)
                    .await?,
            ));
        }
        observation.canonical = Some((*id, prior.map(|value| value.1)));
    }
    let revision = Sequence::new(
        before
            .revision
            .get()
            .checked_add(1)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?;
    let mut header = Header {
        version: 1,
        manifest: capture.manifest,
        transaction: capture.transaction,
        revision,
        previous: before.head,
        head: CheckpointDigest::from_bytes([0; 32]),
        grant: access.current.input().grant_id(),
        principal: access.current.principal_id().into(),
        authorization: access
            .authority
            .authorization
            .ok_or(OperationError::RecoveryRequired)?,
    };
    header.head = record::head(turn.selected, &header)?;
    let mut parts = Vec::new();
    for (role, clear) in capture.pieces.iter().enumerate() {
        for (part, clear) in clear.chunks(16 * 1024).enumerate() {
            let protected = turn
                .crypto
                .seal_stock_record(
                    turn.selected,
                    "source_part",
                    &record::part_id(capture.envelope, role as u8, part as u8),
                    clear,
                )
                .await?;
            parts.push((role as u8, part as u8, protected));
        }
    }
    let protected_header = turn
        .crypto
        .seal_stock_record(
            turn.selected,
            "source_evidence",
            capture.envelope.as_bytes(),
            &record::encode(&header)?,
        )
        .await?;
    let mut next = access.authority.clone();
    next.sources = Some(SourceCheckpoint {
        revision,
        head: Some(header.head),
        records: Sequence::new(
            before
                .records
                .get()
                .checked_add(u64::from(new_record))
                .ok_or(WorkspaceError::Capacity)?,
        )
        .map_err(|_| WorkspaceError::Capacity)?,
        parts: Sequence::new(
            before
                .parts
                .get()
                .checked_add(parts.len() as u64)
                .ok_or(WorkspaceError::Capacity)?,
        )
        .map_err(|_| WorkspaceError::Capacity)?,
    });
    let root = turn
        .crypto
        .seal_stock_root(turn.selected, next.catalog, &next.encode()?)
        .await?;
    let receipt = header.receipt(turn.selected)?;
    turn.namespace.validate()?;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed: Result<(), OperationError> = async {
        access.confirm(&mut tx, turn.namespace, turn.clock, &proof).await?;
        observation.confirm(&mut tx, capture.envelope).await?;
        if let Some((id, protected)) = protected_record {
            sqlx::query("INSERT INTO stock_source_records(record_id,protected) VALUES(?,?)")
                .bind(id.as_bytes().as_slice())
                .bind(protected)
                .execute(&mut *tx)
                .await
                .map_err(|_| OperationError::StorageFailure)?;
        }
        sqlx::query("INSERT INTO stock_source_evidence(envelope_id,record_id,transaction_id,revision,chain_head,protected) VALUES(?,?,?,?,?,?)")
            .bind(capture.envelope.as_bytes().as_slice())
            .bind(header.manifest.record.map(|id| id.as_bytes().to_vec()))
            .bind(capture.transaction.as_bytes().as_slice())
            .bind(revision.get() as i64)
            .bind(header.head.as_bytes().as_slice())
            .bind(protected_header)
            .execute(&mut *tx)
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        for (role, part, protected) in parts {
            sqlx::query("INSERT INTO stock_source_parts(envelope_id,role,part,protected) VALUES(?,?,?,?)")
                .bind(capture.envelope.as_bytes().as_slice())
                .bind(i64::from(role))
                .bind(i64::from(part))
                .bind(protected)
                .execute(&mut *tx)
                .await
                .map_err(|_| OperationError::StorageFailure)?;
        }
        let changed = sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?")
            .bind(root)
            .bind(&access.root)
            .execute(&mut *tx)
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        if changed.rows_affected() != 1 {
            return Err(OperationError::RecoveryRequired);
        }
        turn.namespace.validate()?;
        Ok(())
    }.await;
    let unknown = || StockSourceOutcome::Unknown {
        transaction: capture.transaction,
        envelope: capture.envelope,
    };
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(unknown());
        }
        return Err(error);
    }
    if tx.commit().await.is_err() {
        *turn.uncertain = true;
        return Ok(unknown());
    }
    Ok(StockSourceOutcome::Committed(receipt))
}

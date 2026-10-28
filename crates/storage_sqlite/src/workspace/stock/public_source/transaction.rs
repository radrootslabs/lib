use super::*;
use checkpoint::{Proof, Target};
use radroots_domain::Sequence;
use radroots_storage::{
    command::{OperationError, protected::same_binding},
    workspace::seller_stock::StockListingCheckpointRef,
};
use sha2::{Digest, Sha256};

struct Access {
    authority: Authority,
    record: StockRecord,
    root: Vec<u8>,
    grant: Vec<u8>,
    current: WorkspaceBinding,
    minimum: Proof,
}
async fn access(
    turn: &mut inventory::Turn<'_>,
    binding: &WorkspaceBinding,
    minimum: StockListingCheckpointRef,
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
    if !matches!(authority.catalog, 8..=17) || authority.public_sources.is_none() {
        return Err(OperationError::UnsupportedOperation);
    }
    let current = record.authenticate(binding.input(), proof, turn.clock.now()?)?;
    same_binding(binding, &current)?;
    if current.scope() != WorkspaceScope::SellerStockOwner || !record.administrator() {
        return Err(OperationError::Unauthorized);
    }
    if current.observed_at() < binding.observed_at() {
        return Err(WorkspaceError::Unavailable.into());
    }
    if minimum.authorization() != authority.authorization {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    let minimum = checkpoint::authenticate(
        turn.connection,
        turn.selected,
        turn.crypto,
        &authority,
        Target::Minimum(minimum),
    )
    .await?;
    Ok(Access {
        authority,
        record,
        root,
        grant,
        current,
        minimum,
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
        counts(
            connection,
            self.authority
                .public_sources
                .ok_or(OperationError::RecoveryRequired)?,
        )
        .await?;
        self.minimum.confirm(connection).await?;
        let current = self
            .record
            .authenticate(self.current.input(), proof, clock.now()?)?;
        same_binding(&self.current, &current)?;
        if !self.record.administrator() {
            return Err(OperationError::Unauthorized);
        }
        if current.observed_at() < self.current.observed_at() {
            return Err(WorkspaceError::Unavailable.into());
        }
        namespace.validate()?;
        Ok(current)
    }
}
fn add(value: Sequence, delta: u64) -> Result<Sequence, OperationError> {
    Sequence::new(
        value
            .get()
            .checked_add(delta)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity.into())
}

pub(in crate::workspace) async fn retain(
    turn: &mut inventory::Turn<'_>,
    binding: WorkspaceBinding,
    minimum: StockListingCheckpointRef,
    capture: StockListingCapture,
    proof: AttachmentProof,
) -> Result<StockListingOutcome, OperationError> {
    let access = access(turn, &binding, minimum, &proof).await?;
    if capture.namespace != SellerAuthoritySelection::from_attachment(turn.selected) {
        return Err(OperationError::Unauthorized);
    }
    let event = capture.listing.event_id();
    let coordinate = capture.listing.coordinate();
    let original = record::read(
        turn.connection,
        turn.selected,
        turn.crypto,
        access.authority.input.grant_id(),
        event,
    )
    .await?;
    if let Some(original) = original {
        if original.header.manifest.listing != capture.listing
            || record::time(&original.header.manifest.created_at)? != capture.created_at
        {
            return Err(OperationError::RecoveryRequired);
        }
        // Both captures independently verified the same NIP01 identity. A
        // transport re-encoding may deduplicate, never replace original bytes.
        let parts = record::read_parts(
            turn.connection,
            turn.selected,
            turn.crypto,
            &original.header,
        )
        .await?;
        let identity = checkpoint::authenticate(
            turn.connection,
            turn.selected,
            turn.crypto,
            &access.authority,
            Target::Event(event, Some(original.identity()?)),
        )
        .await?;
        let receipt = original.header.receipt()?;
        let mut tx = turn
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let checked: Result<(), OperationError> = async {
            access
                .confirm(&mut tx, turn.namespace, turn.clock, &proof)
                .await?;
            identity.confirm(&mut tx).await?;
            if record::parts_witness(&mut tx, event).await?
                != (original.header.manifest.parts, parts.witness)
            {
                return Err(OperationError::RecoveryRequired);
            }
            Ok(())
        }
        .await;
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Err(OperationError::RecoveryRequired);
        }
        checked?;
        return Ok(StockListingOutcome::Committed(Box::new(receipt)));
    }
    let absence = checkpoint::authenticate(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.authority,
        Target::Event(event, None),
    )
    .await?;
    let before = record::read_head(turn.connection, turn.selected, turn.crypto, coordinate).await?;
    let projection = checkpoint::authenticate(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.authority,
        Target::Coordinate(coordinate, before.as_ref().map(|value| value.head.clone())),
    )
    .await?;
    let progress = access
        .authority
        .public_sources
        .ok_or(OperationError::RecoveryRequired)?;
    let revision = add(progress.revision, 1)?;
    let candidate = record::Head {
        version: 1,
        listing: capture.listing,
        created_at: capture.created_at.to_string(),
        revision,
    };
    let after = record::select(before.as_ref().map(|value| &value.head), candidate)?;
    let selected = after.listing.event_id() == event;
    let authorization = access
        .authority
        .authorization
        .ok_or(OperationError::RecoveryRequired)?;
    let mut header = record::Header {
        version: 1,
        namespace: capture.namespace.clone(),
        transaction: capture.transaction,
        revision,
        previous: progress.head,
        head: None,
        grant: access.current.input().grant_id(),
        principal: access.current.principal_id().into(),
        authorization,
        observed_at: access.current.observed_at(),
        minimum_revision: minimum.revision(),
        minimum_head: minimum.head(),
        manifest: record::Manifest {
            listing: capture.listing,
            created_at: capture.created_at.to_string(),
            bytes: capture.raw.len() as u32,
            digest: Sha256::digest(capture.raw.as_slice()).into(),
            parts: capture.raw.len().div_ceil(record::PART_BYTES) as u8,
        },
        before: before.as_ref().map(|value| value.head.clone()),
        after,
    };
    header.head = Some(header.calculate_head()?);
    header.validate(turn.selected, access.authority.input.grant_id())?;
    let receipt = header.receipt()?;
    let parts = record::PreparedParts::prepare(turn.crypto, turn.selected, &capture).await?;
    let protected = turn
        .crypto
        .seal_stock_record(
            turn.selected,
            "public_event",
            event.as_bytes(),
            &record::encode(&header)?,
        )
        .await?;
    let protected_head = if selected {
        Some(
            turn.crypto
                .seal_stock_record(
                    turn.selected,
                    "public_head",
                    &record::coordinate_id(coordinate),
                    &record::encode(&header.after)?,
                )
                .await?,
        )
    } else {
        None
    };
    let mut next = access.authority.clone();
    next.public_sources = Some(PublicSourceCheckpoint {
        revision,
        events: add(progress.events, 1)?,
        heads: add(progress.heads, u64::from(before.is_none()))?,
        parts: add(progress.parts, u64::from(header.manifest.parts))?,
        head: header.head,
    });
    let root = turn
        .crypto
        .seal_stock_root(turn.selected, next.catalog, &next.encode()?)
        .await?;
    turn.namespace.validate()?;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed:Result<(),OperationError>=async {
        access.confirm(&mut tx,turn.namespace,turn.clock,&proof).await?;
        absence.confirm(&mut tx).await?;projection.confirm(&mut tx).await?;
        record::confirm_head(&mut tx,coordinate,before.as_ref()).await?;
        let present:i64=sqlx::query_scalar("SELECT count(*) FROM stock_public_events WHERE event_id=?").bind(event.as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::RecoveryRequired)?;
        if present!=0 {return Err(OperationError::StaleState);}
        sqlx::query("INSERT INTO stock_public_events(event_id,listing_author,listing_d,created_at,transaction_id,revision,chain_head,protected) VALUES(?,?,?,?,?,?,?,?)")
            .bind(event.as_bytes().as_slice()).bind(coordinate.author().as_bytes().as_slice()).bind(coordinate.d().as_bytes().as_slice())
            .bind(capture.created_at.to_be_bytes().as_slice()).bind(capture.transaction.as_bytes().as_slice()).bind(revision.get() as i64)
            .bind(header.head.ok_or(OperationError::RecoveryRequired)?.as_bytes().as_slice()).bind(&protected).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
        parts.insert(&mut tx,event).await?;
        if let Some(protected)=&protected_head {
            if let Some(before)=&before {
                let changed=sqlx::query("UPDATE stock_public_heads SET event_id=?,revision=?,protected=? WHERE listing_author=? AND listing_d=? AND protected=?")
                    .bind(event.as_bytes().as_slice()).bind(revision.get() as i64).bind(protected)
                    .bind(coordinate.author().as_bytes().as_slice()).bind(coordinate.d().as_bytes().as_slice()).bind(&before.protected)
                    .execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
                if changed.rows_affected()!=1 {return Err(OperationError::RecoveryRequired);}
            } else {
                sqlx::query("INSERT INTO stock_public_heads(listing_author,listing_d,event_id,revision,protected) VALUES(?,?,?,?,?)")
                    .bind(coordinate.author().as_bytes().as_slice()).bind(coordinate.d().as_bytes().as_slice())
                    .bind(event.as_bytes().as_slice()).bind(revision.get() as i64).bind(protected)
                    .execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            }
        }
        let changed=sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?").bind(root).bind(&access.root)
            .execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
        if changed.rows_affected()!=1 {return Err(OperationError::RecoveryRequired);}
        turn.namespace.validate()?;Ok(())
    }.await;
    let unknown = || StockListingOutcome::Unknown {
        transaction: capture.transaction,
        listing: capture.listing,
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
    Ok(StockListingOutcome::Committed(Box::new(receipt)))
}

pub(in crate::workspace) enum Lookup {
    Revision(ListingRevision),
    Coordinate(ListingCoordinate),
}
pub(in crate::workspace) async fn query(
    turn: &mut inventory::Turn<'_>,
    binding: WorkspaceBinding,
    minimum: StockListingCheckpointRef,
    lookup: Lookup,
    proof: AttachmentProof,
) -> Result<Option<StockListingSnapshot>, OperationError> {
    let access = access(turn, &binding, minimum, &proof).await?;
    let coordinate = match &lookup {
        Lookup::Revision(value) => value.coordinate(),
        Lookup::Coordinate(value) => *value,
    };
    if coordinate.author() != turn.selected.participant() {
        return Err(OperationError::Unauthorized);
    }
    let head = record::read_head(turn.connection, turn.selected, turn.crypto, coordinate).await?;
    let projection = checkpoint::authenticate(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.authority,
        Target::Coordinate(coordinate, head.as_ref().map(|value| value.head.clone())),
    )
    .await?;
    let wanted = match lookup {
        Lookup::Revision(value) => Some(value),
        Lookup::Coordinate(_) => head.as_ref().map(|value| value.head.listing),
    };
    let original = if let Some(wanted) = wanted {
        let original = record::read(
            turn.connection,
            turn.selected,
            turn.crypto,
            access.authority.input.grant_id(),
            wanted.event_id(),
        )
        .await?;
        if original
            .as_ref()
            .is_some_and(|original| original.header.manifest.listing != wanted)
        {
            return Err(OperationError::InvalidCommand);
        }
        original
    } else {
        None
    };
    let parts = if let Some(original) = &original {
        Some(
            record::read_parts(
                turn.connection,
                turn.selected,
                turn.crypto,
                &original.header,
            )
            .await?,
        )
    } else {
        None
    };
    let identity = if let Some(wanted) = wanted {
        Some(
            checkpoint::authenticate(
                turn.connection,
                turn.selected,
                turn.crypto,
                &access.authority,
                Target::Event(
                    wanted.event_id(),
                    original
                        .as_ref()
                        .map(|original| original.identity())
                        .transpose()?,
                ),
            )
            .await?,
        )
    } else {
        None
    };
    let selected = original.as_ref().is_some_and(|original| {
        head.as_ref()
            .is_some_and(|head| head.head.listing == original.header.manifest.listing)
    });
    let progress = access
        .authority
        .public_sources
        .ok_or(OperationError::RecoveryRequired)?;
    let selected_at = StockListingCheckpointRef::from_host_metadata(
        turn.selected.authority(),
        progress.revision,
        progress.head,
    )?
    .with_optional_authorization(access.authority.authorization);
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let checked: Result<WorkspaceBinding, OperationError> = async {
        let current = access
            .confirm(&mut tx, turn.namespace, turn.clock, &proof)
            .await?;
        projection.confirm(&mut tx).await?;
        if let Some(identity) = &identity {
            identity.confirm(&mut tx).await?;
        }
        record::confirm_head(&mut tx, coordinate, head.as_ref()).await?;
        if let (Some(original), Some(parts)) = (&original, &parts)
            && record::parts_witness(&mut tx, original.header.manifest.listing.event_id()).await?
                != (original.header.manifest.parts, parts.witness)
        {
            return Err(OperationError::RecoveryRequired);
        }
        turn.namespace.validate()?;
        Ok(current)
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    let current = checked?;
    match (original, parts) {
        (Some(original), Some(parts)) => Ok(Some(StockListingSnapshot {
            receipt: original.header.receipt()?,
            selection_checkpoint: selected_at,
            observed_at: current.observed_at(),
            current: selected,
            raw: Zeroizing::new(
                String::from_utf8(parts.raw.to_vec())
                    .map_err(|_| OperationError::RecoveryRequired)?,
            ),
        })),
        (None, None) => Ok(None),
        _ => Err(OperationError::RecoveryRequired),
    }
}

//! A confirmed current-catalog replay admission precedes every returned permit.
use super::super::inventory::Turn;
use super::*;
use radroots_domain::CheckpointDigest;
use radroots_storage::workspace::seller_stock::StockAuthorizationRef;
use zeroize::Zeroizing;

const MAGIC: &[u8; 8] = b"RDSTRP1\0";
const BYTES: usize = 184;

/// Authenticated first admission of possible exact replay, never a dispatch or
/// nonissuance certificate. Later current authorization does not erase this fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockReplayAdmissionReceipt {
    attempt: CommitAttempt,
    claim: SigningAttemptId,
    recipient: PublicKey,
    observed_at: UnixSeconds,
    authorization: StockAuthorizationRef,
    checkpoint: InventoryCheckpointRef,
}
impl StockReplayAdmissionReceipt {
    /// Original operation and confirmed first admission transaction.
    pub const fn attempt(&self) -> CommitAttempt {
        self.attempt
    }
    /// Exact original durable signing claim.
    pub const fn signing_attempt_id(&self) -> SigningAttemptId {
        self.claim
    }
    /// Original recipient, independently of other recipient observations.
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    /// First checked authorization observation, captured before protection IO.
    /// Permission and the original window are rechecked before actual commit.
    pub const fn observed_at(&self) -> UnixSeconds {
        self.observed_at
    }
    /// Authenticated grant-set observation captured by the first admission.
    pub const fn authorization(&self) -> StockAuthorizationRef {
        self.authorization
    }
    /// Protected first admission checkpoint; metadata supplies no permission.
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
}
fn body(
    original: &claim::codec::Retained,
    at: UnixSeconds,
    auth: StockAuthorizationRef,
) -> Zeroizing<Vec<u8>> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(BYTES));
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(original.frame.header.operation.as_bytes());
    bytes.extend_from_slice(original.frame.header.transaction.as_bytes());
    bytes.extend_from_slice(original.frame.header.head.as_bytes());
    bytes.extend_from_slice(original.report.binding().digest());
    bytes.extend_from_slice(&at.get().to_be_bytes());
    bytes.extend_from_slice(&auth.generation().get().to_be_bytes());
    bytes.extend_from_slice(auth.digest().as_bytes());
    bytes
}
fn head(header: &ledger::Header, bytes: &[u8]) -> CheckpointDigest {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.replay_admission_chain.v1\0");
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
            .map(|v| *v.as_bytes())
            .unwrap_or([0; 32]),
    );
    hash.update(header.operation.as_bytes());
    hash.update(header.digest.as_bytes());
    hash.update(header.transaction.as_bytes());
    hash.update(header.revision.get().to_be_bytes());
    hash.update([u8::from(header.previous.is_some())]);
    hash.update(header.previous.map(|v| *v.as_bytes()).unwrap_or([0; 32]));
    hash.update(Sha256::digest(bytes));
    CheckpointDigest::from_bytes(hash.finalize().into())
}
fn receipt(
    input: &AttachmentInput,
    header: &ledger::Header,
    original: &claim::codec::Retained,
    at: UnixSeconds,
    auth: StockAuthorizationRef,
) -> Result<StockReplayAdmissionReceipt, OperationError> {
    Ok(StockReplayAdmissionReceipt {
        attempt: CommitAttempt::new(
            input.workspace_id(),
            original.report.attempt().operation_id(),
            header.digest,
            Some(header.transaction),
        ),
        claim: original.report.attempt_id(),
        recipient: original.report.recipient(),
        observed_at: at,
        authorization: auth,
        checkpoint: InventoryCheckpointRef::from_host_metadata(
            input.authority(),
            header.revision,
            Some(header.head),
        )?
        .with_authorization(auth),
    })
}
pub(in crate::workspace::stock) struct Retained {
    frame: ledger::Retained,
    value: StockReplayAdmissionReceipt,
    projection: Projection,
}
impl Retained {
    pub(in crate::workspace::stock) fn receipt(&self) -> StockReplayAdmissionReceipt {
        self.value
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
            delivery: None,
            envelope: None,
            aborted: None,
            preparation: None,
            local_record: None,
            installation: None,
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
pub(in crate::workspace::stock) async fn read_entry(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    entry: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let Some(frame) = ledger::read(connection, input, crypto, entry).await? else {
        return Ok(None);
    };
    let raw = frame.body.as_slice();
    if frame.header.family != ledger::Family::ReplayAdmission
        || raw.len() != BYTES
        || &raw[..8] != MAGIC
    {
        return Err(OperationError::RecoveryRequired);
    }
    let array = |start, end| {
        raw[start..end]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)
    };
    let original = Box::pin(claim::read_entry(
        connection,
        input,
        crypto,
        OperationId::from_bytes(array(8, 40)?),
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    let at = UnixSeconds::new(u64::from_be_bytes(
        raw[136..144]
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    ))
    .map_err(|_| OperationError::RecoveryRequired)?;
    let authorization = StockAuthorizationRef::from_host_metadata(
        Sequence::new(u64::from_be_bytes(
            raw[144..152]
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        ))
        .map_err(|_| OperationError::RecoveryRequired)?,
        CheckpointDigest::from_bytes(array(152, 184)?),
    )?;
    if original.frame.header.transaction != TransactionId::from_bytes(array(40, 72)?)
        || original.frame.header.head != CheckpointDigest::from_bytes(array(72, 104)?)
        || original.report.binding().digest() != &array(104, 136)?
        || frame.header.actor.participant() != original.frame.header.actor.participant()
        || frame.header.related_operation != original.frame.header.related_operation
        || frame.header.order != original.frame.header.order
        || frame.header.digest != original.frame.header.digest
        || frame.header.revision <= original.frame.header.revision
        || original.report.bound_plan().plan().replay()
            != radroots_protocol::intent::SigningReplay::LocalReplaySafe
        || at < original.report.claimed_at()
        || at < original.report.bound_plan().bound_at()
        || at
            >= original
                .report
                .bound_plan()
                .preparation()
                .first_issue_deadline()
        || head(&frame.header, raw) != frame.header.head
    {
        return Err(OperationError::RecoveryRequired);
    }
    let projection = Projection {
        operation: original.report.attempt().operation_id(),
        recipient: original.report.recipient(),
        kind: ProjectionKind::ReplayAdmission,
    };
    let value = receipt(input, &frame.header, &original, at, authorization)?;
    let retained = Retained {
        frame,
        value,
        projection,
    };
    retained.confirm(connection).await?;
    Ok(Some(retained))
}
pub(in crate::workspace::stock) async fn lookup(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    operation: OperationId,
    recipient: PublicKey,
) -> Result<Option<Retained>, OperationError> {
    let entry:Option<Vec<u8>>=sqlx::query_scalar("SELECT CASE WHEN length(entry_id)=32 THEN entry_id END FROM stock_replay_admissions WHERE actor=? AND operation_id=? AND recipient=?")
        .bind(input.participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(entry) = entry else { return Ok(None) };
    let retained = Box::pin(read_entry(
        connection,
        input,
        crypto,
        OperationId::from_bytes(
            entry
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        ),
    ))
    .await?
    .ok_or(OperationError::RecoveryRequired)?;
    if retained.value.attempt.operation_id() != operation || retained.value.recipient != recipient {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(retained))
}
pub(super) struct Pending {
    old: Option<Retained>,
    frame: Option<ledger::Prepared>,
    root: Option<Vec<u8>>,
    value: StockReplayAdmissionReceipt,
}
impl Pending {
    pub(super) fn receipt(&self) -> StockReplayAdmissionReceipt {
        self.value
    }
    /// Called only after current authorization and all original witnesses have
    /// been rechecked in the same transaction, with no provider IO inside it.
    pub(super) async fn apply(
        &self,
        connection: &mut SqliteConnection,
        input: &AttachmentInput,
        auth: &signing_plan::Authorized,
    ) -> Result<(), OperationError> {
        if let Some(old) = &self.old {
            old.confirm(connection).await?;
        }
        if let Some(frame) = &self.frame {
            frame.insert_record(connection).await?;
            sqlx::query("INSERT INTO stock_replay_admissions(actor,operation_id,recipient,entry_id) VALUES(?,?,?,?)")
                .bind(input.participant().as_bytes().as_slice()).bind(self.value.attempt.operation_id().as_bytes().as_slice()).bind(self.value.recipient.as_bytes().as_slice()).bind(frame.header.operation.as_bytes().as_slice())
                .execute(&mut *connection).await.map_err(|_|OperationError::StorageFailure)?;
            let changed = sqlx::query(
                "UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?",
            )
            .bind(self.root.as_ref().ok_or(OperationError::RecoveryRequired)?)
            .bind(&auth.access.root)
            .execute(connection)
            .await
            .map_err(|_| OperationError::StorageFailure)?;
            if changed.rows_affected() != 1 {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(())
    }
    pub(super) fn writes(&self) -> bool {
        self.frame.is_some()
    }
}
pub(super) async fn prepare(
    turn: &mut Turn<'_>,
    auth: &signing_plan::Authorized,
    original: &claim::codec::Retained,
    at: UnixSeconds,
) -> Result<Pending, OperationError> {
    let old = Box::pin(lookup(
        turn.connection,
        turn.selected,
        turn.crypto,
        original.report.attempt().operation_id(),
        original.report.recipient(),
    ))
    .await?;
    if let Some(old) = old {
        if at < old.value.observed_at {
            return Err(WorkspaceError::Unavailable.into());
        }
        if old.value.claim != original.report.attempt_id() {
            return Err(OperationError::RecoveryRequired);
        }
        let value = old.value;
        return Ok(Pending {
            old: Some(old),
            frame: None,
            root: None,
            value,
        });
    }
    let before = auth
        .access
        .authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let authorization = auth
        .access
        .authority
        .authorization
        .ok_or(OperationError::RecoveryRequired)?;
    let revision = Sequence::new(
        before
            .revision
            .get()
            .checked_add(1)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?;
    let bytes = body(original, at, authorization);
    let mut header = ledger::Header {
        version: 5,
        family: ledger::Family::ReplayAdmission,
        namespace: SellerAuthoritySelection::from_attachment(turn.selected),
        actor: auth.access.current.actor(),
        operation: OperationId::from_bytes(super::super::super::random()?),
        digest: original.report.attempt().command_digest(),
        transaction: TransactionId::from_bytes(super::super::super::random()?),
        revision,
        previous: before.head,
        head: CheckpointDigest::from_bytes([0; 32]),
        pool: None,
        order: original.frame.header.order,
        related_operation: Some(original.report.attempt().operation_id()),
        subject_grant: None,
        bytes: 0,
        parts: 0,
        body_digest: [0; 32],
    };
    header.head = head(&header, &bytes);
    let value = receipt(turn.selected, &header, original, at, authorization)?;
    let frame = ledger::prepare(turn.crypto, turn.selected, header, &bytes).await?;
    let mut next = auth.access.authority.clone();
    next.inventory = Some(super::super::InventoryCheckpoint {
        revision,
        head: Some(frame.header.head),
        ..before
    });
    let root = turn
        .crypto
        .seal_stock_root(turn.selected, next.catalog, &next.encode()?)
        .await?;
    Ok(Pending {
        old: None,
        frame: Some(frame),
        root: Some(root),
        value,
    })
}

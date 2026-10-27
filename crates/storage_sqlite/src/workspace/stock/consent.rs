//! Explicit per-operation business scopes. Preparation is untrusted data;
//! only an authenticated administrative transaction can issue these scopes.
#[cfg(test)]
mod tests;
use super::*;
use radroots_domain::{ActorRole, CommandDigest, OperationId, OrderId, UnixSeconds};
use radroots_protocol::command::CapturedOrderAcceptance;
use radroots_storage::{
    command::OperationError, inventory::InventoryCheckpointRef,
    workspace::seller_stock::StockResources,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockConsentMode {
    /// Explicit consent to this exact acceptance and access to its retained
    /// operation. This supplies no signer or publication capability.
    ExecuteAndRecover,
    /// Access to an existing original operation under the named current grant;
    /// it can never admit a missing operation or acquire fresh stock.
    RecoverOnly,
}

/// Prepared before store IO. No constructor verifies consent or issues rights.
pub struct StockConsentCapture {
    namespace: SellerAuthoritySelection,
    operation: OperationId,
    transaction: TransactionId,
    subject: GrantId,
    mode: StockConsentMode,
    resources: StockResources,
    expires: UnixSeconds,
    command: CapturedOrderAcceptance,
    digest: CommandDigest,
}
impl StockConsentCapture {
    pub fn prepare(
        namespace: SellerAuthoritySelection,
        subject: GrantId,
        mode: StockConsentMode,
        resources: StockResources,
        expires: UnixSeconds,
        command: CapturedOrderAcceptance,
    ) -> Result<Self, OperationError> {
        validate_capture(&namespace, subject, mode, expires, &command)?;
        let digest = scope_digest(
            &namespace,
            subject,
            mode,
            &resources,
            expires,
            command.digest(),
        )?;
        let operation = OperationId::from_bytes(super::super::random()?);
        if operation == command.context().operation_id() {
            return Err(WorkspaceError::Unavailable.into());
        }
        Ok(Self {
            namespace,
            operation,
            transaction: TransactionId::from_bytes(super::super::random()?),
            subject,
            mode,
            resources,
            expires,
            command,
            digest,
        })
    }
    pub const fn control_operation_id(&self) -> OperationId {
        self.operation
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn subject_grant(&self) -> GrantId {
        self.subject
    }
    pub const fn mode(&self) -> StockConsentMode {
        self.mode
    }
    pub const fn expires_at(&self) -> UnixSeconds {
        self.expires
    }
    pub fn command(&self) -> &CapturedOrderAcceptance {
        &self.command
    }
    pub fn resources(&self) -> &StockResources {
        &self.resources
    }
    pub(super) fn retained_copy(&self) -> Result<Self, OperationError> {
        Ok(Self {
            namespace: self.namespace.clone(),
            operation: self.operation,
            transaction: self.transaction,
            subject: self.subject,
            mode: self.mode,
            resources: self.resources.clone(),
            expires: self.expires,
            command: CapturedOrderAcceptance::parse(self.command.protected_wire())
                .map_err(|_| OperationError::InvalidCommand)?,
            digest: self.digest,
        })
    }
}
impl std::fmt::Debug for StockConsentCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockConsentCapture(<unissued scope>)")
    }
}
fn validate_capture(
    namespace: &SellerAuthoritySelection,
    subject: GrantId,
    mode: StockConsentMode,
    expires: UnixSeconds,
    command: &CapturedOrderAcceptance,
) -> Result<(), OperationError> {
    let context = command.context();
    let approval = context
        .approval()
        .ok_or(OperationError::UnverifiedApproval)?;
    if context.actor().participant() != namespace.participant()
        || context.actor().role() != ActorRole::Seller
        || context.actor().organization().is_some()
        || context.actor().grant_id().is_none()
        || context.workspace_id() != namespace.workspace_id()
        || context.authority() != namespace.authority()
        || context.route_digest() != namespace.route_digest()
        || context.expected().is_none()
        || expires.get() == 0
        || approval.scope() != "orders.accept"
        || approval.intent_digest() != command.approval_intent_digest()
        || (mode == StockConsentMode::ExecuteAndRecover
            && (context.actor().grant_id() != Some(subject)
                || expires > context.not_after()
                || expires > approval.expires_at()))
    {
        return Err(OperationError::InvalidCommand);
    }
    Ok(())
}
fn scope_digest(
    namespace: &SellerAuthoritySelection,
    subject: GrantId,
    mode: StockConsentMode,
    resources: &StockResources,
    expires: UnixSeconds,
    command: CommandDigest,
) -> Result<CommandDigest, OperationError> {
    #[derive(Serialize)]
    struct Scope<'a> {
        version: u8,
        namespace: &'a SellerAuthoritySelection,
        subject: GrantId,
        mode: StockConsentMode,
        resources: &'a StockResources,
        expires: UnixSeconds,
        command: CommandDigest,
    }
    let bytes = Zeroizing::new(
        serde_json::to_vec(&Scope {
            version: 1,
            namespace,
            subject,
            mode,
            resources,
            expires,
            command,
        })
        .map_err(|_| OperationError::InvalidCommand)?,
    );
    if bytes.len() > 16 * 1024 {
        return Err(WorkspaceError::Capacity.into());
    }
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.consent_scope.v1\0");
    hash.update(bytes.as_slice());
    Ok(CommandDigest::from_bytes(hash.finalize().into()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockConsentReceipt {
    operation: OperationId,
    transaction: TransactionId,
    subject: GrantId,
    command_operation: OperationId,
    command_digest: CommandDigest,
    order: OrderId,
    mode: StockConsentMode,
    expires: UnixSeconds,
    resources: StockResources,
    checkpoint: InventoryCheckpointRef,
}
impl StockConsentReceipt {
    /// The exact resource claim at issuance, without current execution authority.
    pub fn resources(&self) -> &StockResources {
        &self.resources
    }
    pub const fn control_operation_id(&self) -> OperationId {
        self.operation
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn subject_grant(&self) -> GrantId {
        self.subject
    }
    pub const fn command_operation_id(&self) -> OperationId {
        self.command_operation
    }
    pub const fn command_digest(&self) -> CommandDigest {
        self.command_digest
    }
    pub const fn order_id(&self) -> OrderId {
        self.order
    }
    pub const fn mode(&self) -> StockConsentMode {
        self.mode
    }
    pub const fn expires_at(&self) -> UnixSeconds {
        self.expires
    }
    pub const fn checkpoint(&self) -> InventoryCheckpointRef {
        self.checkpoint
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StockConsentOutcome {
    Committed(Box<StockConsentReceipt>),
    Unknown {
        control_operation: OperationId,
        transaction: TransactionId,
        command_operation: OperationId,
        subject: GrantId,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body<'a> {
    version: u8,
    selection: AttachmentInput,
    principal: String,
    grant_revision: Sequence,
    grant_expires: UnixSeconds,
    observed_at: UnixSeconds,
    authorization: StockAuthorizationRef,
    minimum_revision: Sequence,
    #[serde(deserialize_with = "super::required_nullable")]
    minimum_head: Option<radroots_domain::CheckpointDigest>,
    subject: GrantId,
    subject_digest: [u8; 32],
    mode: StockConsentMode,
    resources: StockResources,
    expires: UnixSeconds,
    #[serde(borrow)]
    command: &'a serde_json::value::RawValue,
}
pub(super) struct Retained {
    pub frame: ledger::Retained,
    pub command: CapturedOrderAcceptance,
    pub mode: StockConsentMode,
    pub resources: StockResources,
    pub subject: GrantId,
    pub expires: UnixSeconds,
    pub authorization: StockAuthorizationRef,
    pub subject_digest: [u8; 32],
}
impl Retained {
    pub(super) fn identity(&self) -> inventory::checkpoint::EntryIdentity {
        inventory::checkpoint::EntryIdentity {
            operation: self.frame.header.operation,
            revision: self.frame.header.revision,
            head: self.frame.header.head,
            protected_digest: Sha256::digest(&self.frame.protected).into(),
        }
    }
    pub fn receipt(&self) -> Result<StockConsentReceipt, OperationError> {
        let header = &self.frame.header;
        Ok(StockConsentReceipt {
            operation: header.operation,
            transaction: header.transaction,
            subject: self.subject,
            command_operation: self.command.context().operation_id(),
            command_digest: self.command.digest(),
            order: header.order.ok_or(OperationError::RecoveryRequired)?,
            mode: self.mode,
            expires: self.expires,
            resources: self.resources.clone(),
            checkpoint: InventoryCheckpointRef::from_host_metadata(
                self.command.context().authority(),
                header.revision,
                Some(header.head),
            )?
            .with_authorization(self.authorization),
        })
    }
}
fn chain_head(
    namespace: &SellerAuthoritySelection,
    header: &ledger::Header,
    subject_digest: [u8; 32],
) -> Result<radroots_domain::CheckpointDigest, OperationError> {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.consent_chain.v1\0");
    hash.update(namespace.workspace_id().as_bytes());
    hash.update(namespace.authority().executor_id().as_bytes());
    hash.update(namespace.authority().generation().get().to_be_bytes());
    hash.update(namespace.route_digest().as_bytes());
    hash.update([u8::from(header.previous.is_some())]);
    hash.update(header.previous.map(|v| *v.as_bytes()).unwrap_or([0; 32]));
    hash.update(header.revision.get().to_be_bytes());
    hash.update(header.actor.participant().as_bytes());
    hash.update(
        header
            .actor
            .grant_id()
            .ok_or(OperationError::RecoveryRequired)?
            .as_bytes(),
    );
    hash.update(header.operation.as_bytes());
    hash.update(header.transaction.as_bytes());
    hash.update(header.digest.as_bytes());
    hash.update(subject_digest);
    Ok(radroots_domain::CheckpointDigest::from_bytes(
        hash.finalize().into(),
    ))
}
pub(super) async fn read(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    administrator: GrantId,
    operation: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let Some(frame) = ledger::read(connection, input, crypto, operation).await? else {
        return Ok(None);
    };
    let mut parser = serde_json::Deserializer::from_slice(frame.body.as_slice());
    let body: Body<'_> =
        super::object(&mut parser).map_err(|_| OperationError::RecoveryRequired)?;
    parser.end().map_err(|_| OperationError::RecoveryRequired)?;
    let command = CapturedOrderAcceptance::parse(body.command.get().as_bytes())
        .map_err(|_| OperationError::RecoveryRequired)?;
    let namespace = SellerAuthoritySelection::from_attachment(input);
    validate_capture(&namespace, body.subject, body.mode, body.expires, &command)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let header = &frame.header;
    if header.family != ledger::Family::Consent
        || header.actor.grant_id() != Some(administrator)
        || body.version != 1
        || !namespace.matches_attachment(&body.selection)
        || body.selection.grant_id() != administrator
        || header.subject_grant != Some(body.subject)
        || header.related_operation != Some(command.context().operation_id())
        || header.order != command.context().expected().map(|v| v.order_id())
        || body.principal.is_empty()
        || body.principal.len() > 128
        || body.principal.chars().any(char::is_control)
        || body.grant_revision.get() == 0
        || body.observed_at.get() == 0
        || body.grant_expires <= body.observed_at
        || body.expires <= body.observed_at
        || body.expires > body.grant_expires
        || body.minimum_revision.get() >= header.revision.get()
        || (body.minimum_revision.get() == 0) != body.minimum_head.is_none()
        || header.digest
            != scope_digest(
                &namespace,
                body.subject,
                body.mode,
                &body.resources,
                body.expires,
                command.digest(),
            )?
        || header.head != chain_head(&namespace, header, body.subject_digest)?
    {
        return Err(OperationError::RecoveryRequired);
    }
    let mode = body.mode;
    let resources = body.resources;
    let subject = body.subject;
    let expires = body.expires;
    let authorization = body.authorization;
    let subject_digest = body.subject_digest;
    Ok(Some(Retained {
        frame,
        command,
        mode,
        resources,
        subject,
        expires,
        authorization,
        subject_digest,
    }))
}

pub(super) async fn natural(
    connection: &mut SqliteConnection,
    participant: radroots_domain::PublicKey,
    operation: OperationId,
    subject: GrantId,
) -> Result<Option<OperationId>, OperationError> {
    let id:Option<Vec<u8>>=sqlx::query_scalar("SELECT operation_id FROM stock_operations WHERE actor=? AND family=1 AND related_operation=? AND subject_grant=?")
        .bind(participant.as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(subject.as_bytes().as_slice())
        .fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    id.map(|bytes| {
        bytes
            .try_into()
            .map(OperationId::from_bytes)
            .map_err(|_| OperationError::RecoveryRequired)
    })
    .transpose()
}
pub(in crate::workspace) async fn issue(
    turn: &mut inventory::Turn<'_>,
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    capture: StockConsentCapture,
    proof: AttachmentProof,
) -> Result<StockConsentOutcome, OperationError> {
    let access = turn.access(&binding, minimum, &proof).await?;
    if !matches!(access.authority.catalog, 7..=15) {
        return Err(OperationError::UnsupportedOperation);
    }
    if !access.record.administrator()
        || capture.namespace != SellerAuthoritySelection::from_attachment(turn.selected)
    {
        return Err(OperationError::Unauthorized);
    }
    let command_operation = capture.command.context().operation_id();
    if let Some(operation) = natural(
        turn.connection,
        turn.selected.participant(),
        command_operation,
        capture.subject,
    )
    .await?
    {
        let original = read(
            turn.connection,
            turn.selected,
            turn.crypto,
            access.authority.input.grant_id(),
            operation,
        )
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        let identity = inventory::checkpoint::authenticate_identity(
            turn.connection,
            turn.selected,
            turn.crypto,
            &access.current,
            &access.authority,
            inventory::checkpoint::IdentityTarget::Consent {
                operation: command_operation,
                subject: capture.subject,
                expected: Some(original.identity()),
            },
        )
        .await?;
        if original.frame.header.digest != capture.digest {
            return Err(OperationError::IdempotencyConflict);
        }
        let receipt = original.receipt()?;
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
            ledger::confirm_header(&mut tx, &original.frame.header, &original.frame.protected)
                .await?;
            ledger::confirm_parts(
                &mut tx,
                original.frame.header.actor,
                operation,
                original.frame.parts_witness,
                original.frame.header.parts,
            )
            .await?;
            Ok(())
        }
        .await;
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Err(OperationError::RecoveryRequired);
        }
        checked?;
        return Ok(StockConsentOutcome::Committed(Box::new(receipt)));
    }
    let identity = inventory::checkpoint::authenticate_identity(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.current,
        &access.authority,
        inventory::checkpoint::IdentityTarget::Consent {
            operation: command_operation,
            subject: capture.subject,
            expected: None,
        },
    )
    .await?;
    if access.current.observed_at() >= capture.expires
        || capture.expires > access.current.expires_at()
    {
        return Err(OperationError::DeadlineExceeded);
    }
    let target:Vec<u8>=sqlx::query_scalar("SELECT protected FROM stock_grants WHERE grant_id=? AND length(protected) BETWEEN 1 AND 262144")
        .bind(capture.subject.as_bytes().as_slice()).fetch_optional(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::Unauthorized)?;
    let target_input = capture.namespace.attachment(capture.subject)?;
    let clear = turn
        .crypto
        .open_stock_record(&target_input, "grant", capture.subject.as_bytes(), &target)
        .await?;
    let target_record = clear.expose_secret(StockGrantRecord::decode)?;
    if target_record.input() != &target_input
        || !target_record.belongs_to(&capture.namespace)?
        || target_record.is_revoked()
        || capture.expires > target_record.expires_at()
    {
        return Err(OperationError::Unauthorized);
    }
    let subject_digest: [u8; 32] = Sha256::digest(&target).into();
    let before = access
        .authority
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
    let authorization = access
        .authority
        .authorization
        .ok_or(OperationError::RecoveryRequired)?;
    let command: &serde_json::value::RawValue =
        serde_json::from_slice(capture.command.protected_wire())
            .map_err(|_| OperationError::InvalidCommand)?;
    let body = Body {
        version: 1,
        selection: access.current.input().clone(),
        principal: access.current.principal_id().into(),
        grant_revision: access.current.grant_revision(),
        grant_expires: access.current.expires_at(),
        observed_at: access.current.observed_at(),
        authorization,
        minimum_revision: minimum.revision(),
        minimum_head: minimum.head(),
        subject: capture.subject,
        subject_digest,
        mode: capture.mode,
        resources: capture.resources.clone(),
        expires: capture.expires,
        command,
    };
    let body =
        Zeroizing::new(serde_json::to_vec(&body).map_err(|_| OperationError::StorageFailure)?);
    let mut header = ledger::Header {
        version: 5,
        family: ledger::Family::Consent,
        namespace: capture.namespace.clone(),
        actor: access.current.actor(),
        operation: capture.operation,
        digest: capture.digest,
        transaction: capture.transaction,
        revision,
        previous: before.head,
        head: radroots_domain::CheckpointDigest::from_bytes([0; 32]),
        pool: None,
        order: capture
            .command
            .context()
            .expected()
            .map(|value| value.order_id()),
        related_operation: Some(command_operation),
        subject_grant: Some(capture.subject),
        bytes: 0,
        parts: 0,
        body_digest: [0; 32],
    };
    header.head = chain_head(&capture.namespace, &header, subject_digest)?;
    let head = header.head;
    let prepared = ledger::prepare(turn.crypto, turn.selected, header, &body).await?;
    let mut next = access.authority.clone();
    next.inventory = Some(InventoryCheckpoint {
        revision,
        head: Some(head),
        ..before
    });
    let root = turn
        .crypto
        .seal_stock_root(turn.selected, access.authority.catalog, &next.encode()?)
        .await?;
    let receipt = StockConsentReceipt {
        operation: capture.operation,
        transaction: capture.transaction,
        subject: capture.subject,
        command_operation,
        command_digest: capture.command.digest(),
        order: capture
            .command
            .context()
            .expected()
            .ok_or(OperationError::InvalidCommand)?
            .order_id(),
        mode: capture.mode,
        expires: capture.expires,
        resources: capture.resources.clone(),
        checkpoint: InventoryCheckpointRef::from_host_metadata(
            turn.selected.authority(),
            revision,
            Some(head),
        )?
        .with_authorization(authorization),
    };
    turn.namespace.validate()?;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    let changed:Result<(),OperationError>=async {
        let current=access.confirm(&mut tx,turn.namespace,turn.clock,&proof).await?;
        identity.confirm(&mut tx).await?;
        if !access.record.administrator() {return Err(OperationError::Unauthorized);}
        if current.observed_at()>=capture.expires {return Err(OperationError::DeadlineExceeded);}
        let target_matches:i64=sqlx::query_scalar("SELECT count(*) FROM stock_grants WHERE grant_id=? AND protected=?")
            .bind(capture.subject.as_bytes().as_slice()).bind(&target).fetch_one(&mut *tx).await.map_err(|_|OperationError::RecoveryRequired)?;
        if target_matches!=1 {return Err(OperationError::RecoveryRequired);}
        if natural(&mut tx,turn.selected.participant(),command_operation,capture.subject).await?.is_some() {return Err(OperationError::StaleState);}
        let absent:i64=sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE actor=? AND operation_id=?")
            .bind(current.actor().participant().as_bytes().as_slice()).bind(capture.operation.as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::RecoveryRequired)?;
        if absent!=0 {return Err(OperationError::IdempotencyConflict);}
        sqlx::query("INSERT INTO stock_operations(actor,grant_id,operation_id,command_digest,transaction_id,pool_id,revision,chain_head,protected,family,order_id,related_operation,subject_grant) VALUES(?,?,?,?,?,NULL,?,?,?,1,?,?,?)")
            .bind(current.actor().participant().as_bytes().as_slice()).bind(current.input().grant_id().as_bytes().as_slice())
            .bind(capture.operation.as_bytes().as_slice()).bind(capture.digest.as_bytes().as_slice()).bind(capture.transaction.as_bytes().as_slice())
            .bind(revision.get() as i64).bind(head.as_bytes().as_slice()).bind(&prepared.protected)
            .bind(receipt.order.as_bytes().as_slice()).bind(command_operation.as_bytes().as_slice()).bind(capture.subject.as_bytes().as_slice())
            .execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
        prepared.insert_parts(&mut tx,current.actor(),capture.operation).await?;
        let updated=sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?")
            .bind(root).bind(&access.root).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
        if updated.rows_affected()!=1 {return Err(OperationError::RecoveryRequired);}turn.namespace.validate()?;Ok(())
    }.await;
    let unknown = || StockConsentOutcome::Unknown {
        control_operation: capture.operation,
        transaction: capture.transaction,
        command_operation,
        subject: capture.subject,
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
    Ok(StockConsentOutcome::Committed(Box::new(receipt)))
}

pub(in crate::workspace) async fn query(
    turn: &mut inventory::Turn<'_>,
    binding: WorkspaceBinding,
    minimum: InventoryCheckpointRef,
    command_operation: OperationId,
    subject: GrantId,
    proof: AttachmentProof,
) -> Result<Option<StockConsentReceipt>, OperationError> {
    let access = turn.access(&binding, minimum, &proof).await?;
    if !matches!(access.authority.catalog, 7..=15) {
        return Err(OperationError::UnsupportedOperation);
    }
    if !access.record.administrator() && access.current.input().grant_id() != subject {
        return Err(OperationError::Unauthorized);
    }
    let original = if let Some(operation) = natural(
        turn.connection,
        turn.selected.participant(),
        command_operation,
        subject,
    )
    .await?
    {
        Some(
            read(
                turn.connection,
                turn.selected,
                turn.crypto,
                access.authority.input.grant_id(),
                operation,
            )
            .await?
            .ok_or(OperationError::RecoveryRequired)?,
        )
    } else {
        None
    };
    let identity = inventory::checkpoint::authenticate_identity(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.current,
        &access.authority,
        inventory::checkpoint::IdentityTarget::Consent {
            operation: command_operation,
            subject,
            expected: original.as_ref().map(Retained::identity),
        },
    )
    .await?;
    let receipt = original.as_ref().map(Retained::receipt).transpose()?;
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
        if let Some(original) = &original {
            ledger::confirm_header(&mut tx, &original.frame.header, &original.frame.protected)
                .await?;
            ledger::confirm_parts(
                &mut tx,
                original.frame.header.actor,
                original.frame.header.operation,
                original.frame.parts_witness,
                original.frame.header.parts,
            )
            .await?;
        }
        turn.namespace.validate()?;
        Ok(())
    }
    .await;
    if tx.rollback().await.is_err() {
        *turn.uncertain = true;
        return Err(OperationError::RecoveryRequired);
    }
    checked?;
    Ok(receipt)
}

impl NativeWorkspace {
    /// Issues an explicit scope for one captured business command, using the
    /// current administrative grant. The borrowed capture preserves its original
    /// recovery identity if this observer is lost. Issuance performs no order,
    /// stock, signing or publication effect.
    pub async fn issue_stock_consent(
        &self,
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        capture: &StockConsentCapture,
        proof: &AttachmentProof,
    ) -> Result<StockConsentOutcome, OperationError> {
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker
            .issue_stock_consent(binding, minimum, capture.retained_copy()?, proof)
            .await
    }

    /// Reads a retained issuance under current administrator or subject access.
    /// A receipt describes its original issuance; it does not establish that its
    /// expiry, target grant or business preconditions still permit execution.
    pub async fn stock_consent(
        &self,
        binding: WorkspaceBinding,
        minimum: InventoryCheckpointRef,
        command_operation: OperationId,
        subject: GrantId,
        proof: &AttachmentProof,
    ) -> Result<Option<StockConsentReceipt>, OperationError> {
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker
            .stock_consent(binding, minimum, command_operation, subject, proof)
            .await
    }
}

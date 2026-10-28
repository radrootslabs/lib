use super::*;

/// Constant-memory witness of the exact protected ancestry checked before IO
/// enters the write transaction. The transaction rereads every bound SQL row;
/// no key-provider callback or decryption runs while it owns the SQL lock.
pub(in crate::workspace::stock) struct MinimumProof {
    lower: u64,
    upper: u64,
    digest: [u8; 32],
    multipart: bool,
}
impl MinimumProof {
    pub(in crate::workspace::stock) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
        let mut hash = history_hash(self.multipart);
        for revision in (self.lower..=self.upper).rev() {
            let row=sqlx::query("SELECT actor,grant_id,operation_id,command_digest,transaction_id,pool_id,chain_head,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_operations WHERE revision=?")
                .bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
            hash.update(revision.to_be_bytes());
            let mut actor = [0_u8; 32];
            let mut operation = [0_u8; 32];
            for column in 0..7 {
                if column == 5 {
                    let pool: Option<Vec<u8>> = row
                        .try_get(column)
                        .map_err(|_| OperationError::RecoveryRequired)?;
                    if !self.multipart && pool.is_none() {
                        return Err(OperationError::RecoveryRequired);
                    }
                    hash_optional(&mut hash, pool.as_deref())?;
                    continue;
                }
                let bytes: Vec<u8> = row
                    .try_get(column)
                    .map_err(|_| OperationError::RecoveryRequired)?;
                if bytes.len() != 32 {
                    return Err(OperationError::RecoveryRequired);
                }
                if column == 0 {
                    actor.copy_from_slice(&bytes);
                }
                if column == 2 {
                    operation.copy_from_slice(&bytes);
                }
                hash.update(bytes);
            }
            let protected: Vec<u8> = row
                .try_get::<Option<Vec<u8>>, _>(7)
                .map_err(|_| OperationError::RecoveryRequired)?
                .ok_or(OperationError::RecoveryRequired)?;
            hash.update((protected.len() as u64).to_be_bytes());
            hash.update(protected);
            if self.multipart {
                let row=sqlx::query("SELECT family,order_id,related_operation,subject_grant FROM stock_operations WHERE actor=? AND operation_id=?")
                    .bind(actor.as_slice()).bind(operation.as_slice()).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
                let family: i64 = row
                    .try_get(0)
                    .map_err(|_| OperationError::RecoveryRequired)?;
                let order: Option<Vec<u8>> = row
                    .try_get(1)
                    .map_err(|_| OperationError::RecoveryRequired)?;
                let related: Option<Vec<u8>> = row
                    .try_get(2)
                    .map_err(|_| OperationError::RecoveryRequired)?;
                if !(0..=if catalog >= 16 {
                    10
                } else if catalog >= 15 {
                    9
                } else if catalog >= 14 {
                    8
                } else {
                    7
                })
                    .contains(&family)
                {
                    return Err(OperationError::RecoveryRequired);
                }
                let subject: Option<Vec<u8>> = row
                    .try_get(3)
                    .map_err(|_| OperationError::RecoveryRequired)?;
                hash.update(family.to_be_bytes());
                hash_optional(&mut hash, order.as_deref())?;
                hash_optional(&mut hash, related.as_deref())?;
                hash_optional(&mut hash, subject.as_deref())?;
                let (parts, witness) =
                    super::super::ledger::raw_parts_witness(connection, &actor, &operation).await?;
                hash.update([parts]);
                hash.update(witness);
                if family == 10 {
                    hash.update(
                        super::super::envelope::raw_projection(connection, &actor, &operation)
                            .await?,
                    );
                }
                if family == 7 {
                    hash.update(
                        super::abort::raw_projection(connection, &actor, &operation).await?,
                    );
                }
                if catalog >= 13 && family == 2 {
                    hash.update(b"P13");
                    hash.update(
                        super::acceptance::codec::raw_preparation(connection, &actor, &operation)
                            .await?,
                    );
                }
                if matches!(family, 4 | 5 | 8 | 9) {
                    hash.update(
                        super::super::issuance::raw_projection(
                            connection, &actor, &operation, family,
                        )
                        .await?,
                    );
                }
            }
            if self.multipart {
                let family: i64 = sqlx::query_scalar(
                    "SELECT family FROM stock_operations WHERE actor=? AND operation_id=?",
                )
                .bind(actor.as_slice())
                .bind(operation.as_slice())
                .fetch_one(&mut *connection)
                .await
                .map_err(|_| OperationError::RecoveryRequired)?;
                if family == 6 {
                    hash.update(
                        super::installation::raw_projection(connection, &actor, &operation).await?,
                    );
                }
            }
            hash_current_movements(&mut hash, connection, &actor, &operation).await?;
        }
        let actual: [u8; 32] = hash.finalize().into();
        if actual != self.digest {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
fn history_hash(multipart: bool) -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.ancestry_snapshot.v2\0");
    hash.update([u8::from(multipart)]);
    hash
}
/// Authenticate current order cache data from the complete protected chain.
/// The caller must retain this proof through the final SQL transaction.
pub(in crate::workspace::stock) async fn authenticate_order_state(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    authority: &Authority,
    order: radroots_domain::OrderId,
) -> Result<super::super::order_state::Checked, OperationError> {
    if !matches!(authority.catalog, 13..=16) {
        return Err(OperationError::UnsupportedOperation);
    }
    let current = authority
        .inventory
        .ok_or(OperationError::RecoveryRequired)?;
    let mut previous = current.head;
    let mut hash = history_hash(true);
    let mut chain = super::super::order_state::Chain::new();
    for revision in (1..=current.revision.get()).rev() {
        let id: Vec<u8> = sqlx::query_scalar("SELECT operation_id FROM stock_operations WHERE revision=? AND length(operation_id)=32")
            .bind(revision as i64).fetch_optional(&mut *connection).await
            .map_err(|_| OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let entry = Box::pin(super::super::ledger::history(
            connection,
            input,
            crypto,
            binding,
            authority,
            OperationId::from_bytes(
                id.try_into()
                    .map_err(|_| OperationError::RecoveryRequired)?,
            ),
        ))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        if entry.header.revision.get() != revision || Some(entry.header.head) != previous {
            return Err(OperationError::RecoveryRequired);
        }
        if entry.header.order == Some(order)
            && let Some(marker) = super::super::order_state::Marker::from_history(&entry)?
        {
            chain.observe(marker)?;
        }
        entry.hash(&mut hash)?;
        previous = entry.header.previous;
    }
    if previous.is_some() {
        return Err(OperationError::RecoveryRequired);
    }
    chain.complete()?;
    let witness = super::super::order_state::Witness {
        order,
        expected: chain.latest,
    };
    witness.confirm(connection).await?;
    Ok(super::super::order_state::Checked {
        witness,
        ancestry: MinimumProof {
            lower: 1,
            upper: current.revision.get(),
            digest: hash.finalize().into(),
            multipart: true,
        },
    })
}
/// Initial decisions must prove absence through authenticated effects, rather
/// than trusting the mutable order index or its current row count alone.
pub(super) async fn authenticate_order_absence(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    authority: &Authority,
    order: radroots_domain::OrderId,
) -> Result<MinimumProof, OperationError> {
    let current = authority
        .inventory
        .ok_or(OperationError::UnsupportedOperation)?;
    if !matches!(authority.catalog, 9..=16) {
        return Err(OperationError::UnsupportedOperation);
    }
    let upper = current.revision.get();
    let mut previous = current.head;
    let mut hash = history_hash(true);
    for revision in (1..=upper).rev() {
        let id: Vec<u8> = sqlx::query_scalar("SELECT operation_id FROM stock_operations WHERE revision=? AND length(operation_id)=32")
            .bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let operation = OperationId::from_bytes(
            id.try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        let entry = Box::pin(super::super::ledger::history(
            connection, input, crypto, binding, authority, operation,
        ))
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        if entry.header.revision.get() != revision || Some(entry.header.head) != previous {
            return Err(OperationError::RecoveryRequired);
        }
        if entry.header.family == super::super::ledger::Family::Acceptance
            && entry.header.order == Some(order)
        {
            return Err(OperationError::StaleState);
        }
        entry.hash(&mut hash)?;
        previous = entry.header.previous;
    }
    if previous.is_some() {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(MinimumProof {
        lower: 1,
        upper,
        digest: hash.finalize().into(),
        multipart: true,
    })
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::workspace::stock) struct EntryIdentity {
    pub operation: OperationId,
    pub revision: Sequence,
    pub head: CheckpointDigest,
    pub protected_digest: [u8; 32],
}
pub(in crate::workspace::stock) enum IdentityTarget {
    Abort {
        operation: OperationId,
        expected: Option<EntryIdentity>,
    },
    Operation(OperationId, Option<EntryIdentity>),
    Consent {
        operation: OperationId,
        subject: GrantId,
        expected: Option<EntryIdentity>,
    },
    SigningPlan {
        operation: OperationId,
        expected: Option<EntryIdentity>,
    },
    Installation {
        operation: OperationId,
        expected: Option<EntryIdentity>,
    },
    SigningClaim {
        operation: OperationId,
        recipient: radroots_domain::PublicKey,
        expected: Option<EntryIdentity>,
    },
}
pub(in crate::workspace::stock) async fn authenticate_identity(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    authority: &Authority,
    target: IdentityTarget,
) -> Result<MinimumProof, OperationError> {
    let current = authority
        .inventory
        .ok_or(OperationError::UnsupportedOperation)?;
    let multipart = matches!(authority.catalog, 7..=16);
    let upper = current.revision.get();
    let mut previous = current.head;
    let mut hash = history_hash(multipart);
    for revision in (1..=upper).rev() {
        let id:Vec<u8>=sqlx::query_scalar("SELECT operation_id FROM stock_operations WHERE revision=? AND length(operation_id)=32")
            .bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let operation = OperationId::from_bytes(
            id.try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        let (identity, predecessor, natural, signing_plan, signing_claim, installation, aborted) =
            if multipart {
                let entry = Box::pin(super::super::ledger::history(
                    connection, input, crypto, binding, authority, operation,
                ))
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
                if entry.header.revision.get() != revision || Some(entry.header.head) != previous {
                    return Err(OperationError::RecoveryRequired);
                }
                entry.hash(&mut hash)?;
                (
                    EntryIdentity {
                        operation,
                        revision: entry.header.revision,
                        head: entry.header.head,
                        protected_digest: Sha256::digest(&entry.protected).into(),
                    },
                    entry.header.previous,
                    if entry.header.family == super::super::ledger::Family::Consent {
                        entry
                            .header
                            .related_operation
                            .zip(entry.header.subject_grant)
                    } else {
                        None
                    },
                    if entry.header.family == super::super::ledger::Family::SigningPlan {
                        entry.header.related_operation
                    } else {
                        None
                    },
                    if entry.header.family == super::super::ledger::Family::SigningClaim {
                        entry
                            .signing
                            .map(|value| (value.operation, value.recipient))
                    } else {
                        None
                    },
                    entry.installation.map(|value| value.operation),
                    entry.aborted.map(|value| value.operation),
                )
            } else {
                let entry = read_operation(connection, input, crypto, binding, operation)
                    .await?
                    .ok_or(OperationError::RecoveryRequired)?;
                let checkpoint = entry.receipt.checkpoint();
                if checkpoint.revision().get() != revision || checkpoint.head() != previous {
                    return Err(OperationError::RecoveryRequired);
                }
                hash_retained(&mut hash, revision, binding, operation, &entry)?;
                (
                    EntryIdentity {
                        operation,
                        revision: checkpoint.revision(),
                        head: checkpoint.head().ok_or(OperationError::RecoveryRequired)?,
                        protected_digest: Sha256::digest(&entry.protected).into(),
                    },
                    entry.previous_head,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            };
        let (matched, expected) = match &target {
            IdentityTarget::Abort {
                operation: wanted,
                expected,
            } => (aborted == Some(*wanted), *expected),
            IdentityTarget::Operation(wanted, expected) => (*wanted == operation, *expected),
            IdentityTarget::Consent {
                operation: wanted,
                subject,
                expected,
            } => (natural == Some((*wanted, *subject)), *expected),
            IdentityTarget::SigningPlan {
                operation: wanted,
                expected,
            } => (signing_plan == Some(*wanted), *expected),
            IdentityTarget::Installation {
                operation: wanted,
                expected,
            } => (installation == Some(*wanted), *expected),
            IdentityTarget::SigningClaim {
                operation: wanted,
                recipient,
                expected,
            } => (signing_claim == Some((*wanted, *recipient)), *expected),
        };
        if matched {
            if expected != Some(identity) {
                return Err(OperationError::RecoveryRequired);
            }
            return Ok(MinimumProof {
                lower: revision,
                upper,
                digest: hash.finalize().into(),
                multipart,
            });
        }
        previous = predecessor;
    }
    let expected = match target {
        IdentityTarget::Operation(_, expected)
        | IdentityTarget::Consent { expected, .. }
        | IdentityTarget::SigningPlan { expected, .. }
        | IdentityTarget::SigningClaim { expected, .. }
        | IdentityTarget::Abort { expected, .. }
        | IdentityTarget::Installation { expected, .. } => expected,
    };
    if previous.is_some() || expected.is_some() {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(MinimumProof {
        lower: 1,
        upper,
        digest: hash.finalize().into(),
        multipart,
    })
}
pub(in crate::workspace::stock) async fn authenticate_minimum(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    authority: &Authority,
    minimum: radroots_storage::inventory::InventoryCheckpointRef,
) -> Result<MinimumProof, OperationError> {
    Box::pin(authenticate_minimum_with_local(
        connection, input, crypto, binding, authority, minimum, None,
    ))
    .await
}
pub(in crate::workspace::stock) struct LocalSources<'a> {
    pub scope: radroots_domain::causal::CausalScope,
    pub records: &'a mut std::collections::BTreeMap<
        radroots_domain::RecordId,
        radroots_protocol::commerce::DecodedRecord,
    >,
}
pub(in crate::workspace::stock) async fn authenticate_minimum_with_local(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    authority: &Authority,
    minimum: radroots_storage::inventory::InventoryCheckpointRef,
    mut local: Option<LocalSources<'_>>,
) -> Result<MinimumProof, OperationError> {
    let current = authority
        .inventory
        .ok_or(OperationError::UnsupportedOperation)?;
    if minimum.authority() != input.authority() {
        return Err(WorkspaceError::StaleAuthority.into());
    }
    if minimum.revision().get() > current.revision.get() {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    let lower = if local.is_some() {
        1
    } else {
        minimum.revision().get().max(1)
    };
    let upper = current.revision.get();
    let multipart = matches!(authority.catalog, 7..=16);
    let mut hash = history_hash(multipart);
    let mut expected = current.head;
    for revision in (lower..=upper).rev() {
        let id:Vec<u8>=sqlx::query_scalar("SELECT operation_id FROM stock_operations WHERE revision=? AND length(operation_id)=32")
            .bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let operation = OperationId::from_bytes(
            id.try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        if multipart {
            let mut retained = Box::pin(super::super::ledger::history(
                connection, input, crypto, binding, authority, operation,
            ))
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
            if retained.header.revision.get() != revision || Some(retained.header.head) != expected
            {
                return Err(WorkspaceError::StaleCheckpoint.into());
            }
            if revision == minimum.revision().get() && expected != minimum.head() {
                return Err(WorkspaceError::StaleCheckpoint.into());
            }
            retained.hash(&mut hash)?;
            if let Some(local) = &mut local
                && let Some(record) = retained.local_record.take()
                && local.scope.contains(record.content())
            {
                let id = record.record_id();
                if let Some(known) = local.records.get(&id) {
                    if known.content() != record.content() {
                        return Err(OperationError::RecoveryRequired);
                    }
                } else {
                    local.records.insert(id, *record);
                    if local.records.len() > radroots_domain::causal::MAX_CAUSAL_RECORDS {
                        return Err(WorkspaceError::Capacity.into());
                    }
                }
            }
            expected = retained.header.previous;
            continue;
        }
        let retained = read_operation(connection, input, crypto, binding, operation)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let receipt = &retained.receipt;
        if receipt.checkpoint().revision().get() != revision
            || receipt.checkpoint().head() != expected
        {
            return Err(WorkspaceError::StaleCheckpoint.into());
        }
        if revision == minimum.revision().get() && expected != minimum.head() {
            return Err(WorkspaceError::StaleCheckpoint.into());
        }
        hash_retained(&mut hash, revision, binding, operation, &retained)?;
        expected = retained.previous_head;
    }
    if (minimum.revision().get() == 0 || local.is_some()) && expected.is_some() {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    Ok(MinimumProof {
        lower,
        upper,
        digest: hash.finalize().into(),
        multipart,
    })
}

fn hash_retained(
    hash: &mut Sha256,
    revision: u64,
    binding: &WorkspaceBinding,
    operation: OperationId,
    retained: &Retained,
) -> Result<(), OperationError> {
    let receipt = &retained.receipt;
    hash.update(revision.to_be_bytes());
    hash.update(binding.actor().participant().as_bytes());
    hash.update(retained.original_grant.as_bytes());
    hash.update(operation.as_bytes());
    hash.update(retained.digest.as_bytes());
    hash.update(
        receipt
            .transaction_id()
            .ok_or(OperationError::RecoveryRequired)?
            .as_bytes(),
    );
    hash_optional(hash, Some(receipt.pool().id().as_bytes()))?;
    hash.update(
        receipt
            .checkpoint()
            .head()
            .ok_or(OperationError::RecoveryRequired)?
            .as_bytes(),
    );
    hash.update((retained.protected.len() as u64).to_be_bytes());
    hash.update(&retained.protected);
    if let Some((header, parts)) = &retained.frame {
        hash.update(header.family.code().to_be_bytes());
        hash_optional(hash, header.order.as_ref().map(|v| v.as_bytes().as_slice()))?;
        hash_optional(
            hash,
            header
                .related_operation
                .as_ref()
                .map(|v| v.as_bytes().as_slice()),
        )?;
        hash_optional(
            hash,
            header
                .subject_grant
                .as_ref()
                .map(|v| v.as_bytes().as_slice()),
        )?;
        hash.update([header.parts]);
        hash.update(parts);
    }
    hash.update([u8::from(retained.movement.is_some())]);
    if let (Some(id), Some(protected)) = (retained.movement, &retained.movement_protected) {
        hash.update(id);
        hash.update(receipt.pool().id().as_bytes());
        hash.update(retained.original_grant.as_bytes());
        hash.update((protected.len() as u64).to_be_bytes());
        hash.update(protected);
    }
    Ok(())
}
fn hash_optional(hash: &mut Sha256, value: Option<&[u8]>) -> Result<(), OperationError> {
    hash.update([u8::from(value.is_some())]);
    if let Some(bytes) = value {
        if bytes.len() != 32 {
            return Err(OperationError::RecoveryRequired);
        }
        hash.update(bytes);
    }
    Ok(())
}
async fn hash_current_movements(
    hash: &mut Sha256,
    connection: &mut SqliteConnection,
    actor: &[u8; 32],
    operation: &[u8; 32],
) -> Result<(), OperationError> {
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM stock_movements WHERE actor=? AND operation_id=?")
            .bind(actor.as_slice())
            .bind(operation.as_slice())
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
    if !(0..=64).contains(&count) {
        return Err(OperationError::RecoveryRequired);
    }
    hash.update([count as u8]);
    for offset in 0..count {
        let row=sqlx::query("SELECT movement_id,pool_id,grant_id,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_movements WHERE actor=? AND operation_id=? ORDER BY movement_id LIMIT 1 OFFSET ?")
            .bind(actor.as_slice()).bind(operation.as_slice()).bind(offset).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        for column in 0..3 {
            let bytes: Vec<u8> = row
                .try_get(column)
                .map_err(|_| OperationError::RecoveryRequired)?;
            if bytes.len() != 32 {
                return Err(OperationError::RecoveryRequired);
            }
            hash.update(bytes);
        }
        let protected: Vec<u8> = row
            .try_get::<Option<Vec<u8>>, _>(3)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
        hash.update((protected.len() as u64).to_be_bytes());
        hash.update(protected);
    }
    Ok(())
}

/// Proves the current pool projection against the latest actual protected
/// effect, including every intervening link from the current root. A mutable
/// SQL pool index alone cannot prove the absence of a more recent effect.
pub(super) async fn authenticate_projection(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    authority: &Authority,
    pool: PoolId,
    projected: Option<PoolSnapshot>,
) -> Result<MinimumProof, OperationError> {
    authenticate_projected(
        connection,
        input,
        crypto,
        binding,
        authority,
        Projection::Pool(pool, projected),
    )
    .await
}
pub(super) async fn authenticate_mapping_projection(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    authority: &Authority,
    key: radroots_domain::stock::OptionMappingKey,
    projected: Option<radroots_domain::stock::OptionPoolMapping>,
) -> Result<MinimumProof, OperationError> {
    authenticate_projected(
        connection,
        input,
        crypto,
        binding,
        authority,
        Projection::Mapping(key, projected),
    )
    .await
}
enum Projection {
    Pool(PoolId, Option<PoolSnapshot>),
    Mapping(
        radroots_domain::stock::OptionMappingKey,
        Option<radroots_domain::stock::OptionPoolMapping>,
    ),
}
impl Projection {
    fn matches(&self, receipt: &InventoryReceipt) -> Result<bool, OperationError> {
        let matched = match self {
            Self::Pool(id, projected) if *id == receipt.pool().id() => {
                Some(*projected == Some(receipt.pool()))
            }
            Self::Mapping(key, projected)
                if receipt.mapping().is_some_and(|value| value.key() == *key) =>
            {
                Some(*projected == receipt.mapping())
            }
            _ => None,
        };
        match matched {
            Some(true) => Ok(true),
            Some(false) => Err(OperationError::RecoveryRequired),
            None => Ok(false),
        }
    }
    fn present(&self) -> bool {
        match self {
            Self::Pool(_, value) => value.is_some(),
            Self::Mapping(_, value) => value.is_some(),
        }
    }
}
async fn authenticate_projected(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    authority: &Authority,
    projected: Projection,
) -> Result<MinimumProof, OperationError> {
    let current = authority
        .inventory
        .ok_or(OperationError::UnsupportedOperation)?;
    let upper = current.revision.get();
    let mut expected = current.head;
    let multipart = matches!(authority.catalog, 7..=16);
    let mut hash = history_hash(multipart);
    for revision in (1..=upper).rev() {
        let id:Vec<u8>=sqlx::query_scalar("SELECT operation_id FROM stock_operations WHERE revision=? AND length(operation_id)=32").bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let operation = OperationId::from_bytes(
            id.try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        if multipart {
            let retained = Box::pin(super::super::ledger::history(
                connection, input, crypto, binding, authority, operation,
            ))
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
            if retained.header.revision.get() != revision || Some(retained.header.head) != expected
            {
                return Err(OperationError::RecoveryRequired);
            }
            retained.hash(&mut hash)?;
            let matched = match &projected {
                Projection::Pool(id, observed) => retained
                    .pools
                    .iter()
                    .find(|pool| pool.id() == *id)
                    .map(|actual| *observed == Some(*actual)),
                Projection::Mapping(key, observed) => retained
                    .mapping
                    .filter(|actual| actual.key() == *key)
                    .map(|actual| *observed == Some(actual)),
            };
            if let Some(matched) = matched {
                if !matched {
                    return Err(OperationError::RecoveryRequired);
                }
                return Ok(MinimumProof {
                    lower: revision,
                    upper,
                    digest: hash.finalize().into(),
                    multipart,
                });
            }
            expected = retained.header.previous;
            continue;
        }
        let retained = read_operation(connection, input, crypto, binding, operation)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        if retained.receipt.checkpoint().revision().get() != revision
            || retained.receipt.checkpoint().head() != expected
        {
            return Err(OperationError::RecoveryRequired);
        }
        hash_retained(&mut hash, revision, binding, operation, &retained)?;
        if projected.matches(&retained.receipt)? {
            return Ok(MinimumProof {
                lower: revision,
                upper,
                digest: hash.finalize().into(),
                multipart,
            });
        }
        expected = retained.previous_head;
    }
    if expected.is_some() || projected.present() {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(MinimumProof {
        lower: 1,
        upper,
        digest: hash.finalize().into(),
        multipart,
    })
}

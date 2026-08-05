use crate::SqliteStorage;
use radroots_secrets::{
    EncryptedEnvelope,
    context::{EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId},
};
use radroots_storage::{
    Error,
    event::BoxFuture,
    private_artifact::{
        ArtifactCommitment, ArtifactKind, ArtifactSchemaId, DeletionReason, DurableSecretReference,
        EXPIRED_ARTIFACT_QUERY_LIMIT_MAX, PrivateArtifactEnvelopeMigrationStatus,
        PrivateArtifactId, PrivateArtifactMetadata, PrivateArtifactResealReceipt,
        PrivateArtifactResealRequest, PrivateArtifactRevision, PrivateArtifactStage,
        PrivateArtifactStatus, PrivateArtifactStore, RetentionPolicy,
    },
};
use sha2::{Digest, Sha256};
use sqlx::{Row, Sqlite};

#[cfg_attr(coverage_nightly, coverage(off))]
impl PrivateArtifactStore for SqliteStorage {
    fn put_metadata(
        &self,
        metadata: PrivateArtifactMetadata,
    ) -> BoxFuture<'_, Result<PrivateArtifactMetadata, Error>> {
        Box::pin(async move {
            self.require_private_writer()?;
            let mut transaction = self
                .private_pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let stored = put_metadata_transaction(&mut transaction, metadata, None).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(stored)
        })
    }

    fn metadata(
        &self,
        artifact_id: PrivateArtifactId,
    ) -> BoxFuture<'_, Result<Option<PrivateArtifactMetadata>, Error>> {
        Box::pin(async move {
            sqlx::query("SELECT * FROM radroots_private_artifacts WHERE artifact_id = ?")
                .bind(artifact_id.as_bytes().as_slice())
                .fetch_optional(self.private_pool())
                .await
                .map_err(map_backend)?
                .as_ref()
                .map(decode_metadata)
                .transpose()
        })
    }

    fn reseal_metadata(
        &self,
        request: PrivateArtifactResealRequest,
    ) -> BoxFuture<'_, Result<PrivateArtifactResealReceipt, Error>> {
        Box::pin(async move {
            self.require_private_writer()?;
            let mut transaction = self
                .private_pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            if let Some(receipt) = load_reseal_receipt(&mut transaction, &request).await? {
                transaction.commit().await.map_err(map_backend)?;
                return receipt.replay(&request);
            }
            let current = load_metadata(&mut transaction, request.artifact_id())
                .await?
                .ok_or(Error::PrivateArtifactNotFound)?;
            let next = current.resealed(&request)?;
            let result = sqlx::query(
                "UPDATE radroots_private_artifacts SET
                   commitment = ?, protected_size_bytes = ?, secret_provider = ?,
                   secret_reference = ?, key_version = ?, revision = ?,
                   updated_at_unix_ms = ?, last_reseal_id = ?, last_reseal_fingerprint = ?
                 WHERE artifact_id = ? AND revision = ? AND commitment = ?
                   AND encrypted_envelope IS NULL AND envelope_version IS NULL",
            )
            .bind(next.commitment().as_bytes().as_slice())
            .bind(i64_from_u64(next.protected_size_bytes())?)
            .bind(next.secret_reference().provider())
            .bind(next.secret_reference().opaque_reference())
            .bind(i64::from(next.secret_reference().key_version()))
            .bind(i64_from_u64(next.revision().get())?)
            .bind(i64_from_u64(next.updated_at_unix_ms())?)
            .bind(request.reseal_id().as_bytes().as_slice())
            .bind(request.fingerprint().as_slice())
            .bind(request.artifact_id().as_bytes().as_slice())
            .bind(i64_from_u64(request.expected_revision().get())?)
            .bind(request.expected_commitment().as_bytes().as_slice())
            .execute(&mut *transaction)
            .await
            .map_err(map_reseal)?;
            if result.rows_affected() != 1 {
                return Err(Error::PrivateArtifactResealConflict);
            }
            let receipt = load_reseal_receipt(&mut transaction, &request)
                .await?
                .ok_or(Error::PrivateArtifactPersistenceIndeterminate)?;
            transaction.commit().await.map_err(map_indeterminate)?;
            Ok(receipt)
        })
    }

    fn mark_expired(
        &self,
        artifact_id: PrivateArtifactId,
        expected_revision: PrivateArtifactRevision,
        at_unix_ms: u64,
    ) -> BoxFuture<'_, Result<PrivateArtifactMetadata, Error>> {
        Box::pin(async move {
            self.require_private_writer()?;
            let mut transaction = self
                .private_pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let current = load_metadata(&mut transaction, artifact_id)
                .await?
                .ok_or(Error::PrivateArtifactNotFound)?;
            let next = current.mark_expired(expected_revision, at_unix_ms)?;
            update_metadata(&mut transaction, &next, current.revision(), false).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(next)
        })
    }

    fn tombstone(
        &self,
        artifact_id: PrivateArtifactId,
        expected_revision: PrivateArtifactRevision,
        at_unix_ms: u64,
        reason: DeletionReason,
    ) -> BoxFuture<'_, Result<PrivateArtifactMetadata, Error>> {
        Box::pin(async move {
            self.require_private_writer()?;
            let mut transaction = self
                .private_pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let current = load_metadata(&mut transaction, artifact_id)
                .await?
                .ok_or(Error::PrivateArtifactNotFound)?;
            let next = current.tombstone(expected_revision, at_unix_ms, reason)?;
            update_metadata(&mut transaction, &next, current.revision(), true).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(next)
        })
    }

    fn expired(
        &self,
        at_unix_ms: u64,
        limit: u16,
    ) -> BoxFuture<'_, Result<Vec<PrivateArtifactMetadata>, Error>> {
        Box::pin(async move {
            if at_unix_ms == 0 || limit == 0 || limit > EXPIRED_ARTIFACT_QUERY_LIMIT_MAX {
                return Err(Error::InvalidExpiredArtifactQueryLimit);
            }
            sqlx::query(
                "SELECT * FROM radroots_private_artifacts
                 WHERE stage = 'active' AND expires_at_unix_ms <= ?
                 ORDER BY expires_at_unix_ms, artifact_id LIMIT ?",
            )
            .bind(i64_from_u64(at_unix_ms)?)
            .bind(i64::from(limit))
            .fetch_all(self.private_pool())
            .await
            .map_err(map_backend)?
            .iter()
            .map(decode_metadata)
            .collect()
        })
    }

    fn status(&self) -> BoxFuture<'_, Result<PrivateArtifactStatus, Error>> {
        Box::pin(async move {
            let row = sqlx::query(
                "SELECT
                   COALESCE(SUM(CASE WHEN stage = 'active' THEN 1 ELSE 0 END), 0) AS active,
                   COALESCE(SUM(CASE WHEN stage = 'expired' THEN 1 ELSE 0 END), 0) AS expired,
                   COALESCE(SUM(CASE WHEN stage = 'tombstoned' THEN 1 ELSE 0 END), 0) AS tombstoned
                 FROM radroots_private_artifacts",
            )
            .fetch_one(self.private_pool())
            .await
            .map_err(map_backend)?;
            Ok(PrivateArtifactStatus {
                active: count(&row, "active")?,
                expired: count(&row, "expired")?,
                tombstoned: count(&row, "tombstoned")?,
            })
        })
    }
}

impl SqliteStorage {
    /// Atomically stores validated metadata with its authenticated encrypted envelope.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn put_encrypted_private_artifact(
        &self,
        metadata: PrivateArtifactMetadata,
        envelope: &EncryptedEnvelope,
    ) -> Result<PrivateArtifactMetadata, Error> {
        self.require_private_writer()?;
        let encoded = validate_new_envelope(&metadata, envelope)?;
        let context_fingerprint = metadata.envelope_context().fingerprint();
        let mut transaction = self
            .private_pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(map_backend)?;
        let stored = put_metadata_transaction(
            &mut transaction,
            metadata,
            Some((
                envelope.version(),
                encoded.as_slice(),
                context_fingerprint.as_slice(),
            )),
        )
        .await?;
        transaction.commit().await.map_err(map_backend)?;
        Ok(stored)
    }

    /// Loads and revalidates an encrypted envelope without opening its plaintext.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn encrypted_private_artifact(
        &self,
        artifact_id: PrivateArtifactId,
    ) -> Result<Option<EncryptedEnvelope>, Error> {
        let Some(row) =
            sqlx::query("SELECT * FROM radroots_private_artifacts WHERE artifact_id = ?")
                .bind(artifact_id.as_bytes().as_slice())
                .fetch_optional(self.private_pool())
                .await
                .map_err(map_backend)?
        else {
            return Ok(None);
        };
        let metadata = decode_metadata(&row)?;
        let encoded = row
            .try_get::<Option<Vec<u8>>, _>("encrypted_envelope")
            .map_err(map_corrupt)?;
        let version = row
            .try_get::<Option<i64>, _>("envelope_version")
            .map_err(map_corrupt)?;
        match (encoded, version) {
            (None, None) => Ok(None),
            (Some(encoded), Some(version)) => {
                let envelope = EncryptedEnvelope::decode(encoded.as_slice())
                    .map_err(|_| Error::CorruptPrivateArtifactMetadata)?;
                if u64_from_i64(version)? != u64::from(envelope.version()) {
                    return Err(Error::CorruptPrivateArtifactMetadata);
                }
                validate_stored_envelope(&metadata, &envelope, &row)
                    .map_err(|_| Error::CorruptPrivateArtifactMetadata)?;
                Ok(Some(envelope))
            }
            _ => Err(Error::CorruptPrivateArtifactMetadata),
        }
    }

    /// Returns an identity-free inventory of private-envelope migration state.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn private_artifact_envelope_migration_status(
        &self,
    ) -> Result<PrivateArtifactEnvelopeMigrationStatus, Error> {
        let row = sqlx::query(
            "SELECT
               COALESCE(SUM(CASE WHEN envelope_version = 1
                 AND context_fingerprint IS NULL THEN 1 ELSE 0 END), 0) AS v1_pending,
               COALESCE(SUM(CASE WHEN envelope_version = 2
                 AND context_fingerprint IS NOT NULL THEN 1 ELSE 0 END), 0) AS v2_current,
               COALESCE(SUM(CASE WHEN envelope_version IS NOT NULL AND (
                 envelope_version NOT IN (1, 2)
                 OR (envelope_version = 1 AND context_fingerprint IS NOT NULL)
                 OR (envelope_version = 2 AND context_fingerprint IS NULL)
               ) THEN 1 ELSE 0 END), 0) AS corrupt
             FROM radroots_private_artifacts",
        )
        .fetch_one(self.private_pool())
        .await
        .map_err(map_backend)?;
        Ok(PrivateArtifactEnvelopeMigrationStatus {
            v1_pending: count(&row, "v1_pending")?,
            v2_current: count(&row, "v2_current")?,
            corrupt: count(&row, "corrupt")?,
            blocked_provider: 0,
            conflicted: 0,
        })
    }

    /// Atomically replaces one authenticated v1 envelope with an independently
    /// produced context-bound v2 envelope.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn commit_private_artifact_reseal(
        &self,
        request: PrivateArtifactResealRequest,
        envelope: &EncryptedEnvelope,
    ) -> Result<PrivateArtifactResealReceipt, Error> {
        self.require_private_writer()?;
        let mut transaction = self
            .private_pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(map_backend)?;
        if let Some(receipt) = load_reseal_receipt(&mut transaction, &request).await? {
            transaction.commit().await.map_err(map_backend)?;
            return receipt.replay(&request);
        }
        let current = load_metadata(&mut transaction, request.artifact_id())
            .await?
            .ok_or(Error::PrivateArtifactNotFound)?;
        let next = current.resealed(&request)?;
        let encoded = validate_new_envelope(&next, envelope)?;
        let context_fingerprint = next.envelope_context().fingerprint();
        let result = sqlx::query(
            "UPDATE radroots_private_artifacts SET
               commitment = ?, protected_size_bytes = ?, secret_provider = ?,
               secret_reference = ?, key_version = ?, envelope_version = 2,
               encrypted_envelope = ?, context_fingerprint = ?, revision = ?,
               updated_at_unix_ms = ?, last_reseal_id = ?, last_reseal_fingerprint = ?
             WHERE artifact_id = ? AND revision = ? AND commitment = ?
               AND envelope_version = 1 AND context_fingerprint IS NULL",
        )
        .bind(next.commitment().as_bytes().as_slice())
        .bind(i64_from_u64(next.protected_size_bytes())?)
        .bind(next.secret_reference().provider())
        .bind(next.secret_reference().opaque_reference())
        .bind(i64::from(next.secret_reference().key_version()))
        .bind(encoded.as_slice())
        .bind(context_fingerprint.as_slice())
        .bind(i64_from_u64(next.revision().get())?)
        .bind(i64_from_u64(next.updated_at_unix_ms())?)
        .bind(request.reseal_id().as_bytes().as_slice())
        .bind(request.fingerprint().as_slice())
        .bind(request.artifact_id().as_bytes().as_slice())
        .bind(i64_from_u64(request.expected_revision().get())?)
        .bind(request.expected_commitment().as_bytes().as_slice())
        .execute(&mut *transaction)
        .await
        .map_err(map_reseal)?;
        if result.rows_affected() != 1 {
            return Err(Error::PrivateArtifactResealConflict);
        }
        let receipt = load_reseal_receipt(&mut transaction, &request)
            .await?
            .ok_or(Error::PrivateArtifactPersistenceIndeterminate)?;
        transaction.commit().await.map_err(map_indeterminate)?;
        Ok(receipt)
    }

    fn require_private_writer(&self) -> Result<(), Error> {
        if self.event_mode() == radroots_storage::status::EventStoreMode::ReadOnly {
            return Err(Error::BackendUnavailable);
        }
        Ok(())
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn put_metadata_transaction(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    metadata: PrivateArtifactMetadata,
    envelope: Option<(u16, &[u8], &[u8])>,
) -> Result<PrivateArtifactMetadata, Error> {
    if metadata.stage() != PrivateArtifactStage::Active
        || metadata.revision() != PrivateArtifactRevision::INITIAL
    {
        return Err(Error::InvalidPrivateArtifactMetadata);
    }
    if let Some(row) = sqlx::query("SELECT * FROM radroots_private_artifacts WHERE artifact_id = ?")
        .bind(metadata.artifact_id().as_bytes().as_slice())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(map_backend)?
    {
        let existing = decode_metadata(&row)?;
        if existing != metadata {
            return Err(Error::PrivateArtifactConflict);
        }
        let stored_envelope = row
            .try_get::<Option<Vec<u8>>, _>("encrypted_envelope")
            .map_err(map_corrupt)?;
        return match (stored_envelope, envelope) {
            (None, Some((version, encoded, context_fingerprint))) => {
                let result = sqlx::query(
                    "UPDATE radroots_private_artifacts
                     SET envelope_version = ?, encrypted_envelope = ?, context_fingerprint = ?
                     WHERE artifact_id = ? AND encrypted_envelope IS NULL",
                )
                .bind(i64::from(version))
                .bind(encoded)
                .bind(context_fingerprint)
                .bind(metadata.artifact_id().as_bytes().as_slice())
                .execute(&mut **transaction)
                .await
                .map_err(map_backend)?;
                if result.rows_affected() != 1 {
                    return Err(Error::PrivateArtifactConflict);
                }
                Ok(metadata)
            }
            (Some(stored), Some((_, encoded, _))) if stored.as_slice() == encoded => Ok(metadata),
            (Some(_), Some(_)) => Err(Error::PrivateArtifactConflict),
            (_, None) => Ok(metadata),
        };
    }
    insert_metadata(transaction, &metadata, envelope).await?;
    Ok(metadata)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn insert_metadata(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    metadata: &PrivateArtifactMetadata,
    envelope: Option<(u16, &[u8], &[u8])>,
) -> Result<(), Error> {
    let tombstone = metadata.tombstone_record();
    sqlx::query(
        "INSERT INTO radroots_private_artifacts (
           artifact_id, artifact_kind, schema_id, commitment, protected_size_bytes,
           secret_provider, secret_reference, key_version, envelope_version,
           encrypted_envelope, context_fingerprint, delete_not_before_unix_ms, expires_at_unix_ms,
           revision, stage, created_at_unix_ms, updated_at_unix_ms,
           deleted_at_unix_ms, deletion_reason, tombstone_commitment
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(metadata.artifact_id().as_bytes().as_slice())
    .bind(metadata.kind().as_str())
    .bind(metadata.schema_id().as_str())
    .bind(metadata.commitment().as_bytes().as_slice())
    .bind(i64_from_u64(metadata.protected_size_bytes())?)
    .bind(metadata.secret_reference().provider())
    .bind(metadata.secret_reference().opaque_reference())
    .bind(i64::from(metadata.secret_reference().key_version()))
    .bind(envelope.map(|(version, _, _)| i64::from(version)))
    .bind(envelope.map(|(_, encoded, _)| encoded))
    .bind(envelope.map(|(_, _, context_fingerprint)| context_fingerprint))
    .bind(
        metadata
            .retention()
            .delete_not_before_unix_ms()
            .map(i64_from_u64)
            .transpose()?,
    )
    .bind(
        metadata
            .retention()
            .expires_at_unix_ms()
            .map(i64_from_u64)
            .transpose()?,
    )
    .bind(i64_from_u64(metadata.revision().get())?)
    .bind(stage_name(metadata.stage()))
    .bind(i64_from_u64(metadata.created_at_unix_ms())?)
    .bind(i64_from_u64(metadata.updated_at_unix_ms())?)
    .bind(
        tombstone
            .map(|value| i64_from_u64(value.deleted_at_unix_ms()))
            .transpose()?,
    )
    .bind(tombstone.map(|value| deletion_name(value.reason())))
    .bind(tombstone.map(|value| value.commitment().as_bytes().to_vec()))
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn load_metadata(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    artifact_id: PrivateArtifactId,
) -> Result<Option<PrivateArtifactMetadata>, Error> {
    sqlx::query("SELECT * FROM radroots_private_artifacts WHERE artifact_id = ?")
        .bind(artifact_id.as_bytes().as_slice())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(map_backend)?
        .as_ref()
        .map(decode_metadata)
        .transpose()
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn update_metadata(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    metadata: &PrivateArtifactMetadata,
    prior_revision: PrivateArtifactRevision,
    remove_envelope: bool,
) -> Result<(), Error> {
    let tombstone = metadata.tombstone_record();
    let mut query = if remove_envelope {
        sqlx::query(
            "UPDATE radroots_private_artifacts SET
               revision = ?, stage = ?, updated_at_unix_ms = ?,
               deleted_at_unix_ms = ?, deletion_reason = ?, tombstone_commitment = ?,
               envelope_version = NULL, encrypted_envelope = NULL
             WHERE artifact_id = ? AND revision = ?",
        )
    } else {
        sqlx::query(
            "UPDATE radroots_private_artifacts SET
               revision = ?, stage = ?, updated_at_unix_ms = ?,
               deleted_at_unix_ms = ?, deletion_reason = ?, tombstone_commitment = ?
             WHERE artifact_id = ? AND revision = ?",
        )
    };
    query = query
        .bind(i64_from_u64(metadata.revision().get())?)
        .bind(stage_name(metadata.stage()))
        .bind(i64_from_u64(metadata.updated_at_unix_ms())?)
        .bind(
            tombstone
                .map(|value| i64_from_u64(value.deleted_at_unix_ms()))
                .transpose()?,
        )
        .bind(tombstone.map(|value| deletion_name(value.reason())))
        .bind(tombstone.map(|value| value.commitment().as_bytes().to_vec()))
        .bind(metadata.artifact_id().as_bytes().as_slice())
        .bind(i64_from_u64(prior_revision.get())?);
    let result = query
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
    if result.rows_affected() != 1 {
        return Err(Error::PrivateArtifactRevisionConflict);
    }
    Ok(())
}

fn decode_metadata(row: &sqlx::sqlite::SqliteRow) -> Result<PrivateArtifactMetadata, Error> {
    let artifact_id = PrivateArtifactId::new(array(
        row.try_get::<Vec<u8>, _>("artifact_id")
            .map_err(map_corrupt)?,
    )?)
    .map_err(|_| Error::CorruptPrivateArtifactMetadata)?;
    let kind = ArtifactKind::parse(
        row.try_get::<String, _>("artifact_kind")
            .map_err(map_corrupt)?,
    )
    .map_err(|_| Error::CorruptPrivateArtifactMetadata)?;
    let schema_id =
        ArtifactSchemaId::parse(row.try_get::<String, _>("schema_id").map_err(map_corrupt)?)
            .map_err(|_| Error::CorruptPrivateArtifactMetadata)?;
    let commitment = ArtifactCommitment::new(array(
        row.try_get::<Vec<u8>, _>("commitment")
            .map_err(map_corrupt)?,
    )?);
    let secret_reference = DurableSecretReference::new(
        row.try_get::<String, _>("secret_provider")
            .map_err(map_corrupt)?,
        row.try_get::<String, _>("secret_reference")
            .map_err(map_corrupt)?,
        u32::try_from(row.try_get::<i64, _>("key_version").map_err(map_corrupt)?)
            .map_err(|_| Error::CorruptPrivateArtifactMetadata)?,
    )
    .map_err(|_| Error::CorruptPrivateArtifactMetadata)?;
    let retention = RetentionPolicy::new(
        optional_u64(row, "delete_not_before_unix_ms")?,
        optional_u64(row, "expires_at_unix_ms")?,
    )
    .map_err(|_| Error::CorruptPrivateArtifactMetadata)?;
    let stage = stage(
        row.try_get::<String, _>("stage")
            .map_err(map_corrupt)?
            .as_str(),
    )?;
    let deleted_at = optional_u64(row, "deleted_at_unix_ms")?;
    let deletion_reason = row
        .try_get::<Option<String>, _>("deletion_reason")
        .map_err(map_corrupt)?
        .map(|value| deletion(value.as_str()))
        .transpose()?;
    let tombstone_commitment = row
        .try_get::<Option<Vec<u8>>, _>("tombstone_commitment")
        .map_err(map_corrupt)?
        .map(|value| array(value).map(ArtifactCommitment::new))
        .transpose()?;
    let tombstone = match (deleted_at, deletion_reason, tombstone_commitment) {
        (None, None, None) => None,
        (Some(at), Some(reason), Some(commitment)) => Some((at, reason, commitment)),
        _ => return Err(Error::CorruptPrivateArtifactMetadata),
    };
    PrivateArtifactMetadata::from_durable_parts(
        artifact_id,
        kind,
        schema_id,
        commitment,
        u64_from_i64(row.try_get("protected_size_bytes").map_err(map_corrupt)?)?,
        secret_reference,
        retention,
        PrivateArtifactRevision::new(u64_from_i64(row.try_get("revision").map_err(map_corrupt)?)?)
            .map_err(|_| Error::CorruptPrivateArtifactMetadata)?,
        stage,
        u64_from_i64(row.try_get("created_at_unix_ms").map_err(map_corrupt)?)?,
        u64_from_i64(row.try_get("updated_at_unix_ms").map_err(map_corrupt)?)?,
        tombstone,
    )
    .map_err(|_| Error::CorruptPrivateArtifactMetadata)
}

fn validate_new_envelope(
    metadata: &PrivateArtifactMetadata,
    envelope: &EncryptedEnvelope,
) -> Result<Vec<u8>, Error> {
    let expected_context = secrets_context(metadata)?;
    if envelope.version() != 2
        || envelope.context() != Some(&expected_context)
        || metadata.stage() != PrivateArtifactStage::Active
        || metadata.secret_reference().opaque_reference() != envelope.reference().id().as_str()
        || metadata.secret_reference().key_version() != envelope.reference().key_version().get()
    {
        return Err(Error::InvalidPrivateArtifactMetadata);
    }
    let encoded = envelope
        .encode()
        .map_err(|_| Error::InvalidPrivateArtifactMetadata)?;
    if metadata.protected_size_bytes()
        != u64::try_from(encoded.len()).map_err(|_| Error::InvalidPrivateArtifactMetadata)?
        || metadata.commitment().as_bytes() != Sha256::digest(encoded.as_slice()).as_slice()
    {
        return Err(Error::InvalidPrivateArtifactMetadata);
    }
    Ok(encoded)
}

fn validate_stored_envelope(
    metadata: &PrivateArtifactMetadata,
    envelope: &EncryptedEnvelope,
    row: &sqlx::sqlite::SqliteRow,
) -> Result<(), Error> {
    let encoded = envelope
        .encode()
        .map_err(|_| Error::CorruptPrivateArtifactMetadata)?;
    if metadata.secret_reference().opaque_reference() != envelope.reference().id().as_str()
        || metadata.secret_reference().key_version() != envelope.reference().key_version().get()
        || metadata.protected_size_bytes()
            != u64::try_from(encoded.len()).map_err(|_| Error::CorruptPrivateArtifactMetadata)?
        || metadata.commitment().as_bytes() != Sha256::digest(encoded.as_slice()).as_slice()
    {
        return Err(Error::CorruptPrivateArtifactMetadata);
    }
    let stored_fingerprint = row
        .try_get::<Option<Vec<u8>>, _>("context_fingerprint")
        .map_err(map_corrupt)?;
    match envelope.version() {
        1 if envelope.context().is_none() && stored_fingerprint.is_none() => Ok(()),
        2 => {
            let expected = secrets_context(metadata)?;
            let expected_fingerprint = metadata.envelope_context().fingerprint();
            if envelope.context() == Some(&expected)
                && stored_fingerprint.as_deref() == Some(expected_fingerprint.as_slice())
            {
                Ok(())
            } else {
                Err(Error::CorruptPrivateArtifactMetadata)
            }
        }
        _ => Err(Error::CorruptPrivateArtifactMetadata),
    }
}

fn secrets_context(metadata: &PrivateArtifactMetadata) -> Result<EnvelopeContext, Error> {
    let derived = metadata.envelope_context();
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse(derived.purpose())
            .map_err(|_| Error::CorruptPrivateArtifactMetadata)?,
        EnvelopeSubject::parse(derived.subject_type(), derived.subject())
            .map_err(|_| Error::CorruptPrivateArtifactMetadata)?,
        PayloadSchemaId::parse(derived.payload_schema())
            .map_err(|_| Error::CorruptPrivateArtifactMetadata)?,
    ))
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn load_reseal_receipt(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    request: &PrivateArtifactResealRequest,
) -> Result<Option<PrivateArtifactResealReceipt>, Error> {
    let Some(row) = sqlx::query(
        "SELECT artifact_id, request_fingerprint, committed_revision
         FROM radroots_private_envelope_reseals WHERE reseal_id = ?",
    )
    .bind(request.reseal_id().as_bytes().as_slice())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_backend)?
    else {
        return Ok(None);
    };
    let artifact_id = row
        .try_get::<Vec<u8>, _>("artifact_id")
        .map_err(map_corrupt)?;
    let fingerprint = row
        .try_get::<Vec<u8>, _>("request_fingerprint")
        .map_err(map_corrupt)?;
    if artifact_id.as_slice() != request.artifact_id().as_bytes()
        || fingerprint.as_slice() != request.fingerprint()
    {
        return Err(Error::PrivateArtifactResealConflict);
    }
    let revision = PrivateArtifactRevision::new(u64_from_i64(
        row.try_get::<i64, _>("committed_revision")
            .map_err(map_corrupt)?,
    )?)
    .map_err(|_| Error::CorruptPrivateArtifactMetadata)?;
    Ok(Some(PrivateArtifactResealReceipt::committed(
        request, revision,
    )))
}

fn optional_u64(row: &sqlx::sqlite::SqliteRow, column: &str) -> Result<Option<u64>, Error> {
    row.try_get::<Option<i64>, _>(column)
        .map_err(map_corrupt)?
        .map(u64_from_i64)
        .transpose()
}

const fn stage_name(stage: PrivateArtifactStage) -> &'static str {
    match stage {
        PrivateArtifactStage::Active => "active",
        PrivateArtifactStage::Expired => "expired",
        PrivateArtifactStage::Tombstoned => "tombstoned",
    }
}

fn stage(value: &str) -> Result<PrivateArtifactStage, Error> {
    match value.as_bytes() {
        b"active" => Ok(PrivateArtifactStage::Active),
        b"expired" => Ok(PrivateArtifactStage::Expired),
        b"tombstoned" => Ok(PrivateArtifactStage::Tombstoned),
        _ => Err(Error::CorruptPrivateArtifactMetadata),
    }
}

const fn deletion_name(reason: DeletionReason) -> &'static str {
    match reason {
        DeletionReason::UserRequested => "user_requested",
        DeletionReason::RetentionExpired => "retention_expired",
        DeletionReason::KeyRevoked => "key_revoked",
        DeletionReason::IntegrityFailure => "integrity_failure",
        DeletionReason::OperatorRequested => "operator_requested",
    }
}

fn deletion(value: &str) -> Result<DeletionReason, Error> {
    match value.as_bytes() {
        b"user_requested" => Ok(DeletionReason::UserRequested),
        b"retention_expired" => Ok(DeletionReason::RetentionExpired),
        b"key_revoked" => Ok(DeletionReason::KeyRevoked),
        b"integrity_failure" => Ok(DeletionReason::IntegrityFailure),
        b"operator_requested" => Ok(DeletionReason::OperatorRequested),
        _ => Err(Error::CorruptPrivateArtifactMetadata),
    }
}

fn count(row: &sqlx::sqlite::SqliteRow, column: &str) -> Result<u64, Error> {
    u64_from_i64(row.try_get::<i64, _>(column).map_err(map_corrupt)?)
}

fn array<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N], Error> {
    bytes
        .try_into()
        .map_err(|_| Error::CorruptPrivateArtifactMetadata)
}

fn i64_from_u64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::CorruptPrivateArtifactMetadata)
}

fn u64_from_i64(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::CorruptPrivateArtifactMetadata)
}

fn map_backend(_: sqlx::Error) -> Error {
    Error::BackendUnavailable
}

fn map_reseal(_: sqlx::Error) -> Error {
    Error::PrivateArtifactResealConflict
}

fn map_indeterminate(_: sqlx::Error) -> Error {
    Error::PrivateArtifactPersistenceIndeterminate
}

fn map_corrupt(_: sqlx::Error) -> Error {
    Error::CorruptPrivateArtifactMetadata
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::migration::{
        private::{MIGRATIONS as PRIVATE_MIGRATIONS, migration_sql as private_migration_sql},
        runtime::{MIGRATIONS as RUNTIME_MIGRATIONS, migration_sql as runtime_migration_sql},
    };
    use radroots_secrets::{
        Error as SecretError, KeyWrapping, SecretId, SecretRef,
        envelope::{LegacyV1ResealAuthority, Nonce, SealMaterial, SealRequest},
        error::Operation,
        id::{BackendKind, KeyVersion},
        wrapping::{
            BoxFuture as SecretFuture, LegacyV1UnwrapRequest, SecretMaterial, UnwrapRequest,
            WrapRequest, WrappedSecret,
        },
    };
    use radroots_storage::private_artifact::{
        PrivateArtifactResealDisposition, PrivateArtifactResealId,
    };
    use radroots_storage::status::EventStoreMode;
    use sqlx::sqlite::SqlitePoolOptions;

    struct VectorWrapping;

    impl KeyWrapping for VectorWrapping {
        fn wrap<'a>(
            &'a self,
            request: WrapRequest<'a>,
        ) -> SecretFuture<'a, Result<WrappedSecret, SecretError>> {
            Box::pin(async move {
                if !matches!(
                    request.reference().id().as_str(),
                    "private-artifact-key" | "envelope-key"
                ) {
                    return Err(SecretError::BackendFailure {
                        backend: BackendKind::Memory,
                        operation: Operation::Wrap,
                    });
                }
                WrappedSecret::from_bytes(request.plaintext().expose_secret(|bytes| {
                    bytes.iter().map(|byte| byte ^ 0xA5).collect::<Vec<_>>()
                }))
            })
        }

        fn unwrap<'a>(
            &'a self,
            request: UnwrapRequest<'a>,
        ) -> SecretFuture<'a, Result<SecretMaterial, SecretError>> {
            Box::pin(async move {
                if !matches!(
                    request.reference().id().as_str(),
                    "private-artifact-key" | "envelope-key"
                ) {
                    return Err(SecretError::BackendFailure {
                        backend: BackendKind::Memory,
                        operation: Operation::Unwrap,
                    });
                }
                let plaintext = if request.wrapped().as_bytes() == [0x4b; 32] {
                    vec![0x11; 32]
                } else {
                    request
                        .wrapped()
                        .as_bytes()
                        .iter()
                        .map(|byte| byte ^ 0xA5)
                        .collect::<Vec<_>>()
                };
                SecretMaterial::from_slice(plaintext.as_slice())
            })
        }

        fn unwrap_legacy_v1<'a>(
            &'a self,
            request: LegacyV1UnwrapRequest<'a>,
        ) -> SecretFuture<'a, Result<SecretMaterial, SecretError>> {
            Box::pin(async move {
                if request.reference().id().as_str() != "envelope-key"
                    || request.wrapped().as_bytes() != [0x4b; 32]
                {
                    return Err(SecretError::BackendFailure {
                        backend: BackendKind::Memory,
                        operation: Operation::Unwrap,
                    });
                }
                SecretMaterial::from_slice(&[0x11; 32])
            })
        }
    }

    async fn store(mode: EventStoreMode) -> SqliteStorage {
        let runtime_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("runtime SQLite");
        for migration in RUNTIME_MIGRATIONS {
            sqlx::raw_sql(runtime_migration_sql(migration.version()).expect("runtime SQL"))
                .execute(&runtime_pool)
                .await
                .expect("runtime migration");
        }
        let private_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("private SQLite");
        for migration in PRIVATE_MIGRATIONS {
            sqlx::raw_sql(private_migration_sql(migration.version()).expect("private SQL"))
                .execute(&private_pool)
                .await
                .expect("private migration");
        }
        SqliteStorage::with_private_pool(
            runtime_pool,
            private_pool,
            radroots_storage::event::SourceGeneration::new([91; 32]).expect("generation"),
            mode,
        )
    }

    fn reference(version: u32) -> SecretRef {
        SecretRef::new(
            SecretId::parse("private-artifact-key").expect("secret id"),
            BackendKind::Memory,
            KeyVersion::new(version).expect("key version"),
        )
    }

    fn test_context(id: u8, kind: &str) -> EnvelopeContext {
        let subject = [id; 16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        EnvelopeContext::new(
            EnvelopePurpose::parse(format!("radroots.private_artifact.{kind}")).expect("purpose"),
            EnvelopeSubject::parse("private_artifact", subject).expect("subject"),
            PayloadSchemaId::parse(format!("{kind}.v1")).expect("schema"),
        )
    }

    async fn sealed_envelope(
        plaintext: &[u8],
        version: u32,
        id: u8,
        kind: &str,
    ) -> EncryptedEnvelope {
        let plaintext = SecretMaterial::from_slice(plaintext).expect("plaintext");
        let data_key = SecretMaterial::from_slice(&[0x31; 32]).expect("data key");
        EncryptedEnvelope::seal(
            &VectorWrapping,
            SealRequest::new(
                reference(version),
                test_context(id, kind),
                &plaintext,
                SealMaterial::new(data_key, Nonce::new([0x42; 24])),
            ),
        )
        .await
        .expect("seal envelope")
    }

    fn metadata(
        id: u8,
        kind: &str,
        envelope: &EncryptedEnvelope,
        retention: RetentionPolicy,
    ) -> PrivateArtifactMetadata {
        let encoded = envelope.encode().expect("encoded envelope");
        PrivateArtifactMetadata::new(
            PrivateArtifactId::new([id; 16]).expect("artifact id"),
            ArtifactKind::parse(kind).expect("artifact kind"),
            ArtifactSchemaId::parse(format!("{kind}.v1")).expect("schema id"),
            ArtifactCommitment::new(Sha256::digest(encoded.as_slice()).into()),
            u64::try_from(encoded.len()).expect("encoded length"),
            DurableSecretReference::new(
                "memory",
                envelope.reference().id().as_str(),
                envelope.reference().key_version().get(),
            )
            .expect("secret reference"),
            retention,
            100,
        )
        .expect("metadata")
    }

    async fn migrated_legacy_store() -> (SqliteStorage, PrivateArtifactMetadata, EncryptedEnvelope)
    {
        const V1_ENVELOPE_HEX: &str = "52525331000101010100000007000c656e76656c6f70652d6b6579222222222222222222222222222222222222222222222222000000204b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b00000028f106837e33d690e7c5287abdd815ce9257b7b5b176ea9596abf3b7fe745aec5a8c2487a553d4659d";
        let runtime_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("runtime SQLite");
        for migration in RUNTIME_MIGRATIONS {
            sqlx::raw_sql(runtime_migration_sql(migration.version()).expect("runtime SQL"))
                .execute(&runtime_pool)
                .await
                .expect("runtime migration");
        }
        let private_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("private SQLite");
        for migration in &PRIVATE_MIGRATIONS[..3] {
            sqlx::raw_sql(private_migration_sql(migration.version()).expect("private SQL"))
                .execute(&private_pool)
                .await
                .expect("private migration");
        }
        let encoded = hex::decode(V1_ENVELOPE_HEX).expect("legacy vector");
        let envelope = EncryptedEnvelope::decode(encoded.as_slice()).expect("legacy envelope");
        let metadata = metadata(
            1,
            "trade.private_terms",
            &envelope,
            RetentionPolicy::indefinite(),
        );
        sqlx::query(
            "INSERT INTO radroots_private_artifacts (
               artifact_id, artifact_kind, schema_id, commitment, protected_size_bytes,
               secret_provider, secret_reference, key_version, envelope_version,
               encrypted_envelope, revision, stage, created_at_unix_ms, updated_at_unix_ms
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, 1, 'active', 100, 100)",
        )
        .bind(metadata.artifact_id().as_bytes().as_slice())
        .bind(metadata.kind().as_str())
        .bind(metadata.schema_id().as_str())
        .bind(metadata.commitment().as_bytes().as_slice())
        .bind(i64_from_u64(metadata.protected_size_bytes()).unwrap())
        .bind(metadata.secret_reference().provider())
        .bind(metadata.secret_reference().opaque_reference())
        .bind(i64::from(metadata.secret_reference().key_version()))
        .bind(encoded)
        .execute(&private_pool)
        .await
        .expect("legacy row");
        sqlx::raw_sql(private_migration_sql(4).expect("v4 SQL"))
            .execute(&private_pool)
            .await
            .expect("v4 migration");
        (
            SqliteStorage::with_private_pool(
                runtime_pool,
                private_pool,
                radroots_storage::event::SourceGeneration::new([91; 32]).expect("generation"),
                EventStoreMode::ReadWrite,
            ),
            metadata,
            envelope,
        )
    }

    #[tokio::test]
    async fn encrypted_envelopes_round_trip_with_exact_commitment_and_key_version() {
        let store = store(EventStoreMode::ReadWrite).await;
        let envelope = sealed_envelope(b"private farm coordinates", 7, 1, "farm.location").await;
        let metadata = metadata(1, "farm.location", &envelope, RetentionPolicy::indefinite());
        let stored = store
            .put_encrypted_private_artifact(metadata.clone(), &envelope)
            .await
            .expect("store encrypted artifact");
        assert_eq!(stored, metadata);
        store
            .put_encrypted_private_artifact(metadata.clone(), &envelope)
            .await
            .expect("exact replay");
        let loaded = store
            .encrypted_private_artifact(metadata.artifact_id())
            .await
            .expect("load envelope")
            .expect("encrypted envelope");
        assert_eq!(
            loaded.encode().expect("loaded bytes"),
            envelope.encode().expect("expected bytes")
        );
        let opened = loaded
            .open(&VectorWrapping, &test_context(1, "farm.location"))
            .await
            .expect("open envelope");
        opened.expose_secret(|bytes| assert_eq!(bytes, b"private farm coordinates"));

        let row = sqlx::query(
            "SELECT key_version, envelope_version, encrypted_envelope
             FROM radroots_private_artifacts WHERE artifact_id = ?",
        )
        .bind(metadata.artifact_id().as_bytes().as_slice())
        .fetch_one(store.private_pool())
        .await
        .expect("private row");
        assert_eq!(row.get::<i64, _>("key_version"), 7);
        assert_eq!(row.get::<i64, _>("envelope_version"), 2);
        let encrypted = row.get::<Vec<u8>, _>("encrypted_envelope");
        assert!(
            !encrypted
                .windows(24)
                .any(|bytes| bytes == b"private farm coordinates")
        );

        let wrong_key_envelope =
            sealed_envelope(b"private farm coordinates", 8, 1, "farm.location").await;
        assert_eq!(
            store
                .put_encrypted_private_artifact(metadata, &wrong_key_envelope)
                .await,
            Err(Error::InvalidPrivateArtifactMetadata)
        );

        let envelope = sealed_envelope(b"validation matrix", 9, 9, "test.validation_matrix").await;
        let valid = self::metadata(
            9,
            "test.validation_matrix",
            &envelope,
            RetentionPolicy::new(Some(100), Some(100)).expect("retention"),
        );
        assert!(validate_new_envelope(&valid, &envelope).is_ok());
        let expired = valid
            .mark_expired(valid.revision(), 100)
            .expect("expired metadata");
        assert_eq!(
            validate_new_envelope(&expired, &envelope),
            Err(Error::InvalidPrivateArtifactMetadata)
        );
        for (commitment, protected_size, secret_reference) in [
            (
                ArtifactCommitment::new([0; 32]),
                valid.protected_size_bytes(),
                valid.secret_reference().clone(),
            ),
            (
                valid.commitment(),
                valid.protected_size_bytes() + 1,
                valid.secret_reference().clone(),
            ),
            (
                valid.commitment(),
                valid.protected_size_bytes(),
                DurableSecretReference::new(
                    "memory",
                    "different-private-artifact-key",
                    valid.secret_reference().key_version(),
                )
                .expect("different reference"),
            ),
        ] {
            let invalid = PrivateArtifactMetadata::new(
                valid.artifact_id(),
                valid.kind().clone(),
                valid.schema_id().clone(),
                commitment,
                protected_size,
                secret_reference,
                valid.retention(),
                valid.created_at_unix_ms(),
            )
            .expect("structurally valid metadata");
            assert_eq!(
                validate_new_envelope(&invalid, &envelope),
                Err(Error::InvalidPrivateArtifactMetadata)
            );
        }
    }

    #[tokio::test]
    async fn expiry_and_tombstone_delete_envelope_but_preserve_commitment() {
        let store = store(EventStoreMode::ReadWrite).await;
        let envelope = sealed_envelope(b"private trade artifact", 3, 2, "trade.artifact").await;
        let metadata = metadata(
            2,
            "trade.artifact",
            &envelope,
            RetentionPolicy::new(Some(400), Some(300)).expect("retention"),
        );
        store
            .put_encrypted_private_artifact(metadata.clone(), &envelope)
            .await
            .expect("store artifact");
        assert!(
            store
                .expired(299, 10)
                .await
                .expect("not expired")
                .is_empty()
        );
        assert_eq!(
            store.expired(300, 10).await.expect("expired query"),
            vec![metadata.clone()]
        );
        let expired = store
            .mark_expired(metadata.artifact_id(), metadata.revision(), 300)
            .await
            .expect("mark expired");
        assert_eq!(expired.stage(), PrivateArtifactStage::Expired);
        assert_eq!(
            store
                .tombstone(
                    expired.artifact_id(),
                    expired.revision(),
                    399,
                    DeletionReason::RetentionExpired,
                )
                .await,
            Err(Error::PrivateArtifactRetentionActive)
        );
        let tombstoned = store
            .tombstone(
                expired.artifact_id(),
                expired.revision(),
                400,
                DeletionReason::RetentionExpired,
            )
            .await
            .expect("tombstone");
        assert_eq!(tombstoned.stage(), PrivateArtifactStage::Tombstoned);
        assert_eq!(tombstoned.commitment(), metadata.commitment());
        assert!(
            store
                .encrypted_private_artifact(metadata.artifact_id())
                .await
                .expect("deleted envelope")
                .is_none()
        );
        assert_eq!(
            store.status().await.expect("status"),
            PrivateArtifactStatus {
                active: 0,
                expired: 0,
                tombstoned: 1,
            }
        );
        assert!(
            sqlx::query("DELETE FROM radroots_private_artifacts WHERE artifact_id = ?")
                .bind(metadata.artifact_id().as_bytes().as_slice())
                .execute(store.private_pool())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn all_private_authorities_are_metadata_only_without_an_envelope() {
        let store = store(EventStoreMode::ReadWrite).await;
        for (id, kind) in [
            (10, "signing.reference"),
            (11, "farm.location"),
            (12, "trade.artifact"),
            (13, "nip46.session"),
        ] {
            let envelope = sealed_envelope(kind.as_bytes(), 1, id, kind).await;
            let metadata = metadata(id, kind, &envelope, RetentionPolicy::indefinite());
            store
                .put_metadata(metadata.clone())
                .await
                .expect("put metadata");
            assert_eq!(
                store
                    .metadata(metadata.artifact_id())
                    .await
                    .expect("metadata lookup"),
                Some(metadata.clone())
            );
            assert!(
                store
                    .encrypted_private_artifact(metadata.artifact_id())
                    .await
                    .expect("envelope lookup")
                    .is_none()
            );
            store
                .put_encrypted_private_artifact(metadata.clone(), &envelope)
                .await
                .expect("attach envelope");
        }
        assert_eq!(store.status().await.expect("status").active, 4);
        let forbidden = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE lower(name) LIKE '%studio%' OR lower(name) LIKE '%ui_state%'",
        )
        .fetch_one(store.private_pool())
        .await
        .expect("forbidden schema count");
        assert_eq!(forbidden, 0);
    }

    #[tokio::test]
    async fn conflicts_corruption_and_read_only_mode_fail_closed() {
        let writable_store = store(EventStoreMode::ReadWrite).await;
        let envelope = sealed_envelope(b"signing reference", 5, 20, "signing.reference").await;
        let stored_metadata = metadata(
            20,
            "signing.reference",
            &envelope,
            RetentionPolicy::indefinite(),
        );
        writable_store
            .put_encrypted_private_artifact(stored_metadata.clone(), &envelope)
            .await
            .expect("store artifact");
        let other_envelope =
            sealed_envelope(b"another signing reference", 5, 20, "signing.reference").await;
        let conflicting = metadata(
            20,
            "signing.reference",
            &other_envelope,
            RetentionPolicy::indefinite(),
        );
        assert_eq!(
            writable_store.put_metadata(conflicting).await,
            Err(Error::PrivateArtifactConflict)
        );
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(writable_store.private_pool())
            .await
            .expect("disable checks");
        sqlx::query("UPDATE radroots_private_artifacts SET stage = 'invalid'")
            .execute(writable_store.private_pool())
            .await
            .expect("corrupt stage");
        assert_eq!(
            writable_store.metadata(stored_metadata.artifact_id()).await,
            Err(Error::CorruptPrivateArtifactMetadata)
        );

        let read_only = store(EventStoreMode::ReadOnly).await;
        let envelope = sealed_envelope(b"read only", 1, 21, "nip46.session").await;
        let metadata = metadata(
            21,
            "nip46.session",
            &envelope,
            RetentionPolicy::indefinite(),
        );
        assert_eq!(
            read_only
                .put_encrypted_private_artifact(metadata, &envelope)
                .await,
            Err(Error::BackendUnavailable)
        );
    }

    #[tokio::test]
    async fn legacy_reseal_is_atomic_idempotent_and_context_bound() {
        let (store, metadata, legacy) = migrated_legacy_store().await;
        assert_eq!(legacy.version(), 1);
        assert_eq!(
            store
                .private_artifact_envelope_migration_status()
                .await
                .expect("migration status"),
            PrivateArtifactEnvelopeMigrationStatus {
                v1_pending: 1,
                v2_current: 0,
                corrupt: 0,
                blocked_provider: 0,
                conflicted: 0,
            }
        );

        let context = test_context(1, "trade.private_terms");
        let authority = LegacyV1ResealAuthority::new();
        assert!(matches!(
            legacy
                .reseal_legacy_v1(
                    &VectorWrapping,
                    &authority,
                    legacy.reference(),
                    reference(8),
                    context.clone(),
                    &|_| false,
                    SealMaterial::new(
                        SecretMaterial::from_slice(&[0x33; 32]).expect("fresh key"),
                        Nonce::new([0x44; 24]),
                    ),
                )
                .await,
            Err(SecretError::LegacyPayloadValidationFailed)
        ));
        assert_eq!(
            store
                .encrypted_private_artifact(metadata.artifact_id())
                .await
                .expect("unchanged legacy")
                .expect("legacy envelope")
                .version(),
            1
        );

        let resealed = legacy
            .reseal_legacy_v1(
                &VectorWrapping,
                &authority,
                legacy.reference(),
                reference(8),
                context.clone(),
                &|plaintext| plaintext == b"radroots envelope vector",
                SealMaterial::new(
                    SecretMaterial::from_slice(&[0x33; 32]).expect("fresh key"),
                    Nonce::new([0x44; 24]),
                ),
            )
            .await
            .expect("authorized reseal");
        let encoded = resealed.envelope().encode().expect("v2 bytes");
        let request = PrivateArtifactResealRequest::new(
            PrivateArtifactResealId::new([0x55; 16]).expect("reseal id"),
            metadata.artifact_id(),
            metadata.revision(),
            metadata.commitment(),
            ArtifactCommitment::new(Sha256::digest(encoded.as_slice()).into()),
            u64::try_from(encoded.len()).expect("v2 length"),
            DurableSecretReference::new("memory", "private-artifact-key", 8)
                .expect("next reference"),
            200,
        )
        .expect("reseal request");
        let contender = store.clone();
        let (left, right) = tokio::join!(
            store.commit_private_artifact_reseal(request.clone(), resealed.envelope()),
            contender.commit_private_artifact_reseal(request.clone(), resealed.envelope()),
        );
        let dispositions = [
            left.expect("first concurrent outcome").disposition(),
            right.expect("second concurrent outcome").disposition(),
        ];
        assert!(dispositions.contains(&PrivateArtifactResealDisposition::Committed));
        assert!(dispositions.contains(&PrivateArtifactResealDisposition::Replayed));
        let replayed = store
            .commit_private_artifact_reseal(request.clone(), resealed.envelope())
            .await
            .expect("lost-response replay");
        assert_eq!(
            replayed.disposition(),
            PrivateArtifactResealDisposition::Replayed
        );
        let current = store
            .encrypted_private_artifact(metadata.artifact_id())
            .await
            .expect("current envelope")
            .expect("v2 envelope");
        assert_eq!(current.version(), 2);
        current
            .open(&VectorWrapping, &context)
            .await
            .expect("context-bound open")
            .expose_secret(|plaintext| assert_eq!(plaintext, b"radroots envelope vector"));
        assert_eq!(
            store
                .private_artifact_envelope_migration_status()
                .await
                .expect("migration status"),
            PrivateArtifactEnvelopeMigrationStatus {
                v1_pending: 0,
                v2_current: 1,
                corrupt: 0,
                blocked_provider: 0,
                conflicted: 0,
            }
        );

        let conflict = PrivateArtifactResealRequest::new(
            request.reseal_id(),
            request.artifact_id(),
            request.expected_revision(),
            request.expected_commitment(),
            ArtifactCommitment::new([0x66; 32]),
            request.next_protected_size_bytes(),
            request.next_secret_reference().clone(),
            request.committed_at_unix_ms(),
        )
        .expect("conflicting request");
        assert_eq!(
            store
                .commit_private_artifact_reseal(conflict, resealed.envelope())
                .await,
            Err(Error::PrivateArtifactResealConflict)
        );
        assert!(
            sqlx::query(
                "UPDATE radroots_private_artifacts SET encrypted_envelope = encrypted_envelope
                 WHERE artifact_id = ?",
            )
            .bind(metadata.artifact_id().as_bytes().as_slice())
            .execute(store.private_pool())
            .await
            .is_err()
        );
        assert!(
            sqlx::query("DELETE FROM radroots_private_envelope_reseals")
                .execute(store.private_pool())
                .await
                .is_err()
        );
    }
}

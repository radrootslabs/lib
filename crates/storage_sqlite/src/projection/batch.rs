use super::{SqliteStorage, decode_status, map_backend, put_status_transaction};
use radroots_storage::{
    Error,
    event::EventSourceToken,
    projection::{
        ProjectionCheckpoint, ProjectionDocument, ProjectionGeneration, ProjectionHealth,
        ProjectionId, ProjectionStatus, batch::ProjectionDocumentBatch,
    },
};
use sqlx::{Row, Sqlite, Transaction};

pub(super) async fn fence(
    transaction: &mut Transaction<'_, Sqlite>,
    store: &SqliteStorage,
    source: EventSourceToken,
    id: &ProjectionId,
    expected: Option<ProjectionGeneration>,
) -> Result<u64, Error> {
    if source.generation() != store.generation {
        return Err(Error::SourceEpochChanged);
    }
    let row = sqlx::query("SELECT mutation_epoch, sequence_head FROM radroots_runtime_source_generations WHERE generation = ? AND state = 'active'")
        .bind(source.generation().as_bytes().as_slice()).fetch_optional(&mut **transaction).await.map_err(map_backend)?
        .ok_or(Error::SourceEpochChanged)?;
    let epoch = super::u64_from_i64(row.try_get("mutation_epoch").map_err(super::map_corrupt)?)?;
    if epoch != source.epoch() {
        return Err(Error::SourceEpochChanged);
    }
    let status = sqlx::query(
        "SELECT * FROM radroots_runtime_projection_checkpoints WHERE projection_id = ?",
    )
    .bind(id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_backend)?
    .as_ref()
    .map(decode_status)
    .transpose()?;
    if status.as_ref().map(ProjectionStatus::generation) != expected
        || status
            .as_ref()
            .is_some_and(|value| value.health() != ProjectionHealth::Ready)
    {
        return Err(Error::ProjectionRevisionConflict);
    }
    super::u64_from_i64(row.try_get("sequence_head").map_err(super::map_corrupt)?)
}

pub(super) async fn read_fence(
    transaction: &mut Transaction<'_, Sqlite>,
    store: &SqliteStorage,
    source: EventSourceToken,
    id: &ProjectionId,
    active: ProjectionGeneration,
) -> Result<(), Error> {
    fence(transaction, store, source, id, Some(active)).await?;
    let row = sqlx::query("SELECT source_generation, source_epoch, phase FROM radroots_runtime_projection_generations WHERE projection_id=? AND generation=?")
        .bind(id.as_str()).bind(active.as_bytes().as_slice()).fetch_optional(&mut **transaction).await.map_err(map_backend)?
        .ok_or(Error::ProjectionRevisionConflict)?;
    let phase: String = row.try_get("phase").map_err(super::map_corrupt)?;
    if phase != "promoted" {
        return Err(Error::ProjectionRevisionConflict);
    }
    let generation: Vec<u8> = row
        .try_get("source_generation")
        .map_err(super::map_corrupt)?;
    let epoch = super::u64_from_i64(row.try_get("source_epoch").map_err(super::map_corrupt)?)?;
    if generation != source.generation().as_bytes() || epoch != source.epoch() {
        return Err(Error::SourceEpochChanged);
    }
    Ok(())
}

pub(super) async fn refuse_registered_legacy(
    transaction: &mut Transaction<'_, Sqlite>,
    id: &ProjectionId,
    generation: ProjectionGeneration,
) -> Result<(), Error> {
    let registered: Option<i64> = sqlx::query_scalar("SELECT 1 FROM radroots_runtime_projection_generations WHERE projection_id=? AND generation=?")
        .bind(id.as_str()).bind(generation.as_bytes().as_slice()).fetch_optional(&mut **transaction).await.map_err(map_backend)?;
    if registered.is_some() {
        return Err(Error::ProjectionRevisionConflict);
    }
    Ok(())
}

pub(super) async fn write(
    store: &SqliteStorage,
    batch: ProjectionDocumentBatch,
    checkpoint: Option<ProjectionCheckpoint>,
) -> Result<Option<ProjectionStatus>, Error> {
    store.require_projection_writer()?;
    let mut transaction = store
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(map_backend)?;
    let scope = batch.scope();
    let sequence = fence(
        &mut transaction,
        store,
        scope.source(),
        scope.projection_id(),
        scope.expected_active(),
    )
    .await?;
    let prior = sqlx::query("SELECT source_generation, source_epoch, expected_active, phase, document_rows, value_bytes FROM radroots_runtime_projection_generations WHERE projection_id = ? AND generation = ?")
        .bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice())
        .fetch_optional(&mut *transaction).await.map_err(map_backend)?;
    let (mut count, mut bytes) = if let Some(row) = prior {
        let generation: Vec<u8> = row
            .try_get("source_generation")
            .map_err(super::map_corrupt)?;
        let epoch = super::u64_from_i64(row.try_get("source_epoch").map_err(super::map_corrupt)?)?;
        let active: Option<Vec<u8>> = row.try_get("expected_active").map_err(super::map_corrupt)?;
        let phase: String = row.try_get("phase").map_err(super::map_corrupt)?;
        if generation != scope.source().generation().as_bytes()
            || epoch != scope.source().epoch()
            || active.as_deref()
                != scope
                    .expected_active()
                    .as_ref()
                    .map(|value| value.as_bytes().as_slice())
            || phase != "staging"
        {
            return Err(Error::ProjectionRevisionConflict);
        }
        (
            super::u64_from_i64(row.try_get("document_rows").map_err(super::map_corrupt)?)?,
            super::u64_from_i64(row.try_get("value_bytes").map_err(super::map_corrupt)?)?,
        )
    } else {
        let staging: Option<i64> = sqlx::query_scalar("SELECT 1 FROM radroots_runtime_projection_generations WHERE projection_id = ? AND phase = 'staging' LIMIT 1")
            .bind(scope.projection_id().as_str()).fetch_optional(&mut *transaction).await.map_err(map_backend)?;
        let existing: Option<i64> = sqlx::query_scalar("SELECT 1 FROM radroots_runtime_projection_documents WHERE projection_id = ? AND generation = ? LIMIT 1")
            .bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice())
            .fetch_optional(&mut *transaction).await.map_err(map_backend)?;
        if staging.is_some() || existing.is_some() {
            return Err(Error::ProjectionRevisionConflict);
        }
        sqlx::query("INSERT INTO radroots_runtime_projection_generations (projection_id,generation,source_generation,source_epoch,expected_active,phase) VALUES (?,?,?,?,?,'staging')")
            .bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice())
            .bind(scope.source().generation().as_bytes().as_slice()).bind(super::i64_from_u64(scope.source().epoch())?)
            .bind(scope.expected_active().map(|value| value.as_bytes().to_vec())).execute(&mut *transaction).await.map_err(map_backend)?;
        (0, 0)
    };
    for document in batch.documents() {
        let previous: Option<i64> = sqlx::query_scalar("SELECT length(value) FROM radroots_runtime_projection_documents WHERE projection_id = ? AND generation = ? AND document_key = ?")
            .bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).bind(document.key())
            .fetch_optional(&mut *transaction).await.map_err(map_backend)?;
        count = count
            .checked_add(u64::from(previous.is_none()))
            .ok_or(Error::SpaceInsufficient)?;
        let previous = previous.map(super::u64_from_i64).transpose()?.unwrap_or(0);
        bytes = bytes
            .checked_sub(previous)
            .and_then(|size| size.checked_add(document.value().len() as u64))
            .ok_or(Error::CorruptProjectionRecord)?;
        put(
            &mut transaction,
            scope.projection_id(),
            scope.generation(),
            document,
        )
        .await?;
    }
    let promoted = if let Some(checkpoint) = checkpoint {
        if checkpoint.projection_id() != scope.projection_id()
            || checkpoint.generation() != scope.generation()
            || checkpoint.projected_rows() != count
            || match checkpoint.source_position() {
                Some(position) => {
                    position.generation() != scope.source().generation()
                        || position.sequence().get() != sequence
                }
                None => sequence != 0,
            }
        {
            return Err(Error::ProjectionCheckpointMismatch);
        }
        let status = ProjectionStatus::new(
            scope.projection_id().clone(),
            scope.generation(),
            ProjectionHealth::Ready,
            Some(checkpoint),
            None,
        )?;
        sqlx::query("UPDATE radroots_runtime_projection_generations SET phase = 'retired' WHERE projection_id = ? AND phase = 'promoted'")
            .bind(scope.projection_id().as_str()).execute(&mut *transaction).await.map_err(map_backend)?;
        put_status_transaction(&mut transaction, &status).await?;
        Some(status)
    } else {
        None
    };
    sqlx::query("UPDATE radroots_runtime_projection_generations SET document_rows = ?, value_bytes = ?, phase = ? WHERE projection_id = ? AND generation = ?")
        .bind(super::i64_from_u64(count)?).bind(super::i64_from_u64(bytes)?)
        .bind(if promoted.is_some() { "promoted" } else { "staging" })
        .bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice())
        .execute(&mut *transaction).await.map_err(map_backend)?;
    transaction.commit().await.map_err(map_backend)?;
    Ok(promoted)
}

pub(super) async fn put(
    transaction: &mut Transaction<'_, Sqlite>,
    id: &ProjectionId,
    generation: ProjectionGeneration,
    document: &ProjectionDocument,
) -> Result<(), Error> {
    sqlx::query("INSERT INTO radroots_runtime_projection_documents (projection_id,generation,document_key,value,value_sha256) VALUES (?,?,?,?,?) ON CONFLICT(projection_id,generation,document_key) DO UPDATE SET value=excluded.value,value_sha256=excluded.value_sha256")
        .bind(id.as_str()).bind(generation.as_bytes().as_slice()).bind(document.key()).bind(document.value())
        .bind(document.value_sha256().as_slice()).execute(&mut **transaction).await.map_err(map_backend)?;
    Ok(())
}

pub(super) async fn legacy_put(
    store: &SqliteStorage,
    id: ProjectionId,
    generation: ProjectionGeneration,
    document: ProjectionDocument,
) -> Result<(), Error> {
    store.require_projection_writer()?;
    let mut transaction = store
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(map_backend)?;
    let registered: Option<i64> = sqlx::query_scalar("SELECT 1 FROM radroots_runtime_projection_generations WHERE projection_id=? AND generation=?")
        .bind(id.as_str()).bind(generation.as_bytes().as_slice()).fetch_optional(&mut *transaction).await.map_err(map_backend)?;
    if registered.is_some() {
        return Err(Error::ProjectionRevisionConflict);
    }
    put(&mut transaction, &id, generation, &document).await?;
    transaction.commit().await.map_err(map_backend)
}

pub(super) async fn load_document(
    store: &SqliteStorage,
    id: ProjectionId,
    generation: ProjectionGeneration,
    key: String,
) -> Result<Option<ProjectionDocument>, Error> {
    radroots_storage::projection::document_query::ProjectionDocumentRecord::corrupt(
        generation,
        key.clone(),
    )
    .map_err(|_| Error::InvalidProjectionDocument)?;
    let mut transaction = store.pool().begin().await.map_err(map_backend)?;
    let metadata = sqlx::query("SELECT length(value) AS value_bytes, CASE WHEN length(value_sha256)=32 THEN value_sha256 END AS value_sha256 FROM radroots_runtime_projection_documents WHERE projection_id=? AND generation=? AND document_key=?")
        .bind(id.as_str()).bind(generation.as_bytes().as_slice()).bind(&key)
        .fetch_optional(&mut *transaction).await.map_err(map_backend)?;
    let document = if let Some(row) = metadata {
        let length: i64 = row.try_get("value_bytes").map_err(super::map_corrupt)?;
        if length <= 0
            || length > radroots_storage::projection::PROJECTION_DOCUMENT_VALUE_MAX_BYTES as i64
        {
            return Err(Error::CorruptProjectionDocument);
        }
        let digest: Vec<u8> = row
            .try_get("value_sha256")
            .map_err(|_| Error::CorruptProjectionDocument)?;
        let digest: [u8; 32] = digest
            .try_into()
            .map_err(|_| Error::CorruptProjectionDocument)?;
        let value: Vec<u8> = sqlx::query_scalar("SELECT value FROM radroots_runtime_projection_documents WHERE projection_id=? AND generation=? AND document_key=?")
            .bind(id.as_str()).bind(generation.as_bytes().as_slice()).bind(&key)
            .fetch_one(&mut *transaction).await.map_err(map_backend)?;
        Some(ProjectionDocument::from_stored_parts(key, value, digest)?)
    } else {
        None
    };
    transaction.rollback().await.map_err(map_backend)?;
    Ok(document)
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;

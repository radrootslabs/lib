use super::{SqliteStorage, decode_status, i64_from_u64, map_backend, map_corrupt, u64_from_i64};
use radroots_storage::{
    Error,
    projection::{
        ProjectionHealth, ProjectionId, ProjectionSnapshot,
        cache::{
            PROJECTION_CACHE_COLLECTION_ROWS_MAX, ProjectionCacheCollection, ProjectionCacheLimits,
            ProjectionCacheScope, ProjectionCacheUsage,
        },
    },
};
use sqlx::{Row, Sqlite, Transaction};

pub(super) fn map(error: sqlx::Error) -> Error {
    if error
        .as_database_error()
        .is_some_and(|value| value.message() == "projection cache capacity exceeded")
    {
        Error::SpaceInsufficient
    } else {
        map_backend(error)
    }
}
fn now() -> Result<u64, Error> {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::InvalidProjectionTimestamp)?
            .as_millis(),
    )
    .map_err(|_| Error::InvalidProjectionTimestamp)
}
pub(super) async fn owner(
    transaction: &mut Transaction<'_, Sqlite>,
    id: &ProjectionId,
) -> Result<bool, Error> {
    let row: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM radroots_runtime_projection_cache_owners WHERE projection_id=?",
    )
    .bind(id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map)?;
    Ok(row.is_some())
}
async fn usage(transaction: &mut Transaction<'_, Sqlite>) -> Result<ProjectionCacheUsage, Error> {
    let row = sqlx::query("SELECT value_bytes,protected_generations,legacy_snapshots FROM radroots_runtime_projection_cache_budget WHERE singleton=1").fetch_one(&mut **transaction).await.map_err(map)?;
    Ok(ProjectionCacheUsage {
        value_bytes: u64_from_i64(row.try_get("value_bytes").map_err(map_corrupt)?)?,
        protected_generations: u64_from_i64(
            row.try_get("protected_generations").map_err(map_corrupt)?,
        )?,
        legacy_snapshots: u64_from_i64(row.try_get("legacy_snapshots").map_err(map_corrupt)?)?,
    })
}
async fn limits(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<Option<ProjectionCacheLimits>, Error> {
    let row = sqlx::query("SELECT max_value_bytes,max_protected_generations,max_legacy_snapshots,legacy_age_ms FROM radroots_runtime_projection_cache_budget WHERE singleton=1").fetch_one(&mut **transaction).await.map_err(map)?;
    let bytes: Option<i64> = row.try_get("max_value_bytes").map_err(map_corrupt)?;
    bytes
        .map(|bytes| {
            ProjectionCacheLimits::new(
                u64_from_i64(bytes)?,
                u64_from_i64(
                    row.try_get("max_protected_generations")
                        .map_err(map_corrupt)?,
                )?,
                u64_from_i64(row.try_get("max_legacy_snapshots").map_err(map_corrupt)?)?,
                u64_from_i64(row.try_get("legacy_age_ms").map_err(map_corrupt)?)?,
            )
            .map_err(|_| Error::CorruptProjectionRecord)
        })
        .transpose()
}
async fn time(transaction: &mut Transaction<'_, Sqlite>, now: u64) -> Result<u64, Error> {
    let clock: i64 = sqlx::query_scalar("UPDATE radroots_runtime_projection_cache_budget SET owner_time_ms=max(owner_time_ms,?) WHERE singleton=1 RETURNING owner_time_ms").bind(i64_from_u64(now)?).fetch_one(&mut **transaction).await.map_err(map)?;
    u64_from_i64(clock)
}
pub(super) async fn configure(
    store: &SqliteStorage,
    id: ProjectionId,
    requested: ProjectionCacheLimits,
) -> Result<ProjectionCacheUsage, Error> {
    store.require_projection_writer()?;
    ProjectionId::parse(id.as_str())?;
    let mut transaction = store
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(map)?;
    if limits(&mut transaction)
        .await?
        .is_some_and(|current| current != requested)
    {
        return Err(Error::ProjectionRevisionConflict);
    }
    sqlx::query("UPDATE radroots_runtime_projection_cache_budget SET max_value_bytes=?,max_protected_generations=?,max_legacy_snapshots=?,legacy_age_ms=? WHERE singleton=1")
        .bind(i64_from_u64(requested.value_bytes())?).bind(i64_from_u64(requested.protected_generations())?).bind(i64_from_u64(requested.legacy_snapshots())?).bind(i64_from_u64(requested.legacy_age_ms())?).execute(&mut *transaction).await.map_err(map)?;
    sqlx::query("INSERT INTO radroots_runtime_projection_cache_owners(projection_id) VALUES(?) ON CONFLICT DO NOTHING").bind(id.as_str()).execute(&mut *transaction).await.map_err(map)?;
    time(&mut transaction, now()?).await?;
    let current = usage(&mut transaction).await?;
    transaction.commit().await.map_err(map)?;
    Ok(current)
}
async fn fence(
    transaction: &mut Transaction<'_, Sqlite>,
    store: &SqliteStorage,
    scope: &ProjectionCacheScope,
) -> Result<Option<radroots_storage::projection::ProjectionStatus>, Error> {
    if scope.source().generation() != store.generation {
        return Err(Error::SourceEpochChanged);
    }
    let epoch: Option<i64> = sqlx::query_scalar("SELECT mutation_epoch FROM radroots_runtime_source_generations WHERE generation=? AND state='active'").bind(scope.source().generation().as_bytes().as_slice()).fetch_optional(&mut **transaction).await.map_err(map)?;
    if epoch.map(u64_from_i64).transpose()? != Some(scope.source().epoch()) {
        return Err(Error::SourceEpochChanged);
    }
    let status =
        sqlx::query("SELECT * FROM radroots_runtime_projection_checkpoints WHERE projection_id=?")
            .bind(scope.projection_id().as_str())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(map)?
            .as_ref()
            .map(decode_status)
            .transpose()?;
    if !owner(transaction, scope.projection_id()).await?
        || status.as_ref().map(|value| value.generation()) != scope.expected_active()
    {
        return Err(Error::ProjectionRevisionConflict);
    }
    Ok(status)
}
pub(super) async fn retire(
    store: &SqliteStorage,
    scope: ProjectionCacheScope,
) -> Result<ProjectionCacheUsage, Error> {
    store.require_projection_writer()?;
    let mut transaction = store
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(map)?;
    let status = fence(&mut transaction, store, &scope).await?;
    let row = sqlx::query("SELECT source_generation,source_epoch FROM radroots_runtime_projection_generations WHERE projection_id=? AND generation=?").bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).fetch_optional(&mut *transaction).await.map_err(map)?;
    if scope.expected_active() == Some(scope.generation())
        && status
            .as_ref()
            .is_some_and(|value| value.health() == ProjectionHealth::Ready)
    {
        let stale = if let Some(row) = row.as_ref() {
            let generation: Vec<u8> = row.try_get("source_generation").map_err(map_corrupt)?;
            generation != scope.source().generation().as_bytes()
                || u64_from_i64(row.try_get("source_epoch").map_err(map_corrupt)?)?
                    != scope.source().epoch()
        } else {
            false
        };
        if !stale {
            return Err(Error::ProjectionRevisionConflict);
        }
    }
    sqlx::query("INSERT INTO radroots_runtime_projection_generations(projection_id,generation,source_generation,source_epoch,expected_active,phase) VALUES(?,?,?,?,NULL,'retired') ON CONFLICT(projection_id,generation) DO UPDATE SET phase='retired'")
        .bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).bind(scope.source().generation().as_bytes().as_slice()).bind(i64_from_u64(scope.source().epoch())?).execute(&mut *transaction).await.map_err(map)?;
    if scope.expected_active() == Some(scope.generation()) {
        sqlx::query("DELETE FROM radroots_runtime_projection_checkpoints WHERE projection_id=? AND projection_generation=?").bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).execute(&mut *transaction).await.map_err(map)?;
    }
    let current = usage(&mut transaction).await?;
    transaction.commit().await.map_err(map)?;
    Ok(current)
}
pub(super) async fn collect(
    store: &SqliteStorage,
    scope: ProjectionCacheScope,
) -> Result<ProjectionCacheCollection, Error> {
    store.require_projection_writer()?;
    let mut transaction = store
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(map)?;
    fence(&mut transaction, store, &scope).await?;
    let phase: Option<String> = sqlx::query_scalar("SELECT phase FROM radroots_runtime_projection_generations WHERE projection_id=? AND generation=?").bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).fetch_optional(&mut *transaction).await.map_err(map)?;
    let protected = scope.expected_active() == Some(scope.generation())
        || phase.as_deref().is_some_and(|value| value != "retired");
    let now = time(&mut transaction, now()?).await?;
    let limit = limits(&mut transaction)
        .await?
        .ok_or(Error::ProjectionRevisionConflict)?;
    let mut removed = 0u64;
    let mut freed = 0u64;
    if !protected {
        let rows = sqlx::query("SELECT document_key,length(value) AS bytes FROM radroots_runtime_projection_documents WHERE projection_id=? AND generation=? ORDER BY document_key LIMIT 64")
            .bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).fetch_all(&mut *transaction).await.map_err(map)?;
        for row in rows {
            let key: String = row.try_get("document_key").map_err(map_corrupt)?;
            let bytes = u64_from_i64(row.try_get("bytes").map_err(map_corrupt)?)?;
            sqlx::query("DELETE FROM radroots_runtime_projection_documents WHERE projection_id=? AND generation=? AND document_key=?").bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).bind(key).execute(&mut *transaction).await.map_err(map)?;
            removed += 1;
            freed = freed
                .checked_add(bytes)
                .ok_or(Error::CorruptProjectionRecord)?;
        }
    }
    let mut current = usage(&mut transaction).await?;
    let rows = sqlx::query("SELECT snapshot_id,created_at_unix_ms,length(value) AS bytes FROM radroots_runtime_projection_snapshots INDEXED BY radroots_runtime_projection_snapshots_generation_created_idx WHERE projection_id=? AND generation=? ORDER BY created_at_unix_ms,snapshot_id LIMIT ?")
        .bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).bind((PROJECTION_CACHE_COLLECTION_ROWS_MAX as u64-removed) as i64).fetch_all(&mut *transaction).await.map_err(map)?;
    let mut future_after = now;
    let mut future_id = vec![255; 32];
    for row in rows {
        let created = u64_from_i64(row.try_get("created_at_unix_ms").map_err(map_corrupt)?)?;
        if protected
            && created <= now
            && now - created < limit.legacy_age_ms()
            && current.legacy_snapshots <= limit.legacy_snapshots()
        {
            break;
        }
        let id: Vec<u8> = row.try_get("snapshot_id").map_err(map_corrupt)?;
        let bytes = u64_from_i64(row.try_get("bytes").map_err(map_corrupt)?)?;
        delete_snapshot(&mut transaction, scope.projection_id(), &id).await?;
        if created > now {
            future_after = created;
            future_id = id;
        }
        removed += 1;
        freed = freed
            .checked_add(bytes)
            .ok_or(Error::CorruptProjectionRecord)?;
        current.legacy_snapshots -= 1;
    }
    if protected && removed < PROJECTION_CACHE_COLLECTION_ROWS_MAX as u64 {
        let rows = sqlx::query("SELECT snapshot_id,length(value) AS bytes FROM radroots_runtime_projection_snapshots INDEXED BY radroots_runtime_projection_snapshots_generation_created_idx WHERE projection_id=? AND generation=? AND (created_at_unix_ms,snapshot_id)>(?,?) ORDER BY created_at_unix_ms,snapshot_id LIMIT ?")
            .bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).bind(i64_from_u64(future_after)?).bind(future_id).bind((PROJECTION_CACHE_COLLECTION_ROWS_MAX as u64-removed) as i64).fetch_all(&mut *transaction).await.map_err(map)?;
        for row in rows {
            let id: Vec<u8> = row.try_get("snapshot_id").map_err(map_corrupt)?;
            delete_snapshot(&mut transaction, scope.projection_id(), &id).await?;
            removed += 1;
            freed = freed
                .checked_add(u64_from_i64(row.try_get("bytes").map_err(map_corrupt)?)?)
                .ok_or(Error::CorruptProjectionRecord)?;
        }
    }
    let document_more: bool = !protected && sqlx::query_scalar::<_,i64>("SELECT EXISTS(SELECT 1 FROM radroots_runtime_projection_documents WHERE projection_id=? AND generation=?)").bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).fetch_one(&mut *transaction).await.map_err(map)? != 0;
    current = usage(&mut transaction).await?;
    let snapshot_sql = "SELECT EXISTS(SELECT 1 FROM radroots_runtime_projection_snapshots INDEXED BY radroots_runtime_projection_snapshots_generation_created_idx WHERE projection_id=? AND generation=? AND created_at_unix_ms<=?)";
    let cutoff = if !protected || current.legacy_snapshots > limit.legacy_snapshots() {
        i64::MAX as u64
    } else {
        now.saturating_sub(limit.legacy_age_ms())
    };
    let expired_more: i64 = sqlx::query_scalar(snapshot_sql)
        .bind(scope.projection_id().as_str())
        .bind(scope.generation().as_bytes().as_slice())
        .bind(i64_from_u64(cutoff)?)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map)?;
    let future_more: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM radroots_runtime_projection_snapshots INDEXED BY radroots_runtime_projection_snapshots_generation_created_idx WHERE projection_id=? AND generation=? AND created_at_unix_ms>?)").bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).bind(i64_from_u64(now)?).fetch_one(&mut *transaction).await.map_err(map)?;
    let snapshot_more = expired_more | future_more;
    let mut registration_more = !protected && phase.is_some();
    if !protected
        && !document_more
        && snapshot_more == 0
        && phase.is_some()
        && removed < PROJECTION_CACHE_COLLECTION_ROWS_MAX as u64
    {
        sqlx::query("DELETE FROM radroots_runtime_projection_generations WHERE projection_id=? AND generation=?").bind(scope.projection_id().as_str()).bind(scope.generation().as_bytes().as_slice()).execute(&mut *transaction).await.map_err(map)?;
        removed += 1;
        registration_more = false;
    }
    let result = ProjectionCacheCollection {
        removed_rows: removed,
        removed_value_bytes: freed,
        more: document_more || snapshot_more != 0 || registration_more,
        usage: usage(&mut transaction).await?,
    };
    transaction.commit().await.map_err(map)?;
    Ok(result)
}
async fn delete_snapshot(
    transaction: &mut Transaction<'_, Sqlite>,
    id: &ProjectionId,
    snapshot: &[u8],
) -> Result<(), Error> {
    sqlx::query(
        "DELETE FROM radroots_runtime_projection_snapshots WHERE projection_id=? AND snapshot_id=?",
    )
    .bind(id.as_str())
    .bind(snapshot)
    .execute(&mut **transaction)
    .await
    .map_err(map)?;
    Ok(())
}
async fn load_snapshot(
    transaction: &mut Transaction<'_, Sqlite>,
    id: &ProjectionId,
    snapshot: &[u8; 32],
    expiry: Option<(ProjectionCacheLimits, u64)>,
) -> Result<Option<ProjectionSnapshot>, Error> {
    let row = sqlx::query("SELECT generation,created_at_unix_ms,length(value) AS bytes,CASE WHEN length(value_sha256)=32 THEN value_sha256 END AS digest FROM radroots_runtime_projection_snapshots WHERE projection_id=? AND snapshot_id=?").bind(id.as_str()).bind(snapshot.as_slice()).fetch_optional(&mut **transaction).await.map_err(map)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let created = u64_from_i64(row.try_get("created_at_unix_ms").map_err(map_corrupt)?)?;
    if expiry.is_some_and(|(limit, now)| created > now || now - created >= limit.legacy_age_ms()) {
        return Ok(None);
    }
    let bytes: i64 = row.try_get("bytes").map_err(map_corrupt)?;
    if bytes <= 0
        || bytes > radroots_storage::projection::PROJECTION_DOCUMENT_VALUE_MAX_BYTES as i64
    {
        return Err(Error::CorruptProjectionDocument);
    }
    let digest: [u8; 32] = row
        .try_get::<Vec<u8>, _>("digest")
        .map_err(|_| Error::CorruptProjectionDocument)?
        .try_into()
        .map_err(|_| Error::CorruptProjectionDocument)?;
    let generation = radroots_storage::projection::ProjectionGeneration::new(
        row.try_get::<Vec<u8>, _>("generation")
            .map_err(map_corrupt)?
            .try_into()
            .map_err(|_| Error::CorruptProjectionDocument)?,
    )?;
    let value: Vec<u8> = sqlx::query_scalar("SELECT value FROM radroots_runtime_projection_snapshots WHERE projection_id=? AND snapshot_id=?").bind(id.as_str()).bind(snapshot.as_slice()).fetch_one(&mut **transaction).await.map_err(map)?;
    ProjectionSnapshot::from_stored_parts(id.clone(), *snapshot, generation, created, value, digest)
        .map(Some)
}
pub(super) async fn snapshot(
    store: &SqliteStorage,
    id: ProjectionId,
    snapshot: [u8; 32],
) -> Result<Option<ProjectionSnapshot>, Error> {
    let readonly = store.event_mode() == radroots_storage::status::EventStoreMode::ReadOnly;
    let mut transaction = if readonly {
        store.pool().begin().await
    } else {
        store.pool().begin_with("BEGIN IMMEDIATE").await
    }
    .map_err(map)?;
    let expiry = if owner(&mut transaction, &id).await? {
        if readonly {
            return Ok(None);
        }
        Some((
            limits(&mut transaction)
                .await?
                .ok_or(Error::CorruptProjectionRecord)?,
            time(&mut transaction, now()?).await?,
        ))
    } else {
        None
    };
    let result = load_snapshot(&mut transaction, &id, &snapshot, expiry).await?;
    if readonly {
        transaction.rollback().await.map_err(map)?;
    } else {
        transaction.commit().await.map_err(map)?;
    }
    Ok(result)
}
pub(super) async fn put_snapshot(
    store: &SqliteStorage,
    snapshot: ProjectionSnapshot,
) -> Result<(), Error> {
    store.require_projection_writer()?;
    let mut transaction = store
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(map)?;
    if let Some(existing) = load_snapshot(
        &mut transaction,
        snapshot.projection_id(),
        snapshot.snapshot_id(),
        None,
    )
    .await?
    {
        return if existing == snapshot {
            Ok(())
        } else {
            Err(Error::CorruptProjectionDocument)
        };
    }
    if owner(&mut transaction, snapshot.projection_id()).await? {
        return Err(Error::ProjectionRevisionConflict);
    }
    sqlx::query("INSERT INTO radroots_runtime_projection_snapshots(projection_id,snapshot_id,generation,created_at_unix_ms,value,value_sha256) VALUES(?,?,?,?,?,?)")
        .bind(snapshot.projection_id().as_str()).bind(snapshot.snapshot_id().as_slice()).bind(snapshot.generation().as_bytes().as_slice()).bind(i64_from_u64(snapshot.created_at_unix_ms())?).bind(snapshot.value()).bind(snapshot.value_sha256().as_slice()).execute(&mut *transaction).await.map_err(map)?;
    transaction.commit().await.map_err(map)
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;

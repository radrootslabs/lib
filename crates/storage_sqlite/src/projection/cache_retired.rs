use super::{SqliteStorage, cache};
use radroots_storage::{
    Error,
    event::EventSourceToken,
    projection::{
        ProjectionGeneration, ProjectionId,
        cache::{ProjectionCacheCollection, ProjectionCacheScope},
    },
};
use sqlx::{Sqlite, Transaction};

const SELECTION: &str = "SELECT CASE WHEN length(generation)=32 THEN generation END
    FROM radroots_runtime_projection_generations INDEXED BY radroots_runtime_projection_retired_generation_idx
    WHERE projection_id=? AND phase='retired' AND (? IS NULL OR generation!=?)
    ORDER BY generation LIMIT 1";

async fn next(
    transaction: &mut Transaction<'_, Sqlite>,
    id: &ProjectionId,
    active: Option<ProjectionGeneration>,
) -> Result<Option<ProjectionGeneration>, Error> {
    let active = active.map(|generation| generation.as_bytes().to_vec());
    let value: Option<Option<Vec<u8>>> = sqlx::query_scalar(SELECTION)
        .bind(id.as_str())
        .bind(active.clone())
        .bind(active)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(cache::map)?;
    value
        .map(|value| {
            let bytes = value.ok_or(Error::CorruptProjectionRecord)?;
            ProjectionGeneration::new(
                bytes
                    .try_into()
                    .map_err(|_| Error::CorruptProjectionRecord)?,
            )
            .map_err(|_| Error::CorruptProjectionRecord)
        })
        .transpose()
}

pub(super) async fn collect(
    store: &SqliteStorage,
    id: ProjectionId,
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
) -> Result<ProjectionCacheCollection, Error> {
    store.require_projection_writer()?;
    let mut transaction = store
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(cache::map)?;
    cache::fence_owner(&mut transaction, store, &id, source, active).await?;
    if !cache::owner(&mut transaction, &id).await? {
        return Err(Error::ProjectionRevisionConflict);
    }
    let mut result = if let Some(retired) = next(&mut transaction, &id, active).await? {
        cache::collect_in(
            &mut transaction,
            store,
            &ProjectionCacheScope::new(id.clone(), retired, source, active)?,
            cache::CollectionKind::Generation,
        )
        .await?
    } else {
        ProjectionCacheCollection {
            removed_rows: 0,
            removed_value_bytes: 0,
            more: false,
            usage: cache::usage(&mut transaction).await?,
        }
    };
    result.more |= next(&mut transaction, &id, active).await?.is_some();
    transaction.commit().await.map_err(cache::map)?;
    Ok(result)
}

#[cfg(test)]
#[path = "cache_retired_owner_tests.rs"]
mod tests;

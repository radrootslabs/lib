use crate::SqliteStorage;
use radroots_storage::{
    Error, ProjectionStore,
    event::{EventPosition, SourceGeneration},
    projection::{
        ArtifactDigest, BoxFuture, EVENT_INDEX_SHARDS_MAX, EventId, EventIdRange,
        EventIndexCheckpoint, EventIndexManifest, EventIndexShard, EventIndexShardCheckpoint,
        EventIndexShardId, InvalidationReason, ProjectionCheckpoint, ProjectionGeneration,
        ProjectionHealth, ProjectionId, ProjectionInvalidation, ProjectionRevision,
        ProjectionStatus, RebuildStage, RebuildTicket, RebuildTicketId, RebuildTransition,
    },
};
use sqlx::{Row, Sqlite, SqliteConnection};

#[cfg_attr(coverage_nightly, coverage(off))]
impl ProjectionStore for SqliteStorage {
    fn status(
        &self,
        projection_id: ProjectionId,
    ) -> BoxFuture<'_, Result<Option<ProjectionStatus>, Error>> {
        Box::pin(async move {
            sqlx::query(
                "SELECT * FROM radroots_runtime_projection_checkpoints WHERE projection_id = ?",
            )
            .bind(projection_id.as_str())
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            .as_ref()
            .map(decode_status)
            .transpose()
        })
    }

    fn checkpoint(
        &self,
        checkpoint: ProjectionCheckpoint,
    ) -> BoxFuture<'_, Result<ProjectionStatus, Error>> {
        Box::pin(async move {
            self.require_projection_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let status = checkpoint_transaction(&mut transaction, checkpoint).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(status)
        })
    }

    fn invalidate(
        &self,
        invalidation: ProjectionInvalidation,
    ) -> BoxFuture<'_, Result<ProjectionStatus, Error>> {
        Box::pin(async move {
            self.require_projection_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let row = sqlx::query(
                "SELECT * FROM radroots_runtime_projection_checkpoints WHERE projection_id = ?",
            )
            .bind(invalidation.projection_id().as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_backend)?
            .ok_or(Error::ProjectionCheckpointMismatch)?;
            let current = decode_status(&row)?;
            if current.generation() != invalidation.invalid_generation() {
                return Err(Error::ProjectionCheckpointMismatch);
            }
            sqlx::query(
                "INSERT INTO radroots_runtime_projection_invalidations (
                   projection_id, invalid_generation, replacement_generation, reason,
                   invalidated_at_unix_ms
                 ) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(invalidation.projection_id().as_str())
            .bind(invalidation.invalid_generation().as_bytes().as_slice())
            .bind(invalidation.replacement_generation().as_bytes().as_slice())
            .bind(reason_name(invalidation.reason()))
            .bind(i64_from_u64(invalidation.invalidated_at_unix_ms())?)
            .execute(&mut *transaction)
            .await
            .map_err(map_backend)?;
            let next = ProjectionStatus::new(
                invalidation.projection_id().clone(),
                invalidation.replacement_generation(),
                ProjectionHealth::Invalidated,
                None,
                None,
            )?;
            put_status_transaction(&mut transaction, &next).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(next)
        })
    }

    fn request_rebuild(
        &self,
        ticket: RebuildTicket,
    ) -> BoxFuture<'_, Result<RebuildTicket, Error>> {
        Box::pin(async move {
            self.require_projection_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            if let Some(row) = sqlx::query(
                "SELECT * FROM radroots_runtime_projection_rebuilds WHERE ticket_id = ?",
            )
            .bind(ticket.ticket_id().as_bytes().as_slice())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_backend)?
            {
                let existing = decode_ticket(&mut transaction, &row).await?;
                return if existing == ticket {
                    transaction.commit().await.map_err(map_backend)?;
                    Ok(existing)
                } else {
                    Err(Error::ProjectionRevisionConflict)
                };
            }
            let status_row = sqlx::query(
                "SELECT * FROM radroots_runtime_projection_checkpoints WHERE projection_id = ?",
            )
            .bind(ticket.invalidation().projection_id().as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_backend)?
            .ok_or(Error::ProjectionCheckpointMismatch)?;
            let status = decode_status(&status_row)?;
            if status.generation() != ticket.invalidation().replacement_generation()
                || !matches!(
                    status.health(),
                    ProjectionHealth::Invalidated | ProjectionHealth::Failed
                )
                || load_invalidation(
                    &mut transaction,
                    ticket.invalidation().projection_id(),
                    ticket.invalidation().invalid_generation(),
                )
                .await?
                .as_ref()
                    != Some(ticket.invalidation())
            {
                return Err(Error::ProjectionCheckpointMismatch);
            }
            insert_ticket(&mut transaction, &ticket).await?;
            let next = ProjectionStatus::new(
                status.projection_id().clone(),
                status.generation(),
                ProjectionHealth::Rebuilding,
                None,
                Some(ticket.ticket_id()),
            )?;
            put_status_transaction(&mut transaction, &next).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(ticket)
        })
    }

    fn invalidation(
        &self,
        projection_id: ProjectionId,
        replacement_generation: ProjectionGeneration,
    ) -> BoxFuture<'_, Result<Option<ProjectionInvalidation>, Error>> {
        Box::pin(async move {
            sqlx::query(
                "SELECT * FROM radroots_runtime_projection_invalidations
                 WHERE projection_id = ? AND replacement_generation = ?
                 ORDER BY invalidated_at_unix_ms DESC LIMIT 1",
            )
            .bind(projection_id.as_str())
            .bind(replacement_generation.as_bytes().as_slice())
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            .as_ref()
            .map(decode_invalidation)
            .transpose()
        })
    }

    fn rebuild(
        &self,
        ticket_id: RebuildTicketId,
    ) -> BoxFuture<'_, Result<Option<RebuildTicket>, Error>> {
        Box::pin(async move {
            let mut connection = self.pool().acquire().await.map_err(map_backend)?;
            let row = sqlx::query(
                "SELECT * FROM radroots_runtime_projection_rebuilds WHERE ticket_id = ?",
            )
            .bind(ticket_id.as_bytes().as_slice())
            .fetch_optional(&mut *connection)
            .await
            .map_err(map_backend)?;
            match row {
                Some(row) => decode_ticket(&mut connection, &row).await.map(Some),
                None => Ok(None),
            }
        })
    }

    fn transition_rebuild(
        &self,
        transition: RebuildTransition,
    ) -> BoxFuture<'_, Result<RebuildTicket, Error>> {
        Box::pin(async move {
            self.require_projection_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let row = sqlx::query(
                "SELECT * FROM radroots_runtime_projection_rebuilds WHERE ticket_id = ?",
            )
            .bind(transition.ticket_id().as_bytes().as_slice())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_backend)?
            .ok_or(Error::ProjectionRevisionConflict)?;
            let current = decode_ticket(&mut transaction, &row).await?;
            let status_row = sqlx::query(
                "SELECT * FROM radroots_runtime_projection_checkpoints WHERE projection_id = ?",
            )
            .bind(current.invalidation().projection_id().as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_backend)?
            .ok_or(Error::CorruptProjectionRecord)?;
            let current_status = decode_status(&status_row)?;
            if current_status.generation() != current.invalidation().replacement_generation()
                || current_status.health() != ProjectionHealth::Rebuilding
                || current_status.active_rebuild() != Some(current.ticket_id())
            {
                return Err(Error::CorruptProjectionRecord);
            }
            let next = current.transition(transition)?;
            update_ticket(&mut transaction, &next, current.revision()).await?;
            let (health, active_rebuild) = match next.stage() {
                RebuildStage::Requested | RebuildStage::Running => {
                    (ProjectionHealth::Rebuilding, Some(next.ticket_id()))
                }
                RebuildStage::Completed => (ProjectionHealth::Ready, None),
                RebuildStage::Failed => (ProjectionHealth::Failed, None),
            };
            let status = ProjectionStatus::new(
                next.invalidation().projection_id().clone(),
                next.invalidation().replacement_generation(),
                health,
                next.checkpoint().cloned(),
                active_rebuild,
            )?;
            put_status_transaction(&mut transaction, &status).await?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(next)
        })
    }

    fn event_index_manifest(
        &self,
        generation: ProjectionGeneration,
    ) -> BoxFuture<'_, Result<Option<EventIndexManifest>, Error>> {
        Box::pin(async move {
            let mut connection = self.pool().acquire().await.map_err(map_backend)?;
            load_manifest(&mut connection, generation).await
        })
    }

    fn put_event_index_manifest(
        &self,
        manifest: EventIndexManifest,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            self.require_projection_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            if let Some(existing) = load_manifest(&mut transaction, manifest.generation()).await? {
                return if existing == manifest {
                    transaction.commit().await.map_err(map_backend)?;
                    Ok(())
                } else {
                    Err(Error::CorruptProjectionRecord)
                };
            }
            sqlx::query(
                "INSERT INTO radroots_runtime_event_index_manifests (
                   projection_generation, total_events, target_shard_size,
                   first_published_at_unix_s, last_published_at_unix_s
                 ) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(manifest.generation().as_bytes().as_slice())
            .bind(i64_from_u64(manifest.total_events())?)
            .bind(i64::from(manifest.target_shard_size()))
            .bind(i64_from_u64(manifest.first_published_at_unix_s())?)
            .bind(i64_from_u64(manifest.last_published_at_unix_s())?)
            .execute(&mut *transaction)
            .await
            .map_err(map_backend)?;
            for (ordinal, shard) in manifest.shards().iter().enumerate() {
                sqlx::query(
                    "INSERT INTO radroots_runtime_event_index_shards (
                       projection_generation, shard_id, ordinal, artifact_path, event_count,
                       first_event_id, last_event_id, first_published_at_unix_s,
                       last_published_at_unix_s, artifact_digest
                     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(manifest.generation().as_bytes().as_slice())
                .bind(shard.shard_id().as_str())
                .bind(i64::try_from(ordinal).map_err(|_| Error::CorruptProjectionRecord)?)
                .bind(shard.artifact_path())
                .bind(i64::from(shard.event_count()))
                .bind(shard.event_ids().first().as_bytes().as_slice())
                .bind(shard.event_ids().last().as_bytes().as_slice())
                .bind(i64_from_u64(shard.first_published_at_unix_s())?)
                .bind(i64_from_u64(shard.last_published_at_unix_s())?)
                .bind(shard.sha256().as_bytes().as_slice())
                .execute(&mut *transaction)
                .await
                .map_err(map_backend)?;
            }
            transaction.commit().await.map_err(map_backend)
        })
    }

    fn event_index_checkpoint(
        &self,
        generation: ProjectionGeneration,
    ) -> BoxFuture<'_, Result<Option<EventIndexCheckpoint>, Error>> {
        Box::pin(async move {
            sqlx::query(
                "SELECT generated_at_unix_ms, checkpoint
                 FROM radroots_runtime_event_index_checkpoints
                 WHERE projection_generation = ?",
            )
            .bind(generation.as_bytes().as_slice())
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            .as_ref()
            .map(|row| decode_index_checkpoint(row, generation))
            .transpose()
        })
    }

    fn put_event_index_checkpoint(
        &self,
        checkpoint: EventIndexCheckpoint,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            self.require_projection_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            if let Some(row) = sqlx::query(
                "SELECT generated_at_unix_ms, checkpoint
                 FROM radroots_runtime_event_index_checkpoints
                 WHERE projection_generation = ?",
            )
            .bind(checkpoint.generation().as_bytes().as_slice())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_backend)?
                && checkpoint.generated_at_unix_ms()
                    < decode_index_checkpoint(&row, checkpoint.generation())?.generated_at_unix_ms()
            {
                return Err(Error::InvalidEventIndexCheckpoint);
            }
            sqlx::query(
                "INSERT INTO radroots_runtime_event_index_checkpoints (
                   projection_generation, generated_at_unix_ms, checkpoint
                 ) VALUES (?, ?, ?)
                 ON CONFLICT(projection_generation) DO UPDATE SET
                   generated_at_unix_ms = excluded.generated_at_unix_ms,
                   checkpoint = excluded.checkpoint",
            )
            .bind(checkpoint.generation().as_bytes().as_slice())
            .bind(i64_from_u64(checkpoint.generated_at_unix_ms())?)
            .bind(encode_index_checkpoint(&checkpoint)?)
            .execute(&mut *transaction)
            .await
            .map_err(map_backend)?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(())
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn checkpoint_transaction(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    checkpoint: ProjectionCheckpoint,
) -> Result<ProjectionStatus, Error> {
    let prior = sqlx::query(
        "SELECT * FROM radroots_runtime_projection_checkpoints WHERE projection_id = ?",
    )
    .bind(checkpoint.projection_id().as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_backend)?
    .as_ref()
    .map(decode_status)
    .transpose()?;
    if let Some(prior) = prior.as_ref() {
        if prior.generation() != checkpoint.generation() {
            return Err(Error::ProjectionCheckpointMismatch);
        }
        if prior
            .checkpoint()
            .is_some_and(|value| !checkpoint.advances(value))
        {
            return Err(Error::ProjectionCheckpointRegression);
        }
    }
    let status = ProjectionStatus::new(
        checkpoint.projection_id().clone(),
        checkpoint.generation(),
        ProjectionHealth::Ready,
        Some(checkpoint),
        None,
    )?;
    put_status_transaction(transaction, &status).await?;
    Ok(status)
}

impl SqliteStorage {
    fn require_projection_writer(&self) -> Result<(), Error> {
        if self.event_mode() == radroots_storage::status::EventStoreMode::ReadOnly {
            return Err(Error::BackendUnavailable);
        }
        Ok(())
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn put_status_transaction(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    status: &ProjectionStatus,
) -> Result<(), Error> {
    let values = checkpoint_values(status.checkpoint())?;
    sqlx::query(STATUS_UPSERT)
        .bind(status.projection_id().as_str())
        .bind(status.generation().as_bytes().as_slice())
        .bind(health_name(status.health()))
        .bind(values.0)
        .bind(values.1)
        .bind(values.2)
        .bind(values.3)
        .bind(
            status
                .active_rebuild()
                .map(|ticket| ticket.as_bytes().to_vec()),
        )
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
    Ok(())
}

const STATUS_UPSERT: &str = "INSERT INTO radroots_runtime_projection_checkpoints (
       projection_id, projection_generation, health, source_generation, source_sequence,
       projected_rows, checkpoint_updated_at_unix_ms, active_rebuild
     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(projection_id) DO UPDATE SET
       projection_generation = excluded.projection_generation,
       health = excluded.health,
       source_generation = excluded.source_generation,
       source_sequence = excluded.source_sequence,
       projected_rows = excluded.projected_rows,
       checkpoint_updated_at_unix_ms = excluded.checkpoint_updated_at_unix_ms,
       active_rebuild = excluded.active_rebuild";

type CheckpointValues = (Option<Vec<u8>>, Option<i64>, Option<i64>, Option<i64>);

fn checkpoint_values(checkpoint: Option<&ProjectionCheckpoint>) -> Result<CheckpointValues, Error> {
    let Some(checkpoint) = checkpoint else {
        return Ok((None, None, None, None));
    };
    let (generation, sequence) = checkpoint
        .source_position()
        .map_or((None, None), |position| {
            (
                Some(position.generation().as_bytes().to_vec()),
                Some(i64_from_u64(position.sequence().get())),
            )
        });
    Ok((
        generation,
        sequence.transpose()?,
        Some(i64_from_u64(checkpoint.projected_rows())?),
        Some(i64_from_u64(checkpoint.updated_at_unix_ms())?),
    ))
}

fn decode_status(row: &sqlx::sqlite::SqliteRow) -> Result<ProjectionStatus, Error> {
    let projection_id = ProjectionId::parse(
        row.try_get::<String, _>("projection_id")
            .map_err(map_corrupt)?,
    )
    .map_err(|_| Error::CorruptProjectionRecord)?;
    let generation = projection_generation(row, "projection_generation")?;
    let checkpoint = decode_checkpoint(row, projection_id.clone(), generation, "")?;
    let active = row
        .try_get::<Option<Vec<u8>>, _>("active_rebuild")
        .map_err(map_corrupt)?
        .map(|value| {
            RebuildTicketId::new(array(value)?).map_err(|_| Error::CorruptProjectionRecord)
        })
        .transpose()?;
    ProjectionStatus::new(
        projection_id,
        generation,
        health(
            row.try_get::<String, _>("health")
                .map_err(map_corrupt)?
                .as_str(),
        )?,
        checkpoint,
        active,
    )
}

fn decode_checkpoint(
    row: &sqlx::sqlite::SqliteRow,
    projection_id: ProjectionId,
    generation: ProjectionGeneration,
    prefix: &str,
) -> Result<Option<ProjectionCheckpoint>, Error> {
    let rows = row
        .try_get::<Option<i64>, _>(format!("{prefix}projected_rows").as_str())
        .map_err(map_corrupt)?;
    let updated = row
        .try_get::<Option<i64>, _>(format!("{prefix}updated_at_unix_ms").as_str())
        .or_else(|_| row.try_get(format!("{prefix}checkpoint_updated_at_unix_ms").as_str()))
        .map_err(map_corrupt)?;
    let source_generation = row
        .try_get::<Option<Vec<u8>>, _>(format!("{prefix}source_generation").as_str())
        .map_err(map_corrupt)?;
    let source_sequence = row
        .try_get::<Option<i64>, _>(format!("{prefix}source_sequence").as_str())
        .map_err(map_corrupt)?;
    match (rows, updated, source_generation, source_sequence) {
        (None, None, None, None) => Ok(None),
        (Some(rows), Some(updated), source_generation, source_sequence) => {
            let position = match (source_generation, source_sequence) {
                (None, None) => None,
                (Some(source_generation), Some(source_sequence)) => Some(EventPosition::new(
                    SourceGeneration::new(array(source_generation)?)
                        .map_err(|_| Error::CorruptProjectionRecord)?,
                    radroots_storage::event::EventSequence::new(u64_from_i64(source_sequence)?)
                        .map_err(|_| Error::CorruptProjectionRecord)?,
                )),
                _ => return Err(Error::CorruptProjectionRecord),
            };
            ProjectionCheckpoint::new(
                projection_id,
                generation,
                position,
                u64_from_i64(rows)?,
                u64_from_i64(updated)?,
            )
            .map(Some)
            .map_err(|_| Error::CorruptProjectionRecord)
        }
        _ => Err(Error::CorruptProjectionRecord),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn load_invalidation(
    connection: &mut SqliteConnection,
    projection_id: &ProjectionId,
    generation: ProjectionGeneration,
) -> Result<Option<ProjectionInvalidation>, Error> {
    sqlx::query(
        "SELECT * FROM radroots_runtime_projection_invalidations
         WHERE projection_id = ? AND invalid_generation = ?",
    )
    .bind(projection_id.as_str())
    .bind(generation.as_bytes().as_slice())
    .fetch_optional(&mut *connection)
    .await
    .map_err(map_backend)?
    .as_ref()
    .map(decode_invalidation)
    .transpose()
}

fn decode_invalidation(row: &sqlx::sqlite::SqliteRow) -> Result<ProjectionInvalidation, Error> {
    ProjectionInvalidation::new(
        ProjectionId::parse(
            row.try_get::<String, _>("projection_id")
                .map_err(map_corrupt)?,
        )
        .map_err(|_| Error::CorruptProjectionRecord)?,
        projection_generation(row, "invalid_generation")?,
        projection_generation(row, "replacement_generation")?,
        reason(
            row.try_get::<String, _>("reason")
                .map_err(map_corrupt)?
                .as_str(),
        )?,
        u64_from_i64(row.try_get("invalidated_at_unix_ms").map_err(map_corrupt)?)?,
    )
    .map_err(|_| Error::CorruptProjectionRecord)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn insert_ticket(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    ticket: &RebuildTicket,
) -> Result<(), Error> {
    let checkpoint = checkpoint_values(ticket.checkpoint())?;
    sqlx::query(
        "INSERT INTO radroots_runtime_projection_rebuilds (
           ticket_id, projection_id, invalid_generation, replacement_generation, revision, stage,
           checkpoint_source_generation, checkpoint_source_sequence, checkpoint_projected_rows,
           checkpoint_updated_at_unix_ms, requested_at_unix_ms, updated_at_unix_ms
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(ticket.ticket_id().as_bytes().as_slice())
    .bind(ticket.invalidation().projection_id().as_str())
    .bind(
        ticket
            .invalidation()
            .invalid_generation()
            .as_bytes()
            .as_slice(),
    )
    .bind(
        ticket
            .invalidation()
            .replacement_generation()
            .as_bytes()
            .as_slice(),
    )
    .bind(i64_from_u64(ticket.revision().get())?)
    .bind(rebuild_stage_name(ticket.stage()))
    .bind(checkpoint.0)
    .bind(checkpoint.1)
    .bind(checkpoint.2)
    .bind(checkpoint.3)
    .bind(i64_from_u64(ticket.requested_at_unix_ms())?)
    .bind(i64_from_u64(ticket.updated_at_unix_ms())?)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn update_ticket(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    ticket: &RebuildTicket,
    prior: ProjectionRevision,
) -> Result<(), Error> {
    let checkpoint = checkpoint_values(ticket.checkpoint())?;
    let result = sqlx::query(
        "UPDATE radroots_runtime_projection_rebuilds SET
           revision = ?, stage = ?, checkpoint_source_generation = ?,
           checkpoint_source_sequence = ?, checkpoint_projected_rows = ?,
           checkpoint_updated_at_unix_ms = ?, updated_at_unix_ms = ?
         WHERE ticket_id = ? AND revision = ?",
    )
    .bind(i64_from_u64(ticket.revision().get())?)
    .bind(rebuild_stage_name(ticket.stage()))
    .bind(checkpoint.0)
    .bind(checkpoint.1)
    .bind(checkpoint.2)
    .bind(checkpoint.3)
    .bind(i64_from_u64(ticket.updated_at_unix_ms())?)
    .bind(ticket.ticket_id().as_bytes().as_slice())
    .bind(i64_from_u64(prior.get())?)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    if result.rows_affected() != 1 {
        return Err(Error::ProjectionRevisionConflict);
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn decode_ticket(
    connection: &mut SqliteConnection,
    row: &sqlx::sqlite::SqliteRow,
) -> Result<RebuildTicket, Error> {
    let projection_id = ProjectionId::parse(
        row.try_get::<String, _>("projection_id")
            .map_err(map_corrupt)?,
    )
    .map_err(|_| Error::CorruptProjectionRecord)?;
    let invalid_generation = projection_generation(row, "invalid_generation")?;
    let invalidation = load_invalidation(connection, &projection_id, invalid_generation)
        .await?
        .ok_or(Error::CorruptProjectionRecord)?;
    if invalidation.replacement_generation()
        != projection_generation(row, "replacement_generation")?
    {
        return Err(Error::CorruptProjectionRecord);
    }
    let checkpoint = decode_checkpoint(
        row,
        projection_id,
        invalidation.replacement_generation(),
        "checkpoint_",
    )?;
    RebuildTicket::from_durable_parts(
        RebuildTicketId::new(array(
            row.try_get::<Vec<u8>, _>("ticket_id")
                .map_err(map_corrupt)?,
        )?)
        .map_err(|_| Error::CorruptProjectionRecord)?,
        invalidation,
        ProjectionRevision::new(u64_from_i64(row.try_get("revision").map_err(map_corrupt)?)?)
            .map_err(|_| Error::CorruptProjectionRecord)?,
        rebuild_stage(
            row.try_get::<String, _>("stage")
                .map_err(map_corrupt)?
                .as_str(),
        )?,
        checkpoint,
        u64_from_i64(row.try_get("requested_at_unix_ms").map_err(map_corrupt)?)?,
        u64_from_i64(row.try_get("updated_at_unix_ms").map_err(map_corrupt)?)?,
    )
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn load_manifest(
    connection: &mut SqliteConnection,
    generation: ProjectionGeneration,
) -> Result<Option<EventIndexManifest>, Error> {
    let Some(row) = sqlx::query(
        "SELECT * FROM radroots_runtime_event_index_manifests
         WHERE projection_generation = ?",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_optional(&mut *connection)
    .await
    .map_err(map_backend)?
    else {
        return Ok(None);
    };
    let shards = sqlx::query(
        "SELECT * FROM radroots_runtime_event_index_shards
         WHERE projection_generation = ? ORDER BY ordinal",
    )
    .bind(generation.as_bytes().as_slice())
    .fetch_all(&mut *connection)
    .await
    .map_err(map_backend)?
    .iter()
    .enumerate()
    .map(|(ordinal, row)| {
        if row.try_get::<i64, _>("ordinal").map_err(map_corrupt)?
            != i64::try_from(ordinal).map_err(|_| Error::CorruptProjectionRecord)?
        {
            return Err(Error::CorruptProjectionRecord);
        }
        EventIndexShard::new(
            EventIndexShardId::parse(row.try_get::<String, _>("shard_id").map_err(map_corrupt)?)
                .map_err(|_| Error::CorruptProjectionRecord)?,
            row.try_get::<String, _>("artifact_path")
                .map_err(map_corrupt)?,
            u32::try_from(row.try_get::<i64, _>("event_count").map_err(map_corrupt)?)
                .map_err(|_| Error::CorruptProjectionRecord)?,
            EventIdRange::new(
                event_id(row, "first_event_id")?,
                event_id(row, "last_event_id")?,
            )
            .map_err(|_| Error::CorruptProjectionRecord)?,
            u64_from_i64(
                row.try_get("first_published_at_unix_s")
                    .map_err(map_corrupt)?,
            )?,
            u64_from_i64(
                row.try_get("last_published_at_unix_s")
                    .map_err(map_corrupt)?,
            )?,
            ArtifactDigest::new(array(
                row.try_get::<Vec<u8>, _>("artifact_digest")
                    .map_err(map_corrupt)?,
            )?),
        )
        .map_err(|_| Error::CorruptProjectionRecord)
    })
    .collect::<Result<Vec<_>, _>>()?;
    EventIndexManifest::new(
        generation,
        u64_from_i64(row.try_get("total_events").map_err(map_corrupt)?)?,
        u32::try_from(
            row.try_get::<i64, _>("target_shard_size")
                .map_err(map_corrupt)?,
        )
        .map_err(|_| Error::CorruptProjectionRecord)?,
        u64_from_i64(
            row.try_get("first_published_at_unix_s")
                .map_err(map_corrupt)?,
        )?,
        u64_from_i64(
            row.try_get("last_published_at_unix_s")
                .map_err(map_corrupt)?,
        )?,
        shards,
    )
    .map(Some)
    .map_err(|_| Error::CorruptProjectionRecord)
}

fn encode_index_checkpoint(checkpoint: &EventIndexCheckpoint) -> Result<Vec<u8>, Error> {
    let mut bytes = vec![1];
    let count =
        u16::try_from(checkpoint.shards().len()).map_err(|_| Error::CorruptProjectionRecord)?;
    bytes.extend_from_slice(&count.to_be_bytes());
    for shard in checkpoint.shards() {
        put_string(&mut bytes, shard.shard_id().as_str())?;
        bytes.extend_from_slice(&shard.last_created_at_unix_s().to_be_bytes());
        match shard.last_event_id() {
            Some(event_id) => {
                bytes.push(1);
                bytes.extend_from_slice(event_id.as_bytes());
            }
            None => bytes.push(0),
        }
        match shard.cursor() {
            Some(cursor) => {
                bytes.push(1);
                put_string(&mut bytes, cursor)?;
            }
            None => bytes.push(0),
        }
    }
    Ok(bytes)
}

fn decode_index_checkpoint(
    row: &sqlx::sqlite::SqliteRow,
    generation: ProjectionGeneration,
) -> Result<EventIndexCheckpoint, Error> {
    let bytes = row
        .try_get::<Vec<u8>, _>("checkpoint")
        .map_err(map_corrupt)?;
    let mut cursor = Cursor::new(bytes.as_slice());
    if cursor.byte()? != 1 {
        return Err(Error::CorruptProjectionRecord);
    }
    let count = usize::from(cursor.u16()?);
    if count > EVENT_INDEX_SHARDS_MAX {
        return Err(Error::CorruptProjectionRecord);
    }
    let mut shards = Vec::with_capacity(count);
    for _ in 0..count {
        let shard_id = EventIndexShardId::parse(cursor.string()?.to_owned())
            .map_err(|_| Error::CorruptProjectionRecord)?;
        let last_created_at_unix_s = cursor.u64()?;
        let last_event_id = match cursor.byte()? {
            0 => None,
            1 => Some(EventId::from_bytes(cursor.array()?)),
            _ => return Err(Error::CorruptProjectionRecord),
        };
        let checkpoint_cursor = match cursor.byte()? {
            0 => None,
            1 => Some(cursor.string()?.to_owned()),
            _ => return Err(Error::CorruptProjectionRecord),
        };
        shards.push(
            EventIndexShardCheckpoint::new(
                shard_id,
                last_created_at_unix_s,
                last_event_id,
                checkpoint_cursor,
            )
            .map_err(|_| Error::CorruptProjectionRecord)?,
        );
    }
    cursor.finish()?;
    EventIndexCheckpoint::new(
        generation,
        u64_from_i64(row.try_get("generated_at_unix_ms").map_err(map_corrupt)?)?,
        shards,
    )
    .map_err(|_| Error::CorruptProjectionRecord)
}

pub(crate) fn encode_status_snapshot(status: &ProjectionStatus) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::with_capacity(128);
    bytes.push(1);
    put_string(&mut bytes, status.projection_id().as_str())?;
    bytes.extend_from_slice(status.generation().as_bytes());
    bytes.push(match status.health() {
        ProjectionHealth::Ready => 0,
        ProjectionHealth::Invalidated => 1,
        ProjectionHealth::Rebuilding => 2,
        ProjectionHealth::Failed => 3,
    });
    match status.checkpoint() {
        Some(checkpoint) => {
            bytes.push(1);
            match checkpoint.source_position() {
                Some(position) => {
                    bytes.push(1);
                    bytes.extend_from_slice(position.generation().as_bytes());
                    bytes.extend_from_slice(&position.sequence().get().to_be_bytes());
                }
                None => bytes.push(0),
            }
            bytes.extend_from_slice(&checkpoint.projected_rows().to_be_bytes());
            bytes.extend_from_slice(&checkpoint.updated_at_unix_ms().to_be_bytes());
        }
        None => bytes.push(0),
    }
    match status.active_rebuild() {
        Some(ticket) => {
            bytes.push(1);
            bytes.extend_from_slice(ticket.as_bytes());
        }
        None => bytes.push(0),
    }
    Ok(bytes)
}

pub(crate) fn decode_status_snapshot(bytes: &[u8]) -> Result<ProjectionStatus, Error> {
    let mut cursor = Cursor::new(bytes);
    if cursor.byte()? != 1 {
        return Err(Error::CorruptProjectionRecord);
    }
    let projection_id =
        ProjectionId::parse(cursor.string()?).map_err(|_| Error::CorruptProjectionRecord)?;
    let generation =
        ProjectionGeneration::new(cursor.array()?).map_err(|_| Error::CorruptProjectionRecord)?;
    let health = match cursor.byte()? {
        0 => ProjectionHealth::Ready,
        1 => ProjectionHealth::Invalidated,
        2 => ProjectionHealth::Rebuilding,
        3 => ProjectionHealth::Failed,
        _ => return Err(Error::CorruptProjectionRecord),
    };
    let checkpoint = match cursor.byte()? {
        0 => None,
        1 => {
            let source_position = match cursor.byte()? {
                0 => None,
                1 => Some(EventPosition::new(
                    SourceGeneration::new(cursor.array()?)
                        .map_err(|_| Error::CorruptProjectionRecord)?,
                    radroots_storage::event::EventSequence::new(cursor.u64()?)
                        .map_err(|_| Error::CorruptProjectionRecord)?,
                )),
                _ => return Err(Error::CorruptProjectionRecord),
            };
            Some(
                ProjectionCheckpoint::new(
                    projection_id.clone(),
                    generation,
                    source_position,
                    cursor.u64()?,
                    cursor.u64()?,
                )
                .map_err(|_| Error::CorruptProjectionRecord)?,
            )
        }
        _ => return Err(Error::CorruptProjectionRecord),
    };
    let active_rebuild = match cursor.byte()? {
        0 => None,
        1 => Some(
            RebuildTicketId::new(cursor.array()?).map_err(|_| Error::CorruptProjectionRecord)?,
        ),
        _ => return Err(Error::CorruptProjectionRecord),
    };
    cursor.finish()?;
    ProjectionStatus::new(
        projection_id,
        generation,
        health,
        checkpoint,
        active_rebuild,
    )
    .map_err(|_| Error::CorruptProjectionRecord)
}

fn put_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), Error> {
    let length = u16::try_from(value.len()).map_err(|_| Error::CorruptProjectionRecord)?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn byte(&mut self) -> Result<u8, Error> {
        let value = self
            .bytes
            .get(self.offset)
            .copied()
            .ok_or(Error::CorruptProjectionRecord)?;
        self.offset += 1;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_be_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.array()?))
    }
    fn string(&mut self) -> Result<&'a str, Error> {
        let length = usize::from(self.u16()?);
        core::str::from_utf8(self.take(length)?).map_err(|_| Error::CorruptProjectionRecord)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        self.take(N)?
            .try_into()
            .map_err(|_| Error::CorruptProjectionRecord)
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(Error::CorruptProjectionRecord)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Error::CorruptProjectionRecord)?;
        self.offset = end;
        Ok(value)
    }
    fn finish(self) -> Result<(), Error> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::CorruptProjectionRecord)
        }
    }
}

fn projection_generation(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
) -> Result<ProjectionGeneration, Error> {
    ProjectionGeneration::new(array(
        row.try_get::<Vec<u8>, _>(column).map_err(map_corrupt)?,
    )?)
    .map_err(|_| Error::CorruptProjectionRecord)
}

fn event_id(row: &sqlx::sqlite::SqliteRow, column: &str) -> Result<EventId, Error> {
    Ok(EventId::from_bytes(array(
        row.try_get::<Vec<u8>, _>(column).map_err(map_corrupt)?,
    )?))
}

const fn reason_name(value: InvalidationReason) -> &'static str {
    match value {
        InvalidationReason::SourceGenerationChanged => "source_generation_changed",
        InvalidationReason::ProjectionGenerationChanged => "projection_generation_changed",
        InvalidationReason::EventIndexManifestChanged => "event_index_manifest_changed",
        InvalidationReason::IntegrityFailure => "integrity_failure",
        InvalidationReason::OperatorRequested => "operator_requested",
    }
}

const fn reason(value: &str) -> Result<InvalidationReason, Error> {
    match value.as_bytes() {
        b"source_generation_changed" => Ok(InvalidationReason::SourceGenerationChanged),
        b"projection_generation_changed" => Ok(InvalidationReason::ProjectionGenerationChanged),
        b"event_index_manifest_changed" => Ok(InvalidationReason::EventIndexManifestChanged),
        b"integrity_failure" => Ok(InvalidationReason::IntegrityFailure),
        b"operator_requested" => Ok(InvalidationReason::OperatorRequested),
        _ => Err(Error::CorruptProjectionRecord),
    }
}

const fn health_name(value: ProjectionHealth) -> &'static str {
    match value {
        ProjectionHealth::Ready => "ready",
        ProjectionHealth::Invalidated => "invalidated",
        ProjectionHealth::Rebuilding => "rebuilding",
        ProjectionHealth::Failed => "failed",
    }
}

const fn health(value: &str) -> Result<ProjectionHealth, Error> {
    match value.as_bytes() {
        b"ready" => Ok(ProjectionHealth::Ready),
        b"invalidated" => Ok(ProjectionHealth::Invalidated),
        b"rebuilding" => Ok(ProjectionHealth::Rebuilding),
        b"failed" => Ok(ProjectionHealth::Failed),
        _ => Err(Error::CorruptProjectionRecord),
    }
}

const fn rebuild_stage_name(value: RebuildStage) -> &'static str {
    match value {
        RebuildStage::Requested => "requested",
        RebuildStage::Running => "running",
        RebuildStage::Completed => "completed",
        RebuildStage::Failed => "failed",
    }
}

const fn rebuild_stage(value: &str) -> Result<RebuildStage, Error> {
    match value.as_bytes() {
        b"requested" => Ok(RebuildStage::Requested),
        b"running" => Ok(RebuildStage::Running),
        b"completed" => Ok(RebuildStage::Completed),
        b"failed" => Ok(RebuildStage::Failed),
        _ => Err(Error::CorruptProjectionRecord),
    }
}

fn array<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N], Error> {
    bytes.try_into().map_err(|_| Error::CorruptProjectionRecord)
}

fn i64_from_u64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::CorruptProjectionRecord)
}

fn u64_from_i64(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::CorruptProjectionRecord)
}

fn map_backend(_: sqlx::Error) -> Error {
    Error::BackendUnavailable
}

fn map_corrupt(_: sqlx::Error) -> Error {
    Error::CorruptProjectionRecord
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::migration::runtime::{MIGRATIONS, migration_sql};
    use radroots_storage::{ProjectionStore, event::EventSequence, status::EventStoreMode};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn store(mode: EventStoreMode) -> SqliteStorage {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory SQLite");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign keys");
        for migration in MIGRATIONS {
            sqlx::raw_sql(migration_sql(migration.version()).expect("registered SQL"))
                .execute(&pool)
                .await
                .expect("runtime migration");
        }
        SqliteStorage::new(
            pool,
            SourceGeneration::new([41; 32]).expect("generation"),
            mode,
        )
    }

    fn projection_id() -> ProjectionId {
        ProjectionId::parse("trade_projection").expect("projection id")
    }

    fn generation(byte: u8) -> ProjectionGeneration {
        ProjectionGeneration::new([byte; 32]).expect("projection generation")
    }

    fn checkpoint(
        generation: ProjectionGeneration,
        sequence: u64,
        rows: u64,
        at: u64,
    ) -> ProjectionCheckpoint {
        ProjectionCheckpoint::new(
            projection_id(),
            generation,
            Some(EventPosition::new(
                SourceGeneration::new([51; 32]).expect("source generation"),
                EventSequence::new(sequence).expect("sequence"),
            )),
            rows,
            at,
        )
        .expect("checkpoint")
    }

    fn invalidation() -> ProjectionInvalidation {
        ProjectionInvalidation::new(
            projection_id(),
            generation(1),
            generation(2),
            InvalidationReason::ProjectionGenerationChanged,
            200,
        )
        .expect("invalidation")
    }

    #[tokio::test]
    async fn checkpoints_invalidation_and_rebuild_lifecycle_are_durable() {
        let store = store(EventStoreMode::ReadWrite).await;
        assert!(
            store
                .status(projection_id())
                .await
                .expect("empty status")
                .is_none()
        );
        let first = store
            .checkpoint(checkpoint(generation(1), 1, 10, 100))
            .await
            .expect("first checkpoint");
        assert_eq!(first.health(), ProjectionHealth::Ready);
        let advanced = store
            .checkpoint(checkpoint(generation(1), 2, 20, 150))
            .await
            .expect("advanced checkpoint");
        assert_eq!(
            advanced.checkpoint().expect("checkpoint").projected_rows(),
            20
        );
        assert_eq!(
            store
                .checkpoint(checkpoint(generation(1), 1, 19, 140))
                .await,
            Err(Error::ProjectionCheckpointRegression)
        );

        let invalidation = invalidation();
        let invalidated = store
            .invalidate(invalidation.clone())
            .await
            .expect("invalidate");
        assert_eq!(invalidated.health(), ProjectionHealth::Invalidated);
        let ticket =
            RebuildTicket::requested(RebuildTicketId::new([3; 16]).expect("ticket"), invalidation);
        let requested = store
            .request_rebuild(ticket.clone())
            .await
            .expect("request rebuild");
        assert_eq!(
            store.request_rebuild(ticket).await.expect("replay"),
            requested
        );
        let running = store
            .transition_rebuild(RebuildTransition::start(
                requested.ticket_id(),
                requested.revision(),
                210,
            ))
            .await
            .expect("start");
        let progress = store
            .transition_rebuild(RebuildTransition::checkpoint(
                running.ticket_id(),
                running.revision(),
                220,
                checkpoint(generation(2), 2, 20, 220),
            ))
            .await
            .expect("progress");
        assert_eq!(
            store
                .transition_rebuild(RebuildTransition::fail(
                    progress.ticket_id(),
                    running.revision(),
                    230,
                ))
                .await,
            Err(Error::ProjectionRevisionConflict)
        );
        let completed = store
            .transition_rebuild(RebuildTransition::complete(
                progress.ticket_id(),
                progress.revision(),
                240,
                checkpoint(generation(2), 3, 30, 240),
            ))
            .await
            .expect("complete");
        assert_eq!(completed.stage(), RebuildStage::Completed);
        let status = store
            .status(projection_id())
            .await
            .expect("status")
            .expect("projection");
        assert_eq!(status.health(), ProjectionHealth::Ready);
        assert_eq!(status.generation(), generation(2));
        assert_eq!(status.active_rebuild(), None);
    }

    fn manifest(generation: ProjectionGeneration, digest: u8) -> EventIndexManifest {
        EventIndexManifest::new(
            generation,
            4,
            2,
            10,
            40,
            vec![
                EventIndexShard::new(
                    EventIndexShardId::parse("shard_a").expect("shard id"),
                    "index/shard_a.bin",
                    2,
                    EventIdRange::new(EventId::from_bytes([1; 32]), EventId::from_bytes([2; 32]))
                        .expect("range"),
                    10,
                    20,
                    ArtifactDigest::new([digest; 32]),
                )
                .expect("first shard"),
                EventIndexShard::new(
                    EventIndexShardId::parse("shard_b").expect("shard id"),
                    "index/shard_b.bin",
                    2,
                    EventIdRange::new(EventId::from_bytes([3; 32]), EventId::from_bytes([4; 32]))
                        .expect("range"),
                    30,
                    40,
                    ArtifactDigest::new([digest.wrapping_add(1); 32]),
                )
                .expect("second shard"),
            ],
        )
        .expect("manifest")
    }

    fn index_checkpoint(generation: ProjectionGeneration, at: u64) -> EventIndexCheckpoint {
        EventIndexCheckpoint::new(
            generation,
            at,
            vec![
                EventIndexShardCheckpoint::new(
                    EventIndexShardId::parse("shard_b").expect("shard id"),
                    40,
                    Some(EventId::from_bytes([4; 32])),
                    Some("cursor-b".to_owned()),
                )
                .expect("second checkpoint"),
                EventIndexShardCheckpoint::new(
                    EventIndexShardId::parse("shard_a").expect("shard id"),
                    20,
                    Some(EventId::from_bytes([2; 32])),
                    None,
                )
                .expect("first checkpoint"),
            ],
        )
        .expect("index checkpoint")
    }

    #[tokio::test]
    async fn event_index_manifest_and_checkpoint_round_trip_and_reject_regression() {
        let store = store(EventStoreMode::ReadWrite).await;
        let generation = generation(7);
        let expected_manifest = manifest(generation, 8);
        store
            .put_event_index_manifest(expected_manifest.clone())
            .await
            .expect("put manifest");
        store
            .put_event_index_manifest(expected_manifest.clone())
            .await
            .expect("manifest replay");
        assert_eq!(
            store
                .event_index_manifest(generation)
                .await
                .expect("manifest lookup")
                .expect("manifest"),
            expected_manifest
        );
        assert_eq!(
            store
                .put_event_index_manifest(manifest(generation, 9))
                .await,
            Err(Error::CorruptProjectionRecord)
        );

        let first = index_checkpoint(generation, 100);
        store
            .put_event_index_checkpoint(first.clone())
            .await
            .expect("put checkpoint");
        assert_eq!(
            store
                .event_index_checkpoint(generation)
                .await
                .expect("checkpoint lookup")
                .expect("checkpoint"),
            first
        );
        store
            .put_event_index_checkpoint(index_checkpoint(generation, 110))
            .await
            .expect("advance checkpoint");
        assert_eq!(
            store
                .put_event_index_checkpoint(index_checkpoint(generation, 109))
                .await,
            Err(Error::InvalidEventIndexCheckpoint)
        );
        for corrupt in [&[0_u8][..], &[1_u8, 0xff, 0xff][..]] {
            sqlx::query(
                "UPDATE radroots_runtime_event_index_checkpoints
                 SET checkpoint = ? WHERE projection_generation = ?",
            )
            .bind(corrupt)
            .bind(generation.as_bytes().as_slice())
            .execute(store.pool())
            .await
            .expect("forge corrupt index checkpoint");
            assert_eq!(
                store.event_index_checkpoint(generation).await,
                Err(Error::CorruptProjectionRecord)
            );
        }
    }

    #[tokio::test]
    async fn failed_rebuild_corruption_and_read_only_mode_fail_closed() {
        let store = store(EventStoreMode::ReadWrite).await;
        let initial = store
            .checkpoint(checkpoint(generation(1), 1, 1, 100))
            .await
            .expect("checkpoint");
        let encoded = encode_status_snapshot(&initial).expect("encode projection status");
        for end in 0..encoded.len() {
            let _ = decode_status_snapshot(&encoded[..end]);
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            decode_status_snapshot(&trailing),
            Err(Error::CorruptProjectionRecord)
        );
        for index in 0..encoded.len() {
            let mut corrupt = encoded.clone();
            corrupt[index] ^= 0xff;
            let _ = decode_status_snapshot(&corrupt);
        }
        let invalidation = invalidation();
        store
            .invalidate(invalidation.clone())
            .await
            .expect("invalidate");
        let ticket = store
            .request_rebuild(RebuildTicket::requested(
                RebuildTicketId::new([9; 16]).expect("ticket"),
                invalidation,
            ))
            .await
            .expect("request rebuild");
        let failed = store
            .transition_rebuild(RebuildTransition::fail(
                ticket.ticket_id(),
                ticket.revision(),
                210,
            ))
            .await
            .expect("fail rebuild");
        assert_eq!(failed.stage(), RebuildStage::Failed);
        assert_eq!(
            store
                .status(projection_id())
                .await
                .expect("status")
                .expect("projection")
                .health(),
            ProjectionHealth::Failed
        );

        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(store.pool())
            .await
            .expect("disable constraints");
        sqlx::query(
            "UPDATE radroots_runtime_projection_checkpoints
             SET projection_generation = X'01' WHERE projection_id = ?",
        )
        .bind(projection_id().as_str())
        .execute(store.pool())
        .await
        .expect("forge corruption");
        assert_eq!(
            store.status(projection_id()).await,
            Err(Error::CorruptProjectionRecord)
        );

        let read_only = SqliteStorage::new(
            store.pool().clone(),
            SourceGeneration::new([41; 32]).expect("generation"),
            EventStoreMode::ReadOnly,
        );
        assert_eq!(
            read_only
                .checkpoint(checkpoint(generation(3), 1, 1, 300))
                .await,
            Err(Error::BackendUnavailable)
        );
    }
}

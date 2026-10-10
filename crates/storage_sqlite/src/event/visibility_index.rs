use super::*;
use radroots_storage::event::{
    VisibilityEventMetadata, VisibilityIndexBounds, VisibilityIndexProgress,
};

impl SqliteStorage {
    pub(super) async fn prepare_visibility_index(
        &self,
        bounds: VisibilityIndexBounds,
    ) -> Result<VisibilityIndexProgress, Error> {
        if self.mode == EventStoreMode::ReadOnly {
            return Err(Error::BackendUnavailable);
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(map_backend)?;
        let progress = self
            .prepare_visibility_transaction(&mut transaction, bounds)
            .await?;
        transaction.commit().await.map_err(map_backend)?;
        Ok(progress)
    }

    pub(super) async fn prepare_visibility_transaction(
        &self,
        transaction: &mut sqlx::Transaction<'_, Sqlite>,
        bounds: VisibilityIndexBounds,
    ) -> Result<VisibilityIndexProgress, Error> {
        let mut charged = 4096 + u64::from(bounds.rows()) * 128;
        if charged > bounds.bytes() {
            return Err(Error::SpaceInsufficient);
        }
        let source = sqlx::query(
            "SELECT mutation_epoch, sequence_head FROM radroots_runtime_source_generations
             WHERE generation = ? AND state = 'active'",
        )
        .bind(self.generation.as_bytes().as_slice())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(map_backend)?
        .ok_or(Error::SourceGenerationChanged)?;
        let epoch: i64 = source.try_get("mutation_epoch").map_err(map_corrupt)?;
        let sequence: i64 = source.try_get("sequence_head").map_err(map_corrupt)?;
        sqlx::query(
            "INSERT OR IGNORE INTO radroots_runtime_visibility_progress
             (source_generation, backfill_target, replay_epoch, replay_sequence)
             VALUES (?, ?, ?, 9223372036854775807)",
        )
        .bind(self.generation.as_bytes().as_slice())
        .bind(sequence)
        .bind(epoch)
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
        let progress = sqlx::query(
            "SELECT schema_version, backfill_target, backfill_sequence, replay_epoch, replay_sequence
             FROM radroots_runtime_visibility_progress WHERE source_generation = ?",
        )
        .bind(self.generation.as_bytes().as_slice())
        .fetch_one(&mut **transaction).await.map_err(map_backend)?;
        let version: i64 = progress.try_get("schema_version").map_err(map_corrupt)?;
        let target: i64 = progress.try_get("backfill_target").map_err(map_corrupt)?;
        let mut backfill: i64 = progress.try_get("backfill_sequence").map_err(map_corrupt)?;
        let mut replay_epoch: i64 = progress.try_get("replay_epoch").map_err(map_corrupt)?;
        let mut replay_sequence: i64 = progress.try_get("replay_sequence").map_err(map_corrupt)?;
        if version != 1 || target > sequence || backfill > target || replay_epoch > epoch {
            return Err(Error::CorruptStoredEvent);
        }
        let had_pending: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM radroots_runtime_visibility_pending WHERE source_generation = ?)")
            .bind(self.generation.as_bytes().as_slice()).fetch_one(&mut **transaction).await.map_err(map_backend)?;
        let mut processed_keys = self
            .drain_visibility_keys(transaction, bounds.rows(), &mut charged, bounds.bytes())
            .await?;
        let remaining = if had_pending {
            0
        } else {
            bounds.rows() - processed_keys
        };
        let building = backfill < target;
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT source_sequence, last_mutation_epoch, length(signed_event) AS signed_bytes
             FROM radroots_runtime_events WHERE source_generation = ",
        );
        query.push_bind(self.generation.as_bytes().as_slice());
        if building {
            query.push(" AND source_sequence > ").push_bind(backfill);
            query.push(" AND source_sequence <= ").push_bind(target);
            query
                .push(" ORDER BY source_sequence LIMIT ")
                .push_bind(i64::from(remaining));
        } else {
            query.push(" AND (last_mutation_epoch, source_sequence) > (");
            query
                .push_bind(replay_epoch)
                .push(", ")
                .push_bind(replay_sequence)
                .push(")");
            query
                .push(" ORDER BY last_mutation_epoch, source_sequence LIMIT ")
                .push_bind(i64::from(remaining));
        }
        let lengths = query
            .build()
            .fetch_all(&mut **transaction)
            .await
            .map_err(map_backend)?;
        if building && remaining > 0 && lengths.is_empty() {
            return Err(Error::CorruptStoredEvent);
        }
        let mut processed = 0u16;
        for length in lengths {
            let position: i64 = length.try_get("source_sequence").map_err(map_corrupt)?;
            let changed: i64 = length.try_get("last_mutation_epoch").map_err(map_corrupt)?;
            let size = u64_from_i64(length.try_get("signed_bytes").map_err(map_corrupt)?)?;
            let charge = VisibilityIndexBounds::charge(size)?;
            let next_charge = charged
                .checked_add(charge)
                .ok_or(Error::SpaceInsufficient)?;
            if next_charge > bounds.bytes() {
                if processed == 0 {
                    return Err(Error::SpaceInsufficient);
                }
                break;
            }
            let row = sqlx::query(
                "SELECT source_generation, source_sequence, signed_event, admission_stage,
                 admitted_contract_id, admitted_registry_version FROM radroots_runtime_events
                 WHERE source_generation = ? AND source_sequence = ?",
            )
            .bind(self.generation.as_bytes().as_slice())
            .bind(position)
            .fetch_one(&mut **transaction)
            .await
            .map_err(map_backend)?;
            let decoded = self.decode_event_row(&row)?;
            if decoded.raw_json.len() as u64 != size || changed > epoch {
                return Err(Error::CorruptStoredEvent);
            }
            let metadata = VisibilityEventMetadata::from_event(&decoded.event, decoded.stage)?;
            self.put_visibility_metadata(transaction, position, changed, metadata)
                .await?;
            charged = next_charge;
            processed += 1;
            if building {
                backfill = position;
            } else {
                replay_epoch = changed;
                replay_sequence = position;
            }
        }
        let pending: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM radroots_runtime_events
             WHERE source_generation = ? AND (last_mutation_epoch, source_sequence) > (?, ?))",
        )
        .bind(self.generation.as_bytes().as_slice())
        .bind(replay_epoch)
        .bind(replay_sequence)
        .fetch_one(&mut **transaction)
        .await
        .map_err(map_backend)?;
        processed_keys += self
            .drain_visibility_keys(
                transaction,
                bounds.rows() - processed - processed_keys,
                &mut charged,
                bounds.bytes(),
            )
            .await?;
        let pending_keys: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM radroots_runtime_visibility_pending WHERE source_generation = ?)")
            .bind(self.generation.as_bytes().as_slice()).fetch_one(&mut **transaction).await.map_err(map_backend)?;
        if processed == 0 && processed_keys == 0 && pending_keys {
            return Err(Error::SpaceInsufficient);
        }
        let complete = backfill == target && !pending && !pending_keys;
        if complete {
            let retained: i64 = sqlx::query_scalar(
                "SELECT raw_events FROM radroots_runtime_visibility_progress WHERE source_generation = ?")
                .bind(self.generation.as_bytes().as_slice()).fetch_one(&mut **transaction).await.map_err(map_backend)?;
            if retained != sequence {
                return Err(Error::CorruptStoredEvent);
            }
            replay_epoch = epoch;
            replay_sequence = i64::MAX;
        }
        sqlx::query(
            "UPDATE radroots_runtime_visibility_progress SET backfill_sequence = ?,
             replay_epoch = ?, replay_sequence = ?, ready = ? WHERE source_generation = ?",
        )
        .bind(backfill)
        .bind(replay_epoch)
        .bind(replay_sequence)
        .bind(complete)
        .bind(self.generation.as_bytes().as_slice())
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
        Ok(VisibilityIndexProgress::new(
            EventSourceToken::new(self.generation, u64_from_i64(epoch)?),
            u64_from_i64(backfill)?,
            u64_from_i64(replay_epoch)?,
            processed,
            charged,
            complete,
        )
        .with_processed_keys(processed_keys))
    }

    async fn put_visibility_metadata(
        &self,
        transaction: &mut sqlx::Transaction<'_, Sqlite>,
        position: i64,
        changed: i64,
        metadata: VisibilityEventMetadata,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO radroots_runtime_visibility_events
             (source_generation, source_sequence, event_id, last_mutation_epoch, author, created_at,
              admission_stage, head_coordinate, deletion_coordinate, ephemeral, deletion_request)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(source_generation, source_sequence) DO UPDATE SET
              last_mutation_epoch = excluded.last_mutation_epoch,
              admission_stage = excluded.admission_stage, head_coordinate = excluded.head_coordinate,
              deletion_coordinate = excluded.deletion_coordinate,
              ephemeral = excluded.ephemeral, deletion_request = excluded.deletion_request",
        )
        .bind(self.generation.as_bytes().as_slice()).bind(position)
        .bind(metadata.event_id.as_bytes().as_slice()).bind(changed)
        .bind(metadata.author.as_slice()).bind(metadata.created_at.to_be_bytes().as_slice())
        .bind(stage_name(metadata.stage)).bind(metadata.head_coordinate.as_deref())
        .bind(metadata.deletion_coordinate.as_deref()).bind(metadata.ephemeral)
        .bind(metadata.deletion_request)
        .execute(&mut **transaction).await.map_err(map_backend)?;
        // Signed event bytes and deletion tags are immutable. Replays only add identical targets.
        for target in metadata.event_targets {
            sqlx::query(
                "INSERT OR IGNORE INTO radroots_runtime_visibility_deletion_events
                 (source_generation, target_event, author, request_id) VALUES (?, ?, ?, ?)",
            )
            .bind(self.generation.as_bytes().as_slice())
            .bind(target.as_bytes().as_slice())
            .bind(metadata.author.as_slice())
            .bind(metadata.event_id.as_bytes().as_slice())
            .execute(&mut **transaction)
            .await
            .map_err(map_backend)?;
        }
        for coordinate in metadata.address_targets {
            sqlx::query(
                "INSERT OR IGNORE INTO radroots_runtime_visibility_deletion_addresses
                 (source_generation, target_coordinate, author, created_at, request_id)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(self.generation.as_bytes().as_slice())
            .bind(coordinate)
            .bind(metadata.author.as_slice())
            .bind(metadata.created_at.to_be_bytes().as_slice())
            .bind(metadata.event_id.as_bytes().as_slice())
            .execute(&mut **transaction)
            .await
            .map_err(map_backend)?;
        }
        self.update_visibility_membership(transaction, position)
            .await?;
        Ok(())
    }
}

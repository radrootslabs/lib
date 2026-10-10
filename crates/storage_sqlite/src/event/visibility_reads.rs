use super::*;
use radroots_event::id::Nip01Coordinate;
use radroots_storage::event::VisibilityIndexBounds;

type Transaction<'a> = sqlx::Transaction<'a, Sqlite>;

impl SqliteStorage {
    pub(super) async fn visibility_header(
        &self,
        transaction: &mut Transaction<'_>,
        expected: Option<EventSourceToken>,
    ) -> Result<EventStoreStatus, Error> {
        let sql = "SELECT source.mutation_epoch, source.sequence_head, progress.ready,
                   progress.replay_epoch, progress.raw_events, progress.verified_events,
                   progress.visible_events
                   FROM radroots_runtime_source_generations AS source
                   LEFT JOIN radroots_runtime_visibility_progress AS progress
                     ON progress.source_generation = source.generation
                   WHERE source.generation = ? AND source.state = 'active'";
        #[cfg(test)]
        self.work.request(work::Read::Aggregate, sql);
        let row = sqlx::query(sql)
            .bind(self.generation.as_bytes().as_slice())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(map_backend)?
            .ok_or(Error::SourceGenerationChanged)?;
        #[cfg(test)]
        self.work.aggregate_returned();
        let epoch = u64_from_i64(row.try_get("mutation_epoch").map_err(map_corrupt)?)?;
        if expected.is_some_and(|token| token != EventSourceToken::new(self.generation, epoch)) {
            return Err(Error::SourceEpochChanged);
        }
        let ready: Option<bool> = row.try_get("ready").map_err(map_corrupt)?;
        if ready != Some(true) {
            return Err(Error::VisibilityRebuilding);
        }
        let replay = u64_from_i64(row.try_get("replay_epoch").map_err(map_corrupt)?)?;
        let raw = u64_from_i64(row.try_get("raw_events").map_err(map_corrupt)?)?;
        let sequence = u64_from_i64(row.try_get("sequence_head").map_err(map_corrupt)?)?;
        if replay != epoch || raw != sequence {
            return Err(Error::CorruptStoredEvent);
        }
        EventStoreStatus::new(
            self.generation,
            self.mode,
            EventStoreHealth::Available,
            raw,
            u64_from_i64(row.try_get("verified_events").map_err(map_corrupt)?)?,
            u64_from_i64(row.try_get("visible_events").map_err(map_corrupt)?)?,
        )
    }

    pub(super) async fn indexed_visible_page(
        &self,
        query: EventQuery,
        expected: Option<EventSourceToken>,
    ) -> Result<EventPage<StoredVisibleEvent>, Error> {
        self.validate_cursor(&query)?;
        let mut transaction = self.pool.begin().await.map_err(map_backend)?;
        self.visibility_header(&mut transaction, expected).await?;
        let after = query
            .bounds()
            .cursor()
            .map_or(0, |cursor| cursor.sequence().get());
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT member.source_sequence, length(event.signed_event) AS signed_bytes
             FROM radroots_runtime_visibility_members AS member INDEXED BY ",
        );
        if query.event_ids().is_empty() {
            builder.push("radroots_runtime_visibility_members_page_idx");
        } else {
            builder.push("radroots_runtime_visibility_members_event_idx");
        }
        builder.push(
            " JOIN radroots_runtime_events AS event ON event.event_id = member.event_id
             WHERE member.source_generation = ",
        );
        builder.push_bind(self.generation.as_bytes().as_slice());
        builder
            .push(" AND member.visible = 1 AND member.source_sequence > ")
            .push_bind(i64_from_u64(after)?);
        if !query.event_ids().is_empty() {
            builder.push(" AND member.event_id IN (");
            let mut ids = builder.separated(", ");
            for id in query.event_ids() {
                ids.push_bind(id.as_bytes().to_vec());
            }
            ids.push_unseparated(")");
        }
        builder
            .push(" ORDER BY member.source_sequence LIMIT ")
            .push_bind(i64::from(query.bounds().limit()) + 1);
        #[cfg(test)]
        self.work.request(work::Read::Keys, builder.sql().as_str());
        let lengths = builder
            .build()
            .fetch_all(&mut *transaction)
            .await
            .map_err(map_backend)?;
        #[cfg(test)]
        self.work.keys_returned(lengths.len());
        let mut charged = 4096 + (u64::from(query.bounds().limit()) + 1) * 128;
        let mut items = Vec::new();
        let mut more = false;
        for row in lengths {
            if items.len() == usize::from(query.bounds().limit()) {
                more = true;
                break;
            }
            let sequence: i64 = row.try_get("source_sequence").map_err(map_corrupt)?;
            let bytes = u64_from_i64(row.try_get("signed_bytes").map_err(map_corrupt)?)?;
            let next_charge = charged
                .checked_add(VisibilityIndexBounds::charge(bytes)?)
                .ok_or(Error::SpaceInsufficient)?;
            if next_charge > 16 * 1024 * 1024 {
                if items.is_empty() {
                    return Err(Error::SpaceInsufficient);
                }
                more = true;
                break;
            }
            let sql = "SELECT source_generation, source_sequence, signed_event, admission_stage,
                       admitted_contract_id, admitted_registry_version
                       FROM radroots_runtime_events WHERE source_generation = ? AND source_sequence = ?";
            #[cfg(test)]
            self.work.request(work::Read::Bounded, sql);
            let payload = sqlx::query(sql)
                .bind(self.generation.as_bytes().as_slice())
                .bind(sequence)
                .fetch_one(&mut *transaction)
                .await
                .map_err(map_backend)?;
            #[cfg(test)]
            self.work.returned(std::slice::from_ref(&payload));
            let decoded = self.decode_event_row(&payload)?;
            if decoded.raw_json.len() as u64 != bytes || decoded.stage != AdmissionStage::Visible {
                return Err(Error::CorruptStoredEvent);
            }
            items.push(StoredVisibleEvent::new(decoded.position, decoded.event));
            charged = next_charge;
        }
        let next = more
            .then(|| items.last().map(StoredVisibleEvent::position))
            .flatten();
        transaction.commit().await.map_err(map_backend)?;
        EventPage::new(self.generation, items, next, query.bounds())
    }

    pub(super) async fn update_visibility_membership(
        &self,
        transaction: &mut Transaction<'_>,
        position: i64,
    ) -> Result<(), Error> {
        let row = sqlx::query(
            "SELECT event_id, admission_stage, head_coordinate, created_at
             FROM radroots_runtime_visibility_events WHERE source_generation = ? AND source_sequence = ?")
            .bind(self.generation.as_bytes().as_slice()).bind(position)
            .fetch_one(&mut **transaction).await.map_err(map_backend)?;
        let id: Vec<u8> = row.try_get("event_id").map_err(map_corrupt)?;
        let stage: String = row.try_get("admission_stage").map_err(map_corrupt)?;
        sqlx::query(
            "INSERT INTO radroots_runtime_visibility_members
             (source_generation, source_sequence, event_id, admission_stage, visible)
             VALUES (?, ?, ?, ?, 0) ON CONFLICT(source_generation, source_sequence)
             DO UPDATE SET admission_stage = excluded.admission_stage",
        )
        .bind(self.generation.as_bytes().as_slice())
        .bind(position)
        .bind(&id)
        .bind(stage)
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
        let coordinate: Option<Vec<u8>> = row.try_get("head_coordinate").map_err(map_corrupt)?;
        if let Some(coordinate) = coordinate {
            let old = sqlx::query(
                "SELECT head.source_sequence, metadata.created_at, metadata.event_id
                 FROM radroots_runtime_visibility_heads AS head
                 JOIN radroots_runtime_visibility_events AS metadata ON metadata.event_id = head.event_id
                 WHERE head.source_generation = ? AND head.head_coordinate = ?")
                .bind(self.generation.as_bytes().as_slice()).bind(&coordinate)
                .fetch_optional(&mut **transaction).await.map_err(map_backend)?;
            let time: Vec<u8> = row.try_get("created_at").map_err(map_corrupt)?;
            let replace = match &old {
                None => true,
                Some(old) => {
                    let old_time: Vec<u8> = old.try_get("created_at").map_err(map_corrupt)?;
                    let old_id: Vec<u8> = old.try_get("event_id").map_err(map_corrupt)?;
                    time > old_time || (time == old_time && id < old_id)
                }
            };
            if replace {
                sqlx::query(
                    "INSERT INTO radroots_runtime_visibility_heads
                     (source_generation, head_coordinate, source_sequence, event_id) VALUES (?, ?, ?, ?)
                     ON CONFLICT(source_generation, head_coordinate) DO UPDATE SET
                     source_sequence = excluded.source_sequence, event_id = excluded.event_id")
                    .bind(self.generation.as_bytes().as_slice()).bind(coordinate).bind(position).bind(id)
                    .execute(&mut **transaction).await.map_err(map_backend)?;
                if let Some(old) = old {
                    let previous = old.try_get("source_sequence").map_err(map_corrupt)?;
                    self.refresh_visibility_member(transaction, previous)
                        .await?;
                }
            }
        }
        self.refresh_visibility_member(transaction, position)
            .await?;
        // Immutable targets persist; this queue changes only membership, not source truth.
        sqlx::query(
            "INSERT OR IGNORE INTO radroots_runtime_visibility_pending
             SELECT source_generation, 0, target_event FROM radroots_runtime_visibility_deletion_events
             WHERE request_id = ?")
            .bind(row.try_get::<Vec<u8>, _>("event_id").map_err(map_corrupt)?)
            .execute(&mut **transaction).await.map_err(map_backend)?;
        sqlx::query(
            "INSERT OR IGNORE INTO radroots_runtime_visibility_pending
             SELECT source_generation, 1, CAST(target_coordinate AS BLOB)
             FROM radroots_runtime_visibility_deletion_addresses WHERE request_id = ?",
        )
        .bind(row.try_get::<Vec<u8>, _>("event_id").map_err(map_corrupt)?)
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
        Ok(())
    }

    async fn refresh_visibility_member(
        &self,
        transaction: &mut Transaction<'_>,
        position: i64,
    ) -> Result<(), Error> {
        sqlx::query(
            "UPDATE radroots_runtime_visibility_members SET visible = (
             SELECT candidate.admission_stage = 'visible' AND candidate.ephemeral = 0
             AND (candidate.head_coordinate IS NULL OR candidate.event_id = (
               SELECT head.event_id FROM radroots_runtime_visibility_heads AS head
               WHERE head.source_generation = candidate.source_generation
                 AND head.head_coordinate = candidate.head_coordinate))
             AND (candidate.deletion_request = 1 OR (
               NOT EXISTS (SELECT 1 FROM radroots_runtime_visibility_deletion_events AS deletion
                 WHERE deletion.source_generation = candidate.source_generation
                 AND deletion.target_event = candidate.event_id AND deletion.author = candidate.author)
               AND NOT EXISTS (SELECT 1 FROM radroots_runtime_visibility_deletion_addresses AS deletion
                 WHERE deletion.source_generation = candidate.source_generation
                 AND deletion.target_coordinate = candidate.deletion_coordinate
                 AND deletion.author = candidate.author AND deletion.created_at >= candidate.created_at)))
             FROM radroots_runtime_visibility_events AS candidate
             WHERE candidate.source_generation = ? AND candidate.source_sequence = ?)
             WHERE source_generation = ? AND source_sequence = ?")
            .bind(self.generation.as_bytes().as_slice()).bind(position)
            .bind(self.generation.as_bytes().as_slice()).bind(position)
            .execute(&mut **transaction).await.map_err(map_backend)?;
        Ok(())
    }

    pub(super) async fn drain_visibility_keys(
        &self,
        transaction: &mut Transaction<'_>,
        remaining: u16,
        charged: &mut u64,
        byte_limit: u64,
    ) -> Result<u16, Error> {
        let keys = sqlx::query(
            "SELECT target_type, length(target_key) AS key_bytes FROM radroots_runtime_visibility_pending
             WHERE source_generation = ? ORDER BY target_type, target_key LIMIT ?",
        )
        .bind(self.generation.as_bytes().as_slice())
        .bind(i64::from(remaining))
        .fetch_all(&mut **transaction)
        .await
        .map_err(map_backend)?;
        let mut processed = 0;
        for row in keys {
            let kind: i64 = row.try_get("target_type").map_err(map_corrupt)?;
            let key_bytes = u64_from_i64(row.try_get("key_bytes").map_err(map_corrupt)?)?;
            let charge = charged
                .checked_add(4096 + 16 * key_bytes)
                .ok_or(Error::SpaceInsufficient)?;
            if charge > byte_limit {
                break;
            }
            let key: Vec<u8> = sqlx::query_scalar(
                "SELECT target_key FROM radroots_runtime_visibility_pending
                 WHERE source_generation = ? ORDER BY target_type, target_key LIMIT 1",
            )
            .bind(self.generation.as_bytes().as_slice())
            .fetch_one(&mut **transaction)
            .await
            .map_err(map_backend)?;
            if key.len() as u64 != key_bytes {
                return Err(Error::CorruptStoredEvent);
            }
            let position = if kind == 0 {
                sqlx::query_scalar::<_, i64>(
                    "SELECT source_sequence FROM radroots_runtime_visibility_members
                   WHERE source_generation = ? AND event_id = ?",
                )
                .bind(self.generation.as_bytes().as_slice())
                .bind(&key)
                .fetch_optional(&mut **transaction)
                .await
                .map_err(map_backend)?
            } else {
                let coordinate =
                    std::str::from_utf8(&key).map_err(|_| Error::CorruptStoredEvent)?;
                let coordinate =
                    Nip01Coordinate::parse(coordinate).map_err(|_| Error::CorruptStoredEvent)?;
                let mut head = vec![u8::from(coordinate.kind() >= 30_000)];
                head.extend_from_slice(&coordinate.kind().to_be_bytes());
                head.extend_from_slice(coordinate.pubkey().as_bytes());
                if coordinate.kind() >= 30_000 {
                    head.extend_from_slice(coordinate.identifier().as_bytes());
                }
                sqlx::query_scalar::<_, i64>(
                    "SELECT source_sequence FROM radroots_runtime_visibility_heads
                   WHERE source_generation = ? AND head_coordinate = ?",
                )
                .bind(self.generation.as_bytes().as_slice())
                .bind(head)
                .fetch_optional(&mut **transaction)
                .await
                .map_err(map_backend)?
            };
            if let Some(position) = position {
                self.refresh_visibility_member(transaction, position)
                    .await?;
            }
            sqlx::query(
                "DELETE FROM radroots_runtime_visibility_pending
                WHERE source_generation = ? AND target_type = ? AND target_key = ?",
            )
            .bind(self.generation.as_bytes().as_slice())
            .bind(kind)
            .bind(key)
            .execute(&mut **transaction)
            .await
            .map_err(map_backend)?;
            *charged = charge;
            processed += 1;
        }
        Ok(processed)
    }
}

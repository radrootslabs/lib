use crate::SqliteStorage;
use radroots_storage::{
    Error,
    authored_draft::{
        AUTHORED_DRAFT_QUERY_LIMIT_MAX, AuthoredDraft, AuthoredDraftId, AuthoredDraftRevision,
        AuthoredDraftStage, AuthoredDraftStore, DraftAppendDisposition, DraftAppendReceipt,
    },
    event::BoxFuture,
};
use sqlx::{Row, sqlite::SqliteRow};

const SNAPSHOT_MAX_BYTES: usize = 16 * 1024 * 1024;

impl AuthoredDraftStore for SqliteStorage {
    fn append_authored_draft(
        &self,
        draft: AuthoredDraft,
        expected_head: Option<AuthoredDraftRevision>,
    ) -> BoxFuture<'_, Result<DraftAppendReceipt, Error>> {
        Box::pin(async move {
            draft.validate()?;
            if self.event_mode() == radroots_storage::status::EventStoreMode::ReadOnly {
                return Err(Error::BackendUnavailable);
            }
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            if let Some(row) = sqlx::query(
                "SELECT * FROM radroots_runtime_authored_draft_revisions
                 WHERE draft_id = ? AND revision = ?",
            )
            .bind(draft.draft_id().as_bytes().as_slice())
            .bind(i64_from_u64(draft.revision().get())?)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_backend)?
            {
                let existing = decode_row(&row)?;
                transaction.rollback().await.map_err(map_backend)?;
                return if existing == draft {
                    Ok(DraftAppendReceipt::new(
                        existing,
                        DraftAppendDisposition::Replay,
                    ))
                } else {
                    Err(Error::DraftRevisionConflict)
                };
            }

            let head = sqlx::query(
                "SELECT * FROM radroots_runtime_authored_draft_revisions
                 WHERE draft_id = ? ORDER BY revision DESC LIMIT 1",
            )
            .bind(draft.draft_id().as_bytes().as_slice())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(map_backend)?
            .as_ref()
            .map(decode_row)
            .transpose()?;
            match (head.as_ref(), expected_head) {
                (None, None) if draft.revision() == AuthoredDraftRevision::INITIAL => {}
                (Some(previous), Some(expected)) if previous.revision() == expected => {
                    draft.validate_successor_of(previous)?;
                }
                _ => {
                    let _ = transaction.rollback().await;
                    return Err(Error::DraftRevisionConflict);
                }
            }

            let snapshot = encode_snapshot(&draft)?;
            sqlx::query(
                "INSERT INTO radroots_runtime_authored_draft_revisions (
                   draft_id, revision, author, stage, operation_id, payload_sha256,
                   created_at_unix_ms, updated_at_unix_ms, snapshot
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(draft.draft_id().as_bytes().as_slice())
            .bind(i64_from_u64(draft.revision().get())?)
            .bind(draft.author().as_slice())
            .bind(stage_code(draft.stage()))
            .bind(draft.operation_id().map(|id| id.as_bytes().to_vec()))
            .bind(draft.payload_sha256().as_slice())
            .bind(i64_from_u64(draft.created_at_unix_ms())?)
            .bind(i64_from_u64(draft.updated_at_unix_ms())?)
            .bind(snapshot)
            .execute(&mut *transaction)
            .await
            .map_err(map_backend)?;
            transaction.commit().await.map_err(map_backend)?;
            Ok(DraftAppendReceipt::new(
                draft,
                DraftAppendDisposition::Inserted,
            ))
        })
    }

    fn authored_draft_head(
        &self,
        draft_id: AuthoredDraftId,
    ) -> BoxFuture<'_, Result<Option<AuthoredDraft>, Error>> {
        Box::pin(async move {
            sqlx::query(
                "SELECT * FROM radroots_runtime_authored_draft_revisions
                 WHERE draft_id = ? ORDER BY revision DESC LIMIT 1",
            )
            .bind(draft_id.as_bytes().as_slice())
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            .as_ref()
            .map(decode_row)
            .transpose()
        })
    }

    fn authored_draft_revision(
        &self,
        draft_id: AuthoredDraftId,
        revision: AuthoredDraftRevision,
    ) -> BoxFuture<'_, Result<Option<AuthoredDraft>, Error>> {
        Box::pin(async move {
            sqlx::query(
                "SELECT * FROM radroots_runtime_authored_draft_revisions
                 WHERE draft_id = ? AND revision = ?",
            )
            .bind(draft_id.as_bytes().as_slice())
            .bind(i64_from_u64(revision.get())?)
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            .as_ref()
            .map(decode_row)
            .transpose()
        })
    }

    fn authored_draft_heads(
        &self,
        author: [u8; 32],
        limit: u16,
    ) -> BoxFuture<'_, Result<Vec<AuthoredDraft>, Error>> {
        Box::pin(async move {
            if author.iter().all(|byte| *byte == 0)
                || limit == 0
                || limit > AUTHORED_DRAFT_QUERY_LIMIT_MAX
            {
                return Err(Error::InvalidAuthoredDraft);
            }
            sqlx::query(
                "SELECT revisions.*
                 FROM radroots_runtime_authored_draft_revisions AS revisions
                 WHERE revisions.author = ?
                   AND revisions.revision = (
                     SELECT MAX(head.revision)
                     FROM radroots_runtime_authored_draft_revisions AS head
                     WHERE head.draft_id = revisions.draft_id
                   )
                 ORDER BY revisions.updated_at_unix_ms DESC, revisions.draft_id
                 LIMIT ?",
            )
            .bind(author.as_slice())
            .bind(i64::from(limit))
            .fetch_all(self.pool())
            .await
            .map_err(map_backend)?
            .iter()
            .map(decode_row)
            .collect()
        })
    }
}

fn encode_snapshot(draft: &AuthoredDraft) -> Result<Vec<u8>, Error> {
    let snapshot = serde_json::to_vec(draft).map_err(|_| Error::InvalidAuthoredDraft)?;
    if snapshot.is_empty() || snapshot.len() > SNAPSHOT_MAX_BYTES {
        return Err(Error::InvalidAuthoredDraft);
    }
    Ok(snapshot)
}

fn decode_row(row: &SqliteRow) -> Result<AuthoredDraft, Error> {
    let snapshot = row
        .try_get::<Vec<u8>, _>("snapshot")
        .map_err(|_| Error::CorruptAuthoredDraft)?;
    if snapshot.is_empty() || snapshot.len() > SNAPSHOT_MAX_BYTES {
        return Err(Error::CorruptAuthoredDraft);
    }
    let draft = serde_json::from_slice::<AuthoredDraft>(snapshot.as_slice())
        .map_err(|_| Error::CorruptAuthoredDraft)?;
    let draft_id = fixed::<16>(row, "draft_id")?;
    let revision = u64_from_i64(
        row.try_get::<i64, _>("revision")
            .map_err(|_| Error::CorruptAuthoredDraft)?,
    )?;
    let author = fixed::<32>(row, "author")?;
    let stage = row
        .try_get::<i64, _>("stage")
        .map_err(|_| Error::CorruptAuthoredDraft)?;
    let operation_id = row
        .try_get::<Option<Vec<u8>>, _>("operation_id")
        .map_err(|_| Error::CorruptAuthoredDraft)?;
    let payload_sha256 = fixed::<32>(row, "payload_sha256")?;
    let created = u64_from_i64(
        row.try_get::<i64, _>("created_at_unix_ms")
            .map_err(|_| Error::CorruptAuthoredDraft)?,
    )?;
    let updated = u64_from_i64(
        row.try_get::<i64, _>("updated_at_unix_ms")
            .map_err(|_| Error::CorruptAuthoredDraft)?,
    )?;
    let operation_matches = match (operation_id, draft.operation_id()) {
        (None, None) => true,
        (Some(raw), Some(expected)) => raw.as_slice() == expected.as_bytes(),
        _ => false,
    };
    if draft.draft_id().as_bytes() != &draft_id
        || draft.revision().get() != revision
        || draft.author() != &author
        || stage_code(draft.stage()) != stage
        || !operation_matches
        || draft.payload_sha256() != &payload_sha256
        || draft.created_at_unix_ms() != created
        || draft.updated_at_unix_ms() != updated
    {
        return Err(Error::CorruptAuthoredDraft);
    }
    Ok(draft)
}

fn fixed<const N: usize>(row: &SqliteRow, column: &str) -> Result<[u8; N], Error> {
    row.try_get::<Vec<u8>, _>(column)
        .map_err(|_| Error::CorruptAuthoredDraft)?
        .try_into()
        .map_err(|_| Error::CorruptAuthoredDraft)
}

const fn stage_code(stage: AuthoredDraftStage) -> i64 {
    match stage {
        AuthoredDraftStage::Draft => 0,
        AuthoredDraftStage::MediaPreparing => 1,
        AuthoredDraftStage::MediaUploading => 2,
        AuthoredDraftStage::ReadyToSign => 3,
        AuthoredDraftStage::Queued => 4,
        AuthoredDraftStage::Cancelled => 5,
    }
}

fn i64_from_u64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::InvalidAuthoredDraft)
}

fn u64_from_i64(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::CorruptAuthoredDraft)
}

fn map_backend(_: sqlx::Error) -> Error {
    Error::BackendUnavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OpenMode, OpenOptions, Paths};
    use radroots_storage::event::SourceGeneration;
    use sha2::Digest;
    use tempfile::TempDir;

    fn draft(id: u8, at: u64) -> AuthoredDraft {
        AuthoredDraft::initial(
            AuthoredDraftId::new([id; 16]).unwrap(),
            [7; 32],
            "radroots.phase1-draft.v1",
            vec![id],
            AuthoredDraftStage::Draft,
            None,
            at,
        )
        .unwrap()
    }

    async fn open_store(temp: &TempDir) -> SqliteStorage {
        let paths = Paths::from_directory(temp.path()).unwrap();
        SqliteStorage::open(
            OpenOptions::new(paths, OpenMode::Create)
                .with_source_generation(SourceGeneration::new([9; 32]).unwrap(), 9)
                .unwrap(),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn revisions_survive_reopen_and_replay_exactly() {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;
        let first = draft(1, 10);
        assert_eq!(
            store
                .append_authored_draft(first.clone(), None)
                .await
                .unwrap()
                .disposition(),
            DraftAppendDisposition::Inserted
        );
        assert_eq!(
            store
                .append_authored_draft(first.clone(), None)
                .await
                .unwrap()
                .disposition(),
            DraftAppendDisposition::Replay
        );
        let second = first
            .successor(
                b"next".to_vec(),
                AuthoredDraftStage::MediaPreparing,
                None,
                11,
            )
            .unwrap();
        store
            .append_authored_draft(second.clone(), Some(first.revision()))
            .await
            .unwrap();
        drop(store);
        let reopened = open_store(&temp).await;
        assert_eq!(
            reopened
                .authored_draft_head(first.draft_id())
                .await
                .unwrap(),
            Some(second.clone())
        );
        assert_eq!(
            reopened
                .authored_draft_revision(first.draft_id(), first.revision())
                .await
                .unwrap(),
            Some(first)
        );
        assert_eq!(
            reopened.authored_draft_heads([7; 32], 10).await.unwrap(),
            vec![second]
        );
    }

    #[tokio::test]
    async fn conflicts_and_query_bounds_fail_closed() {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;
        let first = draft(1, 10);
        store
            .append_authored_draft(first.clone(), None)
            .await
            .unwrap();
        let conflicting = AuthoredDraft::initial(
            first.draft_id(),
            [7; 32],
            first.payload_schema(),
            b"conflict".to_vec(),
            AuthoredDraftStage::Draft,
            None,
            10,
        )
        .unwrap();
        assert_eq!(
            store.append_authored_draft(conflicting, None).await,
            Err(Error::DraftRevisionConflict)
        );
        assert_eq!(
            store.authored_draft_heads([7; 32], 0).await,
            Err(Error::InvalidAuthoredDraft)
        );
        assert_eq!(
            store.authored_draft_heads([0; 32], 1).await,
            Err(Error::InvalidAuthoredDraft)
        );
        assert_eq!(
            store
                .authored_draft_heads([7; 32], AUTHORED_DRAFT_QUERY_LIMIT_MAX + 1)
                .await,
            Err(Error::InvalidAuthoredDraft)
        );

        let successor = first
            .successor(
                b"next".to_vec(),
                AuthoredDraftStage::MediaPreparing,
                None,
                11,
            )
            .unwrap();
        assert_eq!(
            store.append_authored_draft(successor.clone(), None).await,
            Err(Error::DraftRevisionConflict)
        );
        assert_eq!(
            store
                .append_authored_draft(successor, Some(AuthoredDraftRevision::new(2).unwrap()),)
                .await,
            Err(Error::DraftRevisionConflict)
        );
        assert_eq!(
            store
                .append_authored_draft(draft(2, 20), Some(AuthoredDraftRevision::INITIAL))
                .await,
            Err(Error::DraftRevisionConflict)
        );
        let noninitial_first = AuthoredDraft::reconstruct(
            AuthoredDraftId::new([3; 16]).unwrap(),
            AuthoredDraftRevision::new(2).unwrap(),
            [7; 32],
            "radroots.phase1-draft.v1",
            vec![3],
            sha2::Sha256::digest([3]).into(),
            AuthoredDraftStage::Draft,
            None,
            30,
            30,
        )
        .unwrap();
        assert_eq!(
            store.append_authored_draft(noninitial_first, None).await,
            Err(Error::DraftRevisionConflict)
        );
    }

    #[tokio::test]
    async fn simultaneous_first_append_has_one_insert_and_one_exact_replay() {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;
        let first = draft(2, 20);
        let (left, right) = tokio::join!(
            store.append_authored_draft(first.clone(), None),
            store.append_authored_draft(first, None),
        );
        let dispositions = [left.unwrap().disposition(), right.unwrap().disposition()];
        assert!(dispositions.contains(&DraftAppendDisposition::Inserted));
        assert!(dispositions.contains(&DraftAppendDisposition::Replay));
    }

    #[tokio::test]
    async fn read_only_append_and_u64_overflow_fail_before_mutation() {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;
        let paths = Paths::from_directory(temp.path()).unwrap();
        drop(store);
        let read_only = SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadOnly))
            .await
            .unwrap();
        assert_eq!(
            read_only.append_authored_draft(draft(3, 30), None).await,
            Err(Error::BackendUnavailable)
        );
        drop(read_only);

        let store = open_store(&temp).await;
        let overflow = AuthoredDraft::initial(
            AuthoredDraftId::new([4; 16]).unwrap(),
            [7; 32],
            "radroots.phase1-draft.v1",
            vec![4],
            AuthoredDraftStage::Draft,
            None,
            i64::MAX as u64 + 1,
        )
        .unwrap();
        assert_eq!(
            store.append_authored_draft(overflow, None).await,
            Err(Error::InvalidAuthoredDraft)
        );
    }

    #[allow(clippy::too_many_arguments)]
    async fn insert_raw_and_decode(
        store: &SqliteStorage,
        draft: &AuthoredDraft,
        draft_id: [u8; 16],
        revision: i64,
        author: [u8; 32],
        stage: i64,
        operation_id: Option<Vec<u8>>,
        payload_sha256: [u8; 32],
        created_at_unix_ms: i64,
        updated_at_unix_ms: i64,
    ) -> Result<AuthoredDraft, Error> {
        sqlx::query(
            "INSERT INTO radroots_runtime_authored_draft_revisions (
               draft_id, revision, author, stage, operation_id, payload_sha256,
               created_at_unix_ms, updated_at_unix_ms, snapshot
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(draft_id.as_slice())
        .bind(revision)
        .bind(author.as_slice())
        .bind(stage)
        .bind(operation_id)
        .bind(payload_sha256.as_slice())
        .bind(created_at_unix_ms)
        .bind(updated_at_unix_ms)
        .bind(encode_snapshot(draft).unwrap())
        .execute(store.pool())
        .await
        .unwrap();
        let row = sqlx::query(
            "SELECT * FROM radroots_runtime_authored_draft_revisions
             WHERE draft_id = ? AND revision = ?",
        )
        .bind(draft_id.as_slice())
        .bind(revision)
        .fetch_one(store.pool())
        .await
        .unwrap();
        decode_row(&row)
    }

    #[tokio::test]
    async fn every_redundant_draft_column_is_verified_against_the_snapshot() {
        let temp = TempDir::new().unwrap();
        let store = open_store(&temp).await;

        let value = draft(10, 100);
        assert_eq!(
            insert_raw_and_decode(
                &store,
                &value,
                [99; 16],
                1,
                *value.author(),
                stage_code(value.stage()),
                None,
                *value.payload_sha256(),
                100,
                100,
            )
            .await,
            Err(Error::CorruptAuthoredDraft)
        );

        for (id, revision, author, stage, operation_id, payload_sha256, created, updated) in [
            (
                11,
                2,
                [7; 32],
                0,
                None,
                *draft(11, 100).payload_sha256(),
                100,
                100,
            ),
            (
                12,
                1,
                [8; 32],
                0,
                None,
                *draft(12, 100).payload_sha256(),
                100,
                100,
            ),
            (
                13,
                1,
                [7; 32],
                1,
                None,
                *draft(13, 100).payload_sha256(),
                100,
                100,
            ),
            (
                14,
                1,
                [7; 32],
                0,
                Some(vec![1; 16]),
                *draft(14, 100).payload_sha256(),
                100,
                100,
            ),
            (15, 1, [7; 32], 0, None, [9; 32], 100, 100),
            (
                16,
                1,
                [7; 32],
                0,
                None,
                *draft(16, 100).payload_sha256(),
                99,
                100,
            ),
            (
                17,
                1,
                [7; 32],
                0,
                None,
                *draft(17, 100).payload_sha256(),
                100,
                101,
            ),
        ] {
            let value = draft(id, 100);
            assert_eq!(
                insert_raw_and_decode(
                    &store,
                    &value,
                    [id; 16],
                    revision,
                    author,
                    stage,
                    operation_id,
                    payload_sha256,
                    created,
                    updated,
                )
                .await,
                Err(Error::CorruptAuthoredDraft),
                "redundant column case {id}"
            );
        }
    }
}

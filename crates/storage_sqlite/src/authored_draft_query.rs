use super::{SqliteStorage, decode_row, map_backend};
use radroots_storage::{
    Error,
    authored_draft::AuthoredDraftRevision,
    authored_draft_query::{
        AUTHORED_DRAFT_PAGE_PAYLOAD_MAX_BYTES, AUTHORED_DRAFT_PAGE_SNAPSHOT_MAX_BYTES,
        AuthoredDraftPage, AuthoredDraftQuery, AuthoredDraftQueryRecord,
    },
};
use sqlx::Row;

pub(super) async fn page(
    store: &SqliteStorage,
    query: AuthoredDraftQuery,
) -> Result<AuthoredDraftPage, Error> {
    let mut transaction = store.pool().begin().await.map_err(map_backend)?;
    let result = read_page(&mut transaction, &query).await;
    // The read snapshot is always released before returning any continuation.
    let rollback = transaction.rollback().await.map_err(map_backend);
    match result {
        Ok(page) => {
            rollback?;
            Ok(page)
        }
        Err(error) => Err(error),
    }
}

async fn read_page(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    query: &AuthoredDraftQuery,
) -> Result<AuthoredDraftPage, Error> {
    let after = query.after().map(|value| value.to_vec());
    let rows = sqlx::query(
        "SELECT revisions.draft_id, revisions.revision, length(revisions.snapshot) AS snapshot_bytes,
                revisions.payload_schema = '' AS unknown_schema
         FROM radroots_runtime_authored_draft_revisions AS revisions
         WHERE revisions.author = ?
           AND ((revisions.payload_schema = ? AND revisions.payload_scope IS ?)
                OR revisions.payload_schema = '')
           AND (? IS NULL OR revisions.draft_id > ?)
           AND revisions.revision = (SELECT MAX(head.revision)
             FROM radroots_runtime_authored_draft_revisions AS head WHERE head.draft_id = revisions.draft_id)
         ORDER BY revisions.draft_id LIMIT ?"
    ).bind(query.author().as_slice()).bind(query.payload_schema())
        .bind(query.scope().map(|value| value.as_bytes().to_vec()))
        .bind(&after).bind(after).bind(i64::from(query.limit()) + 1)
        .fetch_all(&mut **transaction).await.map_err(map_backend)?;
    let mut records = Vec::new();
    let mut snapshot_bytes = 0usize;
    let mut payload_bytes = 0usize;
    let mut has_more = rows.len() > usize::from(query.limit());
    for row in rows.iter().take(usize::from(query.limit())) {
        // Keys and positive revisions are enforced by the STRICT table. A zero
        // key is still a valid scan/repair position for a corrupt domain ID.
        let key: [u8; 16] = row
            .try_get::<Vec<u8>, _>("draft_id")
            .map_err(map_backend)?
            .try_into()
            .map_err(|_| Error::CorruptAuthoredDraft)?;
        let revision = row.try_get::<i64, _>("revision").map_err(map_backend)?;
        let revision = AuthoredDraftRevision::new(
            u64::try_from(revision).map_err(|_| Error::CorruptAuthoredDraft)?,
        )?;
        let size = row
            .try_get::<i64, _>("snapshot_bytes")
            .map_err(map_backend)?;
        let unknown = row
            .try_get::<bool, _>("unknown_schema")
            .map_err(map_backend)?;
        let corrupt = AuthoredDraftQueryRecord::Corrupt {
            draft_key: key,
            revision,
        };
        if unknown || size <= 0 || size as u64 > AUTHORED_DRAFT_PAGE_SNAPSHOT_MAX_BYTES as u64 {
            records.push(corrupt);
            continue;
        }
        let size = size as usize;
        if snapshot_bytes + size > AUTHORED_DRAFT_PAGE_SNAPSHOT_MAX_BYTES {
            has_more = true;
            break;
        }
        snapshot_bytes += size;
        let row = sqlx::query("SELECT * FROM radroots_runtime_authored_draft_revisions WHERE draft_id = ? AND revision = ?")
            .bind(key.as_slice()).bind(revision.get() as i64)
            .fetch_one(&mut **transaction).await.map_err(map_backend)?;
        match decode_row(&row) {
            Ok(draft) if query.matches(&draft) => {
                if payload_bytes + draft.payload().len() > AUTHORED_DRAFT_PAGE_PAYLOAD_MAX_BYTES {
                    has_more = true;
                    break;
                }
                payload_bytes += draft.payload().len();
                records.push(AuthoredDraftQueryRecord::Draft(draft));
            }
            Ok(_) | Err(_) => records.push(corrupt),
        }
    }
    AuthoredDraftPage::new(query, records, has_more)
}

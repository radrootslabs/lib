use super::{SqliteStorage, map_backend};
use radroots_storage::{
    Error,
    projection::{
        ProjectionDocument, ProjectionGeneration,
        document_query::{
            PROJECTION_DOCUMENT_PAGE_BYTES_MAX, ProjectionDocumentGenerations,
            ProjectionDocumentPage, ProjectionDocumentQuery, ProjectionDocumentRecord,
        },
    },
};
use sqlx::Row;

pub(super) async fn page(
    store: &SqliteStorage,
    query: ProjectionDocumentQuery,
) -> Result<ProjectionDocumentPage, Error> {
    let mut transaction = store.pool().begin().await.map_err(map_backend)?;
    let result = read_page(&mut transaction, &query).await;
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
    query: &ProjectionDocumentQuery,
) -> Result<ProjectionDocumentPage, Error> {
    let generation = match query.generations() {
        ProjectionDocumentGenerations::All => None,
        ProjectionDocumentGenerations::Exact(value) => Some(value.as_bytes().to_vec()),
    };
    let after_generation = query
        .after()
        .map(|(generation, _)| generation.as_bytes().to_vec());
    let after_key = query.after().map(|(_, key)| key);
    // CASE bounds every variable-width metadata value before SQLx allocates it.
    // Payloads are loaded separately, only after charging this page's byte budget.
    let rows = sqlx::query(
        "SELECT CASE WHEN length(generation) = 32 THEN generation END AS generation,
                CASE WHEN length(CAST(document_key AS BLOB)) BETWEEN 1 AND 512 THEN document_key END AS document_key,
                CASE WHEN length(value_sha256) = 32 THEN value_sha256 END AS value_sha256,
                length(value) AS value_bytes
         FROM radroots_runtime_projection_documents AS documents
         WHERE projection_id = ? AND (? IS NULL OR generation = ?)
           AND (? IS NULL OR (generation, document_key) > (?, ?))
         ORDER BY documents.generation, documents.document_key LIMIT ?"
    ).bind(query.projection_id().as_str()).bind(&generation).bind(generation)
        .bind(&after_generation).bind(&after_generation).bind(after_key)
        .bind(i64::from(query.limit()) + 1)
        .fetch_all(&mut **transaction).await.map_err(map_backend)?;
    let mut has_more = rows.len() > usize::from(query.limit());
    let mut records = Vec::new();
    let mut bytes = 0;
    for row in rows.iter().take(usize::from(query.limit())) {
        let generation: Vec<u8> = row
            .try_get("generation")
            .map_err(|_| Error::CorruptProjectionDocument)?;
        let generation = ProjectionGeneration::new(
            generation
                .try_into()
                .map_err(|_| Error::CorruptProjectionDocument)?,
        )
        .map_err(|_| Error::CorruptProjectionDocument)?;
        let key: String = row
            .try_get("document_key")
            .map_err(|_| Error::CorruptProjectionDocument)?;
        let corrupt = ProjectionDocumentRecord::corrupt(generation, key.clone())?;
        let digest: Option<Vec<u8>> = row
            .try_get("value_sha256")
            .map_err(|_| Error::CorruptProjectionDocument)?;
        let size: i64 = row
            .try_get("value_bytes")
            .map_err(|_| Error::CorruptProjectionDocument)?;
        let Some(digest) = digest else {
            records.push(corrupt);
            continue;
        };
        let digest: [u8; 32] = digest
            .try_into()
            .map_err(|_| Error::CorruptProjectionDocument)?;
        if size <= 0 || size > PROJECTION_DOCUMENT_PAGE_BYTES_MAX as i64 {
            records.push(corrupt);
            continue;
        }
        let size = size as usize;
        if bytes + size > PROJECTION_DOCUMENT_PAGE_BYTES_MAX {
            has_more = true;
            break;
        }
        bytes += size;
        let value: Vec<u8> = sqlx::query_scalar(
            "SELECT value FROM radroots_runtime_projection_documents WHERE projection_id = ? AND generation = ? AND document_key = ?"
        ).bind(query.projection_id().as_str()).bind(generation.as_bytes().as_slice()).bind(&key)
            .fetch_one(&mut **transaction).await.map_err(map_backend)?;
        records.push(
            match ProjectionDocument::from_stored_parts(key, value, digest) {
                Ok(document) => ProjectionDocumentRecord::new(generation, document)?,
                Err(_) => corrupt,
            },
        );
    }
    ProjectionDocumentPage::new(query, records, has_more)
}

#[cfg(test)]
mod tests;

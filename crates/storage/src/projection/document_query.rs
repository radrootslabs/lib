//! Bounded opaque document inventory. A continuation is not a frozen snapshot.
#[cfg(test)]
mod tests;
use super::{Error, ProjectionDocument, ProjectionGeneration, ProjectionId, valid_document_key};

pub const PROJECTION_DOCUMENT_QUERY_LIMIT_MAX: u16 = 256;
pub const PROJECTION_DOCUMENT_PAGE_BYTES_MAX: usize = super::PROJECTION_DOCUMENT_VALUE_MAX_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionDocumentGenerations {
    Exact(ProjectionGeneration),
    All,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionDocumentQuery {
    projection_id: ProjectionId,
    generations: ProjectionDocumentGenerations,
    limit: u16,
    after: Option<(ProjectionGeneration, String)>,
}

impl ProjectionDocumentQuery {
    pub fn new(
        projection_id: ProjectionId,
        generations: ProjectionDocumentGenerations,
        limit: u16,
    ) -> Result<Self, Error> {
        ProjectionId::parse(projection_id.as_str())?;
        if let ProjectionDocumentGenerations::Exact(generation) = generations {
            ProjectionGeneration::new(*generation.as_bytes())?;
        }
        if limit == 0 || limit > PROJECTION_DOCUMENT_QUERY_LIMIT_MAX {
            return Err(Error::InvalidProjectionDocument);
        }
        Ok(Self {
            projection_id,
            generations,
            limit,
            after: None,
        })
    }
    pub fn with_cursor(mut self, cursor: &ProjectionDocumentCursor) -> Result<Self, Error> {
        if self.projection_id != cursor.projection_id || self.generations != cursor.generations {
            return Err(Error::InvalidProjectionDocument);
        }
        self.after = Some(cursor.after.clone());
        Ok(self)
    }
    pub fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }
    pub const fn generations(&self) -> ProjectionDocumentGenerations {
        self.generations
    }
    pub const fn limit(&self) -> u16 {
        self.limit
    }
    pub fn after(&self) -> Option<(ProjectionGeneration, &str)> {
        self.after
            .as_ref()
            .map(|(generation, key)| (*generation, key.as_str()))
    }
    pub fn matches(&self, generation: ProjectionGeneration, key: &str) -> bool {
        (match self.generations {
            ProjectionDocumentGenerations::Exact(expected) => generation == expected,
            ProjectionDocumentGenerations::All => true,
        }) && self.after().is_none_or(|after| (generation, key) > after)
    }
}

/// In-process continuation; the next call independently supplies its scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionDocumentCursor {
    projection_id: ProjectionId,
    generations: ProjectionDocumentGenerations,
    after: (ProjectionGeneration, String),
}

/// An absent document is explicit corruption requiring caller reconciliation.
#[derive(Clone, Eq, PartialEq)]
pub struct ProjectionDocumentRecord {
    generation: ProjectionGeneration,
    key: String,
    document: Option<ProjectionDocument>,
}
impl ProjectionDocumentRecord {
    pub fn new(
        generation: ProjectionGeneration,
        document: ProjectionDocument,
    ) -> Result<Self, Error> {
        ProjectionGeneration::new(*generation.as_bytes())?;
        Ok(Self {
            generation,
            key: document.key().to_owned(),
            document: Some(document),
        })
    }
    pub fn corrupt(generation: ProjectionGeneration, key: String) -> Result<Self, Error> {
        ProjectionGeneration::new(*generation.as_bytes())?;
        if !valid_document_key(&key) {
            return Err(Error::CorruptProjectionDocument);
        }
        Ok(Self {
            generation,
            key,
            document: None,
        })
    }
    pub const fn generation(&self) -> ProjectionGeneration {
        self.generation
    }
    pub fn key(&self) -> &str {
        &self.key
    }
    pub const fn document(&self) -> Option<&ProjectionDocument> {
        self.document.as_ref()
    }
}
impl std::fmt::Debug for ProjectionDocumentRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectionDocumentRecord")
            .field("generation", &self.generation)
            .field("key", &self.key)
            .field("corrupt", &self.document.is_none())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionDocumentPage {
    records: Vec<ProjectionDocumentRecord>,
    next_cursor: Option<ProjectionDocumentCursor>,
}
impl ProjectionDocumentPage {
    /// Backends must bound bytes read, including invalid payloads, before constructing a page.
    pub fn new(
        query: &ProjectionDocumentQuery,
        records: Vec<ProjectionDocumentRecord>,
        has_more: bool,
    ) -> Result<Self, Error> {
        if records.len() > usize::from(query.limit) || (has_more && records.is_empty()) {
            return Err(Error::InvalidProjectionDocument);
        }
        let mut previous = query.after();
        let mut bytes = 0;
        for record in &records {
            let position = (record.generation, record.key());
            if !query.matches(position.0, position.1)
                || previous.is_some_and(|last| position <= last)
            {
                return Err(Error::InvalidProjectionDocument);
            }
            bytes += record
                .document()
                .map_or(0, |document| document.value().len());
            if bytes > PROJECTION_DOCUMENT_PAGE_BYTES_MAX {
                return Err(Error::InvalidProjectionDocument);
            }
            previous = Some(position);
        }
        let next_cursor = if has_more {
            previous.map(|(generation, key)| ProjectionDocumentCursor {
                projection_id: query.projection_id.clone(),
                generations: query.generations,
                after: (generation, key.to_owned()),
            })
        } else {
            None
        };
        Ok(Self {
            records,
            next_cursor,
        })
    }
    pub fn records(&self) -> &[ProjectionDocumentRecord] {
        &self.records
    }
    pub const fn next_cursor(&self) -> Option<&ProjectionDocumentCursor> {
        self.next_cursor.as_ref()
    }
    pub fn into_records(self) -> Vec<ProjectionDocumentRecord> {
        self.records
    }
}

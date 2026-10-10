//! Bounded opaque writes; callers retain policy and serialization ownership.
use super::{Error, ProjectionDocument, ProjectionGeneration, ProjectionId};
use crate::event::EventSourceToken;
use std::collections::BTreeSet;

pub const PROJECTION_BATCH_ROWS_MAX: usize = 64;
pub const PROJECTION_BATCH_BYTES_MAX: usize = 16_777_216;

/// A content-derived generation binds caller policy/schema and the canonical source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionWriteScope {
    projection_id: ProjectionId,
    generation: ProjectionGeneration,
    source: EventSourceToken,
    expected_active: Option<ProjectionGeneration>,
}
impl ProjectionWriteScope {
    pub fn new(
        projection_id: ProjectionId,
        generation: ProjectionGeneration,
        source: EventSourceToken,
        expected_active: Option<ProjectionGeneration>,
    ) -> Result<Self, Error> {
        ProjectionId::parse(projection_id.as_str())?;
        ProjectionGeneration::new(*generation.as_bytes())?;
        crate::event::SourceGeneration::new(*source.generation().as_bytes())?;
        if let Some(active) = expected_active {
            ProjectionGeneration::new(*active.as_bytes())?;
            if active == generation {
                return Err(Error::InvalidProjectionGeneration);
            }
        }
        Ok(Self {
            projection_id,
            generation,
            source,
            expected_active,
        })
    }
    pub const fn projection_id(&self) -> &ProjectionId {
        &self.projection_id
    }
    pub const fn generation(&self) -> ProjectionGeneration {
        self.generation
    }
    pub const fn source(&self) -> EventSourceToken {
        self.source
    }
    pub const fn expected_active(&self) -> Option<ProjectionGeneration> {
        self.expected_active
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ProjectionDocumentBatch {
    scope: ProjectionWriteScope,
    documents: Vec<ProjectionDocument>,
    logical_bytes: usize,
}
impl ProjectionDocumentBatch {
    /// Empty batches allow publishing a genuinely empty or already staged generation.
    pub fn new(
        scope: ProjectionWriteScope,
        documents: Vec<ProjectionDocument>,
    ) -> Result<Self, Error> {
        if documents.len() > PROJECTION_BATCH_ROWS_MAX {
            return Err(Error::InvalidProjectionDocument);
        }
        let mut keys = BTreeSet::new();
        let mut charge = 4096usize;
        for document in &documents {
            if !keys.insert(document.key()) {
                return Err(Error::InvalidProjectionDocument);
            }
            charge = charge
                .checked_add(
                    document
                        .value()
                        .len()
                        .checked_mul(2)
                        .and_then(|size| size.checked_add(document.key().len() * 2 + 256))
                        .ok_or(Error::SpaceInsufficient)?,
                )
                .ok_or(Error::SpaceInsufficient)?;
            if charge > PROJECTION_BATCH_BYTES_MAX {
                return Err(Error::SpaceInsufficient);
            }
        }
        Ok(Self {
            scope,
            documents,
            logical_bytes: charge,
        })
    }
    pub const fn scope(&self) -> &ProjectionWriteScope {
        &self.scope
    }
    pub fn documents(&self) -> &[ProjectionDocument] {
        &self.documents
    }
    pub const fn logical_bytes(&self) -> usize {
        self.logical_bytes
    }
}
impl std::fmt::Debug for ProjectionDocumentBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectionDocumentBatch")
            .field("scope", &self.scope)
            .field("rows", &self.documents.len())
            .field("logical_bytes", &self.logical_bytes)
            .finish()
    }
}

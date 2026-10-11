use super::{Error, State};
use crate::{
    event::{EventSourceToken, SourceGeneration},
    projection::{
        ProjectionCheckpoint, ProjectionDocument, ProjectionGeneration, ProjectionHealth,
        ProjectionId, ProjectionStatus,
        batch::{ProjectionDocumentBatch, ProjectionWriteScope},
        document_query::{
            ProjectionDocumentGenerations, ProjectionDocumentPage, ProjectionDocumentQuery,
            ProjectionDocumentRecord,
        },
    },
};
use std::ops::Bound::{Excluded, Included, Unbounded};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Phase {
    Staging,
    Promoted,
    Retired,
}

#[derive(Clone)]
pub(super) struct Generation {
    pub(super) scope: ProjectionWriteScope,
    pub(super) phase: Phase,
    pub(super) rows: u64,
    pub(super) bytes: u64,
}

pub(super) fn fence(
    state: &State,
    generation: SourceGeneration,
    source: EventSourceToken,
    id: &ProjectionId,
    active: Option<ProjectionGeneration>,
) -> Result<(), Error> {
    if source != EventSourceToken::new(generation, state.event_mutation_epoch) {
        return Err(Error::SourceEpochChanged);
    }
    let status = state
        .projections
        .iter()
        .find(|value| value.projection_id() == id);
    if status.map(ProjectionStatus::generation) != active
        || status.is_some_and(|value| value.health() != ProjectionHealth::Ready)
    {
        return Err(Error::ProjectionRevisionConflict);
    }
    Ok(())
}

pub(super) fn read_fence(
    state: &State,
    generation: SourceGeneration,
    source: EventSourceToken,
    id: &ProjectionId,
    active: ProjectionGeneration,
) -> Result<(), Error> {
    fence(state, generation, source, id, Some(active))?;
    let registered = state
        .projection_generations
        .get(&(id.clone(), active))
        .filter(|value| value.phase == Phase::Promoted)
        .ok_or(Error::ProjectionRevisionConflict)?;
    if registered.scope.source() != source {
        return Err(Error::SourceEpochChanged);
    }
    Ok(())
}

pub(super) fn write(
    state: &mut State,
    source_generation: SourceGeneration,
    batch: &ProjectionDocumentBatch,
    checkpoint: Option<ProjectionCheckpoint>,
) -> Result<Option<ProjectionStatus>, Error> {
    let scope = batch.scope();
    fence(
        state,
        source_generation,
        scope.source(),
        scope.projection_id(),
        scope.expected_active(),
    )?;
    let identity = (scope.projection_id().clone(), scope.generation());
    let prior = state.projection_generations.get(&identity);
    if prior.is_some_and(|value| value.phase != Phase::Staging || value.scope != *scope) {
        return Err(Error::ProjectionRevisionConflict);
    }
    if prior.is_none()
        && (state
            .projection_generations
            .iter()
            .any(|((id, _), value)| id == scope.projection_id() && value.phase == Phase::Staging)
            || state
                .projection_documents
                .range((
                    Included((identity.0.clone(), identity.1, String::new())),
                    Unbounded,
                ))
                .next()
                .is_some_and(|((id, generation, _), _)| {
                    id == &identity.0 && generation == &identity.1
                }))
    {
        return Err(Error::ProjectionRevisionConflict);
    }
    let mut next = prior.cloned().unwrap_or(Generation {
        scope: scope.clone(),
        phase: Phase::Staging,
        rows: 0,
        bytes: 0,
    });
    for document in batch.documents() {
        let key = (identity.0.clone(), identity.1, document.key().to_owned());
        let previous = state.projection_documents.get(&key);
        next.rows = next
            .rows
            .checked_add(u64::from(previous.is_none()))
            .ok_or(Error::SpaceInsufficient)?;
        next.bytes = next
            .bytes
            .checked_sub(previous.map_or(0, |doc| doc.value().len() as u64))
            .and_then(|bytes| bytes.checked_add(document.value().len() as u64))
            .ok_or(Error::SpaceInsufficient)?;
    }
    let promoted = checkpoint
        .map(|checkpoint| {
            if checkpoint.projection_id() != scope.projection_id()
                || checkpoint.generation() != scope.generation()
                || checkpoint.projected_rows() != next.rows
                || checkpoint.source_position() != state.events.last().map(|entry| entry.position)
            {
                return Err(Error::ProjectionCheckpointMismatch);
            }
            ProjectionStatus::new(
                identity.0.clone(),
                identity.1,
                ProjectionHealth::Ready,
                Some(checkpoint),
                None,
            )
        })
        .transpose()?;
    let cache_bytes = state
        .projection_cache
        .check_bytes(prior.map_or(0, |value| value.bytes), next.bytes)?;
    state.projection_cache.check_protected(&identity)?;
    // Every fallible condition is checked before publishing any in-memory changes.
    let documents: Vec<_> = batch
        .documents()
        .iter()
        .map(|document| {
            (
                (identity.0.clone(), identity.1, document.key().to_owned()),
                document.clone(),
            )
        })
        .collect();
    state.projection_cache.value_bytes = cache_bytes;
    state.projection_cache.protected.insert(identity.clone());
    for (key, document) in documents {
        state.projection_documents.insert(key, document);
    }
    if let Some(status) = promoted.as_ref() {
        next.phase = Phase::Promoted;
        if let Some(active) = scope.expected_active() {
            state
                .projection_cache
                .protected
                .remove(&(identity.0.clone(), active));
        }
        if let Some(active) = scope.expected_active()
            && let Some(previous) = state
                .projection_generations
                .get_mut(&(identity.0.clone(), active))
        {
            previous.phase = Phase::Retired;
        }
        if let Some(prior) = state
            .projections
            .iter_mut()
            .find(|value| value.projection_id() == &identity.0)
        {
            *prior = status.clone();
        } else {
            state.projections.push(status.clone());
        }
    }
    state.projection_generations.insert(identity, next);
    Ok(promoted)
}

pub(super) fn page(
    state: &State,
    query: &ProjectionDocumentQuery,
) -> Result<ProjectionDocumentPage, Error> {
    let id = query.projection_id().clone();
    let generation = match query.generations() {
        ProjectionDocumentGenerations::Exact(generation) => generation,
        ProjectionDocumentGenerations::All => ProjectionGeneration::new([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1,
        ])?,
    };
    // The smallest valid generation is [0,..,1]; zero is never admitted.
    let start = query.after().map_or_else(
        || {
            Included((
                id.clone(),
                generation,
                query
                    .key_range()
                    .map_or_else(String::new, |range| range.first().to_owned()),
            ))
        },
        |(generation, key)| Excluded((id.clone(), generation, key.to_owned())),
    );
    let end = query.key_range().map_or(Unbounded, |range| {
        Included((id.clone(), generation, range.last().to_owned()))
    });
    let selected: Vec<_> = state
        .projection_documents
        .range((start, end))
        .take_while(|((row_id, generation, _), _)| {
            row_id == &id
                && match query.generations() {
                    ProjectionDocumentGenerations::Exact(expected) => *generation == expected,
                    ProjectionDocumentGenerations::All => true,
                }
        })
        .take(usize::from(query.limit()) + 1)
        .collect();
    let mut more = selected.len() > usize::from(query.limit());
    let mut records = Vec::new();
    let mut bytes = 0usize;
    for ((_, generation, _), document) in selected.into_iter().take(usize::from(query.limit())) {
        if bytes + document.value().len() > query.payload_budget() {
            if records.is_empty() {
                return Err(Error::SpaceInsufficient);
            }
            more = true;
            break;
        }
        bytes += document.value().len();
        records.push(ProjectionDocumentRecord::new(
            *generation,
            document.clone(),
        )?);
    }
    ProjectionDocumentPage::new(query, records, more)
}

pub(super) fn validate_key(key: &str) -> Result<(), Error> {
    if !crate::projection::valid_document_key(key) {
        return Err(Error::InvalidProjectionDocument);
    }
    Ok(())
}

pub(super) type Documents =
    std::collections::BTreeMap<(ProjectionId, ProjectionGeneration, String), ProjectionDocument>;

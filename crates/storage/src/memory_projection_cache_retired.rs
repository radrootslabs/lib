use super::{Error, State, projection_batch, projection_cache};
use crate::{
    event::{EventSourceToken, SourceGeneration},
    projection::{
        ProjectionGeneration, ProjectionId,
        cache::{ProjectionCacheCollection, ProjectionCacheScope},
    },
};

fn next(
    state: &State,
    id: &ProjectionId,
    active: Option<ProjectionGeneration>,
) -> Option<ProjectionGeneration> {
    state
        .projection_cache
        .retired
        .get(id)
        .and_then(|generations| {
            generations
                .iter()
                .find(|generation| Some(**generation) != active)
                .copied()
        })
}

pub(super) fn collect(
    state: &mut State,
    generation: SourceGeneration,
    id: ProjectionId,
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
    now: u64,
) -> Result<ProjectionCacheCollection, Error> {
    projection_cache::fence_owner(state, generation, &id, source, active)?;
    if !state.projection_cache.owners.contains(&id) {
        return Err(Error::ProjectionRevisionConflict);
    }
    let Some(retired) = next(state, &id, active) else {
        return Ok(ProjectionCacheCollection {
            removed_rows: 0,
            removed_value_bytes: 0,
            more: false,
            usage: state.projection_cache.usage(),
        });
    };
    if state
        .projection_generations
        .get(&(id.clone(), retired))
        .is_none_or(|value| value.phase != projection_batch::Phase::Retired)
    {
        return Err(Error::CorruptProjectionRecord);
    }
    let scope = ProjectionCacheScope::new(id.clone(), retired, source, active)?;
    let mut result = projection_cache::collect(state, generation, scope, now)?;
    result.more |= next(state, &id, active).is_some();
    Ok(result)
}

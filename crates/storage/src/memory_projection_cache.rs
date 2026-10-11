use super::{Error, State, projection_batch};
use crate::{
    event::{EventSourceToken, SourceGeneration},
    projection::{
        ProjectionGeneration, ProjectionHealth, ProjectionId, ProjectionSnapshot,
        batch::ProjectionWriteScope,
        cache::{
            PROJECTION_CACHE_COLLECTION_ROWS_MAX, ProjectionCacheCollection, ProjectionCacheLimits,
            ProjectionCacheScope, ProjectionCacheUsage,
        },
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Bound::{Excluded, Included, Unbounded},
    time::{SystemTime, UNIX_EPOCH},
};

type Identity = (ProjectionId, ProjectionGeneration);
pub(super) type Snapshots = BTreeMap<(ProjectionId, [u8; 32]), ProjectionSnapshot>;
#[derive(Clone, Default)]
pub(super) struct Cache {
    pub(super) limits: Option<ProjectionCacheLimits>,
    pub(super) owners: BTreeSet<ProjectionId>,
    pub(super) protected: BTreeSet<Identity>,
    pub(super) retired: BTreeMap<ProjectionId, BTreeSet<ProjectionGeneration>>,
    pub(super) snapshot_order: BTreeSet<(ProjectionId, ProjectionGeneration, u64, [u8; 32])>,
    pub(super) value_bytes: u64,
    pub(super) owner_time_ms: u64,
}
impl Cache {
    pub(super) const fn new() -> Self {
        Self {
            limits: None,
            owners: BTreeSet::new(),
            protected: BTreeSet::new(),
            retired: BTreeMap::new(),
            snapshot_order: BTreeSet::new(),
            value_bytes: 0,
            owner_time_ms: 0,
        }
    }
    pub(super) fn usage(&self) -> ProjectionCacheUsage {
        ProjectionCacheUsage {
            value_bytes: self.value_bytes,
            protected_generations: self.protected.len() as u64,
            legacy_snapshots: self.snapshot_order.len() as u64,
        }
    }
    pub(super) fn check_bytes(&self, previous: u64, next: u64) -> Result<u64, Error> {
        let value = self
            .value_bytes
            .checked_sub(previous)
            .and_then(|value| value.checked_add(next))
            .ok_or(Error::SpaceInsufficient)?;
        if next > previous && self.limits.is_some_and(|limit| value > limit.value_bytes()) {
            return Err(Error::SpaceInsufficient);
        }
        Ok(value)
    }
    pub(super) fn check_protected(&self, identity: &Identity) -> Result<(), Error> {
        if !self.protected.contains(identity)
            && self
                .limits
                .is_some_and(|limit| self.protected.len() as u64 >= limit.protected_generations())
        {
            return Err(Error::SpaceInsufficient);
        }
        Ok(())
    }
    pub(super) fn time(&mut self, now: u64) -> u64 {
        self.owner_time_ms = self.owner_time_ms.max(now);
        self.owner_time_ms
    }
    pub(super) fn expired(&self, created: u64, now: u64) -> bool {
        self.limits
            .is_some_and(|limit| created > now || now - created >= limit.legacy_age_ms())
    }
}
pub(super) fn now() -> Result<u64, Error> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::InvalidProjectionTimestamp)?
            .as_millis(),
    )
    .map_err(|_| Error::InvalidProjectionTimestamp)
}
pub(super) fn configure(
    state: &mut State,
    id: ProjectionId,
    limits: ProjectionCacheLimits,
    now: u64,
) -> Result<ProjectionCacheUsage, Error> {
    ProjectionId::parse(id.as_str())?;
    if state
        .projection_cache
        .limits
        .is_some_and(|existing| existing != limits)
    {
        return Err(Error::ProjectionRevisionConflict);
    }
    state.projection_cache.limits = Some(limits);
    state.projection_cache.owners.insert(id);
    state.projection_cache.time(now);
    Ok(state.projection_cache.usage())
}
pub(super) fn fence_owner(
    state: &State,
    generation: SourceGeneration,
    id: &ProjectionId,
    source: EventSourceToken,
    active: Option<ProjectionGeneration>,
) -> Result<(), Error> {
    ProjectionId::parse(id.as_str())?;
    if source != EventSourceToken::new(generation, state.event_mutation_epoch) {
        return Err(Error::SourceEpochChanged);
    }
    if state
        .projections
        .iter()
        .find(|value| value.projection_id() == id)
        .map(|value| value.generation())
        != active
    {
        return Err(Error::ProjectionRevisionConflict);
    }
    Ok(())
}
pub(super) fn retire(
    state: &mut State,
    source: SourceGeneration,
    scope: ProjectionCacheScope,
) -> Result<ProjectionCacheUsage, Error> {
    fence_owner(
        state,
        source,
        scope.projection_id(),
        scope.source(),
        scope.expected_active(),
    )?;
    let identity = (scope.projection_id().clone(), scope.generation());
    let prior = state.projection_generations.get(&identity);
    let configured = state
        .projection_cache
        .owners
        .contains(scope.projection_id());
    if !configured {
        let legacy = state
            .projections
            .iter()
            .find(|value| value.projection_id() == scope.projection_id());
        if scope.expected_active() != Some(scope.generation())
            || legacy.is_none_or(|value| {
                value.health() != ProjectionHealth::Invalidated
                    || value.active_rebuild().is_some()
                    || value
                        .checkpoint()
                        .and_then(|value| value.source_position())
                        .is_some_and(|position| position.generation() != source)
            })
            || prior.is_some_and(|value| value.phase != projection_batch::Phase::Retired)
        {
            return Err(Error::ProjectionRevisionConflict);
        }
    }
    if scope.expected_active() == Some(scope.generation()) {
        let current = state
            .projections
            .iter()
            .find(|value| value.projection_id() == scope.projection_id())
            .ok_or(Error::ProjectionRevisionConflict)?;
        if current.health() == ProjectionHealth::Ready
            && prior.is_none_or(|value| value.scope.source() == scope.source())
        {
            return Err(Error::ProjectionRevisionConflict);
        }
    }
    let retired = prior.cloned().unwrap_or(projection_batch::Generation {
        scope: ProjectionWriteScope::new(
            scope.projection_id().clone(),
            scope.generation(),
            scope.source(),
            None,
        )?,
        phase: projection_batch::Phase::Retired,
        rows: 0,
        bytes: 0,
    });
    state.projection_generations.insert(
        identity.clone(),
        projection_batch::Generation {
            phase: projection_batch::Phase::Retired,
            ..retired
        },
    );
    state.projection_cache.protected.remove(&identity);
    state
        .projection_cache
        .retired
        .entry(identity.0.clone())
        .or_default()
        .insert(identity.1);
    if configured && scope.expected_active() == Some(scope.generation()) {
        state
            .projections
            .retain(|value| value.projection_id() != scope.projection_id());
    }
    Ok(state.projection_cache.usage())
}
pub(super) fn collect(
    state: &mut State,
    source: SourceGeneration,
    scope: ProjectionCacheScope,
    now: u64,
) -> Result<ProjectionCacheCollection, Error> {
    collect_kind(state, source, scope, now, CollectionKind::Generation)
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum CollectionKind {
    Generation,
    ObsoleteSnapshots,
}

pub(super) fn collect_kind(
    state: &mut State,
    source: SourceGeneration,
    scope: ProjectionCacheScope,
    now: u64,
    kind: CollectionKind,
) -> Result<ProjectionCacheCollection, Error> {
    fence_owner(
        state,
        source,
        scope.projection_id(),
        scope.source(),
        scope.expected_active(),
    )?;
    if !state
        .projection_cache
        .owners
        .contains(scope.projection_id())
    {
        return Err(Error::ProjectionRevisionConflict);
    }
    let identity = (scope.projection_id().clone(), scope.generation());
    let reserved = state
        .projection_generations
        .get(&identity)
        .is_some_and(|value| value.phase != projection_batch::Phase::Retired);
    let active = scope.expected_active() == Some(scope.generation());
    let snapshot_only = kind == CollectionKind::ObsoleteSnapshots;
    if snapshot_only {
        let invalidated = active
            && state.projections.iter().any(|status| {
                status.projection_id() == scope.projection_id()
                    && status.health() == ProjectionHealth::Invalidated
                    && status.active_rebuild().is_none()
            });
        let staging = state
            .projection_generations
            .get(&identity)
            .is_some_and(|value| value.phase == projection_batch::Phase::Staging);
        if ((reserved || active) && !invalidated) || staging {
            return Err(Error::ProjectionRevisionConflict);
        }
    }
    let protected = !snapshot_only && (active || reserved);
    let now = state.projection_cache.owner_time_ms.max(now);
    let documents: Vec<_> = if protected || snapshot_only {
        Vec::new()
    } else {
        state
            .projection_documents
            .range((
                Included((identity.0.clone(), identity.1, String::new())),
                Unbounded,
            ))
            .take_while(|((id, generation, _), _)| id == &identity.0 && generation == &identity.1)
            .take(PROJECTION_CACHE_COLLECTION_ROWS_MAX)
            .map(|(key, value)| (key.clone(), value.value().len() as u64))
            .collect()
    };
    let mut snapshots = Vec::new();
    let remaining = PROJECTION_CACHE_COLLECTION_ROWS_MAX - documents.len();
    for key in state
        .projection_cache
        .snapshot_order
        .range((
            Included((identity.0.clone(), identity.1, 0, [0; 32])),
            Unbounded,
        ))
        .take_while(|(id, generation, _, _)| id == &identity.0 && generation == &identity.1)
        .take(remaining)
    {
        let pressure = state.projection_cache.limits.is_some_and(|limit| {
            state.projection_snapshots.len() as u64 - snapshots.len() as u64
                > limit.legacy_snapshots()
        });
        if !protected || pressure || state.projection_cache.expired(key.2, now) {
            let value = state
                .projection_snapshots
                .get(&(key.0.clone(), key.3))
                .ok_or(Error::CorruptProjectionRecord)?;
            snapshots.push((key.clone(), value.value().len() as u64));
        } else {
            break;
        }
    }
    if protected {
        let start = snapshots.last().filter(|(key, _)| key.2 > now).map_or(
            (identity.0.clone(), identity.1, now, [255; 32]),
            |(key, _)| key.clone(),
        );
        for key in state
            .projection_cache
            .snapshot_order
            .range((Excluded(start), Unbounded))
            .take_while(|(id, generation, _, _)| id == &identity.0 && generation == &identity.1)
            .take(remaining - snapshots.len())
        {
            let value = state
                .projection_snapshots
                .get(&(key.0.clone(), key.3))
                .ok_or(Error::CorruptProjectionRecord)?;
            snapshots.push((key.clone(), value.value().len() as u64));
        }
    }
    let freed = documents
        .iter()
        .map(|(_, bytes)| *bytes)
        .chain(snapshots.iter().map(|(_, bytes)| *bytes))
        .try_fold(0u64, |sum, bytes| {
            sum.checked_add(bytes).ok_or(Error::CorruptProjectionRecord)
        })?;
    let value_bytes = state
        .projection_cache
        .value_bytes
        .checked_sub(freed)
        .ok_or(Error::CorruptProjectionRecord)?;
    let mut removed = (documents.len() + snapshots.len()) as u64;
    for (key, _) in documents {
        state.projection_documents.remove(&key);
    }
    for (key, _) in snapshots {
        state.projection_cache.snapshot_order.remove(&key);
        state.projection_snapshots.remove(&(key.0, key.3));
    }
    state.projection_cache.value_bytes = value_bytes;
    state.projection_cache.time(now);
    let document_more = !protected
        && !snapshot_only
        && state
            .projection_documents
            .range((
                Included((identity.0.clone(), identity.1, String::new())),
                Unbounded,
            ))
            .next()
            .is_some_and(|((id, generation, _), _)| id == &identity.0 && generation == &identity.1);
    let snapshot_more = state
        .projection_cache
        .snapshot_order
        .range((
            Included((identity.0.clone(), identity.1, 0, [0; 32])),
            Unbounded,
        ))
        .next()
        .is_some_and(|(id, generation, created, _)| {
            id == &identity.0
                && generation == &identity.1
                && (!protected
                    || state.projection_cache.expired(*created, now)
                    || state.projection_cache.limits.is_some_and(|limit| {
                        state.projection_snapshots.len() as u64 > limit.legacy_snapshots()
                    }))
        })
        || state
            .projection_cache
            .snapshot_order
            .range((
                Excluded((identity.0.clone(), identity.1, now, [255; 32])),
                Unbounded,
            ))
            .next()
            .is_some_and(|(id, generation, _, _)| id == &identity.0 && generation == &identity.1);
    if !protected
        && !snapshot_only
        && !document_more
        && !snapshot_more
        && state.projection_generations.contains_key(&identity)
        && removed < PROJECTION_CACHE_COLLECTION_ROWS_MAX as u64
    {
        state.projection_generations.remove(&identity);
        if let Some(generations) = state.projection_cache.retired.get_mut(&identity.0) {
            generations.remove(&identity.1);
            if generations.is_empty() {
                state.projection_cache.retired.remove(&identity.0);
            }
        }
        removed += 1;
    }
    let registration_more =
        !protected && !snapshot_only && state.projection_generations.contains_key(&identity);
    Ok(ProjectionCacheCollection {
        removed_rows: removed,
        removed_value_bytes: freed,
        more: document_more || snapshot_more || registration_more,
        usage: state.projection_cache.usage(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventStore, ProjectionStore, memory::MemoryStorage};
    use futures_executor::block_on;
    #[test]
    fn projection_cache_memory_owner_time_age_boundary_rollback_and_future_do_not_extend_lifetime()
    {
        let generation = SourceGeneration::new([9; 32]).unwrap();
        let store = MemoryStorage::new(generation);
        let id = ProjectionId::parse("clock_cache").unwrap();
        let projection_generation = ProjectionGeneration::new([1; 32]).unwrap();
        let created = now().unwrap();
        let snapshot =
            ProjectionSnapshot::new(id.clone(), [1; 32], projection_generation, created, vec![7])
                .unwrap();
        block_on(store.put_projection_snapshot(snapshot.clone())).unwrap();
        let limits = ProjectionCacheLimits::new(67_108_864, 2, 32, 1_800_000).unwrap();
        block_on(store.configure_projection_cache(id.clone(), limits)).unwrap();
        {
            let mut state = store.state().unwrap();
            assert!(!state.projection_cache.expired(created, created + 1_799_999));
            assert!(state.projection_cache.expired(created, created + 1_800_000));
            assert!(state.projection_cache.expired(created + 1, created));
            assert_eq!(
                state.projection_cache.time(created + 1_800_001),
                created + 1_800_001
            );
            assert_eq!(
                state.projection_cache.time(created - 1),
                created + 1_800_001
            );
        }
        assert_eq!(
            block_on(store.projection_snapshot(id.clone(), [1; 32])).unwrap(),
            None
        );
        block_on(store.put_projection_snapshot(snapshot.clone())).unwrap();
        assert_eq!(
            block_on(store.projection_snapshot(id.clone(), [1; 32])).unwrap(),
            None
        );
        let source = block_on(store.source_token()).unwrap();
        let exact =
            ProjectionCacheScope::new(id.clone(), projection_generation, source, None).unwrap();
        assert_eq!(
            block_on(store.collect_projection_cache(exact))
                .unwrap()
                .removed_rows,
            1
        );
        assert_eq!(
            block_on(store.put_projection_snapshot(snapshot)),
            Err(Error::ProjectionRevisionConflict)
        );
    }
}

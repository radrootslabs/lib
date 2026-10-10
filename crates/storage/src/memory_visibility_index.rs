use super::State;
use crate::event::{AdmissionStage, EventId};
use crate::{
    Error,
    event::{
        EventSourceToken, SourceGeneration, VisibilityEventMetadata, VisibilityIndexBounds,
        VisibilityIndexProgress,
    },
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub(super) struct Index {
    cursor: Option<Cursor>,
    pub(super) ready: bool,
    pub(super) events: BTreeMap<u64, VisibilityEventMetadata>,
    pub(super) visible: BTreeSet<u64>,
    pub(super) ids: BTreeMap<EventId, u64>,
    pub(super) verified: u64,
    heads: BTreeMap<Vec<u8>, u64>,
    address_heads: BTreeMap<String, u64>,
    deletion_events: BTreeSet<(EventId, [u8; 32])>,
    deletion_addresses: BTreeMap<(String, [u8; 32]), u64>,
    pending: BTreeSet<(u8, Vec<u8>)>,
}

#[derive(Clone, Copy)]
struct Cursor {
    target: u64,
    backfill: u64,
    replay_epoch: u64,
}

impl Index {
    pub(super) const fn new() -> Self {
        Self {
            cursor: Some(Cursor {
                target: 0,
                backfill: 0,
                replay_epoch: 0,
            }),
            ready: true,
            events: BTreeMap::new(),
            visible: BTreeSet::new(),
            ids: BTreeMap::new(),
            verified: 0,
            heads: BTreeMap::new(),
            address_heads: BTreeMap::new(),
            deletion_events: BTreeSet::new(),
            deletion_addresses: BTreeMap::new(),
            pending: BTreeSet::new(),
        }
    }

    fn refresh(&mut self, position: u64) {
        let Some(event) = self.events.get(&position) else {
            return;
        };
        let head = event
            .head_coordinate
            .as_ref()
            .is_none_or(|key| self.heads.get(key) == Some(&position));
        let deleted = !event.deletion_request
            && (self
                .deletion_events
                .contains(&(event.event_id, event.author))
                || event
                    .deletion_coordinate
                    .as_ref()
                    .is_some_and(|coordinate| {
                        self.deletion_addresses
                            .get(&(coordinate.clone(), event.author))
                            .is_some_and(|time| *time >= event.created_at)
                    }));
        if event.stage == AdmissionStage::Visible && !event.ephemeral && head && !deleted {
            self.visible.insert(position);
        } else {
            self.visible.remove(&position);
        }
    }

    fn put(&mut self, position: u64, event: VisibilityEventMetadata) {
        let previously_verified = self
            .events
            .get(&position)
            .is_some_and(|old| old.stage != AdmissionStage::Raw);
        if !previously_verified && event.stage != AdmissionStage::Raw {
            self.verified += 1;
        }
        self.ids.insert(event.event_id, position);
        for target in &event.event_targets {
            self.deletion_events.insert((*target, event.author));
            self.pending.insert((0, target.as_bytes().to_vec()));
        }
        for coordinate in &event.address_targets {
            self.deletion_addresses
                .entry((coordinate.clone(), event.author))
                .and_modify(|time| *time = (*time).max(event.created_at))
                .or_insert(event.created_at);
            self.pending.insert((1, coordinate.as_bytes().to_vec()));
        }
        let old_head = event
            .head_coordinate
            .as_ref()
            .and_then(|key| self.heads.get(key))
            .copied();
        let replace = old_head.is_none_or(|old| {
            self.events.get(&old).is_some_and(|current| {
                event.created_at > current.created_at
                    || (event.created_at == current.created_at && event.event_id < current.event_id)
            })
        });
        if let Some(key) = &event.head_coordinate
            && replace
        {
            self.heads.insert(key.clone(), position);
            if let Some(old) = old_head.and_then(|old| self.events.get(&old))
                && let Some(address) = &old.deletion_coordinate
            {
                self.address_heads.remove(address);
            }
            if let Some(address) = &event.deletion_coordinate {
                self.address_heads.insert(address.clone(), position);
            }
        }
        self.events.insert(position, event);
        if replace && let Some(old) = old_head {
            self.refresh(old);
        }
        self.refresh(position);
    }

    fn drain(&mut self, maximum: u16, charged: &mut u64, byte_limit: u64) -> Result<u16, Error> {
        let mut processed = 0;
        while processed < maximum {
            let Some((kind, key)) = self.pending.first() else {
                break;
            };
            let charge = charged
                .checked_add(4096 + 16 * key.len() as u64)
                .ok_or(Error::SpaceInsufficient)?;
            if charge > byte_limit {
                break;
            }
            let position = if *kind == 0 {
                let id = EventId::from_bytes(
                    key.as_slice()
                        .try_into()
                        .map_err(|_| Error::CorruptStoredEvent)?,
                );
                self.ids.get(&id).copied()
            } else {
                let address = std::str::from_utf8(key).map_err(|_| Error::CorruptStoredEvent)?;
                self.address_heads.get(address).copied()
            };
            self.pending.pop_first();
            if let Some(position) = position {
                self.refresh(position);
            }
            *charged = charge;
            processed += 1;
        }
        Ok(processed)
    }
}

pub(super) fn prepare(
    generation: SourceGeneration,
    state: &mut State,
    bounds: VisibilityIndexBounds,
) -> Result<VisibilityIndexProgress, Error> {
    let mut charged = 4096 + u64::from(bounds.rows()) * 128;
    if charged > bounds.bytes() {
        return Err(Error::SpaceInsufficient);
    }
    let sequence = u64::try_from(state.events.len()).map_err(|_| Error::CorruptStoredEvent)?;
    let mut cursor = state.visibility_index.cursor.unwrap_or(Cursor {
        target: sequence,
        backfill: 0,
        replay_epoch: state.event_mutation_epoch,
    });
    let building = cursor.backfill < cursor.target;
    let positions = if !state.visibility_index.pending.is_empty() {
        Vec::new()
    } else if building {
        ((cursor.backfill + 1)..=cursor.target)
            .take(usize::from(bounds.rows()))
            .collect::<Vec<_>>()
    } else {
        state
            .event_changes
            .range((
                std::ops::Bound::Excluded(cursor.replay_epoch),
                std::ops::Bound::Unbounded,
            ))
            .take(usize::from(bounds.rows()))
            .map(|(_, sequence)| *sequence)
            .collect()
    };
    let mut prepared = Vec::new();
    for position in positions {
        let entry = state
            .events
            .get(usize::try_from(position - 1).map_err(|_| Error::CorruptStoredEvent)?)
            .ok_or(Error::CorruptStoredEvent)?;
        let bytes = u64::try_from(entry.admission.event().raw_json().len())
            .map_err(|_| Error::SpaceInsufficient)?;
        let next_charge = charged
            .checked_add(VisibilityIndexBounds::charge(bytes)?)
            .ok_or(Error::SpaceInsufficient)?;
        if next_charge > bounds.bytes() {
            if prepared.is_empty() {
                return Err(Error::SpaceInsufficient);
            }
            break;
        }
        let metadata =
            VisibilityEventMetadata::from_event(entry.admission.event(), entry.admission.stage())?;
        prepared.push((position, metadata));
        charged = next_charge;
        if building {
            cursor.backfill = position;
        } else {
            cursor.replay_epoch = entry.last_mutation_epoch;
        }
    }
    let pending = state
        .event_changes
        .range((
            std::ops::Bound::Excluded(cursor.replay_epoch),
            std::ops::Bound::Unbounded,
        ))
        .next()
        .is_some();
    let source_complete = cursor.backfill == cursor.target && !pending;
    if source_complete {
        cursor.replay_epoch = state.event_mutation_epoch;
    }
    let processed = u16::try_from(prepared.len()).map_err(|_| Error::CorruptStoredEvent)?;
    for (position, event) in prepared {
        state.visibility_index.put(position, event);
    }
    let keys =
        state
            .visibility_index
            .drain(bounds.rows() - processed, &mut charged, bounds.bytes())?;
    if processed == 0 && keys == 0 && !state.visibility_index.pending.is_empty() {
        return Err(Error::SpaceInsufficient);
    }
    let complete = source_complete && state.visibility_index.pending.is_empty();
    if complete && state.visibility_index.events.len() != state.events.len() {
        return Err(Error::CorruptStoredEvent);
    }
    state.visibility_index.cursor = Some(cursor);
    state.visibility_index.ready = complete;
    Ok(VisibilityIndexProgress::new(
        EventSourceToken::new(generation, state.event_mutation_epoch),
        cursor.backfill,
        cursor.replay_epoch,
        processed,
        charged,
        complete,
    )
    .with_processed_keys(keys))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventStore, memory::MemoryStorage};
    use futures_executor::block_on;
    use radroots_event::{SignedEvent, wire::Nip01EventWire};
    use radroots_transport::{
        Target, TransportId,
        source::{EventProvenance, ObservedEvent},
    };

    fn admission(content: &str, time: u64) -> crate::event::EventAdmission {
        let mut wire = Nip01EventWire {
            id: "0".repeat(64),
            pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
            created_at: time,
            kind: 0,
            tags: vec![],
            content: content.to_owned(),
            sig: "42".repeat(64),
            extra: Default::default(),
        };
        wire.id = wire.computed_event_id().unwrap().to_hex();
        let raw = serde_json::to_string(&wire).unwrap();
        let event = SignedEvent::from_wire_verified_id(wire, raw).unwrap();
        let target = Target::nostr_relay("wss://relay.example").unwrap();
        crate::event::EventAdmission::raw(ObservedEvent::new(
            event,
            EventProvenance::new(TransportId::NOSTR, target.fingerprint().clone(), 100).unwrap(),
        ))
    }

    #[test]
    fn memory_visibility_index_retains_backfill_and_refuses_oversized_rows_without_partial_state() {
        let generation = SourceGeneration::new([82; 32]).unwrap();
        let storage = MemoryStorage::new(generation);
        storage.state().unwrap().visibility_index = Index::default();
        let first = admission("first", 10);
        block_on(storage.admit(first.clone())).unwrap();
        block_on(storage.admit(admission("second", 20))).unwrap();
        let bounds = VisibilityIndexBounds::new(1, 16 * 1024 * 1024).unwrap();
        let part = block_on(storage.maintain_visibility(bounds)).unwrap();
        assert_eq!(part.source_sequence(), 1);
        assert!(!part.is_complete());
        block_on(storage.admit(admission("third", 30))).unwrap();
        let next = block_on(storage.maintain_visibility(bounds)).unwrap();
        assert_eq!(next.source_sequence(), 2);
        assert!(!next.is_complete());
        assert!(
            block_on(storage.maintain_visibility(bounds))
                .unwrap()
                .is_complete()
        );
        assert_eq!(storage.state().unwrap().visibility_index.events.len(), 3);
        block_on(storage.admit(first)).unwrap();
        assert_eq!(
            block_on(storage.maintain_visibility(bounds))
                .unwrap()
                .processed_rows(),
            0
        );
        // Exercise explicit maintenance of an already dirty index. Ordinary
        // ready-index admission is separately covered by indexed-read parity.
        storage.state().unwrap().visibility_index.ready = false;
        block_on(storage.admit(admission(&"x".repeat(1000), 40))).unwrap();
        assert_eq!(
            block_on(storage.maintain_visibility(VisibilityIndexBounds::new(1, 10000).unwrap())),
            Err(Error::SpaceInsufficient)
        );
        assert_eq!(storage.state().unwrap().visibility_index.events.len(), 3);
        assert!(!storage.state().unwrap().visibility_index.ready);
        assert!(
            block_on(storage.maintain_visibility(bounds))
                .unwrap()
                .is_complete()
        );
    }

    #[test]
    fn memory_visibility_index_empty_source_has_a_real_zero_epoch_fence() {
        let generation = SourceGeneration::new([83; 32]).unwrap();
        let storage = MemoryStorage::new(generation);
        let progress = block_on(
            storage.maintain_visibility(VisibilityIndexBounds::new(64, 16 * 1024 * 1024).unwrap()),
        )
        .unwrap();
        assert!(progress.is_complete());
        assert_eq!(progress.token(), EventSourceToken::new(generation, 0));
        assert_eq!(progress.processed_rows(), 0);
        assert!(storage.state().unwrap().visibility_index.events.is_empty());
    }
}

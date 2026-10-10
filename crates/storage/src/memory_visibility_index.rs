use super::State;
use crate::{
    Error,
    event::{
        EventSourceToken, SourceGeneration, VisibilityEventMetadata, VisibilityIndexBounds,
        VisibilityIndexProgress,
    },
};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(super) struct Index {
    cursor: Option<Cursor>,
    pub(super) ready: bool,
    pub(super) events: BTreeMap<u64, VisibilityEventMetadata>,
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
            cursor: None,
            ready: false,
            events: BTreeMap::new(),
        }
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
    let positions = if building {
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
    let complete = cursor.backfill == cursor.target && !pending;
    if complete {
        cursor.replay_epoch = state.event_mutation_epoch;
    }
    let processed = u16::try_from(prepared.len()).map_err(|_| Error::CorruptStoredEvent)?;
    state.visibility_index.events.extend(prepared);
    state.visibility_index.cursor = Some(cursor);
    state.visibility_index.ready = complete;
    Ok(VisibilityIndexProgress::new(
        EventSourceToken::new(generation, state.event_mutation_epoch),
        cursor.backfill,
        cursor.replay_epoch,
        processed,
        charged,
        complete,
    ))
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

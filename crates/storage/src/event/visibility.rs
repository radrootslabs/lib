//! Deterministic current-visibility reduction over immutable event truth.

use std::collections::{BTreeMap, BTreeSet};

use radroots_event::{
    EventId, SignedEvent,
    envelope::event_head::{
        CurrentEventHead, EventHeadCandidateResult, EventHeadCoordinate, EventHeadDecision,
        event_head_candidate_for_nip01_event, select_event_head,
    },
    envelope::kind::KIND_DELETION_REQUEST,
    id::Nip01Coordinate,
};
use sha2::{Digest, Sha256};

use super::{
    AdmissionStage, EventPosition, SourceGeneration, VisibilityDigest, VisibilitySnapshot,
};
use crate::Error;

#[doc(hidden)]
pub struct VisibilityInput<'a> {
    position: EventPosition,
    event: &'a SignedEvent,
    stage: AdmissionStage,
}

impl<'a> VisibilityInput<'a> {
    #[doc(hidden)]
    pub const fn new(
        position: EventPosition,
        event: &'a SignedEvent,
        stage: AdmissionStage,
    ) -> Self {
        Self {
            position,
            event,
            stage,
        }
    }
}

#[doc(hidden)]
pub struct VisibilityEvaluation {
    snapshot: VisibilitySnapshot,
    visible: BTreeSet<EventId>,
}

impl VisibilityEvaluation {
    #[doc(hidden)]
    pub fn is_visible(&self, event_id: &EventId) -> bool {
        self.visible.contains(event_id)
    }

    #[doc(hidden)]
    pub const fn snapshot(&self) -> &VisibilitySnapshot {
        &self.snapshot
    }

    #[doc(hidden)]
    pub fn into_snapshot(self) -> VisibilitySnapshot {
        self.snapshot
    }
}

#[derive(Clone)]
struct Candidate<'a> {
    position: EventPosition,
    event: &'a SignedEvent,
    coordinate: Option<EventHeadCoordinate>,
    ephemeral: bool,
}

struct DeletionRequest {
    request_id: EventId,
    author: [u8; 32],
    created_at: u64,
    event_targets: BTreeSet<EventId>,
    address_targets: BTreeSet<Nip01Coordinate>,
}

#[doc(hidden)]
pub fn evaluate_visibility<'a>(
    generation: SourceGeneration,
    inputs: impl IntoIterator<Item = VisibilityInput<'a>>,
) -> Result<VisibilityEvaluation, Error> {
    let mut candidates = Vec::new();
    let mut deletion_requests = Vec::new();
    let mut heads = BTreeMap::<EventHeadCoordinate, CurrentEventHead>::new();

    for input in inputs {
        if input.position.generation() != generation {
            return Err(Error::CorruptStoredEvent);
        }
        if input.stage != AdmissionStage::Visible {
            continue;
        }
        let envelope = input.event.envelope();
        if envelope.kind_u32() == KIND_DELETION_REQUEST {
            deletion_requests.push(parse_deletion_request(input.event)?);
        }
        let (coordinate, ephemeral) = match event_head_candidate_for_nip01_event(envelope) {
            EventHeadCandidateResult::Candidate(candidate) => {
                let coordinate = candidate.coordinate.clone();
                match select_event_head(candidate, heads.get(&coordinate)) {
                    EventHeadDecision::Applied(head) => {
                        heads.insert(coordinate.clone(), head);
                    }
                    EventHeadDecision::SkippedDuplicate
                    | EventHeadDecision::SkippedOlder
                    | EventHeadDecision::SkippedSameTimestampHigherEventId => {}
                    EventHeadDecision::CoordinateMismatch => {
                        return Err(Error::CorruptStoredEvent);
                    }
                }
                (Some(coordinate), false)
            }
            EventHeadCandidateResult::NotHeadSelected => (None, false),
            EventHeadCandidateResult::NotPersisted => (None, true),
            EventHeadCandidateResult::Malformed(_) => return Err(Error::CorruptStoredEvent),
        };
        candidates.push(Candidate {
            position: input.position,
            event: input.event,
            coordinate,
            ephemeral,
        });
    }

    candidates.sort_by_key(|candidate| candidate.position.sequence());
    deletion_requests.sort_by_key(|request| request.request_id);

    let mut visible = BTreeSet::new();
    let mut suppressed = BTreeSet::new();
    let mut superseded = BTreeSet::new();
    for candidate in candidates {
        let event_id = *candidate.event.id();
        if candidate.ephemeral {
            suppressed.insert(event_id);
            continue;
        }
        if candidate.coordinate.as_ref().is_some_and(|coordinate| {
            heads
                .get(coordinate)
                .is_none_or(|head| head.event_id != event_id)
        }) {
            superseded.insert(event_id);
            continue;
        }
        if is_suppressed(candidate.event, &deletion_requests) {
            suppressed.insert(event_id);
        } else {
            visible.insert(event_id);
        }
    }

    let current_heads = heads.into_values().collect::<Vec<_>>();
    let deletion_request_ids = deletion_requests
        .iter()
        .map(|request| request.request_id)
        .collect::<Vec<_>>();
    let visible_event_ids = visible.iter().copied().collect::<Vec<_>>();
    let suppressed_event_ids = suppressed.into_iter().collect::<Vec<_>>();
    let superseded_event_ids = superseded.into_iter().collect::<Vec<_>>();
    let digest = visibility_digest(
        generation,
        current_heads.as_slice(),
        deletion_request_ids.as_slice(),
        visible_event_ids.as_slice(),
        suppressed_event_ids.as_slice(),
        superseded_event_ids.as_slice(),
    )?;
    Ok(VisibilityEvaluation {
        snapshot: VisibilitySnapshot::new(
            generation,
            current_heads,
            deletion_request_ids,
            visible_event_ids,
            suppressed_event_ids,
            superseded_event_ids,
            digest,
        ),
        visible,
    })
}

fn parse_deletion_request(event: &SignedEvent) -> Result<DeletionRequest, Error> {
    let mut event_targets = BTreeSet::new();
    let mut address_targets = BTreeSet::new();
    for tag in event.envelope().tag_slices() {
        match tag.as_slice().first().map(String::as_str) {
            Some("e") => {
                let value = tag.as_slice().get(1).ok_or(Error::CorruptStoredEvent)?;
                event_targets.insert(EventId::parse(value).map_err(|_| Error::CorruptStoredEvent)?);
            }
            Some("a") => {
                let value = tag.as_slice().get(1).ok_or(Error::CorruptStoredEvent)?;
                address_targets
                    .insert(Nip01Coordinate::parse(value).map_err(|_| Error::CorruptStoredEvent)?);
            }
            _ => {}
        }
    }
    if event_targets.is_empty() && address_targets.is_empty() {
        return Err(Error::CorruptStoredEvent);
    }
    Ok(DeletionRequest {
        request_id: *event.id(),
        author: *event.envelope().author().as_bytes(),
        created_at: event.envelope().created_at_u64(),
        event_targets,
        address_targets,
    })
}

fn is_suppressed(target: &SignedEvent, requests: &[DeletionRequest]) -> bool {
    let target_event = target.envelope();
    if target_event.kind_u32() == KIND_DELETION_REQUEST {
        return false;
    }
    let coordinate = event_coordinate(target_event);
    requests.iter().any(|request| {
        let request_event_id_match = request.event_targets.contains(target_event.id());
        let request_coordinate_match = coordinate.as_ref().is_some_and(|coordinate| {
            request.address_targets.contains(coordinate)
                && target_event.created_at_u64() <= request.created_at
        });
        (request_event_id_match || request_coordinate_match)
            && request.author == *target_event.author().as_bytes()
    })
}

fn event_coordinate(event: &radroots_event::Event) -> Option<Nip01Coordinate> {
    let kind = event.kind_u32();
    let identifier = match event.kind_class() {
        radroots_event::envelope::EventKindClass::Replaceable => "",
        radroots_event::envelope::EventKindClass::Addressable => event
            .tag_slices()
            .iter()
            .find(|tag| tag.as_slice().first().is_some_and(|name| name == "d"))?
            .as_slice()
            .get(1)?
            .as_str(),
        radroots_event::envelope::EventKindClass::Regular
        | radroots_event::envelope::EventKindClass::Ephemeral => return None,
    };
    Nip01Coordinate::parse(format!("{kind}:{}:{identifier}", event.author())).ok()
}

fn visibility_digest(
    generation: SourceGeneration,
    heads: &[CurrentEventHead],
    deletion_requests: &[EventId],
    visible: &[EventId],
    suppressed: &[EventId],
    superseded: &[EventId],
) -> Result<VisibilityDigest, Error> {
    let mut digest = Sha256::new();
    digest.update(b"radroots.storage.visibility.v1\0");
    digest.update(generation.as_bytes());
    for head in heads {
        digest.update(b"head\0");
        match &head.coordinate {
            EventHeadCoordinate::Replaceable { kind, pubkey } => {
                digest.update(b"replaceable\0");
                digest.update(kind.to_be_bytes());
                digest.update(pubkey.as_bytes());
            }
            EventHeadCoordinate::Addressable {
                kind,
                pubkey,
                d_tag,
            } => {
                digest.update(b"addressable\0");
                digest.update(kind.to_be_bytes());
                digest.update(pubkey.as_bytes());
                let d_tag_length =
                    u64::try_from(d_tag.len()).map_err(|_| Error::CorruptStoredEvent)?;
                digest.update(d_tag_length.to_be_bytes());
                digest.update(d_tag.as_bytes());
            }
        }
        digest.update(head.event_id.as_bytes());
        digest.update(head.created_at.to_be_bytes());
    }
    update_event_ids(&mut digest, b"deletion\0", deletion_requests);
    update_event_ids(&mut digest, b"visible\0", visible);
    update_event_ids(&mut digest, b"suppressed\0", suppressed);
    update_event_ids(&mut digest, b"superseded\0", superseded);
    Ok(VisibilityDigest::new(digest.finalize().into()))
}

fn update_event_ids(digest: &mut Sha256, prefix: &[u8], event_ids: &[EventId]) {
    for event_id in event_ids {
        digest.update(prefix);
        digest.update(event_id.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::wire::Nip01EventWire;

    const AUTHOR: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const OTHER_AUTHOR: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    fn signed_event(
        author: &str,
        created_at: u64,
        kind: u32,
        tags: Vec<Vec<&str>>,
        content: &str,
    ) -> SignedEvent {
        let tags = tags
            .into_iter()
            .map(|tag| tag.into_iter().map(str::to_owned).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let mut wire = Nip01EventWire {
            id: "0".repeat(64),
            pubkey: author.to_owned(),
            created_at,
            kind,
            tags,
            content: content.to_owned(),
            sig: "42".repeat(64),
            extra: Default::default(),
        };
        wire.id = wire.computed_event_id().expect("event id").to_hex();
        let raw_json = serde_json::json!({
            "id": &wire.id,
            "pubkey": &wire.pubkey,
            "created_at": wire.created_at,
            "kind": wire.kind,
            "tags": &wire.tags,
            "content": &wire.content,
            "sig": &wire.sig,
        })
        .to_string();
        SignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    fn evaluate<'a>(
        events: impl IntoIterator<Item = (&'a SignedEvent, AdmissionStage)>,
    ) -> VisibilityEvaluation {
        let generation = SourceGeneration::new([7; 32]).expect("generation");
        evaluate_visibility(
            generation,
            events
                .into_iter()
                .enumerate()
                .map(|(index, (event, stage))| {
                    let sequence = u64::try_from(index)
                        .expect("index")
                        .checked_add(1)
                        .and_then(|value| super::super::EventSequence::new(value).ok())
                        .expect("sequence");
                    VisibilityInput::new(EventPosition::new(generation, sequence), event, stage)
                }),
        )
        .expect("visibility")
    }

    #[test]
    fn selected_head_is_stable_and_a_deleted_head_does_not_resurrect() {
        let old = signed_event(AUTHOR, 10, 0, vec![], r#"{"name":"old"}"#);
        let current = signed_event(AUTHOR, 20, 0, vec![], r#"{"name":"current"}"#);
        let deletion = signed_event(
            AUTHOR,
            30,
            KIND_DELETION_REQUEST,
            vec![vec!["e", current.id().to_hex().as_str()]],
            "",
        );
        let evaluation = evaluate([
            (&old, AdmissionStage::Visible),
            (&current, AdmissionStage::Visible),
            (&deletion, AdmissionStage::Visible),
        ]);

        assert!(!evaluation.is_visible(old.id()));
        assert!(!evaluation.is_visible(current.id()));
        assert!(evaluation.is_visible(deletion.id()));
        assert_eq!(evaluation.snapshot().superseded_event_ids(), &[*old.id()]);
        assert_eq!(
            evaluation.snapshot().suppressed_event_ids(),
            &[*current.id()]
        );
        assert_eq!(
            evaluation.snapshot().current_heads()[0].event_id,
            *current.id()
        );
    }

    #[test]
    fn address_cutoff_allows_a_later_replacement_and_wrong_author_is_ineffective() {
        let old = signed_event(AUTHOR, 10, 30_023, vec![vec!["d", "farm-update"]], "old");
        let wrong_author_deletion = signed_event(
            OTHER_AUTHOR,
            15,
            KIND_DELETION_REQUEST,
            vec![vec!["a", format!("30023:{AUTHOR}:farm-update").as_str()]],
            "",
        );
        let cutoff = signed_event(
            AUTHOR,
            20,
            KIND_DELETION_REQUEST,
            vec![vec!["a", format!("30023:{AUTHOR}:farm-update").as_str()]],
            "",
        );
        let later = signed_event(AUTHOR, 30, 30_023, vec![vec!["d", "farm-update"]], "later");
        let evaluation = evaluate([
            (&old, AdmissionStage::Visible),
            (&wrong_author_deletion, AdmissionStage::Visible),
            (&cutoff, AdmissionStage::Visible),
            (&later, AdmissionStage::Visible),
        ]);

        assert!(evaluation.is_visible(later.id()));
        assert!(evaluation.is_visible(wrong_author_deletion.id()));
        assert!(evaluation.is_visible(cutoff.id()));
        assert!(!evaluation.is_visible(old.id()));
        assert_eq!(
            evaluation.snapshot().current_heads()[0].event_id,
            *later.id()
        );
    }

    #[test]
    fn rebuild_is_order_independent_and_ignores_nonvisible_admissions() {
        let visible = signed_event(AUTHOR, 10, 1, vec![], "visible");
        let excluded = signed_event(AUTHOR, 20, 1, vec![], "excluded");
        let ephemeral = signed_event(AUTHOR, 30, 20_000, vec![], "ephemeral");
        let first = evaluate([
            (&visible, AdmissionStage::Visible),
            (&excluded, AdmissionStage::Verified),
            (&ephemeral, AdmissionStage::Visible),
        ]);
        let second = evaluate([
            (&ephemeral, AdmissionStage::Visible),
            (&excluded, AdmissionStage::Verified),
            (&visible, AdmissionStage::Visible),
        ]);

        assert!(first.is_visible(visible.id()));
        assert!(!first.is_visible(excluded.id()));
        assert!(!first.is_visible(ephemeral.id()));
        assert_eq!(first.snapshot().digest(), second.snapshot().digest());
        assert_eq!(first.snapshot(), second.snapshot());
    }

    #[test]
    fn deletion_requests_without_targets_fail_closed() {
        let deletion = signed_event(AUTHOR, 10, KIND_DELETION_REQUEST, vec![], "");
        let generation = SourceGeneration::new([7; 32]).expect("generation");
        let position = EventPosition::new(
            generation,
            super::super::EventSequence::new(1).expect("sequence"),
        );

        let error = evaluate_visibility(
            generation,
            [VisibilityInput::new(
                position,
                &deletion,
                AdmissionStage::Visible,
            )],
        )
        .err()
        .expect("targetless deletion must fail");

        assert_eq!(error, Error::CorruptStoredEvent);
    }

    #[test]
    fn events_from_another_generation_fail_closed() {
        let event = signed_event(AUTHOR, 10, 1, vec![], "event");
        let requested_generation = SourceGeneration::new([7; 32]).expect("generation");
        let stored_generation = SourceGeneration::new([8; 32]).expect("generation");
        let position = EventPosition::new(
            stored_generation,
            super::super::EventSequence::new(1).expect("sequence"),
        );

        let error = evaluate_visibility(
            requested_generation,
            [VisibilityInput::new(
                position,
                &event,
                AdmissionStage::Visible,
            )],
        )
        .err()
        .expect("cross-generation visibility input must fail");

        assert_eq!(error, Error::CorruptStoredEvent);
    }
}

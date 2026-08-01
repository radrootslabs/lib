use radroots_event::EventId;
use radroots_storage::{
    Error, ProjectionStore,
    event::{EventPosition, EventSequence, SourceGeneration},
    projection::{
        ArtifactDigest, EventIdRange, EventIndexCheckpoint, EventIndexManifest, EventIndexShard,
        EventIndexShardCheckpoint, EventIndexShardId, InvalidationReason, ProjectionCheckpoint,
        ProjectionGeneration, ProjectionHealth, ProjectionId, ProjectionRevision, ProjectionStatus,
        RebuildStage, RebuildTicket, RebuildTicketId, RebuildTransition,
    },
};

fn projection_id() -> ProjectionId {
    ProjectionId::parse("food_availability.v1").expect("projection id")
}

fn generation(byte: u8) -> ProjectionGeneration {
    ProjectionGeneration::new([byte; 32]).expect("projection generation")
}

fn event_id(character: char) -> EventId {
    EventId::parse(character.to_string().repeat(64)).expect("event id")
}

fn position(source_byte: u8, sequence: u64) -> EventPosition {
    EventPosition::new(
        SourceGeneration::new([source_byte; 32]).expect("source generation"),
        EventSequence::new(sequence).expect("event sequence"),
    )
}

fn checkpoint(
    generation: ProjectionGeneration,
    sequence: u64,
    rows: u64,
    at: u64,
) -> ProjectionCheckpoint {
    ProjectionCheckpoint::new(
        projection_id(),
        generation,
        Some(position(9, sequence)),
        rows,
        at,
    )
    .expect("projection checkpoint")
}

fn shard(
    id: &str,
    path: &str,
    first: char,
    last: char,
    first_at: u64,
    last_at: u64,
) -> EventIndexShard {
    EventIndexShard::new(
        EventIndexShardId::parse(id).expect("shard id"),
        path,
        2,
        EventIdRange::new(event_id(first), event_id(last)).expect("event range"),
        first_at,
        last_at,
        ArtifactDigest::new([id.as_bytes()[0]; 32]),
    )
    .expect("event-index shard")
}

#[test]
fn checkpoints_require_monotonic_source_and_row_progress() {
    let prior = checkpoint(generation(1), 10, 4, 100);
    assert!(checkpoint(generation(1), 11, 5, 101).advances(&prior));
    assert!(!checkpoint(generation(1), 9, 5, 101).advances(&prior));
    assert!(!checkpoint(generation(1), 11, 3, 101).advances(&prior));
    let other_source = ProjectionCheckpoint::new(
        projection_id(),
        generation(1),
        Some(position(8, 11)),
        5,
        101,
    )
    .expect("checkpoint");
    assert!(!other_source.advances(&prior));
    assert_eq!(
        ProjectionCheckpoint::new(projection_id(), generation(1), None, 0, 0),
        Err(Error::InvalidProjectionTimestamp)
    );
}

#[test]
fn manifests_validate_typed_ranges_totals_order_paths_and_bounds() {
    let first = shard("a", "index/a.json", '0', '3', 10, 20);
    let second = shard("b", "index/b.json", '4', '7', 20, 30);
    let manifest = EventIndexManifest::new(
        generation(2),
        4,
        2,
        10,
        30,
        vec![first.clone(), second.clone()],
    )
    .expect("manifest");
    assert_eq!(manifest.total_events(), 4);
    assert_eq!(manifest.shards().len(), 2);

    assert_eq!(
        EventIndexManifest::new(
            generation(2),
            5,
            2,
            10,
            30,
            vec![first.clone(), second.clone()]
        ),
        Err(Error::InvalidEventIndexManifest)
    );
    let overlap = shard("b", "index/b.json", '3', '7', 20, 30);
    assert_eq!(
        EventIndexManifest::new(generation(2), 4, 2, 10, 30, vec![first.clone(), overlap]),
        Err(Error::InvalidEventIndexManifest)
    );
    let duplicate_path = shard("b", "index/a.json", '4', '7', 20, 30);
    assert_eq!(
        EventIndexManifest::new(generation(2), 4, 2, 10, 30, vec![first, duplicate_path]),
        Err(Error::InvalidEventIndexManifest)
    );
    assert_eq!(
        EventIndexShard::new(
            EventIndexShardId::parse("unsafe").expect("id"),
            "../escape.json",
            1,
            EventIdRange::new(event_id('0'), event_id('1')).expect("range"),
            1,
            2,
            ArtifactDigest::new([1; 32]),
        ),
        Err(Error::InvalidEventIndexArtifactPath)
    );
}

#[test]
fn event_index_checkpoints_sort_lookup_and_reject_duplicates() {
    let first = EventIndexShardCheckpoint::new(
        EventIndexShardId::parse("a").expect("id"),
        10,
        Some(event_id('1')),
        Some("cursor-a".to_owned()),
    )
    .expect("checkpoint");
    let second = EventIndexShardCheckpoint::new(
        EventIndexShardId::parse("b").expect("id"),
        20,
        Some(event_id('2')),
        None,
    )
    .expect("checkpoint");
    let checkpoint = EventIndexCheckpoint::new(generation(2), 100, vec![second, first.clone()])
        .expect("index checkpoint");
    assert_eq!(
        checkpoint.shard(&EventIndexShardId::parse("a").expect("id")),
        Some(&first)
    );
    assert_eq!(
        EventIndexCheckpoint::new(generation(2), 100, vec![first.clone(), first]),
        Err(Error::DuplicateEventIndexShard)
    );
    assert_eq!(
        EventIndexShardCheckpoint::new(
            EventIndexShardId::parse("a").expect("id"),
            10,
            None,
            Some("x".repeat(2_049)),
        ),
        Err(Error::InvalidEventIndexCursor)
    );
}

#[test]
fn invalidation_and_rebuild_lifecycle_is_optimistic_and_terminal() {
    let invalidation = radroots_storage::projection::ProjectionInvalidation::new(
        projection_id(),
        generation(1),
        generation(2),
        InvalidationReason::ProjectionGenerationChanged,
        100,
    )
    .expect("invalidation");
    let ticket_id = RebuildTicketId::new([7; 16]).expect("ticket id");
    let requested = RebuildTicket::requested(ticket_id, invalidation);
    assert_eq!(requested.stage(), RebuildStage::Requested);

    let running = requested
        .transition(RebuildTransition::start(
            ticket_id,
            ProjectionRevision::INITIAL,
            110,
        ))
        .expect("start rebuild");
    assert_eq!(running.stage(), RebuildStage::Running);
    assert_eq!(
        running.transition(RebuildTransition::checkpoint(
            ticket_id,
            ProjectionRevision::INITIAL,
            120,
            checkpoint(generation(2), 10, 4, 120),
        )),
        Err(Error::ProjectionRevisionConflict)
    );
    let progressed = running
        .transition(RebuildTransition::checkpoint(
            ticket_id,
            running.revision(),
            120,
            checkpoint(generation(2), 10, 4, 120),
        ))
        .expect("checkpoint rebuild");
    assert_eq!(
        progressed.transition(RebuildTransition::checkpoint(
            ticket_id,
            progressed.revision(),
            130,
            checkpoint(generation(2), 9, 5, 130),
        )),
        Err(Error::ProjectionCheckpointRegression)
    );
    let completed = progressed
        .transition(RebuildTransition::complete(
            ticket_id,
            progressed.revision(),
            140,
            checkpoint(generation(2), 11, 5, 140),
        ))
        .expect("complete rebuild");
    assert_eq!(completed.stage(), RebuildStage::Completed);
    assert_eq!(
        completed.transition(RebuildTransition::fail(
            ticket_id,
            completed.revision(),
            150,
        )),
        Err(Error::RebuildTicketTerminal)
    );

    let status = ProjectionStatus::new(
        projection_id(),
        generation(2),
        ProjectionHealth::Ready,
        completed.checkpoint().cloned(),
        None,
    )
    .expect("ready status");
    assert_eq!(status.health(), ProjectionHealth::Ready);
}

#[test]
fn projection_spi_is_dyn_compatible_and_validated_identifiers_fail_closed() {
    fn accepts_dyn(_: Option<&dyn ProjectionStore>) {}
    accepts_dyn(None);
    assert_eq!(
        ProjectionId::parse("Uppercase"),
        Err(Error::InvalidProjectionId)
    );
    assert_eq!(
        ProjectionGeneration::new([0; 32]),
        Err(Error::InvalidProjectionGeneration)
    );
    assert_eq!(
        RebuildTicketId::new([0; 16]),
        Err(Error::InvalidRebuildTicketId)
    );
    assert_eq!(
        radroots_storage::projection::ProjectionInvalidation::new(
            projection_id(),
            generation(1),
            generation(1),
            InvalidationReason::OperatorRequested,
            1,
        ),
        Err(Error::InvalidProjectionInvalidation)
    );
}

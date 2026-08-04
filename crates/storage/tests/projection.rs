use radroots_event::EventId;
use radroots_storage::{
    Error, ProjectionStore,
    event::{EventPosition, EventSequence, SourceGeneration},
    projection::{
        ArtifactDigest, EventIdRange, EventIndexCheckpoint, EventIndexManifest, EventIndexShard,
        EventIndexShardCheckpoint, EventIndexShardId, InvalidationReason, ProjectionCheckpoint,
        ProjectionGeneration, ProjectionHealth, ProjectionId, ProjectionInvalidation,
        ProjectionRevision, ProjectionStatus, RawSourceDigest, RebuildFailure, RebuildStage,
        RebuildTicket, RebuildTicketId, RebuildTransition,
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

fn requested_ticket(
    ticket_id: RebuildTicketId,
    invalidation: ProjectionInvalidation,
) -> RebuildTicket {
    RebuildTicket::requested(
        ticket_id,
        invalidation,
        SourceGeneration::new([9; 32]).expect("source generation"),
        Some(position(9, 11)),
        RawSourceDigest::new([8; 32]),
    )
    .expect("requested ticket")
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
    let requested = requested_ticket(ticket_id, invalidation);
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
            RebuildFailure::IntegrityFailure,
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

#[test]
fn projection_models_cover_all_accessors_and_validation_bounds() {
    let id = projection_id();
    let generation_one = generation(1);
    assert_eq!(id.as_str(), "food_availability.v1");
    assert_eq!(generation_one.as_bytes(), &[1; 32]);
    for invalid in ["", "Uppercase", " leading", "trailing ", "bad/slash"] {
        assert_eq!(
            ProjectionId::parse(invalid),
            Err(Error::InvalidProjectionId)
        );
        assert_eq!(
            EventIndexShardId::parse(invalid),
            Err(Error::InvalidEventIndexShardId)
        );
    }
    assert_eq!(
        ProjectionId::parse("x".repeat(radroots_storage::projection::PROJECTION_ID_MAX_BYTES + 1)),
        Err(Error::InvalidProjectionId)
    );
    assert_eq!(
        ProjectionRevision::new(0),
        Err(Error::InvalidProjectionRevision)
    );
    assert_eq!(ProjectionRevision::new(2).unwrap().get(), 2);

    let checkpoint = checkpoint(generation_one, 10, 4, 100);
    assert_eq!(checkpoint.projection_id(), &id);
    assert_eq!(checkpoint.generation(), generation_one);
    assert_eq!(checkpoint.source_position(), Some(position(9, 10)));
    assert_eq!(checkpoint.projected_rows(), 4);
    assert_eq!(checkpoint.updated_at_unix_ms(), 100);
    let empty = ProjectionCheckpoint::new(id.clone(), generation_one, None, 0, 100).unwrap();
    assert!(empty.advances(&empty));
    assert!(checkpoint.advances(&empty));
    assert!(!empty.advances(&checkpoint));
    assert!(
        !ProjectionCheckpoint::new(id.clone(), generation(2), None, 0, 101)
            .unwrap()
            .advances(&empty)
    );
    assert!(
        !ProjectionCheckpoint::new(
            ProjectionId::parse("other").unwrap(),
            generation_one,
            None,
            0,
            101,
        )
        .unwrap()
        .advances(&empty)
    );
    assert!(
        !ProjectionCheckpoint::new(id.clone(), generation_one, None, 0, 99)
            .unwrap()
            .advances(&empty)
    );

    assert_eq!(
        ProjectionInvalidation::new(
            id.clone(),
            generation_one,
            generation(2),
            InvalidationReason::OperatorRequested,
            0,
        ),
        Err(Error::InvalidProjectionInvalidation)
    );
    let invalidation = ProjectionInvalidation::new(
        id.clone(),
        generation_one,
        generation(2),
        InvalidationReason::IntegrityFailure,
        100,
    )
    .unwrap();
    assert_eq!(invalidation.projection_id(), &id);
    assert_eq!(invalidation.invalid_generation(), generation_one);
    assert_eq!(invalidation.replacement_generation(), generation(2));
    assert_eq!(invalidation.reason(), InvalidationReason::IntegrityFailure);
    assert_eq!(invalidation.invalidated_at_unix_ms(), 100);
    let ticket_id = RebuildTicketId::new([3; 16]).unwrap();
    assert_eq!(ticket_id.as_bytes(), &[3; 16]);
    let ticket = requested_ticket(ticket_id, invalidation);
    assert_eq!(ticket.ticket_id(), ticket_id);
    assert_eq!(ticket.revision(), ProjectionRevision::INITIAL);
    assert_eq!(ticket.stage(), RebuildStage::Requested);
    assert!(ticket.checkpoint().is_none());
    assert_eq!(ticket.requested_at_unix_ms(), 100);
    assert_eq!(ticket.updated_at_unix_ms(), 100);
    assert_eq!(
        RebuildTransition::start(ticket_id, ticket.revision(), 101).ticket_id(),
        ticket_id
    );
}

#[test]
fn durable_rebuild_matrix_rejects_every_inconsistent_shape() {
    let invalidation = ProjectionInvalidation::new(
        projection_id(),
        generation(1),
        generation(2),
        InvalidationReason::ProjectionGenerationChanged,
        100,
    )
    .unwrap();
    let ticket_id = RebuildTicketId::new([3; 16]).unwrap();
    let replacement = checkpoint(generation(2), 1, 1, 105);
    let durable = |revision, stage, checkpoint, requested, updated| {
        RebuildTicket::from_durable_parts(
            ticket_id,
            invalidation.clone(),
            revision,
            stage,
            SourceGeneration::new([9; 32]).unwrap(),
            Some(position(9, 11)),
            RawSourceDigest::new([8; 32]),
            checkpoint,
            None,
            requested,
            updated,
        )
    };
    assert!(
        durable(
            ProjectionRevision::INITIAL,
            RebuildStage::Requested,
            None,
            100,
            100
        )
        .is_ok()
    );
    assert!(
        durable(
            ProjectionRevision::new(2).unwrap(),
            RebuildStage::Running,
            Some(replacement.clone()),
            100,
            105
        )
        .is_ok()
    );
    assert!(
        durable(
            ProjectionRevision::new(2).unwrap(),
            RebuildStage::Completed,
            Some(replacement.clone()),
            100,
            105
        )
        .is_ok()
    );
    for result in [
        durable(
            ProjectionRevision::INITIAL,
            RebuildStage::Requested,
            None,
            99,
            100,
        ),
        durable(
            ProjectionRevision::INITIAL,
            RebuildStage::Requested,
            None,
            100,
            99,
        ),
        durable(
            ProjectionRevision::new(2).unwrap(),
            RebuildStage::Requested,
            None,
            100,
            100,
        ),
        durable(
            ProjectionRevision::INITIAL,
            RebuildStage::Requested,
            None,
            100,
            101,
        ),
        durable(
            ProjectionRevision::INITIAL,
            RebuildStage::Running,
            None,
            100,
            101,
        ),
        durable(
            ProjectionRevision::INITIAL,
            RebuildStage::Requested,
            Some(replacement.clone()),
            100,
            100,
        ),
        durable(
            ProjectionRevision::new(2).unwrap(),
            RebuildStage::Completed,
            None,
            100,
            105,
        ),
        durable(
            ProjectionRevision::new(2).unwrap(),
            RebuildStage::Running,
            Some(checkpoint(generation(1), 1, 1, 105)),
            100,
            105,
        ),
        durable(
            ProjectionRevision::new(2).unwrap(),
            RebuildStage::Running,
            Some(checkpoint(generation(2), 1, 1, 106)),
            100,
            105,
        ),
    ] {
        assert_eq!(result, Err(Error::CorruptProjectionRecord));
    }

    let requested = requested_ticket(ticket_id, invalidation.clone());
    assert_eq!(
        requested.transition(RebuildTransition::start(
            ticket_id,
            ProjectionRevision::new(2).unwrap(),
            101
        )),
        Err(Error::ProjectionRevisionConflict)
    );
    assert_eq!(
        requested.transition(RebuildTransition::start(
            RebuildTicketId::new([4; 16]).unwrap(),
            requested.revision(),
            101,
        )),
        Err(Error::ProjectionRevisionConflict)
    );
    assert_eq!(
        requested.transition(RebuildTransition::start(
            ticket_id,
            requested.revision(),
            99
        )),
        Err(Error::InvalidProjectionTimestamp)
    );
    assert_eq!(
        requested.transition(RebuildTransition::checkpoint(
            ticket_id,
            requested.revision(),
            101,
            replacement.clone()
        )),
        Err(Error::InvalidRebuildTransition)
    );
    let running = requested
        .transition(RebuildTransition::start(
            ticket_id,
            requested.revision(),
            101,
        ))
        .unwrap();
    assert_eq!(
        running.transition(RebuildTransition::checkpoint(
            ticket_id,
            running.revision(),
            102,
            checkpoint(generation(1), 1, 1, 102),
        )),
        Err(Error::ProjectionCheckpointMismatch)
    );
    let progressed = running
        .transition(RebuildTransition::checkpoint(
            ticket_id,
            running.revision(),
            103,
            checkpoint(generation(2), 2, 2, 103),
        ))
        .unwrap();
    assert_eq!(
        progressed.transition(RebuildTransition::complete(
            ticket_id,
            progressed.revision(),
            104,
            checkpoint(generation(2), 1, 2, 104),
        )),
        Err(Error::ProjectionCheckpointRegression)
    );
    let failed = running
        .transition(RebuildTransition::fail(
            ticket_id,
            running.revision(),
            102,
            RebuildFailure::ReducerRejected,
        ))
        .unwrap();
    assert_eq!(failed.stage(), RebuildStage::Failed);
    assert_eq!(
        failed.transition(RebuildTransition::fail(
            ticket_id,
            failed.revision(),
            103,
            RebuildFailure::ReducerRejected,
        )),
        Err(Error::RebuildTicketTerminal)
    );
}

#[test]
fn event_index_models_cover_manifest_and_checkpoint_edges() {
    let first_id = event_id('1');
    let last_id = event_id('2');
    assert_eq!(
        EventIdRange::new(last_id, first_id),
        Err(Error::InvalidEventIndexRange)
    );
    let range = EventIdRange::new(first_id, last_id).unwrap();
    assert_eq!(range.first(), &first_id);
    assert_eq!(range.last(), &last_id);
    let digest = ArtifactDigest::new([5; 32]);
    assert_eq!(digest.as_bytes(), &[5; 32]);
    let shard_id = EventIndexShardId::parse("a").unwrap();
    assert_eq!(shard_id.as_str(), "a");
    for path in ["", "/absolute", "../escape", "a/../b", "a//b", "a\\b", " a"] {
        assert_eq!(
            EventIndexShard::new(shard_id.clone(), path, 1, range.clone(), 1, 2, digest),
            Err(Error::InvalidEventIndexArtifactPath)
        );
    }
    assert_eq!(
        EventIndexShard::new(
            shard_id.clone(),
            "x".repeat(radroots_storage::projection::EVENT_INDEX_ARTIFACT_PATH_MAX_BYTES + 1),
            1,
            range.clone(),
            1,
            2,
            digest,
        ),
        Err(Error::InvalidEventIndexArtifactPath)
    );
    assert_eq!(
        EventIndexShard::new(shard_id.clone(), "a.json", 0, range.clone(), 1, 2, digest),
        Err(Error::InvalidEventIndexShardCount)
    );
    assert_eq!(
        EventIndexShard::new(shard_id.clone(), "a.json", 1, range.clone(), 0, 2, digest),
        Err(Error::InvalidEventIndexTimestamp)
    );
    assert_eq!(
        EventIndexShard::new(shard_id.clone(), "a.json", 1, range.clone(), 2, 1, digest),
        Err(Error::InvalidEventIndexTimestamp)
    );
    let shard = EventIndexShard::new(shard_id.clone(), "a.json", 1, range, 1, 2, digest).unwrap();
    assert_eq!(shard.shard_id(), &shard_id);
    assert_eq!(shard.artifact_path(), "a.json");
    assert_eq!(shard.event_count(), 1);
    assert_eq!(shard.first_published_at_unix_s(), 1);
    assert_eq!(shard.last_published_at_unix_s(), 2);
    assert_eq!(shard.sha256(), digest);
    assert_eq!(
        EventIndexManifest::new(generation(1), 1, 1, 1, 2, vec![]),
        Err(Error::InvalidEventIndexShardCount)
    );
    assert_eq!(
        EventIndexManifest::new(generation(1), 1, 0, 1, 2, vec![shard.clone()]),
        Err(Error::InvalidEventIndexManifest)
    );
    assert_eq!(
        EventIndexManifest::new(generation(1), 0, 1, 1, 2, vec![shard.clone()]),
        Err(Error::InvalidEventIndexManifest)
    );
    assert_eq!(
        EventIndexManifest::new(generation(1), 1, 1, 0, 2, vec![shard.clone()]),
        Err(Error::InvalidEventIndexManifest)
    );
    assert_eq!(
        EventIndexManifest::new(generation(1), 1, 1, 1, 3, vec![shard.clone()]),
        Err(Error::InvalidEventIndexManifest)
    );
    assert_eq!(
        EventIndexManifest::new(
            generation(1),
            1,
            1,
            1,
            2,
            vec![shard.clone(); radroots_storage::projection::EVENT_INDEX_SHARDS_MAX + 1],
        ),
        Err(Error::InvalidEventIndexShardCount)
    );
    let manifest = EventIndexManifest::new(generation(1), 1, 1, 1, 2, vec![shard.clone()]).unwrap();
    assert_eq!(manifest.generation(), generation(1));
    assert_eq!(manifest.target_shard_size(), 1);
    assert_eq!(manifest.first_published_at_unix_s(), 1);
    assert_eq!(manifest.last_published_at_unix_s(), 2);

    for cursor in [
        Some(String::new()),
        Some(" leading".to_owned()),
        Some("bad\nvalue".to_owned()),
    ] {
        assert_eq!(
            EventIndexShardCheckpoint::new(shard_id.clone(), 1, None, cursor),
            Err(Error::InvalidEventIndexCursor)
        );
    }
    assert_eq!(
        EventIndexShardCheckpoint::new(shard_id.clone(), 0, None, None),
        Err(Error::InvalidEventIndexTimestamp)
    );
    let shard_checkpoint = EventIndexShardCheckpoint::new(
        shard_id.clone(),
        2,
        Some(last_id),
        Some("cursor".to_owned()),
    )
    .unwrap();
    assert_eq!(shard_checkpoint.shard_id(), &shard_id);
    assert_eq!(shard_checkpoint.last_created_at_unix_s(), 2);
    assert_eq!(shard_checkpoint.last_event_id(), Some(&last_id));
    assert_eq!(shard_checkpoint.cursor(), Some("cursor"));
    assert_eq!(
        EventIndexCheckpoint::new(generation(1), 0, vec![]),
        Err(Error::InvalidEventIndexCheckpoint)
    );
    assert_eq!(
        EventIndexCheckpoint::new(
            generation(1),
            1,
            vec![
                shard_checkpoint.clone();
                radroots_storage::projection::EVENT_INDEX_SHARDS_MAX + 1
            ],
        ),
        Err(Error::InvalidEventIndexCheckpoint)
    );
    let index = EventIndexCheckpoint::new(generation(1), 3, vec![shard_checkpoint]).unwrap();
    assert_eq!(index.generation(), generation(1));
    assert_eq!(index.generated_at_unix_ms(), 3);
    assert_eq!(index.shards().len(), 1);
    assert!(index.shard(&shard_id).is_some());
    assert!(
        index
            .shard(&EventIndexShardId::parse("missing").unwrap())
            .is_none()
    );

    let status = ProjectionStatus::new(
        projection_id(),
        generation(1),
        ProjectionHealth::Ready,
        None,
        None,
    )
    .unwrap();
    assert_eq!(status.projection_id(), &projection_id());
    assert_eq!(status.generation(), generation(1));
    assert!(status.checkpoint().is_none());
    assert!(status.active_rebuild().is_none());
    assert_eq!(
        ProjectionStatus::new(
            projection_id(),
            generation(1),
            ProjectionHealth::Rebuilding,
            None,
            None
        ),
        Err(Error::CorruptProjectionRecord)
    );
    assert_eq!(
        ProjectionStatus::new(
            projection_id(),
            generation(1),
            ProjectionHealth::Ready,
            None,
            Some(RebuildTicketId::new([1; 16]).unwrap())
        ),
        Err(Error::CorruptProjectionRecord)
    );
    assert_eq!(
        ProjectionStatus::new(
            projection_id(),
            generation(1),
            ProjectionHealth::Ready,
            Some(checkpoint(generation(2), 1, 1, 2)),
            None
        ),
        Err(Error::CorruptProjectionRecord)
    );
    assert_eq!(
        ProjectionStatus::new(
            projection_id(),
            generation(1),
            ProjectionHealth::Ready,
            Some(
                ProjectionCheckpoint::new(
                    ProjectionId::parse("other").unwrap(),
                    generation(1),
                    None,
                    1,
                    2,
                )
                .unwrap(),
            ),
            None,
        ),
        Err(Error::CorruptProjectionRecord)
    );
}

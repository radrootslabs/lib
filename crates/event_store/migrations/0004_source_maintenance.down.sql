DROP TRIGGER radroots_event_store_source_capacity_marker_close_guard;
DROP TRIGGER radroots_event_store_source_generation_capacity_advance;
DROP TRIGGER radroots_event_store_source_generation_capacity_guard;
DROP TRIGGER radroots_event_store_source_capacity_delete_guard;
DROP TRIGGER radroots_event_store_source_capacity_update_guard;
DROP TRIGGER radroots_event_store_source_capacity_insert_guard;
DROP TABLE radroots_event_store_source_capacity_v1;

DROP TRIGGER radroots_event_store_food_availability_image_delete_guard;

CREATE TRIGGER radroots_event_store_food_availability_image_delete_guard
BEFORE DELETE ON radroots_event_store_food_availability_image
WHEN NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_food_availability_cursor AS cursor
  JOIN radroots_event_store_source_state AS source ON source.singleton = 1
  WHERE cursor.singleton = 1
    AND cursor.source_generation != source.active_generation
)
AND NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_food_availability_cursor AS cursor
  JOIN radroots_event_store_addressable_head_transition AS transition
    ON transition.source_generation = cursor.source_generation
   AND transition.transition_seq > cursor.last_transition_seq
   AND transition.kind = 30402
   AND transition.pubkey = OLD.pubkey
   AND transition.d_tag = OLD.d_tag
   AND transition.retracted_event_id IS NOT NULL
  WHERE cursor.singleton = 1
)
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability image delete is not backed by a pending retraction');
END;

DROP TRIGGER radroots_event_store_food_availability_projection_delete_guard;

CREATE TRIGGER radroots_event_store_food_availability_projection_delete_guard
BEFORE DELETE ON radroots_event_store_food_availability_projection
WHEN NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_food_availability_cursor AS cursor
  JOIN radroots_event_store_source_state AS source ON source.singleton = 1
  WHERE cursor.singleton = 1
    AND cursor.source_generation != source.active_generation
)
AND NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_food_availability_cursor AS cursor
  JOIN radroots_event_store_addressable_head_transition AS transition
    ON transition.source_generation = cursor.source_generation
   AND transition.transition_seq > cursor.last_transition_seq
   AND transition.kind = OLD.kind
   AND transition.pubkey = OLD.pubkey
   AND transition.d_tag = OLD.d_tag
   AND transition.retracted_event_id = OLD.event_id
  WHERE cursor.singleton = 1
)
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability projection delete is not backed by a pending retraction');
END;

DROP TRIGGER radroots_event_store_source_rebuild_marker_insert_guard;

CREATE TRIGGER radroots_event_store_source_rebuild_marker_insert_guard
BEFORE INSERT ON radroots_event_store_source_rebuild_marker
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker
)
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_source_generation
  WHERE source_generation = NEW.target_generation
)
OR NEW.target_generation_ordinal != (
  SELECT COALESCE(MAX(generation_ordinal), 0) + 1
  FROM radroots_event_store_source_generation
)
OR NEW.transition_floor_seq != (
  SELECT COALESCE(MAX(transition_seq), 0)
  FROM radroots_event_store_addressable_head_transition
)
OR NEW.baseline_raw_event_count != (
  SELECT COUNT(*)
  FROM event_envelopes
)
OR NEW.baseline_raw_tag_count != (
  SELECT COUNT(*)
  FROM event_envelope_tags
)
OR NEW.baseline_raw_high_water_seq != (
  SELECT COALESCE(MAX(seq), 0)
  FROM event_envelopes
)
OR NOT (
  (
    NOT EXISTS (
      SELECT 1
      FROM radroots_event_store_source_state
    )
    AND NOT EXISTS (
      SELECT 1
      FROM radroots_event_store_source_generation
    )
    AND NEW.target_generation_ordinal = 1
    AND NEW.transition_floor_seq = 0
    AND NEW.prior_active_generation IS NULL
    AND NEW.prior_raw_event_count IS NULL
    AND NEW.prior_raw_tag_count IS NULL
    AND NEW.prior_raw_high_water_seq IS NULL
    AND NEW.prior_last_transition_seq IS NULL
  )
  OR EXISTS (
    SELECT 1
    FROM radroots_event_store_source_state AS state
    JOIN radroots_event_store_source_generation AS generation
      ON generation.source_generation = state.active_generation
    WHERE state.singleton = 1
      AND generation.generation_ordinal = (
        SELECT MAX(candidate.generation_ordinal)
        FROM radroots_event_store_source_generation AS candidate
      )
      AND NEW.target_generation_ordinal = generation.generation_ordinal + 1
      AND NEW.prior_active_generation = state.active_generation
      AND NEW.prior_raw_event_count = state.raw_event_count
      AND NEW.prior_raw_tag_count = state.raw_tag_count
      AND NEW.prior_raw_high_water_seq = state.raw_high_water_seq
      AND NEW.prior_last_transition_seq = state.last_transition_seq
      AND NEW.transition_floor_seq = state.last_transition_seq
      AND NEW.baseline_raw_event_count = state.raw_event_count
      AND NEW.baseline_raw_tag_count = state.raw_tag_count
      AND NEW.baseline_raw_high_water_seq = state.raw_high_water_seq
  )
)
BEGIN
  SELECT RAISE(ABORT, 'event-store rebuild marker does not bind exact raw and prior source authority');
END;

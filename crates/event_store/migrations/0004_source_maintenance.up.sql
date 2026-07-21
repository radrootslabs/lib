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
      AND NEW.baseline_raw_event_count = state.raw_event_count
      AND NEW.baseline_raw_tag_count = state.raw_tag_count
      AND NEW.baseline_raw_high_water_seq = state.raw_high_water_seq
  )
)
BEGIN
  SELECT RAISE(ABORT, 'event-store rebuild marker does not bind exact raw and prior source authority');
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
AND NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker AS marker
  JOIN radroots_event_store_source_state AS source
    ON source.singleton = marker.singleton
   AND source.active_generation = marker.target_generation
  WHERE marker.singleton = 1
    AND OLD.source_generation != source.active_generation
)
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability projection delete is not backed by a pending retraction or active source rebuild');
END;

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
AND NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker AS marker
  JOIN radroots_event_store_source_state AS source
    ON source.singleton = marker.singleton
   AND source.active_generation = marker.target_generation
  WHERE marker.singleton = 1
    AND OLD.source_generation != source.active_generation
)
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability image delete is not backed by a pending retraction or active source rebuild');
END;

CREATE TABLE radroots_event_store_source_capacity_v1 (
  singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
  source_generation BLOB NOT NULL UNIQUE CHECK (
    length(source_generation) = 32
  ) REFERENCES radroots_event_store_source_generation(source_generation)
    ON DELETE RESTRICT,
  raw_event_count INTEGER NOT NULL CHECK (
    raw_event_count >= 0 AND raw_event_count <= 25000
  ),
  raw_tag_count INTEGER NOT NULL CHECK (
    raw_tag_count >= 0 AND raw_tag_count <= 250000
  ),
  raw_event_bytes INTEGER NOT NULL CHECK (
    raw_event_bytes >= 0 AND raw_event_bytes <= 67108864
  ),
  raw_tag_bytes INTEGER NOT NULL CHECK (
    raw_tag_bytes >= 0 AND raw_tag_bytes <= 33554432
  ),
  raw_high_water_seq INTEGER NOT NULL CHECK (raw_high_water_seq >= 0),
  retained_generation_count INTEGER NOT NULL CHECK (
    retained_generation_count >= 1 AND retained_generation_count <= 8
  ),
  retained_generation_limit INTEGER NOT NULL CHECK (
    retained_generation_limit = 8
  )
) STRICT, WITHOUT ROWID;

CREATE TRIGGER radroots_event_store_source_capacity_insert_guard
BEFORE INSERT ON radroots_event_store_source_capacity_v1
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_capacity_v1
)
OR NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state AS state
  WHERE state.singleton = 1
    AND NEW.singleton = state.singleton
    AND NEW.source_generation = state.active_generation
    AND NEW.raw_event_count = state.raw_event_count
    AND NEW.raw_tag_count = state.raw_tag_count
    AND NEW.raw_high_water_seq = state.raw_high_water_seq
    AND NEW.raw_event_count = (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM event_envelopes
        LIMIT 25001
      )
    )
    AND NEW.raw_tag_count = (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM event_envelope_tags
        LIMIT 250001
      )
    )
    AND NEW.raw_event_bytes = (
      SELECT COALESCE(SUM(raw_bytes), 0)
      FROM (
        SELECT
          length(CAST(event_id AS BLOB))
          + length(CAST(pubkey AS BLOB))
          + length(CAST(tags_json AS BLOB))
          + length(CAST(content AS BLOB))
          + length(CAST(sig AS BLOB))
          + length(CAST(raw_json AS BLOB)) AS raw_bytes
        FROM event_envelopes
        LIMIT 25001
      )
    )
    AND NEW.raw_tag_bytes = (
      SELECT COALESCE(SUM(raw_bytes), 0)
      FROM (
        SELECT
          length(CAST(event_id AS BLOB))
          + length(CAST(tag_name AS BLOB))
          + COALESCE(length(CAST(tag_value AS BLOB)), 0)
          + length(CAST(tag_json AS BLOB)) AS raw_bytes
        FROM event_envelope_tags
        LIMIT 250001
      )
    )
    AND NEW.retained_generation_count = (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM radroots_event_store_source_generation
        LIMIT 9
      )
    )
    AND NEW.retained_generation_limit = 8
)
BEGIN
  SELECT RAISE(ABORT, 'event-store source capacity initialization must seal exact raw and generation authority');
END;

CREATE TRIGGER radroots_event_store_source_capacity_update_guard
BEFORE UPDATE ON radroots_event_store_source_capacity_v1
WHEN NOT (
  (
    NOT EXISTS (
      SELECT 1
      FROM radroots_event_store_source_rebuild_marker
      WHERE singleton = 1
    )
    AND NEW.singleton IS OLD.singleton
    AND NEW.source_generation IS OLD.source_generation
    AND NEW.retained_generation_count = OLD.retained_generation_count
    AND NEW.retained_generation_limit = OLD.retained_generation_limit
    AND NEW.raw_event_count = OLD.raw_event_count + 1
    AND NEW.raw_tag_count = OLD.raw_tag_count + (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM event_envelope_tags AS tag
        JOIN event_envelopes AS event ON event.event_id = tag.event_id
        WHERE event.seq > OLD.raw_high_water_seq
        LIMIT 250001
      )
    )
    AND NEW.raw_event_bytes = OLD.raw_event_bytes + (
      SELECT COALESCE(SUM(raw_bytes), 0)
      FROM (
        SELECT
          length(CAST(event_id AS BLOB))
          + length(CAST(pubkey AS BLOB))
          + length(CAST(tags_json AS BLOB))
          + length(CAST(content AS BLOB))
          + length(CAST(sig AS BLOB))
          + length(CAST(raw_json AS BLOB)) AS raw_bytes
        FROM event_envelopes
        WHERE seq > OLD.raw_high_water_seq
        LIMIT 2
      )
    )
    AND NEW.raw_tag_bytes = OLD.raw_tag_bytes + (
      SELECT COALESCE(SUM(raw_bytes), 0)
      FROM (
        SELECT
          length(CAST(tag.event_id AS BLOB))
          + length(CAST(tag.tag_name AS BLOB))
          + COALESCE(length(CAST(tag.tag_value AS BLOB)), 0)
          + length(CAST(tag.tag_json AS BLOB)) AS raw_bytes
        FROM event_envelope_tags AS tag
        JOIN event_envelopes AS event ON event.event_id = tag.event_id
        WHERE event.seq > OLD.raw_high_water_seq
        LIMIT 250001
      )
    )
    AND NEW.raw_high_water_seq = (
      SELECT COALESCE(MAX(seq), 0)
      FROM event_envelopes
    )
    AND 1 = (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM event_envelopes
        WHERE seq > OLD.raw_high_water_seq
        LIMIT 2
      )
    )
    AND EXISTS (
      SELECT 1
      FROM radroots_event_store_source_state AS state
      WHERE state.singleton = 1
        AND state.active_generation = NEW.source_generation
        AND state.raw_event_count = NEW.raw_event_count
        AND state.raw_tag_count = NEW.raw_tag_count
        AND state.raw_high_water_seq = NEW.raw_high_water_seq
    )
  )
  OR (
    NEW.singleton IS OLD.singleton
    AND NEW.source_generation IS OLD.source_generation
    AND NEW.raw_event_count = OLD.raw_event_count
    AND NEW.raw_tag_count = OLD.raw_tag_count
    AND NEW.raw_event_bytes = OLD.raw_event_bytes
    AND NEW.raw_tag_bytes = OLD.raw_tag_bytes
    AND NEW.raw_high_water_seq = OLD.raw_high_water_seq
    AND NEW.retained_generation_count = OLD.retained_generation_count + 1
    AND NEW.retained_generation_limit = OLD.retained_generation_limit
    AND NEW.retained_generation_count = (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM radroots_event_store_source_generation
        LIMIT 9
      )
    )
    AND EXISTS (
      SELECT 1
      FROM radroots_event_store_source_rebuild_marker AS marker
      JOIN radroots_event_store_source_generation AS generation
        ON generation.source_generation = marker.target_generation
      WHERE marker.singleton = 1
        AND generation.generation_ordinal = NEW.retained_generation_count
        AND generation.generation_ordinal = (
          SELECT MAX(candidate.generation_ordinal)
          FROM radroots_event_store_source_generation AS candidate
        )
    )
  )
  OR (
    NEW.singleton IS OLD.singleton
    AND NEW.source_generation IS NOT OLD.source_generation
    AND NEW.raw_event_count = OLD.raw_event_count
    AND NEW.raw_tag_count = OLD.raw_tag_count
    AND NEW.raw_event_bytes = OLD.raw_event_bytes
    AND NEW.raw_tag_bytes = OLD.raw_tag_bytes
    AND NEW.raw_high_water_seq = OLD.raw_high_water_seq
    AND NEW.retained_generation_count = OLD.retained_generation_count
    AND NEW.retained_generation_limit = OLD.retained_generation_limit
    AND EXISTS (
      SELECT 1
      FROM radroots_event_store_source_rebuild_marker AS marker
      JOIN radroots_event_store_source_state AS state
        ON state.singleton = marker.singleton
       AND state.active_generation = marker.target_generation
      WHERE marker.singleton = 1
        AND NEW.source_generation = marker.target_generation
        AND NEW.source_generation = state.active_generation
        AND NEW.raw_event_count = state.raw_event_count
        AND NEW.raw_tag_count = state.raw_tag_count
        AND NEW.raw_high_water_seq = state.raw_high_water_seq
        AND NEW.raw_event_count = (
          SELECT COUNT(*)
          FROM (
            SELECT 1
            FROM event_envelopes
            LIMIT 25001
          )
        )
        AND NEW.raw_tag_count = (
          SELECT COUNT(*)
          FROM (
            SELECT 1
            FROM event_envelope_tags
            LIMIT 250001
          )
        )
        AND NEW.raw_event_bytes = (
          SELECT COALESCE(SUM(raw_bytes), 0)
          FROM (
            SELECT
              length(CAST(event_id AS BLOB))
              + length(CAST(pubkey AS BLOB))
              + length(CAST(tags_json AS BLOB))
              + length(CAST(content AS BLOB))
              + length(CAST(sig AS BLOB))
              + length(CAST(raw_json AS BLOB)) AS raw_bytes
            FROM event_envelopes
            LIMIT 25001
          )
        )
        AND NEW.raw_tag_bytes = (
          SELECT COALESCE(SUM(raw_bytes), 0)
          FROM (
            SELECT
              length(CAST(event_id AS BLOB))
              + length(CAST(tag_name AS BLOB))
              + COALESCE(length(CAST(tag_value AS BLOB)), 0)
              + length(CAST(tag_json AS BLOB)) AS raw_bytes
            FROM event_envelope_tags
            LIMIT 250001
          )
        )
        AND NEW.retained_generation_count = (
          SELECT COUNT(*)
          FROM (
            SELECT 1
            FROM radroots_event_store_source_generation
            LIMIT 9
          )
        )
    )
  )
)
BEGIN
  SELECT RAISE(ABORT, 'event-store source capacity update is outside its append or rebuild phase');
END;

CREATE TRIGGER radroots_event_store_source_capacity_delete_guard
BEFORE DELETE ON radroots_event_store_source_capacity_v1
BEGIN
  SELECT RAISE(ABORT, 'event-store source capacity authority is immutable');
END;

CREATE TRIGGER radroots_event_store_source_generation_capacity_guard
BEFORE INSERT ON radroots_event_store_source_generation
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_capacity_v1
  WHERE singleton = 1
    AND retained_generation_count >= retained_generation_limit
)
AND NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_generation
  WHERE source_generation = NEW.source_generation
    OR generation_ordinal = NEW.generation_ordinal
)
BEGIN
  SELECT RAISE(ABORT, 'event-store retained source generation limit reached; replace and resync into a fresh store');
END;

CREATE TRIGGER radroots_event_store_source_generation_capacity_advance
AFTER INSERT ON radroots_event_store_source_generation
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_capacity_v1
  WHERE singleton = 1
)
BEGIN
  UPDATE radroots_event_store_source_capacity_v1
  SET retained_generation_count = retained_generation_count + 1
  WHERE singleton = 1;
  SELECT CASE
    WHEN changes() != 1
    THEN RAISE(ABORT, 'event-store retained source generation authority did not advance')
  END;
END;

CREATE TRIGGER radroots_event_store_source_capacity_marker_close_guard
BEFORE DELETE ON radroots_event_store_source_rebuild_marker
WHEN NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_capacity_v1 AS capacity
  JOIN radroots_event_store_source_state AS state
    ON state.singleton = capacity.singleton
   AND state.active_generation = capacity.source_generation
  JOIN radroots_event_store_source_generation AS generation
    ON generation.source_generation = state.active_generation
  JOIN radroots_event_store_addressable_feed_integrity_v1 AS integrity
    ON integrity.source_generation = state.active_generation
  JOIN radroots_event_store_food_availability_cursor AS cursor
    ON cursor.singleton = state.singleton
   AND cursor.source_generation = state.active_generation
  WHERE capacity.singleton = 1
    AND OLD.singleton = capacity.singleton
    AND OLD.target_generation = capacity.source_generation
    AND capacity.raw_event_count = state.raw_event_count
    AND capacity.raw_tag_count = state.raw_tag_count
    AND capacity.raw_high_water_seq = state.raw_high_water_seq
    AND capacity.raw_event_count = (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM event_envelopes
        LIMIT 25001
      )
    )
    AND capacity.raw_tag_count = (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM event_envelope_tags
        LIMIT 250001
      )
    )
    AND capacity.raw_event_bytes = (
      SELECT COALESCE(SUM(raw_bytes), 0)
      FROM (
        SELECT
          length(CAST(event_id AS BLOB))
          + length(CAST(pubkey AS BLOB))
          + length(CAST(tags_json AS BLOB))
          + length(CAST(content AS BLOB))
          + length(CAST(sig AS BLOB))
          + length(CAST(raw_json AS BLOB)) AS raw_bytes
        FROM event_envelopes
        LIMIT 25001
      )
    )
    AND capacity.raw_tag_bytes = (
      SELECT COALESCE(SUM(raw_bytes), 0)
      FROM (
        SELECT
          length(CAST(event_id AS BLOB))
          + length(CAST(tag_name AS BLOB))
          + COALESCE(length(CAST(tag_value AS BLOB)), 0)
          + length(CAST(tag_json AS BLOB)) AS raw_bytes
        FROM event_envelope_tags
        LIMIT 250001
      )
    )
    AND capacity.retained_generation_count = (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM radroots_event_store_source_generation
        LIMIT 9
      )
    )
    AND capacity.retained_generation_limit = 8
    AND generation.generation_ordinal = capacity.retained_generation_count
    AND integrity.transition_floor_seq = generation.transition_floor_seq
    AND integrity.last_transition_seq = state.last_transition_seq
    AND integrity.transition_count =
      state.last_transition_seq - generation.transition_floor_seq
    AND cursor.feed_version = 1
    AND cursor.projection_version = 1
    AND hex(cursor.scope_fingerprint) =
      '8B63C5DDC48A2CC7DB69295238B96D5F814DBA50427C80B4D0079F061E6D3DE0'
    AND cursor.hook_manifest_sha256 =
      '33b93a3c87ce428e8aa6f5e92643c77203d9aa006c53ce96f3562fe6d68ffd23'
    AND cursor.last_transition_seq = state.last_transition_seq
    AND cursor.projected_row_count = (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM radroots_event_store_food_availability_projection
        WHERE source_generation = state.active_generation
        LIMIT 25001
      )
    )
    AND cursor.projected_row_count = (
      SELECT COUNT(*)
      FROM (
        SELECT 1
        FROM radroots_event_store_food_availability_search_fts
        LIMIT 25001
      )
    )
    AND NOT EXISTS (
      SELECT 1
      FROM radroots_event_store_food_availability_projection
      WHERE source_generation != state.active_generation
    )
)
BEGIN
  SELECT RAISE(ABORT, 'event-store rebuild marker cannot close before capacity, NIP-09, and FoodAvailability seals agree');
END;

CREATE VIEW radroots_event_store_current_visibility_v1 AS
WITH active AS (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
), event_facts AS (
  SELECT
    event.seq AS event_seq,
    event.event_id,
    event.pubkey,
    event.created_at,
    event.kind,
    event.contract_status AS admission_status,
    event.contract_id,
    event.event_class,
    active.active_generation AS source_generation,
    coordinate.raw_d_tag,
    coordinate.nip09_matchable,
    coordinate.nip09_d_tag,
    CASE
      WHEN event.event_class = 'regular' THEN 1
      WHEN head.event_id = event.event_id THEN 1
      ELSE 0
    END AS is_raw_head,
    head.event_id AS raw_head_event_id
  FROM event_envelopes AS event
  CROSS JOIN active
  LEFT JOIN radroots_event_store_event_coordinate AS coordinate
    ON coordinate.source_generation = active.active_generation
   AND coordinate.event_id = event.event_id
  LEFT JOIN event_envelope_head AS head
    ON head.coordinate_type = event.event_class
   AND head.kind = event.kind
   AND head.pubkey = event.pubkey
   AND (
     (event.event_class = 'replaceable' AND head.d_tag IS NULL)
     OR (event.event_class = 'addressable' AND head.d_tag = coordinate.raw_d_tag)
   )
), evidence AS (
  SELECT
    fact.*,
    CASE WHEN fact.kind != 5 THEN (
      SELECT request.request_event_id
      FROM radroots_event_store_nip09_event_target AS target
        INDEXED BY radroots_event_store_nip09_event_target_lookup_idx
      CROSS JOIN radroots_event_store_nip09_request AS request
        ON request.source_generation = target.source_generation
       AND request.request_event_id = target.request_event_id
      WHERE target.source_generation = fact.source_generation
        AND target.target_event_id = fact.event_id
        AND request.request_pubkey = fact.pubkey
      ORDER BY target.request_event_id
      LIMIT 1
    ) END AS event_reference_request_id,
    CASE WHEN fact.kind != 5 THEN (
      SELECT request.request_event_id
      FROM radroots_event_store_nip09_address_target AS target
        INDEXED BY radroots_event_store_nip09_address_target_visibility_lookup_idx
      CROSS JOIN radroots_event_store_nip09_request AS request
        ON request.source_generation = target.source_generation
       AND request.request_event_id = target.request_event_id
      WHERE target.source_generation = fact.source_generation
        AND target.target_kind = fact.kind
        AND target.target_pubkey = fact.pubkey
        AND fact.nip09_matchable = 1
        AND target.target_d_tag = fact.nip09_d_tag
        AND request.request_pubkey = fact.pubkey
      ORDER BY target.inclusive_cutoff DESC, target.request_event_id
      LIMIT 1
    ) END AS address_reference_request_id,
    CASE WHEN fact.kind != 5 THEN (
      SELECT target.inclusive_cutoff
      FROM radroots_event_store_nip09_address_target AS target
        INDEXED BY radroots_event_store_nip09_address_target_visibility_lookup_idx
      CROSS JOIN radroots_event_store_nip09_request AS request
        ON request.source_generation = target.source_generation
       AND request.request_event_id = target.request_event_id
      WHERE target.source_generation = fact.source_generation
        AND target.target_kind = fact.kind
        AND target.target_pubkey = fact.pubkey
        AND fact.nip09_matchable = 1
        AND target.target_d_tag = fact.nip09_d_tag
        AND request.request_pubkey = fact.pubkey
      ORDER BY target.inclusive_cutoff DESC, target.request_event_id
      LIMIT 1
    ) END AS address_reference_cutoff,
    CASE WHEN fact.kind = 5 THEN 0 ELSE EXISTS (
      SELECT 1
      FROM radroots_event_store_nip09_event_target AS target
        INDEXED BY radroots_event_store_nip09_event_target_lookup_idx
      CROSS JOIN radroots_event_store_nip09_request AS request
        ON request.source_generation = target.source_generation
       AND request.request_event_id = target.request_event_id
      WHERE target.source_generation = fact.source_generation
        AND target.target_event_id = fact.event_id
        AND request.request_pubkey != fact.pubkey
    ) OR EXISTS (
      SELECT 1
      FROM radroots_event_store_nip09_address_target AS target
        INDEXED BY radroots_event_store_nip09_address_target_visibility_lookup_idx
      CROSS JOIN radroots_event_store_nip09_request AS request
        ON request.source_generation = target.source_generation
       AND request.request_event_id = target.request_event_id
      WHERE target.source_generation = fact.source_generation
        AND target.target_kind = fact.kind
        AND target.target_pubkey = fact.pubkey
        AND fact.nip09_matchable = 1
        AND target.target_d_tag = fact.nip09_d_tag
        AND request.request_pubkey != fact.pubkey
    ) END AS has_unauthorized_reference
  FROM event_facts AS fact
), suppression AS (
  SELECT
    evidence.*,
    CASE
      WHEN admission_status != 'admitted' THEN NULL
      WHEN kind = 5 THEN 'visible'
      WHEN event_reference_request_id IS NOT NULL THEN 'suppressed'
      WHEN address_reference_cutoff >= created_at THEN 'suppressed'
      ELSE 'visible'
    END AS suppression_outcome,
    CASE
      WHEN admission_status != 'admitted' THEN NULL
      WHEN kind = 5 THEN 'deletion_request_immune'
      WHEN event_reference_request_id IS NOT NULL
        AND address_reference_cutoff >= created_at
        THEN 'deletion_event_id_and_address_reference'
      WHEN event_reference_request_id IS NOT NULL
        THEN 'deletion_event_id_reference'
      WHEN address_reference_cutoff >= created_at
        THEN 'deletion_address_reference'
      WHEN address_reference_request_id IS NOT NULL
        THEN 'deletion_address_cutoff_precedes_target'
      WHEN has_unauthorized_reference = 1
        THEN 'deletion_request_author_mismatch'
      ELSE 'deletion_no_authorized_reference'
    END AS suppression_reason
  FROM evidence
)
SELECT
  event_seq,
  event_id,
  pubkey,
  created_at,
  kind,
  admission_status,
  contract_id,
  event_class,
  source_generation,
  raw_d_tag,
  is_raw_head,
  raw_head_event_id,
  suppression_outcome,
  suppression_reason,
  CASE WHEN admission_status = 'admitted'
    THEN event_reference_request_id ELSE NULL END AS event_reference_request_id,
  CASE WHEN admission_status = 'admitted'
    THEN address_reference_request_id ELSE NULL END AS address_reference_request_id,
  CASE WHEN admission_status = 'admitted'
    THEN address_reference_cutoff ELSE NULL END AS address_reference_cutoff,
  CASE
    WHEN admission_status != 'admitted' THEN 'not_admitted'
    WHEN is_raw_head = 0 THEN 'not_current'
    WHEN suppression_outcome = 'suppressed' THEN 'suppressed'
    ELSE 'visible'
  END AS current_visibility
FROM suppression;

CREATE INDEX radroots_event_store_current_visibility_head_lookup_idx
ON event_envelope_head(coordinate_type, kind, pubkey, d_tag);

CREATE INDEX radroots_event_store_nip09_address_target_visibility_lookup_idx
ON radroots_event_store_nip09_address_target(
  source_generation,
  target_kind,
  target_pubkey,
  target_d_tag,
  inclusive_cutoff DESC,
  request_event_id ASC
);

CREATE TABLE radroots_event_store_addressable_feed_integrity_v1 (
  source_generation BLOB PRIMARY KEY NOT NULL
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  transition_floor_seq INTEGER NOT NULL CHECK (transition_floor_seq >= 0),
  last_transition_seq INTEGER NOT NULL CHECK (
    last_transition_seq >= transition_floor_seq
  ),
  transition_count INTEGER NOT NULL CHECK (
    transition_count >= 0
    AND transition_count = last_transition_seq - transition_floor_seq
  )
) STRICT, WITHOUT ROWID;

INSERT INTO radroots_event_store_addressable_feed_integrity_v1(
  source_generation,
  transition_floor_seq,
  last_transition_seq,
  transition_count
)
SELECT
  generation.source_generation,
  generation.transition_floor_seq,
  COALESCE(MAX(transition.transition_seq), generation.transition_floor_seq),
  COUNT(transition.transition_seq)
FROM radroots_event_store_source_generation AS generation
LEFT JOIN radroots_event_store_addressable_head_transition AS transition
  ON transition.source_generation = generation.source_generation
GROUP BY generation.source_generation, generation.transition_floor_seq;

CREATE TRIGGER radroots_event_store_addressable_feed_generation_insert
AFTER INSERT ON radroots_event_store_source_generation
BEGIN
  INSERT INTO radroots_event_store_addressable_feed_integrity_v1(
    source_generation,
    transition_floor_seq,
    last_transition_seq,
    transition_count
  ) VALUES (
    NEW.source_generation,
    NEW.transition_floor_seq,
    NEW.transition_floor_seq,
    0
  );
END;

CREATE TRIGGER radroots_event_store_addressable_feed_transition_insert
AFTER INSERT ON radroots_event_store_addressable_head_transition
BEGIN
  UPDATE radroots_event_store_addressable_feed_integrity_v1
  SET
    last_transition_seq = NEW.transition_seq,
    transition_count = transition_count + 1
  WHERE source_generation = NEW.source_generation
    AND NEW.transition_seq = last_transition_seq + 1;
  SELECT CASE
    WHEN changes() != 1
    THEN RAISE(ABORT, 'event-store addressable feed integrity advancement failed')
  END;
END;

CREATE INDEX radroots_event_store_addressable_transition_coordinate_idx
ON radroots_event_store_addressable_head_transition(
  source_generation,
  kind,
  pubkey,
  d_tag,
  transition_seq
);

CREATE TABLE radroots_event_store_food_availability_projection (
  source_generation BLOB NOT NULL
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  kind INTEGER NOT NULL CHECK (kind = 30402),
  pubkey TEXT NOT NULL CHECK (
    length(pubkey) = 64
    AND pubkey = lower(pubkey)
    AND pubkey NOT GLOB '*[^0-9a-f]*'
  ),
  d_tag TEXT NOT NULL CHECK (length(d_tag) > 0),
  event_id TEXT NOT NULL CHECK (
    length(event_id) = 64
    AND event_id = lower(event_id)
    AND event_id NOT GLOB '*[^0-9a-f]*'
  ),
  event_seq INTEGER NOT NULL CHECK (event_seq > 0),
  created_at INTEGER NOT NULL CHECK (created_at >= 0),
  contract_id TEXT NOT NULL CHECK (contract_id = 'radroots.food.availability.v1'),
  content TEXT NOT NULL,
  title TEXT NOT NULL CHECK (length(title) > 0),
  summary TEXT NOT NULL CHECK (length(summary) > 0),
  published_at INTEGER NOT NULL CHECK (published_at > 0 AND published_at <= created_at),
  location TEXT NOT NULL CHECK (length(location) > 0),
  price_amount TEXT NOT NULL CHECK (length(price_amount) > 0),
  price_currency TEXT NOT NULL CHECK (
    length(price_currency) = 3
    AND price_currency NOT GLOB '*[^A-Z]*'
  ),
  price_unit TEXT NOT NULL CHECK (
    price_unit IN ('g', 'kg', 'lb', 'oz', 'each', 'dozen', 'bunch', 'punnet', 'bag', 'basket')
  ),
  quantity_amount TEXT,
  quantity_unit TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'sold')),
  diagnostic_codes_json TEXT NOT NULL CHECK (
    json_valid(diagnostic_codes_json)
    AND json_type(diagnostic_codes_json) = 'array'
  ),
  source_transition_seq INTEGER NOT NULL CHECK (source_transition_seq > 0),
  CHECK (
    (quantity_amount IS NULL AND quantity_unit IS NULL)
    OR (
      quantity_amount IS NOT NULL
      AND length(quantity_amount) > 0
      AND quantity_unit = price_unit
    )
  ),
  PRIMARY KEY (source_generation, pubkey, d_tag),
  UNIQUE (source_generation, event_id),
  UNIQUE (source_generation, event_seq),
  FOREIGN KEY (event_seq, event_id)
    REFERENCES event_envelopes(seq, event_id) ON DELETE RESTRICT,
  FOREIGN KEY (source_transition_seq)
    REFERENCES radroots_event_store_addressable_head_transition(transition_seq)
      ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_event_store_food_availability_status_idx
ON radroots_event_store_food_availability_projection(
  source_generation,
  status,
  published_at DESC,
  event_id
);

CREATE INDEX radroots_event_store_food_availability_recent_idx
ON radroots_event_store_food_availability_projection(
  source_generation,
  published_at DESC,
  event_id
);

CREATE INDEX radroots_event_store_food_availability_author_idx
ON radroots_event_store_food_availability_projection(
  source_generation,
  pubkey,
  published_at DESC,
  event_id
);

CREATE TABLE radroots_event_store_food_availability_image (
  source_generation BLOB NOT NULL,
  pubkey TEXT NOT NULL,
  d_tag TEXT NOT NULL,
  image_index INTEGER NOT NULL CHECK (image_index >= 0 AND image_index < 64),
  raw_tag_json TEXT NOT NULL CHECK (
    json_valid(raw_tag_json)
    AND json_type(raw_tag_json) = 'array'
  ),
  url TEXT,
  width INTEGER CHECK (width IS NULL OR width > 0),
  height INTEGER CHECK (height IS NULL OR height > 0),
  blossom_sha256 TEXT CHECK (
    blossom_sha256 IS NULL
    OR (
      length(blossom_sha256) = 64
      AND blossom_sha256 = lower(blossom_sha256)
      AND blossom_sha256 NOT GLOB '*[^0-9a-f]*'
    )
  ),
  qualifies INTEGER NOT NULL CHECK (qualifies IN (0, 1)),
  diagnostic_codes_json TEXT NOT NULL CHECK (
    json_valid(diagnostic_codes_json)
    AND json_type(diagnostic_codes_json) = 'array'
  ),
  PRIMARY KEY (source_generation, pubkey, d_tag, image_index),
  FOREIGN KEY (source_generation, pubkey, d_tag)
    REFERENCES radroots_event_store_food_availability_projection(
      source_generation,
      pubkey,
      d_tag
    ) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

CREATE VIEW radroots_event_store_food_availability_read_v1 AS
SELECT
  projection.*,
  event.raw_json AS immutable_raw_json,
  (
    SELECT json_group_array(json_object(
      'image_index', ordered_image.image_index,
      'raw_tag_json', ordered_image.raw_tag_json,
      'url', ordered_image.url,
      'width', ordered_image.width,
      'height', ordered_image.height,
      'blossom_sha256', ordered_image.blossom_sha256,
      'qualifies', ordered_image.qualifies,
      'diagnostic_codes_json', ordered_image.diagnostic_codes_json
    ))
    FROM (
      SELECT
        image.image_index,
        image.raw_tag_json,
        image.url,
        image.width,
        image.height,
        image.blossom_sha256,
        image.qualifies,
        image.diagnostic_codes_json
      FROM radroots_event_store_food_availability_image AS image
      WHERE image.source_generation = projection.source_generation
        AND image.pubkey = projection.pubkey
        AND image.d_tag = projection.d_tag
      ORDER BY image.image_index
    ) AS ordered_image
  ) AS stored_images_json
FROM radroots_event_store_food_availability_projection AS projection
JOIN event_envelopes AS event
  ON event.seq = projection.event_seq
 AND event.event_id = projection.event_id;

CREATE VIRTUAL TABLE radroots_event_store_food_availability_search_fts USING fts5(
  event_id UNINDEXED,
  pubkey UNINDEXED,
  d_tag UNINDEXED,
  title,
  summary,
  content,
  location,
  tokenize = 'unicode61'
);

CREATE TABLE radroots_event_store_food_availability_cursor (
  singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
  source_generation BLOB NOT NULL
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  feed_version INTEGER NOT NULL CHECK (feed_version = 1),
  projection_version INTEGER NOT NULL CHECK (projection_version = 1),
  scope_fingerprint BLOB NOT NULL CHECK (
    length(scope_fingerprint) = 32
    AND hex(scope_fingerprint) = '8B63C5DDC48A2CC7DB69295238B96D5F814DBA50427C80B4D0079F061E6D3DE0'
  ),
  hook_manifest_sha256 TEXT NOT NULL CHECK (
    length(hook_manifest_sha256) = 64
    AND hook_manifest_sha256 NOT GLOB '*[^0-9a-f]*'
  ),
  last_transition_seq INTEGER NOT NULL CHECK (last_transition_seq >= 0),
  projected_row_count INTEGER NOT NULL CHECK (projected_row_count >= 0)
) STRICT;

CREATE TRIGGER radroots_event_store_food_availability_projection_insert_guard
BEFORE INSERT ON radroots_event_store_food_availability_projection
WHEN NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_food_availability_cursor AS cursor
  JOIN radroots_event_store_source_state AS source
    ON source.active_generation = cursor.source_generation
  JOIN radroots_event_store_addressable_head_transition AS transition
    ON transition.source_generation = source.active_generation
   AND transition.transition_seq = NEW.source_transition_seq
   AND transition.transition_seq > cursor.last_transition_seq
   AND transition.kind = NEW.kind
   AND transition.pubkey = NEW.pubkey
   AND transition.d_tag = NEW.d_tag
   AND transition.visible_event_id = NEW.event_id
   AND transition.visible_event_seq = NEW.event_seq
   AND transition.contract_id = NEW.contract_id
   AND transition.visibility = 'visible'
  WHERE source.singleton = 1
    AND source.active_generation = NEW.source_generation
)
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability projection insert is not backed by a visible transition');
END;

CREATE TRIGGER radroots_event_store_food_availability_projection_update_guard
BEFORE UPDATE ON radroots_event_store_food_availability_projection
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability projection rows are replaced by delete and insert');
END;

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

CREATE TRIGGER radroots_event_store_food_availability_image_insert_guard
BEFORE INSERT ON radroots_event_store_food_availability_image
WHEN NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_food_availability_projection AS projection
  JOIN radroots_event_store_food_availability_cursor AS cursor
    ON cursor.source_generation = projection.source_generation
  WHERE projection.source_generation = NEW.source_generation
    AND projection.pubkey = NEW.pubkey
    AND projection.d_tag = NEW.d_tag
    AND projection.source_transition_seq > cursor.last_transition_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability image insert is not backed by a pending projection');
END;

CREATE TRIGGER radroots_event_store_food_availability_image_update_guard
BEFORE UPDATE ON radroots_event_store_food_availability_image
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability image rows are immutable');
END;

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

CREATE TRIGGER radroots_event_store_food_availability_search_insert
AFTER INSERT ON radroots_event_store_food_availability_projection
BEGIN
  INSERT INTO radroots_event_store_food_availability_search_fts(
    rowid,
    event_id,
    pubkey,
    d_tag,
    title,
    summary,
    content,
    location
  ) VALUES (
    NEW.event_seq,
    NEW.event_id,
    NEW.pubkey,
    NEW.d_tag,
    NEW.title,
    NEW.summary,
    NEW.content,
    NEW.location
  );
END;

CREATE TRIGGER radroots_event_store_food_availability_search_delete
AFTER DELETE ON radroots_event_store_food_availability_projection
BEGIN
  DELETE FROM radroots_event_store_food_availability_search_fts
  WHERE rowid = OLD.event_seq;
END;

CREATE TRIGGER radroots_event_store_food_availability_cursor_insert_guard
BEFORE INSERT ON radroots_event_store_food_availability_cursor
WHEN EXISTS (
  SELECT 1 FROM radroots_event_store_food_availability_cursor
)
OR EXISTS (
  SELECT 1 FROM radroots_event_store_food_availability_projection
)
OR NEW.feed_version != 1
OR NEW.projection_version != 1
OR NEW.projected_row_count != 0
OR hex(NEW.scope_fingerprint) != '8B63C5DDC48A2CC7DB69295238B96D5F814DBA50427C80B4D0079F061E6D3DE0'
OR NEW.hook_manifest_sha256 != lower(NEW.hook_manifest_sha256)
OR NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state AS source
  JOIN radroots_event_store_source_generation AS generation
    ON generation.source_generation = source.active_generation
  JOIN radroots_event_store_addressable_feed_integrity_v1 AS integrity
    ON integrity.source_generation = source.active_generation
  WHERE source.singleton = 1
    AND source.active_generation = NEW.source_generation
    AND generation.addressable_feed_version = NEW.feed_version
    AND NEW.last_transition_seq = generation.transition_floor_seq
    AND integrity.transition_floor_seq = generation.transition_floor_seq
    AND integrity.last_transition_seq = source.last_transition_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability cursor identity is invalid');
END;

CREATE TRIGGER radroots_event_store_food_availability_cursor_update_guard
BEFORE UPDATE ON radroots_event_store_food_availability_cursor
WHEN NEW.singleton != OLD.singleton
OR NEW.source_generation != OLD.source_generation
OR NEW.feed_version != OLD.feed_version
OR NEW.projection_version != OLD.projection_version
OR NEW.scope_fingerprint != OLD.scope_fingerprint
OR NEW.hook_manifest_sha256 != OLD.hook_manifest_sha256
OR NEW.last_transition_seq <= OLD.last_transition_seq
OR NEW.last_transition_seq - OLD.last_transition_seq > 1024
OR abs(NEW.projected_row_count - OLD.projected_row_count) > 64
OR abs(NEW.projected_row_count - OLD.projected_row_count) > (
  SELECT COUNT(*)
  FROM radroots_event_store_addressable_head_transition AS transition
  WHERE transition.source_generation = NEW.source_generation
    AND transition.transition_seq > OLD.last_transition_seq
    AND transition.transition_seq <= NEW.last_transition_seq
    AND transition.kind = 30402
)
OR NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state AS source
  WHERE source.singleton = 1
    AND source.active_generation = NEW.source_generation
    AND NEW.last_transition_seq <= source.last_transition_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability cursor update is invalid');
END;

CREATE TRIGGER radroots_event_store_food_availability_cursor_delete_guard
BEFORE DELETE ON radroots_event_store_food_availability_cursor
WHEN OLD.source_generation = (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
BEGIN
  SELECT RAISE(ABORT, 'event-store FoodAvailability cursor is immutable for the active generation');
END;

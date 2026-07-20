CREATE UNIQUE INDEX radroots_event_store_event_envelopes_seq_event_id_idx
ON event_envelopes(seq, event_id);

CREATE INDEX radroots_event_store_event_envelopes_kind_pubkey_idx
ON event_envelopes(kind, pubkey, event_id);

CREATE TABLE radroots_event_store_source_generation (
  source_generation BLOB PRIMARY KEY NOT NULL CHECK (length(source_generation) = 32),
  generation_ordinal INTEGER NOT NULL UNIQUE
    CHECK (generation_ordinal > 0 AND generation_ordinal < 9223372036854775807),
  reconciliation_version INTEGER NOT NULL CHECK (reconciliation_version > 0),
  addressable_feed_version INTEGER NOT NULL CHECK (addressable_feed_version > 0),
  event_contract_registry_version INTEGER NOT NULL
    CHECK (event_contract_registry_version > 0),
  hook_id TEXT NOT NULL CHECK (length(hook_id) > 0),
  hook_manifest_sha256 TEXT NOT NULL CHECK (
    length(hook_manifest_sha256) = 64
    AND hook_manifest_sha256 NOT GLOB '*[^0-9a-f]*'
  ),
  transition_floor_seq INTEGER NOT NULL CHECK (transition_floor_seq >= 0),
  baseline_raw_event_count INTEGER NOT NULL CHECK (baseline_raw_event_count >= 0),
  baseline_raw_tag_count INTEGER NOT NULL CHECK (baseline_raw_tag_count >= 0),
  baseline_raw_high_water_seq INTEGER NOT NULL
    CHECK (baseline_raw_high_water_seq >= 0)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_event_store_source_rebuild_commit_barrier (
  barrier_key INTEGER PRIMARY KEY NOT NULL
    CHECK (barrier_key = 0 AND barrier_key = 1)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_event_store_source_rebuild_marker (
  singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
  barrier_key INTEGER NOT NULL CHECK (barrier_key = 1),
  target_generation BLOB NOT NULL UNIQUE
    CHECK (length(target_generation) = 32),
  target_generation_ordinal INTEGER NOT NULL UNIQUE
    CHECK (
      target_generation_ordinal > 0
      AND target_generation_ordinal < 9223372036854775807
    ),
  reconciliation_version INTEGER NOT NULL CHECK (reconciliation_version > 0),
  addressable_feed_version INTEGER NOT NULL
    CHECK (addressable_feed_version > 0),
  event_contract_registry_version INTEGER NOT NULL
    CHECK (event_contract_registry_version > 0),
  hook_id TEXT NOT NULL CHECK (length(hook_id) > 0),
  hook_manifest_sha256 TEXT NOT NULL CHECK (
    length(hook_manifest_sha256) = 64
    AND hook_manifest_sha256 NOT GLOB '*[^0-9a-f]*'
  ),
  transition_floor_seq INTEGER NOT NULL CHECK (transition_floor_seq >= 0),
  baseline_raw_event_count INTEGER NOT NULL
    CHECK (baseline_raw_event_count >= 0),
  baseline_raw_tag_count INTEGER NOT NULL
    CHECK (baseline_raw_tag_count >= 0),
  baseline_raw_high_water_seq INTEGER NOT NULL
    CHECK (baseline_raw_high_water_seq >= 0),
  prior_active_generation BLOB
    CHECK (
      prior_active_generation IS NULL
      OR length(prior_active_generation) = 32
    ),
  prior_raw_event_count INTEGER
    CHECK (prior_raw_event_count IS NULL OR prior_raw_event_count >= 0),
  prior_raw_tag_count INTEGER
    CHECK (prior_raw_tag_count IS NULL OR prior_raw_tag_count >= 0),
  prior_raw_high_water_seq INTEGER
    CHECK (
      prior_raw_high_water_seq IS NULL
      OR prior_raw_high_water_seq >= 0
    ),
  prior_last_transition_seq INTEGER
    CHECK (
      prior_last_transition_seq IS NULL
      OR prior_last_transition_seq >= 0
    ),
  CHECK (
    (
      prior_active_generation IS NULL
      AND prior_raw_event_count IS NULL
      AND prior_raw_tag_count IS NULL
      AND prior_raw_high_water_seq IS NULL
      AND prior_last_transition_seq IS NULL
    )
    OR (
      prior_active_generation IS NOT NULL
      AND prior_raw_event_count IS NOT NULL
      AND prior_raw_tag_count IS NOT NULL
      AND prior_raw_high_water_seq IS NOT NULL
      AND prior_last_transition_seq IS NOT NULL
    )
  ),
  FOREIGN KEY (barrier_key)
    REFERENCES radroots_event_store_source_rebuild_commit_barrier(barrier_key)
      DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (target_generation)
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (prior_active_generation)
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT
) STRICT;

CREATE TABLE radroots_event_store_source_state (
  singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
  active_generation BLOB NOT NULL
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  raw_event_count INTEGER NOT NULL CHECK (raw_event_count >= 0),
  raw_tag_count INTEGER NOT NULL CHECK (raw_tag_count >= 0),
  raw_high_water_seq INTEGER NOT NULL CHECK (raw_high_water_seq >= 0),
  last_transition_seq INTEGER NOT NULL CHECK (last_transition_seq >= 0)
) STRICT;

CREATE TABLE radroots_event_store_write_lock (
  singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
  lock_version INTEGER NOT NULL CHECK (lock_version = 0)
) STRICT;

INSERT INTO radroots_event_store_write_lock(singleton, lock_version)
VALUES (1, 0);

CREATE TABLE radroots_event_store_projection_cursor_source (
  projection_id TEXT PRIMARY KEY NOT NULL
    REFERENCES projection_cursor(projection_id) ON DELETE CASCADE,
  source_generation BLOB
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  source_revision INTEGER NOT NULL
    CHECK (source_revision > 0 AND source_revision < 9223372036854775807)
) STRICT;

INSERT INTO radroots_event_store_projection_cursor_source(
  projection_id,
  source_generation,
  source_revision
)
SELECT projection_id, NULL, 1
FROM projection_cursor;

CREATE TRIGGER radroots_event_store_projection_cursor_insert_guard
BEFORE INSERT ON projection_cursor
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker
  WHERE singleton = 1
)
OR typeof(NEW.projection_id) != 'text'
OR length(NEW.projection_id) = 0
OR typeof(NEW.projection_version) != 'integer'
OR NEW.projection_version < 1
OR NEW.projection_version > 4294967295
OR typeof(NEW.last_event_seq) != 'integer'
OR NEW.last_event_seq < 0
OR typeof(NEW.updated_at_ms) != 'integer'
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state
  WHERE singleton = 1
    AND NEW.last_event_seq > raw_high_water_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store projection cursor insert is invalid');
END;

CREATE TRIGGER radroots_event_store_projection_cursor_identity_insert
AFTER INSERT ON projection_cursor
BEGIN
  INSERT INTO radroots_event_store_projection_cursor_source(
    projection_id,
    source_generation,
    source_revision
  )
  VALUES (
    NEW.projection_id,
    (
      SELECT active_generation
      FROM radroots_event_store_source_state
      WHERE singleton = 1
    ),
    1
  );
END;

CREATE TRIGGER radroots_event_store_projection_cursor_update_guard
BEFORE UPDATE ON projection_cursor
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker
  WHERE singleton = 1
)
OR NEW.projection_id IS NOT OLD.projection_id
OR typeof(NEW.projection_id) != 'text'
OR length(NEW.projection_id) = 0
OR typeof(NEW.projection_version) != 'integer'
OR NEW.projection_version < 1
OR NEW.projection_version > 4294967295
OR typeof(NEW.last_event_seq) != 'integer'
OR NEW.last_event_seq < 0
OR typeof(NEW.updated_at_ms) != 'integer'
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state
  WHERE singleton = 1
    AND NEW.last_event_seq > raw_high_water_seq
)
OR NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_projection_cursor_source
  WHERE projection_id = OLD.projection_id
    AND typeof(source_revision) = 'integer'
    AND source_revision > 0
    AND source_revision < 9223372036854775807
)
BEGIN
  SELECT RAISE(ABORT, 'event-store projection cursor update is invalid');
END;

CREATE TRIGGER radroots_event_store_projection_cursor_identity_update
AFTER UPDATE ON projection_cursor
BEGIN
  UPDATE radroots_event_store_projection_cursor_source
  SET
    source_generation = (
      SELECT active_generation
      FROM radroots_event_store_source_state
      WHERE singleton = 1
    ),
    source_revision = source_revision + 1
  WHERE projection_id = NEW.projection_id;
  SELECT CASE
    WHEN changes() != 1
    THEN RAISE(ABORT, 'event-store projection cursor revision update failed')
  END;
END;

CREATE TRIGGER radroots_event_store_projection_cursor_source_insert_guard
BEFORE INSERT ON radroots_event_store_projection_cursor_source
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker
  WHERE singleton = 1
)
OR NEW.source_generation IS NOT (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
OR NEW.source_revision != 1
OR NOT EXISTS (
  SELECT 1
  FROM projection_cursor
  WHERE projection_id = NEW.projection_id
)
BEGIN
  SELECT RAISE(ABORT, 'event-store projection cursor source identity is invalid');
END;

CREATE TRIGGER radroots_event_store_projection_cursor_source_update_guard
BEFORE UPDATE ON radroots_event_store_projection_cursor_source
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker
  WHERE singleton = 1
)
OR NEW.projection_id IS NOT OLD.projection_id
OR typeof(NEW.source_revision) != 'integer'
OR NEW.source_revision != OLD.source_revision + 1
OR NEW.source_generation IS NOT (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
BEGIN
  SELECT RAISE(ABORT, 'event-store projection cursor source update is invalid');
END;

CREATE TRIGGER radroots_event_store_projection_cursor_source_delete_guard
BEFORE DELETE ON radroots_event_store_projection_cursor_source
BEGIN
  SELECT RAISE(ABORT, 'event-store projection cursor source identity is immutable');
END;

CREATE TRIGGER radroots_event_store_projection_cursor_delete_guard
BEFORE DELETE ON projection_cursor
BEGIN
  SELECT RAISE(ABORT, 'event-store projection cursor identity is immutable');
END;

CREATE TABLE radroots_event_store_event_coordinate (
  source_generation BLOB NOT NULL
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  event_id TEXT NOT NULL,
  event_seq INTEGER NOT NULL CHECK (event_seq > 0),
  coordinate_type TEXT NOT NULL
    CHECK (coordinate_type IN ('replaceable', 'addressable')),
  kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 65535),
  pubkey TEXT NOT NULL,
  created_at INTEGER NOT NULL CHECK (created_at >= 0),
  inserted_at_ms INTEGER NOT NULL,
  admission_status TEXT NOT NULL
    CHECK (admission_status IN ('admitted', 'unsupported', 'invalid')),
  admission_code TEXT,
  contract_id TEXT,
  raw_d_tag TEXT NOT NULL,
  nip09_matchable INTEGER NOT NULL CHECK (nip09_matchable IN (0, 1)),
  nip09_d_tag TEXT,
  CHECK (
    (
      admission_status = 'admitted'
      AND admission_code IS NULL
      AND contract_id IS NOT NULL
    )
    OR (
      admission_status IN ('unsupported', 'invalid')
      AND admission_code IS NOT NULL
      AND contract_id IS NULL
    )
  ),
  CHECK (
    (
      coordinate_type = 'replaceable'
      AND (kind IN (0, 3) OR kind BETWEEN 10000 AND 19999)
      AND raw_d_tag = ''
      AND nip09_matchable = 1
      AND nip09_d_tag = ''
    )
    OR (
      coordinate_type = 'addressable'
      AND kind BETWEEN 30000 AND 39999
      AND (
        (nip09_matchable = 1 AND nip09_d_tag = raw_d_tag)
        OR (nip09_matchable = 0 AND nip09_d_tag IS NULL)
      )
    )
  ),
  PRIMARY KEY (source_generation, event_id),
  UNIQUE (source_generation, event_seq, event_id),
  UNIQUE (
    source_generation,
    event_seq,
    event_id,
    kind,
    pubkey,
    nip09_d_tag
  ),
  UNIQUE (
    source_generation,
    event_seq,
    event_id,
    kind,
    pubkey,
    raw_d_tag
  ),
  FOREIGN KEY (event_seq, event_id)
    REFERENCES event_envelopes(seq, event_id) ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_event_store_event_coordinate_raw_lookup_idx
ON radroots_event_store_event_coordinate(
  source_generation,
  coordinate_type,
  kind,
  pubkey,
  raw_d_tag,
  event_seq,
  event_id
);

CREATE INDEX radroots_event_store_event_coordinate_nip09_lookup_idx
ON radroots_event_store_event_coordinate(
  source_generation,
  kind,
  pubkey,
  nip09_d_tag,
  event_seq,
  event_id
)
WHERE nip09_matchable = 1;

CREATE TABLE radroots_event_store_nip09_request (
  source_generation BLOB NOT NULL
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  request_event_id TEXT NOT NULL,
  request_event_seq INTEGER NOT NULL,
  request_pubkey TEXT NOT NULL,
  request_created_at INTEGER NOT NULL CHECK (request_created_at >= 0),
  PRIMARY KEY (source_generation, request_event_id),
  FOREIGN KEY (request_event_seq, request_event_id)
    REFERENCES event_envelopes(seq, event_id) ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_event_store_nip09_request_author_idx
ON radroots_event_store_nip09_request(
  source_generation,
  request_pubkey,
  request_created_at,
  request_event_id
);

CREATE TABLE radroots_event_store_nip09_event_target (
  source_generation BLOB NOT NULL
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  request_event_id TEXT NOT NULL,
  target_event_id TEXT NOT NULL,
  source_tag_index INTEGER NOT NULL CHECK (source_tag_index >= 0),
  source_tag_value TEXT NOT NULL,
  CHECK (
    length(target_event_id) = 64
    AND target_event_id = lower(target_event_id)
    AND target_event_id NOT GLOB '*[^0-9a-f]*'
  ),
  PRIMARY KEY (source_generation, request_event_id, target_event_id),
  FOREIGN KEY (source_generation, request_event_id)
    REFERENCES radroots_event_store_nip09_request(
      source_generation,
      request_event_id
    ) ON DELETE RESTRICT,
  FOREIGN KEY (request_event_id, source_tag_index)
    REFERENCES event_envelope_tags(event_id, tag_index) ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_event_store_nip09_event_target_lookup_idx
ON radroots_event_store_nip09_event_target(
  source_generation,
  target_event_id,
  request_event_id
);

CREATE TABLE radroots_event_store_nip09_address_target (
  source_generation BLOB NOT NULL
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  request_event_id TEXT NOT NULL,
  target_kind INTEGER NOT NULL CHECK (target_kind BETWEEN 0 AND 65535),
  target_pubkey TEXT NOT NULL,
  target_d_tag TEXT NOT NULL,
  inclusive_cutoff INTEGER NOT NULL CHECK (inclusive_cutoff >= 0),
  source_tag_index INTEGER NOT NULL CHECK (source_tag_index >= 0),
  source_tag_value TEXT NOT NULL,
  source_kind_text TEXT NOT NULL,
  source_pubkey_text TEXT NOT NULL,
  source_d_tag TEXT NOT NULL,
  CHECK (
    source_tag_value = source_kind_text || ':' || source_pubkey_text || ':' || source_d_tag
  ),
  CHECK (
    length(target_pubkey) = 64
    AND target_pubkey = lower(target_pubkey)
    AND target_pubkey NOT GLOB '*[^0-9a-f]*'
  ),
  CHECK (
    (
      (target_kind IN (0, 3) OR target_kind BETWEEN 10000 AND 19999)
      AND target_d_tag = ''
    )
    OR target_kind BETWEEN 30000 AND 39999
  ),
  PRIMARY KEY (
    source_generation,
    request_event_id,
    target_kind,
    target_pubkey,
    target_d_tag
  ),
  UNIQUE (
    source_generation,
    request_event_id,
    target_kind,
    target_pubkey,
    target_d_tag,
    inclusive_cutoff
  ),
  FOREIGN KEY (source_generation, request_event_id)
    REFERENCES radroots_event_store_nip09_request(
      source_generation,
      request_event_id
    ) ON DELETE RESTRICT,
  FOREIGN KEY (request_event_id, source_tag_index)
    REFERENCES event_envelope_tags(event_id, tag_index) ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_event_store_nip09_address_target_lookup_idx
ON radroots_event_store_nip09_address_target(
  source_generation,
  target_kind,
  target_pubkey,
  target_d_tag,
  inclusive_cutoff,
  request_event_id
);

CREATE TABLE radroots_event_store_addressable_head_state (
  source_generation BLOB NOT NULL
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  kind INTEGER NOT NULL CHECK (kind BETWEEN 30000 AND 39999),
  pubkey TEXT NOT NULL,
  d_tag TEXT NOT NULL,
  raw_head_event_id TEXT NOT NULL,
  raw_head_event_seq INTEGER NOT NULL,
  raw_head_created_at INTEGER NOT NULL CHECK (raw_head_created_at >= 0),
  admission_status TEXT NOT NULL
    CHECK (admission_status IN ('admitted', 'unsupported', 'invalid')),
  admission_code TEXT,
  contract_id TEXT,
  visibility TEXT NOT NULL
    CHECK (visibility IN ('visible', 'not_admitted', 'suppressed')),
  nip09_outcome TEXT CHECK (nip09_outcome IN ('visible', 'suppressed')),
  nip09_reason TEXT CHECK (
    nip09_reason IN (
      'deletion_no_authorized_reference',
      'deletion_request_author_mismatch',
      'deletion_address_cutoff_precedes_target',
      'deletion_event_id_reference',
      'deletion_address_reference',
      'deletion_event_id_and_address_reference'
    )
  ),
  event_reference_request_id TEXT,
  address_reference_request_id TEXT,
  address_reference_cutoff INTEGER,
  last_origin TEXT NOT NULL CHECK (last_origin IN ('baseline', 'incremental')),
  last_cause_event_seq INTEGER,
  last_cause_event_id TEXT,
  CHECK (
    (last_origin = 'baseline'
      AND last_cause_event_seq IS NULL
      AND last_cause_event_id IS NULL)
    OR (last_origin = 'incremental'
      AND last_cause_event_seq IS NOT NULL
      AND last_cause_event_id IS NOT NULL)
  ),
  CHECK (
    (admission_status = 'admitted' AND contract_id IS NOT NULL)
    OR (admission_status != 'admitted' AND contract_id IS NULL
      AND admission_code IS NOT NULL)
  ),
  CHECK (
    (admission_status = 'admitted' AND admission_code IS NULL)
    OR admission_status != 'admitted'
  ),
  CHECK (
    (address_reference_request_id IS NULL
      AND address_reference_cutoff IS NULL)
    OR (address_reference_request_id IS NOT NULL
      AND address_reference_cutoff IS NOT NULL)
  ),
  CHECK (
    (visibility = 'visible' AND admission_status = 'admitted'
      AND nip09_outcome = 'visible' AND nip09_reason IS NOT NULL)
    OR (visibility = 'not_admitted' AND admission_status != 'admitted'
      AND nip09_outcome IS NULL AND nip09_reason IS NULL
      AND event_reference_request_id IS NULL
      AND address_reference_request_id IS NULL)
    OR (visibility = 'suppressed' AND admission_status = 'admitted'
      AND nip09_outcome = 'suppressed' AND nip09_reason IS NOT NULL)
  ),
  CHECK (
    (nip09_reason IN (
      'deletion_no_authorized_reference',
      'deletion_request_author_mismatch'
    ) AND event_reference_request_id IS NULL
      AND address_reference_request_id IS NULL
      AND nip09_outcome = 'visible')
    OR (nip09_reason = 'deletion_address_cutoff_precedes_target'
      AND event_reference_request_id IS NULL
      AND address_reference_request_id IS NOT NULL
      AND address_reference_cutoff < raw_head_created_at
      AND nip09_outcome = 'visible')
    OR (nip09_reason = 'deletion_event_id_reference'
      AND event_reference_request_id IS NOT NULL
      AND (address_reference_request_id IS NULL
        OR address_reference_cutoff < raw_head_created_at)
      AND nip09_outcome = 'suppressed')
    OR (nip09_reason = 'deletion_address_reference'
      AND event_reference_request_id IS NULL
      AND address_reference_request_id IS NOT NULL
      AND address_reference_cutoff >= raw_head_created_at
      AND nip09_outcome = 'suppressed')
    OR (nip09_reason = 'deletion_event_id_and_address_reference'
      AND event_reference_request_id IS NOT NULL
      AND address_reference_request_id IS NOT NULL
      AND address_reference_cutoff >= raw_head_created_at
      AND nip09_outcome = 'suppressed')
    OR (nip09_reason IS NULL)
  ),
  PRIMARY KEY (source_generation, kind, pubkey, d_tag),
  FOREIGN KEY (
    source_generation,
    raw_head_event_seq,
    raw_head_event_id,
    kind,
    pubkey,
    d_tag
  ) REFERENCES radroots_event_store_event_coordinate(
      source_generation,
      event_seq,
      event_id,
      kind,
      pubkey,
      raw_d_tag
  ) ON DELETE RESTRICT,
  FOREIGN KEY (last_cause_event_seq, last_cause_event_id)
    REFERENCES event_envelopes(seq, event_id) ON DELETE RESTRICT,
  FOREIGN KEY (
    source_generation,
    event_reference_request_id,
    raw_head_event_id
  )
    REFERENCES radroots_event_store_nip09_event_target(
      source_generation,
      request_event_id,
      target_event_id
    ) ON DELETE RESTRICT,
  FOREIGN KEY (
    source_generation,
    address_reference_request_id,
    kind,
    pubkey,
    d_tag,
    address_reference_cutoff
  ) REFERENCES radroots_event_store_nip09_address_target(
      source_generation,
      request_event_id,
      target_kind,
      target_pubkey,
      target_d_tag,
      inclusive_cutoff
  ) ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_event_store_addressable_head_transition (
  transition_seq INTEGER PRIMARY KEY AUTOINCREMENT,
  source_generation BLOB NOT NULL
    REFERENCES radroots_event_store_source_generation(source_generation)
      ON DELETE RESTRICT,
  origin TEXT NOT NULL CHECK (origin IN ('baseline', 'incremental')),
  kind INTEGER NOT NULL CHECK (kind BETWEEN 30000 AND 39999),
  pubkey TEXT NOT NULL,
  d_tag TEXT NOT NULL,
  raw_head_event_id TEXT NOT NULL,
  raw_head_event_seq INTEGER NOT NULL,
  raw_head_created_at INTEGER NOT NULL CHECK (raw_head_created_at >= 0),
  visible_event_id TEXT,
  visible_event_seq INTEGER,
  retracted_event_id TEXT,
  retracted_event_seq INTEGER,
  admission_status TEXT NOT NULL
    CHECK (admission_status IN ('admitted', 'unsupported', 'invalid')),
  admission_code TEXT,
  contract_id TEXT,
  visibility TEXT NOT NULL
    CHECK (visibility IN ('visible', 'not_admitted', 'suppressed')),
  nip09_outcome TEXT CHECK (nip09_outcome IN ('visible', 'suppressed')),
  nip09_reason TEXT CHECK (
    nip09_reason IN (
      'deletion_no_authorized_reference',
      'deletion_request_author_mismatch',
      'deletion_address_cutoff_precedes_target',
      'deletion_event_id_reference',
      'deletion_address_reference',
      'deletion_event_id_and_address_reference'
    )
  ),
  event_reference_request_id TEXT,
  address_reference_request_id TEXT,
  address_reference_cutoff INTEGER,
  cause_event_seq INTEGER,
  cause_event_id TEXT,
  raw_head_decision TEXT NOT NULL CHECK (
    raw_head_decision IN (
      'baseline_rebuild',
      'applied',
      'not_head_selected',
      'skipped_older',
      'skipped_same_timestamp_higher_event_id',
      'malformed_coordinate'
    )
  ),
  CHECK (
    (visible_event_id IS NULL AND visible_event_seq IS NULL)
    OR (visible_event_id = raw_head_event_id
      AND visible_event_seq = raw_head_event_seq)
  ),
  CHECK (
    (retracted_event_id IS NULL AND retracted_event_seq IS NULL)
    OR (retracted_event_id IS NOT NULL AND retracted_event_seq IS NOT NULL
      AND (
        visible_event_id IS NULL
        OR retracted_event_id != visible_event_id
      ))
  ),
  CHECK (
    (origin = 'baseline'
      AND cause_event_seq IS NULL
      AND cause_event_id IS NULL
      AND retracted_event_seq IS NULL
      AND retracted_event_id IS NULL)
    OR (origin = 'incremental'
      AND cause_event_seq IS NOT NULL
      AND cause_event_id IS NOT NULL)
  ),
  CHECK (
    (origin = 'baseline' AND raw_head_decision = 'baseline_rebuild')
    OR (origin = 'incremental' AND raw_head_decision != 'baseline_rebuild')
  ),
  CHECK (
    (admission_status = 'admitted' AND contract_id IS NOT NULL)
    OR (admission_status != 'admitted' AND contract_id IS NULL
      AND admission_code IS NOT NULL)
  ),
  CHECK (
    (admission_status = 'admitted' AND admission_code IS NULL)
    OR admission_status != 'admitted'
  ),
  CHECK (
    (address_reference_request_id IS NULL
      AND address_reference_cutoff IS NULL)
    OR (address_reference_request_id IS NOT NULL
      AND address_reference_cutoff IS NOT NULL)
  ),
  CHECK (
    (visibility = 'visible' AND admission_status = 'admitted'
      AND nip09_outcome = 'visible' AND nip09_reason IS NOT NULL
      AND visible_event_id IS NOT NULL)
    OR (visibility = 'not_admitted' AND admission_status != 'admitted'
      AND nip09_outcome IS NULL AND nip09_reason IS NULL
      AND event_reference_request_id IS NULL
      AND address_reference_request_id IS NULL
      AND visible_event_id IS NULL)
    OR (visibility = 'suppressed' AND admission_status = 'admitted'
      AND nip09_outcome = 'suppressed' AND nip09_reason IS NOT NULL
      AND visible_event_id IS NULL)
  ),
  CHECK (
    (nip09_reason IN (
      'deletion_no_authorized_reference',
      'deletion_request_author_mismatch'
    ) AND event_reference_request_id IS NULL
      AND address_reference_request_id IS NULL
      AND nip09_outcome = 'visible')
    OR (nip09_reason = 'deletion_address_cutoff_precedes_target'
      AND event_reference_request_id IS NULL
      AND address_reference_request_id IS NOT NULL
      AND address_reference_cutoff < raw_head_created_at
      AND nip09_outcome = 'visible')
    OR (nip09_reason = 'deletion_event_id_reference'
      AND event_reference_request_id IS NOT NULL
      AND (address_reference_request_id IS NULL
        OR address_reference_cutoff < raw_head_created_at)
      AND nip09_outcome = 'suppressed')
    OR (nip09_reason = 'deletion_address_reference'
      AND event_reference_request_id IS NULL
      AND address_reference_request_id IS NOT NULL
      AND address_reference_cutoff >= raw_head_created_at
      AND nip09_outcome = 'suppressed')
    OR (nip09_reason = 'deletion_event_id_and_address_reference'
      AND event_reference_request_id IS NOT NULL
      AND address_reference_request_id IS NOT NULL
      AND address_reference_cutoff >= raw_head_created_at
      AND nip09_outcome = 'suppressed')
    OR (nip09_reason IS NULL)
  ),
  FOREIGN KEY (
    source_generation,
    raw_head_event_seq,
    raw_head_event_id,
    kind,
    pubkey,
    d_tag
  ) REFERENCES radroots_event_store_event_coordinate(
      source_generation,
      event_seq,
      event_id,
      kind,
      pubkey,
      raw_d_tag
  ) ON DELETE RESTRICT,
  FOREIGN KEY (visible_event_seq, visible_event_id)
    REFERENCES event_envelopes(seq, event_id) ON DELETE RESTRICT,
  FOREIGN KEY (retracted_event_seq, retracted_event_id)
    REFERENCES event_envelopes(seq, event_id) ON DELETE RESTRICT,
  FOREIGN KEY (cause_event_seq, cause_event_id)
    REFERENCES event_envelopes(seq, event_id) ON DELETE RESTRICT,
  FOREIGN KEY (
    source_generation,
    event_reference_request_id,
    raw_head_event_id
  )
    REFERENCES radroots_event_store_nip09_event_target(
      source_generation,
      request_event_id,
      target_event_id
    ) ON DELETE RESTRICT,
  FOREIGN KEY (
    source_generation,
    address_reference_request_id,
    kind,
    pubkey,
    d_tag,
    address_reference_cutoff
  ) REFERENCES radroots_event_store_nip09_address_target(
      source_generation,
      request_event_id,
      target_kind,
      target_pubkey,
      target_d_tag,
      inclusive_cutoff
  ) ON DELETE RESTRICT
) STRICT;

CREATE INDEX radroots_event_store_addressable_transition_generation_idx
ON radroots_event_store_addressable_head_transition(
  source_generation,
  transition_seq
);

CREATE INDEX radroots_event_store_addressable_transition_kind_idx
ON radroots_event_store_addressable_head_transition(
  source_generation,
  kind,
  transition_seq
);

CREATE VIEW radroots_event_store_addressable_canonical_state AS
WITH raw_head AS (
  SELECT
    source.active_generation AS source_generation,
    coordinate.kind,
    coordinate.pubkey,
    coordinate.raw_d_tag AS d_tag,
    coordinate.event_id AS raw_head_event_id,
    coordinate.event_seq AS raw_head_event_seq,
    coordinate.created_at AS raw_head_created_at,
    coordinate.admission_status,
    coordinate.admission_code,
    coordinate.contract_id,
    coordinate.nip09_matchable,
    coordinate.nip09_d_tag
  FROM radroots_event_store_source_state AS source
  JOIN event_envelope_head AS head
    ON head.coordinate_type = 'addressable'
  JOIN radroots_event_store_event_coordinate AS coordinate
    ON coordinate.source_generation = source.active_generation
   AND coordinate.coordinate_type = 'addressable'
   AND coordinate.event_id = head.event_id
   AND coordinate.kind = head.kind
   AND coordinate.pubkey = head.pubkey
   AND coordinate.raw_d_tag = head.d_tag
  WHERE source.singleton = 1
),
canonical_evidence AS (
  SELECT
    raw_head.*,
    (
      SELECT request.request_event_id
      FROM radroots_event_store_nip09_event_target AS target
      JOIN radroots_event_store_nip09_request AS request
        ON request.source_generation = target.source_generation
       AND request.request_event_id = target.request_event_id
      WHERE target.source_generation = raw_head.source_generation
        AND target.target_event_id = raw_head.raw_head_event_id
        AND request.request_pubkey = raw_head.pubkey
      ORDER BY request.request_event_id
      LIMIT 1
    ) AS event_reference_request_id,
    (
      SELECT request.request_event_id
      FROM radroots_event_store_nip09_address_target AS target
      JOIN radroots_event_store_nip09_request AS request
        ON request.source_generation = target.source_generation
       AND request.request_event_id = target.request_event_id
      WHERE target.source_generation = raw_head.source_generation
        AND raw_head.nip09_matchable = 1
        AND target.target_kind = raw_head.kind
        AND target.target_pubkey = raw_head.pubkey
        AND target.target_d_tag = raw_head.nip09_d_tag
        AND request.request_pubkey = raw_head.pubkey
      ORDER BY target.inclusive_cutoff DESC, request.request_event_id
      LIMIT 1
    ) AS address_reference_request_id,
    (
      SELECT target.inclusive_cutoff
      FROM radroots_event_store_nip09_address_target AS target
      JOIN radroots_event_store_nip09_request AS request
        ON request.source_generation = target.source_generation
       AND request.request_event_id = target.request_event_id
      WHERE target.source_generation = raw_head.source_generation
        AND raw_head.nip09_matchable = 1
        AND target.target_kind = raw_head.kind
        AND target.target_pubkey = raw_head.pubkey
        AND target.target_d_tag = raw_head.nip09_d_tag
        AND request.request_pubkey = raw_head.pubkey
      ORDER BY target.inclusive_cutoff DESC, request.request_event_id
      LIMIT 1
    ) AS address_reference_cutoff,
    (
      EXISTS (
        SELECT 1
        FROM radroots_event_store_nip09_event_target AS target
        JOIN radroots_event_store_nip09_request AS request
          ON request.source_generation = target.source_generation
         AND request.request_event_id = target.request_event_id
        WHERE target.source_generation = raw_head.source_generation
          AND target.target_event_id = raw_head.raw_head_event_id
          AND request.request_pubkey != raw_head.pubkey
      )
      OR EXISTS (
        SELECT 1
        FROM radroots_event_store_nip09_address_target AS target
        JOIN radroots_event_store_nip09_request AS request
          ON request.source_generation = target.source_generation
         AND request.request_event_id = target.request_event_id
        WHERE target.source_generation = raw_head.source_generation
          AND raw_head.nip09_matchable = 1
          AND target.target_kind = raw_head.kind
          AND target.target_pubkey = raw_head.pubkey
          AND target.target_d_tag = raw_head.nip09_d_tag
          AND request.request_pubkey != raw_head.pubkey
      )
    ) AS has_unauthorized_reference
  FROM raw_head
)
SELECT
  source_generation,
  kind,
  pubkey,
  d_tag,
  raw_head_event_id,
  raw_head_event_seq,
  raw_head_created_at,
  admission_status,
  admission_code,
  contract_id,
  CASE
    WHEN admission_status != 'admitted' THEN 'not_admitted'
    WHEN event_reference_request_id IS NOT NULL
      OR (
        address_reference_request_id IS NOT NULL
        AND address_reference_cutoff >= raw_head_created_at
      )
    THEN 'suppressed'
    ELSE 'visible'
  END AS visibility,
  CASE
    WHEN admission_status != 'admitted' THEN NULL
    WHEN event_reference_request_id IS NOT NULL
      OR (
        address_reference_request_id IS NOT NULL
        AND address_reference_cutoff >= raw_head_created_at
      )
    THEN 'suppressed'
    ELSE 'visible'
  END AS nip09_outcome,
  CASE
    WHEN admission_status != 'admitted' THEN NULL
    WHEN event_reference_request_id IS NOT NULL
      AND address_reference_request_id IS NOT NULL
      AND address_reference_cutoff >= raw_head_created_at
    THEN 'deletion_event_id_and_address_reference'
    WHEN event_reference_request_id IS NOT NULL
    THEN 'deletion_event_id_reference'
    WHEN address_reference_request_id IS NOT NULL
      AND address_reference_cutoff >= raw_head_created_at
    THEN 'deletion_address_reference'
    WHEN address_reference_request_id IS NOT NULL
    THEN 'deletion_address_cutoff_precedes_target'
    WHEN has_unauthorized_reference
    THEN 'deletion_request_author_mismatch'
    ELSE 'deletion_no_authorized_reference'
  END AS nip09_reason,
  CASE
    WHEN admission_status = 'admitted' THEN event_reference_request_id
    ELSE NULL
  END AS event_reference_request_id,
  CASE
    WHEN admission_status = 'admitted' THEN address_reference_request_id
    ELSE NULL
  END AS address_reference_request_id,
  CASE
    WHEN admission_status = 'admitted' THEN address_reference_cutoff
    ELSE NULL
  END AS address_reference_cutoff
FROM canonical_evidence;

CREATE TRIGGER radroots_event_store_event_envelopes_insert_conflict_guard
BEFORE INSERT ON event_envelopes
WHEN EXISTS (
  SELECT 1
  FROM event_envelopes
  WHERE seq = NEW.seq OR event_id = NEW.event_id
)
BEGIN
  SELECT RAISE(IGNORE);
END;

CREATE TRIGGER radroots_event_store_event_tags_insert_conflict_guard
BEFORE INSERT ON event_envelope_tags
WHEN EXISTS (
  SELECT 1
  FROM event_envelope_tags
  WHERE event_id = NEW.event_id AND tag_index = NEW.tag_index
)
BEGIN
  SELECT RAISE(IGNORE);
END;

CREATE TRIGGER radroots_event_store_event_envelopes_append_guard
AFTER INSERT ON event_envelopes
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker
  WHERE singleton = 1
)
OR NEW.seq <= 0
OR NEW.seq = 9223372036854775807
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state
  WHERE singleton = 1
    AND NEW.seq <= raw_high_water_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store raw envelopes require an appendable positive sequence above the active high-water');
END;

CREATE TRIGGER radroots_event_store_event_tags_append_guard
AFTER INSERT ON event_envelope_tags
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker
  WHERE singleton = 1
)
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state AS state
  JOIN event_envelopes AS event ON event.event_id = NEW.event_id
  WHERE state.singleton = 1
    AND event.seq <= state.raw_high_water_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store raw tags may only accompany the pending append');
END;

CREATE TRIGGER radroots_event_store_event_head_insert_guard
BEFORE INSERT ON event_envelope_head
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
AND (
  NOT EXISTS (
    SELECT 1
    FROM radroots_event_store_source_state AS state
    JOIN radroots_event_store_event_coordinate AS coordinate
      ON coordinate.source_generation = state.active_generation
     AND coordinate.event_id = NEW.event_id
    JOIN event_envelopes AS event
      ON event.seq = coordinate.event_seq
     AND event.event_id = coordinate.event_id
    WHERE state.singleton = 1
      AND coordinate.coordinate_type = NEW.coordinate_type
      AND coordinate.kind = NEW.kind
      AND coordinate.pubkey = NEW.pubkey
      AND event.created_at = NEW.created_at
      AND event.inserted_at_ms = NEW.updated_at_ms
      AND (
        (NEW.coordinate_type = 'replaceable'
          AND NEW.d_tag IS NULL
          AND coordinate.raw_d_tag = '')
        OR (NEW.coordinate_type = 'addressable'
          AND NEW.d_tag = coordinate.raw_d_tag)
      )
  )
  OR EXISTS (
    SELECT 1
    FROM radroots_event_store_source_state AS state
    JOIN radroots_event_store_event_coordinate AS coordinate
      ON coordinate.source_generation = state.active_generation
     AND coordinate.coordinate_type = NEW.coordinate_type
     AND coordinate.kind = NEW.kind
     AND coordinate.pubkey = NEW.pubkey
     AND coordinate.raw_d_tag = COALESCE(NEW.d_tag, '')
    JOIN event_envelopes AS candidate
      ON candidate.seq = coordinate.event_seq
     AND candidate.event_id = coordinate.event_id
    WHERE state.singleton = 1
      AND (
        candidate.created_at > NEW.created_at
        OR (
          candidate.created_at = NEW.created_at
          AND candidate.event_id < NEW.event_id
        )
      )
  )
)
BEGIN
  SELECT RAISE(ABORT, 'event-store raw head insert must select the canonical coordinate winner');
END;

CREATE TRIGGER radroots_event_store_event_head_update_guard
BEFORE UPDATE ON event_envelope_head
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
BEGIN
  SELECT RAISE(ABORT, 'event-store raw heads are replaced by guarded delete and insert');
END;

CREATE TRIGGER radroots_event_store_event_head_delete_guard
BEFORE DELETE ON event_envelope_head
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
AND NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker AS marker
  JOIN radroots_event_store_source_state AS state
    ON state.singleton = marker.singleton
   AND state.active_generation = marker.target_generation
  WHERE marker.singleton = 1
)
AND NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state AS state
  JOIN radroots_event_store_event_coordinate AS coordinate
    ON coordinate.source_generation = state.active_generation
   AND coordinate.coordinate_type = OLD.coordinate_type
   AND coordinate.kind = OLD.kind
   AND coordinate.pubkey = OLD.pubkey
   AND coordinate.raw_d_tag = COALESCE(OLD.d_tag, '')
  JOIN event_envelopes AS candidate
    ON candidate.seq = coordinate.event_seq
   AND candidate.event_id = coordinate.event_id
  WHERE state.singleton = 1
    AND candidate.seq > state.raw_high_water_seq
    AND (
      candidate.created_at > OLD.created_at
      OR (
        candidate.created_at = OLD.created_at
        AND candidate.event_id < OLD.event_id
      )
    )
)
BEGIN
  SELECT RAISE(ABORT, 'event-store raw head deletion requires a better pending coordinate candidate');
END;

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

CREATE TRIGGER radroots_event_store_source_rebuild_marker_update_guard
BEFORE UPDATE ON radroots_event_store_source_rebuild_marker
BEGIN
  SELECT RAISE(ABORT, 'event-store rebuild marker is immutable');
END;

CREATE TRIGGER radroots_event_store_source_rebuild_marker_delete_guard
BEFORE DELETE ON radroots_event_store_source_rebuild_marker
WHEN NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_generation AS generation
  JOIN radroots_event_store_source_state AS state
    ON state.active_generation = generation.source_generation
  WHERE state.singleton = 1
    AND generation.source_generation = OLD.target_generation
    AND generation.generation_ordinal = OLD.target_generation_ordinal
    AND generation.generation_ordinal = (
      SELECT MAX(candidate.generation_ordinal)
      FROM radroots_event_store_source_generation AS candidate
    )
    AND generation.reconciliation_version = OLD.reconciliation_version
    AND generation.addressable_feed_version = OLD.addressable_feed_version
    AND generation.event_contract_registry_version =
      OLD.event_contract_registry_version
    AND generation.hook_id = OLD.hook_id
    AND generation.hook_manifest_sha256 = OLD.hook_manifest_sha256
    AND generation.transition_floor_seq = OLD.transition_floor_seq
    AND generation.baseline_raw_event_count = OLD.baseline_raw_event_count
    AND generation.baseline_raw_tag_count = OLD.baseline_raw_tag_count
    AND generation.baseline_raw_high_water_seq =
      OLD.baseline_raw_high_water_seq
    AND state.raw_event_count = OLD.baseline_raw_event_count
    AND state.raw_tag_count = OLD.baseline_raw_tag_count
    AND state.raw_high_water_seq = OLD.baseline_raw_high_water_seq
    AND state.raw_event_count = (
      SELECT COUNT(*)
      FROM event_envelopes
    )
    AND state.raw_tag_count = (
      SELECT COUNT(*)
      FROM event_envelope_tags
    )
    AND state.raw_high_water_seq = (
      SELECT COALESCE(MAX(seq), 0)
      FROM event_envelopes
    )
    AND state.last_transition_seq = (
      SELECT COALESCE(
        MAX(transition.transition_seq),
        generation.transition_floor_seq
      )
      FROM radroots_event_store_addressable_head_transition AS transition
      WHERE transition.source_generation = generation.source_generation
    )
    AND NOT EXISTS (
      SELECT 1
      FROM event_envelopes AS event
      WHERE (
        event.kind IN (0, 3)
        OR event.kind BETWEEN 10000 AND 19999
        OR event.kind BETWEEN 30000 AND 39999
      )
        AND NOT EXISTS (
          SELECT 1
          FROM radroots_event_store_event_coordinate AS coordinate
          WHERE coordinate.source_generation = generation.source_generation
            AND coordinate.event_seq = event.seq
            AND coordinate.event_id = event.event_id
        )
    )
    AND NOT EXISTS (
      SELECT 1
      FROM event_envelopes AS event
      WHERE event.kind = 5
        AND event.verification_status = 'verified'
        AND event.contract_status = 'admitted'
        AND event.contract_id = 'radroots.social.deletion_request.v1'
        AND NOT EXISTS (
          SELECT 1
          FROM radroots_event_store_nip09_request AS request
          WHERE request.source_generation = generation.source_generation
            AND request.request_event_seq = event.seq
            AND request.request_event_id = event.event_id
        )
    )
    AND NOT EXISTS (
      SELECT 1
      FROM radroots_event_store_event_coordinate AS coordinate
      WHERE coordinate.source_generation = generation.source_generation
        AND NOT EXISTS (
          SELECT 1
          FROM event_envelope_head AS head
          WHERE head.coordinate_type = coordinate.coordinate_type
            AND head.kind = coordinate.kind
            AND head.pubkey = coordinate.pubkey
            AND COALESCE(head.d_tag, '') = coordinate.raw_d_tag
        )
    )
    AND NOT EXISTS (
      SELECT 1
      FROM radroots_event_store_addressable_canonical_state AS canonical
      WHERE canonical.source_generation = generation.source_generation
        AND NOT EXISTS (
          SELECT 1
          FROM radroots_event_store_addressable_head_state AS stored
          WHERE stored.source_generation = canonical.source_generation
            AND stored.kind = canonical.kind
            AND stored.pubkey = canonical.pubkey
            AND stored.d_tag = canonical.d_tag
        )
    )
    AND (
      SELECT COUNT(*)
      FROM radroots_event_store_addressable_head_transition AS transition
      WHERE transition.source_generation = generation.source_generation
        AND transition.origin = 'baseline'
    ) = (
      SELECT COUNT(*)
      FROM radroots_event_store_addressable_head_state AS stored
      WHERE stored.source_generation = generation.source_generation
        AND stored.last_origin = 'baseline'
    )
    AND NOT EXISTS (
      SELECT 1
      FROM radroots_event_store_addressable_head_transition AS transition
      WHERE transition.source_generation = generation.source_generation
        AND transition.origin != 'baseline'
    )
)
BEGIN
  SELECT RAISE(ABORT, 'event-store rebuild marker cannot close before rebuilt source authority is complete');
END;

CREATE TRIGGER radroots_event_store_source_generation_insert_conflict_guard
BEFORE INSERT ON radroots_event_store_source_generation
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_generation
  WHERE source_generation = NEW.source_generation
    OR generation_ordinal = NEW.generation_ordinal
)
BEGIN
  SELECT RAISE(ABORT, 'event-store source generation already exists');
END;

CREATE TRIGGER radroots_event_store_source_generation_append_guard
BEFORE INSERT ON radroots_event_store_source_generation
WHEN NEW.generation_ordinal != (
  SELECT COALESCE(MAX(generation_ordinal), 0) + 1
  FROM radroots_event_store_source_generation
)
OR NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker
  WHERE singleton = 1
    AND target_generation = NEW.source_generation
    AND target_generation_ordinal = NEW.generation_ordinal
    AND reconciliation_version = NEW.reconciliation_version
    AND addressable_feed_version = NEW.addressable_feed_version
    AND event_contract_registry_version = NEW.event_contract_registry_version
    AND hook_id = NEW.hook_id
    AND hook_manifest_sha256 = NEW.hook_manifest_sha256
    AND transition_floor_seq = NEW.transition_floor_seq
    AND baseline_raw_event_count = NEW.baseline_raw_event_count
    AND baseline_raw_tag_count = NEW.baseline_raw_tag_count
    AND baseline_raw_high_water_seq = NEW.baseline_raw_high_water_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store source generation append requires its exact open rebuild marker');
END;

CREATE TRIGGER radroots_event_store_source_state_insert_conflict_guard
BEFORE INSERT ON radroots_event_store_source_state
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state
  WHERE singleton = NEW.singleton
)
OR NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker AS marker
  JOIN radroots_event_store_source_generation AS generation
    ON generation.source_generation = marker.target_generation
  WHERE marker.singleton = 1
    AND marker.prior_active_generation IS NULL
    AND marker.target_generation = NEW.active_generation
    AND marker.target_generation_ordinal = 1
    AND generation.generation_ordinal = marker.target_generation_ordinal
    AND generation.generation_ordinal = (
      SELECT MAX(candidate.generation_ordinal)
      FROM radroots_event_store_source_generation AS candidate
    )
    AND NEW.raw_event_count = 0
    AND NEW.raw_tag_count = 0
    AND NEW.raw_high_water_seq = 0
    AND NEW.last_transition_seq = marker.transition_floor_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store source state initialization requires the first open rebuild marker');
END;

CREATE TRIGGER radroots_event_store_source_state_active_generation_guard
BEFORE UPDATE OF active_generation ON radroots_event_store_source_state
WHEN NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker AS marker
  JOIN radroots_event_store_source_generation AS generation
    ON generation.source_generation = marker.target_generation
  WHERE marker.singleton = 1
    AND OLD.singleton = 1
    AND NEW.singleton = 1
    AND marker.prior_active_generation = OLD.active_generation
    AND marker.prior_raw_event_count = OLD.raw_event_count
    AND marker.prior_raw_tag_count = OLD.raw_tag_count
    AND marker.prior_raw_high_water_seq = OLD.raw_high_water_seq
    AND marker.prior_last_transition_seq = OLD.last_transition_seq
    AND marker.target_generation = NEW.active_generation
    AND generation.generation_ordinal = marker.target_generation_ordinal
    AND generation.generation_ordinal = (
      SELECT MAX(candidate.generation_ordinal)
      FROM radroots_event_store_source_generation AS candidate
    )
    AND NEW.raw_event_count = 0
    AND NEW.raw_tag_count = 0
    AND NEW.raw_high_water_seq = 0
    AND NEW.last_transition_seq = marker.transition_floor_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store active source generation rotation requires its exact open rebuild marker');
END;

CREATE TRIGGER radroots_event_store_event_coordinate_insert_guard
BEFORE INSERT ON radroots_event_store_event_coordinate
WHEN NEW.source_generation IS NOT (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_event_coordinate
  WHERE source_generation = NEW.source_generation
    AND event_id = NEW.event_id
)
OR NOT EXISTS (
  SELECT 1
  FROM event_envelopes AS event
  WHERE event.seq = NEW.event_seq
    AND event.event_id = NEW.event_id
    AND event.kind = NEW.kind
    AND event.pubkey = NEW.pubkey
    AND event.created_at = NEW.created_at
    AND event.inserted_at_ms = NEW.inserted_at_ms
    AND event.verification_status = 'verified'
    AND event.contract_status = NEW.admission_status
    AND event.contract_id IS NEW.contract_id
    AND (
      (
        NEW.coordinate_type = 'replaceable'
        AND (event.kind IN (0, 3) OR event.kind BETWEEN 10000 AND 19999)
        AND NEW.raw_d_tag = ''
        AND NEW.nip09_matchable = 1
        AND NEW.nip09_d_tag = ''
      )
      OR (
        NEW.coordinate_type = 'addressable'
        AND event.kind BETWEEN 30000 AND 39999
        AND NEW.raw_d_tag = COALESCE((
          SELECT tag_value
          FROM event_envelope_tags
          WHERE event_id = event.event_id
            AND tag_name = 'd'
          ORDER BY tag_index
          LIMIT 1
        ), '')
        AND NEW.nip09_matchable = CASE
          WHEN EXISTS (
            SELECT 1
            FROM event_envelope_tags AS first_d
            WHERE first_d.event_id = event.event_id
              AND first_d.tag_name = 'd'
              AND first_d.tag_index = (
                SELECT MIN(candidate.tag_index)
                FROM event_envelope_tags AS candidate
                WHERE candidate.event_id = event.event_id
                  AND candidate.tag_name = 'd'
              )
              AND first_d.tag_value IS NOT NULL
              AND length(CAST(
                CAST(event.kind AS TEXT) || ':' || event.pubkey
                  || ':' || first_d.tag_value
                AS BLOB
              )) <= 4096
          )
          THEN 1
          ELSE 0
        END
      )
    )
)
BEGIN
  SELECT CASE
    WHEN NEW.source_generation IS NOT (
      SELECT active_generation
      FROM radroots_event_store_source_state
      WHERE singleton = 1
    )
    THEN RAISE(ABORT, 'event-store historical coordinate facts are immutable')
    WHEN EXISTS (
      SELECT 1
      FROM radroots_event_store_event_coordinate
      WHERE source_generation = NEW.source_generation
        AND event_id = NEW.event_id
    )
    THEN RAISE(IGNORE)
    ELSE RAISE(ABORT, 'event-store coordinate fact does not match immutable raw event data')
  END;
END;

CREATE TRIGGER radroots_event_store_nip09_request_insert_guard
BEFORE INSERT ON radroots_event_store_nip09_request
WHEN NEW.source_generation IS NOT (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_nip09_request
  WHERE source_generation = NEW.source_generation
    AND request_event_id = NEW.request_event_id
)
OR NOT EXISTS (
  SELECT 1
  FROM event_envelopes AS event
  WHERE event.seq = NEW.request_event_seq
    AND event.event_id = NEW.request_event_id
    AND event.kind = 5
    AND event.pubkey = NEW.request_pubkey
    AND event.created_at = NEW.request_created_at
    AND event.verification_status = 'verified'
    AND event.contract_status = 'admitted'
    AND event.contract_id = 'radroots.social.deletion_request.v1'
)
BEGIN
  SELECT CASE
    WHEN NEW.source_generation IS NOT (
      SELECT active_generation
      FROM radroots_event_store_source_state
      WHERE singleton = 1
    )
    THEN RAISE(ABORT, 'event-store historical NIP-09 facts are immutable')
    WHEN EXISTS (
      SELECT 1
      FROM radroots_event_store_nip09_request
      WHERE source_generation = NEW.source_generation
        AND request_event_id = NEW.request_event_id
    )
    THEN RAISE(IGNORE)
    ELSE RAISE(ABORT, 'event-store NIP-09 request fact does not match its admitted raw event')
  END;
END;

CREATE TRIGGER radroots_event_store_nip09_event_target_insert_guard
BEFORE INSERT ON radroots_event_store_nip09_event_target
WHEN NEW.source_generation IS NOT (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_nip09_event_target
  WHERE source_generation = NEW.source_generation
    AND request_event_id = NEW.request_event_id
    AND target_event_id = NEW.target_event_id
)
OR NOT EXISTS (
  SELECT 1
  FROM event_envelope_tags AS tag
  WHERE tag.event_id = NEW.request_event_id
    AND tag.tag_index = NEW.source_tag_index
    AND tag.tag_name = 'e'
    AND tag.tag_value = NEW.source_tag_value
    AND length(NEW.source_tag_value) = 64
    AND NEW.source_tag_value NOT GLOB '*[^0-9A-Fa-f]*'
    AND lower(NEW.source_tag_value) = NEW.target_event_id
    AND tag.tag_index = (
      SELECT MIN(candidate.tag_index)
      FROM event_envelope_tags AS candidate
      WHERE candidate.event_id = NEW.request_event_id
        AND candidate.tag_name = 'e'
        AND length(candidate.tag_value) = 64
        AND candidate.tag_value NOT GLOB '*[^0-9A-Fa-f]*'
        AND lower(candidate.tag_value) = NEW.target_event_id
    )
)
BEGIN
  SELECT CASE
    WHEN NEW.source_generation IS NOT (
      SELECT active_generation
      FROM radroots_event_store_source_state
      WHERE singleton = 1
    )
    THEN RAISE(ABORT, 'event-store historical NIP-09 facts are immutable')
    WHEN EXISTS (
      SELECT 1
      FROM radroots_event_store_nip09_event_target
      WHERE source_generation = NEW.source_generation
        AND request_event_id = NEW.request_event_id
        AND target_event_id = NEW.target_event_id
    )
    THEN RAISE(IGNORE)
    ELSE RAISE(ABORT, 'event-store NIP-09 event target does not match its canonical e tag')
  END;
END;

CREATE TRIGGER radroots_event_store_nip09_address_target_insert_guard
BEFORE INSERT ON radroots_event_store_nip09_address_target
WHEN NEW.source_generation IS NOT (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_nip09_address_target
  WHERE source_generation = NEW.source_generation
    AND request_event_id = NEW.request_event_id
    AND target_kind = NEW.target_kind
    AND target_pubkey = NEW.target_pubkey
    AND target_d_tag = NEW.target_d_tag
)
OR NOT EXISTS (
  SELECT 1
  FROM event_envelope_tags AS tag
  WHERE tag.event_id = NEW.request_event_id
    AND tag.tag_index = NEW.source_tag_index
    AND tag.tag_name = 'a'
    AND tag.tag_value = NEW.source_tag_value
    AND length(CAST(NEW.source_tag_value AS BLOB)) <= 4096
    AND length(NEW.source_kind_text) > 0
    AND (
      (
        NEW.source_kind_text NOT GLOB '*[^0-9]*'
        AND length(NEW.source_kind_text) > 0
      )
      OR (
        substr(NEW.source_kind_text, 1, 1) = '+'
        AND length(NEW.source_kind_text) > 1
        AND substr(NEW.source_kind_text, 2) NOT GLOB '*[^0-9]*'
      )
    )
    AND CAST(NEW.source_kind_text AS INTEGER) = NEW.target_kind
    AND length(NEW.source_pubkey_text) = 64
    AND NEW.source_pubkey_text NOT GLOB '*[^0-9A-Fa-f]*'
    AND lower(NEW.source_pubkey_text) = NEW.target_pubkey
    AND NEW.source_d_tag = NEW.target_d_tag
    AND NOT EXISTS (
      SELECT 1
      FROM event_envelope_tags AS candidate
      WHERE candidate.event_id = NEW.request_event_id
        AND candidate.tag_name = 'a'
        AND candidate.tag_index < NEW.source_tag_index
        AND candidate.tag_value IS NOT NULL
        AND length(CAST(candidate.tag_value AS BLOB)) <= 4096
        AND instr(candidate.tag_value, ':') > 1
        AND instr(
          substr(candidate.tag_value, instr(candidate.tag_value, ':') + 1),
          ':'
        ) > 0
        AND (
          (
            substr(
              candidate.tag_value,
              1,
              instr(candidate.tag_value, ':') - 1
            ) NOT GLOB '*[^0-9]*'
          )
          OR (
            substr(candidate.tag_value, 1, 1) = '+'
            AND length(
              substr(
                candidate.tag_value,
                1,
                instr(candidate.tag_value, ':') - 1
              )
            ) > 1
            AND substr(
              candidate.tag_value,
              2,
              instr(candidate.tag_value, ':') - 2
            ) NOT GLOB '*[^0-9]*'
          )
        )
        AND CAST(
          substr(
            candidate.tag_value,
            1,
            instr(candidate.tag_value, ':') - 1
          ) AS INTEGER
        ) = NEW.target_kind
        AND length(
          substr(
            substr(candidate.tag_value, instr(candidate.tag_value, ':') + 1),
            1,
            instr(
              substr(candidate.tag_value, instr(candidate.tag_value, ':') + 1),
              ':'
            ) - 1
          )
        ) = 64
        AND substr(
          substr(candidate.tag_value, instr(candidate.tag_value, ':') + 1),
          1,
          instr(
            substr(candidate.tag_value, instr(candidate.tag_value, ':') + 1),
            ':'
          ) - 1
        ) NOT GLOB '*[^0-9A-Fa-f]*'
        AND lower(substr(
          substr(candidate.tag_value, instr(candidate.tag_value, ':') + 1),
          1,
          instr(
            substr(candidate.tag_value, instr(candidate.tag_value, ':') + 1),
            ':'
          ) - 1
        )) = NEW.target_pubkey
        AND substr(
          substr(candidate.tag_value, instr(candidate.tag_value, ':') + 1),
          instr(
            substr(candidate.tag_value, instr(candidate.tag_value, ':') + 1),
            ':'
          ) + 1
        ) = NEW.target_d_tag
    )
)
OR NEW.inclusive_cutoff IS NOT (
  SELECT request_created_at
  FROM radroots_event_store_nip09_request
  WHERE source_generation = NEW.source_generation
    AND request_event_id = NEW.request_event_id
)
BEGIN
  SELECT CASE
    WHEN NEW.source_generation IS NOT (
      SELECT active_generation
      FROM radroots_event_store_source_state
      WHERE singleton = 1
    )
    THEN RAISE(ABORT, 'event-store historical NIP-09 facts are immutable')
    WHEN EXISTS (
      SELECT 1
      FROM radroots_event_store_nip09_address_target
      WHERE source_generation = NEW.source_generation
        AND request_event_id = NEW.request_event_id
        AND target_kind = NEW.target_kind
        AND target_pubkey = NEW.target_pubkey
        AND target_d_tag = NEW.target_d_tag
    )
    THEN RAISE(IGNORE)
    ELSE RAISE(ABORT, 'event-store NIP-09 address target does not match its canonical a tag')
  END;
END;

CREATE TRIGGER radroots_event_store_addressable_state_insert_guard
BEFORE INSERT ON radroots_event_store_addressable_head_state
WHEN NEW.source_generation IS NOT (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_addressable_head_state
  WHERE source_generation = NEW.source_generation
    AND kind = NEW.kind
    AND pubkey = NEW.pubkey
    AND d_tag = NEW.d_tag
)
OR NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_addressable_head_transition AS transition
  WHERE transition.source_generation = NEW.source_generation
    AND transition.kind = NEW.kind
    AND transition.pubkey = NEW.pubkey
    AND transition.d_tag = NEW.d_tag
    AND transition.transition_seq = (
      SELECT MAX(candidate.transition_seq)
      FROM radroots_event_store_addressable_head_transition AS candidate
      WHERE candidate.source_generation = NEW.source_generation
        AND candidate.kind = NEW.kind
        AND candidate.pubkey = NEW.pubkey
        AND candidate.d_tag = NEW.d_tag
    )
    AND transition.raw_head_event_id IS NEW.raw_head_event_id
    AND transition.raw_head_event_seq IS NEW.raw_head_event_seq
    AND transition.raw_head_created_at IS NEW.raw_head_created_at
    AND transition.admission_status IS NEW.admission_status
    AND transition.admission_code IS NEW.admission_code
    AND transition.contract_id IS NEW.contract_id
    AND transition.visibility IS NEW.visibility
    AND transition.nip09_outcome IS NEW.nip09_outcome
    AND transition.nip09_reason IS NEW.nip09_reason
    AND transition.event_reference_request_id IS NEW.event_reference_request_id
    AND transition.address_reference_request_id IS NEW.address_reference_request_id
    AND transition.address_reference_cutoff IS NEW.address_reference_cutoff
    AND transition.origin IS NEW.last_origin
    AND transition.cause_event_seq IS NEW.last_cause_event_seq
    AND transition.cause_event_id IS NEW.last_cause_event_id
)
BEGIN
  SELECT CASE
    WHEN NEW.source_generation IS NOT (
      SELECT active_generation
      FROM radroots_event_store_source_state
      WHERE singleton = 1
    )
    THEN RAISE(ABORT, 'event-store historical addressable head state is immutable')
    WHEN EXISTS (
      SELECT 1
      FROM radroots_event_store_addressable_head_state
      WHERE source_generation = NEW.source_generation
        AND kind = NEW.kind
        AND pubkey = NEW.pubkey
        AND d_tag = NEW.d_tag
    )
    THEN RAISE(ABORT, 'event-store addressable head state already exists')
    ELSE RAISE(ABORT, 'event-store addressable head state must match its latest transition')
  END;
END;

CREATE TRIGGER radroots_event_store_addressable_transition_insert_guard
BEFORE INSERT ON radroots_event_store_addressable_head_transition
WHEN NEW.source_generation IS NOT (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_addressable_head_transition
  WHERE transition_seq = NEW.transition_seq
)
OR NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_addressable_canonical_state AS canonical
  WHERE canonical.source_generation = NEW.source_generation
    AND canonical.kind = NEW.kind
    AND canonical.pubkey = NEW.pubkey
    AND canonical.d_tag = NEW.d_tag
    AND canonical.raw_head_event_id IS NEW.raw_head_event_id
    AND canonical.raw_head_event_seq IS NEW.raw_head_event_seq
    AND canonical.raw_head_created_at IS NEW.raw_head_created_at
    AND canonical.admission_status IS NEW.admission_status
    AND canonical.admission_code IS NEW.admission_code
    AND canonical.contract_id IS NEW.contract_id
    AND canonical.visibility IS NEW.visibility
    AND canonical.nip09_outcome IS NEW.nip09_outcome
    AND canonical.nip09_reason IS NEW.nip09_reason
    AND canonical.event_reference_request_id IS NEW.event_reference_request_id
    AND canonical.address_reference_request_id IS NEW.address_reference_request_id
    AND canonical.address_reference_cutoff IS NEW.address_reference_cutoff
    AND NEW.visible_event_id IS CASE
      WHEN canonical.visibility = 'visible'
      THEN canonical.raw_head_event_id
      ELSE NULL
    END
    AND NEW.visible_event_seq IS CASE
      WHEN canonical.visibility = 'visible'
      THEN canonical.raw_head_event_seq
      ELSE NULL
    END
)
OR (
  NEW.origin = 'baseline'
  AND NOT EXISTS (
    SELECT 1
    FROM radroots_event_store_source_state
    WHERE singleton = 1
      AND raw_event_count = 0
      AND raw_tag_count = 0
      AND raw_high_water_seq = 0
  )
)
OR (
  NEW.origin = 'incremental'
  AND NOT EXISTS (
    SELECT 1
    FROM radroots_event_store_source_state AS source
    JOIN event_envelopes AS cause
      ON cause.seq = NEW.cause_event_seq
     AND cause.event_id = NEW.cause_event_id
    WHERE source.singleton = 1
      AND NEW.cause_event_seq > source.raw_high_water_seq
      AND NEW.cause_event_seq = (
        SELECT MAX(candidate.seq)
        FROM event_envelopes AS candidate
      )
  )
)
OR EXISTS (
  SELECT 1
  FROM radroots_event_store_addressable_head_state AS current
  WHERE current.source_generation = NEW.source_generation
    AND current.kind = NEW.kind
    AND current.pubkey = NEW.pubkey
    AND current.d_tag = NEW.d_tag
    AND current.raw_head_event_id IS NEW.raw_head_event_id
    AND current.raw_head_event_seq IS NEW.raw_head_event_seq
    AND current.raw_head_created_at IS NEW.raw_head_created_at
    AND current.admission_status IS NEW.admission_status
    AND current.admission_code IS NEW.admission_code
    AND current.contract_id IS NEW.contract_id
    AND current.visibility IS NEW.visibility
    AND current.nip09_outcome IS NEW.nip09_outcome
    AND current.nip09_reason IS NEW.nip09_reason
    AND current.event_reference_request_id IS NEW.event_reference_request_id
    AND current.address_reference_request_id IS NEW.address_reference_request_id
    AND current.address_reference_cutoff IS NEW.address_reference_cutoff
)
OR NEW.retracted_event_id IS NOT (
  SELECT CASE
    WHEN current.visibility = 'visible'
      AND (
        NEW.visibility != 'visible'
        OR current.raw_head_event_id != NEW.raw_head_event_id
      )
    THEN current.raw_head_event_id
    ELSE NULL
  END
  FROM radroots_event_store_addressable_head_state AS current
  WHERE current.source_generation = NEW.source_generation
    AND current.kind = NEW.kind
    AND current.pubkey = NEW.pubkey
    AND current.d_tag = NEW.d_tag
)
OR NEW.retracted_event_seq IS NOT (
  SELECT CASE
    WHEN current.visibility = 'visible'
      AND (
        NEW.visibility != 'visible'
        OR current.raw_head_event_id != NEW.raw_head_event_id
      )
    THEN current.raw_head_event_seq
    ELSE NULL
  END
  FROM radroots_event_store_addressable_head_state AS current
  WHERE current.source_generation = NEW.source_generation
    AND current.kind = NEW.kind
    AND current.pubkey = NEW.pubkey
    AND current.d_tag = NEW.d_tag
)
BEGIN
  SELECT CASE
    WHEN NEW.source_generation IS NOT (
      SELECT active_generation
      FROM radroots_event_store_source_state
      WHERE singleton = 1
    )
    THEN RAISE(ABORT, 'event-store historical addressable transitions are immutable')
    WHEN EXISTS (
      SELECT 1
      FROM radroots_event_store_addressable_head_transition
      WHERE transition_seq = NEW.transition_seq
    )
    THEN RAISE(ABORT, 'event-store transition sequence already exists')
    ELSE RAISE(ABORT, 'event-store transition does not match a pending canonical state change')
  END;
END;

CREATE TRIGGER radroots_event_store_addressable_transition_floor_guard
AFTER INSERT ON radroots_event_store_addressable_head_transition
WHEN NEW.transition_seq <= (
  SELECT transition_floor_seq
  FROM radroots_event_store_source_generation
  WHERE source_generation = NEW.source_generation
)
BEGIN
  SELECT RAISE(ABORT, 'event-store transition sequence is not above its generation floor');
END;

CREATE TRIGGER radroots_event_store_addressable_transition_sequence_guard
AFTER INSERT ON radroots_event_store_addressable_head_transition
WHEN NEW.transition_seq = 9223372036854775807
OR NEW.transition_seq != (
  SELECT COALESCE(MAX(transition_seq), 0) + 1
  FROM radroots_event_store_addressable_head_transition
  WHERE transition_seq != NEW.transition_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store transition sequence must append contiguously');
END;

CREATE TRIGGER radroots_event_store_event_envelopes_raw_update_guard
BEFORE UPDATE OF
  seq,
  event_id,
  pubkey,
  created_at,
  kind,
  tags_json,
  content,
  sig,
  raw_json,
  inserted_at_ms
ON event_envelopes
BEGIN
  SELECT RAISE(ABORT, 'event-store raw envelopes are immutable');
END;

CREATE TRIGGER radroots_event_store_event_envelopes_derived_update_guard
BEFORE UPDATE OF
  verification_status,
  contract_status,
  contract_id,
  event_class,
  projection_eligible,
  updated_at_ms
ON event_envelopes
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
AND NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker AS marker
  JOIN radroots_event_store_source_state AS state
    ON state.singleton = marker.singleton
   AND state.active_generation = marker.target_generation
  WHERE marker.singleton = 1
    AND state.raw_event_count = 0
    AND state.raw_tag_count = 0
    AND state.raw_high_water_seq = 0
    AND state.last_transition_seq = marker.transition_floor_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store derived envelope classification is immutable after reconciliation');
END;

CREATE TRIGGER radroots_event_store_event_envelopes_delete_guard
BEFORE DELETE ON event_envelopes
BEGIN
  SELECT RAISE(ABORT, 'event-store raw envelopes are immutable');
END;

CREATE TRIGGER radroots_event_store_event_tags_raw_update_guard
BEFORE UPDATE OF
  event_id,
  tag_index,
  tag_name,
  tag_value,
  tag_json
ON event_envelope_tags
BEGIN
  SELECT RAISE(ABORT, 'event-store raw event tags are immutable');
END;

CREATE TRIGGER radroots_event_store_event_tags_derived_update_guard
BEFORE UPDATE OF contract_semantic, contract_value_type, relay_indexed
ON event_envelope_tags
WHEN EXISTS (
  SELECT 1
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
AND NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_source_rebuild_marker AS marker
  JOIN radroots_event_store_source_state AS state
    ON state.singleton = marker.singleton
   AND state.active_generation = marker.target_generation
  WHERE marker.singleton = 1
    AND state.raw_event_count = 0
    AND state.raw_tag_count = 0
    AND state.raw_high_water_seq = 0
    AND state.last_transition_seq = marker.transition_floor_seq
)
BEGIN
  SELECT RAISE(ABORT, 'event-store derived tag classification is immutable after reconciliation');
END;

CREATE TRIGGER radroots_event_store_event_tags_delete_guard
BEFORE DELETE ON event_envelope_tags
BEGIN
  SELECT RAISE(ABORT, 'event-store raw event tags are immutable');
END;

CREATE TRIGGER radroots_event_store_source_generation_update_guard
BEFORE UPDATE ON radroots_event_store_source_generation
BEGIN
  SELECT RAISE(ABORT, 'event-store source generations are immutable');
END;

CREATE TRIGGER radroots_event_store_source_generation_delete_guard
BEFORE DELETE ON radroots_event_store_source_generation
BEGIN
  SELECT RAISE(ABORT, 'event-store source generations are immutable');
END;

CREATE TRIGGER radroots_event_store_source_state_authority_update_guard
BEFORE UPDATE OF
  raw_event_count,
  raw_tag_count,
  raw_high_water_seq,
  last_transition_seq
ON radroots_event_store_source_state
WHEN NEW.active_generation IS OLD.active_generation
AND (
  (
    EXISTS (
      SELECT 1
      FROM radroots_event_store_source_rebuild_marker
      WHERE singleton = 1
    )
    AND NOT EXISTS (
      SELECT 1
      FROM radroots_event_store_source_rebuild_marker AS marker
      WHERE marker.singleton = 1
        AND marker.target_generation = OLD.active_generation
        AND OLD.raw_event_count = 0
        AND OLD.raw_tag_count = 0
        AND OLD.raw_high_water_seq = 0
        AND OLD.last_transition_seq = marker.transition_floor_seq
        AND NEW.raw_event_count = marker.baseline_raw_event_count
        AND NEW.raw_tag_count = marker.baseline_raw_tag_count
        AND NEW.raw_high_water_seq = marker.baseline_raw_high_water_seq
        AND NEW.raw_event_count = (
          SELECT COUNT(*)
          FROM event_envelopes
        )
        AND NEW.raw_tag_count = (
          SELECT COUNT(*)
          FROM event_envelope_tags
        )
        AND NEW.raw_high_water_seq = (
          SELECT COALESCE(MAX(seq), 0)
          FROM event_envelopes
        )
        AND NEW.last_transition_seq = (
          SELECT COALESCE(
            MAX(transition.transition_seq),
            marker.transition_floor_seq
          )
          FROM radroots_event_store_addressable_head_transition AS transition
          WHERE transition.source_generation = marker.target_generation
        )
    )
  )
  OR (
    NOT EXISTS (
      SELECT 1
      FROM radroots_event_store_source_rebuild_marker
      WHERE singleton = 1
    )
    AND NOT (
      NEW.raw_event_count = OLD.raw_event_count + 1
      AND NEW.raw_high_water_seq > OLD.raw_high_water_seq
      AND NEW.raw_high_water_seq = (
        SELECT MAX(seq)
        FROM event_envelopes
      )
      AND 1 = (
        SELECT COUNT(*)
        FROM event_envelopes
        WHERE seq > OLD.raw_high_water_seq
      )
      AND NEW.raw_tag_count = OLD.raw_tag_count + (
        SELECT COUNT(*)
        FROM event_envelope_tags AS tag
        JOIN event_envelopes AS event ON event.event_id = tag.event_id
        WHERE event.seq > OLD.raw_high_water_seq
      )
      AND NEW.last_transition_seq = (
        SELECT COALESCE(
          MAX(transition.transition_seq),
          generation.transition_floor_seq
        )
        FROM radroots_event_store_source_generation AS generation
        LEFT JOIN radroots_event_store_addressable_head_transition AS transition
          ON transition.source_generation = NEW.active_generation
        WHERE generation.source_generation = NEW.active_generation
      )
    )
  )
)
BEGIN
  SELECT RAISE(ABORT, 'event-store source authority update is outside its rebuild or append phase');
END;

CREATE TRIGGER radroots_event_store_source_state_delete_guard
BEFORE DELETE ON radroots_event_store_source_state
BEGIN
  SELECT RAISE(ABORT, 'event-store source state cannot be deleted');
END;

CREATE TRIGGER radroots_event_store_write_lock_insert_guard
BEFORE INSERT ON radroots_event_store_write_lock
BEGIN
  SELECT RAISE(ABORT, 'event-store write lock row already exists');
END;

CREATE TRIGGER radroots_event_store_write_lock_update_guard
BEFORE UPDATE ON radroots_event_store_write_lock
WHEN NEW.singleton IS NOT OLD.singleton
OR NEW.lock_version IS NOT OLD.lock_version
BEGIN
  SELECT RAISE(ABORT, 'event-store write lock identity is immutable');
END;

CREATE TRIGGER radroots_event_store_write_lock_delete_guard
BEFORE DELETE ON radroots_event_store_write_lock
BEGIN
  SELECT RAISE(ABORT, 'event-store write lock row is immutable');
END;

CREATE TRIGGER radroots_event_store_source_rebuild_commit_barrier_insert_guard
BEFORE INSERT ON radroots_event_store_source_rebuild_commit_barrier
BEGIN
  SELECT RAISE(ABORT, 'event-store rebuild commit barrier must remain empty');
END;

CREATE TRIGGER radroots_event_store_source_rebuild_commit_barrier_update_guard
BEFORE UPDATE ON radroots_event_store_source_rebuild_commit_barrier
BEGIN
  SELECT RAISE(ABORT, 'event-store rebuild commit barrier must remain empty');
END;

CREATE TRIGGER radroots_event_store_source_rebuild_commit_barrier_delete_guard
BEFORE DELETE ON radroots_event_store_source_rebuild_commit_barrier
BEGIN
  SELECT RAISE(ABORT, 'event-store rebuild commit barrier must remain empty');
END;

CREATE TRIGGER radroots_event_store_event_coordinate_update_guard
BEFORE UPDATE ON radroots_event_store_event_coordinate
BEGIN
  SELECT RAISE(ABORT, 'event-store coordinate facts are immutable');
END;

CREATE TRIGGER radroots_event_store_event_coordinate_delete_guard
BEFORE DELETE ON radroots_event_store_event_coordinate
BEGIN
  SELECT RAISE(ABORT, 'event-store coordinate facts are immutable');
END;

CREATE TRIGGER radroots_event_store_nip09_request_update_guard
BEFORE UPDATE ON radroots_event_store_nip09_request
BEGIN
  SELECT RAISE(ABORT, 'event-store NIP-09 request facts are immutable');
END;

CREATE TRIGGER radroots_event_store_nip09_request_delete_guard
BEFORE DELETE ON radroots_event_store_nip09_request
BEGIN
  SELECT RAISE(ABORT, 'event-store NIP-09 request facts are immutable');
END;

CREATE TRIGGER radroots_event_store_nip09_event_target_update_guard
BEFORE UPDATE ON radroots_event_store_nip09_event_target
BEGIN
  SELECT RAISE(ABORT, 'event-store NIP-09 event targets are immutable');
END;

CREATE TRIGGER radroots_event_store_nip09_event_target_delete_guard
BEFORE DELETE ON radroots_event_store_nip09_event_target
BEGIN
  SELECT RAISE(ABORT, 'event-store NIP-09 event targets are immutable');
END;

CREATE TRIGGER radroots_event_store_nip09_address_target_update_guard
BEFORE UPDATE ON radroots_event_store_nip09_address_target
BEGIN
  SELECT RAISE(ABORT, 'event-store NIP-09 address targets are immutable');
END;

CREATE TRIGGER radroots_event_store_nip09_address_target_delete_guard
BEFORE DELETE ON radroots_event_store_nip09_address_target
BEGIN
  SELECT RAISE(ABORT, 'event-store NIP-09 address targets are immutable');
END;

CREATE TRIGGER radroots_event_store_addressable_transition_update_guard
BEFORE UPDATE ON radroots_event_store_addressable_head_transition
BEGIN
  SELECT RAISE(ABORT, 'event-store addressable head transitions are immutable');
END;

CREATE TRIGGER radroots_event_store_addressable_transition_delete_guard
BEFORE DELETE ON radroots_event_store_addressable_head_transition
BEGIN
  SELECT RAISE(ABORT, 'event-store addressable head transitions are immutable');
END;

CREATE TRIGGER radroots_event_store_addressable_state_identity_update_guard
BEFORE UPDATE OF source_generation, kind, pubkey, d_tag
ON radroots_event_store_addressable_head_state
BEGIN
  SELECT RAISE(ABORT, 'event-store addressable head state identity is immutable');
END;

CREATE TRIGGER radroots_event_store_addressable_state_old_update_guard
BEFORE UPDATE ON radroots_event_store_addressable_head_state
WHEN OLD.source_generation IS NOT (
  SELECT active_generation
  FROM radroots_event_store_source_state
  WHERE singleton = 1
)
OR NOT EXISTS (
  SELECT 1
  FROM radroots_event_store_addressable_head_transition AS transition
  WHERE transition.source_generation = NEW.source_generation
    AND transition.kind = NEW.kind
    AND transition.pubkey = NEW.pubkey
    AND transition.d_tag = NEW.d_tag
    AND transition.transition_seq = (
      SELECT MAX(candidate.transition_seq)
      FROM radroots_event_store_addressable_head_transition AS candidate
      WHERE candidate.source_generation = NEW.source_generation
        AND candidate.kind = NEW.kind
        AND candidate.pubkey = NEW.pubkey
        AND candidate.d_tag = NEW.d_tag
    )
    AND transition.raw_head_event_id IS NEW.raw_head_event_id
    AND transition.raw_head_event_seq IS NEW.raw_head_event_seq
    AND transition.raw_head_created_at IS NEW.raw_head_created_at
    AND transition.admission_status IS NEW.admission_status
    AND transition.admission_code IS NEW.admission_code
    AND transition.contract_id IS NEW.contract_id
    AND transition.visibility IS NEW.visibility
    AND transition.nip09_outcome IS NEW.nip09_outcome
    AND transition.nip09_reason IS NEW.nip09_reason
    AND transition.event_reference_request_id IS NEW.event_reference_request_id
    AND transition.address_reference_request_id IS NEW.address_reference_request_id
    AND transition.address_reference_cutoff IS NEW.address_reference_cutoff
    AND transition.origin IS NEW.last_origin
    AND transition.cause_event_seq IS NEW.last_cause_event_seq
    AND transition.cause_event_id IS NEW.last_cause_event_id
)
BEGIN
  SELECT CASE
    WHEN OLD.source_generation IS NOT (
      SELECT active_generation
      FROM radroots_event_store_source_state
      WHERE singleton = 1
    )
    THEN RAISE(ABORT, 'event-store historical addressable head state is immutable')
    ELSE RAISE(ABORT, 'event-store addressable head state must match its latest transition')
  END;
END;

CREATE TRIGGER radroots_event_store_addressable_state_delete_guard
BEFORE DELETE ON radroots_event_store_addressable_head_state
BEGIN
  SELECT RAISE(ABORT, 'event-store addressable head state is immutable');
END;

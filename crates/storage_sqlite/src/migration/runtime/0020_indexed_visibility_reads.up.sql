ALTER TABLE radroots_runtime_visibility_progress
ADD COLUMN raw_events INTEGER NOT NULL DEFAULT 0 CHECK (raw_events >= 0);
ALTER TABLE radroots_runtime_visibility_progress
ADD COLUMN verified_events INTEGER NOT NULL DEFAULT 0 CHECK (verified_events >= 0);
ALTER TABLE radroots_runtime_visibility_progress
ADD COLUMN visible_events INTEGER NOT NULL DEFAULT 0 CHECK (visible_events >= 0);

-- A one-time forward upgrade rebuilds membership through bounded source work.
-- No event payload is decoded or updated by migration.
UPDATE radroots_runtime_visibility_progress
SET backfill_target = (SELECT sequence_head FROM radroots_runtime_source_generations
                       WHERE generation = source_generation),
    backfill_sequence = 0, replay_epoch = 0,
    replay_sequence = 9223372036854775807, ready = 0;

CREATE TABLE radroots_runtime_visibility_members (
  source_generation BLOB NOT NULL REFERENCES radroots_runtime_visibility_progress(source_generation),
  source_sequence INTEGER NOT NULL CHECK (source_sequence > 0),
  event_id BLOB NOT NULL UNIQUE REFERENCES radroots_runtime_visibility_events(event_id),
  admission_stage TEXT NOT NULL CHECK (admission_stage IN ('raw', 'verified', 'visible')),
  visible INTEGER NOT NULL CHECK (visible IN (0, 1)),
  PRIMARY KEY (source_generation, source_sequence)
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_visibility_members_page_idx
ON radroots_runtime_visibility_members(source_generation, visible, source_sequence);

CREATE UNIQUE INDEX radroots_runtime_visibility_members_event_idx
ON radroots_runtime_visibility_members(source_generation, event_id);

CREATE TABLE radroots_runtime_visibility_heads (
  source_generation BLOB NOT NULL REFERENCES radroots_runtime_visibility_progress(source_generation),
  head_coordinate BLOB NOT NULL,
  source_sequence INTEGER NOT NULL CHECK (source_sequence > 0),
  event_id BLOB NOT NULL REFERENCES radroots_runtime_visibility_events(event_id),
  PRIMARY KEY (source_generation, head_coordinate)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_visibility_pending (
  source_generation BLOB NOT NULL REFERENCES radroots_runtime_visibility_progress(source_generation),
  target_type INTEGER NOT NULL CHECK (target_type IN (0, 1)),
  target_key BLOB NOT NULL CHECK (
    (target_type = 0 AND length(target_key) = 32) OR
    (target_type = 1 AND length(target_key) BETWEEN 1 AND 4096)),
  PRIMARY KEY (source_generation, target_type, target_key)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER radroots_runtime_visibility_members_count_insert
AFTER INSERT ON radroots_runtime_visibility_members
BEGIN
  UPDATE radroots_runtime_visibility_progress
  SET raw_events = raw_events + 1,
      verified_events = verified_events + (NEW.admission_stage != 'raw'),
      visible_events = visible_events + NEW.visible
  WHERE source_generation = NEW.source_generation;
END;

CREATE TRIGGER radroots_runtime_visibility_members_count_update
AFTER UPDATE OF admission_stage, visible ON radroots_runtime_visibility_members
BEGIN
  UPDATE radroots_runtime_visibility_progress
  SET verified_events = verified_events + (NEW.admission_stage != 'raw') - (OLD.admission_stage != 'raw'),
      visible_events = visible_events + NEW.visible - OLD.visible
  WHERE source_generation = NEW.source_generation;
END;

CREATE TRIGGER radroots_runtime_visibility_empty_source
AFTER INSERT ON radroots_runtime_source_generations
WHEN NEW.sequence_head = 0 AND NEW.mutation_epoch = 0
BEGIN
  INSERT INTO radroots_runtime_visibility_progress
  (source_generation, backfill_target, backfill_sequence, replay_epoch, replay_sequence, ready)
  VALUES (NEW.generation, 0, 0, 0, 9223372036854775807, 1);
END;

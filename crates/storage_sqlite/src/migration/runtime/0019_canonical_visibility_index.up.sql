ALTER TABLE radroots_runtime_events
ADD COLUMN last_mutation_epoch INTEGER NOT NULL DEFAULT 0 CHECK (last_mutation_epoch >= 0);

CREATE INDEX radroots_runtime_events_mutation_idx
ON radroots_runtime_events(source_generation, last_mutation_epoch, source_sequence);

CREATE TABLE radroots_runtime_visibility_progress (
  source_generation BLOB PRIMARY KEY NOT NULL
    REFERENCES radroots_runtime_source_generations(generation),
  schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version = 1),
  backfill_target INTEGER NOT NULL CHECK (backfill_target >= 0),
  backfill_sequence INTEGER NOT NULL DEFAULT 0 CHECK (backfill_sequence >= 0),
  replay_epoch INTEGER NOT NULL CHECK (replay_epoch >= 0),
  replay_sequence INTEGER NOT NULL CHECK (replay_sequence >= 0),
  ready INTEGER NOT NULL DEFAULT 0 CHECK (ready IN (0, 1)),
  CHECK (backfill_sequence <= backfill_target)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_visibility_events (
  source_generation BLOB NOT NULL
    REFERENCES radroots_runtime_source_generations(generation),
  source_sequence INTEGER NOT NULL CHECK (source_sequence > 0),
  event_id BLOB NOT NULL UNIQUE REFERENCES radroots_runtime_events(event_id),
  last_mutation_epoch INTEGER NOT NULL CHECK (last_mutation_epoch >= 0),
  author BLOB NOT NULL CHECK (length(author) = 32),
  created_at BLOB NOT NULL CHECK (length(created_at) = 8),
  admission_stage TEXT NOT NULL CHECK (admission_stage IN ('raw', 'verified', 'visible')),
  head_coordinate BLOB,
  deletion_coordinate TEXT,
  ephemeral INTEGER NOT NULL CHECK (ephemeral IN (0, 1)),
  deletion_request INTEGER NOT NULL CHECK (deletion_request IN (0, 1)),
  PRIMARY KEY (source_generation, source_sequence)
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_visibility_events_head_idx
ON radroots_runtime_visibility_events(
  source_generation, head_coordinate, created_at DESC, event_id ASC
);

CREATE INDEX radroots_runtime_visibility_events_stage_idx
ON radroots_runtime_visibility_events(source_generation, admission_stage, source_sequence);

CREATE TABLE radroots_runtime_visibility_deletion_events (
  source_generation BLOB NOT NULL,
  target_event BLOB NOT NULL CHECK (length(target_event) = 32),
  author BLOB NOT NULL CHECK (length(author) = 32),
  request_id BLOB NOT NULL
    REFERENCES radroots_runtime_visibility_events(event_id) ON DELETE CASCADE,
  PRIMARY KEY (source_generation, target_event, author, request_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_visibility_deletion_events_request_idx
ON radroots_runtime_visibility_deletion_events(request_id);

CREATE TABLE radroots_runtime_visibility_deletion_addresses (
  source_generation BLOB NOT NULL,
  target_coordinate TEXT NOT NULL,
  author BLOB NOT NULL CHECK (length(author) = 32),
  created_at BLOB NOT NULL CHECK (length(created_at) = 8),
  request_id BLOB NOT NULL
    REFERENCES radroots_runtime_visibility_events(event_id) ON DELETE CASCADE,
  PRIMARY KEY (source_generation, target_coordinate, author, created_at, request_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_visibility_deletion_addresses_request_idx
ON radroots_runtime_visibility_deletion_addresses(request_id);

DROP TRIGGER radroots_runtime_events_mutation_insert;
DROP TRIGGER radroots_runtime_events_mutation_update;

CREATE TRIGGER radroots_runtime_events_mutation_insert
AFTER INSERT ON radroots_runtime_events
BEGIN
  SELECT CASE WHEN (
    SELECT mutation_epoch FROM radroots_runtime_source_generations
    WHERE generation = NEW.source_generation
  ) = 9223372036854775807
  THEN RAISE(ABORT, 'runtime event mutation epoch overflow') END;
  UPDATE radroots_runtime_source_generations
  SET mutation_epoch = mutation_epoch + 1
  WHERE generation = NEW.source_generation;
  UPDATE radroots_runtime_events SET last_mutation_epoch = (
    SELECT mutation_epoch FROM radroots_runtime_source_generations
    WHERE generation = NEW.source_generation
  ) WHERE event_id = NEW.event_id;
  UPDATE radroots_runtime_visibility_progress SET ready = 0
  WHERE source_generation = NEW.source_generation;
END;

CREATE TRIGGER radroots_runtime_events_mutation_update
AFTER UPDATE OF admission_stage, admitted_contract_id, admitted_registry_version
ON radroots_runtime_events
WHEN NEW.admission_stage IS NOT OLD.admission_stage
  OR NEW.admitted_contract_id IS NOT OLD.admitted_contract_id
  OR NEW.admitted_registry_version IS NOT OLD.admitted_registry_version
BEGIN
  SELECT CASE WHEN (
    SELECT mutation_epoch FROM radroots_runtime_source_generations
    WHERE generation = NEW.source_generation
  ) = 9223372036854775807
  THEN RAISE(ABORT, 'runtime event mutation epoch overflow') END;
  UPDATE radroots_runtime_source_generations
  SET mutation_epoch = mutation_epoch + 1
  WHERE generation = NEW.source_generation;
  UPDATE radroots_runtime_events SET last_mutation_epoch = (
    SELECT mutation_epoch FROM radroots_runtime_source_generations
    WHERE generation = NEW.source_generation
  ) WHERE event_id = NEW.event_id;
  UPDATE radroots_runtime_visibility_progress SET ready = 0
  WHERE source_generation = NEW.source_generation;
END;

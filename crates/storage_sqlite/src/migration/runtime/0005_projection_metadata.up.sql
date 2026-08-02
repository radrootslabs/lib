DROP TABLE radroots_runtime_event_index_checkpoints;
DROP TABLE radroots_runtime_event_index_shards;
DROP TABLE radroots_runtime_event_index_manifests;
DROP INDEX radroots_runtime_projection_rebuilds_stage_idx;
DROP TABLE radroots_runtime_projection_rebuilds;
DROP TABLE radroots_runtime_projection_invalidations;
DROP TABLE radroots_runtime_projection_checkpoints;

CREATE TABLE radroots_runtime_projection_checkpoints (
  projection_id TEXT PRIMARY KEY NOT NULL CHECK (length(projection_id) BETWEEN 1 AND 128),
  projection_generation BLOB NOT NULL CHECK (length(projection_generation) = 32),
  health TEXT NOT NULL CHECK (health IN ('ready', 'invalidated', 'rebuilding', 'failed')),
  source_generation BLOB,
  source_sequence INTEGER,
  projected_rows INTEGER,
  checkpoint_updated_at_unix_ms INTEGER,
  active_rebuild BLOB CHECK (active_rebuild IS NULL OR length(active_rebuild) = 16),
  CHECK (
    (projected_rows IS NULL AND checkpoint_updated_at_unix_ms IS NULL
      AND source_generation IS NULL AND source_sequence IS NULL)
    OR (projected_rows >= 0 AND checkpoint_updated_at_unix_ms > 0
      AND ((source_generation IS NULL AND source_sequence IS NULL)
        OR (length(source_generation) = 32 AND source_sequence > 0)))
  ),
  CHECK ((health = 'rebuilding') = (active_rebuild IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_projection_invalidations (
  projection_id TEXT NOT NULL CHECK (length(projection_id) BETWEEN 1 AND 128),
  invalid_generation BLOB NOT NULL CHECK (length(invalid_generation) = 32),
  replacement_generation BLOB NOT NULL CHECK (length(replacement_generation) = 32),
  reason TEXT NOT NULL CHECK (reason IN ('source_generation_changed', 'projection_generation_changed', 'event_index_manifest_changed', 'integrity_failure', 'operator_requested')),
  invalidated_at_unix_ms INTEGER NOT NULL CHECK (invalidated_at_unix_ms > 0),
  PRIMARY KEY (projection_id, invalid_generation),
  CHECK (invalid_generation <> replacement_generation)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_projection_rebuilds (
  ticket_id BLOB PRIMARY KEY NOT NULL CHECK (length(ticket_id) = 16),
  projection_id TEXT NOT NULL,
  invalid_generation BLOB NOT NULL,
  replacement_generation BLOB NOT NULL CHECK (length(replacement_generation) = 32),
  revision INTEGER NOT NULL CHECK (revision > 0),
  stage TEXT NOT NULL CHECK (stage IN ('requested', 'running', 'completed', 'failed')),
  checkpoint_source_generation BLOB,
  checkpoint_source_sequence INTEGER,
  checkpoint_projected_rows INTEGER,
  checkpoint_updated_at_unix_ms INTEGER,
  requested_at_unix_ms INTEGER NOT NULL CHECK (requested_at_unix_ms > 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= requested_at_unix_ms),
  FOREIGN KEY (projection_id, invalid_generation)
    REFERENCES radroots_runtime_projection_invalidations(projection_id, invalid_generation),
  CHECK (
    (checkpoint_projected_rows IS NULL AND checkpoint_updated_at_unix_ms IS NULL
      AND checkpoint_source_generation IS NULL AND checkpoint_source_sequence IS NULL)
    OR (checkpoint_projected_rows >= 0 AND checkpoint_updated_at_unix_ms > 0
      AND ((checkpoint_source_generation IS NULL AND checkpoint_source_sequence IS NULL)
        OR (length(checkpoint_source_generation) = 32 AND checkpoint_source_sequence > 0)))
  )
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_projection_rebuilds_stage_idx
ON radroots_runtime_projection_rebuilds(stage, updated_at_unix_ms, ticket_id);

CREATE TABLE radroots_runtime_event_index_manifests (
  projection_generation BLOB PRIMARY KEY NOT NULL CHECK (length(projection_generation) = 32),
  total_events INTEGER NOT NULL CHECK (total_events > 0),
  target_shard_size INTEGER NOT NULL CHECK (target_shard_size > 0),
  first_published_at_unix_s INTEGER NOT NULL CHECK (first_published_at_unix_s > 0),
  last_published_at_unix_s INTEGER NOT NULL CHECK (last_published_at_unix_s >= first_published_at_unix_s)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_event_index_shards (
  projection_generation BLOB NOT NULL
    REFERENCES radroots_runtime_event_index_manifests(projection_generation) ON DELETE CASCADE,
  shard_id TEXT NOT NULL CHECK (length(shard_id) BETWEEN 1 AND 128),
  ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
  artifact_path TEXT NOT NULL CHECK (length(artifact_path) BETWEEN 1 AND 512),
  event_count INTEGER NOT NULL CHECK (event_count > 0),
  first_event_id BLOB NOT NULL CHECK (length(first_event_id) = 32),
  last_event_id BLOB NOT NULL CHECK (length(last_event_id) = 32),
  first_published_at_unix_s INTEGER NOT NULL CHECK (first_published_at_unix_s > 0),
  last_published_at_unix_s INTEGER NOT NULL CHECK (last_published_at_unix_s >= first_published_at_unix_s),
  artifact_digest BLOB NOT NULL CHECK (length(artifact_digest) = 32),
  PRIMARY KEY (projection_generation, shard_id),
  UNIQUE (projection_generation, ordinal),
  UNIQUE (projection_generation, artifact_path)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_event_index_checkpoints (
  projection_generation BLOB PRIMARY KEY NOT NULL CHECK (length(projection_generation) = 32),
  generated_at_unix_ms INTEGER NOT NULL CHECK (generated_at_unix_ms > 0),
  checkpoint BLOB NOT NULL CHECK (length(checkpoint) > 0)
) STRICT, WITHOUT ROWID;

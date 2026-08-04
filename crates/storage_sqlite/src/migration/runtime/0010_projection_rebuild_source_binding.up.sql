DROP INDEX radroots_runtime_projection_rebuilds_stage_idx;
ALTER TABLE radroots_runtime_projection_rebuilds
RENAME TO radroots_runtime_projection_rebuilds_v9;

CREATE TABLE radroots_runtime_projection_rebuilds (
  ticket_id BLOB PRIMARY KEY NOT NULL CHECK (length(ticket_id) = 16),
  projection_id TEXT NOT NULL,
  invalid_generation BLOB NOT NULL,
  replacement_generation BLOB NOT NULL CHECK (length(replacement_generation) = 32),
  revision INTEGER NOT NULL CHECK (revision > 0),
  stage TEXT NOT NULL CHECK (stage IN ('requested', 'running', 'completed', 'failed')),
  source_generation BLOB NOT NULL CHECK (length(source_generation) = 32),
  source_sequence INTEGER CHECK (source_sequence > 0),
  source_digest BLOB NOT NULL CHECK (length(source_digest) = 32),
  checkpoint_source_generation BLOB,
  checkpoint_source_sequence INTEGER,
  checkpoint_projected_rows INTEGER,
  checkpoint_updated_at_unix_ms INTEGER,
  failure TEXT CHECK (failure IN (
    'reducer_rejected', 'source_changed', 'integrity_failure', 'promotion_rejected'
  )),
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
  ),
  CHECK ((stage = 'failed') = (failure IS NOT NULL))
) STRICT, WITHOUT ROWID;

INSERT INTO radroots_runtime_projection_rebuilds (
  ticket_id, projection_id, invalid_generation, replacement_generation, revision, stage,
  source_generation, source_sequence, source_digest, checkpoint_source_generation, checkpoint_source_sequence,
  checkpoint_projected_rows, checkpoint_updated_at_unix_ms, failure,
  requested_at_unix_ms, updated_at_unix_ms
)
SELECT
  ticket_id, projection_id, invalid_generation, replacement_generation, revision, stage,
  COALESCE(
    checkpoint_source_generation,
    (SELECT generation FROM radroots_runtime_source_generations WHERE state = 'active')
  ),
  checkpoint_source_sequence,
  zeroblob(32),
  checkpoint_source_generation, checkpoint_source_sequence, checkpoint_projected_rows,
  checkpoint_updated_at_unix_ms,
  CASE WHEN stage = 'failed' THEN 'integrity_failure' ELSE NULL END,
  requested_at_unix_ms, updated_at_unix_ms
FROM radroots_runtime_projection_rebuilds_v9;

DROP TABLE radroots_runtime_projection_rebuilds_v9;

CREATE INDEX radroots_runtime_projection_rebuilds_stage_idx
ON radroots_runtime_projection_rebuilds(stage, updated_at_unix_ms, ticket_id);

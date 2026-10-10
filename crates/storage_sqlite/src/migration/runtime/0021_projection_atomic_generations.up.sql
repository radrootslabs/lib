CREATE TABLE radroots_runtime_projection_generations (
  projection_id TEXT NOT NULL CHECK(length(projection_id) BETWEEN 1 AND 128),
  generation BLOB NOT NULL CHECK(length(generation) = 32),
  source_generation BLOB NOT NULL CHECK(length(source_generation) = 32),
  source_epoch INTEGER NOT NULL CHECK(source_epoch >= 0),
  expected_active BLOB CHECK(expected_active IS NULL OR length(expected_active) = 32),
  phase TEXT NOT NULL CHECK(phase IN ('staging', 'promoted', 'retired')),
  document_rows INTEGER NOT NULL DEFAULT 0 CHECK(document_rows >= 0),
  value_bytes INTEGER NOT NULL DEFAULT 0 CHECK(value_bytes >= 0),
  PRIMARY KEY(projection_id, generation)
) STRICT, WITHOUT ROWID;

CREATE UNIQUE INDEX radroots_runtime_projection_generations_staging_idx
ON radroots_runtime_projection_generations(projection_id) WHERE phase = 'staging';

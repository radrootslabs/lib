CREATE TABLE radroots_runtime_projection_documents (
  projection_id TEXT NOT NULL CHECK(length(projection_id) BETWEEN 1 AND 128),
  generation BLOB NOT NULL CHECK(length(generation) = 32),
  document_key TEXT NOT NULL CHECK(length(document_key) BETWEEN 1 AND 512),
  value BLOB NOT NULL CHECK(length(value) BETWEEN 1 AND 16777216),
  value_sha256 BLOB NOT NULL CHECK(length(value_sha256) = 32),
  PRIMARY KEY(projection_id, generation, document_key)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_projection_snapshots (
  projection_id TEXT NOT NULL CHECK(length(projection_id) BETWEEN 1 AND 128),
  snapshot_id BLOB NOT NULL CHECK(length(snapshot_id) = 32),
  generation BLOB NOT NULL CHECK(length(generation) = 32),
  created_at_unix_ms INTEGER NOT NULL CHECK(created_at_unix_ms > 0),
  value BLOB NOT NULL CHECK(length(value) BETWEEN 1 AND 16777216),
  value_sha256 BLOB NOT NULL CHECK(length(value_sha256) = 32),
  PRIMARY KEY(projection_id, snapshot_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_projection_snapshots_created_idx
ON radroots_runtime_projection_snapshots(projection_id, created_at_unix_ms, snapshot_id);

CREATE TRIGGER radroots_runtime_projection_snapshots_update_guard
BEFORE UPDATE ON radroots_runtime_projection_snapshots
BEGIN
  SELECT RAISE(ABORT, 'projection snapshots are immutable');
END;

CREATE TABLE radroots_runtime_authored_draft_revisions (
  draft_id BLOB NOT NULL CHECK(length(draft_id) = 16),
  revision INTEGER NOT NULL CHECK(revision > 0),
  author BLOB NOT NULL CHECK(length(author) = 32),
  stage INTEGER NOT NULL CHECK(stage BETWEEN 0 AND 5),
  operation_id BLOB CHECK(operation_id IS NULL OR length(operation_id) = 16),
  payload_sha256 BLOB NOT NULL CHECK(length(payload_sha256) = 32),
  created_at_unix_ms INTEGER NOT NULL CHECK(created_at_unix_ms > 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK(updated_at_unix_ms >= created_at_unix_ms),
  snapshot BLOB NOT NULL CHECK(length(snapshot) BETWEEN 1 AND 16777216),
  PRIMARY KEY(draft_id, revision)
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_authored_draft_author_head_idx
ON radroots_runtime_authored_draft_revisions(author, updated_at_unix_ms DESC, draft_id, revision DESC);

CREATE TRIGGER radroots_runtime_authored_draft_revisions_update_guard
BEFORE UPDATE ON radroots_runtime_authored_draft_revisions
BEGIN
  SELECT RAISE(ABORT, 'authored draft revisions are immutable');
END;

CREATE TRIGGER radroots_runtime_authored_draft_revisions_delete_guard
BEFORE DELETE ON radroots_runtime_authored_draft_revisions
BEGIN
  SELECT RAISE(ABORT, 'authored draft revisions are immutable');
END;

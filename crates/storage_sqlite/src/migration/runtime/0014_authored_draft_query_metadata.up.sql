-- Backfill only generic query metadata; immutable revision snapshots are retained.
DROP TRIGGER radroots_runtime_authored_draft_revisions_update_guard;
ALTER TABLE radroots_runtime_authored_draft_revisions
  ADD COLUMN payload_schema TEXT NOT NULL DEFAULT '' CHECK(length(CAST(payload_schema AS BLOB)) <= 128);
ALTER TABLE radroots_runtime_authored_draft_revisions
  ADD COLUMN payload_scope BLOB CHECK(payload_scope IS NULL OR length(payload_scope) = 32);
UPDATE radroots_runtime_authored_draft_revisions SET payload_schema =
  CASE WHEN json_valid(CAST(snapshot AS TEXT)) THEN
    CASE WHEN json_type(CAST(snapshot AS TEXT), '$.payload_schema') = 'text'
      AND length(CAST(json_extract(CAST(snapshot AS TEXT), '$.payload_schema') AS BLOB)) BETWEEN 1 AND 128
    THEN json_extract(CAST(snapshot AS TEXT), '$.payload_schema') ELSE '' END
  ELSE '' END;
CREATE INDEX radroots_runtime_authored_draft_scope_head_idx
  ON radroots_runtime_authored_draft_revisions(author, payload_schema, payload_scope, draft_id, revision DESC);
CREATE TRIGGER radroots_runtime_authored_draft_revisions_update_guard
BEFORE UPDATE ON radroots_runtime_authored_draft_revisions
BEGIN
  SELECT RAISE(ABORT, 'authored draft revisions are immutable');
END;

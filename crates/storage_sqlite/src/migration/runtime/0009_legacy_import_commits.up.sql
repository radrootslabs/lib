CREATE TABLE radroots_runtime_legacy_import_commits (
  import_id BLOB PRIMARY KEY NOT NULL CHECK(length(import_id) = 16)
    REFERENCES radroots_runtime_legacy_imports(import_id) ON DELETE RESTRICT,
  validation_sha256 BLOB NOT NULL CHECK(length(validation_sha256) = 32),
  imported_row_count INTEGER NOT NULL CHECK(imported_row_count >= 0),
  completed_at_ms INTEGER NOT NULL CHECK(completed_at_ms > 0)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER radroots_runtime_legacy_import_commit_update_guard
BEFORE UPDATE ON radroots_runtime_legacy_import_commits
BEGIN
  SELECT RAISE(ABORT, 'legacy import commit identity is immutable');
END;

CREATE TRIGGER radroots_runtime_legacy_import_commit_delete_guard
BEFORE DELETE ON radroots_runtime_legacy_import_commits
BEGIN
  SELECT RAISE(ABORT, 'legacy import commit evidence is retained');
END;

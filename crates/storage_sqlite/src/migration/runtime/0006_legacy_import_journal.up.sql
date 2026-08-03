CREATE TABLE radroots_runtime_legacy_imports (
  import_id BLOB PRIMARY KEY NOT NULL CHECK(length(import_id) = 16),
  target_generation BLOB NOT NULL UNIQUE CHECK(length(target_generation) = 32)
    REFERENCES radroots_runtime_source_generations(generation) ON DELETE RESTRICT,
  manifest_sha256 BLOB NOT NULL CHECK(length(manifest_sha256) = 32),
  classification_sha256 BLOB NOT NULL CHECK(length(classification_sha256) = 32),
  state TEXT NOT NULL CHECK(state IN ('classified', 'staging', 'ready', 'committing', 'complete')),
  started_at_ms INTEGER NOT NULL CHECK(started_at_ms > 0),
  updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms >= started_at_ms),
  completed_at_ms INTEGER CHECK(completed_at_ms IS NULL OR completed_at_ms >= started_at_ms),
  CHECK((state = 'complete') = (completed_at_ms IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_legacy_import_members (
  import_id BLOB NOT NULL REFERENCES radroots_runtime_legacy_imports(import_id) ON DELETE CASCADE,
  source_kind TEXT NOT NULL CHECK(source_kind IN ('event_store', 'outbox', 'private', 'studio')),
  legacy_schema TEXT NOT NULL CHECK(legacy_schema IN (
    'event_store_v1',
    'event_store_v2',
    'event_store_v3',
    'event_store_v4',
    'outbox_v1',
    'private_v1',
    'studio_v1_host_handoff'
  )),
  disposition TEXT NOT NULL CHECK(disposition IN ('import', 'host_handoff')),
  catalog_sha256 BLOB NOT NULL CHECK(length(catalog_sha256) = 32),
  state TEXT NOT NULL CHECK(state IN ('pending', 'staging', 'ready', 'complete')),
  resume_cursor BLOB,
  staged_row_count INTEGER NOT NULL DEFAULT 0 CHECK(staged_row_count >= 0),
  updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms > 0),
  PRIMARY KEY(import_id, source_kind)
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_legacy_import_state_idx
ON radroots_runtime_legacy_imports(state, updated_at_ms, import_id);

CREATE TRIGGER radroots_runtime_legacy_import_identity_guard
BEFORE UPDATE OF import_id, target_generation, manifest_sha256, classification_sha256, started_at_ms
ON radroots_runtime_legacy_imports
BEGIN
  SELECT RAISE(ABORT, 'legacy import identity is immutable');
END;

CREATE TRIGGER radroots_runtime_legacy_import_delete_guard
BEFORE DELETE ON radroots_runtime_legacy_imports
BEGIN
  SELECT RAISE(ABORT, 'legacy import history is retained');
END;

CREATE TRIGGER radroots_runtime_legacy_import_state_guard
BEFORE UPDATE OF state, updated_at_ms, completed_at_ms
ON radroots_runtime_legacy_imports
WHEN NEW.updated_at_ms < OLD.updated_at_ms
  OR NOT (
    NEW.state = OLD.state
    OR (OLD.state = 'classified' AND NEW.state = 'staging')
    OR (OLD.state = 'staging' AND NEW.state = 'ready')
    OR (OLD.state = 'ready' AND NEW.state = 'committing')
    OR (OLD.state = 'committing' AND NEW.state = 'complete')
  )
BEGIN
  SELECT RAISE(ABORT, 'legacy import state must advance monotonically');
END;

CREATE TRIGGER radroots_runtime_legacy_import_member_identity_guard
BEFORE UPDATE OF import_id, source_kind, legacy_schema, disposition, catalog_sha256
ON radroots_runtime_legacy_import_members
BEGIN
  SELECT RAISE(ABORT, 'legacy import member identity is immutable');
END;

CREATE TRIGGER radroots_runtime_legacy_import_member_delete_guard
BEFORE DELETE ON radroots_runtime_legacy_import_members
BEGIN
  SELECT RAISE(ABORT, 'legacy import member history is retained');
END;

CREATE TRIGGER radroots_runtime_legacy_import_member_state_guard
BEFORE UPDATE OF state, resume_cursor, staged_row_count, updated_at_ms
ON radroots_runtime_legacy_import_members
WHEN NEW.updated_at_ms < OLD.updated_at_ms
  OR NEW.staged_row_count < OLD.staged_row_count
  OR NOT (
    NEW.state = OLD.state
    OR (OLD.state = 'pending' AND NEW.state = 'staging')
    OR (OLD.state = 'staging' AND NEW.state = 'ready')
    OR (OLD.state = 'ready' AND NEW.state = 'complete')
  )
BEGIN
  SELECT RAISE(ABORT, 'legacy import member state must advance monotonically');
END;

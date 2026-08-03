CREATE TABLE radroots_runtime_legacy_event_staging (
  import_id BLOB NOT NULL CHECK(length(import_id) = 16),
  source_kind TEXT NOT NULL DEFAULT 'event_store' CHECK(source_kind = 'event_store'),
  legacy_sequence INTEGER NOT NULL CHECK(legacy_sequence > 0),
  event_id BLOB NOT NULL CHECK(length(event_id) = 32),
  signed_event BLOB NOT NULL CHECK(length(signed_event) > 0),
  legacy_verification_status TEXT NOT NULL
    CHECK(length(legacy_verification_status) BETWEEN 1 AND 64),
  legacy_contract_status TEXT NOT NULL
    CHECK(length(legacy_contract_status) BETWEEN 1 AND 64),
  legacy_projection_eligible INTEGER NOT NULL
    CHECK(legacy_projection_eligible IN (0, 1)),
  legacy_inserted_at_ms INTEGER NOT NULL CHECK(legacy_inserted_at_ms > 0),
  legacy_updated_at_ms INTEGER NOT NULL
    CHECK(legacy_updated_at_ms >= legacy_inserted_at_ms),
  PRIMARY KEY(import_id, legacy_sequence),
  UNIQUE(import_id, event_id),
  FOREIGN KEY(import_id, source_kind)
    REFERENCES radroots_runtime_legacy_import_members(import_id, source_kind)
    ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;

CREATE TRIGGER radroots_runtime_legacy_event_staging_insert_guard
BEFORE INSERT ON radroots_runtime_legacy_event_staging
WHEN NOT EXISTS (
  SELECT 1
  FROM radroots_runtime_legacy_import_members
  WHERE import_id = NEW.import_id
    AND source_kind = 'event_store'
    AND disposition = 'import'
    AND state = 'staging'
)
BEGIN
  SELECT RAISE(ABORT, 'legacy event staging requires an active event-store member');
END;

CREATE TRIGGER radroots_runtime_legacy_event_staging_update_guard
BEFORE UPDATE ON radroots_runtime_legacy_event_staging
BEGIN
  SELECT RAISE(ABORT, 'legacy event staging rows are immutable');
END;

CREATE TRIGGER radroots_runtime_legacy_event_staging_delete_guard
BEFORE DELETE ON radroots_runtime_legacy_event_staging
BEGIN
  SELECT RAISE(ABORT, 'legacy event staging rows are retained until import completion');
END;

CREATE TABLE radroots_private_legacy_import_staging (
  import_id BLOB NOT NULL CHECK(length(import_id) = 16),
  table_kind TEXT NOT NULL CHECK(table_kind IN (
    'metadata', 'wrapped_profile_keys', 'signing_secrets', 'farm_locations',
    'trade_artifacts', 'cursor_keys', 'nip46_sessions', 'rotation_progress'
  )),
  key_cursor TEXT NOT NULL CHECK(length(key_cursor) BETWEEN 1 AND 1024),
  parent_key_version INTEGER CHECK(parent_key_version IS NULL OR parent_key_version > 0),
  record_json BLOB NOT NULL CHECK(length(record_json) > 0),
  PRIMARY KEY(import_id, table_kind, key_cursor),
  CHECK(
    (table_kind IN ('metadata', 'wrapped_profile_keys', 'rotation_progress')
      AND parent_key_version IS NULL)
    OR (table_kind IN (
      'signing_secrets', 'farm_locations', 'trade_artifacts', 'cursor_keys',
      'nip46_sessions'
    ) AND parent_key_version IS NOT NULL)
  )
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_private_legacy_import_staging_parent_idx
ON radroots_private_legacy_import_staging(
  import_id, table_kind, parent_key_version, key_cursor
);

CREATE TRIGGER radroots_private_legacy_import_staging_insert_guard
BEFORE INSERT ON radroots_private_legacy_import_staging
WHEN NEW.parent_key_version IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM radroots_private_legacy_import_staging
  WHERE import_id = NEW.import_id
    AND table_kind = 'wrapped_profile_keys'
    AND key_cursor = printf('%020d', NEW.parent_key_version)
)
BEGIN
  SELECT RAISE(ABORT, 'legacy private staging requires a staged wrapping key');
END;

CREATE TRIGGER radroots_private_legacy_import_staging_update_guard
BEFORE UPDATE ON radroots_private_legacy_import_staging
BEGIN
  SELECT RAISE(ABORT, 'legacy private staging rows are immutable');
END;

CREATE TRIGGER radroots_private_legacy_import_staging_delete_guard
BEFORE DELETE ON radroots_private_legacy_import_staging
BEGIN
  SELECT RAISE(ABORT, 'legacy private staging rows are retained until import completion');
END;

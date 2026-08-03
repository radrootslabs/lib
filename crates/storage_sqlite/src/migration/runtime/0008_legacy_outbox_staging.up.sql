CREATE TABLE radroots_runtime_legacy_outbox_staging (
  import_id BLOB NOT NULL CHECK(length(import_id) = 16),
  source_kind TEXT NOT NULL DEFAULT 'outbox' CHECK(source_kind = 'outbox'),
  table_kind TEXT NOT NULL CHECK(table_kind IN (
    'operations', 'events', 'delivery_plans', 'delivery_targets', 'delivery_attempts'
  )),
  legacy_id INTEGER NOT NULL CHECK(legacy_id > 0),
  parent_legacy_id INTEGER CHECK(parent_legacy_id IS NULL OR parent_legacy_id > 0),
  related_legacy_id INTEGER CHECK(related_legacy_id IS NULL OR related_legacy_id > 0),
  record_json BLOB NOT NULL CHECK(length(record_json) > 0),
  PRIMARY KEY(import_id, table_kind, legacy_id),
  FOREIGN KEY(import_id, source_kind)
    REFERENCES radroots_runtime_legacy_import_members(import_id, source_kind)
    ON DELETE RESTRICT,
  CHECK(
    (table_kind = 'operations' AND parent_legacy_id IS NULL AND related_legacy_id IS NULL)
    OR (table_kind IN ('events', 'delivery_plans', 'delivery_targets')
      AND parent_legacy_id IS NOT NULL AND related_legacy_id IS NULL)
    OR (table_kind = 'delivery_attempts'
      AND parent_legacy_id IS NOT NULL AND related_legacy_id IS NOT NULL)
  )
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_legacy_outbox_staging_parent_idx
ON radroots_runtime_legacy_outbox_staging(
  import_id, table_kind, parent_legacy_id, legacy_id
);

CREATE TRIGGER radroots_runtime_legacy_outbox_staging_insert_guard
BEFORE INSERT ON radroots_runtime_legacy_outbox_staging
WHEN NOT EXISTS (
  SELECT 1 FROM radroots_runtime_legacy_import_members
  WHERE import_id = NEW.import_id
    AND source_kind = 'outbox'
    AND disposition = 'import'
    AND state = 'staging'
)
OR (NEW.table_kind = 'events' AND NOT EXISTS (
  SELECT 1 FROM radroots_runtime_legacy_outbox_staging
  WHERE import_id = NEW.import_id AND table_kind = 'operations'
    AND legacy_id = NEW.parent_legacy_id
))
OR (NEW.table_kind = 'delivery_plans' AND NOT EXISTS (
  SELECT 1 FROM radroots_runtime_legacy_outbox_staging
  WHERE import_id = NEW.import_id AND table_kind = 'events'
    AND legacy_id = NEW.parent_legacy_id
))
OR (NEW.table_kind = 'delivery_targets' AND NOT EXISTS (
  SELECT 1 FROM radroots_runtime_legacy_outbox_staging
  WHERE import_id = NEW.import_id AND table_kind = 'delivery_plans'
    AND legacy_id = NEW.parent_legacy_id
))
OR (NEW.table_kind = 'delivery_attempts' AND (
  NOT EXISTS (
    SELECT 1 FROM radroots_runtime_legacy_outbox_staging
    WHERE import_id = NEW.import_id AND table_kind = 'delivery_targets'
      AND legacy_id = NEW.parent_legacy_id
      AND parent_legacy_id = NEW.related_legacy_id
  )
  OR NOT EXISTS (
    SELECT 1 FROM radroots_runtime_legacy_outbox_staging
    WHERE import_id = NEW.import_id AND table_kind = 'delivery_plans'
      AND legacy_id = NEW.related_legacy_id
  )
))
BEGIN
  SELECT RAISE(ABORT, 'legacy outbox staging requires ordered valid graph references');
END;

CREATE TRIGGER radroots_runtime_legacy_outbox_staging_update_guard
BEFORE UPDATE ON radroots_runtime_legacy_outbox_staging
BEGIN
  SELECT RAISE(ABORT, 'legacy outbox staging rows are immutable');
END;

CREATE TRIGGER radroots_runtime_legacy_outbox_staging_delete_guard
BEFORE DELETE ON radroots_runtime_legacy_outbox_staging
BEGIN
  SELECT RAISE(ABORT, 'legacy outbox staging rows are retained until import completion');
END;

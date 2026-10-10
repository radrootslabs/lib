CREATE TABLE radroots_runtime_projection_cache_budget (
  singleton INTEGER PRIMARY KEY CHECK(singleton=1),
  value_bytes INTEGER NOT NULL CHECK(value_bytes>=0),
  protected_generations INTEGER NOT NULL CHECK(protected_generations>=0),
  legacy_snapshots INTEGER NOT NULL CHECK(legacy_snapshots>=0),
  max_value_bytes INTEGER CHECK(max_value_bytes>0),
  max_protected_generations INTEGER CHECK(max_protected_generations>0),
  max_legacy_snapshots INTEGER CHECK(max_legacy_snapshots>0),
  legacy_age_ms INTEGER CHECK(legacy_age_ms>0),
  batch_value_ceiling INTEGER CHECK(batch_value_ceiling>=0),
  owner_time_ms INTEGER NOT NULL DEFAULT 0 CHECK(owner_time_ms>=0)
) STRICT;
CREATE TABLE radroots_runtime_projection_cache_owners (
  projection_id TEXT PRIMARY KEY CHECK(length(projection_id) BETWEEN 1 AND 128)
) STRICT, WITHOUT ROWID;
INSERT INTO radroots_runtime_projection_cache_budget(singleton,value_bytes,protected_generations,legacy_snapshots)
SELECT 1,
  (SELECT coalesce(sum(length(value)),0) FROM radroots_runtime_projection_documents)
    +(SELECT coalesce(sum(length(value)),0) FROM radroots_runtime_projection_snapshots),
  (SELECT count(*) FROM radroots_runtime_projection_generations WHERE phase!='retired')
    +(SELECT count(*) FROM radroots_runtime_projection_checkpoints AS c WHERE NOT EXISTS
       (SELECT 1 FROM radroots_runtime_projection_generations AS g WHERE g.projection_id=c.projection_id AND g.generation=c.projection_generation)),
  (SELECT count(*) FROM radroots_runtime_projection_snapshots);
CREATE INDEX radroots_runtime_projection_snapshots_generation_created_idx
ON radroots_runtime_projection_snapshots(projection_id,generation,created_at_unix_ms,snapshot_id);
CREATE TRIGGER radroots_runtime_projection_cache_document_insert AFTER INSERT ON radroots_runtime_projection_documents
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET value_bytes=value_bytes+(length(NEW.value)) WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_document_delete AFTER DELETE ON radroots_runtime_projection_documents
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET value_bytes=value_bytes+(-length(OLD.value)) WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_document_update AFTER UPDATE OF value ON radroots_runtime_projection_documents
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET value_bytes=value_bytes+(length(NEW.value)-length(OLD.value)) WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_snapshot_insert AFTER INSERT ON radroots_runtime_projection_snapshots
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET value_bytes=value_bytes+(length(NEW.value)),legacy_snapshots=legacy_snapshots+(1) WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_snapshot_delete AFTER DELETE ON radroots_runtime_projection_snapshots
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET value_bytes=value_bytes+(-length(OLD.value)),legacy_snapshots=legacy_snapshots+(-1) WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_generation_insert AFTER INSERT ON radroots_runtime_projection_generations
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET protected_generations=protected_generations+(NEW.phase!='retired')-(EXISTS(SELECT 1 FROM radroots_runtime_projection_checkpoints WHERE projection_id=NEW.projection_id AND projection_generation=NEW.generation)) WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_generation_delete AFTER DELETE ON radroots_runtime_projection_generations
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET protected_generations=protected_generations-(OLD.phase!='retired')+(EXISTS(SELECT 1 FROM radroots_runtime_projection_checkpoints WHERE projection_id=OLD.projection_id AND projection_generation=OLD.generation)) WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_generation_update AFTER UPDATE OF phase ON radroots_runtime_projection_generations
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET protected_generations=protected_generations+(NEW.phase!='retired')-(OLD.phase!='retired') WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_checkpoint_insert AFTER INSERT ON radroots_runtime_projection_checkpoints
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET protected_generations=protected_generations+NOT(EXISTS(SELECT 1 FROM radroots_runtime_projection_generations WHERE projection_id=NEW.projection_id AND generation=NEW.projection_generation)) WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_checkpoint_delete AFTER DELETE ON radroots_runtime_projection_checkpoints
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET protected_generations=protected_generations-NOT(EXISTS(SELECT 1 FROM radroots_runtime_projection_generations WHERE projection_id=OLD.projection_id AND generation=OLD.projection_generation)) WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_checkpoint_update AFTER UPDATE OF projection_generation ON radroots_runtime_projection_checkpoints
BEGIN
  UPDATE radroots_runtime_projection_cache_budget SET protected_generations=protected_generations+NOT(EXISTS(SELECT 1 FROM radroots_runtime_projection_generations WHERE projection_id=NEW.projection_id AND generation=NEW.projection_generation))-NOT(EXISTS(SELECT 1 FROM radroots_runtime_projection_generations WHERE projection_id=OLD.projection_id AND generation=OLD.projection_generation)) WHERE singleton=1;
END;
CREATE TRIGGER radroots_runtime_projection_cache_capacity_guard BEFORE UPDATE ON radroots_runtime_projection_cache_budget
WHEN (NEW.value_bytes>OLD.value_bytes AND NEW.value_bytes>coalesce(NEW.batch_value_ceiling,NEW.max_value_bytes))
  OR (NEW.protected_generations>OLD.protected_generations AND NEW.protected_generations>NEW.max_protected_generations)
  OR (NEW.legacy_snapshots>OLD.legacy_snapshots AND NEW.legacy_snapshots>NEW.max_legacy_snapshots)
BEGIN SELECT RAISE(ABORT,'projection cache capacity exceeded'); END;

ALTER TABLE radroots_runtime_source_generations
ADD COLUMN mutation_epoch INTEGER NOT NULL DEFAULT 0 CHECK (mutation_epoch >= 0);

UPDATE radroots_runtime_source_generations
SET mutation_epoch = 1
WHERE EXISTS (
  SELECT 1 FROM radroots_runtime_events
  WHERE source_generation = generation
);

CREATE TRIGGER radroots_runtime_source_generations_mutation_guard
BEFORE UPDATE OF mutation_epoch ON radroots_runtime_source_generations
WHEN NEW.mutation_epoch < OLD.mutation_epoch
BEGIN
  SELECT RAISE(ABORT, 'runtime source mutation epoch cannot regress');
END;

CREATE TRIGGER radroots_runtime_events_mutation_insert
AFTER INSERT ON radroots_runtime_events
BEGIN
  SELECT CASE WHEN (
    SELECT mutation_epoch FROM radroots_runtime_source_generations
    WHERE generation = NEW.source_generation
  ) = 9223372036854775807
  THEN RAISE(ABORT, 'runtime event mutation epoch overflow') END;
  UPDATE radroots_runtime_source_generations
  SET mutation_epoch = mutation_epoch + 1
  WHERE generation = NEW.source_generation;
END;

CREATE TRIGGER radroots_runtime_events_mutation_update
AFTER UPDATE OF admission_stage, admitted_contract_id, admitted_registry_version
ON radroots_runtime_events
WHEN NEW.admission_stage IS NOT OLD.admission_stage
  OR NEW.admitted_contract_id IS NOT OLD.admitted_contract_id
  OR NEW.admitted_registry_version IS NOT OLD.admitted_registry_version
BEGIN
  SELECT CASE WHEN (
    SELECT mutation_epoch FROM radroots_runtime_source_generations
    WHERE generation = NEW.source_generation
  ) = 9223372036854775807
  THEN RAISE(ABORT, 'runtime event mutation epoch overflow') END;
  UPDATE radroots_runtime_source_generations
  SET mutation_epoch = mutation_epoch + 1
  WHERE generation = NEW.source_generation;
END;

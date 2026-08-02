CREATE TABLE radroots_runtime_source_generations (
  generation BLOB PRIMARY KEY NOT NULL CHECK (length(generation) = 32),
  sequence_head INTEGER NOT NULL DEFAULT 0 CHECK (sequence_head >= 0),
  state TEXT NOT NULL CHECK (state IN ('active', 'retired')),
  created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms > 0),
  retired_at_unix_ms INTEGER,
  CHECK (
    (state = 'active' AND retired_at_unix_ms IS NULL)
    OR (state = 'retired' AND retired_at_unix_ms >= created_at_unix_ms)
  )
) STRICT, WITHOUT ROWID;

CREATE TRIGGER radroots_runtime_source_generations_delete_guard
BEFORE DELETE ON radroots_runtime_source_generations
BEGIN
  SELECT RAISE(ABORT, 'runtime source generations are append-only');
END;

CREATE TRIGGER radroots_runtime_source_generations_identity_guard
BEFORE UPDATE OF generation, created_at_unix_ms ON radroots_runtime_source_generations
BEGIN
  SELECT RAISE(ABORT, 'runtime source generation identity is immutable');
END;

CREATE TABLE radroots_runtime_events (
  source_generation BLOB NOT NULL
    REFERENCES radroots_runtime_source_generations(generation),
  source_sequence INTEGER NOT NULL CHECK (source_sequence > 0),
  event_id BLOB NOT NULL CHECK (length(event_id) = 32),
  admission_stage TEXT NOT NULL CHECK (admission_stage IN ('raw', 'verified', 'visible')),
  signed_event BLOB NOT NULL CHECK (length(signed_event) > 0),
  admitted_at_unix_ms INTEGER NOT NULL CHECK (admitted_at_unix_ms > 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= admitted_at_unix_ms),
  PRIMARY KEY (source_generation, source_sequence),
  UNIQUE (event_id)
) STRICT, WITHOUT ROWID;

CREATE UNIQUE INDEX radroots_runtime_events_event_id_idx
ON radroots_runtime_events(event_id);

CREATE INDEX radroots_runtime_events_admission_idx
ON radroots_runtime_events(admission_stage, source_generation, source_sequence);

CREATE TRIGGER radroots_runtime_events_delete_guard
BEFORE DELETE ON radroots_runtime_events
BEGIN
  SELECT RAISE(ABORT, 'canonical runtime events are append-only');
END;

CREATE TRIGGER radroots_runtime_events_raw_update_guard
BEFORE UPDATE OF source_generation, source_sequence, event_id, signed_event, admitted_at_unix_ms
ON radroots_runtime_events
BEGIN
  SELECT RAISE(ABORT, 'canonical runtime event authority is immutable');
END;

CREATE TABLE radroots_runtime_event_provenance (
  event_id BLOB NOT NULL REFERENCES radroots_runtime_events(event_id),
  transport_kind TEXT NOT NULL CHECK (length(transport_kind) > 0),
  endpoint_fingerprint BLOB NOT NULL CHECK (length(endpoint_fingerprint) > 0),
  observation_kind TEXT NOT NULL CHECK (length(observation_kind) > 0),
  first_observed_at_unix_ms INTEGER NOT NULL CHECK (first_observed_at_unix_ms > 0),
  last_observed_at_unix_ms INTEGER NOT NULL
    CHECK (last_observed_at_unix_ms >= first_observed_at_unix_ms),
  observation_count INTEGER NOT NULL CHECK (observation_count > 0),
  PRIMARY KEY (event_id, transport_kind, endpoint_fingerprint, observation_kind)
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_event_provenance_observed_idx
ON radroots_runtime_event_provenance(last_observed_at_unix_ms, event_id);

CREATE TABLE radroots_runtime_journal_operations (
  instance_id BLOB PRIMARY KEY NOT NULL CHECK (length(instance_id) = 16),
  operation_id BLOB NOT NULL CHECK (length(operation_id) > 0),
  idempotency_key TEXT NOT NULL CHECK (length(idempotency_key) BETWEEN 1 AND 256),
  input_digest BLOB NOT NULL CHECK (length(input_digest) = 32),
  prepared_at_unix_ms INTEGER NOT NULL CHECK (prepared_at_unix_ms > 0),
  revision INTEGER NOT NULL CHECK (revision > 0),
  stage TEXT NOT NULL CHECK (stage IN ('prepared', 'signed', 'recoverable', 'committed')),
  event_id BLOB CHECK (event_id IS NULL OR length(event_id) = 32),
  recovery_record BLOB,
  cancellation_state TEXT NOT NULL
    CHECK (cancellation_state IN ('not_requested', 'cancelled_before_commit', 'observed_after_commit')),
  committed_at_unix_ms INTEGER,
  updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= prepared_at_unix_ms),
  CHECK ((stage IN ('signed', 'committed') AND event_id IS NOT NULL) OR stage IN ('prepared', 'recoverable')),
  CHECK ((stage = 'recoverable' AND recovery_record IS NOT NULL) OR (stage <> 'recoverable' AND recovery_record IS NULL)),
  CHECK ((stage = 'committed' AND committed_at_unix_ms >= prepared_at_unix_ms) OR (stage <> 'committed' AND committed_at_unix_ms IS NULL))
) STRICT, WITHOUT ROWID;

CREATE UNIQUE INDEX radroots_runtime_journal_idempotency_idx
ON radroots_runtime_journal_operations(operation_id, idempotency_key);

CREATE INDEX radroots_runtime_journal_recovery_idx
ON radroots_runtime_journal_operations(stage, updated_at_unix_ms, instance_id)
WHERE stage = 'recoverable';

CREATE TABLE radroots_runtime_outbox_items (
  item_id BLOB PRIMARY KEY NOT NULL CHECK (length(item_id) = 16),
  operation_instance_id BLOB NOT NULL
    REFERENCES radroots_runtime_journal_operations(instance_id),
  plan_digest BLOB NOT NULL CHECK (length(plan_digest) = 32),
  delivery_request BLOB NOT NULL CHECK (length(delivery_request) > 0),
  revision INTEGER NOT NULL CHECK (revision > 0),
  stage TEXT NOT NULL CHECK (stage IN ('pending', 'leased', 'retryable', 'satisfied', 'exhausted')),
  lease_id BLOB CHECK (lease_id IS NULL OR length(lease_id) = 16),
  lease_owner TEXT CHECK (lease_owner IS NULL OR length(lease_owner) BETWEEN 1 AND 128),
  lease_acquired_at_unix_ms INTEGER,
  lease_expires_at_unix_ms INTEGER,
  last_attempt INTEGER CHECK (last_attempt IS NULL OR last_attempt > 0),
  satisfaction TEXT NOT NULL CHECK (satisfaction IN ('pending', 'satisfied', 'exhausted')),
  retry_not_before_unix_ms INTEGER,
  created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms > 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= created_at_unix_ms),
  UNIQUE (operation_instance_id, plan_digest),
  CHECK (
    (stage = 'leased' AND lease_id IS NOT NULL AND lease_owner IS NOT NULL
      AND lease_acquired_at_unix_ms > 0 AND lease_expires_at_unix_ms > lease_acquired_at_unix_ms)
    OR (stage <> 'leased' AND lease_id IS NULL AND lease_owner IS NULL
      AND lease_acquired_at_unix_ms IS NULL AND lease_expires_at_unix_ms IS NULL)
  )
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_outbox_ready_idx
ON radroots_runtime_outbox_items(stage, retry_not_before_unix_ms, created_at_unix_ms, item_id);

CREATE TABLE radroots_runtime_outbox_targets (
  item_id BLOB NOT NULL REFERENCES radroots_runtime_outbox_items(item_id) ON DELETE CASCADE,
  target_fingerprint BLOB NOT NULL CHECK (length(target_fingerprint) > 0),
  target_request BLOB NOT NULL CHECK (length(target_request) > 0),
  ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
  PRIMARY KEY (item_id, target_fingerprint),
  UNIQUE (item_id, ordinal)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_delivery_evidence (
  item_id BLOB NOT NULL,
  target_fingerprint BLOB NOT NULL,
  attempt INTEGER NOT NULL CHECK (attempt > 0),
  attempted INTEGER NOT NULL CHECK (attempted IN (0, 1)),
  outcome BLOB NOT NULL CHECK (length(outcome) > 0),
  retryability TEXT NOT NULL CHECK (retryability IN ('retryable', 'terminal', 'not_applicable')),
  recorded_at_unix_ms INTEGER NOT NULL CHECK (recorded_at_unix_ms > 0),
  PRIMARY KEY (item_id, target_fingerprint, attempt),
  FOREIGN KEY (item_id, target_fingerprint)
    REFERENCES radroots_runtime_outbox_targets(item_id, target_fingerprint) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_delivery_evidence_item_idx
ON radroots_runtime_delivery_evidence(item_id, attempt, target_fingerprint);

CREATE TABLE radroots_runtime_projection_checkpoints (
  projection_id TEXT NOT NULL CHECK (length(projection_id) BETWEEN 1 AND 128),
  projection_generation BLOB NOT NULL CHECK (length(projection_generation) = 32),
  source_generation BLOB,
  source_sequence INTEGER,
  projected_rows INTEGER NOT NULL CHECK (projected_rows >= 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms > 0),
  PRIMARY KEY (projection_id, projection_generation),
  FOREIGN KEY (source_generation) REFERENCES radroots_runtime_source_generations(generation),
  CHECK ((source_generation IS NULL AND source_sequence IS NULL) OR (length(source_generation) = 32 AND source_sequence > 0))
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_projection_invalidations (
  projection_id TEXT NOT NULL CHECK (length(projection_id) BETWEEN 1 AND 128),
  invalid_generation BLOB NOT NULL CHECK (length(invalid_generation) = 32),
  replacement_generation BLOB NOT NULL CHECK (length(replacement_generation) = 32),
  reason TEXT NOT NULL CHECK (reason IN ('source_generation_changed', 'projection_generation_changed', 'event_index_manifest_changed', 'integrity_failure', 'operator_requested')),
  invalidated_at_unix_ms INTEGER NOT NULL CHECK (invalidated_at_unix_ms > 0),
  PRIMARY KEY (projection_id, invalid_generation),
  CHECK (invalid_generation <> replacement_generation)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_projection_rebuilds (
  ticket_id BLOB PRIMARY KEY NOT NULL CHECK (length(ticket_id) = 16),
  projection_id TEXT NOT NULL,
  invalid_generation BLOB NOT NULL,
  replacement_generation BLOB NOT NULL,
  revision INTEGER NOT NULL CHECK (revision > 0),
  stage TEXT NOT NULL CHECK (stage IN ('requested', 'running', 'completed', 'failed')),
  requested_at_unix_ms INTEGER NOT NULL CHECK (requested_at_unix_ms > 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= requested_at_unix_ms),
  FOREIGN KEY (projection_id, invalid_generation)
    REFERENCES radroots_runtime_projection_invalidations(projection_id, invalid_generation)
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_runtime_projection_rebuilds_stage_idx
ON radroots_runtime_projection_rebuilds(stage, updated_at_unix_ms, ticket_id);

CREATE TABLE radroots_runtime_event_index_manifests (
  projection_id TEXT NOT NULL CHECK (length(projection_id) BETWEEN 1 AND 128),
  projection_generation BLOB NOT NULL CHECK (length(projection_generation) = 32),
  manifest_digest BLOB NOT NULL CHECK (length(manifest_digest) = 32),
  source_generation BLOB NOT NULL
    REFERENCES radroots_runtime_source_generations(generation),
  created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms > 0),
  PRIMARY KEY (projection_id, projection_generation),
  UNIQUE (manifest_digest)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_event_index_shards (
  manifest_digest BLOB NOT NULL
    REFERENCES radroots_runtime_event_index_manifests(manifest_digest) ON DELETE CASCADE,
  shard_id TEXT NOT NULL CHECK (length(shard_id) BETWEEN 1 AND 128),
  ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
  artifact_path TEXT NOT NULL CHECK (length(artifact_path) BETWEEN 1 AND 512),
  artifact_digest BLOB NOT NULL CHECK (length(artifact_digest) = 32),
  cursor BLOB NOT NULL CHECK (length(cursor) BETWEEN 1 AND 2048),
  PRIMARY KEY (manifest_digest, shard_id),
  UNIQUE (manifest_digest, ordinal),
  UNIQUE (manifest_digest, artifact_path)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_event_index_checkpoints (
  manifest_digest BLOB NOT NULL,
  shard_id TEXT NOT NULL,
  indexed_through_event_id BLOB CHECK (indexed_through_event_id IS NULL OR length(indexed_through_event_id) = 32),
  indexed_events INTEGER NOT NULL CHECK (indexed_events >= 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms > 0),
  PRIMARY KEY (manifest_digest, shard_id),
  FOREIGN KEY (manifest_digest, shard_id)
    REFERENCES radroots_runtime_event_index_shards(manifest_digest, shard_id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_atomic_commits (
  commit_id BLOB PRIMARY KEY NOT NULL CHECK (length(commit_id) = 16),
  commit_digest BLOB NOT NULL CHECK (length(commit_digest) = 32),
  workflow_kind TEXT NOT NULL CHECK (workflow_kind IN ('prepared', 'signed', 'enqueued', 'delivered', 'ingested')),
  requested_at_unix_ms INTEGER NOT NULL CHECK (requested_at_unix_ms > 0),
  committed_at_unix_ms INTEGER NOT NULL CHECK (committed_at_unix_ms >= requested_at_unix_ms),
  receipt BLOB NOT NULL CHECK (length(receipt) > 0)
) STRICT, WITHOUT ROWID;

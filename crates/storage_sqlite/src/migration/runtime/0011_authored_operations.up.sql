ALTER TABLE radroots_runtime_events
ADD COLUMN admitted_contract_id TEXT
CHECK(admitted_contract_id IS NULL OR (
  length(admitted_contract_id) BETWEEN 1 AND 192
  AND admitted_contract_id = lower(admitted_contract_id)
  AND admitted_contract_id LIKE 'radroots.%'
));

ALTER TABLE radroots_runtime_events
ADD COLUMN admitted_registry_version INTEGER
CHECK(admitted_registry_version IS NULL OR admitted_registry_version > 0);

CREATE TRIGGER radroots_runtime_events_contract_metadata_guard
BEFORE UPDATE OF admitted_contract_id, admitted_registry_version
ON radroots_runtime_events
WHEN
  (OLD.admitted_contract_id IS NOT NULL AND (
    NEW.admitted_contract_id IS NULL OR NEW.admitted_contract_id != OLD.admitted_contract_id
  ))
  OR (OLD.admitted_registry_version IS NOT NULL AND (
    NEW.admitted_registry_version IS NULL
    OR NEW.admitted_registry_version != OLD.admitted_registry_version
  ))
  OR ((NEW.admitted_contract_id IS NULL) != (NEW.admitted_registry_version IS NULL))
BEGIN
  SELECT RAISE(ABORT, 'event contract metadata must be paired and immutable');
END;

CREATE TRIGGER radroots_runtime_events_contract_metadata_insert_guard
BEFORE INSERT ON radroots_runtime_events
WHEN ((NEW.admitted_contract_id IS NULL) != (NEW.admitted_registry_version IS NULL))
BEGIN
  SELECT RAISE(ABORT, 'event contract metadata must be paired');
END;

CREATE TABLE radroots_runtime_authored_operations (
  operation_id BLOB PRIMARY KEY CHECK(length(operation_id) = 16),
  artifact_count INTEGER NOT NULL CHECK(artifact_count BETWEEN 1 AND 1024),
  created_at_unix_ms INTEGER NOT NULL CHECK(created_at_unix_ms > 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK(updated_at_unix_ms >= created_at_unix_ms),
  revision INTEGER NOT NULL CHECK(revision > 0),
  snapshot BLOB NOT NULL CHECK(length(snapshot) BETWEEN 2 AND 4194304)
) STRICT;

CREATE TABLE radroots_runtime_authored_artifacts (
  artifact_id BLOB PRIMARY KEY CHECK(length(artifact_id) = 16),
  operation_id BLOB NOT NULL REFERENCES radroots_runtime_authored_operations(operation_id) ON DELETE CASCADE,
  ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 65535),
  origin TEXT NOT NULL CHECK(origin IN ('planned', 'imported_signed')),
  signing_state TEXT NOT NULL CHECK(signing_state IN (
    'planned', 'signed', 'retryable', 'indeterminate', 'failed_terminal', 'cancelled'
  )),
  admission_state TEXT NOT NULL CHECK(admission_state IN (
    'pending', 'inserted', 'duplicate', 'retryable', 'rejected', 'cancelled'
  )),
  plan_wire BLOB CHECK(plan_wire IS NULL OR length(plan_wire) BETWEEN 2 AND 1048576),
  signed_raw_json BLOB CHECK(signed_raw_json IS NULL OR length(signed_raw_json) BETWEEN 2 AND 1048576),
  signed_raw_sha256 BLOB CHECK(signed_raw_sha256 IS NULL OR length(signed_raw_sha256) = 32),
  signing_claim_token BLOB CHECK(signing_claim_token IS NULL OR length(signing_claim_token) = 16),
  signing_claim_generation INTEGER CHECK(signing_claim_generation IS NULL OR signing_claim_generation > 0),
  signing_claim_revision INTEGER CHECK(signing_claim_revision IS NULL OR signing_claim_revision > 0),
  signing_claim_expires_at_unix_ms INTEGER CHECK(signing_claim_expires_at_unix_ms IS NULL OR signing_claim_expires_at_unix_ms > 0),
  admission_claim_token BLOB CHECK(admission_claim_token IS NULL OR length(admission_claim_token) = 16),
  admission_claim_generation INTEGER CHECK(admission_claim_generation IS NULL OR admission_claim_generation > 0),
  admission_claim_revision INTEGER CHECK(admission_claim_revision IS NULL OR admission_claim_revision > 0),
  admission_claim_expires_at_unix_ms INTEGER CHECK(admission_claim_expires_at_unix_ms IS NULL OR admission_claim_expires_at_unix_ms > 0),
  retry_not_before_unix_ms INTEGER CHECK(retry_not_before_unix_ms IS NULL OR retry_not_before_unix_ms > 0),
  last_failure_code TEXT CHECK(last_failure_code IS NULL OR length(last_failure_code) BETWEEN 1 AND 96),
  created_at_unix_ms INTEGER NOT NULL CHECK(created_at_unix_ms > 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK(updated_at_unix_ms >= created_at_unix_ms),
  revision INTEGER NOT NULL CHECK(revision > 0),
  snapshot BLOB NOT NULL CHECK(length(snapshot) BETWEEN 2 AND 4194304),
  UNIQUE(operation_id, ordinal),
  CHECK((signing_state = 'signed') = (signed_raw_json IS NOT NULL)),
  CHECK((signed_raw_json IS NULL) = (signed_raw_sha256 IS NULL)),
  CHECK((plan_wire IS NULL) = (origin = 'imported_signed')),
  CHECK((signing_claim_token IS NULL) = (signing_claim_generation IS NULL)),
  CHECK((signing_claim_token IS NULL) = (signing_claim_revision IS NULL)),
  CHECK((signing_claim_token IS NULL) = (signing_claim_expires_at_unix_ms IS NULL)),
  CHECK((admission_claim_token IS NULL) = (admission_claim_generation IS NULL)),
  CHECK((admission_claim_token IS NULL) = (admission_claim_revision IS NULL)),
  CHECK((admission_claim_token IS NULL) = (admission_claim_expires_at_unix_ms IS NULL))
) STRICT;

CREATE INDEX radroots_runtime_authored_artifacts_signing_ready_idx
ON radroots_runtime_authored_artifacts(
  signing_state, retry_not_before_unix_ms, signing_claim_expires_at_unix_ms,
  updated_at_unix_ms, artifact_id
);

CREATE INDEX radroots_runtime_authored_artifacts_admission_ready_idx
ON radroots_runtime_authored_artifacts(
  admission_state, retry_not_before_unix_ms, admission_claim_expires_at_unix_ms,
  updated_at_unix_ms, artifact_id
);

CREATE TABLE radroots_runtime_authored_delivery_plans (
  plan_id BLOB PRIMARY KEY CHECK(length(plan_id) = 16),
  artifact_id BLOB NOT NULL REFERENCES radroots_runtime_authored_artifacts(artifact_id) ON DELETE CASCADE,
  request_digest BLOB NOT NULL CHECK(length(request_digest) = 32),
  state TEXT NOT NULL CHECK(state IN (
    'pending', 'retryable', 'satisfied', 'exhausted', 'failed_terminal', 'cancelled'
  )),
  attempt_count INTEGER NOT NULL CHECK(attempt_count BETWEEN 0 AND 1024),
  claim_token BLOB CHECK(claim_token IS NULL OR length(claim_token) = 16),
  claim_generation INTEGER CHECK(claim_generation IS NULL OR claim_generation > 0),
  claim_revision INTEGER CHECK(claim_revision IS NULL OR claim_revision > 0),
  claim_expires_at_unix_ms INTEGER CHECK(claim_expires_at_unix_ms IS NULL OR claim_expires_at_unix_ms > 0),
  retry_not_before_unix_ms INTEGER CHECK(retry_not_before_unix_ms IS NULL OR retry_not_before_unix_ms > 0),
  last_failure_code TEXT CHECK(last_failure_code IS NULL OR length(last_failure_code) BETWEEN 1 AND 96),
  created_at_unix_ms INTEGER NOT NULL CHECK(created_at_unix_ms > 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK(updated_at_unix_ms >= created_at_unix_ms),
  revision INTEGER NOT NULL CHECK(revision > 0),
  snapshot BLOB NOT NULL CHECK(length(snapshot) BETWEEN 2 AND 4194304),
  CHECK((claim_token IS NULL) = (claim_generation IS NULL)),
  CHECK((claim_token IS NULL) = (claim_revision IS NULL)),
  CHECK((claim_token IS NULL) = (claim_expires_at_unix_ms IS NULL)),
  CHECK((state = 'retryable') = (retry_not_before_unix_ms IS NOT NULL))
) STRICT;

CREATE INDEX radroots_runtime_authored_delivery_ready_idx
ON radroots_runtime_authored_delivery_plans(
  state, retry_not_before_unix_ms, claim_expires_at_unix_ms,
  updated_at_unix_ms, plan_id
);

CREATE TABLE radroots_runtime_authored_delivery_targets (
  plan_id BLOB NOT NULL REFERENCES radroots_runtime_authored_delivery_plans(plan_id) ON DELETE CASCADE,
  ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 65535),
  target_fingerprint TEXT NOT NULL CHECK(length(target_fingerprint) BETWEEN 1 AND 512),
  target_snapshot BLOB NOT NULL CHECK(length(target_snapshot) BETWEEN 2 AND 65536),
  PRIMARY KEY(plan_id, ordinal),
  UNIQUE(plan_id, target_fingerprint)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_authored_delivery_attempts (
  plan_id BLOB NOT NULL REFERENCES radroots_runtime_authored_delivery_plans(plan_id) ON DELETE CASCADE,
  attempt INTEGER NOT NULL CHECK(attempt BETWEEN 1 AND 1024),
  satisfaction TEXT NOT NULL CHECK(satisfaction IN ('satisfied', 'pending', 'exhausted')),
  recorded_at_unix_ms INTEGER NOT NULL CHECK(recorded_at_unix_ms > 0),
  outcome_snapshot BLOB NOT NULL CHECK(length(outcome_snapshot) BETWEEN 2 AND 4194304),
  PRIMARY KEY(plan_id, attempt)
) STRICT, WITHOUT ROWID;

CREATE TABLE radroots_runtime_authored_atomic_commits (
  commit_id BLOB PRIMARY KEY CHECK(length(commit_id) = 16),
  commit_digest BLOB NOT NULL CHECK(length(commit_digest) = 32),
  phase TEXT NOT NULL CHECK(phase IN (
    'prepare', 'claim', 'signing', 'admission', 'delivery',
    'signing_failure', 'admission_failure', 'delivery_failure', 'cancel'
  )),
  target_id BLOB NOT NULL CHECK(length(target_id) = 16),
  requested_at_unix_ms INTEGER NOT NULL CHECK(requested_at_unix_ms > 0),
  committed_at_unix_ms INTEGER NOT NULL CHECK(committed_at_unix_ms >= requested_at_unix_ms),
  receipt BLOB NOT NULL CHECK(length(receipt) BETWEEN 2 AND 4194304)
) STRICT;

CREATE TRIGGER radroots_runtime_authored_atomic_commits_update_guard
BEFORE UPDATE ON radroots_runtime_authored_atomic_commits
BEGIN
  SELECT RAISE(ABORT, 'authored atomic receipts are immutable');
END;

CREATE TABLE radroots_runtime_authored_migration_evidence (
  source_version INTEGER PRIMARY KEY CHECK(source_version = 10),
  operation_count INTEGER NOT NULL CHECK(operation_count >= 0),
  event_count INTEGER NOT NULL CHECK(event_count >= 0),
  outbox_count INTEGER NOT NULL CHECK(outbox_count >= 0),
  target_count INTEGER NOT NULL CHECK(target_count >= 0),
  attempt_count INTEGER NOT NULL CHECK(attempt_count >= 0),
  imported_count INTEGER NOT NULL CHECK(imported_count >= 0),
  source_digest BLOB NOT NULL CHECK(length(source_digest) = 32),
  completed_at_unix_ms INTEGER NOT NULL CHECK(completed_at_unix_ms > 0),
  CHECK(imported_count <= operation_count),
  CHECK(imported_count <= outbox_count)
) STRICT;

CREATE TRIGGER radroots_runtime_authored_migration_evidence_update_guard
BEFORE UPDATE ON radroots_runtime_authored_migration_evidence
BEGIN
  SELECT RAISE(ABORT, 'authored migration evidence is immutable');
END;

CREATE TRIGGER radroots_runtime_authored_migration_evidence_delete_guard
BEFORE DELETE ON radroots_runtime_authored_migration_evidence
BEGIN
  SELECT RAISE(ABORT, 'authored migration evidence is retained');
END;

CREATE TRIGGER radroots_runtime_authored_atomic_commits_delete_guard
BEFORE DELETE ON radroots_runtime_authored_atomic_commits
BEGIN
  SELECT RAISE(ABORT, 'authored atomic receipts are retained');
END;

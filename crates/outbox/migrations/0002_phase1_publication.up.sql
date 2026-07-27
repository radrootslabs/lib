CREATE TABLE outbox_phase1_publication (
  publication_id INTEGER PRIMARY KEY AUTOINCREMENT,
  operation_digest BLOB NOT NULL UNIQUE CHECK (length(operation_digest) = 32),
  artifact_schema_version INTEGER NOT NULL CHECK (artifact_schema_version = 1),
  artifact_json BLOB NOT NULL CHECK (length(artifact_json) BETWEEN 1 AND 2097152),
  artifact_digest BLOB NOT NULL CHECK (length(artifact_digest) = 32),
  readiness_schema_version INTEGER NOT NULL CHECK (readiness_schema_version = 1),
  readiness_json BLOB NOT NULL CHECK (length(readiness_json) BETWEEN 1 AND 4194304),
  readiness_digest BLOB NOT NULL CHECK (length(readiness_digest) = 32),
  semantic_role TEXT NOT NULL CHECK (semantic_role IN ('profile', 'update', 'photo_update', 'ask', 'event_date', 'event_time', 'food_availability')),
  expected_author BLOB NOT NULL CHECK (length(expected_author) = 32),
  expected_event_id BLOB NOT NULL CHECK (length(expected_event_id) = 32),
  target_policy_digest BLOB NOT NULL CHECK (length(target_policy_digest) = 32),
  target_count INTEGER NOT NULL CHECK (target_count BETWEEN 1 AND 16),
  required_target_count INTEGER NOT NULL CHECK (required_target_count BETWEEN 1 AND target_count),
  state TEXT NOT NULL CHECK (state IN ('ready', 'claimed-for-signing', 'signed-ready', 'dispatching', 'published', 'failed-retryable', 'failed-terminal', 'quarantined', 'cancelled')),
  state_revision INTEGER NOT NULL CHECK (state_revision >= 0),
  claim_token BLOB CHECK (claim_token IS NULL OR length(claim_token) = 32),
  claim_expires_at_ms INTEGER,
  next_attempt_after_ms INTEGER NOT NULL,
  signed_event_json BLOB CHECK (signed_event_json IS NULL OR length(signed_event_json) BETWEEN 1 AND 1048576),
  signed_event_digest BLOB CHECK (signed_event_digest IS NULL OR length(signed_event_digest) = 32),
  signed_event_id BLOB CHECK (signed_event_id IS NULL OR length(signed_event_id) = 32),
  last_error TEXT CHECK (last_error IS NULL OR length(CAST(last_error AS BLOB)) <= 4096),
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  CHECK (
    (claim_token IS NULL AND claim_expires_at_ms IS NULL)
    OR (claim_token IS NOT NULL AND claim_expires_at_ms IS NOT NULL AND state = 'claimed-for-signing')
  ),
  CHECK (
    (signed_event_json IS NULL AND signed_event_digest IS NULL AND signed_event_id IS NULL)
    OR (signed_event_json IS NOT NULL AND signed_event_digest IS NOT NULL AND signed_event_id IS NOT NULL)
  ),
  CHECK (state NOT IN ('signed-ready', 'dispatching', 'published') OR signed_event_json IS NOT NULL)
) STRICT;

CREATE INDEX outbox_phase1_publication_ready_idx
ON outbox_phase1_publication(state, next_attempt_after_ms, claim_expires_at_ms, created_at_ms, publication_id);

CREATE INDEX outbox_phase1_publication_event_idx
ON outbox_phase1_publication(expected_event_id, publication_id);

CREATE TABLE outbox_phase1_delivery_target (
  target_id INTEGER PRIMARY KEY AUTOINCREMENT,
  publication_id INTEGER NOT NULL REFERENCES outbox_phase1_publication(publication_id) ON DELETE CASCADE,
  target_ordinal INTEGER NOT NULL CHECK (target_ordinal BETWEEN 0 AND 15),
  endpoint_uri TEXT NOT NULL CHECK (length(CAST(endpoint_uri AS BLOB)) BETWEEN 1 AND 2048),
  endpoint_fingerprint BLOB NOT NULL CHECK (length(endpoint_fingerprint) = 32),
  dispatch_digest BLOB NOT NULL CHECK (length(dispatch_digest) = 32),
  state TEXT NOT NULL CHECK (state IN ('pending', 'in-flight', 'accepted-observation-pending', 'accepted-observed', 'failed-retryable', 'failed-terminal', 'uncertain', 'cancelled')),
  state_revision INTEGER NOT NULL CHECK (state_revision >= 0),
  claim_token BLOB CHECK (claim_token IS NULL OR length(claim_token) = 32),
  claim_expires_at_ms INTEGER,
  next_attempt_after_ms INTEGER NOT NULL,
  last_error TEXT CHECK (last_error IS NULL OR length(CAST(last_error AS BLOB)) <= 4096),
  updated_at_ms INTEGER NOT NULL,
  UNIQUE(publication_id, target_ordinal),
  UNIQUE(publication_id, endpoint_uri),
  UNIQUE(dispatch_digest),
  CHECK (
    (claim_token IS NULL AND claim_expires_at_ms IS NULL)
    OR (claim_token IS NOT NULL AND claim_expires_at_ms IS NOT NULL AND state = 'in-flight')
  )
) STRICT;

CREATE INDEX outbox_phase1_delivery_target_ready_idx
ON outbox_phase1_delivery_target(state, next_attempt_after_ms, claim_expires_at_ms, publication_id, target_id);

CREATE TABLE outbox_phase1_dispatch_intent (
  intent_digest BLOB PRIMARY KEY CHECK (length(intent_digest) = 32),
  target_id INTEGER NOT NULL UNIQUE REFERENCES outbox_phase1_delivery_target(target_id) ON DELETE CASCADE,
  signed_event_digest BLOB NOT NULL CHECK (length(signed_event_digest) = 32),
  state TEXT NOT NULL CHECK (state IN ('in-flight', 'uncertain', 'completed', 'cancelled')),
  state_revision INTEGER NOT NULL CHECK (state_revision >= 0),
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE outbox_phase1_target_receipt (
  receipt_digest BLOB PRIMARY KEY CHECK (length(receipt_digest) = 32),
  target_id INTEGER NOT NULL REFERENCES outbox_phase1_delivery_target(target_id) ON DELETE CASCADE,
  observation_kind TEXT NOT NULL CHECK (observation_kind IN ('accepted-pending', 'accepted-observed')),
  observed_at_ms INTEGER NOT NULL,
  UNIQUE(target_id, observation_kind)
) STRICT, WITHOUT ROWID;

CREATE TABLE outbox_phase1_observation_repair (
  repair_digest BLOB PRIMARY KEY CHECK (length(repair_digest) = 32),
  target_id INTEGER NOT NULL UNIQUE REFERENCES outbox_phase1_delivery_target(target_id) ON DELETE CASCADE,
  state TEXT NOT NULL CHECK (state IN ('pending', 'complete', 'failed-terminal')),
  state_revision INTEGER NOT NULL CHECK (state_revision >= 0),
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

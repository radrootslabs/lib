CREATE TABLE IF NOT EXISTS outbox_operations (
  operation_id INTEGER PRIMARY KEY AUTOINCREMENT,
  operation_kind TEXT NOT NULL,
  expected_pubkey TEXT NOT NULL,
  idempotency_key TEXT,
  operation_idempotency_digest TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'complete', 'failed_terminal', 'cancelled')),
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS outbox_operation_idempotency_idx
ON outbox_operations(operation_kind, expected_pubkey, idempotency_key)
WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS outbox_operation_status_idx
ON outbox_operations(status, created_at_ms, operation_id);

CREATE TABLE IF NOT EXISTS outbox_event (
  outbox_event_id INTEGER PRIMARY KEY AUTOINCREMENT,
  operation_id INTEGER NOT NULL REFERENCES outbox_operations(operation_id) ON DELETE CASCADE,
  event_id TEXT NOT NULL,
  expected_pubkey TEXT NOT NULL,
  draft_json TEXT NOT NULL,
  signed_event_json TEXT,
  raw_event_json TEXT,
  state TEXT NOT NULL CHECK (state IN ('draft_queued', 'signing', 'signed', 'publishing', 'published', 'sign_retryable', 'publish_retryable', 'failed_terminal', 'cancelled')),
  attempt_count INTEGER NOT NULL,
  claim_token TEXT,
  claim_owner TEXT,
  claim_expires_at_ms INTEGER,
  next_attempt_after_ms INTEGER NOT NULL,
  last_error TEXT,
  event_store_ingested INTEGER NOT NULL,
  event_store_inserted INTEGER NOT NULL,
  event_store_ingested_at_ms INTEGER,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS outbox_event_ready_idx
ON outbox_event(state, next_attempt_after_ms, claim_expires_at_ms, created_at_ms, outbox_event_id);

CREATE INDEX IF NOT EXISTS outbox_event_event_id_idx
ON outbox_event(event_id);

CREATE TABLE IF NOT EXISTS outbox_delivery_plan (
  delivery_plan_id INTEGER PRIMARY KEY AUTOINCREMENT,
  outbox_event_id INTEGER NOT NULL REFERENCES outbox_event(outbox_event_id) ON DELETE CASCADE,
  transport_profile_id TEXT NOT NULL,
  target_policy_fingerprint TEXT NOT NULL,
  target_policy_version INTEGER NOT NULL,
  satisfaction_policy TEXT NOT NULL,
  required_success_count INTEGER NOT NULL,
  delivery_plan_idempotency_digest TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'complete', 'deferred_until_implemented', 'preview_unavailable', 'failed_terminal', 'cancelled')),
  satisfied_at_ms INTEGER,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  UNIQUE(outbox_event_id, delivery_plan_idempotency_digest)
);

CREATE INDEX IF NOT EXISTS outbox_delivery_plan_event_idx
ON outbox_delivery_plan(outbox_event_id, status, delivery_plan_id);

CREATE TABLE IF NOT EXISTS outbox_delivery_target (
  delivery_target_id INTEGER PRIMARY KEY AUTOINCREMENT,
  delivery_plan_id INTEGER NOT NULL REFERENCES outbox_delivery_plan(delivery_plan_id) ON DELETE CASCADE,
  transport_kind TEXT NOT NULL,
  endpoint_uri TEXT NOT NULL,
  endpoint_fingerprint TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'delivered', 'forwarded', 'stored_by_gateway', 'seen', 'deferred_until_implemented', 'preview_unavailable', 'skipped_policy_denied', 'failed_retryable', 'failed_terminal')),
  attempt_count INTEGER NOT NULL,
  last_attempt_at_ms INTEGER,
  completed_at_ms INTEGER,
  last_error TEXT,
  UNIQUE(delivery_plan_id, endpoint_fingerprint)
);

CREATE INDEX IF NOT EXISTS outbox_delivery_target_ready_idx
ON outbox_delivery_target(status, delivery_plan_id, delivery_target_id);

CREATE TABLE IF NOT EXISTS outbox_delivery_attempt (
  delivery_attempt_id INTEGER PRIMARY KEY AUTOINCREMENT,
  delivery_plan_id INTEGER NOT NULL REFERENCES outbox_delivery_plan(delivery_plan_id) ON DELETE CASCADE,
  delivery_target_id INTEGER NOT NULL REFERENCES outbox_delivery_target(delivery_target_id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  attempted_at_ms INTEGER NOT NULL,
  message TEXT
);

CREATE INDEX IF NOT EXISTS outbox_delivery_attempt_target_idx
ON outbox_delivery_attempt(delivery_target_id, attempted_at_ms, delivery_attempt_id);

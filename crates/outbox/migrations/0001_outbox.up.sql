CREATE TABLE outbox_operation (
  operation_id INTEGER PRIMARY KEY AUTOINCREMENT,
  operation_kind TEXT NOT NULL,
  expected_pubkey TEXT NOT NULL,
  idempotency_key TEXT,
  idempotency_digest TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'complete', 'failed_terminal', 'cancelled')),
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE UNIQUE INDEX outbox_operation_idempotency_idx
ON outbox_operation(operation_kind, expected_pubkey, idempotency_key)
WHERE idempotency_key IS NOT NULL;

CREATE INDEX outbox_operation_status_idx
ON outbox_operation(status, created_at_ms, operation_id);

CREATE TABLE outbox_event (
  outbox_event_id INTEGER PRIMARY KEY AUTOINCREMENT,
  operation_id INTEGER NOT NULL REFERENCES outbox_operation(operation_id) ON DELETE CASCADE,
  event_id TEXT NOT NULL,
  expected_pubkey TEXT NOT NULL,
  draft_json TEXT NOT NULL,
  signed_event_json TEXT,
  raw_event_json TEXT,
  state TEXT NOT NULL CHECK (state IN ('draft_queued', 'signing', 'signed', 'publishing', 'published', 'sign_retryable', 'publish_retryable', 'failed_terminal', 'cancelled')),
  accepted_quorum INTEGER NOT NULL CHECK (accepted_quorum >= 0),
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

CREATE INDEX outbox_event_ready_idx
ON outbox_event(state, next_attempt_after_ms, claim_expires_at_ms, created_at_ms, outbox_event_id);

CREATE INDEX outbox_event_event_id_idx
ON outbox_event(event_id);

CREATE TABLE outbox_event_relay_status (
  outbox_event_id INTEGER NOT NULL REFERENCES outbox_event(outbox_event_id) ON DELETE CASCADE,
  relay_url TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'failed_retryable', 'failed_terminal')),
  attempt_count INTEGER NOT NULL,
  last_attempt_at_ms INTEGER,
  acknowledged_at_ms INTEGER,
  last_error TEXT,
  PRIMARY KEY (outbox_event_id, relay_url)
);

CREATE INDEX outbox_event_relay_status_idx
ON outbox_event_relay_status(status, relay_url, outbox_event_id);

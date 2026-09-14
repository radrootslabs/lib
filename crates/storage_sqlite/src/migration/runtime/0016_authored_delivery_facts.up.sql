-- Preserve scheduling state and revision while retaining independent effects.
ALTER TABLE radroots_runtime_authored_delivery_plans
ADD COLUMN stop_requested_at_unix_ms INTEGER CHECK (
  stop_requested_at_unix_ms IS NULL OR (
    stop_requested_at_unix_ms >= created_at_unix_ms
    AND stop_requested_at_unix_ms <= updated_at_unix_ms
    AND state IN ('satisfied', 'exhausted', 'failed_terminal', 'cancelled')
    AND claim_token IS NULL
  )
);

UPDATE radroots_runtime_authored_delivery_plans
SET stop_requested_at_unix_ms = updated_at_unix_ms WHERE state = 'cancelled';

CREATE TRIGGER radroots_runtime_authored_delivery_stop_guard
BEFORE UPDATE ON radroots_runtime_authored_delivery_plans
WHEN OLD.stop_requested_at_unix_ms IS NOT NULL
  AND NEW.stop_requested_at_unix_ms IS NOT OLD.stop_requested_at_unix_ms
BEGIN
  SELECT RAISE(ABORT, 'delivery stop intent is immutable');
END;

CREATE TABLE radroots_runtime_authored_delivery_facts (
  plan_id BLOB NOT NULL CHECK (length(plan_id) = 16) REFERENCES radroots_runtime_authored_delivery_plans(plan_id),
  ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal < 1024),
  claim_id BLOB NOT NULL CHECK (length(claim_id) = 16) REFERENCES radroots_runtime_authored_atomic_commits(commit_id),
  observed_at_unix_ms INTEGER NOT NULL CHECK (observed_at_unix_ms > 0),
  fact_snapshot BLOB NOT NULL CHECK (length(fact_snapshot) BETWEEN 2 AND 4194304),
  PRIMARY KEY (plan_id, ordinal),
  UNIQUE (plan_id, claim_id)
) STRICT;

CREATE TRIGGER radroots_runtime_authored_delivery_facts_update_guard
BEFORE UPDATE ON radroots_runtime_authored_delivery_facts
BEGIN
  SELECT RAISE(ABORT, 'delivery facts are immutable');
END;

CREATE TRIGGER radroots_runtime_authored_delivery_facts_delete_guard
BEFORE DELETE ON radroots_runtime_authored_delivery_facts
BEGIN
  SELECT RAISE(ABORT, 'delivery facts are immutable');
END;

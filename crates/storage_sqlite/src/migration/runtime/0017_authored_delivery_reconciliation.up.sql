-- Project original issued delivery claims without rewriting their receipts.
-- Reject ambiguous claim namespaces before absence of a delivery projection
-- can be interpreted as evidence that no work was issued.
SELECT json(CASE
  WHEN json_type(CAST(receipt AS TEXT), '$.outcome.delivery_plan') = 'object'
   AND json_type(CAST(receipt AS TEXT), '$.outcome.artifact') IS NULL THEN 'null'
  WHEN json_type(CAST(receipt AS TEXT), '$.outcome.artifact') = 'object'
   AND json_type(CAST(receipt AS TEXT), '$.outcome.delivery_plan') IS NULL THEN 'null'
  ELSE 'invalid claim outcome'
END)
FROM radroots_runtime_authored_atomic_commits WHERE phase = 'claim';

CREATE INDEX radroots_runtime_authored_atomic_target_phase_idx
ON radroots_runtime_authored_atomic_commits(target_id, phase, commit_id);

CREATE TABLE radroots_runtime_authored_delivery_claims (
  plan_id BLOB NOT NULL REFERENCES radroots_runtime_authored_delivery_plans(plan_id),
  claim_id BLOB NOT NULL REFERENCES radroots_runtime_authored_atomic_commits(commit_id),
  PRIMARY KEY (plan_id, claim_id)
) STRICT, WITHOUT ROWID;

INSERT INTO radroots_runtime_authored_delivery_claims (plan_id, claim_id)
SELECT target_id, commit_id FROM radroots_runtime_authored_atomic_commits
WHERE phase = 'claim'
  AND json_type(CAST(receipt AS TEXT), '$.outcome.delivery_plan') = 'object';

CREATE TRIGGER radroots_runtime_authored_delivery_claims_update_guard
BEFORE UPDATE ON radroots_runtime_authored_delivery_claims
BEGIN
  SELECT RAISE(ABORT, 'issued delivery claims are immutable');
END;

CREATE TRIGGER radroots_runtime_authored_delivery_claims_delete_guard
BEFORE DELETE ON radroots_runtime_authored_delivery_claims
BEGIN
  SELECT RAISE(ABORT, 'issued delivery claims are retained');
END;

CREATE TABLE radroots_runtime_authored_delivery_reconciliations (
  plan_id BLOB NOT NULL,
  attempt INTEGER NOT NULL CHECK (attempt BETWEEN 1 AND 1024),
  claim_id BLOB NOT NULL,
  PRIMARY KEY (plan_id, attempt),
  UNIQUE (plan_id, claim_id),
  FOREIGN KEY (plan_id, claim_id)
    REFERENCES radroots_runtime_authored_delivery_claims(plan_id, claim_id),
  -- The compatible normalized writer replaces attempt rows inside one
  -- transaction. Require their exact final keys at COMMIT, without cascading
  -- deletion or changing this immutable provenance during that replacement.
  FOREIGN KEY (plan_id, attempt)
    REFERENCES radroots_runtime_authored_delivery_attempts(plan_id, attempt)
    DEFERRABLE INITIALLY DEFERRED
) STRICT, WITHOUT ROWID;

CREATE TRIGGER radroots_runtime_authored_delivery_reconciliations_update_guard
BEFORE UPDATE ON radroots_runtime_authored_delivery_reconciliations
BEGIN
  SELECT RAISE(ABORT, 'delivery reconciliation provenance is immutable');
END;

CREATE TRIGGER radroots_runtime_authored_delivery_reconciliations_delete_guard
BEFORE DELETE ON radroots_runtime_authored_delivery_reconciliations
BEGIN
  SELECT RAISE(ABORT, 'delivery reconciliation provenance is retained');
END;

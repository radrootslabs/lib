-- Keep prior snapshots, receipts and the v11 signed/raw CHECK unchanged.
-- A stopped artifact can retain signature facts without regaining scheduling authority.
ALTER TABLE radroots_runtime_authored_artifacts
ADD COLUMN signing_stop TEXT CHECK(signing_stop IS NULL OR (
  signing_stop IN ('cancelled', 'failed_terminal')
  AND origin = 'planned'
  AND signing_state = 'signed'
  AND admission_state = 'pending'
  AND signing_claim_token IS NULL
  AND admission_claim_token IS NULL
  AND retry_not_before_unix_ms IS NULL
));

CREATE TRIGGER radroots_runtime_authored_artifacts_signed_fact_guard
BEFORE UPDATE ON radroots_runtime_authored_artifacts
WHEN
  (OLD.signed_raw_json IS NOT NULL AND (
    NEW.signed_raw_json IS NOT OLD.signed_raw_json
    OR NEW.signed_raw_sha256 IS NOT OLD.signed_raw_sha256
  ))
  OR (OLD.signing_stop IS NOT NULL AND NEW.signing_stop IS NOT OLD.signing_stop)
  OR (OLD.signing_state IN ('cancelled', 'failed_terminal')
    AND NEW.signed_raw_json IS NOT NULL
    AND NEW.signing_stop IS NOT OLD.signing_state)
BEGIN
  SELECT RAISE(ABORT, 'signed facts and signing stops are immutable');
END;

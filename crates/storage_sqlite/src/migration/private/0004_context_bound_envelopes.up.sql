CREATE TABLE radroots_private_envelope_v2_preflight (
  unsupported_v2_rows INTEGER NOT NULL CHECK(unsupported_v2_rows = 0)
) STRICT;

INSERT INTO radroots_private_envelope_v2_preflight(unsupported_v2_rows)
SELECT COUNT(*) FROM radroots_private_artifacts WHERE envelope_version = 2;

DROP TABLE radroots_private_envelope_v2_preflight;

ALTER TABLE radroots_private_artifacts
ADD COLUMN context_fingerprint BLOB
CHECK(context_fingerprint IS NULL OR length(context_fingerprint) = 32);

ALTER TABLE radroots_private_artifacts
ADD COLUMN last_reseal_id BLOB
CHECK(last_reseal_id IS NULL OR length(last_reseal_id) = 16);

ALTER TABLE radroots_private_artifacts
ADD COLUMN last_reseal_fingerprint BLOB
CHECK(last_reseal_fingerprint IS NULL OR length(last_reseal_fingerprint) = 32);

CREATE TABLE radroots_private_envelope_reseals (
  reseal_id BLOB PRIMARY KEY NOT NULL CHECK(length(reseal_id) = 16),
  artifact_id BLOB NOT NULL CHECK(length(artifact_id) = 16),
  request_fingerprint BLOB NOT NULL CHECK(length(request_fingerprint) = 32),
  expected_revision INTEGER NOT NULL CHECK(expected_revision > 0),
  expected_commitment BLOB NOT NULL CHECK(length(expected_commitment) = 32),
  committed_revision INTEGER NOT NULL CHECK(committed_revision = expected_revision + 1),
  next_commitment BLOB NOT NULL CHECK(length(next_commitment) = 32),
  committed_at_unix_ms INTEGER NOT NULL CHECK(committed_at_unix_ms > 0),
  FOREIGN KEY(artifact_id) REFERENCES radroots_private_artifacts(artifact_id),
  UNIQUE(artifact_id, committed_revision),
  UNIQUE(artifact_id, request_fingerprint)
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_private_envelope_reseals_artifact_idx
ON radroots_private_envelope_reseals(artifact_id, committed_revision);

CREATE TRIGGER radroots_private_envelope_reseals_update_guard
BEFORE UPDATE ON radroots_private_envelope_reseals
BEGIN
  SELECT RAISE(ABORT, 'private envelope reseal evidence is immutable');
END;

CREATE TRIGGER radroots_private_envelope_reseals_delete_guard
BEFORE DELETE ON radroots_private_envelope_reseals
BEGIN
  SELECT RAISE(ABORT, 'private envelope reseal evidence is retained');
END;

DROP TRIGGER radroots_private_artifacts_identity_guard;
DROP TRIGGER radroots_private_artifacts_envelope_guard;

CREATE TRIGGER radroots_private_artifacts_identity_guard
BEFORE UPDATE OF artifact_id, artifact_kind, schema_id, created_at_unix_ms
ON radroots_private_artifacts
BEGIN
  SELECT RAISE(ABORT, 'private artifact identity is immutable');
END;

CREATE TRIGGER radroots_private_artifacts_insert_envelope_guard
BEFORE INSERT ON radroots_private_artifacts
WHEN NOT (
  (NEW.encrypted_envelope IS NULL
    AND NEW.envelope_version IS NULL
    AND NEW.context_fingerprint IS NULL
    AND NEW.last_reseal_id IS NULL
    AND NEW.last_reseal_fingerprint IS NULL)
  OR
  (NEW.encrypted_envelope IS NOT NULL
    AND NEW.envelope_version = 2
    AND NEW.context_fingerprint IS NOT NULL
    AND NEW.last_reseal_id IS NULL
    AND NEW.last_reseal_fingerprint IS NULL)
)
BEGIN
  SELECT RAISE(ABORT, 'new private envelopes must be context-bound v2');
END;

CREATE TRIGGER radroots_private_artifacts_envelope_guard
BEFORE UPDATE OF
  commitment,
  protected_size_bytes,
  secret_provider,
  secret_reference,
  key_version,
  envelope_version,
  encrypted_envelope,
  context_fingerprint,
  last_reseal_id,
  last_reseal_fingerprint
ON radroots_private_artifacts
WHEN NOT (
  (
    OLD.encrypted_envelope IS NULL
    AND OLD.envelope_version IS NULL
    AND OLD.context_fingerprint IS NULL
    AND NEW.encrypted_envelope IS NOT NULL
    AND NEW.envelope_version = 2
    AND NEW.context_fingerprint IS NOT NULL
    AND NEW.last_reseal_id IS NULL
    AND NEW.last_reseal_fingerprint IS NULL
    AND NEW.commitment = OLD.commitment
    AND NEW.protected_size_bytes = OLD.protected_size_bytes
    AND NEW.secret_provider = OLD.secret_provider
    AND NEW.secret_reference = OLD.secret_reference
    AND NEW.key_version = OLD.key_version
    AND NEW.revision = OLD.revision
    AND NEW.stage = OLD.stage
    AND NEW.updated_at_unix_ms = OLD.updated_at_unix_ms
  )
  OR
  (
    OLD.encrypted_envelope IS NULL
    AND OLD.envelope_version IS NULL
    AND OLD.context_fingerprint IS NULL
    AND NEW.encrypted_envelope IS NULL
    AND NEW.envelope_version IS NULL
    AND NEW.context_fingerprint IS NULL
    AND NEW.last_reseal_id IS NOT NULL
    AND NEW.last_reseal_id IS NOT OLD.last_reseal_id
    AND NEW.last_reseal_fingerprint IS NOT NULL
    AND NEW.revision = OLD.revision + 1
    AND NEW.stage = 'active'
    AND NEW.updated_at_unix_ms > OLD.updated_at_unix_ms
    AND NEW.deleted_at_unix_ms IS NULL
    AND NEW.deletion_reason IS NULL
    AND NEW.tombstone_commitment IS NULL
  )
  OR
  (
    OLD.encrypted_envelope IS NOT NULL
    AND OLD.envelope_version = 1
    AND OLD.context_fingerprint IS NULL
    AND OLD.last_reseal_id IS NULL
    AND OLD.last_reseal_fingerprint IS NULL
    AND OLD.stage = 'active'
    AND NEW.encrypted_envelope IS NOT NULL
    AND NEW.envelope_version = 2
    AND NEW.context_fingerprint IS NOT NULL
    AND NEW.last_reseal_id IS NOT NULL
    AND NEW.last_reseal_fingerprint IS NOT NULL
    AND NEW.revision = OLD.revision + 1
    AND NEW.stage = 'active'
    AND NEW.updated_at_unix_ms > OLD.updated_at_unix_ms
    AND NEW.deleted_at_unix_ms IS NULL
    AND NEW.deletion_reason IS NULL
    AND NEW.tombstone_commitment IS NULL
  )
  OR
  (
    OLD.encrypted_envelope IS NOT NULL
    AND NEW.encrypted_envelope IS NULL
    AND NEW.envelope_version IS NULL
    AND NEW.context_fingerprint = OLD.context_fingerprint
    AND NEW.last_reseal_id IS OLD.last_reseal_id
    AND NEW.last_reseal_fingerprint IS OLD.last_reseal_fingerprint
    AND NEW.commitment = OLD.commitment
    AND NEW.protected_size_bytes = OLD.protected_size_bytes
    AND NEW.secret_provider = OLD.secret_provider
    AND NEW.secret_reference = OLD.secret_reference
    AND NEW.key_version = OLD.key_version
    AND NEW.revision = OLD.revision + 1
    AND NEW.stage = 'tombstoned'
    AND NEW.updated_at_unix_ms > OLD.updated_at_unix_ms
  )
)
BEGIN
  SELECT RAISE(ABORT, 'private envelope mutation is not an authorized transition');
END;

CREATE TRIGGER radroots_private_artifacts_reseal_audit
AFTER UPDATE OF last_reseal_id, last_reseal_fingerprint ON radroots_private_artifacts
WHEN NEW.last_reseal_id IS NOT NULL AND NEW.last_reseal_id IS NOT OLD.last_reseal_id
BEGIN
  INSERT INTO radroots_private_envelope_reseals (
    reseal_id,
    artifact_id,
    request_fingerprint,
    expected_revision,
    expected_commitment,
    committed_revision,
    next_commitment,
    committed_at_unix_ms
  ) VALUES (
    NEW.last_reseal_id,
    NEW.artifact_id,
    NEW.last_reseal_fingerprint,
    OLD.revision,
    OLD.commitment,
    NEW.revision,
    NEW.commitment,
    NEW.updated_at_unix_ms
  );
END;

CREATE TABLE radroots_private_artifacts (
  artifact_id BLOB PRIMARY KEY NOT NULL CHECK (length(artifact_id) = 16),
  artifact_kind TEXT NOT NULL CHECK (length(artifact_kind) BETWEEN 1 AND 128),
  schema_id TEXT NOT NULL CHECK (length(schema_id) BETWEEN 1 AND 128),
  commitment BLOB NOT NULL CHECK (length(commitment) = 32),
  protected_size_bytes INTEGER NOT NULL CHECK (protected_size_bytes > 0),
  secret_provider TEXT NOT NULL CHECK (length(secret_provider) BETWEEN 1 AND 64),
  secret_reference TEXT NOT NULL CHECK (length(secret_reference) BETWEEN 1 AND 512),
  key_version INTEGER NOT NULL CHECK (key_version BETWEEN 1 AND 4294967295),
  envelope_version INTEGER CHECK (envelope_version IS NULL OR envelope_version > 0),
  encrypted_envelope BLOB,
  delete_not_before_unix_ms INTEGER CHECK (delete_not_before_unix_ms > 0),
  expires_at_unix_ms INTEGER CHECK (expires_at_unix_ms > 0),
  revision INTEGER NOT NULL CHECK (revision > 0),
  stage TEXT NOT NULL CHECK (stage IN ('active', 'expired', 'tombstoned')),
  created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms > 0),
  updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= created_at_unix_ms),
  deleted_at_unix_ms INTEGER,
  deletion_reason TEXT CHECK (
    deletion_reason IS NULL OR deletion_reason IN (
      'user_requested',
      'retention_expired',
      'key_revoked',
      'integrity_failure',
      'operator_requested'
    )
  ),
  tombstone_commitment BLOB CHECK (
    tombstone_commitment IS NULL OR length(tombstone_commitment) = 32
  ),
  CHECK (
    (encrypted_envelope IS NULL AND envelope_version IS NULL)
    OR (
      encrypted_envelope IS NOT NULL
      AND envelope_version IS NOT NULL
      AND length(encrypted_envelope) = protected_size_bytes
    )
  ),
  CHECK (
    (stage <> 'tombstoned' AND deleted_at_unix_ms IS NULL
      AND deletion_reason IS NULL AND tombstone_commitment IS NULL)
    OR (stage = 'tombstoned' AND deleted_at_unix_ms = updated_at_unix_ms
      AND deletion_reason IS NOT NULL AND tombstone_commitment = commitment
      AND encrypted_envelope IS NULL AND envelope_version IS NULL)
  )
) STRICT, WITHOUT ROWID;

CREATE INDEX radroots_private_artifacts_expiry_idx
ON radroots_private_artifacts(stage, expires_at_unix_ms, artifact_id)
WHERE stage = 'active' AND expires_at_unix_ms IS NOT NULL;

CREATE INDEX radroots_private_artifacts_kind_idx
ON radroots_private_artifacts(artifact_kind, schema_id, stage, artifact_id);

CREATE INDEX radroots_private_artifacts_key_version_idx
ON radroots_private_artifacts(secret_provider, key_version, stage, artifact_id);

CREATE TRIGGER radroots_private_artifacts_delete_guard
BEFORE DELETE ON radroots_private_artifacts
BEGIN
  SELECT RAISE(ABORT, 'private artifacts require durable tombstones');
END;

CREATE TRIGGER radroots_private_artifacts_identity_guard
BEFORE UPDATE OF
  artifact_id,
  artifact_kind,
  schema_id,
  commitment,
  protected_size_bytes,
  secret_provider,
  secret_reference,
  key_version,
  created_at_unix_ms
ON radroots_private_artifacts
BEGIN
  SELECT RAISE(ABORT, 'private artifact identity is immutable');
END;

CREATE TRIGGER radroots_private_artifacts_envelope_guard
BEFORE UPDATE OF encrypted_envelope, envelope_version ON radroots_private_artifacts
WHEN OLD.encrypted_envelope IS NOT NULL AND NEW.encrypted_envelope IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'private artifact envelopes are immutable');
END;

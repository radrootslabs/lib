CREATE TABLE IF NOT EXISTS private_metadata (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  schema_version INTEGER NOT NULL CHECK(schema_version = 1),
  profile_id BLOB NOT NULL CHECK(length(profile_id) = 16),
  runtime_contract_hash BLOB NOT NULL CHECK(length(runtime_contract_hash) = 32),
  key_version INTEGER NOT NULL CHECK(key_version > 0),
  sqlite_source_id TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS wrapped_profile_key (
  key_version INTEGER PRIMARY KEY CHECK(key_version > 0),
  credential_backend TEXT NOT NULL,
  wrapped_key BLOB NOT NULL,
  nonce BLOB NOT NULL CHECK(length(nonce) = 24),
  created_at_ms INTEGER NOT NULL,
  retired_at_ms INTEGER
) STRICT;

CREATE TABLE IF NOT EXISTS wrapped_signing_secret (
  account_id BLOB PRIMARY KEY CHECK(length(account_id) = 16),
  public_key BLOB NOT NULL UNIQUE CHECK(length(public_key) = 32),
  key_version INTEGER NOT NULL REFERENCES wrapped_profile_key(key_version),
  ciphertext BLOB NOT NULL,
  nonce BLOB NOT NULL CHECK(length(nonce) = 24),
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS private_farm_location (
  farm_kind INTEGER NOT NULL CHECK(farm_kind = 30340),
  owner_pubkey BLOB NOT NULL CHECK(length(owner_pubkey) = 32),
  farm_d_tag TEXT NOT NULL,
  key_version INTEGER NOT NULL REFERENCES wrapped_profile_key(key_version),
  ciphertext BLOB NOT NULL,
  nonce BLOB NOT NULL CHECK(length(nonce) = 24),
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY(farm_kind, owner_pubkey, farm_d_tag)
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS private_trade_artifacts (
  artifact_id TEXT PRIMARY KEY NOT NULL,
  trade_id TEXT NOT NULL CHECK(length(trade_id) = 32),
  candidate_id TEXT CHECK(candidate_id IS NULL OR length(candidate_id) = 64),
  artifact_kind TEXT NOT NULL CHECK(artifact_kind IN ('binding_terms','message','contact_bundle','delivery_instruction')),
  schema_id TEXT NOT NULL,
  ciphertext_commitment TEXT NOT NULL CHECK(length(ciphertext_commitment) = 64),
  key_version INTEGER NOT NULL REFERENCES wrapped_profile_key(key_version),
  ciphertext BLOB NOT NULL,
  encryption_metadata BLOB NOT NULL,
  retention_class TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  expires_at_ms INTEGER,
  deleted_at_ms INTEGER,
  UNIQUE(artifact_kind, ciphertext_commitment)
) STRICT;

CREATE INDEX IF NOT EXISTS private_trade_artifacts_trade_idx
  ON private_trade_artifacts(trade_id, candidate_id, artifact_kind, deleted_at_ms);

CREATE INDEX IF NOT EXISTS private_trade_artifacts_expiry_idx
  ON private_trade_artifacts(expires_at_ms, artifact_id)
  WHERE expires_at_ms IS NOT NULL AND deleted_at_ms IS NULL;

CREATE TABLE IF NOT EXISTS cursor_hmac_key (
  key_id BLOB PRIMARY KEY CHECK(length(key_id) = 16),
  key_version INTEGER NOT NULL REFERENCES wrapped_profile_key(key_version),
  ciphertext BLOB NOT NULL,
  nonce BLOB NOT NULL CHECK(length(nonce) = 24),
  created_at_ms INTEGER NOT NULL,
  retired_at_ms INTEGER
) STRICT;

CREATE TABLE IF NOT EXISTS nip46_session_private (
  session_id BLOB PRIMARY KEY CHECK(length(session_id) = 16),
  user_pubkey BLOB NOT NULL CHECK(length(user_pubkey) = 32),
  remote_signer_pubkey BLOB NOT NULL CHECK(length(remote_signer_pubkey) = 32),
  client_pubkey BLOB NOT NULL CHECK(length(client_pubkey) = 32),
  key_version INTEGER NOT NULL REFERENCES wrapped_profile_key(key_version),
  ciphertext BLOB NOT NULL,
  nonce BLOB NOT NULL CHECK(length(nonce) = 24),
  expires_at_ms INTEGER NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('active','expired','revoked')),
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS key_rotation_progress (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  from_key_version INTEGER NOT NULL,
  to_key_version INTEGER NOT NULL,
  table_name TEXT NOT NULL,
  last_primary_key BLOB,
  state TEXT NOT NULL CHECK(state IN ('running','verifying','complete','failed')),
  started_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  error_code TEXT
) STRICT;

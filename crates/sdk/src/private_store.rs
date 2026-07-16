#![cfg(feature = "runtime")]

use crate::RadrootsSdkError;
use radroots_event::ids::{RadrootsAddressableCoordinate, RadrootsAddressableCoordinateParts};
use radroots_event::kinds::KIND_FARM;
use radroots_event::trade::RADROOTS_TRADE_MAX_PRIVATE_ARTIFACT_BYTES;
use radroots_protected_store::{RadrootsProtectedFileKeySource, RadrootsProtectedStoreEnvelope};
use radroots_secret_vault::{RadrootsSecretKeyWrapping, RadrootsSecretVaultAccessError};
use radroots_trade::workflow::{
    RadrootsTradePrivateTermsEvidenceV1, RadrootsTradePrivateTermsStateV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

pub(crate) const SDK_PRIVATE_STORE_SCHEMA_VERSION: i64 = 1;

const PRIVATE_STORE_KEY_VERSION: i64 = 1;
const PRIVATE_STORE_FILE_CREDENTIAL_BACKEND: &str = "protected_file_wrapped_v1";
const PRIVATE_STORE_MEMORY_CREDENTIAL_BACKEND: &str = "memory_test_wrapped_v1";

const PRIVATE_STORE_MIGRATION_UP: &str = r#"
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
"#;

#[derive(Clone)]
pub(crate) struct SdkPrivateStore {
    pool: SqlitePool,
    key_source: SdkPrivateStoreKeySource,
    credential_backend: &'static str,
}

#[derive(Clone)]
enum SdkPrivateStoreKeySource {
    Memory(Arc<SdkPrivateStoreMemoryKeySource>),
    File(RadrootsProtectedFileKeySource),
}

#[derive(Default)]
struct SdkPrivateStoreMemoryKeySource;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SdkPrivateFarmLocationRecord {
    pub farm_addr: RadrootsAddressableCoordinate,
    pub farm_pubkey: String,
    pub farm_d_tag: String,
    pub label: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub locality_primary: String,
    pub locality_city: Option<String>,
    pub locality_region: Option<String>,
    pub locality_country: Option<String>,
    pub geohash5: String,
    pub geonames_feature_id: Option<i64>,
    pub geonames_country_id: Option<String>,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SdkPrivateStoreStatusSummary {
    pub farm_private_locations: i64,
    pub trade_private_artifacts: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct SdkPrivateFarmLocationPayload {
    label: Option<String>,
    latitude: f64,
    longitude: f64,
    locality_primary: String,
    locality_city: Option<String>,
    locality_region: Option<String>,
    locality_country: Option<String>,
    geohash5: String,
    geonames_feature_id: Option<i64>,
    geonames_country_id: Option<String>,
    updated_at_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SdkPrivateTradeArtifactKind {
    BindingTerms,
    Message,
    ContactBundle,
    DeliveryInstruction,
}

impl SdkPrivateTradeArtifactKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::BindingTerms => "binding_terms",
            Self::Message => "message",
            Self::ContactBundle => "contact_bundle",
            Self::DeliveryInstruction => "delivery_instruction",
        }
    }

    fn from_str(value: &str) -> Result<Self, RadrootsSdkError> {
        match value {
            "binding_terms" => Ok(Self::BindingTerms),
            "message" => Ok(Self::Message),
            "contact_bundle" => Ok(Self::ContactBundle),
            "delivery_instruction" => Ok(Self::DeliveryInstruction),
            _ => Err(RadrootsSdkError::PrivateStore {
                message: format!("unknown private trade artifact kind `{value}`"),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SdkPrivateTradeArtifactInput {
    pub artifact_id: String,
    pub trade_id: String,
    pub candidate_id: Option<String>,
    pub artifact_kind: SdkPrivateTradeArtifactKind,
    pub schema_id: String,
    pub plaintext: Vec<u8>,
    pub retention_class: String,
    pub created_at_ms: i64,
    pub expires_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SdkPrivateTradeArtifactRecord {
    pub artifact_id: String,
    pub trade_id: String,
    pub candidate_id: Option<String>,
    pub artifact_kind: SdkPrivateTradeArtifactKind,
    pub schema_id: String,
    pub ciphertext_commitment: String,
    pub plaintext: Vec<u8>,
    pub retention_class: String,
    pub created_at_ms: i64,
    pub expires_at_ms: Option<i64>,
    pub deleted_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SdkPrivateTradeArtifactMetadata {
    pub artifact_id: String,
    pub trade_id: String,
    pub candidate_id: Option<String>,
    pub artifact_kind: SdkPrivateTradeArtifactKind,
    pub schema_id: String,
    pub ciphertext_commitment: String,
    pub retention_class: String,
    pub created_at_ms: i64,
    pub expires_at_ms: Option<i64>,
    pub deleted_at_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SdkPrivateTradeArtifactPayload {
    artifact_id: String,
    trade_id: String,
    artifact_kind: SdkPrivateTradeArtifactKind,
    schema_id: String,
    plaintext: Vec<u8>,
    retention_class: String,
    created_at_ms: i64,
    expires_at_ms: Option<i64>,
}

impl SdkPrivateStore {
    pub async fn open_memory() -> Result<Self, RadrootsSdkError> {
        let options =
            SqliteConnectOptions::from_str("sqlite::memory:").map_err(private_store_error)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(private_store_error)?;
        let store = Self {
            pool,
            key_source: SdkPrivateStoreKeySource::Memory(Arc::default()),
            credential_backend: PRIVATE_STORE_MEMORY_CREDENTIAL_BACKEND,
        };
        store.configure_connection(false).await?;
        store.apply_up().await?;
        Ok(store)
    }

    pub async fn open_file(path: impl AsRef<Path>) -> Result<Self, RadrootsSdkError> {
        let path = path.as_ref();
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(private_store_error)?;
        let store = Self {
            pool,
            key_source: SdkPrivateStoreKeySource::File(
                RadrootsProtectedFileKeySource::from_sidecar_suffix(path, ".vault.key"),
            ),
            credential_backend: PRIVATE_STORE_FILE_CREDENTIAL_BACKEND,
        };
        store.configure_connection(true).await?;
        store.reject_pre_v1_private_store().await?;
        store.apply_up().await?;
        Ok(store)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn pragma_foreign_keys(&self) -> Result<i64, RadrootsSdkError> {
        query_i64(&self.pool, "PRAGMA foreign_keys").await
    }

    pub async fn pragma_busy_timeout(&self) -> Result<i64, RadrootsSdkError> {
        query_i64(&self.pool, "PRAGMA busy_timeout").await
    }

    pub async fn pragma_journal_mode(&self) -> Result<String, RadrootsSdkError> {
        query_string(&self.pool, "PRAGMA journal_mode").await
    }

    pub async fn status_summary(&self) -> Result<SdkPrivateStoreStatusSummary, RadrootsSdkError> {
        Ok(SdkPrivateStoreStatusSummary {
            farm_private_locations: query_i64(
                &self.pool,
                "SELECT COUNT(*) FROM private_farm_location",
            )
            .await?,
            trade_private_artifacts: query_i64(
                &self.pool,
                "SELECT COUNT(*) FROM private_trade_artifacts WHERE deleted_at_ms IS NULL",
            )
            .await?,
        })
    }

    pub async fn upsert_farm_location(
        &self,
        record: &SdkPrivateFarmLocationRecord,
    ) -> Result<(), RadrootsSdkError> {
        validate_location_record(record)?;
        let parts = farm_location_parts(&record.farm_addr)?;
        let owner_pubkey = public_key_bytes(parts.pubkey.as_str())?;
        let envelope = self.seal_farm_location(record)?;
        let nonce = envelope.header.nonce.to_vec();
        let ciphertext = envelope.encode_json().map_err(private_store_error)?;
        sqlx::query(
            r#"
            INSERT INTO private_farm_location (
              farm_kind,
              owner_pubkey,
              farm_d_tag,
              key_version,
              ciphertext,
              nonce,
              created_at_ms,
              updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(farm_kind, owner_pubkey, farm_d_tag) DO UPDATE SET
              key_version = excluded.key_version,
              ciphertext = excluded.ciphertext,
              nonce = excluded.nonce,
              updated_at_ms = excluded.updated_at_ms
            "#,
        )
        .bind(i64::from(KIND_FARM))
        .bind(owner_pubkey)
        .bind(parts.d_tag.as_str())
        .bind(PRIVATE_STORE_KEY_VERSION)
        .bind(ciphertext)
        .bind(nonce)
        .bind(record.updated_at_ms)
        .bind(record.updated_at_ms)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(private_store_error)
    }

    pub async fn farm_location(
        &self,
        farm_addr: &RadrootsAddressableCoordinate,
    ) -> Result<Option<SdkPrivateFarmLocationRecord>, RadrootsSdkError> {
        let parts = farm_location_parts(farm_addr)?;
        let owner_pubkey = public_key_bytes(parts.pubkey.as_str())?;
        let row = sqlx::query(
            r#"
            SELECT ciphertext, nonce
            FROM private_farm_location
            WHERE farm_kind = ?1 AND owner_pubkey = ?2 AND farm_d_tag = ?3
            "#,
        )
        .bind(i64::from(KIND_FARM))
        .bind(owner_pubkey)
        .bind(parts.d_tag.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(private_store_error)?;
        row.map(|row| self.private_farm_location_from_row(farm_addr.clone(), parts, row))
            .transpose()
    }

    pub async fn delete_farm_location(
        &self,
        farm_addr: &RadrootsAddressableCoordinate,
    ) -> Result<bool, RadrootsSdkError> {
        let parts = farm_location_parts(farm_addr)?;
        let owner_pubkey = public_key_bytes(parts.pubkey.as_str())?;
        sqlx::query(
            r#"
            DELETE FROM private_farm_location
            WHERE farm_kind = ?1 AND owner_pubkey = ?2 AND farm_d_tag = ?3
            "#,
        )
        .bind(i64::from(KIND_FARM))
        .bind(owner_pubkey)
        .bind(parts.d_tag.as_str())
        .execute(&self.pool)
        .await
        .map(|receipt| receipt.rows_affected() > 0)
        .map_err(private_store_error)
    }

    pub async fn upsert_trade_artifact(
        &self,
        input: &SdkPrivateTradeArtifactInput,
    ) -> Result<SdkPrivateTradeArtifactMetadata, RadrootsSdkError> {
        validate_trade_artifact_input(input)?;
        let envelope = self.seal_trade_artifact(input)?;
        let nonce = envelope.header.nonce.to_vec();
        let ciphertext = envelope.encode_json().map_err(private_store_error)?;
        let ciphertext_commitment = hex::encode(Sha256::digest(ciphertext.as_slice()));
        let encryption_metadata = serde_json::to_vec(&serde_json::json!({
            "key_version": PRIVATE_STORE_KEY_VERSION,
            "nonce": hex::encode(nonce.as_slice())
        }))
        .map_err(private_store_error)?;
        sqlx::query(
            r#"
            INSERT INTO private_trade_artifacts (
              artifact_id,
              trade_id,
              candidate_id,
              artifact_kind,
              schema_id,
              ciphertext_commitment,
              key_version,
              ciphertext,
              encryption_metadata,
              retention_class,
              created_at_ms,
              expires_at_ms,
              deleted_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL)
            ON CONFLICT(artifact_id) DO UPDATE SET
              trade_id = excluded.trade_id,
              candidate_id = excluded.candidate_id,
              artifact_kind = excluded.artifact_kind,
              schema_id = excluded.schema_id,
              ciphertext_commitment = excluded.ciphertext_commitment,
              key_version = excluded.key_version,
              ciphertext = excluded.ciphertext,
              encryption_metadata = excluded.encryption_metadata,
              retention_class = excluded.retention_class,
              created_at_ms = excluded.created_at_ms,
              expires_at_ms = excluded.expires_at_ms,
              deleted_at_ms = NULL
            "#,
        )
        .bind(input.artifact_id.as_str())
        .bind(input.trade_id.as_str())
        .bind(input.candidate_id.as_deref())
        .bind(input.artifact_kind.as_str())
        .bind(input.schema_id.as_str())
        .bind(ciphertext_commitment.as_str())
        .bind(PRIVATE_STORE_KEY_VERSION)
        .bind(ciphertext)
        .bind(encryption_metadata)
        .bind(input.retention_class.as_str())
        .bind(input.created_at_ms)
        .bind(input.expires_at_ms)
        .execute(&self.pool)
        .await
        .map_err(private_store_error)?;
        Ok(SdkPrivateTradeArtifactMetadata {
            artifact_id: input.artifact_id.clone(),
            trade_id: input.trade_id.clone(),
            candidate_id: input.candidate_id.clone(),
            artifact_kind: input.artifact_kind,
            schema_id: input.schema_id.clone(),
            ciphertext_commitment,
            retention_class: input.retention_class.clone(),
            created_at_ms: input.created_at_ms,
            expires_at_ms: input.expires_at_ms,
            deleted_at_ms: None,
        })
    }

    pub async fn trade_artifact(
        &self,
        artifact_id: &str,
    ) -> Result<Option<SdkPrivateTradeArtifactRecord>, RadrootsSdkError> {
        let row = sqlx::query(
            r#"
            SELECT artifact_id, trade_id, candidate_id, artifact_kind, schema_id,
              ciphertext_commitment, ciphertext, retention_class, created_at_ms,
              expires_at_ms, deleted_at_ms
            FROM private_trade_artifacts
            WHERE artifact_id = ?1
            "#,
        )
        .bind(artifact_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(private_store_error)?;
        row.map(|row| self.trade_artifact_from_row(row)).transpose()
    }

    pub async fn trade_artifact_metadata_for_trade(
        &self,
        trade_id: &str,
    ) -> Result<Vec<SdkPrivateTradeArtifactMetadata>, RadrootsSdkError> {
        let rows = sqlx::query(
            r#"
            SELECT artifact_id, trade_id, candidate_id, artifact_kind, schema_id,
              ciphertext_commitment, retention_class, created_at_ms, expires_at_ms,
              deleted_at_ms
            FROM private_trade_artifacts
            WHERE trade_id = ?1
            ORDER BY created_at_ms, artifact_id
            "#,
        )
        .bind(trade_id)
        .fetch_all(&self.pool)
        .await
        .map_err(private_store_error)?;
        rows.into_iter()
            .map(trade_artifact_metadata_from_row)
            .collect()
    }

    pub async fn delete_trade_artifact(
        &self,
        artifact_id: &str,
        deleted_at_ms: i64,
    ) -> Result<bool, RadrootsSdkError> {
        sqlx::query(
            r#"
            UPDATE private_trade_artifacts
            SET deleted_at_ms = ?2
            WHERE artifact_id = ?1 AND deleted_at_ms IS NULL
            "#,
        )
        .bind(artifact_id)
        .bind(deleted_at_ms)
        .execute(&self.pool)
        .await
        .map(|receipt| receipt.rows_affected() > 0)
        .map_err(private_store_error)
    }

    pub async fn private_terms_evidence(
        &self,
        trade_id: &str,
        candidate_id: &str,
        artifact_id: &str,
        schema_id: &str,
        ciphertext_commitment: &str,
    ) -> Result<RadrootsTradePrivateTermsEvidenceV1, RadrootsSdkError> {
        let state = match self.trade_artifact(artifact_id).await? {
            None => RadrootsTradePrivateTermsStateV1::Missing,
            Some(record) => {
                if record.deleted_at_ms.is_some()
                    || record.trade_id != trade_id
                    || record.artifact_kind != SdkPrivateTradeArtifactKind::BindingTerms
                {
                    RadrootsTradePrivateTermsStateV1::Missing
                } else if record
                    .candidate_id
                    .as_deref()
                    .is_some_and(|stored_candidate_id| stored_candidate_id != candidate_id)
                {
                    RadrootsTradePrivateTermsStateV1::Missing
                } else if record.schema_id != schema_id
                    || record.ciphertext_commitment != ciphertext_commitment
                {
                    RadrootsTradePrivateTermsStateV1::CommitmentMismatch
                } else {
                    RadrootsTradePrivateTermsStateV1::AvailableVerified
                }
            }
        };
        Ok(RadrootsTradePrivateTermsEvidenceV1 {
            candidate_id: candidate_id.parse().map_err(|error| {
                RadrootsSdkError::InvalidRequest {
                    message: format!("private terms candidate id is invalid: {error}"),
                }
            })?,
            state,
        })
    }

    async fn configure_connection(&self, file_backed: bool) -> Result<(), RadrootsSdkError> {
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await
            .map_err(private_store_error)?;
        sqlx::query("PRAGMA busy_timeout = 5000")
            .execute(&self.pool)
            .await
            .map_err(private_store_error)?;
        sqlx::query("PRAGMA trusted_schema = OFF")
            .execute(&self.pool)
            .await
            .map_err(private_store_error)?;
        sqlx::query("PRAGMA temp_store = MEMORY")
            .execute(&self.pool)
            .await
            .map_err(private_store_error)?;
        sqlx::query("PRAGMA secure_delete = FAST")
            .execute(&self.pool)
            .await
            .map_err(private_store_error)?;
        if file_backed {
            sqlx::query("PRAGMA journal_mode = WAL")
                .execute(&self.pool)
                .await
                .map_err(private_store_error)?;
        }
        Ok(())
    }

    async fn apply_up(&self) -> Result<(), RadrootsSdkError> {
        sqlx::raw_sql(PRIVATE_STORE_MIGRATION_UP)
            .execute(&self.pool)
            .await
            .map_err(private_store_error)?;
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO private_metadata (
              singleton,
              schema_version,
              profile_id,
              runtime_contract_hash,
              key_version,
              sqlite_source_id,
              created_at_ms,
              updated_at_ms
            ) VALUES (
              1,
              1,
              randomblob(16),
              zeroblob(32),
              1,
              sqlite_source_id(),
              CAST(strftime('%s','now') AS INTEGER) * 1000,
              CAST(strftime('%s','now') AS INTEGER) * 1000
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(private_store_error)?;
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO wrapped_profile_key (
              key_version,
              credential_backend,
              wrapped_key,
              nonce,
              created_at_ms,
              retired_at_ms
            ) VALUES (
              1,
              ?1,
              zeroblob(1),
              randomblob(24),
              CAST(strftime('%s','now') AS INTEGER) * 1000,
              NULL
            )
            "#,
        )
        .bind(self.credential_backend)
        .execute(&self.pool)
        .await
        .map_err(private_store_error)?;
        sqlx::query("PRAGMA user_version = 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(private_store_error)
    }

    async fn reject_pre_v1_private_store(&self) -> Result<(), RadrootsSdkError> {
        let exists = query_i64(
            &self.pool,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'sdk_private_farm_location'",
        )
        .await?;
        if exists != 0 {
            return Err(RadrootsSdkError::UnsupportedProfileSchema {
                path: Path::new("private.sqlite").to_path_buf(),
                message: "pre-V1 SDK private store is unsupported by release-product v1".to_owned(),
            });
        }
        Ok(())
    }

    fn seal_farm_location(
        &self,
        record: &SdkPrivateFarmLocationRecord,
    ) -> Result<RadrootsProtectedStoreEnvelope, RadrootsSdkError> {
        let payload = SdkPrivateFarmLocationPayload {
            label: record.label.clone(),
            latitude: record.latitude,
            longitude: record.longitude,
            locality_primary: record.locality_primary.clone(),
            locality_city: record.locality_city.clone(),
            locality_region: record.locality_region.clone(),
            locality_country: record.locality_country.clone(),
            geohash5: record.geohash5.clone(),
            geonames_feature_id: record.geonames_feature_id,
            geonames_country_id: record.geonames_country_id.clone(),
            updated_at_ms: record.updated_at_ms,
        };
        let plaintext = serde_json::to_vec(&payload).map_err(private_store_error)?;
        RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(
            &self.key_source,
            farm_location_key_slot(record.farm_addr.as_str()).as_str(),
            plaintext.as_slice(),
        )
        .map_err(private_store_error)
    }

    fn seal_trade_artifact(
        &self,
        input: &SdkPrivateTradeArtifactInput,
    ) -> Result<RadrootsProtectedStoreEnvelope, RadrootsSdkError> {
        let payload = SdkPrivateTradeArtifactPayload {
            artifact_id: input.artifact_id.clone(),
            trade_id: input.trade_id.clone(),
            artifact_kind: input.artifact_kind,
            schema_id: input.schema_id.clone(),
            plaintext: input.plaintext.clone(),
            retention_class: input.retention_class.clone(),
            created_at_ms: input.created_at_ms,
            expires_at_ms: input.expires_at_ms,
        };
        let plaintext = serde_json::to_vec(&payload).map_err(private_store_error)?;
        RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(
            &self.key_source,
            trade_artifact_key_slot(input.artifact_id.as_str()).as_str(),
            plaintext.as_slice(),
        )
        .map_err(private_store_error)
    }

    fn private_farm_location_from_row(
        &self,
        farm_addr: RadrootsAddressableCoordinate,
        parts: RadrootsAddressableCoordinateParts,
        row: sqlx::sqlite::SqliteRow,
    ) -> Result<SdkPrivateFarmLocationRecord, RadrootsSdkError> {
        let ciphertext: Vec<u8> = row.try_get("ciphertext").map_err(private_store_error)?;
        let nonce: Vec<u8> = row.try_get("nonce").map_err(private_store_error)?;
        let envelope = RadrootsProtectedStoreEnvelope::decode_json(ciphertext.as_slice())
            .map_err(private_store_error)?;
        if envelope.header.nonce.as_slice() != nonce.as_slice() {
            return Err(RadrootsSdkError::PrivateStore {
                message: "private farm location envelope nonce does not match row nonce".to_owned(),
            });
        }
        let plaintext = envelope
            .open_with_wrapped_key(&self.key_source)
            .map_err(private_store_error)?;
        let payload: SdkPrivateFarmLocationPayload =
            serde_json::from_slice(plaintext.as_slice()).map_err(private_store_error)?;
        Ok(SdkPrivateFarmLocationRecord {
            farm_addr,
            farm_pubkey: parts.pubkey.as_str().to_owned(),
            farm_d_tag: parts.d_tag.as_str().to_owned(),
            label: payload.label,
            latitude: payload.latitude,
            longitude: payload.longitude,
            locality_primary: payload.locality_primary,
            locality_city: payload.locality_city,
            locality_region: payload.locality_region,
            locality_country: payload.locality_country,
            geohash5: payload.geohash5,
            geonames_feature_id: payload.geonames_feature_id,
            geonames_country_id: payload.geonames_country_id,
            updated_at_ms: payload.updated_at_ms,
        })
    }

    fn trade_artifact_from_row(
        &self,
        row: sqlx::sqlite::SqliteRow,
    ) -> Result<SdkPrivateTradeArtifactRecord, RadrootsSdkError> {
        let artifact_id: String = row.try_get("artifact_id").map_err(private_store_error)?;
        let trade_id: String = row.try_get("trade_id").map_err(private_store_error)?;
        let candidate_id: Option<String> =
            row.try_get("candidate_id").map_err(private_store_error)?;
        let artifact_kind = SdkPrivateTradeArtifactKind::from_str(
            row.try_get::<String, _>("artifact_kind")
                .map_err(private_store_error)?
                .as_str(),
        )?;
        let schema_id: String = row.try_get("schema_id").map_err(private_store_error)?;
        let ciphertext_commitment: String = row
            .try_get("ciphertext_commitment")
            .map_err(private_store_error)?;
        let ciphertext: Vec<u8> = row.try_get("ciphertext").map_err(private_store_error)?;
        if hex::encode(Sha256::digest(ciphertext.as_slice())) != ciphertext_commitment {
            return Err(RadrootsSdkError::PrivateStore {
                message:
                    "private trade artifact ciphertext commitment does not match row commitment"
                        .to_owned(),
            });
        }
        let envelope = RadrootsProtectedStoreEnvelope::decode_json(ciphertext.as_slice())
            .map_err(private_store_error)?;
        let plaintext = envelope
            .open_with_wrapped_key(&self.key_source)
            .map_err(private_store_error)?;
        let payload: SdkPrivateTradeArtifactPayload =
            serde_json::from_slice(plaintext.as_slice()).map_err(private_store_error)?;
        if payload.artifact_id != artifact_id
            || payload.trade_id != trade_id
            || payload.artifact_kind != artifact_kind
            || payload.schema_id != schema_id
        {
            return Err(RadrootsSdkError::PrivateStore {
                message: "private trade artifact envelope metadata does not match row metadata"
                    .to_owned(),
            });
        }
        Ok(SdkPrivateTradeArtifactRecord {
            artifact_id,
            trade_id,
            candidate_id,
            artifact_kind,
            schema_id,
            ciphertext_commitment,
            plaintext: payload.plaintext,
            retention_class: row
                .try_get("retention_class")
                .map_err(private_store_error)?,
            created_at_ms: row.try_get("created_at_ms").map_err(private_store_error)?,
            expires_at_ms: row.try_get("expires_at_ms").map_err(private_store_error)?,
            deleted_at_ms: row.try_get("deleted_at_ms").map_err(private_store_error)?,
        })
    }
}

impl RadrootsSecretKeyWrapping for SdkPrivateStoreKeySource {
    type Error = RadrootsSecretVaultAccessError;

    fn wrap_data_key(&self, key_slot: &str, plaintext_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        match self {
            Self::Memory(source) => source.wrap_data_key(key_slot, plaintext_key),
            Self::File(source) => source.wrap_data_key(key_slot, plaintext_key),
        }
    }

    fn unwrap_data_key(&self, key_slot: &str, wrapped_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        match self {
            Self::Memory(source) => source.unwrap_data_key(key_slot, wrapped_key),
            Self::File(source) => source.unwrap_data_key(key_slot, wrapped_key),
        }
    }
}

impl RadrootsSecretKeyWrapping for SdkPrivateStoreMemoryKeySource {
    type Error = RadrootsSecretVaultAccessError;

    fn wrap_data_key(&self, _key_slot: &str, plaintext_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Ok(plaintext_key.to_vec())
    }

    fn unwrap_data_key(&self, _key_slot: &str, wrapped_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Ok(wrapped_key.to_vec())
    }
}

async fn query_i64(pool: &SqlitePool, sql: &'static str) -> Result<i64, RadrootsSdkError> {
    let row = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .map_err(private_store_error)?;
    row.try_get(0).map_err(private_store_error)
}

async fn query_string(pool: &SqlitePool, sql: &'static str) -> Result<String, RadrootsSdkError> {
    let row = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .map_err(private_store_error)?;
    row.try_get(0).map_err(private_store_error)
}

fn farm_location_parts(
    farm_addr: &RadrootsAddressableCoordinate,
) -> Result<RadrootsAddressableCoordinateParts, RadrootsSdkError> {
    let parts = RadrootsAddressableCoordinateParts::parse(farm_addr.as_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("farm address is invalid: {error}"),
        }
    })?;
    if parts.kind != KIND_FARM {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "private farm location address kind must be {KIND_FARM}, got {}",
                parts.kind
            ),
        });
    }
    Ok(parts)
}

fn public_key_bytes(pubkey: &str) -> Result<Vec<u8>, RadrootsSdkError> {
    hex::decode(pubkey).map_err(|error| RadrootsSdkError::InvalidRequest {
        message: format!("public key is invalid hex: {error}"),
    })
}

fn farm_location_key_slot(farm_addr: &str) -> String {
    format!("private_farm_location:{farm_addr}")
}

fn trade_artifact_key_slot(artifact_id: &str) -> String {
    format!("private_trade_artifact:{artifact_id}")
}

fn validate_location_record(record: &SdkPrivateFarmLocationRecord) -> Result<(), RadrootsSdkError> {
    if !record.latitude.is_finite()
        || !record.longitude.is_finite()
        || record.latitude < -90.0
        || record.latitude > 90.0
        || record.longitude < -180.0
        || record.longitude > 180.0
    {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "farm exact location coordinates are outside valid latitude/longitude bounds"
                .to_owned(),
        });
    }
    if record.locality_primary.trim().is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "farm public locality primary name must not be empty".to_owned(),
        });
    }
    if record.geohash5.len() != 5 {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "farm public locality geohash must be precision 5".to_owned(),
        });
    }
    if record
        .label
        .as_deref()
        .is_some_and(|label| label.trim().is_empty())
    {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "farm private location label must not be empty".to_owned(),
        });
    }
    let parts = farm_location_parts(&record.farm_addr)?;
    if parts.pubkey.as_str() != record.farm_pubkey {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "farm private location address pubkey does not match record pubkey".to_owned(),
        });
    }
    if parts.d_tag.as_str() != record.farm_d_tag {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "farm private location address d tag does not match record d tag".to_owned(),
        });
    }
    Ok(())
}

fn validate_trade_artifact_input(
    input: &SdkPrivateTradeArtifactInput,
) -> Result<(), RadrootsSdkError> {
    for (field, value) in [
        ("artifact_id", input.artifact_id.as_str()),
        ("trade_id", input.trade_id.as_str()),
        ("schema_id", input.schema_id.as_str()),
        ("retention_class", input.retention_class.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!("private trade artifact {field} must not be empty"),
            });
        }
    }
    if input.plaintext.is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "private trade artifact plaintext must not be empty".to_owned(),
        });
    }
    if input.plaintext.len() > RADROOTS_TRADE_MAX_PRIVATE_ARTIFACT_BYTES {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "private trade artifact plaintext exceeds {RADROOTS_TRADE_MAX_PRIVATE_ARTIFACT_BYTES} bytes"
            ),
        });
    }
    if input
        .expires_at_ms
        .is_some_and(|expires| expires <= input.created_at_ms)
    {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "private trade artifact expiration must be after creation time".to_owned(),
        });
    }
    if input.trade_id.len() != 32 || !input.trade_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "private trade artifact trade_id must be 32 hex characters".to_owned(),
        });
    }
    if input.candidate_id.as_deref().is_some_and(|candidate_id| {
        candidate_id.len() != 64 || !candidate_id.chars().all(|c| c.is_ascii_hexdigit())
    }) {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "private trade artifact candidate_id must be 64 hex characters".to_owned(),
        });
    }
    Ok(())
}

fn trade_artifact_metadata_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<SdkPrivateTradeArtifactMetadata, RadrootsSdkError> {
    Ok(SdkPrivateTradeArtifactMetadata {
        artifact_id: row.try_get("artifact_id").map_err(private_store_error)?,
        trade_id: row.try_get("trade_id").map_err(private_store_error)?,
        candidate_id: row.try_get("candidate_id").map_err(private_store_error)?,
        artifact_kind: SdkPrivateTradeArtifactKind::from_str(
            row.try_get::<String, _>("artifact_kind")
                .map_err(private_store_error)?
                .as_str(),
        )?,
        schema_id: row.try_get("schema_id").map_err(private_store_error)?,
        ciphertext_commitment: row
            .try_get("ciphertext_commitment")
            .map_err(private_store_error)?,
        retention_class: row
            .try_get("retention_class")
            .map_err(private_store_error)?,
        created_at_ms: row.try_get("created_at_ms").map_err(private_store_error)?,
        expires_at_ms: row.try_get("expires_at_ms").map_err(private_store_error)?,
        deleted_at_ms: row.try_get("deleted_at_ms").map_err(private_store_error)?,
    })
}

fn private_store_error(error: impl ToString) -> RadrootsSdkError {
    RadrootsSdkError::PrivateStore {
        message: error.to_string(),
    }
}

#[cfg(test)]
#[path = "../tests/unit/private_store_tests.rs"]
mod tests;

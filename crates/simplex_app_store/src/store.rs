use crate::error::RadrootsSimplexAppStoreError;
use crate::model::{
    RadrootsSimplexAppChatDirection, RadrootsSimplexAppChatItem, RadrootsSimplexAppConnection,
    RadrootsSimplexAppContact, RadrootsSimplexAppConversation, RadrootsSimplexAppDiagnostics,
    RadrootsSimplexAppInboundMessageLogEntry, RadrootsSimplexAppOutboxMessage,
    RadrootsSimplexAppProfile, RadrootsSimplexAppQueueEndpoint,
    RadrootsSimplexAppUnsupportedProtocolEvent,
};
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use getrandom::getrandom;
use radroots_secret_vault::RadrootsSecretVault;
#[cfg(feature = "os-keyring")]
use radroots_secret_vault::RadrootsSecretVaultOsKeyring;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, params};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zeroize::Zeroize;

const CURRENT_SCHEMA_VERSION: i64 = 1;
const DEFAULT_KEYCHAIN_SERVICE: &str = "org.radroots.simplex.app-store";
const DATABASE_KEY_BYTES: usize = 32;

pub struct RadrootsSimplexAppStore {
    connection: Connection,
    diagnostics: RadrootsSimplexAppDiagnostics,
}

impl RadrootsSimplexAppStore {
    #[cfg(feature = "os-keyring")]
    pub fn open_keychain_backed(
        path: impl AsRef<Path>,
    ) -> Result<Self, RadrootsSimplexAppStoreError> {
        let path = path.as_ref();
        let key_slot = derived_key_slot(path);
        Self::open_with_vault(
            path,
            Arc::new(RadrootsSecretVaultOsKeyring::new(DEFAULT_KEYCHAIN_SERVICE)),
            key_slot,
            "host_vault",
        )
    }

    pub fn open_with_vault(
        path: impl AsRef<Path>,
        vault: Arc<dyn RadrootsSecretVault>,
        key_slot: impl Into<String>,
        key_source: impl Into<String>,
    ) -> Result<Self, RadrootsSimplexAppStoreError> {
        let path = path.as_ref();
        let key_slot = key_slot.into();
        let key_source = key_source.into();
        let existed = path.exists();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                RadrootsSimplexAppStoreError::Io(format!(
                    "failed to create SimpleX app store directory: {error}"
                ))
            })?;
        }

        let mut key_hex = load_or_create_database_key(vault.as_ref(), &key_slot, existed)?;
        let key_slot_digest = key_slot_digest(&key_slot);
        let mut connection = open_keyed_connection(path, &key_hex)?;
        key_hex.zeroize();
        let cipher = verify_encryption(&connection)?;
        configure_connection(&connection)?;
        migrate(&mut connection, &key_slot_digest, &key_source)?;
        verify_metadata(&connection, &key_slot_digest)?;
        let diagnostics = diagnostics_for(&connection, cipher, key_source, key_slot_digest)?;
        Ok(Self {
            connection,
            diagnostics,
        })
    }

    pub fn diagnostics(&self) -> &RadrootsSimplexAppDiagnostics {
        &self.diagnostics
    }

    pub fn upsert_profile(
        &self,
        profile: &RadrootsSimplexAppProfile,
    ) -> Result<(), RadrootsSimplexAppStoreError> {
        self.connection.execute(
            "INSERT INTO profiles (profile_id, display_name, created_at_unix)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(profile_id) DO UPDATE SET display_name = excluded.display_name",
            params![
                profile.profile_id,
                profile.display_name,
                profile.created_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn get_profile(
        &self,
        profile_id: &str,
    ) -> Result<Option<RadrootsSimplexAppProfile>, RadrootsSimplexAppStoreError> {
        self.connection
            .query_row(
                "SELECT profile_id, display_name, created_at_unix FROM profiles WHERE profile_id = ?1",
                params![profile_id],
                profile_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_profiles(
        &self,
    ) -> Result<Vec<RadrootsSimplexAppProfile>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT profile_id, display_name, created_at_unix FROM profiles ORDER BY profile_id",
        )?;
        collect_rows(statement.query_map([], profile_from_row)?)
    }

    pub fn upsert_contact(
        &self,
        contact: &RadrootsSimplexAppContact,
    ) -> Result<(), RadrootsSimplexAppStoreError> {
        self.connection.execute(
            "INSERT INTO contacts (contact_id, profile_id, display_name, lifecycle, created_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(contact_id) DO UPDATE SET
                display_name = excluded.display_name,
                lifecycle = excluded.lifecycle",
            params![
                contact.contact_id,
                contact.profile_id,
                contact.display_name,
                contact.lifecycle,
                contact.created_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn list_contacts(
        &self,
    ) -> Result<Vec<RadrootsSimplexAppContact>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT contact_id, profile_id, display_name, lifecycle, created_at_unix
             FROM contacts ORDER BY contact_id",
        )?;
        collect_rows(statement.query_map([], contact_from_row)?)
    }

    pub fn upsert_connection(
        &self,
        connection: &RadrootsSimplexAppConnection,
    ) -> Result<(), RadrootsSimplexAppStoreError> {
        self.connection.execute(
            "INSERT INTO connections
                (connection_id, profile_id, contact_id, state, agent_connection_id, created_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(connection_id) DO UPDATE SET
                contact_id = excluded.contact_id,
                state = excluded.state,
                agent_connection_id = excluded.agent_connection_id",
            params![
                connection.connection_id,
                connection.profile_id,
                connection.contact_id,
                connection.state,
                connection.agent_connection_id,
                connection.created_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn list_connections_by_state(
        &self,
        state: &str,
    ) -> Result<Vec<RadrootsSimplexAppConnection>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT connection_id, profile_id, contact_id, state, agent_connection_id, created_at_unix
             FROM connections WHERE state = ?1 ORDER BY connection_id",
        )?;
        collect_rows(statement.query_map(params![state], connection_from_row)?)
    }

    pub fn upsert_queue_endpoint(
        &self,
        queue: &RadrootsSimplexAppQueueEndpoint,
    ) -> Result<(), RadrootsSimplexAppStoreError> {
        self.connection.execute(
            "INSERT INTO queue_endpoints
                (queue_endpoint_id, connection_id, role, server, sender_id, status, created_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(queue_endpoint_id) DO UPDATE SET
                role = excluded.role,
                server = excluded.server,
                sender_id = excluded.sender_id,
                status = excluded.status",
            params![
                queue.queue_endpoint_id,
                queue.connection_id,
                queue.role,
                queue.server,
                queue.sender_id,
                queue.status,
                queue.created_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn list_queues_by_status(
        &self,
        status: &str,
    ) -> Result<Vec<RadrootsSimplexAppQueueEndpoint>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT queue_endpoint_id, connection_id, role, server, sender_id, status, created_at_unix
             FROM queue_endpoints WHERE status = ?1 ORDER BY queue_endpoint_id",
        )?;
        collect_rows(statement.query_map(params![status], queue_endpoint_from_row)?)
    }

    pub fn upsert_conversation(
        &self,
        conversation: &RadrootsSimplexAppConversation,
    ) -> Result<(), RadrootsSimplexAppStoreError> {
        self.connection.execute(
            "INSERT INTO conversations (conversation_id, profile_id, contact_id, created_at_unix)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(conversation_id) DO UPDATE SET contact_id = excluded.contact_id",
            params![
                conversation.conversation_id,
                conversation.profile_id,
                conversation.contact_id,
                conversation.created_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn append_chat_item(
        &self,
        item: &RadrootsSimplexAppChatItem,
    ) -> Result<(), RadrootsSimplexAppStoreError> {
        self.connection.execute(
            "INSERT INTO chat_items
                (chat_item_id, conversation_id, logical_order, direction, body, delivery_status, created_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                item.chat_item_id,
                item.conversation_id,
                item.logical_order,
                item.direction.as_str(),
                item.body,
                item.delivery_status,
                item.created_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn chat_page(
        &self,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<RadrootsSimplexAppChatItem>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT chat_item_id, conversation_id, logical_order, direction, body, delivery_status, created_at_unix
             FROM chat_items
             WHERE conversation_id = ?1
             ORDER BY logical_order DESC, chat_item_id DESC
             LIMIT ?2",
        )?;
        collect_rows(
            statement.query_map(params![conversation_id, limit as i64], chat_item_from_row)?,
        )
    }

    pub fn record_inbound_message(
        &self,
        entry: &RadrootsSimplexAppInboundMessageLogEntry,
    ) -> Result<(), RadrootsSimplexAppStoreError> {
        self.connection.execute(
            "INSERT INTO inbound_message_log
                (inbound_id, connection_id, broker_message_id_hash, inbound_sequence, message_hash, ack_status, received_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.inbound_id,
                entry.connection_id,
                entry.broker_message_id_hash,
                entry.inbound_sequence,
                entry.message_hash,
                entry.ack_status,
                entry.received_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn pending_ack_messages(
        &self,
    ) -> Result<Vec<RadrootsSimplexAppInboundMessageLogEntry>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT inbound_id, connection_id, broker_message_id_hash, inbound_sequence, message_hash, ack_status, received_at_unix
             FROM inbound_message_log
             WHERE ack_status = 'pending'
             ORDER BY received_at_unix, inbound_id",
        )?;
        collect_rows(statement.query_map([], inbound_message_from_row)?)
    }

    pub fn enqueue_outbox_message(
        &self,
        message: &RadrootsSimplexAppOutboxMessage,
    ) -> Result<(), RadrootsSimplexAppStoreError> {
        self.connection.execute(
            "INSERT INTO outbox_messages
                (outbox_id, connection_id, conversation_id, body, status, retry_after_unix, created_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                message.outbox_id,
                message.connection_id,
                message.conversation_id,
                message.body,
                message.status,
                message.retry_after_unix,
                message.created_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn pending_outbox_messages(
        &self,
    ) -> Result<Vec<RadrootsSimplexAppOutboxMessage>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT outbox_id, connection_id, conversation_id, body, status, retry_after_unix, created_at_unix
             FROM outbox_messages
             WHERE status IN ('pending', 'retryable')
             ORDER BY created_at_unix, outbox_id",
        )?;
        collect_rows(statement.query_map([], outbox_message_from_row)?)
    }

    pub fn record_unsupported_protocol_event(
        &self,
        event: &RadrootsSimplexAppUnsupportedProtocolEvent,
    ) -> Result<(), RadrootsSimplexAppStoreError> {
        self.connection.execute(
            "INSERT INTO unsupported_protocol_events
                (event_id, connection_id, event_kind, payload_json, status, received_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.event_id,
                event.connection_id,
                event.event_kind,
                event.payload_json,
                event.status,
                event.received_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn list_unsupported_protocol_events(
        &self,
    ) -> Result<Vec<RadrootsSimplexAppUnsupportedProtocolEvent>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT event_id, connection_id, event_kind, payload_json, status, received_at_unix
             FROM unsupported_protocol_events ORDER BY received_at_unix, event_id",
        )?;
        collect_rows(statement.query_map([], unsupported_event_from_row)?)
    }
}

fn load_or_create_database_key(
    vault: &dyn RadrootsSecretVault,
    key_slot: &str,
    database_exists: bool,
) -> Result<String, RadrootsSimplexAppStoreError> {
    match vault.load_secret(key_slot)? {
        Some(secret) => validate_database_key(secret),
        None if database_exists => Err(RadrootsSimplexAppStoreError::MissingDatabaseKey),
        None => {
            let key = generate_database_key_hex()?;
            vault.store_secret(key_slot, &key)?;
            Ok(key)
        }
    }
}

fn generate_database_key_hex() -> Result<String, RadrootsSimplexAppStoreError> {
    let mut key = [0_u8; DATABASE_KEY_BYTES];
    getrandom(&mut key).map_err(|_| {
        RadrootsSimplexAppStoreError::InvalidDatabaseKey("entropy unavailable".into())
    })?;
    let hex = hex::encode(key);
    key.zeroize();
    Ok(hex)
}

fn validate_database_key(secret: String) -> Result<String, RadrootsSimplexAppStoreError> {
    if secret.len() != DATABASE_KEY_BYTES * 2 {
        return Err(RadrootsSimplexAppStoreError::InvalidDatabaseKey(
            "expected 32-byte hex key".into(),
        ));
    }
    if !secret.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(RadrootsSimplexAppStoreError::InvalidDatabaseKey(
            "key is not hex encoded".into(),
        ));
    }
    Ok(secret)
}

fn open_keyed_connection(
    path: &Path,
    key_hex: &str,
) -> Result<Connection, RadrootsSimplexAppStoreError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))?;
    match connection.query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(())) {
        Ok(()) => Ok(connection),
        Err(_) => Err(RadrootsSimplexAppStoreError::EncryptionKeyRejected),
    }
}

fn verify_encryption(connection: &Connection) -> Result<String, RadrootsSimplexAppStoreError> {
    let cipher = connection
        .query_row("PRAGMA cipher_version", [], |row| row.get::<_, String>(0))
        .optional()?
        .ok_or(RadrootsSimplexAppStoreError::EncryptionUnavailable)?;
    if cipher.trim().is_empty() {
        return Err(RadrootsSimplexAppStoreError::EncryptionUnavailable);
    }
    Ok(cipher)
}

fn configure_connection(connection: &Connection) -> Result<(), RadrootsSimplexAppStoreError> {
    connection.pragma_update(None, "foreign_keys", true)?;
    let foreign_keys: i64 =
        connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    if foreign_keys != 1 {
        return Err(RadrootsSimplexAppStoreError::Schema(
            "foreign keys did not enable".into(),
        ));
    }
    let journal_mode: String =
        connection.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(RadrootsSimplexAppStoreError::Schema(format!(
            "WAL journal mode unavailable: {journal_mode}"
        )));
    }
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

fn migrate(
    connection: &mut Connection,
    key_slot_digest: &str,
    key_source: &str,
) -> Result<(), RadrootsSimplexAppStoreError> {
    let user_version: i64 =
        connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if user_version > CURRENT_SCHEMA_VERSION {
        return Err(RadrootsSimplexAppStoreError::Schema(format!(
            "unsupported future schema version `{user_version}`"
        )));
    }
    if user_version == CURRENT_SCHEMA_VERSION {
        return Ok(());
    }
    if user_version != 0 {
        return Err(RadrootsSimplexAppStoreError::Schema(format!(
            "unsupported schema version `{user_version}`"
        )));
    }

    let transaction = connection.transaction()?;
    apply_schema_v1(&transaction)?;
    transaction.execute(
        "INSERT INTO encryption_metadata
            (id, key_slot_digest, key_source, cipher, created_at_unix)
         VALUES (1, ?1, ?2, 'sqlcipher', ?3)",
        params![key_slot_digest, key_source, now_unix_secs()],
    )?;
    transaction.execute(
        "INSERT INTO simplex_schema_migrations (version, name, applied_at_unix)
         VALUES (1, 'initial-simplex-app-store', ?1)",
        params![now_unix_secs()],
    )?;
    transaction.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
    transaction.commit()?;
    Ok(())
}

fn apply_schema_v1(transaction: &Transaction<'_>) -> Result<(), RadrootsSimplexAppStoreError> {
    transaction.execute_batch(
        "
        CREATE TABLE encryption_metadata (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            key_slot_digest TEXT NOT NULL,
            key_source TEXT NOT NULL,
            cipher TEXT NOT NULL,
            created_at_unix INTEGER NOT NULL
        );

        CREATE TABLE simplex_schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at_unix INTEGER NOT NULL
        );

        CREATE TABLE profiles (
            profile_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            created_at_unix INTEGER NOT NULL
        );

        CREATE TABLE contacts (
            contact_id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            display_name TEXT NOT NULL,
            lifecycle TEXT NOT NULL,
            created_at_unix INTEGER NOT NULL
        );

        CREATE TABLE connections (
            connection_id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            contact_id TEXT REFERENCES contacts(contact_id) ON DELETE SET NULL,
            state TEXT NOT NULL,
            agent_connection_id TEXT,
            created_at_unix INTEGER NOT NULL
        );

        CREATE TABLE queue_endpoints (
            queue_endpoint_id TEXT PRIMARY KEY,
            connection_id TEXT NOT NULL REFERENCES connections(connection_id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            server TEXT NOT NULL,
            sender_id BLOB NOT NULL,
            status TEXT NOT NULL,
            created_at_unix INTEGER NOT NULL
        );

        CREATE TABLE conversations (
            conversation_id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            contact_id TEXT REFERENCES contacts(contact_id) ON DELETE SET NULL,
            created_at_unix INTEGER NOT NULL
        );

        CREATE TABLE chat_items (
            chat_item_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL REFERENCES conversations(conversation_id) ON DELETE CASCADE,
            logical_order INTEGER NOT NULL,
            direction TEXT NOT NULL,
            body TEXT NOT NULL,
            delivery_status TEXT NOT NULL,
            created_at_unix INTEGER NOT NULL
        );

        CREATE TABLE inbound_message_log (
            inbound_id TEXT PRIMARY KEY,
            connection_id TEXT NOT NULL REFERENCES connections(connection_id) ON DELETE CASCADE,
            broker_message_id_hash BLOB NOT NULL,
            inbound_sequence INTEGER,
            message_hash BLOB NOT NULL,
            ack_status TEXT NOT NULL,
            received_at_unix INTEGER NOT NULL,
            UNIQUE(connection_id, broker_message_id_hash)
        );

        CREATE TABLE outbox_messages (
            outbox_id TEXT PRIMARY KEY,
            connection_id TEXT NOT NULL REFERENCES connections(connection_id) ON DELETE CASCADE,
            conversation_id TEXT REFERENCES conversations(conversation_id) ON DELETE SET NULL,
            body TEXT NOT NULL,
            status TEXT NOT NULL,
            retry_after_unix INTEGER,
            created_at_unix INTEGER NOT NULL
        );

        CREATE TABLE unsupported_protocol_events (
            event_id TEXT PRIMARY KEY,
            connection_id TEXT REFERENCES connections(connection_id) ON DELETE SET NULL,
            event_kind TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            status TEXT NOT NULL,
            received_at_unix INTEGER NOT NULL
        );

        CREATE INDEX chat_items_page_idx
            ON chat_items(conversation_id, logical_order DESC, chat_item_id DESC);
        CREATE UNIQUE INDEX inbound_message_log_sequence_hash_idx
            ON inbound_message_log(connection_id, inbound_sequence, message_hash)
            WHERE inbound_sequence IS NOT NULL;
        CREATE INDEX inbound_message_log_pending_ack_idx
            ON inbound_message_log(connection_id, inbound_id)
            WHERE ack_status = 'pending';
        CREATE INDEX outbox_messages_pending_retryable_idx
            ON outbox_messages(connection_id, outbox_id)
            WHERE status IN ('pending', 'retryable');
        CREATE INDEX connections_state_idx ON connections(state);
        CREATE INDEX queue_endpoints_status_idx ON queue_endpoints(status);
        CREATE INDEX contacts_lifecycle_idx ON contacts(lifecycle);
        ",
    )?;
    Ok(())
}

fn verify_metadata(
    connection: &Connection,
    expected_key_slot_digest: &str,
) -> Result<(), RadrootsSimplexAppStoreError> {
    let actual_key_slot_digest: String = connection.query_row(
        "SELECT key_slot_digest FROM encryption_metadata WHERE id = 1",
        [],
        |row| row.get(0),
    )?;
    if actual_key_slot_digest != expected_key_slot_digest {
        return Err(RadrootsSimplexAppStoreError::EncryptionKeyRejected);
    }
    Ok(())
}

fn diagnostics_for(
    connection: &Connection,
    cipher: String,
    key_source: String,
    key_slot_digest: String,
) -> Result<RadrootsSimplexAppDiagnostics, RadrootsSimplexAppStoreError> {
    let schema_version: i64 =
        connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let migration_count: i64 = connection.query_row(
        "SELECT count(*) FROM simplex_schema_migrations",
        [],
        |row| row.get(0),
    )?;
    let foreign_keys: i64 =
        connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    let journal_mode: String =
        connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    Ok(RadrootsSimplexAppDiagnostics {
        encrypted: true,
        cipher,
        schema_version: schema_version as u32,
        migration_count: migration_count as usize,
        foreign_keys_enabled: foreign_keys == 1,
        wal_enabled: journal_mode.eq_ignore_ascii_case("wal"),
        key_source,
        key_slot_digest,
    })
}

fn key_slot_digest(key_slot: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key_slot.as_bytes());
    hex::encode(hasher.finalize())
}

fn derived_key_slot(path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_os_str().as_encoded_bytes());
    format!(
        "radroots_simplex_app_store_{}",
        hex::encode(hasher.finalize())
    )
}

fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, RadrootsSimplexAppStoreError> {
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn profile_from_row(row: &Row<'_>) -> rusqlite::Result<RadrootsSimplexAppProfile> {
    Ok(RadrootsSimplexAppProfile {
        profile_id: row.get(0)?,
        display_name: row.get(1)?,
        created_at_unix: row.get(2)?,
    })
}

fn contact_from_row(row: &Row<'_>) -> rusqlite::Result<RadrootsSimplexAppContact> {
    Ok(RadrootsSimplexAppContact {
        contact_id: row.get(0)?,
        profile_id: row.get(1)?,
        display_name: row.get(2)?,
        lifecycle: row.get(3)?,
        created_at_unix: row.get(4)?,
    })
}

fn connection_from_row(row: &Row<'_>) -> rusqlite::Result<RadrootsSimplexAppConnection> {
    Ok(RadrootsSimplexAppConnection {
        connection_id: row.get(0)?,
        profile_id: row.get(1)?,
        contact_id: row.get(2)?,
        state: row.get(3)?,
        agent_connection_id: row.get(4)?,
        created_at_unix: row.get(5)?,
    })
}

fn queue_endpoint_from_row(row: &Row<'_>) -> rusqlite::Result<RadrootsSimplexAppQueueEndpoint> {
    Ok(RadrootsSimplexAppQueueEndpoint {
        queue_endpoint_id: row.get(0)?,
        connection_id: row.get(1)?,
        role: row.get(2)?,
        server: row.get(3)?,
        sender_id: row.get(4)?,
        status: row.get(5)?,
        created_at_unix: row.get(6)?,
    })
}

fn chat_item_from_row(row: &Row<'_>) -> rusqlite::Result<RadrootsSimplexAppChatItem> {
    let direction: String = row.get(3)?;
    Ok(RadrootsSimplexAppChatItem {
        chat_item_id: row.get(0)?,
        conversation_id: row.get(1)?,
        logical_order: row.get(2)?,
        direction: RadrootsSimplexAppChatDirection::parse(&direction)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))?,
        body: row.get(4)?,
        delivery_status: row.get(5)?,
        created_at_unix: row.get(6)?,
    })
}

fn inbound_message_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<RadrootsSimplexAppInboundMessageLogEntry> {
    Ok(RadrootsSimplexAppInboundMessageLogEntry {
        inbound_id: row.get(0)?,
        connection_id: row.get(1)?,
        broker_message_id_hash: row.get(2)?,
        inbound_sequence: row.get(3)?,
        message_hash: row.get(4)?,
        ack_status: row.get(5)?,
        received_at_unix: row.get(6)?,
    })
}

fn outbox_message_from_row(row: &Row<'_>) -> rusqlite::Result<RadrootsSimplexAppOutboxMessage> {
    Ok(RadrootsSimplexAppOutboxMessage {
        outbox_id: row.get(0)?,
        connection_id: row.get(1)?,
        conversation_id: row.get(2)?,
        body: row.get(3)?,
        status: row.get(4)?,
        retry_after_unix: row.get(5)?,
        created_at_unix: row.get(6)?,
    })
}

fn unsupported_event_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<RadrootsSimplexAppUnsupportedProtocolEvent> {
    Ok(RadrootsSimplexAppUnsupportedProtocolEvent {
        event_id: row.get(0)?,
        connection_id: row.get(1)?,
        event_kind: row.get(2)?,
        payload_json: row.get(3)?,
        status: row.get(4)?,
        received_at_unix: row.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_secret_vault::{RadrootsSecretVault, RadrootsSecretVaultMemory};
    use std::sync::Arc;

    fn memory_store(
        path: &Path,
        vault: Arc<RadrootsSecretVaultMemory>,
    ) -> Result<RadrootsSimplexAppStore, RadrootsSimplexAppStoreError> {
        RadrootsSimplexAppStore::open_with_vault(path, vault, "test-simplex-app-store", "memory")
    }

    fn profile() -> RadrootsSimplexAppProfile {
        RadrootsSimplexAppProfile {
            profile_id: "profile-1".into(),
            display_name: "Local Profile".into(),
            created_at_unix: 1,
        }
    }

    fn contact() -> RadrootsSimplexAppContact {
        RadrootsSimplexAppContact {
            contact_id: "contact-1".into(),
            profile_id: "profile-1".into(),
            display_name: "Phone Contact".into(),
            lifecycle: "active".into(),
            created_at_unix: 2,
        }
    }

    fn connection() -> RadrootsSimplexAppConnection {
        RadrootsSimplexAppConnection {
            connection_id: "connection-1".into(),
            profile_id: "profile-1".into(),
            contact_id: Some("contact-1".into()),
            state: "connected".into(),
            agent_connection_id: Some("agent-connection-1".into()),
            created_at_unix: 3,
        }
    }

    fn queue() -> RadrootsSimplexAppQueueEndpoint {
        RadrootsSimplexAppQueueEndpoint {
            queue_endpoint_id: "queue-1".into(),
            connection_id: "connection-1".into(),
            role: "receive".into(),
            server: "smp.example".into(),
            sender_id: b"sender-id".to_vec(),
            status: "active".into(),
            created_at_unix: 4,
        }
    }

    fn conversation() -> RadrootsSimplexAppConversation {
        RadrootsSimplexAppConversation {
            conversation_id: "conversation-1".into(),
            profile_id: "profile-1".into(),
            contact_id: Some("contact-1".into()),
            created_at_unix: 5,
        }
    }

    fn seed_store(store: &RadrootsSimplexAppStore) {
        store.upsert_profile(&profile()).expect("profile");
        store.upsert_contact(&contact()).expect("contact");
        store.upsert_connection(&connection()).expect("connection");
        store.upsert_queue_endpoint(&queue()).expect("queue");
        store
            .upsert_conversation(&conversation())
            .expect("conversation");
    }

    #[test]
    fn empty_store_initializes_encrypted_schema() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");

        let diagnostics = store.diagnostics();
        assert!(diagnostics.encrypted);
        assert!(!diagnostics.cipher.is_empty());
        assert_eq!(diagnostics.schema_version, 1);
        assert_eq!(diagnostics.migration_count, 1);
        assert!(diagnostics.foreign_keys_enabled);
        assert!(diagnostics.wal_enabled);
        assert_eq!(diagnostics.key_source, "memory");
        assert_eq!(diagnostics.key_slot_digest.len(), 64);
    }

    #[test]
    fn typed_repositories_round_trip_and_indexes_support_queries() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        assert_eq!(
            store.get_profile("profile-1").expect("profile"),
            Some(profile())
        );
        assert_eq!(store.list_profiles().expect("profiles"), vec![profile()]);
        assert_eq!(store.list_contacts().expect("contacts"), vec![contact()]);
        assert_eq!(
            store
                .list_connections_by_state("connected")
                .expect("connections"),
            vec![connection()]
        );
        assert_eq!(
            store.list_queues_by_status("active").expect("queues"),
            vec![queue()]
        );

        store
            .append_chat_item(&RadrootsSimplexAppChatItem {
                chat_item_id: "chat-1".into(),
                conversation_id: "conversation-1".into(),
                logical_order: 1,
                direction: RadrootsSimplexAppChatDirection::Outbound,
                body: "hello encrypted iPhone".into(),
                delivery_status: "sent".into(),
                created_at_unix: 6,
            })
            .expect("chat 1");
        store
            .append_chat_item(&RadrootsSimplexAppChatItem {
                chat_item_id: "chat-2".into(),
                conversation_id: "conversation-1".into(),
                logical_order: 2,
                direction: RadrootsSimplexAppChatDirection::Inbound,
                body: "hello encrypted runtime".into(),
                delivery_status: "received".into(),
                created_at_unix: 7,
            })
            .expect("chat 2");

        let page = store.chat_page("conversation-1", 10).expect("page");
        assert_eq!(page[0].chat_item_id, "chat-2");
        assert_eq!(page[1].chat_item_id, "chat-1");

        store
            .record_inbound_message(&RadrootsSimplexAppInboundMessageLogEntry {
                inbound_id: "inbound-1".into(),
                connection_id: "connection-1".into(),
                broker_message_id_hash: b"broker-hash".to_vec(),
                inbound_sequence: Some(1),
                message_hash: b"message-hash".to_vec(),
                ack_status: "pending".into(),
                received_at_unix: 8,
            })
            .expect("inbound");
        assert_eq!(
            store.pending_ack_messages().expect("pending ack")[0].inbound_id,
            "inbound-1"
        );

        store
            .enqueue_outbox_message(&RadrootsSimplexAppOutboxMessage {
                outbox_id: "outbox-1".into(),
                connection_id: "connection-1".into(),
                conversation_id: Some("conversation-1".into()),
                body: "queued plaintext before encryption".into(),
                status: "retryable".into(),
                retry_after_unix: Some(9),
                created_at_unix: 9,
            })
            .expect("outbox");
        assert_eq!(
            store.pending_outbox_messages().expect("outbox")[0].outbox_id,
            "outbox-1"
        );

        store
            .record_unsupported_protocol_event(&RadrootsSimplexAppUnsupportedProtocolEvent {
                event_id: "event-1".into(),
                connection_id: Some("connection-1".into()),
                event_kind: "future_event".into(),
                payload_json: "{\"field\":\"value\"}".into(),
                status: "stored".into(),
                received_at_unix: 10,
            })
            .expect("unsupported");
        assert_eq!(
            store
                .list_unsupported_protocol_events()
                .expect("unsupported")[0]
                .event_id,
            "event-1"
        );
    }

    #[test]
    fn database_bytes_do_not_expose_message_or_profile_text() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);
        store
            .append_chat_item(&RadrootsSimplexAppChatItem {
                chat_item_id: "chat-1".into(),
                conversation_id: "conversation-1".into(),
                logical_order: 1,
                direction: RadrootsSimplexAppChatDirection::Outbound,
                body: "plaintext should not appear in sqlite bytes".into(),
                delivery_status: "sent".into(),
                created_at_unix: 6,
            })
            .expect("chat");
        drop(store);

        let raw = fs::read(&path).expect("read database");
        let raw_text = String::from_utf8_lossy(&raw);
        assert!(!raw_text.contains("Local Profile"));
        assert!(!raw_text.contains("plaintext should not appear"));
    }

    #[test]
    fn existing_store_reopens_with_same_vault_key() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault.clone()).expect("store");
        seed_store(&store);
        drop(store);

        let reopened = memory_store(&path, vault).expect("reopen");
        assert_eq!(
            reopened.get_profile("profile-1").expect("profile"),
            Some(profile())
        );
    }

    #[test]
    fn missing_key_for_existing_store_fails_closed() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);
        drop(store);

        let missing_vault = Arc::new(RadrootsSecretVaultMemory::new());
        let error = memory_store(&path, missing_vault)
            .err()
            .expect("missing key error");
        assert_eq!(error, RadrootsSimplexAppStoreError::MissingDatabaseKey);
    }

    #[test]
    fn corrupt_key_fails_before_database_open() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        vault
            .store_secret("test-simplex-app-store", "not-a-valid-key")
            .expect("secret");

        let error = memory_store(&path, vault).err().expect("invalid key error");
        assert!(matches!(
            error,
            RadrootsSimplexAppStoreError::InvalidDatabaseKey(_)
        ));
    }

    #[test]
    fn wrong_key_for_existing_store_fails_closed() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);
        drop(store);

        let wrong_vault = Arc::new(RadrootsSecretVaultMemory::new());
        wrong_vault
            .store_secret(
                "test-simplex-app-store",
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("wrong");
        let error = memory_store(&path, wrong_vault)
            .err()
            .expect("wrong key error");
        assert_eq!(error, RadrootsSimplexAppStoreError::EncryptionKeyRejected);
    }

    #[test]
    fn foreign_keys_and_unique_dedupe_fail_closed() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        let invalid_contact = RadrootsSimplexAppContact {
            profile_id: "missing-profile".into(),
            ..contact()
        };
        assert!(store.upsert_contact(&invalid_contact).is_err());

        seed_store(&store);
        let inbound = RadrootsSimplexAppInboundMessageLogEntry {
            inbound_id: "inbound-1".into(),
            connection_id: "connection-1".into(),
            broker_message_id_hash: b"dedupe".to_vec(),
            inbound_sequence: Some(1),
            message_hash: b"hash".to_vec(),
            ack_status: "pending".into(),
            received_at_unix: 8,
        };
        store.record_inbound_message(&inbound).expect("inbound");
        let duplicate = RadrootsSimplexAppInboundMessageLogEntry {
            inbound_id: "inbound-2".into(),
            ..inbound
        };
        assert!(store.record_inbound_message(&duplicate).is_err());
    }
}

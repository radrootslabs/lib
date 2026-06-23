use crate::error::RadrootsSimplexAppStoreError;
use crate::model::{
    RadrootsSimplexAppChatDirection, RadrootsSimplexAppChatItem, RadrootsSimplexAppConnection,
    RadrootsSimplexAppContact, RadrootsSimplexAppConversation, RadrootsSimplexAppDiagnostics,
    RadrootsSimplexAppInboundChildEvent, RadrootsSimplexAppInboundCommit,
    RadrootsSimplexAppInboundMessageLogEntry, RadrootsSimplexAppInboundTextRequest,
    RadrootsSimplexAppInboundUnsupportedEventRequest, RadrootsSimplexAppOutboundTextDraft,
    RadrootsSimplexAppOutboundTextRequest, RadrootsSimplexAppOutboxMessage,
    RadrootsSimplexAppProfile, RadrootsSimplexAppQueueEndpoint,
    RadrootsSimplexAppUnsupportedProtocolEvent,
};
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
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

const CURRENT_SCHEMA_VERSION: i64 = 4;
const DEFAULT_KEYCHAIN_SERVICE: &str = "org.radroots.simplex.app-store";
const DATABASE_KEY_BYTES: usize = 32;
const CHAT_MSG_ID_BYTES: usize = 12;

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
                (chat_item_id, conversation_id, logical_order, direction, chat_msg_id, body, delivery_status, created_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                item.chat_item_id,
                item.conversation_id,
                item.logical_order,
                item.direction.as_str(),
                item.chat_msg_id,
                item.body,
                item.delivery_status,
                item.created_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn create_outbound_text(
        &self,
        request: &RadrootsSimplexAppOutboundTextRequest,
    ) -> Result<RadrootsSimplexAppOutboundTextDraft, RadrootsSimplexAppStoreError> {
        let chat_msg_id = generate_chat_msg_id()?;
        self.create_outbound_text_with_msg_id(request, &chat_msg_id)
    }

    #[cfg(test)]
    fn create_outbound_text_with_test_msg_id(
        &self,
        request: &RadrootsSimplexAppOutboundTextRequest,
        chat_msg_id: &str,
    ) -> Result<RadrootsSimplexAppOutboundTextDraft, RadrootsSimplexAppStoreError> {
        self.create_outbound_text_with_msg_id(request, chat_msg_id)
    }

    fn create_outbound_text_with_msg_id(
        &self,
        request: &RadrootsSimplexAppOutboundTextRequest,
        chat_msg_id: &str,
    ) -> Result<RadrootsSimplexAppOutboundTextDraft, RadrootsSimplexAppStoreError> {
        validate_outbound_text_request(request)?;
        validate_chat_msg_id(chat_msg_id)?;
        let transaction = self.connection.unchecked_transaction()?;
        if let Some(existing) =
            outbound_text_by_msg_id(&transaction, &request.connection_id, chat_msg_id)?
        {
            transaction.commit()?;
            return Ok(existing);
        }
        let logical_order = next_logical_order(&transaction, &request.conversation_id)?;
        let chat_item_id = derive_outbound_local_id("chat", &request.connection_id, chat_msg_id);
        let outbox_id = derive_outbound_local_id("outbox", &request.connection_id, chat_msg_id);
        let chat_item = RadrootsSimplexAppChatItem {
            chat_item_id: chat_item_id.clone(),
            conversation_id: request.conversation_id.clone(),
            logical_order,
            direction: RadrootsSimplexAppChatDirection::Outbound,
            chat_msg_id: Some(chat_msg_id.to_owned()),
            body: request.body.clone(),
            delivery_status: "pending".to_owned(),
            created_at_unix: request.created_at_unix,
        };
        transaction.execute(
            "INSERT INTO chat_items
                (chat_item_id, conversation_id, logical_order, direction, chat_msg_id, body, delivery_status, created_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                chat_item.chat_item_id,
                chat_item.conversation_id,
                chat_item.logical_order,
                chat_item.direction.as_str(),
                chat_item.chat_msg_id,
                chat_item.body,
                chat_item.delivery_status,
                chat_item.created_at_unix
            ],
        )?;
        let outbox_message = RadrootsSimplexAppOutboxMessage {
            outbox_id,
            chat_item_id,
            connection_id: request.connection_id.clone(),
            conversation_id: Some(request.conversation_id.clone()),
            chat_msg_id: chat_msg_id.to_owned(),
            body: request.body.clone(),
            status: "pending".to_owned(),
            runtime_message_id: None,
            retry_after_unix: None,
            created_at_unix: request.created_at_unix,
        };
        transaction.execute(
            "INSERT INTO outbox_messages
                (outbox_id, chat_item_id, connection_id, conversation_id, chat_msg_id, body, status, runtime_message_id, retry_after_unix, created_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                outbox_message.outbox_id,
                outbox_message.chat_item_id,
                outbox_message.connection_id,
                outbox_message.conversation_id,
                outbox_message.chat_msg_id,
                outbox_message.body,
                outbox_message.status,
                outbox_message.runtime_message_id,
                outbox_message.retry_after_unix,
                outbox_message.created_at_unix
            ],
        )?;
        transaction.commit()?;
        Ok(RadrootsSimplexAppOutboundTextDraft {
            chat_item,
            outbox_message,
        })
    }

    pub fn chat_page(
        &self,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<RadrootsSimplexAppChatItem>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT chat_item_id, conversation_id, logical_order, direction, chat_msg_id, body, delivery_status, created_at_unix
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
                (inbound_id, connection_id, broker_message_id_hash, inbound_sequence, message_hash, runtime_ack_handle, ack_status, app_record_kind, app_record_id, received_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.inbound_id,
                entry.connection_id,
                entry.broker_message_id_hash,
                entry.inbound_sequence,
                entry.message_hash,
                entry.runtime_ack_handle,
                entry.ack_status,
                entry.app_record_kind,
                entry.app_record_id,
                entry.received_at_unix
            ],
        )?;
        Ok(())
    }

    pub fn commit_inbound_text(
        &self,
        request: &RadrootsSimplexAppInboundTextRequest,
    ) -> Result<RadrootsSimplexAppInboundCommit, RadrootsSimplexAppStoreError> {
        validate_inbound_text_request(request)?;
        let transaction = self.connection.unchecked_transaction()?;
        let inbound = ensure_inbound_frame(
            &transaction,
            &request.connection_id,
            &request.broker_message_id_hash,
            request.inbound_sequence,
            &request.message_hash,
            &request.runtime_ack_handle,
            request.received_at_unix,
        )?;
        if let Some(existing) =
            inbound_child_commit_by_ordinal(&transaction, &inbound, request.child_ordinal)?
        {
            transaction.commit()?;
            return Ok(existing);
        }
        let chat_item_id = derive_inbound_child_local_id(
            "chat",
            &inbound.inbound_id,
            request.child_ordinal,
            request.chat_msg_id.as_deref().unwrap_or(""),
        );
        let logical_order = next_logical_order(&transaction, &request.conversation_id)?;
        let chat_item = RadrootsSimplexAppChatItem {
            chat_item_id: chat_item_id.clone(),
            conversation_id: request.conversation_id.clone(),
            logical_order,
            direction: RadrootsSimplexAppChatDirection::Inbound,
            chat_msg_id: request.chat_msg_id.clone(),
            body: request.body.clone(),
            delivery_status: "received".to_owned(),
            created_at_unix: request.received_at_unix,
        };
        transaction.execute(
            "INSERT INTO chat_items
                (chat_item_id, conversation_id, logical_order, direction, chat_msg_id, body, delivery_status, created_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                chat_item.chat_item_id,
                chat_item.conversation_id,
                chat_item.logical_order,
                chat_item.direction.as_str(),
                chat_item.chat_msg_id,
                chat_item.body,
                chat_item.delivery_status,
                chat_item.created_at_unix
            ],
        )?;
        let child_event = RadrootsSimplexAppInboundChildEvent {
            child_event_id: derive_inbound_child_local_id(
                "child",
                &inbound.inbound_id,
                request.child_ordinal,
                request.chat_msg_id.as_deref().unwrap_or(""),
            ),
            inbound_id: inbound.inbound_id.clone(),
            child_ordinal: request.child_ordinal,
            app_record_kind: "chat_item".to_owned(),
            app_record_id: chat_item_id,
            event_kind: "x.msg.new".to_owned(),
            chat_msg_id: request.chat_msg_id.clone(),
            received_at_unix: request.received_at_unix,
        };
        insert_inbound_child_event(&transaction, &child_event)?;
        transaction.commit()?;
        Ok(RadrootsSimplexAppInboundCommit {
            inbound,
            child_event,
            chat_item: Some(chat_item),
            unsupported_event: None,
            duplicate: false,
        })
    }

    pub fn commit_inbound_unsupported_event(
        &self,
        request: &RadrootsSimplexAppInboundUnsupportedEventRequest,
    ) -> Result<RadrootsSimplexAppInboundCommit, RadrootsSimplexAppStoreError> {
        validate_inbound_unsupported_request(request)?;
        let transaction = self.connection.unchecked_transaction()?;
        let inbound = ensure_inbound_frame(
            &transaction,
            &request.connection_id,
            &request.broker_message_id_hash,
            request.inbound_sequence,
            &request.message_hash,
            &request.runtime_ack_handle,
            request.received_at_unix,
        )?;
        if let Some(existing) =
            inbound_child_commit_by_ordinal(&transaction, &inbound, request.child_ordinal)?
        {
            transaction.commit()?;
            return Ok(existing);
        }
        let event_id = derive_inbound_child_local_id(
            "unsupported",
            &inbound.inbound_id,
            request.child_ordinal,
            &request.event_kind,
        );
        let unsupported_event = RadrootsSimplexAppUnsupportedProtocolEvent {
            event_id: event_id.clone(),
            connection_id: Some(request.connection_id.clone()),
            event_kind: request.event_kind.clone(),
            payload_json: request.payload_json.clone(),
            status: "stored".to_owned(),
            received_at_unix: request.received_at_unix,
        };
        transaction.execute(
            "INSERT INTO unsupported_protocol_events
                (event_id, connection_id, event_kind, payload_json, status, received_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                unsupported_event.event_id,
                unsupported_event.connection_id,
                unsupported_event.event_kind,
                unsupported_event.payload_json,
                unsupported_event.status,
                unsupported_event.received_at_unix
            ],
        )?;
        let child_event = RadrootsSimplexAppInboundChildEvent {
            child_event_id: derive_inbound_child_local_id(
                "child",
                &inbound.inbound_id,
                request.child_ordinal,
                &request.event_kind,
            ),
            inbound_id: inbound.inbound_id.clone(),
            child_ordinal: request.child_ordinal,
            app_record_kind: "unsupported_event".to_owned(),
            app_record_id: event_id,
            event_kind: request.event_kind.clone(),
            chat_msg_id: None,
            received_at_unix: request.received_at_unix,
        };
        insert_inbound_child_event(&transaction, &child_event)?;
        transaction.commit()?;
        Ok(RadrootsSimplexAppInboundCommit {
            inbound,
            child_event,
            chat_item: None,
            unsupported_event: Some(unsupported_event),
            duplicate: false,
        })
    }

    pub fn pending_ack_messages(
        &self,
    ) -> Result<Vec<RadrootsSimplexAppInboundMessageLogEntry>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT inbound_id, connection_id, broker_message_id_hash, inbound_sequence, message_hash, runtime_ack_handle, ack_status, app_record_kind, app_record_id, received_at_unix
             FROM inbound_message_log
             WHERE ack_status = 'pending_ack'
             ORDER BY received_at_unix, inbound_id",
        )?;
        collect_rows(statement.query_map([], inbound_message_from_row)?)
    }

    pub fn mark_inbound_ack_delivered(
        &self,
        connection_id: &str,
        inbound_sequence: i64,
        message_hash: &[u8],
    ) -> Result<Option<RadrootsSimplexAppInboundMessageLogEntry>, RadrootsSimplexAppStoreError>
    {
        if connection_id.is_empty() {
            return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
                "connection id must not be empty".into(),
            ));
        }
        if inbound_sequence < 0 {
            return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
                "inbound sequence must not be negative".into(),
            ));
        }
        if message_hash.is_empty() {
            return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
                "message hash must not be empty".into(),
            ));
        }
        let Some(inbound) = self
            .connection
            .query_row(
                "SELECT inbound_id, connection_id, broker_message_id_hash, inbound_sequence, message_hash, runtime_ack_handle, ack_status, app_record_kind, app_record_id, received_at_unix
                 FROM inbound_message_log
                 WHERE connection_id = ?1 AND inbound_sequence = ?2 AND message_hash = ?3
                 LIMIT 1",
                params![connection_id, inbound_sequence, message_hash],
                inbound_message_from_row,
            )
            .optional()? else {
            return Ok(None);
        };
        self.mark_inbound_ack_delivered_by_handle(&inbound.runtime_ack_handle)
    }

    pub fn mark_inbound_ack_delivered_by_handle(
        &self,
        runtime_ack_handle: &str,
    ) -> Result<Option<RadrootsSimplexAppInboundMessageLogEntry>, RadrootsSimplexAppStoreError>
    {
        if runtime_ack_handle.is_empty() {
            return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
                "runtime ack handle must not be empty".into(),
            ));
        }
        self.connection.execute(
            "UPDATE inbound_message_log
             SET ack_status = 'acked'
             WHERE runtime_ack_handle = ?1 AND ack_status = 'pending_ack'",
            params![runtime_ack_handle],
        )?;
        self.connection
            .query_row(
                "SELECT inbound_id, connection_id, broker_message_id_hash, inbound_sequence, message_hash, runtime_ack_handle, ack_status, app_record_kind, app_record_id, received_at_unix
                 FROM inbound_message_log
                 WHERE runtime_ack_handle = ?1
                 LIMIT 1",
                params![runtime_ack_handle],
                inbound_message_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn enqueue_outbox_message(
        &self,
        message: &RadrootsSimplexAppOutboxMessage,
    ) -> Result<(), RadrootsSimplexAppStoreError> {
        self.connection.execute(
            "INSERT INTO outbox_messages
                (outbox_id, chat_item_id, connection_id, conversation_id, chat_msg_id, body, status, runtime_message_id, retry_after_unix, created_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                message.outbox_id,
                message.chat_item_id,
                message.connection_id,
                message.conversation_id,
                message.chat_msg_id,
                message.body,
                message.status,
                message.runtime_message_id,
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
            "SELECT outbox_id, chat_item_id, connection_id, conversation_id, chat_msg_id, body, status, runtime_message_id, retry_after_unix, created_at_unix
             FROM outbox_messages
             WHERE status IN ('pending', 'retryable') AND runtime_message_id IS NULL
             ORDER BY created_at_unix, outbox_id",
        )?;
        collect_rows(statement.query_map([], outbox_message_from_row)?)
    }

    pub fn list_outbox_messages(
        &self,
    ) -> Result<Vec<RadrootsSimplexAppOutboxMessage>, RadrootsSimplexAppStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT outbox_id, chat_item_id, connection_id, conversation_id, chat_msg_id, body, status, runtime_message_id, retry_after_unix, created_at_unix
             FROM outbox_messages
             ORDER BY created_at_unix, outbox_id",
        )?;
        collect_rows(statement.query_map([], outbox_message_from_row)?)
    }

    pub fn mark_outbox_message_queued(
        &self,
        outbox_id: &str,
        runtime_message_id: u64,
    ) -> Result<Option<RadrootsSimplexAppOutboundTextDraft>, RadrootsSimplexAppStoreError> {
        if outbox_id.is_empty() {
            return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
                "outbox id must not be empty".into(),
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let Some(current) = outbound_text_by_outbox_id(&transaction, outbox_id)? else {
            transaction.commit()?;
            return Ok(None);
        };
        match current.outbox_message.status.as_str() {
            "pending" | "retryable" => {}
            other => {
                return Err(RadrootsSimplexAppStoreError::MessageLifecycle(format!(
                    "cannot queue outbound message `{outbox_id}` from `{other}`"
                )));
            }
        }
        let runtime_message_id = i64::try_from(runtime_message_id).map_err(|_| {
            RadrootsSimplexAppStoreError::MessageLifecycle(format!(
                "runtime message id `{runtime_message_id}` exceeds app-store range"
            ))
        })?;
        transaction.execute(
            "UPDATE outbox_messages
             SET runtime_message_id = ?2
             WHERE outbox_id = ?1",
            params![outbox_id, runtime_message_id],
        )?;
        let updated = outbound_text_by_outbox_id(&transaction, outbox_id)?;
        transaction.commit()?;
        Ok(updated)
    }

    pub fn mark_outbox_message_sent(
        &self,
        outbox_id: &str,
    ) -> Result<Option<RadrootsSimplexAppOutboundTextDraft>, RadrootsSimplexAppStoreError> {
        self.mark_outbox_message_delivery_status(outbox_id, "sent", false)
    }

    pub fn mark_outbox_message_acknowledged(
        &self,
        outbox_id: &str,
    ) -> Result<Option<RadrootsSimplexAppOutboundTextDraft>, RadrootsSimplexAppStoreError> {
        self.mark_outbox_message_delivery_status(outbox_id, "acknowledged", true)
    }

    fn mark_outbox_message_delivery_status(
        &self,
        outbox_id: &str,
        status: &str,
        terminal: bool,
    ) -> Result<Option<RadrootsSimplexAppOutboundTextDraft>, RadrootsSimplexAppStoreError> {
        if outbox_id.is_empty() {
            return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
                "outbox id must not be empty".into(),
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let Some(current) = outbound_text_by_outbox_id(&transaction, outbox_id)? else {
            transaction.commit()?;
            return Ok(None);
        };
        match (current.outbox_message.status.as_str(), status) {
            ("pending" | "retryable" | "sent", "sent")
            | ("sent" | "acknowledged", "acknowledged")
            | ("acknowledged", "sent") => {}
            (current_status, next_status) => {
                return Err(RadrootsSimplexAppStoreError::MessageLifecycle(format!(
                    "cannot transition outbound message `{outbox_id}` from `{current_status}` to `{next_status}`"
                )));
            }
        }
        if terminal {
            transaction.execute(
                "UPDATE outbox_messages SET status = ?2 WHERE outbox_id = ?1",
                params![outbox_id, status],
            )?;
            transaction.execute(
                "UPDATE chat_items
                 SET delivery_status = ?2
                 WHERE chat_item_id = (
                    SELECT chat_item_id FROM outbox_messages WHERE outbox_id = ?1
                 )",
                params![outbox_id, status],
            )?;
        } else {
            transaction.execute(
                "UPDATE outbox_messages
                 SET status = CASE WHEN status = 'acknowledged' THEN status ELSE ?2 END
                 WHERE outbox_id = ?1",
                params![outbox_id, status],
            )?;
            transaction.execute(
                "UPDATE chat_items
                 SET delivery_status = CASE
                    WHEN delivery_status = 'acknowledged' THEN delivery_status
                    ELSE ?2
                 END
                 WHERE chat_item_id = (
                    SELECT chat_item_id FROM outbox_messages WHERE outbox_id = ?1
                 )",
                params![outbox_id, status],
            )?;
        }
        let updated = outbound_text_by_outbox_id(&transaction, outbox_id)?;
        transaction.commit()?;
        Ok(updated)
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

    pub fn reset_disposable_runtime_state(&self) -> Result<(), RadrootsSimplexAppStoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute("DELETE FROM unsupported_protocol_events", [])?;
        transaction.execute("DELETE FROM inbound_child_events", [])?;
        transaction.execute("DELETE FROM inbound_message_log", [])?;
        transaction.execute("DELETE FROM outbox_messages", [])?;
        transaction.execute("DELETE FROM chat_items", [])?;
        transaction.execute("DELETE FROM queue_endpoints", [])?;
        transaction.execute("DELETE FROM conversations", [])?;
        transaction.execute("DELETE FROM connections", [])?;
        transaction.execute("DELETE FROM contacts", [])?;
        transaction.commit()?;
        Ok(())
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
    match user_version {
        0 => {
            let transaction = connection.transaction()?;
            apply_schema_v4(&transaction)?;
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
            transaction.execute(
                "INSERT INTO simplex_schema_migrations (version, name, applied_at_unix)
                 VALUES (2, 'message-lifecycle-outbound', ?1)",
                params![now_unix_secs()],
            )?;
            transaction.execute(
                "INSERT INTO simplex_schema_migrations (version, name, applied_at_unix)
                 VALUES (3, 'message-lifecycle-inbound', ?1)",
                params![now_unix_secs()],
            )?;
            transaction.execute(
                "INSERT INTO simplex_schema_migrations (version, name, applied_at_unix)
                 VALUES (4, 'message-lifecycle-frame-children', ?1)",
                params![now_unix_secs()],
            )?;
            transaction.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        1 => {
            let transaction = connection.transaction()?;
            apply_migration_v2(&transaction)?;
            apply_migration_v3(&transaction)?;
            apply_migration_v4(&transaction)?;
            transaction.execute(
                "INSERT INTO simplex_schema_migrations (version, name, applied_at_unix)
                 VALUES (2, 'message-lifecycle-outbound', ?1)",
                params![now_unix_secs()],
            )?;
            transaction.execute(
                "INSERT INTO simplex_schema_migrations (version, name, applied_at_unix)
                 VALUES (3, 'message-lifecycle-inbound', ?1)",
                params![now_unix_secs()],
            )?;
            transaction.execute(
                "INSERT INTO simplex_schema_migrations (version, name, applied_at_unix)
                 VALUES (4, 'message-lifecycle-frame-children', ?1)",
                params![now_unix_secs()],
            )?;
            transaction.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        2 => {
            let transaction = connection.transaction()?;
            apply_migration_v3(&transaction)?;
            apply_migration_v4(&transaction)?;
            transaction.execute(
                "INSERT INTO simplex_schema_migrations (version, name, applied_at_unix)
                 VALUES (3, 'message-lifecycle-inbound', ?1)",
                params![now_unix_secs()],
            )?;
            transaction.execute(
                "INSERT INTO simplex_schema_migrations (version, name, applied_at_unix)
                 VALUES (4, 'message-lifecycle-frame-children', ?1)",
                params![now_unix_secs()],
            )?;
            transaction.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        3 => {
            let transaction = connection.transaction()?;
            apply_migration_v4(&transaction)?;
            transaction.execute(
                "INSERT INTO simplex_schema_migrations (version, name, applied_at_unix)
                 VALUES (4, 'message-lifecycle-frame-children', ?1)",
                params![now_unix_secs()],
            )?;
            transaction.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        CURRENT_SCHEMA_VERSION => {}
        _ => {
            return Err(RadrootsSimplexAppStoreError::Schema(format!(
                "unsupported schema version `{user_version}`"
            )));
        }
    }
    Ok(())
}

fn apply_schema_v4(transaction: &Transaction<'_>) -> Result<(), RadrootsSimplexAppStoreError> {
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
            chat_msg_id TEXT,
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
            runtime_ack_handle TEXT NOT NULL,
            ack_status TEXT NOT NULL,
            app_record_kind TEXT NOT NULL,
            app_record_id TEXT NOT NULL,
            received_at_unix INTEGER NOT NULL,
            UNIQUE(connection_id, broker_message_id_hash)
        );

        CREATE TABLE inbound_child_events (
            child_event_id TEXT PRIMARY KEY,
            inbound_id TEXT NOT NULL REFERENCES inbound_message_log(inbound_id) ON DELETE CASCADE,
            child_ordinal INTEGER NOT NULL,
            app_record_kind TEXT NOT NULL,
            app_record_id TEXT NOT NULL,
            event_kind TEXT NOT NULL,
            chat_msg_id TEXT,
            received_at_unix INTEGER NOT NULL,
            UNIQUE(inbound_id, child_ordinal)
        );

        CREATE TABLE outbox_messages (
            outbox_id TEXT PRIMARY KEY,
            chat_item_id TEXT NOT NULL REFERENCES chat_items(chat_item_id) ON DELETE CASCADE,
            connection_id TEXT NOT NULL REFERENCES connections(connection_id) ON DELETE CASCADE,
            conversation_id TEXT REFERENCES conversations(conversation_id) ON DELETE SET NULL,
            chat_msg_id TEXT NOT NULL,
            body TEXT NOT NULL,
            status TEXT NOT NULL,
            runtime_message_id INTEGER,
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
        CREATE UNIQUE INDEX chat_items_conversation_msg_id_idx
            ON chat_items(conversation_id, chat_msg_id)
            WHERE chat_msg_id IS NOT NULL;
        CREATE UNIQUE INDEX inbound_message_log_sequence_hash_idx
            ON inbound_message_log(connection_id, inbound_sequence, message_hash)
            WHERE inbound_sequence IS NOT NULL;
        CREATE INDEX inbound_message_log_pending_ack_idx
            ON inbound_message_log(connection_id, inbound_id)
            WHERE ack_status = 'pending_ack';
        CREATE INDEX inbound_child_events_frame_idx
            ON inbound_child_events(inbound_id, child_ordinal);
        CREATE INDEX outbox_messages_pending_retryable_idx
            ON outbox_messages(connection_id, outbox_id)
            WHERE status IN ('pending', 'retryable') AND runtime_message_id IS NULL;
        CREATE UNIQUE INDEX outbox_messages_connection_msg_id_idx
            ON outbox_messages(connection_id, chat_msg_id);
        CREATE UNIQUE INDEX outbox_messages_chat_item_idx
            ON outbox_messages(chat_item_id);
        CREATE INDEX connections_state_idx ON connections(state);
        CREATE INDEX queue_endpoints_status_idx ON queue_endpoints(status);
        CREATE INDEX contacts_lifecycle_idx ON contacts(lifecycle);
        ",
    )?;
    Ok(())
}

fn apply_migration_v2(transaction: &Transaction<'_>) -> Result<(), RadrootsSimplexAppStoreError> {
    transaction.execute_batch(
        "
        ALTER TABLE chat_items ADD COLUMN chat_msg_id TEXT;
        ALTER TABLE outbox_messages ADD COLUMN chat_item_id TEXT NOT NULL DEFAULT '';
        ALTER TABLE outbox_messages ADD COLUMN chat_msg_id TEXT NOT NULL DEFAULT '';
        UPDATE outbox_messages
        SET chat_item_id = outbox_id
        WHERE chat_item_id = '';
        UPDATE outbox_messages
        SET chat_msg_id = outbox_id
        WHERE chat_msg_id = '';
        CREATE UNIQUE INDEX chat_items_conversation_msg_id_idx
            ON chat_items(conversation_id, chat_msg_id)
            WHERE chat_msg_id IS NOT NULL;
        CREATE UNIQUE INDEX outbox_messages_connection_msg_id_idx
            ON outbox_messages(connection_id, chat_msg_id);
        CREATE UNIQUE INDEX outbox_messages_chat_item_idx
            ON outbox_messages(chat_item_id);
        ",
    )?;
    Ok(())
}

fn apply_migration_v3(transaction: &Transaction<'_>) -> Result<(), RadrootsSimplexAppStoreError> {
    transaction.execute_batch(
        "
        ALTER TABLE inbound_message_log ADD COLUMN app_record_kind TEXT NOT NULL DEFAULT 'inbound_log';
        ALTER TABLE inbound_message_log ADD COLUMN app_record_id TEXT NOT NULL DEFAULT '';
        UPDATE inbound_message_log
        SET app_record_id = inbound_id
        WHERE app_record_id = '';
        UPDATE inbound_message_log
        SET ack_status = 'pending_ack'
        WHERE ack_status = 'pending';
        ",
    )?;
    Ok(())
}

fn apply_migration_v4(transaction: &Transaction<'_>) -> Result<(), RadrootsSimplexAppStoreError> {
    transaction.execute_batch(
        "
        ALTER TABLE inbound_message_log ADD COLUMN runtime_ack_handle TEXT NOT NULL DEFAULT '';
        ALTER TABLE outbox_messages ADD COLUMN runtime_message_id INTEGER;
        CREATE TABLE inbound_child_events (
            child_event_id TEXT PRIMARY KEY,
            inbound_id TEXT NOT NULL REFERENCES inbound_message_log(inbound_id) ON DELETE CASCADE,
            child_ordinal INTEGER NOT NULL,
            app_record_kind TEXT NOT NULL,
            app_record_id TEXT NOT NULL,
            event_kind TEXT NOT NULL,
            chat_msg_id TEXT,
            received_at_unix INTEGER NOT NULL,
            UNIQUE(inbound_id, child_ordinal)
        );
        INSERT INTO inbound_child_events
            (child_event_id, inbound_id, child_ordinal, app_record_kind, app_record_id, event_kind, chat_msg_id, received_at_unix)
        SELECT
            'child_' || inbound_id,
            inbound_id,
            0,
            app_record_kind,
            app_record_id,
            app_record_kind,
            NULL,
            received_at_unix
        FROM inbound_message_log
        WHERE app_record_id <> '';
        UPDATE inbound_message_log
        SET runtime_ack_handle = 'legacy:' || connection_id || ':' || COALESCE(CAST(inbound_sequence AS TEXT), inbound_id);
        CREATE INDEX inbound_child_events_frame_idx
            ON inbound_child_events(inbound_id, child_ordinal);
        DROP INDEX IF EXISTS outbox_messages_pending_retryable_idx;
        CREATE INDEX outbox_messages_pending_retryable_idx
            ON outbox_messages(connection_id, outbox_id)
            WHERE status IN ('pending', 'retryable') AND runtime_message_id IS NULL;
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

fn insert_inbound_log(
    transaction: &Transaction<'_>,
    inbound: &RadrootsSimplexAppInboundMessageLogEntry,
) -> Result<(), RadrootsSimplexAppStoreError> {
    transaction.execute(
        "INSERT INTO inbound_message_log
            (inbound_id, connection_id, broker_message_id_hash, inbound_sequence, message_hash, runtime_ack_handle, ack_status, app_record_kind, app_record_id, received_at_unix)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            inbound.inbound_id,
            inbound.connection_id,
            inbound.broker_message_id_hash,
            inbound.inbound_sequence,
            inbound.message_hash,
            inbound.runtime_ack_handle,
            inbound.ack_status,
            inbound.app_record_kind,
            inbound.app_record_id,
            inbound.received_at_unix
        ],
    )?;
    Ok(())
}

fn generate_chat_msg_id() -> Result<String, RadrootsSimplexAppStoreError> {
    let mut bytes = [0_u8; CHAT_MSG_ID_BYTES];
    getrandom(&mut bytes).map_err(|_| {
        RadrootsSimplexAppStoreError::MessageLifecycle("entropy unavailable".into())
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn validate_chat_msg_id(value: &str) -> Result<(), RadrootsSimplexAppStoreError> {
    let decoded = URL_SAFE_NO_PAD.decode(value.as_bytes()).map_err(|_| {
        RadrootsSimplexAppStoreError::MessageLifecycle("chat msgId must be base64url".into())
    })?;
    if decoded.len() != CHAT_MSG_ID_BYTES {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(format!(
            "chat msgId must decode to {CHAT_MSG_ID_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_outbound_text_request(
    request: &RadrootsSimplexAppOutboundTextRequest,
) -> Result<(), RadrootsSimplexAppStoreError> {
    if request.connection_id.is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "connection id must not be empty".into(),
        ));
    }
    if request.conversation_id.is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "conversation id must not be empty".into(),
        ));
    }
    if request.body.trim().is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "outbound text must not be empty".into(),
        ));
    }
    Ok(())
}

fn validate_inbound_text_request(
    request: &RadrootsSimplexAppInboundTextRequest,
) -> Result<(), RadrootsSimplexAppStoreError> {
    validate_inbound_identity(
        &request.connection_id,
        &request.broker_message_id_hash,
        &request.message_hash,
        &request.runtime_ack_handle,
    )?;
    if request.conversation_id.is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "conversation id must not be empty".into(),
        ));
    }
    if request.body.trim().is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "inbound text must not be empty".into(),
        ));
    }
    if let Some(chat_msg_id) = &request.chat_msg_id {
        validate_chat_msg_id(chat_msg_id)?;
    }
    Ok(())
}

fn validate_inbound_unsupported_request(
    request: &RadrootsSimplexAppInboundUnsupportedEventRequest,
) -> Result<(), RadrootsSimplexAppStoreError> {
    validate_inbound_identity(
        &request.connection_id,
        &request.broker_message_id_hash,
        &request.message_hash,
        &request.runtime_ack_handle,
    )?;
    if request.event_kind.is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "unsupported event kind must not be empty".into(),
        ));
    }
    if request.payload_json.is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "unsupported event payload must not be empty".into(),
        ));
    }
    Ok(())
}

fn validate_inbound_identity(
    connection_id: &str,
    broker_message_id_hash: &[u8],
    message_hash: &[u8],
    runtime_ack_handle: &str,
) -> Result<(), RadrootsSimplexAppStoreError> {
    if connection_id.is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "connection id must not be empty".into(),
        ));
    }
    if broker_message_id_hash.is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "broker message id hash must not be empty".into(),
        ));
    }
    if message_hash.is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "message hash must not be empty".into(),
        ));
    }
    if runtime_ack_handle.is_empty() {
        return Err(RadrootsSimplexAppStoreError::MessageLifecycle(
            "runtime ack handle must not be empty".into(),
        ));
    }
    Ok(())
}

fn next_logical_order(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<i64, RadrootsSimplexAppStoreError> {
    let current: Option<i64> = transaction.query_row(
        "SELECT MAX(logical_order) FROM chat_items WHERE conversation_id = ?1",
        params![conversation_id],
        |row| row.get(0),
    )?;
    Ok(current.unwrap_or(0).saturating_add(1))
}

fn outbound_text_by_msg_id(
    transaction: &Transaction<'_>,
    connection_id: &str,
    chat_msg_id: &str,
) -> Result<Option<RadrootsSimplexAppOutboundTextDraft>, RadrootsSimplexAppStoreError> {
    transaction
        .query_row(
            "SELECT
                c.chat_item_id,
                c.conversation_id,
                c.logical_order,
                c.direction,
                c.chat_msg_id,
                c.body,
                c.delivery_status,
                c.created_at_unix,
                o.outbox_id,
                o.chat_item_id,
                o.connection_id,
                o.conversation_id,
                o.chat_msg_id,
                o.body,
                o.status,
                o.runtime_message_id,
                o.retry_after_unix,
                o.created_at_unix
             FROM outbox_messages o
             JOIN chat_items c ON c.chat_item_id = o.chat_item_id
             WHERE o.connection_id = ?1 AND o.chat_msg_id = ?2",
            params![connection_id, chat_msg_id],
            outbound_text_draft_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn outbound_text_by_outbox_id(
    transaction: &Transaction<'_>,
    outbox_id: &str,
) -> Result<Option<RadrootsSimplexAppOutboundTextDraft>, RadrootsSimplexAppStoreError> {
    transaction
        .query_row(
            "SELECT
                c.chat_item_id,
                c.conversation_id,
                c.logical_order,
                c.direction,
                c.chat_msg_id,
                c.body,
                c.delivery_status,
                c.created_at_unix,
                o.outbox_id,
                o.chat_item_id,
                o.connection_id,
                o.conversation_id,
                o.chat_msg_id,
                o.body,
                o.status,
                o.runtime_message_id,
                o.retry_after_unix,
                o.created_at_unix
             FROM outbox_messages o
             JOIN chat_items c ON c.chat_item_id = o.chat_item_id
             WHERE o.outbox_id = ?1",
            params![outbox_id],
            outbound_text_draft_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn inbound_frame_by_identity(
    transaction: &Transaction<'_>,
    connection_id: &str,
    broker_message_id_hash: &[u8],
    inbound_sequence: Option<i64>,
    message_hash: &[u8],
) -> Result<Option<RadrootsSimplexAppInboundMessageLogEntry>, RadrootsSimplexAppStoreError> {
    Ok(match inbound_sequence {
        Some(sequence) => transaction
            .query_row(
                "SELECT inbound_id, connection_id, broker_message_id_hash, inbound_sequence, message_hash, runtime_ack_handle, ack_status, app_record_kind, app_record_id, received_at_unix
                 FROM inbound_message_log
                 WHERE connection_id = ?1
                   AND (broker_message_id_hash = ?2 OR (inbound_sequence = ?3 AND message_hash = ?4))
                 ORDER BY received_at_unix, inbound_id
                 LIMIT 1",
                params![connection_id, broker_message_id_hash, sequence, message_hash],
                inbound_message_from_row,
            )
            .optional()?,
        None => transaction
            .query_row(
                "SELECT inbound_id, connection_id, broker_message_id_hash, inbound_sequence, message_hash, runtime_ack_handle, ack_status, app_record_kind, app_record_id, received_at_unix
                 FROM inbound_message_log
                 WHERE connection_id = ?1 AND broker_message_id_hash = ?2
                 ORDER BY received_at_unix, inbound_id
                 LIMIT 1",
                params![connection_id, broker_message_id_hash],
                inbound_message_from_row,
            )
            .optional()?,
    })
}

fn ensure_inbound_frame(
    transaction: &Transaction<'_>,
    connection_id: &str,
    broker_message_id_hash: &[u8],
    inbound_sequence: Option<i64>,
    message_hash: &[u8],
    runtime_ack_handle: &str,
    received_at_unix: i64,
) -> Result<RadrootsSimplexAppInboundMessageLogEntry, RadrootsSimplexAppStoreError> {
    if let Some(existing) = inbound_frame_by_identity(
        transaction,
        connection_id,
        broker_message_id_hash,
        inbound_sequence,
        message_hash,
    )? {
        return Ok(existing);
    }
    let inbound_id = derive_inbound_frame_local_id(
        "inbound",
        connection_id,
        broker_message_id_hash,
        message_hash,
    );
    let inbound = RadrootsSimplexAppInboundMessageLogEntry {
        inbound_id: inbound_id.clone(),
        connection_id: connection_id.to_owned(),
        broker_message_id_hash: broker_message_id_hash.to_vec(),
        inbound_sequence,
        message_hash: message_hash.to_vec(),
        runtime_ack_handle: runtime_ack_handle.to_owned(),
        ack_status: "pending_ack".to_owned(),
        app_record_kind: "frame".to_owned(),
        app_record_id: inbound_id,
        received_at_unix,
    };
    insert_inbound_log(transaction, &inbound)?;
    Ok(inbound)
}

fn inbound_child_commit_by_ordinal(
    transaction: &Transaction<'_>,
    inbound: &RadrootsSimplexAppInboundMessageLogEntry,
    child_ordinal: u32,
) -> Result<Option<RadrootsSimplexAppInboundCommit>, RadrootsSimplexAppStoreError> {
    let child_event = transaction
        .query_row(
            "SELECT child_event_id, inbound_id, child_ordinal, app_record_kind, app_record_id, event_kind, chat_msg_id, received_at_unix
             FROM inbound_child_events
             WHERE inbound_id = ?1 AND child_ordinal = ?2
             LIMIT 1",
            params![inbound.inbound_id, i64::from(child_ordinal)],
            inbound_child_event_from_row,
        )
        .optional()?;
    let Some(child_event) = child_event else {
        return Ok(None);
    };
    let chat_item = if child_event.app_record_kind == "chat_item" {
        chat_item_by_id(transaction, &child_event.app_record_id)?
    } else {
        None
    };
    let unsupported_event = if child_event.app_record_kind == "unsupported_event" {
        unsupported_event_by_id(transaction, &child_event.app_record_id)?
    } else {
        None
    };
    Ok(Some(RadrootsSimplexAppInboundCommit {
        inbound: inbound.clone(),
        child_event,
        chat_item,
        unsupported_event,
        duplicate: true,
    }))
}

fn insert_inbound_child_event(
    transaction: &Transaction<'_>,
    child_event: &RadrootsSimplexAppInboundChildEvent,
) -> Result<(), RadrootsSimplexAppStoreError> {
    transaction.execute(
        "INSERT INTO inbound_child_events
            (child_event_id, inbound_id, child_ordinal, app_record_kind, app_record_id, event_kind, chat_msg_id, received_at_unix)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            child_event.child_event_id,
            child_event.inbound_id,
            i64::from(child_event.child_ordinal),
            child_event.app_record_kind,
            child_event.app_record_id,
            child_event.event_kind,
            child_event.chat_msg_id,
            child_event.received_at_unix
        ],
    )?;
    Ok(())
}

fn chat_item_by_id(
    transaction: &Transaction<'_>,
    chat_item_id: &str,
) -> Result<Option<RadrootsSimplexAppChatItem>, RadrootsSimplexAppStoreError> {
    transaction
        .query_row(
            "SELECT chat_item_id, conversation_id, logical_order, direction, chat_msg_id, body, delivery_status, created_at_unix
             FROM chat_items
             WHERE chat_item_id = ?1",
            params![chat_item_id],
            chat_item_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn unsupported_event_by_id(
    transaction: &Transaction<'_>,
    event_id: &str,
) -> Result<Option<RadrootsSimplexAppUnsupportedProtocolEvent>, RadrootsSimplexAppStoreError> {
    transaction
        .query_row(
            "SELECT event_id, connection_id, event_kind, payload_json, status, received_at_unix
             FROM unsupported_protocol_events
             WHERE event_id = ?1",
            params![event_id],
            unsupported_event_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn derive_outbound_local_id(prefix: &str, connection_id: &str, chat_msg_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update([0]);
    hasher.update(connection_id.as_bytes());
    hasher.update([0]);
    hasher.update(chat_msg_id.as_bytes());
    let digest = hasher.finalize();
    format!("{prefix}_{}", hex::encode(&digest[..16]))
}

fn derive_inbound_frame_local_id(
    prefix: &str,
    connection_id: &str,
    broker_message_id_hash: &[u8],
    message_hash: &[u8],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update([0]);
    hasher.update(connection_id.as_bytes());
    hasher.update([0]);
    hasher.update(broker_message_id_hash);
    hasher.update([0]);
    hasher.update(message_hash);
    let digest = hasher.finalize();
    format!("{prefix}_{}", hex::encode(&digest[..16]))
}

fn derive_inbound_child_local_id(
    prefix: &str,
    inbound_id: &str,
    child_ordinal: u32,
    key: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update([0]);
    hasher.update(inbound_id.as_bytes());
    hasher.update([0]);
    hasher.update(child_ordinal.to_be_bytes());
    hasher.update([0]);
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    format!("{prefix}_{}", hex::encode(&digest[..16]))
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
        chat_msg_id: row.get(4)?,
        body: row.get(5)?,
        delivery_status: row.get(6)?,
        created_at_unix: row.get(7)?,
    })
}

fn outbound_text_draft_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<RadrootsSimplexAppOutboundTextDraft> {
    let direction: String = row.get(3)?;
    Ok(RadrootsSimplexAppOutboundTextDraft {
        chat_item: RadrootsSimplexAppChatItem {
            chat_item_id: row.get(0)?,
            conversation_id: row.get(1)?,
            logical_order: row.get(2)?,
            direction: RadrootsSimplexAppChatDirection::parse(&direction)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))?,
            chat_msg_id: row.get(4)?,
            body: row.get(5)?,
            delivery_status: row.get(6)?,
            created_at_unix: row.get(7)?,
        },
        outbox_message: RadrootsSimplexAppOutboxMessage {
            outbox_id: row.get(8)?,
            chat_item_id: row.get(9)?,
            connection_id: row.get(10)?,
            conversation_id: row.get(11)?,
            chat_msg_id: row.get(12)?,
            body: row.get(13)?,
            status: row.get(14)?,
            runtime_message_id: row.get(15)?,
            retry_after_unix: row.get(16)?,
            created_at_unix: row.get(17)?,
        },
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
        runtime_ack_handle: row.get(5)?,
        ack_status: row.get(6)?,
        app_record_kind: row.get(7)?,
        app_record_id: row.get(8)?,
        received_at_unix: row.get(9)?,
    })
}

fn inbound_child_event_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<RadrootsSimplexAppInboundChildEvent> {
    let child_ordinal: i64 = row.get(2)?;
    let child_ordinal = u32::try_from(child_ordinal).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    Ok(RadrootsSimplexAppInboundChildEvent {
        child_event_id: row.get(0)?,
        inbound_id: row.get(1)?,
        child_ordinal,
        app_record_kind: row.get(3)?,
        app_record_id: row.get(4)?,
        event_kind: row.get(5)?,
        chat_msg_id: row.get(6)?,
        received_at_unix: row.get(7)?,
    })
}

fn outbox_message_from_row(row: &Row<'_>) -> rusqlite::Result<RadrootsSimplexAppOutboxMessage> {
    Ok(RadrootsSimplexAppOutboxMessage {
        outbox_id: row.get(0)?,
        chat_item_id: row.get(1)?,
        connection_id: row.get(2)?,
        conversation_id: row.get(3)?,
        chat_msg_id: row.get(4)?,
        body: row.get(5)?,
        status: row.get(6)?,
        runtime_message_id: row.get(7)?,
        retry_after_unix: row.get(8)?,
        created_at_unix: row.get(9)?,
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

    fn outbound_request() -> RadrootsSimplexAppOutboundTextRequest {
        RadrootsSimplexAppOutboundTextRequest {
            connection_id: "connection-1".into(),
            conversation_id: "conversation-1".into(),
            body: "hello encrypted iPhone".into(),
            created_at_unix: 11,
        }
    }

    fn inbound_text_request() -> RadrootsSimplexAppInboundTextRequest {
        RadrootsSimplexAppInboundTextRequest {
            connection_id: "connection-1".into(),
            conversation_id: "conversation-1".into(),
            broker_message_id_hash: b"broker-message-hash-1".to_vec(),
            inbound_sequence: Some(21),
            message_hash: b"agent-message-hash-1".to_vec(),
            runtime_ack_handle: "ack-handle-1".into(),
            child_ordinal: 0,
            chat_msg_id: Some("AQIDBAUGBwgJCgsM".into()),
            body: "hello from the iPhone".into(),
            received_at_unix: 12,
        }
    }

    fn inbound_unsupported_request() -> RadrootsSimplexAppInboundUnsupportedEventRequest {
        RadrootsSimplexAppInboundUnsupportedEventRequest {
            connection_id: "connection-1".into(),
            broker_message_id_hash: b"broker-message-hash-2".to_vec(),
            inbound_sequence: Some(22),
            message_hash: b"agent-message-hash-2".to_vec(),
            runtime_ack_handle: "ack-handle-2".into(),
            child_ordinal: 0,
            event_kind: "x.future.dm".into(),
            payload_json: "{\"event\":\"x.future.dm\"}".into(),
            received_at_unix: 13,
        }
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
        assert_eq!(diagnostics.schema_version, 4);
        assert_eq!(diagnostics.migration_count, 4);
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
                chat_msg_id: Some("AQIDBAUGBwgJCgsM".into()),
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
                chat_msg_id: None,
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
                runtime_ack_handle: "ack-handle-manual".into(),
                ack_status: "pending_ack".into(),
                app_record_kind: "chat_item".into(),
                app_record_id: "chat-2".into(),
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
                chat_item_id: "chat-1".into(),
                connection_id: "connection-1".into(),
                conversation_id: Some("conversation-1".into()),
                chat_msg_id: "AQIDBAUGBwgJCgsM".into(),
                body: "queued plaintext before encryption".into(),
                status: "retryable".into(),
                runtime_message_id: None,
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
                chat_msg_id: Some("AQIDBAUGBwgJCgsM".into()),
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
            runtime_ack_handle: "ack-handle-dedupe".into(),
            ack_status: "pending_ack".into(),
            app_record_kind: "chat_item".into(),
            app_record_id: "chat-1".into(),
            received_at_unix: 8,
        };
        store.record_inbound_message(&inbound).expect("inbound");
        let duplicate = RadrootsSimplexAppInboundMessageLogEntry {
            inbound_id: "inbound-2".into(),
            ..inbound
        };
        assert!(store.record_inbound_message(&duplicate).is_err());
    }

    #[test]
    fn inbound_text_commit_persists_chat_item_and_pending_ack() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let commit = store
            .commit_inbound_text(&inbound_text_request())
            .expect("commit");

        assert!(!commit.duplicate);
        assert_eq!(commit.inbound.ack_status, "pending_ack");
        assert_eq!(commit.inbound.app_record_kind, "frame");
        assert_eq!(commit.inbound.runtime_ack_handle, "ack-handle-1");
        let chat_item = commit.chat_item.expect("chat item");
        assert_eq!(commit.child_event.app_record_kind, "chat_item");
        assert_eq!(commit.child_event.app_record_id, chat_item.chat_item_id);
        assert_eq!(
            chat_item.direction,
            RadrootsSimplexAppChatDirection::Inbound
        );
        assert_eq!(chat_item.chat_msg_id.as_deref(), Some("AQIDBAUGBwgJCgsM"));
        assert_eq!(chat_item.body, "hello from the iPhone");
        assert_eq!(
            store.chat_page("conversation-1", 10).expect("page"),
            vec![chat_item]
        );
        assert_eq!(
            store.pending_ack_messages().expect("pending ack"),
            vec![commit.inbound]
        );
    }

    #[test]
    fn inbound_text_duplicate_redelivery_returns_prior_commit() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let first = store
            .commit_inbound_text(&inbound_text_request())
            .expect("first");
        let second = store
            .commit_inbound_text(&inbound_text_request())
            .expect("second");

        assert!(second.duplicate);
        assert_eq!(second.inbound, first.inbound);
        assert_eq!(second.chat_item, first.chat_item);
        assert_eq!(
            store.chat_page("conversation-1", 10).expect("page").len(),
            1
        );
        assert_eq!(store.pending_ack_messages().expect("pending").len(), 1);
    }

    #[test]
    fn inbound_frame_persists_multiple_child_events_with_one_pending_ack() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let first = store
            .commit_inbound_text(&inbound_text_request())
            .expect("first");
        let second = store
            .commit_inbound_text(&RadrootsSimplexAppInboundTextRequest {
                child_ordinal: 1,
                chat_msg_id: Some("AgIDBAUGBwgJCgsM".into()),
                body: "second child event".into(),
                ..inbound_text_request()
            })
            .expect("second");

        assert_eq!(first.inbound.inbound_id, second.inbound.inbound_id);
        assert_eq!(first.child_event.child_ordinal, 0);
        assert_eq!(second.child_event.child_ordinal, 1);
        assert_eq!(store.pending_ack_messages().expect("pending").len(), 1);
        assert_eq!(
            store.chat_page("conversation-1", 10).expect("page").len(),
            2
        );
    }

    #[test]
    fn inbound_unsupported_event_commit_persists_safe_record_and_pending_ack() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let commit = store
            .commit_inbound_unsupported_event(&inbound_unsupported_request())
            .expect("commit");

        assert!(!commit.duplicate);
        assert_eq!(commit.inbound.ack_status, "pending_ack");
        assert_eq!(commit.inbound.app_record_kind, "frame");
        let unsupported = commit.unsupported_event.expect("unsupported event");
        assert_eq!(commit.child_event.app_record_kind, "unsupported_event");
        assert_eq!(commit.child_event.app_record_id, unsupported.event_id);
        assert_eq!(unsupported.event_kind, "x.future.dm");
        assert_eq!(unsupported.status, "stored");
        assert_eq!(
            store
                .list_unsupported_protocol_events()
                .expect("unsupported"),
            vec![unsupported]
        );
        assert_eq!(store.pending_ack_messages().expect("pending").len(), 1);
    }

    #[test]
    fn inbound_ack_delivery_marks_pending_row_acked() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let commit = store
            .commit_inbound_text(&inbound_text_request())
            .expect("commit");
        let acked = store
            .mark_inbound_ack_delivered("connection-1", 21, b"agent-message-hash-1")
            .expect("ack")
            .expect("row");

        assert_eq!(acked.inbound_id, commit.inbound.inbound_id);
        assert_eq!(acked.ack_status, "acked");
        assert!(store.pending_ack_messages().expect("pending").is_empty());
        assert_eq!(
            store
                .mark_inbound_ack_delivered("connection-1", 21, b"agent-message-hash-1")
                .expect("idempotent")
                .expect("row")
                .ack_status,
            "acked"
        );
        assert!(
            store
                .mark_inbound_ack_delivered("connection-1", 21, b"wrong-hash")
                .expect("missing")
                .is_none()
        );
    }

    #[test]
    fn invalid_inbound_text_does_not_create_chat_or_pending_ack() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);
        let invalid = RadrootsSimplexAppInboundTextRequest {
            body: " ".into(),
            ..inbound_text_request()
        };

        assert!(store.commit_inbound_text(&invalid).is_err());
        assert!(
            store
                .chat_page("conversation-1", 10)
                .expect("page")
                .is_empty()
        );
        assert!(store.pending_ack_messages().expect("pending").is_empty());
    }

    #[test]
    fn outbound_text_lifecycle_persists_chat_item_outbox_and_msg_id() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let draft = store
            .create_outbound_text_with_test_msg_id(&outbound_request(), "AQIDBAUGBwgJCgsM")
            .expect("draft");

        assert_eq!(
            draft.chat_item.direction,
            RadrootsSimplexAppChatDirection::Outbound
        );
        assert_eq!(
            draft.chat_item.chat_msg_id.as_deref(),
            Some("AQIDBAUGBwgJCgsM")
        );
        assert_eq!(draft.chat_item.delivery_status, "pending");
        assert_eq!(
            draft.outbox_message.chat_item_id,
            draft.chat_item.chat_item_id
        );
        assert_eq!(draft.outbox_message.chat_msg_id, "AQIDBAUGBwgJCgsM");
        assert_eq!(draft.outbox_message.status, "pending");
        let page = store.chat_page("conversation-1", 10).expect("page");
        assert_eq!(page, vec![draft.chat_item]);
        let pending = store.pending_outbox_messages().expect("pending");
        assert_eq!(pending, vec![draft.outbox_message.clone()]);
        assert_eq!(
            store.list_outbox_messages().expect("outbox"),
            vec![draft.outbox_message]
        );
    }

    #[test]
    fn reset_disposable_runtime_state_preserves_profiles_and_clears_messages() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let draft = store
            .create_outbound_text_with_test_msg_id(&outbound_request(), "AQIDBAUGBwgJCgsM")
            .expect("draft");
        let commit = store
            .commit_inbound_text(&RadrootsSimplexAppInboundTextRequest {
                chat_msg_id: Some("AgIDBAUGBwgJCgsM".into()),
                broker_message_id_hash: b"reset-broker-hash".to_vec(),
                message_hash: b"reset-message-hash".to_vec(),
                runtime_ack_handle: "ack-handle-reset".into(),
                ..inbound_text_request()
            })
            .expect("inbound");
        store
            .record_unsupported_protocol_event(&RadrootsSimplexAppUnsupportedProtocolEvent {
                event_id: "unsupported-1".into(),
                connection_id: Some("connection-1".into()),
                event_kind: "x.future".into(),
                payload_json: "{}".into(),
                status: "stored".into(),
                received_at_unix: 11,
            })
            .expect("unsupported");

        assert_eq!(store.pending_outbox_messages().expect("outbox").len(), 1);
        assert_eq!(
            store.pending_ack_messages().expect("acks"),
            vec![commit.inbound]
        );
        assert_eq!(
            store
                .list_unsupported_protocol_events()
                .expect("unsupported")
                .len(),
            1
        );

        store
            .reset_disposable_runtime_state()
            .expect("reset disposable state");

        assert_eq!(
            store.get_profile("profile-1").expect("profile"),
            Some(profile())
        );
        assert!(store.pending_outbox_messages().expect("outbox").is_empty());
        assert!(store.list_outbox_messages().expect("outbox").is_empty());
        assert!(store.pending_ack_messages().expect("acks").is_empty());
        assert!(
            store
                .chat_page("conversation-1", 10)
                .expect("chat")
                .is_empty()
        );
        assert!(
            store
                .list_unsupported_protocol_events()
                .expect("unsupported")
                .is_empty()
        );
        assert!(
            store
                .mark_outbox_message_sent(&draft.outbox_message.outbox_id)
                .expect("missing")
                .is_none()
        );
    }

    #[test]
    fn outbound_text_retry_preserves_msg_id_and_chat_item() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let first = store
            .create_outbound_text_with_test_msg_id(&outbound_request(), "AQIDBAUGBwgJCgsM")
            .expect("first");
        let second = store
            .create_outbound_text_with_test_msg_id(&outbound_request(), "AQIDBAUGBwgJCgsM")
            .expect("second");

        assert_eq!(second, first);
        assert_eq!(
            store.chat_page("conversation-1", 10).expect("page").len(),
            1
        );
        assert_eq!(store.pending_outbox_messages().expect("pending").len(), 1);
    }

    #[test]
    fn outbound_runtime_correlation_removes_message_from_retry_queue() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let draft = store
            .create_outbound_text_with_test_msg_id(&outbound_request(), "AQIDBAUGBwgJCgsM")
            .expect("draft");
        let queued = store
            .mark_outbox_message_queued(&draft.outbox_message.outbox_id, 42)
            .expect("queued")
            .expect("queued row");

        assert_eq!(queued.outbox_message.runtime_message_id, Some(42));
        assert!(store.pending_outbox_messages().expect("pending").is_empty());
    }

    #[test]
    fn outbound_delivery_state_updates_are_idempotent() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let draft = store
            .create_outbound_text_with_test_msg_id(&outbound_request(), "AQIDBAUGBwgJCgsM")
            .expect("draft");
        let sent = store
            .mark_outbox_message_sent(&draft.outbox_message.outbox_id)
            .expect("sent")
            .expect("sent row");

        assert_eq!(sent.outbox_message.status, "sent");
        assert_eq!(sent.chat_item.delivery_status, "sent");
        assert!(store.pending_outbox_messages().expect("pending").is_empty());
        assert_eq!(
            store
                .mark_outbox_message_sent(&draft.outbox_message.outbox_id)
                .expect("sent again")
                .expect("sent row")
                .outbox_message
                .status,
            "sent"
        );

        let acknowledged = store
            .mark_outbox_message_acknowledged(&draft.outbox_message.outbox_id)
            .expect("acknowledged")
            .expect("acknowledged row");
        assert_eq!(acknowledged.outbox_message.status, "acknowledged");
        assert_eq!(acknowledged.chat_item.delivery_status, "acknowledged");
        assert_eq!(
            store
                .mark_outbox_message_sent(&draft.outbox_message.outbox_id)
                .expect("sent after acknowledged")
                .expect("row")
                .outbox_message
                .status,
            "acknowledged"
        );
        assert!(
            store
                .mark_outbox_message_acknowledged("missing-outbox")
                .expect("missing")
                .is_none()
        );
    }

    #[test]
    fn outbound_delivery_transitions_fail_closed() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let draft = store
            .create_outbound_text_with_test_msg_id(&outbound_request(), "AQIDBAUGBwgJCgsM")
            .expect("draft");
        let error = store
            .mark_outbox_message_acknowledged(&draft.outbox_message.outbox_id)
            .err()
            .expect("transition error");

        assert!(matches!(
            error,
            RadrootsSimplexAppStoreError::MessageLifecycle(_)
        ));
    }

    #[test]
    fn outbound_text_generates_twelve_byte_base64url_msg_id() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("simplex.sqlite");
        let vault = Arc::new(RadrootsSecretVaultMemory::new());
        let store = memory_store(&path, vault).expect("store");
        seed_store(&store);

        let draft = store
            .create_outbound_text(&outbound_request())
            .expect("draft");
        let chat_msg_id = draft.outbox_message.chat_msg_id;
        let decoded = URL_SAFE_NO_PAD
            .decode(chat_msg_id.as_bytes())
            .expect("base64url");

        assert_eq!(decoded.len(), CHAT_MSG_ID_BYTES);
        assert_eq!(
            draft.chat_item.chat_msg_id.as_deref(),
            Some(chat_msg_id.as_str())
        );
    }
}

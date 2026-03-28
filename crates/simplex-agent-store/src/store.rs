use crate::error::RadrootsSimplexAgentStoreError;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use radroots_simplex_agent_proto::prelude::{
    RadrootsSimplexAgentConnectionLink, RadrootsSimplexAgentConnectionMode,
    RadrootsSimplexAgentConnectionStatus, RadrootsSimplexAgentEnvelope,
    RadrootsSimplexAgentMessageId, RadrootsSimplexAgentMessageReceipt,
    RadrootsSimplexAgentQueueAddress, RadrootsSimplexAgentQueueDescriptor,
    RadrootsSimplexSmpRatchetState, decode_connection_link, decode_envelope,
    encode_connection_link, encode_envelope,
};
use radroots_simplex_smp_crypto::prelude::RadrootsSimplexSmpEd25519Keypair;
use radroots_simplex_smp_proto::prelude::{
    RadrootsSimplexSmpQueueUri, RadrootsSimplexSmpServerAddress,
};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::fs;
#[cfg(feature = "std")]
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum RadrootsSimplexAgentQueueRole {
    Receive,
    Send,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RadrootsSimplexAgentQueueAuthState {
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentQueueRecord {
    pub descriptor: RadrootsSimplexAgentQueueDescriptor,
    pub role: RadrootsSimplexAgentQueueRole,
    pub subscribed: bool,
    pub primary: bool,
    pub tested: bool,
    pub auth_state: Option<RadrootsSimplexAgentQueueAuthState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RadrootsSimplexAgentDeliveryCursor {
    pub last_sent_message_id: Option<RadrootsSimplexAgentMessageId>,
    pub last_received_message_id: Option<RadrootsSimplexAgentMessageId>,
    pub last_sent_message_hash: Option<Vec<u8>>,
    pub last_received_message_hash: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RadrootsSimplexAgentRecentMessageRecord {
    pub message_id: RadrootsSimplexAgentMessageId,
    pub message_hash: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RadrootsSimplexAgentOutboundMessage {
    pub message_id: RadrootsSimplexAgentMessageId,
    pub message_hash: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentPreparedOutboundMessage {
    pub message_id: RadrootsSimplexAgentMessageId,
    pub previous_message_hash: Vec<u8>,
    pub message_hash: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentPendingCommandKind {
    CreateQueue {
        descriptor: RadrootsSimplexAgentQueueDescriptor,
    },
    SecureQueue {
        queue: RadrootsSimplexAgentQueueAddress,
        sender_key: Option<Vec<u8>>,
    },
    SendEnvelope {
        queue: RadrootsSimplexAgentQueueAddress,
        envelope: RadrootsSimplexAgentEnvelope,
        delivery: Option<RadrootsSimplexAgentOutboundMessage>,
    },
    SubscribeQueue {
        queue: RadrootsSimplexAgentQueueAddress,
    },
    AckInboxMessage {
        queue: RadrootsSimplexAgentQueueAddress,
        receipt: RadrootsSimplexAgentMessageReceipt,
    },
    RotateQueues {
        descriptors: Vec<RadrootsSimplexAgentQueueDescriptor>,
    },
    TestQueues {
        queues: Vec<RadrootsSimplexAgentQueueAddress>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentPendingCommand {
    pub id: u64,
    pub connection_id: String,
    pub kind: RadrootsSimplexAgentPendingCommandKind,
    pub attempts: u32,
    pub ready_at: u64,
    pub inflight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentConnectionRecord {
    pub id: String,
    pub mode: RadrootsSimplexAgentConnectionMode,
    pub status: RadrootsSimplexAgentConnectionStatus,
    pub invitation: Option<RadrootsSimplexAgentConnectionLink>,
    pub queues: Vec<RadrootsSimplexAgentQueueRecord>,
    pub ratchet_state: Option<RadrootsSimplexSmpRatchetState>,
    pub delivery_cursor: RadrootsSimplexAgentDeliveryCursor,
    pub recent_messages: Vec<RadrootsSimplexAgentRecentMessageRecord>,
    pub staged_outbound_message: Option<RadrootsSimplexAgentOutboundMessage>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentStoreSnapshot {
    next_connection_sequence: u64,
    next_command_sequence: u64,
    connections: Vec<RadrootsSimplexAgentConnectionSnapshot>,
    pending_commands: Vec<RadrootsSimplexAgentPendingCommandSnapshot>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentConnectionSnapshot {
    id: String,
    mode: String,
    status: String,
    invitation: Option<Vec<u8>>,
    queues: Vec<RadrootsSimplexAgentQueueRecordSnapshot>,
    ratchet_state: Option<RadrootsSimplexAgentRatchetStateSnapshot>,
    delivery_cursor: RadrootsSimplexAgentDeliveryCursor,
    recent_messages: Vec<RadrootsSimplexAgentRecentMessageRecord>,
    staged_outbound_message: Option<RadrootsSimplexAgentOutboundMessage>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentQueueRecordSnapshot {
    descriptor: RadrootsSimplexAgentQueueDescriptorSnapshot,
    role: String,
    subscribed: bool,
    primary: bool,
    tested: bool,
    auth_state: Option<RadrootsSimplexAgentQueueAuthState>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentQueueDescriptorSnapshot {
    queue_uri: String,
    replaced_queue: Option<RadrootsSimplexAgentQueueAddressSnapshot>,
    primary: bool,
    sender_key: Option<Vec<u8>>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentQueueAddressSnapshot {
    server_identity: String,
    hosts: Vec<String>,
    port: Option<u16>,
    sender_id: Vec<u8>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentRatchetStateSnapshot {
    role: String,
    root_epoch: u64,
    previous_sending_chain_length: u32,
    sending_chain_length: u32,
    receiving_chain_length: u32,
    local_dh_public_key: Vec<u8>,
    remote_dh_public_key: Vec<u8>,
    current_pq_public_key: Option<Vec<u8>>,
    remote_pq_public_key: Option<Vec<u8>>,
    pending_outbound_pq_ciphertext: Option<Vec<u8>>,
    pending_inbound_pq_ciphertext: Option<Vec<u8>>,
    current_pq_shared_secret: Option<Vec<u8>>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentPendingCommandSnapshot {
    id: u64,
    connection_id: String,
    kind: RadrootsSimplexAgentPendingCommandKindSnapshot,
    attempts: u32,
    ready_at: u64,
    inflight: bool,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RadrootsSimplexAgentPendingCommandKindSnapshot {
    CreateQueue {
        descriptor: RadrootsSimplexAgentQueueDescriptorSnapshot,
    },
    SecureQueue {
        queue: RadrootsSimplexAgentQueueAddressSnapshot,
        sender_key: Option<Vec<u8>>,
    },
    SendEnvelope {
        queue: RadrootsSimplexAgentQueueAddressSnapshot,
        envelope: Vec<u8>,
        delivery: Option<RadrootsSimplexAgentOutboundMessage>,
    },
    SubscribeQueue {
        queue: RadrootsSimplexAgentQueueAddressSnapshot,
    },
    AckInboxMessage {
        queue: RadrootsSimplexAgentQueueAddressSnapshot,
        receipt: RadrootsSimplexAgentMessageReceiptSnapshot,
    },
    RotateQueues {
        descriptors: Vec<RadrootsSimplexAgentQueueDescriptorSnapshot>,
    },
    TestQueues {
        queues: Vec<RadrootsSimplexAgentQueueAddressSnapshot>,
    },
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentMessageReceiptSnapshot {
    message_id: RadrootsSimplexAgentMessageId,
    message_hash: Vec<u8>,
    receipt_info: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct RadrootsSimplexAgentStore {
    next_connection_sequence: u64,
    next_command_sequence: u64,
    connections: BTreeMap<String, RadrootsSimplexAgentConnectionRecord>,
    pending_commands: BTreeMap<u64, RadrootsSimplexAgentPendingCommand>,
    #[cfg(feature = "std")]
    persistence_path: Option<PathBuf>,
}

impl Default for RadrootsSimplexAgentStore {
    fn default() -> Self {
        Self {
            next_connection_sequence: 0,
            next_command_sequence: 0,
            connections: BTreeMap::new(),
            pending_commands: BTreeMap::new(),
            #[cfg(feature = "std")]
            persistence_path: None,
        }
    }
}

impl RadrootsSimplexAgentStore {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "std")]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RadrootsSimplexAgentStoreError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            let mut store = Self::default();
            store.persistence_path = Some(path);
            return Ok(store);
        }

        let raw = fs::read(&path).map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to read SimpleX agent store snapshot `{}`: {error}",
                path.display()
            ))
        })?;

        let snapshot: RadrootsSimplexAgentStoreSnapshot =
            serde_json::from_slice(&raw).map_err(|error| {
                RadrootsSimplexAgentStoreError::Persistence(format!(
                    "failed to parse SimpleX agent store snapshot `{}`: {error}",
                    path.display()
                ))
            })?;

        let mut store = Self::from_snapshot(snapshot)?;
        store.persistence_path = Some(path);
        Ok(store)
    }

    #[cfg(feature = "std")]
    pub fn set_persistence_path(&mut self, path: impl AsRef<Path>) {
        self.persistence_path = Some(path.as_ref().to_path_buf());
    }

    #[cfg(feature = "std")]
    pub fn flush(&self) -> Result<(), RadrootsSimplexAgentStoreError> {
        let Some(path) = self.persistence_path.as_ref() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RadrootsSimplexAgentStoreError::Persistence(format!(
                    "failed to create SimpleX agent store directory `{}`: {error}",
                    parent.display()
                ))
            })?;
        }
        let snapshot = self.snapshot()?;
        let mut encoded = serde_json::to_vec_pretty(&snapshot).map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to serialize SimpleX agent store snapshot `{}`: {error}",
                path.display()
            ))
        })?;
        encoded.push(b'\n');
        fs::write(path, encoded).map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to write SimpleX agent store snapshot `{}`: {error}",
                path.display()
            ))
        })
    }

    pub fn create_connection(
        &mut self,
        mode: RadrootsSimplexAgentConnectionMode,
        status: RadrootsSimplexAgentConnectionStatus,
        invitation: Option<RadrootsSimplexAgentConnectionLink>,
        ratchet_state: Option<RadrootsSimplexSmpRatchetState>,
    ) -> RadrootsSimplexAgentConnectionRecord {
        self.next_connection_sequence = self.next_connection_sequence.saturating_add(1);
        let id = alloc::format!("conn-{}", self.next_connection_sequence);
        let record = RadrootsSimplexAgentConnectionRecord {
            id: id.clone(),
            mode,
            status,
            invitation,
            queues: Vec::new(),
            ratchet_state,
            delivery_cursor: RadrootsSimplexAgentDeliveryCursor {
                last_sent_message_id: None,
                last_received_message_id: None,
                last_sent_message_hash: None,
                last_received_message_hash: None,
            },
            recent_messages: Vec::new(),
            staged_outbound_message: None,
        };
        self.connections.insert(id, record.clone());
        record
    }

    pub fn connection(
        &self,
        connection_id: &str,
    ) -> Result<&RadrootsSimplexAgentConnectionRecord, RadrootsSimplexAgentStoreError> {
        self.connections
            .get(connection_id)
            .ok_or_else(|| RadrootsSimplexAgentStoreError::ConnectionNotFound(connection_id.into()))
    }

    pub fn connection_mut(
        &mut self,
        connection_id: &str,
    ) -> Result<&mut RadrootsSimplexAgentConnectionRecord, RadrootsSimplexAgentStoreError> {
        self.connections
            .get_mut(connection_id)
            .ok_or_else(|| RadrootsSimplexAgentStoreError::ConnectionNotFound(connection_id.into()))
    }

    pub fn set_status(
        &mut self,
        connection_id: &str,
        status: RadrootsSimplexAgentConnectionStatus,
    ) -> Result<(), RadrootsSimplexAgentStoreError> {
        self.connection_mut(connection_id)?.status = status;
        Ok(())
    }

    pub fn add_queue(
        &mut self,
        connection_id: &str,
        descriptor: RadrootsSimplexAgentQueueDescriptor,
        role: RadrootsSimplexAgentQueueRole,
        primary: bool,
        auth_state: RadrootsSimplexAgentQueueAuthState,
    ) -> Result<(), RadrootsSimplexAgentStoreError> {
        let connection = self.connection_mut(connection_id)?;
        let address = descriptor.queue_address();
        if let Some(queue) = connection
            .queues
            .iter_mut()
            .find(|queue| queue.descriptor.queue_address() == address)
        {
            queue.descriptor = descriptor;
            queue.role = role;
            queue.primary = primary;
            queue.auth_state = Some(auth_state);
            return Ok(());
        }
        connection.queues.push(RadrootsSimplexAgentQueueRecord {
            descriptor,
            role,
            subscribed: false,
            primary,
            tested: false,
            auth_state: Some(auth_state),
        });
        Ok(())
    }

    pub fn generate_queue_auth_state(
        &self,
    ) -> Result<RadrootsSimplexAgentQueueAuthState, RadrootsSimplexAgentStoreError> {
        let keypair = RadrootsSimplexSmpEd25519Keypair::generate().map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to generate SimpleX queue auth keypair: {error}"
            ))
        })?;
        Ok(RadrootsSimplexAgentQueueAuthState {
            public_key: keypair.public_key,
            private_key: keypair.private_key,
        })
    }

    pub fn queue_record(
        &self,
        connection_id: &str,
        queue_address: &RadrootsSimplexAgentQueueAddress,
    ) -> Result<RadrootsSimplexAgentQueueRecord, RadrootsSimplexAgentStoreError> {
        let connection = self.connection(connection_id)?;
        connection
            .queues
            .iter()
            .find(|queue| &queue.descriptor.queue_address() == queue_address)
            .cloned()
            .ok_or_else(|| RadrootsSimplexAgentStoreError::QueueNotFound(connection_id.into()))
    }

    pub fn mark_queue_subscribed(
        &mut self,
        connection_id: &str,
        queue_address: &RadrootsSimplexAgentQueueAddress,
    ) -> Result<(), RadrootsSimplexAgentStoreError> {
        let connection = self.connection_mut(connection_id)?;
        let Some(queue) = connection
            .queues
            .iter_mut()
            .find(|queue| &queue.descriptor.queue_address() == queue_address)
        else {
            return Err(RadrootsSimplexAgentStoreError::QueueNotFound(
                connection_id.into(),
            ));
        };
        queue.subscribed = true;
        Ok(())
    }

    pub fn mark_queue_tested(
        &mut self,
        connection_id: &str,
        queue_address: &RadrootsSimplexAgentQueueAddress,
    ) -> Result<(), RadrootsSimplexAgentStoreError> {
        let connection = self.connection_mut(connection_id)?;
        let Some(queue) = connection
            .queues
            .iter_mut()
            .find(|queue| &queue.descriptor.queue_address() == queue_address)
        else {
            return Err(RadrootsSimplexAgentStoreError::QueueNotFound(
                connection_id.into(),
            ));
        };
        queue.tested = true;
        Ok(())
    }

    pub fn primary_send_queue(
        &self,
        connection_id: &str,
    ) -> Result<RadrootsSimplexAgentQueueRecord, RadrootsSimplexAgentStoreError> {
        let connection = self.connection(connection_id)?;
        connection
            .queues
            .iter()
            .find(|queue| queue.role == RadrootsSimplexAgentQueueRole::Send && queue.primary)
            .cloned()
            .ok_or_else(|| {
                RadrootsSimplexAgentStoreError::MissingPrimarySendQueue(connection_id.into())
            })
    }

    pub fn receive_queues(
        &self,
        connection_id: &str,
    ) -> Result<Vec<RadrootsSimplexAgentQueueRecord>, RadrootsSimplexAgentStoreError> {
        let connection = self.connection(connection_id)?;
        Ok(connection
            .queues
            .iter()
            .filter(|queue| queue.role == RadrootsSimplexAgentQueueRole::Receive)
            .cloned()
            .collect())
    }

    pub fn queue_auth_state(
        &self,
        connection_id: &str,
        queue_address: &RadrootsSimplexAgentQueueAddress,
    ) -> Result<RadrootsSimplexAgentQueueAuthState, RadrootsSimplexAgentStoreError> {
        self.queue_record(connection_id, queue_address)?
            .auth_state
            .ok_or_else(|| {
                RadrootsSimplexAgentStoreError::QueueAuthStateMissing(connection_id.into())
            })
    }

    pub fn prepare_outbound_message(
        &mut self,
        connection_id: &str,
        message_hash: Vec<u8>,
    ) -> Result<RadrootsSimplexAgentPreparedOutboundMessage, RadrootsSimplexAgentStoreError> {
        let connection = self.connection_mut(connection_id)?;
        if connection.staged_outbound_message.is_some() {
            return Err(RadrootsSimplexAgentStoreError::PendingOutboundMessage(
                connection_id.into(),
            ));
        }
        let prepared = RadrootsSimplexAgentPreparedOutboundMessage {
            message_id: connection
                .delivery_cursor
                .last_sent_message_id
                .unwrap_or(0)
                .saturating_add(1),
            previous_message_hash: connection
                .delivery_cursor
                .last_sent_message_hash
                .clone()
                .unwrap_or_default(),
            message_hash: message_hash.clone(),
        };
        connection.staged_outbound_message = Some(RadrootsSimplexAgentOutboundMessage {
            message_id: prepared.message_id,
            message_hash,
        });
        Ok(prepared)
    }

    pub fn confirm_outbound_message(
        &mut self,
        connection_id: &str,
        message_id: RadrootsSimplexAgentMessageId,
    ) -> Result<RadrootsSimplexAgentOutboundMessage, RadrootsSimplexAgentStoreError> {
        let connection = self.connection_mut(connection_id)?;
        let staged = connection.staged_outbound_message.take().ok_or_else(|| {
            RadrootsSimplexAgentStoreError::StagedOutboundMessageMissing(connection_id.into())
        })?;
        if staged.message_id != message_id {
            connection.staged_outbound_message = Some(staged.clone());
            return Err(
                RadrootsSimplexAgentStoreError::StagedOutboundMessageMismatch {
                    connection_id: connection_id.into(),
                    expected: staged.message_id,
                    actual: message_id,
                },
            );
        }
        connection.delivery_cursor.last_sent_message_id = Some(staged.message_id);
        connection.delivery_cursor.last_sent_message_hash = Some(staged.message_hash.clone());
        connection
            .recent_messages
            .push(RadrootsSimplexAgentRecentMessageRecord {
                message_id: staged.message_id,
                message_hash: staged.message_hash.clone(),
            });
        Ok(staged)
    }

    pub fn clear_staged_outbound_message(
        &mut self,
        connection_id: &str,
        message_id: RadrootsSimplexAgentMessageId,
    ) -> Result<RadrootsSimplexAgentOutboundMessage, RadrootsSimplexAgentStoreError> {
        let connection = self.connection_mut(connection_id)?;
        let staged = connection.staged_outbound_message.take().ok_or_else(|| {
            RadrootsSimplexAgentStoreError::StagedOutboundMessageMissing(connection_id.into())
        })?;
        if staged.message_id != message_id {
            connection.staged_outbound_message = Some(staged.clone());
            return Err(
                RadrootsSimplexAgentStoreError::StagedOutboundMessageMismatch {
                    connection_id: connection_id.into(),
                    expected: staged.message_id,
                    actual: message_id,
                },
            );
        }
        Ok(staged)
    }

    pub fn record_inbound_message(
        &mut self,
        connection_id: &str,
        message_id: RadrootsSimplexAgentMessageId,
        message_hash: Vec<u8>,
    ) -> Result<(), RadrootsSimplexAgentStoreError> {
        let connection = self.connection_mut(connection_id)?;
        connection.delivery_cursor.last_received_message_id = Some(message_id);
        connection.delivery_cursor.last_received_message_hash = Some(message_hash.clone());
        connection
            .recent_messages
            .push(RadrootsSimplexAgentRecentMessageRecord {
                message_id,
                message_hash,
            });
        Ok(())
    }

    pub fn enqueue_command(
        &mut self,
        connection_id: &str,
        kind: RadrootsSimplexAgentPendingCommandKind,
        ready_at: u64,
    ) -> Result<RadrootsSimplexAgentPendingCommand, RadrootsSimplexAgentStoreError> {
        let _ = self.connection(connection_id)?;
        self.next_command_sequence = self.next_command_sequence.saturating_add(1);
        let command = RadrootsSimplexAgentPendingCommand {
            id: self.next_command_sequence,
            connection_id: connection_id.into(),
            kind,
            attempts: 0,
            ready_at,
            inflight: false,
        };
        self.pending_commands.insert(command.id, command.clone());
        Ok(command)
    }

    pub fn take_ready_commands(
        &mut self,
        now: u64,
        limit: usize,
    ) -> Vec<RadrootsSimplexAgentPendingCommand> {
        let ready_ids = self
            .pending_commands
            .iter()
            .filter(|(_, command)| !command.inflight && command.ready_at <= now)
            .map(|(id, _)| *id)
            .take(limit)
            .collect::<Vec<_>>();

        ready_ids
            .into_iter()
            .filter_map(|id| {
                let command = self.pending_commands.get_mut(&id)?;
                command.inflight = true;
                command.attempts = command.attempts.saturating_add(1);
                Some(command.clone())
            })
            .collect()
    }

    pub fn mark_command_delivered(
        &mut self,
        command_id: u64,
    ) -> Result<RadrootsSimplexAgentPendingCommand, RadrootsSimplexAgentStoreError> {
        self.pending_commands
            .remove(&command_id)
            .ok_or(RadrootsSimplexAgentStoreError::CommandNotFound(command_id))
    }

    pub fn mark_command_retry(
        &mut self,
        command_id: u64,
        ready_at: u64,
    ) -> Result<RadrootsSimplexAgentPendingCommand, RadrootsSimplexAgentStoreError> {
        let command = self
            .pending_commands
            .get_mut(&command_id)
            .ok_or(RadrootsSimplexAgentStoreError::CommandNotFound(command_id))?;
        command.inflight = false;
        command.ready_at = ready_at;
        Ok(command.clone())
    }

    pub fn mark_command_failed(
        &mut self,
        command_id: u64,
    ) -> Result<RadrootsSimplexAgentPendingCommand, RadrootsSimplexAgentStoreError> {
        self.pending_commands
            .remove(&command_id)
            .ok_or(RadrootsSimplexAgentStoreError::CommandNotFound(command_id))
    }

    #[cfg(feature = "std")]
    fn snapshot(
        &self,
    ) -> Result<RadrootsSimplexAgentStoreSnapshot, RadrootsSimplexAgentStoreError> {
        let connections = self
            .connections
            .values()
            .cloned()
            .map(connection_to_snapshot)
            .collect::<Result<Vec<_>, _>>()?;
        let pending_commands = self
            .pending_commands
            .values()
            .cloned()
            .map(command_to_snapshot)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RadrootsSimplexAgentStoreSnapshot {
            next_connection_sequence: self.next_connection_sequence,
            next_command_sequence: self.next_command_sequence,
            connections,
            pending_commands,
        })
    }

    #[cfg(feature = "std")]
    fn from_snapshot(
        snapshot: RadrootsSimplexAgentStoreSnapshot,
    ) -> Result<Self, RadrootsSimplexAgentStoreError> {
        let mut connections = BTreeMap::new();
        for connection in snapshot.connections {
            let record = connection_from_snapshot(connection)?;
            connections.insert(record.id.clone(), record);
        }
        let mut pending_commands = BTreeMap::new();
        for command in snapshot.pending_commands {
            let record = command_from_snapshot(command)?;
            pending_commands.insert(record.id, record);
        }
        Ok(Self {
            next_connection_sequence: snapshot.next_connection_sequence,
            next_command_sequence: snapshot.next_command_sequence,
            connections,
            pending_commands,
            persistence_path: None,
        })
    }
}

#[cfg(feature = "std")]
fn connection_to_snapshot(
    record: RadrootsSimplexAgentConnectionRecord,
) -> Result<RadrootsSimplexAgentConnectionSnapshot, RadrootsSimplexAgentStoreError> {
    Ok(RadrootsSimplexAgentConnectionSnapshot {
        id: record.id,
        mode: encode_connection_mode(record.mode).into(),
        status: encode_connection_status(record.status).into(),
        invitation: record
            .invitation
            .as_ref()
            .map(encode_connection_link)
            .transpose()
            .map_err(|error| {
                RadrootsSimplexAgentStoreError::Persistence(format!(
                    "failed to encode SimpleX connection invitation: {error}"
                ))
            })?,
        queues: record
            .queues
            .into_iter()
            .map(queue_record_to_snapshot)
            .collect::<Result<Vec<_>, _>>()?,
        ratchet_state: record.ratchet_state.map(ratchet_state_to_snapshot),
        delivery_cursor: record.delivery_cursor,
        recent_messages: record.recent_messages,
        staged_outbound_message: record.staged_outbound_message,
    })
}

#[cfg(feature = "std")]
fn connection_from_snapshot(
    snapshot: RadrootsSimplexAgentConnectionSnapshot,
) -> Result<RadrootsSimplexAgentConnectionRecord, RadrootsSimplexAgentStoreError> {
    Ok(RadrootsSimplexAgentConnectionRecord {
        id: snapshot.id,
        mode: decode_connection_mode(&snapshot.mode)?,
        status: decode_connection_status(&snapshot.status)?,
        invitation: snapshot
            .invitation
            .as_ref()
            .map(|value| {
                decode_connection_link(value).map_err(|error| {
                    RadrootsSimplexAgentStoreError::Persistence(format!(
                        "failed to decode SimpleX connection invitation: {error}"
                    ))
                })
            })
            .transpose()?,
        queues: snapshot
            .queues
            .into_iter()
            .map(queue_record_from_snapshot)
            .collect::<Result<Vec<_>, _>>()?,
        ratchet_state: snapshot
            .ratchet_state
            .map(ratchet_state_from_snapshot)
            .transpose()?,
        delivery_cursor: snapshot.delivery_cursor,
        recent_messages: snapshot.recent_messages,
        staged_outbound_message: snapshot.staged_outbound_message,
    })
}

#[cfg(feature = "std")]
fn queue_record_to_snapshot(
    record: RadrootsSimplexAgentQueueRecord,
) -> Result<RadrootsSimplexAgentQueueRecordSnapshot, RadrootsSimplexAgentStoreError> {
    Ok(RadrootsSimplexAgentQueueRecordSnapshot {
        descriptor: queue_descriptor_to_snapshot(record.descriptor),
        role: encode_queue_role(record.role).into(),
        subscribed: record.subscribed,
        primary: record.primary,
        tested: record.tested,
        auth_state: record.auth_state,
    })
}

#[cfg(feature = "std")]
fn queue_record_from_snapshot(
    snapshot: RadrootsSimplexAgentQueueRecordSnapshot,
) -> Result<RadrootsSimplexAgentQueueRecord, RadrootsSimplexAgentStoreError> {
    Ok(RadrootsSimplexAgentQueueRecord {
        descriptor: queue_descriptor_from_snapshot(snapshot.descriptor)?,
        role: decode_queue_role(&snapshot.role)?,
        subscribed: snapshot.subscribed,
        primary: snapshot.primary,
        tested: snapshot.tested,
        auth_state: snapshot.auth_state,
    })
}

#[cfg(feature = "std")]
fn queue_descriptor_to_snapshot(
    descriptor: RadrootsSimplexAgentQueueDescriptor,
) -> RadrootsSimplexAgentQueueDescriptorSnapshot {
    RadrootsSimplexAgentQueueDescriptorSnapshot {
        queue_uri: descriptor.queue_uri.to_string(),
        replaced_queue: descriptor.replaced_queue.map(queue_address_to_snapshot),
        primary: descriptor.primary,
        sender_key: descriptor.sender_key,
    }
}

#[cfg(feature = "std")]
fn queue_descriptor_from_snapshot(
    snapshot: RadrootsSimplexAgentQueueDescriptorSnapshot,
) -> Result<RadrootsSimplexAgentQueueDescriptor, RadrootsSimplexAgentStoreError> {
    let queue_uri = RadrootsSimplexSmpQueueUri::parse(&snapshot.queue_uri).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to parse SimpleX queue uri `{}`: {error}",
            snapshot.queue_uri
        ))
    })?;
    Ok(RadrootsSimplexAgentQueueDescriptor {
        queue_uri,
        replaced_queue: snapshot
            .replaced_queue
            .map(queue_address_from_snapshot)
            .transpose()?,
        primary: snapshot.primary,
        sender_key: snapshot.sender_key,
    })
}

#[cfg(feature = "std")]
fn queue_address_to_snapshot(
    address: RadrootsSimplexAgentQueueAddress,
) -> RadrootsSimplexAgentQueueAddressSnapshot {
    RadrootsSimplexAgentQueueAddressSnapshot {
        server_identity: address.server.server_identity,
        hosts: address.server.hosts,
        port: address.server.port,
        sender_id: address.sender_id,
    }
}

#[cfg(feature = "std")]
fn queue_address_from_snapshot(
    snapshot: RadrootsSimplexAgentQueueAddressSnapshot,
) -> Result<RadrootsSimplexAgentQueueAddress, RadrootsSimplexAgentStoreError> {
    if snapshot.server_identity.is_empty() || snapshot.hosts.is_empty() {
        return Err(RadrootsSimplexAgentStoreError::Persistence(
            "invalid SimpleX queue address snapshot".into(),
        ));
    }
    Ok(RadrootsSimplexAgentQueueAddress {
        server: RadrootsSimplexSmpServerAddress {
            server_identity: snapshot.server_identity,
            hosts: snapshot.hosts,
            port: snapshot.port,
        },
        sender_id: snapshot.sender_id,
    })
}

#[cfg(feature = "std")]
fn ratchet_state_to_snapshot(
    state: RadrootsSimplexSmpRatchetState,
) -> RadrootsSimplexAgentRatchetStateSnapshot {
    RadrootsSimplexAgentRatchetStateSnapshot {
        role: alloc::format!("{:?}", state.role).to_ascii_lowercase(),
        root_epoch: state.root_epoch,
        previous_sending_chain_length: state.previous_sending_chain_length,
        sending_chain_length: state.sending_chain_length,
        receiving_chain_length: state.receiving_chain_length,
        local_dh_public_key: state.local_dh_public_key,
        remote_dh_public_key: state.remote_dh_public_key,
        current_pq_public_key: state.current_pq_public_key,
        remote_pq_public_key: state.remote_pq_public_key,
        pending_outbound_pq_ciphertext: state.pending_outbound_pq_ciphertext,
        pending_inbound_pq_ciphertext: state.pending_inbound_pq_ciphertext,
        current_pq_shared_secret: state.current_pq_shared_secret,
    }
}

#[cfg(feature = "std")]
fn ratchet_state_from_snapshot(
    snapshot: RadrootsSimplexAgentRatchetStateSnapshot,
) -> Result<RadrootsSimplexSmpRatchetState, RadrootsSimplexAgentStoreError> {
    let mut state = match snapshot.role.as_str() {
        "initiator" => RadrootsSimplexSmpRatchetState::initiator(
            snapshot.local_dh_public_key.clone(),
            snapshot.remote_dh_public_key.clone(),
            snapshot.remote_pq_public_key.clone(),
        )
        .map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to restore initiator ratchet state: {error}"
            ))
        })?,
        "responder" => RadrootsSimplexSmpRatchetState::responder(
            snapshot.local_dh_public_key.clone(),
            snapshot.remote_dh_public_key.clone(),
            snapshot.current_pq_public_key.clone(),
        )
        .map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to restore responder ratchet state: {error}"
            ))
        })?,
        other => {
            return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
                "invalid SimpleX ratchet role `{other}`"
            )));
        }
    };
    state.root_epoch = snapshot.root_epoch;
    state.previous_sending_chain_length = snapshot.previous_sending_chain_length;
    state.sending_chain_length = snapshot.sending_chain_length;
    state.receiving_chain_length = snapshot.receiving_chain_length;
    state.current_pq_public_key = snapshot.current_pq_public_key;
    state.remote_pq_public_key = snapshot.remote_pq_public_key;
    state.pending_outbound_pq_ciphertext = snapshot.pending_outbound_pq_ciphertext;
    state.pending_inbound_pq_ciphertext = snapshot.pending_inbound_pq_ciphertext;
    state.current_pq_shared_secret = snapshot.current_pq_shared_secret;
    Ok(state)
}

#[cfg(feature = "std")]
fn command_to_snapshot(
    command: RadrootsSimplexAgentPendingCommand,
) -> Result<RadrootsSimplexAgentPendingCommandSnapshot, RadrootsSimplexAgentStoreError> {
    Ok(RadrootsSimplexAgentPendingCommandSnapshot {
        id: command.id,
        connection_id: command.connection_id,
        kind: command_kind_to_snapshot(command.kind)?,
        attempts: command.attempts,
        ready_at: command.ready_at,
        inflight: command.inflight,
    })
}

#[cfg(feature = "std")]
fn command_from_snapshot(
    snapshot: RadrootsSimplexAgentPendingCommandSnapshot,
) -> Result<RadrootsSimplexAgentPendingCommand, RadrootsSimplexAgentStoreError> {
    Ok(RadrootsSimplexAgentPendingCommand {
        id: snapshot.id,
        connection_id: snapshot.connection_id,
        kind: command_kind_from_snapshot(snapshot.kind)?,
        attempts: snapshot.attempts,
        ready_at: snapshot.ready_at,
        inflight: snapshot.inflight,
    })
}

#[cfg(feature = "std")]
fn command_kind_to_snapshot(
    kind: RadrootsSimplexAgentPendingCommandKind,
) -> Result<RadrootsSimplexAgentPendingCommandKindSnapshot, RadrootsSimplexAgentStoreError> {
    Ok(match kind {
        RadrootsSimplexAgentPendingCommandKind::CreateQueue { descriptor } => {
            RadrootsSimplexAgentPendingCommandKindSnapshot::CreateQueue {
                descriptor: queue_descriptor_to_snapshot(descriptor),
            }
        }
        RadrootsSimplexAgentPendingCommandKind::SecureQueue { queue, sender_key } => {
            RadrootsSimplexAgentPendingCommandKindSnapshot::SecureQueue {
                queue: queue_address_to_snapshot(queue),
                sender_key,
            }
        }
        RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
            queue,
            envelope,
            delivery,
        } => RadrootsSimplexAgentPendingCommandKindSnapshot::SendEnvelope {
            queue: queue_address_to_snapshot(queue),
            envelope: encode_envelope(&envelope).map_err(|error| {
                RadrootsSimplexAgentStoreError::Persistence(format!(
                    "failed to encode SimpleX envelope: {error}"
                ))
            })?,
            delivery,
        },
        RadrootsSimplexAgentPendingCommandKind::SubscribeQueue { queue } => {
            RadrootsSimplexAgentPendingCommandKindSnapshot::SubscribeQueue {
                queue: queue_address_to_snapshot(queue),
            }
        }
        RadrootsSimplexAgentPendingCommandKind::AckInboxMessage { queue, receipt } => {
            RadrootsSimplexAgentPendingCommandKindSnapshot::AckInboxMessage {
                queue: queue_address_to_snapshot(queue),
                receipt: RadrootsSimplexAgentMessageReceiptSnapshot {
                    message_id: receipt.message_id,
                    message_hash: receipt.message_hash,
                    receipt_info: receipt.receipt_info,
                },
            }
        }
        RadrootsSimplexAgentPendingCommandKind::RotateQueues { descriptors } => {
            RadrootsSimplexAgentPendingCommandKindSnapshot::RotateQueues {
                descriptors: descriptors
                    .into_iter()
                    .map(queue_descriptor_to_snapshot)
                    .collect(),
            }
        }
        RadrootsSimplexAgentPendingCommandKind::TestQueues { queues } => {
            RadrootsSimplexAgentPendingCommandKindSnapshot::TestQueues {
                queues: queues.into_iter().map(queue_address_to_snapshot).collect(),
            }
        }
    })
}

#[cfg(feature = "std")]
fn command_kind_from_snapshot(
    snapshot: RadrootsSimplexAgentPendingCommandKindSnapshot,
) -> Result<RadrootsSimplexAgentPendingCommandKind, RadrootsSimplexAgentStoreError> {
    Ok(match snapshot {
        RadrootsSimplexAgentPendingCommandKindSnapshot::CreateQueue { descriptor } => {
            RadrootsSimplexAgentPendingCommandKind::CreateQueue {
                descriptor: queue_descriptor_from_snapshot(descriptor)?,
            }
        }
        RadrootsSimplexAgentPendingCommandKindSnapshot::SecureQueue { queue, sender_key } => {
            RadrootsSimplexAgentPendingCommandKind::SecureQueue {
                queue: queue_address_from_snapshot(queue)?,
                sender_key,
            }
        }
        RadrootsSimplexAgentPendingCommandKindSnapshot::SendEnvelope {
            queue,
            envelope,
            delivery,
        } => RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
            queue: queue_address_from_snapshot(queue)?,
            envelope: decode_envelope(&envelope).map_err(|error| {
                RadrootsSimplexAgentStoreError::Persistence(format!(
                    "failed to decode SimpleX envelope: {error}"
                ))
            })?,
            delivery,
        },
        RadrootsSimplexAgentPendingCommandKindSnapshot::SubscribeQueue { queue } => {
            RadrootsSimplexAgentPendingCommandKind::SubscribeQueue {
                queue: queue_address_from_snapshot(queue)?,
            }
        }
        RadrootsSimplexAgentPendingCommandKindSnapshot::AckInboxMessage { queue, receipt } => {
            RadrootsSimplexAgentPendingCommandKind::AckInboxMessage {
                queue: queue_address_from_snapshot(queue)?,
                receipt: RadrootsSimplexAgentMessageReceipt {
                    message_id: receipt.message_id,
                    message_hash: receipt.message_hash,
                    receipt_info: receipt.receipt_info,
                },
            }
        }
        RadrootsSimplexAgentPendingCommandKindSnapshot::RotateQueues { descriptors } => {
            RadrootsSimplexAgentPendingCommandKind::RotateQueues {
                descriptors: descriptors
                    .into_iter()
                    .map(queue_descriptor_from_snapshot)
                    .collect::<Result<Vec<_>, _>>()?,
            }
        }
        RadrootsSimplexAgentPendingCommandKindSnapshot::TestQueues { queues } => {
            RadrootsSimplexAgentPendingCommandKind::TestQueues {
                queues: queues
                    .into_iter()
                    .map(queue_address_from_snapshot)
                    .collect::<Result<Vec<_>, _>>()?,
            }
        }
    })
}

#[cfg(feature = "std")]
fn encode_connection_mode(mode: RadrootsSimplexAgentConnectionMode) -> &'static str {
    match mode {
        RadrootsSimplexAgentConnectionMode::Direct => "direct",
        RadrootsSimplexAgentConnectionMode::ContactAddress => "contact_address",
    }
}

#[cfg(feature = "std")]
fn decode_connection_mode(
    value: &str,
) -> Result<RadrootsSimplexAgentConnectionMode, RadrootsSimplexAgentStoreError> {
    match value {
        "direct" => Ok(RadrootsSimplexAgentConnectionMode::Direct),
        "contact_address" => Ok(RadrootsSimplexAgentConnectionMode::ContactAddress),
        other => Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "invalid SimpleX connection mode `{other}`"
        ))),
    }
}

#[cfg(feature = "std")]
fn encode_connection_status(status: RadrootsSimplexAgentConnectionStatus) -> &'static str {
    match status {
        RadrootsSimplexAgentConnectionStatus::CreatePending => "create_pending",
        RadrootsSimplexAgentConnectionStatus::InvitationReady => "invitation_ready",
        RadrootsSimplexAgentConnectionStatus::JoinPending => "join_pending",
        RadrootsSimplexAgentConnectionStatus::AwaitingApproval => "awaiting_approval",
        RadrootsSimplexAgentConnectionStatus::Allowed => "allowed",
        RadrootsSimplexAgentConnectionStatus::Connected => "connected",
        RadrootsSimplexAgentConnectionStatus::Suspended => "suspended",
        RadrootsSimplexAgentConnectionStatus::Rotating => "rotating",
        RadrootsSimplexAgentConnectionStatus::Deleted => "deleted",
    }
}

#[cfg(feature = "std")]
fn decode_connection_status(
    value: &str,
) -> Result<RadrootsSimplexAgentConnectionStatus, RadrootsSimplexAgentStoreError> {
    match value {
        "create_pending" => Ok(RadrootsSimplexAgentConnectionStatus::CreatePending),
        "invitation_ready" => Ok(RadrootsSimplexAgentConnectionStatus::InvitationReady),
        "join_pending" => Ok(RadrootsSimplexAgentConnectionStatus::JoinPending),
        "awaiting_approval" => Ok(RadrootsSimplexAgentConnectionStatus::AwaitingApproval),
        "allowed" => Ok(RadrootsSimplexAgentConnectionStatus::Allowed),
        "connected" => Ok(RadrootsSimplexAgentConnectionStatus::Connected),
        "suspended" => Ok(RadrootsSimplexAgentConnectionStatus::Suspended),
        "rotating" => Ok(RadrootsSimplexAgentConnectionStatus::Rotating),
        "deleted" => Ok(RadrootsSimplexAgentConnectionStatus::Deleted),
        other => Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "invalid SimpleX connection status `{other}`"
        ))),
    }
}

#[cfg(feature = "std")]
fn encode_queue_role(role: RadrootsSimplexAgentQueueRole) -> &'static str {
    match role {
        RadrootsSimplexAgentQueueRole::Receive => "receive",
        RadrootsSimplexAgentQueueRole::Send => "send",
    }
}

#[cfg(feature = "std")]
fn decode_queue_role(
    value: &str,
) -> Result<RadrootsSimplexAgentQueueRole, RadrootsSimplexAgentStoreError> {
    match value {
        "receive" => Ok(RadrootsSimplexAgentQueueRole::Receive),
        "send" => Ok(RadrootsSimplexAgentQueueRole::Send),
        other => Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "invalid SimpleX queue role `{other}`"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpQueueUri;

    fn sample_descriptor(primary: bool) -> RadrootsSimplexAgentQueueDescriptor {
        RadrootsSimplexAgentQueueDescriptor {
            queue_uri: RadrootsSimplexSmpQueueUri::parse(
                "smp://aGVsbG8@relay.example/cXVldWU#/?v=4&dh=Zm9vYmFy&q=m",
            )
            .unwrap(),
            replaced_queue: None,
            primary,
            sender_key: Some(b"sender-auth".to_vec()),
        }
    }

    fn sample_auth_state() -> RadrootsSimplexAgentQueueAuthState {
        RadrootsSimplexAgentQueueAuthState {
            public_key: vec![7_u8; 32],
            private_key: vec![9_u8; 32],
        }
    }

    #[test]
    fn stores_connections_queues_and_retryable_commands() {
        let mut store = RadrootsSimplexAgentStore::new();
        let connection = store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::CreatePending,
            None,
            None,
        );
        store
            .add_queue(
                &connection.id,
                sample_descriptor(true),
                RadrootsSimplexAgentQueueRole::Send,
                true,
                sample_auth_state(),
            )
            .unwrap();
        let command = store
            .enqueue_command(
                &connection.id,
                RadrootsSimplexAgentPendingCommandKind::SubscribeQueue {
                    queue: sample_descriptor(true).queue_address(),
                },
                10,
            )
            .unwrap();
        let ready = store.take_ready_commands(10, 10);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, command.id);
        let retried = store.mark_command_retry(command.id, 20).unwrap();
        assert_eq!(retried.ready_at, 20);
        let queue = store.primary_send_queue(&connection.id).unwrap();
        assert_eq!(queue.descriptor, sample_descriptor(true));
        assert!(queue.auth_state.is_some());
    }

    #[test]
    fn stages_and_confirms_outbound_message_without_consuming_cursor_early() {
        let mut store = RadrootsSimplexAgentStore::new();
        let connection = store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::Connected,
            None,
            None,
        );

        let prepared = store
            .prepare_outbound_message(&connection.id, b"ciphertext".to_vec())
            .unwrap();
        assert_eq!(prepared.message_id, 1);
        assert!(prepared.previous_message_hash.is_empty());
        assert_eq!(
            store
                .connection(&connection.id)
                .unwrap()
                .delivery_cursor
                .last_sent_message_id,
            None
        );

        let error = store
            .prepare_outbound_message(&connection.id, b"next".to_vec())
            .unwrap_err();
        assert_eq!(
            error,
            RadrootsSimplexAgentStoreError::PendingOutboundMessage(connection.id.clone())
        );

        store
            .confirm_outbound_message(&connection.id, prepared.message_id)
            .unwrap();
        let cursor = &store.connection(&connection.id).unwrap().delivery_cursor;
        assert_eq!(cursor.last_sent_message_id, Some(1));
        assert_eq!(cursor.last_sent_message_hash, Some(b"ciphertext".to_vec()));
    }

    #[cfg(feature = "std")]
    #[test]
    fn flush_and_reopen_persisted_store_state() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");

        let mut store = RadrootsSimplexAgentStore::open(&path).unwrap();
        let connection = store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::Connected,
            None,
            None,
        );
        store
            .add_queue(
                &connection.id,
                sample_descriptor(true),
                RadrootsSimplexAgentQueueRole::Send,
                true,
                sample_auth_state(),
            )
            .unwrap();
        let prepared = store
            .prepare_outbound_message(&connection.id, b"persisted".to_vec())
            .unwrap();
        store
            .enqueue_command(
                &connection.id,
                RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                    queue: sample_descriptor(true).queue_address(),
                    envelope: RadrootsSimplexAgentEnvelope::Invitation {
                        request: b"req".to_vec(),
                        connection_info: b"info".to_vec(),
                    },
                    delivery: Some(RadrootsSimplexAgentOutboundMessage {
                        message_id: prepared.message_id,
                        message_hash: prepared.message_hash.clone(),
                    }),
                },
                10,
            )
            .unwrap();
        store.flush().unwrap();

        let loaded = RadrootsSimplexAgentStore::open(&path).unwrap();
        let loaded_connection = loaded.connection(&connection.id).unwrap();
        assert_eq!(
            loaded_connection.staged_outbound_message,
            Some(RadrootsSimplexAgentOutboundMessage {
                message_id: 1,
                message_hash: b"persisted".to_vec(),
            })
        );
        assert_eq!(loaded.pending_commands.len(), 1);
        assert!(
            loaded
                .primary_send_queue(&connection.id)
                .unwrap()
                .auth_state
                .is_some()
        );
    }
}

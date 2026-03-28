use crate::error::RadrootsSimplexAgentStoreError;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use radroots_simplex_agent_proto::prelude::{
    RadrootsSimplexAgentConnectionLink, RadrootsSimplexAgentConnectionMode,
    RadrootsSimplexAgentConnectionStatus, RadrootsSimplexAgentEnvelope,
    RadrootsSimplexAgentMessageId, RadrootsSimplexAgentMessageReceipt,
    RadrootsSimplexAgentQueueAddress, RadrootsSimplexAgentQueueDescriptor,
    RadrootsSimplexSmpRatchetState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsSimplexAgentQueueRole {
    Receive,
    Send,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentQueueRecord {
    pub descriptor: RadrootsSimplexAgentQueueDescriptor,
    pub role: RadrootsSimplexAgentQueueRole,
    pub subscribed: bool,
    pub primary: bool,
    pub tested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentDeliveryCursor {
    pub last_sent_message_id: Option<RadrootsSimplexAgentMessageId>,
    pub last_received_message_id: Option<RadrootsSimplexAgentMessageId>,
    pub last_sent_message_hash: Option<Vec<u8>>,
    pub last_received_message_hash: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentRecentMessageRecord {
    pub message_id: RadrootsSimplexAgentMessageId,
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
}

#[derive(Debug, Clone, Default)]
pub struct RadrootsSimplexAgentStore {
    next_connection_sequence: u64,
    next_command_sequence: u64,
    connections: BTreeMap<String, RadrootsSimplexAgentConnectionRecord>,
    pending_commands: BTreeMap<u64, RadrootsSimplexAgentPendingCommand>,
}

impl RadrootsSimplexAgentStore {
    pub fn new() -> Self {
        Self::default()
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
            return Ok(());
        }
        connection.queues.push(RadrootsSimplexAgentQueueRecord {
            descriptor,
            role,
            subscribed: false,
            primary,
            tested: false,
        });
        Ok(())
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

    pub fn record_outbound_message(
        &mut self,
        connection_id: &str,
        message_id: RadrootsSimplexAgentMessageId,
        message_hash: Vec<u8>,
    ) -> Result<(), RadrootsSimplexAgentStoreError> {
        let connection = self.connection_mut(connection_id)?;
        connection.delivery_cursor.last_sent_message_id = Some(message_id);
        connection.delivery_cursor.last_sent_message_hash = Some(message_hash.clone());
        connection
            .recent_messages
            .push(RadrootsSimplexAgentRecentMessageRecord {
                message_id,
                message_hash,
            });
        Ok(())
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
            sender_key: None,
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
        assert_eq!(
            store.primary_send_queue(&connection.id).unwrap().descriptor,
            sample_descriptor(true)
        );
    }
}

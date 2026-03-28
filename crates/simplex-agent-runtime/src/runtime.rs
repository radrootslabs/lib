use crate::error::RadrootsSimplexAgentRuntimeError;
use crate::types::{RadrootsSimplexAgentCommandOutcome, RadrootsSimplexAgentRuntimeEvent};
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use radroots_simplex_agent_proto::prelude::{
    RadrootsSimplexAgentConnectionLink, RadrootsSimplexAgentConnectionMode,
    RadrootsSimplexAgentConnectionStatus, RadrootsSimplexAgentDecryptedMessage,
    RadrootsSimplexAgentEncryptedPayload, RadrootsSimplexAgentEnvelope,
    RadrootsSimplexAgentMessage, RadrootsSimplexAgentMessageFrame,
    RadrootsSimplexAgentMessageHeader, RadrootsSimplexAgentMessageReceipt,
    RadrootsSimplexAgentQueueDescriptor, RadrootsSimplexSmpRatchetState, encode_decrypted_message,
};
use radroots_simplex_agent_store::prelude::{
    RadrootsSimplexAgentPendingCommand, RadrootsSimplexAgentPendingCommandKind,
    RadrootsSimplexAgentQueueRole, RadrootsSimplexAgentStore,
};
use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpQueueUri;
use sha2::{Digest, Sha256};

pub struct RadrootsSimplexAgentRuntimeBuilder {
    store: Option<RadrootsSimplexAgentStore>,
    queue_capacity: usize,
    retry_delay_ms: u64,
}

impl RadrootsSimplexAgentRuntimeBuilder {
    pub const DEFAULT_QUEUE_CAPACITY: usize = 2_048;
    pub const DEFAULT_RETRY_DELAY_MS: u64 = 5_000;

    pub fn new() -> Self {
        Self {
            store: None,
            queue_capacity: Self::DEFAULT_QUEUE_CAPACITY,
            retry_delay_ms: Self::DEFAULT_RETRY_DELAY_MS,
        }
    }

    pub fn store(mut self, store: RadrootsSimplexAgentStore) -> Self {
        self.store = Some(store);
        self
    }

    pub fn queue_capacity(mut self, queue_capacity: usize) -> Self {
        self.queue_capacity = queue_capacity;
        self
    }

    pub fn retry_delay_ms(mut self, retry_delay_ms: u64) -> Self {
        self.retry_delay_ms = retry_delay_ms;
        self
    }

    pub fn build(self) -> Result<RadrootsSimplexAgentRuntime, RadrootsSimplexAgentRuntimeError> {
        if self.queue_capacity == 0 {
            return Err(RadrootsSimplexAgentRuntimeError::InvalidConfig(
                "queue_capacity",
            ));
        }
        Ok(RadrootsSimplexAgentRuntime {
            store: self.store.unwrap_or_default(),
            events: VecDeque::with_capacity(self.queue_capacity),
            retry_delay_ms: self.retry_delay_ms,
        })
    }
}

impl Default for RadrootsSimplexAgentRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RadrootsSimplexAgentRuntime {
    store: RadrootsSimplexAgentStore,
    events: VecDeque<RadrootsSimplexAgentRuntimeEvent>,
    retry_delay_ms: u64,
}

impl RadrootsSimplexAgentRuntime {
    pub fn create_connection(
        &mut self,
        invitation_queue: RadrootsSimplexSmpQueueUri,
        e2e_public_key: Vec<u8>,
        contact_address: bool,
        now: u64,
    ) -> Result<String, RadrootsSimplexAgentRuntimeError> {
        let ratchet_state = RadrootsSimplexSmpRatchetState::initiator(
            b"local-dh".to_vec(),
            invitation_queue.recipient_dh_public_key.as_bytes().to_vec(),
            None,
        )
        .ok();
        let connection = self.store.create_connection(
            if contact_address {
                RadrootsSimplexAgentConnectionMode::ContactAddress
            } else {
                RadrootsSimplexAgentConnectionMode::Direct
            },
            RadrootsSimplexAgentConnectionStatus::InvitationReady,
            None,
            ratchet_state,
        );
        let invitation = RadrootsSimplexAgentConnectionLink {
            invitation_queue: invitation_queue.clone(),
            connection_id: connection.id.as_bytes().to_vec(),
            e2e_public_key,
            contact_address,
        };
        self.store.connection_mut(&connection.id)?.invitation = Some(invitation.clone());
        let descriptor = RadrootsSimplexAgentQueueDescriptor {
            queue_uri: invitation_queue,
            replaced_queue: None,
            primary: true,
            sender_key: None,
        };
        self.store.add_queue(
            &connection.id,
            descriptor.clone(),
            RadrootsSimplexAgentQueueRole::Receive,
            true,
        )?;
        self.store.enqueue_command(
            &connection.id,
            RadrootsSimplexAgentPendingCommandKind::CreateQueue {
                descriptor: descriptor.clone(),
            },
            now,
        )?;
        self.store.enqueue_command(
            &connection.id,
            RadrootsSimplexAgentPendingCommandKind::SubscribeQueue {
                queue: descriptor.queue_address(),
            },
            now,
        )?;
        self.events
            .push_back(RadrootsSimplexAgentRuntimeEvent::InvitationReady {
                connection_id: connection.id.clone(),
                invitation,
            });
        Ok(connection.id)
    }

    pub fn join_connection(
        &mut self,
        invitation: RadrootsSimplexAgentConnectionLink,
        reply_queue: RadrootsSimplexSmpQueueUri,
        now: u64,
    ) -> Result<String, RadrootsSimplexAgentRuntimeError> {
        let ratchet_state = RadrootsSimplexSmpRatchetState::responder(
            b"reply-dh".to_vec(),
            invitation
                .invitation_queue
                .recipient_dh_public_key
                .as_bytes()
                .to_vec(),
            None,
        )
        .ok();
        let connection = self.store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::JoinPending,
            Some(invitation.clone()),
            ratchet_state,
        );
        let send_descriptor = RadrootsSimplexAgentQueueDescriptor {
            queue_uri: invitation.invitation_queue.clone(),
            replaced_queue: None,
            primary: true,
            sender_key: Some(b"sender-auth".to_vec()),
        };
        let receive_descriptor = RadrootsSimplexAgentQueueDescriptor {
            queue_uri: reply_queue,
            replaced_queue: None,
            primary: true,
            sender_key: None,
        };
        self.store.add_queue(
            &connection.id,
            send_descriptor.clone(),
            RadrootsSimplexAgentQueueRole::Send,
            true,
        )?;
        self.store.add_queue(
            &connection.id,
            receive_descriptor.clone(),
            RadrootsSimplexAgentQueueRole::Receive,
            true,
        )?;
        self.store.enqueue_command(
            &connection.id,
            RadrootsSimplexAgentPendingCommandKind::SecureQueue {
                queue: send_descriptor.queue_address(),
                sender_key: send_descriptor.sender_key.clone(),
            },
            now,
        )?;
        self.store.enqueue_command(
            &connection.id,
            RadrootsSimplexAgentPendingCommandKind::CreateQueue {
                descriptor: receive_descriptor.clone(),
            },
            now,
        )?;
        self.store.enqueue_command(
            &connection.id,
            RadrootsSimplexAgentPendingCommandKind::SubscribeQueue {
                queue: receive_descriptor.queue_address(),
            },
            now,
        )?;
        self.store.enqueue_command(
            &connection.id,
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                queue: send_descriptor.queue_address(),
                envelope: RadrootsSimplexAgentEnvelope::Confirmation {
                    reply_queue: true,
                    encrypted: RadrootsSimplexAgentEncryptedPayload {
                        ratchet_header: None,
                        ciphertext: invitation.connection_id,
                    },
                },
            },
            now,
        )?;
        self.events
            .push_back(RadrootsSimplexAgentRuntimeEvent::ConfirmationRequired {
                connection_id: connection.id.clone(),
            });
        Ok(connection.id)
    }

    pub fn allow_connection(
        &mut self,
        connection_id: &str,
        local_info: Vec<u8>,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        self.store
            .set_status(connection_id, RadrootsSimplexAgentConnectionStatus::Allowed)?;
        let send_queue = self.store.primary_send_queue(connection_id)?;
        let encrypted = RadrootsSimplexAgentEncryptedPayload {
            ratchet_header: None,
            ciphertext: local_info,
        };
        self.store.enqueue_command(
            connection_id,
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                queue: send_queue.descriptor.queue_address(),
                envelope: RadrootsSimplexAgentEnvelope::Confirmation {
                    reply_queue: false,
                    encrypted,
                },
            },
            now,
        )?;
        Ok(())
    }

    pub fn subscribe_connection(
        &mut self,
        connection_id: &str,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        for queue in self.store.receive_queues(connection_id)? {
            self.store.enqueue_command(
                connection_id,
                RadrootsSimplexAgentPendingCommandKind::SubscribeQueue {
                    queue: queue.descriptor.queue_address(),
                },
                now,
            )?;
        }
        self.events
            .push_back(RadrootsSimplexAgentRuntimeEvent::SubscriptionQueued {
                connection_id: connection_id.into(),
            });
        Ok(())
    }

    pub fn send_message(
        &mut self,
        connection_id: &str,
        body: Vec<u8>,
        now: u64,
    ) -> Result<u64, RadrootsSimplexAgentRuntimeError> {
        let send_queue = self.store.primary_send_queue(connection_id)?;
        let previous_hash = self
            .store
            .connection(connection_id)?
            .delivery_cursor
            .last_sent_message_hash
            .clone()
            .unwrap_or_default();
        let message_id = self
            .store
            .connection(connection_id)?
            .delivery_cursor
            .last_sent_message_id
            .unwrap_or(0)
            .saturating_add(1);
        let frame = RadrootsSimplexAgentMessageFrame {
            header: RadrootsSimplexAgentMessageHeader {
                message_id,
                previous_message_hash: previous_hash,
            },
            message: RadrootsSimplexAgentMessage::UserMessage(body.clone()),
            padding: Vec::new(),
        };
        let ciphertext =
            encode_decrypted_message(&RadrootsSimplexAgentDecryptedMessage::Message(frame))?;
        let message_hash = Sha256::digest(&ciphertext).to_vec();
        self.store
            .record_outbound_message(connection_id, message_id, message_hash)?;
        self.store.enqueue_command(
            connection_id,
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                queue: send_queue.descriptor.queue_address(),
                envelope: RadrootsSimplexAgentEnvelope::Message(
                    RadrootsSimplexAgentEncryptedPayload {
                        ratchet_header: None,
                        ciphertext,
                    },
                ),
            },
            now,
        )?;
        self.events
            .push_back(RadrootsSimplexAgentRuntimeEvent::MessageQueued {
                connection_id: connection_id.into(),
                message_id,
            });
        Ok(message_id)
    }

    pub fn ack_message(
        &mut self,
        connection_id: &str,
        message_id: u64,
        message_hash: Vec<u8>,
        receipt_info: Vec<u8>,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        let send_queue = self.store.primary_send_queue(connection_id)?;
        self.store.enqueue_command(
            connection_id,
            RadrootsSimplexAgentPendingCommandKind::AckInboxMessage {
                queue: send_queue.descriptor.queue_address(),
                receipt: RadrootsSimplexAgentMessageReceipt {
                    message_id,
                    message_hash,
                    receipt_info,
                },
            },
            now,
        )?;
        Ok(())
    }

    pub fn reconnect_connection(
        &mut self,
        connection_id: &str,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        self.subscribe_connection(connection_id, now)?;
        let ready = self.store.take_ready_commands(now, usize::MAX);
        for command in ready {
            self.store
                .mark_command_retry(command.id, now + self.retry_delay_ms)?;
            self.events
                .push_back(RadrootsSimplexAgentRuntimeEvent::RetryQueued {
                    connection_id: connection_id.into(),
                    command_id: command.id,
                });
        }
        Ok(())
    }

    pub fn queue_rotation(
        &mut self,
        connection_id: &str,
        descriptors: Vec<RadrootsSimplexAgentQueueDescriptor>,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        self.store.set_status(
            connection_id,
            RadrootsSimplexAgentConnectionStatus::Rotating,
        )?;
        self.store.enqueue_command(
            connection_id,
            RadrootsSimplexAgentPendingCommandKind::RotateQueues { descriptors },
            now,
        )?;
        self.events
            .push_back(RadrootsSimplexAgentRuntimeEvent::QueueRotationQueued {
                connection_id: connection_id.into(),
            });
        Ok(())
    }

    pub fn handle_inbound_decrypted_message(
        &mut self,
        connection_id: &str,
        message: RadrootsSimplexAgentDecryptedMessage,
        transport_hash: Vec<u8>,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        match message {
            RadrootsSimplexAgentDecryptedMessage::ConnectionInfo(info) => {
                self.events
                    .push_back(RadrootsSimplexAgentRuntimeEvent::ConnectionInfo {
                        connection_id: connection_id.into(),
                        info,
                    });
            }
            RadrootsSimplexAgentDecryptedMessage::ConnectionInfoReply { reply_queues, info } => {
                for descriptor in reply_queues {
                    self.store.add_queue(
                        connection_id,
                        descriptor,
                        RadrootsSimplexAgentQueueRole::Send,
                        true,
                    )?;
                }
                self.store.set_status(
                    connection_id,
                    RadrootsSimplexAgentConnectionStatus::AwaitingApproval,
                )?;
                self.events
                    .push_back(RadrootsSimplexAgentRuntimeEvent::ConfirmationRequired {
                        connection_id: connection_id.into(),
                    });
                self.events
                    .push_back(RadrootsSimplexAgentRuntimeEvent::ConnectionInfo {
                        connection_id: connection_id.into(),
                        info,
                    });
            }
            RadrootsSimplexAgentDecryptedMessage::RatchetInfo(info) => {
                self.events
                    .push_back(RadrootsSimplexAgentRuntimeEvent::ConnectionInfo {
                        connection_id: connection_id.into(),
                        info,
                    });
            }
            RadrootsSimplexAgentDecryptedMessage::Message(frame) => {
                self.store.record_inbound_message(
                    connection_id,
                    frame.header.message_id,
                    transport_hash,
                )?;
                match frame.message {
                    RadrootsSimplexAgentMessage::Hello => {
                        self.store.set_status(
                            connection_id,
                            RadrootsSimplexAgentConnectionStatus::Connected,
                        )?;
                        self.events.push_back(
                            RadrootsSimplexAgentRuntimeEvent::ConnectionEstablished {
                                connection_id: connection_id.into(),
                            },
                        );
                    }
                    RadrootsSimplexAgentMessage::Receipt(receipt) => {
                        self.events.push_back(
                            RadrootsSimplexAgentRuntimeEvent::MessageAcknowledged {
                                connection_id: connection_id.into(),
                                message_id: receipt.message_id,
                            },
                        );
                    }
                    RadrootsSimplexAgentMessage::QueueAdd(_)
                    | RadrootsSimplexAgentMessage::QueueKey(_)
                    | RadrootsSimplexAgentMessage::QueueUse(_)
                    | RadrootsSimplexAgentMessage::QueueTest(_)
                    | RadrootsSimplexAgentMessage::QueueContinue(_) => {
                        self.events.push_back(
                            RadrootsSimplexAgentRuntimeEvent::QueueRotationQueued {
                                connection_id: connection_id.into(),
                            },
                        );
                    }
                    _ => {
                        self.events
                            .push_back(RadrootsSimplexAgentRuntimeEvent::MessageReceived {
                                connection_id: connection_id.into(),
                                message_id: frame.header.message_id,
                            });
                    }
                }
            }
        }
        Ok(())
    }

    pub fn record_command_outcome(
        &mut self,
        command_id: u64,
        outcome: RadrootsSimplexAgentCommandOutcome,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        match outcome {
            RadrootsSimplexAgentCommandOutcome::Delivered => {
                let _ = self.store.mark_command_delivered(command_id)?;
            }
            RadrootsSimplexAgentCommandOutcome::RetryAt { ready_at } => {
                let command = self.store.mark_command_retry(command_id, ready_at)?;
                self.events
                    .push_back(RadrootsSimplexAgentRuntimeEvent::RetryQueued {
                        connection_id: command.connection_id,
                        command_id,
                    });
            }
            RadrootsSimplexAgentCommandOutcome::Failed { message } => {
                let command = self.store.mark_command_failed(command_id)?;
                self.events
                    .push_back(RadrootsSimplexAgentRuntimeEvent::Error {
                        connection_id: Some(command.connection_id),
                        message,
                    });
            }
        }
        Ok(())
    }

    pub fn retry_pending(
        &mut self,
        now: u64,
        limit: usize,
    ) -> Vec<RadrootsSimplexAgentPendingCommand> {
        self.store.take_ready_commands(now, limit)
    }

    pub fn drain_events(&mut self, max: usize) -> Vec<RadrootsSimplexAgentRuntimeEvent> {
        let take = self.events.len().min(max);
        (0..take)
            .filter_map(|_| self.events.pop_front())
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpQueueUri;

    fn invitation_queue() -> RadrootsSimplexSmpQueueUri {
        RadrootsSimplexSmpQueueUri::parse(
            "smp://aGVsbG8@relay.example/cXVldWU#/?v=4&dh=Zm9vYmFy&q=m",
        )
        .unwrap()
    }

    fn reply_queue() -> RadrootsSimplexSmpQueueUri {
        RadrootsSimplexSmpQueueUri::parse(
            "smp://aGVsbG8@relay.example/cmVwbHk#/?v=4&dh=YmF6cXV4&q=m",
        )
        .unwrap()
    }

    #[test]
    fn create_join_allow_send_and_retry_flow() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let created = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
        assert!(matches!(
            runtime.drain_events(10).remove(0),
            RadrootsSimplexAgentRuntimeEvent::InvitationReady { .. }
        ));

        let invitation = runtime
            .store
            .connection(&created)
            .unwrap()
            .invitation
            .clone()
            .unwrap();
        let joined = runtime
            .join_connection(invitation, reply_queue(), 20)
            .unwrap();
        runtime
            .allow_connection(&joined, b"local-info".to_vec(), 30)
            .unwrap();
        runtime.subscribe_connection(&joined, 40).unwrap();
        let message_id = runtime
            .send_message(&joined, b"hello simplex".to_vec(), 50)
            .unwrap();
        assert_eq!(message_id, 1);
        runtime
            .ack_message(
                &joined,
                message_id,
                b"hash".to_vec(),
                b"receipt".to_vec(),
                60,
            )
            .unwrap();
        runtime.reconnect_connection(&joined, 70).unwrap();
        let ready = runtime.retry_pending(70 + 5_000, 64);
        assert!(!ready.is_empty());
    }

    #[test]
    fn handles_inbound_hello_and_receipt_events() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let connection_id = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
        runtime.drain_events(8);

        runtime
            .handle_inbound_decrypted_message(
                &connection_id,
                RadrootsSimplexAgentDecryptedMessage::Message(RadrootsSimplexAgentMessageFrame {
                    header: RadrootsSimplexAgentMessageHeader {
                        message_id: 1,
                        previous_message_hash: Vec::new(),
                    },
                    message: RadrootsSimplexAgentMessage::Hello,
                    padding: Vec::new(),
                }),
                b"transport-hash".to_vec(),
            )
            .unwrap();
        runtime
            .handle_inbound_decrypted_message(
                &connection_id,
                RadrootsSimplexAgentDecryptedMessage::Message(RadrootsSimplexAgentMessageFrame {
                    header: RadrootsSimplexAgentMessageHeader {
                        message_id: 2,
                        previous_message_hash: b"transport-hash".to_vec(),
                    },
                    message: RadrootsSimplexAgentMessage::Receipt(
                        RadrootsSimplexAgentMessageReceipt {
                            message_id: 1,
                            message_hash: b"transport-hash".to_vec(),
                            receipt_info: Vec::new(),
                        },
                    ),
                    padding: Vec::new(),
                }),
                b"transport-hash-2".to_vec(),
            )
            .unwrap();

        let events = runtime.drain_events(16);
        assert!(events.iter().any(|event| matches!(
            event,
            RadrootsSimplexAgentRuntimeEvent::ConnectionEstablished { .. }
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            RadrootsSimplexAgentRuntimeEvent::MessageAcknowledged { message_id: 1, .. }
        )));
    }
}

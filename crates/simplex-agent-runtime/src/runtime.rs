use crate::error::RadrootsSimplexAgentRuntimeError;
use crate::types::{RadrootsSimplexAgentCommandOutcome, RadrootsSimplexAgentRuntimeEvent};
use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use radroots_simplex_agent_proto::prelude::{
    RadrootsSimplexAgentConnectionLink, RadrootsSimplexAgentConnectionMode,
    RadrootsSimplexAgentConnectionStatus, RadrootsSimplexAgentDecryptedMessage,
    RadrootsSimplexAgentEncryptedPayload, RadrootsSimplexAgentEnvelope,
    RadrootsSimplexAgentMessage, RadrootsSimplexAgentMessageFrame,
    RadrootsSimplexAgentMessageHeader, RadrootsSimplexAgentMessageReceipt,
    RadrootsSimplexAgentQueueDescriptor, encode_decrypted_message, encode_envelope,
};
use radroots_simplex_agent_store::prelude::{
    RadrootsSimplexAgentOutboundMessage, RadrootsSimplexAgentPendingCommand,
    RadrootsSimplexAgentPendingCommandKind, RadrootsSimplexAgentQueueRole,
    RadrootsSimplexAgentStore,
};
use radroots_simplex_smp_crypto::prelude::{
    RadrootsSimplexSmpQueueAuthorizationMaterial, RadrootsSimplexSmpQueueAuthorizationScope,
    RadrootsSimplexSmpRatchetState,
};
use radroots_simplex_smp_proto::prelude::{
    RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION, RadrootsSimplexSmpBrokerMessage,
    RadrootsSimplexSmpCommand, RadrootsSimplexSmpCorrelationId, RadrootsSimplexSmpMessageFlags,
    RadrootsSimplexSmpNewQueueRequest, RadrootsSimplexSmpQueueMode,
    RadrootsSimplexSmpQueueRequestData, RadrootsSimplexSmpQueueUri, RadrootsSimplexSmpSendCommand,
    RadrootsSimplexSmpSubscriptionMode,
};
use radroots_simplex_smp_transport::prelude::{
    RadrootsSimplexSmpCommandTransport, RadrootsSimplexSmpTransportRequest,
    RadrootsSimplexSmpTransportResponse,
};
use sha2::{Digest, Sha256};
#[cfg(feature = "std")]
use std::path::{Path, PathBuf};

pub struct RadrootsSimplexAgentRuntimeBuilder {
    store: Option<RadrootsSimplexAgentStore>,
    queue_capacity: usize,
    retry_delay_ms: u64,
    #[cfg(feature = "std")]
    persistent_store_path: Option<PathBuf>,
}

impl RadrootsSimplexAgentRuntimeBuilder {
    pub const DEFAULT_QUEUE_CAPACITY: usize = 2_048;
    pub const DEFAULT_RETRY_DELAY_MS: u64 = 5_000;

    pub fn new() -> Self {
        Self {
            store: None,
            queue_capacity: Self::DEFAULT_QUEUE_CAPACITY,
            retry_delay_ms: Self::DEFAULT_RETRY_DELAY_MS,
            #[cfg(feature = "std")]
            persistent_store_path: None,
        }
    }

    pub fn store(mut self, store: RadrootsSimplexAgentStore) -> Self {
        self.store = Some(store);
        self
    }

    #[cfg(feature = "std")]
    pub fn persistent_store_path(mut self, path: impl AsRef<Path>) -> Self {
        self.persistent_store_path = Some(path.as_ref().to_path_buf());
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
        #[cfg(feature = "std")]
        let store = match (self.store, self.persistent_store_path) {
            (Some(mut store), Some(path)) => {
                store.set_persistence_path(path);
                store
            }
            (Some(store), None) => store,
            (None, Some(path)) => RadrootsSimplexAgentStore::open(path)?,
            (None, None) => RadrootsSimplexAgentStore::default(),
        };
        #[cfg(not(feature = "std"))]
        let store = self.store.unwrap_or_default();

        Ok(RadrootsSimplexAgentRuntime {
            store,
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
        let local_dh_public_key = derive_material(
            b"connection-create-local-dh",
            &[
                invitation_queue.to_string().as_bytes(),
                &e2e_public_key,
                &now.to_be_bytes(),
            ],
        );
        let ratchet_state = RadrootsSimplexSmpRatchetState::initiator(
            local_dh_public_key,
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
        self.flush_store()?;
        Ok(connection.id)
    }

    pub fn join_connection(
        &mut self,
        invitation: RadrootsSimplexAgentConnectionLink,
        reply_queue: RadrootsSimplexSmpQueueUri,
        now: u64,
    ) -> Result<String, RadrootsSimplexAgentRuntimeError> {
        let local_dh_public_key = derive_material(
            b"connection-join-local-dh",
            &[
                invitation.connection_id.as_slice(),
                reply_queue.to_string().as_bytes(),
                &now.to_be_bytes(),
            ],
        );
        let ratchet_state = RadrootsSimplexSmpRatchetState::responder(
            local_dh_public_key,
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
            sender_key: Some(derive_material(
                b"join-sender-auth",
                &[invitation.connection_id.as_slice(), &now.to_be_bytes()],
            )),
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
        let confirmation_payload =
            self.next_encrypted_payload(&connection.id, invitation.connection_id)?;
        self.store.enqueue_command(
            &connection.id,
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                queue: send_descriptor.queue_address(),
                envelope: RadrootsSimplexAgentEnvelope::Confirmation {
                    reply_queue: true,
                    encrypted: confirmation_payload,
                },
                delivery: None,
            },
            now,
        )?;
        self.events
            .push_back(RadrootsSimplexAgentRuntimeEvent::ConfirmationRequired {
                connection_id: connection.id.clone(),
            });
        self.flush_store()?;
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
        let encrypted = self.next_encrypted_payload(connection_id, local_info)?;
        self.store.enqueue_command(
            connection_id,
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                queue: send_queue.descriptor.queue_address(),
                envelope: RadrootsSimplexAgentEnvelope::Confirmation {
                    reply_queue: false,
                    encrypted,
                },
                delivery: None,
            },
            now,
        )?;
        self.flush_store()?;
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
        self.flush_store()?;
        Ok(())
    }

    pub fn send_message(
        &mut self,
        connection_id: &str,
        body: Vec<u8>,
        now: u64,
    ) -> Result<u64, RadrootsSimplexAgentRuntimeError> {
        let send_queue = self.store.primary_send_queue(connection_id)?;
        let connection = self.store.connection(connection_id)?;
        if connection.staged_outbound_message.is_some() {
            return Err(RadrootsSimplexAgentRuntimeError::Store(
                radroots_simplex_agent_store::prelude::RadrootsSimplexAgentStoreError::PendingOutboundMessage(
                    connection_id.into(),
                ),
            ));
        }
        let previous_hash = connection
            .delivery_cursor
            .last_sent_message_hash
            .clone()
            .unwrap_or_default();
        let message_id = connection
            .delivery_cursor
            .last_sent_message_id
            .unwrap_or(0)
            .saturating_add(1);
        let frame = RadrootsSimplexAgentMessageFrame {
            header: RadrootsSimplexAgentMessageHeader {
                message_id,
                previous_message_hash: previous_hash,
            },
            message: RadrootsSimplexAgentMessage::UserMessage(body),
            padding: Vec::new(),
        };
        let ciphertext =
            encode_decrypted_message(&RadrootsSimplexAgentDecryptedMessage::Message(frame))?;
        let message_hash = Sha256::digest(&ciphertext).to_vec();
        let prepared = self
            .store
            .prepare_outbound_message(connection_id, message_hash.clone())?;
        let encrypted = self.next_encrypted_payload(connection_id, ciphertext)?;
        self.store.enqueue_command(
            connection_id,
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                queue: send_queue.descriptor.queue_address(),
                envelope: RadrootsSimplexAgentEnvelope::Message(encrypted),
                delivery: Some(RadrootsSimplexAgentOutboundMessage {
                    message_id: prepared.message_id,
                    message_hash: prepared.message_hash,
                }),
            },
            now,
        )?;
        self.events
            .push_back(RadrootsSimplexAgentRuntimeEvent::MessageQueued {
                connection_id: connection_id.into(),
                message_id,
            });
        self.flush_store()?;
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
        self.flush_store()?;
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
        self.flush_store()?;
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
        self.flush_store()?;
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
        self.flush_store()?;
        Ok(())
    }

    pub fn record_command_outcome(
        &mut self,
        command_id: u64,
        outcome: RadrootsSimplexAgentCommandOutcome,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        match outcome {
            RadrootsSimplexAgentCommandOutcome::Delivered => {
                let command = self.store.mark_command_delivered(command_id)?;
                self.apply_delivery_side_effects(&command)?;
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
                self.apply_failure_side_effects(&command)?;
                self.events
                    .push_back(RadrootsSimplexAgentRuntimeEvent::Error {
                        connection_id: Some(command.connection_id),
                        message,
                    });
            }
        }
        self.flush_store()?;
        Ok(())
    }

    pub fn execute_ready_commands<T: RadrootsSimplexSmpCommandTransport>(
        &mut self,
        transport: &mut T,
        now: u64,
        limit: usize,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        for command in self.store.take_ready_commands(now, limit) {
            self.dispatch_ready_command(transport, &command, now)?;
        }
        self.flush_store()?;
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

    fn dispatch_ready_command<T: RadrootsSimplexSmpCommandTransport>(
        &mut self,
        transport: &mut T,
        command: &RadrootsSimplexAgentPendingCommand,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        match &command.kind {
            RadrootsSimplexAgentPendingCommandKind::RotateQueues { descriptors } => {
                for descriptor in descriptors.clone() {
                    self.store.add_queue(
                        &command.connection_id,
                        descriptor,
                        RadrootsSimplexAgentQueueRole::Receive,
                        true,
                    )?;
                }
                self.record_command_outcome(
                    command.id,
                    RadrootsSimplexAgentCommandOutcome::Delivered,
                )
            }
            RadrootsSimplexAgentPendingCommandKind::TestQueues { queues } => {
                for queue in queues {
                    self.store
                        .mark_queue_tested(&command.connection_id, queue)?;
                }
                self.record_command_outcome(
                    command.id,
                    RadrootsSimplexAgentCommandOutcome::Delivered,
                )
            }
            _ => {
                let request = self.build_transport_request(command)?;
                match transport.execute(request) {
                    Ok(response) => self.apply_transport_response(command, response),
                    Err(error) => {
                        self.events
                            .push_back(RadrootsSimplexAgentRuntimeEvent::Error {
                                connection_id: Some(command.connection_id.clone()),
                                message: format!(
                                    "SimpleX transport execution failed for command `{}`: {error}",
                                    command.id
                                ),
                            });
                        self.record_command_outcome(
                            command.id,
                            RadrootsSimplexAgentCommandOutcome::RetryAt {
                                ready_at: now + self.retry_delay_ms,
                            },
                        )
                    }
                }
            }
        }
    }

    fn build_transport_request(
        &self,
        command: &RadrootsSimplexAgentPendingCommand,
    ) -> Result<RadrootsSimplexSmpTransportRequest, RadrootsSimplexAgentRuntimeError> {
        let (queue_address, smp_command) = self.command_transport_parts(command)?;
        let queue = self
            .store
            .queue_record(&command.connection_id, &queue_address)?;
        let auth = queue.auth_state.ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Store(
                radroots_simplex_agent_store::prelude::RadrootsSimplexAgentStoreError::QueueAuthStateMissing(
                    command.connection_id.clone(),
                ),
            )
        })?;
        let correlation_id = correlation_id_for_command(command.id);
        let scope = RadrootsSimplexSmpQueueAuthorizationScope::new(
            auth.session_identifier.clone(),
            correlation_id,
            queue_address.sender_id.clone(),
        )
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        let material = RadrootsSimplexSmpQueueAuthorizationMaterial::for_command(
            &scope,
            &smp_command,
            RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            auth.queue_key_material.clone(),
            auth.server_session_key.clone(),
        )
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        Ok(RadrootsSimplexSmpTransportRequest {
            server: queue.descriptor.queue_uri.server.clone(),
            transport_version: RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            transmission:
                radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpCommandTransmission {
                    authorization: material.authorized_digest.to_vec(),
                    correlation_id: Some(correlation_id),
                    entity_id: queue_address.sender_id,
                    command: smp_command,
                },
        })
    }

    fn command_transport_parts(
        &self,
        command: &RadrootsSimplexAgentPendingCommand,
    ) -> Result<
        (
            radroots_simplex_agent_proto::prelude::RadrootsSimplexAgentQueueAddress,
            RadrootsSimplexSmpCommand,
        ),
        RadrootsSimplexAgentRuntimeError,
    > {
        match &command.kind {
            RadrootsSimplexAgentPendingCommandKind::CreateQueue { descriptor } => Ok((
                descriptor.queue_address(),
                RadrootsSimplexSmpCommand::New(RadrootsSimplexSmpNewQueueRequest {
                    recipient_auth_public_key: descriptor.queue_uri.sender_id.as_bytes().to_vec(),
                    recipient_dh_public_key: descriptor
                        .queue_uri
                        .recipient_dh_public_key
                        .as_bytes()
                        .to_vec(),
                    basic_auth: None,
                    subscription_mode: RadrootsSimplexSmpSubscriptionMode::Subscribe,
                    queue_request_data: Some(
                        match descriptor
                            .queue_uri
                            .queue_mode
                            .unwrap_or(RadrootsSimplexSmpQueueMode::Messaging)
                        {
                            RadrootsSimplexSmpQueueMode::Messaging => {
                                RadrootsSimplexSmpQueueRequestData::Messaging(None)
                            }
                            RadrootsSimplexSmpQueueMode::Contact => {
                                RadrootsSimplexSmpQueueRequestData::Contact(None)
                            }
                        },
                    ),
                    notifier_credentials: None,
                }),
            )),
            RadrootsSimplexAgentPendingCommandKind::SecureQueue { queue, sender_key } => Ok((
                queue.clone(),
                RadrootsSimplexSmpCommand::SKey(sender_key.clone().unwrap_or_default()),
            )),
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                queue, envelope, ..
            } => Ok((
                queue.clone(),
                RadrootsSimplexSmpCommand::Send(RadrootsSimplexSmpSendCommand {
                    flags: RadrootsSimplexSmpMessageFlags::notifications_enabled(),
                    message_body: encode_envelope(envelope)?,
                }),
            )),
            RadrootsSimplexAgentPendingCommandKind::SubscribeQueue { queue } => {
                Ok((queue.clone(), RadrootsSimplexSmpCommand::Sub))
            }
            RadrootsSimplexAgentPendingCommandKind::AckInboxMessage { queue, receipt } => Ok((
                queue.clone(),
                RadrootsSimplexSmpCommand::Ack(receipt.message_id.to_be_bytes().to_vec()),
            )),
            RadrootsSimplexAgentPendingCommandKind::RotateQueues { descriptors } => Ok((
                descriptors
                    .first()
                    .ok_or_else(|| {
                        RadrootsSimplexAgentRuntimeError::Runtime(
                            "queue rotation command requires at least one descriptor".into(),
                        )
                    })?
                    .queue_address(),
                RadrootsSimplexSmpCommand::Que,
            )),
            RadrootsSimplexAgentPendingCommandKind::TestQueues { queues } => Ok((
                queues.first().cloned().ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(
                        "queue test command requires at least one queue".into(),
                    )
                })?,
                RadrootsSimplexSmpCommand::Ping,
            )),
        }
    }

    fn apply_transport_response(
        &mut self,
        command: &RadrootsSimplexAgentPendingCommand,
        response: RadrootsSimplexSmpTransportResponse,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        match response.transmission.message {
            RadrootsSimplexSmpBrokerMessage::Err(error) => self.record_command_outcome(
                command.id,
                RadrootsSimplexAgentCommandOutcome::Failed {
                    message: format!(
                        "SimpleX broker rejected command `{}`: {:?}",
                        command.id, error
                    ),
                },
            ),
            _ => self
                .record_command_outcome(command.id, RadrootsSimplexAgentCommandOutcome::Delivered),
        }
    }

    fn apply_delivery_side_effects(
        &mut self,
        command: &RadrootsSimplexAgentPendingCommand,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        match &command.kind {
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                delivery: Some(delivery),
                ..
            } => {
                let _ = self
                    .store
                    .confirm_outbound_message(&command.connection_id, delivery.message_id)?;
            }
            RadrootsSimplexAgentPendingCommandKind::SubscribeQueue { queue } => {
                self.store
                    .mark_queue_subscribed(&command.connection_id, queue)?;
            }
            RadrootsSimplexAgentPendingCommandKind::TestQueues { queues } => {
                for queue in queues {
                    self.store
                        .mark_queue_tested(&command.connection_id, queue)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn apply_failure_side_effects(
        &mut self,
        command: &RadrootsSimplexAgentPendingCommand,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        if let RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
            delivery: Some(delivery),
            ..
        } = &command.kind
        {
            let _ = self
                .store
                .clear_staged_outbound_message(&command.connection_id, delivery.message_id)?;
        }
        Ok(())
    }

    fn next_encrypted_payload(
        &mut self,
        connection_id: &str,
        ciphertext: Vec<u8>,
    ) -> Result<RadrootsSimplexAgentEncryptedPayload, RadrootsSimplexAgentRuntimeError> {
        let ratchet_header = self
            .store
            .connection_mut(connection_id)?
            .ratchet_state
            .as_mut()
            .map(|state| {
                state
                    .next_outbound_header()
                    .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))
            })
            .transpose()?;
        Ok(RadrootsSimplexAgentEncryptedPayload {
            ratchet_header,
            ciphertext,
        })
    }

    #[cfg(feature = "std")]
    fn flush_store(&self) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        self.store.flush().map_err(Into::into)
    }

    #[cfg(not(feature = "std"))]
    fn flush_store(&self) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        Ok(())
    }
}

fn derive_material(label: &[u8], parts: &[&[u8]]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(label);
    for part in parts {
        hasher.update((*part).len().to_be_bytes());
        hasher.update(*part);
    }
    hasher.finalize().to_vec()
}

fn correlation_id_for_command(command_id: u64) -> RadrootsSimplexSmpCorrelationId {
    let digest = derive_material(b"simplex-command-correlation", &[&command_id.to_be_bytes()]);
    let mut correlation = [0_u8; RadrootsSimplexSmpCorrelationId::LENGTH];
    correlation.copy_from_slice(&digest[..RadrootsSimplexSmpCorrelationId::LENGTH]);
    RadrootsSimplexSmpCorrelationId::new(correlation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::VecDeque;
    use radroots_simplex_smp_proto::prelude::{
        RadrootsSimplexSmpBrokerTransmission, RadrootsSimplexSmpQueueIdsResponse,
    };
    use radroots_simplex_smp_transport::prelude::RadrootsSimplexSmpTransportBlock;

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

    #[derive(Default)]
    struct ScriptedTransport {
        responses: VecDeque<RadrootsSimplexSmpBrokerMessage>,
        requests: Vec<RadrootsSimplexSmpTransportRequest>,
    }

    impl ScriptedTransport {
        fn with_responses(responses: Vec<RadrootsSimplexSmpBrokerMessage>) -> Self {
            Self {
                responses: responses.into(),
                requests: Vec::new(),
            }
        }
    }

    impl RadrootsSimplexSmpCommandTransport for ScriptedTransport {
        type Error = String;

        fn execute(
            &mut self,
            request: RadrootsSimplexSmpTransportRequest,
        ) -> Result<RadrootsSimplexSmpTransportResponse, Self::Error> {
            let block =
                RadrootsSimplexSmpTransportBlock::from_current_command_transmissions(&[request
                    .transmission
                    .clone()])
                .map_err(|error| error.to_string())?;
            let encoded = block.encode().map_err(|error| error.to_string())?;
            let decoded = RadrootsSimplexSmpTransportBlock::decode(&encoded)
                .map_err(|error| error.to_string())?;
            let decoded_transmissions = decoded
                .decode_command_transmissions(request.transport_version)
                .map_err(|error| error.to_string())?;
            assert_eq!(decoded_transmissions.len(), 1);
            assert_eq!(decoded_transmissions[0], request.transmission);

            let response_message = self
                .responses
                .pop_front()
                .ok_or_else(|| "missing scripted transport response".to_owned())?;
            let response_transmission = RadrootsSimplexSmpBrokerTransmission {
                authorization: Vec::new(),
                correlation_id: request.transmission.correlation_id,
                entity_id: request.transmission.entity_id.clone(),
                message: response_message,
            };
            let response_block = RadrootsSimplexSmpTransportBlock::from_broker_transmissions(
                &[response_transmission.clone()],
                request.transport_version,
            )
            .map_err(|error| error.to_string())?;
            let response_encoded = response_block.encode().map_err(|error| error.to_string())?;
            self.requests.push(request.clone());
            Ok(RadrootsSimplexSmpTransportResponse {
                server: request.server,
                transport_version: request.transport_version,
                transmission: response_transmission,
                transport_hash: Sha256::digest(&response_encoded).to_vec(),
            })
        }
    }

    #[test]
    fn create_and_join_commands_execute_through_transport() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let created = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
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

        let mut transport = ScriptedTransport::with_responses(vec![
            RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
                recipient_id: b"recipient".to_vec(),
                sender_id: b"sender".to_vec(),
                server_dh_public_key: b"server-dh".to_vec(),
                queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
                link_id: None,
                service_id: None,
                server_notification_credentials: None,
            }),
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
                recipient_id: b"recipient-2".to_vec(),
                sender_id: b"sender-2".to_vec(),
                server_dh_public_key: b"server-dh-2".to_vec(),
                queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
                link_id: None,
                service_id: None,
                server_notification_credentials: None,
            }),
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut transport, 30, 16)
            .unwrap();

        let created_queue = runtime.store.receive_queues(&created).unwrap();
        assert!(created_queue[0].subscribed);
        assert_eq!(transport.requests.len(), 6);
        assert!(matches!(
            runtime.drain_events(16).first(),
            Some(RadrootsSimplexAgentRuntimeEvent::InvitationReady { .. })
        ));
        assert_eq!(
            runtime.store.connection(&joined).unwrap().status,
            RadrootsSimplexAgentConnectionStatus::JoinPending
        );
    }

    #[test]
    fn delivered_send_confirms_cursor_only_after_transport_success() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let created = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
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

        let mut setup_transport = ScriptedTransport::with_responses(vec![
            RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
                recipient_id: b"recipient".to_vec(),
                sender_id: b"sender".to_vec(),
                server_dh_public_key: b"server-dh".to_vec(),
                queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
                link_id: None,
                service_id: None,
                server_notification_credentials: None,
            }),
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
                recipient_id: b"recipient-2".to_vec(),
                sender_id: b"sender-2".to_vec(),
                server_dh_public_key: b"server-dh-2".to_vec(),
                queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
                link_id: None,
                service_id: None,
                server_notification_credentials: None,
            }),
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut setup_transport, 30, 16)
            .unwrap();

        let message_id = runtime
            .send_message(&joined, b"hello simplex".to_vec(), 40)
            .unwrap();
        assert_eq!(message_id, 1);
        assert_eq!(
            runtime
                .store
                .connection(&joined)
                .unwrap()
                .delivery_cursor
                .last_sent_message_id,
            None
        );

        let mut delivery_transport =
            ScriptedTransport::with_responses(vec![RadrootsSimplexSmpBrokerMessage::Ok]);
        runtime
            .execute_ready_commands(&mut delivery_transport, 50, 16)
            .unwrap();

        let cursor = &runtime.store.connection(&joined).unwrap().delivery_cursor;
        assert_eq!(cursor.last_sent_message_id, Some(1));
        assert!(cursor.last_sent_message_hash.is_some());
        assert_eq!(
            runtime
                .store
                .connection(&joined)
                .unwrap()
                .staged_outbound_message,
            None
        );
    }

    #[test]
    fn transport_retry_keeps_staged_outbound_message() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let created = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
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

        let mut setup_transport = ScriptedTransport::with_responses(vec![
            RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
                recipient_id: b"recipient".to_vec(),
                sender_id: b"sender".to_vec(),
                server_dh_public_key: b"server-dh".to_vec(),
                queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
                link_id: None,
                service_id: None,
                server_notification_credentials: None,
            }),
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
                recipient_id: b"recipient-2".to_vec(),
                sender_id: b"sender-2".to_vec(),
                server_dh_public_key: b"server-dh-2".to_vec(),
                queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
                link_id: None,
                service_id: None,
                server_notification_credentials: None,
            }),
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut setup_transport, 30, 16)
            .unwrap();

        runtime
            .send_message(&joined, b"hello simplex".to_vec(), 40)
            .unwrap();

        struct FailingTransport;
        impl RadrootsSimplexSmpCommandTransport for FailingTransport {
            type Error = String;
            fn execute(
                &mut self,
                _request: RadrootsSimplexSmpTransportRequest,
            ) -> Result<RadrootsSimplexSmpTransportResponse, Self::Error> {
                Err("synthetic failure".to_owned())
            }
        }

        runtime
            .execute_ready_commands(&mut FailingTransport, 50, 16)
            .unwrap();

        assert_eq!(
            runtime
                .store
                .connection(&joined)
                .unwrap()
                .delivery_cursor
                .last_sent_message_id,
            None
        );
        assert_eq!(
            runtime
                .store
                .connection(&joined)
                .unwrap()
                .staged_outbound_message
                .as_ref()
                .map(|message| message.message_id),
            Some(1)
        );
        let ready_again = runtime.retry_pending(50 + 5_000, 16);
        assert_eq!(ready_again.len(), 1);
    }

    #[cfg(feature = "std")]
    #[test]
    fn builder_opens_persistent_store_path() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("runtime-store.json");
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new()
            .persistent_store_path(&path)
            .build()
            .unwrap();
        runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
        assert!(path.exists());
    }

    #[test]
    fn manual_record_command_failure_clears_staged_delivery_state() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let created = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
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

        let mut setup_transport = ScriptedTransport::with_responses(vec![
            RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
                recipient_id: b"recipient".to_vec(),
                sender_id: b"sender".to_vec(),
                server_dh_public_key: b"server-dh".to_vec(),
                queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
                link_id: None,
                service_id: None,
                server_notification_credentials: None,
            }),
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
                recipient_id: b"recipient-2".to_vec(),
                sender_id: b"sender-2".to_vec(),
                server_dh_public_key: b"server-dh-2".to_vec(),
                queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
                link_id: None,
                service_id: None,
                server_notification_credentials: None,
            }),
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut setup_transport, 30, 16)
            .unwrap();

        runtime
            .send_message(&joined, b"hello simplex".to_vec(), 40)
            .unwrap();
        let command = runtime.retry_pending(40, 16).remove(0);
        runtime
            .record_command_outcome(
                command.id,
                RadrootsSimplexAgentCommandOutcome::Failed {
                    message: "synthetic failure".into(),
                },
            )
            .unwrap();
        assert_eq!(
            runtime
                .store
                .connection(&joined)
                .unwrap()
                .staged_outbound_message,
            None
        );
    }
}

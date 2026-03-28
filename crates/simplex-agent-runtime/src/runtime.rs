use crate::error::RadrootsSimplexAgentRuntimeError;
use crate::types::{RadrootsSimplexAgentCommandOutcome, RadrootsSimplexAgentRuntimeEvent};
use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use radroots_simplex_agent_proto::prelude::{
    RadrootsSimplexAgentConnectionLink, RadrootsSimplexAgentConnectionMode,
    RadrootsSimplexAgentConnectionStatus, RadrootsSimplexAgentDecryptedMessage,
    RadrootsSimplexAgentEncryptedPayload, RadrootsSimplexAgentEnvelope,
    RadrootsSimplexAgentMessage, RadrootsSimplexAgentMessageFrame,
    RadrootsSimplexAgentMessageHeader, RadrootsSimplexAgentMessageReceipt,
    RadrootsSimplexAgentQueueAddress, RadrootsSimplexAgentQueueDescriptor,
    decode_decrypted_message, decode_envelope, encode_decrypted_message, encode_envelope,
};
use radroots_simplex_agent_store::prelude::{
    RadrootsSimplexAgentOutboundMessage, RadrootsSimplexAgentPendingCommand,
    RadrootsSimplexAgentPendingCommandKind, RadrootsSimplexAgentQueueRole,
    RadrootsSimplexAgentStore,
};
use radroots_simplex_smp_crypto::prelude::{
    RADROOTS_SIMPLEX_SMP_NONCE_LENGTH, RadrootsSimplexSmpCommandAuthorization,
    RadrootsSimplexSmpRatchetState, RadrootsSimplexSmpX25519Keypair, decrypt_padded,
    derive_shared_secret, encrypt_padded, random_nonce,
};
use radroots_simplex_smp_proto::prelude::{
    RADROOTS_SIMPLEX_SMP_CURRENT_CLIENT_VERSION, RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
    RadrootsSimplexSmpBrokerMessage, RadrootsSimplexSmpCommand, RadrootsSimplexSmpCorrelationId,
    RadrootsSimplexSmpMessageFlags, RadrootsSimplexSmpNewQueueRequest,
    RadrootsSimplexSmpQueueIdsResponse, RadrootsSimplexSmpQueueMode,
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

const SIMPLEX_E2E_CONFIRMATION_LENGTH: usize = 15_904;
const SIMPLEX_E2E_MESSAGE_LENGTH: usize = 16_000;

#[derive(Debug, Clone)]
struct SimplexClientMessageEnvelope {
    sender_public_key: Option<Vec<u8>>,
    nonce: [u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH],
    ciphertext: Vec<u8>,
}

#[derive(Debug, Clone)]
struct SimplexReceivedBody {
    timestamp: u64,
    flags: RadrootsSimplexSmpMessageFlags,
    sent_body: Vec<u8>,
}

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
        mut invitation_queue: RadrootsSimplexSmpQueueUri,
        e2e_seed: Vec<u8>,
        contact_address: bool,
        now: u64,
    ) -> Result<String, RadrootsSimplexAgentRuntimeError> {
        let e2e_keypair = RadrootsSimplexSmpX25519Keypair::from_seed(&e2e_seed);
        invitation_queue.recipient_dh_public_key = encode_queue_public_key(&e2e_keypair.public_key);
        invitation_queue.sender_id = placeholder_sender_id(
            invitation_queue.server.server_identity.as_bytes(),
            &now.to_be_bytes(),
        );
        let local_dh_public_key = derive_material(
            b"connection-create-local-dh",
            &[
                invitation_queue.to_string().as_bytes(),
                &e2e_keypair.public_key,
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
            RadrootsSimplexAgentConnectionStatus::CreatePending,
            None,
            ratchet_state,
        );
        let invitation = RadrootsSimplexAgentConnectionLink {
            invitation_queue: invitation_queue.clone(),
            connection_id: connection.id.as_bytes().to_vec(),
            e2e_public_key: e2e_keypair.public_key.clone(),
            contact_address,
        };
        self.store.connection_mut(&connection.id)?.invitation = Some(invitation);
        let receive_auth_state = self.store.generate_queue_auth_state()?;
        let delivery_keypair = RadrootsSimplexSmpX25519Keypair::generate()
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
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
            receive_auth_state,
        )?;
        {
            let connection = self.store.connection_mut(&connection.id)?;
            connection.local_e2e_public_key = Some(e2e_keypair.public_key);
            connection.local_e2e_private_key = Some(e2e_keypair.private_key);
            let queue = connection
                .queues
                .iter_mut()
                .find(|queue| queue.descriptor.queue_address() == descriptor.queue_address())
                .ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(
                        "SimpleX receive queue missing after create_connection".into(),
                    )
                })?;
            queue.delivery_private_key = Some(delivery_keypair.private_key);
        }
        self.store.enqueue_command(
            &connection.id,
            RadrootsSimplexAgentPendingCommandKind::CreateQueue { descriptor },
            now,
        )?;
        self.flush_store()?;
        Ok(connection.id)
    }

    pub fn join_connection(
        &mut self,
        invitation: RadrootsSimplexAgentConnectionLink,
        mut reply_queue: RadrootsSimplexSmpQueueUri,
        now: u64,
    ) -> Result<String, RadrootsSimplexAgentRuntimeError> {
        let local_e2e_keypair = RadrootsSimplexSmpX25519Keypair::generate()
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        let shared_secret =
            derive_shared_secret(&local_e2e_keypair.private_key, &invitation.e2e_public_key)
                .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        reply_queue.recipient_dh_public_key =
            encode_queue_public_key(&local_e2e_keypair.public_key);
        reply_queue.sender_id =
            placeholder_sender_id(invitation.connection_id.as_slice(), &now.to_be_bytes());
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
        let send_auth_state = self.store.generate_queue_auth_state()?;
        let send_descriptor = RadrootsSimplexAgentQueueDescriptor {
            queue_uri: invitation.invitation_queue.clone(),
            replaced_queue: None,
            primary: true,
            sender_key: Some(send_auth_state.public_key.clone()),
        };
        let receive_auth_state = self.store.generate_queue_auth_state()?;
        let delivery_keypair = RadrootsSimplexSmpX25519Keypair::generate()
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
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
            send_auth_state,
        )?;
        self.store.add_queue(
            &connection.id,
            receive_descriptor.clone(),
            RadrootsSimplexAgentQueueRole::Receive,
            true,
            receive_auth_state,
        )?;
        {
            let connection = self.store.connection_mut(&connection.id)?;
            connection.local_e2e_public_key = Some(local_e2e_keypair.public_key.clone());
            connection.local_e2e_private_key = Some(local_e2e_keypair.private_key);
            connection.shared_secret = Some(shared_secret);
            let queue = connection
                .queues
                .iter_mut()
                .find(|queue| {
                    queue.descriptor.queue_address() == receive_descriptor.queue_address()
                })
                .ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(
                        "SimpleX reply receive queue missing after join_connection".into(),
                    )
                })?;
            queue.delivery_private_key = Some(delivery_keypair.private_key);
        }
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
        let encrypted = self.next_encrypted_payload(
            connection_id,
            encode_decrypted_message(&RadrootsSimplexAgentDecryptedMessage::ConnectionInfo(
                local_info,
            ))?,
            None,
        )?;
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
        let encrypted = self.next_encrypted_payload(connection_id, ciphertext, None)?;
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
        let receive_queue = self
            .store
            .connection(connection_id)?
            .last_received_queue
            .clone()
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has no received queue to acknowledge"
                ))
            })?;
        let broker_message_id = self
            .store
            .connection(connection_id)?
            .last_received_broker_message_id
            .clone()
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has no broker message id to acknowledge"
                ))
            })?;
        self.store.enqueue_command(
            connection_id,
            RadrootsSimplexAgentPendingCommandKind::AckInboxMessage {
                queue: receive_queue,
                broker_message_id,
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

    pub fn ack_last_received_message(
        &mut self,
        connection_id: &str,
        message_id: u64,
        receipt_info: Vec<u8>,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        let message_hash = self
            .store
            .connection(connection_id)?
            .delivery_cursor
            .last_received_message_hash
            .clone()
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has no received message hash to acknowledge"
                ))
            })?;
        self.ack_message(connection_id, message_id, message_hash, receipt_info, now)
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
                    let auth_state = self.store.generate_queue_auth_state()?;
                    let mut descriptor = descriptor;
                    descriptor.sender_key = Some(auth_state.public_key.clone());
                    self.store.add_queue(
                        connection_id,
                        descriptor,
                        RadrootsSimplexAgentQueueRole::Send,
                        true,
                        auth_state,
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
                let _ = transport_hash;
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
                    RadrootsSimplexAgentMessage::UserMessage(body) => {
                        self.events
                            .push_back(RadrootsSimplexAgentRuntimeEvent::MessageReceived {
                                connection_id: connection_id.into(),
                                message_id: frame.header.message_id,
                                body,
                            });
                    }
                    _ => {}
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
        let mut remaining = limit;
        while remaining > 0 {
            let ready = self.store.take_ready_commands(now, remaining);
            if ready.is_empty() {
                break;
            }
            remaining = remaining.saturating_sub(ready.len());
            for command in ready {
                self.dispatch_ready_command(transport, &command, now)?;
            }
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
                    let auth_state = self.store.generate_queue_auth_state()?;
                    self.store.add_queue(
                        &command.connection_id,
                        descriptor,
                        RadrootsSimplexAgentQueueRole::Receive,
                        true,
                        auth_state,
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
        let (queue_address, _entity_id, smp_command) = self.command_transport_parts(command)?;
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
        let authorization = match &command.kind {
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope { .. }
                if queue.role == RadrootsSimplexAgentQueueRole::Send
                    && matches!(
                        self.store.connection(&command.connection_id)?.status,
                        RadrootsSimplexAgentConnectionStatus::JoinPending
                    ) =>
            {
                RadrootsSimplexSmpCommandAuthorization::None
            }
            _ => RadrootsSimplexSmpCommandAuthorization::Ed25519(
                radroots_simplex_smp_crypto::prelude::RadrootsSimplexSmpEd25519Keypair {
                    public_key: auth.public_key,
                    private_key: auth.private_key,
                },
            ),
        };
        Ok(RadrootsSimplexSmpTransportRequest {
            server: queue.descriptor.queue_uri.server.clone(),
            transport_version: RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            correlation_id: Some(correlation_id),
            entity_id: queue.entity_id,
            command: smp_command,
            authorization,
        })
    }

    fn command_transport_parts(
        &self,
        command: &RadrootsSimplexAgentPendingCommand,
    ) -> Result<
        (
            radroots_simplex_agent_proto::prelude::RadrootsSimplexAgentQueueAddress,
            Vec<u8>,
            RadrootsSimplexSmpCommand,
        ),
        RadrootsSimplexAgentRuntimeError,
    > {
        match &command.kind {
            RadrootsSimplexAgentPendingCommandKind::CreateQueue { descriptor } => {
                let auth_state = self
                    .store
                    .queue_auth_state(&command.connection_id, &descriptor.queue_address())?;
                let delivery_private_key = self
                    .store
                    .queue_record(&command.connection_id, &descriptor.queue_address())?
                    .delivery_private_key
                    .ok_or_else(|| {
                        RadrootsSimplexAgentRuntimeError::Runtime(
                            "SimpleX receive queue missing delivery private key".into(),
                        )
                    })?;
                Ok((
                    descriptor.queue_address(),
                    Vec::new(),
                    RadrootsSimplexSmpCommand::New(RadrootsSimplexSmpNewQueueRequest {
                        recipient_auth_public_key: auth_state.public_key,
                        recipient_dh_public_key:
                            RadrootsSimplexSmpX25519Keypair::public_key_from_private(
                                &delivery_private_key,
                            )
                            .map_err(|error| {
                                RadrootsSimplexAgentRuntimeError::Runtime(error.to_string())
                            })?,
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
                ))
            }
            RadrootsSimplexAgentPendingCommandKind::SecureQueue { queue, sender_key } => Ok((
                queue.clone(),
                queue.sender_id.clone(),
                RadrootsSimplexSmpCommand::SKey(sender_key.clone().unwrap_or_default()),
            )),
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                queue, envelope, ..
            } => Ok((
                queue.clone(),
                queue.sender_id.clone(),
                RadrootsSimplexSmpCommand::Send(RadrootsSimplexSmpSendCommand {
                    flags: RadrootsSimplexSmpMessageFlags::notifications_enabled(),
                    message_body: self.encode_smp_message_body(&command.connection_id, envelope)?,
                }),
            )),
            RadrootsSimplexAgentPendingCommandKind::SubscribeQueue { queue } => Ok((
                queue.clone(),
                queue.sender_id.clone(),
                RadrootsSimplexSmpCommand::Get,
            )),
            RadrootsSimplexAgentPendingCommandKind::AckInboxMessage {
                queue,
                broker_message_id,
                ..
            } => Ok((
                queue.clone(),
                queue.sender_id.clone(),
                RadrootsSimplexSmpCommand::Ack(broker_message_id.clone()),
            )),
            RadrootsSimplexAgentPendingCommandKind::RotateQueues { descriptors } => {
                let address = descriptors
                    .first()
                    .ok_or_else(|| {
                        RadrootsSimplexAgentRuntimeError::Runtime(
                            "queue rotation command requires at least one descriptor".into(),
                        )
                    })?
                    .queue_address();
                let entity_id = address.sender_id.clone();
                Ok((address, entity_id, RadrootsSimplexSmpCommand::Que))
            }
            RadrootsSimplexAgentPendingCommandKind::TestQueues { queues } => {
                let address = queues.first().cloned().ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(
                        "queue test command requires at least one queue".into(),
                    )
                })?;
                let entity_id = address.sender_id.clone();
                Ok((address, entity_id, RadrootsSimplexSmpCommand::Ping))
            }
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
            RadrootsSimplexSmpBrokerMessage::Ids(ids) => {
                self.process_queue_ids_response(command, ids)?;
                self.record_command_outcome(
                    command.id,
                    RadrootsSimplexAgentCommandOutcome::Delivered,
                )
            }
            RadrootsSimplexSmpBrokerMessage::Msg(message) => {
                let queue = queue_for_command(command).ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(format!(
                        "SimpleX command `{}` has no queue context for broker message",
                        command.id
                    ))
                })?;
                self.process_received_message_response(
                    &command.connection_id,
                    &queue,
                    message,
                    response.transport_hash,
                )?;
                self.record_command_outcome(
                    command.id,
                    RadrootsSimplexAgentCommandOutcome::Delivered,
                )
            }
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

    fn encode_smp_message_body(
        &self,
        connection_id: &str,
        envelope: &RadrootsSimplexAgentEnvelope,
    ) -> Result<Vec<u8>, RadrootsSimplexAgentRuntimeError> {
        let shared_secret = self
            .store
            .connection(connection_id)?
            .shared_secret
            .clone()
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has no shared queue secret"
                ))
            })?;
        let sender_public_key = match envelope {
            RadrootsSimplexAgentEnvelope::Confirmation { encrypted, .. } => encrypted
                .ratchet_header
                .as_ref()
                .map(|header| header.dh_public_key.clone()),
            _ => None,
        };
        let mut body = Vec::with_capacity(1 + 512);
        body.push(b'_');
        body.extend_from_slice(&encode_envelope(envelope)?);
        let nonce = random_nonce()
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        let padded_len = match envelope {
            RadrootsSimplexAgentEnvelope::Confirmation { .. } => SIMPLEX_E2E_CONFIRMATION_LENGTH,
            _ => SIMPLEX_E2E_MESSAGE_LENGTH,
        };
        let ciphertext = encrypt_padded(&shared_secret, &nonce, &body, padded_len)
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        encode_client_message_envelope(&SimplexClientMessageEnvelope {
            sender_public_key,
            nonce,
            ciphertext,
        })
    }

    fn process_queue_ids_response(
        &mut self,
        command: &RadrootsSimplexAgentPendingCommand,
        ids: RadrootsSimplexSmpQueueIdsResponse,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        let RadrootsSimplexAgentPendingCommandKind::CreateQueue { descriptor } = &command.kind
        else {
            return Err(RadrootsSimplexAgentRuntimeError::Runtime(
                "SimpleX IDS response received for non-create command".into(),
            ));
        };

        let old_address = descriptor.queue_address();
        let sender_id = URL_SAFE_NO_PAD.encode(&ids.sender_id);
        let mut invitation_event = None;
        let mut join_confirmation = None;
        let subscribe_queue;

        {
            let connection = self.store.connection_mut(&command.connection_id)?;
            let queue = connection
                .queues
                .iter_mut()
                .find(|queue| queue.descriptor.queue_address() == old_address)
                .ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(format!(
                        "SimpleX connection `{}` missing receive queue for IDS",
                        command.connection_id
                    ))
                })?;
            let delivery_private_key = queue.delivery_private_key.clone().ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(
                    "SimpleX receive queue missing delivery private key".into(),
                )
            })?;
            queue.delivery_shared_secret = Some(
                derive_shared_secret(&delivery_private_key, &ids.server_dh_public_key).map_err(
                    |error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()),
                )?,
            );
            queue.entity_id = ids.recipient_id.clone();
            queue.descriptor.queue_uri.sender_id = sender_id;
            if let Some(queue_mode) = ids.queue_mode {
                queue.descriptor.queue_uri.queue_mode = Some(queue_mode);
            }
            let new_address = queue.descriptor.queue_address();
            subscribe_queue = new_address.clone();

            if connection.status == RadrootsSimplexAgentConnectionStatus::CreatePending {
                connection.status = RadrootsSimplexAgentConnectionStatus::InvitationReady;
                if let Some(invitation) = connection.invitation.as_mut() {
                    invitation.invitation_queue = queue.descriptor.queue_uri.clone();
                    invitation_event = Some(invitation.clone());
                }
            } else if connection.status == RadrootsSimplexAgentConnectionStatus::JoinPending {
                join_confirmation = Some((
                    queue.descriptor.clone(),
                    connection.local_e2e_public_key.clone().ok_or_else(|| {
                        RadrootsSimplexAgentRuntimeError::Runtime(format!(
                            "SimpleX connection `{}` missing local E2E public key",
                            command.connection_id
                        ))
                    })?,
                ));
            }
        }

        self.store.enqueue_command(
            &command.connection_id,
            RadrootsSimplexAgentPendingCommandKind::SubscribeQueue {
                queue: subscribe_queue,
            },
            command.ready_at,
        )?;
        if let Some(invitation) = invitation_event {
            self.events
                .push_back(RadrootsSimplexAgentRuntimeEvent::InvitationReady {
                    connection_id: command.connection_id.clone(),
                    invitation,
                });
        }
        if let Some((reply_descriptor, sender_public_key)) = join_confirmation {
            let send_queue = self.store.primary_send_queue(&command.connection_id)?;
            let confirmation_payload = self.next_encrypted_payload(
                &command.connection_id,
                encode_decrypted_message(
                    &RadrootsSimplexAgentDecryptedMessage::ConnectionInfoReply {
                        reply_queues: vec![reply_descriptor],
                        info: Vec::new(),
                    },
                )?,
                Some(sender_public_key),
            )?;
            self.store.enqueue_command(
                &command.connection_id,
                RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                    queue: send_queue.descriptor.queue_address(),
                    envelope: RadrootsSimplexAgentEnvelope::Confirmation {
                        reply_queue: true,
                        encrypted: confirmation_payload,
                    },
                    delivery: None,
                },
                command.ready_at,
            )?;
            self.events
                .push_back(RadrootsSimplexAgentRuntimeEvent::ConfirmationRequired {
                    connection_id: command.connection_id.clone(),
                });
        }
        Ok(())
    }

    fn process_received_message_response(
        &mut self,
        connection_id: &str,
        queue: &radroots_simplex_agent_proto::prelude::RadrootsSimplexAgentQueueAddress,
        message: radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpReceivedMessage,
        transport_hash: Vec<u8>,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        let received = self.decode_received_message_body(connection_id, queue, &message)?;
        if received.sent_body.is_empty() {
            return Ok(());
        }
        let (envelope, derived_secret) =
            self.decode_agent_envelope_payload(connection_id, &received.sent_body)?;
        if let Some(shared_secret) = derived_secret {
            self.store.connection_mut(connection_id)?.shared_secret = Some(shared_secret);
        }
        let decrypted = extract_decrypted_message(&envelope)?;
        {
            let connection = self.store.connection_mut(connection_id)?;
            connection.last_received_queue = Some(queue.clone());
        }
        let _ = received.timestamp;
        let _ = received.flags;
        if let RadrootsSimplexAgentDecryptedMessage::Message(frame) = &decrypted {
            self.store.record_inbound_message(
                connection_id,
                queue.clone(),
                message.message_id.clone(),
                frame.header.message_id,
                transport_hash.clone(),
            )?;
        }
        self.handle_inbound_decrypted_message(connection_id, decrypted, transport_hash)
    }

    fn decode_received_message_body(
        &mut self,
        connection_id: &str,
        queue: &radroots_simplex_agent_proto::prelude::RadrootsSimplexAgentQueueAddress,
        message: &radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpReceivedMessage,
    ) -> Result<SimplexReceivedBody, RadrootsSimplexAgentRuntimeError> {
        let queue_record = self.store.queue_record(connection_id, queue)?;
        let delivery_secret = queue_record.delivery_shared_secret.ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX receive queue on `{connection_id}` is missing delivery secret"
            ))
        })?;
        let decrypted = decrypt_padded(
            &delivery_secret,
            &message.message_id,
            &message.encrypted_body,
        )
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        decode_received_body(&decrypted)
    }

    fn decode_agent_envelope_payload(
        &self,
        connection_id: &str,
        payload: &[u8],
    ) -> Result<(RadrootsSimplexAgentEnvelope, Option<Vec<u8>>), RadrootsSimplexAgentRuntimeError>
    {
        let sent = decode_client_message_envelope(payload)?;
        let derived_secret = match self.store.connection(connection_id)?.shared_secret.clone() {
            Some(secret) => Some(secret),
            None => {
                let sender_public_key = sent.sender_public_key.as_deref().ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(format!(
                        "SimpleX connection `{connection_id}` received encrypted body without sender key"
                    ))
                })?;
                let private_key = self
                    .store
                    .connection(connection_id)?
                    .local_e2e_private_key
                    .as_deref()
                    .ok_or_else(|| {
                        RadrootsSimplexAgentRuntimeError::Runtime(format!(
                            "SimpleX connection `{connection_id}` missing local E2E private key"
                        ))
                    })?;
                Some(
                    derive_shared_secret(private_key, sender_public_key).map_err(|error| {
                        RadrootsSimplexAgentRuntimeError::Runtime(error.to_string())
                    })?,
                )
            }
        };
        let shared_secret = derived_secret.clone().ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX connection `{connection_id}` has no shared secret"
            ))
        })?;
        let decrypted = decrypt_padded(&shared_secret, &sent.nonce, &sent.ciphertext)
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        let (_, payload) = decrypted.split_first().ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(
                "SimpleX decrypted client body is empty".into(),
            )
        })?;
        let envelope = decode_envelope(payload)?;
        let should_store_secret = self
            .store
            .connection(connection_id)?
            .shared_secret
            .is_none()
            && sent.sender_public_key.is_some();
        Ok((
            envelope,
            if should_store_secret {
                derived_secret
            } else {
                None
            },
        ))
    }

    fn next_encrypted_payload(
        &mut self,
        connection_id: &str,
        ciphertext: Vec<u8>,
        sender_public_key: Option<Vec<u8>>,
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
        let ratchet_header = match (ratchet_header, sender_public_key) {
            (Some(mut header), Some(public_key)) => {
                header.dh_public_key = public_key;
                Some(header)
            }
            (None, Some(public_key)) => Some(
                radroots_simplex_smp_crypto::prelude::RadrootsSimplexSmpRatchetHeader {
                    previous_sending_chain_length: 0,
                    message_number: 0,
                    dh_public_key: public_key,
                    pq_public_key: None,
                    pq_ciphertext: None,
                },
            ),
            (header, None) => header,
        };
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

fn encode_queue_public_key(public_key: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(public_key)
}

fn placeholder_sender_id(seed_a: &[u8], seed_b: &[u8]) -> String {
    let digest = derive_material(b"simplex-placeholder-sender-id", &[seed_a, seed_b]);
    URL_SAFE_NO_PAD.encode(&digest[..18])
}

fn queue_for_command(
    command: &RadrootsSimplexAgentPendingCommand,
) -> Option<RadrootsSimplexAgentQueueAddress> {
    match &command.kind {
        RadrootsSimplexAgentPendingCommandKind::CreateQueue { descriptor } => {
            Some(descriptor.queue_address())
        }
        RadrootsSimplexAgentPendingCommandKind::SecureQueue { queue, .. }
        | RadrootsSimplexAgentPendingCommandKind::SendEnvelope { queue, .. }
        | RadrootsSimplexAgentPendingCommandKind::SubscribeQueue { queue }
        | RadrootsSimplexAgentPendingCommandKind::AckInboxMessage { queue, .. } => {
            Some(queue.clone())
        }
        RadrootsSimplexAgentPendingCommandKind::RotateQueues { descriptors } => descriptors
            .first()
            .map(RadrootsSimplexAgentQueueDescriptor::queue_address),
        RadrootsSimplexAgentPendingCommandKind::TestQueues { queues } => queues.first().cloned(),
    }
}

fn encode_client_message_envelope(
    envelope: &SimplexClientMessageEnvelope,
) -> Result<Vec<u8>, RadrootsSimplexAgentRuntimeError> {
    let mut buffer = Vec::with_capacity(
        2 + 1
            + envelope
                .sender_public_key
                .as_ref()
                .map_or(0, |value| 1 + value.len())
            + 24
            + envelope.ciphertext.len(),
    );
    buffer.extend_from_slice(&RADROOTS_SIMPLEX_SMP_CURRENT_CLIENT_VERSION.to_be_bytes());
    match envelope.sender_public_key.as_deref() {
        Some(sender_public_key) => {
            if sender_public_key.len() > u8::MAX as usize {
                return Err(RadrootsSimplexAgentRuntimeError::Runtime(
                    "SimpleX sender public key exceeds short-field limit".into(),
                ));
            }
            buffer.push(b'1');
            buffer.push(sender_public_key.len() as u8);
            buffer.extend_from_slice(sender_public_key);
        }
        None => buffer.push(b'0'),
    }
    buffer.extend_from_slice(&envelope.nonce);
    buffer.extend_from_slice(&envelope.ciphertext);
    Ok(buffer)
}

fn decode_client_message_envelope(
    bytes: &[u8],
) -> Result<SimplexClientMessageEnvelope, RadrootsSimplexAgentRuntimeError> {
    if bytes.len() < 2 + 1 + RADROOTS_SIMPLEX_SMP_NONCE_LENGTH {
        return Err(RadrootsSimplexAgentRuntimeError::Runtime(
            "SimpleX client message envelope is truncated".into(),
        ));
    }
    let _version = u16::from_be_bytes([bytes[0], bytes[1]]);
    let mut index = 2;
    let sender_public_key = match bytes[index] {
        b'0' => {
            index += 1;
            None
        }
        b'1' => {
            index += 1;
            let length = *bytes.get(index).ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(
                    "SimpleX confirmation envelope is missing sender key length".into(),
                )
            })? as usize;
            index += 1;
            let sender_public_key = bytes
                .get(index..index + length)
                .ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(
                        "SimpleX confirmation envelope is missing sender key bytes".into(),
                    )
                })?
                .to_vec();
            index += length;
            Some(sender_public_key)
        }
        _ => {
            return Err(RadrootsSimplexAgentRuntimeError::Runtime(
                "SimpleX client message envelope has an unknown public header".into(),
            ));
        }
    };
    let nonce_slice = bytes
        .get(index..index + RADROOTS_SIMPLEX_SMP_NONCE_LENGTH)
        .ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(
                "SimpleX client message envelope is missing nonce".into(),
            )
        })?;
    let mut nonce = [0_u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH];
    nonce.copy_from_slice(nonce_slice);
    index += RADROOTS_SIMPLEX_SMP_NONCE_LENGTH;
    let ciphertext = bytes
        .get(index..)
        .ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(
                "SimpleX client message envelope is missing ciphertext".into(),
            )
        })?
        .to_vec();
    Ok(SimplexClientMessageEnvelope {
        sender_public_key,
        nonce,
        ciphertext,
    })
}

fn decode_received_body(
    bytes: &[u8],
) -> Result<SimplexReceivedBody, RadrootsSimplexAgentRuntimeError> {
    if let Some(timestamp_bytes) = bytes.strip_prefix(b"QUOTA ") {
        let timestamp: [u8; 8] = timestamp_bytes.try_into().map_err(|_| {
            RadrootsSimplexAgentRuntimeError::Runtime(
                "SimpleX quota notification has an invalid timestamp".into(),
            )
        })?;
        return Ok(SimplexReceivedBody {
            timestamp: u64::from_be_bytes(timestamp),
            flags: RadrootsSimplexSmpMessageFlags::notifications_disabled(),
            sent_body: Vec::new(),
        });
    }
    if bytes.len() < 10 {
        return Err(RadrootsSimplexAgentRuntimeError::Runtime(
            "SimpleX received body is truncated".into(),
        ));
    }
    let timestamp = u64::from_be_bytes(bytes[..8].try_into().map_err(|_| {
        RadrootsSimplexAgentRuntimeError::Runtime(
            "SimpleX received body is missing timestamp".into(),
        )
    })?);
    let flags_offset = bytes[8..]
        .iter()
        .position(|byte| *byte == b' ')
        .ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(
                "SimpleX received body is missing message flags separator".into(),
            )
        })?
        + 8;
    let flags_bytes = &bytes[8..flags_offset];
    if flags_bytes.is_empty() {
        return Err(RadrootsSimplexAgentRuntimeError::Runtime(
            "SimpleX received body is missing message flags".into(),
        ));
    }
    let flags = RadrootsSimplexSmpMessageFlags {
        notification: match flags_bytes[0] {
            0 => false,
            1 => true,
            other => {
                return Err(RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX received body has invalid notification flag `{other}`"
                )));
            }
        },
        reserved: flags_bytes[1..].to_vec(),
    };
    Ok(SimplexReceivedBody {
        timestamp,
        flags,
        sent_body: bytes[flags_offset + 1..].to_vec(),
    })
}

fn extract_decrypted_message(
    envelope: &RadrootsSimplexAgentEnvelope,
) -> Result<RadrootsSimplexAgentDecryptedMessage, RadrootsSimplexAgentRuntimeError> {
    match envelope {
        RadrootsSimplexAgentEnvelope::Confirmation { encrypted, .. }
        | RadrootsSimplexAgentEnvelope::Message(encrypted)
        | RadrootsSimplexAgentEnvelope::RatchetKey { encrypted, .. } => {
            decode_decrypted_message(&encrypted.ciphertext).map_err(Into::into)
        }
        RadrootsSimplexAgentEnvelope::Invitation {
            connection_info, ..
        } => decode_decrypted_message(connection_info).map_err(Into::into),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::VecDeque;
    use radroots_simplex_smp_crypto::prelude::{
        RadrootsSimplexSmpQueueAuthorizationMaterial, RadrootsSimplexSmpQueueAuthorizationScope,
        RadrootsSimplexSmpX25519Keypair,
    };
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

    fn ids_response(
        recipient_id: &[u8],
        sender_id: &[u8],
        seed: &[u8],
    ) -> RadrootsSimplexSmpBrokerMessage {
        RadrootsSimplexSmpBrokerMessage::Ids(RadrootsSimplexSmpQueueIdsResponse {
            recipient_id: recipient_id.to_vec(),
            sender_id: sender_id.to_vec(),
            server_dh_public_key: RadrootsSimplexSmpX25519Keypair::from_seed(seed).public_key,
            queue_mode: Some(RadrootsSimplexSmpQueueMode::Messaging),
            link_id: None,
            service_id: None,
            server_notification_credentials: None,
        })
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
            let correlation_id = request
                .correlation_id
                .ok_or_else(|| "missing scripted transport correlation id".to_owned())?;
            let scope = RadrootsSimplexSmpQueueAuthorizationScope::new(
                b"scripted-session".to_vec(),
                correlation_id,
                request.entity_id.clone(),
            )
            .map_err(|error| error.to_string())?;
            let material = RadrootsSimplexSmpQueueAuthorizationMaterial::for_command(
                &scope,
                &request.command,
                request.transport_version,
                &request.authorization,
            )
            .map_err(|error| error.to_string())?;
            let transmission =
                radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpCommandTransmission {
                    authorization: material.authorization,
                    correlation_id: Some(correlation_id),
                    entity_id: request.entity_id.clone(),
                    command: request.command.clone(),
                };
            let block = RadrootsSimplexSmpTransportBlock::from_current_command_transmissions(&[
                transmission.clone(),
            ])
            .map_err(|error| error.to_string())?;
            let encoded = block.encode().map_err(|error| error.to_string())?;
            let decoded = RadrootsSimplexSmpTransportBlock::decode(&encoded)
                .map_err(|error| error.to_string())?;
            let decoded_transmissions = decoded
                .decode_command_transmissions(request.transport_version)
                .map_err(|error| error.to_string())?;
            assert_eq!(decoded_transmissions.len(), 1);
            assert_eq!(decoded_transmissions[0], transmission);

            let response_message = self
                .responses
                .pop_front()
                .ok_or_else(|| "missing scripted transport response".to_owned())?;
            let response_transmission = RadrootsSimplexSmpBrokerTransmission {
                authorization: Vec::new(),
                correlation_id: Some(correlation_id),
                entity_id: request.entity_id.clone(),
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
            ids_response(b"recipient", b"sender", b"server-dh"),
            RadrootsSimplexSmpBrokerMessage::Ok,
            ids_response(b"recipient-2", b"sender-2", b"server-dh-2"),
            RadrootsSimplexSmpBrokerMessage::Ok,
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
            ids_response(b"recipient", b"sender", b"server-dh"),
            RadrootsSimplexSmpBrokerMessage::Ok,
            ids_response(b"recipient-2", b"sender-2", b"server-dh-2"),
            RadrootsSimplexSmpBrokerMessage::Ok,
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
            ids_response(b"recipient", b"sender", b"server-dh"),
            RadrootsSimplexSmpBrokerMessage::Ok,
            ids_response(b"recipient-2", b"sender-2", b"server-dh-2"),
            RadrootsSimplexSmpBrokerMessage::Ok,
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
            ids_response(b"recipient", b"sender", b"server-dh"),
            RadrootsSimplexSmpBrokerMessage::Ok,
            ids_response(b"recipient-2", b"sender-2", b"server-dh-2"),
            RadrootsSimplexSmpBrokerMessage::Ok,
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

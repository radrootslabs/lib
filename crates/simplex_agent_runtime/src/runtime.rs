use crate::error::RadrootsSimplexAgentRuntimeError;
use crate::types::{RadrootsSimplexAgentCommandOutcome, RadrootsSimplexAgentRuntimeEvent};
use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use base64::Engine as _;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use radroots_simplex_agent_proto::prelude::{
    RadrootsSimplexAgentConnectionLink, RadrootsSimplexAgentConnectionMode,
    RadrootsSimplexAgentConnectionStatus, RadrootsSimplexAgentDecryptedMessage,
    RadrootsSimplexAgentEncryptedPayload, RadrootsSimplexAgentEnvelope,
    RadrootsSimplexAgentMessage, RadrootsSimplexAgentMessageFrame,
    RadrootsSimplexAgentMessageHeader, RadrootsSimplexAgentMessageReceipt,
    RadrootsSimplexAgentQueueAddress, RadrootsSimplexAgentQueueDescriptor,
    RadrootsSimplexAgentShortInvitationLink, RadrootsSimplexAgentShortLinkScheme,
    decode_decrypted_message, decode_envelope, decode_short_invitation_fixed_data,
    encode_decrypted_message, encode_envelope, encode_short_invitation_fixed_data,
    encode_short_invitation_user_data,
};
use radroots_simplex_agent_store::prelude::{
    RadrootsSimplexAgentOutboundMessage, RadrootsSimplexAgentPendingCommand,
    RadrootsSimplexAgentPendingCommandKind, RadrootsSimplexAgentPqKeypair,
    RadrootsSimplexAgentQueueRole, RadrootsSimplexAgentShortLinkCredentials,
    RadrootsSimplexAgentStore, RadrootsSimplexAgentX3dhKeypair,
};
use radroots_simplex_smp_crypto::prelude::{
    RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION, RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION,
    RADROOTS_SIMPLEX_SMP_NONCE_LENGTH, RADROOTS_SIMPLEX_SMP_SHORT_LINK_SIGNATURE_LENGTH,
    RadrootsSimplexOfficialSntrup761Keypair, RadrootsSimplexOfficialX3dhParams,
    RadrootsSimplexOfficialX448Keypair, RadrootsSimplexSmpCommandAuthorization,
    RadrootsSimplexSmpCryptoError, RadrootsSimplexSmpEd25519Keypair,
    RadrootsSimplexSmpRatchetState, RadrootsSimplexSmpX25519Keypair, decode_x25519_public_key_x509,
    decrypt_padded, decrypt_short_link_data, derive_invitation_short_link_data_key,
    derive_shared_secret, encode_ed25519_public_key_x509, encode_x25519_public_key_x509,
    encrypt_padded, encrypt_short_link_data, official_sntrup761_keypair_from_seed,
    official_x3dh_receiver_init, official_x3dh_receiver_init_accepting_pq,
    official_x3dh_sender_init, official_x3dh_sender_init_accepting_pq,
    official_x448_keypair_from_seed, random_nonce, sign_short_link_data,
    verify_signed_short_link_data,
};
use radroots_simplex_smp_proto::prelude::{
    RADROOTS_SIMPLEX_SMP_CURRENT_CLIENT_VERSION, RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
    RadrootsSimplexSmpBrokerMessage, RadrootsSimplexSmpCommand, RadrootsSimplexSmpCorrelationId,
    RadrootsSimplexSmpMessageFlags, RadrootsSimplexSmpMessagingQueueRequest,
    RadrootsSimplexSmpNewQueueRequest, RadrootsSimplexSmpQueueIdsResponse,
    RadrootsSimplexSmpQueueLinkData, RadrootsSimplexSmpQueueMode,
    RadrootsSimplexSmpQueueRequestData, RadrootsSimplexSmpQueueUri, RadrootsSimplexSmpSendCommand,
    RadrootsSimplexSmpServerAddress, RadrootsSimplexSmpSubscriptionMode,
    RadrootsSimplexSmpVersionRange,
};
use radroots_simplex_smp_transport::prelude::{
    RadrootsSimplexSmpCommandTransport, RadrootsSimplexSmpSubscriptionReceiveRequest,
    RadrootsSimplexSmpSubscriptionTransport, RadrootsSimplexSmpTransportRequest,
    RadrootsSimplexSmpTransportResponse,
};
use sha2::{Digest, Sha256};
#[cfg(feature = "std")]
use std::path::{Path, PathBuf};

const SIMPLEX_E2E_CONFIRMATION_LENGTH: usize = 15_904;
const SIMPLEX_E2E_MESSAGE_LENGTH: usize = 16_000;
const SIMPLEX_AGENT_E2E_CONN_INFO_LENGTH: usize = 14_832;
const SIMPLEX_AGENT_E2E_CONN_INFO_PQ_LENGTH: usize = 11_106;
const SIMPLEX_AGENT_E2E_MESSAGE_LENGTH: usize = 15_840;
const SIMPLEX_AGENT_E2E_MESSAGE_PQ_LENGTH: usize = 13_618;

#[derive(Debug, Clone)]
struct SimplexClientMessageEnvelope {
    sender_public_key: Option<Vec<u8>>,
    nonce: [u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH],
    ciphertext: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
enum SimplexAgentPayloadKind {
    ConnectionInfo,
    Message,
}

#[derive(Debug, Clone)]
struct SimplexReceivedBody {
    timestamp: u64,
    flags: RadrootsSimplexSmpMessageFlags,
    sent_body: Vec<u8>,
}

struct SimplexPreparedShortInvitationLinkData {
    link_key: Vec<u8>,
    link_public_signature_key: Vec<u8>,
    link_private_signature_key: Vec<u8>,
    encrypted_link_data: RadrootsSimplexSmpQueueLinkData,
}

pub fn decrypt_short_invitation_link_data(
    invitation: &RadrootsSimplexAgentShortInvitationLink,
    encrypted_link_data: &RadrootsSimplexSmpQueueLinkData,
) -> Result<RadrootsSimplexAgentConnectionLink, RadrootsSimplexAgentRuntimeError> {
    let link_data_key = derive_invitation_short_link_data_key(&invitation.link_key)
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
    let signed_link_data = decrypt_short_link_data(&link_data_key, encrypted_link_data)
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
    if signed_link_data.fixed_data.len() <= RADROOTS_SIMPLEX_SMP_SHORT_LINK_SIGNATURE_LENGTH {
        return Err(RadrootsSimplexAgentRuntimeError::Runtime(
            "SimpleX short invitation fixed data is missing its signed payload".into(),
        ));
    }
    let fixed_payload =
        &signed_link_data.fixed_data[RADROOTS_SIMPLEX_SMP_SHORT_LINK_SIGNATURE_LENGTH..];
    let fixed_data = decode_short_invitation_fixed_data(fixed_payload)?;
    let verified = verify_signed_short_link_data(
        &invitation.link_key,
        &fixed_data.root_public_signature_key,
        &signed_link_data,
    )
    .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
    if verified.user_data != encode_short_invitation_user_data(&fixed_data.invitation) {
        return Err(RadrootsSimplexAgentRuntimeError::Runtime(
            "SimpleX short invitation user data does not match the fixed connection link".into(),
        ));
    }
    Ok(fixed_data.invitation)
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
        invitation_queue.recipient_dh_public_key = encode_queue_public_key(&e2e_keypair.public_key)
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        invitation_queue.sender_id = placeholder_sender_id(
            invitation_queue.server.server_identity.as_bytes(),
            &now.to_be_bytes(),
        );
        let x3dh_key_1 = official_x448_keypair_from_seed(&derive_material(
            b"connection-create-x3dh-1",
            &[
                invitation_queue.to_string().as_bytes(),
                &e2e_keypair.public_key,
                &now.to_be_bytes(),
            ],
        ));
        let x3dh_key_2 = official_x448_keypair_from_seed(&derive_material(
            b"connection-create-x3dh-2",
            &[
                invitation_queue.to_string().as_bytes(),
                &e2e_keypair.public_key,
                &now.to_be_bytes(),
            ],
        ));
        let pq_keypair = official_sntrup761_keypair_from_seed(&derive_material(
            b"connection-create-pq-kem",
            &[
                invitation_queue.to_string().as_bytes(),
                &e2e_keypair.public_key,
                &now.to_be_bytes(),
            ],
        ));
        let e2e_ratchet_params = RadrootsSimplexOfficialX3dhParams {
            version_range: RadrootsSimplexSmpVersionRange::new(
                RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION,
                RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            )
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?,
            key_1: x3dh_key_1.public_key.clone(),
            key_2: x3dh_key_2.public_key.clone(),
            pq_public_key: Some(pq_keypair.public_key.clone()),
            pq_ciphertext: None,
        };
        let mut ratchet_state = RadrootsSimplexSmpRatchetState::initiator(
            x3dh_key_2.public_key.clone(),
            x3dh_key_1.public_key.clone(),
            None,
        )
        .ok();
        if let Some(ratchet_state) = ratchet_state.as_mut() {
            ratchet_state.current_pq_public_key = Some(pq_keypair.public_key.clone());
            ratchet_state.local_pq_private_key = Some(pq_keypair.private_key.clone());
        }
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
            e2e_ratchet_params,
            contact_address,
        };
        let prepared_short_link = if contact_address {
            None
        } else {
            Some(prepare_short_invitation_link_data(&invitation)?)
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
            connection.local_x3dh_key_1 = Some(agent_x3dh_keypair(x3dh_key_1));
            connection.local_x3dh_key_2 = Some(agent_x3dh_keypair(x3dh_key_2));
            connection.local_pq_keypair = Some(agent_pq_keypair(pq_keypair));
            connection.short_link =
                prepared_short_link.map(|prepared| RadrootsSimplexAgentShortLinkCredentials {
                    scheme: RadrootsSimplexAgentShortLinkScheme::Simplex,
                    hosts: descriptor.queue_uri.server.hosts.clone(),
                    port: descriptor.queue_uri.server.port,
                    server_key_hash: None,
                    link_id: Vec::new(),
                    link_key: prepared.link_key,
                    link_public_signature_key: prepared.link_public_signature_key,
                    link_private_signature_key: prepared.link_private_signature_key,
                    encrypted_fixed_data: Some(prepared.encrypted_link_data.fixed_data),
                    encrypted_user_data: Some(prepared.encrypted_link_data.user_data),
                });
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
        reply_queue: RadrootsSimplexSmpQueueUri,
        now: u64,
    ) -> Result<String, RadrootsSimplexAgentRuntimeError> {
        let connection = self.store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::JoinPending,
            None,
            None,
        );
        let connection_id = connection.id.clone();
        self.prepare_join_connection(&connection_id, invitation, reply_queue, now)?;
        self.flush_store()?;
        Ok(connection_id)
    }

    pub fn join_short_invitation(
        &mut self,
        invitation: RadrootsSimplexAgentShortInvitationLink,
        reply_queue: RadrootsSimplexSmpQueueUri,
        now: u64,
    ) -> Result<String, RadrootsSimplexAgentRuntimeError> {
        let _ = short_invitation_server(&invitation)?;
        let connection = self.store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::JoinPending,
            None,
            None,
        );
        self.store.enqueue_command(
            &connection.id,
            RadrootsSimplexAgentPendingCommandKind::SecureGetQueueLinkData {
                invitation,
                reply_queue,
            },
            now,
        )?;
        self.flush_store()?;
        Ok(connection.id)
    }

    fn prepare_join_connection(
        &mut self,
        connection_id: &str,
        invitation: RadrootsSimplexAgentConnectionLink,
        mut reply_queue: RadrootsSimplexSmpQueueUri,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        let local_e2e_keypair = RadrootsSimplexSmpX25519Keypair::generate()
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        let invitation_e2e_public_key =
            decode_queue_public_key(&invitation.invitation_queue.recipient_dh_public_key)?;
        let shared_secret =
            derive_shared_secret(&local_e2e_keypair.private_key, &invitation_e2e_public_key)
                .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        reply_queue.recipient_dh_public_key =
            encode_queue_public_key(&local_e2e_keypair.public_key)
                .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        reply_queue.sender_id =
            placeholder_sender_id(invitation.connection_id.as_slice(), &now.to_be_bytes());
        let local_x3dh_key_1 = official_x448_keypair_from_seed(&derive_material(
            b"connection-join-x3dh-1",
            &[
                invitation.connection_id.as_slice(),
                reply_queue.to_string().as_bytes(),
                &now.to_be_bytes(),
            ],
        ));
        let local_x3dh_key_2 = official_x448_keypair_from_seed(&derive_material(
            b"connection-join-x3dh-2",
            &[
                invitation.connection_id.as_slice(),
                reply_queue.to_string().as_bytes(),
                &now.to_be_bytes(),
            ],
        ));
        let local_pq_keypair = invitation
            .e2e_ratchet_params
            .pq_public_key
            .as_ref()
            .map(|_| {
                official_sntrup761_keypair_from_seed(&derive_material(
                    b"connection-join-pq-kem",
                    &[
                        invitation.connection_id.as_slice(),
                        reply_queue.to_string().as_bytes(),
                        &now.to_be_bytes(),
                    ],
                ))
            });
        let mut ratchet_state = RadrootsSimplexSmpRatchetState::responder(
            local_x3dh_key_2.public_key.clone(),
            invitation.e2e_ratchet_params.key_2.clone(),
            local_pq_keypair
                .as_ref()
                .map(|keypair| keypair.public_key.clone()),
        )
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        let local_pq_keypair = if let Some(local_pq_keypair) = local_pq_keypair {
            let sender_init = official_x3dh_sender_init_accepting_pq(
                &local_x3dh_key_1,
                &local_x3dh_key_2,
                local_pq_keypair,
                &invitation.e2e_ratchet_params,
                &derive_material(
                    b"connection-join-pq-encapsulation",
                    &[
                        invitation.connection_id.as_slice(),
                        reply_queue.to_string().as_bytes(),
                        &now.to_be_bytes(),
                    ],
                ),
            )
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
            ratchet_state
                .initialize_official_sender(local_x3dh_key_2.private_key.clone(), sender_init.init)
                .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
            ratchet_state.current_pq_public_key = sender_init.sender_params.pq_public_key.clone();
            ratchet_state.pending_outbound_pq_ciphertext =
                sender_init.sender_params.pq_ciphertext.clone();
            ratchet_state.local_pq_private_key =
                Some(sender_init.local_pq_keypair.private_key.clone());
            Some(sender_init.local_pq_keypair)
        } else {
            let sender_init = official_x3dh_sender_init(
                &local_x3dh_key_1,
                &local_x3dh_key_2,
                &invitation.e2e_ratchet_params,
            )
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
            ratchet_state
                .initialize_official_sender(local_x3dh_key_2.private_key.clone(), sender_init)
                .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
            None
        };
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
        {
            let connection = self.store.connection_mut(connection_id)?;
            connection.mode = RadrootsSimplexAgentConnectionMode::Direct;
            connection.status = RadrootsSimplexAgentConnectionStatus::JoinPending;
            connection.invitation = Some(invitation);
            connection.ratchet_state = Some(ratchet_state);
        }
        self.store.add_queue(
            connection_id,
            send_descriptor.clone(),
            RadrootsSimplexAgentQueueRole::Send,
            true,
            send_auth_state,
        )?;
        self.store.add_queue(
            connection_id,
            receive_descriptor.clone(),
            RadrootsSimplexAgentQueueRole::Receive,
            true,
            receive_auth_state,
        )?;
        {
            let connection = self.store.connection_mut(connection_id)?;
            connection.local_e2e_public_key = Some(local_e2e_keypair.public_key.clone());
            connection.local_e2e_private_key = Some(local_e2e_keypair.private_key);
            connection.local_x3dh_key_1 = Some(agent_x3dh_keypair(local_x3dh_key_1));
            connection.local_x3dh_key_2 = Some(agent_x3dh_keypair(local_x3dh_key_2));
            connection.local_pq_keypair = local_pq_keypair.map(agent_pq_keypair);
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
            connection_id,
            RadrootsSimplexAgentPendingCommandKind::SecureQueue {
                queue: send_descriptor.queue_address(),
                sender_key: send_descriptor.sender_key.clone(),
            },
            now,
        )?;
        self.store.enqueue_command(
            connection_id,
            RadrootsSimplexAgentPendingCommandKind::CreateQueue {
                descriptor: receive_descriptor.clone(),
            },
            now,
        )?;
        Ok(())
    }

    pub fn allow_connection(
        &mut self,
        connection_id: &str,
        local_info: Vec<u8>,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        if self.store.connection(connection_id)?.status
            != RadrootsSimplexAgentConnectionStatus::AwaitingApproval
        {
            return Err(RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX connection `{connection_id}` is not awaiting approval"
            )));
        }
        self.store
            .set_status(connection_id, RadrootsSimplexAgentConnectionStatus::Allowed)?;
        let send_queue = self.store.primary_send_queue(connection_id)?;
        let encrypted = self.next_encrypted_payload(
            connection_id,
            encode_decrypted_message(&RadrootsSimplexAgentDecryptedMessage::ConnectionInfo(
                local_info,
            ))?,
            SimplexAgentPayloadKind::ConnectionInfo,
        )?;
        self.store.enqueue_command(
            connection_id,
            RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                queue: send_queue.descriptor.queue_address(),
                envelope: RadrootsSimplexAgentEnvelope::Confirmation {
                    reply_queue: false,
                    e2e_ratchet_params: None,
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

    pub fn get_connection_message(
        &mut self,
        connection_id: &str,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        for queue in self.store.receive_queues(connection_id)? {
            self.store.enqueue_command(
                connection_id,
                RadrootsSimplexAgentPendingCommandKind::GetQueueMessage {
                    queue: queue.descriptor.queue_address(),
                },
                now,
            )?;
        }
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
        if connection.status != RadrootsSimplexAgentConnectionStatus::Connected {
            return Err(RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX connection `{connection_id}` is not connected"
            )));
        }
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
        let encrypted = self.next_encrypted_payload(
            connection_id,
            ciphertext,
            SimplexAgentPayloadKind::Message,
        )?;
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

    pub fn send_hello(
        &mut self,
        connection_id: &str,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        self.enqueue_hello(connection_id, now)?;
        self.flush_store()?;
        Ok(())
    }

    pub fn ack_message(
        &mut self,
        connection_id: &str,
        message_id: u64,
        message_hash: Vec<u8>,
        receipt_info: Vec<u8>,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        if self
            .store
            .has_pending_ack_message(connection_id, message_id, &message_hash)
        {
            return Ok(());
        }
        let (receive_queue, broker_message_id) = self
            .store
            .inbound_ack_target(connection_id, message_id, &message_hash)?
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has no frame-specific ACK target for message `{message_id}`"
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
        message_hash: Vec<u8>,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        match message {
            RadrootsSimplexAgentDecryptedMessage::ConnectionInfo(info) => {
                if self.store.connection(connection_id)?.status
                    != RadrootsSimplexAgentConnectionStatus::Connected
                {
                    self.store
                        .set_status(connection_id, RadrootsSimplexAgentConnectionStatus::Allowed)?;
                }
                self.enqueue_hello(connection_id, 0)?;
                self.events
                    .push_back(RadrootsSimplexAgentRuntimeEvent::ConnectionInfo {
                        connection_id: connection_id.into(),
                        info,
                    });
            }
            RadrootsSimplexAgentDecryptedMessage::ConnectionInfoReply { reply_queues, info } => {
                let mut secure_queues = Vec::new();
                for descriptor in reply_queues {
                    let auth_state = self.store.generate_queue_auth_state()?;
                    let mut descriptor = descriptor;
                    descriptor.sender_key = Some(auth_state.public_key.clone());
                    let secure_queue = descriptor.queue_address();
                    let sender_key = descriptor.sender_key.clone();
                    self.store.add_queue(
                        connection_id,
                        descriptor,
                        RadrootsSimplexAgentQueueRole::Send,
                        true,
                        auth_state,
                    )?;
                    secure_queues.push((secure_queue, sender_key));
                }
                self.store.set_status(
                    connection_id,
                    RadrootsSimplexAgentConnectionStatus::AwaitingApproval,
                )?;
                for (queue, sender_key) in secure_queues {
                    self.store.enqueue_command(
                        connection_id,
                        RadrootsSimplexAgentPendingCommandKind::SecureQueue { queue, sender_key },
                        0,
                    )?;
                }
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
            RadrootsSimplexAgentDecryptedMessage::Message(frame) => match frame.message {
                RadrootsSimplexAgentMessage::Hello => {
                    let connection = self.store.connection(connection_id)?;
                    let was_connected =
                        connection.status == RadrootsSimplexAgentConnectionStatus::Connected;
                    let should_send_hello = !connection.hello_sent;
                    {
                        let connection = self.store.connection_mut(connection_id)?;
                        connection.hello_received = true;
                    }
                    if should_send_hello {
                        self.enqueue_hello(connection_id, 0)?;
                    }
                    if !was_connected {
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
                }
                RadrootsSimplexAgentMessage::Receipt(receipt) => {
                    self.events
                        .push_back(RadrootsSimplexAgentRuntimeEvent::MessageAcknowledged {
                            connection_id: connection_id.into(),
                            message_id: receipt.message_id,
                        });
                }
                RadrootsSimplexAgentMessage::QueueAdd(_)
                | RadrootsSimplexAgentMessage::QueueKey(_)
                | RadrootsSimplexAgentMessage::QueueUse(_)
                | RadrootsSimplexAgentMessage::QueueTest(_)
                | RadrootsSimplexAgentMessage::QueueContinue(_) => {
                    self.events
                        .push_back(RadrootsSimplexAgentRuntimeEvent::QueueRotationQueued {
                            connection_id: connection_id.into(),
                        });
                }
                RadrootsSimplexAgentMessage::UserMessage(body) => {
                    let broker_message_id_hash = self
                        .store
                        .connection(connection_id)?
                        .last_received_broker_message_id
                        .as_ref()
                        .map(|broker_message_id| Sha256::digest(broker_message_id).to_vec())
                        .unwrap_or_default();
                    self.events
                        .push_back(RadrootsSimplexAgentRuntimeEvent::MessageReceived {
                            connection_id: connection_id.into(),
                            message_id: frame.header.message_id,
                            broker_message_id_hash,
                            message_hash,
                            body,
                        });
                }
                _ => {}
            },
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

    pub fn receive_subscription_messages<T: RadrootsSimplexSmpSubscriptionTransport>(
        &mut self,
        transport: &mut T,
        limit: usize,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        let mut remaining = limit;
        for server in self.store.subscribed_receive_servers() {
            while remaining > 0 {
                match transport.receive_subscription(RadrootsSimplexSmpSubscriptionReceiveRequest {
                    server: server.clone(),
                }) {
                    Ok(Some(response)) => {
                        self.apply_subscription_response(response)?;
                        remaining = remaining.saturating_sub(1);
                    }
                    Ok(None) => break,
                    Err(error) => {
                        self.events
                            .push_back(RadrootsSimplexAgentRuntimeEvent::Error {
                                connection_id: None,
                                message: format!(
                                    "SimpleX subscription receive failed for server `{}`: {error}",
                                    server.server_identity
                                ),
                            });
                        break;
                    }
                }
            }
            if remaining == 0 {
                break;
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
        match &command.kind {
            RadrootsSimplexAgentPendingCommandKind::SecureGetQueueLinkData {
                invitation, ..
            } => {
                let server = short_invitation_server(invitation)?;
                return Ok(self.server_transport_request(
                    command.id,
                    &server,
                    invitation.link_id.clone(),
                    RadrootsSimplexSmpCommand::LKey(invitation.link_key.clone()),
                ));
            }
            RadrootsSimplexAgentPendingCommandKind::GetQueueLinkData { invitation, .. } => {
                let server = short_invitation_server(invitation)?;
                return Ok(self.server_transport_request(
                    command.id,
                    &server,
                    invitation.link_id.clone(),
                    RadrootsSimplexSmpCommand::LGet,
                ));
            }
            _ => {}
        }
        let (queue_address, entity_id, smp_command) = self.command_transport_parts(command)?;
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
            entity_id,
            command: smp_command,
            authorization,
        })
    }

    fn server_transport_request(
        &self,
        command_id: u64,
        server: &RadrootsSimplexSmpServerAddress,
        entity_id: Vec<u8>,
        command: RadrootsSimplexSmpCommand,
    ) -> RadrootsSimplexSmpTransportRequest {
        RadrootsSimplexSmpTransportRequest {
            server: server.clone(),
            transport_version: RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            correlation_id: Some(correlation_id_for_command(command_id)),
            entity_id,
            command,
            authorization: RadrootsSimplexSmpCommandAuthorization::None,
        }
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
                        recipient_auth_public_key: encode_ed25519_public_key_x509(
                            &auth_state.public_key,
                        )
                        .map_err(|error| {
                            RadrootsSimplexAgentRuntimeError::Runtime(error.to_string())
                        })?,
                        recipient_dh_public_key: encode_x25519_public_key_x509(
                            &RadrootsSimplexSmpX25519Keypair::public_key_from_private(
                                &delivery_private_key,
                            )
                            .map_err(|error| {
                                RadrootsSimplexAgentRuntimeError::Runtime(error.to_string())
                            })?,
                        )
                        .map_err(|error| {
                            RadrootsSimplexAgentRuntimeError::Runtime(error.to_string())
                        })?,
                        basic_auth: None,
                        subscription_mode: RadrootsSimplexSmpSubscriptionMode::OnlyCreate,
                        queue_request_data: Some(
                            match descriptor
                                .queue_uri
                                .queue_mode
                                .unwrap_or(RadrootsSimplexSmpQueueMode::Messaging)
                            {
                                RadrootsSimplexSmpQueueMode::Messaging => {
                                    RadrootsSimplexSmpQueueRequestData::Messaging(
                                        self.short_link_messaging_queue_request(
                                            &command.connection_id,
                                            descriptor,
                                        )?,
                                    )
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
                RadrootsSimplexSmpCommand::SKey(
                    encode_ed25519_public_key_x509(sender_key.as_deref().unwrap_or_default())
                        .map_err(|error| {
                            RadrootsSimplexAgentRuntimeError::Runtime(error.to_string())
                        })?,
                ),
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
                self.store
                    .queue_record(&command.connection_id, queue)?
                    .entity_id,
                RadrootsSimplexSmpCommand::Sub,
            )),
            RadrootsSimplexAgentPendingCommandKind::GetQueueMessage { queue } => Ok((
                queue.clone(),
                self.store
                    .queue_record(&command.connection_id, queue)?
                    .entity_id,
                RadrootsSimplexSmpCommand::Get,
            )),
            RadrootsSimplexAgentPendingCommandKind::AckInboxMessage {
                queue,
                broker_message_id,
                ..
            } => Ok((
                queue.clone(),
                self.store
                    .queue_record(&command.connection_id, queue)?
                    .entity_id,
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
            RadrootsSimplexAgentPendingCommandKind::SetQueueLinkData {
                queue,
                link_id,
                link_data,
            } => Ok((
                queue.clone(),
                self.store
                    .queue_record(&command.connection_id, queue)?
                    .entity_id,
                RadrootsSimplexSmpCommand::LSet {
                    link_id: link_id.clone(),
                    link_data: link_data.clone(),
                },
            )),
            RadrootsSimplexAgentPendingCommandKind::SecureGetQueueLinkData { .. }
            | RadrootsSimplexAgentPendingCommandKind::GetQueueLinkData { .. } => {
                Err(RadrootsSimplexAgentRuntimeError::Runtime(
                    "SimpleX short-link retrieval commands require server transport dispatch"
                        .into(),
                ))
            }
        }
    }

    fn short_link_messaging_queue_request(
        &self,
        connection_id: &str,
        descriptor: &RadrootsSimplexAgentQueueDescriptor,
    ) -> Result<Option<RadrootsSimplexSmpMessagingQueueRequest>, RadrootsSimplexAgentRuntimeError>
    {
        let connection = self.store.connection(connection_id)?;
        if connection.status != RadrootsSimplexAgentConnectionStatus::CreatePending {
            return Ok(None);
        }
        let Some(short_link) = connection.short_link.as_ref() else {
            return Ok(None);
        };
        let fixed_data = short_link.encrypted_fixed_data.clone().ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX connection `{connection_id}` is missing encrypted short-link fixed data"
            ))
        })?;
        let user_data = short_link.encrypted_user_data.clone().ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX connection `{connection_id}` is missing encrypted short-link user data"
            ))
        })?;
        Ok(Some(RadrootsSimplexSmpMessagingQueueRequest {
            sender_id: descriptor.queue_address().sender_id,
            link_data: RadrootsSimplexSmpQueueLinkData {
                fixed_data,
                user_data,
            },
        }))
    }

    fn process_short_link_response(
        &mut self,
        command: &RadrootsSimplexAgentPendingCommand,
        sender_id: Vec<u8>,
        link_data: RadrootsSimplexSmpQueueLinkData,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        let RadrootsSimplexAgentPendingCommandKind::GetQueueLinkData {
            invitation,
            reply_queue,
        } = &command.kind
        else {
            return Err(RadrootsSimplexAgentRuntimeError::Runtime(
                "SimpleX LNK response received for non-retrieval command".into(),
            ));
        };
        let mut connection_link = decrypt_short_invitation_link_data(invitation, &link_data)?;
        connection_link.invitation_queue.sender_id = URL_SAFE_NO_PAD.encode(sender_id);
        self.prepare_join_connection(
            &command.connection_id,
            connection_link,
            reply_queue.clone(),
            command.ready_at,
        )
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
            RadrootsSimplexSmpBrokerMessage::Lnk {
                sender_id,
                link_data,
            } => {
                self.process_short_link_response(command, sender_id, link_data)?;
                self.record_command_outcome(
                    command.id,
                    RadrootsSimplexAgentCommandOutcome::Delivered,
                )
            }
            _ => self
                .record_command_outcome(command.id, RadrootsSimplexAgentCommandOutcome::Delivered),
        }
    }

    fn apply_subscription_response(
        &mut self,
        response: RadrootsSimplexSmpTransportResponse,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        let entity_id = response.transmission.entity_id.clone();
        let (connection_id, queue) = self
            .store
            .receive_queue_by_entity_id(&response.server, &entity_id)
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX subscription response for server `{}` used unknown queue entity `{}`",
                    response.server.server_identity,
                    URL_SAFE_NO_PAD.encode(&entity_id)
                ))
            })?;
        match response.transmission.message {
            RadrootsSimplexSmpBrokerMessage::Msg(message) => self
                .process_received_message_response(
                    &connection_id,
                    &queue,
                    message,
                    response.transport_hash,
                ),
            RadrootsSimplexSmpBrokerMessage::Err(error) => {
                self.events
                    .push_back(RadrootsSimplexAgentRuntimeEvent::Error {
                        connection_id: Some(connection_id),
                        message: format!(
                            "SimpleX subscription broker error for queue entity `{}`: {:?}",
                            URL_SAFE_NO_PAD.encode(&entity_id),
                            error
                        ),
                    });
                Ok(())
            }
            _ => Ok(()),
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
                let delivered = self
                    .store
                    .confirm_outbound_message(&command.connection_id, delivery.message_id)?;
                self.events
                    .push_back(RadrootsSimplexAgentRuntimeEvent::OutboundMessageDelivered {
                        connection_id: command.connection_id.clone(),
                        message_id: delivered.message_id,
                        message_hash: delivered.message_hash,
                    });
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
            RadrootsSimplexAgentPendingCommandKind::AckInboxMessage { receipt, .. } => {
                self.events.push_back(
                    RadrootsSimplexAgentRuntimeEvent::InboundMessageAckDelivered {
                        connection_id: command.connection_id.clone(),
                        message_id: receipt.message_id,
                        message_hash: receipt.message_hash.clone(),
                    },
                );
            }
            RadrootsSimplexAgentPendingCommandKind::SecureGetQueueLinkData {
                invitation,
                reply_queue,
            } => {
                self.store.enqueue_command(
                    &command.connection_id,
                    RadrootsSimplexAgentPendingCommandKind::GetQueueLinkData {
                        invitation: invitation.clone(),
                        reply_queue: reply_queue.clone(),
                    },
                    command.ready_at,
                )?;
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

    fn enqueue_hello(
        &mut self,
        connection_id: &str,
        now: u64,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        if self.store.connection(connection_id)?.hello_sent {
            return Ok(());
        }
        let send_queue = self.store.primary_send_queue(connection_id)?;
        let connection = self.store.connection(connection_id)?;
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
            message: RadrootsSimplexAgentMessage::Hello,
            padding: Vec::new(),
        };
        let ciphertext =
            encode_decrypted_message(&RadrootsSimplexAgentDecryptedMessage::Message(frame))?;
        let message_hash = Sha256::digest(&ciphertext).to_vec();
        let prepared = self
            .store
            .prepare_outbound_message(connection_id, message_hash)?;
        let encrypted = self.next_encrypted_payload(
            connection_id,
            ciphertext,
            SimplexAgentPayloadKind::Message,
        )?;
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
        self.store.connection_mut(connection_id)?.hello_sent = true;
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
            RadrootsSimplexAgentEnvelope::Confirmation {
                reply_queue: true, ..
            } => Some(
                self.store
                    .connection(connection_id)?
                    .local_e2e_public_key
                    .clone()
                    .ok_or_else(|| {
                        RadrootsSimplexAgentRuntimeError::Runtime(format!(
                            "SimpleX connection `{connection_id}` is missing local E2E public key"
                        ))
                    })?,
            ),
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
            let server_dh_public_key = decode_x25519_public_key_x509(&ids.server_dh_public_key)
                .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
            queue.delivery_shared_secret = Some(
                derive_shared_secret(&delivery_private_key, &server_dh_public_key).map_err(
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
                }
                if let Some(short_link) = connection.short_link.as_mut() {
                    short_link.link_id = ids.link_id.clone().ok_or_else(|| {
                        RadrootsSimplexAgentRuntimeError::Runtime(format!(
                            "SimpleX broker IDS response for `{}` did not include a short-link id",
                            command.connection_id
                        ))
                    })?;
                    short_link.hosts = queue.descriptor.queue_uri.server.hosts.clone();
                    short_link.port = queue.descriptor.queue_uri.server.port;
                    invitation_event = Some(short_link.invitation_link());
                }
            } else if connection.status == RadrootsSimplexAgentConnectionStatus::JoinPending {
                let local_x3dh_key_1 = connection.local_x3dh_key_1.as_ref().ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(format!(
                        "SimpleX connection `{}` missing local X3DH key 1",
                        command.connection_id
                    ))
                })?;
                let local_x3dh_key_2 = connection.local_x3dh_key_2.as_ref().ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(format!(
                        "SimpleX connection `{}` missing local X3DH key 2",
                        command.connection_id
                    ))
                })?;
                let ratchet_state = connection.ratchet_state.as_ref().ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(format!(
                        "SimpleX connection `{}` missing ratchet state",
                        command.connection_id
                    ))
                })?;
                join_confirmation = Some((
                    queue.descriptor.clone(),
                    official_x3dh_params_from_parts(
                        &local_x3dh_key_1.public_key,
                        &local_x3dh_key_2.public_key,
                        ratchet_state.current_pq_public_key.clone(),
                        ratchet_state.pending_outbound_pq_ciphertext.clone(),
                    )?,
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
        if let Some((reply_descriptor, e2e_ratchet_params)) = join_confirmation {
            let send_queue = self.store.primary_send_queue(&command.connection_id)?;
            let confirmation_payload = self.next_encrypted_payload(
                &command.connection_id,
                encode_decrypted_message(
                    &RadrootsSimplexAgentDecryptedMessage::ConnectionInfoReply {
                        reply_queues: vec![reply_descriptor],
                        info: Vec::new(),
                    },
                )?,
                SimplexAgentPayloadKind::ConnectionInfo,
            )?;
            self.store.enqueue_command(
                &command.connection_id,
                RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                    queue: send_queue.descriptor.queue_address(),
                    envelope: RadrootsSimplexAgentEnvelope::Confirmation {
                        reply_queue: true,
                        e2e_ratchet_params: Some(e2e_ratchet_params),
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
        self.initialize_receiver_ratchet_from_confirmation(connection_id, &envelope)?;
        let decrypted = self.extract_decrypted_message(connection_id, &envelope)?;
        let agent_message_hash =
            if let RadrootsSimplexAgentDecryptedMessage::Message(frame) = &decrypted {
                let encoded = encode_decrypted_message(&decrypted)?;
                let message_hash = Sha256::digest(&encoded).to_vec();
                self.validate_inbound_frame_progress(connection_id, frame, &message_hash)?;
                Some(message_hash)
            } else {
                None
            };
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
                agent_message_hash.clone().unwrap_or_default(),
            )?;
        }
        self.handle_inbound_decrypted_message(
            connection_id,
            decrypted,
            agent_message_hash.unwrap_or(transport_hash),
        )
    }

    fn validate_inbound_frame_progress(
        &self,
        connection_id: &str,
        frame: &RadrootsSimplexAgentMessageFrame,
        message_hash: &[u8],
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        if frame.header.message_id == 0 {
            return Err(RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX inbound message id for `{connection_id}` must start at 1"
            )));
        }
        let connection = self.store.connection(connection_id)?;
        let Some(last_message_id) = connection.delivery_cursor.last_received_message_id else {
            if frame.header.message_id != 1 {
                return Err(RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX inbound message id for `{connection_id}` started at `{}` instead of `1`",
                    frame.header.message_id
                )));
            }
            if !frame.header.previous_message_hash.is_empty() {
                return Err(RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX first inbound message for `{connection_id}` carried a previous-message hash"
                )));
            }
            return Ok(());
        };
        let last_message_hash = connection
            .delivery_cursor
            .last_received_message_hash
            .as_deref()
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has a received message id without a message hash"
                ))
            })?;
        if frame.header.message_id == last_message_id {
            if message_hash == last_message_hash {
                return Ok(());
            }
            return Err(RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX inbound message id `{last_message_id}` for `{connection_id}` was replayed with a different message hash"
            )));
        }
        if frame.header.message_id < last_message_id {
            return Err(RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX inbound message id `{}` for `{connection_id}` regressed below `{last_message_id}`",
                frame.header.message_id
            )));
        }
        let expected_message_id = last_message_id.checked_add(1).ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX inbound message id for `{connection_id}` overflowed"
            ))
        })?;
        if frame.header.message_id != expected_message_id {
            return Err(RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX inbound message id `{}` for `{connection_id}` skipped expected `{expected_message_id}`",
                frame.header.message_id
            )));
        }
        if frame.header.previous_message_hash != last_message_hash {
            return Err(RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX inbound message `{}` for `{connection_id}` carried an unexpected previous-message hash",
                frame.header.message_id
            )));
        }
        Ok(())
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

    fn initialize_receiver_ratchet_from_confirmation(
        &mut self,
        connection_id: &str,
        envelope: &RadrootsSimplexAgentEnvelope,
    ) -> Result<(), RadrootsSimplexAgentRuntimeError> {
        let RadrootsSimplexAgentEnvelope::Confirmation {
            e2e_ratchet_params: Some(params),
            ..
        } = envelope
        else {
            return Ok(());
        };
        let connection = self.store.connection(connection_id)?;
        let local_key_1 = connection.local_x3dh_key_1.clone().ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX connection `{connection_id}` missing local X3DH key 1"
            ))
        })?;
        let local_key_2 = connection.local_x3dh_key_2.clone().ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX connection `{connection_id}` missing local X3DH key 2"
            ))
        })?;
        let local_pq_keypair = connection.local_pq_keypair.clone();
        let local_key_1 = official_x3dh_keypair_from_agent(local_key_1);
        let local_key_2 = official_x3dh_keypair_from_agent(local_key_2);
        let receiver_init = if params.pq_public_key.is_some() || params.pq_ciphertext.is_some() {
            let local_pq_keypair = local_pq_keypair.as_ref().ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` missing local PQ keypair"
                ))
            })?;
            official_x3dh_receiver_init_accepting_pq(
                &local_key_1,
                &local_key_2,
                &official_pq_keypair_from_agent(local_pq_keypair.clone()),
                params,
            )
            .map(|init| init.init)
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?
        } else {
            official_x3dh_receiver_init(&local_key_1, &local_key_2, params)
                .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?
        };
        let connection = self.store.connection_mut(connection_id)?;
        let ratchet_state = connection.ratchet_state.as_mut().ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX connection `{connection_id}` has no ratchet state"
            ))
        })?;
        if let Some(local_pq_keypair) = local_pq_keypair {
            ratchet_state.current_pq_public_key = Some(local_pq_keypair.public_key);
            ratchet_state.local_pq_private_key = Some(local_pq_keypair.private_key);
        }
        ratchet_state
            .initialize_official_receiver(local_key_2.private_key, receiver_init)
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))
    }

    fn next_encrypted_payload(
        &mut self,
        connection_id: &str,
        plaintext: Vec<u8>,
        payload_kind: SimplexAgentPayloadKind,
    ) -> Result<RadrootsSimplexAgentEncryptedPayload, RadrootsSimplexAgentRuntimeError> {
        let shared_secret = self
            .store
            .connection(connection_id)?
            .shared_secret
            .clone()
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has no shared secret"
                ))
            })?;
        let padded_len = self.agent_payload_padded_len(connection_id, payload_kind)?;
        let official_message = self
            .store
            .connection_mut(connection_id)?
            .ratchet_state
            .as_mut()
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has no ratchet state"
                ))
            })?
            .encrypt_official_payload(&shared_secret, &plaintext, padded_len)
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
        Ok(RadrootsSimplexAgentEncryptedPayload {
            ratchet_header: None,
            official_message: Some(official_message),
            ciphertext: Vec::new(),
        })
    }

    fn extract_decrypted_message(
        &mut self,
        connection_id: &str,
        envelope: &RadrootsSimplexAgentEnvelope,
    ) -> Result<RadrootsSimplexAgentDecryptedMessage, RadrootsSimplexAgentRuntimeError> {
        match envelope {
            RadrootsSimplexAgentEnvelope::Confirmation { encrypted, .. }
            | RadrootsSimplexAgentEnvelope::Message(encrypted)
            | RadrootsSimplexAgentEnvelope::RatchetKey { encrypted, .. } => {
                let plaintext = self.decrypt_agent_payload(connection_id, encrypted)?;
                decode_decrypted_message(&plaintext).map_err(Into::into)
            }
            RadrootsSimplexAgentEnvelope::Invitation {
                connection_info, ..
            } => decode_decrypted_message(connection_info).map_err(Into::into),
        }
    }

    fn decrypt_agent_payload(
        &mut self,
        connection_id: &str,
        encrypted: &RadrootsSimplexAgentEncryptedPayload,
    ) -> Result<Vec<u8>, RadrootsSimplexAgentRuntimeError> {
        let shared_secret = self
            .store
            .connection(connection_id)?
            .shared_secret
            .clone()
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has no shared secret"
                ))
            })?;
        if let Some(official_message) = encrypted.official_message.as_ref() {
            return self
                .store
                .connection_mut(connection_id)?
                .ratchet_state
                .as_mut()
                .ok_or_else(|| {
                    RadrootsSimplexAgentRuntimeError::Runtime(format!(
                        "SimpleX connection `{connection_id}` has no ratchet state"
                    ))
                })?
                .decrypt_official_payload(&shared_secret, official_message)
                .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()));
        }
        let header = encrypted.ratchet_header.as_ref().ok_or_else(|| {
            RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "SimpleX connection `{connection_id}` received agent payload without ratchet header"
            ))
        })?;
        self.store
            .connection_mut(connection_id)?
            .ratchet_state
            .as_mut()
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has no ratchet state"
                ))
            })?
            .decrypt_payload(&shared_secret, header, &encrypted.ciphertext)
            .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))
    }

    fn agent_payload_padded_len(
        &self,
        connection_id: &str,
        payload_kind: SimplexAgentPayloadKind,
    ) -> Result<usize, RadrootsSimplexAgentRuntimeError> {
        let ratchet = self
            .store
            .connection(connection_id)?
            .ratchet_state
            .as_ref()
            .ok_or_else(|| {
                RadrootsSimplexAgentRuntimeError::Runtime(format!(
                    "SimpleX connection `{connection_id}` has no ratchet state"
                ))
            })?;
        let pq_enabled = ratchet.current_pq_public_key.is_some()
            || ratchet.remote_pq_public_key.is_some()
            || ratchet.current_pq_shared_secret.is_some()
            || ratchet.local_pq_private_key.is_some();
        Ok(match (payload_kind, pq_enabled) {
            (SimplexAgentPayloadKind::ConnectionInfo, true) => {
                SIMPLEX_AGENT_E2E_CONN_INFO_PQ_LENGTH
            }
            (SimplexAgentPayloadKind::ConnectionInfo, false) => SIMPLEX_AGENT_E2E_CONN_INFO_LENGTH,
            (SimplexAgentPayloadKind::Message, true) => SIMPLEX_AGENT_E2E_MESSAGE_PQ_LENGTH,
            (SimplexAgentPayloadKind::Message, false) => SIMPLEX_AGENT_E2E_MESSAGE_LENGTH,
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

fn agent_x3dh_keypair(
    keypair: RadrootsSimplexOfficialX448Keypair,
) -> RadrootsSimplexAgentX3dhKeypair {
    RadrootsSimplexAgentX3dhKeypair {
        public_key: keypair.public_key,
        private_key: keypair.private_key,
    }
}

fn official_x3dh_keypair_from_agent(
    keypair: RadrootsSimplexAgentX3dhKeypair,
) -> RadrootsSimplexOfficialX448Keypair {
    RadrootsSimplexOfficialX448Keypair {
        public_key: keypair.public_key,
        private_key: keypair.private_key,
    }
}

fn agent_pq_keypair(
    keypair: RadrootsSimplexOfficialSntrup761Keypair,
) -> RadrootsSimplexAgentPqKeypair {
    RadrootsSimplexAgentPqKeypair {
        public_key: keypair.public_key,
        private_key: keypair.private_key,
    }
}

fn official_pq_keypair_from_agent(
    keypair: RadrootsSimplexAgentPqKeypair,
) -> RadrootsSimplexOfficialSntrup761Keypair {
    RadrootsSimplexOfficialSntrup761Keypair {
        public_key: keypair.public_key,
        private_key: keypair.private_key,
    }
}

fn official_x3dh_params_from_parts(
    key_1: &[u8],
    key_2: &[u8],
    pq_public_key: Option<Vec<u8>>,
    pq_ciphertext: Option<Vec<u8>>,
) -> Result<RadrootsSimplexOfficialX3dhParams, RadrootsSimplexAgentRuntimeError> {
    Ok(RadrootsSimplexOfficialX3dhParams {
        version_range: RadrootsSimplexSmpVersionRange::new(
            RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION,
            RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
        )
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?,
        key_1: key_1.to_vec(),
        key_2: key_2.to_vec(),
        pq_public_key,
        pq_ciphertext,
    })
}

fn prepare_short_invitation_link_data(
    invitation: &RadrootsSimplexAgentConnectionLink,
) -> Result<SimplexPreparedShortInvitationLinkData, RadrootsSimplexAgentRuntimeError> {
    let root_keypair = RadrootsSimplexSmpEd25519Keypair::generate()
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
    let fixed_data = encode_short_invitation_fixed_data(&root_keypair.public_key, invitation)?;
    let user_data = encode_short_invitation_user_data(invitation);
    let (link_key, signed_link_data) = sign_short_link_data(&root_keypair, &fixed_data, &user_data)
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
    let link_data_key = derive_invitation_short_link_data_key(&link_key)
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
    let encrypted_link_data = encrypt_short_link_data(&link_data_key, &signed_link_data)
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))?;
    Ok(SimplexPreparedShortInvitationLinkData {
        link_key,
        link_public_signature_key: root_keypair.public_key,
        link_private_signature_key: root_keypair.private_key,
        encrypted_link_data,
    })
}

fn short_invitation_server(
    invitation: &RadrootsSimplexAgentShortInvitationLink,
) -> Result<RadrootsSimplexSmpServerAddress, RadrootsSimplexAgentRuntimeError> {
    let server_identity = invitation.hosts.first().cloned().ok_or_else(|| {
        RadrootsSimplexAgentRuntimeError::Runtime(
            "SimpleX short invitation link does not include a relay host".into(),
        )
    })?;
    Ok(RadrootsSimplexSmpServerAddress {
        server_identity,
        hosts: invitation.hosts.clone(),
        port: invitation.port,
    })
}

fn correlation_id_for_command(command_id: u64) -> RadrootsSimplexSmpCorrelationId {
    let digest = derive_material(b"simplex-command-correlation", &[&command_id.to_be_bytes()]);
    let mut correlation = [0_u8; RadrootsSimplexSmpCorrelationId::LENGTH];
    correlation.copy_from_slice(&digest[..RadrootsSimplexSmpCorrelationId::LENGTH]);
    RadrootsSimplexSmpCorrelationId::new(correlation)
}

fn encode_queue_public_key(public_key: &[u8]) -> Result<String, RadrootsSimplexSmpCryptoError> {
    Ok(URL_SAFE.encode(encode_x25519_public_key_x509(public_key)?))
}

fn decode_queue_public_key(encoded: &str) -> Result<Vec<u8>, RadrootsSimplexAgentRuntimeError> {
    let bytes = URL_SAFE
        .decode(encoded.as_bytes())
        .or_else(|_| URL_SAFE_NO_PAD.decode(encoded.as_bytes()))
        .map_err(|error| {
            RadrootsSimplexAgentRuntimeError::Runtime(format!(
                "failed to decode SimpleX queue E2E public key: {error}"
            ))
        })?;
    decode_x25519_public_key_x509(&bytes)
        .map_err(|error| RadrootsSimplexAgentRuntimeError::Runtime(error.to_string()))
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
        | RadrootsSimplexAgentPendingCommandKind::GetQueueMessage { queue }
        | RadrootsSimplexAgentPendingCommandKind::AckInboxMessage { queue, .. }
        | RadrootsSimplexAgentPendingCommandKind::SetQueueLinkData { queue, .. } => {
            Some(queue.clone())
        }
        RadrootsSimplexAgentPendingCommandKind::RotateQueues { descriptors } => descriptors
            .first()
            .map(RadrootsSimplexAgentQueueDescriptor::queue_address),
        RadrootsSimplexAgentPendingCommandKind::TestQueues { queues } => queues.first().cloned(),
        RadrootsSimplexAgentPendingCommandKind::SecureGetQueueLinkData { .. }
        | RadrootsSimplexAgentPendingCommandKind::GetQueueLinkData { .. } => None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::VecDeque;
    use radroots_simplex_smp_crypto::prelude::{
        RadrootsSimplexSmpQueueAuthorizationMaterial, RadrootsSimplexSmpQueueAuthorizationScope,
        RadrootsSimplexSmpX25519Keypair,
    };
    use radroots_simplex_smp_proto::prelude::{
        RadrootsSimplexSmpBrokerTransmission, RadrootsSimplexSmpError,
        RadrootsSimplexSmpQueueIdsResponse, RadrootsSimplexSmpVersionRange,
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

    fn reply_descriptor() -> RadrootsSimplexAgentQueueDescriptor {
        RadrootsSimplexAgentQueueDescriptor {
            queue_uri: reply_queue(),
            replaced_queue: None,
            primary: true,
            sender_key: None,
        }
    }

    fn hello_message(message_id: u64) -> RadrootsSimplexAgentDecryptedMessage {
        RadrootsSimplexAgentDecryptedMessage::Message(RadrootsSimplexAgentMessageFrame {
            header: RadrootsSimplexAgentMessageHeader {
                message_id,
                previous_message_hash: Vec::new(),
            },
            message: RadrootsSimplexAgentMessage::Hello,
            padding: Vec::new(),
        })
    }

    fn user_message_frame(
        message_id: u64,
        previous_message_hash: Vec<u8>,
        body: &[u8],
    ) -> RadrootsSimplexAgentMessageFrame {
        RadrootsSimplexAgentMessageFrame {
            header: RadrootsSimplexAgentMessageHeader {
                message_id,
                previous_message_hash,
            },
            message: RadrootsSimplexAgentMessage::UserMessage(body.to_vec()),
            padding: Vec::new(),
        }
    }

    fn agent_message_hash(frame: &RadrootsSimplexAgentMessageFrame) -> Vec<u8> {
        let encoded = encode_decrypted_message(&RadrootsSimplexAgentDecryptedMessage::Message(
            frame.clone(),
        ))
        .unwrap();
        Sha256::digest(&encoded).to_vec()
    }

    fn mark_connected(runtime: &mut RadrootsSimplexAgentRuntime, connection_id: &str) {
        runtime
            .store
            .set_status(
                connection_id,
                RadrootsSimplexAgentConnectionStatus::Connected,
            )
            .unwrap();
    }

    fn initialize_test_outbound_official_ratchet(
        runtime: &mut RadrootsSimplexAgentRuntime,
        connection_id: &str,
    ) {
        let local_key_1 = official_x448_keypair_from_seed(b"rr-synth-runtime-test-local-x3dh-1");
        let local_key_2 = official_x448_keypair_from_seed(b"rr-synth-runtime-test-local-x3dh-2");
        let remote_key_1 = official_x448_keypair_from_seed(b"rr-synth-runtime-test-remote-x3dh-1");
        let remote_key_2 = official_x448_keypair_from_seed(b"rr-synth-runtime-test-remote-x3dh-2");
        let remote_params = RadrootsSimplexOfficialX3dhParams {
            version_range: RadrootsSimplexSmpVersionRange::new(
                RADROOTS_SIMPLEX_OFFICIAL_E2E_KDF_VERSION,
                RADROOTS_SIMPLEX_OFFICIAL_E2E_CURRENT_VERSION,
            )
            .unwrap(),
            key_1: remote_key_1.public_key,
            key_2: remote_key_2.public_key.clone(),
            pq_public_key: None,
            pq_ciphertext: None,
        };
        let sender_init =
            official_x3dh_sender_init(&local_key_1, &local_key_2, &remote_params).unwrap();
        let mut ratchet = RadrootsSimplexSmpRatchetState::responder(
            local_key_2.public_key,
            remote_key_2.public_key,
            None,
        )
        .unwrap();
        ratchet
            .initialize_official_sender(local_key_2.private_key, sender_init)
            .unwrap();
        runtime
            .store
            .connection_mut(connection_id)
            .unwrap()
            .ratchet_state = Some(ratchet);
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
            link_id: Some(synthetic_link_id(seed)),
            service_id: None,
            server_notification_credentials: None,
        })
    }

    fn synthetic_link_id(seed: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(b"rr-synth-runtime-link-id");
        hasher.update(seed);
        let digest = hasher.finalize();
        digest[..24].to_vec()
    }

    #[derive(Default)]
    struct ScriptedTransport {
        responses: VecDeque<RadrootsSimplexSmpBrokerMessage>,
        subscription_responses: VecDeque<RadrootsSimplexSmpBrokerTransmission>,
        requests: Vec<RadrootsSimplexSmpTransportRequest>,
        subscription_requests: Vec<RadrootsSimplexSmpSubscriptionReceiveRequest>,
    }

    impl ScriptedTransport {
        fn with_responses(responses: Vec<RadrootsSimplexSmpBrokerMessage>) -> Self {
            Self {
                responses: responses.into(),
                subscription_responses: VecDeque::new(),
                requests: Vec::new(),
                subscription_requests: Vec::new(),
            }
        }

        fn with_subscription_responses(
            responses: Vec<RadrootsSimplexSmpBrokerTransmission>,
        ) -> Self {
            Self {
                responses: VecDeque::new(),
                subscription_responses: responses.into(),
                requests: Vec::new(),
                subscription_requests: Vec::new(),
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

    impl RadrootsSimplexSmpSubscriptionTransport for ScriptedTransport {
        fn receive_subscription(
            &mut self,
            request: RadrootsSimplexSmpSubscriptionReceiveRequest,
        ) -> Result<Option<RadrootsSimplexSmpTransportResponse>, Self::Error> {
            self.subscription_requests.push(request.clone());
            let Some(response_transmission) = self.subscription_responses.pop_front() else {
                return Ok(None);
            };
            let response_block = RadrootsSimplexSmpTransportBlock::from_broker_transmissions(
                &[response_transmission.clone()],
                RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            )
            .map_err(|error| error.to_string())?;
            let response_encoded = response_block.encode().map_err(|error| error.to_string())?;
            Ok(Some(RadrootsSimplexSmpTransportResponse {
                server: request.server,
                transport_version: RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
                transmission: response_transmission,
                transport_hash: Sha256::digest(&response_encoded).to_vec(),
            }))
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
        let RadrootsSimplexSmpCommand::New(create_request) = &transport.requests[0].command else {
            panic!("first request should create the invitation queue");
        };
        let Some(RadrootsSimplexSmpQueueRequestData::Messaging(Some(link_request))) =
            create_request.queue_request_data.as_ref()
        else {
            panic!("invitation NEW should carry short-link messaging data");
        };
        assert!(!link_request.sender_id.is_empty());
        assert!(!link_request.link_data.fixed_data.is_empty());
        assert!(!link_request.link_data.user_data.is_empty());
        assert!(matches!(
            transport.requests[3].command,
            RadrootsSimplexSmpCommand::Sub
        ));
        assert_eq!(transport.requests[3].entity_id, b"recipient".to_vec());
        assert!(matches!(
            transport.requests[4].command,
            RadrootsSimplexSmpCommand::Sub
        ));
        assert_eq!(transport.requests[4].entity_id, b"recipient-2".to_vec());
        assert!(
            !transport
                .requests
                .iter()
                .any(|request| matches!(request.command, RadrootsSimplexSmpCommand::Get))
        );
        let events = runtime.drain_events(16);
        let Some(RadrootsSimplexAgentRuntimeEvent::InvitationReady { invitation, .. }) =
            events.first()
        else {
            panic!("runtime should emit a short invitation event");
        };
        let rendered = invitation.render().unwrap();
        assert!(rendered.starts_with("simplex:/i#"));
        assert_eq!(
            radroots_simplex_agent_proto::prelude::parse_short_invitation_link(&rendered).unwrap(),
            invitation.clone()
        );
        let short_link = runtime
            .store
            .connection(&created)
            .unwrap()
            .short_link
            .as_ref()
            .unwrap();
        assert_eq!(short_link.link_id, synthetic_link_id(b"server-dh"));
        let link_data_key = derive_invitation_short_link_data_key(&short_link.link_key).unwrap();
        let stored_link_data = RadrootsSimplexSmpQueueLinkData {
            fixed_data: short_link.encrypted_fixed_data.clone().unwrap(),
            user_data: short_link.encrypted_user_data.clone().unwrap(),
        };
        let verified = radroots_simplex_smp_crypto::prelude::decrypt_verify_short_link_data(
            &short_link.link_key,
            &link_data_key,
            &short_link.link_public_signature_key,
            &stored_link_data,
        )
        .unwrap();
        let decoded = radroots_simplex_agent_proto::prelude::decode_short_invitation_fixed_data(
            &verified.fixed_data,
        )
        .unwrap();
        assert_eq!(
            decoded.root_public_signature_key,
            short_link.link_public_signature_key
        );
        assert_eq!(
            decoded.invitation.connection_id,
            created.as_bytes().to_vec()
        );
        assert_eq!(verified.user_data, created.as_bytes().to_vec());
        let decrypted_invitation =
            decrypt_short_invitation_link_data(invitation, &stored_link_data).unwrap();
        assert_eq!(
            decrypted_invitation.connection_id,
            created.as_bytes().to_vec()
        );
        assert_eq!(
            runtime.store.connection(&joined).unwrap().status,
            RadrootsSimplexAgentConnectionStatus::JoinPending
        );
    }

    #[test]
    fn join_short_invitation_retrieves_link_data_and_continues_join() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let created = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
        let mut setup_transport = ScriptedTransport::with_responses(vec![
            ids_response(b"recipient", b"sender", b"server-dh"),
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut setup_transport, 30, 16)
            .unwrap();
        let events = runtime.drain_events(16);
        let Some(RadrootsSimplexAgentRuntimeEvent::InvitationReady {
            invitation: short_invitation,
            ..
        }) = events.first()
        else {
            panic!("runtime should emit a short invitation event");
        };
        let short_link = runtime
            .store
            .connection(&created)
            .unwrap()
            .short_link
            .as_ref()
            .unwrap();
        let stored_link_data = RadrootsSimplexSmpQueueLinkData {
            fixed_data: short_link.encrypted_fixed_data.clone().unwrap(),
            user_data: short_link.encrypted_user_data.clone().unwrap(),
        };
        let joined = runtime
            .join_short_invitation(short_invitation.clone(), reply_queue(), 40)
            .unwrap();
        let mut join_transport = ScriptedTransport::with_responses(vec![
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Lnk {
                sender_id: b"sender".to_vec(),
                link_data: stored_link_data,
            },
            RadrootsSimplexSmpBrokerMessage::Ok,
            ids_response(b"recipient-2", b"sender-2", b"server-dh-2"),
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut join_transport, 50, 16)
            .unwrap();

        assert_eq!(join_transport.requests.len(), 6);
        let RadrootsSimplexSmpCommand::LKey(link_key) = &join_transport.requests[0].command else {
            panic!("short invitation join should authorize link retrieval first");
        };
        assert_eq!(link_key, &short_invitation.link_key);
        assert_eq!(
            join_transport.requests[0].entity_id,
            short_invitation.link_id.clone()
        );
        assert!(matches!(
            join_transport.requests[1].command,
            RadrootsSimplexSmpCommand::LGet
        ));
        let RadrootsSimplexSmpCommand::SKey(_) = &join_transport.requests[2].command else {
            panic!("short invitation join should secure the invitation send queue");
        };
        let RadrootsSimplexSmpCommand::New(_) = &join_transport.requests[3].command else {
            panic!("short invitation join should create the reply queue");
        };
        let joined_connection = runtime.store.connection(&joined).unwrap();
        assert_eq!(
            joined_connection.status,
            RadrootsSimplexAgentConnectionStatus::JoinPending
        );
        assert_eq!(
            joined_connection.invitation.as_ref().unwrap().connection_id,
            created.as_bytes().to_vec()
        );
        assert_eq!(
            runtime
                .store
                .primary_send_queue(&joined)
                .unwrap()
                .descriptor
                .queue_uri
                .sender_id,
            URL_SAFE_NO_PAD.encode(b"sender")
        );
        assert!(runtime.drain_events(16).iter().any(|event| matches!(
            event,
            RadrootsSimplexAgentRuntimeEvent::ConfirmationRequired { connection_id }
                if connection_id == &joined
        )));
    }

    #[test]
    fn join_confirmation_carries_sender_x3dh_params() {
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
        ]);
        runtime
            .execute_ready_commands(&mut transport, 30, 3)
            .unwrap();
        let local_key_1 = runtime
            .store
            .connection(&joined)
            .unwrap()
            .local_x3dh_key_1
            .clone()
            .unwrap();
        let local_key_2 = runtime
            .store
            .connection(&joined)
            .unwrap()
            .local_x3dh_key_2
            .clone()
            .unwrap();
        let local_pq_keypair = runtime
            .store
            .connection(&joined)
            .unwrap()
            .local_pq_keypair
            .clone()
            .unwrap();
        let ready = runtime.retry_pending(30, 16);
        let confirmation_params = ready
            .into_iter()
            .find_map(|command| match command.kind {
                RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                    envelope:
                        RadrootsSimplexAgentEnvelope::Confirmation {
                            reply_queue: true,
                            e2e_ratchet_params: Some(params),
                            ..
                        },
                    ..
                } => Some(params),
                _ => None,
            })
            .unwrap();

        assert_eq!(confirmation_params.key_1, local_key_1.public_key);
        assert_eq!(confirmation_params.key_2, local_key_2.public_key);
        assert_eq!(
            confirmation_params.pq_public_key,
            Some(local_pq_keypair.public_key)
        );
        assert!(confirmation_params.pq_ciphertext.is_some());
    }

    #[test]
    fn confirmation_params_initialize_receiver_ratchet() {
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
        let joined_connection = runtime.store.connection(&joined).unwrap();
        let joined_key_1 = joined_connection.local_x3dh_key_1.as_ref().unwrap();
        let joined_key_2 = joined_connection.local_x3dh_key_2.as_ref().unwrap();
        let joined_ratchet = joined_connection.ratchet_state.as_ref().unwrap();
        let e2e_ratchet_params = official_x3dh_params_from_parts(
            &joined_key_1.public_key,
            &joined_key_2.public_key,
            joined_ratchet.current_pq_public_key.clone(),
            joined_ratchet.pending_outbound_pq_ciphertext.clone(),
        )
        .unwrap();
        let envelope = RadrootsSimplexAgentEnvelope::Confirmation {
            reply_queue: true,
            e2e_ratchet_params: Some(e2e_ratchet_params),
            encrypted: RadrootsSimplexAgentEncryptedPayload {
                ratchet_header: None,
                official_message: Some(Vec::new()),
                ciphertext: Vec::new(),
            },
        };

        runtime
            .initialize_receiver_ratchet_from_confirmation(&created, &envelope)
            .unwrap();
        let mut sender_ratchet = runtime
            .store
            .connection(&joined)
            .unwrap()
            .ratchet_state
            .clone()
            .unwrap();
        let encrypted = sender_ratchet
            .encrypt_official_payload(&[0_u8; 32], b"reply-info", 96)
            .unwrap();
        let receiver_ratchet = runtime
            .store
            .connection_mut(&created)
            .unwrap()
            .ratchet_state
            .as_mut()
            .unwrap();
        let decrypted = receiver_ratchet
            .decrypt_official_payload(&[0_u8; 32], &encrypted)
            .unwrap();

        assert_eq!(decrypted, b"reply-info");
        assert!(receiver_ratchet.official_sending_chain_key.is_some());
        assert!(receiver_ratchet.official_receiving_chain_key.is_some());
    }

    #[test]
    fn explicit_get_connection_message_executes_smp_get() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let created = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();

        let mut setup_transport = ScriptedTransport::with_responses(vec![
            ids_response(b"recipient", b"sender", b"server-dh"),
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut setup_transport, 30, 16)
            .unwrap();
        assert!(matches!(
            setup_transport.requests[1].command,
            RadrootsSimplexSmpCommand::Sub
        ));
        assert_eq!(setup_transport.requests[1].entity_id, b"recipient".to_vec());
        assert!(runtime.store.receive_queues(&created).unwrap()[0].subscribed);

        runtime.get_connection_message(&created, 40).unwrap();
        let mut get_transport =
            ScriptedTransport::with_responses(vec![RadrootsSimplexSmpBrokerMessage::Ok]);
        runtime
            .execute_ready_commands(&mut get_transport, 50, 16)
            .unwrap();

        assert_eq!(get_transport.requests.len(), 1);
        assert!(matches!(
            get_transport.requests[0].command,
            RadrootsSimplexSmpCommand::Get
        ));
        assert_eq!(get_transport.requests[0].entity_id, b"recipient".to_vec());
        assert!(runtime.store.receive_queues(&created).unwrap()[0].subscribed);
    }

    #[test]
    fn subscription_receive_routes_broker_transmission_by_entity_id() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let created = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();

        let mut setup_transport = ScriptedTransport::with_responses(vec![
            ids_response(b"recipient", b"sender", b"server-dh"),
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut setup_transport, 30, 16)
            .unwrap();
        let receive_queue = runtime.store.receive_queues(&created).unwrap()[0].clone();
        let _ = runtime.drain_events(16);

        let mut subscription_transport = ScriptedTransport::with_subscription_responses(vec![
            RadrootsSimplexSmpBrokerTransmission {
                authorization: Vec::new(),
                correlation_id: None,
                entity_id: receive_queue.entity_id,
                message: RadrootsSimplexSmpBrokerMessage::Err(RadrootsSimplexSmpError::NoMsg),
            },
        ]);
        runtime
            .receive_subscription_messages(&mut subscription_transport, 4)
            .unwrap();

        assert_eq!(subscription_transport.subscription_requests.len(), 2);
        assert_eq!(
            subscription_transport.subscription_requests[0].server,
            receive_queue.descriptor.queue_uri.server
        );
        assert!(matches!(
            runtime.drain_events(16).first(),
            Some(RadrootsSimplexAgentRuntimeEvent::Error {
                connection_id: Some(connection_id),
                message,
            }) if connection_id == &created && message.contains("NoMsg")
        ));
    }

    #[test]
    fn inbound_progress_accepts_exact_duplicate_for_latest_ack_target() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let connection_id = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
        mark_connected(&mut runtime, &connection_id);
        let first_queue = reply_descriptor().queue_address();
        let second_queue = RadrootsSimplexAgentQueueAddress {
            server: first_queue.server.clone(),
            sender_id: b"second-duplicate-broker".to_vec(),
        };
        let frame = user_message_frame(1, Vec::new(), b"first");
        let frame_hash = agent_message_hash(&frame);

        runtime
            .validate_inbound_frame_progress(&connection_id, &frame, &frame_hash)
            .unwrap();
        runtime
            .store
            .record_inbound_message(
                &connection_id,
                first_queue,
                b"first-broker-message".to_vec(),
                frame.header.message_id,
                frame_hash.clone(),
            )
            .unwrap();
        runtime
            .validate_inbound_frame_progress(&connection_id, &frame, &frame_hash)
            .unwrap();
        runtime
            .store
            .record_inbound_message(
                &connection_id,
                second_queue.clone(),
                b"second-broker-message".to_vec(),
                frame.header.message_id,
                frame_hash,
            )
            .unwrap();

        assert_eq!(
            runtime
                .store
                .inbound_ack_target(&connection_id, 1, &agent_message_hash(&frame))
                .unwrap(),
            Some((second_queue, b"second-broker-message".to_vec()))
        );
    }

    #[test]
    fn inbound_progress_rejects_gap_and_previous_hash_mismatch() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let connection_id = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
        mark_connected(&mut runtime, &connection_id);
        let queue = reply_descriptor().queue_address();
        let first_frame = user_message_frame(1, Vec::new(), b"first");
        let first_hash = agent_message_hash(&first_frame);
        runtime
            .store
            .record_inbound_message(
                &connection_id,
                queue,
                b"first-broker-message".to_vec(),
                first_frame.header.message_id,
                first_hash.clone(),
            )
            .unwrap();

        let gap_frame = user_message_frame(3, first_hash.clone(), b"gap");
        let gap_error = runtime
            .validate_inbound_frame_progress(
                &connection_id,
                &gap_frame,
                &agent_message_hash(&gap_frame),
            )
            .unwrap_err();
        assert!(gap_error.to_string().contains("skipped expected `2`"));

        let mismatch_frame = user_message_frame(2, b"wrong-previous-hash".to_vec(), b"second");
        let mismatch_error = runtime
            .validate_inbound_frame_progress(
                &connection_id,
                &mismatch_frame,
                &agent_message_hash(&mismatch_frame),
            )
            .unwrap_err();
        assert!(
            mismatch_error
                .to_string()
                .contains("unexpected previous-message hash")
        );
    }

    #[test]
    fn inbound_progress_rejects_regression_after_accepted_next_message() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let connection_id = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
        mark_connected(&mut runtime, &connection_id);
        let queue = reply_descriptor().queue_address();
        let first_frame = user_message_frame(1, Vec::new(), b"first");
        let first_hash = agent_message_hash(&first_frame);
        let second_frame = user_message_frame(2, first_hash.clone(), b"second");
        let second_hash = agent_message_hash(&second_frame);
        runtime
            .store
            .record_inbound_message(
                &connection_id,
                queue.clone(),
                b"first-broker-message".to_vec(),
                first_frame.header.message_id,
                first_hash,
            )
            .unwrap();
        runtime
            .validate_inbound_frame_progress(&connection_id, &second_frame, &second_hash)
            .unwrap();
        runtime
            .store
            .record_inbound_message(
                &connection_id,
                queue,
                b"second-broker-message".to_vec(),
                second_frame.header.message_id,
                second_hash,
            )
            .unwrap();

        let regression_frame = user_message_frame(1, Vec::new(), b"first");
        let regression_error = runtime
            .validate_inbound_frame_progress(
                &connection_id,
                &regression_frame,
                &agent_message_hash(&regression_frame),
            )
            .unwrap_err();
        assert!(regression_error.to_string().contains("regressed below `2`"));
    }

    #[test]
    fn send_message_requires_connected_state() {
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

        let error = runtime
            .send_message(&joined, b"blocked before connected".to_vec(), 30)
            .unwrap_err();
        assert!(error.to_string().contains("is not connected"));
    }

    #[test]
    fn allow_and_hello_lifecycle_reaches_connected() {
        let mut runtime = RadrootsSimplexAgentRuntimeBuilder::new().build().unwrap();
        let created = runtime
            .create_connection(invitation_queue(), b"e2e".to_vec(), false, 10)
            .unwrap();
        let mut setup_transport = ScriptedTransport::with_responses(vec![
            ids_response(b"recipient", b"sender", b"server-dh"),
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut setup_transport, 30, 16)
            .unwrap();
        runtime
            .store
            .connection_mut(&created)
            .unwrap()
            .shared_secret = Some(vec![3_u8; 32]);

        runtime
            .handle_inbound_decrypted_message(
                &created,
                RadrootsSimplexAgentDecryptedMessage::ConnectionInfoReply {
                    reply_queues: vec![reply_descriptor()],
                    info: b"peer-info".to_vec(),
                },
                b"reply-confirmation".to_vec(),
            )
            .unwrap();
        assert_eq!(
            runtime.store.connection(&created).unwrap().status,
            RadrootsSimplexAgentConnectionStatus::AwaitingApproval
        );
        initialize_test_outbound_official_ratchet(&mut runtime, &created);

        runtime
            .allow_connection(&created, b"local-info".to_vec(), 40)
            .unwrap();
        let mut allow_transport = ScriptedTransport::with_responses(vec![
            RadrootsSimplexSmpBrokerMessage::Ok,
            RadrootsSimplexSmpBrokerMessage::Ok,
        ]);
        runtime
            .execute_ready_commands(&mut allow_transport, 50, 16)
            .unwrap();
        assert!(matches!(
            allow_transport.requests[0].command,
            RadrootsSimplexSmpCommand::SKey(_)
        ));
        assert!(matches!(
            allow_transport.requests[1].command,
            RadrootsSimplexSmpCommand::Send(_)
        ));
        assert!(!runtime.store.connection(&created).unwrap().hello_sent);

        runtime
            .handle_inbound_decrypted_message(&created, hello_message(1), b"hello-in".to_vec())
            .unwrap();
        let connection = runtime.store.connection(&created).unwrap();
        assert_eq!(
            connection.status,
            RadrootsSimplexAgentConnectionStatus::Connected
        );
        assert!(connection.hello_sent);
        assert!(connection.hello_received);
        assert!(runtime.drain_events(16).into_iter().any(|event| matches!(
            event,
            RadrootsSimplexAgentRuntimeEvent::ConnectionEstablished { connection_id }
                if connection_id == created
        )));

        let mut hello_transport =
            ScriptedTransport::with_responses(vec![RadrootsSimplexSmpBrokerMessage::Ok]);
        runtime
            .execute_ready_commands(&mut hello_transport, 60, 16)
            .unwrap();
        assert_eq!(hello_transport.requests.len(), 1);
        assert!(matches!(
            hello_transport.requests[0].command,
            RadrootsSimplexSmpCommand::Send(_)
        ));
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
        mark_connected(&mut runtime, &joined);

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
        assert!(runtime.drain_events(64).into_iter().any(|event| matches!(
            event,
            RadrootsSimplexAgentRuntimeEvent::OutboundMessageDelivered {
                connection_id,
                message_id: 1,
                message_hash,
            } if connection_id == joined && !message_hash.is_empty()
        )));
    }

    #[test]
    fn send_message_stores_opaque_encrypted_agent_payload() {
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
        mark_connected(&mut runtime, &joined);

        runtime
            .send_message(&joined, b"hello simplex".to_vec(), 40)
            .unwrap();
        let command = runtime.retry_pending(40, 16).remove(0);
        let RadrootsSimplexAgentPendingCommandKind::SendEnvelope { envelope, .. } = command.kind
        else {
            panic!("expected send envelope command");
        };
        let RadrootsSimplexAgentEnvelope::Message(encrypted) = envelope else {
            panic!("expected encrypted message envelope");
        };
        let expected_plaintext = encode_decrypted_message(
            &RadrootsSimplexAgentDecryptedMessage::Message(RadrootsSimplexAgentMessageFrame {
                header: RadrootsSimplexAgentMessageHeader {
                    message_id: 1,
                    previous_message_hash: Vec::new(),
                },
                message: RadrootsSimplexAgentMessage::UserMessage(b"hello simplex".to_vec()),
                padding: Vec::new(),
            }),
        )
        .unwrap();

        assert!(encrypted.ratchet_header.is_none());
        assert!(encrypted.ciphertext.is_empty());
        let official_message = encrypted.official_message.as_ref().unwrap();
        assert_ne!(official_message, &expected_plaintext);
        assert_eq!(
            official_message.len(),
            2 + 124 + 16 + SIMPLEX_AGENT_E2E_MESSAGE_LENGTH
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
        mark_connected(&mut runtime, &joined);

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
        mark_connected(&mut runtime, &joined);

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

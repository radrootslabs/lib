use crate::error::RadrootsSimplexAgentStoreError;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
#[cfg(feature = "std")]
use alloc::string::ToString;
#[cfg(feature = "std")]
use alloc::sync::Arc;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
#[cfg(feature = "std")]
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
#[cfg(feature = "std")]
use getrandom::getrandom;
#[cfg(feature = "std")]
use radroots_protected_store::file::{
    RADROOTS_PROTECTED_FILE_SECRET_SUFFIX, RADROOTS_PROTECTED_FILE_WRAPPING_KEY_FILE,
};
#[cfg(feature = "std")]
use radroots_protected_store::{
    RADROOTS_PROTECTED_STORE_KEY_LENGTH, RADROOTS_PROTECTED_STORE_NONCE_LENGTH,
    RadrootsProtectedFileKeySource, RadrootsProtectedStoreEnvelope, sidecar_path,
};
#[cfg(all(feature = "std", feature = "os-keyring"))]
use radroots_secret_vault::RadrootsSecretVaultOsKeyring;
#[cfg(feature = "std")]
use radroots_secret_vault::{
    RadrootsSecretKeyWrapping, RadrootsSecretVault, RadrootsSecretVaultAccessError,
};
use radroots_simplex_agent_proto::prelude::{
    RadrootsSimplexAgentConnectionLink, RadrootsSimplexAgentConnectionMode,
    RadrootsSimplexAgentConnectionStatus, RadrootsSimplexAgentEnvelope,
    RadrootsSimplexAgentMessageId, RadrootsSimplexAgentMessageReceipt,
    RadrootsSimplexAgentQueueAddress, RadrootsSimplexAgentQueueDescriptor,
    RadrootsSimplexAgentShortInvitationLink, RadrootsSimplexAgentShortLinkScheme,
    RadrootsSimplexSmpRatchetState,
};
#[cfg(feature = "std")]
use radroots_simplex_agent_proto::prelude::{
    decode_connection_link, decode_envelope, encode_connection_link, encode_envelope,
};
use radroots_simplex_smp_crypto::prelude::RadrootsSimplexSmpEd25519Keypair;
#[cfg(feature = "std")]
use radroots_simplex_smp_crypto::prelude::{
    RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH, RadrootsSimplexSmpSkippedMessageKey,
};
use radroots_simplex_smp_proto::prelude::{
    RadrootsSimplexSmpQueueLinkData, RadrootsSimplexSmpQueueUri, RadrootsSimplexSmpServerAddress,
};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use sha2::{Digest, Sha256};
#[cfg(feature = "std")]
use std::ffi::OsString;
#[cfg(feature = "std")]
use std::fs;
#[cfg(feature = "std")]
use std::io::Write;
#[cfg(feature = "std")]
use std::path::{Path, PathBuf};
#[cfg(feature = "std")]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(feature = "std")]
use zeroize::Zeroize;

#[cfg(feature = "std")]
const RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SECRETS_VERSION: u8 = 1;
#[cfg(feature = "std")]
const RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SECRETS_KEY_SLOT: &str =
    "radroots_simplex_agent_store_secrets";
#[cfg(feature = "std")]
const RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SNAPSHOT_KEY_SLOT: &str =
    "radroots_simplex_agent_store_snapshot";
#[cfg(feature = "std")]
const RADROOTS_SIMPLEX_AGENT_STORE_VAULT_MASTER_KEY_BYTES: usize =
    RADROOTS_PROTECTED_STORE_KEY_LENGTH;
#[cfg(feature = "std")]
const RADROOTS_SIMPLEX_AGENT_STORE_WRAPPED_KEY_VERSION: u8 = 1;
#[cfg(all(feature = "std", feature = "os-keyring"))]
const RADROOTS_SIMPLEX_AGENT_STORE_KEYCHAIN_SERVICE: &str = "org.radroots.simplex.agent-store";

#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentStoreProtectedSecretsDiagnostics {
    pub store_path: PathBuf,
    pub protected_secrets_path: PathBuf,
    pub wrapping_key_path: PathBuf,
    pub public_snapshot_exists: bool,
    pub protected_secrets_configured: bool,
    pub protected_secrets_exists: bool,
    pub wrapping_key_exists: bool,
    pub protected_connection_count: usize,
    pub protected_pending_command_count: usize,
    pub protected_generation: Option<String>,
    pub protected_envelope_suffix: Option<String>,
    pub protected_wrapping_key_suffix: Option<String>,
}

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
    pub entity_id: Vec<u8>,
    pub role: RadrootsSimplexAgentQueueRole,
    pub subscribed: bool,
    pub primary: bool,
    pub tested: bool,
    pub auth_state: Option<RadrootsSimplexAgentQueueAuthState>,
    pub delivery_private_key: Option<Vec<u8>>,
    pub delivery_shared_secret: Option<Vec<u8>>,
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
    #[cfg_attr(
        feature = "std",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub inbound_queue: Option<RadrootsSimplexAgentRecentQueueAddress>,
    #[cfg_attr(
        feature = "std",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub inbound_broker_message_id: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RadrootsSimplexAgentRecentQueueAddress {
    pub server_identity: String,
    pub hosts: Vec<String>,
    pub port: Option<u16>,
    pub sender_id: Vec<u8>,
}

impl RadrootsSimplexAgentRecentQueueAddress {
    fn from_queue_address(queue: &RadrootsSimplexAgentQueueAddress) -> Self {
        Self {
            server_identity: queue.server.server_identity.clone(),
            hosts: queue.server.hosts.clone(),
            port: queue.server.port,
            sender_id: queue.sender_id.clone(),
        }
    }

    fn into_queue_address(self) -> RadrootsSimplexAgentQueueAddress {
        RadrootsSimplexAgentQueueAddress {
            server: RadrootsSimplexSmpServerAddress {
                server_identity: self.server_identity,
                hosts: self.hosts,
                port: self.port,
            },
            sender_id: self.sender_id,
        }
    }
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
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RadrootsSimplexAgentX3dhKeypair {
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RadrootsSimplexAgentPqKeypair {
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentShortLinkCredentials {
    pub scheme: RadrootsSimplexAgentShortLinkScheme,
    pub hosts: Vec<String>,
    pub port: Option<u16>,
    pub server_key_hash: Option<Vec<u8>>,
    pub link_id: Vec<u8>,
    pub link_key: Vec<u8>,
    pub link_public_signature_key: Vec<u8>,
    pub link_private_signature_key: Vec<u8>,
    pub encrypted_fixed_data: Option<Vec<u8>>,
    pub encrypted_user_data: Option<Vec<u8>>,
}

impl RadrootsSimplexAgentShortLinkCredentials {
    pub fn invitation_link(&self) -> RadrootsSimplexAgentShortInvitationLink {
        RadrootsSimplexAgentShortInvitationLink {
            scheme: self.scheme,
            hosts: self.hosts.clone(),
            port: self.port,
            server_key_hash: self.server_key_hash.clone(),
            link_id: self.link_id.clone(),
            link_key: self.link_key.clone(),
        }
    }
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
    GetQueueMessage {
        queue: RadrootsSimplexAgentQueueAddress,
    },
    AckInboxMessage {
        queue: RadrootsSimplexAgentQueueAddress,
        broker_message_id: Vec<u8>,
        receipt: RadrootsSimplexAgentMessageReceipt,
    },
    RotateQueues {
        descriptors: Vec<RadrootsSimplexAgentQueueDescriptor>,
    },
    TestQueues {
        queues: Vec<RadrootsSimplexAgentQueueAddress>,
    },
    SetQueueLinkData {
        queue: RadrootsSimplexAgentQueueAddress,
        link_id: Vec<u8>,
        link_data: RadrootsSimplexSmpQueueLinkData,
    },
    SecureGetQueueLinkData {
        invitation: RadrootsSimplexAgentShortInvitationLink,
        reply_queue: RadrootsSimplexSmpQueueUri,
    },
    GetQueueLinkData {
        invitation: RadrootsSimplexAgentShortInvitationLink,
        reply_queue: RadrootsSimplexSmpQueueUri,
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
    pub short_link: Option<RadrootsSimplexAgentShortLinkCredentials>,
    pub queues: Vec<RadrootsSimplexAgentQueueRecord>,
    pub ratchet_state: Option<RadrootsSimplexSmpRatchetState>,
    pub local_e2e_public_key: Option<Vec<u8>>,
    pub local_e2e_private_key: Option<Vec<u8>>,
    pub local_x3dh_key_1: Option<RadrootsSimplexAgentX3dhKeypair>,
    pub local_x3dh_key_2: Option<RadrootsSimplexAgentX3dhKeypair>,
    pub local_pq_keypair: Option<RadrootsSimplexAgentPqKeypair>,
    pub shared_secret: Option<Vec<u8>>,
    pub delivery_cursor: RadrootsSimplexAgentDeliveryCursor,
    pub last_received_queue: Option<RadrootsSimplexAgentQueueAddress>,
    pub last_received_broker_message_id: Option<Vec<u8>>,
    pub recent_messages: Vec<RadrootsSimplexAgentRecentMessageRecord>,
    pub staged_outbound_message: Option<RadrootsSimplexAgentOutboundMessage>,
    pub hello_sent: bool,
    pub hello_received: bool,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentStoreSnapshot {
    next_connection_sequence: u64,
    next_command_sequence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    protected_secrets: Option<RadrootsSimplexAgentStoreProtectedSecretsRef>,
    connections: Vec<RadrootsSimplexAgentConnectionSnapshot>,
    pending_commands: Vec<RadrootsSimplexAgentPendingCommandSnapshot>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentStoreProtectedSecretsRef {
    version: u8,
    generation: String,
    envelope_suffix: String,
    wrapping_key_suffix: String,
    key_slot: String,
    connection_count: usize,
    pending_command_count: usize,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentConnectionSnapshot {
    id: String,
    mode: String,
    status: String,
    invitation: Option<Vec<u8>>,
    short_link: Option<RadrootsSimplexAgentShortLinkCredentialsSnapshot>,
    queues: Vec<RadrootsSimplexAgentQueueRecordSnapshot>,
    ratchet_state: Option<RadrootsSimplexAgentRatchetStateSnapshot>,
    local_e2e_public_key: Option<Vec<u8>>,
    local_e2e_private_key: Option<Vec<u8>>,
    local_x3dh_key_1: Option<RadrootsSimplexAgentX3dhKeypair>,
    local_x3dh_key_2: Option<RadrootsSimplexAgentX3dhKeypair>,
    local_pq_keypair: Option<RadrootsSimplexAgentPqKeypair>,
    shared_secret: Option<Vec<u8>>,
    delivery_cursor: RadrootsSimplexAgentDeliveryCursor,
    last_received_queue: Option<RadrootsSimplexAgentQueueAddressSnapshot>,
    last_received_broker_message_id: Option<Vec<u8>>,
    recent_messages: Vec<RadrootsSimplexAgentRecentMessageRecord>,
    staged_outbound_message: Option<RadrootsSimplexAgentOutboundMessage>,
    hello_sent: bool,
    hello_received: bool,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentQueueRecordSnapshot {
    descriptor: RadrootsSimplexAgentQueueDescriptorSnapshot,
    entity_id: Vec<u8>,
    role: String,
    subscribed: bool,
    primary: bool,
    tested: bool,
    auth_state: Option<RadrootsSimplexAgentQueueAuthState>,
    delivery_private_key: Option<Vec<u8>>,
    delivery_shared_secret: Option<Vec<u8>>,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RadrootsSimplexAgentQueueAddressSnapshot {
    server_identity: String,
    hosts: Vec<String>,
    port: Option<u16>,
    sender_id: Vec<u8>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentShortLinkCredentialsSnapshot {
    scheme: String,
    hosts: Vec<String>,
    port: Option<u16>,
    server_key_hash: Option<Vec<u8>>,
    link_id: Vec<u8>,
    link_key: Vec<u8>,
    link_public_signature_key: Vec<u8>,
    link_private_signature_key: Vec<u8>,
    encrypted_fixed_data: Option<Vec<u8>>,
    encrypted_user_data: Option<Vec<u8>>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentShortInvitationLinkSnapshot {
    scheme: String,
    hosts: Vec<String>,
    port: Option<u16>,
    server_key_hash: Option<Vec<u8>>,
    link_id: Vec<u8>,
    link_key: Vec<u8>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentQueueLinkDataSnapshot {
    fixed_data: Vec<u8>,
    user_data: Vec<u8>,
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
    local_pq_private_key: Option<Vec<u8>>,
    local_dh_private_key: Option<Vec<u8>>,
    official_associated_data: Option<Vec<u8>>,
    official_root_key: Option<Vec<u8>>,
    official_sending_chain_key: Option<Vec<u8>>,
    official_receiving_chain_key: Option<Vec<u8>>,
    official_sending_header_key: Option<Vec<u8>>,
    official_receiving_header_key: Option<Vec<u8>>,
    official_next_sending_header_key: Option<Vec<u8>>,
    official_next_receiving_header_key: Option<Vec<u8>>,
    official_skipped_message_keys: Vec<RadrootsSimplexAgentSkippedMessageKeySnapshot>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentSkippedMessageKeySnapshot {
    header_key: Vec<u8>,
    message_number: u32,
    message_key: Vec<u8>,
    message_iv: Vec<u8>,
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
    GetQueueMessage {
        queue: RadrootsSimplexAgentQueueAddressSnapshot,
    },
    AckInboxMessage {
        queue: RadrootsSimplexAgentQueueAddressSnapshot,
        broker_message_id: Vec<u8>,
        receipt: RadrootsSimplexAgentMessageReceiptSnapshot,
    },
    RotateQueues {
        descriptors: Vec<RadrootsSimplexAgentQueueDescriptorSnapshot>,
    },
    TestQueues {
        queues: Vec<RadrootsSimplexAgentQueueAddressSnapshot>,
    },
    SetQueueLinkData {
        queue: RadrootsSimplexAgentQueueAddressSnapshot,
        link_id: Vec<u8>,
        link_data: RadrootsSimplexAgentQueueLinkDataSnapshot,
    },
    SecureGetQueueLinkData {
        invitation: RadrootsSimplexAgentShortInvitationLinkSnapshot,
        reply_queue: String,
    },
    GetQueueLinkData {
        invitation: RadrootsSimplexAgentShortInvitationLinkSnapshot,
        reply_queue: String,
    },
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentMessageReceiptSnapshot {
    message_id: RadrootsSimplexAgentMessageId,
    message_hash: Vec<u8>,
    receipt_info: Vec<u8>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentStoreSecretsSnapshot {
    version: u8,
    generation: String,
    connections: Vec<RadrootsSimplexAgentConnectionSecretsSnapshot>,
    pending_commands: Vec<RadrootsSimplexAgentPendingCommandSecretsSnapshot>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentConnectionSecretsSnapshot {
    id: String,
    short_link_link_key: Option<Vec<u8>>,
    short_link_private_signature_key: Option<Vec<u8>>,
    queues: Vec<RadrootsSimplexAgentQueueSecretsSnapshot>,
    ratchet_state: Option<RadrootsSimplexAgentRatchetSecretsSnapshot>,
    local_e2e_private_key: Option<Vec<u8>>,
    local_x3dh_key_1_private_key: Option<Vec<u8>>,
    local_x3dh_key_2_private_key: Option<Vec<u8>>,
    local_pq_private_key: Option<Vec<u8>>,
    shared_secret: Option<Vec<u8>>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentQueueSecretsSnapshot {
    entity_id: Vec<u8>,
    role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    queue_address: Option<RadrootsSimplexAgentQueueAddressSnapshot>,
    auth_private_key: Option<Vec<u8>>,
    delivery_private_key: Option<Vec<u8>>,
    delivery_shared_secret: Option<Vec<u8>>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentRatchetSecretsSnapshot {
    current_pq_shared_secret: Option<Vec<u8>>,
    local_pq_private_key: Option<Vec<u8>>,
    local_dh_private_key: Option<Vec<u8>>,
    official_root_key: Option<Vec<u8>>,
    official_sending_chain_key: Option<Vec<u8>>,
    official_receiving_chain_key: Option<Vec<u8>>,
    official_sending_header_key: Option<Vec<u8>>,
    official_receiving_header_key: Option<Vec<u8>>,
    official_next_sending_header_key: Option<Vec<u8>>,
    official_next_receiving_header_key: Option<Vec<u8>>,
    official_skipped_message_keys: Vec<RadrootsSimplexAgentSkippedMessageKeySnapshot>,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadrootsSimplexAgentPendingCommandSecretsSnapshot {
    id: u64,
    connection_id: String,
    short_invitation_link_key: Option<Vec<u8>>,
}

#[cfg(feature = "std")]
#[derive(Clone)]
enum RadrootsSimplexAgentStorePersistence {
    PublicSnapshot {
        path: PathBuf,
    },
    ProtectedSnapshot {
        path: PathBuf,
        key_source: RadrootsSimplexAgentStoreVaultKeySource,
    },
}

#[cfg(feature = "std")]
impl core::fmt::Debug for RadrootsSimplexAgentStorePersistence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PublicSnapshot { path } => f
                .debug_struct("PublicSnapshot")
                .field("path", path)
                .finish(),
            Self::ProtectedSnapshot { path, key_source } => f
                .debug_struct("ProtectedSnapshot")
                .field("path", path)
                .field("key_source", key_source)
                .finish(),
        }
    }
}

#[cfg(feature = "std")]
#[derive(Clone)]
struct RadrootsSimplexAgentStoreVaultKeySource {
    vault: Arc<dyn RadrootsSecretVault>,
    master_key_slot: String,
}

#[cfg(feature = "std")]
impl core::fmt::Debug for RadrootsSimplexAgentStoreVaultKeySource {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RadrootsSimplexAgentStoreVaultKeySource")
            .field("master_key_slot", &self.master_key_slot)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RadrootsSimplexAgentStore {
    next_connection_sequence: u64,
    next_command_sequence: u64,
    connections: BTreeMap<String, RadrootsSimplexAgentConnectionRecord>,
    pending_commands: BTreeMap<u64, RadrootsSimplexAgentPendingCommand>,
    #[cfg(feature = "std")]
    persistence: Option<RadrootsSimplexAgentStorePersistence>,
}

impl RadrootsSimplexAgentStore {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "std")]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RadrootsSimplexAgentStoreError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Ok(Self {
                persistence: Some(RadrootsSimplexAgentStorePersistence::PublicSnapshot { path }),
                ..Default::default()
            });
        }

        let raw = fs::read(&path).map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to read SimpleX agent store snapshot `{}`: {error}",
                path.display()
            ))
        })?;

        let mut snapshot: RadrootsSimplexAgentStoreSnapshot = serde_json::from_slice(&raw)
            .map_err(|error| {
                RadrootsSimplexAgentStoreError::Persistence(format!(
                    "failed to parse SimpleX agent store snapshot `{}`: {error}",
                    path.display()
                ))
            })?;
        let protected_secrets_configured = snapshot.protected_secrets.is_some();
        validate_public_snapshot_secret_posture(&snapshot, protected_secrets_configured)?;
        if protected_secrets_configured {
            let protected = read_protected_secrets_snapshot(&path, &snapshot)?;
            merge_protected_secrets(&mut snapshot, protected)?;
        }

        let mut store = Self::from_snapshot(snapshot)?;
        store.persistence = Some(RadrootsSimplexAgentStorePersistence::PublicSnapshot { path });
        Ok(store)
    }

    #[cfg(all(feature = "std", feature = "os-keyring"))]
    pub fn open_keychain_protected(
        path: impl AsRef<Path>,
    ) -> Result<Self, RadrootsSimplexAgentStoreError> {
        let path = path.as_ref();
        Self::open_protected_with_vault(
            path,
            Arc::new(RadrootsSecretVaultOsKeyring::new(
                RADROOTS_SIMPLEX_AGENT_STORE_KEYCHAIN_SERVICE,
            )),
            protected_snapshot_master_key_slot(path),
        )
    }

    #[cfg(feature = "std")]
    pub fn open_protected_with_vault(
        path: impl AsRef<Path>,
        vault: Arc<dyn RadrootsSecretVault>,
        master_key_slot: impl Into<String>,
    ) -> Result<Self, RadrootsSimplexAgentStoreError> {
        let path = path.as_ref().to_path_buf();
        let key_source = RadrootsSimplexAgentStoreVaultKeySource {
            vault,
            master_key_slot: master_key_slot.into(),
        };
        if !path.exists() {
            return Ok(Self {
                persistence: Some(RadrootsSimplexAgentStorePersistence::ProtectedSnapshot {
                    path,
                    key_source,
                }),
                ..Default::default()
            });
        }

        let snapshot = read_protected_snapshot(&path, &key_source)?;
        let mut store = Self::from_snapshot(snapshot)?;
        store.persistence =
            Some(RadrootsSimplexAgentStorePersistence::ProtectedSnapshot { path, key_source });
        Ok(store)
    }

    #[cfg(feature = "std")]
    pub fn set_persistence_path(&mut self, path: impl AsRef<Path>) {
        self.persistence = Some(RadrootsSimplexAgentStorePersistence::PublicSnapshot {
            path: path.as_ref().to_path_buf(),
        });
    }

    #[cfg(feature = "std")]
    pub fn set_protected_persistence(
        &mut self,
        path: impl AsRef<Path>,
        vault: Arc<dyn RadrootsSecretVault>,
        master_key_slot: impl Into<String>,
    ) {
        self.persistence = Some(RadrootsSimplexAgentStorePersistence::ProtectedSnapshot {
            path: path.as_ref().to_path_buf(),
            key_source: RadrootsSimplexAgentStoreVaultKeySource {
                vault,
                master_key_slot: master_key_slot.into(),
            },
        });
    }

    #[cfg(feature = "std")]
    pub fn flush(&self) -> Result<(), RadrootsSimplexAgentStoreError> {
        let Some(persistence) = self.persistence.as_ref() else {
            return Ok(());
        };
        match persistence {
            RadrootsSimplexAgentStorePersistence::PublicSnapshot { path } => {
                ensure_parent_dir(path)?;
                let mut snapshot = self.snapshot()?;
                let mut secrets = redact_snapshot_secrets(&mut snapshot)?;
                if secrets.has_secret_material() {
                    let generation = compute_protected_generation(&snapshot, &secrets)?;
                    secrets.generation = generation.clone();
                    snapshot.protected_secrets = Some(write_protected_secrets_snapshot(
                        path, &secrets, generation,
                    )?);
                    atomic_write_public_snapshot(path, &snapshot)
                } else {
                    snapshot.protected_secrets = None;
                    atomic_write_public_snapshot(path, &snapshot)?;
                    remove_protected_secrets_files(path)
                }
            }
            RadrootsSimplexAgentStorePersistence::ProtectedSnapshot { path, key_source } => {
                ensure_parent_dir(path)?;
                write_protected_snapshot(path, key_source, &self.snapshot()?)?;
                remove_protected_secrets_files(path)
            }
        }
    }

    #[cfg(feature = "std")]
    pub fn protected_secrets_path(path: impl AsRef<Path>) -> PathBuf {
        protected_secrets_path(path.as_ref())
    }

    #[cfg(feature = "std")]
    pub fn protected_secrets_wrapping_key_path(path: impl AsRef<Path>) -> PathBuf {
        protected_secrets_wrapping_key_path(path.as_ref())
    }

    #[cfg(feature = "std")]
    pub fn protected_secrets_diagnostics(
        path: impl AsRef<Path>,
    ) -> Result<RadrootsSimplexAgentStoreProtectedSecretsDiagnostics, RadrootsSimplexAgentStoreError>
    {
        protected_secrets_diagnostics(path.as_ref())
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
            short_link: None,
            queues: Vec::new(),
            ratchet_state,
            local_e2e_public_key: None,
            local_e2e_private_key: None,
            local_x3dh_key_1: None,
            local_x3dh_key_2: None,
            local_pq_keypair: None,
            shared_secret: None,
            delivery_cursor: RadrootsSimplexAgentDeliveryCursor {
                last_sent_message_id: None,
                last_received_message_id: None,
                last_sent_message_hash: None,
                last_received_message_hash: None,
            },
            last_received_queue: None,
            last_received_broker_message_id: None,
            recent_messages: Vec::new(),
            staged_outbound_message: None,
            hello_sent: false,
            hello_received: false,
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
            queue.entity_id = address.sender_id.clone();
            queue.role = role;
            queue.primary = primary;
            queue.auth_state = Some(auth_state);
            return Ok(());
        }
        connection.queues.push(RadrootsSimplexAgentQueueRecord {
            entity_id: address.sender_id.clone(),
            descriptor,
            role,
            subscribed: false,
            primary,
            tested: false,
            auth_state: Some(auth_state),
            delivery_private_key: None,
            delivery_shared_secret: None,
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

    pub fn subscribed_receive_servers(&self) -> Vec<RadrootsSimplexSmpServerAddress> {
        let mut servers = Vec::new();
        for connection in self.connections.values() {
            for queue in &connection.queues {
                if queue.role == RadrootsSimplexAgentQueueRole::Receive
                    && queue.subscribed
                    && !servers.contains(&queue.descriptor.queue_uri.server)
                {
                    servers.push(queue.descriptor.queue_uri.server.clone());
                }
            }
        }
        servers
    }

    pub fn receive_queue_by_entity_id(
        &self,
        server: &RadrootsSimplexSmpServerAddress,
        entity_id: &[u8],
    ) -> Option<(String, RadrootsSimplexAgentQueueAddress)> {
        for connection in self.connections.values() {
            for queue in &connection.queues {
                if queue.role == RadrootsSimplexAgentQueueRole::Receive
                    && queue.descriptor.queue_uri.server == *server
                    && queue.entity_id == entity_id
                {
                    return Some((connection.id.clone(), queue.descriptor.queue_address()));
                }
            }
        }
        None
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
                inbound_queue: None,
                inbound_broker_message_id: None,
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
        queue_address: RadrootsSimplexAgentQueueAddress,
        broker_message_id: Vec<u8>,
        message_id: RadrootsSimplexAgentMessageId,
        message_hash: Vec<u8>,
    ) -> Result<(), RadrootsSimplexAgentStoreError> {
        let connection = self.connection_mut(connection_id)?;
        connection.delivery_cursor.last_received_message_id = Some(message_id);
        connection.delivery_cursor.last_received_message_hash = Some(message_hash.clone());
        connection.last_received_queue = Some(queue_address.clone());
        connection.last_received_broker_message_id = Some(broker_message_id.clone());
        connection
            .recent_messages
            .push(RadrootsSimplexAgentRecentMessageRecord {
                message_id,
                message_hash,
                inbound_queue: Some(RadrootsSimplexAgentRecentQueueAddress::from_queue_address(
                    &queue_address,
                )),
                inbound_broker_message_id: Some(broker_message_id),
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

    pub fn has_pending_ack_message(
        &self,
        connection_id: &str,
        message_id: RadrootsSimplexAgentMessageId,
        message_hash: &[u8],
    ) -> bool {
        self.pending_commands.values().any(|command| {
            command.connection_id == connection_id
                && matches!(
                    &command.kind,
                    RadrootsSimplexAgentPendingCommandKind::AckInboxMessage { receipt, .. }
                    if receipt.message_id == message_id && receipt.message_hash == message_hash
                )
        })
    }

    pub fn inbound_ack_target(
        &self,
        connection_id: &str,
        message_id: RadrootsSimplexAgentMessageId,
        message_hash: &[u8],
    ) -> Result<Option<(RadrootsSimplexAgentQueueAddress, Vec<u8>)>, RadrootsSimplexAgentStoreError>
    {
        let connection = self.connection(connection_id)?;
        Ok(connection
            .recent_messages
            .iter()
            .rev()
            .find(|message| {
                message.message_id == message_id && message.message_hash == message_hash
            })
            .and_then(|message| {
                Some((
                    message.inbound_queue.clone()?.into_queue_address(),
                    message.inbound_broker_message_id.clone()?,
                ))
            }))
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
            protected_secrets: None,
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
            persistence: None,
        })
    }
}

#[cfg(feature = "std")]
impl RadrootsSimplexAgentStoreSecretsSnapshot {
    fn has_secret_material(&self) -> bool {
        self.connections
            .iter()
            .any(RadrootsSimplexAgentConnectionSecretsSnapshot::has_secret_material)
            || self
                .pending_commands
                .iter()
                .any(RadrootsSimplexAgentPendingCommandSecretsSnapshot::has_secret_material)
    }
}

#[cfg(feature = "std")]
impl RadrootsSecretKeyWrapping for RadrootsSimplexAgentStoreVaultKeySource {
    type Error = RadrootsSimplexAgentStoreError;

    fn wrap_data_key(&self, key_slot: &str, plaintext_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let mut master_key = load_or_create_vault_master_key(self)?;
        let mut nonce = [0_u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH];
        getrandom(&mut nonce).map_err(|_| {
            RadrootsSimplexAgentStoreError::Persistence(
                "entropy unavailable for SimpleX agent protected snapshot key wrapping".into(),
            )
        })?;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&master_key));
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext_key,
                    aad: key_slot.as_bytes(),
                },
            )
            .map_err(|_| {
                RadrootsSimplexAgentStoreError::Persistence(
                    "failed to wrap SimpleX agent protected snapshot data key".into(),
                )
            })?;
        master_key.zeroize();
        let mut encoded = Vec::with_capacity(1 + nonce.len() + ciphertext.len());
        encoded.push(RADROOTS_SIMPLEX_AGENT_STORE_WRAPPED_KEY_VERSION);
        encoded.extend_from_slice(&nonce);
        encoded.extend_from_slice(ciphertext.as_slice());
        Ok(encoded)
    }

    fn unwrap_data_key(&self, key_slot: &str, wrapped_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        if wrapped_key.len() <= 1 + RADROOTS_PROTECTED_STORE_NONCE_LENGTH {
            return Err(RadrootsSimplexAgentStoreError::Persistence(
                "SimpleX agent protected snapshot wrapped key is truncated".into(),
            ));
        }
        if wrapped_key[0] != RADROOTS_SIMPLEX_AGENT_STORE_WRAPPED_KEY_VERSION {
            return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
                "unsupported SimpleX agent protected snapshot wrapped key version `{}`",
                wrapped_key[0]
            )));
        }

        let mut master_key = load_vault_master_key(self)?;
        let nonce_offset = 1;
        let ciphertext_offset = nonce_offset + RADROOTS_PROTECTED_STORE_NONCE_LENGTH;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&master_key));
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&wrapped_key[nonce_offset..ciphertext_offset]),
                Payload {
                    msg: &wrapped_key[ciphertext_offset..],
                    aad: key_slot.as_bytes(),
                },
            )
            .map_err(|_| {
                RadrootsSimplexAgentStoreError::Persistence(
                    "failed to unwrap SimpleX agent protected snapshot data key".into(),
                )
            })?;
        master_key.zeroize();
        Ok(plaintext)
    }
}

#[cfg(all(feature = "std", feature = "os-keyring"))]
fn protected_snapshot_master_key_slot(path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_os_str().as_encoded_bytes());
    format!(
        "radroots_simplex_agent_store_snapshot_{}",
        encode_digest_hex(hasher.finalize().as_slice())
    )
}

#[cfg(feature = "std")]
fn load_or_create_vault_master_key(
    source: &RadrootsSimplexAgentStoreVaultKeySource,
) -> Result<[u8; RADROOTS_SIMPLEX_AGENT_STORE_VAULT_MASTER_KEY_BYTES], RadrootsSimplexAgentStoreError>
{
    if let Some(encoded) = source
        .vault
        .load_secret(&source.master_key_slot)
        .map_err(|error| vault_access_error("load", error))?
    {
        return decode_vault_master_key(&encoded);
    }

    let mut key = [0_u8; RADROOTS_SIMPLEX_AGENT_STORE_VAULT_MASTER_KEY_BYTES];
    getrandom(&mut key).map_err(|_| {
        RadrootsSimplexAgentStoreError::Persistence(
            "entropy unavailable for SimpleX agent protected snapshot master key".into(),
        )
    })?;
    let encoded = encode_digest_hex(key.as_slice());
    let store_result = source
        .vault
        .store_secret(&source.master_key_slot, &encoded)
        .map_err(|error| vault_access_error("store", error));
    if let Err(error) = store_result {
        key.zeroize();
        return Err(error);
    }
    Ok(key)
}

#[cfg(feature = "std")]
fn load_vault_master_key(
    source: &RadrootsSimplexAgentStoreVaultKeySource,
) -> Result<[u8; RADROOTS_SIMPLEX_AGENT_STORE_VAULT_MASTER_KEY_BYTES], RadrootsSimplexAgentStoreError>
{
    let encoded = source
        .vault
        .load_secret(&source.master_key_slot)
        .map_err(|error| vault_access_error("load", error))?
        .ok_or_else(|| {
            RadrootsSimplexAgentStoreError::Persistence(
                "SimpleX agent protected snapshot master key is missing".into(),
            )
        })?;
    decode_vault_master_key(&encoded)
}

#[cfg(feature = "std")]
fn decode_vault_master_key(
    encoded: &str,
) -> Result<[u8; RADROOTS_SIMPLEX_AGENT_STORE_VAULT_MASTER_KEY_BYTES], RadrootsSimplexAgentStoreError>
{
    if encoded.len() != RADROOTS_SIMPLEX_AGENT_STORE_VAULT_MASTER_KEY_BYTES * 2 {
        return Err(RadrootsSimplexAgentStoreError::Persistence(
            "SimpleX agent protected snapshot master key has invalid length".into(),
        ));
    }
    let mut key = [0_u8; RADROOTS_SIMPLEX_AGENT_STORE_VAULT_MASTER_KEY_BYTES];
    for (index, chunk) in encoded.as_bytes().chunks_exact(2).enumerate() {
        key[index] = (decode_ascii_hex_nibble(chunk[0])? << 4) | decode_ascii_hex_nibble(chunk[1])?;
    }
    Ok(key)
}

#[cfg(feature = "std")]
fn decode_ascii_hex_nibble(value: u8) -> Result<u8, RadrootsSimplexAgentStoreError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(RadrootsSimplexAgentStoreError::Persistence(
            "SimpleX agent protected snapshot master key is not hex encoded".into(),
        )),
    }
}

#[cfg(feature = "std")]
fn vault_access_error(
    action: &str,
    source: RadrootsSecretVaultAccessError,
) -> RadrootsSimplexAgentStoreError {
    RadrootsSimplexAgentStoreError::Persistence(format!(
        "failed to {action} SimpleX agent protected snapshot key: {source}"
    ))
}

#[cfg(feature = "std")]
impl RadrootsSimplexAgentConnectionSecretsSnapshot {
    fn has_secret_material(&self) -> bool {
        self.short_link_link_key.is_some()
            || self.short_link_private_signature_key.is_some()
            || self.local_e2e_private_key.is_some()
            || self.local_x3dh_key_1_private_key.is_some()
            || self.local_x3dh_key_2_private_key.is_some()
            || self.local_pq_private_key.is_some()
            || self.shared_secret.is_some()
            || self
                .queues
                .iter()
                .any(RadrootsSimplexAgentQueueSecretsSnapshot::has_secret_material)
            || self
                .ratchet_state
                .as_ref()
                .is_some_and(RadrootsSimplexAgentRatchetSecretsSnapshot::has_secret_material)
    }
}

#[cfg(feature = "std")]
impl RadrootsSimplexAgentQueueSecretsSnapshot {
    fn has_secret_material(&self) -> bool {
        self.auth_private_key.is_some()
            || self.delivery_private_key.is_some()
            || self.delivery_shared_secret.is_some()
    }
}

#[cfg(feature = "std")]
impl RadrootsSimplexAgentRatchetSecretsSnapshot {
    fn has_secret_material(&self) -> bool {
        self.current_pq_shared_secret.is_some()
            || self.local_pq_private_key.is_some()
            || self.local_dh_private_key.is_some()
            || self.official_root_key.is_some()
            || self.official_sending_chain_key.is_some()
            || self.official_receiving_chain_key.is_some()
            || self.official_sending_header_key.is_some()
            || self.official_receiving_header_key.is_some()
            || self.official_next_sending_header_key.is_some()
            || self.official_next_receiving_header_key.is_some()
            || !self.official_skipped_message_keys.is_empty()
    }
}

#[cfg(feature = "std")]
impl RadrootsSimplexAgentPendingCommandSecretsSnapshot {
    fn has_secret_material(&self) -> bool {
        self.short_invitation_link_key.is_some()
    }
}

#[cfg(feature = "std")]
fn protected_secrets_path(path: &Path) -> PathBuf {
    sidecar_path(path, RADROOTS_PROTECTED_FILE_SECRET_SUFFIX)
}

#[cfg(feature = "std")]
fn protected_secrets_wrapping_key_path(path: &Path) -> PathBuf {
    sidecar_path(path, RADROOTS_PROTECTED_FILE_WRAPPING_KEY_FILE)
}

#[cfg(feature = "std")]
fn protected_secrets_diagnostics(
    path: &Path,
) -> Result<RadrootsSimplexAgentStoreProtectedSecretsDiagnostics, RadrootsSimplexAgentStoreError> {
    let store_path = path.to_path_buf();
    let protected_secrets_path = protected_secrets_path(path);
    let wrapping_key_path = protected_secrets_wrapping_key_path(path);
    let public_snapshot_exists = path.exists();
    let mut protected_secrets_configured = false;
    let mut protected_connection_count = 0;
    let mut protected_pending_command_count = 0;
    let mut protected_generation = None;
    let mut protected_envelope_suffix = None;
    let mut protected_wrapping_key_suffix = None;

    if public_snapshot_exists {
        let raw = fs::read(path).map_err(|error| {
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
        let protected_configured = snapshot.protected_secrets.is_some();
        validate_public_snapshot_secret_posture(&snapshot, protected_configured)?;
        if let Some(protected) = snapshot.protected_secrets.as_ref() {
            protected_secrets_configured = true;
            let secrets = read_protected_secrets_snapshot(path, &snapshot)?;
            protected_connection_count = secrets.connections.len();
            protected_pending_command_count = secrets.pending_commands.len();
            protected_generation = Some(protected.generation.clone());
            protected_envelope_suffix = Some(protected.envelope_suffix.clone());
            protected_wrapping_key_suffix = Some(protected.wrapping_key_suffix.clone());
        }
    }

    Ok(RadrootsSimplexAgentStoreProtectedSecretsDiagnostics {
        store_path,
        protected_secrets_path: protected_secrets_path.clone(),
        wrapping_key_path: wrapping_key_path.clone(),
        public_snapshot_exists,
        protected_secrets_configured,
        protected_secrets_exists: protected_secrets_path.exists(),
        wrapping_key_exists: wrapping_key_path.exists(),
        protected_connection_count,
        protected_pending_command_count,
        protected_generation,
        protected_envelope_suffix,
        protected_wrapping_key_suffix,
    })
}

#[cfg(feature = "std")]
fn redact_snapshot_secrets(
    snapshot: &mut RadrootsSimplexAgentStoreSnapshot,
) -> Result<RadrootsSimplexAgentStoreSecretsSnapshot, RadrootsSimplexAgentStoreError> {
    let connections = snapshot
        .connections
        .iter_mut()
        .map(redact_connection_secrets)
        .collect::<Result<Vec<_>, _>>()?;
    let pending_commands = snapshot
        .pending_commands
        .iter_mut()
        .map(redact_pending_command_secrets)
        .filter(RadrootsSimplexAgentPendingCommandSecretsSnapshot::has_secret_material)
        .collect::<Vec<_>>();
    Ok(RadrootsSimplexAgentStoreSecretsSnapshot {
        version: RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SECRETS_VERSION,
        generation: String::new(),
        connections,
        pending_commands,
    })
}

#[cfg(feature = "std")]
fn redact_connection_secrets(
    connection: &mut RadrootsSimplexAgentConnectionSnapshot,
) -> Result<RadrootsSimplexAgentConnectionSecretsSnapshot, RadrootsSimplexAgentStoreError> {
    let queues = connection
        .queues
        .iter_mut()
        .map(redact_queue_secrets)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RadrootsSimplexAgentConnectionSecretsSnapshot {
        id: connection.id.clone(),
        short_link_link_key: connection
            .short_link
            .as_mut()
            .and_then(|short_link| take_non_empty_vec(&mut short_link.link_key)),
        short_link_private_signature_key: connection
            .short_link
            .as_mut()
            .and_then(|short_link| take_non_empty_vec(&mut short_link.link_private_signature_key)),
        queues,
        ratchet_state: connection
            .ratchet_state
            .as_mut()
            .map(redact_ratchet_secrets),
        local_e2e_private_key: connection.local_e2e_private_key.take(),
        local_x3dh_key_1_private_key: redact_x3dh_keypair_private(&mut connection.local_x3dh_key_1),
        local_x3dh_key_2_private_key: redact_x3dh_keypair_private(&mut connection.local_x3dh_key_2),
        local_pq_private_key: redact_pq_keypair_private(&mut connection.local_pq_keypair),
        shared_secret: connection.shared_secret.take(),
    })
}

#[cfg(feature = "std")]
fn redact_queue_secrets(
    queue: &mut RadrootsSimplexAgentQueueRecordSnapshot,
) -> Result<RadrootsSimplexAgentQueueSecretsSnapshot, RadrootsSimplexAgentStoreError> {
    let descriptor = queue_descriptor_from_snapshot(queue.descriptor.clone())?;
    Ok(RadrootsSimplexAgentQueueSecretsSnapshot {
        entity_id: queue.entity_id.clone(),
        role: queue.role.clone(),
        queue_address: Some(queue_address_to_snapshot(descriptor.queue_address())),
        auth_private_key: queue
            .auth_state
            .as_mut()
            .and_then(|auth| take_non_empty_vec(&mut auth.private_key)),
        delivery_private_key: queue.delivery_private_key.take(),
        delivery_shared_secret: queue.delivery_shared_secret.take(),
    })
}

#[cfg(feature = "std")]
fn redact_ratchet_secrets(
    ratchet: &mut RadrootsSimplexAgentRatchetStateSnapshot,
) -> RadrootsSimplexAgentRatchetSecretsSnapshot {
    RadrootsSimplexAgentRatchetSecretsSnapshot {
        current_pq_shared_secret: ratchet.current_pq_shared_secret.take(),
        local_pq_private_key: ratchet.local_pq_private_key.take(),
        local_dh_private_key: ratchet.local_dh_private_key.take(),
        official_root_key: ratchet.official_root_key.take(),
        official_sending_chain_key: ratchet.official_sending_chain_key.take(),
        official_receiving_chain_key: ratchet.official_receiving_chain_key.take(),
        official_sending_header_key: ratchet.official_sending_header_key.take(),
        official_receiving_header_key: ratchet.official_receiving_header_key.take(),
        official_next_sending_header_key: ratchet.official_next_sending_header_key.take(),
        official_next_receiving_header_key: ratchet.official_next_receiving_header_key.take(),
        official_skipped_message_keys: core::mem::take(&mut ratchet.official_skipped_message_keys),
    }
}

#[cfg(feature = "std")]
fn redact_pending_command_secrets(
    command: &mut RadrootsSimplexAgentPendingCommandSnapshot,
) -> RadrootsSimplexAgentPendingCommandSecretsSnapshot {
    RadrootsSimplexAgentPendingCommandSecretsSnapshot {
        id: command.id,
        connection_id: command.connection_id.clone(),
        short_invitation_link_key: redact_pending_command_short_invitation_link_key(
            &mut command.kind,
        ),
    }
}

#[cfg(feature = "std")]
fn redact_pending_command_short_invitation_link_key(
    kind: &mut RadrootsSimplexAgentPendingCommandKindSnapshot,
) -> Option<Vec<u8>> {
    match kind {
        RadrootsSimplexAgentPendingCommandKindSnapshot::SecureGetQueueLinkData {
            invitation,
            ..
        }
        | RadrootsSimplexAgentPendingCommandKindSnapshot::GetQueueLinkData { invitation, .. } => {
            take_non_empty_vec(&mut invitation.link_key)
        }
        _ => None,
    }
}

#[cfg(feature = "std")]
fn redact_x3dh_keypair_private(
    keypair: &mut Option<RadrootsSimplexAgentX3dhKeypair>,
) -> Option<Vec<u8>> {
    keypair
        .as_mut()
        .and_then(|keypair| take_non_empty_vec(&mut keypair.private_key))
}

#[cfg(feature = "std")]
fn redact_pq_keypair_private(
    keypair: &mut Option<RadrootsSimplexAgentPqKeypair>,
) -> Option<Vec<u8>> {
    keypair
        .as_mut()
        .and_then(|keypair| take_non_empty_vec(&mut keypair.private_key))
}

#[cfg(feature = "std")]
fn take_non_empty_vec(value: &mut Vec<u8>) -> Option<Vec<u8>> {
    if value.is_empty() {
        None
    } else {
        Some(core::mem::take(value))
    }
}

#[cfg(feature = "std")]
fn compute_protected_generation(
    snapshot: &RadrootsSimplexAgentStoreSnapshot,
    secrets: &RadrootsSimplexAgentStoreSecretsSnapshot,
) -> Result<String, RadrootsSimplexAgentStoreError> {
    let mut public_snapshot = snapshot.clone();
    public_snapshot.protected_secrets = None;
    let mut secrets_snapshot = secrets.clone();
    secrets_snapshot.generation.clear();
    let public_encoded = serde_json::to_vec(&public_snapshot).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to encode SimpleX agent public generation input: {error}"
        ))
    })?;
    let secrets_encoded = serde_json::to_vec(&secrets_snapshot).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to encode SimpleX agent protected generation input: {error}"
        ))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(public_encoded);
    hasher.update(b"\n");
    hasher.update(secrets_encoded);
    Ok(encode_digest_hex(hasher.finalize().as_slice()))
}

#[cfg(feature = "std")]
fn encode_digest_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(feature = "std")]
fn atomic_write_public_snapshot(
    path: &Path,
    snapshot: &RadrootsSimplexAgentStoreSnapshot,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    let mut encoded = serde_json::to_vec_pretty(snapshot).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to serialize SimpleX agent store snapshot `{}`: {error}",
            path.display()
        ))
    })?;
    encoded.push(b'\n');
    atomic_write_bytes(path, encoded.as_slice(), false)
}

#[cfg(feature = "std")]
fn write_protected_snapshot(
    path: &Path,
    key_source: &RadrootsSimplexAgentStoreVaultKeySource,
    snapshot: &RadrootsSimplexAgentStoreSnapshot,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    let mut protected_snapshot = snapshot.clone();
    protected_snapshot.protected_secrets = None;
    let plaintext = serde_json::to_vec(&protected_snapshot).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to serialize SimpleX agent protected snapshot `{}`: {error}",
            path.display()
        ))
    })?;
    let envelope = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(
        key_source,
        RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SNAPSHOT_KEY_SLOT,
        &plaintext,
    )
    .map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to seal SimpleX agent protected snapshot `{}`: {error}",
            path.display()
        ))
    })?;
    let encoded = envelope.encode_json().map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to encode SimpleX agent protected snapshot `{}`: {error}",
            path.display()
        ))
    })?;
    atomic_write_bytes(path, encoded.as_slice(), true)
}

#[cfg(feature = "std")]
fn read_protected_snapshot(
    path: &Path,
    key_source: &RadrootsSimplexAgentStoreVaultKeySource,
) -> Result<RadrootsSimplexAgentStoreSnapshot, RadrootsSimplexAgentStoreError> {
    let encoded = fs::read(path).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to read SimpleX agent protected snapshot `{}`: {error}",
            path.display()
        ))
    })?;
    let envelope = RadrootsProtectedStoreEnvelope::decode_json(&encoded).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to decode SimpleX agent protected snapshot `{}`: {error}",
            path.display()
        ))
    })?;
    if envelope.header.key_slot != RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SNAPSHOT_KEY_SLOT {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "SimpleX agent protected snapshot `{}` uses key slot `{}`",
            path.display(),
            envelope.header.key_slot
        )));
    }
    let plaintext = envelope
        .open_with_wrapped_key(key_source)
        .map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to open SimpleX agent protected snapshot `{}`: {error}",
                path.display()
            ))
        })?;
    let snapshot: RadrootsSimplexAgentStoreSnapshot =
        serde_json::from_slice(&plaintext).map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to parse SimpleX agent protected snapshot `{}`: {error}",
                path.display()
            ))
        })?;
    if snapshot.protected_secrets.is_some() {
        return Err(RadrootsSimplexAgentStoreError::Persistence(
            "SimpleX agent protected snapshot must not reference protected sidecar secrets".into(),
        ));
    }
    Ok(snapshot)
}

#[cfg(feature = "std")]
fn ensure_parent_dir(path: &Path) -> Result<(), RadrootsSimplexAgentStoreError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to create SimpleX agent store directory `{}`: {error}",
                parent.display()
            ))
        })?;
    }
    Ok(())
}

#[cfg(feature = "std")]
fn atomic_write_bytes(
    path: &Path,
    bytes: &[u8],
    secret_permissions: bool,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    let temp_path = temp_sibling_path(path);
    let result = atomic_write_bytes_inner(path, &temp_path, bytes, secret_permissions);
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

#[cfg(feature = "std")]
fn atomic_write_bytes_inner(
    path: &Path,
    temp_path: &Path,
    bytes: &[u8],
    secret_permissions: bool,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    remove_file_if_exists(temp_path)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)
        .map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to create SimpleX agent store temp file `{}`: {error}",
                temp_path.display()
            ))
        })?;
    file.write_all(bytes).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to write SimpleX agent store temp file `{}`: {error}",
            temp_path.display()
        ))
    })?;
    file.sync_all().map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to sync SimpleX agent store temp file `{}`: {error}",
            temp_path.display()
        ))
    })?;
    drop(file);
    if secret_permissions {
        set_secret_permissions(temp_path)?;
    }
    fs::rename(temp_path, path).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to replace SimpleX agent store file `{}` from temp `{}`: {error}",
            path.display(),
            temp_path.display()
        ))
    })
}

#[cfg(feature = "std")]
fn temp_sibling_path(path: &Path) -> PathBuf {
    let mut value = OsString::from(path.as_os_str());
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    value.push(format!(".tmp.{}.{}", std::process::id(), unique));
    PathBuf::from(value)
}

#[cfg(feature = "std")]
fn write_protected_secrets_snapshot(
    path: &Path,
    secrets: &RadrootsSimplexAgentStoreSecretsSnapshot,
    generation: String,
) -> Result<RadrootsSimplexAgentStoreProtectedSecretsRef, RadrootsSimplexAgentStoreError> {
    let protected_path = protected_secrets_path(path);
    if let Some(parent) = protected_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to create SimpleX agent protected store directory `{}`: {error}",
                parent.display()
            ))
        })?;
    }

    let payload = serde_json::to_vec(secrets).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to serialize SimpleX agent protected secrets snapshot `{}`: {error}",
            protected_path.display()
        ))
    })?;
    let key_source = RadrootsProtectedFileKeySource::new(protected_secrets_wrapping_key_path(path));
    let envelope = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(
        &key_source,
        RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SECRETS_KEY_SLOT,
        &payload,
    )
    .map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to seal SimpleX agent protected secrets snapshot `{}`: {error}",
            protected_path.display()
        ))
    })?;
    let encoded = envelope.encode_json().map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to encode SimpleX agent protected secrets snapshot `{}`: {error}",
            protected_path.display()
        ))
    })?;
    atomic_write_bytes(&protected_path, encoded.as_slice(), true)?;

    Ok(RadrootsSimplexAgentStoreProtectedSecretsRef {
        version: RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SECRETS_VERSION,
        generation,
        envelope_suffix: RADROOTS_PROTECTED_FILE_SECRET_SUFFIX.into(),
        wrapping_key_suffix: RADROOTS_PROTECTED_FILE_WRAPPING_KEY_FILE.into(),
        key_slot: RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SECRETS_KEY_SLOT.into(),
        connection_count: secrets.connections.len(),
        pending_command_count: secrets.pending_commands.len(),
    })
}

#[cfg(feature = "std")]
fn read_protected_secrets_snapshot(
    path: &Path,
    snapshot: &RadrootsSimplexAgentStoreSnapshot,
) -> Result<RadrootsSimplexAgentStoreSecretsSnapshot, RadrootsSimplexAgentStoreError> {
    let protected_ref = snapshot.protected_secrets.as_ref().ok_or_else(|| {
        RadrootsSimplexAgentStoreError::Persistence(
            "SimpleX agent store snapshot does not reference protected secrets".into(),
        )
    })?;
    validate_protected_secrets_ref(protected_ref)?;

    let protected_path = protected_secrets_path(path);
    let encoded = fs::read(&protected_path).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to read SimpleX agent protected secrets snapshot `{}`: {error}",
            protected_path.display()
        ))
    })?;
    let envelope = RadrootsProtectedStoreEnvelope::decode_json(&encoded).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to decode SimpleX agent protected secrets snapshot `{}`: {error}",
            protected_path.display()
        ))
    })?;
    if envelope.header.key_slot != RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SECRETS_KEY_SLOT {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "SimpleX agent protected secrets snapshot `{}` uses key slot `{}`",
            protected_path.display(),
            envelope.header.key_slot
        )));
    }

    let key_source = RadrootsProtectedFileKeySource::new(protected_secrets_wrapping_key_path(path));
    let plaintext = envelope
        .open_with_wrapped_key(&key_source)
        .map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to open SimpleX agent protected secrets snapshot `{}`: {error}",
                protected_path.display()
            ))
        })?;
    let secrets: RadrootsSimplexAgentStoreSecretsSnapshot = serde_json::from_slice(&plaintext)
        .map_err(|error| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "failed to parse SimpleX agent protected secrets snapshot `{}`: {error}",
                protected_path.display()
            ))
        })?;
    if secrets.version != RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SECRETS_VERSION {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "unsupported SimpleX agent protected secrets version `{}`",
            secrets.version
        )));
    }
    if secrets.generation != protected_ref.generation {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "SimpleX agent protected secrets generation `{}` does not match public snapshot generation `{}`",
            secrets.generation, protected_ref.generation
        )));
    }
    if secrets.connections.len() != protected_ref.connection_count {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "SimpleX agent protected secrets connection count `{}` does not match public snapshot count `{}`",
            secrets.connections.len(),
            protected_ref.connection_count
        )));
    }
    if secrets.pending_commands.len() != protected_ref.pending_command_count {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "SimpleX agent protected secrets pending command count `{}` does not match public snapshot count `{}`",
            secrets.pending_commands.len(),
            protected_ref.pending_command_count
        )));
    }
    let expected_generation = compute_protected_generation(snapshot, &secrets)?;
    if expected_generation != protected_ref.generation {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "SimpleX agent protected secrets generation `{}` does not match protected content generation `{expected_generation}`",
            protected_ref.generation
        )));
    }
    Ok(secrets)
}

#[cfg(feature = "std")]
fn validate_protected_secrets_ref(
    protected_ref: &RadrootsSimplexAgentStoreProtectedSecretsRef,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    if protected_ref.version != RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SECRETS_VERSION {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "unsupported SimpleX agent protected secrets reference version `{}`",
            protected_ref.version
        )));
    }
    if protected_ref.generation.len() != 64
        || !protected_ref
            .generation
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "invalid SimpleX agent protected secrets generation `{}`",
            protected_ref.generation
        )));
    }
    if protected_ref.envelope_suffix != RADROOTS_PROTECTED_FILE_SECRET_SUFFIX {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "unsupported SimpleX agent protected secrets envelope suffix `{}`",
            protected_ref.envelope_suffix
        )));
    }
    if protected_ref.wrapping_key_suffix != RADROOTS_PROTECTED_FILE_WRAPPING_KEY_FILE {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "unsupported SimpleX agent protected secrets wrapping key suffix `{}`",
            protected_ref.wrapping_key_suffix
        )));
    }
    if protected_ref.key_slot != RADROOTS_SIMPLEX_AGENT_STORE_PROTECTED_SECRETS_KEY_SLOT {
        return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "unsupported SimpleX agent protected secrets key slot `{}`",
            protected_ref.key_slot
        )));
    }
    Ok(())
}

#[cfg(feature = "std")]
fn validate_public_snapshot_secret_posture(
    snapshot: &RadrootsSimplexAgentStoreSnapshot,
    protected_secrets_configured: bool,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    for connection in &snapshot.connections {
        validate_public_connection_secret_posture(connection, protected_secrets_configured)?;
    }
    for command in &snapshot.pending_commands {
        validate_public_pending_command_secret_posture(command, protected_secrets_configured)?;
    }
    Ok(())
}

#[cfg(feature = "std")]
fn validate_public_connection_secret_posture(
    connection: &RadrootsSimplexAgentConnectionSnapshot,
    protected_secrets_configured: bool,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    reject_public_secret_option(
        connection.local_e2e_private_key.as_ref(),
        protected_secrets_configured,
        "local e2e private key",
        &connection.id,
    )?;
    reject_public_keypair_private(
        connection.local_x3dh_key_1.as_ref(),
        protected_secrets_configured,
        "first X3DH private key",
        &connection.id,
    )?;
    reject_public_keypair_private(
        connection.local_x3dh_key_2.as_ref(),
        protected_secrets_configured,
        "second X3DH private key",
        &connection.id,
    )?;
    reject_public_pq_private(
        connection.local_pq_keypair.as_ref(),
        protected_secrets_configured,
        "PQ private key",
        &connection.id,
    )?;
    reject_public_secret_option(
        connection.shared_secret.as_ref(),
        protected_secrets_configured,
        "connection shared secret",
        &connection.id,
    )?;
    if let Some(short_link) = connection.short_link.as_ref() {
        reject_public_secret_vec(
            short_link.link_key.as_slice(),
            protected_secrets_configured,
            "short-link link key",
            &connection.id,
        )?;
        reject_public_secret_vec(
            short_link.link_private_signature_key.as_slice(),
            protected_secrets_configured,
            "short-link private signature key",
            &connection.id,
        )?;
    }
    for queue in &connection.queues {
        reject_public_queue_secret_posture(queue, protected_secrets_configured, &connection.id)?;
    }
    if let Some(ratchet) = connection.ratchet_state.as_ref() {
        reject_public_ratchet_secret_posture(
            ratchet,
            protected_secrets_configured,
            &connection.id,
        )?;
    }
    Ok(())
}

#[cfg(feature = "std")]
fn reject_public_queue_secret_posture(
    queue: &RadrootsSimplexAgentQueueRecordSnapshot,
    protected_secrets_configured: bool,
    connection_id: &str,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    if let Some(auth) = queue.auth_state.as_ref() {
        reject_public_secret_vec(
            auth.private_key.as_slice(),
            protected_secrets_configured,
            "queue auth private key",
            connection_id,
        )?;
    }
    reject_public_secret_option(
        queue.delivery_private_key.as_ref(),
        protected_secrets_configured,
        "delivery private key",
        connection_id,
    )?;
    reject_public_secret_option(
        queue.delivery_shared_secret.as_ref(),
        protected_secrets_configured,
        "delivery shared secret",
        connection_id,
    )
}

#[cfg(feature = "std")]
fn reject_public_ratchet_secret_posture(
    ratchet: &RadrootsSimplexAgentRatchetStateSnapshot,
    protected_secrets_configured: bool,
    connection_id: &str,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    for (label, value) in [
        (
            "current PQ shared secret",
            ratchet.current_pq_shared_secret.as_ref(),
        ),
        (
            "local PQ private key",
            ratchet.local_pq_private_key.as_ref(),
        ),
        (
            "local DH private key",
            ratchet.local_dh_private_key.as_ref(),
        ),
        ("official root key", ratchet.official_root_key.as_ref()),
        (
            "official sending chain key",
            ratchet.official_sending_chain_key.as_ref(),
        ),
        (
            "official receiving chain key",
            ratchet.official_receiving_chain_key.as_ref(),
        ),
        (
            "official sending header key",
            ratchet.official_sending_header_key.as_ref(),
        ),
        (
            "official receiving header key",
            ratchet.official_receiving_header_key.as_ref(),
        ),
        (
            "official next sending header key",
            ratchet.official_next_sending_header_key.as_ref(),
        ),
        (
            "official next receiving header key",
            ratchet.official_next_receiving_header_key.as_ref(),
        ),
    ] {
        reject_public_secret_option(value, protected_secrets_configured, label, connection_id)?;
    }
    if !ratchet.official_skipped_message_keys.is_empty() {
        return Err(public_secret_error(
            protected_secrets_configured,
            "skipped message keys",
            connection_id,
        ));
    }
    Ok(())
}

#[cfg(feature = "std")]
fn validate_public_pending_command_secret_posture(
    command: &RadrootsSimplexAgentPendingCommandSnapshot,
    protected_secrets_configured: bool,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    match &command.kind {
        RadrootsSimplexAgentPendingCommandKindSnapshot::SecureGetQueueLinkData {
            invitation,
            ..
        }
        | RadrootsSimplexAgentPendingCommandKindSnapshot::GetQueueLinkData { invitation, .. } => {
            reject_public_secret_vec(
                invitation.link_key.as_slice(),
                protected_secrets_configured,
                "pending short-link invitation link key",
                &command.connection_id,
            )
        }
        _ => Ok(()),
    }
}

#[cfg(feature = "std")]
fn reject_public_keypair_private(
    keypair: Option<&RadrootsSimplexAgentX3dhKeypair>,
    protected_secrets_configured: bool,
    label: &str,
    connection_id: &str,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    if let Some(keypair) = keypair {
        reject_public_secret_vec(
            keypair.private_key.as_slice(),
            protected_secrets_configured,
            label,
            connection_id,
        )?;
    }
    Ok(())
}

#[cfg(feature = "std")]
fn reject_public_pq_private(
    keypair: Option<&RadrootsSimplexAgentPqKeypair>,
    protected_secrets_configured: bool,
    label: &str,
    connection_id: &str,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    if let Some(keypair) = keypair {
        reject_public_secret_vec(
            keypair.private_key.as_slice(),
            protected_secrets_configured,
            label,
            connection_id,
        )?;
    }
    Ok(())
}

#[cfg(feature = "std")]
fn reject_public_secret_option(
    value: Option<&Vec<u8>>,
    protected_secrets_configured: bool,
    label: &str,
    connection_id: &str,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    if let Some(value) = value {
        reject_public_secret_vec(
            value.as_slice(),
            protected_secrets_configured,
            label,
            connection_id,
        )?;
    }
    Ok(())
}

#[cfg(feature = "std")]
fn reject_public_secret_vec(
    value: &[u8],
    protected_secrets_configured: bool,
    label: &str,
    connection_id: &str,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    if !value.is_empty() || !protected_secrets_configured {
        return Err(public_secret_error(
            protected_secrets_configured,
            label,
            connection_id,
        ));
    }
    Ok(())
}

#[cfg(feature = "std")]
fn public_secret_error(
    protected_secrets_configured: bool,
    label: &str,
    connection_id: &str,
) -> RadrootsSimplexAgentStoreError {
    let posture = if protected_secrets_configured {
        "plaintext secret material"
    } else {
        "secret material or redacted secret markers without protected metadata"
    };
    RadrootsSimplexAgentStoreError::Persistence(format!(
        "SimpleX agent public snapshot contains {posture} for {label} on `{connection_id}`"
    ))
}

#[cfg(feature = "std")]
fn merge_protected_secrets(
    snapshot: &mut RadrootsSimplexAgentStoreSnapshot,
    secrets: RadrootsSimplexAgentStoreSecretsSnapshot,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    for secret_connection in secrets.connections {
        let connection = snapshot
            .connections
            .iter_mut()
            .find(|connection| connection.id == secret_connection.id)
            .ok_or_else(|| {
                RadrootsSimplexAgentStoreError::Persistence(format!(
                    "SimpleX agent protected secrets reference unknown connection `{}`",
                    secret_connection.id
                ))
            })?;
        merge_connection_secrets(connection, secret_connection)?;
    }
    for command_secrets in secrets.pending_commands {
        merge_pending_command_secrets(snapshot, command_secrets)?;
    }
    Ok(())
}

#[cfg(feature = "std")]
fn merge_connection_secrets(
    connection: &mut RadrootsSimplexAgentConnectionSnapshot,
    secrets: RadrootsSimplexAgentConnectionSecretsSnapshot,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    for queue_secrets in secrets.queues {
        let queue_index = protected_queue_secret_match_index(connection, &queue_secrets)?;
        let queue = &mut connection.queues[queue_index];
        merge_queue_secrets(queue, queue_secrets, &connection.id)?;
    }

    if let Some(ratchet_secrets) = secrets.ratchet_state {
        let ratchet = connection.ratchet_state.as_mut().ok_or_else(|| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "SimpleX agent protected secrets reference missing ratchet state on `{}`",
                connection.id
            ))
        })?;
        merge_ratchet_secrets(ratchet, ratchet_secrets);
    }

    connection.local_e2e_private_key = secrets.local_e2e_private_key;
    if let Some(private_key) = secrets.local_x3dh_key_1_private_key {
        let keypair = connection.local_x3dh_key_1.as_mut().ok_or_else(|| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "SimpleX agent protected secrets reference missing first X3DH keypair on `{}`",
                connection.id
            ))
        })?;
        keypair.private_key = private_key;
    }
    if let Some(private_key) = secrets.local_x3dh_key_2_private_key {
        let keypair = connection.local_x3dh_key_2.as_mut().ok_or_else(|| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "SimpleX agent protected secrets reference missing second X3DH keypair on `{}`",
                connection.id
            ))
        })?;
        keypair.private_key = private_key;
    }
    if let Some(private_key) = secrets.local_pq_private_key {
        let keypair = connection.local_pq_keypair.as_mut().ok_or_else(|| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "SimpleX agent protected secrets reference missing PQ keypair on `{}`",
                connection.id
            ))
        })?;
        keypair.private_key = private_key;
    }
    connection.shared_secret = secrets.shared_secret;
    if secrets.short_link_link_key.is_some() || secrets.short_link_private_signature_key.is_some() {
        let short_link = connection.short_link.as_mut().ok_or_else(|| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "SimpleX agent protected secrets reference missing short-link credentials on `{}`",
                connection.id
            ))
        })?;
        if let Some(link_key) = secrets.short_link_link_key {
            short_link.link_key = link_key;
        }
        if let Some(private_key) = secrets.short_link_private_signature_key {
            short_link.link_private_signature_key = private_key;
        }
    }
    Ok(())
}

#[cfg(feature = "std")]
fn protected_queue_secret_match_index(
    connection: &RadrootsSimplexAgentConnectionSnapshot,
    secrets: &RadrootsSimplexAgentQueueSecretsSnapshot,
) -> Result<usize, RadrootsSimplexAgentStoreError> {
    let mut matched_index = None;
    for (index, queue) in connection.queues.iter().enumerate() {
        if !protected_queue_secret_matches(queue, secrets)? {
            continue;
        }
        if matched_index.replace(index).is_some() {
            return Err(RadrootsSimplexAgentStoreError::Persistence(format!(
                "SimpleX agent protected secrets reference ambiguous queue on `{}`",
                connection.id
            )));
        }
    }
    matched_index.ok_or_else(|| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "SimpleX agent protected secrets reference unknown queue on `{}`",
            connection.id
        ))
    })
}

#[cfg(feature = "std")]
fn protected_queue_secret_matches(
    queue: &RadrootsSimplexAgentQueueRecordSnapshot,
    secrets: &RadrootsSimplexAgentQueueSecretsSnapshot,
) -> Result<bool, RadrootsSimplexAgentStoreError> {
    if queue.entity_id != secrets.entity_id || queue.role != secrets.role {
        return Ok(false);
    }
    let Some(address) = secrets.queue_address.as_ref() else {
        return Ok(true);
    };
    let descriptor = queue_descriptor_from_snapshot(queue.descriptor.clone())?;
    Ok(queue_address_to_snapshot(descriptor.queue_address()) == *address)
}

#[cfg(feature = "std")]
fn merge_queue_secrets(
    queue: &mut RadrootsSimplexAgentQueueRecordSnapshot,
    secrets: RadrootsSimplexAgentQueueSecretsSnapshot,
    connection_id: &str,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    if let Some(private_key) = secrets.auth_private_key {
        let auth = queue.auth_state.as_mut().ok_or_else(|| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "SimpleX agent protected secrets reference missing queue auth state on `{connection_id}`"
            ))
        })?;
        auth.private_key = private_key;
    }
    queue.delivery_private_key = secrets.delivery_private_key;
    queue.delivery_shared_secret = secrets.delivery_shared_secret;
    Ok(())
}

#[cfg(feature = "std")]
fn merge_ratchet_secrets(
    ratchet: &mut RadrootsSimplexAgentRatchetStateSnapshot,
    secrets: RadrootsSimplexAgentRatchetSecretsSnapshot,
) {
    ratchet.current_pq_shared_secret = secrets.current_pq_shared_secret;
    ratchet.local_pq_private_key = secrets.local_pq_private_key;
    ratchet.local_dh_private_key = secrets.local_dh_private_key;
    ratchet.official_root_key = secrets.official_root_key;
    ratchet.official_sending_chain_key = secrets.official_sending_chain_key;
    ratchet.official_receiving_chain_key = secrets.official_receiving_chain_key;
    ratchet.official_sending_header_key = secrets.official_sending_header_key;
    ratchet.official_receiving_header_key = secrets.official_receiving_header_key;
    ratchet.official_next_sending_header_key = secrets.official_next_sending_header_key;
    ratchet.official_next_receiving_header_key = secrets.official_next_receiving_header_key;
    ratchet.official_skipped_message_keys = secrets.official_skipped_message_keys;
}

#[cfg(feature = "std")]
fn merge_pending_command_secrets(
    snapshot: &mut RadrootsSimplexAgentStoreSnapshot,
    secrets: RadrootsSimplexAgentPendingCommandSecretsSnapshot,
) -> Result<(), RadrootsSimplexAgentStoreError> {
    let command = snapshot
        .pending_commands
        .iter_mut()
        .find(|command| command.id == secrets.id && command.connection_id == secrets.connection_id)
        .ok_or_else(|| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "SimpleX agent protected secrets reference unknown pending command `{}`",
                secrets.id
            ))
        })?;

    let Some(link_key) = secrets.short_invitation_link_key else {
        return Ok(());
    };

    match &mut command.kind {
        RadrootsSimplexAgentPendingCommandKindSnapshot::SecureGetQueueLinkData {
            invitation,
            ..
        }
        | RadrootsSimplexAgentPendingCommandKindSnapshot::GetQueueLinkData { invitation, .. } => {
            invitation.link_key = link_key;
            Ok(())
        }
        _ => Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "SimpleX agent protected secrets reference pending command `{}` without short invitation link data",
            secrets.id
        ))),
    }
}

#[cfg(feature = "std")]
fn remove_protected_secrets_files(path: &Path) -> Result<(), RadrootsSimplexAgentStoreError> {
    remove_file_if_exists(&protected_secrets_path(path))?;
    remove_file_if_exists(&protected_secrets_wrapping_key_path(path))
}

#[cfg(feature = "std")]
fn remove_file_if_exists(path: &Path) -> Result<(), RadrootsSimplexAgentStoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to remove SimpleX agent protected store file `{}`: {error}",
            path.display()
        ))),
    }
}

#[cfg(feature = "std")]
fn set_secret_permissions(path: &Path) -> Result<(), RadrootsSimplexAgentStoreError> {
    set_secret_permissions_inner(path).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to set SimpleX agent protected store permissions `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(all(feature = "std", unix))]
fn set_secret_permissions_inner(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(all(feature = "std", not(unix)))]
fn set_secret_permissions_inner(_path: &Path) -> std::io::Result<()> {
    Ok(())
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
        short_link: record.short_link.map(short_link_to_snapshot),
        queues: record
            .queues
            .into_iter()
            .map(queue_record_to_snapshot)
            .collect::<Result<Vec<_>, _>>()?,
        ratchet_state: record.ratchet_state.map(ratchet_state_to_snapshot),
        local_e2e_public_key: record.local_e2e_public_key,
        local_e2e_private_key: record.local_e2e_private_key,
        local_x3dh_key_1: record.local_x3dh_key_1,
        local_x3dh_key_2: record.local_x3dh_key_2,
        local_pq_keypair: record.local_pq_keypair,
        shared_secret: record.shared_secret,
        delivery_cursor: record.delivery_cursor,
        last_received_queue: record.last_received_queue.map(queue_address_to_snapshot),
        last_received_broker_message_id: record.last_received_broker_message_id,
        recent_messages: record.recent_messages,
        staged_outbound_message: record.staged_outbound_message,
        hello_sent: record.hello_sent,
        hello_received: record.hello_received,
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
        short_link: snapshot
            .short_link
            .map(short_link_from_snapshot)
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
        local_e2e_public_key: snapshot.local_e2e_public_key,
        local_e2e_private_key: snapshot.local_e2e_private_key,
        local_x3dh_key_1: snapshot.local_x3dh_key_1,
        local_x3dh_key_2: snapshot.local_x3dh_key_2,
        local_pq_keypair: snapshot.local_pq_keypair,
        shared_secret: snapshot.shared_secret,
        delivery_cursor: snapshot.delivery_cursor,
        last_received_queue: snapshot
            .last_received_queue
            .map(queue_address_from_snapshot)
            .transpose()?,
        last_received_broker_message_id: snapshot.last_received_broker_message_id,
        recent_messages: snapshot.recent_messages,
        staged_outbound_message: snapshot.staged_outbound_message,
        hello_sent: snapshot.hello_sent,
        hello_received: snapshot.hello_received,
    })
}

#[cfg(feature = "std")]
fn queue_record_to_snapshot(
    record: RadrootsSimplexAgentQueueRecord,
) -> Result<RadrootsSimplexAgentQueueRecordSnapshot, RadrootsSimplexAgentStoreError> {
    Ok(RadrootsSimplexAgentQueueRecordSnapshot {
        descriptor: queue_descriptor_to_snapshot(record.descriptor),
        entity_id: record.entity_id,
        role: encode_queue_role(record.role).into(),
        subscribed: record.subscribed,
        primary: record.primary,
        tested: record.tested,
        auth_state: record.auth_state,
        delivery_private_key: record.delivery_private_key,
        delivery_shared_secret: record.delivery_shared_secret,
    })
}

#[cfg(feature = "std")]
fn queue_record_from_snapshot(
    snapshot: RadrootsSimplexAgentQueueRecordSnapshot,
) -> Result<RadrootsSimplexAgentQueueRecord, RadrootsSimplexAgentStoreError> {
    Ok(RadrootsSimplexAgentQueueRecord {
        descriptor: queue_descriptor_from_snapshot(snapshot.descriptor)?,
        entity_id: snapshot.entity_id,
        role: decode_queue_role(&snapshot.role)?,
        subscribed: snapshot.subscribed,
        primary: snapshot.primary,
        tested: snapshot.tested,
        auth_state: snapshot.auth_state,
        delivery_private_key: snapshot.delivery_private_key,
        delivery_shared_secret: snapshot.delivery_shared_secret,
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
    Ok(RadrootsSimplexAgentQueueDescriptor {
        queue_uri: queue_uri_from_string(&snapshot.queue_uri)?,
        replaced_queue: snapshot
            .replaced_queue
            .map(queue_address_from_snapshot)
            .transpose()?,
        primary: snapshot.primary,
        sender_key: snapshot.sender_key,
    })
}

#[cfg(feature = "std")]
fn queue_uri_from_string(
    value: &str,
) -> Result<RadrootsSimplexSmpQueueUri, RadrootsSimplexAgentStoreError> {
    RadrootsSimplexSmpQueueUri::parse(value).map_err(|error| {
        RadrootsSimplexAgentStoreError::Persistence(format!(
            "failed to parse SimpleX queue uri `{value}`: {error}"
        ))
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
fn short_link_to_snapshot(
    credentials: RadrootsSimplexAgentShortLinkCredentials,
) -> RadrootsSimplexAgentShortLinkCredentialsSnapshot {
    RadrootsSimplexAgentShortLinkCredentialsSnapshot {
        scheme: encode_short_link_scheme(credentials.scheme).into(),
        hosts: credentials.hosts,
        port: credentials.port,
        server_key_hash: credentials.server_key_hash,
        link_id: credentials.link_id,
        link_key: credentials.link_key,
        link_public_signature_key: credentials.link_public_signature_key,
        link_private_signature_key: credentials.link_private_signature_key,
        encrypted_fixed_data: credentials.encrypted_fixed_data,
        encrypted_user_data: credentials.encrypted_user_data,
    }
}

#[cfg(feature = "std")]
fn short_link_from_snapshot(
    snapshot: RadrootsSimplexAgentShortLinkCredentialsSnapshot,
) -> Result<RadrootsSimplexAgentShortLinkCredentials, RadrootsSimplexAgentStoreError> {
    Ok(RadrootsSimplexAgentShortLinkCredentials {
        scheme: decode_short_link_scheme(&snapshot.scheme)?,
        hosts: snapshot.hosts,
        port: snapshot.port,
        server_key_hash: snapshot.server_key_hash,
        link_id: snapshot.link_id,
        link_key: snapshot.link_key,
        link_public_signature_key: snapshot.link_public_signature_key,
        link_private_signature_key: snapshot.link_private_signature_key,
        encrypted_fixed_data: snapshot.encrypted_fixed_data,
        encrypted_user_data: snapshot.encrypted_user_data,
    })
}

#[cfg(feature = "std")]
fn short_invitation_to_snapshot(
    invitation: RadrootsSimplexAgentShortInvitationLink,
) -> RadrootsSimplexAgentShortInvitationLinkSnapshot {
    RadrootsSimplexAgentShortInvitationLinkSnapshot {
        scheme: encode_short_link_scheme(invitation.scheme).into(),
        hosts: invitation.hosts,
        port: invitation.port,
        server_key_hash: invitation.server_key_hash,
        link_id: invitation.link_id,
        link_key: invitation.link_key,
    }
}

#[cfg(feature = "std")]
fn short_invitation_from_snapshot(
    snapshot: RadrootsSimplexAgentShortInvitationLinkSnapshot,
) -> Result<RadrootsSimplexAgentShortInvitationLink, RadrootsSimplexAgentStoreError> {
    Ok(RadrootsSimplexAgentShortInvitationLink {
        scheme: decode_short_link_scheme(&snapshot.scheme)?,
        hosts: snapshot.hosts,
        port: snapshot.port,
        server_key_hash: snapshot.server_key_hash,
        link_id: snapshot.link_id,
        link_key: snapshot.link_key,
    })
}

#[cfg(feature = "std")]
fn queue_link_data_to_snapshot(
    link_data: RadrootsSimplexSmpQueueLinkData,
) -> RadrootsSimplexAgentQueueLinkDataSnapshot {
    RadrootsSimplexAgentQueueLinkDataSnapshot {
        fixed_data: link_data.fixed_data,
        user_data: link_data.user_data,
    }
}

#[cfg(feature = "std")]
fn queue_link_data_from_snapshot(
    snapshot: RadrootsSimplexAgentQueueLinkDataSnapshot,
) -> RadrootsSimplexSmpQueueLinkData {
    RadrootsSimplexSmpQueueLinkData {
        fixed_data: snapshot.fixed_data,
        user_data: snapshot.user_data,
    }
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
        local_pq_private_key: state.local_pq_private_key,
        local_dh_private_key: state.local_dh_private_key,
        official_associated_data: state.official_associated_data,
        official_root_key: state.official_root_key,
        official_sending_chain_key: state.official_sending_chain_key,
        official_receiving_chain_key: state.official_receiving_chain_key,
        official_sending_header_key: state.official_sending_header_key,
        official_receiving_header_key: state.official_receiving_header_key,
        official_next_sending_header_key: state.official_next_sending_header_key,
        official_next_receiving_header_key: state.official_next_receiving_header_key,
        official_skipped_message_keys: state
            .official_skipped_message_keys
            .into_iter()
            .map(skipped_message_key_to_snapshot)
            .collect(),
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
    state.local_pq_private_key = snapshot.local_pq_private_key;
    state.local_dh_private_key = snapshot.local_dh_private_key;
    state.official_associated_data = snapshot.official_associated_data;
    state.official_root_key = snapshot.official_root_key;
    state.official_sending_chain_key = snapshot.official_sending_chain_key;
    state.official_receiving_chain_key = snapshot.official_receiving_chain_key;
    state.official_sending_header_key = snapshot.official_sending_header_key;
    state.official_receiving_header_key = snapshot.official_receiving_header_key;
    state.official_next_sending_header_key = snapshot.official_next_sending_header_key;
    state.official_next_receiving_header_key = snapshot.official_next_receiving_header_key;
    state.official_skipped_message_keys = snapshot
        .official_skipped_message_keys
        .into_iter()
        .map(skipped_message_key_from_snapshot)
        .collect::<Result<_, _>>()?;
    Ok(state)
}

#[cfg(feature = "std")]
fn skipped_message_key_to_snapshot(
    key: RadrootsSimplexSmpSkippedMessageKey,
) -> RadrootsSimplexAgentSkippedMessageKeySnapshot {
    RadrootsSimplexAgentSkippedMessageKeySnapshot {
        header_key: key.header_key,
        message_number: key.message_number,
        message_key: key.message_key,
        message_iv: key.message_iv.to_vec(),
    }
}

#[cfg(feature = "std")]
fn skipped_message_key_from_snapshot(
    snapshot: RadrootsSimplexAgentSkippedMessageKeySnapshot,
) -> Result<RadrootsSimplexSmpSkippedMessageKey, RadrootsSimplexAgentStoreError> {
    let message_iv: [u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH] = snapshot
        .message_iv
        .try_into()
        .map_err(|message_iv: Vec<u8>| {
            RadrootsSimplexAgentStoreError::Persistence(format!(
                "invalid SimpleX skipped message IV length {}",
                message_iv.len()
            ))
        })?;
    Ok(RadrootsSimplexSmpSkippedMessageKey {
        header_key: snapshot.header_key,
        message_number: snapshot.message_number,
        message_key: snapshot.message_key,
        message_iv,
    })
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
        RadrootsSimplexAgentPendingCommandKind::GetQueueMessage { queue } => {
            RadrootsSimplexAgentPendingCommandKindSnapshot::GetQueueMessage {
                queue: queue_address_to_snapshot(queue),
            }
        }
        RadrootsSimplexAgentPendingCommandKind::AckInboxMessage {
            queue,
            broker_message_id,
            receipt,
        } => RadrootsSimplexAgentPendingCommandKindSnapshot::AckInboxMessage {
            queue: queue_address_to_snapshot(queue),
            broker_message_id,
            receipt: RadrootsSimplexAgentMessageReceiptSnapshot {
                message_id: receipt.message_id,
                message_hash: receipt.message_hash,
                receipt_info: receipt.receipt_info,
            },
        },
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
        RadrootsSimplexAgentPendingCommandKind::SetQueueLinkData {
            queue,
            link_id,
            link_data,
        } => RadrootsSimplexAgentPendingCommandKindSnapshot::SetQueueLinkData {
            queue: queue_address_to_snapshot(queue),
            link_id,
            link_data: queue_link_data_to_snapshot(link_data),
        },
        RadrootsSimplexAgentPendingCommandKind::SecureGetQueueLinkData {
            invitation,
            reply_queue,
        } => RadrootsSimplexAgentPendingCommandKindSnapshot::SecureGetQueueLinkData {
            invitation: short_invitation_to_snapshot(invitation),
            reply_queue: reply_queue.to_string(),
        },
        RadrootsSimplexAgentPendingCommandKind::GetQueueLinkData {
            invitation,
            reply_queue,
        } => RadrootsSimplexAgentPendingCommandKindSnapshot::GetQueueLinkData {
            invitation: short_invitation_to_snapshot(invitation),
            reply_queue: reply_queue.to_string(),
        },
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
        RadrootsSimplexAgentPendingCommandKindSnapshot::GetQueueMessage { queue } => {
            RadrootsSimplexAgentPendingCommandKind::GetQueueMessage {
                queue: queue_address_from_snapshot(queue)?,
            }
        }
        RadrootsSimplexAgentPendingCommandKindSnapshot::AckInboxMessage {
            queue,
            broker_message_id,
            receipt,
        } => RadrootsSimplexAgentPendingCommandKind::AckInboxMessage {
            queue: queue_address_from_snapshot(queue)?,
            broker_message_id,
            receipt: RadrootsSimplexAgentMessageReceipt {
                message_id: receipt.message_id,
                message_hash: receipt.message_hash,
                receipt_info: receipt.receipt_info,
            },
        },
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
        RadrootsSimplexAgentPendingCommandKindSnapshot::SetQueueLinkData {
            queue,
            link_id,
            link_data,
        } => RadrootsSimplexAgentPendingCommandKind::SetQueueLinkData {
            queue: queue_address_from_snapshot(queue)?,
            link_id,
            link_data: queue_link_data_from_snapshot(link_data),
        },
        RadrootsSimplexAgentPendingCommandKindSnapshot::SecureGetQueueLinkData {
            invitation,
            reply_queue,
        } => RadrootsSimplexAgentPendingCommandKind::SecureGetQueueLinkData {
            invitation: short_invitation_from_snapshot(invitation)?,
            reply_queue: queue_uri_from_string(&reply_queue)?,
        },
        RadrootsSimplexAgentPendingCommandKindSnapshot::GetQueueLinkData {
            invitation,
            reply_queue,
        } => RadrootsSimplexAgentPendingCommandKind::GetQueueLinkData {
            invitation: short_invitation_from_snapshot(invitation)?,
            reply_queue: queue_uri_from_string(&reply_queue)?,
        },
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

#[cfg(feature = "std")]
fn encode_short_link_scheme(scheme: RadrootsSimplexAgentShortLinkScheme) -> &'static str {
    match scheme {
        RadrootsSimplexAgentShortLinkScheme::Simplex => "simplex",
        RadrootsSimplexAgentShortLinkScheme::Https => "https",
    }
}

#[cfg(feature = "std")]
fn decode_short_link_scheme(
    value: &str,
) -> Result<RadrootsSimplexAgentShortLinkScheme, RadrootsSimplexAgentStoreError> {
    match value {
        "simplex" => Ok(RadrootsSimplexAgentShortLinkScheme::Simplex),
        "https" => Ok(RadrootsSimplexAgentShortLinkScheme::Https),
        other => Err(RadrootsSimplexAgentStoreError::Persistence(format!(
            "invalid SimpleX short-link scheme `{other}`"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "std")]
    use radroots_secret_vault::{RadrootsSecretVault, RadrootsSecretVaultAccessError};
    use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpQueueUri;
    #[cfg(feature = "std")]
    use std::collections::HashMap;
    #[cfg(feature = "std")]
    use std::path::Path;
    #[cfg(feature = "std")]
    use std::sync::{Arc, RwLock};

    #[cfg(feature = "std")]
    #[derive(Clone, Default)]
    struct TestSecretVault {
        entries: Arc<RwLock<HashMap<String, String>>>,
    }

    #[cfg(feature = "std")]
    impl TestSecretVault {
        fn new() -> Self {
            Self::default()
        }
    }

    #[cfg(feature = "std")]
    impl RadrootsSecretVault for TestSecretVault {
        fn store_secret(
            &self,
            slot: &str,
            secret: &str,
        ) -> Result<(), RadrootsSecretVaultAccessError> {
            let mut entries = self.entries.write().map_err(|_| {
                RadrootsSecretVaultAccessError::Backend("test vault poisoned".into())
            })?;
            entries.insert(slot.to_owned(), secret.to_owned());
            Ok(())
        }

        fn load_secret(
            &self,
            slot: &str,
        ) -> Result<Option<String>, RadrootsSecretVaultAccessError> {
            let entries = self.entries.read().map_err(|_| {
                RadrootsSecretVaultAccessError::Backend("test vault poisoned".into())
            })?;
            Ok(entries.get(slot).cloned())
        }

        fn remove_secret(&self, slot: &str) -> Result<(), RadrootsSecretVaultAccessError> {
            let mut entries = self.entries.write().map_err(|_| {
                RadrootsSecretVaultAccessError::Backend("test vault poisoned".into())
            })?;
            entries.remove(slot);
            Ok(())
        }
    }

    fn sample_descriptor(primary: bool) -> RadrootsSimplexAgentQueueDescriptor {
        sample_descriptor_with_uri(
            "smp://aGVsbG8@relay.example/cXVldWU#/?v=4&dh=Zm9vYmFy&q=m",
            primary,
        )
    }

    fn sample_descriptor_with_uri(uri: &str, primary: bool) -> RadrootsSimplexAgentQueueDescriptor {
        RadrootsSimplexAgentQueueDescriptor {
            queue_uri: RadrootsSimplexSmpQueueUri::parse(uri).unwrap(),
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

    fn sample_short_link_credentials() -> RadrootsSimplexAgentShortLinkCredentials {
        RadrootsSimplexAgentShortLinkCredentials {
            scheme: RadrootsSimplexAgentShortLinkScheme::Simplex,
            hosts: vec!["relay-a.example".to_owned(), "relay-b.example".to_owned()],
            port: Some(5223),
            server_key_hash: Some(vec![5_u8; 32]),
            link_id: vec![6_u8; 24],
            link_key: b"short-link-key-must-be-secret!!".to_vec(),
            link_public_signature_key: vec![7_u8; 32],
            link_private_signature_key: b"short-link-private-signature-key".to_vec(),
            encrypted_fixed_data: Some(b"encrypted-fixed-link-data".to_vec()),
            encrypted_user_data: Some(b"encrypted-user-link-data".to_vec()),
        }
    }

    fn sample_short_invitation_link(link_id: Vec<u8>) -> RadrootsSimplexAgentShortInvitationLink {
        RadrootsSimplexAgentShortInvitationLink {
            scheme: RadrootsSimplexAgentShortLinkScheme::Simplex,
            hosts: vec!["relay-a.example".to_owned(), "relay-b.example".to_owned()],
            port: Some(5223),
            server_key_hash: Some(vec![5_u8; 32]),
            link_id,
            link_key: b"short-link-key-must-be-secret!!".to_vec(),
        }
    }

    #[cfg(feature = "std")]
    fn persisted_store_with_secret_material(path: &Path) -> String {
        let mut store = RadrootsSimplexAgentStore::open(path).unwrap();
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
        {
            let connection = store.connection_mut(&connection.id).unwrap();
            connection.local_e2e_private_key = Some(b"e2e-private".to_vec());
            connection.shared_secret = Some(b"connection-shared-secret".to_vec());
            let queue = connection.queues.first_mut().unwrap();
            queue.auth_state.as_mut().unwrap().private_key = b"queue-auth-private".to_vec();
            queue.delivery_private_key = Some(b"queue-delivery-private".to_vec());
            queue.delivery_shared_secret = Some(b"queue-delivery-shared-secret".to_vec());
        }
        store.flush().unwrap();
        connection.id
    }

    #[cfg(feature = "std")]
    fn read_public_snapshot(path: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[cfg(feature = "std")]
    fn write_public_snapshot(path: &Path, value: &serde_json::Value) {
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(value).unwrap()),
        )
        .unwrap();
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

    #[test]
    fn inbound_ack_target_uses_frame_specific_queue_after_cursor_moves() {
        let mut store = RadrootsSimplexAgentStore::new();
        let connection = store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::Connected,
            None,
            None,
        );
        let first_queue = sample_descriptor(true).queue_address();
        let second_queue = sample_descriptor_with_uri(
            "smp://aGVsbG8@relay.example/c2Vjb25k#/?v=4&dh=Zm9vYmFy&q=m",
            true,
        )
        .queue_address();

        store
            .record_inbound_message(
                &connection.id,
                first_queue.clone(),
                b"first-broker-message".to_vec(),
                7,
                b"first-message-hash".to_vec(),
            )
            .unwrap();
        store
            .record_inbound_message(
                &connection.id,
                second_queue.clone(),
                b"second-broker-message".to_vec(),
                8,
                b"second-message-hash".to_vec(),
            )
            .unwrap();

        assert_eq!(
            store
                .connection(&connection.id)
                .unwrap()
                .last_received_queue,
            Some(second_queue.clone())
        );
        assert_eq!(
            store
                .inbound_ack_target(&connection.id, 7, b"first-message-hash")
                .unwrap(),
            Some((first_queue, b"first-broker-message".to_vec()))
        );
        assert_eq!(
            store
                .inbound_ack_target(&connection.id, 8, b"second-message-hash")
                .unwrap(),
            Some((second_queue, b"second-broker-message".to_vec()))
        );
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
        let queue = sample_descriptor(true).queue_address();
        let short_reply_queue = sample_descriptor_with_uri(
            "smp://aGVsbG8@relay.example/cmVwbHk#/?v=4&dh=cmVwbHkta2V5&q=m",
            true,
        )
        .queue_uri;
        let secure_short_invitation = sample_short_invitation_link(vec![2_u8; 24]);
        let get_short_invitation = sample_short_invitation_link(vec![3_u8; 24]);
        store
            .enqueue_command(
                &connection.id,
                RadrootsSimplexAgentPendingCommandKind::SendEnvelope {
                    queue: queue.clone(),
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
        store
            .enqueue_command(
                &connection.id,
                RadrootsSimplexAgentPendingCommandKind::GetQueueMessage {
                    queue: queue.clone(),
                },
                11,
            )
            .unwrap();
        store
            .enqueue_command(
                &connection.id,
                RadrootsSimplexAgentPendingCommandKind::SetQueueLinkData {
                    queue: queue.clone(),
                    link_id: vec![1_u8; 24],
                    link_data: RadrootsSimplexSmpQueueLinkData {
                        fixed_data: b"fixed-link-data".to_vec(),
                        user_data: b"user-link-data".to_vec(),
                    },
                },
                12,
            )
            .unwrap();
        store
            .enqueue_command(
                &connection.id,
                RadrootsSimplexAgentPendingCommandKind::SecureGetQueueLinkData {
                    invitation: secure_short_invitation.clone(),
                    reply_queue: short_reply_queue.clone(),
                },
                13,
            )
            .unwrap();
        store
            .enqueue_command(
                &connection.id,
                RadrootsSimplexAgentPendingCommandKind::GetQueueLinkData {
                    invitation: get_short_invitation.clone(),
                    reply_queue: short_reply_queue.clone(),
                },
                14,
            )
            .unwrap();
        {
            let connection = store.connection_mut(&connection.id).unwrap();
            connection.hello_sent = true;
            connection.hello_received = true;
            connection.short_link = Some(sample_short_link_credentials());
            connection.local_e2e_public_key = Some(b"e2e-public".to_vec());
            connection.local_e2e_private_key = Some(b"e2e-private".to_vec());
            connection.shared_secret = Some(b"connection-shared-secret".to_vec());
            let queue = connection.queues.first_mut().unwrap();
            queue.auth_state.as_mut().unwrap().private_key = b"queue-auth-private".to_vec();
            queue.delivery_private_key = Some(b"queue-delivery-private".to_vec());
            queue.delivery_shared_secret = Some(b"queue-delivery-shared-secret".to_vec());
            let mut ratchet =
                RadrootsSimplexSmpRatchetState::initiator(vec![1_u8; 56], vec![2_u8; 56], None)
                    .unwrap();
            ratchet.current_pq_public_key = Some(b"ratchet-pq-public".to_vec());
            ratchet.local_pq_private_key = Some(b"ratchet-pq-private".to_vec());
            ratchet.local_dh_private_key = Some(b"official-private".to_vec());
            ratchet.official_associated_data = Some(b"official-ad".to_vec());
            ratchet.official_root_key = Some(b"official-root".to_vec());
            ratchet.official_sending_chain_key = Some(b"official-send-chain".to_vec());
            ratchet.official_receiving_chain_key = Some(b"official-recv-chain".to_vec());
            ratchet.official_sending_header_key = Some(b"official-send-header".to_vec());
            ratchet.official_receiving_header_key = Some(b"official-recv-header".to_vec());
            ratchet.official_next_sending_header_key = Some(b"official-next-send-header".to_vec());
            ratchet.official_next_receiving_header_key =
                Some(b"official-next-recv-header".to_vec());
            ratchet
                .official_skipped_message_keys
                .push(RadrootsSimplexSmpSkippedMessageKey {
                    header_key: b"official-skipped-header".to_vec(),
                    message_number: 7,
                    message_key: b"official-skipped-message".to_vec(),
                    message_iv: [3_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH],
                });
            connection.ratchet_state = Some(ratchet);
            connection.local_x3dh_key_1 = Some(RadrootsSimplexAgentX3dhKeypair {
                public_key: b"x3dh-public-1".to_vec(),
                private_key: b"x3dh-private-1".to_vec(),
            });
            connection.local_x3dh_key_2 = Some(RadrootsSimplexAgentX3dhKeypair {
                public_key: b"x3dh-public-2".to_vec(),
                private_key: b"x3dh-private-2".to_vec(),
            });
            connection.local_pq_keypair = Some(RadrootsSimplexAgentPqKeypair {
                public_key: b"pq-public".to_vec(),
                private_key: b"pq-private".to_vec(),
            });
        }
        store.flush().unwrap();
        let raw_public = fs::read_to_string(&path).unwrap();
        let public_json: serde_json::Value = serde_json::from_str(&raw_public).unwrap();
        let public_connection = &public_json["connections"][0];
        assert!(public_connection["local_e2e_public_key"].is_array());
        assert!(public_connection["local_e2e_private_key"].is_null());
        assert!(public_connection["shared_secret"].is_null());
        let public_short_link = &public_connection["short_link"];
        assert!(public_short_link["link_id"].is_array());
        assert_eq!(public_short_link["link_key"].as_array().unwrap().len(), 0);
        assert!(public_short_link["link_public_signature_key"].is_array());
        assert_eq!(
            public_short_link["link_private_signature_key"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert!(public_connection["local_x3dh_key_1"]["public_key"].is_array());
        assert_eq!(
            public_connection["local_x3dh_key_1"]["private_key"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            public_connection["local_x3dh_key_2"]["private_key"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert!(public_connection["local_pq_keypair"]["public_key"].is_array());
        assert_eq!(
            public_connection["local_pq_keypair"]["private_key"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        let public_queue = &public_connection["queues"][0];
        assert_eq!(
            public_queue["auth_state"]["private_key"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert!(public_queue["delivery_private_key"].is_null());
        assert!(public_queue["delivery_shared_secret"].is_null());
        let public_ratchet = &public_connection["ratchet_state"];
        for field in [
            "current_pq_shared_secret",
            "local_pq_private_key",
            "local_dh_private_key",
            "official_root_key",
            "official_sending_chain_key",
            "official_receiving_chain_key",
            "official_sending_header_key",
            "official_receiving_header_key",
            "official_next_sending_header_key",
            "official_next_receiving_header_key",
        ] {
            assert!(
                public_ratchet[field].is_null(),
                "public ratchet leaked {field}"
            );
        }
        assert_eq!(
            public_ratchet["official_skipped_message_keys"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert!(raw_public.contains("protected_secrets"));
        let public_pending_commands = public_json["pending_commands"].as_array().unwrap();
        let redacted_pending_short_links = public_pending_commands
            .iter()
            .filter(|command| {
                matches!(
                    command["kind"]["kind"].as_str(),
                    Some("secure_get_queue_link_data") | Some("get_queue_link_data")
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(redacted_pending_short_links.len(), 2);
        for command in redacted_pending_short_links {
            assert_eq!(
                command["kind"]["invitation"]["link_key"]
                    .as_array()
                    .unwrap()
                    .len(),
                0
            );
        }
        let protected_path = RadrootsSimplexAgentStore::protected_secrets_path(&path);
        let protected_raw = fs::read_to_string(&protected_path).unwrap();
        for secret in [
            "e2e-private",
            "queue-auth-private",
            "connection-shared-secret",
            "short-link-key-must-be-secret",
            "short-link-private-signature-key",
            "short-link-key-must-be-secret!!",
            "official-root",
            "x3dh-private-1",
            "pq-private",
        ] {
            assert!(
                !protected_raw.contains(secret),
                "protected envelope leaked {secret}"
            );
        }
        assert!(RadrootsSimplexAgentStore::protected_secrets_wrapping_key_path(&path).is_file());
        let diagnostics = RadrootsSimplexAgentStore::protected_secrets_diagnostics(&path).unwrap();
        assert!(diagnostics.public_snapshot_exists);
        assert!(diagnostics.protected_secrets_configured);
        assert!(diagnostics.protected_secrets_exists);
        assert!(diagnostics.wrapping_key_exists);
        assert_eq!(diagnostics.protected_connection_count, 1);
        assert_eq!(diagnostics.protected_pending_command_count, 2);

        let loaded = RadrootsSimplexAgentStore::open(&path).unwrap();
        let loaded_connection = loaded.connection(&connection.id).unwrap();
        assert_eq!(
            loaded_connection.staged_outbound_message,
            Some(RadrootsSimplexAgentOutboundMessage {
                message_id: 1,
                message_hash: b"persisted".to_vec(),
            })
        );
        assert!(loaded_connection.hello_sent);
        assert!(loaded_connection.hello_received);
        assert_eq!(
            loaded_connection.short_link.as_ref(),
            Some(&sample_short_link_credentials())
        );
        assert_eq!(
            loaded_connection
                .short_link
                .as_ref()
                .map(|short_link| short_link.invitation_link().link_key),
            Some(b"short-link-key-must-be-secret!!".to_vec())
        );
        assert_eq!(
            loaded_connection.local_e2e_private_key.as_deref(),
            Some(&b"e2e-private"[..])
        );
        assert_eq!(
            loaded_connection.shared_secret.as_deref(),
            Some(&b"connection-shared-secret"[..])
        );
        let loaded_queue = loaded.primary_send_queue(&connection.id).unwrap();
        assert_eq!(
            loaded_queue
                .auth_state
                .as_ref()
                .map(|auth| auth.private_key.as_slice()),
            Some(&b"queue-auth-private"[..])
        );
        assert_eq!(
            loaded_queue.delivery_private_key.as_deref(),
            Some(&b"queue-delivery-private"[..])
        );
        assert_eq!(
            loaded_queue.delivery_shared_secret.as_deref(),
            Some(&b"queue-delivery-shared-secret"[..])
        );
        let loaded_ratchet = loaded_connection.ratchet_state.as_ref().unwrap();
        assert_eq!(
            loaded_ratchet.official_associated_data.as_deref(),
            Some(&b"official-ad"[..])
        );
        assert_eq!(
            loaded_ratchet.official_sending_chain_key.as_deref(),
            Some(&b"official-send-chain"[..])
        );
        assert_eq!(
            loaded_ratchet.official_next_receiving_header_key.as_deref(),
            Some(&b"official-next-recv-header"[..])
        );
        assert_eq!(
            loaded_ratchet.official_skipped_message_keys,
            vec![RadrootsSimplexSmpSkippedMessageKey {
                header_key: b"official-skipped-header".to_vec(),
                message_number: 7,
                message_key: b"official-skipped-message".to_vec(),
                message_iv: [3_u8; RADROOTS_SIMPLEX_OFFICIAL_AES_IV_LENGTH],
            }]
        );
        assert_eq!(
            loaded_ratchet.local_pq_private_key.as_deref(),
            Some(&b"ratchet-pq-private"[..])
        );
        assert_eq!(
            loaded_connection
                .local_x3dh_key_1
                .as_ref()
                .map(|key| (key.public_key.as_slice(), key.private_key.as_slice())),
            Some((&b"x3dh-public-1"[..], &b"x3dh-private-1"[..]))
        );
        assert_eq!(
            loaded_connection
                .local_x3dh_key_2
                .as_ref()
                .map(|key| (key.public_key.as_slice(), key.private_key.as_slice())),
            Some((&b"x3dh-public-2"[..], &b"x3dh-private-2"[..]))
        );
        assert_eq!(
            loaded_connection
                .local_pq_keypair
                .as_ref()
                .map(|key| (key.public_key.as_slice(), key.private_key.as_slice())),
            Some((&b"pq-public"[..], &b"pq-private"[..]))
        );
        assert_eq!(loaded.pending_commands.len(), 5);
        assert!(loaded.pending_commands.values().any(|command| matches!(
            &command.kind,
            RadrootsSimplexAgentPendingCommandKind::GetQueueMessage { queue: persisted_queue }
                if persisted_queue == &queue
        )));
        assert!(loaded.pending_commands.values().any(|command| matches!(
            &command.kind,
            RadrootsSimplexAgentPendingCommandKind::SetQueueLinkData {
                queue: persisted_queue,
                link_id,
                link_data
            } if persisted_queue == &queue
                && link_id.as_slice() == &[1_u8; 24]
                && link_data.fixed_data.as_slice() == b"fixed-link-data"
                && link_data.user_data.as_slice() == b"user-link-data"
        )));
        assert!(loaded.pending_commands.values().any(|command| matches!(
            &command.kind,
            RadrootsSimplexAgentPendingCommandKind::SecureGetQueueLinkData {
                invitation,
                reply_queue
            } if invitation == &secure_short_invitation
                && reply_queue == &short_reply_queue
        )));
        assert!(loaded.pending_commands.values().any(|command| matches!(
            &command.kind,
            RadrootsSimplexAgentPendingCommandKind::GetQueueLinkData {
                invitation,
                reply_queue
            } if invitation == &get_short_invitation
                && reply_queue == &short_reply_queue
        )));
        assert!(
            loaded
                .primary_send_queue(&connection.id)
                .unwrap()
                .auth_state
                .is_some()
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn protected_snapshot_persists_full_agent_state_without_plaintext_json() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.protected.json");
        let vault = Arc::new(TestSecretVault::new());
        let mut store = RadrootsSimplexAgentStore::open_protected_with_vault(
            &path,
            vault.clone(),
            "test-agent-master",
        )
        .unwrap();
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
        store.connection_mut(&connection.id).unwrap().shared_secret =
            Some(b"connection-shared-secret".to_vec());
        store.flush().unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("connections"));
        assert!(!raw.contains("relay.example"));
        assert!(!raw.contains("connection-shared-secret"));
        assert!(serde_json::from_str::<RadrootsSimplexAgentStoreSnapshot>(&raw).is_err());
        assert!(!RadrootsSimplexAgentStore::protected_secrets_path(&path).exists());
        assert!(!RadrootsSimplexAgentStore::protected_secrets_wrapping_key_path(&path).exists());

        let loaded = RadrootsSimplexAgentStore::open_protected_with_vault(
            &path,
            vault.clone(),
            "test-agent-master",
        )
        .unwrap();
        let loaded_connection = loaded.connection(&connection.id).unwrap();
        assert_eq!(
            loaded_connection.shared_secret.as_deref(),
            Some(&b"connection-shared-secret"[..])
        );
        assert!(
            loaded
                .primary_send_queue(&connection.id)
                .unwrap()
                .auth_state
                .is_some()
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn protected_snapshot_wrong_vault_fails_open() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.protected.json");
        let vault = Arc::new(TestSecretVault::new());
        let mut store = RadrootsSimplexAgentStore::open_protected_with_vault(
            &path,
            vault.clone(),
            "test-agent-master",
        )
        .unwrap();
        store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::Connected,
            None,
            None,
        );
        store.flush().unwrap();

        let error = RadrootsSimplexAgentStore::open_protected_with_vault(
            &path,
            Arc::new(TestSecretVault::new()),
            "test-agent-master",
        )
        .unwrap_err();

        assert!(error.to_string().contains("failed to open"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn corrupt_protected_snapshot_fails_open() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.protected.json");
        let vault = Arc::new(TestSecretVault::new());
        fs::write(&path, b"not-json").unwrap();

        let error = RadrootsSimplexAgentStore::open_protected_with_vault(
            &path,
            vault.clone(),
            "test-agent-master",
        )
        .unwrap_err();

        assert!(error.to_string().contains("failed to decode"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn corrupt_protected_sidecar_fails_open_and_diagnostics() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");
        persisted_store_with_secret_material(&path);
        fs::write(
            RadrootsSimplexAgentStore::protected_secrets_path(&path),
            b"not-json",
        )
        .unwrap();

        let open_error = RadrootsSimplexAgentStore::open(&path).unwrap_err();
        assert!(open_error.to_string().contains("failed to decode"));
        let diagnostics_error =
            RadrootsSimplexAgentStore::protected_secrets_diagnostics(&path).unwrap_err();
        assert!(diagnostics_error.to_string().contains("failed to decode"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn missing_wrapping_key_fails_open_and_diagnostics() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");
        persisted_store_with_secret_material(&path);
        fs::remove_file(RadrootsSimplexAgentStore::protected_secrets_wrapping_key_path(&path))
            .unwrap();

        let open_error = RadrootsSimplexAgentStore::open(&path).unwrap_err();
        assert!(open_error.to_string().contains("failed to open"));
        let diagnostics_error =
            RadrootsSimplexAgentStore::protected_secrets_diagnostics(&path).unwrap_err();
        assert!(diagnostics_error.to_string().contains("failed to open"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn missing_protected_sidecar_fails_open_and_diagnostics() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");
        persisted_store_with_secret_material(&path);
        fs::remove_file(RadrootsSimplexAgentStore::protected_secrets_path(&path)).unwrap();

        let open_error = RadrootsSimplexAgentStore::open(&path).unwrap_err();
        assert!(open_error.to_string().contains("failed to read"));
        let diagnostics_error =
            RadrootsSimplexAgentStore::protected_secrets_diagnostics(&path).unwrap_err();
        assert!(diagnostics_error.to_string().contains("failed to read"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn stale_protected_generation_fails_open_and_diagnostics() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");
        persisted_store_with_secret_material(&path);
        let mut public_json = read_public_snapshot(&path);
        public_json["protected_secrets"]["generation"] = serde_json::Value::String("0".repeat(64));
        write_public_snapshot(&path, &public_json);

        let open_error = RadrootsSimplexAgentStore::open(&path).unwrap_err();
        assert!(open_error.to_string().contains("does not match"));
        let diagnostics_error =
            RadrootsSimplexAgentStore::protected_secrets_diagnostics(&path).unwrap_err();
        assert!(diagnostics_error.to_string().contains("does not match"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn public_snapshot_and_protected_sidecar_skew_is_rejected() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");
        persisted_store_with_secret_material(&path);
        let old_public_json = read_public_snapshot(&path);
        let mut store = RadrootsSimplexAgentStore::open(&path).unwrap();
        let second_connection = store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::Connected,
            None,
            None,
        );
        store
            .add_queue(
                &second_connection.id,
                sample_descriptor_with_uri(
                    "smp://aGVsbG8@relay-second.example/cXVldWU#/?v=4&dh=Zm9vYmFy&q=m",
                    true,
                ),
                RadrootsSimplexAgentQueueRole::Send,
                true,
                sample_auth_state(),
            )
            .unwrap();
        store
            .connection_mut(&second_connection.id)
            .unwrap()
            .shared_secret = Some(b"second-secret".to_vec());
        store.flush().unwrap();
        write_public_snapshot(&path, &old_public_json);

        let open_error = RadrootsSimplexAgentStore::open(&path).unwrap_err();
        assert!(open_error.to_string().contains("does not match"));
        let diagnostics_error =
            RadrootsSimplexAgentStore::protected_secrets_diagnostics(&path).unwrap_err();
        assert!(diagnostics_error.to_string().contains("does not match"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn plaintext_snapshot_without_protected_metadata_is_rejected() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");
        let mut store = RadrootsSimplexAgentStore::new();
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
        store.connection_mut(&connection.id).unwrap().shared_secret =
            Some(b"plaintext-secret".to_vec());
        let snapshot = store.snapshot().unwrap();
        fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&snapshot).unwrap()),
        )
        .unwrap();

        let error = RadrootsSimplexAgentStore::open(&path).unwrap_err();
        assert!(error.to_string().contains("without protected metadata"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn pending_short_invitation_link_key_without_protected_metadata_is_rejected() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");
        let mut store = RadrootsSimplexAgentStore::new();
        let connection = store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::JoinPending,
            None,
            None,
        );
        store
            .enqueue_command(
                &connection.id,
                RadrootsSimplexAgentPendingCommandKind::GetQueueLinkData {
                    invitation: sample_short_invitation_link(vec![4_u8; 24]),
                    reply_queue: sample_descriptor(true).queue_uri,
                },
                10,
            )
            .unwrap();
        let snapshot = store.snapshot().unwrap();
        fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&snapshot).unwrap()),
        )
        .unwrap();

        let error = RadrootsSimplexAgentStore::open(&path).unwrap_err();
        assert!(error.to_string().contains("without protected metadata"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn pending_short_invitation_plaintext_link_key_with_protected_metadata_is_rejected() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");
        let mut store = RadrootsSimplexAgentStore::open(&path).unwrap();
        let connection = store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::JoinPending,
            None,
            None,
        );
        store
            .enqueue_command(
                &connection.id,
                RadrootsSimplexAgentPendingCommandKind::SecureGetQueueLinkData {
                    invitation: sample_short_invitation_link(vec![5_u8; 24]),
                    reply_queue: sample_descriptor(true).queue_uri,
                },
                10,
            )
            .unwrap();
        store.flush().unwrap();
        let mut public_json = read_public_snapshot(&path);
        public_json["pending_commands"][0]["kind"]["invitation"]["link_key"] =
            serde_json::Value::Array(vec![serde_json::Value::from(7)]);
        write_public_snapshot(&path, &public_json);

        let error = RadrootsSimplexAgentStore::open(&path).unwrap_err();
        assert!(error.to_string().contains("plaintext secret material"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn redacted_markers_without_protected_metadata_are_rejected() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");
        persisted_store_with_secret_material(&path);
        let mut public_json = read_public_snapshot(&path);
        public_json
            .as_object_mut()
            .unwrap()
            .remove("protected_secrets");
        write_public_snapshot(&path, &public_json);

        let error = RadrootsSimplexAgentStore::open(&path).unwrap_err();
        assert!(error.to_string().contains("without protected metadata"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn ambiguous_queue_secret_merge_is_rejected() {
        let mut store = RadrootsSimplexAgentStore::new();
        let connection = store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::Connected,
            None,
            None,
        );
        store
            .add_queue(
                &connection.id,
                sample_descriptor_with_uri(
                    "smp://aGVsbG8@relay-a.example/cXVldWU#/?v=4&dh=Zm9vYmFy&q=m",
                    true,
                ),
                RadrootsSimplexAgentQueueRole::Send,
                true,
                sample_auth_state(),
            )
            .unwrap();
        store
            .add_queue(
                &connection.id,
                sample_descriptor_with_uri(
                    "smp://aGVsbG8@relay-b.example/cXVldWU#/?v=4&dh=Zm9vYmFy&q=m",
                    false,
                ),
                RadrootsSimplexAgentQueueRole::Send,
                false,
                sample_auth_state(),
            )
            .unwrap();
        let mut snapshot =
            connection_to_snapshot(store.connection(&connection.id).unwrap().clone())
                .expect("snapshot");
        let entity_id = snapshot.queues[0].entity_id.clone();
        let secrets = RadrootsSimplexAgentConnectionSecretsSnapshot {
            id: connection.id,
            short_link_link_key: None,
            short_link_private_signature_key: None,
            queues: vec![RadrootsSimplexAgentQueueSecretsSnapshot {
                entity_id,
                role: "send".to_owned(),
                queue_address: None,
                auth_private_key: Some(b"secret".to_vec()),
                delivery_private_key: None,
                delivery_shared_secret: None,
            }],
            ratchet_state: None,
            local_e2e_private_key: None,
            local_x3dh_key_1_private_key: None,
            local_x3dh_key_2_private_key: None,
            local_pq_private_key: None,
            shared_secret: None,
        };

        let error = merge_connection_secrets(&mut snapshot, secrets).unwrap_err();
        assert!(error.to_string().contains("ambiguous queue"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn flush_without_secrets_removes_stale_protected_sidecars() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("agent-store.json");
        fs::write(
            RadrootsSimplexAgentStore::protected_secrets_path(&path),
            b"stale",
        )
        .unwrap();
        fs::write(
            RadrootsSimplexAgentStore::protected_secrets_wrapping_key_path(&path),
            b"stale",
        )
        .unwrap();

        let mut store = RadrootsSimplexAgentStore::open(&path).unwrap();
        store.create_connection(
            RadrootsSimplexAgentConnectionMode::Direct,
            RadrootsSimplexAgentConnectionStatus::Connected,
            None,
            None,
        );
        store.flush().unwrap();

        let raw_public = fs::read_to_string(&path).unwrap();
        assert!(!raw_public.contains("protected_secrets"));
        assert!(!RadrootsSimplexAgentStore::protected_secrets_path(&path).exists());
        assert!(!RadrootsSimplexAgentStore::protected_secrets_wrapping_key_path(&path).exists());
    }
}

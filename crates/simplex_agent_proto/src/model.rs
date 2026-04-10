use alloc::vec::Vec;
use radroots_simplex_smp_crypto::prelude::RadrootsSimplexSmpRatchetHeader;
use radroots_simplex_smp_proto::prelude::{
    RadrootsSimplexSmpQueueUri, RadrootsSimplexSmpServerAddress, RadrootsSimplexSmpVersionRange,
};

pub const RADROOTS_SIMPLEX_AGENT_CURRENT_VERSION: u16 = 5;
pub type RadrootsSimplexAgentMessageId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsSimplexAgentConnectionMode {
    Direct,
    ContactAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsSimplexAgentConnectionStatus {
    CreatePending,
    InvitationReady,
    JoinPending,
    AwaitingApproval,
    Allowed,
    Connected,
    Suspended,
    Rotating,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentConnectionLink {
    pub invitation_queue: RadrootsSimplexSmpQueueUri,
    pub connection_id: Vec<u8>,
    pub e2e_public_key: Vec<u8>,
    pub contact_address: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentQueueAddress {
    pub server: RadrootsSimplexSmpServerAddress,
    pub sender_id: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentQueueDescriptor {
    pub queue_uri: RadrootsSimplexSmpQueueUri,
    pub replaced_queue: Option<RadrootsSimplexAgentQueueAddress>,
    pub primary: bool,
    pub sender_key: Option<Vec<u8>>,
}

impl RadrootsSimplexAgentQueueDescriptor {
    pub const fn client_version_range(&self) -> RadrootsSimplexSmpVersionRange {
        self.queue_uri.version_range
    }

    pub fn queue_address(&self) -> RadrootsSimplexAgentQueueAddress {
        RadrootsSimplexAgentQueueAddress {
            server: self.queue_uri.server.clone(),
            sender_id: self.queue_uri.sender_id.as_bytes().to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentQueueUseDecision {
    pub queue_address: RadrootsSimplexAgentQueueAddress,
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentMessageHeader {
    pub message_id: RadrootsSimplexAgentMessageId,
    pub previous_message_hash: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentMessageReceipt {
    pub message_id: RadrootsSimplexAgentMessageId,
    pub message_hash: Vec<u8>,
    pub receipt_info: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentMessage {
    Hello,
    UserMessage(Vec<u8>),
    Receipt(RadrootsSimplexAgentMessageReceipt),
    EncryptionReady {
        up_to_message_id: RadrootsSimplexAgentMessageId,
    },
    QueueContinue(RadrootsSimplexAgentQueueAddress),
    QueueAdd(Vec<RadrootsSimplexAgentQueueDescriptor>),
    QueueKey(Vec<RadrootsSimplexAgentQueueDescriptor>),
    QueueUse(Vec<RadrootsSimplexAgentQueueUseDecision>),
    QueueTest(Vec<RadrootsSimplexAgentQueueAddress>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentMessageFrame {
    pub header: RadrootsSimplexAgentMessageHeader,
    pub message: RadrootsSimplexAgentMessage,
    pub padding: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentDecryptedMessage {
    ConnectionInfo(Vec<u8>),
    ConnectionInfoReply {
        reply_queues: Vec<RadrootsSimplexAgentQueueDescriptor>,
        info: Vec<u8>,
    },
    RatchetInfo(Vec<u8>),
    Message(RadrootsSimplexAgentMessageFrame),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentEncryptedPayload {
    pub ratchet_header: Option<RadrootsSimplexSmpRatchetHeader>,
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentEnvelope {
    Confirmation {
        reply_queue: bool,
        encrypted: RadrootsSimplexAgentEncryptedPayload,
    },
    Message(RadrootsSimplexAgentEncryptedPayload),
    Invitation {
        request: Vec<u8>,
        connection_info: Vec<u8>,
    },
    RatchetKey {
        info: Vec<u8>,
        encrypted: RadrootsSimplexAgentEncryptedPayload,
    },
}

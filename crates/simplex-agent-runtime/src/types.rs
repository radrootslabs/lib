use alloc::string::String;
use alloc::vec::Vec;
use radroots_simplex_agent_proto::prelude::RadrootsSimplexAgentConnectionLink;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentRuntimeEvent {
    InvitationReady {
        connection_id: String,
        invitation: RadrootsSimplexAgentConnectionLink,
    },
    ConfirmationRequired {
        connection_id: String,
    },
    ConnectionInfo {
        connection_id: String,
        info: Vec<u8>,
    },
    ConnectionEstablished {
        connection_id: String,
    },
    MessageQueued {
        connection_id: String,
        message_id: u64,
    },
    MessageReceived {
        connection_id: String,
        message_id: u64,
        body: Vec<u8>,
    },
    MessageAcknowledged {
        connection_id: String,
        message_id: u64,
    },
    SubscriptionQueued {
        connection_id: String,
    },
    RetryQueued {
        connection_id: String,
        command_id: u64,
    },
    QueueRotationQueued {
        connection_id: String,
    },
    Error {
        connection_id: Option<String>,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentCommandOutcome {
    Delivered,
    RetryAt { ready_at: u64 },
    Failed { message: String },
}

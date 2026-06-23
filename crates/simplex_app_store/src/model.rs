use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppProfile {
    pub profile_id: String,
    pub display_name: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppContact {
    pub contact_id: String,
    pub profile_id: String,
    pub display_name: String,
    pub lifecycle: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppConnection {
    pub connection_id: String,
    pub profile_id: String,
    pub contact_id: Option<String>,
    pub state: String,
    pub agent_connection_id: Option<String>,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppQueueEndpoint {
    pub queue_endpoint_id: String,
    pub connection_id: String,
    pub role: String,
    pub server: String,
    pub sender_id: Vec<u8>,
    pub status: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppConversation {
    pub conversation_id: String,
    pub profile_id: String,
    pub contact_id: Option<String>,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsSimplexAppChatDirection {
    Inbound,
    Outbound,
    System,
}

impl RadrootsSimplexAppChatDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
            Self::System => "system",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "inbound" => Ok(Self::Inbound),
            "outbound" => Ok(Self::Outbound),
            "system" => Ok(Self::System),
            other => Err(alloc::format!("unknown chat direction `{other}`")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppChatItem {
    pub chat_item_id: String,
    pub conversation_id: String,
    pub logical_order: i64,
    pub direction: RadrootsSimplexAppChatDirection,
    pub chat_msg_id: Option<String>,
    pub body: String,
    pub delivery_status: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppInboundMessageLogEntry {
    pub inbound_id: String,
    pub connection_id: String,
    pub broker_message_id_hash: Vec<u8>,
    pub inbound_sequence: Option<i64>,
    pub message_hash: Vec<u8>,
    pub runtime_ack_handle: String,
    pub ack_status: String,
    pub app_record_kind: String,
    pub app_record_id: String,
    pub received_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppInboundChildEvent {
    pub child_event_id: String,
    pub inbound_id: String,
    pub child_ordinal: u32,
    pub app_record_kind: String,
    pub app_record_id: String,
    pub event_kind: String,
    pub chat_msg_id: Option<String>,
    pub received_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppOutboxMessage {
    pub outbox_id: String,
    pub chat_item_id: String,
    pub connection_id: String,
    pub conversation_id: Option<String>,
    pub chat_msg_id: String,
    pub body: String,
    pub status: String,
    pub runtime_message_id: Option<i64>,
    pub retry_after_unix: Option<i64>,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppOutboundTextRequest {
    pub connection_id: String,
    pub conversation_id: String,
    pub body: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppOutboundTextDraft {
    pub chat_item: RadrootsSimplexAppChatItem,
    pub outbox_message: RadrootsSimplexAppOutboxMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppInboundTextRequest {
    pub connection_id: String,
    pub conversation_id: String,
    pub broker_message_id_hash: Vec<u8>,
    pub inbound_sequence: Option<i64>,
    pub message_hash: Vec<u8>,
    pub runtime_ack_handle: String,
    pub child_ordinal: u32,
    pub chat_msg_id: Option<String>,
    pub body: String,
    pub received_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppInboundUnsupportedEventRequest {
    pub connection_id: String,
    pub broker_message_id_hash: Vec<u8>,
    pub inbound_sequence: Option<i64>,
    pub message_hash: Vec<u8>,
    pub runtime_ack_handle: String,
    pub child_ordinal: u32,
    pub event_kind: String,
    pub payload_json: String,
    pub received_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppInboundCommit {
    pub inbound: RadrootsSimplexAppInboundMessageLogEntry,
    pub child_event: RadrootsSimplexAppInboundChildEvent,
    pub chat_item: Option<RadrootsSimplexAppChatItem>,
    pub unsupported_event: Option<RadrootsSimplexAppUnsupportedProtocolEvent>,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsSimplexAppUnsupportedProtocolEvent {
    pub event_id: String,
    pub connection_id: Option<String>,
    pub event_kind: String,
    pub payload_json: String,
    pub status: String,
    pub received_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAppDiagnostics {
    pub encrypted: bool,
    pub cipher: String,
    pub schema_version: u32,
    pub migration_count: usize,
    pub foreign_keys_enabled: bool,
    pub wal_enabled: bool,
    pub key_source: String,
    pub key_slot_digest: String,
}

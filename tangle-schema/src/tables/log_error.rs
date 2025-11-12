use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS)]
#[ts(export, export_to = "types.ts")]
pub struct LogError {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub error: String,
    pub message: String,
    pub stack_trace: Option<String>,
    pub cause: Option<String>,
    pub app_system: String,
    pub app_version: String,
    pub nostr_pubkey: String,
    pub data: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, TS)]
#[ts(export, export_to = "types.ts")]
pub struct ILogErrorFields {
    pub error: String,
    pub message: String,
    pub stack_trace: Option<String>,
    pub cause: Option<String>,
    pub app_system: String,
    pub app_version: String,
    pub nostr_pubkey: String,
    pub data: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, TS)]
#[ts(export, export_to = "types.ts")]
pub struct ILogErrorFieldsPartial {
    #[ts(optional, type = "string | null")]
    pub error: Option<Value>,
    #[ts(optional, type = "string | null")]
    pub message: Option<Value>,
    #[ts(optional, type = "string | null")]
    pub stack_trace: Option<Value>,
    #[ts(optional, type = "string | null")]
    pub cause: Option<Value>,
    #[ts(optional, type = "string | null")]
    pub app_system: Option<Value>,
    #[ts(optional, type = "string | null")]
    pub app_version: Option<Value>,
    #[ts(optional, type = "string | null")]
    pub nostr_pubkey: Option<Value>,
    #[ts(optional, type = "string | null")]
    pub data: Option<Value>,
}

#[derive(Clone, Deserialize, Serialize, TS)]
#[ts(export, export_to = "types.ts")]
pub struct ILogErrorFieldsFilter {
    #[ts(optional)]
    pub id: Option<String>,
    #[ts(optional)]
    pub created_at: Option<String>,
    #[ts(optional)]
    pub updated_at: Option<String>,
    #[ts(optional)]
    pub error: Option<String>,
    #[ts(optional)]
    pub message: Option<String>,
    #[ts(optional)]
    pub stack_trace: Option<String>,
    #[ts(optional)]
    pub cause: Option<String>,
    #[ts(optional)]
    pub app_system: Option<String>,
    #[ts(optional)]
    pub app_version: Option<String>,
    #[ts(optional)]
    pub nostr_pubkey: Option<String>,
    #[ts(optional)]
    pub data: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export, export_to = "types.ts")]
pub enum LogErrorQueryBindValues {
    Id { id: String },
    NostrPubkey { nostr_pubkey: String },
}

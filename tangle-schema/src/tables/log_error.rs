use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Serialize, Deserialize)]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
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

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct ILogErrorFieldsPartial {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub error: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub message: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub stack_trace: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub cause: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub app_system: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub app_version: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub nostr_pubkey: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub data: Option<serde_json::Value>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Deserialize, Serialize)]
pub struct ILogErrorFieldsFilter {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub created_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub updated_at: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub error: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub message: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub stack_trace: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub cause: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub app_system: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub app_version: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub nostr_pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub data: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LogErrorQueryBindValues {
    Id { id: String },
    NostrPubkey { nostr_pubkey: String },
}

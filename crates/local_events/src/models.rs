#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::LocalEventsError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalRecordFamily {
    LocalWork,
    SignedEvent,
}

impl LocalRecordFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalWork => "local_work",
            Self::SignedEvent => "signed_event",
        }
    }

    pub fn parse(value: &str) -> Result<Self, LocalEventsError> {
        match value {
            "local_work" => Ok(Self::LocalWork),
            "signed_event" => Ok(Self::SignedEvent),
            other => Err(LocalEventsError::InvalidRecord(format!(
                "unknown record family `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalRecordStatus {
    LocalDraft,
    LocalSaved,
    PendingPublish,
    Published,
    Failed,
    Conflict,
}

impl LocalRecordStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalDraft => "local_draft",
            Self::LocalSaved => "local_saved",
            Self::PendingPublish => "pending_publish",
            Self::Published => "published",
            Self::Failed => "failed",
            Self::Conflict => "conflict",
        }
    }

    pub fn parse(value: &str) -> Result<Self, LocalEventsError> {
        match value {
            "local_draft" => Ok(Self::LocalDraft),
            "local_saved" => Ok(Self::LocalSaved),
            "pending_publish" => Ok(Self::PendingPublish),
            "published" => Ok(Self::Published),
            "failed" => Ok(Self::Failed),
            "conflict" => Ok(Self::Conflict),
            other => Err(LocalEventsError::InvalidRecord(format!(
                "unknown record status `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishOutboxStatus {
    None,
    Pending,
    Acknowledged,
    Failed,
}

impl PublishOutboxStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Pending => "pending",
            Self::Acknowledged => "acknowledged",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, LocalEventsError> {
        match value {
            "none" => Ok(Self::None),
            "pending" => Ok(Self::Pending),
            "acknowledged" => Ok(Self::Acknowledged),
            "failed" => Ok(Self::Failed),
            other => Err(LocalEventsError::InvalidRecord(format!(
                "unknown outbox status `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRuntime {
    Cli,
    App,
    Service,
    Worker,
    Test,
}

impl SourceRuntime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::App => "app",
            Self::Service => "service",
            Self::Worker => "worker",
            Self::Test => "test",
        }
    }

    pub fn parse(value: &str) -> Result<Self, LocalEventsError> {
        match value {
            "cli" => Ok(Self::Cli),
            "app" => Ok(Self::App),
            "service" => Ok(Self::Service),
            "worker" => Ok(Self::Worker),
            "test" => Ok(Self::Test),
            other => Err(LocalEventsError::InvalidRecord(format!(
                "unknown source runtime `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalEventRecordInput {
    pub record_id: String,
    pub family: LocalRecordFamily,
    pub status: LocalRecordStatus,
    pub source_runtime: SourceRuntime,
    pub created_at_ms: i64,
    pub inserted_at_ms: i64,
    pub owner_account_id: Option<String>,
    pub owner_pubkey: Option<String>,
    pub farm_id: Option<String>,
    pub listing_addr: Option<String>,
    pub local_work_json: Option<Value>,
    pub event_id: Option<String>,
    pub event_kind: Option<i64>,
    pub event_pubkey: Option<String>,
    pub event_created_at: Option<i64>,
    pub event_tags_json: Option<Value>,
    pub event_content: Option<String>,
    pub event_sig: Option<String>,
    pub raw_event_json: Option<Value>,
    pub outbox_status: PublishOutboxStatus,
    pub relay_set_fingerprint: Option<String>,
    pub relay_delivery_json: Option<Value>,
}

impl LocalEventRecordInput {
    pub fn validate(&self) -> Result<(), LocalEventsError> {
        validate_non_empty("record_id", &self.record_id)?;
        if let Some(value) = self.owner_account_id.as_deref() {
            validate_non_empty("owner_account_id", value)?;
        }
        if let Some(value) = self.owner_pubkey.as_deref() {
            validate_non_empty("owner_pubkey", value)?;
        }
        if let Some(value) = self.farm_id.as_deref() {
            validate_non_empty("farm_id", value)?;
        }
        if let Some(value) = self.listing_addr.as_deref() {
            validate_non_empty("listing_addr", value)?;
        }
        match self.family {
            LocalRecordFamily::LocalWork => {
                if self.local_work_json.is_none() {
                    return Err(LocalEventsError::InvalidRecord(
                        "local work records require local_work_json".to_owned(),
                    ));
                }
                if self.outbox_status != PublishOutboxStatus::None {
                    return Err(LocalEventsError::InvalidRecord(
                        "local work records must use outbox status none".to_owned(),
                    ));
                }
            }
            LocalRecordFamily::SignedEvent => {
                validate_required("event_id", self.event_id.as_deref())?;
                validate_required("event_pubkey", self.event_pubkey.as_deref())?;
                validate_required("event_sig", self.event_sig.as_deref())?;
                if self.event_kind.is_none() {
                    return Err(LocalEventsError::InvalidRecord(
                        "signed event records require event_kind".to_owned(),
                    ));
                }
                if self.raw_event_json.is_none() {
                    return Err(LocalEventsError::InvalidRecord(
                        "signed event records require raw_event_json".to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalEventRecord {
    pub seq: i64,
    pub record_id: String,
    pub family: LocalRecordFamily,
    pub status: LocalRecordStatus,
    pub source_runtime: SourceRuntime,
    pub created_at_ms: i64,
    pub inserted_at_ms: i64,
    pub updated_at_ms: i64,
    pub owner_account_id: Option<String>,
    pub owner_pubkey: Option<String>,
    pub farm_id: Option<String>,
    pub listing_addr: Option<String>,
    pub local_work_json: Option<Value>,
    pub event_id: Option<String>,
    pub event_kind: Option<i64>,
    pub event_pubkey: Option<String>,
    pub event_created_at: Option<i64>,
    pub event_tags_json: Option<Value>,
    pub event_content: Option<String>,
    pub event_sig: Option<String>,
    pub raw_event_json: Option<Value>,
    pub outbox_status: PublishOutboxStatus,
    pub relay_set_fingerprint: Option<String>,
    pub relay_delivery_json: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalEventRecordUpdate {
    pub record_id: String,
    pub status: LocalRecordStatus,
    pub outbox_status: PublishOutboxStatus,
    pub relay_set_fingerprint: Option<String>,
    pub relay_delivery_json: Option<Value>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEventsCursor {
    pub consumer_id: String,
    pub last_seq: i64,
    pub updated_at_ms: i64,
}

pub(crate) fn validate_non_empty(field: &str, value: &str) -> Result<(), LocalEventsError> {
    if value.trim().is_empty() {
        return Err(LocalEventsError::InvalidRecord(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

fn validate_required(field: &str, value: Option<&str>) -> Result<(), LocalEventsError> {
    match value {
        Some(value) => validate_non_empty(field, value),
        None => Err(LocalEventsError::InvalidRecord(format!(
            "{field} is required"
        ))),
    }
}

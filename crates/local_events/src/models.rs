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
    Network,
    Service,
    Worker,
    Test,
}

impl SourceRuntime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::App => "app",
            Self::Network => "network",
            Self::Service => "service",
            Self::Worker => "worker",
            Self::Test => "test",
        }
    }

    pub fn parse(value: &str) -> Result<Self, LocalEventsError> {
        match value {
            "cli" => Ok(Self::Cli),
            "app" => Ok(Self::App),
            "network" => Ok(Self::Network),
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
    pub change_seq: i64,
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
    pub last_change_seq: i64,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn enum_strings_and_parse_errors_cover_all_model_variants() {
        for (variant, value) in [
            (LocalRecordFamily::LocalWork, "local_work"),
            (LocalRecordFamily::SignedEvent, "signed_event"),
        ] {
            assert_eq!(variant.as_str(), value);
            assert_eq!(
                LocalRecordFamily::parse(value).expect("record family"),
                variant
            );
        }

        for (variant, value) in [
            (LocalRecordStatus::LocalDraft, "local_draft"),
            (LocalRecordStatus::LocalSaved, "local_saved"),
            (LocalRecordStatus::PendingPublish, "pending_publish"),
            (LocalRecordStatus::Published, "published"),
            (LocalRecordStatus::Failed, "failed"),
            (LocalRecordStatus::Conflict, "conflict"),
        ] {
            assert_eq!(variant.as_str(), value);
            assert_eq!(
                LocalRecordStatus::parse(value).expect("record status"),
                variant
            );
        }

        for (variant, value) in [
            (PublishOutboxStatus::None, "none"),
            (PublishOutboxStatus::Pending, "pending"),
            (PublishOutboxStatus::Acknowledged, "acknowledged"),
            (PublishOutboxStatus::Failed, "failed"),
        ] {
            assert_eq!(variant.as_str(), value);
            assert_eq!(
                PublishOutboxStatus::parse(value).expect("outbox status"),
                variant
            );
        }

        for (variant, value) in [
            (SourceRuntime::Cli, "cli"),
            (SourceRuntime::App, "app"),
            (SourceRuntime::Network, "network"),
            (SourceRuntime::Service, "service"),
            (SourceRuntime::Worker, "worker"),
            (SourceRuntime::Test, "test"),
        ] {
            assert_eq!(variant.as_str(), value);
            assert_eq!(
                SourceRuntime::parse(value).expect("source runtime"),
                variant
            );
        }

        assert!(LocalRecordFamily::parse("other").is_err());
        assert!(LocalRecordStatus::parse("other").is_err());
        assert!(PublishOutboxStatus::parse("other").is_err());
        assert!(SourceRuntime::parse("other").is_err());
    }

    #[test]
    fn local_record_input_validation_covers_success_and_error_paths() {
        let mut local_work = local_work_input();
        local_work.validate().expect("valid local work");

        for (field, update) in [
            (
                "owner_account_id",
                Box::new(|input: &mut LocalEventRecordInput| {
                    input.owner_account_id = Some(" ".to_owned());
                }) as Box<dyn Fn(&mut LocalEventRecordInput)>,
            ),
            (
                "owner_pubkey",
                Box::new(|input: &mut LocalEventRecordInput| {
                    input.owner_pubkey = Some(" ".to_owned());
                }),
            ),
            (
                "farm_id",
                Box::new(|input: &mut LocalEventRecordInput| {
                    input.farm_id = Some(" ".to_owned());
                }),
            ),
            (
                "listing_addr",
                Box::new(|input: &mut LocalEventRecordInput| {
                    input.listing_addr = Some(" ".to_owned());
                }),
            ),
        ] {
            let mut input = local_work_input();
            update(&mut input);
            assert_error_contains(input.validate(), field);
        }

        local_work.record_id = " ".to_owned();
        assert_error_contains(local_work.validate(), "record_id");

        let mut missing_work = local_work_input();
        missing_work.local_work_json = None;
        assert_error_contains(missing_work.validate(), "local_work_json");

        let mut queued_work = local_work_input();
        queued_work.outbox_status = PublishOutboxStatus::Pending;
        assert_error_contains(queued_work.validate(), "outbox status none");

        let signed_event = signed_event_input();
        signed_event.validate().expect("valid signed event");

        for (field, update) in [
            (
                "event_id",
                Box::new(|input: &mut LocalEventRecordInput| {
                    input.event_id = Some(" ".to_owned());
                }) as Box<dyn Fn(&mut LocalEventRecordInput)>,
            ),
            (
                "event_pubkey",
                Box::new(|input: &mut LocalEventRecordInput| {
                    input.event_pubkey = None;
                }),
            ),
            (
                "event_sig",
                Box::new(|input: &mut LocalEventRecordInput| {
                    input.event_sig = None;
                }),
            ),
            (
                "event_kind",
                Box::new(|input: &mut LocalEventRecordInput| {
                    input.event_kind = None;
                }),
            ),
            (
                "raw_event_json",
                Box::new(|input: &mut LocalEventRecordInput| {
                    input.raw_event_json = None;
                }),
            ),
        ] {
            let mut input = signed_event_input();
            update(&mut input);
            assert_error_contains(input.validate(), field);
        }
    }

    fn local_work_input() -> LocalEventRecordInput {
        LocalEventRecordInput {
            record_id: "local-work-a".to_owned(),
            family: LocalRecordFamily::LocalWork,
            status: LocalRecordStatus::LocalSaved,
            source_runtime: SourceRuntime::App,
            created_at_ms: 10,
            inserted_at_ms: 11,
            owner_account_id: Some("account-a".to_owned()),
            owner_pubkey: Some("pubkey-a".to_owned()),
            farm_id: Some("farm-a".to_owned()),
            listing_addr: Some("listing-a".to_owned()),
            local_work_json: Some(json!({"kind":"buyer_order_request_v1"})),
            event_id: None,
            event_kind: None,
            event_pubkey: None,
            event_created_at: None,
            event_tags_json: None,
            event_content: None,
            event_sig: None,
            raw_event_json: None,
            outbox_status: PublishOutboxStatus::None,
            relay_set_fingerprint: None,
            relay_delivery_json: None,
        }
    }

    fn signed_event_input() -> LocalEventRecordInput {
        LocalEventRecordInput {
            record_id: "signed-event-a".to_owned(),
            family: LocalRecordFamily::SignedEvent,
            status: LocalRecordStatus::PendingPublish,
            source_runtime: SourceRuntime::Service,
            created_at_ms: 20,
            inserted_at_ms: 21,
            owner_account_id: None,
            owner_pubkey: None,
            farm_id: None,
            listing_addr: None,
            local_work_json: None,
            event_id: Some("event-a".to_owned()),
            event_kind: Some(30402),
            event_pubkey: Some("pubkey-a".to_owned()),
            event_created_at: Some(20),
            event_tags_json: Some(json!([["d", "listing-a"]])),
            event_content: Some("{}".to_owned()),
            event_sig: Some("sig-a".to_owned()),
            raw_event_json: Some(json!({"id":"event-a"})),
            outbox_status: PublishOutboxStatus::Pending,
            relay_set_fingerprint: Some("relay-set-a".to_owned()),
            relay_delivery_json: Some(json!({"state":"pending"})),
        }
    }

    fn assert_error_contains(result: Result<(), LocalEventsError>, expected: &str) {
        let err = result.expect_err("validation error");
        assert!(
            err.to_string().contains(expected),
            "expected error to contain {expected}, got {err}"
        );
    }
}

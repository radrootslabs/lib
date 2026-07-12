#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::RuntimeStoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStoreRecordFamily {
    LocalWork,
    SignedEvent,
}

impl RuntimeStoreRecordFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalWork => "local_work",
            Self::SignedEvent => "signed_event",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RuntimeStoreError> {
        match value {
            "local_work" => Ok(Self::LocalWork),
            "signed_event" => Ok(Self::SignedEvent),
            other => Err(RuntimeStoreError::InvalidRecord(format!(
                "unknown record family `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStoreRecordStatus {
    LocalDraft,
    LocalSaved,
    PendingPublish,
    Published,
    Failed,
    Conflict,
}

impl RuntimeStoreRecordStatus {
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

    pub fn parse(value: &str) -> Result<Self, RuntimeStoreError> {
        match value {
            "local_draft" => Ok(Self::LocalDraft),
            "local_saved" => Ok(Self::LocalSaved),
            "pending_publish" => Ok(Self::PendingPublish),
            "published" => Ok(Self::Published),
            "failed" => Ok(Self::Failed),
            "conflict" => Ok(Self::Conflict),
            other => Err(RuntimeStoreError::InvalidRecord(format!(
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

    pub fn parse(value: &str) -> Result<Self, RuntimeStoreError> {
        match value {
            "none" => Ok(Self::None),
            "pending" => Ok(Self::Pending),
            "acknowledged" => Ok(Self::Acknowledged),
            "failed" => Ok(Self::Failed),
            other => Err(RuntimeStoreError::InvalidRecord(format!(
                "unknown outbox status `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayDeliveryState {
    Pending,
    Acknowledged,
    Observed,
    Failed,
}

impl RelayDeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Acknowledged => "acknowledged",
            Self::Observed => "observed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayDeliveryFailure {
    pub relay_url: String,
    pub error: String,
}

impl RelayDeliveryFailure {
    pub fn new(
        relay_url: impl Into<String>,
        error: impl Into<String>,
    ) -> Result<Self, RuntimeStoreError> {
        let relay_url = relay_url.into();
        let error = error.into();
        validate_non_empty("relay_url", &relay_url)?;
        validate_non_empty("relay_delivery_error", &error)?;
        Ok(Self { relay_url, error })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayDeliveryEvidence {
    pub state: RelayDeliveryState,
    #[serde(default)]
    pub target_relays: Vec<String>,
    #[serde(default)]
    pub connected_relays: Vec<String>,
    #[serde(default)]
    pub acknowledged_relays: Vec<String>,
    #[serde(default)]
    pub observed_relays: Vec<String>,
    #[serde(default)]
    pub failed_relays: Vec<RelayDeliveryFailure>,
}

impl RelayDeliveryEvidence {
    pub fn pending<I, S>(target_relays: I) -> Result<Self, RuntimeStoreError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let target_relays = normalized_relay_values(target_relays)?;
        if target_relays.is_empty() {
            return Err(RuntimeStoreError::InvalidRecord(
                "pending relay delivery evidence requires target_relays".to_owned(),
            ));
        }
        Ok(Self {
            state: RelayDeliveryState::Pending,
            target_relays,
            connected_relays: Vec::new(),
            acknowledged_relays: Vec::new(),
            observed_relays: Vec::new(),
            failed_relays: Vec::new(),
        })
    }

    pub fn acknowledged<T, C, A, TS, CS, AS>(
        target_relays: T,
        connected_relays: C,
        acknowledged_relays: A,
        failed_relays: Vec<RelayDeliveryFailure>,
    ) -> Result<Self, RuntimeStoreError>
    where
        T: IntoIterator<Item = TS>,
        C: IntoIterator<Item = CS>,
        A: IntoIterator<Item = AS>,
        TS: AsRef<str>,
        CS: AsRef<str>,
        AS: AsRef<str>,
    {
        let target_relays = normalized_relay_values(target_relays)?;
        let connected_relays = normalized_relay_values(connected_relays)?;
        let acknowledged_relays = normalized_relay_values(acknowledged_relays)?;
        if target_relays.is_empty() {
            return Err(RuntimeStoreError::InvalidRecord(
                "acknowledged relay delivery evidence requires target_relays".to_owned(),
            ));
        }
        if acknowledged_relays.is_empty() {
            return Err(RuntimeStoreError::InvalidRecord(
                "acknowledged relay delivery evidence requires acknowledged_relays".to_owned(),
            ));
        }
        Ok(Self {
            state: RelayDeliveryState::Acknowledged,
            target_relays,
            connected_relays,
            acknowledged_relays,
            observed_relays: Vec::new(),
            failed_relays,
        })
    }

    pub fn observed<T, C, O, TS, CS, OS>(
        target_relays: T,
        connected_relays: C,
        observed_relays: O,
        failed_relays: Vec<RelayDeliveryFailure>,
    ) -> Result<Self, RuntimeStoreError>
    where
        T: IntoIterator<Item = TS>,
        C: IntoIterator<Item = CS>,
        O: IntoIterator<Item = OS>,
        TS: AsRef<str>,
        CS: AsRef<str>,
        OS: AsRef<str>,
    {
        let target_relays = normalized_relay_values(target_relays)?;
        let connected_relays = normalized_relay_values(connected_relays)?;
        let observed_relays = normalized_relay_values(observed_relays)?;
        if target_relays.is_empty() {
            return Err(RuntimeStoreError::InvalidRecord(
                "observed relay delivery evidence requires target_relays".to_owned(),
            ));
        }
        if observed_relays.is_empty() {
            return Err(RuntimeStoreError::InvalidRecord(
                "observed relay delivery evidence requires observed_relays".to_owned(),
            ));
        }
        Ok(Self {
            state: RelayDeliveryState::Observed,
            target_relays,
            connected_relays,
            acknowledged_relays: Vec::new(),
            observed_relays,
            failed_relays,
        })
    }

    pub fn from_json_value(value: &Value) -> Result<Self, RuntimeStoreError> {
        let evidence: Self = serde_json::from_value(value.clone())?;
        evidence.validate()?;
        Ok(evidence)
    }

    pub fn to_json_value(&self) -> Result<Value, RuntimeStoreError> {
        self.validate()?;
        Ok(serde_json::to_value(self)?)
    }

    pub fn relay_set_fingerprint(&self) -> Option<String> {
        let relays = self
            .target_relays
            .iter()
            .chain(self.connected_relays.iter())
            .chain(self.acknowledged_relays.iter())
            .chain(self.observed_relays.iter())
            .map(String::as_str)
            .chain(
                self.failed_relays
                    .iter()
                    .map(|failure| failure.relay_url.as_str()),
            )
            .map(str::trim)
            .filter(|relay| !relay.is_empty())
            .collect::<BTreeSet<_>>();
        if relays.is_empty() {
            None
        } else {
            Some(relays.into_iter().collect::<Vec<_>>().join("\n"))
        }
    }

    fn validate(&self) -> Result<(), RuntimeStoreError> {
        validate_relays("target_relays", &self.target_relays)?;
        validate_relays("connected_relays", &self.connected_relays)?;
        validate_relays("acknowledged_relays", &self.acknowledged_relays)?;
        validate_relays("observed_relays", &self.observed_relays)?;
        match self.state {
            RelayDeliveryState::Pending => {
                if self.target_relays.is_empty() {
                    return Err(RuntimeStoreError::InvalidRecord(
                        "pending relay delivery evidence requires target_relays".to_owned(),
                    ));
                }
            }
            RelayDeliveryState::Acknowledged => {
                if self.acknowledged_relays.is_empty() {
                    return Err(RuntimeStoreError::InvalidRecord(
                        "acknowledged relay delivery evidence requires acknowledged_relays"
                            .to_owned(),
                    ));
                }
            }
            RelayDeliveryState::Observed => {
                if self.observed_relays.is_empty() {
                    return Err(RuntimeStoreError::InvalidRecord(
                        "observed relay delivery evidence requires observed_relays".to_owned(),
                    ));
                }
            }
            RelayDeliveryState::Failed => {
                if self.failed_relays.is_empty() {
                    return Err(RuntimeStoreError::InvalidRecord(
                        "failed relay delivery evidence requires failed_relays".to_owned(),
                    ));
                }
            }
        }
        for failure in &self.failed_relays {
            validate_non_empty("failed_relay_url", &failure.relay_url)?;
            validate_non_empty("failed_relay_error", &failure.error)?;
        }
        Ok(())
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

    pub fn parse(value: &str) -> Result<Self, RuntimeStoreError> {
        match value {
            "cli" => Ok(Self::Cli),
            "app" => Ok(Self::App),
            "network" => Ok(Self::Network),
            "service" => Ok(Self::Service),
            "worker" => Ok(Self::Worker),
            "test" => Ok(Self::Test),
            other => Err(RuntimeStoreError::InvalidRecord(format!(
                "unknown source runtime `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeStoreRecordInput {
    pub record_id: String,
    pub family: RuntimeStoreRecordFamily,
    pub status: RuntimeStoreRecordStatus,
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

impl RuntimeStoreRecordInput {
    pub fn validate(&self) -> Result<(), RuntimeStoreError> {
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
        if let Some(value) = self.relay_set_fingerprint.as_deref() {
            validate_non_empty("relay_set_fingerprint", value)?;
        }
        if let Some(value) = self.relay_delivery_json.as_ref() {
            RelayDeliveryEvidence::from_json_value(value)?;
        }
        match self.family {
            RuntimeStoreRecordFamily::LocalWork => {
                if self.local_work_json.is_none() {
                    return Err(RuntimeStoreError::InvalidRecord(
                        "local work records require local_work_json".to_owned(),
                    ));
                }
                if self.outbox_status != PublishOutboxStatus::None {
                    return Err(RuntimeStoreError::InvalidRecord(
                        "local work records must use outbox status none".to_owned(),
                    ));
                }
            }
            RuntimeStoreRecordFamily::SignedEvent => {
                validate_required("event_id", self.event_id.as_deref())?;
                validate_required("event_pubkey", self.event_pubkey.as_deref())?;
                validate_required("event_sig", self.event_sig.as_deref())?;
                if self.event_kind.is_none() {
                    return Err(RuntimeStoreError::InvalidRecord(
                        "signed event records require event_kind".to_owned(),
                    ));
                }
                if self.raw_event_json.is_none() {
                    return Err(RuntimeStoreError::InvalidRecord(
                        "signed event records require raw_event_json".to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeStoreRecord {
    pub seq: i64,
    pub change_seq: i64,
    pub record_id: String,
    pub family: RuntimeStoreRecordFamily,
    pub status: RuntimeStoreRecordStatus,
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
pub struct RuntimeStoreRecordUpdate {
    pub record_id: String,
    pub status: RuntimeStoreRecordStatus,
    pub outbox_status: PublishOutboxStatus,
    pub relay_set_fingerprint: Option<String>,
    pub relay_delivery_json: Option<Value>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStoreCursor {
    pub consumer_id: String,
    pub last_change_seq: i64,
    pub updated_at_ms: i64,
}

pub(crate) fn validate_non_empty(field: &str, value: &str) -> Result<(), RuntimeStoreError> {
    if value.trim().is_empty() {
        return Err(RuntimeStoreError::InvalidRecord(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

fn normalized_relay_values<I, S>(values: I) -> Result<Vec<String>, RuntimeStoreError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let value = value.as_ref().trim();
        validate_non_empty("relay_url", value)?;
        if seen.insert(value.to_owned()) {
            normalized.push(value.to_owned());
        }
    }
    Ok(normalized)
}

fn validate_relays(field: &str, relays: &[String]) -> Result<(), RuntimeStoreError> {
    for relay in relays {
        validate_non_empty(field, relay)?;
    }
    Ok(())
}

fn validate_required(field: &str, value: Option<&str>) -> Result<(), RuntimeStoreError> {
    match value {
        Some(value) => validate_non_empty(field, value),
        None => Err(RuntimeStoreError::InvalidRecord(format!(
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
            (RuntimeStoreRecordFamily::LocalWork, "local_work"),
            (RuntimeStoreRecordFamily::SignedEvent, "signed_event"),
        ] {
            assert_eq!(variant.as_str(), value);
            assert_eq!(
                RuntimeStoreRecordFamily::parse(value).expect("record family"),
                variant
            );
        }

        for (variant, value) in [
            (RuntimeStoreRecordStatus::LocalDraft, "local_draft"),
            (RuntimeStoreRecordStatus::LocalSaved, "local_saved"),
            (RuntimeStoreRecordStatus::PendingPublish, "pending_publish"),
            (RuntimeStoreRecordStatus::Published, "published"),
            (RuntimeStoreRecordStatus::Failed, "failed"),
            (RuntimeStoreRecordStatus::Conflict, "conflict"),
        ] {
            assert_eq!(variant.as_str(), value);
            assert_eq!(
                RuntimeStoreRecordStatus::parse(value).expect("record status"),
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

        assert!(RuntimeStoreRecordFamily::parse("other").is_err());
        assert!(RuntimeStoreRecordStatus::parse("other").is_err());
        assert!(PublishOutboxStatus::parse("other").is_err());
        assert!(SourceRuntime::parse("other").is_err());
    }

    #[test]
    fn relay_delivery_evidence_models_cover_states_validation_and_json() {
        for (variant, value) in [
            (RelayDeliveryState::Pending, "pending"),
            (RelayDeliveryState::Acknowledged, "acknowledged"),
            (RelayDeliveryState::Observed, "observed"),
            (RelayDeliveryState::Failed, "failed"),
        ] {
            assert_eq!(variant.as_str(), value);
        }

        let failure =
            RelayDeliveryFailure::new(" wss://relay-a.example ", " timeout ").expect("failure");
        assert_eq!(failure.relay_url, " wss://relay-a.example ");
        assert_eq!(failure.error, " timeout ");
        assert_model_error_contains(RelayDeliveryFailure::new(" ", "timeout"), "relay_url");
        assert_model_error_contains(
            RelayDeliveryFailure::new("wss://relay-a.example", " "),
            "relay_delivery_error",
        );

        let pending =
            RelayDeliveryEvidence::pending([" wss://relay-a.example ", "wss://relay-a.example"])
                .expect("pending evidence");
        assert_eq!(pending.state, RelayDeliveryState::Pending);
        assert_eq!(pending.target_relays, vec!["wss://relay-a.example"]);
        assert!(pending.relay_set_fingerprint().is_some());
        pending.to_json_value().expect("pending evidence json");
        assert_model_error_contains(
            RelayDeliveryEvidence::pending(Vec::<String>::new()),
            "target_relays",
        );

        let acknowledged = RelayDeliveryEvidence::acknowledged(
            ["wss://relay-a.example"],
            ["wss://relay-a.example"],
            ["wss://relay-a.example"],
            Vec::new(),
        )
        .expect("acknowledged evidence");
        assert_eq!(acknowledged.state, RelayDeliveryState::Acknowledged);
        assert_model_error_contains(
            RelayDeliveryEvidence::acknowledged(
                Vec::<String>::new(),
                Vec::<String>::new(),
                ["wss://relay-a.example"],
                Vec::new(),
            ),
            "target_relays",
        );
        assert_model_error_contains(
            RelayDeliveryEvidence::acknowledged(
                ["wss://relay-a.example"],
                Vec::<String>::new(),
                Vec::<String>::new(),
                Vec::new(),
            ),
            "acknowledged_relays",
        );

        let observed = RelayDeliveryEvidence::observed(
            ["wss://relay-a.example"],
            ["wss://relay-a.example"],
            ["wss://relay-b.example"],
            vec![failure.clone()],
        )
        .expect("observed evidence");
        assert_eq!(observed.state, RelayDeliveryState::Observed);
        assert_eq!(
            observed.relay_set_fingerprint(),
            Some("wss://relay-a.example\nwss://relay-b.example".to_owned())
        );
        observed.to_json_value().expect("observed evidence json");
        assert_model_error_contains(
            RelayDeliveryEvidence::observed(
                Vec::<String>::new(),
                Vec::<String>::new(),
                ["wss://relay-b.example"],
                Vec::new(),
            ),
            "target_relays",
        );
        assert_model_error_contains(
            RelayDeliveryEvidence::observed(
                ["wss://relay-a.example"],
                Vec::<String>::new(),
                Vec::<String>::new(),
                Vec::new(),
            ),
            "observed_relays",
        );

        let failed = RelayDeliveryEvidence {
            state: RelayDeliveryState::Failed,
            target_relays: vec!["wss://relay-a.example".to_owned()],
            connected_relays: Vec::new(),
            acknowledged_relays: Vec::new(),
            observed_relays: Vec::new(),
            failed_relays: vec![failure],
        };
        let failed_json = failed.to_json_value().expect("failed evidence json");
        assert_eq!(
            RelayDeliveryEvidence::from_json_value(&failed_json).expect("failed evidence"),
            failed
        );

        assert_model_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Pending,
                target_relays: Vec::new(),
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .to_json_value(),
            "target_relays",
        );
        assert_model_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Acknowledged,
                target_relays: vec!["wss://relay-a.example".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .to_json_value(),
            "acknowledged_relays",
        );
        assert_model_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Observed,
                target_relays: vec!["wss://relay-a.example".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .to_json_value(),
            "observed_relays",
        );
        assert_model_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Failed,
                target_relays: vec!["wss://relay-a.example".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .to_json_value(),
            "failed_relays",
        );
        assert_model_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Failed,
                target_relays: vec!["wss://relay-a.example".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: vec![RelayDeliveryFailure {
                    relay_url: " ".to_owned(),
                    error: "timeout".to_owned(),
                }],
            }
            .to_json_value(),
            "failed_relay_url",
        );
        assert_model_error_contains(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Failed,
                target_relays: vec!["wss://relay-a.example".to_owned()],
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: vec![RelayDeliveryFailure {
                    relay_url: "wss://relay-a.example".to_owned(),
                    error: " ".to_owned(),
                }],
            }
            .to_json_value(),
            "failed_relay_error",
        );
        assert_eq!(
            RelayDeliveryEvidence {
                state: RelayDeliveryState::Pending,
                target_relays: Vec::new(),
                connected_relays: Vec::new(),
                acknowledged_relays: Vec::new(),
                observed_relays: Vec::new(),
                failed_relays: Vec::new(),
            }
            .relay_set_fingerprint(),
            None
        );
    }

    #[test]
    fn local_record_input_validation_covers_success_and_error_paths() {
        let mut local_work = local_work_input();
        local_work.validate().expect("valid local work");

        for (field, update) in [
            (
                "owner_account_id",
                Box::new(|input: &mut RuntimeStoreRecordInput| {
                    input.owner_account_id = Some(" ".to_owned());
                }) as Box<dyn Fn(&mut RuntimeStoreRecordInput)>,
            ),
            (
                "owner_pubkey",
                Box::new(|input: &mut RuntimeStoreRecordInput| {
                    input.owner_pubkey = Some(" ".to_owned());
                }),
            ),
            (
                "farm_id",
                Box::new(|input: &mut RuntimeStoreRecordInput| {
                    input.farm_id = Some(" ".to_owned());
                }),
            ),
            (
                "listing_addr",
                Box::new(|input: &mut RuntimeStoreRecordInput| {
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
                Box::new(|input: &mut RuntimeStoreRecordInput| {
                    input.event_id = Some(" ".to_owned());
                }) as Box<dyn Fn(&mut RuntimeStoreRecordInput)>,
            ),
            (
                "event_pubkey",
                Box::new(|input: &mut RuntimeStoreRecordInput| {
                    input.event_pubkey = None;
                }),
            ),
            (
                "event_sig",
                Box::new(|input: &mut RuntimeStoreRecordInput| {
                    input.event_sig = None;
                }),
            ),
            (
                "event_kind",
                Box::new(|input: &mut RuntimeStoreRecordInput| {
                    input.event_kind = None;
                }),
            ),
            (
                "raw_event_json",
                Box::new(|input: &mut RuntimeStoreRecordInput| {
                    input.raw_event_json = None;
                }),
            ),
        ] {
            let mut input = signed_event_input();
            update(&mut input);
            assert_error_contains(input.validate(), field);
        }
    }

    fn local_work_input() -> RuntimeStoreRecordInput {
        RuntimeStoreRecordInput {
            record_id: "local-work-a".to_owned(),
            family: RuntimeStoreRecordFamily::LocalWork,
            status: RuntimeStoreRecordStatus::LocalSaved,
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

    fn signed_event_input() -> RuntimeStoreRecordInput {
        RuntimeStoreRecordInput {
            record_id: "signed-event-a".to_owned(),
            family: RuntimeStoreRecordFamily::SignedEvent,
            status: RuntimeStoreRecordStatus::PendingPublish,
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
            relay_set_fingerprint: None,
            relay_delivery_json: None,
        }
    }

    fn assert_error_contains(result: Result<(), RuntimeStoreError>, expected: &str) {
        let err = result.expect_err("validation error");
        assert!(
            err.to_string().contains(expected),
            "expected error to contain {expected}, got {err}"
        );
    }

    fn assert_model_error_contains<T>(result: Result<T, RuntimeStoreError>, expected: &str) {
        let err = match result {
            Ok(_) => panic!("expected validation error"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains(expected),
            "expected error to contain {expected}, got {err}"
        );
    }
}

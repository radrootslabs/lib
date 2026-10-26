use crate::{
    Admission, ArtifactRole, BusinessInstallation, ChatValue, Delivery, Error, Issuance,
    ListingValue, OperationKind, RecoveryRequirement, SatisfactionClass, TargetObservation, Value,
    WaitTermination,
};

/// Exact protected artifact references, without payload or invocation rights.
#[derive(Clone, uniffi::Record)]
pub struct Artifact {
    pub role: ArtifactRole,
    pub recipient: Option<String>,
    pub expected_event: Option<String>,
    pub issuance: Issuance,
    pub attempt_id: Option<String>,
    pub claim_transaction: Option<String>,
    pub intent_digest: Option<String>,
    pub capture_transaction: Option<String>,
    pub obtained_digest: Option<String>,
    pub publication_transaction: Option<String>,
    pub observed_at_unix_ms: Option<String>,
}
#[derive(Clone, uniffi::Record)]
pub struct Observation {
    pub event_id: String,
    pub recipient: Option<String>,
    pub target: String,
    pub request_id: String,
    pub target_fingerprint: String,
    pub target_ordinal: u8,
    pub outcome: TargetObservation,
    pub observed_at_unix_ms: Option<String>,
}
#[derive(Clone, uniffi::Enum)]
pub enum TargetPolicy {
    Any,
    All,
    Quorum { threshold: u16 },
    Required { fingerprints: Vec<String> },
}
#[derive(Clone, uniffi::Record)]
pub struct Target {
    pub uri: String,
    pub kind: String,
    pub fingerprint: String,
}
#[derive(Clone, uniffi::Record)]
pub struct DeliveryAttempt {
    pub request_id: String,
    pub event_id: String,
    pub role: ArtifactRole,
    pub recipient: Option<String>,
    pub targets: Vec<Target>,
    pub selected_fingerprints: Vec<String>,
    pub satisfaction_class: SatisfactionClass,
    pub policy: TargetPolicy,
    pub publication_policy_digest: String,
    pub intent_digest: String,
    pub original_bytes_digest: String,
    pub transaction_id: String,
    pub work_transaction_id: String,
    pub retained_at: String,
    pub deadline_unix_ms: String,
    pub target_ordinals: Vec<u8>,
}

/// Source-projected current facts. This data is never an executable context or
/// a new authority to sign, install, dispatch or infer recipient delivery.
#[derive(Clone, uniffi::Record)]
pub struct Receipt {
    pub kind: OperationKind,
    pub operation_id: String,
    pub captured_context_json: Vec<u8>,
    pub command_digest: String,
    pub admission: Admission,
    pub admission_transaction_id: String,
    pub plan_transaction_id: Option<String>,
    pub installation_transaction_id: Option<String>,
    pub installed_at: Option<String>,
    pub prepared_at: String,
    pub first_issue_not_after: String,
    pub authority_checkpoint_json: Vec<u8>,
    pub operation_checkpoint: String,
    pub observed_at: String,
    pub business_installation: BusinessInstallation,
    pub business_result: Option<Value>,
    pub issuance: Issuance,
    pub delivery: Delivery,
    pub wait_termination: WaitTermination,
    pub message_id: Option<String>,
    pub record_ids: Vec<String>,
    pub missing_sources: Vec<String>,
    pub conflicting_records: Vec<String>,
    pub conflicting_sources: Vec<String>,
    pub artifacts: Vec<Artifact>,
    pub delivery_attempts: Vec<DeliveryAttempt>,
    pub delivery_observations: Vec<Observation>,
    pub recovery: Vec<RecoveryRequirement>,
    pub error: Option<Error>,
}
impl Receipt {
    pub(crate) fn listing(
        value: &radroots::OperationReceipt<radroots::listings::Listing>,
    ) -> Result<Self, Error> {
        project(
            value,
            value.business_result().map(|v| Value::Listing {
                listing: ListingValue::captured(v),
            }),
        )
    }
    pub(crate) fn chat(
        value: &radroots::OperationReceipt<radroots::domain::ChatMessage>,
    ) -> Result<Self, Error> {
        project(
            value,
            value.business_result().map(|v| Value::Chat {
                message: ChatValue::captured(v),
            }),
        )
    }
    pub(crate) fn status(value: &radroots::OperationStatus) -> Result<Self, Error> {
        let result = value.business_result().map(|v| match v {
            radroots::OperationValue::Listing(v) => Value::Listing {
                listing: ListingValue::captured(v),
            },
            radroots::OperationValue::Chat(v) => Value::Chat {
                message: ChatValue::captured(v),
            },
        });
        project(value, result)
    }
}
fn project<T>(
    value: &radroots::OperationReceipt<T>,
    business_result: Option<Value>,
) -> Result<Receipt, Error> {
    let operation = value.context().operation_id().to_string();
    project_inner(value, business_result).map_err(|v| v.with_operation(operation))
}
fn project_inner<T>(
    value: &radroots::OperationReceipt<T>,
    business_result: Option<Value>,
) -> Result<Receipt, Error> {
    let context = || Error::UnsupportedState {
        axis: "receipt_data".to_owned(),
        operation_id: Some(value.context().operation_id().to_string()),
        signer_rejection: None,
    };
    let artifacts = value
        .artifacts()
        .iter()
        .map(|v| {
            Ok(Artifact {
                role: v.role().try_into()?,
                recipient: v.recipient().map(|v| v.to_hex()),
                expected_event: v.expected_event().map(|v| v.to_string()),
                issuance: v.issuance().try_into()?,
                attempt_id: v.attempt_id().map(|v| v.to_string()),
                claim_transaction: v.claim_transaction_id().map(|v| v.to_string()),
                intent_digest: v.intent_digest().map(|v| hex(&v)),
                capture_transaction: v.capture_transaction_id().map(|v| v.to_string()),
                obtained_digest: v.obtained_digest().map(|v| hex(&v)),
                publication_transaction: v.publication_transaction_id().map(|v| v.to_string()),
                observed_at_unix_ms: v.observed_at_unix_ms().map(|v| v.to_string()),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let observations = value
        .delivery_observations()
        .iter()
        .map(|v| {
            Ok(Observation {
                event_id: v.event_id().to_string(),
                recipient: v.recipient().map(|v| v.to_hex()),
                target: v.target().to_owned(),
                request_id: v.request_id().to_owned(),
                target_fingerprint: v.target_fingerprint().to_owned(),
                target_ordinal: v.target_ordinal(),
                outcome: v.outcome().try_into()?,
                observed_at_unix_ms: v.observed_at_unix_ms().map(|v| v.to_string()),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let attempts = value
        .delivery_attempts()
        .iter()
        .map(|v| {
            let intent = v.intent();
            let selected = intent.policy().targets();
            let policy = if selected.is_any() {
                TargetPolicy::Any
            } else if selected.is_all() {
                TargetPolicy::All
            } else if let Some(threshold) = selected.quorum_threshold() {
                TargetPolicy::Quorum { threshold }
            } else if let Some(required) = selected.required_targets() {
                TargetPolicy::Required {
                    fingerprints: required.iter().map(|v| v.as_str().to_owned()).collect(),
                }
            } else {
                return Err(context());
            };
            Ok(DeliveryAttempt {
                request_id: intent.request_id().as_str().to_owned(),
                event_id: intent.event_id().to_string(),
                role: intent.role().try_into()?,
                recipient: v.work().target().recipient().map(|v| v.to_hex()),
                targets: intent
                    .targets()
                    .targets()
                    .iter()
                    .map(|v| Target {
                        uri: v.uri().as_str().to_owned(),
                        kind: v.kind().as_str().to_owned(),
                        fingerprint: v.fingerprint().as_str().to_owned(),
                    })
                    .collect(),
                selected_fingerprints: intent
                    .selection()
                    .iter()
                    .map(|v| v.as_str().to_owned())
                    .collect(),
                satisfaction_class: intent.policy().class().into(),
                policy,
                publication_policy_digest: hex(intent.publication_policy_digest()),
                intent_digest: hex(intent.digest()),
                original_bytes_digest: hex(v.envelope_digest()),
                transaction_id: v.transaction_id().to_string(),
                work_transaction_id: v.work().transaction_id().to_string(),
                retained_at: v.retained_at().to_string(),
                deadline_unix_ms: intent.deadline_unix_ms().to_string(),
                target_ordinals: v.target_ordinals().to_vec(),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(Receipt {
        kind: value.kind().try_into()?,
        operation_id: value.context().operation_id().to_string(),
        captured_context_json: serde_json::to_vec(value.context()).map_err(|_| context())?,
        command_digest: value.command_digest().to_string(),
        admission: value.admission().try_into()?,
        admission_transaction_id: value.admission_transaction_id().to_string(),
        plan_transaction_id: value.plan_transaction_id().map(|v| v.to_string()),
        installation_transaction_id: value.installation_transaction_id().map(|v| v.to_string()),
        installed_at: value.installed_at().map(|v| v.to_string()),
        prepared_at: value.issuance_window().prepared_at().to_string(),
        first_issue_not_after: value.issuance_window().first_issue_not_after().to_string(),
        authority_checkpoint_json: serde_json::to_vec(value.authority_checkpoint())
            .map_err(|_| context())?,
        operation_checkpoint: value.operation_checkpoint().to_string(),
        observed_at: value.observed_at().to_string(),
        business_installation: value.business_installation().try_into()?,
        business_result,
        issuance: value.issuance().try_into()?,
        delivery: value.delivery().try_into()?,
        wait_termination: value.wait_termination().try_into()?,
        message_id: value.message_id().map(|v| v.to_string()),
        record_ids: value.record_ids().iter().map(ToString::to_string).collect(),
        missing_sources: value
            .missing_sources()
            .iter()
            .map(ToString::to_string)
            .collect(),
        conflicting_records: value
            .conflicting_records()
            .iter()
            .map(ToString::to_string)
            .collect(),
        conflicting_sources: value
            .conflicting_sources()
            .iter()
            .map(ToString::to_string)
            .collect(),
        artifacts,
        delivery_attempts: attempts,
        delivery_observations: observations,
        recovery: value
            .recovery()
            .iter()
            .copied()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, Error>>()?,
        error: value.error().map(Error::from),
    })
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 15) as usize] as char);
    }
    result
}

use super::{
    Admission, ArtifactRole, BusinessInstallation, Delivery, Error, Failure, Issuance,
    OperationKind, RecoveryRequirement, SatisfactionClass, TargetObservation, Value,
    WaitTermination,
};
use wasm_bindgen::prelude::*;

/// Exact protected artifact references, without payload or invocation rights.
#[derive(Clone)]
#[wasm_bindgen]
pub struct Artifact {
    #[wasm_bindgen(readonly)]
    pub role: ArtifactRole,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub recipient: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub expected_event: Option<String>,
    #[wasm_bindgen(readonly)]
    pub issuance: Issuance,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub attempt_id: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub claim_transaction: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub intent_digest: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub capture_transaction: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub obtained_digest: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub publication_transaction: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub observed_at_unix_ms: Option<String>,
}
#[derive(Clone)]
#[wasm_bindgen]
pub struct Observation {
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub event_id: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub recipient: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub target: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub request_id: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub target_fingerprint: String,
    #[wasm_bindgen(readonly)]
    pub target_ordinal: u8,
    #[wasm_bindgen(readonly)]
    pub outcome: TargetObservation,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub observed_at_unix_ms: Option<String>,
}
/// Closed target policy without inventing a default for future variants.
#[derive(Clone)]
#[wasm_bindgen]
pub struct TargetPolicy {
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub kind: String,
    #[wasm_bindgen(readonly)]
    pub threshold: Option<u16>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub fingerprints: Vec<String>,
}
#[derive(Clone)]
#[wasm_bindgen]
pub struct Target {
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub uri: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub kind: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub fingerprint: String,
}
#[derive(Clone)]
#[wasm_bindgen]
pub struct DeliveryAttempt {
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub request_id: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub event_id: String,
    #[wasm_bindgen(readonly)]
    pub role: ArtifactRole,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub recipient: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub targets: Vec<Target>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub selected_fingerprints: Vec<String>,
    #[wasm_bindgen(readonly)]
    pub satisfaction_class: SatisfactionClass,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub policy: TargetPolicy,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub publication_policy_digest: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub intent_digest: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub original_bytes_digest: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub transaction_id: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub work_transaction_id: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub retained_at: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub deadline_unix_ms: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub target_ordinals: Vec<u8>,
}

/// Source-projected current facts. This data is never an executable context or
/// a new authority to sign, install, dispatch or infer recipient delivery.
#[derive(Clone)]
#[wasm_bindgen]
pub struct Receipt {
    #[wasm_bindgen(readonly)]
    pub kind: OperationKind,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub operation_id: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub captured_context_json: Vec<u8>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub command_digest: String,
    #[wasm_bindgen(readonly)]
    pub admission: Admission,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub admission_transaction_id: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub plan_transaction_id: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub installation_transaction_id: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub installed_at: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub prepared_at: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub first_issue_not_after: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub authority_checkpoint_json: Vec<u8>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub operation_checkpoint: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub observed_at: String,
    #[wasm_bindgen(readonly)]
    pub business_installation: BusinessInstallation,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub business_result: Option<Value>,
    #[wasm_bindgen(readonly)]
    pub issuance: Issuance,
    #[wasm_bindgen(readonly)]
    pub delivery: Delivery,
    #[wasm_bindgen(readonly)]
    pub wait_termination: WaitTermination,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub message_id: Option<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub record_ids: Vec<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub missing_sources: Vec<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub conflicting_records: Vec<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub conflicting_sources: Vec<String>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub artifacts: Vec<Artifact>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub delivery_attempts: Vec<DeliveryAttempt>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub delivery_observations: Vec<Observation>,
    recovery: Vec<RecoveryRequirement>,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub error: Option<Failure>,
}
#[wasm_bindgen]
impl Receipt {
    /// Exact closed shared recovery states, never an untyped or defaulted axis.
    #[wasm_bindgen(getter, unchecked_return_type = "RecoveryRequirement[]")]
    pub fn recovery(&self) -> Vec<RecoveryRequirement> {
        self.recovery.clone()
    }
}
impl Receipt {
    pub(crate) fn listing(
        value: &radroots_execution::OperationReceipt<radroots_domain::listing::Listing>,
    ) -> Result<Self, Error> {
        project(value, value.business_result().map(Value::listing))
    }
    pub(crate) fn chat(
        value: &radroots_execution::OperationReceipt<radroots_domain::ChatMessage>,
    ) -> Result<Self, Error> {
        project(value, value.business_result().map(Value::chat))
    }
    pub(crate) fn status(value: &radroots_execution::OperationStatus) -> Result<Self, Error> {
        let result = value.business_result().map(|v| match v {
            radroots_execution::OperationValue::Listing(v) => Value::listing(v),
            radroots_execution::OperationValue::Chat(v) => Value::chat(v),
        });
        project(value, result)
    }
}
fn project<T>(
    value: &radroots_execution::OperationReceipt<T>,
    business_result: Option<Value>,
) -> Result<Receipt, Error> {
    let operation = value.context().operation_id().to_string();
    project_inner(value, business_result).map_err(|v| v.with_operation(operation))
}
fn project_inner<T>(
    value: &radroots_execution::OperationReceipt<T>,
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
                TargetPolicy {
                    kind: "any".into(),
                    threshold: None,
                    fingerprints: vec![],
                }
            } else if selected.is_all() {
                TargetPolicy {
                    kind: "all".into(),
                    threshold: None,
                    fingerprints: vec![],
                }
            } else if let Some(threshold) = selected.quorum_threshold() {
                TargetPolicy {
                    kind: "quorum".into(),
                    threshold: Some(threshold),
                    fingerprints: vec![],
                }
            } else if let Some(required) = selected.required_targets() {
                TargetPolicy {
                    kind: "required".into(),
                    threshold: None,
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
        error: value.error().map(Error::from).map(Failure::from),
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

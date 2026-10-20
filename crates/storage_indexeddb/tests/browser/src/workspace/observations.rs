//! Synthetic normalized facts prove storage linkage, not remote execution.
use super::{WorkspaceFixture, copied};
use core::num::NonZeroU64;
use futures::FutureExt;
use radroots_domain::OperationId;
use radroots_storage::{
    command::{DeliveryObservation, DeliveryObservationOutcome},
    workspace::{AttachmentProof, WorkspaceBinding},
};
use radroots_transport::{
    outcome::{DeliveryOutcome, Retryability},
    sink::{DeliveryRequestId, DeliveryTargetReceipt},
};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

fn summary(value: &DeliveryObservation) -> Value {
    let target = value.target_receipt();
    let outcome = target.outcome();
    json!({"request":value.request_id().as_str(),"event":value.event_id().to_string(),"digest":hex::encode(value.envelope_digest()),"recipient":value.work().private().unwrap().recipient().to_string(),"capture":value.work().evidence().transaction_id().to_string(),"work":value.work().transaction_id().to_string(),"target":target.target().uri().as_str(),"fingerprint":target.target().fingerprint().as_str(),"attempted":target.was_attempted(),"class":value.kind().code(),"ordinal":value.target_ordinal(),"at":value.observed_at_unix_ms().map(|t|t.get()),"code":outcome.code(),"message":outcome.message()})
}
fn result(value: DeliveryObservationOutcome) -> String {
    match value {
        DeliveryObservationOutcome::Committed(value) => {
            json!({"kind":"committed","report":summary(&value)})
        }
        DeliveryObservationOutcome::Unknown(value) => {
            json!({"kind":"unknown","report":summary(&value)})
        }
    }
    .to_string()
}
#[wasm_bindgen]
pub struct ObtainedObservation {
    binding: WorkspaceBinding,
    value: DeliveryObservation,
}
#[wasm_bindgen]
impl ObtainedObservation {
    pub fn original(&self) -> String {
        summary(&self.value).to_string()
    }
}
#[wasm_bindgen]
impl WorkspaceFixture {
    pub async fn observation(
        &self,
        n: u8,
        index: u8,
        id: u8,
        target: u8,
        kind: u8,
        time: u32,
    ) -> Result<ObtainedObservation, JsValue> {
        let request = self.request(n, false).await?;
        let mut recipients = vec![
            self.input.participant(),
            request.chat_command().unwrap().peer(),
        ];
        recipients.sort_unstable();
        recipients.dedup();
        let attempt = self
            .journal()
            .delivery_attempt(
                request.binding().clone(),
                OperationId::from_bytes([n; 32]),
                request.command().digest(),
                recipients[index as usize].into(),
                DeliveryRequestId::parse(hex::encode([id; 32])).unwrap(),
                copied(&self.proof),
            )
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
            .ok_or(JsValue::from_str("missing_attempt"))?;
        let endpoint = attempt.intent().targets().targets()[target as usize].clone();
        let outcome = match kind {
            1 => DeliveryOutcome::accepted(),
            2 => DeliveryOutcome::rejected(),
            7 => DeliveryOutcome::delivered(),
            _ => DeliveryOutcome::failed(Retryability::Retryable).unwrap(),
        };
        let code = match kind {
            3 => "timeout",
            4 => "disconnected",
            _ => "synthetic_result",
        };
        let outcome = outcome
            .with_detail(code, format!("synthetic normalized detail {time}"))
            .unwrap();
        let report = if kind == 6 {
            DeliveryTargetReceipt::skipped(endpoint, outcome).unwrap()
        } else {
            DeliveryTargetReceipt::attempted(endpoint, outcome)
        };
        let value =
            DeliveryObservation::from_store_report(&attempt, &report, NonZeroU64::new(time as u64))
                .map_err(|e| JsValue::from_str(e.code()))?;
        Ok(ObtainedObservation {
            binding: request.binding().clone(),
            value,
        })
    }
    pub async fn retain_observation(
        &self,
        original: &ObtainedObservation,
        wrong_proof: bool,
    ) -> Result<String, JsValue> {
        let proof = if wrong_proof {
            AttachmentProof::from_protected_input(&[0x31; 32]).unwrap()
        } else {
            copied(&self.proof)
        };
        self.journal()
            .record_delivery_observation(original.binding.clone(), proof, original.value.clone())
            .await
            .map(result)
            .map_err(|e| JsValue::from_str(e.code()))
    }
    pub async fn retain_observation_unknown(
        &self,
        original: &ObtainedObservation,
        hook: js_sys::Function,
    ) -> Result<String, JsValue> {
        hook.call0(&JsValue::NULL)?;
        self.journal()
            .record_delivery_observation(
                original.binding.clone(),
                copied(&self.proof),
                original.value.clone(),
            )
            .await
            .map(result)
            .map_err(|e| JsValue::from_str(e.code()))
    }
    pub fn abandon_observation(&self, original: &ObtainedObservation) -> Result<(), JsValue> {
        if self
            .journal()
            .record_delivery_observation(
                original.binding.clone(),
                copied(&self.proof),
                original.value.clone(),
            )
            .now_or_never()
            .is_some()
        {
            return Err(JsValue::from_str("unexpected_immediate_observation"));
        }
        Ok(())
    }
    pub async fn query_observations(&self, n: u8, index: u8, id: u8) -> Result<String, JsValue> {
        let request = self.request(n, false).await?;
        let mut recipients = vec![
            self.input.participant(),
            request.chat_command().unwrap().peer(),
        ];
        recipients.sort_unstable();
        recipients.dedup();
        self.journal()
            .delivery_observations(
                request.binding().clone(),
                OperationId::from_bytes([n; 32]),
                request.command().digest(),
                recipients[index as usize].into(),
                DeliveryRequestId::parse(hex::encode([id; 32])).unwrap(),
                copied(&self.proof),
            )
            .await
            .map(|values| values.iter().map(summary).collect::<Vec<_>>())
            .map(|v| json!(v).to_string())
            .map_err(|e| JsValue::from_str(e.code()))
    }
    pub async fn query_observation_scope(
        &self,
        original: &ObtainedObservation,
    ) -> Result<String, JsValue> {
        let evidence = original.value.work().evidence();
        self.journal()
            .delivery_observations(
                original.binding.clone(),
                evidence.operation_id(),
                evidence.command_digest(),
                evidence.target(),
                original.value.request_id().clone(),
                copied(&self.proof),
            )
            .await
            .map(|values| json!(values.iter().map(summary).collect::<Vec<_>>()).to_string())
            .map_err(|e| JsValue::from_str(e.code()))
    }
}

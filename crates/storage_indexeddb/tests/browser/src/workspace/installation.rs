//! Actual T2 and coherent original-state projections for the isolated fixture.
use super::{WorkspaceFixture, copied};
use futures::FutureExt;
use radroots_storage::command::{
    ChatInstallation, InstallationOutcome, InstallationRequest, OperationSnapshot,
};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

fn installation(value: ChatInstallation) -> Value {
    json!({"transaction":value.transaction_id().to_string(),"message":value.message_id().to_string(),"at":value.installed_at().get(),"basis_capture":value.basis().transaction_id().to_string(),"basis_recipient":value.basis().recipient().unwrap().to_string()})
}
fn snapshot(value: &OperationSnapshot) -> Value {
    json!({
        "checkpoint":value.checkpoint_digest().to_string(),
        "preparation":value.prepared().transaction_id().to_string(),
        "plan":value.bound_plan().map(|bound|bound.transaction_id().to_string()),
        "installation":value.installation().map(installation),
        "publications":value.publications().iter().map(|work|json!({"recipient":work.recipient().to_string(),"transaction":work.transaction_id().to_string(),"capture":work.evidence().transaction_id().to_string(),"installation":work.installation_transaction_id().to_string(),"at":work.queued_at().get()})).collect::<Vec<_>>(),
        "claims":value.artifacts().iter().map(|recipient|json!({"recipient":recipient.recipient().unwrap().to_string(),"claim":recipient.claim().map(|claim|claim.attempt_id().to_string()),"capture":recipient.obtained().map(|capture|capture.transaction_id().to_string())})).collect::<Vec<_>>(),
        "message":value.installed_message().map(|message|json!({"id":message.id().to_string(),"peer":message.peer().to_string(),"text":message.text(),"created_at":message.created_at().get()})),
        "deliveries":value.delivery_attempts().len(),"observations":value.delivery_observations().len(),
    })
}
impl WorkspaceFixture {
    async fn installation_request(&self, n: u8) -> Result<InstallationRequest, JsValue> {
        let request = self.request(n, false).await?;
        let bound = self
            .journal()
            .signing_plan(
                request.binding().clone(),
                request.command().context().operation_id(),
                request.command().digest(),
                copied(&self.proof),
            )
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
            .ok_or(JsValue::from_str("missing_plan"))?;
        Ok(InstallationRequest::new(
            request.binding().clone(),
            request.command().context().operation_id(),
            request.command().digest(),
            *bound.plan().digest(),
        ))
    }
    async fn install_mode(&self, n: u8, after: Option<u64>) -> Result<String, JsValue> {
        let request = self.installation_request(n).await?;
        self.clock.after.set(after);
        match self
            .journal()
            .install_transition_and_outbox(request, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            InstallationOutcome::Committed(value) => Ok(installation(*value).to_string()),
            InstallationOutcome::ListingCommitted(_) => {
                Err(JsValue::from_str("unexpected_listing_installation"))
            }
            InstallationOutcome::Unknown(_) => Err(JsValue::from_str("commit_unknown")),
        }
    }
}
#[wasm_bindgen]
impl WorkspaceFixture {
    pub async fn install(&self, n: u8) -> Result<String, JsValue> {
        self.install_mode(n, None).await
    }
    pub async fn install_bad_clock(&self, n: u8) -> Result<String, JsValue> {
        self.install_mode(n, Some(90)).await
    }
    pub async fn install_expired_grant(&self, n: u8) -> Result<String, JsValue> {
        self.install_mode(n, Some(2000)).await
    }
    pub async fn install_unknown(&self, n: u8, hook: js_sys::Function) -> Result<Vec<u8>, JsValue> {
        let request = self.installation_request(n).await?;
        let operation = request.operation_id();
        let digest = request.command_digest();
        hook.call0(&JsValue::NULL)?;
        match self
            .journal()
            .install_transition_and_outbox(request, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            InstallationOutcome::Unknown(anchor) => {
                assert_eq!(anchor.operation_id(), operation);
                assert_eq!(anchor.command_digest(), digest);
                let mut raw = operation.as_bytes().to_vec();
                raw.extend_from_slice(digest.as_bytes());
                raw.extend_from_slice(anchor.transaction_id().unwrap().as_bytes());
                Ok(raw)
            }
            _ => Err(JsValue::from_str("unexpected_known_installation")),
        }
    }
    pub async fn abandon_install(&self, n: u8) -> Result<(), JsValue> {
        let request = self.installation_request(n).await?;
        if self
            .journal()
            .install_transition_and_outbox(request, copied(&self.proof))
            .now_or_never()
            .is_some()
        {
            return Err(JsValue::from_str("unexpected_immediate_installation"));
        }
        Ok(())
    }
    pub async fn snapshot(&self, n: u8) -> Result<String, JsValue> {
        let request = self.request(n, false).await?;
        self.journal()
            .operation_snapshot(
                request.binding().clone(),
                request.command().context().operation_id(),
                Some(request.command().digest()),
                copied(&self.proof),
            )
            .await
            .map(|value| {
                value
                    .as_ref()
                    .map(snapshot)
                    .unwrap_or(Value::Null)
                    .to_string()
            })
            .map_err(|e| JsValue::from_str(e.code()))
    }
}

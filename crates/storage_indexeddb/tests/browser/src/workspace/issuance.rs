//! Synthetic real-signature S workflow through the actual protected host.
use super::{WorkspaceFixture, copied, plans::BrowserRng};
use core::num::NonZeroU64;
use futures::FutureExt;
use nostr::JsonUtil;
use radroots_storage::command::{
    ClaimAttemptOutcome, ClaimAttemptRequest, EvidenceCaptureOutcome, EvidenceReceipt,
    SealEvidence, SigningAttempt,
};
use wasm_bindgen::prelude::*;

fn summary(attempt: &SigningAttempt) -> Vec<u8> {
    let mut raw = attempt.attempt_id().as_bytes().to_vec();
    raw.extend_from_slice(attempt.transaction_id().as_bytes());
    raw.extend_from_slice(attempt.binding().digest());
    raw.extend_from_slice(&attempt.claimed_at().get().to_be_bytes());
    raw.extend_from_slice(&attempt.deadline_unix_ms().get().to_be_bytes());
    raw.extend_from_slice(&attempt.seal().unwrap().event_id());
    raw
}
fn receipt(value: EvidenceReceipt) -> Vec<u8> {
    let mut raw = value.transaction_id().as_bytes().to_vec();
    raw.extend_from_slice(value.digest());
    raw.extend_from_slice(value.attempt_id().as_bytes());
    raw.extend_from_slice(
        &value
            .observed_at_unix_ms()
            .map_or(0, NonZeroU64::get)
            .to_be_bytes(),
    );
    raw.extend_from_slice(value.event_id().as_bytes());
    raw
}
fn signed_output(
    attempt: &SigningAttempt,
    observed: Option<NonZeroU64>,
) -> Result<SealEvidence, JsValue> {
    let fields = attempt.seal().unwrap().fields();
    let context = nostr::secp256k1::Secp256k1::new();
    let mut scalar = [0; 32];
    scalar[31] = 1;
    let keys = nostr::Keys::new_with_ctx(&context, nostr::SecretKey::from_slice(&scalar).unwrap());
    assert_eq!(keys.public_key().as_bytes(), fields.author.as_bytes());
    let signed = nostr::UnsignedEvent {
        id: None,
        pubkey: keys.public_key(),
        created_at: nostr::Timestamp::from_secs(fields.created_at),
        kind: nostr::Kind::Custom(u16::try_from(fields.kind).unwrap()),
        tags: nostr::Tags::from_list(vec![]),
        content: fields.content.to_owned(),
    }
    .sign_with_ctx(&context, &mut BrowserRng, &keys)
    .unwrap();
    signed.verify_with_ctx(&context).unwrap();
    SealEvidence::verify(attempt, &signed.as_json(), observed)
        .map_err(|e| JsValue::from_str(e.code()))
}
/// The test caller keeps its opaque original through failed/unknown capture.
#[wasm_bindgen]
pub struct ObtainedSeal {
    value: SealEvidence,
    attempt: SigningAttempt,
}
#[wasm_bindgen]
impl ObtainedSeal {
    pub fn original_claim(&self) -> Vec<u8> {
        summary(&self.attempt)
    }
    pub fn digest(&self) -> Vec<u8> {
        self.value.digest().to_vec()
    }
    pub fn malformed_output_refused(&self) -> bool {
        let original: serde_json::Value = serde_json::from_str(self.value.raw_json()).unwrap();
        ["sig", "pubkey", "content", "kind"]
            .into_iter()
            .all(|field| {
                let mut wrong = original.clone();
                wrong[field] = match field {
                    "sig" => serde_json::json!("00".repeat(64)),
                    "pubkey" => serde_json::json!("00".repeat(32)),
                    "content" => serde_json::json!("wrong captured body"),
                    "kind" => serde_json::json!(14),
                    _ => unreachable!(),
                };
                SealEvidence::verify(&self.attempt, &serde_json::to_string(&wrong).unwrap(), None)
                    .is_err()
            })
    }
}
impl WorkspaceFixture {
    async fn claim_request(&self, n: u8, index: u8) -> Result<ClaimAttemptRequest, JsValue> {
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
        let recipient = bound
            .chat_plan()
            .unwrap()
            .seals()
            .get(index as usize)
            .ok_or(JsValue::from_str("invalid_recipient"))?
            .recipient();
        Ok(ClaimAttemptRequest::new(
            request.binding().clone(),
            request.command().context().operation_id(),
            request.command().digest(),
            *bound.plan().digest(),
            recipient,
        ))
    }
}
#[wasm_bindgen]
impl WorkspaceFixture {
    /// Queries through the actual original request even when retained T1 is
    /// damaged; fixture request reconstruction must not hide the host refusal.
    pub async fn captured_original(&self, original: &ObtainedSeal) -> Result<Vec<u8>, JsValue> {
        let bound = original.attempt.bound_plan();
        let identity = bound.prepared().identity();
        let request = ClaimAttemptRequest::new(
            bound.prepared().request().binding().clone(),
            identity.operation_id(),
            identity.command_digest(),
            *bound.plan().digest(),
            original.attempt.seal().unwrap().recipient(),
        );
        self.journal()
            .obtained_evidence(request, copied(&self.proof))
            .await
            .map(|value| value.map(|v| receipt(v.receipt())).unwrap_or_default())
            .map_err(|e| JsValue::from_str(e.code()))
    }
    pub async fn claim(&self, n: u8, index: u8) -> Result<Vec<u8>, JsValue> {
        let request = self.claim_request(n, index).await?;
        match self
            .journal()
            .claim_attempt(request, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            ClaimAttemptOutcome::Claimed(attempt) => {
                let mut raw = vec![1];
                raw.extend(summary(&attempt));
                Ok(raw)
            }
            ClaimAttemptOutcome::Retained(attempt) => {
                let mut raw = vec![0];
                raw.extend(summary(&attempt));
                Ok(raw)
            }
            ClaimAttemptOutcome::Unknown(_) => Err(JsValue::from_str("commit_unknown")),
        }
    }
    pub async fn claim_unknown(
        &self,
        n: u8,
        index: u8,
        hook: js_sys::Function,
    ) -> Result<Vec<u8>, JsValue> {
        let request = self.claim_request(n, index).await?;
        let operation = request.operation_id();
        let digest = request.command_digest();
        hook.call0(&JsValue::NULL)?;
        match self
            .journal()
            .claim_attempt(request, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            ClaimAttemptOutcome::Unknown(anchor) => {
                assert_eq!(anchor.operation_id(), operation);
                assert_eq!(anchor.command_digest(), digest);
                let mut raw = operation.as_bytes().to_vec();
                raw.extend_from_slice(digest.as_bytes());
                raw.extend_from_slice(anchor.transaction_id().unwrap().as_bytes());
                Ok(raw)
            }
            _ => Err(JsValue::from_str("unexpected_known_claim")),
        }
    }
    pub async fn attempt(&self, n: u8, index: u8) -> Result<Vec<u8>, JsValue> {
        let request = self.claim_request(n, index).await?;
        self.journal()
            .signing_attempt(request, copied(&self.proof))
            .await
            .map(|value| value.as_ref().map(summary).unwrap_or_default())
            .map_err(|e| JsValue::from_str(e.code()))
    }
    pub async fn obtain(
        &self,
        n: u8,
        index: u8,
        known_time: bool,
    ) -> Result<ObtainedSeal, JsValue> {
        let request = self.claim_request(n, index).await?;
        let attempt = match self
            .journal()
            .claim_attempt(request, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            ClaimAttemptOutcome::Claimed(attempt) => attempt,
            ClaimAttemptOutcome::Retained(_) => {
                return Err(JsValue::from_str("retained_no_dispatch"));
            }
            ClaimAttemptOutcome::Unknown(_) => return Err(JsValue::from_str("commit_unknown")),
        };
        let value = signed_output(
            &attempt,
            if known_time {
                NonZeroU64::new(1_000_000)
            } else {
                None
            },
        )?;
        Ok(ObtainedSeal { value, attempt })
    }
    /// Constructs a second valid synthetic output solely to test the store's
    /// refusal to replace an original capture. It grants no claim/dispatch.
    pub async fn capture_alternate(&self, original: &ObtainedSeal) -> Result<(), JsValue> {
        let alternate = signed_output(&original.attempt, None)?;
        assert_ne!(alternate.raw_json(), original.value.raw_json());
        self.journal()
            .capture_evidence(self.input.clone(), &self.proof, &alternate)
            .await
            .map(|_| ())
            .map_err(|e| JsValue::from_str(e.code()))
    }
    pub async fn capture_reobserved(&self, original: &ObtainedSeal) -> Result<Vec<u8>, JsValue> {
        let evidence = SealEvidence::verify(
            &original.attempt,
            original.value.raw_json(),
            NonZeroU64::new(1_000_100),
        )
        .unwrap();
        match self
            .journal()
            .capture_evidence(self.input.clone(), &self.proof, &evidence)
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            EvidenceCaptureOutcome::Committed(value) => Ok(receipt(value)),
            EvidenceCaptureOutcome::Unknown(_) => Err(JsValue::from_str("commit_unknown")),
        }
    }
    pub async fn capture(&self, original: &ObtainedSeal) -> Result<Vec<u8>, JsValue> {
        match self
            .journal()
            .capture_evidence(self.input.clone(), &self.proof, &original.value)
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            EvidenceCaptureOutcome::Committed(value) => Ok(receipt(value)),
            EvidenceCaptureOutcome::Unknown(_) => Err(JsValue::from_str("commit_unknown")),
        }
    }
    pub async fn reconcile(
        &self,
        original: &ObtainedSeal,
        wrong_proof: bool,
    ) -> Result<Vec<u8>, JsValue> {
        let proof = if wrong_proof {
            radroots_storage::workspace::AttachmentProof::from_protected_input(&[0x31; 32]).unwrap()
        } else {
            copied(&self.proof)
        };
        match self
            .journal()
            .reconcile_unknown(self.input.clone(), &proof, &original.value)
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            EvidenceCaptureOutcome::Committed(value) => Ok(receipt(value)),
            EvidenceCaptureOutcome::Unknown(_) => Err(JsValue::from_str("commit_unknown")),
        }
    }
    pub fn abandon_reconcile(&self, original: &ObtainedSeal) -> Result<(), JsValue> {
        if self
            .journal()
            .reconcile_unknown(self.input.clone(), &self.proof, &original.value)
            .now_or_never()
            .is_some()
        {
            return Err(JsValue::from_str("unexpected_immediate_recovery"));
        }
        Ok(())
    }
    pub async fn capture_unknown(&self, original: &ObtainedSeal) -> Result<Vec<u8>, JsValue> {
        match self
            .journal()
            .capture_evidence(self.input.clone(), &self.proof, &original.value)
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            EvidenceCaptureOutcome::Unknown(anchor) => {
                assert_eq!(anchor.operation_id(), original.value.operation_id());
                assert_eq!(anchor.command_digest(), original.value.command_digest());
                let mut raw = anchor.operation_id().as_bytes().to_vec();
                raw.extend_from_slice(anchor.command_digest().as_bytes());
                raw.extend_from_slice(anchor.transaction_id().unwrap().as_bytes());
                Ok(raw)
            }
            _ => Err(JsValue::from_str("unexpected_known_capture")),
        }
    }
    pub async fn captured(&self, n: u8, index: u8) -> Result<Vec<u8>, JsValue> {
        let request = self.claim_request(n, index).await?;
        self.journal()
            .obtained_evidence(request, copied(&self.proof))
            .await
            .map(|value| {
                value
                    .map(|value| receipt(value.receipt()))
                    .unwrap_or_default()
            })
            .map_err(|e| JsValue::from_str(e.code()))
    }
    pub fn abandon_capture(&self, original: &ObtainedSeal) -> Result<(), JsValue> {
        if self
            .journal()
            .capture_evidence(self.input.clone(), &self.proof, &original.value)
            .now_or_never()
            .is_some()
        {
            return Err(JsValue::from_str("unexpected_immediate_capture"));
        }
        Ok(())
    }
}

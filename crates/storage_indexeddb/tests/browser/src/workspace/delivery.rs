//! Actual protected D ports using independently signed synthetic carriers.
use super::{WorkspaceFixture, copied, plans::BrowserRng};
use core::num::NonZeroU64;
use futures::FutureExt;
use nostr::{JsonUtil, nips::nip44};
use radroots_domain::{OperationId, PublicKey};
use radroots_storage::command::{
    ClaimAttemptRequest, DeliveryAttempt, DeliveryAttemptOutcome, DeliveryAttemptRequest,
    DeliveryIntent, EnvelopeCandidate,
};
use radroots_transport::{
    Target, TargetSet,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryRequestId,
};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct ObtainedEnvelope {
    candidate: EnvelopeCandidate,
}
#[wasm_bindgen]
impl ObtainedEnvelope {
    pub fn event(&self) -> String {
        self.candidate.event_id().to_string()
    }
    pub fn raw(&self) -> String {
        self.candidate.raw_json().into()
    }
}
fn summary(report: &DeliveryAttempt) -> Value {
    json!({"request":report.intent().request_id().as_str(),"event":report.intent().event_id().to_string(),"transaction":report.transaction_id().to_string(),"at":report.retained_at().get(),"raw":report.raw_json(),"digest":hex::encode(report.envelope_digest()),"policy":hex::encode(report.intent().publication_policy_digest()),"targets":report.intent().targets().targets().iter().map(|target|target.uri().as_str()).collect::<Vec<_>>(),"selection":report.intent().selected_targets().map(|target|target.uri().as_str()).collect::<Vec<_>>(),"ordinals":report.target_ordinals()})
}
fn outcome(value: DeliveryAttemptOutcome) -> Value {
    match value {
        DeliveryAttemptOutcome::Claimed(report) => {
            json!({"kind":"claimed","report":summary(&report)})
        }
        DeliveryAttemptOutcome::Retained(report) => {
            json!({"kind":"retained","report":summary(&report)})
        }
        DeliveryAttemptOutcome::Unknown { operation, attempt } => {
            json!({"kind":"unknown","operation":operation.operation_id().to_string(),"command":operation.command_digest().to_string(),"transaction":operation.transaction_id().map(|tx|tx.to_string()),"request":attempt.as_str()})
        }
    }
}
impl WorkspaceFixture {
    async fn delivery_request(
        &self,
        n: u8,
        id: u8,
        selection: u16,
        mode: u8,
        envelope: &ObtainedEnvelope,
    ) -> Result<DeliveryAttemptRequest, JsValue> {
        let request = self.request(n, false).await?;
        let mut relays = super::plans::delivery_relays(mode & 15 == 6);
        if mode & 15 == 4 {
            relays[1] = "wss://unapproved.fixture.example/".into();
        }
        let targets = TargetSet::new(
            relays
                .iter()
                .map(|relay| Target::nostr_relay(relay).unwrap())
                .collect(),
        )
        .unwrap();
        let policy = SatisfactionPolicy::new(
            SatisfactionClass::Accepted,
            if mode & 15 == 5 {
                TargetPolicy::any()
            } else {
                TargetPolicy::all()
            },
        );
        let selected = TargetSet::new(
            targets
                .targets()
                .iter()
                .enumerate()
                .filter(|(index, _)| selection & (1 << index) != 0)
                .map(|(_, target)| target.clone())
                .collect(),
        )
        .unwrap();
        let intent = DeliveryIntent::new(
            &hex::encode([id; 32]),
            envelope.candidate.event_id(),
            &targets,
            &policy,
            NonZeroU64::new(1_800_000).unwrap(),
        )
        .unwrap()
        .with_selection(&selected)
        .unwrap();
        DeliveryAttemptRequest::new(request.binding().clone(), envelope.candidate.work(), intent)
            .map_err(|error| JsValue::from_str(error.code()))
    }
}
#[wasm_bindgen]
impl WorkspaceFixture {
    pub fn alternate_envelope(
        &self,
        original: &ObtainedEnvelope,
    ) -> Result<ObtainedEnvelope, JsValue> {
        let event = nostr::Event::from_json(original.candidate.raw_json()).unwrap();
        let context = nostr::secp256k1::Secp256k1::new();
        let mut scalar = [0; 32];
        scalar[31] = 3;
        let keys =
            nostr::Keys::new_with_ctx(&context, nostr::SecretKey::from_slice(&scalar).unwrap());
        let alternate = nostr::UnsignedEvent {
            id: Some(event.id),
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags.clone(),
            content: event.content.clone(),
        }
        .sign_with_ctx(&context, &mut BrowserRng, &keys)
        .unwrap();
        alternate.verify_with_ctx(&context).unwrap();
        assert_eq!(alternate.id, event.id);
        assert_ne!(alternate.as_json(), event.as_json());
        Ok(ObtainedEnvelope {
            candidate: EnvelopeCandidate::from_obtained(
                original.candidate.work(),
                &alternate.as_json(),
                &scalar,
            )
            .map_err(|e| JsValue::from_str(e.code()))?,
        })
    }
    pub async fn envelope(&self, n: u8, index: u8, mode: u8) -> Result<ObtainedEnvelope, JsValue> {
        let request = self.request(n, false).await?;
        let snapshot = self
            .journal()
            .operation_snapshot(
                request.binding().clone(),
                OperationId::from_bytes([n; 32]),
                Some(request.command().digest()),
                copied(&self.proof),
            )
            .await
            .map_err(|error| JsValue::from_str(error.code()))?
            .ok_or(JsValue::from_str("missing_operation"))?;
        let recipient = snapshot
            .bound_plan()
            .unwrap()
            .chat_plan()
            .unwrap()
            .seals()
            .get(index as usize)
            .ok_or(JsValue::from_str("invalid_recipient"))?
            .recipient();
        let work = snapshot
            .publications()
            .iter()
            .find(|work| work.recipient() == recipient)
            .copied()
            .ok_or(JsValue::from_str("missing_work"))?;
        let retained = self
            .journal()
            .obtained_evidence(
                ClaimAttemptRequest::new(
                    request.binding().clone(),
                    OperationId::from_bytes([n; 32]),
                    request.command().digest(),
                    *snapshot.bound_plan().unwrap().plan().digest(),
                    recipient,
                ),
                copied(&self.proof),
            )
            .await
            .map_err(|error| JsValue::from_str(error.code()))?
            .ok_or(JsValue::from_str("missing_output"))?;
        let context = nostr::secp256k1::Secp256k1::new();
        let mut scalar = [0; 32];
        scalar[31] = if mode == 3 { 1 } else { 3 };
        let keys =
            nostr::Keys::new_with_ctx(&context, nostr::SecretKey::from_slice(&scalar).unwrap());
        let recipient = nostr::PublicKey::from_slice(recipient.as_bytes()).unwrap();
        let plain = if mode == 2 {
            "different original seal"
        } else {
            retained.evidence().raw_json()
        };
        let encrypted = nip44::encrypt_with_rng(
            &mut BrowserRng,
            keys.secret_key(),
            &recipient,
            plain,
            nip44::Version::V2,
        )
        .unwrap();
        let event = nostr::UnsignedEvent {
            id: None,
            pubkey: keys.public_key(),
            created_at: nostr::Timestamp::from_secs(900),
            kind: nostr::Kind::GiftWrap,
            tags: nostr::Tags::from_list(vec![nostr::Tag::public_key(recipient)]),
            content: encrypted,
        }
        .sign_with_ctx(&context, &mut BrowserRng, &keys)
        .unwrap();
        event.verify_with_ctx(&context).unwrap();
        if mode == 1 {
            scalar[31] = 4;
        }
        let candidate = EnvelopeCandidate::from_obtained(work, &event.as_json(), &scalar)
            .map_err(|error| JsValue::from_str(error.code()))?;
        Ok(ObtainedEnvelope { candidate })
    }
    pub async fn store_envelope(
        &self,
        n: u8,
        id: u8,
        selection: u16,
        mode: u8,
        envelope: &ObtainedEnvelope,
    ) -> Result<String, JsValue> {
        let request = self
            .delivery_request(n, id, selection, mode, envelope)
            .await?;
        self.clock.after.set(match mode & 15 {
            1 => Some(900),
            2 => Some(2000),
            3 => Some(1800),
            _ => None,
        });
        self.journal()
            .prepare_delivery_attempt(
                request,
                copied(&self.proof),
                if mode & 16 != 0 {
                    None
                } else {
                    Some(&envelope.candidate)
                },
            )
            .await
            .map(|value| outcome(value).to_string())
            .map_err(|error| JsValue::from_str(error.code()))
    }
    pub async fn store_envelope_unknown(
        &self,
        n: u8,
        id: u8,
        selection: u16,
        envelope: &ObtainedEnvelope,
        hook: js_sys::Function,
    ) -> Result<String, JsValue> {
        let request = self.delivery_request(n, id, selection, 0, envelope).await?;
        hook.call0(&JsValue::NULL)?;
        self.journal()
            .prepare_delivery_attempt(request, copied(&self.proof), Some(&envelope.candidate))
            .await
            .map(|value| outcome(value).to_string())
            .map_err(|error| JsValue::from_str(error.code()))
    }
    pub async fn abandon_envelope(
        &self,
        n: u8,
        id: u8,
        selection: u16,
        envelope: &ObtainedEnvelope,
    ) -> Result<(), JsValue> {
        let request = self.delivery_request(n, id, selection, 0, envelope).await?;
        if self
            .journal()
            .prepare_delivery_attempt(request, copied(&self.proof), Some(&envelope.candidate))
            .now_or_never()
            .is_some()
        {
            return Err(JsValue::from_str("unexpected_immediate_delivery_retention"));
        }
        Ok(())
    }
    pub async fn query_envelope(&self, n: u8, index: u8, id: u8) -> Result<String, JsValue> {
        let request = self.request(n, false).await?;
        let mut recipients = vec![
            self.input.participant(),
            request.chat_command().unwrap().peer(),
        ];
        recipients.sort_unstable();
        recipients.dedup();
        let recipient: PublicKey = *recipients
            .get(index as usize)
            .ok_or(JsValue::from_str("invalid_recipient"))?;
        self.journal()
            .delivery_attempt(
                request.binding().clone(),
                OperationId::from_bytes([n; 32]),
                request.command().digest(),
                recipient,
                DeliveryRequestId::parse(hex::encode([id; 32])).unwrap(),
                copied(&self.proof),
            )
            .await
            .map(|value| {
                value
                    .as_ref()
                    .map(summary)
                    .unwrap_or(Value::Null)
                    .to_string()
            })
            .map_err(|error| JsValue::from_str(error.code()))
    }
}

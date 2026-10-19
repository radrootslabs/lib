//! Real synthetic peer/self NIP-44 and signed route inputs for T1 persistence.
use super::{WorkspaceFixture, copied};
use futures::FutureExt;
use js_sys::Reflect;
use nostr::{
    JsonUtil,
    nips::nip44,
    prelude::rand::{CryptoRng, Error, RngCore},
};
use radroots_domain::{EnvelopeEventId, UnixSeconds};
use radroots_protocol::chat::{CapturedDmRoute, ChatReplay, ChatRumor, ChatSeal, ChatSigningPlan};
use radroots_storage::command::{BoundSigningPlan, PlanBindingOutcome};
use radroots_storage::workspace::WorkspaceStore;
use wasm_bindgen::{JsCast, prelude::*};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn fixture_panic(message: &str);
}

pub(super) struct BrowserRng;
impl RngCore for BrowserRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }
    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }
    fn fill_bytes(&mut self, bytes: &mut [u8]) {
        Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
            .unwrap()
            .dyn_into::<web_sys::Crypto>()
            .unwrap()
            .get_random_values_with_u8_array(bytes)
            .unwrap();
    }
    fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), Error> {
        self.fill_bytes(bytes);
        Ok(())
    }
}
impl CryptoRng for BrowserRng {}
fn summary(bound: BoundSigningPlan) -> Vec<u8> {
    let mut bytes = bound.transaction_id().as_bytes().to_vec();
    bytes.extend_from_slice(bound.plan().digest());
    bytes.extend_from_slice(&bound.bound_at().get().to_be_bytes());
    bytes.extend_from_slice(&(bound.plan().protected_wire().len() as u32).to_be_bytes());
    bytes.extend_from_slice(bound.prepared().transaction_id().as_bytes());
    bytes
}
#[wasm_bindgen]
impl WorkspaceFixture {
    pub async fn new_plan(&self, n: u8) -> Result<Vec<u8>, JsValue> {
        std::panic::set_hook(Box::new(|info| fixture_panic(&format!("{info}"))));
        let request = self.request(n, false).await?;
        let command = request.command();
        let time = UnixSeconds::new(1000).unwrap();
        let rumor = ChatRumor::from_command(command, time).unwrap();
        let context = nostr::secp256k1::Secp256k1::new();
        let mut scalar = [0; 32];
        scalar[31] = 1;
        let sender =
            nostr::Keys::new_with_ctx(&context, nostr::SecretKey::from_slice(&scalar).unwrap());
        let mut seals = Vec::new();
        let mut routes = Vec::new();
        let mut rng = BrowserRng;
        for recipient in rumor.recipients() {
            let public = nostr::PublicKey::from_slice(recipient.as_bytes()).unwrap();
            let encrypted = nip44::encrypt_with_rng(
                &mut rng,
                sender.secret_key(),
                &public,
                rumor.json(),
                nip44::Version::V2,
            )
            .unwrap();
            scalar[31] = if *recipient == super::participant() {
                1
            } else {
                2
            };
            let receiver =
                nostr::Keys::new_with_ctx(&context, nostr::SecretKey::from_slice(&scalar).unwrap());
            assert_eq!(receiver.public_key(), public);
            assert_eq!(
                nip44::decrypt(receiver.secret_key(), &sender.public_key(), &encrypted).unwrap(),
                rumor.json()
            );
            seals.push(
                ChatSeal::new(
                    &rumor,
                    *recipient,
                    UnixSeconds::new(999).unwrap(),
                    &encrypted,
                )
                .unwrap(),
            );
            let relay = "wss://plan.fixture.example/";
            let source = nostr::UnsignedEvent {
                id: None,
                pubkey: public,
                created_at: nostr::Timestamp::from_secs(999),
                kind: nostr::Kind::Custom(10050),
                tags: nostr::Tags::from_list(vec![nostr::Tag::parse(["relay", relay]).unwrap()]),
                content: "synthetic public route metadata ".repeat(1000),
            }
            .sign_with_ctx(&context, &mut rng, &receiver)
            .unwrap();
            source.verify_with_ctx(&context).unwrap();
            routes.push(
                CapturedDmRoute::from_observed_descriptor(
                    *recipient,
                    EnvelopeEventId::from_bytes(*source.id.as_bytes()),
                    &source.as_json(),
                    &[relay.into()],
                )
                .unwrap(),
            );
        }
        Ok(
            ChatSigningPlan::new(command, time, seals, routes, ChatReplay::NonReplayable)
                .unwrap()
                .protected_wire()
                .to_vec(),
        )
    }
    pub async fn bind_plan(&self, n: u8, wire: Vec<u8>) -> Result<Vec<u8>, JsValue> {
        self.bind_plan_mode(n, wire, 0).await
    }
    pub async fn bind_plan_read_unknown(
        &self,
        n: u8,
        wire: Vec<u8>,
        hook: js_sys::Function,
    ) -> Result<Vec<u8>, JsValue> {
        let request = self.request(n, false).await?;
        let plan = ChatSigningPlan::parse(request.command(), &wire).unwrap();
        hook.call0(&JsValue::NULL)?;
        match self
            .owner
            .bind_signing_intents(request.binding().clone(), plan, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            PlanBindingOutcome::Committed(_) => Err(JsValue::from_str("unexpected_commit")),
            PlanBindingOutcome::Unknown(anchor) => {
                assert_eq!(anchor.workspace_id(), self.input.workspace_id());
                assert_eq!(
                    anchor.operation_id(),
                    request.command().context().operation_id()
                );
                assert_eq!(anchor.command_digest(), request.command().digest());
                assert_eq!(anchor.transaction_id(), None);
                let mut bytes = anchor.operation_id().as_bytes().to_vec();
                bytes.extend_from_slice(anchor.command_digest().as_bytes());
                Ok(bytes)
            }
        }
    }
    pub async fn bind_plan_foreign(&self, n: u8, wire: Vec<u8>) -> Result<(), JsValue> {
        let request = self.request(n, false).await?;
        let plan = ChatSigningPlan::parse(request.command(), &wire).unwrap();
        let sibling = self.sibling().await?;
        let foreign = sibling
            .owner
            .authenticate(sibling.input.clone(), copied(&sibling.proof))
            .await
            .map_err(super::fail)?;
        let result = self
            .owner
            .bind_signing_intents(foreign, plan, copied(&self.proof))
            .await;
        sibling.close().await?;
        result.map(|_| ()).map_err(|e| JsValue::from_str(e.code()))
    }
    pub async fn bind_plan_expiring(&self, n: u8, wire: Vec<u8>) -> Result<Vec<u8>, JsValue> {
        self.bind_plan_mode(n, wire, 1).await
    }
    pub async fn bind_plan_unknown(&self, n: u8, wire: Vec<u8>) -> Result<Vec<u8>, JsValue> {
        self.bind_plan_mode(n, wire, 2).await
    }
    pub async fn bind_plan_bad_clock(&self, n: u8, wire: Vec<u8>) -> Result<Vec<u8>, JsValue> {
        self.bind_plan_mode(n, wire, 3).await
    }
    pub async fn bind_plan_expired_grant(&self, n: u8, wire: Vec<u8>) -> Result<Vec<u8>, JsValue> {
        self.bind_plan_mode(n, wire, 4).await
    }
    pub async fn abandon_plan(&self, n: u8, wire: Vec<u8>) -> Result<(), JsValue> {
        let request = self.request(n, false).await?;
        let plan = ChatSigningPlan::parse(request.command(), &wire).unwrap();
        if self
            .owner
            .bind_signing_intents(request.binding().clone(), plan, copied(&self.proof))
            .now_or_never()
            .is_some()
        {
            return Err(JsValue::from_str("unexpected_immediate_plan"));
        }
        Ok(())
    }
    pub async fn plan_query(&self, n: u8) -> Result<Vec<u8>, JsValue> {
        let request = self.request(n, false).await?;
        self.owner
            .signing_plan(
                request.binding().clone(),
                request.command().context().operation_id(),
                request.command().digest(),
                copied(&self.proof),
            )
            .await
            .map(|bound| bound.map(summary).unwrap_or_default())
            .map_err(|e| JsValue::from_str(e.code()))
    }
}
impl WorkspaceFixture {
    async fn bind_plan_mode(&self, n: u8, wire: Vec<u8>, mode: u8) -> Result<Vec<u8>, JsValue> {
        let request = self.request(n, false).await?;
        let plan = ChatSigningPlan::parse(request.command(), &wire)
            .map_err(|_| JsValue::from_str("invalid_plan"))?;
        if mode == 1 {
            self.clock.after.set(Some(1500));
        }
        if mode == 3 {
            self.clock.after.set(Some(90));
        }
        if mode == 4 {
            self.clock.after.set(Some(2000));
        }
        let outcome = self
            .owner
            .bind_signing_intents(request.binding().clone(), plan, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?;
        match outcome {
            PlanBindingOutcome::Committed(bound) => Ok(summary(bound)),
            PlanBindingOutcome::Unknown(anchor) => {
                if mode != 2 {
                    return Err(JsValue::from_str("commit_unknown"));
                }
                assert_eq!(
                    anchor.operation_id(),
                    request.command().context().operation_id()
                );
                assert_eq!(anchor.command_digest(), request.command().digest());
                let mut bytes = anchor.operation_id().as_bytes().to_vec();
                bytes.extend_from_slice(anchor.transaction_id().unwrap().as_bytes());
                Ok(bytes)
            }
        }
    }
}

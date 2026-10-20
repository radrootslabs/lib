#![forbid(unsafe_code)]
use futures::FutureExt;
use nostr::JsonUtil;
use radroots_browser::{
    BrowserConfig, BrowserNonceSource, Client, InquireInput, SessionState, StartMode,
    WorkspaceSession,
};
use radroots_domain::{OperationId, PublicKey, ThreadId, UnixSeconds};
use radroots_nostr::{key::parse_secret_key, signing::LocalSigner};
use radroots_secrets::{
    SecretId, SecretRef,
    browser::BrowserProvider,
    id::{BackendKind, KeyVersion},
    provider::{AccessPolicy, HardwarePolicy, ResidencyPolicy, UserPresencePolicy},
};
use radroots_signing::{
    provider::{NonceSource, ProviderError, SignerCallbacks, SignerHandle},
    signer::BoxFuture,
};
use radroots_storage::workspace::{
    AttachmentInput, AttachmentProof, WorkspaceClock, WorkspaceError,
};
use radroots_storage_indexeddb::workspace::{BrowserWorkspace, LocalOutboxConfig};
use radroots_transport::policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy};
use radroots_transport_nostr::{
    Config, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = globalThis, js_name = browserSocketCount)]
    fn socket_count() -> u32;
    #[wasm_bindgen(catch, js_namespace = globalThis, js_name = assertStorageCallbacksClosed)]
    fn storage_callbacks_closed() -> Result<(), JsValue>;
}
fn require(value: bool, message: &str) -> Result<(), JsValue> {
    if value { Ok(()) } else { Err(message.into()) }
}
fn fail(error: impl std::fmt::Debug) -> JsValue {
    JsValue::from_str(&format!("fixture: {error:?}"))
}
fn now() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}
fn proof_copy(proof: &AttachmentProof) -> AttachmentProof {
    proof.with_secret(|secret| AttachmentProof::from_protected_input(secret).unwrap())
}
fn reference_copy(reference: &SecretRef) -> SecretRef {
    SecretRef::new(
        reference.id().clone(),
        reference.backend(),
        reference.key_version(),
    )
}
fn key(last: u8) -> nostr::Keys {
    let mut bytes = [0; 32];
    bytes[31] = last;
    nostr::Keys::new(nostr::SecretKey::from_slice(&bytes).unwrap())
}
fn participant(last: u8) -> PublicKey {
    PublicKey::from_bytes(key(last).public_key().to_bytes()).unwrap()
}
fn access() -> AccessPolicy {
    AccessPolicy::new(
        ResidencyPolicy::Any,
        UserPresencePolicy::NotRequired,
        HardwarePolicy::Any,
    )
}
struct Clock;
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        UnixSeconds::new(now()).map_err(|_| WorkspaceError::Unavailable)
    }
}
struct Callbacks {
    quiesced: Cell<u32>,
    closed: Cell<u32>,
}
impl SignerCallbacks for Callbacks {
    fn quiesce(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async move {
            self.quiesced.set(self.quiesced.get() + 1);
            Ok(())
        })
    }
    fn close(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async move {
            self.closed.set(self.closed.get() + 1);
            Ok(())
        })
    }
}
fn signer_clock() -> Result<u64, radroots_signing::Error> {
    Ok(js_sys::Date::now() as u64)
}
async fn signer(client: &Client) -> Result<Rc<Callbacks>, JsValue> {
    let signer = LocalSigner::with_clock(
        parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
            .unwrap(),
        signer_clock,
    )
    .unwrap()
    .with_nonce_source(Rc::new(BrowserNonceSource));
    let callbacks = Rc::new(Callbacks {
        quiesced: Cell::new(0),
        closed: Cell::new(0),
    });
    client
        .attach_signer(SignerHandle::new(Rc::new(signer), callbacks.clone()))
        .await
        .map_err(fail)?;
    Ok(callbacks)
}
struct Existing {
    namespace: SecretId,
    reference: SecretRef,
    fingerprint: [u8; 32],
    input: AttachmentInput,
    proof: AttachmentProof,
}
impl Existing {
    async fn create(label: &str) -> Result<Self, JsValue> {
        let namespace = SecretId::parse(label).unwrap();
        let reference = SecretRef::new(
            SecretId::parse("ordinary_browser_workspace").unwrap(),
            BackendKind::External,
            KeyVersion::new(1).unwrap(),
        );
        let provider = Rc::new(
            BrowserProvider::create(namespace.clone(), reference_copy(&reference))
                .await
                .map_err(fail)?,
        );
        let fingerprint = provider.fingerprint();
        let (workspace, input, proof) = BrowserWorkspace::initialize_local_outbox(
            LocalOutboxConfig::new(
                participant(1),
                participant(3),
                "isolated-browser-fixture",
                UnixSeconds::new(now() + 600).unwrap(),
            )
            .unwrap(),
            provider.clone(),
            reference_copy(&reference),
            access(),
            Rc::new(Clock),
        )
        .await
        .map_err(fail)?;
        workspace.close().await.map_err(fail)?;
        provider.close().await.map_err(fail)?;
        Ok(Self {
            namespace,
            reference,
            fingerprint,
            input,
            proof,
        })
    }
    fn config(&self, base: &str) -> BrowserConfig {
        let transport = Config::from_profile(
            RelayProfile::explicit(
                RelayProfileKind::Simulator,
                [
                    RelayEndpoint::new(base, RelayUrlPolicy::Local, RelayAccess::ReadWrite)
                        .unwrap(),
                ],
            )
            .unwrap(),
        )
        .with_timeouts(2000, 2000, 2000)
        .unwrap();
        BrowserConfig::existing(
            self.namespace.clone(),
            reference_copy(&self.reference),
            self.fingerprint,
            self.input.clone(),
            proof_copy(&self.proof),
            access(),
        )
        .with_nostr(
            transport,
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        )
        .unwrap()
    }
    async fn open(&self, base: &str) -> Result<(Client, WorkspaceSession), JsValue> {
        let client = Client::open(self.config(base)).await.map_err(fail)?;
        require(
            client.state() == SessionState::Ready,
            "open must remain ready",
        )?;
        let wrong = AttachmentProof::from_protected_input(&[99; 32]).unwrap();
        require(
            client.attach(self.input.clone(), wrong).await.is_err(),
            "key ownership cannot replace attachment proof",
        )?;
        let scope = client
            .attach(self.input.clone(), proof_copy(&self.proof))
            .await
            .map_err(fail)?;
        Ok((client, scope))
    }
}
fn id() -> OperationId {
    OperationId::from_bytes(BrowserNonceSource.next_nonce().unwrap())
}
fn inquire() -> InquireInput {
    InquireInput::new(
        ThreadId::from_bytes([33; 32]),
        participant(2),
        None,
        "ordinary browser inquiry",
    )
    .unwrap()
}
fn context(
    scope: &WorkspaceSession,
    operation: OperationId,
    deadline: u64,
) -> radroots_browser::CommandContext {
    scope
        .command_context(operation, None, None, UnixSeconds::new(deadline).unwrap())
        .unwrap()
}

#[wasm_bindgen]
pub fn advertisements(base: String) -> String {
    let values: Vec<_> = (1..=2)
        .map(|last| {
            nostr::EventBuilder::new(nostr::Kind::Custom(10050), "")
                .tags([nostr::Tag::parse(["relay", base.as_str()]).unwrap()])
                .custom_created_at(nostr::Timestamp::from_secs(now() - 1))
                .sign_with_keys(&key(last))
                .unwrap()
                .as_json()
        })
        .collect();
    serde_json::to_string(&values).unwrap()
}
#[wasm_bindgen]
pub async fn verify_client(base: String, label: String) -> Result<String, JsValue> {
    let mut passed = Vec::new();
    let existing = Existing::create(&label).await?;
    // Build the alternate selection without exposing a key or mutating storage.
    let mut wrong_pin = existing.fingerprint;
    wrong_pin[0] ^= 1;
    let wrong = BrowserConfig::existing(
        existing.namespace.clone(),
        reference_copy(&existing.reference),
        wrong_pin,
        existing.input.clone(),
        proof_copy(&existing.proof),
        access(),
    );
    require(
        Client::open(wrong).await.is_err(),
        "wrong provider identity must refuse",
    )?;
    let (client, scope) = existing.open(&base).await?;
    let callbacks = signer(&client).await?;
    client.start(StartMode::Online).await.map_err(fail)?;
    require(
        socket_count() == 0,
        "open, attach and start cannot perform network work",
    )?;
    passed.push("real existing IndexedDB and explicit authority, inert startup");
    let operation = id();
    let deadline = now() + 120;
    let receipt = client
        .inbox()
        .inquire(context(&scope, operation, deadline), inquire())
        .await
        .map_err(fail)?;
    require(
        receipt.durable() && receipt.business_result().is_some(),
        "real inquiry must install business result",
    )?;
    let original = receipt.operation_checkpoint();
    let message = receipt.business_result().unwrap().id().to_string();
    let replay = client
        .inbox()
        .inquire(context(&scope, operation, deadline), inquire())
        .await
        .map_err(fail)?;
    require(
        replay.operation_checkpoint() == original,
        "replay must retain original checkpoint",
    )?;
    passed.push("shared T0 T1 real signing S T2 and self/peer relay acceptance");
    passed.push("same original command does not replace issuance or publication");
    require(
        client.close().await.map_err(fail)?.storage_closed(),
        "actual close receipt",
    )?;
    storage_callbacks_closed()?;
    require(
        callbacks.closed.get() == 1 && callbacks.quiesced.get() == 1,
        "original signer callbacks close once",
    )?;
    require(
        client.close().await.map_err(fail)?.storage_closed(),
        "repeated close",
    )?;
    require(
        client.operations(&scope).get(operation).await.is_err(),
        "closed resource admission remains closed",
    )?;
    let sockets = socket_count();
    let (local, scope) = existing.open(&base).await?;
    local.start(StartMode::LocalOnly).await.map_err(fail)?;
    let retained = local
        .operations(&scope)
        .get(operation)
        .await
        .map_err(fail)?
        .ok_or_else(|| JsValue::from_str("missing original operation"))?;
    require(
        retained.operation_checkpoint() == original,
        "actual reopen retains original checkpoint",
    )?;
    require(
        socket_count() == sockets,
        "reopen/read/local start cannot open a socket",
    )?;
    passed.push("actual protected close/reopen retains original completed operation");
    let lost = id();
    let inbox = local.inbox();
    let mut observer = Box::pin(inbox.inquire(context(&scope, lost, deadline), inquire()));
    require(
        observer.as_mut().now_or_never().is_none(),
        "owned command starts pending",
    )?;
    drop(observer);
    local.close().await.map_err(fail)?;
    let (resumed, scope) = existing.open(&base).await?;
    let callbacks = signer(&resumed).await?;
    resumed.start(StartMode::Online).await.map_err(fail)?;
    let pending = resumed
        .operations(&scope)
        .get(lost)
        .await
        .map_err(fail)?
        .ok_or_else(|| JsValue::from_str("observer loss erased original T0"))?;
    require(
        pending.durable() && pending.business_result().is_none(),
        "local observer loss retains T0 without invented signing",
    )?;
    passed.push("lost observer and close drain preserve actual durable T0");
    let completed = resumed
        .operations(&scope)
        .resume(lost, None)
        .await
        .map_err(fail)?;
    require(
        completed.durable() && completed.business_result().is_some(),
        "explicit resume must finish original inquiry",
    )?;
    let resumed_message = completed
        .business_result()
        .and_then(|value| value.as_chat())
        .ok_or_else(|| JsValue::from_str("resumed inquiry must retain its actual chat result"))?
        .id()
        .to_string();
    resumed.close().await.map_err(fail)?;
    storage_callbacks_closed()?;
    require(callbacks.closed.get() == 1, "resumed signer scope disposed")?;
    passed.push("explicit original-operation resume signs and delivers after reopen");
    passed.push("ordered repeated close and permanent admission");
    Ok(serde_json::json!({"passed":passed,"messages":[message,resumed_message]}).to_string())
}
#[wasm_bindgen]
pub async fn verify_publications(raw_events: String, expected: String) -> Result<(), JsValue> {
    let events: Vec<String> = serde_json::from_str(&raw_events).map_err(fail)?;
    let expected: Vec<String> = serde_json::from_str(&expected).map_err(fail)?;
    require(
        events.len() == 4 && expected.len() == 2,
        "original self/peer envelope counts",
    )?;
    let mut seen = Vec::new();
    for raw in events {
        let event = nostr::Event::from_json(&raw).map_err(fail)?;
        event.verify().map_err(fail)?;
        let recipient = event
            .tags
            .iter()
            .find(|tag| tag.as_slice()[0] == "p")
            .ok_or_else(|| JsValue::from_str("missing envelope recipient"))?
            .as_slice()[1]
            .clone();
        let n = if recipient == key(1).public_key().to_hex() {
            1
        } else {
            2
        };
        require(
            recipient == key(n).public_key().to_hex(),
            "unapproved envelope recipient",
        )?;
        let message = nostr::nips::nip59::extract_rumor(&key(n), &event)
            .await
            .map_err(fail)?;
        require(
            message.sender == key(1).public_key()
                && message.rumor.content == "ordinary browser inquiry",
            "independent nested authentication and original text",
        )?;
        let id = message
            .rumor
            .id
            .ok_or_else(|| JsValue::from_str("missing rumor ID"))?
            .to_hex();
        require(expected.contains(&id), "exact installed business result")?;
        seen.push((id, n));
    }
    seen.sort();
    seen.dedup();
    require(
        seen.len() == 4,
        "each original rumor has independent self and peer delivery",
    )
}

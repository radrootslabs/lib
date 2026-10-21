//! Synthetic isolated host-effect fixture, never a production custody API.
mod delivery;
mod installation;
mod issuance;
mod observations;
mod plans;
pub use delivery::ObtainedEnvelope;
use futures::FutureExt;
pub use issuance::ObtainedSeal;
pub use observations::ObtainedObservation;
use radroots_domain::{
    ActorRole, CommandContextData, Coverage, OperationId, PublicKey, UnixSeconds, WorkspaceId,
};
use radroots_secrets::{
    SecretId, SecretRef,
    browser::BrowserProvider,
    id::{BackendKind, KeyVersion},
    provider::AccessPolicy,
};
use radroots_storage::{
    command::{AdmissionOutcome, AdmissionRequest, PreparedOperation},
    event::SourceGeneration,
    workspace::{AttachmentInput, AttachmentProof, WorkspaceClock, WorkspaceError, WorkspaceStore},
};
use radroots_storage_indexeddb::{
    Namespace,
    workspace::{BrowserWorkspace, LocalOutboxConfig},
};
use sha2::{Digest, Sha256};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::prelude::*;

fn fail(e: WorkspaceError) -> JsValue {
    JsValue::from_str(e.code())
}
fn reference() -> SecretRef {
    SecretRef::new(
        SecretId::parse("workspace_fixture_key").unwrap(),
        BackendKind::External,
        KeyVersion::new(1).unwrap(),
    )
}
fn key_namespace(n: u8) -> SecretId {
    SecretId::parse(format!("workspace_fixture_{n}")).unwrap()
}
fn participant() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn copied(proof: &AttachmentProof) -> AttachmentProof {
    proof.with_secret(|secret| AttachmentProof::from_protected_input(secret).unwrap())
}
struct Clock {
    now: Cell<u64>,
    after: Cell<Option<u64>>,
}
impl Clock {
    fn new() -> Self {
        Self {
            now: Cell::new(1000),
            after: Cell::new(None),
        }
    }
}
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        let now = self.now.get();
        if let Some(next) = self.after.take() {
            self.now.set(next);
        }
        if now == 0 {
            return Err(WorkspaceError::Unavailable);
        }
        UnixSeconds::new(now).map_err(|_| WorkspaceError::Unavailable)
    }
}
#[wasm_bindgen]
pub struct WorkspaceFixture {
    owner: BrowserWorkspace,
    provider: Rc<BrowserProvider>,
    input: AttachmentInput,
    proof: AttachmentProof,
    clock: Rc<Clock>,
}
impl WorkspaceFixture {
    fn journal(&self) -> &dyn radroots_storage::command::OperationJournal {
        &self.owner
    }
}
#[wasm_bindgen]
impl WorkspaceFixture {
    pub async fn admit_read_unknown(
        &self,
        n: u8,
        hook: js_sys::Function,
    ) -> Result<Vec<u8>, JsValue> {
        let request = self.request(n, false).await?;
        let digest = request.command().digest();
        hook.call0(&JsValue::NULL)?;
        match self
            .journal()
            .admit_command_and_hold(request, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            AdmissionOutcome::Committed(_) => Err(JsValue::from_str("unexpected_commit")),
            AdmissionOutcome::Unknown(anchor) => {
                assert_eq!(anchor.workspace_id(), self.input.workspace_id());
                assert_eq!(anchor.operation_id(), OperationId::from_bytes([n; 32]));
                assert_eq!(anchor.command_digest(), digest);
                assert_eq!(anchor.transaction_id(), None);
                let mut bytes = anchor.operation_id().as_bytes().to_vec();
                bytes.extend_from_slice(anchor.command_digest().as_bytes());
                Ok(bytes)
            }
        }
    }
    pub async fn admit_uncertain(&self, n: u8) -> Result<Vec<u8>, JsValue> {
        let request = self.request(n, false).await?;
        let digest = request.command().digest();
        match self
            .journal()
            .admit_command_and_hold(request, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            AdmissionOutcome::Committed(_) => Err(JsValue::from_str("unexpected_commit")),
            AdmissionOutcome::Unknown(anchor) => {
                assert_eq!(anchor.workspace_id(), self.input.workspace_id());
                assert_eq!(anchor.operation_id(), OperationId::from_bytes([n; 32]));
                assert_eq!(anchor.command_digest(), digest);
                let mut bytes = anchor.operation_id().as_bytes().to_vec();
                bytes.extend_from_slice(anchor.command_digest().as_bytes());
                bytes.extend_from_slice(anchor.transaction_id().unwrap().as_bytes());
                Ok(bytes)
            }
        }
    }
    pub async fn admit(&self, n: u8, changed: bool) -> Result<Vec<u8>, JsValue> {
        let request = self.request(n, changed).await?;
        match self
            .journal()
            .admit_command_and_hold(request, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            AdmissionOutcome::Committed(prepared) => Ok(summary(prepared)),
            AdmissionOutcome::Unknown(_) => Err(JsValue::from_str("commit_unknown")),
        }
    }
    pub async fn admit_public(&self, n: u8) -> Result<Vec<u8>, JsValue> {
        let public = self.public_request(n).await?;
        match self
            .journal()
            .admit_command_and_hold(public, copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            AdmissionOutcome::Committed(prepared) => Ok(summary(prepared)),
            AdmissionOutcome::Unknown(_) => Err(JsValue::from_str("commit_unknown")),
        }
    }
    pub async fn bind_public(&self, n: u8) -> Result<Vec<u8>, JsValue> {
        let public = self.public_request(n).await?;
        let plan = radroots_protocol::listing::PublicSigningPlan::capture(
            public.command().listing().unwrap(),
            UnixSeconds::new(1000).unwrap(),
            &["wss://public-plan.fixture.example/a".to_string()],
            radroots_protocol::plan::SigningReplay::NonReplayable,
        )
        .unwrap();
        match self
            .journal()
            .bind_signing_intents(public.binding().clone(), plan.into(), copied(&self.proof))
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
        {
            radroots_storage::command::PlanBindingOutcome::Committed(bound) => {
                Ok(plans::summary(bound))
            }
            radroots_storage::command::PlanBindingOutcome::Unknown(_) => {
                Err(JsValue::from_str("commit_unknown"))
            }
        }
    }
    pub async fn public_plan_query(&self, n: u8) -> Result<Vec<u8>, JsValue> {
        let request = self.public_request(n).await?;
        self.journal()
            .signing_plan(
                request.binding().clone(),
                request.command().context().operation_id(),
                request.command().digest(),
                copied(&self.proof),
            )
            .await
            .map(|v| v.map(plans::summary).unwrap_or_default())
            .map_err(|e| JsValue::from_str(e.code()))
    }
    pub async fn install_public(&self, n: u8) -> Result<String, JsValue> {
        let request = self.public_request(n).await?;
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
            .ok_or_else(|| JsValue::from_str("missing_plan"))?;
        let input = radroots_storage::command::InstallationRequest::new(
            request.binding().clone(),
            request.command().context().operation_id(),
            request.command().digest(),
            *bound.plan().digest(),
        );
        match self.journal().install_transition_and_outbox(input,copied(&self.proof)).await.map_err(|e|JsValue::from_str(e.code()))?{
            radroots_storage::command::InstallationOutcome::ListingCommitted(value)=>Ok(serde_json::json!({"transaction":value.transaction_id().to_string(),"revision":value.revision().to_string(),"capture":value.basis().transaction_id().to_string(),"observed":value.basis().observed_at_unix_ms().map(|v|v.get()),"installed_at":value.installed_at().to_string()}).to_string()),
            radroots_storage::command::InstallationOutcome::Unknown(_)=>Err(JsValue::from_str("commit_unknown")),
            _=>Err(JsValue::from_str("unexpected_private_installation")),
        }
    }
    pub async fn public_snapshot(&self, n: u8) -> Result<String, JsValue> {
        let request = self.public_request(n).await?;
        let snapshot = self
            .journal()
            .operation_snapshot(
                request.binding().clone(),
                request.command().context().operation_id(),
                Some(request.command().digest()),
                copied(&self.proof),
            )
            .await
            .map_err(|e| JsValue::from_str(e.code()))?
            .ok_or_else(|| JsValue::from_str("missing_operation"))?;
        Ok(serde_json::json!({"checkpoint":snapshot.checkpoint_digest().to_string(),"installation":snapshot.listing_installation().map(|v|v.transaction_id().to_string()),"publications":snapshot.public_publications().len()}).to_string())
    }
    pub async fn public_coordinate_key(&self, n: u8) -> Result<Vec<u8>, JsValue> {
        let request = self.public_request(n).await?;
        let command = request.command();
        let listing = command
            .listing()
            .ok_or_else(|| JsValue::from_str("missing_listing"))?;
        Ok(Sha256::new()
            .chain_update(b"radroots.browser.local_outbox.listing_coordinate.v1\0")
            .chain_update(self.input.workspace_id().as_bytes())
            .chain_update(self.input.participant().as_bytes())
            .chain_update(listing.input().id().as_bytes())
            .finalize()
            .to_vec())
    }
    pub async fn query(&self, n: u8) -> Result<Vec<u8>, JsValue> {
        let binding = self
            .owner
            .authenticate(self.input.clone(), copied(&self.proof))
            .await
            .map_err(fail)?;
        let result = self
            .journal()
            .operation(
                binding,
                OperationId::from_bytes([n; 32]),
                None,
                copied(&self.proof),
            )
            .await
            .map_err(|e| JsValue::from_str(e.code()))?;
        Ok(result.map(summary).unwrap_or_default())
    }
    pub async fn admit_expiring(&self, n: u8) -> Result<(), JsValue> {
        let request = self.request(n, false).await?;
        self.clock.after.set(Some(1500));
        self.journal()
            .admit_command_and_hold(request, copied(&self.proof))
            .await
            .map(|_| ())
            .map_err(|e| JsValue::from_str(e.code()))
    }
    pub async fn abandon_admission(&self, n: u8) -> Result<(), JsValue> {
        let request = self.request(n, false).await?;
        if self
            .journal()
            .admit_command_and_hold(request, copied(&self.proof))
            .now_or_never()
            .is_some()
        {
            return Err(JsValue::from_str("unexpected_immediate_admission"));
        }
        Ok(())
    }
    pub async fn sibling(&self) -> Result<Self, JsValue> {
        let clock = Rc::new(Clock::new());
        let config = LocalOutboxConfig::new(
            participant(),
            participant(),
            "synthetic-browser",
            UnixSeconds::new(2000).unwrap(),
        )
        .map_err(fail)?;
        let (owner, input, proof) = BrowserWorkspace::initialize_local_outbox(
            config,
            self.provider.clone(),
            reference(),
            AccessPolicy::standard(),
            clock.clone(),
        )
        .await
        .map_err(fail)?;
        Ok(Self {
            owner,
            provider: self.provider.clone(),
            input,
            proof,
            clock,
        })
    }
    pub async fn create(n: u8) -> Result<Self, JsValue> {
        Self::initialize(n, false, false).await
    }
    pub async fn create_seller(n: u8) -> Result<Self, JsValue> {
        Self::initialize(n, false, true).await
    }
    pub async fn create_expiring(n: u8) -> Result<(), JsValue> {
        let owner = Self::initialize(n, true, false).await?;
        owner.close().await
    }
    pub async fn open(
        n: u8,
        fingerprint: Vec<u8>,
        selection: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<Self, JsValue> {
        Self::open_fixture(n, fingerprint, selection, proof, false).await
    }
    pub async fn open_capture(
        n: u8,
        fingerprint: Vec<u8>,
        selection: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<Self, JsValue> {
        Self::open_fixture(n, fingerprint, selection, proof, true).await
    }
    pub fn selection(&self) -> Vec<u8> {
        serde_json::to_vec(&self.input).unwrap()
    }
    pub fn proof(&self) -> Vec<u8> {
        self.proof.with_secret(|bytes| bytes.to_vec())
    }
    pub fn fingerprint(&self) -> Vec<u8> {
        self.provider.fingerprint().to_vec()
    }
    pub fn data_namespace(&self) -> String {
        let mut h = Sha256::new();
        h.update(b"radroots.browser.local_outbox.namespace.v2\0");
        h.update(self.input.workspace_id().as_bytes());
        Namespace::new(SourceGeneration::new(h.finalize().into()).unwrap()).name()
    }
    pub fn set_time(&self, now: u32) {
        self.clock.now.set(now as u64);
    }
    pub fn expire_after_next_read(&self) {
        self.clock.after.set(Some(2000));
    }
    pub async fn authenticate(&self) -> Result<u32, JsValue> {
        let bound = self
            .owner
            .authenticate(self.input.clone(), copied(&self.proof))
            .await
            .map_err(fail)?;
        assert_eq!(
            bound.actor().role(),
            match bound.scope() {
                radroots_storage::workspace::LocalOutboxScope::NewBuyerOutbox => ActorRole::Buyer,
                radroots_storage::workspace::LocalOutboxScope::NewSellerOutbox => ActorRole::Seller,
            }
        );
        assert_eq!(bound.actor().organization(), None);
        assert_eq!(bound.input(), &self.input);
        assert_eq!(bound.actor().grant_id(), Some(self.input.grant_id()));
        assert_eq!(bound.input().checkpoint().coverage(), Coverage::Partial);
        Ok(bound.grant_revision().get() as u32)
    }
    pub async fn is_seller(&self) -> Result<bool, JsValue> {
        let bound = self
            .owner
            .authenticate(self.input.clone(), copied(&self.proof))
            .await
            .map_err(fail)?;
        Ok(
            bound.scope() == radroots_storage::workspace::LocalOutboxScope::NewSellerOutbox
                && bound.actor().role() == ActorRole::Seller,
        )
    }
    pub async fn wrong_proof(&self) -> Result<(), JsValue> {
        self.owner
            .authenticate(
                self.input.clone(),
                AttachmentProof::from_protected_input(&[0x31; 32]).unwrap(),
            )
            .await
            .map(|_| ())
            .map_err(fail)
    }
    pub async fn wrong_selection(&self) -> Result<(), JsValue> {
        let wrong = AttachmentInput::new(
            WorkspaceId::from_bytes([3; 32]),
            self.input.authority(),
            self.input.participant(),
            self.input.endpoint(),
            self.input.endpoint_identity(),
            self.input.grant_id(),
            self.input.checkpoint().clone(),
            self.input.route_digest(),
        )
        .unwrap();
        self.owner
            .authenticate(wrong, copied(&self.proof))
            .await
            .map(|_| ())
            .map_err(fail)
    }
    pub async fn contended_open(&self) -> Result<(), JsValue> {
        let owner = BrowserWorkspace::open_existing(
            self.input.clone(),
            copied(&self.proof),
            self.provider.clone(),
            reference(),
            AccessPolicy::standard(),
            self.clock.clone(),
        )
        .await
        .map_err(fail)?;
        owner.close().await.map_err(fail)
    }
    pub async fn revoke(&self) -> Result<(), JsValue> {
        self.owner
            .revoke(self.input.clone(), copied(&self.proof))
            .await
            .map_err(fail)
    }
    pub fn abandon_auths(&self, count: u32) -> Result<u32, JsValue> {
        for n in 0..count {
            if let Some(result) = self
                .owner
                .authenticate(self.input.clone(), copied(&self.proof))
                .now_or_never()
            {
                return if matches!(result, Err(WorkspaceError::Capacity)) {
                    Ok(n)
                } else {
                    Err(JsValue::from_str("unexpected_immediate_auth"))
                };
            }
        }
        Ok(count)
    }
    pub fn abandon_close(&self) -> Result<(), JsValue> {
        if self.owner.close().now_or_never().is_some() {
            return Err(JsValue::from_str("unexpected_immediate_workspace_close"));
        }
        Ok(())
    }
    pub async fn close(&self) -> Result<(), JsValue> {
        self.owner.close().await.map_err(fail)
    }
}
impl WorkspaceFixture {
    async fn open_fixture(
        n: u8,
        fingerprint: Vec<u8>,
        selection: Vec<u8>,
        proof: Vec<u8>,
        capture_only: bool,
    ) -> Result<Self, JsValue> {
        if selection.len() > 16 * 1024 {
            return Err(JsValue::from_str("oversized_selection"));
        }
        let input: AttachmentInput = serde_json::from_slice(&selection)
            .map_err(|_| JsValue::from_str("invalid_selection"))?;
        let proof = AttachmentProof::from_protected_input(&proof).map_err(fail)?;
        let fingerprint = fingerprint
            .as_slice()
            .try_into()
            .map_err(|_| JsValue::from_str("invalid_fingerprint"))?;
        let provider = Rc::new(
            BrowserProvider::open(key_namespace(n), reference(), fingerprint)
                .await
                .map_err(|e| JsValue::from_str(e.code()))?,
        );
        let clock = Rc::new(Clock::new());
        let owner = if capture_only {
            BrowserWorkspace::open_existing_for_capture(
                input.clone(),
                copied(&proof),
                provider.clone(),
                reference(),
                AccessPolicy::standard(),
                clock.clone(),
            )
            .await
        } else {
            BrowserWorkspace::open_existing(
                input.clone(),
                copied(&proof),
                provider.clone(),
                reference(),
                AccessPolicy::standard(),
                clock.clone(),
            )
            .await
        }
        .map_err(fail)?;
        Ok(Self {
            owner,
            provider,
            input,
            proof,
            clock,
        })
    }
    async fn public_request(&self, n: u8) -> Result<AdmissionRequest, JsValue> {
        let request = self.request(n, false).await?;
        let input = serde_json::json!({"id":"01".repeat(16),"title":"Carrots","summary":"Quote required",
            "description_markdown":"Public terms","published_at":"1000","profile":{
            "profile":"radroots.listing","version":1,"state":"active",
            "product":{"name":"Carrots","description":"","producer":null,"origin_summary":null},
            "default_option":"02".repeat(32),"options":[{"id":"02".repeat(32),"label":"One bunch",
            "unit":{"type":"package","label":"bunch","contents":{"type":"unspecified"}},
            "increment":"1","minimum":"1","price":{"type":"quote_required"},
            "supply":{"type":"unknown"},"lead_seconds":"0"}],
            "fulfillment":{"modes":["pickup"],"area":"Example region","lead_seconds":"0"},
            "authority":null,"media":[]}});
        let wire = serde_json::to_vec(&serde_json::json!({"type":"listing.publish",
            "context":request.command().context(),"input":input}))
        .unwrap();
        let public = AdmissionRequest::from_protected_wire(request.binding().clone(), &wire)
            .map_err(|e| JsValue::from_str(e.code()))?;
        Ok(public)
    }
    async fn request(&self, n: u8, changed: bool) -> Result<AdmissionRequest, JsValue> {
        let binding = self
            .owner
            .authenticate(self.input.clone(), copied(&self.proof))
            .await
            .map_err(fail)?;
        let context = CommandContextData::new(
            OperationId::from_bytes([n; 32]),
            binding.actor(),
            self.input.workspace_id(),
            self.input.authority(),
            self.input.route_digest(),
            None,
            None,
            UnixSeconds::new(1500).unwrap(),
            None,
        )
        .unwrap();
        // Escaped control bytes exercise multiple private-record parts without
        // exceeding the shared 4096-byte chat text limit.
        let text = if changed {
            "changed".to_string()
        } else {
            // T1 additionally constructs the bounded complete rumor, whose
            // JSON framing must fit alongside the escaped text. Preserve the
            // earlier T0-only maximum-input cases exactly.
            "\u{0001}".repeat(if n >= 180 { 3000 } else { 4096 })
        };
        let peer = if n >= 180 {
            PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
                .unwrap()
        } else {
            participant()
        };
        let wire = serde_json::to_vec(
            &serde_json::json!({"type":"inbox.inquire","context":context,
            "input":{"thread_id":"66".repeat(32),"peer":peer,"source":null,"text":text}}),
        )
        .unwrap();
        AdmissionRequest::from_protected_wire(binding, &wire)
            .map_err(|e| JsValue::from_str(e.code()))
    }
    async fn initialize(n: u8, expire: bool, seller: bool) -> Result<Self, JsValue> {
        let provider = Rc::new(
            BrowserProvider::create(key_namespace(n), reference())
                .await
                .map_err(|e| JsValue::from_str(e.code()))?,
        );
        let clock = Rc::new(Clock::new());
        if expire {
            clock.after.set(Some(2000));
        }
        let constructor = if seller {
            LocalOutboxConfig::seller
        } else {
            LocalOutboxConfig::new
        };
        let config = constructor(
            participant(),
            participant(),
            "synthetic-browser",
            UnixSeconds::new(2000).unwrap(),
        )
        .map_err(fail)?;
        let (owner, input, proof) = BrowserWorkspace::initialize_local_outbox(
            config,
            provider.clone(),
            reference(),
            AccessPolicy::standard(),
            clock.clone(),
        )
        .await
        .map_err(fail)?;
        Ok(Self {
            owner,
            provider,
            input,
            proof,
            clock,
        })
    }
}
fn summary(prepared: PreparedOperation) -> Vec<u8> {
    let mut bytes = prepared.transaction_id().as_bytes().to_vec();
    bytes.extend_from_slice(prepared.request().command().digest().as_bytes());
    bytes.extend_from_slice(&prepared.issuance_window().prepared_at().get().to_be_bytes());
    bytes.extend_from_slice(
        &prepared
            .issuance_window()
            .first_issue_not_after()
            .get()
            .to_be_bytes(),
    );
    bytes
}

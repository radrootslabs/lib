//! Synthetic isolated host-effect fixture, never a production custody API.
use futures::FutureExt;
use radroots_domain::{ActorRole, Coverage, PublicKey, UnixSeconds, WorkspaceId};
use radroots_secrets::{
    SecretId, SecretRef,
    browser::BrowserProvider,
    id::{BackendKind, KeyVersion},
    provider::AccessPolicy,
};
use radroots_storage::{
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
#[wasm_bindgen]
impl WorkspaceFixture {
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
        Self::initialize(n, false).await
    }
    pub async fn create_expiring(n: u8) -> Result<(), JsValue> {
        let owner = Self::initialize(n, true).await?;
        owner.close().await
    }
    pub async fn open(
        n: u8,
        fingerprint: Vec<u8>,
        selection: Vec<u8>,
        proof: Vec<u8>,
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
        let owner = BrowserWorkspace::open_existing(
            input.clone(),
            copied(&proof),
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
        h.update(b"radroots.browser.local_outbox.namespace.v1\0");
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
        assert_eq!(bound.actor().role(), ActorRole::Buyer);
        assert_eq!(bound.actor().organization(), None);
        assert_eq!(bound.input(), &self.input);
        assert_eq!(bound.actor().grant_id(), Some(self.input.grant_id()));
        assert_eq!(bound.input().checkpoint().coverage(), Coverage::Partial);
        Ok(bound.grant_revision().get() as u32)
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
    async fn initialize(n: u8, expire: bool) -> Result<Self, JsValue> {
        let provider = Rc::new(
            BrowserProvider::create(key_namespace(n), reference())
                .await
                .map_err(|e| JsValue::from_str(e.code()))?,
        );
        let clock = Rc::new(Clock::new());
        if expire {
            clock.after.set(Some(2000));
        }
        let config = LocalOutboxConfig::new(
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

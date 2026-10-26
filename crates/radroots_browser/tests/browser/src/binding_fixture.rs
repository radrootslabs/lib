//! Synthetic host assembly only; all JS command entry points are production.
use super::*;
use radroots_browser::bindings::{BrowserClient, Workspace};

#[derive(Clone, Copy)]
#[wasm_bindgen]
pub enum FixtureRejection {
    Malformed,
    Oversized,
    Foreign,
}

// Controlled callback responses through the production scoped issuer.
struct RejectedSigner {
    local: LocalSigner,
    mode: FixtureRejection,
    calls: Rc<Cell<u32>>,
}
impl radroots_signing::Signer for RejectedSigner {
    fn status(
        &self,
    ) -> BoxFuture<'_, Result<radroots_signing::SignerStatus, radroots_signing::Error>> {
        radroots_signing::Signer::status(&self.local)
    }
    fn sign(
        &self,
        _: radroots_signing::SignRequest,
    ) -> BoxFuture<'_, Result<radroots_signing::SignReceipt, radroots_signing::Error>> {
        panic!("rejection fixture must not invoke legacy signing")
    }
}
impl radroots_signing::provider::ParticipantProvider for RejectedSigner {
    fn provider_state(
        &self,
    ) -> BoxFuture<'_, Result<radroots_signing::provider::ProviderState, ProviderError>> {
        radroots_signing::provider::ParticipantProvider::provider_state(&self.local)
    }
    fn sign_exact(
        &self,
        intent: radroots_signing::issuance::SigningIntent,
    ) -> BoxFuture<'_, radroots_signing::issuance::SigningOutcome> {
        use radroots_signing::issuance::{ArtifactRole, IntentBinding, SigningOutcome};
        self.calls.set(self.calls.get() + 1);
        Box::pin(async move {
            match self.mode {
                FixtureRejection::Malformed => SigningOutcome::capture(
                    &intent,
                    "synthetic private rejection canary".into(),
                    None,
                ),
                FixtureRejection::Oversized => {
                    SigningOutcome::capture(&intent, "X".repeat(65537), None)
                }
                FixtureRejection::Foreign => SigningOutcome::not_dispatched(
                    IntentBinding::from_components(
                        OperationId::from_bytes([0xf1; 32]),
                        [0xf2; 32],
                        [0xf3; 32],
                        ArtifactRole::PublicEvent,
                        None,
                        core::num::NonZeroU64::new(1).unwrap(),
                    )
                    .unwrap(),
                    ProviderError::Cancelled,
                ),
            }
        })
    }
    fn encrypt_private(
        &self,
        call: radroots_signing::provider::ProviderCall,
        recipient: PublicKey,
        text: radroots_signing::provider::PrivateText,
    ) -> BoxFuture<'_, Result<radroots_signing::provider::PrivateCiphertext, ProviderError>> {
        radroots_signing::provider::ParticipantProvider::encrypt_private(
            &self.local,
            call,
            recipient,
            text,
        )
    }
    fn decrypt_private(
        &self,
        call: radroots_signing::provider::ProviderCall,
        sender: PublicKey,
        text: radroots_signing::provider::PrivateCiphertext,
    ) -> BoxFuture<'_, Result<radroots_signing::provider::PrivateText, ProviderError>> {
        radroots_signing::provider::ParticipantProvider::decrypt_private(
            &self.local,
            call,
            sender,
            text,
        )
    }
}

#[wasm_bindgen]
pub struct BindingFixture {
    existing: Rc<Existing>,
    base: String,
    client: Client,
    scope: WorkspaceSession,
    callbacks: Rc<Callbacks>,
    rejection: Option<FixtureRejection>,
    signing_calls: Rc<Cell<u32>>,
}
#[wasm_bindgen]
impl BindingFixture {
    pub async fn create(
        base: String,
        label: String,
        seller: bool,
    ) -> Result<BindingFixture, JsValue> {
        let existing = Rc::new(Existing::create_scope(&label, seller).await?);
        Self::open(existing, base, None).await
    }
    pub async fn create_rejected(
        base: String,
        label: String,
        mode: FixtureRejection,
    ) -> Result<BindingFixture, JsValue> {
        let existing = Rc::new(Existing::create_scope(&label, true).await?);
        Self::open(existing, base, Some(mode)).await
    }
    pub fn signing_calls(&self) -> u32 {
        self.signing_calls.get()
    }
    pub fn client(&self) -> BrowserClient {
        BrowserClient::from_browser(self.client.clone())
    }
    pub fn workspace(&self) -> Workspace {
        Workspace::from_session(self.scope.clone())
    }
    pub fn callbacks_closed(&self) -> u32 {
        self.callbacks.closed.get()
    }
    pub async fn reopen(&self) -> Result<BindingFixture, JsValue> {
        require(
            self.client.state() == SessionState::Closed,
            "fixture reopen requires explicit actual close",
        )?;
        Self::open(self.existing.clone(), self.base.clone(), self.rejection).await
    }
}
impl BindingFixture {
    async fn open(
        existing: Rc<Existing>,
        base: String,
        rejection: Option<FixtureRejection>,
    ) -> Result<Self, JsValue> {
        let (client, scope) = existing.open(&base).await?;
        let signing_calls = Rc::new(Cell::new(0));
        let callbacks = if let Some(mode) = rejection {
            let callbacks = Rc::new(Callbacks {
                quiesced: Cell::new(0),
                closed: Cell::new(0),
            });
            client
                .attach_signer(SignerHandle::new(
                    Rc::new(RejectedSigner {
                        local: local_signer(),
                        mode,
                        calls: signing_calls.clone(),
                    }),
                    callbacks.clone(),
                ))
                .await
                .map_err(fail)?;
            callbacks
        } else {
            signer(&client).await?
        };
        Ok(Self {
            existing,
            base,
            client,
            scope,
            callbacks,
            rejection,
            signing_calls,
        })
    }
}

/// Independent nested authentication of the original private JS command text.
#[wasm_bindgen]
pub async fn verify_binding_inquiry(
    raw_events: String,
    expected: String,
    text: String,
) -> Result<(), JsValue> {
    let events: Vec<String> = serde_json::from_str(&raw_events).map_err(fail)?;
    require(events.len() == 2, "one original self and peer envelope")?;
    let mut recipients = Vec::new();
    for raw in events {
        let event = nostr::Event::from_json(&raw).map_err(fail)?;
        event.verify().map_err(fail)?;
        let recipient = event
            .tags
            .iter()
            .find(|tag| tag.as_slice()[0] == "p")
            .ok_or_else(|| JsValue::from_str("missing private recipient"))?
            .as_slice()[1]
            .clone();
        let n = if recipient == key(1).public_key().to_hex() {
            1
        } else {
            2
        };
        require(
            recipient == key(n).public_key().to_hex(),
            "unexpected private recipient",
        )?;
        let message = nostr::nips::nip59::extract_rumor(&key(n), &event)
            .await
            .map_err(fail)?;
        require(
            message.sender == key(1).public_key()
                && message.rumor.content == text
                && message.rumor.id.map(|v| v.to_hex()).as_deref() == Some(&expected),
            "private JS text and original rumor identity",
        )?;
        recipients.push(n);
    }
    recipients.sort();
    require(recipients == [1, 2], "independent self and peer originals")
}

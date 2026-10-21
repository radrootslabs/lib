//! Synthetic host assembly only; all JS command entry points are production.
use super::*;
use radroots_browser::bindings::{BrowserClient, Workspace};

#[wasm_bindgen]
pub struct BindingFixture {
    existing: Rc<Existing>,
    base: String,
    client: Client,
    scope: WorkspaceSession,
    callbacks: Rc<Callbacks>,
}
#[wasm_bindgen]
impl BindingFixture {
    pub async fn create(
        base: String,
        label: String,
        seller: bool,
    ) -> Result<BindingFixture, JsValue> {
        let existing = Rc::new(Existing::create_scope(&label, seller).await?);
        Self::open(existing, base).await
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
        Self::open(self.existing.clone(), self.base.clone()).await
    }
}
impl BindingFixture {
    async fn open(existing: Rc<Existing>, base: String) -> Result<Self, JsValue> {
        let (client, scope) = existing.open(&base).await?;
        let callbacks = signer(&client).await?;
        Ok(Self {
            existing,
            base,
            client,
            scope,
            callbacks,
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

use super::inputs::bounded_string;
use super::{CommandRequest, Error, InquiryInput, ListingInput, Receipt, SessionState, StartMode};
use js_sys::JsString;
use radroots_domain::{CommandDigest, OperationId};
use wasm_bindgen::prelude::*;

/// One actual origin-thread client. Foreign calls add no task owner or runtime.
#[derive(Clone)]
#[wasm_bindgen(js_name = Client)]
pub struct BrowserClient {
    inner: crate::Client,
}
impl BrowserClient {
    /// Injects the explicitly configured actual browser client without starting it.
    pub fn from_browser(inner: crate::Client) -> Self {
        Self { inner }
    }
}
/// Actual authenticated shared scope, never constructed from a JS descriptor.
#[derive(Clone)]
#[wasm_bindgen]
pub struct Workspace {
    inner: crate::WorkspaceSession,
}
impl Workspace {
    /// Injects an actual shared-client attachment; ownership is rechecked on use.
    pub fn from_session(inner: crate::WorkspaceSession) -> Self {
        Self { inner }
    }
}
#[wasm_bindgen]
impl Workspace {
    /// Releases the attachment credential root without revoking the grant.
    pub fn close(&self) {
        self.inner.close();
    }
}
#[wasm_bindgen(js_class = Client)]
impl BrowserClient {
    pub fn state(&self) -> SessionState {
        self.inner.state().into()
    }
    pub async fn start(&self, mode: StartMode) -> Result<SessionState, JsValue> {
        self.inner
            .start(mode.into())
            .await
            .map(Into::into)
            .map_err(|v| Error::from(v).into())
    }
    /// Resolves only after actual owned tasks, callbacks and storage close.
    pub async fn close(&self) -> Result<(), JsValue> {
        self.inner
            .close()
            .await
            .map(|_| ())
            .map_err(|v| Error::from(v).into())
    }
    pub async fn publish_listing(
        &self,
        workspace: &Workspace,
        request: &CommandRequest,
        input: &ListingInput,
    ) -> Result<Receipt, JsValue> {
        let context = request.capture(&workspace.inner)?;
        let value = self
            .inner
            .listings()
            .publish(context, input.inner.clone())
            .await
            .map_err(Error::from)?;
        Receipt::listing(&value).map_err(Into::into)
    }
    pub async fn inquire(
        &self,
        workspace: &Workspace,
        request: &CommandRequest,
        input: &InquiryInput,
    ) -> Result<Receipt, JsValue> {
        let context = request.capture(&workspace.inner)?;
        let value = self
            .inner
            .inbox()
            .inquire(context, input.captured_copy()?)
            .await
            .map_err(Error::from)?;
        Receipt::chat(&value).map_err(Into::into)
    }
    /// Scoped observation; absence never proves safe nonissuance.
    pub async fn operation(
        &self,
        workspace: &Workspace,
        operation_id: JsString,
        expected_digest: Option<JsString>,
    ) -> Result<Option<Receipt>, JsValue> {
        let (id, expected) = selector(operation_id, expected_digest)?;
        self.inner
            .operations(&workspace.inner)
            .get_matching(id, expected)
            .await
            .map_err(Error::from)?
            .as_ref()
            .map(Receipt::status)
            .transpose()
            .map_err(Into::into)
    }
    /// Explicitly continues original retained work, without reissuing a claim.
    pub async fn resume(
        &self,
        workspace: &Workspace,
        operation_id: JsString,
        expected_digest: Option<JsString>,
    ) -> Result<Receipt, JsValue> {
        let (id, expected) = selector(operation_id, expected_digest)?;
        let value = self
            .inner
            .operations(&workspace.inner)
            .resume(id, expected)
            .await
            .map_err(Error::from)?;
        Receipt::status(&value).map_err(Into::into)
    }
}
fn selector(
    operation: JsString,
    expected: Option<JsString>,
) -> Result<(OperationId, Option<CommandDigest>), Error> {
    let operation = bounded_string(operation, 64, "operation_id")?
        .parse()
        .map_err(|_| Error::input("operation_id"))?;
    let expected = expected
        .map(|v| {
            bounded_string(v, 64, "command_digest")
                .and_then(|v| v.parse().map_err(|_| Error::input("command_digest")))
        })
        .transpose()?;
    Ok((operation, expected))
}

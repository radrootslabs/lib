use crate::{CommandRequest, Error, InquiryInput, ListingInput, Receipt, SessionState, StartMode};
use radroots::domain::{CommandDigest, OperationId};
use std::sync::Arc;

/// Holds the actual native owner; no FFI runtime or outbox is added.
#[derive(uniffi::Object)]
pub struct NativeClient {
    inner: radroots::Client,
}
impl NativeClient {
    /// Native host startup supplies its explicitly configured shared client.
    /// This Rust constructor performs no additional attachment or activation.
    pub fn from_native(inner: radroots::Client) -> Arc<Self> {
        Arc::new(Self { inner })
    }
}

/// Holds an actual authenticated shared scope. Descriptors cannot construct it.
#[derive(uniffi::Object)]
pub struct Workspace {
    inner: radroots::WorkspaceSession,
}
impl Workspace {
    /// Injects a scope obtained by actual shared-client authentication. The
    /// client still enforces that it owns this scope on every command/query.
    pub fn from_session(inner: radroots::WorkspaceSession) -> Arc<Self> {
        Arc::new(Self { inner })
    }
}
#[uniffi::export]
impl Workspace {
    /// Releases this attachment's credential root without revoking its grant.
    pub fn close(&self) {
        self.inner.close();
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl NativeClient {
    pub fn state(&self) -> SessionState {
        self.inner.state().into()
    }
    pub async fn start(&self, mode: StartMode) -> Result<SessionState, Error> {
        self.inner
            .start(mode.into())
            .await
            .map(|v| v.state().into())
            .map_err(Error::from)
    }
    /// Completion is the shared owner's actual confirmed resource closure.
    pub async fn close(&self) -> Result<(), Error> {
        self.inner.close().await.map(|_| ()).map_err(Error::from)
    }
    pub async fn publish_listing(
        &self,
        workspace: Arc<Workspace>,
        request: CommandRequest,
        input: Arc<ListingInput>,
    ) -> Result<Receipt, Error> {
        let context = request.capture(&workspace.inner)?;
        let receipt = self
            .inner
            .listings()
            .publish(context, input.inner.clone())
            .await
            .map_err(Error::from)?;
        Receipt::listing(&receipt)
    }
    pub async fn inquire(
        &self,
        workspace: Arc<Workspace>,
        request: CommandRequest,
        input: Arc<InquiryInput>,
    ) -> Result<Receipt, Error> {
        let context = request.capture(&workspace.inner)?;
        let receipt = self
            .inner
            .inbox()
            .inquire(context, input.captured_copy()?)
            .await
            .map_err(Error::from)?;
        Receipt::chat(&receipt)
    }
    /// Scoped observation only; absence never proves safe nonissuance.
    pub async fn operation(
        &self,
        workspace: Arc<Workspace>,
        operation_id: String,
        expected_digest: Option<String>,
    ) -> Result<Option<Receipt>, Error> {
        let (operation, expected) = selector(&operation_id, expected_digest.as_deref())?;
        self.inner
            .operations(&workspace.inner)
            .get_matching(operation, expected)
            .await
            .map_err(Error::from)?
            .as_ref()
            .map(Receipt::status)
            .transpose()
    }
    /// Explicit continuation through the original shared owner and policy.
    pub async fn resume(
        &self,
        workspace: Arc<Workspace>,
        operation_id: String,
        expected_digest: Option<String>,
    ) -> Result<Receipt, Error> {
        let (operation, expected) = selector(&operation_id, expected_digest.as_deref())?;
        let receipt = self
            .inner
            .operations(&workspace.inner)
            .resume(operation, expected)
            .await
            .map_err(Error::from)?;
        Receipt::status(&receipt)
    }
}
fn selector(
    operation: &str,
    expected: Option<&str>,
) -> Result<(OperationId, Option<CommandDigest>), Error> {
    let operation = operation
        .parse()
        .map_err(|_| Error::input("operation_id"))?;
    let expected = expected
        .map(str::parse)
        .transpose()
        .map_err(|_| Error::input("command_digest"))?;
    Ok((operation, expected))
}

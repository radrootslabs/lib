use crate::Error;
use radroots::domain::{CorrelationId, OperationId, SourceId, ThreadId, UnixSeconds};
use std::sync::Arc;

/// Explicit first-issue context for the currently supported known commands.
#[derive(Clone, uniffi::Record)]
pub struct CommandRequest {
    pub operation_id: String,
    pub first_issue_not_after: String,
    pub correlation_id: Option<String>,
}
impl CommandRequest {
    pub(crate) fn capture(
        self,
        scope: &radroots::WorkspaceSession,
    ) -> Result<radroots::CommandContext, Error> {
        let operation: OperationId = self
            .operation_id
            .parse()
            .map_err(|_| Error::input("operation_id"))?;
        let time: UnixSeconds = self
            .first_issue_not_after
            .parse()
            .map_err(|_| Error::input("first_issue_not_after"))?;
        let correlation = self
            .correlation_id
            .map(CorrelationId::new)
            .transpose()
            .map_err(|_| Error::input("correlation_id"))?;
        scope
            .command_context_with_correlation(operation, None, None, time, correlation)
            .map_err(Error::from)
    }
}

/// Validated shared public input. No participant or installation is inferred.
#[derive(uniffi::Object)]
pub struct ListingInput {
    pub(crate) inner: radroots::listings::PublishInput,
}
#[uniffi::export]
impl ListingInput {
    /// Runs the existing bounded, closed shared parser, before host effects.
    #[uniffi::constructor]
    pub fn parse(input: Vec<u8>) -> Result<Arc<Self>, Error> {
        let inner = radroots::listings::PublishInput::parse(&input)
            .map_err(|error| Error::command("listing_input", error))?;
        Ok(Arc::new(Self { inner }))
    }
}

/// Public reference descriptor; it authenticates no event, terms or stock.
#[derive(Clone, uniffi::Record)]
pub struct ListingReference {
    pub author: String,
    pub discriminator: String,
    pub event_id: String,
}
impl ListingReference {
    fn capture(&self) -> Result<radroots::inbox::ListingRevisionRef, Error> {
        let author = public_key(&self.author, "listing_author")?;
        let event: SourceId = self
            .event_id
            .parse()
            .map_err(|_| Error::input("listing_event_id"))?;
        radroots::inbox::ListingRevisionRef::new(author, &self.discriminator, event)
            .map_err(|error| Error::command("listing_reference", error))
    }
}

/// Shared bounded private input, retained with the source's zeroizing owner.
#[derive(uniffi::Object)]
pub struct InquiryInput {
    inner: radroots::inbox::InquireInput,
}
#[uniffi::export]
impl InquiryInput {
    #[uniffi::constructor]
    pub fn new(
        thread_id: String,
        peer: String,
        source: Option<ListingReference>,
        text: String,
    ) -> Result<Arc<Self>, Error> {
        let text = zeroize::Zeroizing::new(text);
        let thread: ThreadId = thread_id.parse().map_err(|_| Error::input("thread_id"))?;
        let peer = public_key(&peer, "peer")?;
        let source = source.as_ref().map(ListingReference::capture).transpose()?;
        let inner = radroots::inbox::InquireInput::new(thread, peer, source, &text)
            .map_err(|error| Error::command("inquiry_input", error))?;
        Ok(Arc::new(Self { inner }))
    }
}
impl InquiryInput {
    // Explicit bounded copy into the one execution owner. The retained input
    // keeps its own zeroizing storage; there is no unprotected string cache.
    pub(crate) fn captured_copy(&self) -> Result<radroots::inbox::InquireInput, Error> {
        radroots::inbox::InquireInput::new(
            self.inner.thread_id(),
            self.inner.peer(),
            self.inner.source().cloned(),
            self.inner.text(),
        )
        .map_err(|error| Error::command("inquiry_input", error))
    }
}

fn public_key(value: &str, field: &'static str) -> Result<radroots::identity::PublicKey, Error> {
    let key = radroots::identity::PublicKey::from_hex(value).map_err(|_| Error::input(field))?;
    if key.to_hex() != value {
        return Err(Error::input(field));
    }
    Ok(key)
}

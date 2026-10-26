//! Initial public listing commands through the shared protected execution owner.
use crate::{Client, CommandContext, OperationReceipt};
pub use radroots_domain::listing::Listing;
pub use radroots_protocol::listing::PublishInput;

/// Borrowed public-listing access. Construction performs no host effects.
pub struct Listings<'a> {
    client: &'a Client,
}
impl<'a> Listings<'a> {
    pub(crate) const fn new(client: &'a Client) -> Self {
        Self { client }
    }
    /// Publishes an initial coordinate through actual T0/T1/S/T2/D. Current
    /// seller authority is required; another publish cannot act as an edit.
    pub async fn publish(
        &self,
        context: CommandContext,
        input: PublishInput,
    ) -> crate::ExecutionResult<OperationReceipt<Listing>> {
        self.client
            .session
            .publish_listing(self.client.journal()?, context, input)
            .await
    }
}

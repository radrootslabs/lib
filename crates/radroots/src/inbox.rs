//! Private inquiry commands through the client's actual protected journal.
use crate::{Client, CommandContext, OperationReceipt, Result};
use radroots_domain::ChatMessage;
pub use radroots_execution::inbox::{InquireInput, ListingRevisionRef};

/// Borrowed access to one client's inquiry execution boundary.
pub struct Inbox<'a> {
    client: &'a Client,
}
impl<'a> Inbox<'a> {
    pub(crate) const fn new(client: &'a Client) -> Self {
        Self { client }
    }
    /// Durably admits an inquiry and returns its actual coherent receipt.
    /// Pending state does not claim signing, installation or relay acceptance.
    /// Dropping observation never rolls back an accepted native transaction.
    pub async fn inquire(
        &self,
        context: CommandContext,
        input: InquireInput,
    ) -> Result<OperationReceipt<ChatMessage>> {
        self.client
            .session
            .inquire(self.client.journal()?, context, input)
            .await
    }
}

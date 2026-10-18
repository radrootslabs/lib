//! Current authenticated reads of original retained operations.
use crate::{Client, OperationId, OperationReceipt, Result, WorkspaceSession};
use radroots_domain::{ChatMessage, CommandDigest};

/// Reads are scoped to one explicit client-owned authenticated attachment.
pub struct Operations<'a> {
    client: &'a Client,
    scope: WorkspaceSession,
}
impl<'a> Operations<'a> {
    pub(crate) const fn new(client: &'a Client, scope: WorkspaceSession) -> Self {
        Self { client, scope }
    }
    /// Reads current original progress. Absence is not safe-nonissuance proof.
    pub async fn get(
        &self,
        operation: OperationId,
    ) -> Result<Option<OperationReceipt<ChatMessage>>> {
        self.get_matching(operation, None).await
    }
    /// Additionally requires the original semantic command commitment.
    pub async fn get_matching(
        &self,
        operation: OperationId,
        expected: Option<CommandDigest>,
    ) -> Result<Option<OperationReceipt<ChatMessage>>> {
        self.client
            .session
            .operation(self.client.journal()?, &self.scope, operation, expected)
            .await
    }
}

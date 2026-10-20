//! Current authenticated reads of original retained operations.
use crate::{Client, OperationId, Result, WorkspaceSession};
use radroots_domain::CommandDigest;

/// Reads are scoped to one explicit client-owned authenticated attachment.
pub struct Operations<'a> {
    client: &'a Client,
    scope: WorkspaceSession,
}
impl<'a> Operations<'a> {
    /// Explicitly resumes one original command. Retained signing claims never
    /// permit reissuance. Delivery retries use the original exact artifact and
    /// criterion, preserving accepted targets and the original business result.
    pub async fn resume(
        &self,
        operation: OperationId,
        expected: Option<CommandDigest>,
    ) -> Result<radroots_execution::OperationStatus> {
        self.client
            .session
            .resume_command(self.client.journal()?, &self.scope, operation, expected)
            .await
    }
    pub(crate) const fn new(client: &'a Client, scope: WorkspaceSession) -> Self {
        Self { client, scope }
    }
    /// Reads current original progress. Absence is not safe-nonissuance proof.
    pub async fn get(
        &self,
        operation: OperationId,
    ) -> Result<Option<radroots_execution::OperationStatus>> {
        self.get_matching(operation, None).await
    }
    /// Additionally requires the original semantic command commitment.
    pub async fn get_matching(
        &self,
        operation: OperationId,
        expected: Option<CommandDigest>,
    ) -> Result<Option<radroots_execution::OperationStatus>> {
        self.client
            .session
            .operation(self.client.journal()?, &self.scope, operation, expected)
            .await
    }
}

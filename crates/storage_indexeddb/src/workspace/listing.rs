//! Atomic initial-coordinate ownership, separate from stock authority.
use super::{Mutation, RecordKey, State, crypto, journal::storage_error};
use radroots_domain::{
    CommandDigest, OperationId, PublicKey, TransactionId, WorkspaceId, listing::ListingId,
};
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_storage::command::{OperationError, PreparedOperation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Decision {
    version: u8,
    workspace: WorkspaceId,
    operation: OperationId,
    command: CommandDigest,
    author: PublicKey,
    listing: ListingId,
    transaction: TransactionId,
}
impl Decision {
    fn new(prepared: &PreparedOperation) -> Result<Self, OperationError> {
        let request = prepared.request();
        let command = request
            .command()
            .listing()
            .ok_or(OperationError::UnsupportedOperation)?;
        let context = request.command().context();
        Ok(Self {
            version: 1,
            workspace: context.workspace_id(),
            operation: context.operation_id(),
            command: request.command().digest(),
            author: context.actor().participant(),
            listing: command.input().id(),
            transaction: prepared.transaction_id(),
        })
    }
    fn digest(&self) -> [u8; 32] {
        Sha256::new()
            .chain_update(b"radroots.browser.local_outbox.listing_coordinate.v1\0")
            .chain_update(self.workspace.as_bytes())
            .chain_update(self.author.as_bytes())
            .chain_update(self.listing.as_bytes())
            .finalize()
            .into()
    }
    fn key(&self) -> RecordKey {
        RecordKey::new(self.digest())
    }
    fn context(&self) -> Result<EnvelopeContext, OperationError> {
        let subject = self
            .digest()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        Ok(EnvelopeContext::new(
            EnvelopePurpose::parse("radroots.browser.workspace.listing_coordinate")
                .map_err(|_| OperationError::InvalidCommand)?,
            EnvelopeSubject::parse("listing_coordinate", subject)
                .map_err(|_| OperationError::InvalidCommand)?,
            PayloadSchemaId::parse("radroots.browser.workspace.listing_coordinate.v1")
                .map_err(|_| OperationError::InvalidCommand)?,
        ))
    }
}
impl State {
    pub(super) async fn coordinate_creation(
        &self,
        prepared: &PreparedOperation,
    ) -> Result<Vec<Mutation>, OperationError> {
        if prepared.request().command().listing().is_none() {
            return Ok(Vec::new());
        }
        let decision = Decision::new(prepared)?;
        if let Some(protected) = self
            .database()
            .get(decision.key())
            .await
            .map_err(storage_error)?
        {
            let raw = crypto::open_bound(
                self.provider.as_ref(),
                &self.reference,
                decision.context()?,
                &protected,
            )
            .await?;
            let original: Decision = raw.expose_secret(|bytes| {
                if bytes.len() > 2048 {
                    return Err(OperationError::RecoveryRequired);
                }
                serde_json::from_slice(bytes).map_err(|_| OperationError::RecoveryRequired)
            })?;
            if original.version != 1
                || original.workspace != decision.workspace
                || original.author != decision.author
                || original.listing != decision.listing
            {
                return Err(OperationError::RecoveryRequired);
            }
            let view = self.read_view(original.operation).await?;
            let records = self.operation_records(&view, original.operation).await?;
            let prepared = self
                .preparation(original.operation, &records)
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
            if Decision::new(&prepared)? != original {
                return Err(OperationError::RecoveryRequired);
            }
            return Err(OperationError::StaleState);
        }
        let raw = Zeroizing::new(
            serde_json::to_vec(&decision).map_err(|_| OperationError::InvalidCommand)?,
        );
        let protected = crypto::seal_bound(
            self.provider.as_ref(),
            &self.reference,
            decision.context()?,
            &raw,
        )
        .await?;
        Ok(vec![
            Mutation::new(decision.key(), None, Some(protected)).map_err(storage_error)?,
        ])
    }
    /// Verifies the exact T0 decision and returns an unchanged CAS entry. It is
    /// included with every later write, even though the index is outside the
    /// operation prefix. There is no overwrite, lease or automatic reclamation.
    pub(super) async fn coordinate_guard(
        &self,
        prepared: &PreparedOperation,
    ) -> Result<Vec<Mutation>, OperationError> {
        if prepared.request().command().listing().is_none() {
            return Ok(Vec::new());
        }
        if prepared.request().binding().scope()
            != radroots_storage::workspace::LocalOutboxScope::NewSellerOutbox
        {
            return Err(OperationError::RecoveryRequired);
        }
        let expected = Decision::new(prepared)?;
        let protected = self
            .database()
            .get(expected.key())
            .await
            .map_err(storage_error)?
            .ok_or(OperationError::RecoveryRequired)?;
        let raw = crypto::open_bound(
            self.provider.as_ref(),
            &self.reference,
            expected.context()?,
            &protected,
        )
        .await?;
        let decision: Decision = raw.expose_secret(|bytes| {
            if bytes.len() > 2048 {
                return Err(OperationError::RecoveryRequired);
            }
            serde_json::from_slice(bytes).map_err(|_| OperationError::RecoveryRequired)
        })?;
        if decision != expected {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(vec![
            Mutation::new(expected.key(), Some(protected.clone()), Some(protected))
                .map_err(storage_error)?,
        ])
    }
}

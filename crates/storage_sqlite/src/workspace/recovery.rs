//! Original obtained-evidence recovery on the same namespace owner.
use super::{
    AttachmentInput, AttachmentProof, WorkspaceClock, WorkspaceCrypto, admission::Journal,
    fs::Namespace, record::GrantRecord, worker,
};
use radroots_storage::command::{ArtifactEvidence, EvidenceCaptureOutcome, OperationError};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

pub(super) struct RecoveryAuthorization {
    grant: GrantRecord,
    close_confirmed: bool,
}
pub(super) struct RecoveryTurn<'a> {
    pub connection: &'a mut Option<SqliteConnection>,
    pub cached_grant: &'a mut Option<RecoveryAuthorization>,
    pub namespace: &'a Namespace,
    pub selected: &'a AttachmentInput,
    pub crypto: &'a WorkspaceCrypto,
    pub clock: &'a dyn WorkspaceClock,
    pub uncertain: &'a mut bool,
    pub options: &'a SqliteConnectOptions,
}
impl RecoveryTurn<'_> {
    pub(super) async fn capture(
        &mut self,
        input: AttachmentInput,
        proof: AttachmentProof,
        evidence: ArtifactEvidence,
    ) -> Result<EvidenceCaptureOutcome, OperationError> {
        if input != *self.selected {
            return Err(OperationError::Unauthorized);
        }
        if !*self.uncertain {
            let connection = self
                .connection
                .as_mut()
                .ok_or(OperationError::RecoveryRequired)?;
            return Journal {
                connection,
                namespace: self.namespace,
                selected: self.selected,
                crypto: self.crypto,
                clock: self.clock,
                uncertain: self.uncertain,
            }
            .capture_issuance(input, proof, evidence)
            .await;
        }

        self.namespace.validate()?;
        // Authenticate original capture ownership before disposing any existing
        // connection. No current lease, clock or fresh command grant is acquired.
        if let Some(connection) = self.connection.as_mut() {
            let protected = worker::read(connection, self.selected).await?;
            let plaintext = self
                .crypto
                .open(
                    self.selected.workspace_id(),
                    self.selected.grant_id(),
                    &protected,
                )
                .await?;
            let record = plaintext.expose_secret(GrantRecord::decode_outbox)?;
            record.authenticate_retained_capture(&input, &proof)?;
            *self.cached_grant = Some(RecoveryAuthorization {
                grant: record,
                close_confirmed: false,
            });
        } else {
            let cached = self
                .cached_grant
                .as_ref()
                .ok_or(OperationError::RecoveryRequired)?;
            cached.grant.authenticate_retained_capture(&input, &proof)?;
            if !cached.close_confirmed {
                return Err(OperationError::RecoveryRequired);
            }
        }

        // Namespace authority stays owned throughout the close, read-only
        // preflight and replacement connection. Missing/foreign schemas are
        // refused; there is no initialization, migration or restored dispatch.
        if let Some(connection) = self.connection.take() {
            connection
                .close()
                .await
                .map_err(|_| OperationError::RecoveryRequired)?;
            self.cached_grant
                .as_mut()
                .ok_or(OperationError::RecoveryRequired)?
                .close_confirmed = true;
        }
        worker::preflight_existing(self.namespace, self.selected, self.crypto).await?;
        let connection = SqliteConnection::connect_with(self.options)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
        *self.connection = Some(connection);
        self.cached_grant
            .as_mut()
            .ok_or(OperationError::RecoveryRequired)?
            .close_confirmed = false;
        let connection = self
            .connection
            .as_mut()
            .ok_or(OperationError::RecoveryRequired)?;
        // A failed validation retains this replacement connection. Its close
        // must be confirmed by the same owner before any subsequent replacement.
        worker::initialize(connection, self.namespace, self.selected, None, self.crypto).await?;
        let mut capture_uncertain = false;
        let result = Journal {
            connection,
            namespace: self.namespace,
            selected: self.selected,
            crypto: self.crypto,
            clock: self.clock,
            uncertain: &mut capture_uncertain,
        }
        .capture_issuance(input, proof, evidence)
        .await;
        if matches!(&result, Ok(EvidenceCaptureOutcome::Committed(_))) {
            *self.uncertain = false;
            self.cached_grant.take();
        }
        result
    }
}

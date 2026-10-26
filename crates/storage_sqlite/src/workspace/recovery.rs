//! Original obtained-evidence recovery on the same namespace owner.
use super::{
    AttachmentInput, AttachmentProof, WorkspaceClock, WorkspaceCrypto,
    admission::Journal,
    fs::{Kind, Namespace},
    record::GrantRecord,
    stock, worker,
};
use radroots_storage::{
    command::{ArtifactEvidence, EvidenceCaptureOutcome, OperationError},
    inventory::{StockEvidenceOutcome, StockEvidenceRequest},
    workspace::seller_stock::SellerAuthoritySelection,
};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

enum GrantAuthorization {
    Outbox(GrantRecord),
    Stock(stock::CaptureAuthorization),
}
impl GrantAuthorization {
    fn authenticate(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), OperationError> {
        match self {
            Self::Outbox(record) => record
                .authenticate_retained_capture(input, proof)
                .map_err(Into::into),
            Self::Stock(record) => record.authenticate(input, proof).map_err(Into::into),
        }
    }
}
pub(super) struct RecoveryAuthorization {
    grant: GrantAuthorization,
    close_confirmed: bool,
}
enum Material {
    Outbox(Box<(AttachmentInput, ArtifactEvidence)>),
    Stock(Box<StockEvidenceRequest>),
}
impl Material {
    fn input(&self) -> &AttachmentInput {
        match self {
            Self::Outbox(value) => &value.0,
            Self::Stock(value) => value.binding().input(),
        }
    }
}
enum Captured {
    Outbox(EvidenceCaptureOutcome),
    Stock(StockEvidenceOutcome),
}
impl Captured {
    fn committed(&self) -> bool {
        matches!(
            self,
            Self::Outbox(EvidenceCaptureOutcome::Committed(_))
                | Self::Stock(StockEvidenceOutcome::Committed(_))
        )
    }
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
        if self.namespace.kind() != Kind::Outbox {
            return Err(OperationError::UnsupportedOperation);
        }
        if input != *self.selected {
            return Err(OperationError::Unauthorized);
        }
        match self
            .recover(Material::Outbox(Box::new((input, evidence))), proof)
            .await?
        {
            Captured::Outbox(value) => Ok(value),
            _ => Err(OperationError::RecoveryRequired),
        }
    }
    pub(super) async fn capture_stock(
        &mut self,
        request: StockEvidenceRequest,
        proof: AttachmentProof,
    ) -> Result<StockEvidenceOutcome, OperationError> {
        if self.namespace.kind() != Kind::SellerStock {
            return Err(OperationError::UnsupportedOperation);
        }
        if !SellerAuthoritySelection::from_attachment(self.selected)
            .matches_attachment(request.binding().input())
        {
            return Err(OperationError::Unauthorized);
        }
        match self
            .recover(Material::Stock(Box::new(request)), proof)
            .await?
        {
            Captured::Stock(value) => Ok(value),
            _ => Err(OperationError::RecoveryRequired),
        }
    }
    async fn recover(
        &mut self,
        material: Material,
        proof: AttachmentProof,
    ) -> Result<Captured, OperationError> {
        let replacing = *self.uncertain;
        if replacing {
            self.prepare(material.input(), &proof).await?;
        }
        let connection = self
            .connection
            .as_mut()
            .ok_or(OperationError::RecoveryRequired)?;
        let mut capture_uncertain = false;
        let uncertainty = if replacing {
            &mut capture_uncertain
        } else {
            &mut *self.uncertain
        };
        let result = match material {
            Material::Outbox(value) => {
                let (input, evidence) = *value;
                Journal {
                    connection,
                    namespace: self.namespace,
                    selected: self.selected,
                    crypto: self.crypto,
                    clock: self.clock,
                    uncertain: uncertainty,
                }
                .capture_issuance(input, proof, evidence)
                .await
                .map(Captured::Outbox)
            }
            Material::Stock(request) => Box::pin(stock::issuance::evidence::capture(
                &mut stock::inventory::Turn {
                    connection,
                    namespace: self.namespace,
                    selected: self.selected,
                    crypto: self.crypto,
                    clock: self.clock,
                    uncertain: uncertainty,
                },
                *request,
                proof,
            ))
            .await
            .map(Captured::Stock),
        };
        if replacing && result.as_ref().is_ok_and(Captured::committed) {
            *self.uncertain = false;
            self.cached_grant.take();
        }
        result
    }
    async fn prepare(
        &mut self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), OperationError> {
        self.namespace.validate()?;
        // Authenticate original capture ownership before disposing the existing
        // connection. No current lease, clock or fresh command grant is acquired.
        if let Some(connection) = self.connection.as_mut() {
            let grant = match self.namespace.kind() {
                Kind::Outbox => {
                    let protected = worker::read(connection, self.selected).await?;
                    let plaintext = self
                        .crypto
                        .open(
                            self.selected.workspace_id(),
                            self.selected.grant_id(),
                            &protected,
                        )
                        .await?;
                    GrantAuthorization::Outbox(plaintext.expose_secret(GrantRecord::decode_outbox)?)
                }
                Kind::SellerStock => GrantAuthorization::Stock(
                    stock::capture_authorization(connection, input, self.crypto, proof).await?,
                ),
            };
            grant.authenticate(input, proof)?;
            *self.cached_grant = Some(RecoveryAuthorization {
                grant,
                close_confirmed: false,
            });
        } else {
            let cached = self
                .cached_grant
                .as_ref()
                .ok_or(OperationError::RecoveryRequired)?;
            cached.grant.authenticate(input, proof)?;
            if !cached.close_confirmed {
                return Err(OperationError::RecoveryRequired);
            }
        }
        // The namespace writer lease stays owned through confirmed close,
        // read-only preflight and replacement. No initialization or migration.
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
        // A failed validation retains the replacement; its close must be
        // confirmed by this owner before a subsequent replacement.
        match self.namespace.kind() {
            Kind::Outbox => {
                worker::initialize(connection, self.namespace, self.selected, None, self.crypto)
                    .await?
            }
            Kind::SellerStock => {
                stock::initialize(connection, self.namespace, self.selected, None, self.crypto)
                    .await?
            }
        }
        Ok(())
    }
}

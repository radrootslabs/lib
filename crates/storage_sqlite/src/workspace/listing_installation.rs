//! Atomic initial-listing revision and exact public-work retention. No signer,
//! provider, transport or stock allocator runs inside the write transaction.
use super::{
    AttachmentInput, AttachmentProof,
    admission::{Journal, recheck},
    crypto::{IssuancePayload, IssuanceScope, WorkspaceCrypto},
    listing,
    snapshot::{RetainedFacts, RetainedGraph},
};
use radroots_domain::{
    CommandDigest, EnvelopeEventId, OperationId, SigningAttemptId, TransactionId, UnixSeconds,
    WorkspaceId,
};
use radroots_storage::command::{
    BoundSigningPlan, CommitAttempt, EvidenceReceipt, InstallationOutcome, InstallationRequest,
    ListingInstallation, OperationError, PublicPublicationWork, SigningTarget,
};
use serde::{Deserialize, Serialize};
use sqlx::{Connection, Row, SqliteConnection};
use zeroize::Zeroizing;

pub(super) const TABLE: &str = "CREATE TABLE listing_installations (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), plan_digest BLOB NOT NULL CHECK(length(plan_digest)=32), event_id BLOB NOT NULL CHECK(length(event_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id), FOREIGN KEY(workspace,operation_id) REFERENCES listing_creations(workspace,operation_id)) STRICT, WITHOUT ROWID";
pub(super) const WORK: &str = "CREATE TABLE public_publication_work (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), plan_digest BLOB NOT NULL CHECK(length(plan_digest)=32), event_id BLOB NOT NULL CHECK(length(event_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), artifact_key BLOB NOT NULL CHECK(length(artifact_key)=1 AND hex(artifact_key)='00'), PRIMARY KEY(workspace,operation_id), FOREIGN KEY(workspace,operation_id) REFERENCES listing_installations(workspace,operation_id), FOREIGN KEY(workspace,operation_id,artifact_key) REFERENCES artifact_evidence(workspace,operation_id,artifact_key)) STRICT, WITHOUT ROWID";
pub(super) const TX_INDEX: &str = "CREATE UNIQUE INDEX listing_installation_transaction_identity ON listing_installations(workspace,transaction_id)";
#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct PublicEvidenceRef {
    workspace: WorkspaceId,
    operation: OperationId,
    command: CommandDigest,
    attempt: SigningAttemptId,
    event: EnvelopeEventId,
    digest: [u8; 32],
    transaction: TransactionId,
    observed_at: Option<u64>,
}
impl PublicEvidenceRef {
    pub(super) fn new(e: EvidenceReceipt) -> Self {
        Self {
            workspace: e.workspace_id(),
            operation: e.operation_id(),
            command: e.command_digest(),
            attempt: e.attempt_id(),
            event: e.event_id(),
            digest: *e.digest(),
            transaction: e.transaction_id(),
            observed_at: e.observed_at_unix_ms().map(core::num::NonZeroU64::get),
        }
    }
}
#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct InstallationRecord {
    version: u8,
    plan: [u8; 32],
    preparation: TransactionId,
    plan_transaction: TransactionId,
    transaction: TransactionId,
    installed_at: UnixSeconds,
    evidence: PublicEvidenceRef,
}
impl InstallationRecord {
    fn new(i: ListingInstallation) -> Self {
        Self {
            version: 1,
            plan: *i.plan_digest(),
            preparation: i.preparation_transaction_id(),
            plan_transaction: i.plan_transaction_id(),
            transaction: i.transaction_id(),
            installed_at: i.installed_at(),
            evidence: PublicEvidenceRef::new(i.basis()),
        }
    }
}
#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct WorkRecord {
    version: u8,
    installation: TransactionId,
    plan: [u8; 32],
    transaction: TransactionId,
    queued_at: UnixSeconds,
    evidence: PublicEvidenceRef,
}
impl WorkRecord {
    fn new(w: PublicPublicationWork) -> Self {
        Self {
            version: 1,
            installation: w.installation_transaction_id(),
            plan: *w.plan_digest(),
            transaction: w.transaction_id(),
            queued_at: w.queued_at(),
            evidence: PublicEvidenceRef::new(w.evidence()),
        }
    }
}
fn scope(bound: &BoundSigningPlan, evidence: EvidenceReceipt) -> IssuanceScope {
    IssuanceScope {
        workspace: evidence.workspace_id(),
        operation: evidence.operation_id(),
        plan_digest: *bound.plan().digest(),
        attempt: evidence.attempt_id(),
        target: SigningTarget::PublicEvent,
    }
}
struct Protected<T> {
    report: T,
    protected: Vec<u8>,
}
pub(super) struct Records {
    installation: Option<Protected<ListingInstallation>>,
    work: Option<Protected<PublicPublicationWork>>,
}
impl Records {
    pub(super) fn installation(&self) -> Option<ListingInstallation> {
        self.installation.as_ref().map(|v| v.report)
    }
    pub(super) fn work(&self) -> Option<PublicPublicationWork> {
        self.work.as_ref().map(|v| v.report)
    }
    pub(super) async fn read(
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        operation: OperationId,
        graph: &RetainedGraph,
        crypto: &WorkspaceCrypto,
    ) -> Result<Self, OperationError> {
        let empty = Self {
            installation: None,
            work: None,
        };
        if !listing::current(connection).await? {
            return Ok(empty);
        }
        let installation = read_row(connection, selected, operation, Kind::Installation).await?;
        let work = read_row(connection, selected, operation, Kind::Work).await?;
        if installation.is_none() && work.is_none() {
            return Ok(empty);
        }
        let bound = graph.bound_plan().ok_or(OperationError::RecoveryRequired)?;
        if bound.plan().listing().is_none() {
            return Err(OperationError::RecoveryRequired);
        }
        let evidence = graph
            .reports()
            .find(|r| r.target() == SigningTarget::PublicEvent)
            .and_then(|r| r.obtained())
            .ok_or(OperationError::RecoveryRequired)?;
        let installation = installation.ok_or(OperationError::RecoveryRequired)?;
        let work = work.ok_or(OperationError::RecoveryRequired)?;
        let raw = crypto
            .open_issuance(
                scope(bound, evidence),
                IssuancePayload::ListingInstallation,
                0,
                &installation.protected,
            )
            .await?;
        let encoded: InstallationRecord = raw
            .expose_secret(|raw| serde_json::from_slice(raw))
            .map_err(|_| OperationError::RecoveryRequired)?;
        let installed = ListingInstallation::from_committed_store(
            bound,
            evidence,
            encoded.transaction,
            encoded.installed_at,
        )?;
        if encoded != InstallationRecord::new(installed)
            || !installation.matches(
                installed.transaction_id(),
                installed.plan_digest(),
                installed.revision(),
            )
        {
            return Err(OperationError::RecoveryRequired);
        }
        let raw = crypto
            .open_issuance(
                scope(bound, evidence),
                IssuancePayload::Publication,
                0,
                &work.protected,
            )
            .await?;
        let encoded: WorkRecord = raw
            .expose_secret(|raw| serde_json::from_slice(raw))
            .map_err(|_| OperationError::RecoveryRequired)?;
        let queued = PublicPublicationWork::from_committed_store(
            bound,
            installed,
            evidence,
            encoded.transaction,
            encoded.queued_at,
        )?;
        if encoded != WorkRecord::new(queued)
            || !work.matches(
                queued.transaction_id(),
                queued.plan_digest(),
                evidence.event_id(),
            )
            || queued.transaction_id() != installed.transaction_id()
            || queued.queued_at() != installed.installed_at()
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self {
            installation: Some(Protected {
                report: installed,
                protected: installation.protected,
            }),
            work: Some(Protected {
                report: queued,
                protected: work.protected,
            }),
        })
    }
    pub(super) async fn unchanged(
        &self,
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        operation: OperationId,
    ) -> Result<(), OperationError> {
        if !listing::current(connection).await? {
            return Ok(());
        }
        let current = read_row(connection, selected, operation, Kind::Installation).await?;
        match (&self.installation, current) {
            (None, None) => (),
            (Some(a), Some(b))
                if b.matches(
                    a.report.transaction_id(),
                    a.report.plan_digest(),
                    a.report.revision(),
                ) && a.protected == b.protected => {}
            _ => return Err(OperationError::RecoveryRequired),
        }
        let current = read_row(connection, selected, operation, Kind::Work).await?;
        match (&self.work, current) {
            (None, None) => (),
            (Some(a), Some(b))
                if b.matches(
                    a.report.transaction_id(),
                    a.report.plan_digest(),
                    a.report.evidence().event_id(),
                ) && a.protected == b.protected => {}
            _ => return Err(OperationError::RecoveryRequired),
        }
        Ok(())
    }
}
enum Kind {
    Installation,
    Work,
}
struct RowFacts {
    transaction: Vec<u8>,
    plan: Vec<u8>,
    event: Vec<u8>,
    protected: Vec<u8>,
}
impl RowFacts {
    fn matches(&self, tx: TransactionId, plan: &[u8; 32], event: EnvelopeEventId) -> bool {
        self.transaction == tx.as_bytes() && self.plan == plan && self.event == event.as_bytes()
    }
}
async fn read_row(
    connection: &mut SqliteConnection,
    selected: &AttachmentInput,
    operation: OperationId,
    kind: Kind,
) -> Result<Option<RowFacts>, OperationError> {
    let query = match kind {
        Kind::Installation => {
            "SELECT CASE WHEN length(transaction_id)=32 THEN transaction_id END,CASE WHEN length(plan_digest)=32 THEN plan_digest END,CASE WHEN length(event_id)=32 THEN event_id END,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM listing_installations WHERE workspace=? AND operation_id=?"
        }
        Kind::Work => {
            "SELECT CASE WHEN length(transaction_id)=32 THEN transaction_id END,CASE WHEN length(plan_digest)=32 THEN plan_digest END,CASE WHEN length(event_id)=32 THEN event_id END,CASE WHEN length(protected)>0 AND length(protected)<=262144 AND length(artifact_key)=1 AND hex(artifact_key)='00' THEN protected END FROM public_publication_work WHERE workspace=? AND operation_id=?"
        }
    };
    let row = sqlx::query(query)
        .bind(selected.workspace_id().as_bytes().as_slice())
        .bind(operation.as_bytes().as_slice())
        .fetch_optional(connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    row.map(|r| {
        let bytes = |i| -> Result<Vec<u8>, OperationError> {
            r.try_get::<Option<Vec<u8>>, _>(i)
                .map_err(|_| OperationError::RecoveryRequired)?
                .ok_or(OperationError::RecoveryRequired)
        };
        Ok(RowFacts {
            transaction: bytes(0)?,
            plan: bytes(1)?,
            event: bytes(2)?,
            protected: bytes(3)?,
        })
    })
    .transpose()
}
pub(super) async fn absent(
    connection: &mut SqliteConnection,
    selected: &AttachmentInput,
    operation: OperationId,
) -> Result<(), OperationError> {
    if !listing::current(connection).await? {
        return Ok(());
    }
    if read_row(connection, selected, operation, Kind::Installation)
        .await?
        .is_some()
        || read_row(connection, selected, operation, Kind::Work)
            .await?
            .is_some()
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
impl Journal<'_> {
    pub(super) async fn install_listing(
        &mut self,
        request: InstallationRequest,
        proof: AttachmentProof,
        record: super::record::GrantRecord,
        grant: Vec<u8>,
        current: super::WorkspaceBinding,
        graph: RetainedGraph,
    ) -> Result<InstallationOutcome, OperationError> {
        if !listing::current(self.connection).await? {
            return Err(OperationError::UnsupportedOperation);
        }
        let facts = RetainedFacts::from_graph(
            self.connection,
            self.selected,
            request.operation_id(),
            graph,
            self.crypto,
        )
        .await?;
        let bound = facts
            .graph()
            .bound_plan()
            .ok_or(OperationError::RecoveryRequired)?;
        if bound.plan().digest() != request.plan_digest() {
            return Err(OperationError::IdempotencyConflict);
        }
        let retained = facts.listing();
        let evidence = facts
            .graph()
            .reports()
            .find(|r| r.target() == SigningTarget::PublicEvent)
            .and_then(|r| r.obtained())
            .ok_or(OperationError::InvalidEvidence)?;
        let fresh = retained.installation().is_none();
        let installed = match retained.installation() {
            Some(v) => v,
            None => ListingInstallation::from_committed_store(
                bound,
                evidence,
                TransactionId::from_bytes(super::random()?),
                current.observed_at(),
            )?,
        };
        let prepared = if fresh {
            let work = PublicPublicationWork::from_committed_store(
                bound,
                installed,
                evidence,
                installed.transaction_id(),
                installed.installed_at(),
            )?;
            let raw = Zeroizing::new(
                serde_json::to_vec(&InstallationRecord::new(installed))
                    .map_err(|_| OperationError::InvalidCommand)?,
            );
            let installation = self
                .crypto
                .seal_issuance(
                    scope(bound, evidence),
                    IssuancePayload::ListingInstallation,
                    0,
                    &raw,
                )
                .await?;
            let raw = Zeroizing::new(
                serde_json::to_vec(&WorkRecord::new(work))
                    .map_err(|_| OperationError::InvalidCommand)?,
            );
            let work = self
                .crypto
                .seal_issuance(
                    scope(bound, evidence),
                    IssuancePayload::Publication,
                    0,
                    &raw,
                )
                .await?;
            Some((installation, work))
        } else {
            None
        };
        let anchor = CommitAttempt::new(
            installed.workspace_id(),
            installed.operation_id(),
            installed.command_digest(),
            Some(installed.transaction_id()),
        );
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let checked=async {
            let now=recheck(&mut transaction,self.namespace,self.selected,&grant,&record,request.binding(),&proof,self.clock).await?;
            facts.unchanged(&mut transaction,self.selected,request.operation_id()).await?;
            if fresh && now.observed_at()<current.observed_at(){return Err(super::WorkspaceError::Unavailable.into());}
            if let Some((installation,work))=&prepared {
                sqlx::query("INSERT INTO listing_installations(workspace,operation_id,transaction_id,plan_digest,event_id,protected) VALUES(?,?,?,?,?,?)")
                    .bind(installed.workspace_id().as_bytes().as_slice()).bind(installed.operation_id().as_bytes().as_slice()).bind(installed.transaction_id().as_bytes().as_slice()).bind(installed.plan_digest().as_slice()).bind(installed.revision().as_bytes().as_slice()).bind(installation).execute(&mut *transaction).await.map_err(|_|OperationError::StorageFailure)?;
                sqlx::query("INSERT INTO public_publication_work(workspace,operation_id,transaction_id,plan_digest,event_id,protected,artifact_key) VALUES(?,?,?,?,?,?,X'00')")
                    .bind(installed.workspace_id().as_bytes().as_slice()).bind(installed.operation_id().as_bytes().as_slice()).bind(installed.transaction_id().as_bytes().as_slice()).bind(installed.plan_digest().as_slice()).bind(installed.revision().as_bytes().as_slice()).bind(work).execute(&mut *transaction).await.map_err(|_|OperationError::StorageFailure)?;
            }
            Ok::<_,OperationError>(())
        }.await;
        if let Err(error) = checked {
            if transaction.rollback().await.is_err() {
                *self.uncertain = true;
                return Ok(InstallationOutcome::Unknown(anchor));
            }
            return Err(error);
        }
        let completed = if fresh {
            transaction.commit().await
        } else {
            transaction.rollback().await
        };
        if completed.is_err() {
            *self.uncertain = true;
            return Ok(InstallationOutcome::Unknown(anchor));
        }
        Ok(InstallationOutcome::ListingCommitted(Box::new(installed)))
    }
}

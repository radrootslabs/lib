//! Protected logical chat installation and original recipient publication links.
//! Bodies remain in the authenticated captured command; no private text is
//! duplicated or reserialized into these fixed bounded transaction records.
use super::{
    AttachmentInput, AttachmentProof,
    admission::{Journal, recheck},
    crypto::{IssuancePayload, IssuanceScope, WorkspaceCrypto},
    snapshot::RetainedGraph,
};
use core::num::NonZeroU64;
use radroots_domain::{
    CommandDigest, EnvelopeEventId, MessageId, OperationId, PublicKey, SigningAttemptId, ThreadId,
    TransactionId, UnixSeconds, WorkspaceId,
};
use radroots_storage::command::{
    BoundSigningPlan, ChatInstallation, CommitAttempt, EvidenceReceipt, InstallationOutcome,
    InstallationRequest, OperationError, PublicationWork,
};
use serde::{Deserialize, Serialize};
use sqlx::{Connection, Row, SqliteConnection};
use zeroize::Zeroizing;

pub(super) const TABLE: &str = "CREATE TABLE chat_messages (workspace BLOB NOT NULL, message_id BLOB NOT NULL CHECK(length(message_id)=32), operation_id BLOB NOT NULL, thread_id BLOB NOT NULL CHECK(length(thread_id)=32), plan_digest BLOB NOT NULL CHECK(length(plan_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,message_id), FOREIGN KEY(workspace,operation_id) REFERENCES signing_plans(workspace,operation_id)) STRICT, WITHOUT ROWID";
pub(super) const OP_INDEX: &str =
    "CREATE UNIQUE INDEX chat_message_operation_identity ON chat_messages(workspace,operation_id)";
pub(super) const TX_INDEX: &str = "CREATE UNIQUE INDEX chat_message_transaction_identity ON chat_messages(workspace,transaction_id)";
pub(super) const THREAD_INDEX: &str =
    "CREATE INDEX chat_message_thread ON chat_messages(workspace,thread_id,message_id)";
pub(super) const WORK: &str = "CREATE TABLE chat_publication_work (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL CHECK(length(recipient)=32), installation_transaction BLOB NOT NULL CHECK(length(installation_transaction)=32), capture_transaction BLOB NOT NULL CHECK(length(capture_transaction)=32), event_id BLOB NOT NULL CHECK(length(event_id)=32), evidence_digest BLOB NOT NULL CHECK(length(evidence_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient), FOREIGN KEY(workspace,operation_id) REFERENCES chat_messages(workspace,operation_id), FOREIGN KEY(workspace,operation_id,recipient) REFERENCES artifact_evidence(workspace,operation_id,artifact_key)) STRICT, WITHOUT ROWID";
pub(super) const WORK_V8: &str = "CREATE TABLE chat_publication_work (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL CHECK(length(recipient)=32), installation_transaction BLOB NOT NULL CHECK(length(installation_transaction)=32), capture_transaction BLOB NOT NULL CHECK(length(capture_transaction)=32), event_id BLOB NOT NULL CHECK(length(event_id)=32), evidence_digest BLOB NOT NULL CHECK(length(evidence_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient), FOREIGN KEY(workspace,operation_id) REFERENCES chat_messages(workspace,operation_id), FOREIGN KEY(workspace,operation_id,recipient) REFERENCES seal_evidence(workspace,operation_id,recipient)) STRICT, WITHOUT ROWID";

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct EvidenceRef {
    workspace: WorkspaceId,
    operation: OperationId,
    command: CommandDigest,
    recipient: PublicKey,
    attempt: SigningAttemptId,
    event: EnvelopeEventId,
    digest: [u8; 32],
    transaction: TransactionId,
    observed_at: Option<u64>,
}
impl EvidenceRef {
    pub(super) fn new(value: EvidenceReceipt, recipient: PublicKey) -> Self {
        Self {
            workspace: value.workspace_id(),
            operation: value.operation_id(),
            command: value.command_digest(),
            recipient,
            attempt: value.attempt_id(),
            event: value.event_id(),
            digest: *value.digest(),
            transaction: value.transaction_id(),
            observed_at: value.observed_at_unix_ms().map(NonZeroU64::get),
        }
    }
}
#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct MessageRecord {
    version: u8,
    plan_digest: [u8; 32],
    message: MessageId,
    thread: ThreadId,
    preparation_transaction: TransactionId,
    plan_transaction: TransactionId,
    transaction: TransactionId,
    installed_at: UnixSeconds,
    basis: EvidenceRef,
}
impl MessageRecord {
    fn new(bound: &BoundSigningPlan, value: ChatInstallation) -> Result<Self, OperationError> {
        Ok(Self {
            version: 1,
            plan_digest: *value.plan_digest(),
            message: value.message_id(),
            thread: bound.prepared().request().chat_command()?.thread_id(),
            preparation_transaction: value.preparation_transaction_id(),
            plan_transaction: value.plan_transaction_id(),
            transaction: value.transaction_id(),
            installed_at: value.installed_at(),
            basis: EvidenceRef::new(value.basis(), value.basis().recipient()?),
        })
    }
}
#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct WorkRecord {
    version: u8,
    plan_digest: [u8; 32],
    message: MessageId,
    installation_transaction: TransactionId,
    preparation_transaction: TransactionId,
    plan_transaction: TransactionId,
    transaction: TransactionId,
    queued_at: UnixSeconds,
    evidence: EvidenceRef,
}
impl WorkRecord {
    fn new(installed: ChatInstallation, value: PublicationWork) -> Self {
        Self {
            version: 1,
            plan_digest: *value.plan_digest(),
            message: installed.message_id(),
            installation_transaction: value.installation_transaction_id(),
            preparation_transaction: installed.preparation_transaction_id(),
            plan_transaction: installed.plan_transaction_id(),
            transaction: value.transaction_id(),
            queued_at: value.queued_at(),
            evidence: EvidenceRef::new(value.evidence(), value.recipient()),
        }
    }
}
pub(super) struct ProtectedMessage {
    pub report: ChatInstallation,
    pub thread: ThreadId,
    pub protected: Vec<u8>,
}
pub(super) struct ProtectedWork {
    pub report: PublicationWork,
    pub protected: Vec<u8>,
}
pub(super) struct Records {
    pub message: Option<ProtectedMessage>,
    pub work: Vec<ProtectedWork>,
}
fn scope(bound: &BoundSigningPlan, receipt: EvidenceReceipt) -> IssuanceScope {
    IssuanceScope {
        workspace: receipt.workspace_id(),
        operation: receipt.operation_id(),
        plan_digest: *bound.plan().digest(),
        attempt: receipt.attempt_id(),
        target: receipt.target(),
    }
}
fn bytes(row: &sqlx::sqlite::SqliteRow, index: usize) -> Result<Vec<u8>, OperationError> {
    row.try_get::<Option<Vec<u8>>, _>(index)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)
}
fn obtained(
    graph: &RetainedGraph,
    recipient: PublicKey,
) -> Result<EvidenceReceipt, OperationError> {
    graph
        .reports()
        .find(|value| value.target().recipient() == Some(recipient))
        .and_then(|value| value.obtained())
        .ok_or(OperationError::RecoveryRequired)
}
impl Records {
    pub(super) async fn read(
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        operation: OperationId,
        graph: &RetainedGraph,
        crypto: &WorkspaceCrypto,
    ) -> Result<Self, OperationError> {
        // At most one message and two recipient work rows can be materialized;
        // fixed 262144-byte protected bounds cap their total below 1 MiB.
        // Excess or orphan rows are rejected by bounded transaction sentinels.
        let row=sqlx::query("SELECT CASE WHEN length(message_id)=32 THEN message_id END,CASE WHEN length(thread_id)=32 THEN thread_id END,CASE WHEN length(plan_digest)=32 THEN plan_digest END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM chat_messages WHERE workspace=? AND operation_id=? LIMIT 1")
            .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        let message = if let Some(row) = row {
            let bound = graph.bound_plan().ok_or(OperationError::RecoveryRequired)?;
            let protected = bytes(&row, 4)?;
            let plaintext = crypto
                .open_message(
                    selected.workspace_id(),
                    operation,
                    bound.plan().digest(),
                    &protected,
                )
                .await?;
            let encoded: MessageRecord = plaintext
                .expose_secret(|raw| serde_json::from_slice(raw))
                .map_err(|_| OperationError::RecoveryRequired)?;
            let basis = obtained(graph, encoded.basis.recipient)?;
            let report = ChatInstallation::from_committed_store(
                bound,
                basis,
                encoded.transaction,
                encoded.installed_at,
            )?;
            if encoded != MessageRecord::new(bound, report)?
                || bytes(&row, 0)? != report.message_id().as_bytes()
                || bytes(&row, 1)?
                    != bound
                        .prepared()
                        .request()
                        .chat_command()?
                        .thread_id()
                        .as_bytes()
                || bytes(&row, 2)? != report.plan_digest()
                || bytes(&row, 3)? != report.transaction_id().as_bytes()
            {
                return Err(OperationError::RecoveryRequired);
            }
            Some(ProtectedMessage {
                report,
                thread: encoded.thread,
                protected,
            })
        } else {
            None
        };
        let mut work = Vec::new();
        if let Some(bound) = graph.bound_plan() {
            for recipient in graph
                .reports()
                .filter_map(|value| value.target().recipient())
            {
                let row=sqlx::query("SELECT CASE WHEN length(installation_transaction)=32 THEN installation_transaction END,CASE WHEN length(capture_transaction)=32 THEN capture_transaction END,CASE WHEN length(event_id)=32 THEN event_id END,CASE WHEN length(evidence_digest)=32 THEN evidence_digest END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM chat_publication_work WHERE workspace=? AND operation_id=? AND recipient=?")
                    .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(recipient.as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
                if let Some(row) = row {
                    let installed = message
                        .as_ref()
                        .ok_or(OperationError::RecoveryRequired)?
                        .report;
                    let evidence = obtained(graph, recipient)?;
                    let protected = bytes(&row, 5)?;
                    let plaintext = crypto
                        .open_issuance(
                            scope(bound, evidence),
                            IssuancePayload::Publication,
                            0,
                            &protected,
                        )
                        .await?;
                    let encoded: WorkRecord = plaintext
                        .expose_secret(|raw| serde_json::from_slice(raw))
                        .map_err(|_| OperationError::RecoveryRequired)?;
                    let report = PublicationWork::from_committed_store(
                        bound,
                        installed,
                        evidence,
                        encoded.transaction,
                        encoded.queued_at,
                    )?;
                    if encoded != WorkRecord::new(installed, report)
                        || bytes(&row, 0)? != installed.transaction_id().as_bytes()
                        || bytes(&row, 1)? != evidence.transaction_id().as_bytes()
                        || bytes(&row, 2)? != evidence.event_id().as_bytes()
                        || bytes(&row, 3)? != evidence.digest()
                        || bytes(&row, 4)? != report.transaction_id().as_bytes()
                    {
                        return Err(OperationError::RecoveryRequired);
                    }
                    work.push(ProtectedWork { report, protected });
                }
            }
        }
        if let Some(message) = &message
            && !work
                .iter()
                .any(|value| value.report.evidence() == message.report.basis())
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Self { message, work })
    }
    pub(super) async fn unchanged(
        &self,
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        operation: OperationId,
    ) -> Result<(), OperationError> {
        let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM (SELECT 1 FROM chat_messages WHERE workspace=?1 AND operation_id=?2 LIMIT 2)),(SELECT count(*) FROM (SELECT 1 FROM chat_publication_work WHERE workspace=?1 AND operation_id=?2 LIMIT 3))")
            .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        if counts != (i64::from(self.message.is_some()), self.work.len() as i64) {
            return Err(OperationError::RecoveryRequired);
        }
        if let Some(message) = &self.message {
            let installed = message.report;
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM chat_messages WHERE workspace=? AND operation_id=? AND message_id=? AND thread_id=? AND plan_digest=? AND transaction_id=? AND protected=?")
                .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(installed.message_id().as_bytes().as_slice()).bind(message.thread.as_bytes().as_slice()).bind(installed.plan_digest().as_slice()).bind(installed.transaction_id().as_bytes().as_slice()).bind(&message.protected).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
            if count != 1 {
                return Err(OperationError::RecoveryRequired);
            }
        }
        for value in &self.work {
            let work = value.report;
            let evidence = work.evidence();
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM chat_publication_work WHERE workspace=? AND operation_id=? AND recipient=? AND installation_transaction=? AND capture_transaction=? AND event_id=? AND evidence_digest=? AND transaction_id=? AND protected=?")
                .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(evidence.recipient()?.as_bytes().as_slice()).bind(work.installation_transaction_id().as_bytes().as_slice()).bind(evidence.transaction_id().as_bytes().as_slice()).bind(evidence.event_id().as_bytes().as_slice()).bind(evidence.digest().as_slice()).bind(work.transaction_id().as_bytes().as_slice()).bind(&value.protected).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
            if count != 1 {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(())
    }
}
pub(super) async fn protect_message(
    bound: &BoundSigningPlan,
    report: ChatInstallation,
    crypto: &WorkspaceCrypto,
) -> Result<ProtectedMessage, OperationError> {
    let plaintext = Zeroizing::new(
        serde_json::to_vec(&MessageRecord::new(bound, report)?)
            .map_err(|_| OperationError::InvalidCommand)?,
    );
    let protected = crypto
        .seal_message(
            report.workspace_id(),
            report.operation_id(),
            report.plan_digest(),
            &plaintext,
        )
        .await?;
    Ok(ProtectedMessage {
        report,
        thread: bound.prepared().request().chat_command()?.thread_id(),
        protected,
    })
}
pub(super) async fn protect_work(
    bound: &BoundSigningPlan,
    installed: ChatInstallation,
    report: PublicationWork,
    crypto: &WorkspaceCrypto,
) -> Result<ProtectedWork, OperationError> {
    let plaintext = Zeroizing::new(
        serde_json::to_vec(&WorkRecord::new(installed, report))
            .map_err(|_| OperationError::InvalidCommand)?,
    );
    let protected = crypto
        .seal_issuance(
            scope(bound, report.evidence()),
            IssuancePayload::Publication,
            0,
            &plaintext,
        )
        .await?;
    Ok(ProtectedWork { report, protected })
}
pub(super) async fn insert_message(
    connection: &mut SqliteConnection,
    bound: &BoundSigningPlan,
    value: &ProtectedMessage,
) -> Result<(), OperationError> {
    let report = value.report;
    sqlx::query("INSERT INTO chat_messages(workspace,message_id,operation_id,thread_id,plan_digest,transaction_id,protected) VALUES(?,?,?,?,?,?,?)")
        .bind(report.workspace_id().as_bytes().as_slice()).bind(report.message_id().as_bytes().as_slice()).bind(report.operation_id().as_bytes().as_slice()).bind(bound.prepared().request().chat_command()?.thread_id().as_bytes().as_slice()).bind(report.plan_digest().as_slice()).bind(report.transaction_id().as_bytes().as_slice()).bind(&value.protected).execute(connection).await.map_err(write_error)?;
    Ok(())
}
pub(super) async fn insert_work(
    connection: &mut SqliteConnection,
    value: &ProtectedWork,
) -> Result<(), OperationError> {
    let work = value.report;
    let evidence = work.evidence();
    sqlx::query("INSERT INTO chat_publication_work(workspace,operation_id,recipient,installation_transaction,capture_transaction,event_id,evidence_digest,transaction_id,protected) VALUES(?,?,?,?,?,?,?,?,?)")
        .bind(evidence.workspace_id().as_bytes().as_slice()).bind(evidence.operation_id().as_bytes().as_slice()).bind(evidence.recipient()?.as_bytes().as_slice()).bind(work.installation_transaction_id().as_bytes().as_slice()).bind(evidence.transaction_id().as_bytes().as_slice()).bind(evidence.event_id().as_bytes().as_slice()).bind(evidence.digest().as_slice()).bind(work.transaction_id().as_bytes().as_slice()).bind(&value.protected).execute(connection).await.map_err(write_error)?;
    Ok(())
}
fn write_error(error: sqlx::Error) -> OperationError {
    if error
        .as_database_error()
        .is_some_and(|value| value.is_unique_violation())
    {
        OperationError::IdempotencyConflict
    } else {
        OperationError::StorageFailure
    }
}

impl Journal<'_> {
    pub(super) async fn install_chat(
        &mut self,
        request: InstallationRequest,
        proof: AttachmentProof,
    ) -> Result<InstallationOutcome, OperationError> {
        let (record, grant, current) = self.access(request.binding(), &proof).await?;
        let graph = RetainedGraph::read(
            self.connection,
            self.selected,
            request.binding(),
            request.operation_id(),
            Some(request.command_digest()),
            self.crypto,
        )
        .await?;
        if graph
            .bound_plan()
            .is_some_and(|bound| bound.plan().listing().is_some())
        {
            return self
                .install_listing(request, proof, record, grant, current, graph)
                .await;
        }
        let bound = graph.bound_plan().ok_or(OperationError::RecoveryRequired)?;
        if bound.plan().digest() != request.plan_digest() {
            return Err(OperationError::IdempotencyConflict);
        }
        let records = Records::read(
            self.connection,
            self.selected,
            request.operation_id(),
            &graph,
            self.crypto,
        )
        .await?;
        let missing: Vec<EvidenceReceipt> = graph
            .reports()
            .filter_map(|value| value.obtained())
            .filter(|evidence| {
                !records
                    .work
                    .iter()
                    .any(|value| value.report.evidence().target() == evidence.target())
            })
            .collect();
        let fresh = records.message.is_none() || !missing.is_empty();
        if records.message.is_none() && missing.is_empty() {
            return Err(OperationError::InvalidEvidence);
        }
        // Exact replay never replaces the installation, reruns allocation,
        // re-signs, consumes new entropy or prepares replacement ciphertext.
        let transaction_id = if fresh {
            TransactionId::from_bytes(super::random()?)
        } else {
            records
                .message
                .as_ref()
                .ok_or(OperationError::RecoveryRequired)?
                .report
                .transaction_id()
        };
        let installed = match &records.message {
            Some(value) => value.report,
            None => ChatInstallation::from_committed_store(
                bound,
                *missing.first().ok_or(OperationError::InvalidEvidence)?,
                transaction_id,
                current.observed_at(),
            )?,
        };
        let prepared_message = if records.message.is_none() {
            Some(protect_message(bound, installed, self.crypto).await?)
        } else {
            None
        };
        let mut prepared_work = Vec::with_capacity(missing.len());
        for evidence in missing {
            let work = PublicationWork::from_committed_store(
                bound,
                installed,
                evidence,
                transaction_id,
                current.observed_at(),
            )?;
            prepared_work.push(protect_work(bound, installed, work, self.crypto).await?);
        }
        let anchor = CommitAttempt::new(
            installed.workspace_id(),
            installed.operation_id(),
            installed.command_digest(),
            Some(transaction_id),
        );
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let changed = async {
            let now = recheck(
                &mut transaction,
                self.namespace,
                self.selected,
                &grant,
                &record,
                request.binding(),
                &proof,
                self.clock,
            )
            .await?;
            graph
                .unchanged(&mut transaction, self.selected, request.operation_id())
                .await?;
            records
                .unchanged(&mut transaction, self.selected, request.operation_id())
                .await?;
            if fresh && now.observed_at() < current.observed_at() {
                return Err(super::WorkspaceError::Unavailable.into());
            }
            // The original window constrains first issuance, not installation
            // of independently authenticated already obtained original bytes.
            if let Some(message) = &prepared_message {
                insert_message(&mut transaction, bound, message).await?;
            }
            for work in &prepared_work {
                insert_work(&mut transaction, work).await?;
            }
            Ok::<_, OperationError>(())
        }
        .await;
        if let Err(error) = changed {
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
        Ok(InstallationOutcome::Committed(Box::new(installed)))
    }
}

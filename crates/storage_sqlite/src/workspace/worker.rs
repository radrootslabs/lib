use super::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceClock, WorkspaceCrypto,
    WorkspaceError, admission::Journal, fs::Namespace, operation, record::GrantRecord,
};
use radroots_domain::{CommandDigest, OperationId};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, CommitAttempt, OperationError, PreparedOperation,
};
use sqlx::{
    Connection, Row, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
    time::Duration,
};
use tokio::sync::{oneshot, watch};

// Each request holds bounded descriptors and one 32-byte proof. With 32
// queued requests and a single in-flight request this is below the 4 MiB
// contract. Command data has a 64 KiB canonical bound, 4 KiB decoded text and
// bounded exact-sized descriptors; no unbounded event/SQL enters this queue.
const QUEUE_CAPACITY: usize = 32;
const TABLE: &str = "CREATE TABLE workspace_grant (singleton INTEGER PRIMARY KEY CHECK(singleton=1), workspace BLOB NOT NULL CHECK(length(workspace)=32), grant_id BLOB NOT NULL CHECK(length(grant_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT";
type Reply<T, E = WorkspaceError> = oneshot::Sender<Result<T, E>>;
enum Job {
    Authenticate(AttachmentInput, AttachmentProof, Reply<WorkspaceBinding>),
    Revoke(AttachmentInput, AttachmentProof, Reply<()>),
    Admit(
        Box<AdmissionRequest>,
        AttachmentProof,
        Reply<AdmissionOutcome, OperationError>,
    ),
    Operation(
        Box<WorkspaceBinding>,
        OperationId,
        Option<CommandDigest>,
        AttachmentProof,
        Reply<Option<PreparedOperation>, OperationError>,
    ),
}
pub(super) struct Handle {
    sender: Mutex<Option<SyncSender<Job>>>,
    completed: watch::Receiver<Option<Result<(), WorkspaceError>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
impl Handle {
    pub(super) async fn launch(
        path: PathBuf,
        input: AttachmentInput,
        initial: Option<GrantRecord>,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        let (sender, requests) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (ready, started) = oneshot::channel();
        let (completion, completed) = watch::channel(None);
        let thread = std::thread::Builder::new()
            .name("radroots_workspace".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(_) => {
                        let _ = ready.send(Err(WorkspaceError::Unavailable));
                        let _ = completion.send(Some(Err(WorkspaceError::Unavailable)));
                        return;
                    }
                };
                // Construction, every request and explicit close complete on this
                // owner even when an async observer disappears between await points.
                let namespace = match Namespace::acquire(&path, initial.is_some()) {
                    Ok(namespace) => namespace,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        let _ = completion.send(Some(Err(error)));
                        return;
                    }
                };
                let result = runtime.block_on(async {
                    if initial.is_none() {
                        let preflight = preflight_existing(&namespace, &input, &crypto).await;
                        if let Err(error) = preflight {
                            let _ = ready.send(Err(error));
                            return Err(error);
                        }
                    }
                    let options = SqliteConnectOptions::new()
                        .filename(namespace.database_path())
                        .create_if_missing(false)
                        .journal_mode(SqliteJournalMode::Wal)
                        .synchronous(SqliteSynchronous::Full)
                        .foreign_keys(true)
                        .busy_timeout(Duration::from_secs(5));
                    let mut connection = match SqliteConnection::connect_with(&options).await {
                        Ok(connection) => connection,
                        Err(_) => {
                            let _ = ready.send(Err(WorkspaceError::StorageFailure));
                            return Err(WorkspaceError::StorageFailure);
                        }
                    };
                    let initialized =
                        initialize(&mut connection, &namespace, &input, initial, &crypto).await;
                    if let Err(error) = initialized {
                        let _ = ready.send(Err(error));
                        // Do not release namespace authority at the first failure;
                        // first await the actual connection's close completion.
                        let _ = connection.close().await;
                        return Err(error);
                    }
                    let mut uncertain = false;
                    if ready.send(Ok(())).is_ok() {
                        while let Ok(job) = requests.recv() {
                            match job {
                                Job::Authenticate(selected, proof, reply) => {
                                    let result = authenticate(
                                        &mut connection,
                                        &namespace,
                                        &input,
                                        &crypto,
                                        clock.as_ref(),
                                        &selected,
                                        &proof,
                                    )
                                    .await
                                    .map(|(_, binding)| binding);
                                    let _ = reply.send(result);
                                }
                                Job::Revoke(selected, proof, reply) => {
                                    let result = revoke(
                                        &mut connection,
                                        &namespace,
                                        &input,
                                        &crypto,
                                        clock.as_ref(),
                                        &selected,
                                        &proof,
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Admit(request, proof, reply) => {
                                    let result = Journal {
                                        connection: &mut connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .admit(*request, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Operation(binding, operation_id, digest, proof, reply) => {
                                    let result = Journal {
                                        connection: &mut connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .query(*binding, operation_id, digest, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                            }
                        }
                    }
                    // The admission sender is gone. All admitted work has drained.
                    let checkpoint = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                        .fetch_one(&mut connection)
                        .await;
                    let checked = match checkpoint {
                        Ok(row) if row.try_get::<i64, _>(0).ok() == Some(0) => Ok(()),
                        _ => Err(WorkspaceError::StorageFailure),
                    };
                    let closed = connection
                        .close()
                        .await
                        .map_err(|_| WorkspaceError::StorageFailure);
                    checked.and(closed).and_then(|()| namespace.validate())
                });
                drop(namespace);
                let _ = completion.send(Some(result));
            })
            .map_err(|_| WorkspaceError::Unavailable)?;
        let handle = Self {
            sender: Mutex::new(Some(sender)),
            completed,
            thread: Mutex::new(Some(thread)),
        };
        match started.await {
            Ok(Ok(())) => Ok(handle),
            Ok(Err(error)) => {
                let _ = handle.close().await;
                Err(error)
            }
            Err(_) => {
                let _ = handle.close().await;
                Err(WorkspaceError::Unavailable)
            }
        }
    }
    fn send(&self, job: Job) -> Result<(), WorkspaceError> {
        self.sender
            .lock()
            .map_err(|_| WorkspaceError::Unavailable)?
            .as_ref()
            .ok_or(WorkspaceError::Closed)?
            .try_send(job)
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => WorkspaceError::Unavailable,
                mpsc::TrySendError::Disconnected(_) => WorkspaceError::Closed,
            })
    }
    pub(super) async fn authenticate(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> Result<WorkspaceBinding, WorkspaceError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::Authenticate(input, proof, reply))?;
        result.await.map_err(|_| WorkspaceError::Unavailable)?
    }
    pub(super) async fn revoke(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), WorkspaceError> {
        let proof = proof.with_secret(|secret| AttachmentProof::from_protected_input(secret))?;
        let (reply, result) = oneshot::channel();
        self.send(Job::Revoke(input, proof, reply))?;
        result.await.map_err(|_| WorkspaceError::CommitUnknown)?
    }
    pub(super) async fn admit(
        &self,
        request: AdmissionRequest,
        proof: AttachmentProof,
    ) -> Result<AdmissionOutcome, OperationError> {
        let context = request.command().context();
        let anchor = CommitAttempt::new(
            context.workspace_id(),
            context.operation_id(),
            request.command().digest(),
            None,
        );
        let (reply, result) = oneshot::channel();
        self.send(Job::Admit(Box::new(request), proof, reply))?;
        match result.await {
            Ok(result) => result,
            Err(_) => Ok(AdmissionOutcome::Unknown(anchor)),
        }
    }
    pub(super) async fn operation(
        &self,
        binding: WorkspaceBinding,
        operation_id: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> Result<Option<PreparedOperation>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::Operation(
            Box::new(binding),
            operation_id,
            digest,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn close(&self) -> Result<(), WorkspaceError> {
        self.sender
            .lock()
            .map_err(|_| WorkspaceError::Unavailable)?
            .take();
        let mut completed = self.completed.clone();
        let result = loop {
            if let Some(result) = *completed.borrow_and_update() {
                break result;
            }
            completed
                .changed()
                .await
                .map_err(|_| WorkspaceError::Unavailable)?;
        };
        if let Some(thread) = self
            .thread
            .lock()
            .map_err(|_| WorkspaceError::Unavailable)?
            .take()
        {
            thread.join().map_err(|_| WorkspaceError::Unavailable)?;
        }
        result
    }
}

async fn preflight_existing(
    namespace: &Namespace,
    expected: &AttachmentInput,
    crypto: &WorkspaceCrypto,
) -> Result<(), WorkspaceError> {
    // Inspect through SQLite read-only before any persistent journal setting or
    // writable connection. Do not turn a foreign schema into a workspace.
    let options = SqliteConnectOptions::new()
        .filename(namespace.database_path())
        .read_only(true)
        .create_if_missing(false)
        .pragma("query_only", "ON")
        .busy_timeout(Duration::from_secs(5));
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let inspected = initialize(&mut connection, namespace, expected, None, crypto).await;
    let closed = connection
        .close()
        .await
        .map_err(|_| WorkspaceError::StorageFailure);
    inspected.and(closed)
}

async fn initialize(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    expected: &AttachmentInput,
    initial: Option<GrantRecord>,
    crypto: &WorkspaceCrypto,
) -> Result<(), WorkspaceError> {
    namespace.validate()?;
    if let Some(initial) = initial {
        let plaintext = initial.encode()?;
        let protected = crypto
            .seal(expected.workspace_id(), expected.grant_id(), &plaintext)
            .await?;
        let mut transaction = connection
            .begin()
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        sqlx::raw_sql(TABLE)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        sqlx::raw_sql(operation::TABLE)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        sqlx::raw_sql(operation::INDEX)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        sqlx::query("PRAGMA user_version=2")
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        sqlx::query(
            "INSERT INTO workspace_grant(singleton,workspace,grant_id,protected) VALUES(1,?,?,?)",
        )
        .bind(expected.workspace_id().as_bytes().as_slice())
        .bind(expected.grant_id().as_bytes().as_slice())
        .bind(protected)
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkspaceError::StorageFailure)?;
        transaction
            .commit()
            .await
            .map_err(|_| WorkspaceError::CommitUnknown)?;
    }
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if version != 2 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    let catalog_matches: i64 = sqlx::query_scalar("SELECT CASE WHEN count(*)=3 AND sum(CASE WHEN type='table' AND name='workspace_grant' AND tbl_name='workspace_grant' AND sql=? THEN 1 WHEN type='table' AND name='operations' AND tbl_name='operations' AND sql=? THEN 1 WHEN type='index' AND name='operation_transaction_identity' AND tbl_name='operations' AND sql=? THEN 1 ELSE 0 END)=3 THEN 1 ELSE 0 END FROM sqlite_schema")
        .bind(TABLE).bind(operation::TABLE).bind(operation::INDEX).fetch_one(&mut *connection).await.map_err(|_| WorkspaceError::RecoveryRequired)?;
    if catalog_matches != 1 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    let integrity: String = sqlx::query_scalar("PRAGMA quick_check(1)")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if integrity != "ok" {
        return Err(WorkspaceError::RecoveryRequired);
    }
    let protected = read(connection, expected).await?;
    let plaintext = crypto
        .open(expected.workspace_id(), expected.grant_id(), &protected)
        .await?;
    plaintext.expose_secret(GrantRecord::decode)?;
    namespace.validate()
}
pub(super) async fn read(
    connection: &mut SqliteConnection,
    expected: &AttachmentInput,
) -> Result<Vec<u8>, WorkspaceError> {
    // Refuse oversized input in SQL before materializing the protected blob.
    let row = sqlx::query("SELECT CASE WHEN length(workspace)=32 THEN workspace END,CASE WHEN length(grant_id)=32 THEN grant_id END,length(protected),CASE WHEN length(protected)<=262144 THEN protected END FROM workspace_grant WHERE singleton=1")
        .fetch_optional(connection).await.map_err(|_| WorkspaceError::RecoveryRequired)?.ok_or(WorkspaceError::RecoveryRequired)?;
    let workspace: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(0)
        .map_err(|_| WorkspaceError::RecoveryRequired)?
        .ok_or(WorkspaceError::RecoveryRequired)?;
    let grant: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(1)
        .map_err(|_| WorkspaceError::RecoveryRequired)?
        .ok_or(WorkspaceError::RecoveryRequired)?;
    if workspace != expected.workspace_id().as_bytes() || grant != expected.grant_id().as_bytes() {
        return Err(WorkspaceError::WrongNamespace);
    }
    let length: i64 = row
        .try_get(2)
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if length <= 0 || length > 262144 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    row.try_get(3).map_err(|_| WorkspaceError::RecoveryRequired)
}
async fn authenticate(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    expected: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    clock: &dyn WorkspaceClock,
    selected: &AttachmentInput,
    proof: &AttachmentProof,
) -> Result<(GrantRecord, WorkspaceBinding), WorkspaceError> {
    namespace.validate()?;
    let protected = read(connection, expected).await?;
    let plaintext = crypto
        .open(expected.workspace_id(), expected.grant_id(), &protected)
        .await?;
    let record = plaintext.expose_secret(GrantRecord::decode)?;
    let binding = record.authenticate(selected, proof, clock.now()?)?;
    namespace.validate()?;
    Ok((record, binding))
}
async fn revoke(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    expected: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    clock: &dyn WorkspaceClock,
    selected: &AttachmentInput,
    proof: &AttachmentProof,
) -> Result<(), WorkspaceError> {
    let (mut record, _) = authenticate(
        connection, namespace, expected, crypto, clock, selected, proof,
    )
    .await?;
    record.revoke()?;
    let plaintext = record.encode()?;
    let protected = crypto
        .seal(expected.workspace_id(), expected.grant_id(), &plaintext)
        .await?;
    namespace.validate()?;
    let mut transaction = connection
        .begin()
        .await
        .map_err(|_| WorkspaceError::StorageFailure)?;
    let changed = sqlx::query(
        "UPDATE workspace_grant SET protected=? WHERE singleton=1 AND workspace=? AND grant_id=?",
    )
    .bind(protected)
    .bind(expected.workspace_id().as_bytes().as_slice())
    .bind(expected.grant_id().as_bytes().as_slice())
    .execute(&mut *transaction)
    .await
    .map_err(|_| WorkspaceError::StorageFailure)?;
    if changed.rows_affected() != 1 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    transaction
        .commit()
        .await
        .map_err(|_| WorkspaceError::CommitUnknown)
}

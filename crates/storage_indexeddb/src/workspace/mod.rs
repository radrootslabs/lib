//! Authenticated browser local outboxes, independent of participant signing.
//! Creation establishes a fresh unrelated buyer outbox only. Existing state
//! requires its independently retained selection and protected proof.
mod address;
mod crypto;
mod installation;
mod issuance;
mod journal;
mod lease;
mod plans;
mod retained;

use super::{Batch, Database, Error, Mutation, Namespace, OpenMode, RecordKey};
use futures::{channel::oneshot, lock::Mutex};
use radroots_domain::{ExecutorId, GrantId, UnixSeconds, WorkspaceId};
use radroots_secrets::{
    SecretRef,
    provider::{AccessPolicy, ResidencySupport, SecretProvider, SelectionPolicy},
};
pub use radroots_storage::workspace::local_outbox::LocalOutboxConfig;
use radroots_storage::{
    event::SourceGeneration,
    workspace::{
        AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceClock, WorkspaceError,
        WorkspaceStore, local_outbox::LocalOutboxGrant,
    },
};
use radroots_transport::BoxFuture;
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    fmt,
    future::{Future, poll_fn},
    rc::{Rc, Weak},
    task::{Poll, Waker},
};
use wasm_bindgen_futures::spawn_local;

const JOB_BYTES: usize = 4 * 1024 * 1024;
const LIVE_BYTES: usize = 4 * 1024 * 1024;
const JOBS: usize = LIVE_BYTES / JOB_BYTES;
const SETUPS_MAX: usize = 8;
const WAITERS: usize = 32;
type Signal = RefCell<Option<Waker>>;
thread_local! { static SETUPS: Cell<usize> = const { Cell::new(0) }; }
struct Setup;
impl Setup {
    fn admit() -> Result<Self, WorkspaceError> {
        SETUPS.with(|count| {
            if count.get() >= SETUPS_MAX {
                return Err(WorkspaceError::Capacity);
            }
            count.set(count.get() + 1);
            Ok(Self)
        })
    }
}
impl Drop for Setup {
    fn drop(&mut self) {
        SETUPS.with(|count| count.set(count.get() - 1));
    }
}
fn stored(error: Error) -> WorkspaceError {
    match error {
        Error::Closed => WorkspaceError::Closed,
        Error::Capacity => WorkspaceError::Capacity,
        Error::CommitUnknown => WorkspaceError::CommitUnknown,
        Error::MissingNamespace | Error::NamespaceExists => WorkspaceError::NamespaceUnavailable,
        _ => WorkspaceError::RecoveryRequired,
    }
}
fn namespace(input: &AttachmentInput) -> Result<Namespace, WorkspaceError> {
    let mut digest = Sha256::new();
    digest.update(b"radroots.browser.local_outbox.namespace.v2\0");
    digest.update(input.workspace_id().as_bytes());
    SourceGeneration::new(digest.finalize().into())
        .map(Namespace::new)
        .map_err(|_| WorkspaceError::InvalidInput)
}
fn grant_key() -> RecordKey {
    RecordKey::new(Sha256::digest(b"radroots.browser.local_outbox.grant.v1\0").into())
}
async fn commit_current(
    db: &Database,
    batch: Batch,
    clock: Rc<dyn WorkspaceClock>,
    observed: UnixSeconds,
    expires: UnixSeconds,
    active: Rc<Cell<bool>>,
) -> Result<(), WorkspaceError> {
    let failure = Rc::new(Cell::new(None));
    let guard_failure = failure.clone();
    let result = db
        .compare_exchange_checked(batch, move || {
            let eligible = (|| {
                if !active.get() {
                    return Err(WorkspaceError::Closed);
                }
                let now = clock.now()?;
                if now.get() == 0 || now < observed {
                    return Err(WorkspaceError::Unavailable);
                }
                if now >= expires {
                    return Err(WorkspaceError::GrantExpired);
                }
                Ok(())
            })();
            if let Err(error) = eligible {
                guard_failure.set(Some(error));
                false
            } else {
                true
            }
        })
        .await;
    // Only a confirmed abort makes the callback refusal a known failure.
    // A host that completed despite refusal must retain CommitUnknown.
    if matches!(result, Err(Error::GuardRefused)) {
        return Err(failure.get().unwrap_or(WorkspaceError::RecoveryRequired));
    }
    result.map(|_| ()).map_err(stored)
}
fn select(
    provider: &dyn SecretProvider,
    reference: &SecretRef,
    policy: AccessPolicy,
) -> Result<(), WorkspaceError> {
    if provider.capabilities().residency() == ResidencySupport::Volatile {
        return Err(WorkspaceError::Unavailable);
    }
    SelectionPolicy::new(reference.backend(), policy)
        .select(&[provider])
        .map_err(|_| WorkspaceError::Unavailable)?;
    Ok(())
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum AccessMode {
    Current,
    CaptureOnly,
}
struct State {
    // Database drops before the lease if an unobserved owner is discarded.
    db: Database,
    provider: Rc<dyn SecretProvider>,
    reference: SecretRef,
    selected: AttachmentInput,
    clock: Rc<dyn WorkspaceClock>,
    mode: AccessMode,
    lease: RefCell<Option<lease::Lease>>,
    serial: Mutex<()>,
    jobs: Cell<usize>,
    closing: Cell<bool>,
    closed: Cell<Option<Result<(), WorkspaceError>>>,
    drain: RefCell<Option<Waker>>,
    waiters: RefCell<Vec<Weak<Signal>>>,
}
impl State {
    fn active(&self) -> Result<Rc<Cell<bool>>, WorkspaceError> {
        let active = self
            .lease
            .borrow()
            .as_ref()
            .map(lease::Lease::active)
            .ok_or(WorkspaceError::Closed)?;
        if !active.get() {
            return Err(WorkspaceError::Closed);
        }
        Ok(active)
    }
    fn admit(self: &Rc<Self>) -> Result<Work, WorkspaceError> {
        if self.closing.get() {
            return Err(WorkspaceError::Closed);
        }
        if self.active().is_err() {
            self.stop();
            return Err(WorkspaceError::Closed);
        }
        if self.jobs.get() >= JOBS {
            return Err(WorkspaceError::Capacity);
        }
        self.jobs.set(self.jobs.get() + 1);
        Ok(Work(self.clone()))
    }
    fn stop(self: &Rc<Self>) {
        if self.closing.replace(true) {
            return;
        }
        let state = self.clone();
        spawn_local(async move {
            poll_fn(|cx| {
                if state.jobs.get() == 0 {
                    Poll::Ready(())
                } else {
                    *state.drain.borrow_mut() = Some(cx.waker().clone());
                    Poll::Pending
                }
            })
            .await;
            let database = state.db.close().await.map_err(stored);
            let lease = state.lease.borrow_mut().take();
            let released = match lease {
                Some(lease) => lease.release().await,
                None => Err(WorkspaceError::RecoveryRequired),
            };
            state.closed.set(Some(database.and(released)));
            state.waiters.borrow_mut().retain(|weak| {
                if let Some(signal) = weak.upgrade() {
                    if let Some(waker) = signal.borrow_mut().take() {
                        waker.wake();
                    }
                    true
                } else {
                    false
                }
            });
        });
    }
    async fn access(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(LocalOutboxGrant, Vec<u8>, WorkspaceBinding), WorkspaceError> {
        if self.mode != AccessMode::Current {
            return Err(WorkspaceError::Unauthorized);
        }
        let (record, protected) = self.capture_grant(input, proof).await?;
        let binding = record.authenticate(input, proof, self.clock.now()?)?;
        self.active()?;
        Ok((record, protected, binding))
    }
    async fn capture_grant(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(LocalOutboxGrant, Vec<u8>), WorkspaceError> {
        self.active()?;
        if input != &self.selected {
            return Err(WorkspaceError::Unauthorized);
        }
        let protected = self
            .db
            .get(grant_key())
            .await
            .map_err(stored)?
            .ok_or(WorkspaceError::RecoveryRequired)?;
        let plaintext = crypto::open(
            self.provider.as_ref(),
            &self.reference,
            self.selected.workspace_id(),
            self.selected.grant_id(),
            &protected,
        )
        .await?;
        let record = plaintext.expose_secret(LocalOutboxGrant::decode)?;
        record.authenticate_retained_capture(input, proof)?;
        self.active()?;
        Ok((record, protected))
    }
}
struct Work(Rc<State>);
impl Drop for Work {
    fn drop(&mut self) {
        self.0.jobs.set(self.0.jobs.get() - 1);
        if self.0.jobs.get() == 0
            && let Some(waker) = self.0.drain.borrow_mut().take()
        {
            waker.wake();
        }
    }
}
trait JobError: From<WorkspaceError> {
    fn workspace_failure(&self) -> Option<WorkspaceError>;
}
impl JobError for WorkspaceError {
    fn workspace_failure(&self) -> Option<WorkspaceError> {
        Some(*self)
    }
}
impl JobError for radroots_storage::command::OperationError {
    fn workspace_failure(&self) -> Option<WorkspaceError> {
        match self {
            Self::Workspace(error) => Some(*error),
            Self::RecoveryRequired => Some(WorkspaceError::RecoveryRequired),
            _ => None,
        }
    }
}
async fn owned<T: 'static, E: JobError + 'static>(
    work: Work,
    future: impl Future<Output = Result<T, E>> + 'static,
) -> Result<T, E> {
    let (send, observe) = oneshot::channel();
    spawn_local(async move {
        let state = work.0.clone();
        let _work = work;
        let _serial = state.serial.lock().await;
        let result = future.await;
        if matches!(
            result.as_ref().err().and_then(JobError::workspace_failure),
            Some(
                WorkspaceError::Closed
                    | WorkspaceError::Unavailable
                    | WorkspaceError::RecoveryRequired
                    | WorkspaceError::CommitUnknown
            )
        ) {
            state.stop();
        }
        let _ = send.send(result);
    });
    observe
        .await
        .unwrap_or_else(|_| Err(WorkspaceError::CommitUnknown.into()))
}
/// One bounded browser writer. Clones share admission, serial work and actual
/// close completion. No proof or participant signer is retained in the handle.
#[derive(Clone)]
pub struct BrowserWorkspace {
    state: Rc<State>,
}
impl BrowserWorkspace {
    /// Enrolls only a new unrelated buyer outbox and returns its independently
    /// retained selection/proof after the protected grant actually commits.
    pub async fn initialize_local_outbox(
        config: LocalOutboxConfig,
        provider: Rc<dyn SecretProvider>,
        reference: SecretRef,
        policy: AccessPolicy,
        clock: Rc<dyn WorkspaceClock>,
    ) -> Result<(Self, AttachmentInput, AttachmentProof), WorkspaceError> {
        select(provider.as_ref(), &reference, policy)?;
        let setup = Setup::admit()?;
        let now = clock.now()?;
        config.validate_at(now)?;
        let workspace = WorkspaceId::from_bytes(*crypto::random::<32>()?);
        let executor = ExecutorId::from_bytes(*crypto::random::<32>()?);
        let grant = GrantId::from_bytes(*crypto::random::<32>()?);
        let secret = crypto::random::<32>()?;
        let proof = AttachmentProof::from_protected_input(secret.as_ref())?;
        let (input, proof, record) = config.enroll(now, workspace, executor, grant, proof)?;
        let (send, observe) = oneshot::channel();
        spawn_local(async move {
            let _setup = setup;
            let result = async {
                let selected = namespace(&input)?;
                let lease = lease::Lease::acquire(&selected.name()).await?;
                let db = match Database::open(selected, OpenMode::CreateNew).await {
                    Ok(db) => db,
                    Err(error) => {
                        lease.release().await?;
                        return Err(stored(error));
                    }
                };
                let owner = Self::assemble(
                    db,
                    provider,
                    reference,
                    input.clone(),
                    clock,
                    lease,
                    AccessMode::Current,
                );
                let initialized = async {
                    let bytes = record.encode()?;
                    let binding = record.authenticate(&input, &proof, now)?;
                    let protected = crypto::seal(
                        owner.state.provider.as_ref(),
                        &owner.state.reference,
                        workspace,
                        grant,
                        &bytes,
                    )
                    .await?;
                    let batch = Batch::new(vec![
                        Mutation::new(grant_key(), None, Some(protected)).map_err(stored)?,
                    ])
                    .map_err(stored)?;
                    commit_current(
                        &owner.state.db,
                        batch,
                        owner.state.clock.clone(),
                        binding.observed_at(),
                        binding.expires_at(),
                        owner.state.active()?,
                    )
                    .await?;
                    record.authenticate(&input, &proof, owner.state.clock.now()?)?;
                    Ok::<(), WorkspaceError>(())
                }
                .await;
                if let Err(error) = initialized {
                    owner.close().await?;
                    return Err(error);
                }
                Ok((owner, input, proof))
            }
            .await;
            let _ = send.send(result);
        });
        observe.await.unwrap_or(Err(WorkspaceError::CommitUnknown))
    }
    /// Opens an exact existing outbox and authenticates its retained protected
    /// proof. Missing records never become a newly enrolled grant.
    pub async fn open_existing(
        input: AttachmentInput,
        proof: AttachmentProof,
        provider: Rc<dyn SecretProvider>,
        reference: SecretRef,
        policy: AccessPolicy,
        clock: Rc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        Self::open_mode(
            input,
            proof,
            provider,
            reference,
            policy,
            clock,
            AccessMode::Current,
        )
        .await
    }
    /// Opens only for appending matching already obtained output to an actual
    /// original claim. This handle cannot authenticate or admit fresh work.
    pub async fn open_existing_for_capture(
        input: AttachmentInput,
        proof: AttachmentProof,
        provider: Rc<dyn SecretProvider>,
        reference: SecretRef,
        policy: AccessPolicy,
        clock: Rc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        Self::open_mode(
            input,
            proof,
            provider,
            reference,
            policy,
            clock,
            AccessMode::CaptureOnly,
        )
        .await
    }
    async fn open_mode(
        input: AttachmentInput,
        proof: AttachmentProof,
        provider: Rc<dyn SecretProvider>,
        reference: SecretRef,
        policy: AccessPolicy,
        clock: Rc<dyn WorkspaceClock>,
        mode: AccessMode,
    ) -> Result<Self, WorkspaceError> {
        select(provider.as_ref(), &reference, policy)?;
        let setup = Setup::admit()?;
        let (send, observe) = oneshot::channel();
        spawn_local(async move {
            let _setup = setup;
            let result = async {
                let selected = namespace(&input)?;
                let lease = lease::Lease::acquire(&selected.name()).await?;
                let db = match Database::open(selected, OpenMode::Existing).await {
                    Ok(db) => db,
                    Err(error) => {
                        lease.release().await?;
                        return Err(stored(error));
                    }
                };
                let owner =
                    Self::assemble(db, provider, reference, input.clone(), clock, lease, mode);
                let authenticated = match mode {
                    AccessMode::Current => owner.state.access(&input, &proof).await.map(|_| ()),
                    AccessMode::CaptureOnly => {
                        owner.state.capture_grant(&input, &proof).await.map(|_| ())
                    }
                };
                if let Err(error) = authenticated {
                    owner.close().await?;
                    return Err(error);
                }
                Ok(owner)
            }
            .await;
            let _ = send.send(result);
        });
        observe.await.unwrap_or(Err(WorkspaceError::CommitUnknown))
    }
    fn assemble(
        db: Database,
        provider: Rc<dyn SecretProvider>,
        reference: SecretRef,
        selected: AttachmentInput,
        clock: Rc<dyn WorkspaceClock>,
        lease: lease::Lease,
        mode: AccessMode,
    ) -> Self {
        Self {
            state: Rc::new(State {
                db,
                provider,
                reference,
                selected,
                clock,
                mode,
                lease: RefCell::new(Some(lease)),
                serial: Mutex::new(()),
                jobs: Cell::new(0),
                closing: Cell::new(false),
                closed: Cell::new(None),
                drain: RefCell::new(None),
                waiters: RefCell::new(Vec::new()),
            }),
        }
    }
    /// Authenticates and atomically revokes the current protected grant.
    pub async fn revoke(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> Result<(), WorkspaceError> {
        let work = self.state.admit()?;
        let state = self.state.clone();
        owned(work, async move {
            let (mut record, expected, current) = state.access(&input, &proof).await?;
            record.revoke()?;
            let bytes = record.encode()?;
            let protected = crypto::seal(
                state.provider.as_ref(),
                &state.reference,
                input.workspace_id(),
                input.grant_id(),
                &bytes,
            )
            .await?;
            // A foreign writer cannot substitute the grant across async crypto.
            let batch = Batch::new(vec![
                Mutation::new(grant_key(), Some(expected), Some(protected)).map_err(stored)?,
            ])
            .map_err(stored)?;
            commit_current(
                &state.db,
                batch,
                state.clock.clone(),
                current.observed_at(),
                current.expires_at(),
                state.active()?,
            )
            .await?;
            Ok(())
        })
        .await
    }
    /// Stops admission and observes drained database closure and Web Lock release.
    /// Dropping this observation does not cancel the Rust-owned close operation.
    pub async fn close(&self) -> Result<(), WorkspaceError> {
        self.state.stop();
        if let Some(result) = self.state.closed.get() {
            return result;
        }
        let signal = Rc::new(RefCell::new(None));
        {
            let mut waiters = self.state.waiters.borrow_mut();
            waiters.retain(|weak| weak.strong_count() > 0);
            if waiters.len() >= WAITERS {
                return Err(WorkspaceError::Capacity);
            }
            waiters.push(Rc::downgrade(&signal));
        }
        poll_fn(|cx| {
            if let Some(result) = self.state.closed.get() {
                Poll::Ready(result)
            } else {
                *signal.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
}
impl fmt::Debug for BrowserWorkspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BrowserWorkspace(<redacted>)")
    }
}
impl WorkspaceStore for BrowserWorkspace {
    fn authenticate(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<WorkspaceBinding, WorkspaceError>> {
        Box::pin(async move {
            let work = self.state.admit()?;
            let state = self.state.clone();
            owned(work, async move {
                state
                    .access(&input, &proof)
                    .await
                    .map(|(_, _, binding)| binding)
            })
            .await
        })
    }
    fn close_workspace(&self) -> BoxFuture<'_, Result<(), WorkspaceError>> {
        Box::pin(self.close())
    }
}

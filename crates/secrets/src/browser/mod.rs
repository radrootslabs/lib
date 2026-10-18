//! Explicit browser-origin data-key wrapping with pinned nonextractable keys.
//! No participant signer, hardware/presence claim, fallback or stock authority.
use crate::{
    Error, KeyWrapping, SecretId, SecretProvider, SecretRef,
    error::Operation,
    id::BackendKind,
    provider::{CapabilitySupport, ResidencySupport, SecretCapabilities},
    wrapping::{BoxFuture, SecretMaterial, UnwrapRequest, WrapRequest, WrappedSecret},
};
use futures::channel::oneshot;
use js_sys::Reflect;
use std::{
    cell::{Cell, RefCell},
    fmt,
    future::{Future, poll_fn},
    rc::{Rc, Weak},
    task::{Poll, Waker},
};
use subtle::ConstantTimeEq;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::spawn_local;
use web_sys::{CryptoKey, Event, IdbDatabase, IdbFactory, SubtleCrypto};

mod crypto;
mod idb;

const JOBS_MAX: usize = 32;
const LIVE_BYTES_MAX: usize = 4 * 1024 * 1024;
type Callback = Closure<dyn FnMut(Event)>;
type CloseSignal = RefCell<Option<Waker>>;

// Setup has no provider instance yet. Keep its accepted jobs bounded per host
// thread, independently of whether the caller retains the observation.
thread_local! {
    static SETUP_JOBS: Cell<usize> = const { Cell::new(0) };
}
struct Setup;
impl Setup {
    fn admit() -> Result<Self, BrowserError> {
        SETUP_JOBS.with(|jobs| {
            if jobs.get() >= JOBS_MAX {
                return Err(BrowserError::Capacity);
            }
            jobs.set(jobs.get() + 1);
            Ok(Self)
        })
    }
}
impl Drop for Setup {
    fn drop(&mut self) {
        SETUP_JOBS.with(|jobs| jobs.set(jobs.get() - 1));
    }
}

/// Fixed source-redacted browser setup/storage outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrowserError {
    Unavailable,
    MissingNamespace,
    NamespaceExists,
    IncompatibleSchema,
    MissingKey,
    KeyMismatch,
    Closed,
    Capacity,
    Aborted,
    CommitUnknown,
    Provider(Error),
}
impl BrowserError {
    /// Returns a stable code without native exception messages or identifiers.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "browser_provider_unavailable",
            Self::MissingNamespace => "missing_namespace",
            Self::NamespaceExists => "namespace_exists",
            Self::IncompatibleSchema => "incompatible_schema",
            Self::MissingKey => "missing_key",
            Self::KeyMismatch => "provider_key_mismatch",
            Self::Closed => "closed",
            Self::Capacity => "capacity_exhausted",
            Self::Aborted => "transaction_aborted",
            Self::CommitUnknown => "commit_unknown",
            Self::Provider(_) => "provider_failure",
        }
    }
}
impl fmt::Display for BrowserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for BrowserError {}

struct State {
    db: IdbDatabase,
    subtle: SubtleCrypto,
    namespace: SecretId,
    reference: SecretRef,
    fingerprint: [u8; 32],
    closing: Cell<bool>,
    jobs: Cell<usize>,
    bytes: Cell<usize>,
    waiters: RefCell<Vec<Weak<CloseSignal>>>,
    version_change: RefCell<Option<Callback>>,
}
impl State {
    fn wake(&self) {
        self.waiters.borrow_mut().retain(|weak| {
            if let Some(signal) = weak.upgrade() {
                if let Some(waker) = signal.borrow_mut().take() {
                    waker.wake();
                }
                true
            } else {
                false
            }
        });
    }
    fn stop(&self) {
        self.closing.set(true);
        if self.jobs.get() == 0 {
            self.db.close();
        }
        self.wake();
    }
    fn admit(self: &Rc<Self>, bytes: usize) -> Result<Work, BrowserError> {
        if self.closing.get() {
            return Err(BrowserError::Closed);
        }
        let retained = self
            .bytes
            .get()
            .checked_add(bytes)
            .ok_or(BrowserError::Capacity)?;
        if self.jobs.get() >= JOBS_MAX || retained > LIVE_BYTES_MAX {
            return Err(BrowserError::Capacity);
        }
        self.jobs.set(self.jobs.get() + 1);
        self.bytes.set(retained);
        Ok(Work {
            state: self.clone(),
            bytes,
        })
    }
    async fn key(&self) -> Result<CryptoKey, BrowserError> {
        let (key, binding) = idb::load(&self.db).await?;
        let expected = binding_bytes(&self.reference, &self.fingerprint);
        if !bool::from(binding.as_slice().ct_eq(expected.as_slice())) {
            return Err(BrowserError::KeyMismatch);
        }
        let actual =
            crypto::fingerprint(&self.subtle, &key, &self.namespace, &self.reference).await?;
        if !bool::from(actual.ct_eq(&self.fingerprint)) {
            return Err(BrowserError::KeyMismatch);
        }
        Ok(key)
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.db.set_onversionchange(None);
        self.db.set_onclose(None);
        self.db.close();
    }
}
struct Work {
    state: Rc<State>,
    bytes: usize,
}
impl Drop for Work {
    fn drop(&mut self) {
        self.state.jobs.set(self.state.jobs.get() - 1);
        self.state.bytes.set(self.state.bytes.get() - self.bytes);
        if self.state.closing.get() && self.state.jobs.get() == 0 {
            self.state.db.close();
        }
        self.state.wake();
    }
}
async fn owned<T: 'static>(
    work: Work,
    future: impl Future<Output = Result<T, BrowserError>> + 'static,
) -> Result<T, BrowserError> {
    let (send, observe) = oneshot::channel();
    spawn_local(async move {
        let _work = work;
        let _ = send.send(future.await);
    });
    observe.await.unwrap_or(Err(BrowserError::CommitUnknown))
}
fn factory() -> Result<IdbFactory, BrowserError> {
    Reflect::get(&js_sys::global(), &JsValue::from_str("indexedDB"))
        .map_err(|_| BrowserError::Unavailable)?
        .dyn_into::<IdbFactory>()
        .map_err(|_| BrowserError::Unavailable)
}
fn name(namespace: &SecretId) -> String {
    format!("radroots.secrets.browser.v1.{}", namespace.as_str())
}
fn binding_bytes(reference: &SecretRef, fingerprint: &[u8; 32]) -> Vec<u8> {
    let mut bytes = Vec::from(b"RRSB\x01".as_slice());
    bytes.push(reference.id().as_str().len() as u8);
    bytes.extend_from_slice(reference.id().as_str().as_bytes());
    bytes.extend_from_slice(&reference.key_version().get().to_be_bytes());
    bytes.extend_from_slice(fingerprint);
    bytes
}
fn selected(reference: &SecretRef) -> Result<(), BrowserError> {
    if reference.backend() != BackendKind::External {
        return Err(BrowserError::Provider(Error::BackendMismatch {
            provider: BackendKind::External,
            reference: reference.backend(),
        }));
    }
    Ok(())
}
fn normalize(error: BrowserError, operation: Operation, version: u32) -> Error {
    match error {
        BrowserError::Provider(error) => error,
        BrowserError::MissingKey => Error::SecretNotFound {
            backend: BackendKind::External,
            key_version: version,
        },
        BrowserError::Closed | BrowserError::Unavailable => Error::BackendUnavailable {
            backend: BackendKind::External,
        },
        _ => Error::BackendFailure {
            backend: BackendKind::External,
            operation,
        },
    }
}

/// One immutable selected provider key in explicit origin storage. The public
/// fingerprint is an independently pinned identity, never signing/stock authority.
/// Native targets do not compile this adapter. Imports and capabilities do no IO.
pub struct BrowserProvider {
    state: Rc<State>,
}
impl BrowserProvider {
    /// Explicitly provisions a new namespace. The generated key is never used
    /// for private payloads or exposed before its binding commits atomically.
    /// Existing, including damaged, namespaces are never replaced.
    pub async fn create(namespace: SecretId, reference: SecretRef) -> Result<Self, BrowserError> {
        selected(&reference)?;
        let setup = Setup::admit()?;
        let factory = factory()?;
        let (crypto, subtle) = crypto::host()?;
        let (send, observe) = oneshot::channel();
        spawn_local(async move {
            let _setup = setup;
            let result = async {
                match idb::open(&factory, &name(&namespace), None).await {
                    Ok(db) => {
                        db.close();
                        return Err(BrowserError::NamespaceExists);
                    }
                    Err(BrowserError::MissingNamespace) => {}
                    Err(error) => return Err(error),
                }
                let key = crypto::generate(&crypto, &subtle).await?;
                let fingerprint =
                    crypto::fingerprint(&subtle, &key, &namespace, &reference).await?;
                let seed = idb::Seed {
                    key,
                    binding: binding_bytes(&reference, &fingerprint),
                };
                let db = idb::open(&factory, &name(&namespace), Some(seed)).await?;
                Ok(Self::assemble(
                    db,
                    subtle,
                    namespace,
                    reference,
                    fingerprint,
                ))
            }
            .await;
            let _ = send.send(result);
        });
        observe.await.unwrap_or(Err(BrowserError::CommitUnknown))
    }
    /// Opens an existing namespace only with the caller's independently pinned
    /// fingerprint. Missing or changed keys never authorize fresh generation.
    pub async fn open(
        namespace: SecretId,
        reference: SecretRef,
        expected_fingerprint: [u8; 32],
    ) -> Result<Self, BrowserError> {
        selected(&reference)?;
        let setup = Setup::admit()?;
        let factory = factory()?;
        let (_, subtle) = crypto::host()?;
        let (send, observe) = oneshot::channel();
        spawn_local(async move {
            let _setup = setup;
            let result = async {
                let db = idb::open(&factory, &name(&namespace), None).await?;
                let provider =
                    Self::assemble(db, subtle, namespace, reference, expected_fingerprint);
                provider.state.key().await?;
                Ok(provider)
            }
            .await;
            let _ = send.send(result);
        });
        observe.await.unwrap_or(Err(BrowserError::CommitUnknown))
    }
    fn assemble(
        db: IdbDatabase,
        subtle: SubtleCrypto,
        namespace: SecretId,
        reference: SecretRef,
        fingerprint: [u8; 32],
    ) -> Self {
        let state = Rc::new(State {
            db,
            subtle,
            namespace,
            reference,
            fingerprint,
            closing: Cell::new(false),
            jobs: Cell::new(0),
            bytes: Cell::new(0),
            waiters: RefCell::new(Vec::new()),
            version_change: RefCell::new(None),
        });
        let weak = Rc::downgrade(&state);
        let changed = Closure::new(move |_: Event| {
            if let Some(state) = weak.upgrade() {
                state.stop();
                state.db.close();
            }
        });
        state
            .db
            .set_onversionchange(Some(changed.as_ref().unchecked_ref()));
        state.db.set_onclose(Some(changed.as_ref().unchecked_ref()));
        *state.version_change.borrow_mut() = Some(changed);
        Self { state }
    }
    /// Returns the public fingerprint that must be retained independently
    /// for exact subsequent opening. It reveals no provider key material.
    pub fn fingerprint(&self) -> [u8; 32] {
        self.state.fingerprint
    }
    /// Stops admission, drains accepted jobs and closes the actual connection.
    /// Cancellation of this observation does not cancel accepted jobs.
    pub async fn close(&self) -> Result<(), BrowserError> {
        self.state.stop();
        if self.state.jobs.get() == 0 {
            return Ok(());
        }
        let signal = Rc::new(RefCell::new(None));
        {
            let mut waiters = self.state.waiters.borrow_mut();
            waiters.retain(|weak| weak.strong_count() > 0);
            if waiters.len() >= JOBS_MAX {
                return Err(BrowserError::Capacity);
            }
            waiters.push(Rc::downgrade(&signal));
        }
        poll_fn(|cx| {
            if self.state.jobs.get() == 0 {
                Poll::Ready(())
            } else {
                *signal.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await;
        Ok(())
    }
    fn check(&self, reference: &SecretRef) -> Result<(), Error> {
        selected(reference)
            .map_err(|e| normalize(e, Operation::Read, reference.key_version().get()))?;
        if reference.id() != self.state.reference.id()
            || reference.key_version() != self.state.reference.key_version()
        {
            return Err(Error::SecretNotFound {
                backend: BackendKind::External,
                key_version: reference.key_version().get(),
            });
        }
        Ok(())
    }
}
impl fmt::Debug for BrowserProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BrowserProvider(<redacted>)")
    }
}
impl SecretProvider for BrowserProvider {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::External
    }
    fn capabilities(&self) -> SecretCapabilities {
        if self.state.closing.get() {
            SecretCapabilities::unavailable()
        } else {
            SecretCapabilities::available(
                ResidencySupport::UserProfile,
                CapabilitySupport::Unavailable,
                CapabilitySupport::Unavailable,
            )
        }
    }
}
impl KeyWrapping for BrowserProvider {
    fn wrap<'a>(&'a self, request: WrapRequest<'a>) -> BoxFuture<'a, Result<WrappedSecret, Error>> {
        Box::pin(async move {
            self.check(request.reference())?;
            if request.plaintext().len() != 32 {
                return Err(Error::InvalidDataKeyLength {
                    actual_bytes: request.plaintext().len(),
                });
            }
            let version = request.reference().key_version().get();
            let work = self
                .state
                .admit(16 * 1024)
                .map_err(|e| normalize(e, Operation::Wrap, version))?;
            let plaintext = request
                .plaintext()
                .expose_secret(SecretMaterial::from_slice)?;
            let context = request.context().clone();
            let state = self.state.clone();
            owned(work, async move {
                let key = state.key().await?;
                crypto::wrap(
                    &state.subtle,
                    &key,
                    &state.namespace,
                    &state.reference,
                    &context,
                    &plaintext,
                )
                .await
            })
            .await
            .map_err(|e| normalize(e, Operation::Wrap, version))
        })
    }
    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        Box::pin(async move {
            self.check(request.reference())?;
            let version = request.reference().key_version().get();
            crypto::validate_wrapped(request.wrapped())
                .map_err(|e| normalize(e, Operation::Unwrap, version))?;
            let work = self
                .state
                .admit(16 * 1024)
                .map_err(|e| normalize(e, Operation::Unwrap, version))?;
            let wrapped = request.wrapped().clone();
            let context = request.context().clone();
            let state = self.state.clone();
            owned(work, async move {
                let key = state.key().await?;
                crypto::unwrap(
                    &state.subtle,
                    &key,
                    &state.namespace,
                    &state.reference,
                    &context,
                    &wrapped,
                )
                .await
            })
            .await
            .map_err(|e| normalize(e, Operation::Unwrap, version))
        })
    }
}

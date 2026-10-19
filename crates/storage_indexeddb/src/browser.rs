use super::{
    Batch, Commit, Error, LIVE_BYTES_MAX, Mutation, Namespace, OpenMode, RECORD_BYTES_MAX,
    RecordKey,
};
use futures::channel::oneshot;
use js_sys::{ArrayBuffer, Function, Object, Reflect, Uint8Array};
use std::{
    cell::{Cell, RefCell},
    future::poll_fn,
    rc::{Rc, Weak},
    task::{Poll, Waker},
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::spawn_local;
use web_sys::{
    Event, IdbCursorWithValue, IdbDatabase, IdbFactory, IdbKeyRange, IdbObjectStore,
    IdbOpenDbRequest, IdbRequest, IdbTransaction, IdbTransactionMode,
};

#[cfg(feature = "workspace")]
mod snapshot;
#[cfg(feature = "workspace")]
pub use snapshot::{ReadSnapshot, RecordPrefix};

const STORE: &str = "records";
const WORK_MAX: usize = 32;
type EventCallback = Closure<dyn FnMut(Event)>;

fn classify(value: JsValue) -> Error {
    match value
        .dyn_ref::<web_sys::DomException>()
        .map(web_sys::DomException::name)
        .as_deref()
    {
        Some("QuotaExceededError") => Error::Capacity,
        Some("VersionError") => Error::IncompatibleSchema,
        Some("InvalidStateError") => Error::Closed,
        _ => Error::Unavailable,
    }
}
fn key(value: RecordKey) -> JsValue {
    Uint8Array::from(value.as_bytes().as_slice()).into()
}
fn bytes(value: JsValue) -> Result<Option<Vec<u8>>, Error> {
    if value.is_undefined() {
        return Ok(None);
    }
    let value = value
        .dyn_into::<Uint8Array>()
        .map_err(|_| Error::CorruptRecord)?;
    if value.length() as usize > RECORD_BYTES_MAX {
        return Err(Error::CorruptRecord);
    }
    Ok(Some(value.to_vec()))
}

fn record_key(raw: JsValue) -> Result<RecordKey, Error> {
    let view = if let Some(view) = raw.dyn_ref::<Uint8Array>() {
        view.clone()
    } else if raw.is_instance_of::<ArrayBuffer>() {
        Uint8Array::new(&raw)
    } else {
        return Err(Error::CorruptRecord);
    };
    if view.length() != 32 {
        return Err(Error::CorruptRecord);
    }
    let mut value = [0; 32];
    view.copy_to(&mut value);
    Ok(RecordKey::new(value))
}
fn lookup(store: &IdbObjectStore, record: RecordKey) -> Result<IdbRequest, JsValue> {
    let range = IdbKeyRange::only(&key(record))?;
    store.open_cursor_with_range(range.as_ref())
}
fn lookup_value(request: &IdbRequest, expected: RecordKey) -> Result<Option<Vec<u8>>, Error> {
    let value = request.result().map_err(classify)?;
    if value.is_null() {
        return Ok(None);
    }
    let cursor = value
        .dyn_into::<IdbCursorWithValue>()
        .map_err(|_| Error::CorruptRecord)?;
    if record_key(cursor.key().map_err(classify)?)? != expected {
        return Err(Error::CorruptRecord);
    }
    // A cursor distinguishes absent keys from stored undefined. Undefined is
    // foreign data, never permission to overwrite an allegedly missing value.
    let value = bytes(cursor.value().map_err(classify)?)?.ok_or(Error::CorruptRecord)?;
    Ok(Some(value))
}

fn write_transaction(db: &IdbDatabase) -> Result<IdbTransaction, Error> {
    // The pinned web-sys options binding requires a global unstable cfg. Keep
    // that compiler input unchanged and use the checked standard host method.
    let options = Object::new();
    if !Reflect::set(&options, &"durability".into(), &"strict".into()).map_err(classify)? {
        return Err(Error::Unavailable);
    }
    let method = Reflect::get(db, &"transaction".into())
        .map_err(classify)?
        .dyn_into::<Function>()
        .map_err(|_| Error::Unavailable)?;
    let tx = method
        .call3(db, &STORE.into(), &"readwrite".into(), &options)
        .map_err(classify)?
        .dyn_into::<IdbTransaction>()
        .map_err(|_| Error::Unavailable)?;
    if Reflect::get(&tx, &"durability".into())
        .map_err(classify)?
        .as_string()
        .as_deref()
        != Some("strict")
    {
        let _ = tx.abort();
        return Err(Error::Unavailable);
    }
    Ok(tx)
}

struct State {
    db: IdbDatabase,
    closing: Cell<bool>,
    count: Cell<usize>,
    bytes: Cell<usize>,
    waiters: RefCell<Vec<Weak<RefCell<Option<Waker>>>>>,
    version_change: RefCell<Option<EventCallback>>,
}
impl State {
    fn stop(&self) {
        self.closing.set(true);
        if self.count.get() == 0 {
            self.db.close();
        }
        self.wake();
    }
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
    fn admit(self: &Rc<Self>, bytes: usize) -> Result<Work, Error> {
        if self.closing.get() {
            return Err(Error::Closed);
        }
        let total = self.bytes.get().checked_add(bytes).ok_or(Error::Capacity)?;
        if self.count.get() >= WORK_MAX || total > LIVE_BYTES_MAX {
            return Err(Error::Capacity);
        }
        self.count.set(self.count.get() + 1);
        self.bytes.set(total);
        Ok(Work {
            state: self.clone(),
            bytes,
        })
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.db.set_onversionchange(None);
        self.db.close();
    }
}
struct Work {
    state: Rc<State>,
    bytes: usize,
}
impl Drop for Work {
    fn drop(&mut self) {
        self.state.count.set(self.state.count.get() - 1);
        self.state.bytes.set(self.state.bytes.get() - self.bytes);
        if self.state.closing.get() && self.state.count.get() == 0 {
            self.state.db.close();
        }
        self.state.wake();
    }
}

/// One explicit connection. Clones share admission, accepted work and closure.
/// Independent opens use independent handles; IndexedDB serializes conflicts.
#[derive(Clone)]
pub struct Database {
    state: Rc<State>,
}
impl Database {
    /// Explicitly opens the exact namespace. Dropped observation cannot leak a
    /// subsequently opened connection or cancel an accepted schema transaction.
    pub async fn open(namespace: Namespace, mode: OpenMode) -> Result<Self, Error> {
        let factory = Reflect::get(&js_sys::global(), &JsValue::from_str("indexedDB"))
            .map_err(classify)?
            .dyn_into::<IdbFactory>()
            .map_err(|_| Error::Unavailable)?;
        let request = factory
            .open_with_u32(&namespace.name(), 1)
            .map_err(classify)?;
        let (send, observe) = oneshot::channel();
        spawn_local(async move {
            let _ = send.send(open(request, mode).await);
        });
        observe.await.unwrap_or(Err(Error::Unavailable))
    }
    /// Reads exact bytes, acknowledging the read transaction only on complete.
    /// Missing is distinct from an empty value and from a missing namespace.
    pub async fn get(&self, record: RecordKey) -> Result<Option<Vec<u8>>, Error> {
        let work = self.state.admit(RECORD_BYTES_MAX)?;
        let (send, observe) = oneshot::channel();
        spawn_local(async move {
            let owner = work;
            let result = match owner
                .state
                .db
                .transaction_with_str_and_mode(STORE, IdbTransactionMode::Readonly)
            {
                Ok(tx) => read(tx, record).await,
                Err(error) => Err(classify(error)),
            };
            let _ = send.send(result);
        });
        observe.await.unwrap_or(Err(Error::CommitUnknown))
    }
    /// Atomically compares original values and installs all replacements.
    /// All requests are enqueued in active callbacks; no user future runs
    /// inside a transaction. Only the complete event returns a Commit.
    pub async fn compare_exchange(&self, batch: Batch) -> Result<Commit, Error> {
        self.exchange(batch, None).await
    }
    /// Runs a trusted host's synchronous eligibility check in each active
    /// write callback. Refusal aborts the whole transaction, including earlier
    /// writes. Captures must obey the concrete host's retained-input budget.
    pub async fn compare_exchange_checked(
        &self,
        batch: Batch,
        guard: impl Fn() -> bool + 'static,
    ) -> Result<Commit, Error> {
        self.exchange(batch, Some(Rc::new(guard))).await
    }
    async fn exchange(
        &self,
        batch: Batch,
        guard: Option<Rc<dyn Fn() -> bool>>,
    ) -> Result<Commit, Error> {
        let work = self.state.admit(batch.bytes)?;
        let (send, observe) = oneshot::channel();
        spawn_local(async move {
            let owner = work;
            let result = match write_transaction(&owner.state.db) {
                Ok(tx) => exchange(tx, batch, guard).await,
                Err(error) => Err(error),
            };
            let _ = send.send(result);
        });
        observe.await.unwrap_or(Err(Error::CommitUnknown))
    }
    /// Stops admission immediately when polled, drains every accepted job and
    /// closes the connection. A cancelled observer can explicitly resume close.
    pub async fn close(&self) -> Result<(), Error> {
        self.state.stop();
        if self.state.count.get() == 0 {
            return Ok(());
        }
        let signal = Rc::new(RefCell::new(None));
        {
            let mut waiters = self.state.waiters.borrow_mut();
            waiters.retain(|weak| weak.strong_count() > 0);
            if waiters.len() >= WORK_MAX {
                return Err(Error::Capacity);
            }
            waiters.push(Rc::downgrade(&signal));
        }
        poll_fn(|cx| {
            if self.state.count.get() == 0 {
                return Poll::Ready(());
            }
            *signal.borrow_mut() = Some(cx.waker().clone());
            Poll::Pending
        })
        .await;
        Ok(())
    }
}

struct OpenHandlers {
    request: IdbOpenDbRequest,
    _upgrade: Closure<dyn FnMut(Event)>,
    _success: Closure<dyn FnMut(Event)>,
    _error: Closure<dyn FnMut(Event)>,
}
impl Drop for OpenHandlers {
    fn drop(&mut self) {
        self.request.set_onupgradeneeded(None);
        self.request.set_onsuccess(None);
        self.request.set_onerror(None);
    }
}
async fn open(request: IdbOpenDbRequest, mode: OpenMode) -> Result<Database, Error> {
    let created = Rc::new(Cell::new(false));
    let failure = Rc::new(Cell::new(None));
    let (send, observe) = oneshot::channel();
    let reply = Rc::new(RefCell::new(Some(send)));
    let upgrade = {
        let request = request.clone();
        let created = created.clone();
        let failure = failure.clone();
        Closure::new(move |_: Event| {
            let result = (|| {
                if mode == OpenMode::Existing {
                    return Err(Error::MissingNamespace);
                }
                let db = request
                    .result()
                    .map_err(classify)?
                    .dyn_into::<IdbDatabase>()
                    .map_err(|_| Error::IncompatibleSchema)?;
                db.create_object_store(STORE).map_err(classify)?;
                created.set(true);
                Ok(())
            })();
            if let Err(error) = result {
                failure.set(Some(error));
                if let Some(tx) = request.transaction() {
                    let _ = tx.abort();
                }
            }
        })
    };
    let success = {
        let request = request.clone();
        let created = created.clone();
        let failure = failure.clone();
        let reply = reply.clone();
        Closure::new(move |_: Event| {
            let result = (|| {
                let db = request
                    .result()
                    .map_err(classify)?
                    .dyn_into::<IdbDatabase>()
                    .map_err(|_| Error::IncompatibleSchema)?;
                let validation = if let Some(error) = failure.get() {
                    Err(error)
                } else if mode == OpenMode::CreateNew && !created.get() {
                    Err(Error::NamespaceExists)
                } else {
                    validate_schema(&db)
                };
                if let Err(error) = validation {
                    db.close();
                    return Err(error);
                }
                let state = Rc::new(State {
                    db,
                    closing: Cell::new(false),
                    count: Cell::new(0),
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
                *state.version_change.borrow_mut() = Some(changed);
                Ok(Database { state })
            })();
            if let Some(send) = reply.borrow_mut().take() {
                let _ = send.send(result);
            }
        })
    };
    let error = {
        let request = request.clone();
        let reply = reply.clone();
        let failure = failure.clone();
        Closure::new(move |_: Event| {
            let error = failure.get().unwrap_or_else(|| {
                request
                    .error()
                    .ok()
                    .flatten()
                    .map(|e| classify(e.into()))
                    .unwrap_or(Error::Unavailable)
            });
            if let Some(send) = reply.borrow_mut().take() {
                let _ = send.send(Err(error));
            }
        })
    };
    request.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    request.set_onsuccess(Some(success.as_ref().unchecked_ref()));
    request.set_onerror(Some(error.as_ref().unchecked_ref()));
    let _handlers = OpenHandlers {
        request,
        _upgrade: upgrade,
        _success: success,
        _error: error,
    };
    observe.await.unwrap_or(Err(Error::Unavailable))
}
fn validate_schema(db: &IdbDatabase) -> Result<(), Error> {
    if db.version() != 1.0
        || db.object_store_names().length() != 1
        || !db.object_store_names().contains(STORE)
    {
        return Err(Error::IncompatibleSchema);
    }
    let tx = db.transaction_with_str(STORE).map_err(classify)?;
    let store = tx.object_store(STORE).map_err(classify)?;
    if store.auto_increment()
        || !store.key_path().map_err(classify)?.is_null()
        || store.index_names().length() != 0
    {
        return Err(Error::IncompatibleSchema);
    }
    Ok(())
}

struct Completion {
    tx: IdbTransaction,
    result: oneshot::Receiver<Result<(), Error>>,
    _complete: Closure<dyn FnMut(Event)>,
    _abort: Closure<dyn FnMut(Event)>,
    _error: Closure<dyn FnMut(Event)>,
}
impl Completion {
    fn new(tx: IdbTransaction, failure: Rc<Cell<Option<Error>>>) -> Self {
        let (send, result) = oneshot::channel();
        let reply = Rc::new(RefCell::new(Some(send)));
        let complete = {
            let reply = reply.clone();
            let failure = failure.clone();
            Closure::new(move |_: Event| {
                // An observed local refusal must never become a committed claim if
                // an unexpected host failed to abort the transaction.
                let result = if failure.get().is_some() {
                    Err(Error::CommitUnknown)
                } else {
                    Ok(())
                };
                if let Some(send) = reply.borrow_mut().take() {
                    let _ = send.send(result);
                }
            })
        };
        let abort = {
            let tx = tx.clone();
            let failure = failure.clone();
            Closure::new(move |_: Event| {
                let error = failure.get().unwrap_or_else(|| {
                    tx.error()
                        .map(|e| {
                            if e.name() == "QuotaExceededError" {
                                Error::Capacity
                            } else {
                                Error::Aborted
                            }
                        })
                        .unwrap_or(Error::Aborted)
                });
                if let Some(send) = reply.borrow_mut().take() {
                    let _ = send.send(Err(error));
                }
            })
        };
        let error = {
            let tx = tx.clone();
            let failure = failure.clone();
            Closure::new(move |event: Event| {
                let quota = event
                    .target()
                    .and_then(|target| target.dyn_into::<IdbRequest>().ok())
                    .and_then(|request| request.error().ok().flatten())
                    .is_some_and(|error| error.name() == "QuotaExceededError");
                // Remember asynchronous request failure even if another host
                // listener prevents its default abort. Unexpected completion
                // then remains unknown, never a successful partial batch.
                refuse(
                    &tx,
                    &failure,
                    if quota {
                        Error::Capacity
                    } else {
                        Error::Aborted
                    },
                );
            })
        };
        tx.set_oncomplete(Some(complete.as_ref().unchecked_ref()));
        tx.set_onabort(Some(abort.as_ref().unchecked_ref()));
        tx.set_onerror(Some(error.as_ref().unchecked_ref()));
        Self {
            tx,
            result,
            _complete: complete,
            _abort: abort,
            _error: error,
        }
    }
    async fn wait(&mut self) -> Result<(), Error> {
        (&mut self.result)
            .await
            .unwrap_or(Err(Error::CommitUnknown))
    }
}
impl Drop for Completion {
    fn drop(&mut self) {
        self.tx.set_oncomplete(None);
        self.tx.set_onabort(None);
        self.tx.set_onerror(None);
    }
}
struct RequestHandler {
    request: IdbRequest,
    _success: Closure<dyn FnMut(Event)>,
}
impl Drop for RequestHandler {
    fn drop(&mut self) {
        self.request.set_onsuccess(None);
    }
}
fn refuse(tx: &IdbTransaction, failure: &Cell<Option<Error>>, error: Error) {
    if failure.get().is_none() {
        failure.set(Some(error));
    }
    let _ = tx.abort();
}
async fn read(tx: IdbTransaction, record: RecordKey) -> Result<Option<Vec<u8>>, Error> {
    let failure = Rc::new(Cell::new(None));
    let mut completion = Completion::new(tx.clone(), failure.clone());
    let value = Rc::new(RefCell::new(None));
    let request = match tx
        .object_store(STORE)
        .and_then(|store| lookup(&store, record))
    {
        Ok(request) => request,
        Err(error) => {
            refuse(&tx, &failure, classify(error));
            completion.wait().await?;
            return Err(Error::CommitUnknown);
        }
    };
    let success = {
        let request = request.clone();
        let value = value.clone();
        let tx = tx.clone();
        Closure::new(move |_: Event| match lookup_value(&request, record) {
            Ok(result) => *value.borrow_mut() = Some(result),
            Err(error) => refuse(&tx, &failure, error),
        })
    };
    request.set_onsuccess(Some(success.as_ref().unchecked_ref()));
    let _handler = RequestHandler {
        request,
        _success: success,
    };
    completion.wait().await?;
    value.borrow_mut().take().ok_or(Error::CommitUnknown)
}
async fn exchange(
    tx: IdbTransaction,
    batch: Batch,
    guard: Option<Rc<dyn Fn() -> bool>>,
) -> Result<Commit, Error> {
    let failure = Rc::new(Cell::new(None));
    let mut completion = Completion::new(tx.clone(), failure.clone());
    let mut handlers = Vec::with_capacity(batch.entries.len());
    let count = batch.entries.len();
    let store = match tx.object_store(STORE) {
        Ok(store) => store,
        Err(error) => {
            refuse(&tx, &failure, classify(error));
            completion.wait().await?;
            return Err(Error::CommitUnknown);
        }
    };
    queue_batch(&store, &tx, batch, guard, &failure, &mut handlers);
    completion.wait().await?;
    Ok(Commit { records: count })
}

fn queue_batch(
    store: &IdbObjectStore,
    tx: &IdbTransaction,
    batch: Batch,
    guard: Option<Rc<dyn Fn() -> bool>>,
    failure: &Rc<Cell<Option<Error>>>,
    handlers: &mut Vec<RequestHandler>,
) {
    // All reads are queued in this active turn. Each callback queues its write
    // synchronously; any later conflict aborts every earlier write atomically.
    for entry in batch.entries {
        let request = match lookup(store, entry.key) {
            Ok(request) => request,
            Err(error) => {
                refuse(tx, failure, classify(error));
                break;
            }
        };
        let success = {
            let request = request.clone();
            let store = store.clone();
            let tx = tx.clone();
            let failure = failure.clone();
            let guard = guard.clone();
            Closure::new(move |_: Event| {
                if failure.get().is_some() {
                    return;
                }
                if guard.as_ref().is_some_and(|check| !check()) {
                    refuse(&tx, &failure, Error::GuardRefused);
                    return;
                }
                if let Err(error) = apply(&store, &request, &entry) {
                    refuse(&tx, &failure, error);
                }
            })
        };
        request.set_onsuccess(Some(success.as_ref().unchecked_ref()));
        handlers.push(RequestHandler {
            request,
            _success: success,
        });
    }
}
fn apply(store: &IdbObjectStore, request: &IdbRequest, entry: &Mutation) -> Result<(), Error> {
    let actual = lookup_value(request, entry.key)?;
    if actual != entry.expected {
        return Err(Error::Conflict);
    }
    match &entry.replacement {
        Some(value) => store.put_with_key(&Uint8Array::from(value.as_slice()), &key(entry.key)),
        None => store.delete(&key(entry.key)),
    }
    .map_err(classify)?;
    Ok(())
}

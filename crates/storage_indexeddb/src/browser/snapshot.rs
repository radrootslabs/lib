//! Bounded opaque read witnesses; no crypto provider runs in a transaction.
use super::{
    Batch, Commit, Completion, Database, Error, RECORD_BYTES_MAX, RecordKey, RequestHandler, STORE,
    State, bytes, classify, queue_batch, record_key, refuse, write_transaction,
};
use futures::channel::oneshot;
use js_sys::Uint8Array;
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    fmt,
    mem::size_of,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::spawn_local;
use web_sys::{
    Event, IdbCursorWithValue, IdbKeyRange, IdbObjectStore, IdbRequest, IdbTransaction,
    IdbTransactionMode,
};

const ROWS_MAX: usize = 16_384;
const SCAN_RESERVATION: usize = 2 * 1024 * 1024;

/// Opaque binary-key prefix. It carries no workspace or business permission.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct RecordPrefix([u8; 16]);
impl RecordPrefix {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    fn range(self) -> Result<IdbKeyRange, Error> {
        let lower: JsValue = Uint8Array::from(self.0.as_slice()).into();
        let mut upper = self.0;
        for i in (0..upper.len()).rev() {
            if upper[i] != u8::MAX {
                upper[i] += 1;
                let upper: JsValue = Uint8Array::from(upper.as_slice()).into();
                return IdbKeyRange::bound_with_lower_open_and_upper_open(
                    &lower, &upper, false, true,
                )
                .map_err(classify);
            }
            upper[i] = 0;
        }
        IdbKeyRange::lower_bound(&lower).map_err(classify)
    }
    fn contains(self, key: RecordKey) -> bool {
        key.as_bytes()[..16] == self.0
    }
}
impl fmt::Debug for RecordPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecordPrefix(<redacted>)")
    }
}
struct Witness {
    key: RecordKey,
    length: usize,
    digest: [u8; 32],
}
// Vec growth, one bounded record, and callback scaffolding fit the reservation.
const _: () = assert!(
    size_of::<Witness>() * ROWS_MAX * 3 / 2 + 2 * RECORD_BYTES_MAX + 4096 <= SCAN_RESERVATION
);

/// Exact completed read set on one live connection. No serialization or clone
/// can turn it into a portable authorization token. Values are not retained.
pub struct ReadSnapshot {
    owner: Weak<State>,
    prefix: RecordPrefix,
    rows: Vec<Witness>,
}
impl ReadSnapshot {
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn contains(&self, key: RecordKey) -> bool {
        self.rows.binary_search_by_key(&key, |row| row.key).is_ok()
    }
    pub fn keys(&self) -> impl Iterator<Item = RecordKey> + '_ {
        self.rows.iter().map(|row| row.key)
    }
    /// Binds each value decoded outside the transaction to the completed view.
    pub fn verify(&self, key: RecordKey, actual: Option<&[u8]>) -> Result<(), Error> {
        if !self.prefix.contains(key) {
            return Err(Error::InvalidInput);
        }
        match (self.rows.binary_search_by_key(&key, |row| row.key), actual) {
            (Err(_), None) => Ok(()),
            (Ok(i), Some(raw)) => matches(&self.rows[i], raw),
            _ => Err(Error::Conflict),
        }
    }
    fn reservation(&self) -> usize {
        size_of::<Self>() + self.rows.capacity() * size_of::<Witness>()
    }
}
impl fmt::Debug for ReadSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReadSnapshot(<redacted>)")
    }
}
fn matches(witness: &Witness, raw: &[u8]) -> Result<(), Error> {
    if raw.len() > RECORD_BYTES_MAX {
        return Err(Error::CorruptRecord);
    }
    if raw.len() != witness.length || <[u8; 32]>::from(Sha256::digest(raw)) != witness.digest {
        return Err(Error::Conflict);
    }
    Ok(())
}
fn cursor_key(cursor: &IdbCursorWithValue, prefix: RecordPrefix) -> Result<RecordKey, Error> {
    let key = record_key(cursor.key().map_err(classify)?)?;
    if !prefix.contains(key) {
        return Err(Error::CorruptRecord);
    }
    Ok(key)
}
fn start_cursor(store: &IdbObjectStore, prefix: RecordPrefix) -> Result<IdbRequest, Error> {
    store
        .open_cursor_with_range(prefix.range()?.as_ref())
        .map_err(classify)
}
impl Database {
    /// Captures bounded key/length/digest witnesses, observing real completion.
    /// Accepted scanning survives loss of its observer and participates in close.
    pub async fn read_snapshot(&self, prefix: RecordPrefix) -> Result<ReadSnapshot, Error> {
        let work = self.state.admit(SCAN_RESERVATION)?;
        let (send, observe) = oneshot::channel();
        spawn_local(async move {
            let owner = work;
            let result = match owner
                .state
                .db
                .transaction_with_str_and_mode(STORE, IdbTransactionMode::Readonly)
            {
                Ok(tx) => scan(tx, Rc::downgrade(&owner.state), prefix).await,
                Err(error) => Err(classify(error)),
            };
            let _ = send.send(result);
        });
        observe.await.unwrap_or(Err(Error::CommitUnknown))
    }
    /// Rechecks the complete witnessed set before any batch write, in the same
    /// real transaction. All cursor and mutation callbacks recheck eligibility.
    pub async fn compare_exchange_snapshot_checked(
        &self,
        batch: Batch,
        snapshot: ReadSnapshot,
        guard: impl Fn() -> bool + 'static,
    ) -> Result<Commit, Error> {
        if !snapshot
            .owner
            .upgrade()
            .is_some_and(|owner| Rc::ptr_eq(&owner, &self.state))
        {
            return Err(Error::InvalidInput);
        }
        let reservation = batch
            .bytes
            .checked_add(snapshot.reservation())
            .ok_or(Error::Capacity)?;
        let work = self.state.admit(reservation)?;
        let (send, observe) = oneshot::channel();
        spawn_local(async move {
            let owner = work;
            let result = match write_transaction(&owner.state.db) {
                Ok(tx) => checked_exchange(tx, batch, snapshot, Rc::new(guard)).await,
                Err(error) => Err(error),
            };
            let _ = send.send(result);
        });
        observe.await.unwrap_or(Err(Error::CommitUnknown))
    }
}
async fn scan(
    tx: IdbTransaction,
    owner: Weak<State>,
    prefix: RecordPrefix,
) -> Result<ReadSnapshot, Error> {
    let failure = Rc::new(Cell::new(None));
    let mut completion = Completion::new(tx.clone(), failure.clone());
    let rows = Rc::new(RefCell::new(Vec::<Witness>::new()));
    let done = Rc::new(Cell::new(false));
    let request = match tx
        .object_store(STORE)
        .map_err(classify)
        .and_then(|store| start_cursor(&store, prefix))
    {
        Ok(request) => request,
        Err(error) => {
            refuse(&tx, &failure, error);
            completion.wait().await?;
            return Err(Error::CommitUnknown);
        }
    };
    let success = {
        let request = request.clone();
        let tx = tx.clone();
        let failure = failure.clone();
        let rows = rows.clone();
        let done = done.clone();
        Closure::new(move |_: Event| {
            if failure.get().is_some() {
                return;
            }
            let result = (|| {
                let value = request.result().map_err(classify)?;
                if value.is_null() {
                    done.set(true);
                    return Ok(());
                }
                let cursor = value
                    .dyn_into::<IdbCursorWithValue>()
                    .map_err(|_| Error::CorruptRecord)?;
                let key = cursor_key(&cursor, prefix)?;
                let mut rows = rows.borrow_mut();
                if rows.len() >= ROWS_MAX {
                    return Err(Error::Capacity);
                }
                if rows.last().is_some_and(|previous| previous.key >= key) {
                    return Err(Error::CorruptRecord);
                }
                let raw = bytes(cursor.value().map_err(classify)?)?.ok_or(Error::CorruptRecord)?;
                rows.push(Witness {
                    key,
                    length: raw.len(),
                    digest: Sha256::digest(&raw).into(),
                });
                drop(rows);
                cursor.continue_().map_err(classify)
            })();
            if let Err(error) = result {
                refuse(&tx, &failure, error);
            }
        })
    };
    request.set_onsuccess(Some(success.as_ref().unchecked_ref()));
    let _handler = RequestHandler {
        request,
        _success: success,
    };
    completion.wait().await?;
    if !done.get() {
        return Err(Error::CommitUnknown);
    }
    let rows = std::mem::take(&mut *rows.borrow_mut());
    Ok(ReadSnapshot {
        owner,
        prefix,
        rows,
    })
}
async fn checked_exchange(
    tx: IdbTransaction,
    batch: Batch,
    snapshot: ReadSnapshot,
    guard: Rc<dyn Fn() -> bool>,
) -> Result<Commit, Error> {
    let failure = Rc::new(Cell::new(None));
    let mut completion = Completion::new(tx.clone(), failure.clone());
    let count = batch.entries.len();
    let store = match tx.object_store(STORE) {
        Ok(store) => store,
        Err(error) => {
            refuse(&tx, &failure, classify(error));
            completion.wait().await?;
            return Err(Error::CommitUnknown);
        }
    };
    let request = match start_cursor(&store, snapshot.prefix) {
        Ok(request) => request,
        Err(error) => {
            refuse(&tx, &failure, error);
            completion.wait().await?;
            return Err(Error::CommitUnknown);
        }
    };
    let handlers = Rc::new(RefCell::new(Vec::new()));
    let batch = Rc::new(RefCell::new(Some(batch)));
    let queued = Rc::new(Cell::new(false));
    let success = {
        let request = request.clone();
        let tx = tx.clone();
        let failure = failure.clone();
        let queued = queued.clone();
        let handlers = handlers.clone();
        let guard = guard.clone();
        let mut seen = 0;
        Closure::new(move |_: Event| {
            if failure.get().is_some() {
                return;
            }
            let result = (|| {
                if !guard() {
                    return Err(Error::GuardRefused);
                }
                let value = request.result().map_err(classify)?;
                if value.is_null() {
                    if seen != snapshot.rows.len() {
                        return Err(Error::Conflict);
                    }
                    let batch = batch.borrow_mut().take().ok_or(Error::CorruptRecord)?;
                    queue_batch(
                        &store,
                        &tx,
                        batch,
                        Some(guard.clone()),
                        &failure,
                        &mut handlers.borrow_mut(),
                    );
                    queued.set(true);
                    return Ok(());
                }
                let cursor = value
                    .dyn_into::<IdbCursorWithValue>()
                    .map_err(|_| Error::CorruptRecord)?;
                let key = cursor_key(&cursor, snapshot.prefix)?;
                let expected = snapshot.rows.get(seen).ok_or(Error::Conflict)?;
                if key != expected.key {
                    return Err(Error::Conflict);
                }
                let raw = bytes(cursor.value().map_err(classify)?)?.ok_or(Error::CorruptRecord)?;
                matches(expected, &raw)?;
                seen += 1;
                cursor.continue_().map_err(classify)
            })();
            if let Err(error) = result {
                refuse(&tx, &failure, error);
            }
        })
    };
    request.set_onsuccess(Some(success.as_ref().unchecked_ref()));
    let _cursor = RequestHandler {
        request,
        _success: success,
    };
    completion.wait().await?;
    if !queued.get() {
        return Err(Error::CommitUnknown);
    }
    Ok(Commit { records: count })
}

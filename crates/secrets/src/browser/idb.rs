//! Browser event roots follow the qualified local transaction ownership pattern.
use super::BrowserError;
use futures::channel::oneshot;
use js_sys::Uint8Array;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{
    CryptoKey, Event, IdbDatabase, IdbFactory, IdbOpenDbRequest, IdbRequest, IdbTransaction,
};

const STORE: &str = "provider";
type Callback = Closure<dyn FnMut(Event)>;

pub(super) struct Seed {
    pub key: CryptoKey,
    pub binding: Vec<u8>,
}
struct OpenRoots {
    request: IdbOpenDbRequest,
    _upgrade: Callback,
    _success: Callback,
    _error: Callback,
}
impl Drop for OpenRoots {
    fn drop(&mut self) {
        self.request.set_onupgradeneeded(None);
        self.request.set_onsuccess(None);
        self.request.set_onerror(None);
    }
}
pub(super) async fn open(
    factory: &IdbFactory,
    name: &str,
    seed: Option<Seed>,
) -> Result<IdbDatabase, BrowserError> {
    let request = factory
        .open_with_u32(name, 1)
        .map_err(|_| BrowserError::Unavailable)?;
    let create = seed.is_some();
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
                let seed = seed.as_ref().ok_or(BrowserError::MissingNamespace)?;
                let db = request
                    .result()
                    .map_err(|_| BrowserError::Unavailable)?
                    .dyn_into::<IdbDatabase>()
                    .map_err(|_| BrowserError::IncompatibleSchema)?;
                let store = db
                    .create_object_store(STORE)
                    .map_err(|_| BrowserError::Aborted)?;
                store
                    .add_with_key(seed.key.as_ref(), &JsValue::from_str("key"))
                    .map_err(|_| BrowserError::Aborted)?;
                store
                    .add_with_key(
                        &Uint8Array::from(seed.binding.as_slice()),
                        &JsValue::from_str("binding"),
                    )
                    .map_err(|_| BrowserError::Aborted)?;
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
        let failure = failure.clone();
        let created = created.clone();
        let reply = reply.clone();
        Closure::new(move |_: Event| {
            let result = (|| {
                let db = request
                    .result()
                    .map_err(|_| BrowserError::Unavailable)?
                    .dyn_into::<IdbDatabase>()
                    .map_err(|_| BrowserError::IncompatibleSchema)?;
                let checked = if let Some(error) = failure.get() {
                    Err(error)
                } else if create && !created.get() {
                    Err(BrowserError::NamespaceExists)
                } else {
                    schema(&db)
                };
                if let Err(error) = checked {
                    db.close();
                    return Err(error);
                }
                Ok(db)
            })();
            if let Some(send) = reply.borrow_mut().take() {
                let _ = send.send(result);
            }
        })
    };
    let error = {
        let request = request.clone();
        let failure = failure.clone();
        let reply = reply.clone();
        Closure::new(move |_: Event| {
            let error = failure.get().unwrap_or_else(|| {
                if request
                    .error()
                    .ok()
                    .flatten()
                    .is_some_and(|e| e.name() == "VersionError")
                {
                    BrowserError::IncompatibleSchema
                } else {
                    BrowserError::Aborted
                }
            });
            if let Some(send) = reply.borrow_mut().take() {
                let _ = send.send(Err(error));
            }
        })
    };
    request.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    request.set_onsuccess(Some(success.as_ref().unchecked_ref()));
    request.set_onerror(Some(error.as_ref().unchecked_ref()));
    let _roots = OpenRoots {
        request,
        _upgrade: upgrade,
        _success: success,
        _error: error,
    };
    observe.await.unwrap_or(Err(BrowserError::CommitUnknown))
}
fn schema(db: &IdbDatabase) -> Result<(), BrowserError> {
    if db.version() != 1.0
        || db.object_store_names().length() != 1
        || !db.object_store_names().contains(STORE)
    {
        return Err(BrowserError::IncompatibleSchema);
    }
    let tx = db
        .transaction_with_str(STORE)
        .map_err(|_| BrowserError::Unavailable)?;
    let store = tx
        .object_store(STORE)
        .map_err(|_| BrowserError::IncompatibleSchema)?;
    if store.auto_increment()
        || !store
            .key_path()
            .map_err(|_| BrowserError::IncompatibleSchema)?
            .is_null()
        || store.index_names().length() != 0
    {
        return Err(BrowserError::IncompatibleSchema);
    }
    Ok(())
}
struct Terminal {
    tx: IdbTransaction,
    observe: oneshot::Receiver<Result<(), BrowserError>>,
    _complete: Callback,
    _abort: Callback,
}
impl Terminal {
    fn new(tx: IdbTransaction) -> Self {
        let (send, observe) = oneshot::channel();
        let reply = Rc::new(RefCell::new(Some(send)));
        let complete = {
            let reply = reply.clone();
            Closure::new(move |_: Event| {
                if let Some(send) = reply.borrow_mut().take() {
                    let _ = send.send(Ok(()));
                }
            })
        };
        let abort = Closure::new(move |_: Event| {
            if let Some(send) = reply.borrow_mut().take() {
                let _ = send.send(Err(BrowserError::Aborted));
            }
        });
        tx.set_oncomplete(Some(complete.as_ref().unchecked_ref()));
        tx.set_onabort(Some(abort.as_ref().unchecked_ref()));
        Self {
            tx,
            observe,
            _complete: complete,
            _abort: abort,
        }
    }
    async fn wait(&mut self) -> Result<(), BrowserError> {
        (&mut self.observe)
            .await
            .unwrap_or(Err(BrowserError::CommitUnknown))
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        self.tx.set_oncomplete(None);
        self.tx.set_onabort(None);
    }
}
struct ReadRoot {
    request: IdbRequest,
    _success: Callback,
}
impl Drop for ReadRoot {
    fn drop(&mut self) {
        self.request.set_onsuccess(None);
    }
}
pub(super) async fn load(db: &IdbDatabase) -> Result<(CryptoKey, Vec<u8>), BrowserError> {
    let tx = db
        .transaction_with_str(STORE)
        .map_err(|_| BrowserError::Closed)?;
    let mut terminal = Terminal::new(tx.clone());
    let store = match tx.object_store(STORE) {
        Ok(store) => store,
        Err(_) => {
            let _ = tx.abort();
            terminal.wait().await?;
            return Err(BrowserError::IncompatibleSchema);
        }
    };
    let values = Rc::new(RefCell::new([None, None, None]));
    let mut roots = Vec::with_capacity(3);
    for index in 0..3 {
        let request = match index {
            0 => store.get(&JsValue::from_str("key")),
            1 => store.get(&JsValue::from_str("binding")),
            _ => store.count(),
        };
        let request = match request {
            Ok(request) => request,
            Err(_) => {
                let _ = tx.abort();
                terminal.wait().await?;
                return Err(BrowserError::IncompatibleSchema);
            }
        };
        let success = {
            let request = request.clone();
            let values = values.clone();
            let tx = tx.clone();
            Closure::new(move |_: Event| match request.result() {
                Ok(value) => values.borrow_mut()[index] = Some(value),
                Err(_) => {
                    let _ = tx.abort();
                }
            })
        };
        request.set_onsuccess(Some(success.as_ref().unchecked_ref()));
        roots.push(ReadRoot {
            request,
            _success: success,
        });
    }
    terminal.wait().await?;
    let mut values = values.borrow_mut();
    let key = values[0].take().ok_or(BrowserError::CommitUnknown)?;
    if key.is_undefined() {
        return Err(BrowserError::MissingKey);
    }
    let key = key
        .dyn_into::<CryptoKey>()
        .map_err(|_| BrowserError::KeyMismatch)?;
    let binding = values[1]
        .take()
        .ok_or(BrowserError::CommitUnknown)?
        .dyn_into::<Uint8Array>()
        .map_err(|_| BrowserError::KeyMismatch)?;
    if binding.length() > 170
        || binding.length() < 42
        || values[2].take().and_then(|v| v.as_f64()) != Some(2.0)
    {
        return Err(BrowserError::IncompatibleSchema);
    }
    Ok((key, binding.to_vec()))
}

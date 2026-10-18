//! Origin-scoped Web Lock, held until the actual database owner is finished.
use futures::channel::oneshot;
use js_sys::{Function, Object, Promise, Reflect};
use radroots_storage::workspace::WorkspaceError;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, future_to_promise, spawn_local};

pub(super) struct Lease {
    release: Option<oneshot::Sender<()>>,
    finished: Option<oneshot::Receiver<Result<(), WorkspaceError>>>,
    active: Rc<Cell<bool>>,
}
impl Lease {
    pub async fn acquire(name: &str) -> Result<Self, WorkspaceError> {
        let unavailable = |_| WorkspaceError::Unavailable;
        let navigator =
            Reflect::get(&js_sys::global(), &"navigator".into()).map_err(unavailable)?;
        let locks = Reflect::get(&navigator, &"locks".into()).map_err(unavailable)?;
        let request = Reflect::get(&locks, &"request".into())
            .map_err(unavailable)?
            .dyn_into::<Function>()
            .map_err(unavailable)?;
        let options = Object::new();
        Reflect::set(&options, &"mode".into(), &"exclusive".into()).map_err(unavailable)?;
        Reflect::set(&options, &"ifAvailable".into(), &JsValue::TRUE).map_err(unavailable)?;
        let (acquired, observe) = oneshot::channel();
        let acquired = Rc::new(RefCell::new(Some(acquired)));
        let (release, released) = oneshot::channel();
        let mut released = Some(released);
        let signal = acquired.clone();
        let active = Rc::new(Cell::new(false));
        let granted = active.clone();
        let callback = Closure::<dyn FnMut(JsValue) -> Promise>::new(move |lock: JsValue| {
            if lock.is_null() || lock.is_undefined() {
                if let Some(send) = signal.borrow_mut().take() {
                    let _ = send.send(Err(WorkspaceError::WriterActive));
                }
                return Promise::resolve(&JsValue::UNDEFINED);
            }
            granted.set(true);
            if let Some(send) = signal.borrow_mut().take() {
                let _ = send.send(Ok(()));
            }
            let wait = released.take();
            future_to_promise(async move {
                if let Some(wait) = wait {
                    let _ = wait.await;
                }
                Ok(JsValue::UNDEFINED)
            })
        });
        // The stable Web Locks API is called locally; no global unstable
        // web-sys compiler flag or public JavaScript object is required.
        let promise = request
            .call3(
                &locks,
                &JsValue::from_str(name),
                &options,
                callback.as_ref().unchecked_ref(),
            )
            .map_err(unavailable)?
            .dyn_into::<Promise>()
            .map_err(unavailable)?;
        let (done, finished) = oneshot::channel();
        let lifetime = active.clone();
        spawn_local(async move {
            let result = JsFuture::from(promise)
                .await
                .map(|_| ())
                .map_err(unavailable);
            lifetime.set(false);
            if let Some(send) = acquired.borrow_mut().take() {
                let _ = send.send(Err(WorkspaceError::Unavailable));
            }
            let _ = done.send(result);
            drop(callback);
        });
        let lease = Self {
            release: Some(release),
            finished: Some(finished),
            active,
        };
        observe.await.map_err(|_| WorkspaceError::Unavailable)??;
        Ok(lease)
    }
    pub fn active(&self) -> Rc<Cell<bool>> {
        self.active.clone()
    }
    pub async fn release(mut self) -> Result<(), WorkspaceError> {
        if let Some(send) = self.release.take() {
            let _ = send.send(());
        }
        match self.finished.take() {
            Some(done) => done.await.map_err(|_| WorkspaceError::Unavailable)?,
            None => Err(WorkspaceError::Unavailable),
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(send) = self.release.take() {
            let _ = send.send(());
        }
    }
}

//! Local disposal inventory only; the common session owns protocol membership.

use crate::state::Shared;
use futures::{future::poll_fn, lock::Mutex as AsyncMutex};
use radroots_transport::host::websocket::Error;
use std::{
    sync::{Arc, Mutex, Weak},
    task::Poll,
};

const MAX_SOCKET_OWNERS: usize = 64;

#[derive(Default)]
struct State {
    closed: bool,
    owners: Vec<Weak<Shared>>,
}

#[derive(Default)]
pub(crate) struct Scope {
    state: Mutex<State>,
    closing: AsyncMutex<()>,
}

impl Scope {
    pub(crate) fn register(&self, owner: &Arc<Shared>) -> Result<(), Error> {
        let mut state = self.state.lock().map_err(|_| Error::Closed)?;
        if state.closed {
            return Err(Error::Closed);
        }
        // Weak entries confer no lifetime or effect authority. Do not drop a
        // last strong owner or call foreign callbacks while holding this lock.
        state.owners.retain(|owner| owner.strong_count() != 0);
        if state.owners.len() == MAX_SOCKET_OWNERS {
            return Err(Error::Policy);
        }
        state.owners.push(Arc::downgrade(owner));
        Ok(())
    }

    pub(crate) async fn close(&self) -> Result<(), Error> {
        let _closing = self.closing.lock().await;
        let owners: Vec<_> = {
            let mut state = self.state.lock().map_err(|_| Error::Closed)?;
            state.closed = true;
            state.owners.iter().filter_map(Weak::upgrade).collect()
        };
        for owner in &owners {
            owner.stop();
        }
        for owner in &owners {
            poll_fn(|context| {
                owner.scope_closer.register(context.waker());
                let finished = owner.lock()?.finished;
                if finished {
                    owner.scope_closer.take();
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
            .await?;
        }
        self.state.lock().map_err(|_| Error::Closed)?.owners.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::AdmissionCounter;
    use futures::{
        FutureExt,
        task::{ArcWake, waker},
    };
    use std::{
        future::Future,
        sync::atomic::{AtomicUsize, Ordering},
        task::Context,
    };
    fn owner() -> Arc<Shared> {
        Arc::new(Shared::new(AdmissionCounter::new(true), 8))
    }

    #[test]
    fn empty_and_unpolled_close_are_inert_until_observed_and_then_permanent() {
        let scope = Scope::default();
        drop(scope.close());
        let socket = owner();
        scope.register(&socket).unwrap();
        socket.finish();
        futures::executor::block_on(scope.close()).unwrap();
        futures::executor::block_on(scope.close()).unwrap();
        assert_eq!(scope.register(&owner()), Err(Error::Closed));
        futures::executor::block_on(Scope::default().close()).unwrap();
    }

    #[test]
    fn registration_is_bounded_and_dropped_owners_release_inventory_credit() {
        let scope = Scope::default();
        let mut held = Vec::new();
        for _ in 0..MAX_SOCKET_OWNERS {
            let socket = owner();
            scope.register(&socket).unwrap();
            held.push(socket);
        }
        let extra = owner();
        assert_eq!(scope.register(&extra), Err(Error::Policy));
        held.pop();
        scope.register(&extra).unwrap();
        assert_eq!(scope.state.lock().unwrap().owners.len(), MAX_SOCKET_OWNERS);
    }

    #[test]
    fn cancelled_close_retains_all_original_owners_and_does_not_affect_a_sibling() {
        let scope = Scope::default();
        let sibling = Scope::default();
        let a = owner();
        let b = owner();
        let other = owner();
        scope.register(&a).unwrap();
        scope.register(&b).unwrap();
        sibling.register(&other).unwrap();
        let mut close = Box::pin(scope.close());
        assert!(close.as_mut().now_or_never().is_none());
        assert!(a.lock().unwrap().stopping && b.lock().unwrap().stopping);
        assert!(!other.lock().unwrap().stopping);
        assert_eq!(scope.register(&owner()), Err(Error::Closed));
        drop(close);
        a.finish();
        let mut retry = Box::pin(scope.close());
        assert!(retry.as_mut().now_or_never().is_none());
        b.finish();
        futures::executor::block_on(retry).unwrap();
        assert!(!other.lock().unwrap().stopping);
    }

    #[test]
    fn socket_and_scope_close_observers_have_independent_completion_wakes() {
        struct Count(AtomicUsize);
        impl ArcWake for Count {
            fn wake_by_ref(this: &Arc<Self>) {
                this.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let scope = Scope::default();
        let socket = owner();
        scope.register(&socket).unwrap();
        let scope_count = Arc::new(Count(AtomicUsize::new(0)));
        let socket_count = Arc::new(Count(AtomicUsize::new(0)));
        let scope_waker = waker(scope_count.clone());
        let socket_waker = waker(socket_count.clone());
        let mut close = Box::pin(scope.close());
        assert!(
            close
                .as_mut()
                .poll(&mut Context::from_waker(&scope_waker))
                .is_pending()
        );
        socket.closer.register(&socket_waker);
        socket.finish();
        assert!(scope_count.0.load(Ordering::SeqCst) > 0);
        assert!(socket_count.0.load(Ordering::SeqCst) > 0);
        futures::executor::block_on(close).unwrap();
    }

    #[test]
    fn poisoned_inventory_never_claims_disposal_or_admits_another_owner() {
        let scope = Scope::default();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = scope.state.lock().unwrap();
            panic!("intentional scope poison");
        }));
        assert_eq!(scope.register(&owner()), Err(Error::Closed));
        assert_eq!(
            futures::executor::block_on(scope.close()),
            Err(Error::Closed)
        );
    }
}

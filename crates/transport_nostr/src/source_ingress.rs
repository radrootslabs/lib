//! Bounded fetch admission at the shared socket's pre-Nostr decoding boundary.

use crate::source::budget::FetchBudget;
use futures::task::AtomicWaker;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    task::Context,
};

const MAX_ACTIVE_FETCHES: usize = 64;

#[derive(Debug, Default)]
struct RelayState {
    generation: u64,
    invalidated: u64,
    waker: AtomicWaker,
    write_waker: AtomicWaker,
}

#[derive(Debug)]
struct Registration {
    targets: BTreeSet<String>,
    budget: Arc<FetchBudget>,
}

#[derive(Debug, Default)]
struct State {
    sequence: u64,
    relays: BTreeMap<String, RelayState>,
    active: BTreeMap<u64, Registration>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct IngressRegistry(Arc<Mutex<State>>);

impl IngressRegistry {
    pub(crate) fn new(targets: impl Iterator<Item = String>) -> Self {
        Self(Arc::new(Mutex::new(State {
            relays: targets
                .map(|target| (target, RelayState::default()))
                .collect(),
            ..State::default()
        })))
    }

    pub(crate) fn register(
        &self,
        targets: impl Iterator<Item = String>,
        budget: Arc<FetchBudget>,
    ) -> Option<FetchRegistration> {
        let mut state = self.0.lock().ok()?;
        if state.active.len() == MAX_ACTIVE_FETCHES {
            return None;
        }
        let targets: BTreeSet<_> = targets.collect();
        if targets
            .iter()
            .any(|target| !state.relays.contains_key(target))
        {
            return None;
        }
        let id = state.sequence.checked_add(1)?;
        state.sequence = id;
        state.active.insert(id, Registration { targets, budget });
        Some(FetchRegistration {
            registry: self.clone(),
            id,
            finished: false,
        })
    }

    pub(crate) fn connection(&self, relay: &str) -> Option<IngressConnection> {
        let mut state = self.0.lock().ok()?;
        let row = state.relays.get_mut(relay)?;
        row.generation = row.generation.checked_add(1)?;
        row.waker.wake();
        row.write_waker.wake();
        Some(IngressConnection {
            registry: self.clone(),
            relay: relay.to_owned(),
            generation: row.generation,
        })
    }

    fn admitted(&self, relay: &str, generation: u64, context: &Context<'_>, write: bool) -> bool {
        let Ok(state) = self.0.lock() else {
            return false;
        };
        let Some(row) = state.relays.get(relay) else {
            return false;
        };
        if generation <= row.invalidated || generation != row.generation {
            return false;
        }
        if write {
            row.write_waker.register(context.waker());
        } else {
            row.waker.register(context.waker());
        }
        true
    }

    fn charge(
        &self,
        relay: &str,
        generation: u64,
        bytes: usize,
        frames: usize,
        data: usize,
    ) -> bool {
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        let Some(row) = state.relays.get(relay) else {
            return false;
        };
        if generation <= row.invalidated || generation != row.generation {
            return false;
        }
        let mut denied = BTreeSet::new();
        for entry in state.active.values() {
            if entry.targets.contains(relay) && !entry.budget.wire(bytes, frames, data) {
                denied.extend(entry.targets.iter().cloned());
            }
        }
        for target in &denied {
            if let Some(row) = state.relays.get_mut(target) {
                row.invalidated = row.generation;
                row.waker.wake();
                row.write_waker.wake();
            }
        }
        denied.is_empty()
    }

    fn remove(&self, id: u64, cancel: bool) -> bool {
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        let Some(entry) = state.active.remove(&id) else {
            return false;
        };
        // Charge and completion use this same mutex. No ingress can exhaust
        // this registration between the budget check and its removal.
        let complete = !cancel && !entry.budget.exhausted();
        if !complete {
            for target in entry.targets {
                if let Some(row) = state.relays.get_mut(&target) {
                    row.invalidated = row.generation;
                    row.waker.wake();
                    row.write_waker.wake();
                }
            }
        }
        complete
    }
}

pub(crate) struct FetchRegistration {
    registry: IngressRegistry,
    id: u64,
    finished: bool,
}

impl FetchRegistration {
    pub(crate) fn finish(mut self) -> bool {
        let complete = self.registry.remove(self.id, false);
        self.finished = true;
        complete
    }
}

impl Drop for FetchRegistration {
    fn drop(&mut self) {
        if !self.finished {
            self.registry.remove(self.id, true);
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct IngressConnection {
    registry: IngressRegistry,
    relay: String,
    generation: u64,
}

impl IngressConnection {
    pub(crate) fn admitted(&self, context: &Context<'_>) -> bool {
        self.registry
            .admitted(&self.relay, self.generation, context, false)
    }

    pub(crate) fn admitted_write(&self, context: &Context<'_>) -> bool {
        self.registry
            .admitted(&self.relay, self.generation, context, true)
    }

    pub(crate) fn charge(&self, bytes: usize, frames: usize, data: usize) -> bool {
        self.registry
            .charge(&self.relay, self.generation, bytes, frames, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::budget::{MAX_FETCH_BYTES, MAX_FETCH_EVENTS, MAX_FETCH_NOTIFICATIONS};

    fn registry() -> IngressRegistry {
        IngressRegistry::new(["one".to_owned(), "two".to_owned()].into_iter())
    }

    fn register(
        registry: &IngressRegistry,
        targets: &[&str],
        budget: &Arc<FetchBudget>,
    ) -> FetchRegistration {
        registry
            .register(
                targets.iter().map(|target| (*target).to_owned()),
                Arc::clone(budget),
            )
            .unwrap()
    }

    fn admitted(connection: &IngressConnection) -> bool {
        connection.admitted(&Context::from_waker(futures::task::noop_waker_ref()))
    }

    #[test]
    fn cancellation_closes_the_exact_generation_and_a_new_fetch_can_reconnect() {
        let registry = registry();
        let budget = Arc::new(FetchBudget::default());
        let registration = register(&registry, &["one"], &budget);
        let first = registry.connection("one").unwrap();
        assert!(admitted(&first));
        drop(registration);
        assert!(!admitted(&first));
        assert!(!first.charge(1, 1, 1));
        let fresh = register(&registry, &["one"], &Arc::new(FetchBudget::default()));
        let second = registry.connection("one").unwrap();
        assert!(admitted(&second));
        assert!(second.charge(1, 1, 1));
        assert!(!admitted(&first));
        fresh.finish();
        assert!(admitted(&second));
        assert!(second.charge(MAX_FETCH_BYTES + 1, 0, 0));
    }

    #[test]
    fn all_batches_and_reconnections_share_one_monotonic_budget() {
        let registry = registry();
        let budget = Arc::new(FetchBudget::default());
        let registration = register(&registry, &["one", "two"], &budget);
        let first = registry.connection("one").unwrap();
        assert!(first.charge(MAX_FETCH_BYTES / 2, 1, 1));
        let retried = registry.connection("one").unwrap();
        assert!(!admitted(&first));
        assert!(!first.charge(1, 0, 0));
        assert!(retried.charge(MAX_FETCH_BYTES / 2, 1, 1));
        let second = registry.connection("two").unwrap();
        assert!(!second.charge(1, 0, 0));
        assert!(budget.exhausted());
        assert!(!admitted(&retried));
        assert!(!admitted(&second));
        drop(registration);
        assert!(!admitted(&retried));
    }

    #[test]
    fn overlapping_fetches_each_charge_shared_traffic_and_unselected_relays_do_not() {
        let registry = registry();
        let first_budget = Arc::new(FetchBudget::default());
        let second_budget = Arc::new(FetchBudget::default());
        let first = register(&registry, &["one"], &first_budget);
        let second = register(&registry, &["one"], &second_budget);
        let other = registry.connection("two").unwrap();
        assert!(other.charge(
            MAX_FETCH_BYTES + 1,
            MAX_FETCH_NOTIFICATIONS + 1,
            MAX_FETCH_EVENTS + 1
        ));
        let connection = registry.connection("one").unwrap();
        assert!(connection.charge(MAX_FETCH_BYTES, 0, 0));
        first.finish();
        assert!(!connection.charge(1, 0, 0));
        assert!(!first_budget.exhausted());
        assert!(second_budget.exhausted());
        assert!(admitted(&other));
        drop(second);
    }

    #[test]
    fn registration_capacity_unknown_targets_and_counter_overflow_fail_closed() {
        let registry = registry();
        let budget = Arc::new(FetchBudget::default());
        assert!(
            registry
                .register(["unknown".to_owned()].into_iter(), Arc::clone(&budget))
                .is_none()
        );
        assert!(registry.connection("unknown").is_none());
        let mut registrations = (0..MAX_ACTIVE_FETCHES)
            .map(|_| register(&registry, &["one"], &budget))
            .collect::<Vec<_>>();
        assert!(
            registry
                .register(["one".to_owned()].into_iter(), Arc::clone(&budget))
                .is_none()
        );
        registrations.pop().unwrap().finish();
        register(&registry, &["one"], &budget).finish();
        for registration in registrations {
            registration.finish();
        }
        registry.0.lock().unwrap().sequence = u64::MAX;
        assert!(
            registry
                .register(["one".to_owned()].into_iter(), budget)
                .is_none()
        );
        registry
            .0
            .lock()
            .unwrap()
            .relays
            .get_mut("one")
            .unwrap()
            .generation = u64::MAX;
        assert!(registry.connection("one").is_none());
    }

    #[test]
    fn cancel_notifies_the_pending_reader() {
        struct Wake(std::sync::atomic::AtomicBool);
        impl futures::task::ArcWake for Wake {
            fn wake_by_ref(arc_self: &Arc<Self>) {
                arc_self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let registry = registry();
        let registration = register(&registry, &["one"], &Arc::new(FetchBudget::default()));
        let connection = registry.connection("one").unwrap();
        let wake = Arc::new(Wake(std::sync::atomic::AtomicBool::new(false)));
        let waker = futures::task::waker(Arc::clone(&wake));
        assert!(connection.admitted(&Context::from_waker(&waker)));
        drop(registration);
        assert!(wake.0.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!admitted(&connection));
    }

    #[test]
    fn a_stale_generation_cannot_steal_live_reader_or_writer_wakeups() {
        struct Wake(std::sync::atomic::AtomicBool);
        impl futures::task::ArcWake for Wake {
            fn wake_by_ref(arc_self: &Arc<Self>) {
                arc_self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let registry = registry();
        let registration = register(&registry, &["one"], &Arc::new(FetchBudget::default()));
        let stale = registry.connection("one").unwrap();
        let current = registry.connection("one").unwrap();
        let read = Arc::new(Wake(std::sync::atomic::AtomicBool::new(false)));
        let write = Arc::new(Wake(std::sync::atomic::AtomicBool::new(false)));
        let read_waker = futures::task::waker(Arc::clone(&read));
        let write_waker = futures::task::waker(Arc::clone(&write));
        assert!(current.admitted(&Context::from_waker(&read_waker)));
        assert!(current.admitted_write(&Context::from_waker(&write_waker)));
        assert!(!stale.admitted(&Context::from_waker(futures::task::noop_waker_ref())));
        assert!(!stale.admitted_write(&Context::from_waker(futures::task::noop_waker_ref())));
        drop(registration);
        assert!(read.0.load(std::sync::atomic::Ordering::SeqCst));
        assert!(write.0.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn finalization_and_ingress_have_one_ordered_completion_boundary() {
        for ingress_first in [false, true] {
            let registry = registry();
            let budget = Arc::new(FetchBudget::default());
            let registration = register(&registry, &["one"], &budget);
            let connection = registry.connection("one").unwrap();
            assert!(connection.charge(MAX_FETCH_BYTES, 0, 0));
            if ingress_first {
                assert!(!connection.charge(1, 0, 0));
                assert!(!registration.finish());
                assert!(budget.exhausted());
                assert!(!admitted(&connection));
            } else {
                assert!(registration.finish());
                assert!(connection.charge(1, 0, 0));
                assert!(!budget.exhausted());
                assert!(admitted(&connection));
            }
            assert!(registry.0.lock().unwrap().active.is_empty());
        }
    }

    #[test]
    fn racing_completion_and_last_byte_cannot_both_claim_admission() {
        for _ in 0..32 {
            let registry = registry();
            let budget = Arc::new(FetchBudget::default());
            let registration = register(&registry, &["one"], &budget);
            let connection = registry.connection("one").unwrap();
            assert!(connection.charge(MAX_FETCH_BYTES, 0, 0));
            let barrier = std::sync::Barrier::new(2);
            let (complete, admitted_after_limit) = std::thread::scope(|scope| {
                let finish = scope.spawn(|| {
                    barrier.wait();
                    registration.finish()
                });
                barrier.wait();
                let admitted_after_limit = connection.charge(1, 0, 0);
                (finish.join().unwrap(), admitted_after_limit)
            });
            assert_eq!(complete, admitted_after_limit);
            assert_eq!(budget.exhausted(), !complete);
        }
    }

    #[test]
    fn missing_or_poisoned_registration_cannot_finalize_successfully() {
        let registry = registry();
        let budget = Arc::new(FetchBudget::default());
        let missing = register(&registry, &["one"], &budget);
        registry.0.lock().unwrap().active.remove(&missing.id);
        assert!(!missing.finish());
        let poisoned = register(&registry, &["one"], &budget);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = registry.0.lock().unwrap();
            panic!("poison the registration lock");
        }));
        assert!(!poisoned.finish());
    }

    #[test]
    fn missing_or_poisoned_connection_state_refuses_both_io_directions_and_charging() {
        let registry = registry();
        let missing = IngressConnection {
            registry: registry.clone(),
            relay: "missing".into(),
            generation: 1,
        };
        let context = Context::from_waker(futures::task::noop_waker_ref());
        assert!(!missing.admitted(&context));
        assert!(!missing.admitted_write(&context));
        assert!(!missing.charge(0, 0, 0));
        let current = registry.connection("one").unwrap();
        assert!(current.admitted(&context));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = registry.0.lock().unwrap();
            panic!("injected connection registry failure");
        }));
        assert!(!current.admitted(&context));
        assert!(!current.admitted_write(&context));
        assert!(!current.charge(1, 1, 1));
        assert!(registry.connection("one").is_none());
        assert!(
            registry
                .register(
                    ["one".to_owned()].into_iter(),
                    Arc::new(FetchBudget::default())
                )
                .is_none()
        );
    }
}

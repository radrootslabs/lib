//! Bounded fetch admission at the shared socket's pre-Nostr decoding boundary.

use crate::source::budget::FetchBudget;
use futures::task::AtomicWaker;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    task::Context,
};

const MAX_ACTIVE_SCOPES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopeMode {
    Fetch,
    Live,
}

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
    originals: Vec<OriginalScope>,
    mode: ScopeMode,
}

#[derive(Debug)]
struct OriginalScope {
    relay: String,
    subscription: String,
    scope: u64,
}

#[derive(Debug, Default)]
struct State {
    sequence: u64,
    original_sequence: u64,
    relays: BTreeMap<String, RelayState>,
    active: BTreeMap<u64, Registration>,
    originals: crate::source_original::OriginalQueue,
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
    ) -> Option<IngressRegistration> {
        self.register_scope(targets, budget, ScopeMode::Fetch)
    }

    pub(crate) fn register_live(
        &self,
        targets: impl Iterator<Item = String>,
    ) -> Option<IngressRegistration> {
        self.register_scope(targets, Arc::new(FetchBudget::default()), ScopeMode::Live)
    }

    fn register_scope(
        &self,
        targets: impl Iterator<Item = String>,
        budget: Arc<FetchBudget>,
        mode: ScopeMode,
    ) -> Option<IngressRegistration> {
        let mut state = self.0.lock().ok()?;
        if state.active.len() == MAX_ACTIVE_SCOPES {
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
        state.active.insert(
            id,
            Registration {
                targets,
                budget,
                originals: Vec::new(),
                mode,
            },
        );
        Some(IngressRegistration {
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
            if entry.mode == ScopeMode::Fetch
                && entry.targets.contains(relay)
                && !entry.budget.wire(bytes, frames, data)
            {
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
        state.originals.remove_owner(id);
        // Charge and completion use this same mutex. No ingress can exhaust
        // this registration between the budget check and its removal.
        let complete = !cancel && !entry.budget.exhausted();
        if !complete && entry.mode == ScopeMode::Fetch {
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

    fn bind_originals(
        &self,
        owner: u64,
        relay: &str,
        subscription: &str,
    ) -> Option<OriginalSubscription> {
        if subscription.is_empty() || subscription.len() > 256 {
            return None;
        }
        let mut state = self.0.lock().ok()?;
        if state.active.values().any(|entry| {
            entry
                .originals
                .iter()
                .any(|scope| scope.relay == relay && scope.subscription == subscription)
        }) {
            return None;
        }
        let entry = state.active.get(&owner)?;
        if !entry.targets.contains(relay) || entry.originals.len() >= crate::client::MAX_RELAYS {
            return None;
        }
        let scope = state.original_sequence.checked_add(1)?;
        state.original_sequence = scope;
        state.active.get_mut(&owner)?.originals.push(OriginalScope {
            relay: relay.into(),
            subscription: subscription.into(),
            scope,
        });
        Some(OriginalSubscription {
            registry: self.clone(),
            owner,
            scope,
        })
    }

    fn capture(&self, relay: &str, generation: u64, text: &str) -> bool {
        let Some((subscription, raw)) = crate::source_original::frame(text) else {
            return true;
        };
        let selected = {
            let Ok(state) = self.0.lock() else {
                return false;
            };
            if !state
                .relays
                .get(relay)
                .is_some_and(|row| generation == row.generation && generation > row.invalidated)
            {
                return false;
            }
            state.active.iter().find_map(|(owner, entry)| {
                entry
                    .originals
                    .iter()
                    .find(|scope| scope.relay == relay && scope.subscription == subscription)
                    .map(|scope| (*owner, scope.scope))
            })
        };
        let Some((owner, scope)) = selected else {
            return true;
        };
        // Reuse the exact pinned upstream decoder only to correlate its later
        // notification. It supplies no authentication or original-wire admission.
        let Ok(event) = nostr_sdk::prelude::Event::from_json(raw.get()) else {
            return true;
        };
        let correlation = crate::source_original::correlation(&event);
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        if !state
            .relays
            .get(relay)
            .is_some_and(|row| generation == row.generation && generation > row.invalidated)
        {
            return false;
        }
        let Some(entry) = state.active.get(&owner) else {
            return true;
        };
        if !entry.originals.iter().any(|entry| entry.scope == scope) {
            return true;
        }
        if entry.budget.exhausted() {
            return entry.mode == ScopeMode::Live;
        }
        if state.originals.insert(owner, scope, correlation, raw.get()) {
            return true;
        }
        let entry = &state.active[&owner];
        entry.budget.exhaust();
        if entry.mode == ScopeMode::Live {
            return true;
        }
        let targets = entry.targets.clone();
        for target in targets {
            if let Some(row) = state.relays.get_mut(&target) {
                row.invalidated = row.generation;
                row.waker.wake();
                row.write_waker.wake();
            }
        }
        false
    }
}

pub(crate) struct IngressRegistration {
    registry: IngressRegistry,
    id: u64,
    finished: bool,
}

impl IngressRegistration {
    pub(crate) async fn wait_exhausted(&self) {
        let budget = self.registry.0.lock().ok().and_then(|state| {
            state
                .active
                .get(&self.id)
                .map(|entry| Arc::clone(&entry.budget))
        });
        if let Some(budget) = budget {
            budget.wait_exhausted().await;
        }
    }
    pub(crate) fn exhausted(&self) -> bool {
        self.registry
            .0
            .lock()
            .ok()
            .and_then(|state| {
                state
                    .active
                    .get(&self.id)
                    .map(|entry| entry.budget.exhausted())
            })
            .unwrap_or(true)
    }
    pub(crate) fn bind_originals(
        &self,
        relay: &str,
        subscription: &str,
    ) -> Option<OriginalSubscription> {
        self.registry.bind_originals(self.id, relay, subscription)
    }
    pub(crate) fn finish(mut self) -> bool {
        let complete = self.registry.remove(self.id, false);
        self.finished = true;
        complete
    }
}

pub(crate) struct OriginalSubscription {
    registry: IngressRegistry,
    owner: u64,
    scope: u64,
}
impl OriginalSubscription {
    pub(crate) fn take(&self, event: &nostr_sdk::prelude::Event) -> Option<String> {
        let key = crate::source_original::correlation(event);
        self.registry.0.lock().ok()?.originals.take(self.scope, key)
    }
}
impl Drop for OriginalSubscription {
    fn drop(&mut self) {
        if let Ok(mut state) = self.registry.0.lock() {
            state.originals.remove_scope(self.scope);
            if let Some(entry) = state.active.get_mut(&self.owner) {
                entry.originals.retain(|entry| entry.scope != self.scope);
            }
        }
    }
}

impl Drop for IngressRegistration {
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
    pub(crate) fn capture(&self, text: &str) -> bool {
        self.registry.capture(&self.relay, self.generation, text)
    }
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

    fn original() -> (String, nostr_sdk::prelude::Event) {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/source_original.v1.json"))
                .unwrap();
        let raw = fixture["raw_wrap"].as_str().unwrap().to_owned();
        let event = nostr_sdk::prelude::Event::from_json(&raw).unwrap();
        (raw, event)
    }
    fn event_frame(subscription: &str, raw: &str) -> String {
        format!(
            "[\"EVENT\",{},{}]",
            serde_json::to_string(subscription).unwrap(),
            raw
        )
    }

    #[test]
    fn original_scope_refusals_and_unowned_frames_retain_no_evidence() {
        let registry = registry();
        let registration = register(&registry, &["one"], &Arc::new(FetchBudget::default()));
        assert!(
            registration
                .bind_originals("one", &"a".repeat(257))
                .is_none()
        );
        let mut scopes = Vec::new();
        for index in 0..crate::client::MAX_RELAYS {
            scopes.push(
                registration
                    .bind_originals("one", &format!("scope-{index}"))
                    .unwrap(),
            );
        }
        assert!(registration.bind_originals("one", "over-limit").is_none());
        let connection = registry.connection("one").unwrap();
        let (raw, event) = original();
        assert!(connection.capture(&event_frame("unowned", &raw)));
        assert!(connection.capture(&event_frame("scope-0", "{}")));
        assert!(scopes[0].take(&event).is_none());
        assert!(connection.capture(&event_frame("scope-0", &raw)));
        assert_eq!(scopes[0].take(&event).as_deref(), Some(raw.as_str()));
        assert!(registration.finish());
        assert!(scopes[0].take(&event).is_none());
    }

    #[test]
    fn originals_are_owned_by_exact_fetch_relay_and_subscription() {
        let registry = registry();
        let first = register(&registry, &["one"], &Arc::new(FetchBudget::default()));
        let second = register(&registry, &["two"], &Arc::new(FetchBudget::default()));
        let a = first.bind_originals("one", "a").unwrap();
        let b = second.bind_originals("two", "a").unwrap();
        assert!(first.bind_originals("one", "a").is_none());
        assert!(first.bind_originals("missing", "b").is_none());
        assert!(first.bind_originals("one", "").is_none());
        let one = registry.connection("one").unwrap();
        let two = registry.connection("two").unwrap();
        let (raw, event) = original();
        assert!(one.capture(&event_frame("a", &raw)));
        assert!(b.take(&event).is_none());
        assert!(two.capture(&event_frame("a", &raw)));
        assert_eq!(a.take(&event).as_deref(), Some(raw.as_str()));
        assert_eq!(b.take(&event).as_deref(), Some(raw.as_str()));
        assert!(a.take(&event).is_none());
        assert!(first.finish());
        assert!(second.finish());
    }

    #[test]
    fn original_scope_cleanup_and_reconnection_preserve_obtained_data_and_reject_stale_ingress() {
        let registry = registry();
        let first = register(&registry, &["one"], &Arc::new(FetchBudget::default()));
        let scope = first.bind_originals("one", "a").unwrap();
        let stale = registry.connection("one").unwrap();
        let (raw, event) = original();
        assert!(stale.capture(&event_frame("a", &raw)));
        let fresh = registry.connection("one").unwrap();
        assert!(!stale.capture(&event_frame("a", &raw)));
        assert_eq!(scope.take(&event).as_deref(), Some(raw.as_str()));
        assert!(fresh.capture(&event_frame("a", &raw)));
        drop(scope);
        assert_eq!(
            registry.0.lock().unwrap().originals.retained_bytes(),
            crate::source_original::OriginalQueue::default().retained_bytes()
        );
        assert!(fresh.capture(&event_frame("a", &raw)));
        assert!(first.finish());
        registry.0.lock().unwrap().original_sequence = u64::MAX;
        let second = register(&registry, &["one"], &Arc::new(FetchBudget::default()));
        assert!(second.bind_originals("one", "b").is_none());
    }

    #[test]
    fn original_queue_is_globally_bounded_and_exhaustion_does_not_evict_other_fetch_data() {
        let registry = registry();
        let a_budget = Arc::new(FetchBudget::default());
        let b_budget = Arc::new(FetchBudget::default());
        let a = register(&registry, &["one"], &a_budget);
        let b = register(&registry, &["two"], &b_budget);
        let a_scope = a.bind_originals("one", "a").unwrap();
        let b_scope = b.bind_originals("two", "b").unwrap();
        let one = registry.connection("one").unwrap();
        let two = registry.connection("two").unwrap();
        let (raw, event) = original();
        assert!(
            raw.len() * crate::source_original::MAX_PENDING_EVENTS
                + registry.0.lock().unwrap().originals.retained_bytes()
                <= crate::source_original::MAX_PENDING_BYTES
        );
        for _ in 0..crate::source_original::MAX_PENDING_EVENTS / 2 {
            assert!(one.capture(&event_frame("a", &raw)));
            assert!(two.capture(&event_frame("b", &raw)));
        }
        assert!(!one.capture(&event_frame("a", &raw)));
        assert!(a_budget.exhausted());
        assert!(!b_budget.exhausted());
        assert!(!admitted(&one));
        assert!(admitted(&two));
        assert_eq!(b_scope.take(&event).as_deref(), Some(raw.as_str()));
        assert!(two.capture(&event_frame("b", &raw)));
        assert_eq!(a_scope.take(&event).as_deref(), Some(raw.as_str()));
        drop(a);
        assert!(b_scope.take(&event).is_some());
        assert!(b.finish());
        assert_eq!(
            registry.0.lock().unwrap().originals.retained_bytes(),
            crate::source_original::OriginalQueue::default().retained_bytes()
        );
    }

    #[test]
    fn live_scope_cleanup_preserves_other_subscriptions_and_has_no_cumulative_fetch_budget() {
        let registry = registry();
        let first = registry
            .register_live(["one".to_owned()].into_iter())
            .unwrap();
        let second = registry
            .register_live(["one".to_owned()].into_iter())
            .unwrap();
        let a = first.bind_originals("one", "a").unwrap();
        let b = second.bind_originals("one", "b").unwrap();
        let connection = registry.connection("one").unwrap();
        assert!(connection.charge(
            MAX_FETCH_BYTES + 1,
            MAX_FETCH_NOTIFICATIONS + 1,
            MAX_FETCH_EVENTS + 1
        ));
        assert!(!first.exhausted());
        assert!(!second.exhausted());
        let (raw, event) = original();
        assert!(connection.capture(&event_frame("a", &raw)));
        assert!(connection.capture(&event_frame("b", &raw)));
        drop(a);
        drop(first);
        assert!(admitted(&connection));
        assert_eq!(b.take(&event).as_deref(), Some(raw.as_str()));
        assert!(!second.exhausted());
        drop(second);
        assert!(b.take(&event).is_none());
        assert!(admitted(&connection));
    }

    #[tokio::test]
    async fn live_queue_loss_wakes_a_pending_observer_without_closing_another_scope() {
        let registry = registry();
        let live = registry
            .register_live(["one".to_owned()].into_iter())
            .unwrap();
        let other = registry
            .register_live(["one".to_owned()].into_iter())
            .unwrap();
        let scope = live.bind_originals("one", "a").unwrap();
        let _other_scope = other.bind_originals("one", "b").unwrap();
        let connection = registry.connection("one").unwrap();
        let (raw, event) = original();
        let wait = live.wait_exhausted();
        futures::pin_mut!(wait);
        assert!(futures::poll!(wait.as_mut()).is_pending());
        for _ in 0..crate::source_original::MAX_PENDING_EVENTS {
            assert!(connection.capture(&event_frame("a", &raw)));
        }
        assert!(connection.capture(&event_frame("a", &raw)));
        tokio::time::timeout(std::time::Duration::from_secs(1), wait)
            .await
            .unwrap();
        assert!(live.exhausted());
        assert!(!other.exhausted());
        assert!(admitted(&connection));
        assert_eq!(scope.take(&event).as_deref(), Some(raw.as_str()));
        // A lost scope cannot resume admission after unrelated dequeueing.
        assert!(connection.capture(&event_frame("a", &raw)));
        assert!(live.exhausted());
    }

    fn registry() -> IngressRegistry {
        IngressRegistry::new(["one".to_owned(), "two".to_owned()].into_iter())
    }

    fn register(
        registry: &IngressRegistry,
        targets: &[&str],
        budget: &Arc<FetchBudget>,
    ) -> IngressRegistration {
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
        let mut registrations = (0..MAX_ACTIVE_SCOPES)
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

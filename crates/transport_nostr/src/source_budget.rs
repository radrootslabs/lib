use std::sync::Mutex;

pub(crate) const MAX_EVENT_BYTES: usize = radroots_event_codec::decode::MAX_EVENT_JSON_BYTES;
pub(crate) const MAX_FETCH_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_FETCH_EVENTS: usize = 4096;
pub(crate) const MAX_FETCH_NOTIFICATIONS: usize = 8192;

#[derive(Debug, Default)]
struct Usage {
    bytes: usize,
    events: usize,
    notifications: usize,
    wire_bytes: usize,
    wire_messages: usize,
    wire_data: usize,
    exhausted: bool,
}

/// One monotonic inventory shared by all relay batches. Reservations are not
/// refunded when a duplicate, malformed event or completed batch is discarded.
#[derive(Debug, Default)]
pub(crate) struct FetchBudget(Mutex<Usage>, tokio::sync::Notify);

impl FetchBudget {
    pub(crate) fn wire(&self, bytes: usize, frames: usize, data: usize) -> bool {
        let Ok(mut usage) = self.0.lock() else {
            return false;
        };
        if usage.exhausted
            || bytes > MAX_FETCH_BYTES - usage.wire_bytes
            || frames > MAX_FETCH_NOTIFICATIONS - usage.wire_messages
            || data > MAX_FETCH_EVENTS - usage.wire_data
        {
            usage.exhausted = true;
            self.1.notify_waiters();
            return false;
        }
        usage.wire_bytes += bytes;
        usage.wire_messages += frames;
        usage.wire_data += data;
        true
    }

    pub(crate) fn exhausted(&self) -> bool {
        self.0.lock().map_or(true, |usage| usage.exhausted)
    }

    pub(super) async fn wait_exhausted(&self) {
        loop {
            let notified = self.1.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.exhausted() {
                return;
            }
            notified.await;
        }
    }

    pub(super) fn notification(&self) -> bool {
        let Ok(mut usage) = self.0.lock() else {
            return false;
        };
        if usage.exhausted || usage.notifications == MAX_FETCH_NOTIFICATIONS {
            usage.exhausted = true;
            self.1.notify_waiters();
            return false;
        }
        usage.notifications += 1;
        true
    }

    pub(super) fn event(&self, bytes: usize) -> bool {
        let Ok(mut usage) = self.0.lock() else {
            return false;
        };
        if usage.exhausted
            || bytes > MAX_EVENT_BYTES
            || usage.events == MAX_FETCH_EVENTS
            || bytes > MAX_FETCH_BYTES - usage.bytes
        {
            usage.exhausted = true;
            self.1.notify_waiters();
            return false;
        }
        usage.events += 1;
        usage.bytes += bytes;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_maximum_is_accepted_and_the_next_unit_is_rejected() {
        let bytes = FetchBudget::default();
        assert!(!FetchBudget::default().event(MAX_EVENT_BYTES + 1));
        for _ in 0..MAX_FETCH_BYTES / MAX_EVENT_BYTES {
            assert!(bytes.event(MAX_EVENT_BYTES));
        }
        assert!(!bytes.event(1));
        let events = FetchBudget::default();
        for _ in 0..MAX_FETCH_EVENTS {
            assert!(events.event(1));
        }
        assert!(!events.event(1));
        let notifications = FetchBudget::default();
        for _ in 0..MAX_FETCH_NOTIFICATIONS {
            assert!(notifications.notification());
        }
        assert!(!notifications.notification());
    }

    #[test]
    fn competing_relays_cannot_overreserve_the_aggregate_budget() {
        let budget = FetchBudget::default();
        let accepted = std::thread::scope(|scope| {
            let tasks = (0..8)
                .map(|_| scope.spawn(|| (0..32).filter(|_| budget.event(MAX_EVENT_BYTES)).count()))
                .collect::<Vec<_>>();
            tasks
                .into_iter()
                .map(|task| task.join().unwrap())
                .sum::<usize>()
        });
        assert_eq!(accepted * MAX_EVENT_BYTES, MAX_FETCH_BYTES);
        assert!(!budget.event(1));
    }

    #[tokio::test]
    async fn raw_bytes_frames_and_data_attempts_have_sticky_independent_limits() {
        for (bytes, frames, data) in [
            (MAX_FETCH_BYTES, 0, 0),
            (0, MAX_FETCH_NOTIFICATIONS, 0),
            (0, 0, MAX_FETCH_EVENTS),
        ] {
            let budget = FetchBudget::default();
            assert!(budget.wire(bytes, frames, data));
            assert!(!budget.exhausted());
            assert!(!budget.wire(
                usize::from(bytes > 0),
                usize::from(frames > 0),
                usize::from(data > 0)
            ));
            assert!(budget.exhausted());
            assert!(!budget.wire(0, 0, 0));
            assert!(!budget.event(0));
            assert!(!budget.notification());
            tokio::time::timeout(std::time::Duration::from_secs(1), budget.wait_exhausted())
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn exhaustion_wakes_every_waiter_without_lost_notifications() {
        let budget = FetchBudget::default();
        let first = budget.wait_exhausted();
        let second = budget.wait_exhausted();
        tokio::pin!(first, second);
        assert!(futures::poll!(&mut first).is_pending());
        assert!(futures::poll!(&mut second).is_pending());
        assert!(!budget.wire(MAX_FETCH_BYTES + 1, 0, 0));
        assert!(futures::poll!(&mut first).is_ready());
        assert!(futures::poll!(&mut second).is_ready());
    }

    #[test]
    fn poisoned_budget_refuses_every_inventory_entry_without_resetting_authority() {
        let budget = FetchBudget::default();
        assert!(budget.wire(1, 1, 1));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = budget.0.lock().unwrap();
            panic!("injected budget lock failure");
        }));
        assert!(budget.exhausted());
        assert!(!budget.wire(0, 0, 0));
        assert!(!budget.notification());
        assert!(!budget.event(0));
        futures::executor::block_on(budget.wait_exhausted());
    }
}

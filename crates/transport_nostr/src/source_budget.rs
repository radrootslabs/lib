use std::sync::Mutex;

pub(super) const MAX_EVENT_BYTES: usize = radroots_event_codec::decode::MAX_EVENT_JSON_BYTES;
pub(super) const MAX_FETCH_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_FETCH_EVENTS: usize = 4096;
pub(super) const MAX_FETCH_NOTIFICATIONS: usize = 8192;

#[derive(Debug, Default)]
struct Usage {
    bytes: usize,
    events: usize,
    notifications: usize,
}

/// One monotonic inventory shared by all relay batches. Reservations are not
/// refunded when a duplicate, malformed event or completed batch is discarded.
#[derive(Debug, Default)]
pub(super) struct FetchBudget(Mutex<Usage>);

impl FetchBudget {
    pub(super) fn notification(&self) -> bool {
        let Ok(mut usage) = self.0.lock() else {
            return false;
        };
        if usage.notifications == MAX_FETCH_NOTIFICATIONS {
            return false;
        }
        usage.notifications += 1;
        true
    }

    pub(super) fn event(&self, bytes: usize) -> bool {
        if bytes > MAX_EVENT_BYTES {
            return false;
        }
        let Ok(mut usage) = self.0.lock() else {
            return false;
        };
        if usage.events == MAX_FETCH_EVENTS || bytes > MAX_FETCH_BYTES - usage.bytes {
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
        assert!(!bytes.event(MAX_EVENT_BYTES + 1));
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
}

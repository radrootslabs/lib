//! Host clocks and deadlines shared by the Nostr session adapters.

use core::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use tokio::time::{Instant, timeout, timeout_at};
#[cfg(target_arch = "wasm32")]
#[path = "host_time_browser.rs"]
mod browser;
#[cfg(target_arch = "wasm32")]
pub(crate) use browser::{Instant, timeout, timeout_at};

#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) fn unix_time_ms() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    use std::time::{SystemTime, UNIX_EPOCH};
    #[cfg(target_arch = "wasm32")]
    use web_time::{SystemTime, UNIX_EPOCH};

    wall_millis(SystemTime::now().duration_since(UNIX_EPOCH).ok())
}

fn wall_millis(since_epoch: Option<Duration>) -> u64 {
    since_epoch
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use core::{cell::Cell, future::pending};
    use futures::FutureExt;
    use std::rc::Rc;

    struct OnDrop(Rc<Cell<bool>>);

    impl Drop for OnDrop {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    #[test]
    fn wall_clock_conversion_preserves_unavailable_and_saturated_values() {
        assert_eq!(wall_millis(None), 0);
        assert_eq!(wall_millis(Some(Duration::ZERO)), 0);
        assert_eq!(wall_millis(Some(Duration::from_micros(1999))), 1);
        assert_eq!(wall_millis(Some(Duration::MAX)), u64::MAX);
    }

    #[tokio::test(start_paused = true)]
    async fn ready_local_future_wins_even_at_an_expired_deadline() {
        let local = Rc::new(Cell::new(7));
        let deadline = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            timeout_at(deadline, async { local.get() }).await.unwrap(),
            7
        );
    }

    #[tokio::test(start_paused = true)]
    async fn expiry_drops_pending_local_work() {
        let dropped = Rc::new(Cell::new(false));
        let guard = OnDrop(Rc::clone(&dropped));
        assert!(
            timeout(Duration::from_millis(10), async move {
                let _guard = guard;
                pending::<()>().await;
            })
            .await
            .is_err()
        );
        assert!(dropped.get());
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_drops_work_and_a_fresh_timeout_still_runs() {
        let dropped = Rc::new(Cell::new(false));
        let guard = OnDrop(Rc::clone(&dropped));
        assert!(
            timeout(Duration::from_secs(1), async move {
                let _guard = guard;
                pending::<()>().await;
            })
            .now_or_never()
            .is_none()
        );
        assert!(dropped.get());
        assert_eq!(
            timeout(Duration::from_secs(1), async { 9 }).await.unwrap(),
            9
        );
    }

    #[tokio::test(start_paused = true)]
    async fn queued_work_consumes_one_original_deadline() {
        let started = Instant::now();
        let deadline = started + Duration::from_millis(20);
        tokio::time::sleep(Duration::from_millis(15)).await;
        assert!(timeout_at(deadline, pending::<()>()).await.is_err());
        assert_eq!(Instant::now() - started, Duration::from_millis(20));
        assert!(timeout_at(deadline, pending::<()>()).await.is_err());
    }
}

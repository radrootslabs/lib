#![forbid(unsafe_code)]

// Execute the production module, without substituting a native runtime or
// claiming that this isolated host check qualifies the full Nostr transport.
#[path = "../../../src/host_time.rs"]
mod host_time;

use core::{cell::Cell, future::pending, time::Duration};
use futures::FutureExt;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

struct OnDrop(Rc<Cell<bool>>);

impl Drop for OnDrop {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

fn require(value: bool, name: &str) -> Result<(), JsValue> {
    if value {
        Ok(())
    } else {
        Err(JsValue::from_str(name))
    }
}

#[wasm_bindgen]
pub async fn verify_delayed_timer_owner() -> Result<(), JsValue> {
    use host_time::{Instant, timeout_at};
    let deadline = Instant::now() + Duration::from_millis(5);
    require(
        timeout_at(deadline, async {
            // Deliberately occupy this poll until the original deadline is past.
            // The separate timer owner must schedule zero remaining time, rather
            // than granting another relative budget when it eventually runs.
            while Instant::now() < deadline + Duration::from_millis(15) {}
            pending::<()>().await;
        })
        .await
        .is_err(),
        "delayed timer owner expiry",
    )
}

#[wasm_bindgen]
pub async fn verify_long_timer_owner() -> Result<(), JsValue> {
    long_timer(u64::from(i32::MAX as u32) + 100).await
}

#[wasm_bindgen]
pub async fn verify_extreme_timer_owner() -> Result<(), JsValue> {
    long_timer(u64::MAX).await
}

async fn long_timer(millis: u64) -> Result<(), JsValue> {
    let dropped = Rc::new(Cell::new(false));
    let guard = OnDrop(Rc::clone(&dropped));
    let mut operation = Box::pin(host_time::timeout(
        Duration::from_millis(millis),
        async move {
            let _guard = guard;
            pending::<()>().await;
        },
    ));
    require(
        operation.as_mut().now_or_never().is_none(),
        "long timer starts pending",
    )?;
    async_utility::time::sleep(Duration::ZERO).await;
    require(
        operation.as_mut().now_or_never().is_none(),
        "long timer cannot expire immediately",
    )?;
    drop(operation);
    require(dropped.get(), "long observer cancellation drops work")?;
    async_utility::time::sleep(Duration::ZERO).await;
    Ok(())
}

#[wasm_bindgen]
pub async fn verify_fractional_deadlines() -> Result<(), JsValue> {
    for _ in 0..8 {
        let deadline = host_time::Instant::now() + Duration::from_micros(1_500);
        require(
            host_time::timeout_at(deadline, pending::<()>())
                .await
                .is_err(),
            "fractional expiry",
        )?;
        require(
            host_time::Instant::now() >= deadline,
            "timer cannot expire early",
        )?;
    }
    Ok(())
}

#[wasm_bindgen]
pub async fn verify_host_time() -> Result<js_sys::Array, JsValue> {
    use host_time::{Instant, timeout, timeout_at, unix_time_ms};
    let passed = js_sys::Array::new();
    let start = Instant::now();
    let now = unix_time_ms();
    require(
        now.abs_diff(js_sys::Date::now() as u64) < 1_000,
        "browser wall clock",
    )?;
    passed.push(&"browser wall clock".into());

    let local = Rc::new(Cell::new(7));
    let expired = Instant::now();
    require(
        timeout_at(expired, async { local.get() }).await.ok() == Some(7),
        "expired ready local future",
    )?;
    passed.push(&"expired ready local future".into());

    struct DeferredReady(Rc<Cell<bool>>);
    impl core::future::IntoFuture for DeferredReady {
        type Output = u32;
        type IntoFuture = futures::future::Ready<u32>;
        fn into_future(self) -> Self::IntoFuture {
            self.0.set(true);
            futures::future::ready(7)
        }
    }
    let converted = Rc::new(Cell::new(false));
    let builder = timeout_at(expired, DeferredReady(Rc::clone(&converted)));
    require(!converted.get(), "unpolled IntoFuture builder is inert")?;
    require(
        builder.await.ok() == Some(7) && converted.get(),
        "IntoFuture builder timeout",
    )?;
    passed.push(&"local IntoFuture builder".into());
    require(
        timeout_at(expired, pending::<()>()).await.is_err(),
        "expired pending future",
    )?;
    passed.push(&"expired pending future".into());

    let dropped = Rc::new(Cell::new(false));
    let guard = OnDrop(Rc::clone(&dropped));
    require(
        timeout(Duration::from_millis(10), async move {
            let _guard = guard;
            pending::<()>().await;
        })
        .await
        .is_err()
            && dropped.get(),
        "expiry drops local work",
    )?;
    passed.push(&"expiry drops local work".into());

    let deadline = Instant::now() + Duration::from_millis(10);
    async_utility::time::sleep(Duration::from_millis(20)).await;
    // A ready custom future checks its original absolute deadline during its
    // first poll. This does not rely on tight wall-time scheduling thresholds.
    require(
        timeout_at(deadline, async { Instant::now() >= deadline })
            .await
            .ok()
            == Some(true),
        "one absolute deadline",
    )?;
    require(
        timeout_at(deadline, pending::<()>()).await.is_err(),
        "elapsed queued work",
    )?;
    passed.push(&"one absolute deadline".into());

    for _ in 0..16 {
        let dropped = Rc::new(Cell::new(false));
        let guard = OnDrop(Rc::clone(&dropped));
        require(
            timeout(Duration::from_secs(1), async move {
                let _guard = guard;
                pending::<()>().await;
            })
            .now_or_never()
            .is_none()
                && dropped.get(),
            "observer cancellation drops work",
        )?;
        // Give the separate timer owner an executor turn to dispose its timer.
        async_utility::time::sleep(Duration::ZERO).await;
    }
    passed.push(&"repeated observer cancellation".into());
    require(
        timeout(Duration::from_secs(1), async { 9 }).await.ok() == Some(9),
        "ready after timer cancellation",
    )?;
    passed.push(&"ready after timer cancellation".into());
    require(
        Instant::now() >= start && unix_time_ms() >= now,
        "host clocks advance",
    )?;
    passed.push(&"host clocks advance".into());
    Ok(passed)
}

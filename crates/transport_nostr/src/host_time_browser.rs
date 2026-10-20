//! Absolute deadlines with timer disposal confined to its local owner.

use core::{future::IntoFuture, pin::pin, time::Duration};
use futures::{
    channel::oneshot,
    future::{Either, select},
};
pub(crate) use web_time::Instant;

#[derive(Debug)]
pub(crate) struct Elapsed;

pub(crate) async fn timeout<F: IntoFuture>(
    duration: Duration,
    future: F,
) -> Result<F::Output, Elapsed> {
    timeout_at(Instant::now() + duration, future).await
}

pub(crate) async fn timeout_at<F: IntoFuture>(
    deadline: Instant,
    future: F,
) -> Result<F::Output, Elapsed> {
    let (elapsed, receiver) = oneshot::channel();
    let (cancel_on_drop, cancellation) = oneshot::channel::<()>();
    wasm_bindgen_futures::spawn_local(async move {
        // Measure here: this owner may begin after the original caller yields.
        // Dropping a losing timer here also avoids browser executor reentry in
        // the task that polls the network future.
        let timer = pin!(sleep_until(deadline));
        if matches!(select(timer, pin!(cancellation)).await, Either::Left(_)) {
            let _ = elapsed.send(());
        }
    });
    let result = match select(pin!(future.into_future()), pin!(receiver)).await {
        Either::Left((value, _)) => Ok(value),
        Either::Right(_) => Err(Elapsed),
    };
    // This sender also drops if the observer cancels while select is pending.
    drop(cancel_on_drop);
    result
}

async fn sleep_until(deadline: Instant) {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        // Gloo truncates to whole milliseconds, then casts to the browser's
        // signed timer argument. Round up and split long waits; recheck after
        // every wake so neither truncation nor an early callback proves expiry.
        let millis = remaining
            .as_nanos()
            .div_ceil(1_000_000)
            .min(i32::MAX as u128) as u64;
        async_utility::time::sleep(Duration::from_millis(millis)).await;
    }
}

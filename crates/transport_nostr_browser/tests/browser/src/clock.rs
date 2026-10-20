//! The pinned SDK requires exactly one clock selection in the final WASM
//! artifact. This fixture is an artifact root, not a provider library. Use real
//! browser clocks in both pages and workers; never substitute a test timestamp.

use std::time::Duration;
use universal_time::{Instant, MonotonicClock, SystemTime, WallClock};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn monotonic_ms() -> f64;
}

struct BrowserClock;
impl WallClock for BrowserClock {
    fn system_time(&self) -> SystemTime {
        SystemTime::from_unix_duration(Duration::from_secs_f64(js_sys::Date::now() / 1_000.0))
    }
}
impl MonotonicClock for BrowserClock {
    fn instant(&self) -> Instant {
        Instant::from_ticks(Duration::from_secs_f64(monotonic_ms() / 1_000.0))
    }
}
universal_time::define_time_provider!(BrowserClock);

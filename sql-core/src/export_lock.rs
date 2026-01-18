#![cfg(target_arch = "wasm32")]
#![forbid(unsafe_code)]

use crate::error::SqlError;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) const EXPORT_LOCK_ERR: &str = "tangle db export in progress";

static EXPORT_LOCK_ACTIVE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static EXPORT_LOCK_BYPASS: Cell<bool> = Cell::new(false);
}

pub fn export_lock_begin() -> Result<(), SqlError> {
    let was_active = EXPORT_LOCK_ACTIVE.swap(true, Ordering::SeqCst);
    if was_active {
        return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
    }
    Ok(())
}

pub fn export_lock_end() {
    EXPORT_LOCK_ACTIVE.store(false, Ordering::SeqCst);
}

pub fn export_lock_active() -> bool {
    EXPORT_LOCK_ACTIVE.load(Ordering::SeqCst)
}

pub fn with_export_lock_bypass<T>(f: impl FnOnce() -> T) -> T {
    EXPORT_LOCK_BYPASS.with(|flag| {
        let prev = flag.replace(true);
        let out = f();
        flag.set(prev);
        out
    })
}

pub(crate) fn export_lock_blocked() -> bool {
    if !EXPORT_LOCK_ACTIVE.load(Ordering::SeqCst) {
        return false;
    }
    EXPORT_LOCK_BYPASS.with(|flag| !flag.get())
}

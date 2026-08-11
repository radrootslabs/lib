#![forbid(unsafe_code)]

//! Reusable, service-neutral host mechanics for Radroots services.

pub mod entropy;
pub mod error;
pub mod time;

pub use entropy::{EntropyError, EntropySource, SystemEntropy};
pub use error::{HostError, HostErrorCode, HostErrorKind, SafeHostError};
pub use time::{
    MonotonicClock, MonotonicClockError, MonotonicDeadline, MonotonicTime, SystemMonotonicClock,
    SystemWallClock, UnixTimeSeconds, WallClock, WallClockError,
};

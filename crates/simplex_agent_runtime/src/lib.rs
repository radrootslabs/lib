#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod error;
pub mod runtime;
pub mod types;

pub mod prelude {
    pub use crate::error::RadrootsSimplexAgentRuntimeError;
    pub use crate::runtime::{RadrootsSimplexAgentRuntime, RadrootsSimplexAgentRuntimeBuilder};
    pub use crate::types::{RadrootsSimplexAgentCommandOutcome, RadrootsSimplexAgentRuntimeEvent};
}

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod error;
pub mod runtime;
pub mod types;

pub mod prelude {
    pub use crate::error::RadrootsSimplexAgentRuntimeError;
    pub use crate::runtime::{
        RadrootsSimplexAgentRuntime, RadrootsSimplexAgentRuntimeBuilder,
        decrypt_short_invitation_link_data,
    };
    pub use crate::types::{RadrootsSimplexAgentCommandOutcome, RadrootsSimplexAgentRuntimeEvent};
}

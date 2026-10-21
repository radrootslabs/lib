#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg(any(target_arch = "wasm32", test))]
mod tasks;

#[cfg(target_arch = "wasm32")]
mod client;
#[cfg(all(target_arch = "wasm32", feature = "artifact_clock"))]
mod clock;
#[cfg(target_arch = "wasm32")]
mod commands;

#[cfg(target_arch = "wasm32")]
pub use client::{BrowserConfig, Client, CloseReceipt, Inbox, Listings, Operations};
#[cfg(target_arch = "wasm32")]
pub use commands::BrowserNonceSource;
pub use radroots_domain::{CommandDigest, OperationId};
pub use radroots_execution::{
    OperationKind, OperationReceipt, OperationStatus, OperationValue,
    inbox::InquireInput,
    session::{SessionState, StartMode},
    workspace::{CommandContext, WorkspaceSession},
};
pub type Result<T> = radroots_execution::Result<T>;

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]
#![deny(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]

pub mod client;
pub mod error;
pub mod message;
pub mod method;
pub mod permission;
pub mod server;
pub mod uri;

pub use client::Client;
pub use error::RadrootsNostrConnectError as Error;
pub use message::{Request, Response};
pub use method::Method;
pub use permission::Permission;
pub use server::Server;
pub use uri::{BunkerUri, ClientUri};

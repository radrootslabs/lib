#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod connector;
mod socket;
mod wire;

pub use connector::NativeConnector;

#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

pub mod capability;
pub mod endpoint;
pub mod error;
mod id;
pub mod outcome;
pub mod policy;
pub mod sink;
pub mod source;
mod status;
pub mod target;

pub use error::Error;
pub use id::{TRANSPORT_ID_MAX_BYTES, TransportId};
pub use sink::{DeliveryReceipt, DeliveryRequest, EventSink, SinkFailure, SinkStatus};
pub use source::{
    BoxFuture, BoxSubscription, EventSource, EventSubscriber, EventSubscription, FetchPage,
    FetchRequest, SourceStatus, SubscriptionEnd, SubscriptionEndReason, SubscriptionEvent,
    SubscriptionNext, SubscriptionRequest,
};
pub use target::{TARGET_SET_MAX_ITEMS, Target, TargetNetworkPolicy, TargetSet};

#[cfg(test)]
extern crate self as radroots_transport;

#[cfg(test)]
extern crate std;

//! Curated transport entry points.

pub use radroots_sdk::transport::Profile;
pub use radroots_transport::{
    Error, EventSink, EventSource, Target, TargetSet, TransportId,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
};

#[cfg(any(feature = "radrootsd", feature = "full"))]
pub use radroots_sdk::transport::{DaemonAuth, DaemonConfig, DaemonDelivery, DaemonError};

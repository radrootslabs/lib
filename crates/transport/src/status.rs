use alloc::string::String;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportDeliveryTargetStatus {
    Pending,
    Accepted,
    Deferred,
    Rejected,
    Failed,
    Unavailable,
}

impl RadrootsTransportDeliveryTargetStatus {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Pending)
    }

    pub fn counts_as_satisfied(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportOutcome {
    pub status: RadrootsTransportDeliveryTargetStatus,
    pub code: Option<String>,
    pub message: Option<String>,
}

impl RadrootsTransportOutcome {
    pub fn new(status: RadrootsTransportDeliveryTargetStatus) -> Self {
        Self {
            status,
            code: None,
            message: None,
        }
    }
}

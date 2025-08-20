#[cfg(not(feature = "std"))]
use alloc::string::String;

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsEventsIndexedShardId(pub String);

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventsIndexedIdRange {
    pub start: String,
    pub end: String,
}

impl RadrootsEventsIndexedIdRange {
    pub fn is_valid(&self) -> bool {
        !self.start.is_empty() && !self.end.is_empty() && self.start <= self.end
    }
}

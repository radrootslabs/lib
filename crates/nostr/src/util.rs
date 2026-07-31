use crate::types::{RadrootsNostrEvent, RadrootsNostrTimestamp};

pub fn created_at_u32_saturating(ts: RadrootsNostrTimestamp) -> u32 {
    u32::try_from(ts.as_secs()).unwrap_or(u32::MAX)
}

pub fn event_created_at_u32_saturating(event: &RadrootsNostrEvent) -> u32 {
    created_at_u32_saturating(event.created_at)
}

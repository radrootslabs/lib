//! Original source evidence, correlated without treating a normalized key as
//! admission. Only original bytes leave this bounded queue.
use core::mem;
use nostr_sdk::prelude::Event;
use serde_json::value::RawValue;
use sha2::{Digest as _, Sha256};
use std::{collections::VecDeque, fmt};

pub(crate) const MAX_PENDING_EVENTS: usize = 256;
pub(crate) const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_ORIGINAL_EVENT_BYTES: usize =
    radroots_protocol::profile::SIGNED_EVENT_MAX_BYTES;

struct Original {
    owner: u64,
    scope: u64,
    correlation: [u8; 32],
    raw: Box<str>,
}

pub(crate) struct OriginalQueue {
    entries: VecDeque<Original>,
    payload_bytes: usize,
}
impl Default for OriginalQueue {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(MAX_PENDING_EVENTS),
            payload_bytes: 0,
        }
    }
}
impl fmt::Debug for OriginalQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OriginalQueue")
            .field("events", &self.entries.len())
            .field("retained_bytes", &self.retained_bytes())
            .finish()
    }
}
impl OriginalQueue {
    pub(crate) fn retained_bytes(&self) -> usize {
        mem::size_of::<Self>()
            + self.entries.capacity() * mem::size_of::<Original>()
            + self.payload_bytes
    }
    pub(crate) fn insert(
        &mut self,
        owner: u64,
        scope: u64,
        correlation: [u8; 32],
        raw: &str,
    ) -> bool {
        if self.entries.len() == MAX_PENDING_EVENTS
            || raw.len() > MAX_ORIGINAL_EVENT_BYTES
            || raw.len() > MAX_PENDING_BYTES.saturating_sub(self.retained_bytes())
        {
            return false;
        }
        self.entries.push_back(Original {
            owner,
            scope,
            correlation,
            raw: Box::from(raw),
        });
        self.payload_bytes += raw.len();
        true
    }
    pub(crate) fn take(&mut self, scope: u64, correlation: [u8; 32]) -> Option<String> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.scope == scope && entry.correlation == correlation)?;
        let value = self.entries.remove(index)?;
        self.payload_bytes -= value.raw.len();
        Some(value.raw.into_string())
    }
    fn retain(&mut self, mut keep: impl FnMut(&Original) -> bool) {
        let mut removed = 0;
        self.entries.retain(|value| {
            if keep(value) {
                true
            } else {
                removed += value.raw.len();
                false
            }
        });
        self.payload_bytes -= removed;
    }
    pub(crate) fn remove_scope(&mut self, scope: u64) {
        self.retain(|entry| entry.scope != scope);
    }
    pub(crate) fn remove_owner(&mut self, owner: u64) {
        self.retain(|entry| entry.owner != owner);
    }
}

/// The key is only correlation with the same already-pinned upstream decoder.
/// It grants no identity/signature/profile authority and is never substituted
/// for original bytes. Different raw spellings remain separate queued entries.
pub(crate) fn correlation(event: &Event) -> [u8; 32] {
    Sha256::digest(event.as_json().as_bytes()).into()
}

pub(crate) fn frame(raw: &str) -> Option<(String, &RawValue)> {
    if raw.len() > crate::session_socket::MAX_WIRE_MESSAGE_BYTES {
        return None;
    }
    let [kind, subscription, event]: [&RawValue; 3] = serde_json::from_str(raw).ok()?;
    if kind.get().len() > 32 || subscription.get().len() > 6 * 256 + 2 {
        return None;
    }
    let kind: String = serde_json::from_str(kind.get()).ok()?;
    if kind != "EVENT" || event.get().len() > MAX_ORIGINAL_EVENT_BYTES {
        return None;
    }
    let subscription: String = serde_json::from_str(subscription.get()).ok()?;
    if subscription.is_empty() || subscription.len() > 256 {
        return None;
    }
    Some((subscription, event))
}

#[cfg(test)]
#[path = "source_original_queue_tests.rs"]
mod tests;

//! Deterministic relay event cursor shared by paging and reconnect catch-up.

use crate::Error;

/// Stable total-order position for one Nostr event.
///
/// Relay timestamps are only second-granular. The canonical lowercase event id
/// is therefore a required tie-breaker for both descending fetch pages and
/// inclusive reconnect catch-up after a subscription interruption.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelayCursor {
    created_at_unix_s: u64,
    event_id: String,
}

impl RelayCursor {
    /// Creates a cursor from an event timestamp and canonical lowercase id.
    pub fn new(created_at_unix_s: u64, event_id: impl Into<String>) -> Result<Self, Error> {
        let event_id = event_id.into();
        if event_id.len() != 64
            || !event_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::InvalidRelayCursor);
        }
        Ok(Self {
            created_at_unix_s,
            event_id,
        })
    }

    /// Returns the second-granular Nostr timestamp.
    #[must_use]
    pub const fn created_at_unix_s(&self) -> u64 {
        self.created_at_unix_s
    }

    /// Returns the canonical event-id tie breaker.
    #[must_use]
    pub fn event_id(&self) -> &str {
        self.event_id.as_str()
    }

    /// Returns whether a candidate follows this cursor in ascending reconnect
    /// order. Equal timestamps are resolved by event id, preventing loss when
    /// a reconnect query uses an inclusive `since` timestamp.
    #[must_use]
    pub fn precedes(&self, created_at_unix_s: u64, event_id: &str) -> bool {
        created_at_unix_s > self.created_at_unix_s
            || created_at_unix_s == self.created_at_unix_s && event_id > self.event_id.as_str()
    }

    /// Returns whether a candidate follows this cursor in descending page
    /// order. Equal timestamps are resolved by the inverse event-id order.
    #[must_use]
    pub(crate) fn page_precedes(&self, created_at_unix_s: u64, event_id: &str) -> bool {
        created_at_unix_s < self.created_at_unix_s
            || created_at_unix_s == self.created_at_unix_s && event_id < self.event_id.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_timestamp_ties_are_lossless_in_both_directions() {
        let cursor = RelayCursor::new(10, "b".repeat(64)).expect("cursor");
        assert!(cursor.precedes(11, &"0".repeat(64)));
        assert!(cursor.precedes(10, &"c".repeat(64)));
        assert!(!cursor.precedes(9, &"f".repeat(64)));
        assert!(!cursor.precedes(10, &"b".repeat(64)));
        assert!(cursor.page_precedes(9, &"f".repeat(64)));
        assert!(cursor.page_precedes(10, &"a".repeat(64)));
        assert!(!cursor.page_precedes(10, &"c".repeat(64)));
        assert_eq!(cursor.created_at_unix_s(), 10);
        assert_eq!(cursor.event_id(), "b".repeat(64));
    }

    #[test]
    fn cursor_rejects_noncanonical_event_ids() {
        for invalid in ["a".repeat(63), "A".repeat(64), "g".repeat(64)] {
            assert!(RelayCursor::new(1, invalid).is_err());
        }
    }
}

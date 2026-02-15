#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsEventsIndexedShardId(pub String);

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventsIndexedIdRange {
    pub start: String,
    pub end: String,
}

impl RadrootsEventsIndexedIdRange {
    pub fn is_valid(&self) -> bool {
        if self.start.is_empty() || self.end.is_empty() {
            return false;
        }
        if self.start.len() != self.end.len() {
            return false;
        }
        if !self.start.chars().all(|c| c.is_ascii_hexdigit())
            || !self.end.chars().all(|c| c.is_ascii_hexdigit())
        {
            return false;
        }
        self.start <= self.end
    }
}

#[cfg(test)]
mod tests {
    use super::RadrootsEventsIndexedIdRange;
    #[cfg(not(feature = "std"))]
    use alloc::string::String;
    #[cfg(feature = "std")]
    use std::string::String;

    #[test]
    fn id_range_rejects_non_hex() {
        let range = RadrootsEventsIndexedIdRange {
            start: String::from("zz"),
            end: String::from("ff"),
        };
        assert!(!range.is_valid());
    }

    #[test]
    fn id_range_rejects_mismatched_length() {
        let range = RadrootsEventsIndexedIdRange {
            start: String::from("0a"),
            end: String::from("0aa"),
        };
        assert!(!range.is_valid());
    }

    #[test]
    fn id_range_rejects_reverse_order() {
        let range = RadrootsEventsIndexedIdRange {
            start: String::from("0f"),
            end: String::from("0a"),
        };
        assert!(!range.is_valid());
    }

    #[test]
    fn id_range_accepts_hex_order() {
        let range = RadrootsEventsIndexedIdRange {
            start: String::from("0a"),
            end: String::from("0f"),
        };
        assert!(range.is_valid());
    }
}

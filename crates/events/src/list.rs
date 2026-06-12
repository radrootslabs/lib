#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsList {
    pub content: String,
    pub entries: Vec<RadrootsListEntry>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListEntry {
    pub tag: String,
    pub values: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{KIND_LIST_READ_WRITE_RELAYS, is_public_social_kind};

    #[test]
    fn generic_list_model_covers_nip65_relay_entries() {
        let list = RadrootsList {
            content: String::new(),
            entries: vec![
                RadrootsListEntry {
                    tag: "r".to_string(),
                    values: vec!["wss://read.example".to_string(), "read".to_string()],
                },
                RadrootsListEntry {
                    tag: "r".to_string(),
                    values: vec!["wss://write.example".to_string(), "write".to_string()],
                },
            ],
        };

        assert_eq!(list.entries.len(), 2);
        assert_eq!(list.entries[0].tag, "r");
        assert_eq!(list.entries[0].values[1], "read");
        assert!(is_public_social_kind(KIND_LIST_READ_WRITE_RELAYS));
    }
}

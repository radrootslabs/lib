#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct List {
    pub content: String,
    pub entries: Vec<ListEntry>,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct ListEntry {
    pub tag: String,
    pub values: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::kind::{KIND_LIST_READ_WRITE_RELAYS, is_nip51_standard_list_kind};

    #[test]
    fn generic_list_model_covers_nip65_relay_entries() {
        let list = List {
            content: String::new(),
            entries: vec![
                ListEntry {
                    tag: "r".to_string(),
                    values: vec!["wss://read.example".to_string(), "read".to_string()],
                },
                ListEntry {
                    tag: "r".to_string(),
                    values: vec!["wss://write.example".to_string(), "write".to_string()],
                },
            ],
        };

        assert_eq!(list.entries.len(), 2);
        assert_eq!(list.entries[0].tag, "r");
        assert_eq!(list.entries[0].values[1], "read");
        assert!(is_nip51_standard_list_kind(KIND_LIST_READ_WRITE_RELAYS));
    }
}

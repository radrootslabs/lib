pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use super::{decode::list_from_tags, encode::list_build_tags};
    use radroots_events::{
        kinds::KIND_LIST_MUTE,
        list::{RadrootsList, RadrootsListEntry},
    };

    #[test]
    fn list_tags_round_trip() {
        let list = RadrootsList {
            content: "private".to_string(),
            entries: vec![
                RadrootsListEntry {
                    tag: "p".to_string(),
                    values: vec!["abc".to_string(), "wss://relay".to_string()],
                },
                RadrootsListEntry {
                    tag: "t".to_string(),
                    values: vec!["radroots".to_string()],
                },
            ],
        };
        let tags = list_build_tags(&list).expect("build tags");
        let parsed =
            list_from_tags(KIND_LIST_MUTE, list.content.clone(), &tags).expect("parse list");
        assert_eq!(parsed.content, list.content);
        assert_eq!(parsed.entries.len(), list.entries.len());
        assert_eq!(parsed.entries[0].tag, "p");
        assert_eq!(parsed.entries[0].values[0], "abc");
        assert_eq!(parsed.entries[1].tag, "t");
        assert_eq!(parsed.entries[1].values[0], "radroots");
    }
}

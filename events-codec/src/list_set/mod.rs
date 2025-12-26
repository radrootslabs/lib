pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use super::{decode::list_set_from_tags, encode::list_set_build_tags};
    use radroots_events::{
        kinds::KIND_LIST_SET_FOLLOW,
        list::{RadrootsListEntry},
        list_set::RadrootsListSet,
    };

    #[test]
    fn list_set_tags_round_trip() {
        let list = RadrootsListSet {
            d_tag: "members.owners".to_string(),
            content: "".to_string(),
            entries: vec![
                RadrootsListEntry {
                    tag: "p".to_string(),
                    values: vec!["owner_pubkey".to_string()],
                },
                RadrootsListEntry {
                    tag: "p".to_string(),
                    values: vec!["worker_pubkey".to_string(), "wss://relay".to_string()],
                },
            ],
            title: Some("Owners".to_string()),
            description: None,
            image: None,
        };
        let tags = list_set_build_tags(&list).expect("build tags");
        let parsed = list_set_from_tags(KIND_LIST_SET_FOLLOW, list.content.clone(), &tags)
            .expect("parse list set");
        assert_eq!(parsed.d_tag, list.d_tag);
        assert_eq!(parsed.title, list.title);
        assert_eq!(parsed.entries.len(), list.entries.len());
        assert_eq!(parsed.entries[0].values[0], "owner_pubkey");
    }
}

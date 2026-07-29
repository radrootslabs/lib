pub mod decode;
pub mod encode;

use radroots_event::envelope::kind::{
    KIND_CALENDAR, is_nip51_list_set_kind, is_nip51_standard_list_kind,
};

pub(crate) fn is_generic_list_codec_kind(kind: u32) -> bool {
    is_nip51_standard_list_kind(kind) || (is_nip51_list_set_kind(kind) && kind != KIND_CALENDAR)
}

#[cfg(test)]
mod tests {
    use super::{decode::list_from_tags, encode::list_build_tags, is_generic_list_codec_kind};
    use radroots_event::{
        envelope::kind::{KIND_CALENDAR, KIND_LIST_MUTE, KIND_LIST_SET_FOLLOW, KIND_POST},
        social::list::{List, ListEntry},
    };

    #[test]
    fn list_tags_round_trip() {
        let list = List {
            content: "private".to_string(),
            entries: vec![
                ListEntry {
                    tag: "p".to_string(),
                    values: vec!["abc".to_string(), "wss://relay".to_string()],
                },
                ListEntry {
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

    #[test]
    fn generic_list_codec_kind_excludes_calendar() {
        assert!(is_generic_list_codec_kind(KIND_LIST_MUTE));
        assert!(is_generic_list_codec_kind(KIND_LIST_SET_FOLLOW));
        assert!(!is_generic_list_codec_kind(KIND_CALENDAR));
        assert!(!is_generic_list_codec_kind(KIND_POST));
    }
}

pub const KIND_PROFILE: u32 = 0;
pub const KIND_POST: u32 = 1;
pub const KIND_FOLLOW: u32 = 3;
pub const KIND_REACTION: u32 = 7;
pub const KIND_SEAL: u32 = 13;
pub const KIND_MESSAGE: u32 = 14;
pub const KIND_MESSAGE_FILE: u32 = 15;
pub const KIND_GIFT_WRAP: u32 = 1059;
pub const KIND_COMMENT: u32 = 1111;
pub const KIND_GEOCHAT: u32 = 20000;
pub const KIND_LIST_MUTE: u32 = 10000;
pub const KIND_LIST_PINNED_NOTES: u32 = 10001;
pub const KIND_LIST_READ_WRITE_RELAYS: u32 = 10002;
pub const KIND_LIST_BOOKMARKS: u32 = 10003;
pub const KIND_LIST_COMMUNITIES: u32 = 10004;
pub const KIND_LIST_PUBLIC_CHATS: u32 = 10005;
pub const KIND_LIST_BLOCKED_RELAYS: u32 = 10006;
pub const KIND_LIST_SEARCH_RELAYS: u32 = 10007;
pub const KIND_LIST_SIMPLE_GROUPS: u32 = 10009;
pub const KIND_LIST_RELAY_FEEDS: u32 = 10012;
pub const KIND_LIST_INTERESTS: u32 = 10015;
pub const KIND_LIST_MEDIA_FOLLOWS: u32 = 10020;
pub const KIND_LIST_EMOJIS: u32 = 10030;
pub const KIND_LIST_DM_RELAYS: u32 = 10050;
pub const KIND_LIST_GOOD_WIKI_AUTHORS: u32 = 10101;
pub const KIND_LIST_GOOD_WIKI_RELAYS: u32 = 10102;
pub const KIND_LIST_SET_FOLLOW: u32 = 30000;
pub const KIND_LIST_SET_GENERIC: u32 = 30001;
pub const KIND_LIST_SET_RELAY: u32 = 30002;
pub const KIND_LIST_SET_BOOKMARK: u32 = 30003;
pub const KIND_LIST_SET_CURATION: u32 = 30004;
pub const KIND_LIST_SET_VIDEO: u32 = 30005;
pub const KIND_LIST_SET_PICTURE: u32 = 30006;
pub const KIND_LIST_SET_KIND_MUTE: u32 = 30007;
pub const KIND_LIST_SET_INTEREST: u32 = 30015;
pub const KIND_LIST_SET_EMOJI: u32 = 30030;
pub const KIND_LIST_SET_RELEASE_ARTIFACT: u32 = 30063;
pub const KIND_LIST_SET_APP_CURATION: u32 = 30267;
pub const KIND_LIST_SET_CALENDAR: u32 = 31924;
pub const KIND_LIST_SET_STARTER_PACK: u32 = 39089;
pub const KIND_LIST_SET_MEDIA_STARTER_PACK: u32 = 39092;
pub const KIND_FARM: u32 = 30340;
pub const KIND_PLOT: u32 = 30350;
pub const KIND_COOP: u32 = 30360;
pub const KIND_DOCUMENT: u32 = 30361;
pub const KIND_RESOURCE_AREA: u32 = 30370;
pub const KIND_RESOURCE_HARVEST_CAP: u32 = 30371;
pub const KIND_ACCOUNT_CLAIM: u32 = 30380;
pub const KIND_APP_DATA: u32 = 30078;
pub const KIND_LISTING: u32 = 30402;
pub const KIND_APPLICATION_HANDLER: u32 = 31990;

pub const KIND_JOB_REQUEST_MIN: u32 = 5000;
pub const KIND_JOB_REQUEST_MAX: u32 = 5999;
pub const KIND_JOB_RESULT_MIN: u32 = 6000;
pub const KIND_JOB_RESULT_MAX: u32 = 6999;
pub const KIND_JOB_FEEDBACK: u32 = 7000;

#[inline]
pub const fn is_nip51_standard_list_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_LIST_MUTE
            | KIND_LIST_PINNED_NOTES
            | KIND_LIST_READ_WRITE_RELAYS
            | KIND_LIST_BOOKMARKS
            | KIND_LIST_COMMUNITIES
            | KIND_LIST_PUBLIC_CHATS
            | KIND_LIST_BLOCKED_RELAYS
            | KIND_LIST_SEARCH_RELAYS
            | KIND_LIST_SIMPLE_GROUPS
            | KIND_LIST_RELAY_FEEDS
            | KIND_LIST_INTERESTS
            | KIND_LIST_MEDIA_FOLLOWS
            | KIND_LIST_EMOJIS
            | KIND_LIST_DM_RELAYS
            | KIND_LIST_GOOD_WIKI_AUTHORS
            | KIND_LIST_GOOD_WIKI_RELAYS
    )
}
#[inline]
pub const fn is_nip51_list_set_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_LIST_SET_FOLLOW
            | KIND_LIST_SET_GENERIC
            | KIND_LIST_SET_RELAY
            | KIND_LIST_SET_BOOKMARK
            | KIND_LIST_SET_CURATION
            | KIND_LIST_SET_VIDEO
            | KIND_LIST_SET_PICTURE
            | KIND_LIST_SET_KIND_MUTE
            | KIND_LIST_SET_INTEREST
            | KIND_LIST_SET_EMOJI
            | KIND_LIST_SET_RELEASE_ARTIFACT
            | KIND_LIST_SET_APP_CURATION
            | KIND_LIST_SET_CALENDAR
            | KIND_LIST_SET_STARTER_PACK
            | KIND_LIST_SET_MEDIA_STARTER_PACK
    )
}

#[inline]
pub const fn is_request_kind(kind: u32) -> bool {
    kind >= KIND_JOB_REQUEST_MIN && kind <= KIND_JOB_REQUEST_MAX
}
#[inline]
pub const fn is_result_kind(kind: u32) -> bool {
    kind >= KIND_JOB_RESULT_MIN && kind <= KIND_JOB_RESULT_MAX
}
#[inline]
pub const fn result_kind_for_request_kind(kind: u32) -> Option<u32> {
    if is_request_kind(kind) {
        Some(kind + 1000)
    } else {
        None
    }
}
#[inline]
pub const fn request_kind_for_result_kind(kind: u32) -> Option<u32> {
    if is_result_kind(kind) {
        Some(kind - 1000)
    } else {
        None
    }
}

#[cfg(all(test, feature = "ts-rs", feature = "std"))]
mod kinds_constants_tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    fn workspace_root(manifest_dir: &Path) -> PathBuf {
        let parent = manifest_dir.parent().unwrap_or(manifest_dir);
        if parent.file_name().and_then(|name| name.to_str()) == Some("crates") {
            parent.parent().unwrap_or(parent).to_path_buf()
        } else {
            parent.to_path_buf()
        }
    }

    fn ts_export_dir_from(export_dir: Option<&str>) -> PathBuf {
        if let Some(export_dir) = export_dir {
            return PathBuf::from(export_dir);
        }
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        workspace_root(&manifest_dir)
            .join("target")
            .join("ts-rs")
            .join("events")
    }

    fn ts_export_dir() -> PathBuf {
        ts_export_dir_from(option_env!("TS_RS_EXPORT_DIR"))
    }

    const KIND_EXPORTS: &[(&str, u32)] = &[
        ("KIND_PROFILE", KIND_PROFILE),
        ("KIND_POST", KIND_POST),
        ("KIND_FOLLOW", KIND_FOLLOW),
        ("KIND_REACTION", KIND_REACTION),
        ("KIND_SEAL", KIND_SEAL),
        ("KIND_MESSAGE", KIND_MESSAGE),
        ("KIND_MESSAGE_FILE", KIND_MESSAGE_FILE),
        ("KIND_GIFT_WRAP", KIND_GIFT_WRAP),
        ("KIND_COMMENT", KIND_COMMENT),
        ("KIND_GEOCHAT", KIND_GEOCHAT),
        ("KIND_LIST_MUTE", KIND_LIST_MUTE),
        ("KIND_LIST_PINNED_NOTES", KIND_LIST_PINNED_NOTES),
        ("KIND_LIST_READ_WRITE_RELAYS", KIND_LIST_READ_WRITE_RELAYS),
        ("KIND_LIST_BOOKMARKS", KIND_LIST_BOOKMARKS),
        ("KIND_LIST_COMMUNITIES", KIND_LIST_COMMUNITIES),
        ("KIND_LIST_PUBLIC_CHATS", KIND_LIST_PUBLIC_CHATS),
        ("KIND_LIST_BLOCKED_RELAYS", KIND_LIST_BLOCKED_RELAYS),
        ("KIND_LIST_SEARCH_RELAYS", KIND_LIST_SEARCH_RELAYS),
        ("KIND_LIST_SIMPLE_GROUPS", KIND_LIST_SIMPLE_GROUPS),
        ("KIND_LIST_RELAY_FEEDS", KIND_LIST_RELAY_FEEDS),
        ("KIND_LIST_INTERESTS", KIND_LIST_INTERESTS),
        ("KIND_LIST_MEDIA_FOLLOWS", KIND_LIST_MEDIA_FOLLOWS),
        ("KIND_LIST_EMOJIS", KIND_LIST_EMOJIS),
        ("KIND_LIST_DM_RELAYS", KIND_LIST_DM_RELAYS),
        ("KIND_LIST_GOOD_WIKI_AUTHORS", KIND_LIST_GOOD_WIKI_AUTHORS),
        ("KIND_LIST_GOOD_WIKI_RELAYS", KIND_LIST_GOOD_WIKI_RELAYS),
        ("KIND_LIST_SET_FOLLOW", KIND_LIST_SET_FOLLOW),
        ("KIND_LIST_SET_GENERIC", KIND_LIST_SET_GENERIC),
        ("KIND_LIST_SET_RELAY", KIND_LIST_SET_RELAY),
        ("KIND_LIST_SET_BOOKMARK", KIND_LIST_SET_BOOKMARK),
        ("KIND_LIST_SET_CURATION", KIND_LIST_SET_CURATION),
        ("KIND_LIST_SET_VIDEO", KIND_LIST_SET_VIDEO),
        ("KIND_LIST_SET_PICTURE", KIND_LIST_SET_PICTURE),
        ("KIND_LIST_SET_KIND_MUTE", KIND_LIST_SET_KIND_MUTE),
        ("KIND_LIST_SET_INTEREST", KIND_LIST_SET_INTEREST),
        ("KIND_LIST_SET_EMOJI", KIND_LIST_SET_EMOJI),
        (
            "KIND_LIST_SET_RELEASE_ARTIFACT",
            KIND_LIST_SET_RELEASE_ARTIFACT,
        ),
        ("KIND_LIST_SET_APP_CURATION", KIND_LIST_SET_APP_CURATION),
        ("KIND_LIST_SET_CALENDAR", KIND_LIST_SET_CALENDAR),
        ("KIND_LIST_SET_STARTER_PACK", KIND_LIST_SET_STARTER_PACK),
        (
            "KIND_LIST_SET_MEDIA_STARTER_PACK",
            KIND_LIST_SET_MEDIA_STARTER_PACK,
        ),
        ("KIND_FARM", KIND_FARM),
        ("KIND_PLOT", KIND_PLOT),
        ("KIND_COOP", KIND_COOP),
        ("KIND_DOCUMENT", KIND_DOCUMENT),
        ("KIND_RESOURCE_AREA", KIND_RESOURCE_AREA),
        ("KIND_RESOURCE_HARVEST_CAP", KIND_RESOURCE_HARVEST_CAP),
        ("KIND_ACCOUNT_CLAIM", KIND_ACCOUNT_CLAIM),
        ("KIND_APP_DATA", KIND_APP_DATA),
        ("KIND_LISTING", KIND_LISTING),
        ("KIND_APPLICATION_HANDLER", KIND_APPLICATION_HANDLER),
        ("KIND_JOB_REQUEST_MIN", KIND_JOB_REQUEST_MIN),
        ("KIND_JOB_REQUEST_MAX", KIND_JOB_REQUEST_MAX),
        ("KIND_JOB_RESULT_MIN", KIND_JOB_RESULT_MIN),
        ("KIND_JOB_RESULT_MAX", KIND_JOB_RESULT_MAX),
        ("KIND_JOB_FEEDBACK", KIND_JOB_FEEDBACK),
    ];

    #[test]
    fn export_kind_constants() {
        let path = ts_export_dir().join("kinds.ts");
        let parent = path.parent().expect("kinds export path has parent");
        fs::create_dir_all(parent).expect("create ts export dir");
        let mut content = String::new();
        for (name, value) in KIND_EXPORTS {
            content.push_str(&format!("export const {name} = {value};\n"));
        }
        fs::write(&path, content).expect("write kinds");
    }

    #[test]
    fn classifies_standard_list_kinds() {
        assert!(is_nip51_standard_list_kind(KIND_LIST_MUTE));
        assert!(is_nip51_standard_list_kind(KIND_LIST_GOOD_WIKI_RELAYS));
        assert!(!is_nip51_standard_list_kind(KIND_PROFILE));
    }

    #[test]
    fn classifies_list_set_kinds() {
        assert!(is_nip51_list_set_kind(KIND_LIST_SET_FOLLOW));
        assert!(is_nip51_list_set_kind(KIND_LIST_SET_MEDIA_STARTER_PACK));
        assert!(!is_nip51_list_set_kind(KIND_LIST_MUTE));
    }

    #[test]
    fn maps_job_request_and_result_kinds() {
        assert!(is_request_kind(KIND_JOB_REQUEST_MIN));
        assert!(is_request_kind(KIND_JOB_REQUEST_MAX));
        assert!(!is_request_kind(KIND_JOB_REQUEST_MIN - 1));
        assert!(!is_request_kind(KIND_JOB_REQUEST_MAX + 1));

        assert!(is_result_kind(KIND_JOB_RESULT_MIN));
        assert!(is_result_kind(KIND_JOB_RESULT_MAX));
        assert!(!is_result_kind(KIND_JOB_RESULT_MIN - 1));
        assert!(!is_result_kind(KIND_JOB_RESULT_MAX + 1));

        assert_eq!(
            result_kind_for_request_kind(KIND_JOB_REQUEST_MIN),
            Some(KIND_JOB_RESULT_MIN)
        );
        assert_eq!(result_kind_for_request_kind(KIND_JOB_RESULT_MIN), None);
        assert_eq!(
            request_kind_for_result_kind(KIND_JOB_RESULT_MIN),
            Some(KIND_JOB_REQUEST_MIN)
        );
        assert_eq!(request_kind_for_result_kind(KIND_JOB_REQUEST_MIN), None);
    }

    #[test]
    fn resolves_workspace_root_for_crates_and_non_crates_paths() {
        let inside_crates = PathBuf::from("/tmp/radroots/crates/events");
        let non_crates = PathBuf::from("/tmp/radroots/events");
        assert_eq!(
            workspace_root(&inside_crates),
            PathBuf::from("/tmp/radroots")
        );
        assert_eq!(workspace_root(&non_crates), PathBuf::from("/tmp/radroots"));
    }

    #[test]
    fn resolves_export_dir_for_override_and_fallback() {
        let override_dir = PathBuf::from("/tmp/radroots-ts-export");
        assert_eq!(
            ts_export_dir_from(Some("/tmp/radroots-ts-export")),
            override_dir
        );

        let expected = workspace_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .join("target")
            .join("ts-rs")
            .join("events");
        assert_eq!(ts_export_dir_from(None), expected);
    }
}

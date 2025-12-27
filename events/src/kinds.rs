pub const KIND_PROFILE: u32 = 0;
pub const KIND_POST: u32 = 1;
pub const KIND_FOLLOW: u32 = 3;
pub const KIND_REACTION: u32 = 7;
pub const KIND_SEAL: u32 = 13;
pub const KIND_MESSAGE: u32 = 14;
pub const KIND_MESSAGE_FILE: u32 = 15;
pub const KIND_GIFT_WRAP: u32 = 1059;
pub const KIND_COMMENT: u32 = 1111;
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
    use std::{env, fs, path::Path};

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
        ("KIND_LIST_SET_RELEASE_ARTIFACT", KIND_LIST_SET_RELEASE_ARTIFACT),
        ("KIND_LIST_SET_APP_CURATION", KIND_LIST_SET_APP_CURATION),
        ("KIND_LIST_SET_CALENDAR", KIND_LIST_SET_CALENDAR),
        ("KIND_LIST_SET_STARTER_PACK", KIND_LIST_SET_STARTER_PACK),
        ("KIND_LIST_SET_MEDIA_STARTER_PACK", KIND_LIST_SET_MEDIA_STARTER_PACK),
        ("KIND_FARM", KIND_FARM),
        ("KIND_PLOT", KIND_PLOT),
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
        let out_dir = env::var("TS_RS_EXPORT_DIR").unwrap_or_else(|_| "./bindings".to_string());
        let path = Path::new(&out_dir).join("kinds.ts");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create ts export dir");
        }
        let mut content = String::new();
        for (name, value) in KIND_EXPORTS {
            content.push_str(&format!("export const {name} = {value};\n"));
        }
        fs::write(&path, content).expect("write kinds");
    }
}

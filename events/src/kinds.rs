#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_PROFILE: u32 = 0;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_POST: u32 = 1;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_FOLLOW: u32 = 3;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_REACTION: u32 = 7;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_SEAL: u32 = 13;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_MESSAGE: u32 = 14;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_MESSAGE_FILE: u32 = 15;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_GIFT_WRAP: u32 = 1059;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_COMMENT: u32 = 1111;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_MUTE: u32 = 10000;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_PINNED_NOTES: u32 = 10001;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_READ_WRITE_RELAYS: u32 = 10002;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_BOOKMARKS: u32 = 10003;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_COMMUNITIES: u32 = 10004;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_PUBLIC_CHATS: u32 = 10005;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_BLOCKED_RELAYS: u32 = 10006;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SEARCH_RELAYS: u32 = 10007;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SIMPLE_GROUPS: u32 = 10009;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_RELAY_FEEDS: u32 = 10012;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_INTERESTS: u32 = 10015;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_MEDIA_FOLLOWS: u32 = 10020;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_EMOJIS: u32 = 10030;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_DM_RELAYS: u32 = 10050;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_GOOD_WIKI_AUTHORS: u32 = 10101;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_GOOD_WIKI_RELAYS: u32 = 10102;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_FOLLOW: u32 = 30000;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_GENERIC: u32 = 30001;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_RELAY: u32 = 30002;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_BOOKMARK: u32 = 30003;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_CURATION: u32 = 30004;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_VIDEO: u32 = 30005;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_PICTURE: u32 = 30006;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_KIND_MUTE: u32 = 30007;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_INTEREST: u32 = 30015;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_EMOJI: u32 = 30030;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_RELEASE_ARTIFACT: u32 = 30063;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_APP_CURATION: u32 = 30267;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_CALENDAR: u32 = 31924;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_STARTER_PACK: u32 = 39089;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LIST_SET_MEDIA_STARTER_PACK: u32 = 39092;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_FARM: u32 = 30340;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_PLOT: u32 = 30350;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_APP_DATA: u32 = 30078;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_LISTING: u32 = 30402;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_APPLICATION_HANDLER: u32 = 31990;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_JOB_REQUEST_MIN: u32 = 5000;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_JOB_REQUEST_MAX: u32 = 5999;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_JOB_RESULT_MIN: u32 = 6000;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
pub const KIND_JOB_RESULT_MAX: u32 = 6999;
#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
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

#[typeshare::typeshare]
pub const KIND_PROFILE: u32 = 0;
#[typeshare::typeshare]
pub const KIND_POST: u32 = 1;
#[typeshare::typeshare]
pub const KIND_FOLLOW: u32 = 3;
#[typeshare::typeshare]
pub const KIND_REACTION: u32 = 7;
#[typeshare::typeshare]
pub const KIND_SEAL: u32 = 13;
#[typeshare::typeshare]
pub const KIND_MESSAGE: u32 = 14;
#[typeshare::typeshare]
pub const KIND_MESSAGE_FILE: u32 = 15;
#[typeshare::typeshare]
pub const KIND_GIFT_WRAP: u32 = 1059;
#[typeshare::typeshare]
pub const KIND_COMMENT: u32 = 1111;
#[typeshare::typeshare]
pub const KIND_LIST_MUTE: u32 = 10000;
#[typeshare::typeshare]
pub const KIND_LIST_PINNED_NOTES: u32 = 10001;
#[typeshare::typeshare]
pub const KIND_LIST_READ_WRITE_RELAYS: u32 = 10002;
#[typeshare::typeshare]
pub const KIND_LIST_BOOKMARKS: u32 = 10003;
#[typeshare::typeshare]
pub const KIND_LIST_COMMUNITIES: u32 = 10004;
#[typeshare::typeshare]
pub const KIND_LIST_PUBLIC_CHATS: u32 = 10005;
#[typeshare::typeshare]
pub const KIND_LIST_BLOCKED_RELAYS: u32 = 10006;
#[typeshare::typeshare]
pub const KIND_LIST_SEARCH_RELAYS: u32 = 10007;
#[typeshare::typeshare]
pub const KIND_LIST_SIMPLE_GROUPS: u32 = 10009;
#[typeshare::typeshare]
pub const KIND_LIST_RELAY_FEEDS: u32 = 10012;
#[typeshare::typeshare]
pub const KIND_LIST_INTERESTS: u32 = 10015;
#[typeshare::typeshare]
pub const KIND_LIST_MEDIA_FOLLOWS: u32 = 10020;
#[typeshare::typeshare]
pub const KIND_LIST_EMOJIS: u32 = 10030;
#[typeshare::typeshare]
pub const KIND_LIST_DM_RELAYS: u32 = 10050;
#[typeshare::typeshare]
pub const KIND_LIST_GOOD_WIKI_AUTHORS: u32 = 10101;
#[typeshare::typeshare]
pub const KIND_LIST_GOOD_WIKI_RELAYS: u32 = 10102;
#[typeshare::typeshare]
pub const KIND_LIST_SET_FOLLOW: u32 = 30000;
#[typeshare::typeshare]
pub const KIND_LIST_SET_GENERIC: u32 = 30001;
#[typeshare::typeshare]
pub const KIND_LIST_SET_RELAY: u32 = 30002;
#[typeshare::typeshare]
pub const KIND_LIST_SET_BOOKMARK: u32 = 30003;
#[typeshare::typeshare]
pub const KIND_LIST_SET_CURATION: u32 = 30004;
#[typeshare::typeshare]
pub const KIND_LIST_SET_VIDEO: u32 = 30005;
#[typeshare::typeshare]
pub const KIND_LIST_SET_PICTURE: u32 = 30006;
#[typeshare::typeshare]
pub const KIND_LIST_SET_KIND_MUTE: u32 = 30007;
#[typeshare::typeshare]
pub const KIND_LIST_SET_INTEREST: u32 = 30015;
#[typeshare::typeshare]
pub const KIND_LIST_SET_EMOJI: u32 = 30030;
#[typeshare::typeshare]
pub const KIND_LIST_SET_RELEASE_ARTIFACT: u32 = 30063;
#[typeshare::typeshare]
pub const KIND_LIST_SET_APP_CURATION: u32 = 30267;
#[typeshare::typeshare]
pub const KIND_LIST_SET_CALENDAR: u32 = 31924;
#[typeshare::typeshare]
pub const KIND_LIST_SET_STARTER_PACK: u32 = 39089;
#[typeshare::typeshare]
pub const KIND_LIST_SET_MEDIA_STARTER_PACK: u32 = 39092;
#[typeshare::typeshare]
pub const KIND_APP_DATA: u32 = 30078;
#[typeshare::typeshare]
pub const KIND_LISTING: u32 = 30402;
#[typeshare::typeshare]
pub const KIND_APPLICATION_HANDLER: u32 = 31990;

#[typeshare::typeshare]
pub const KIND_JOB_REQUEST_MIN: u32 = 5000;
#[typeshare::typeshare]
pub const KIND_JOB_REQUEST_MAX: u32 = 5999;
#[typeshare::typeshare]
pub const KIND_JOB_RESULT_MIN: u32 = 6000;
#[typeshare::typeshare]
pub const KIND_JOB_RESULT_MAX: u32 = 6999;
#[typeshare::typeshare]
pub const KIND_JOB_FEEDBACK: u32 = 7000;

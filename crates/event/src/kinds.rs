pub const KIND_PROFILE: u32 = 0;
pub const KIND_POST: u32 = 1;
pub const KIND_FOLLOW: u32 = 3;
pub const KIND_DELETION_REQUEST: u32 = 5;
pub const KIND_REPOST: u32 = 6;
pub const KIND_REACTION: u32 = 7;
pub const KIND_SEAL: u32 = 13;
pub const KIND_MESSAGE: u32 = 14;
pub const KIND_MESSAGE_FILE: u32 = 15;
pub const KIND_GENERIC_REPOST: u32 = 16;
pub const KIND_APP_CUSTOM_DATA: u32 = 78;
pub const KIND_FARM_CRDT_CHANGE: u32 = KIND_APP_CUSTOM_DATA;
pub const KIND_GIFT_WRAP: u32 = 1059;
pub const KIND_FILE_METADATA: u32 = 1063;
pub const KIND_FARM_FILE_METADATA: u32 = KIND_FILE_METADATA;
pub const KIND_PUBLIC_FILE_METADATA: u32 = KIND_FILE_METADATA;
pub const KIND_COMMENT: u32 = 1111;
pub const KIND_REPORT: u32 = 1984;
pub const KIND_GROUP_PUT_USER: u32 = 9000;
pub const KIND_GROUP_REMOVE_USER: u32 = 9001;
pub const KIND_GROUP_EDIT_METADATA: u32 = 9002;
pub const KIND_GROUP_DELETE_EVENT: u32 = 9005;
pub const KIND_GROUP_CREATE_GROUP: u32 = 9007;
pub const KIND_GROUP_DELETE_GROUP: u32 = 9008;
pub const KIND_GROUP_CREATE_INVITE: u32 = 9009;
pub const KIND_GROUP_JOIN_REQUEST: u32 = 9021;
pub const KIND_GROUP_LEAVE_REQUEST: u32 = 9022;
pub const KIND_GEOCHAT: u32 = 20000;
pub const KIND_RELAY_AUTH: u32 = 22242;
pub const KIND_HTTP_AUTH: u32 = 27235;
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
pub const KIND_WIKI_MERGE_REQUEST: u32 = 818;
pub const KIND_WIKI_ARTICLE: u32 = 30818;
pub const KIND_WIKI_REDIRECT: u32 = 30819;
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
pub const KIND_ARTICLE: u32 = 30023;
pub const KIND_CALENDAR_DATE_EVENT: u32 = 31922;
pub const KIND_CALENDAR_TIME_EVENT: u32 = 31923;
pub const KIND_LIST_SET_CALENDAR: u32 = 31924;
pub const KIND_CALENDAR: u32 = KIND_LIST_SET_CALENDAR;
pub const KIND_CALENDAR_EVENT_RSVP: u32 = 31925;
pub const KIND_LIST_SET_STARTER_PACK: u32 = 39089;
pub const KIND_LIST_SET_MEDIA_STARTER_PACK: u32 = 39092;
pub const KIND_FARM: u32 = 30340;
pub const KIND_PLOT: u32 = 30350;
pub const KIND_COOP: u32 = 30360;
pub const KIND_DOCUMENT: u32 = 30361;
pub const KIND_RESOURCE_AREA: u32 = 30370;
pub const KIND_RESOURCE_HARVEST_CAP: u32 = 30371;
pub const KIND_ACCOUNT_CLAIM: u32 = 30380;
pub const KIND_VALIDATOR_SET: u32 = 30381;
pub const KIND_APP_DATA: u32 = 30078;
pub const KIND_FARM_WORKSPACE_MANIFEST: u32 = KIND_APP_DATA;
pub const KIND_CLASSIFIED_LISTING: u32 = 30402;
pub const KIND_APPLICATION_HANDLER: u32 = 31990;
pub const KIND_GROUP_METADATA: u32 = 39000;
pub const KIND_GROUP_ADMINS: u32 = 39001;
pub const KIND_GROUP_MEMBERS: u32 = 39002;
pub const KIND_GROUP_ROLES: u32 = 39003;

pub const KIND_ORDER_REQUEST: u32 = 3422;
pub const KIND_ORDER_DECISION: u32 = 3423;
pub const KIND_ORDER_CANCELLATION: u32 = 3432;
pub const KIND_TRADE_FORBIDDEN_3431: u32 = 3431;
pub const KIND_RHI_EVIDENCE_ATTESTATION: u32 = 3441;
pub const KIND_KNOWLEDGE_CLAIM: u32 = 3460;
pub const KIND_KNOWLEDGE_RELATION: u32 = 3461;
pub const KIND_KNOWLEDGE_REVIEW: u32 = 3462;
pub const KIND_KNOWLEDGE_FIELD_REPORT: u32 = 3463;
pub const KIND_KNOWLEDGE_CHANGE_PROPOSAL: u32 = 3464;
pub const KIND_CONTRIBUTION_ATTESTATION: u32 = 3465;
pub const KIND_TRADE_PROPOSAL: u32 = 3470;
pub const KIND_TRADE_DECISION: u32 = 3471;
pub const KIND_TRADE_REVISION_PROPOSAL: u32 = 3472;
pub const KIND_TRADE_REVISION_DECISION: u32 = 3473;
pub const KIND_TRADE_CANCELLATION: u32 = 3474;
pub const KIND_TRADE_SELLER_RESERVATION_ASSERTION: u32 = 3475;
pub const KIND_KNOWLEDGE_SOURCE: u32 = 30450;
pub const KIND_EVIDENCE_BOUNTY: u32 = 30451;

pub const CLASSIFIED_LISTING_EVENT_KINDS: [u32; 1] = [KIND_CLASSIFIED_LISTING];

pub const WIKI_EVENT_KINDS: [u32; 3] = [
    KIND_WIKI_MERGE_REQUEST,
    KIND_WIKI_ARTICLE,
    KIND_WIKI_REDIRECT,
];

pub const KNOWLEDGE_MVP_EVENT_KINDS: [u32; 8] = [
    KIND_WIKI_MERGE_REQUEST,
    KIND_WIKI_ARTICLE,
    KIND_WIKI_REDIRECT,
    KIND_KNOWLEDGE_SOURCE,
    KIND_KNOWLEDGE_CLAIM,
    KIND_KNOWLEDGE_RELATION,
    KIND_KNOWLEDGE_REVIEW,
    KIND_KNOWLEDGE_FIELD_REPORT,
];

pub const KNOWLEDGE_BETA_EVENT_KINDS: [u32; 3] = [
    KIND_EVIDENCE_BOUNTY,
    KIND_KNOWLEDGE_CHANGE_PROPOSAL,
    KIND_CONTRIBUTION_ATTESTATION,
];

pub const KNOWLEDGE_EVENT_KINDS: [u32; 11] = [
    KIND_WIKI_MERGE_REQUEST,
    KIND_WIKI_ARTICLE,
    KIND_WIKI_REDIRECT,
    KIND_KNOWLEDGE_SOURCE,
    KIND_KNOWLEDGE_CLAIM,
    KIND_KNOWLEDGE_RELATION,
    KIND_KNOWLEDGE_REVIEW,
    KIND_KNOWLEDGE_FIELD_REPORT,
    KIND_EVIDENCE_BOUNTY,
    KIND_KNOWLEDGE_CHANGE_PROPOSAL,
    KIND_CONTRIBUTION_ATTESTATION,
];

pub const KNOWLEDGE_ADDRESSABLE_EVENT_KINDS: [u32; 4] = [
    KIND_WIKI_ARTICLE,
    KIND_WIKI_REDIRECT,
    KIND_KNOWLEDGE_SOURCE,
    KIND_EVIDENCE_BOUNTY,
];

pub const KNOWLEDGE_REGULAR_EVENT_KINDS: [u32; 7] = [
    KIND_WIKI_MERGE_REQUEST,
    KIND_KNOWLEDGE_CLAIM,
    KIND_KNOWLEDGE_RELATION,
    KIND_KNOWLEDGE_REVIEW,
    KIND_KNOWLEDGE_FIELD_REPORT,
    KIND_KNOWLEDGE_CHANGE_PROPOSAL,
    KIND_CONTRIBUTION_ATTESTATION,
];

pub const ORDER_EVENT_KINDS: [u32; 3] = [
    KIND_ORDER_REQUEST,
    KIND_ORDER_DECISION,
    KIND_ORDER_CANCELLATION,
];

pub const TRADE_MUTATION_EVENT_KINDS: [u32; 5] = [
    KIND_TRADE_PROPOSAL,
    KIND_TRADE_DECISION,
    KIND_TRADE_REVISION_PROPOSAL,
    KIND_TRADE_REVISION_DECISION,
    KIND_TRADE_CANCELLATION,
];

pub const COMMERCIAL_EVENT_KINDS: [u32; 6] = [
    KIND_CLASSIFIED_LISTING,
    KIND_TRADE_PROPOSAL,
    KIND_TRADE_DECISION,
    KIND_TRADE_REVISION_PROPOSAL,
    KIND_TRADE_REVISION_DECISION,
    KIND_TRADE_CANCELLATION,
];

pub const KIND_JOB_REQUEST_MIN: u32 = 5000;
pub const KIND_JOB_REQUEST_MAX: u32 = 5999;
pub const KIND_JOB_RESULT_MIN: u32 = 6000;
pub const KIND_JOB_RESULT_MAX: u32 = 6999;
pub const KIND_JOB_FEEDBACK: u32 = 7000;

pub const HOME_FEED_CANDIDATE_KINDS: [u32; 9] = [
    KIND_POST,
    KIND_REPOST,
    KIND_GENERIC_REPOST,
    KIND_ARTICLE,
    KIND_CLASSIFIED_LISTING,
    KIND_CALENDAR_DATE_EVENT,
    KIND_CALENDAR_TIME_EVENT,
    KIND_FARM,
    KIND_PUBLIC_FILE_METADATA,
];

pub const EVENTS_CANDIDATE_KINDS: [u32; 4] = [
    KIND_CALENDAR_DATE_EVENT,
    KIND_CALENDAR_TIME_EVENT,
    KIND_CALENDAR,
    KIND_CALENDAR_EVENT_RSVP,
];

pub const MARKET_CANDIDATE_KINDS: [u32; 3] = [
    KIND_CLASSIFIED_LISTING,
    KIND_FARM,
    KIND_PUBLIC_FILE_METADATA,
];

pub const MAP_CANDIDATE_KINDS: [u32; 7] = [
    KIND_FARM,
    KIND_CLASSIFIED_LISTING,
    KIND_CALENDAR_DATE_EVENT,
    KIND_CALENDAR_TIME_EVENT,
    KIND_POST,
    KIND_ARTICLE,
    KIND_PUBLIC_FILE_METADATA,
];

pub const PROFILE_PUBLIC_CONTENT_KINDS: [u32; 8] = [
    KIND_POST,
    KIND_REPOST,
    KIND_GENERIC_REPOST,
    KIND_ARTICLE,
    KIND_CLASSIFIED_LISTING,
    KIND_CALENDAR_DATE_EVENT,
    KIND_CALENDAR_TIME_EVENT,
    KIND_PUBLIC_FILE_METADATA,
];

pub const MODERATION_ADMIN_CANDIDATE_KINDS: [u32; 1] = [KIND_REPORT];

pub const NIP29_GROUP_KINDS: [u32; 13] = [
    KIND_GROUP_METADATA,
    KIND_GROUP_ADMINS,
    KIND_GROUP_MEMBERS,
    KIND_GROUP_ROLES,
    KIND_GROUP_PUT_USER,
    KIND_GROUP_REMOVE_USER,
    KIND_GROUP_EDIT_METADATA,
    KIND_GROUP_DELETE_EVENT,
    KIND_GROUP_CREATE_GROUP,
    KIND_GROUP_DELETE_GROUP,
    KIND_GROUP_CREATE_INVITE,
    KIND_GROUP_JOIN_REQUEST,
    KIND_GROUP_LEAVE_REQUEST,
];

pub const PRIVATE_FARM_OPS_KINDS: [u32; 16] = [
    KIND_FARM_WORKSPACE_MANIFEST,
    KIND_FARM_CRDT_CHANGE,
    KIND_FARM_FILE_METADATA,
    KIND_GROUP_METADATA,
    KIND_GROUP_ADMINS,
    KIND_GROUP_MEMBERS,
    KIND_GROUP_ROLES,
    KIND_GROUP_PUT_USER,
    KIND_GROUP_REMOVE_USER,
    KIND_GROUP_EDIT_METADATA,
    KIND_GROUP_DELETE_EVENT,
    KIND_GROUP_CREATE_GROUP,
    KIND_GROUP_DELETE_GROUP,
    KIND_GROUP_CREATE_INVITE,
    KIND_GROUP_JOIN_REQUEST,
    KIND_GROUP_LEAVE_REQUEST,
];

pub const PUBLIC_SOCIAL_KINDS: [u32; 12] = [
    KIND_POST,
    KIND_DELETION_REQUEST,
    KIND_REPOST,
    KIND_REACTION,
    KIND_GENERIC_REPOST,
    KIND_PUBLIC_FILE_METADATA,
    KIND_COMMENT,
    KIND_ARTICLE,
    KIND_CALENDAR_DATE_EVENT,
    KIND_CALENDAR_TIME_EVENT,
    KIND_CALENDAR,
    KIND_CALENDAR_EVENT_RSVP,
];

pub const UNAMBIGUOUS_PUBLIC_SOCIAL_KINDS: [u32; 11] = [
    KIND_POST,
    KIND_DELETION_REQUEST,
    KIND_REPOST,
    KIND_REACTION,
    KIND_GENERIC_REPOST,
    KIND_COMMENT,
    KIND_ARTICLE,
    KIND_CALENDAR_DATE_EVENT,
    KIND_CALENDAR_TIME_EVENT,
    KIND_CALENDAR,
    KIND_CALENDAR_EVENT_RSVP,
];

pub const MVP_SOCIAL_KINDS: [u32; 7] = [
    KIND_POST,
    KIND_DELETION_REQUEST,
    KIND_PUBLIC_FILE_METADATA,
    KIND_COMMENT,
    KIND_ARTICLE,
    KIND_CALENDAR_DATE_EVENT,
    KIND_CALENDAR_TIME_EVENT,
];

pub const PRODUCTION_SOCIAL_KINDS: [u32; 4] = [
    KIND_REPOST,
    KIND_GENERIC_REPOST,
    KIND_CALENDAR,
    KIND_CALENDAR_EVENT_RSVP,
];

#[inline]
pub const fn is_classified_listing_kind(kind: u32) -> bool {
    kind == KIND_CLASSIFIED_LISTING
}

#[inline]
pub const fn is_public_file_metadata_kind(kind: u32) -> bool {
    kind == KIND_PUBLIC_FILE_METADATA
}

#[inline]
pub const fn is_ambiguous_public_social_kind(kind: u32) -> bool {
    kind == KIND_PUBLIC_FILE_METADATA
}

#[inline]
pub const fn is_unambiguous_public_social_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_POST
            | KIND_DELETION_REQUEST
            | KIND_REPOST
            | KIND_REACTION
            | KIND_GENERIC_REPOST
            | KIND_COMMENT
            | KIND_ARTICLE
            | KIND_CALENDAR_DATE_EVENT
            | KIND_CALENDAR_TIME_EVENT
            | KIND_CALENDAR
            | KIND_CALENDAR_EVENT_RSVP
    )
}

#[inline]
pub const fn is_public_social_kind(kind: u32) -> bool {
    is_unambiguous_public_social_kind(kind) || is_ambiguous_public_social_kind(kind)
}

#[inline]
pub const fn is_mvp_social_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_POST
            | KIND_DELETION_REQUEST
            | KIND_PUBLIC_FILE_METADATA
            | KIND_COMMENT
            | KIND_ARTICLE
            | KIND_CALENDAR_DATE_EVENT
            | KIND_CALENDAR_TIME_EVENT
    )
}

#[inline]
pub const fn is_production_social_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_REPOST | KIND_GENERIC_REPOST | KIND_CALENDAR | KIND_CALENDAR_EVENT_RSVP
    )
}

#[inline]
pub const fn is_wiki_event_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_WIKI_MERGE_REQUEST | KIND_WIKI_ARTICLE | KIND_WIKI_REDIRECT
    )
}

#[inline]
pub const fn is_knowledge_mvp_event_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_WIKI_MERGE_REQUEST
            | KIND_WIKI_ARTICLE
            | KIND_WIKI_REDIRECT
            | KIND_KNOWLEDGE_SOURCE
            | KIND_KNOWLEDGE_CLAIM
            | KIND_KNOWLEDGE_RELATION
            | KIND_KNOWLEDGE_REVIEW
            | KIND_KNOWLEDGE_FIELD_REPORT
    )
}

#[inline]
pub const fn is_knowledge_beta_event_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_EVIDENCE_BOUNTY | KIND_KNOWLEDGE_CHANGE_PROPOSAL | KIND_CONTRIBUTION_ATTESTATION
    )
}

#[inline]
pub const fn is_knowledge_event_kind(kind: u32) -> bool {
    is_knowledge_mvp_event_kind(kind) || is_knowledge_beta_event_kind(kind)
}

#[inline]
pub const fn is_knowledge_addressable_event_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_WIKI_ARTICLE | KIND_WIKI_REDIRECT | KIND_KNOWLEDGE_SOURCE | KIND_EVIDENCE_BOUNTY
    )
}

#[inline]
pub const fn is_knowledge_regular_event_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_WIKI_MERGE_REQUEST
            | KIND_KNOWLEDGE_CLAIM
            | KIND_KNOWLEDGE_RELATION
            | KIND_KNOWLEDGE_REVIEW
            | KIND_KNOWLEDGE_FIELD_REPORT
            | KIND_KNOWLEDGE_CHANGE_PROPOSAL
            | KIND_CONTRIBUTION_ATTESTATION
    )
}

#[inline]
pub const fn is_home_feed_candidate_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_POST
            | KIND_REPOST
            | KIND_GENERIC_REPOST
            | KIND_ARTICLE
            | KIND_CLASSIFIED_LISTING
            | KIND_CALENDAR_DATE_EVENT
            | KIND_CALENDAR_TIME_EVENT
            | KIND_FARM
            | KIND_PUBLIC_FILE_METADATA
    )
}

#[inline]
pub const fn is_events_candidate_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_CALENDAR_DATE_EVENT
            | KIND_CALENDAR_TIME_EVENT
            | KIND_CALENDAR
            | KIND_CALENDAR_EVENT_RSVP
    )
}

#[inline]
pub const fn is_market_candidate_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_CLASSIFIED_LISTING | KIND_FARM | KIND_PUBLIC_FILE_METADATA
    )
}

#[inline]
pub const fn is_map_candidate_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_FARM
            | KIND_CLASSIFIED_LISTING
            | KIND_CALENDAR_DATE_EVENT
            | KIND_CALENDAR_TIME_EVENT
            | KIND_POST
            | KIND_ARTICLE
            | KIND_PUBLIC_FILE_METADATA
    )
}

#[inline]
pub const fn is_profile_public_content_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_POST
            | KIND_REPOST
            | KIND_GENERIC_REPOST
            | KIND_ARTICLE
            | KIND_CLASSIFIED_LISTING
            | KIND_CALENDAR_DATE_EVENT
            | KIND_CALENDAR_TIME_EVENT
            | KIND_PUBLIC_FILE_METADATA
    )
}

#[inline]
pub const fn is_moderation_admin_candidate_kind(kind: u32) -> bool {
    kind == KIND_REPORT
}

#[inline]
pub const fn is_nip29_group_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_GROUP_METADATA
            | KIND_GROUP_ADMINS
            | KIND_GROUP_MEMBERS
            | KIND_GROUP_ROLES
            | KIND_GROUP_PUT_USER
            | KIND_GROUP_REMOVE_USER
            | KIND_GROUP_EDIT_METADATA
            | KIND_GROUP_DELETE_EVENT
            | KIND_GROUP_CREATE_GROUP
            | KIND_GROUP_DELETE_GROUP
            | KIND_GROUP_CREATE_INVITE
            | KIND_GROUP_JOIN_REQUEST
            | KIND_GROUP_LEAVE_REQUEST
    )
}

#[inline]
pub const fn is_private_farm_ops_kind(kind: u32) -> bool {
    kind == KIND_FARM_WORKSPACE_MANIFEST
        || kind == KIND_FARM_CRDT_CHANGE
        || kind == KIND_FARM_FILE_METADATA
        || is_nip29_group_kind(kind)
}

#[inline]
pub const fn is_order_event_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_ORDER_REQUEST | KIND_ORDER_DECISION | KIND_ORDER_CANCELLATION
    )
}

#[inline]
pub const fn is_trade_mutation_event_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_TRADE_PROPOSAL
            | KIND_TRADE_DECISION
            | KIND_TRADE_REVISION_PROPOSAL
            | KIND_TRADE_REVISION_DECISION
            | KIND_TRADE_CANCELLATION
    )
}

#[inline]
pub const fn is_commercial_event_kind(kind: u32) -> bool {
    is_classified_listing_kind(kind) || is_trade_mutation_event_kind(kind)
}

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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

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
    fn exposes_field_event_kind_aliases() {
        assert_eq!(KIND_APP_CUSTOM_DATA, 78);
        assert_eq!(KIND_FARM_CRDT_CHANGE, KIND_APP_CUSTOM_DATA);
        assert_eq!(KIND_FILE_METADATA, 1063);
        assert_eq!(KIND_FARM_FILE_METADATA, KIND_FILE_METADATA);
        assert_eq!(KIND_PUBLIC_FILE_METADATA, KIND_FILE_METADATA);
        assert_eq!(KIND_FARM_WORKSPACE_MANIFEST, KIND_APP_DATA);
        assert_eq!(KIND_RELAY_AUTH, 22242);
        assert_eq!(KIND_HTTP_AUTH, 27235);
    }

    #[test]
    fn exposes_social_event_kind_constants() {
        assert_eq!(KIND_DELETION_REQUEST, 5);
        assert_eq!(KIND_REPOST, 6);
        assert_eq!(KIND_GENERIC_REPOST, 16);
        assert_eq!(KIND_REPORT, 1984);
        assert_eq!(KIND_ARTICLE, 30023);
        assert_eq!(KIND_CALENDAR_DATE_EVENT, 31922);
        assert_eq!(KIND_CALENDAR_TIME_EVENT, 31923);
        assert_eq!(KIND_CALENDAR, KIND_LIST_SET_CALENDAR);
        assert_eq!(KIND_CALENDAR_EVENT_RSVP, 31925);
    }

    #[test]
    fn classifies_public_social_kinds() {
        assert_eq!(PUBLIC_SOCIAL_KINDS.len(), 12);
        assert_eq!(UNAMBIGUOUS_PUBLIC_SOCIAL_KINDS.len(), 11);
        assert_eq!(MVP_SOCIAL_KINDS.len(), 7);
        assert_eq!(PRODUCTION_SOCIAL_KINDS.len(), 4);

        assert!(is_public_social_kind(KIND_POST));
        assert!(is_public_social_kind(KIND_DELETION_REQUEST));
        assert!(is_public_social_kind(KIND_PUBLIC_FILE_METADATA));
        assert!(is_public_file_metadata_kind(KIND_PUBLIC_FILE_METADATA));
        assert!(!is_public_file_metadata_kind(KIND_POST));
        assert!(is_public_social_kind(KIND_COMMENT));
        assert!(is_public_social_kind(KIND_REACTION));
        assert!(is_public_social_kind(KIND_ARTICLE));
        assert!(is_public_social_kind(KIND_CALENDAR_DATE_EVENT));
        assert!(is_public_social_kind(KIND_CALENDAR_TIME_EVENT));
        assert!(is_public_social_kind(KIND_REPOST));
        assert!(is_public_social_kind(KIND_GENERIC_REPOST));
        assert!(is_public_social_kind(KIND_CALENDAR));
        assert!(is_public_social_kind(KIND_CALENDAR_EVENT_RSVP));
        assert!(!is_public_social_kind(KIND_REPORT));
        assert!(!is_public_social_kind(KIND_CLASSIFIED_LISTING));
        assert!(!is_public_social_kind(KIND_LIST_READ_WRITE_RELAYS));
        assert!(!is_public_social_kind(KIND_FARM_CRDT_CHANGE));
        assert!(!is_public_social_kind(KIND_FARM_WORKSPACE_MANIFEST));

        assert!(is_mvp_social_kind(KIND_ARTICLE));
        assert!(is_mvp_social_kind(KIND_COMMENT));
        assert!(is_mvp_social_kind(KIND_DELETION_REQUEST));
        assert!(!is_mvp_social_kind(KIND_REPORT));
        assert!(!is_production_social_kind(KIND_REPORT));
        assert!(!is_production_social_kind(KIND_ARTICLE));
        assert!(is_ambiguous_public_social_kind(KIND_PUBLIC_FILE_METADATA));
        assert!(!is_unambiguous_public_social_kind(
            KIND_PUBLIC_FILE_METADATA
        ));
        assert!(is_unambiguous_public_social_kind(KIND_ARTICLE));
        assert!(is_unambiguous_public_social_kind(KIND_DELETION_REQUEST));
    }

    #[test]
    fn classifies_knowledge_event_kinds() {
        assert_eq!(
            WIKI_EVENT_KINDS,
            [
                KIND_WIKI_MERGE_REQUEST,
                KIND_WIKI_ARTICLE,
                KIND_WIKI_REDIRECT
            ]
        );
        assert_eq!(KNOWLEDGE_MVP_EVENT_KINDS.len(), 8);
        assert_eq!(KNOWLEDGE_BETA_EVENT_KINDS.len(), 3);
        assert_eq!(KNOWLEDGE_EVENT_KINDS.len(), 11);
        assert_eq!(KNOWLEDGE_ADDRESSABLE_EVENT_KINDS.len(), 4);
        assert_eq!(KNOWLEDGE_REGULAR_EVENT_KINDS.len(), 7);

        assert_eq!(KIND_WIKI_MERGE_REQUEST, 818);
        assert_eq!(KIND_WIKI_ARTICLE, 30818);
        assert_eq!(KIND_WIKI_REDIRECT, 30819);
        assert_eq!(KIND_KNOWLEDGE_SOURCE, 30450);
        assert_eq!(KIND_EVIDENCE_BOUNTY, 30451);
        assert_eq!(KIND_KNOWLEDGE_CLAIM, 3460);
        assert_eq!(KIND_KNOWLEDGE_RELATION, 3461);
        assert_eq!(KIND_KNOWLEDGE_REVIEW, 3462);
        assert_eq!(KIND_KNOWLEDGE_FIELD_REPORT, 3463);
        assert_eq!(KIND_KNOWLEDGE_CHANGE_PROPOSAL, 3464);
        assert_eq!(KIND_CONTRIBUTION_ATTESTATION, 3465);

        assert!(is_wiki_event_kind(KIND_WIKI_MERGE_REQUEST));
        assert!(is_wiki_event_kind(KIND_WIKI_ARTICLE));
        assert!(is_wiki_event_kind(KIND_WIKI_REDIRECT));
        assert!(!is_wiki_event_kind(KIND_KNOWLEDGE_CLAIM));
        assert!(is_knowledge_event_kind(KIND_WIKI_ARTICLE));
        assert!(is_knowledge_event_kind(KIND_KNOWLEDGE_CLAIM));
        assert!(is_knowledge_mvp_event_kind(KIND_KNOWLEDGE_FIELD_REPORT));
        assert!(is_knowledge_beta_event_kind(KIND_EVIDENCE_BOUNTY));
        assert!(is_knowledge_addressable_event_kind(KIND_KNOWLEDGE_SOURCE));
        assert!(is_knowledge_addressable_event_kind(KIND_EVIDENCE_BOUNTY));
        assert!(is_knowledge_regular_event_kind(KIND_WIKI_MERGE_REQUEST));
        assert!(is_knowledge_regular_event_kind(
            KIND_CONTRIBUTION_ATTESTATION
        ));
        assert!(!is_knowledge_regular_event_kind(KIND_KNOWLEDGE_SOURCE));
        assert!(!is_knowledge_addressable_event_kind(KIND_KNOWLEDGE_CLAIM));
        assert!(!is_knowledge_event_kind(KIND_ARTICLE));
    }

    #[test]
    fn classifies_product_surface_candidate_kinds() {
        assert_eq!(HOME_FEED_CANDIDATE_KINDS.len(), 9);
        assert_eq!(EVENTS_CANDIDATE_KINDS.len(), 4);
        assert_eq!(MARKET_CANDIDATE_KINDS.len(), 3);
        assert_eq!(MAP_CANDIDATE_KINDS.len(), 7);
        assert_eq!(PROFILE_PUBLIC_CONTENT_KINDS.len(), 8);
        assert_eq!(MODERATION_ADMIN_CANDIDATE_KINDS.len(), 1);
        assert_eq!(NIP29_GROUP_KINDS.len(), 13);
        assert_eq!(PRIVATE_FARM_OPS_KINDS.len(), 16);

        assert!(is_home_feed_candidate_kind(KIND_POST));
        assert!(is_home_feed_candidate_kind(KIND_REPOST));
        assert!(is_home_feed_candidate_kind(KIND_GENERIC_REPOST));
        assert!(is_home_feed_candidate_kind(KIND_ARTICLE));
        assert!(is_home_feed_candidate_kind(KIND_CLASSIFIED_LISTING));
        assert!(is_home_feed_candidate_kind(KIND_CALENDAR_DATE_EVENT));
        assert!(is_home_feed_candidate_kind(KIND_CALENDAR_TIME_EVENT));
        assert!(is_home_feed_candidate_kind(KIND_FARM));
        assert!(is_home_feed_candidate_kind(KIND_PUBLIC_FILE_METADATA));
        assert!(!is_home_feed_candidate_kind(KIND_REPORT));
        assert!(!is_home_feed_candidate_kind(KIND_FARM_CRDT_CHANGE));
        assert!(!is_home_feed_candidate_kind(KIND_RELAY_AUTH));
        assert!(!is_home_feed_candidate_kind(KIND_HTTP_AUTH));

        assert!(is_events_candidate_kind(KIND_CALENDAR_DATE_EVENT));
        assert!(is_events_candidate_kind(KIND_CALENDAR_TIME_EVENT));
        assert!(is_events_candidate_kind(KIND_CALENDAR));
        assert!(is_events_candidate_kind(KIND_CALENDAR_EVENT_RSVP));
        assert!(!is_events_candidate_kind(KIND_POST));
        assert!(!is_events_candidate_kind(KIND_FARM_CRDT_CHANGE));

        assert!(is_market_candidate_kind(KIND_CLASSIFIED_LISTING));
        assert!(is_market_candidate_kind(KIND_FARM));
        assert!(is_market_candidate_kind(KIND_PUBLIC_FILE_METADATA));
        assert!(!is_market_candidate_kind(KIND_REPORT));

        assert!(is_map_candidate_kind(KIND_FARM));
        assert!(is_map_candidate_kind(KIND_CLASSIFIED_LISTING));
        assert!(is_map_candidate_kind(KIND_CALENDAR_DATE_EVENT));
        assert!(is_map_candidate_kind(KIND_CALENDAR_TIME_EVENT));
        assert!(is_map_candidate_kind(KIND_POST));
        assert!(is_map_candidate_kind(KIND_ARTICLE));
        assert!(is_map_candidate_kind(KIND_PUBLIC_FILE_METADATA));
        assert!(!is_map_candidate_kind(KIND_REPORT));

        assert!(is_profile_public_content_kind(KIND_POST));
        assert!(is_profile_public_content_kind(KIND_REPOST));
        assert!(is_profile_public_content_kind(KIND_GENERIC_REPOST));
        assert!(is_profile_public_content_kind(KIND_ARTICLE));
        assert!(is_profile_public_content_kind(KIND_CLASSIFIED_LISTING));
        assert!(is_profile_public_content_kind(KIND_CALENDAR_DATE_EVENT));
        assert!(is_profile_public_content_kind(KIND_CALENDAR_TIME_EVENT));
        assert!(is_profile_public_content_kind(KIND_PUBLIC_FILE_METADATA));
        assert!(!is_profile_public_content_kind(KIND_REPORT));

        assert!(is_moderation_admin_candidate_kind(KIND_REPORT));
        assert!(!is_moderation_admin_candidate_kind(KIND_POST));

        assert!(is_private_farm_ops_kind(KIND_FARM_WORKSPACE_MANIFEST));
        assert!(is_private_farm_ops_kind(KIND_FARM_CRDT_CHANGE));
        assert!(is_private_farm_ops_kind(KIND_FARM_FILE_METADATA));
        assert!(is_nip29_group_kind(KIND_GROUP_METADATA));
        assert!(is_private_farm_ops_kind(KIND_GROUP_METADATA));
        assert!(is_private_farm_ops_kind(KIND_GROUP_ADMINS));
        assert!(is_private_farm_ops_kind(KIND_GROUP_MEMBERS));
        assert!(is_private_farm_ops_kind(KIND_GROUP_ROLES));
        assert!(is_private_farm_ops_kind(KIND_GROUP_PUT_USER));
        assert!(is_private_farm_ops_kind(KIND_GROUP_REMOVE_USER));
        assert!(is_private_farm_ops_kind(KIND_GROUP_EDIT_METADATA));
        assert!(is_private_farm_ops_kind(KIND_GROUP_DELETE_EVENT));
        assert!(is_private_farm_ops_kind(KIND_GROUP_CREATE_GROUP));
        assert!(is_private_farm_ops_kind(KIND_GROUP_DELETE_GROUP));
        assert!(is_private_farm_ops_kind(KIND_GROUP_CREATE_INVITE));
        assert!(is_private_farm_ops_kind(KIND_GROUP_JOIN_REQUEST));
        assert!(is_private_farm_ops_kind(KIND_GROUP_LEAVE_REQUEST));
        assert!(!is_private_farm_ops_kind(KIND_RELAY_AUTH));
        assert!(!is_private_farm_ops_kind(KIND_HTTP_AUTH));
        assert!(!is_private_farm_ops_kind(KIND_REPORT));
    }

    #[test]
    fn exposes_nip29_group_kind_constants() {
        assert_eq!(KIND_GROUP_PUT_USER, 9000);
        assert_eq!(KIND_GROUP_REMOVE_USER, 9001);
        assert_eq!(KIND_GROUP_EDIT_METADATA, 9002);
        assert_eq!(KIND_GROUP_DELETE_EVENT, 9005);
        assert_eq!(KIND_GROUP_CREATE_GROUP, 9007);
        assert_eq!(KIND_GROUP_DELETE_GROUP, 9008);
        assert_eq!(KIND_GROUP_CREATE_INVITE, 9009);
        assert_eq!(KIND_GROUP_JOIN_REQUEST, 9021);
        assert_eq!(KIND_GROUP_LEAVE_REQUEST, 9022);
        assert_eq!(KIND_GROUP_METADATA, 39000);
        assert_eq!(KIND_GROUP_ADMINS, 39001);
        assert_eq!(KIND_GROUP_MEMBERS, 39002);
        assert_eq!(KIND_GROUP_ROLES, 39003);
    }

    #[test]
    fn classifies_commercial_event_kinds() {
        assert_eq!(CLASSIFIED_LISTING_EVENT_KINDS, [KIND_CLASSIFIED_LISTING]);
        assert_eq!(
            ORDER_EVENT_KINDS,
            [
                KIND_ORDER_REQUEST,
                KIND_ORDER_DECISION,
                KIND_ORDER_CANCELLATION
            ]
        );
        assert_eq!(
            TRADE_MUTATION_EVENT_KINDS,
            [
                KIND_TRADE_PROPOSAL,
                KIND_TRADE_DECISION,
                KIND_TRADE_REVISION_PROPOSAL,
                KIND_TRADE_REVISION_DECISION,
                KIND_TRADE_CANCELLATION,
            ]
        );
        assert_eq!(
            COMMERCIAL_EVENT_KINDS,
            [
                KIND_CLASSIFIED_LISTING,
                KIND_TRADE_PROPOSAL,
                KIND_TRADE_DECISION,
                KIND_TRADE_REVISION_PROPOSAL,
                KIND_TRADE_REVISION_DECISION,
                KIND_TRADE_CANCELLATION,
            ]
        );

        assert!(is_classified_listing_kind(KIND_CLASSIFIED_LISTING));
        assert!(!is_classified_listing_kind(KIND_PROFILE));

        assert!(is_order_event_kind(KIND_ORDER_REQUEST));
        assert!(is_order_event_kind(KIND_ORDER_DECISION));
        assert!(is_order_event_kind(KIND_ORDER_CANCELLATION));
        assert!(!is_order_event_kind(3424));
        assert!(!is_order_event_kind(3425));
        assert!(!is_order_event_kind(3433));
        assert!(!is_order_event_kind(3434));
        assert!(!is_order_event_kind(3435));
        assert!(!is_order_event_kind(3436));
        assert!(!is_order_event_kind(3440));
        assert!(!is_order_event_kind(3431));

        assert!(is_trade_mutation_event_kind(KIND_TRADE_PROPOSAL));
        assert!(is_trade_mutation_event_kind(KIND_TRADE_DECISION));
        assert!(is_trade_mutation_event_kind(KIND_TRADE_REVISION_PROPOSAL));
        assert!(is_trade_mutation_event_kind(KIND_TRADE_REVISION_DECISION));
        assert!(is_trade_mutation_event_kind(KIND_TRADE_CANCELLATION));
        assert!(!is_trade_mutation_event_kind(KIND_ORDER_REQUEST));
        assert!(!is_trade_mutation_event_kind(3440));

        assert!(is_commercial_event_kind(KIND_CLASSIFIED_LISTING));
        assert!(is_commercial_event_kind(KIND_TRADE_PROPOSAL));
        assert!(!is_commercial_event_kind(3440));
        assert!(!is_commercial_event_kind(KIND_ORDER_REQUEST));
        assert!(!is_commercial_event_kind(KIND_PROFILE));
        assert!(!is_commercial_event_kind(30403));
        assert!(!is_commercial_event_kind(5321));
        assert!(!is_commercial_event_kind(6321));
    }
}

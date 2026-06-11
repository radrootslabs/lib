pub const KIND_PROFILE: u32 = 0;
pub const KIND_POST: u32 = 1;
pub const KIND_FOLLOW: u32 = 3;
pub const KIND_REACTION: u32 = 7;
pub const KIND_SEAL: u32 = 13;
pub const KIND_MESSAGE: u32 = 14;
pub const KIND_MESSAGE_FILE: u32 = 15;
pub const KIND_APP_CUSTOM_DATA: u32 = 78;
pub const KIND_FARM_CRDT_CHANGE: u32 = KIND_APP_CUSTOM_DATA;
pub const KIND_GIFT_WRAP: u32 = 1059;
pub const KIND_FILE_METADATA: u32 = 1063;
pub const KIND_FARM_FILE_METADATA: u32 = KIND_FILE_METADATA;
pub const KIND_COMMENT: u32 = 1111;
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
pub const KIND_FARM_WORKSPACE_MANIFEST: u32 = KIND_APP_DATA;
pub const KIND_LISTING: u32 = 30402;
pub const KIND_LISTING_DRAFT: u32 = 30403;
pub const KIND_APPLICATION_HANDLER: u32 = 31990;
pub const KIND_GROUP_METADATA: u32 = 39000;
pub const KIND_GROUP_ADMINS: u32 = 39001;
pub const KIND_GROUP_MEMBERS: u32 = 39002;
pub const KIND_GROUP_ROLES: u32 = 39003;

pub const KIND_TRADE_LISTING_VALIDATE_REQ: u32 = 5321;
pub const KIND_TRADE_LISTING_VALIDATE_RES: u32 = 6321;
pub const KIND_WORKER_TRADE_TRANSITION_PROOF_REQ: u32 = 5322;
pub const KIND_WORKER_TRADE_TRANSITION_PROOF_RES: u32 = 6322;
pub const KIND_TRADE_ORDER_REQUEST: u32 = 3422;
pub const KIND_TRADE_ORDER_RESPONSE: u32 = 3423;
pub const KIND_TRADE_ORDER_DECISION: u32 = KIND_TRADE_ORDER_RESPONSE;
pub const KIND_TRADE_ORDER_REVISION: u32 = 3424;
pub const KIND_TRADE_ORDER_REVISION_RESPONSE: u32 = 3425;
pub const KIND_TRADE_QUESTION: u32 = 3426;
pub const KIND_TRADE_ANSWER: u32 = 3427;
pub const KIND_TRADE_DISCOUNT_REQUEST: u32 = 3428;
pub const KIND_TRADE_DISCOUNT_OFFER: u32 = 3429;
pub const KIND_TRADE_DISCOUNT_ACCEPT: u32 = 3430;
pub const KIND_TRADE_FORBIDDEN_3431: u32 = 3431;
pub const KIND_TRADE_DISCOUNT_DECLINE: u32 = KIND_TRADE_FORBIDDEN_3431;
pub const KIND_TRADE_CANCEL: u32 = 3432;
pub const KIND_TRADE_FULFILLMENT_UPDATE: u32 = 3433;
pub const KIND_TRADE_RECEIPT: u32 = 3434;
pub const KIND_TRADE_VALIDATION_RECEIPT: u32 = 3440;
pub const KIND_TRADE_PAYMENT_RECORDED: u32 = 3435;
pub const KIND_TRADE_SETTLEMENT_DECISION: u32 = 3436;

pub const KIND_TRADE_LISTING_ORDER_REQ: u32 = KIND_TRADE_ORDER_REQUEST;
pub const KIND_TRADE_LISTING_ORDER_RES: u32 = KIND_TRADE_ORDER_RESPONSE;
pub const KIND_TRADE_LISTING_ORDER_REVISION_REQ: u32 = KIND_TRADE_ORDER_REVISION;
pub const KIND_TRADE_LISTING_ORDER_REVISION_RES: u32 = KIND_TRADE_ORDER_REVISION_RESPONSE;
pub const KIND_TRADE_LISTING_QUESTION_REQ: u32 = KIND_TRADE_QUESTION;
pub const KIND_TRADE_LISTING_ANSWER_RES: u32 = KIND_TRADE_ANSWER;
pub const KIND_TRADE_LISTING_DISCOUNT_REQ: u32 = KIND_TRADE_DISCOUNT_REQUEST;
pub const KIND_TRADE_LISTING_DISCOUNT_OFFER_RES: u32 = KIND_TRADE_DISCOUNT_OFFER;
pub const KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ: u32 = KIND_TRADE_DISCOUNT_ACCEPT;
pub const KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ: u32 = KIND_TRADE_FORBIDDEN_3431;
pub const KIND_TRADE_LISTING_CANCEL_REQ: u32 = KIND_TRADE_CANCEL;
pub const KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ: u32 = KIND_TRADE_FULFILLMENT_UPDATE;
pub const KIND_TRADE_LISTING_RECEIPT_REQ: u32 = KIND_TRADE_RECEIPT;

pub const TRADE_SERVICE_KINDS: [u32; 4] = [
    KIND_TRADE_LISTING_VALIDATE_REQ,
    KIND_TRADE_LISTING_VALIDATE_RES,
    KIND_WORKER_TRADE_TRANSITION_PROOF_REQ,
    KIND_WORKER_TRADE_TRANSITION_PROOF_RES,
];

pub const TRADE_PUBLIC_KINDS: [u32; 14] = [
    KIND_TRADE_ORDER_REQUEST,
    KIND_TRADE_ORDER_RESPONSE,
    KIND_TRADE_ORDER_REVISION,
    KIND_TRADE_ORDER_REVISION_RESPONSE,
    KIND_TRADE_QUESTION,
    KIND_TRADE_ANSWER,
    KIND_TRADE_DISCOUNT_REQUEST,
    KIND_TRADE_DISCOUNT_OFFER,
    KIND_TRADE_DISCOUNT_ACCEPT,
    KIND_TRADE_CANCEL,
    KIND_TRADE_FULFILLMENT_UPDATE,
    KIND_TRADE_RECEIPT,
    KIND_TRADE_PAYMENT_RECORDED,
    KIND_TRADE_SETTLEMENT_DECISION,
];

pub const TRADE_KINDS: [u32; 18] = [
    KIND_TRADE_LISTING_VALIDATE_REQ,
    KIND_TRADE_LISTING_VALIDATE_RES,
    KIND_WORKER_TRADE_TRANSITION_PROOF_REQ,
    KIND_WORKER_TRADE_TRANSITION_PROOF_RES,
    KIND_TRADE_ORDER_REQUEST,
    KIND_TRADE_ORDER_RESPONSE,
    KIND_TRADE_ORDER_REVISION,
    KIND_TRADE_ORDER_REVISION_RESPONSE,
    KIND_TRADE_QUESTION,
    KIND_TRADE_ANSWER,
    KIND_TRADE_DISCOUNT_REQUEST,
    KIND_TRADE_DISCOUNT_OFFER,
    KIND_TRADE_DISCOUNT_ACCEPT,
    KIND_TRADE_CANCEL,
    KIND_TRADE_FULFILLMENT_UPDATE,
    KIND_TRADE_RECEIPT,
    KIND_TRADE_PAYMENT_RECORDED,
    KIND_TRADE_SETTLEMENT_DECISION,
];

pub const TRADE_LISTING_KINDS: [u32; 18] = TRADE_KINDS;

pub const ACTIVE_TRADE_LISTING_KINDS: [u32; 2] = [KIND_LISTING, KIND_LISTING_DRAFT];

pub const ACTIVE_TRADE_PUBLIC_KINDS: [u32; 9] = [
    KIND_TRADE_ORDER_REQUEST,
    KIND_TRADE_ORDER_DECISION,
    KIND_TRADE_ORDER_REVISION,
    KIND_TRADE_ORDER_REVISION_RESPONSE,
    KIND_TRADE_CANCEL,
    KIND_TRADE_FULFILLMENT_UPDATE,
    KIND_TRADE_RECEIPT,
    KIND_TRADE_PAYMENT_RECORDED,
    KIND_TRADE_SETTLEMENT_DECISION,
];

pub const ACTIVE_TRADE_KINDS: [u32; 11] = [
    KIND_LISTING,
    KIND_LISTING_DRAFT,
    KIND_TRADE_ORDER_REQUEST,
    KIND_TRADE_ORDER_DECISION,
    KIND_TRADE_ORDER_REVISION,
    KIND_TRADE_ORDER_REVISION_RESPONSE,
    KIND_TRADE_CANCEL,
    KIND_TRADE_FULFILLMENT_UPDATE,
    KIND_TRADE_RECEIPT,
    KIND_TRADE_PAYMENT_RECORDED,
    KIND_TRADE_SETTLEMENT_DECISION,
];

pub const TRADE_VALIDATION_RECEIPT_KINDS: [u32; 1] = [KIND_TRADE_VALIDATION_RECEIPT];

pub const KIND_JOB_REQUEST_MIN: u32 = 5000;
pub const KIND_JOB_REQUEST_MAX: u32 = 5999;
pub const KIND_JOB_RESULT_MIN: u32 = 6000;
pub const KIND_JOB_RESULT_MAX: u32 = 6999;
pub const KIND_JOB_FEEDBACK: u32 = 7000;

#[inline]
pub const fn is_listing_kind(kind: u32) -> bool {
    matches!(kind, KIND_LISTING | KIND_LISTING_DRAFT)
}

#[inline]
pub const fn is_trade_service_request_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_TRADE_LISTING_VALIDATE_REQ | KIND_WORKER_TRADE_TRANSITION_PROOF_REQ
    )
}

#[inline]
pub const fn is_trade_service_result_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_TRADE_LISTING_VALIDATE_RES | KIND_WORKER_TRADE_TRANSITION_PROOF_RES
    )
}

#[inline]
pub const fn is_trade_service_kind(kind: u32) -> bool {
    is_trade_service_request_kind(kind) || is_trade_service_result_kind(kind)
}

#[inline]
pub const fn is_trade_public_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_TRADE_ORDER_REQUEST
            | KIND_TRADE_ORDER_RESPONSE
            | KIND_TRADE_ORDER_REVISION
            | KIND_TRADE_ORDER_REVISION_RESPONSE
            | KIND_TRADE_QUESTION
            | KIND_TRADE_ANSWER
            | KIND_TRADE_DISCOUNT_REQUEST
            | KIND_TRADE_DISCOUNT_OFFER
            | KIND_TRADE_DISCOUNT_ACCEPT
            | KIND_TRADE_CANCEL
            | KIND_TRADE_FULFILLMENT_UPDATE
            | KIND_TRADE_RECEIPT
            | KIND_TRADE_PAYMENT_RECORDED
            | KIND_TRADE_SETTLEMENT_DECISION
    )
}

#[inline]
pub const fn is_trade_kind(kind: u32) -> bool {
    is_trade_service_kind(kind) || is_trade_public_kind(kind)
}

#[inline]
pub const fn is_active_trade_listing_kind(kind: u32) -> bool {
    matches!(kind, KIND_LISTING | KIND_LISTING_DRAFT)
}

#[inline]
pub const fn is_active_trade_public_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_TRADE_ORDER_REQUEST
            | KIND_TRADE_ORDER_DECISION
            | KIND_TRADE_ORDER_REVISION
            | KIND_TRADE_ORDER_REVISION_RESPONSE
            | KIND_TRADE_CANCEL
            | KIND_TRADE_FULFILLMENT_UPDATE
            | KIND_TRADE_RECEIPT
            | KIND_TRADE_PAYMENT_RECORDED
            | KIND_TRADE_SETTLEMENT_DECISION
    )
}

#[inline]
pub const fn is_active_trade_kind(kind: u32) -> bool {
    is_active_trade_listing_kind(kind) || is_active_trade_public_kind(kind)
}

#[inline]
pub const fn is_trade_validation_receipt_kind(kind: u32) -> bool {
    kind == KIND_TRADE_VALIDATION_RECEIPT
}

#[inline]
pub const fn is_trade_listing_request_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_TRADE_LISTING_VALIDATE_REQ
            | KIND_TRADE_ORDER_REQUEST
            | KIND_TRADE_ORDER_REVISION
            | KIND_TRADE_QUESTION
            | KIND_TRADE_DISCOUNT_REQUEST
            | KIND_TRADE_DISCOUNT_ACCEPT
            | KIND_TRADE_CANCEL
            | KIND_TRADE_FULFILLMENT_UPDATE
            | KIND_TRADE_RECEIPT
    )
}

#[inline]
pub const fn is_trade_listing_result_kind(kind: u32) -> bool {
    matches!(
        kind,
        KIND_TRADE_LISTING_VALIDATE_RES
            | KIND_TRADE_ORDER_RESPONSE
            | KIND_TRADE_ORDER_REVISION_RESPONSE
            | KIND_TRADE_ANSWER
            | KIND_TRADE_DISCOUNT_OFFER
    )
}

#[inline]
pub const fn is_trade_listing_kind(kind: u32) -> bool {
    is_trade_kind(kind)
}

#[inline]
pub const fn trade_service_result_kind_for_request(kind: u32) -> Option<u32> {
    match kind {
        KIND_TRADE_LISTING_VALIDATE_REQ => Some(KIND_TRADE_LISTING_VALIDATE_RES),
        KIND_WORKER_TRADE_TRANSITION_PROOF_REQ => Some(KIND_WORKER_TRADE_TRANSITION_PROOF_RES),
        _ => None,
    }
}

#[inline]
pub const fn trade_service_request_kind_for_result(kind: u32) -> Option<u32> {
    match kind {
        KIND_TRADE_LISTING_VALIDATE_RES => Some(KIND_TRADE_LISTING_VALIDATE_REQ),
        KIND_WORKER_TRADE_TRANSITION_PROOF_RES => Some(KIND_WORKER_TRADE_TRANSITION_PROOF_REQ),
        _ => None,
    }
}

#[inline]
pub const fn trade_listing_result_kind_for_request(kind: u32) -> Option<u32> {
    match kind {
        KIND_TRADE_LISTING_VALIDATE_REQ => Some(KIND_TRADE_LISTING_VALIDATE_RES),
        KIND_TRADE_ORDER_REQUEST => Some(KIND_TRADE_ORDER_RESPONSE),
        KIND_TRADE_ORDER_REVISION => Some(KIND_TRADE_ORDER_REVISION_RESPONSE),
        KIND_TRADE_QUESTION => Some(KIND_TRADE_ANSWER),
        KIND_TRADE_DISCOUNT_REQUEST => Some(KIND_TRADE_DISCOUNT_OFFER),
        _ => None,
    }
}

#[inline]
pub const fn trade_listing_request_kind_for_result(kind: u32) -> Option<u32> {
    match kind {
        KIND_TRADE_LISTING_VALIDATE_RES => Some(KIND_TRADE_LISTING_VALIDATE_REQ),
        KIND_TRADE_ORDER_RESPONSE => Some(KIND_TRADE_ORDER_REQUEST),
        KIND_TRADE_ORDER_REVISION_RESPONSE => Some(KIND_TRADE_ORDER_REVISION),
        KIND_TRADE_ANSWER => Some(KIND_TRADE_QUESTION),
        KIND_TRADE_DISCOUNT_OFFER => Some(KIND_TRADE_DISCOUNT_REQUEST),
        _ => None,
    }
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
        assert_eq!(KIND_FARM_WORKSPACE_MANIFEST, KIND_APP_DATA);
        assert_eq!(KIND_RELAY_AUTH, 22242);
        assert_eq!(KIND_HTTP_AUTH, 27235);
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
    fn classifies_trade_listing_kinds() {
        assert!(is_listing_kind(KIND_LISTING));
        assert!(is_listing_kind(KIND_LISTING_DRAFT));
        assert!(!is_listing_kind(KIND_PROFILE));

        assert!(is_trade_service_request_kind(
            KIND_TRADE_LISTING_VALIDATE_REQ
        ));
        assert!(is_trade_service_request_kind(
            KIND_WORKER_TRADE_TRANSITION_PROOF_REQ
        ));
        assert!(!is_trade_service_request_kind(
            KIND_TRADE_LISTING_VALIDATE_RES
        ));
        assert!(is_trade_service_result_kind(
            KIND_TRADE_LISTING_VALIDATE_RES
        ));
        assert!(is_trade_service_result_kind(
            KIND_WORKER_TRADE_TRANSITION_PROOF_RES
        ));
        assert!(!is_trade_service_result_kind(
            KIND_TRADE_LISTING_VALIDATE_REQ
        ));
        assert!(is_trade_service_kind(KIND_TRADE_LISTING_VALIDATE_REQ));
        assert!(is_trade_service_kind(KIND_TRADE_LISTING_VALIDATE_RES));
        assert!(is_trade_service_kind(
            KIND_WORKER_TRADE_TRANSITION_PROOF_REQ
        ));
        assert!(is_trade_service_kind(
            KIND_WORKER_TRADE_TRANSITION_PROOF_RES
        ));
        assert!(!is_trade_service_kind(KIND_TRADE_ORDER_REQUEST));
        assert!(is_trade_public_kind(KIND_TRADE_ORDER_REQUEST));
        assert!(is_trade_public_kind(KIND_TRADE_ORDER_RESPONSE));
        assert!(is_trade_public_kind(KIND_TRADE_RECEIPT));
        assert!(!is_trade_public_kind(KIND_TRADE_LISTING_VALIDATE_REQ));
        assert!(is_trade_kind(KIND_TRADE_ORDER_REQUEST));
        assert!(is_trade_kind(KIND_TRADE_LISTING_VALIDATE_REQ));
        assert!(!is_trade_kind(KIND_LISTING));
        assert!(is_trade_listing_request_kind(KIND_TRADE_LISTING_ORDER_REQ));
        assert!(is_trade_listing_request_kind(
            KIND_TRADE_LISTING_ORDER_REVISION_REQ
        ));
        assert!(is_trade_listing_request_kind(
            KIND_TRADE_LISTING_QUESTION_REQ
        ));
        assert!(is_trade_listing_request_kind(
            KIND_TRADE_LISTING_DISCOUNT_REQ
        ));
        assert!(is_trade_listing_request_kind(
            KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ
        ));
        assert!(is_trade_listing_request_kind(KIND_TRADE_LISTING_CANCEL_REQ));
        assert!(is_trade_listing_request_kind(
            KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ
        ));
        assert!(is_trade_listing_request_kind(
            KIND_TRADE_LISTING_RECEIPT_REQ
        ));
        assert!(!is_trade_listing_request_kind(KIND_TRADE_LISTING_ORDER_RES));
        assert!(is_trade_listing_result_kind(KIND_TRADE_LISTING_ORDER_RES));
        assert!(is_trade_listing_result_kind(
            KIND_TRADE_LISTING_ORDER_REVISION_RES
        ));
        assert!(is_trade_listing_result_kind(KIND_TRADE_LISTING_ANSWER_RES));
        assert!(is_trade_listing_result_kind(
            KIND_TRADE_LISTING_DISCOUNT_OFFER_RES
        ));
        assert!(!is_trade_listing_result_kind(KIND_TRADE_LISTING_CANCEL_REQ));
        assert!(is_trade_listing_kind(KIND_TRADE_LISTING_RECEIPT_REQ));
        assert!(!is_trade_listing_kind(KIND_LISTING));
        assert!(!is_trade_public_kind(KIND_TRADE_FORBIDDEN_3431));
        assert!(!is_trade_kind(KIND_TRADE_FORBIDDEN_3431));
        assert!(!is_trade_listing_request_kind(KIND_TRADE_FORBIDDEN_3431));
        assert_eq!(
            trade_service_result_kind_for_request(KIND_TRADE_LISTING_VALIDATE_REQ),
            Some(KIND_TRADE_LISTING_VALIDATE_RES)
        );
        assert_eq!(
            trade_service_result_kind_for_request(KIND_WORKER_TRADE_TRANSITION_PROOF_REQ),
            Some(KIND_WORKER_TRADE_TRANSITION_PROOF_RES)
        );
        assert_eq!(
            trade_service_result_kind_for_request(KIND_TRADE_ORDER_REQUEST),
            None
        );
        assert_eq!(
            trade_service_request_kind_for_result(KIND_TRADE_LISTING_VALIDATE_RES),
            Some(KIND_TRADE_LISTING_VALIDATE_REQ)
        );
        assert_eq!(
            trade_service_request_kind_for_result(KIND_WORKER_TRADE_TRANSITION_PROOF_RES),
            Some(KIND_WORKER_TRADE_TRANSITION_PROOF_REQ)
        );
        assert_eq!(
            trade_service_request_kind_for_result(KIND_TRADE_ORDER_RESPONSE),
            None
        );
        assert_eq!(
            trade_listing_result_kind_for_request(KIND_TRADE_LISTING_VALIDATE_REQ),
            Some(KIND_TRADE_LISTING_VALIDATE_RES)
        );
        assert_eq!(
            trade_listing_result_kind_for_request(KIND_TRADE_LISTING_ORDER_REQ),
            Some(KIND_TRADE_LISTING_ORDER_RES)
        );
        assert_eq!(
            trade_listing_result_kind_for_request(KIND_TRADE_LISTING_ORDER_REVISION_REQ),
            Some(KIND_TRADE_LISTING_ORDER_REVISION_RES)
        );
        assert_eq!(
            trade_listing_result_kind_for_request(KIND_TRADE_LISTING_QUESTION_REQ),
            Some(KIND_TRADE_LISTING_ANSWER_RES)
        );
        assert_eq!(
            trade_listing_result_kind_for_request(KIND_TRADE_LISTING_DISCOUNT_REQ),
            Some(KIND_TRADE_LISTING_DISCOUNT_OFFER_RES)
        );
        assert_eq!(
            trade_listing_result_kind_for_request(KIND_TRADE_LISTING_CANCEL_REQ),
            None
        );
        assert_eq!(
            trade_listing_request_kind_for_result(KIND_TRADE_LISTING_VALIDATE_RES),
            Some(KIND_TRADE_LISTING_VALIDATE_REQ)
        );
        assert_eq!(
            trade_listing_request_kind_for_result(KIND_TRADE_LISTING_ORDER_RES),
            Some(KIND_TRADE_LISTING_ORDER_REQ)
        );
        assert_eq!(
            trade_listing_request_kind_for_result(KIND_TRADE_LISTING_ORDER_REVISION_RES),
            Some(KIND_TRADE_LISTING_ORDER_REVISION_REQ)
        );
        assert_eq!(
            trade_listing_request_kind_for_result(KIND_TRADE_LISTING_ANSWER_RES),
            Some(KIND_TRADE_LISTING_QUESTION_REQ)
        );
        assert_eq!(
            trade_listing_request_kind_for_result(KIND_TRADE_LISTING_DISCOUNT_OFFER_RES),
            Some(KIND_TRADE_LISTING_DISCOUNT_REQ)
        );
        assert_eq!(
            trade_listing_request_kind_for_result(KIND_TRADE_LISTING_RECEIPT_REQ),
            None
        );
    }

    #[test]
    fn active_trade_kind_set_contains_listing_order_revision_decision_fulfillment_cancellation_and_receipt()
     {
        assert_eq!(
            ACTIVE_TRADE_LISTING_KINDS,
            [KIND_LISTING, KIND_LISTING_DRAFT]
        );
        assert_eq!(
            ACTIVE_TRADE_PUBLIC_KINDS,
            [
                KIND_TRADE_ORDER_REQUEST,
                KIND_TRADE_ORDER_DECISION,
                KIND_TRADE_ORDER_REVISION,
                KIND_TRADE_ORDER_REVISION_RESPONSE,
                KIND_TRADE_CANCEL,
                KIND_TRADE_FULFILLMENT_UPDATE,
                KIND_TRADE_RECEIPT,
                KIND_TRADE_PAYMENT_RECORDED,
                KIND_TRADE_SETTLEMENT_DECISION,
            ]
        );
        assert_eq!(
            ACTIVE_TRADE_KINDS,
            [
                KIND_LISTING,
                KIND_LISTING_DRAFT,
                KIND_TRADE_ORDER_REQUEST,
                KIND_TRADE_ORDER_DECISION,
                KIND_TRADE_ORDER_REVISION,
                KIND_TRADE_ORDER_REVISION_RESPONSE,
                KIND_TRADE_CANCEL,
                KIND_TRADE_FULFILLMENT_UPDATE,
                KIND_TRADE_RECEIPT,
                KIND_TRADE_PAYMENT_RECORDED,
                KIND_TRADE_SETTLEMENT_DECISION,
            ]
        );

        assert!(is_active_trade_kind(KIND_LISTING));
        assert!(is_active_trade_kind(KIND_LISTING_DRAFT));
        assert!(is_active_trade_public_kind(KIND_TRADE_ORDER_REQUEST));
        assert!(is_active_trade_public_kind(KIND_TRADE_ORDER_DECISION));
        assert!(is_active_trade_public_kind(KIND_TRADE_ORDER_REVISION));
        assert!(is_active_trade_public_kind(
            KIND_TRADE_ORDER_REVISION_RESPONSE
        ));
        assert!(is_active_trade_public_kind(KIND_TRADE_CANCEL));
        assert!(is_active_trade_public_kind(KIND_TRADE_FULFILLMENT_UPDATE));
        assert!(is_active_trade_public_kind(KIND_TRADE_RECEIPT));
        assert!(is_active_trade_public_kind(KIND_TRADE_PAYMENT_RECORDED));
        assert!(is_active_trade_public_kind(KIND_TRADE_SETTLEMENT_DECISION));
        assert!(!is_active_trade_public_kind(
            KIND_TRADE_LISTING_VALIDATE_REQ
        ));
        assert!(!is_active_trade_public_kind(KIND_TRADE_QUESTION));
        assert!(!is_active_trade_public_kind(KIND_TRADE_ANSWER));
        assert!(!is_active_trade_public_kind(KIND_TRADE_DISCOUNT_REQUEST));
        assert!(!is_active_trade_public_kind(KIND_TRADE_DISCOUNT_OFFER));
        assert!(!is_active_trade_public_kind(KIND_TRADE_DISCOUNT_ACCEPT));
        assert!(!is_active_trade_public_kind(KIND_TRADE_FORBIDDEN_3431));
    }

    #[test]
    fn validation_receipt_kind_is_registered_outside_buyer_receipt_lifecycle() {
        assert_eq!(KIND_TRADE_RECEIPT, 3434);
        assert_eq!(KIND_TRADE_VALIDATION_RECEIPT, 3440);
        assert_ne!(KIND_TRADE_VALIDATION_RECEIPT, KIND_TRADE_RECEIPT);
        assert_eq!(
            TRADE_VALIDATION_RECEIPT_KINDS,
            [KIND_TRADE_VALIDATION_RECEIPT]
        );
        assert!(is_trade_validation_receipt_kind(
            KIND_TRADE_VALIDATION_RECEIPT
        ));
        assert!(!is_trade_validation_receipt_kind(KIND_TRADE_RECEIPT));
        assert!(!is_trade_public_kind(KIND_TRADE_VALIDATION_RECEIPT));
        assert!(!is_active_trade_public_kind(KIND_TRADE_VALIDATION_RECEIPT));
        assert!(!is_active_trade_kind(KIND_TRADE_VALIDATION_RECEIPT));
    }
}

use dto_bindgen_backend_ts::{
    TypeScriptDeclaration, TypeScriptImport, TypeScriptModule, TypeScriptType, TypeScriptValue,
};
use radroots_event::{kinds, operational_listing::RADROOTS_OPERATIONAL_LISTING_PRODUCT_TAG_KEYS};

pub fn constants_module() -> TypeScriptModule {
    TypeScriptModule::new("src/generated/constants.ts")
        .with_import(TypeScriptImport::type_only(
            ["RadrootsOperationalListingProductTagKeys"],
            "./types.js",
        ))
        .with_declaration(TypeScriptDeclaration::constant(
            "RADROOTS_OPERATIONAL_LISTING_PRODUCT_TAG_KEYS",
            Some(TypeScriptType::named(
                "RadrootsOperationalListingProductTagKeys",
            )),
            TypeScriptValue::array(
                RADROOTS_OPERATIONAL_LISTING_PRODUCT_TAG_KEYS
                    .iter()
                    .map(|value| TypeScriptValue::string(*value))
                    .collect::<Vec<_>>(),
            ),
        ))
}

pub fn kinds_module() -> TypeScriptModule {
    EVENT_KIND_EXPORTS.iter().fold(
        TypeScriptModule::new("src/generated/kinds.ts"),
        |module, (name, value)| {
            module.with_declaration(TypeScriptDeclaration::constant(
                *name,
                None,
                TypeScriptValue::number(i64::from(*value)),
            ))
        },
    )
}

const EVENT_KIND_EXPORTS: &[(&str, u32)] = &[
    ("KIND_PROFILE", kinds::KIND_PROFILE),
    ("KIND_POST", kinds::KIND_POST),
    ("KIND_FOLLOW", kinds::KIND_FOLLOW),
    ("KIND_REACTION", kinds::KIND_REACTION),
    ("KIND_SEAL", kinds::KIND_SEAL),
    ("KIND_MESSAGE", kinds::KIND_MESSAGE),
    ("KIND_MESSAGE_FILE", kinds::KIND_MESSAGE_FILE),
    ("KIND_GIFT_WRAP", kinds::KIND_GIFT_WRAP),
    ("KIND_COMMENT", kinds::KIND_COMMENT),
    ("KIND_GEOCHAT", kinds::KIND_GEOCHAT),
    ("KIND_WIKI_MERGE_REQUEST", kinds::KIND_WIKI_MERGE_REQUEST),
    ("KIND_WIKI_ARTICLE", kinds::KIND_WIKI_ARTICLE),
    ("KIND_WIKI_REDIRECT", kinds::KIND_WIKI_REDIRECT),
    ("KIND_LIST_MUTE", kinds::KIND_LIST_MUTE),
    ("KIND_LIST_PINNED_NOTES", kinds::KIND_LIST_PINNED_NOTES),
    (
        "KIND_LIST_READ_WRITE_RELAYS",
        kinds::KIND_LIST_READ_WRITE_RELAYS,
    ),
    ("KIND_LIST_BOOKMARKS", kinds::KIND_LIST_BOOKMARKS),
    ("KIND_LIST_COMMUNITIES", kinds::KIND_LIST_COMMUNITIES),
    ("KIND_LIST_PUBLIC_CHATS", kinds::KIND_LIST_PUBLIC_CHATS),
    ("KIND_LIST_BLOCKED_RELAYS", kinds::KIND_LIST_BLOCKED_RELAYS),
    ("KIND_LIST_SEARCH_RELAYS", kinds::KIND_LIST_SEARCH_RELAYS),
    ("KIND_LIST_SIMPLE_GROUPS", kinds::KIND_LIST_SIMPLE_GROUPS),
    ("KIND_LIST_RELAY_FEEDS", kinds::KIND_LIST_RELAY_FEEDS),
    ("KIND_LIST_INTERESTS", kinds::KIND_LIST_INTERESTS),
    ("KIND_LIST_MEDIA_FOLLOWS", kinds::KIND_LIST_MEDIA_FOLLOWS),
    ("KIND_LIST_EMOJIS", kinds::KIND_LIST_EMOJIS),
    ("KIND_LIST_DM_RELAYS", kinds::KIND_LIST_DM_RELAYS),
    (
        "KIND_LIST_GOOD_WIKI_AUTHORS",
        kinds::KIND_LIST_GOOD_WIKI_AUTHORS,
    ),
    (
        "KIND_LIST_GOOD_WIKI_RELAYS",
        kinds::KIND_LIST_GOOD_WIKI_RELAYS,
    ),
    ("KIND_LIST_SET_FOLLOW", kinds::KIND_LIST_SET_FOLLOW),
    ("KIND_LIST_SET_GENERIC", kinds::KIND_LIST_SET_GENERIC),
    ("KIND_LIST_SET_RELAY", kinds::KIND_LIST_SET_RELAY),
    ("KIND_LIST_SET_BOOKMARK", kinds::KIND_LIST_SET_BOOKMARK),
    ("KIND_LIST_SET_CURATION", kinds::KIND_LIST_SET_CURATION),
    ("KIND_LIST_SET_VIDEO", kinds::KIND_LIST_SET_VIDEO),
    ("KIND_LIST_SET_PICTURE", kinds::KIND_LIST_SET_PICTURE),
    ("KIND_LIST_SET_KIND_MUTE", kinds::KIND_LIST_SET_KIND_MUTE),
    ("KIND_LIST_SET_INTEREST", kinds::KIND_LIST_SET_INTEREST),
    ("KIND_LIST_SET_EMOJI", kinds::KIND_LIST_SET_EMOJI),
    (
        "KIND_LIST_SET_RELEASE_ARTIFACT",
        kinds::KIND_LIST_SET_RELEASE_ARTIFACT,
    ),
    (
        "KIND_LIST_SET_APP_CURATION",
        kinds::KIND_LIST_SET_APP_CURATION,
    ),
    ("KIND_LIST_SET_CALENDAR", kinds::KIND_LIST_SET_CALENDAR),
    (
        "KIND_LIST_SET_STARTER_PACK",
        kinds::KIND_LIST_SET_STARTER_PACK,
    ),
    (
        "KIND_LIST_SET_MEDIA_STARTER_PACK",
        kinds::KIND_LIST_SET_MEDIA_STARTER_PACK,
    ),
    ("KIND_FARM", kinds::KIND_FARM),
    ("KIND_PLOT", kinds::KIND_PLOT),
    ("KIND_COOP", kinds::KIND_COOP),
    ("KIND_DOCUMENT", kinds::KIND_DOCUMENT),
    ("KIND_RESOURCE_AREA", kinds::KIND_RESOURCE_AREA),
    (
        "KIND_RESOURCE_HARVEST_CAP",
        kinds::KIND_RESOURCE_HARVEST_CAP,
    ),
    ("KIND_ACCOUNT_CLAIM", kinds::KIND_ACCOUNT_CLAIM),
    ("KIND_APP_DATA", kinds::KIND_APP_DATA),
    ("KIND_CLASSIFIED_LISTING", kinds::KIND_CLASSIFIED_LISTING),
    ("KIND_APPLICATION_HANDLER", kinds::KIND_APPLICATION_HANDLER),
    ("KIND_TRADE_PROPOSAL", kinds::KIND_TRADE_PROPOSAL),
    ("KIND_TRADE_DECISION", kinds::KIND_TRADE_DECISION),
    (
        "KIND_TRADE_REVISION_PROPOSAL",
        kinds::KIND_TRADE_REVISION_PROPOSAL,
    ),
    (
        "KIND_TRADE_REVISION_DECISION",
        kinds::KIND_TRADE_REVISION_DECISION,
    ),
    ("KIND_TRADE_CANCELLATION", kinds::KIND_TRADE_CANCELLATION),
    (
        "KIND_TRADE_SELLER_RESERVATION_ASSERTION",
        kinds::KIND_TRADE_SELLER_RESERVATION_ASSERTION,
    ),
    (
        "KIND_TRADE_VALIDATION_RECEIPT",
        kinds::KIND_TRADE_VALIDATION_RECEIPT,
    ),
    ("KIND_KNOWLEDGE_CLAIM", kinds::KIND_KNOWLEDGE_CLAIM),
    ("KIND_KNOWLEDGE_RELATION", kinds::KIND_KNOWLEDGE_RELATION),
    ("KIND_KNOWLEDGE_REVIEW", kinds::KIND_KNOWLEDGE_REVIEW),
    (
        "KIND_KNOWLEDGE_FIELD_REPORT",
        kinds::KIND_KNOWLEDGE_FIELD_REPORT,
    ),
    (
        "KIND_KNOWLEDGE_CHANGE_PROPOSAL",
        kinds::KIND_KNOWLEDGE_CHANGE_PROPOSAL,
    ),
    (
        "KIND_CONTRIBUTION_ATTESTATION",
        kinds::KIND_CONTRIBUTION_ATTESTATION,
    ),
    ("KIND_KNOWLEDGE_SOURCE", kinds::KIND_KNOWLEDGE_SOURCE),
    ("KIND_EVIDENCE_BOUNTY", kinds::KIND_EVIDENCE_BOUNTY),
    ("KIND_JOB_REQUEST_MIN", kinds::KIND_JOB_REQUEST_MIN),
    ("KIND_JOB_REQUEST_MAX", kinds::KIND_JOB_REQUEST_MAX),
    ("KIND_JOB_RESULT_MIN", kinds::KIND_JOB_RESULT_MIN),
    ("KIND_JOB_RESULT_MAX", kinds::KIND_JOB_RESULT_MAX),
    ("KIND_JOB_FEEDBACK", kinds::KIND_JOB_FEEDBACK),
];

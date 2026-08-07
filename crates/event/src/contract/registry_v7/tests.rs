use super::*;
use crate::envelope::EventEnvelopeParts;
use std::collections::BTreeSet;

static AMBIGUOUS_TEST_CONTRACTS: &[EventContract] = &[
    event_contract!(
        "radroots.test.one.v1",
        KIND_POST,
        "Test One",
        "Test",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventDiscriminator::KindOnly,
        NO_TAGS,
        SOCIAL_REDUCERS,
    ),
    event_contract!(
        "radroots.test.two.v1",
        KIND_POST,
        "Test Two",
        "Test",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventDiscriminator::KindOnly,
        NO_TAGS,
        SOCIAL_REDUCERS,
    ),
];

static REQUIRED_MANY_TEST_TAGS: &[TagContract] = &[tag(
    "test_many",
    TagCardinality::RequiredMany,
    TagSemantic::Topic,
    TagValueType::Text,
    false,
)];

static OPTIONAL_ONE_TEST_TAGS: &[TagContract] = &[tag(
    "test_optional",
    TagCardinality::OptionalOne,
    TagSemantic::Topic,
    TagValueType::Text,
    false,
)];

static DUPLICATE_REQUIRED_TEST_TAGS: &[TagContract] = &[
    tag(
        "test_required",
        TagCardinality::RequiredOne,
        TagSemantic::Topic,
        TagValueType::Text,
        false,
    ),
    tag(
        "test_required",
        TagCardinality::RequiredOne,
        TagSemantic::Category,
        TagValueType::Text,
        false,
    ),
];

static DUPLICATE_OPTIONAL_TEST_TAGS: &[TagContract] = &[
    tag(
        "test_optional",
        TagCardinality::OptionalOne,
        TagSemantic::Topic,
        TagValueType::Text,
        false,
    ),
    tag(
        "test_optional",
        TagCardinality::OptionalOne,
        TagSemantic::Category,
        TagValueType::Text,
        false,
    ),
];

fn synthetic_event_contract(id: &'static str, tags: &'static [TagContract]) -> EventContract {
    EventContract {
        id,
        kind: KIND_POST,
        name: "Test",
        payload_type: "Test",
        class: EventClass::Regular,
        stability: EventStability::Experimental,
        privacy: EventPrivacy::Public,
        required_author_role: AuthorRole::Any,
        content_schema: ContentSchema::PlainText,
        authoring_policy: EventAuthoringPolicy::GenericDraft,
        discriminator: EventDiscriminator::KindOnly,
        tags,
        reducers: SOCIAL_REDUCERS,
    }
}

fn synthetic_kind_contract(kind: u32) -> KindContract {
    KindContract {
        kind,
        canonical_constant: "KIND_TEST",
        name: "Test",
        class: EventClass::Regular,
        standard: NostrStandard::Radroots,
        accepted_event_contracts: &[],
    }
}

#[test]
fn author_role_catalog_is_event_owned_complete_and_schema_stable() {
    let declared = AuthorRole::ALL.into_iter().collect::<BTreeSet<_>>();
    let used = all_event_contracts_registry_v7()
        .iter()
        .map(EventContract::required_author_role)
        .collect::<BTreeSet<_>>();
    let labels = AuthorRole::ALL
        .into_iter()
        .map(AuthorRole::as_str)
        .collect::<BTreeSet<_>>();

    assert_eq!(used, declared, "every event author role must be exercised");
    assert_eq!(labels.len(), AuthorRole::ALL.len());
    assert!(
        labels.iter().all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        }),
        "author-role manifest labels must remain canonical snake case"
    );
}

#[test]
fn authoring_policy_matrix_is_centralized_and_complete() {
    let generic = EventAuthoringPolicy::GenericDraft;
    assert!(generic.permits_generic_draft());
    assert!(generic.permits_typed_authoring());
    assert!(!generic.is_read_only());

    let typed = EventAuthoringPolicy::TypedOnly;
    assert!(!typed.permits_generic_draft());
    assert!(typed.permits_typed_authoring());
    assert!(!typed.is_read_only());

    let read_only = EventAuthoringPolicy::ReadOnly;
    assert!(!read_only.permits_generic_draft());
    assert!(!read_only.permits_typed_authoring());
    assert!(read_only.is_read_only());
}

fn unsigned_event(kind: u32, tags: Vec<Vec<&str>>, content: &str) -> EventEnvelope {
    EventEnvelope::new(EventEnvelopeParts {
        id: "0".repeat(64),
        author: crate::test_valid_hex_64('1'),
        created_at: 1_700_000_000,
        kind,
        tags: tags
            .into_iter()
            .map(|tag| tag.into_iter().map(ToOwned::to_owned).collect())
            .collect(),
        content: content.to_owned(),
        sig: "2".repeat(128),
    })
    .expect("event envelope")
}

fn unsigned_event_owned(kind: u32, tags: Vec<Vec<String>>, content: &str) -> EventEnvelope {
    EventEnvelope::new(EventEnvelopeParts {
        id: "0".repeat(64),
        author: crate::test_valid_hex_64('1'),
        created_at: 1_700_000_000,
        kind,
        tags,
        content: content.to_owned(),
        sig: "2".repeat(128),
    })
    .expect("event envelope")
}

fn hex_64(character: char) -> String {
    crate::test_valid_hex_64(character)
}

fn event_ref_tag(name: &str, event_id: &str, author: &str, kind: u32) -> Vec<String> {
    vec![
        name.to_owned(),
        event_id.to_owned(),
        author.to_owned(),
        kind.to_string(),
        String::new(),
    ]
}

fn owned_tag(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn exposes_one_kind_contract_per_supported_kind() {
    let mut kinds = BTreeSet::new();
    for contract in all_kind_contracts() {
        assert!(
            kinds.insert(contract.kind),
            "duplicate kind {}",
            contract.kind
        );
        assert!(!contract.accepted_event_contracts.is_empty());
    }
}

#[test]
fn exposes_unique_event_contract_ids() {
    let mut ids = BTreeSet::new();
    for contract in all_event_contracts() {
        assert!(
            ids.insert(contract.id),
            "duplicate event contract {}",
            contract.id
        );
        assert!(kind_contract(contract.kind).is_some());
    }
}

#[test]
fn every_kind_references_known_matching_event_contracts() {
    for kind in all_kind_contracts() {
        for id in kind.accepted_event_contracts {
            let event = event_contract(id).expect("accepted event contract");
            assert_eq!(event.kind, kind.kind, "{}", id);
        }
    }
}

#[test]
fn event_contract_classes_match_kind_contracts() {
    for contract in all_event_contracts() {
        let kind = kind_contract(contract.kind).expect("event kind contract");
        assert_eq!(contract.class, kind.class, "{}", contract.id);
    }
}

#[test]
fn every_event_contract_is_listed_by_its_kind_contract() {
    for contract in all_event_contracts() {
        let kind = kind_contract(contract.kind).expect("event kind contract");
        assert!(
            kind.accepted_event_contracts.contains(&contract.id),
            "{}",
            contract.id
        );
    }
}

#[test]
fn calendar_contracts_expose_current_nip52_types_content_and_tags() {
    let date = event_contract("radroots.calendar.date_event.v1").expect("calendar date");
    assert_eq!(
        date.payload_type,
        "RadrootsAuthoredCalendarDateEvent / RadrootsParsedNip52CalendarDateEvent / RadrootsAdmittedCalendarDateEvent"
    );
    assert_eq!(date.content_schema, ContentSchema::PlainText);
    assert_eq!(date.authoring_policy(), EventAuthoringPolicy::TypedOnly);
    assert!(
        date.tags
            .iter()
            .any(|tag| { tag.name == "title" && tag.cardinality == TagCardinality::RequiredOne })
    );
    assert!(
        date.tags
            .iter()
            .any(|tag| { tag.name == "name" && tag.cardinality == TagCardinality::OptionalOne })
    );
    assert!(
        date.tags
            .iter()
            .any(|tag| tag.name == "start" && tag.value_type == TagValueType::CalendarDate)
    );
    assert!(!date.tags.iter().any(|tag| tag.name == "D"));

    let time = event_contract("radroots.calendar.time_event.v1").expect("calendar time");
    assert_eq!(
        time.payload_type,
        "RadrootsAuthoredCalendarTimeEvent / RadrootsParsedNip52CalendarTimeEvent / RadrootsAdmittedCalendarTimeEvent"
    );
    assert_eq!(time.content_schema, ContentSchema::PlainText);
    assert_eq!(time.authoring_policy(), EventAuthoringPolicy::TypedOnly);
    assert!(time.tags.iter().any(|tag| {
        tag.name == "D"
            && tag.cardinality == TagCardinality::RequiredMany
            && tag.value_type == TagValueType::UtcDayIndex
    }));
    assert!(
        time.tags
            .iter()
            .any(|tag| { tag.name == "name" && tag.cardinality == TagCardinality::OptionalOne })
    );

    let calendar = event_contract("radroots.calendar.collection.v1").expect("calendar");
    assert_eq!(calendar.payload_type, "RadrootsAdmittedCalendar");
    assert_eq!(calendar.content_schema, ContentSchema::PlainText);
    assert!(calendar.tags.iter().any(|tag| {
        tag.name == "a"
            && tag.cardinality == TagCardinality::OptionalMany
            && tag.value_type == TagValueType::CalendarEventCoordinate
    }));

    let rsvp = event_contract("radroots.calendar.rsvp.v1").expect("calendar RSVP");
    assert_eq!(rsvp.payload_type, "RadrootsAdmittedCalendarEventRsvp");
    assert_eq!(rsvp.content_schema, ContentSchema::PlainText);
    assert!(rsvp.tags.iter().any(|tag| {
        tag.name == "status"
            && tag.cardinality == TagCardinality::RequiredOne
            && tag.value_type == TagValueType::CalendarRsvpStatus
    }));
}

#[test]
fn calendar_collection_and_rsvp_contracts_enforce_strict_nip52_shapes() {
    let author = "a".repeat(64);
    let event_coordinate = format!("31923:{author}:wash-pack");
    let second_coordinate = format!("31922:{author}:market-day");
    let event_id = "b".repeat(64);
    let blossom_image = format!("https://media.example/{}.webp", "c".repeat(64));
    let collection = vec![
        owned_tag(&["d", "AAAAAAAAAAAAAAAAAAAAAA"]),
        owned_tag(&["title", "Farm calendar"]),
        owned_tag(&["description", "Upcoming farm work"]),
        vec!["image".to_owned(), blossom_image],
        vec![
            "a".to_owned(),
            event_coordinate.clone(),
            "wss://relay.example/events".to_owned(),
        ],
        vec!["a".to_owned(), second_coordinate],
    ];
    assert_eq!(
        validate_event_contract_parts(
            KIND_CALENDAR,
            &collection,
            "Detailed shared schedule.",
            "radroots.calendar.collection.v1",
        ),
        Ok(())
    );

    let mut duplicate_collection = collection.clone();
    duplicate_collection.push(vec!["a".to_owned(), event_coordinate.clone()]);
    assert!(matches!(
        validate_event_contract_parts(
            KIND_CALENDAR,
            &duplicate_collection,
            "",
            "radroots.calendar.collection.v1",
        ),
        Err(ContractValidationError::TagValueMismatch { name: "a", .. })
    ));

    let rsvp = vec![
        owned_tag(&["d", "AAAAAAAAAAAAAAAAAAAAAQ"]),
        vec![
            "a".to_owned(),
            event_coordinate.clone(),
            "wss://relay.example/a".to_owned(),
        ],
        vec!["e".to_owned(), event_id, "wss://relay.example/e".to_owned()],
        owned_tag(&["status", "tentative"]),
        owned_tag(&["fb", "busy"]),
        vec![
            "p".to_owned(),
            author.clone(),
            "wss://relay.example/p".to_owned(),
        ],
    ];
    assert_eq!(
        validate_event_contract_parts(
            KIND_CALENDAR_EVENT_RSVP,
            &rsvp,
            "I expect to attend.",
            "radroots.calendar.rsvp.v1",
        ),
        Ok(())
    );

    let mut mismatched_author = rsvp.clone();
    let participant = mismatched_author
        .iter_mut()
        .find(|tag| tag.first().map(String::as_str) == Some("p"))
        .unwrap();
    participant[1] = "d".repeat(64);
    assert!(matches!(
        validate_event_contract_parts(
            KIND_CALENDAR_EVENT_RSVP,
            &mismatched_author,
            "",
            "radroots.calendar.rsvp.v1",
        ),
        Err(ContractValidationError::TagValueMismatch { name: "p", .. })
    ));

    let declined_with_observed_free_busy = vec![
        owned_tag(&["d", "AAAAAAAAAAAAAAAAAAAAAg"]),
        vec!["a".to_owned(), event_coordinate],
        owned_tag(&["status", "declined"]),
        owned_tag(&["fb", "free"]),
    ];
    assert_eq!(
        validate_event_contract_parts(
            KIND_CALENDAR_EVENT_RSVP,
            &declined_with_observed_free_busy,
            "",
            "radroots.calendar.rsvp.v1",
        ),
        Ok(())
    );
}

#[test]
fn calendar_date_contract_validates_gregorian_exclusive_ranges_without_day_tags() {
    let valid = vec![
        owned_tag(&["d", "market-day"]),
        owned_tag(&["title", "Market day"]),
        owned_tag(&["start", "2026-06-20"]),
        owned_tag(&["end", "2026-06-21"]),
        owned_tag(&["location", "Moss Street Market"]),
        owned_tag(&["location", "Victoria, BC"]),
    ];
    assert_eq!(
        validate_event_contract_parts(
            KIND_CALENDAR_DATE_EVENT,
            &valid,
            "Farm stand pickup window.",
            "radroots.calendar.date_event.v1",
        ),
        Ok(())
    );

    let duplicate_legacy_name = vec![
        owned_tag(&["d", "market-day"]),
        owned_tag(&["title", "Market day"]),
        owned_tag(&["name", "Market day"]),
        owned_tag(&["name", "Deprecated duplicate"]),
        owned_tag(&["start", "2026-06-20"]),
    ];
    assert!(matches!(
        validate_event_contract_parts(
            KIND_CALENDAR_DATE_EVENT,
            &duplicate_legacy_name,
            "description",
            "radroots.calendar.date_event.v1",
        ),
        Err(ContractValidationError::TagCardinalityMismatch { name: "name", .. })
    ));

    for invalid in [
        vec![
            owned_tag(&["d", "market-day"]),
            owned_tag(&["title", "Market day"]),
            owned_tag(&["start", "2026-02-29"]),
        ],
        vec![
            owned_tag(&["d", "market-day"]),
            owned_tag(&["title", "Market day"]),
            owned_tag(&["start", "2026-06-20"]),
            owned_tag(&["end", "2026-06-20"]),
        ],
        vec![
            owned_tag(&["d", "market-day"]),
            owned_tag(&["title", "Market day"]),
            owned_tag(&["start", "2026-06-20"]),
            owned_tag(&["D", "20624"]),
        ],
    ] {
        assert!(matches!(
            validate_event_contract_parts(
                KIND_CALENDAR_DATE_EVENT,
                &invalid,
                "description",
                "radroots.calendar.date_event.v1",
            ),
            Err(ContractValidationError::TagValueMismatch { .. })
        ));
    }
}

#[test]
fn calendar_time_contract_requires_exact_derived_bounded_day_coverage() {
    let valid = vec![
        owned_tag(&["d", "wash-pack"]),
        owned_tag(&["title", "Wash and pack"]),
        owned_tag(&["start", "86399"]),
        owned_tag(&["end", "86401"]),
        owned_tag(&["D", "0"]),
        owned_tag(&["D", "1"]),
    ];
    assert_eq!(
        validate_event_contract_parts(
            KIND_CALENDAR_TIME_EVENT,
            &valid,
            "Pack CSA shares.",
            "radroots.calendar.time_event.v1",
        ),
        Ok(())
    );

    for invalid in [
        vec![
            owned_tag(&["d", "wash-pack"]),
            owned_tag(&["title", "Wash and pack"]),
            owned_tag(&["start", "086399"]),
            owned_tag(&["D", "0"]),
        ],
        vec![
            owned_tag(&["d", "wash-pack"]),
            owned_tag(&["title", "Wash and pack"]),
            owned_tag(&["start", "86399"]),
            owned_tag(&["end", "86401"]),
            owned_tag(&["D", "1"]),
            owned_tag(&["D", "0"]),
        ],
        vec![
            owned_tag(&["d", "wash-pack"]),
            owned_tag(&["title", "Wash and pack"]),
            owned_tag(&["start", "0"]),
            owned_tag(&["end", "31708800"]),
            owned_tag(&["D", "0"]),
        ],
    ] {
        assert!(matches!(
            validate_event_contract_parts(
                KIND_CALENDAR_TIME_EVENT,
                &invalid,
                "description",
                "radroots.calendar.time_event.v1",
            ),
            Err(ContractValidationError::TagValueMismatch { .. })
        ));
    }
}

#[test]
fn trade_mutation_contract_requires_exact_contract_tag() {
    let contract = event_contract("radroots.trade.proposal.v1").expect("trade proposal");
    let tag = contract
        .tags
        .iter()
        .find(|tag| tag.name == "contract")
        .expect("contract tag");

    assert_eq!(tag.semantic, TagSemantic::Contract);
    assert_eq!(tag.value_type, TagValueType::ContractId);
    assert!(!tag.relay_indexed);
}

#[test]
fn covers_public_kind_arrays() {
    for kind in COMMERCIAL_EVENT_KINDS
        .iter()
        .chain(PUBLIC_SOCIAL_KINDS.iter())
        .chain(PRIVATE_FARM_OPS_KINDS.iter())
        .chain(NIP29_GROUP_KINDS.iter())
        .chain(KNOWLEDGE_EVENT_KINDS.iter())
    {
        assert!(kind_contract(*kind).is_some(), "missing kind {kind}");
    }
}

#[test]
fn classified_listing_kind_profiles_are_partitioned_and_explicit() {
    let kind = kind_contract(KIND_CLASSIFIED_LISTING).expect("classified listing kind");
    assert_eq!(kind.canonical_constant, "KIND_CLASSIFIED_LISTING");
    assert_eq!(kind.name, "Classified Listing");
    assert_eq!(kind.class, EventClass::Addressable);
    assert_eq!(kind.standard, NostrStandard::Nip99);
    assert_eq!(
        kind.accepted_event_contracts,
        &[
            "radroots.operational_listing.published.v1",
            "radroots.food.availability.v1",
        ]
    );

    let operational = event_contract("radroots.operational_listing.published.v1")
        .expect("operational listing profile");
    assert_eq!(operational.name, "Operational Listing");
    assert_eq!(operational.payload_type, "RadrootsOperationalListing");
    assert_eq!(operational.content_schema, ContentSchema::Markdown);
    assert_eq!(
        operational.discriminator,
        EventDiscriminator::ClassifiedListingPartition(
            ClassifiedListingPartition::OperationalListing,
        )
    );

    let food =
        event_contract("radroots.food.availability.v1").expect("focused food availability profile");
    assert_eq!(food.name, "Food Availability");
    assert_eq!(
        food.payload_type,
        "RadrootsFoodAvailabilityDetails / RadrootsInboundFoodAvailabilityProjection"
    );
    assert_eq!(food.class, EventClass::Addressable);
    assert_eq!(food.privacy, EventPrivacy::Public);
    assert_eq!(food.required_author_role(), AuthorRole::Seller);
    assert_eq!(food.content_schema, ContentSchema::Markdown);
    assert_eq!(food.authoring_policy(), EventAuthoringPolicy::TypedOnly);
    assert_eq!(food.discriminator, EventDiscriminator::AdmissionOnly);
    assert_eq!(food.reducers, &[Reducer::MarketProjection]);
    assert_eq!(event_contract_family(food), Some(ContractFamily::Market));
    assert_eq!(
        food.tags
            .iter()
            .map(|tag| (tag.name, tag.cardinality))
            .collect::<Vec<_>>(),
        vec![
            ("d", TagCardinality::RequiredOne),
            ("title", TagCardinality::RequiredOne),
            ("summary", TagCardinality::RequiredOne),
            ("published_at", TagCardinality::RequiredOne),
            ("location", TagCardinality::RequiredOne),
            ("price", TagCardinality::RequiredOne),
            ("radroots:price_unit", TagCardinality::RequiredOne),
            ("radroots:quantity", TagCardinality::OptionalOne),
            ("status", TagCardinality::RequiredOne),
            ("image", TagCardinality::OptionalMany),
        ]
    );
    assert!(event_contract("radroots.listing.published.v1").is_none());

    let seller = "a".repeat(64);
    let tags = vec![
        owned_tag(&["d", "carrots"]),
        owned_tag(&["p", seller.as_str()]),
        owned_tag(&["a", format!("30340:{seller}:victoria-farm").as_str()]),
        owned_tag(&["key", "carrots"]),
        owned_tag(&["title", "Carrots"]),
        owned_tag(&["category", "produce"]),
        owned_tag(&["radroots:primary_bin", "field-bin"]),
        owned_tag(&["radroots:bin", "field-bin", "1000", "g"]),
        owned_tag(&["radroots:price", "field-bin", "3", "CAD", "1", "lb"]),
    ];
    assert_eq!(
        validate_event_contract_parts(
            KIND_CLASSIFIED_LISTING,
            &tags,
            "# Carrots",
            "radroots.operational_listing.published.v1",
        ),
        Ok(())
    );
    assert_eq!(
        identify_event_contract(KIND_CLASSIFIED_LISTING, &tags, "# Carrots")
            .expect("operational discriminator")
            .id,
        operational.id
    );

    let malformed_operational = vec![owned_tag(&["radroots:bin"])];
    assert_eq!(
        identify_event_contract(KIND_CLASSIFIED_LISTING, &malformed_operational, "Carrots",)
            .expect("raw operational marker partitions before shape validation")
            .id,
        operational.id
    );
    assert_eq!(
        validate_event_contract_parts(
            KIND_CLASSIFIED_LISTING,
            &malformed_operational,
            "Carrots",
            operational.id,
        ),
        Err(ContractValidationError::MissingTag {
            contract_id: "radroots.operational_listing.published.v1",
            name: "d",
        })
    );

    let focused = vec![owned_tag(&["radroots:price_unit", "lb"])];
    assert_eq!(
        identify_event_contract(KIND_CLASSIFIED_LISTING, &focused, "Carrots"),
        Err(ContractMatchError::UnsupportedShape(
            KIND_CLASSIFIED_LISTING
        ))
    );
    assert_eq!(
        validate_event_contract_parts(KIND_CLASSIFIED_LISTING, &focused, "Carrots", food.id,),
        Err(ContractValidationError::AdmissionRequired {
            contract_id: "radroots.food.availability.v1",
        })
    );
    assert_eq!(
        validate_event_contract_parts(KIND_CLASSIFIED_LISTING, &focused, "Carrots", operational.id,),
        Err(ContractValidationError::ContractMatch {
            error: ContractMatchError::UnsupportedShape(KIND_CLASSIFIED_LISTING),
        })
    );

    let generic = vec![owned_tag(&["d", "carrots"])];
    assert_eq!(
        identify_event_contract(KIND_CLASSIFIED_LISTING, &generic, "Carrots"),
        Err(ContractMatchError::UnsupportedShape(
            KIND_CLASSIFIED_LISTING
        ))
    );

    let ambiguous = vec![
        owned_tag(&["radroots:price_unit"]),
        owned_tag(&["radroots:primary_bin"]),
    ];
    assert_eq!(
        identify_event_contract(KIND_CLASSIFIED_LISTING, &ambiguous, "Carrots"),
        Err(ContractMatchError::UnsupportedShape(
            KIND_CLASSIFIED_LISTING
        ))
    );
}

#[test]
fn event_contract_lookup_supports_many_contracts_per_kind() {
    let contracts = event_contracts_for_kind(KIND_LIST_SET_GENERIC).collect::<Vec<_>>();
    assert_eq!(contracts.len(), 6);
    assert!(
        contracts
            .iter()
            .any(|contract| contract.id == "radroots.list_set.farm.members.v1")
    );
    assert_eq!(
        event_contract("radroots.list_set.member_of.farms.v1").map(|contract| contract.kind),
        Some(KIND_LIST_SET_GENERIC)
    );
    assert!(event_contracts_for_kind(999_999).next().is_none());
}

#[test]
fn event_contract_lookup_supports_knowledge_contract_kinds() {
    let contracts = event_contracts_for_kind(KIND_WIKI_ARTICLE).collect::<Vec<_>>();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].id, "radroots.wiki.article.v1");
    assert_eq!(
        identify_event_contract(KIND_WIKI_ARTICLE, &[], "# Soil")
            .expect("wiki article contract")
            .id,
        "radroots.wiki.article.v1"
    );
}

#[test]
fn exposes_contract_family_metadata() {
    assert!(
        contract_families()
            .iter()
            .any(|family| family.family == ContractFamily::Knowledge && family.id == "knowledge")
    );
    assert_eq!(
        event_contract_family(event_contract("radroots.wiki.article.v1").expect("wiki")),
        Some(ContractFamily::Knowledge)
    );
    assert_eq!(
        kind_contract_family(kind_contract(KIND_KNOWLEDGE_CLAIM).expect("claim kind")),
        Some(ContractFamily::Knowledge)
    );
    assert_eq!(
        kind_contract_family(kind_contract(KIND_LIST_SET_GENERIC).expect("list kind")),
        Some(ContractFamily::List)
    );
}

#[test]
fn contract_family_helpers_cover_prefixes_and_kind_branches() {
    for (id, family) in [
        ("radroots.account.test.v1", Some(ContractFamily::Account)),
        (
            "radroots.application.test.v1",
            Some(ContractFamily::Application),
        ),
        ("radroots.calendar.test.v1", Some(ContractFamily::Calendar)),
        ("radroots.farm.test.v1", Some(ContractFamily::Farm)),
        ("radroots.group.test.v1", Some(ContractFamily::Group)),
        ("radroots.http.test.v1", Some(ContractFamily::Http)),
        ("radroots.job.test.v1", Some(ContractFamily::Job)),
        (
            "radroots.knowledge.test.v1",
            Some(ContractFamily::Knowledge),
        ),
        ("radroots.wiki.test.v1", Some(ContractFamily::Knowledge)),
        ("radroots.list.test.v1", Some(ContractFamily::List)),
        ("radroots.list_set.test.v1", Some(ContractFamily::List)),
        (
            "radroots.operational_listing.test.v1",
            Some(ContractFamily::Market),
        ),
        ("radroots.food.test.v1", Some(ContractFamily::Market)),
        ("radroots.message.test.v1", Some(ContractFamily::Message)),
        ("radroots.profile.test.v1", Some(ContractFamily::Profile)),
        ("radroots.relay.test.v1", Some(ContractFamily::Relay)),
        ("radroots.social.test.v1", Some(ContractFamily::Social)),
        ("radroots.trade.test.v1", Some(ContractFamily::Trade)),
        ("radroots.order.test.v1", None),
        ("radroots.test.unknown.v1", None),
    ] {
        assert_eq!(contract_family_for_id(id), family, "{id}");
    }

    for (kind, family) in [
        (KIND_PROFILE, ContractFamily::Profile),
        (KIND_MESSAGE, ContractFamily::Message),
        (KIND_POST, ContractFamily::Social),
        (KIND_RELAY_AUTH, ContractFamily::Relay),
        (KIND_GROUP_ROLES, ContractFamily::Group),
        (KIND_LIST_SET_GENERIC, ContractFamily::List),
        (KIND_CALENDAR_EVENT_RSVP, ContractFamily::Calendar),
        (KIND_FARM_CRDT_CHANGE, ContractFamily::Farm),
        (KIND_CLASSIFIED_LISTING, ContractFamily::Market),
        (KIND_TRADE_CANCELLATION, ContractFamily::Trade),
        (KIND_KNOWLEDGE_CLAIM, ContractFamily::Knowledge),
        (KIND_JOB_FEEDBACK, ContractFamily::Job),
        (KIND_JOB_REQUEST_MIN, ContractFamily::Job),
        (KIND_JOB_RESULT_MIN, ContractFamily::Job),
    ] {
        assert_eq!(
            kind_contract_family(&synthetic_kind_contract(kind)),
            Some(family),
            "{kind}"
        );
    }

    assert_eq!(
        kind_contract_family(&synthetic_kind_contract(999_999)),
        None
    );
}

#[test]
fn scalar_contract_validators_cover_canonical_boundaries() {
    assert_eq!(canonical_u64("0"), Some(0));
    assert_eq!(canonical_u64(u64::MAX.to_string().as_str()), Some(u64::MAX));
    for invalid in ["", "00", "01", "+1", "18446744073709551616"] {
        assert_eq!(canonical_u64(invalid), None, "{invalid}");
    }

    for valid in ["0", "u4pruydqqvj", "U4PRUYDQQVJ"] {
        assert!(geohash_is_valid(valid), "{valid}");
    }
    for invalid in ["", "u4pruydqqvjz0x", "a", "u4pruydqqv-i"] {
        assert!(!geohash_is_valid(invalid), "{invalid}");
    }
}

#[test]
fn exposes_knowledge_contracts() {
    let wiki_article = event_contract("radroots.wiki.article.v1").expect("wiki article");
    assert_eq!(wiki_article.kind, KIND_WIKI_ARTICLE);
    assert_eq!(wiki_article.stability, EventStability::Experimental);
    assert_eq!(
        kind_contract(KIND_WIKI_ARTICLE)
            .expect("wiki kind")
            .standard,
        NostrStandard::Nip54
    );
    assert_eq!(wiki_article.content_schema, ContentSchema::Djot);

    let wiki_merge_request =
        event_contract("radroots.wiki.merge_request.v1").expect("wiki merge request");
    assert_eq!(wiki_merge_request.stability, EventStability::Experimental);
    assert_eq!(wiki_merge_request.content_schema, ContentSchema::PlainText);

    let wiki_redirect = event_contract("radroots.wiki.redirect.v1").expect("wiki redirect");
    assert_eq!(wiki_redirect.kind, KIND_WIKI_REDIRECT);
    assert_eq!(wiki_redirect.stability, EventStability::Experimental);
    assert_eq!(wiki_redirect.content_schema, ContentSchema::Empty);

    for id in [
        "radroots.knowledge.source.v1",
        "radroots.knowledge.evidence_bounty.v1",
        "radroots.knowledge.claim.v1",
        "radroots.knowledge.relation.v1",
        "radroots.knowledge.review.v1",
        "radroots.knowledge.field_report.v1",
        "radroots.knowledge.change_proposal.v1",
        "radroots.knowledge.contribution_attestation.v1",
    ] {
        let contract = event_contract(id).expect(id);
        assert_eq!(contract.stability, EventStability::Experimental);
        assert_eq!(
            event_contract_family(contract),
            Some(ContractFamily::Knowledge)
        );
        let contract_tag = contract
            .tags
            .iter()
            .find(|tag| tag.name == "contract")
            .expect("contract tag");
        assert_eq!(contract_tag.semantic, TagSemantic::Contract);
        assert_eq!(contract_tag.value_type, TagValueType::ContractId);
    }
}

#[test]
fn custom_knowledge_schema_lookup_covers_registered_ids() {
    for id in [
        "radroots.knowledge.source.v1",
        "radroots.knowledge.evidence_bounty.v1",
        "radroots.knowledge.claim.v1",
        "radroots.knowledge.relation.v1",
        "radroots.knowledge.review.v1",
        "radroots.knowledge.field_report.v1",
        "radroots.knowledge.change_proposal.v1",
        "radroots.knowledge.contribution_attestation.v1",
    ] {
        assert_eq!(custom_knowledge_schema(id), Some(id), "{id}");
    }
    assert_eq!(custom_knowledge_schema("radroots.wiki.article.v1"), None);
}

#[test]
fn post_subtype_contracts_require_verified_admission() {
    let tags = vec![vec!["t".to_owned(), "radroots-ask".to_owned()]];
    let generic = identify_event_contract(KIND_POST, &tags, "Question")
        .expect("unsigned kind-1 identification remains generic");
    assert_eq!(generic.id, "radroots.social.post.v1");
    assert_eq!(generic.authoring_policy(), EventAuthoringPolicy::ReadOnly);

    for id in [
        "radroots.social.update.v1",
        "radroots.social.photo_update.v1",
        "radroots.social.ask.v1",
        "radroots.social.reply.v1",
    ] {
        let contract = event_contract(id).expect(id);
        assert_eq!(
            event_contract_family(contract),
            Some(ContractFamily::Social)
        );
        assert_eq!(contract.authoring_policy(), EventAuthoringPolicy::TypedOnly);
        assert_eq!(
            validate_event_contract_parts(KIND_POST, &tags, "Question", id),
            Err(ContractValidationError::AdmissionRequired { contract_id: id })
        );
    }

    assert_eq!(
        event_contract("radroots.profile.metadata.v1")
            .expect("strict authored profile contract")
            .authoring_policy(),
        EventAuthoringPolicy::TypedOnly
    );
    assert_eq!(
        event_contract("radroots.social.geochat.v1")
            .expect("generic-draft control contract")
            .authoring_policy(),
        EventAuthoringPolicy::GenericDraft
    );
}

#[test]
fn nip22_comment_contract_is_typed_and_admission_only() {
    assert_eq!(RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION, 7);

    let kind = kind_contract(KIND_COMMENT).expect("Comment kind");
    assert_eq!(kind.canonical_constant, "KIND_COMMENT");
    assert_eq!(kind.kind, 1111);
    assert_eq!(kind.class, EventClass::Regular);
    assert_eq!(kind.standard, NostrStandard::Nip22);
    assert_eq!(
        kind.accepted_event_contracts,
        &["radroots.social.comment.v1"]
    );

    let contract = event_contract("radroots.social.comment.v1").expect("NIP-22 Comment contract");
    assert_eq!(
        contract.payload_type,
        "RadrootsAuthoredNip22Comment / RadrootsInboundNip22CommentProjection"
    );
    assert_eq!(contract.kind, KIND_COMMENT);
    assert_eq!(contract.class, EventClass::Regular);
    assert_eq!(contract.privacy, EventPrivacy::Public);
    assert_eq!(contract.required_author_role(), AuthorRole::Any);
    assert_eq!(contract.content_schema, ContentSchema::PlainText);
    assert_eq!(contract.authoring_policy(), EventAuthoringPolicy::TypedOnly);
    assert_eq!(contract.discriminator, EventDiscriminator::AdmissionOnly);
    assert_eq!(contract.reducers, &[Reducer::SocialProjection]);
    assert_eq!(
        event_contract_family(contract),
        Some(ContractFamily::Social)
    );
    assert_eq!(
        contract.tags.iter().map(|tag| tag.name).collect::<Vec<_>>(),
        vec!["E", "A", "K", "P", "a", "e", "k", "p"]
    );
    assert!(
        contract.tags.iter().all(|tag| tag.relay_indexed),
        "every single-letter NIP-22 tag must remain relay-indexed"
    );
    assert_eq!(
        validate_event_contract_parts(KIND_COMMENT, &[], "Comment", contract.id),
        Err(ContractValidationError::AdmissionRequired {
            contract_id: "radroots.social.comment.v1",
        })
    );
}

#[test]
fn nip09_deletion_request_contract_is_typed_and_admission_only() {
    use crate::post::deletion::{
        RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES, RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES,
        RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES, RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
        RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT,
        RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES, RADROOTS_NIP09_DELETION_TARGET_KIND_MAX,
    };

    assert_eq!(RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION, 7);

    let kind = kind_contract(KIND_DELETION_REQUEST).expect("deletion request kind");
    assert_eq!(kind.canonical_constant, "KIND_DELETION_REQUEST");
    assert_eq!(kind.kind, 5);
    assert_eq!(kind.class, EventClass::Regular);
    assert_eq!(kind.standard, NostrStandard::Nip09);
    assert_eq!(
        kind.accepted_event_contracts,
        &["radroots.social.deletion_request.v1"]
    );

    let contract = event_contract("radroots.social.deletion_request.v1")
        .expect("NIP-09 deletion request contract");
    assert_eq!(
        contract.payload_type,
        "RadrootsAuthoredNip09DeletionRequest / RadrootsInboundNip09DeletionProjection"
    );
    assert_eq!(contract.kind, KIND_DELETION_REQUEST);
    assert_eq!(contract.class, EventClass::Regular);
    assert_eq!(contract.privacy, EventPrivacy::Public);
    assert_eq!(contract.required_author_role(), AuthorRole::Any);
    assert_eq!(contract.content_schema, ContentSchema::PlainText);
    assert_eq!(contract.authoring_policy(), EventAuthoringPolicy::TypedOnly);
    assert_eq!(contract.discriminator, EventDiscriminator::AdmissionOnly);
    assert_eq!(contract.reducers, &[Reducer::SocialProjection]);
    assert_eq!(
        event_contract_family(contract),
        Some(ContractFamily::Social)
    );
    assert_eq!(
        contract
            .tags
            .iter()
            .map(|tag| (
                tag.name,
                tag.cardinality,
                tag.semantic,
                tag.value_type,
                tag.relay_indexed,
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "e",
                TagCardinality::OptionalMany,
                TagSemantic::EventPointer,
                TagValueType::EventId,
                true,
            ),
            (
                "a",
                TagCardinality::OptionalMany,
                TagSemantic::Nip01Coordinate,
                TagValueType::Nip01Coordinate,
                true,
            ),
            (
                "k",
                TagCardinality::OptionalMany,
                TagSemantic::Kind,
                TagValueType::Kind,
                true,
            ),
        ]
    );
    assert_eq!(
        (
            RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES,
            RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
            RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT,
            RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES,
            RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES,
            RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES,
            RADROOTS_NIP09_DELETION_TARGET_KIND_MAX,
        ),
        (131_072, 1_024, 4_096, 4_096, 131_072, 262_144, 65_535)
    );
    assert_eq!(
        validate_event_contract_parts(KIND_DELETION_REQUEST, &[], "superseded", contract.id,),
        Err(ContractValidationError::AdmissionRequired {
            contract_id: "radroots.social.deletion_request.v1",
        })
    );
    let selected = unsigned_event(
        KIND_DELETION_REQUEST,
        vec![vec![
            "e",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ]],
        "superseded",
    );
    assert_eq!(
        validate_event_contract_for_admission(&selected, contract.id),
        Ok(contract)
    );
    assert_eq!(
        validate_event_contract_for_admission(
            &unsigned_event(KIND_DELETION_REQUEST, vec![], "superseded"),
            contract.id,
        ),
        Err(ContractValidationError::ContractMatch {
            error: ContractMatchError::UnsupportedShape(KIND_DELETION_REQUEST),
        })
    );
}

#[test]
fn identifies_exact_list_set_shape() {
    let tags = vec![vec!["d".to_owned(), "member_of.farms".to_owned()]];
    let contract = identify_event_contract(KIND_LIST_SET_GENERIC, &tags, "{}")
        .expect("member_of farms contract");
    assert_eq!(contract.id, "radroots.list_set.member_of.farms.v1");
}

#[test]
fn identifies_composite_list_set_shape() {
    let tags = vec![vec![
        "d".to_owned(),
        "farm:farm_01:members.workers".to_owned(),
    ]];
    let contract =
        identify_event_contract(KIND_LIST_SET_GENERIC, &tags, "{}").expect("farm workers contract");
    assert_eq!(contract.id, "radroots.list_set.farm.members.workers.v1");
}

#[test]
fn rejects_unknown_or_unsupported_shapes() {
    assert_eq!(
        identify_event_contract(999_999, &[], "{}"),
        Err(ContractMatchError::UnsupportedKind(999_999))
    );
    assert_eq!(
        validate_event_contract(&unsigned_event(999_999, Vec::new(), "{}")),
        Err(ContractValidationError::ContractMatch {
            error: ContractMatchError::UnsupportedKind(999_999),
        })
    );

    let tags = vec![vec!["d".to_owned(), "unknown".to_owned()]];
    assert_eq!(
        identify_event_contract(KIND_LIST_SET_GENERIC, &tags, "{}"),
        Err(ContractMatchError::UnsupportedShape(KIND_LIST_SET_GENERIC))
    );
}

#[test]
fn rejects_ambiguous_shapes() {
    assert_eq!(
        identify_from_contracts(AMBIGUOUS_TEST_CONTRACTS.iter(), KIND_POST, &[], ""),
        Err(ContractMatchError::AmbiguousShape(KIND_POST))
    );
}

#[test]
fn supports_content_field_discriminators() {
    assert!(discriminator_matches(
        &EventDiscriminator::EnvelopeType("proposal"),
        &[],
        r#"{"domain":"radroots.trade","type":"proposal"}"#
    ));
    assert!(discriminator_matches(
        &EventDiscriminator::ContentJsonFieldEquals {
            field: "domain",
            value: "radroots.trade"
        },
        &[],
        r#"{"domain": "radroots.trade", "type": "proposal"}"#
    ));

    let base = *event_contract("radroots.trade.proposal.v1").expect("trade proposal");
    for discriminator in [
        EventDiscriminator::ContentJsonFieldEquals {
            field: "type",
            value: "proposal",
        },
        EventDiscriminator::EnvelopeType("proposal"),
    ] {
        let contract = EventContract {
            discriminator,
            ..base
        };
        assert!(validate_discriminator_parts(r#"{"type":"proposal"}"#, &contract, false).is_ok());
        assert!(matches!(
            validate_discriminator_parts(r#"{"type":"decision"}"#, &contract, false),
            Err(ContractValidationError::ContentFieldMismatch { .. })
        ));
        assert!(matches!(
            validate_discriminator_parts("{}", &contract, false),
            Err(ContractValidationError::MissingContentField { .. })
        ));
    }
    let contract = EventContract {
        discriminator: EventDiscriminator::KindOnly,
        ..base
    };
    assert!(validate_discriminator_parts("not-json", &contract, false).is_ok());
}

#[test]
fn supports_tag_equals_discriminators() {
    let tags = vec![vec!["status".to_owned(), "accepted".to_owned()]];

    assert!(discriminator_matches(
        &EventDiscriminator::TagEquals {
            name: "status",
            value: "accepted",
        },
        &tags,
        "{}"
    ));
    assert!(!discriminator_matches(
        &EventDiscriminator::TagEquals {
            name: "status",
            value: "declined",
        },
        &tags,
        "{}"
    ));
}

#[test]
fn validates_custom_knowledge_contract_shape() {
    let event = unsigned_event(
        KIND_KNOWLEDGE_CLAIM,
        vec![vec!["contract", "radroots.knowledge.claim.v1"]],
        r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1,"text":"soil improves with cover crops"}"#,
    );

    assert_eq!(
        validate_event_contract_shape(&event, "radroots.knowledge.claim.v1"),
        Ok(())
    );
    assert_eq!(
        validate_event_contract(&event).expect("validated").id,
        "radroots.knowledge.claim.v1"
    );
}

#[test]
fn rejects_custom_knowledge_contract_tag_mismatch() {
    let event = unsigned_event(
        KIND_KNOWLEDGE_CLAIM,
        vec![vec!["contract", "radroots.knowledge.relation.v1"]],
        r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#,
    );

    assert_eq!(
        validate_event_contract_shape(&event, "radroots.knowledge.claim.v1"),
        Err(ContractValidationError::TagValueMismatch {
            contract_id: "radroots.knowledge.claim.v1",
            name: "contract",
            expected: "radroots.knowledge.claim.v1".to_owned(),
            actual: Some("radroots.knowledge.relation.v1".to_owned()),
        })
    );
}

#[test]
fn rejects_custom_knowledge_schema_mismatch() {
    let event = unsigned_event(
        KIND_KNOWLEDGE_CLAIM,
        vec![vec!["contract", "radroots.knowledge.claim.v1"]],
        r#"{"schema":"radroots.knowledge.relation.v1","schema_version":1}"#,
    );

    assert_eq!(
        validate_event_contract_shape(&event, "radroots.knowledge.claim.v1"),
        Err(ContractValidationError::ContentFieldMismatch {
            contract_id: "radroots.knowledge.claim.v1",
            field: "schema",
            expected: "radroots.knowledge.claim.v1".to_owned(),
        })
    );
}

#[test]
fn rejects_custom_knowledge_missing_schema_version() {
    let event = unsigned_event(
        KIND_KNOWLEDGE_CLAIM,
        vec![vec!["contract", "radroots.knowledge.claim.v1"]],
        r#"{"schema":"radroots.knowledge.claim.v1"}"#,
    );

    assert_eq!(
        validate_event_contract_shape(&event, "radroots.knowledge.claim.v1"),
        Err(ContractValidationError::MissingContentField {
            contract_id: "radroots.knowledge.claim.v1",
            field: "schema_version",
        })
    );
}

#[test]
fn rejects_authoritative_knowledge_status_fields() {
    let event = unsigned_event_owned(
        KIND_KNOWLEDGE_REVIEW,
        vec![
            vec![
                "contract".to_owned(),
                "radroots.knowledge.review.v1".to_owned(),
            ],
            vec![
                "review_target".to_owned(),
                "0".repeat(64),
                crate::test_valid_hex_64('1'),
                "30818".to_owned(),
                "soil".to_owned(),
            ],
        ],
        r#"{"schema":"radroots.knowledge.review.v1","schema_version":1,"canon_status":"approved"}"#,
    );

    assert_eq!(
        validate_event_contract_shape(&event, "radroots.knowledge.review.v1"),
        Err(ContractValidationError::ForbiddenContentField {
            contract_id: "radroots.knowledge.review.v1",
            field: "canon_status",
        })
    );
}

#[test]
fn validate_event_contract_shape_reports_registry_kind_and_content_errors() {
    let event = unsigned_event(KIND_POST, Vec::new(), "hello");
    assert_eq!(
        validate_event_contract_shape(&event, "missing.contract.v1"),
        Err(ContractValidationError::UnknownContract {
            contract_id: "missing.contract.v1".to_owned(),
        })
    );
    assert_eq!(
        validate_event_contract_shape(&event, "radroots.profile.metadata.v1"),
        Err(ContractValidationError::KindMismatch {
            expected: KIND_PROFILE,
            actual: KIND_POST,
        })
    );

    let invalid_json = unsigned_event(
        KIND_KNOWLEDGE_CLAIM,
        vec![vec!["contract", "radroots.knowledge.claim.v1"]],
        "not-json",
    );
    assert_eq!(
        validate_event_contract_shape(&invalid_json, "radroots.knowledge.claim.v1"),
        Err(ContractValidationError::InvalidJsonContent {
            contract_id: "radroots.knowledge.claim.v1",
        })
    );

    assert_eq!(
        validate_event_contract_shape(
            &unsigned_event(KIND_POST, Vec::new(), "plain text"),
            "radroots.social.post.v1",
        ),
        Ok(())
    );
}

#[test]
fn validate_contract_tags_reports_cardinality_errors() {
    let missing_required_one = unsigned_event(
        KIND_KNOWLEDGE_CLAIM,
        Vec::new(),
        r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#,
    );
    assert_eq!(
        validate_event_contract_shape(&missing_required_one, "radroots.knowledge.claim.v1"),
        Err(ContractValidationError::MissingTag {
            contract_id: "radroots.knowledge.claim.v1",
            name: "contract",
        })
    );

    let duplicate_required_one = unsigned_event(
        KIND_KNOWLEDGE_CLAIM,
        vec![
            vec!["contract", "radroots.knowledge.claim.v1"],
            vec!["contract", "radroots.knowledge.claim.v1"],
        ],
        r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#,
    );
    assert_eq!(
        validate_event_contract_shape(&duplicate_required_one, "radroots.knowledge.claim.v1"),
        Err(ContractValidationError::TagCardinalityMismatch {
            contract_id: "radroots.knowledge.claim.v1",
            name: "contract",
        })
    );

    let required_many =
        synthetic_event_contract("radroots.test.required_many.v1", REQUIRED_MANY_TEST_TAGS);
    assert_eq!(
        validate_contract_tags_parts(&[], &required_many),
        Err(ContractValidationError::MissingTag {
            contract_id: "radroots.test.required_many.v1",
            name: "test_many",
        })
    );
    assert_eq!(
        validate_contract_tags_parts(
            &[vec!["test_many".to_owned(), "one".to_owned()]],
            &required_many
        ),
        Ok(())
    );

    let optional_one =
        synthetic_event_contract("radroots.test.optional_one.v1", OPTIONAL_ONE_TEST_TAGS);
    assert_eq!(validate_contract_tags_parts(&[], &optional_one), Ok(()));
    assert_eq!(
        validate_contract_tags_parts(
            &[
                vec!["test_optional".to_owned(), "one".to_owned()],
                vec!["test_optional".to_owned(), "two".to_owned()],
            ],
            &optional_one,
        ),
        Err(ContractValidationError::TagCardinalityMismatch {
            contract_id: "radroots.test.optional_one.v1",
            name: "test_optional",
        })
    );

    let duplicate_required = synthetic_event_contract(
        "radroots.test.duplicate_required.v1",
        DUPLICATE_REQUIRED_TEST_TAGS,
    );
    assert_eq!(
        validate_contract_tags_parts(
            &[
                vec!["test_required".to_owned(), "one".to_owned()],
                vec!["test_required".to_owned(), "two".to_owned()],
            ],
            &duplicate_required,
        ),
        Ok(())
    );

    let duplicate_optional = synthetic_event_contract(
        "radroots.test.duplicate_optional.v1",
        DUPLICATE_OPTIONAL_TEST_TAGS,
    );
    assert_eq!(
        validate_contract_tags_parts(
            &[
                vec!["test_optional".to_owned(), "one".to_owned()],
                vec!["test_optional".to_owned(), "two".to_owned()],
            ],
            &duplicate_optional,
        ),
        Ok(())
    );
}

#[test]
fn validate_contract_tags_enforces_declared_value_types() {
    let claim_content = r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#;
    let invalid_source = unsigned_event_owned(
        KIND_KNOWLEDGE_CLAIM,
        vec![
            vec![
                "contract".to_owned(),
                "radroots.knowledge.claim.v1".to_owned(),
            ],
            vec!["source".to_owned(), "not-an-event-id".to_owned()],
        ],
        claim_content,
    );
    assert_eq!(
        validate_event_contract_shape(&invalid_source, "radroots.knowledge.claim.v1"),
        Err(ContractValidationError::TagValueMismatch {
            contract_id: "radroots.knowledge.claim.v1",
            name: "source",
            expected: "event_pointer".to_owned(),
            actual: Some("not-an-event-id".to_owned()),
        })
    );

    let invalid_citation = unsigned_event(
        KIND_KNOWLEDGE_CLAIM,
        vec![
            vec!["contract", "radroots.knowledge.claim.v1"],
            vec!["citation", "not-hex"],
        ],
        claim_content,
    );
    assert_eq!(
        validate_event_contract_shape(&invalid_citation, "radroots.knowledge.claim.v1"),
        Err(ContractValidationError::TagValueMismatch {
            contract_id: "radroots.knowledge.claim.v1",
            name: "citation",
            expected: "sha256".to_owned(),
            actual: Some("not-hex".to_owned()),
        })
    );

    let invalid_review = unsigned_event(
        KIND_KNOWLEDGE_REVIEW,
        vec![
            vec!["contract", "radroots.knowledge.review.v1"],
            vec!["review_target", "not-an-event-id"],
        ],
        r#"{"schema":"radroots.knowledge.review.v1","schema_version":1}"#,
    );
    assert_eq!(
        validate_event_contract_shape(&invalid_review, "radroots.knowledge.review.v1"),
        Err(ContractValidationError::TagValueMismatch {
            contract_id: "radroots.knowledge.review.v1",
            name: "review_target",
            expected: "event_pointer".to_owned(),
            actual: Some("not-an-event-id".to_owned()),
        })
    );

    let invalid_geohash = unsigned_event(
        KIND_KNOWLEDGE_FIELD_REPORT,
        vec![
            vec!["contract", "radroots.knowledge.field_report.v1"],
            vec!["g", "invalid-a"],
        ],
        r#"{"schema":"radroots.knowledge.field_report.v1","schema_version":1}"#,
    );
    assert_eq!(
        validate_event_contract_shape(&invalid_geohash, "radroots.knowledge.field_report.v1"),
        Err(ContractValidationError::TagValueMismatch {
            contract_id: "radroots.knowledge.field_report.v1",
            name: "g",
            expected: "geohash".to_owned(),
            actual: Some("invalid-a".to_owned()),
        })
    );

    let invalid_address = unsigned_event(
        KIND_WIKI_REDIRECT,
        vec![vec!["d", "soil"], vec!["a", "30818:not-hex:soil"]],
        "",
    );
    assert_eq!(
        validate_event_contract_shape(&invalid_address, "radroots.wiki.redirect.v1"),
        Err(ContractValidationError::TagValueMismatch {
            contract_id: "radroots.wiki.redirect.v1",
            name: "a",
            expected: "addressable_coordinate".to_owned(),
            actual: Some("30818:not-hex:soil".to_owned()),
        })
    );

    let invalid_event_id = unsigned_event_owned(
        KIND_WIKI_MERGE_REQUEST,
        vec![
            vec![
                "a".to_owned(),
                format!("30818:{}:soil", crate::test_valid_hex_64('0')),
            ],
            vec!["p".to_owned(), crate::test_valid_hex_64('1')],
            vec!["e".to_owned(), "not-hex".to_owned()],
        ],
        "",
    );
    assert_eq!(
        validate_event_contract_shape(&invalid_event_id, "radroots.wiki.merge_request.v1"),
        Err(ContractValidationError::TagValueMismatch {
            contract_id: "radroots.wiki.merge_request.v1",
            name: "e",
            expected: "event_id".to_owned(),
            actual: Some("not-hex".to_owned()),
        })
    );

    let valid_source = unsigned_event_owned(
        KIND_KNOWLEDGE_CLAIM,
        vec![
            vec![
                "contract".to_owned(),
                "radroots.knowledge.claim.v1".to_owned(),
            ],
            event_ref_tag(
                "source",
                hex_64('a').as_str(),
                hex_64('b').as_str(),
                KIND_KNOWLEDGE_SOURCE,
            ),
            vec!["citation".to_owned(), hex_64('c')],
        ],
        claim_content,
    );
    assert_eq!(
        validate_event_contract_shape(&valid_source, "radroots.knowledge.claim.v1"),
        Ok(())
    );
}

#[test]
fn tag_value_shape_helpers_cover_contract_registry_value_types() {
    let event_id = hex_64('a');
    let public_key = hex_64('b');
    let coordinate = format!("{KIND_WIKI_ARTICLE}:{public_key}:soil");
    let replaceable_coordinate = format!("0:{public_key}:");
    let valid_pointer = vec![
        "source".to_owned(),
        event_id.clone(),
        public_key.clone(),
        KIND_KNOWLEDGE_SOURCE.to_string(),
        "soil".to_owned(),
        "ws://relay.example.com".to_owned(),
        "wss://relay.example.net".to_owned(),
    ];
    let empty_d_pointer = vec![
        "source".to_owned(),
        event_id.clone(),
        public_key.clone(),
        KIND_KNOWLEDGE_SOURCE.to_string(),
        String::new(),
    ];

    assert!(!tag_value_is_valid(
        &owned_tag(&["source"]),
        TagValueType::EventPointer
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["a", coordinate.as_str()]),
        TagValueType::AddressableCoordinate
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["a", "30818:not-hex:soil"]),
        TagValueType::AddressableCoordinate
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["a", replaceable_coordinate.as_str()]),
        TagValueType::Nip01Coordinate
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["a", format!("0:{public_key}:profile").as_str()]),
        TagValueType::Nip01Coordinate
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["contract", "radroots.knowledge.claim.v1"]),
        TagValueType::ContractId
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["contract", "radroots.unknown.v1"]),
        TagValueType::ContractId
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["d", "soil"]),
        TagValueType::DTag
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["d", ""]),
        TagValueType::DTag
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["e", event_id.as_str()]),
        TagValueType::EventId
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["citation", event_id.as_str()]),
        TagValueType::Sha256
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["e", "not-hex"]),
        TagValueType::EventId
    ));
    assert!(tag_value_is_valid(
        &valid_pointer,
        TagValueType::EventPointer
    ));
    assert!(tag_value_is_valid(
        &empty_d_pointer,
        TagValueType::EventPointer
    ));
    assert!(!event_pointer_tag_is_valid(&owned_tag(&[
        "source",
        "not-hex",
        public_key.as_str(),
        "1",
        ""
    ])));
    assert!(!event_pointer_tag_is_valid(&owned_tag(&[
        "source",
        event_id.as_str(),
        "not-hex",
        "1",
        ""
    ])));
    assert!(!event_pointer_tag_is_valid(&owned_tag(&[
        "source",
        event_id.as_str(),
        public_key.as_str(),
        "not-a-kind",
        ""
    ])));
    assert!(!event_pointer_tag_is_valid(&owned_tag(&[
        "source",
        event_id.as_str(),
        public_key.as_str(),
        "1"
    ])));
    assert!(!event_pointer_tag_is_valid(&owned_tag(&[
        "source",
        event_id.as_str(),
        public_key.as_str(),
        "1",
        "bad tag"
    ])));
    assert!(!event_pointer_tag_is_valid(&owned_tag(&[
        "source",
        event_id.as_str(),
        public_key.as_str(),
        "1",
        "",
        "https://relay.example.com"
    ])));
    assert!(tag_value_is_valid(
        &owned_tag(&["g", "9q8yy"]),
        TagValueType::Geohash
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["g", "9Q8YY"]),
        TagValueType::Geohash
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["g", ""]),
        TagValueType::Geohash
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["g", "1234567890123"]),
        TagValueType::Geohash
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["g", "aaaaa"]),
        TagValueType::Geohash
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["k", "30818"]),
        TagValueType::Kind
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["k", "not-a-kind"]),
        TagValueType::Kind
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["p", public_key.as_str()]),
        TagValueType::PublicKey
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["p", "not-hex"]),
        TagValueType::PublicKey
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["relay", "ws://relay.example.com"]),
        TagValueType::RelayUrl
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["relay", "wss://relay.example.com"]),
        TagValueType::RelayUrl
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["relay", "http://relay.example.com"]),
        TagValueType::RelayUrl
    ));
    assert!(relay_url_is_valid("ws://relay.example.com"));
    assert!(relay_url_is_valid("wss://relay.example.com"));
    assert!(!relay_url_is_valid("ws://"));
    assert!(!relay_url_is_valid("http://relay.example.com"));
    assert!(!relay_url_is_valid(" wss://relay.example.com"));
    assert!(!relay_url_is_valid("wss://relay.example.com "));
    assert!(!relay_url_is_valid("wss://relay.example.com\nmiddle"));
    assert!(tag_value_is_valid(
        &owned_tag(&["title", "Soil Guide"]),
        TagValueType::Text
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["title", "   "]),
        TagValueType::Text
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["title", "Soil\nGuide"]),
        TagValueType::Text
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["expiration", "1700000000"]),
        TagValueType::UnixTimestamp
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["expiration", "not-time"]),
        TagValueType::UnixTimestamp
    ));
    assert!(tag_value_is_valid(
        &owned_tag(&["image", "https://example.com"]),
        TagValueType::Url
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["image", "wss://example.com"]),
        TagValueType::Url
    ));
    assert!(url_is_valid("http://example.com"));
    assert!(url_is_valid("https://example.com"));
    assert!(!url_is_valid("http://"));
    assert!(!url_is_valid("wss://example.com"));
    assert!(!url_is_valid(" https://example.com"));
    assert!(!url_is_valid("https://example.com "));
    assert!(!url_is_valid("https://example.com\nmiddle"));
    assert!(tag_value_is_valid(
        &owned_tag(&["uuid", "123e4567-e89b-12d3-a456-426614174000"]),
        TagValueType::Uuid
    ));
    assert!(!tag_value_is_valid(
        &owned_tag(&["uuid", "123e4567-e89b-12d3-a456-42661417400"]),
        TagValueType::Uuid
    ));
    assert!(uuid_is_valid("123e4567-e89b-12d3-a456-426614174000"));
    assert!(!uuid_is_valid("123e4567-e89b-12d3-a456-42661417400"));
    assert!(!uuid_is_valid("123e4567xe89b-12d3-a456-426614174000"));
    assert!(!uuid_is_valid("123e4567-e89b-12d3-a456-42661417400x"));

    let expectations = [
        (
            TagValueType::AddressableCoordinate,
            "addressable_coordinate",
        ),
        (TagValueType::ContractId, "contract_id"),
        (TagValueType::DTag, "d_tag"),
        (TagValueType::EventId, "event_id"),
        (TagValueType::EventPointer, "event_pointer"),
        (TagValueType::Geohash, "geohash"),
        (TagValueType::Kind, "kind"),
        (TagValueType::Nip01Coordinate, "nip01_coordinate"),
        (TagValueType::PublicKey, "public_key"),
        (TagValueType::RelayUrl, "relay_url"),
        (TagValueType::Sha256, "sha256"),
        (TagValueType::Text, "text"),
        (TagValueType::UnixTimestamp, "unix_timestamp"),
        (TagValueType::Url, "url"),
        (TagValueType::Uuid, "uuid"),
    ];
    for (value_type, expected) in expectations {
        assert_eq!(tag_value_type_expectation(value_type), expected);
    }
}

#[test]
fn validate_custom_knowledge_contract_rejects_missing_schema_and_bad_version() {
    let missing_schema = unsigned_event(
        KIND_KNOWLEDGE_CLAIM,
        vec![vec!["contract", "radroots.knowledge.claim.v1"]],
        r#"{"schema_version":1}"#,
    );
    assert_eq!(
        validate_event_contract_shape(&missing_schema, "radroots.knowledge.claim.v1"),
        Err(ContractValidationError::MissingContentField {
            contract_id: "radroots.knowledge.claim.v1",
            field: "schema",
        })
    );

    let bad_version = unsigned_event(
        KIND_KNOWLEDGE_CLAIM,
        vec![vec!["contract", "radroots.knowledge.claim.v1"]],
        r#"{"schema":"radroots.knowledge.claim.v1","schema_version":2}"#,
    );
    assert_eq!(
        validate_event_contract_shape(&bad_version, "radroots.knowledge.claim.v1"),
        Err(ContractValidationError::ContentFieldMismatch {
            contract_id: "radroots.knowledge.claim.v1",
            field: "schema_version",
            expected: "1".to_owned(),
        })
    );
}

#[test]
fn validates_nip54_empty_redirect_content() {
    let event = unsigned_event_owned(
        KIND_WIKI_REDIRECT,
        vec![
            vec!["d".to_owned(), "soil".to_owned()],
            vec![
                "a".to_owned(),
                format!("30818:{}:soil", crate::test_valid_hex_64('0')),
            ],
        ],
        "",
    );

    assert_eq!(
        validate_event_contract_shape(&event, "radroots.wiki.redirect.v1"),
        Ok(())
    );

    let invalid = unsigned_event_owned(
        KIND_WIKI_REDIRECT,
        vec![
            vec!["d".to_owned(), "soil".to_owned()],
            vec![
                "a".to_owned(),
                format!("30818:{}:soil", crate::test_valid_hex_64('0')),
            ],
        ],
        "{}",
    );
    assert_eq!(
        validate_event_contract_shape(&invalid, "radroots.wiki.redirect.v1"),
        Err(ContractValidationError::ContentMustBeEmpty {
            contract_id: "radroots.wiki.redirect.v1",
        })
    );
}

#[test]
fn exposes_validation_error_codes() {
    for (error, code) in [
        (
            ContractValidationError::UnknownContract {
                contract_id: "missing".to_owned(),
            },
            "unknown_contract",
        ),
        (
            ContractValidationError::AdmissionRequired {
                contract_id: "radroots.social.ask.v1",
            },
            "admission_required",
        ),
        (
            ContractValidationError::ContractMatch {
                error: ContractMatchError::UnsupportedKind(999_999),
            },
            "contract_match",
        ),
        (
            ContractValidationError::KindMismatch {
                expected: KIND_PROFILE,
                actual: KIND_POST,
            },
            "kind_mismatch",
        ),
        (
            ContractValidationError::ContentMustBeEmpty {
                contract_id: "radroots.wiki.redirect.v1",
            },
            "content_must_be_empty",
        ),
        (
            ContractValidationError::InvalidJsonContent {
                contract_id: "radroots.knowledge.claim.v1",
            },
            "invalid_json_content",
        ),
        (
            ContractValidationError::MissingTag {
                contract_id: "radroots.knowledge.claim.v1",
                name: "contract",
            },
            "missing_tag",
        ),
        (
            ContractValidationError::TagCardinalityMismatch {
                contract_id: "radroots.knowledge.claim.v1",
                name: "contract",
            },
            "tag_cardinality_mismatch",
        ),
        (
            ContractValidationError::TagValueMismatch {
                contract_id: "radroots.knowledge.claim.v1",
                name: "contract",
                expected: "radroots.knowledge.claim.v1".to_owned(),
                actual: None,
            },
            "tag_value_mismatch",
        ),
        (
            ContractValidationError::MissingContentField {
                contract_id: "radroots.knowledge.claim.v1",
                field: "schema",
            },
            "missing_content_field",
        ),
        (
            ContractValidationError::ContentFieldMismatch {
                contract_id: "radroots.knowledge.claim.v1",
                field: "schema",
                expected: "radroots.knowledge.claim.v1".to_owned(),
            },
            "content_field_mismatch",
        ),
        (
            ContractValidationError::ForbiddenContentField {
                contract_id: "radroots.knowledge.claim.v1",
                field: "review_status",
            },
            "forbidden_content_field",
        ),
    ] {
        assert_eq!(error.code(), code);
    }
}

#[test]
fn tag_helpers_cover_missing_names_and_cardinality_mismatches() {
    let tags = vec![
        vec!["p".to_owned(), "counterparty".to_owned()],
        vec!["d".to_owned()],
    ];

    assert_eq!(tag_value(&tags, "d"), None);
    assert_eq!(tag_value(&tags, "p"), Some("counterparty"));

    let malformed = [
        tag(
            "d",
            TagCardinality::OptionalOne,
            TagSemantic::Identifier,
            TagValueType::DTag,
            true,
        ),
        tag(
            "p",
            TagCardinality::RequiredOne,
            TagSemantic::Counterparty,
            TagValueType::PublicKey,
            true,
        ),
    ];

    assert!(
        !malformed
            .iter()
            .any(|tag| tag.name == "d" && tag.cardinality == TagCardinality::RequiredOne)
    );
}

#[test]
fn relay_indexed_tags_are_single_letter() {
    for contract in all_event_contracts() {
        for tag in contract.tags {
            if tag.relay_indexed {
                assert_eq!(tag.name.len(), 1, "{}:{}", contract.id, tag.name);
            }
        }
    }
}

#[test]
fn addressable_event_contracts_require_d_tags() {
    for contract in all_event_contracts() {
        if contract.class == EventClass::Addressable {
            let d_tag_cardinality = contract
                .tags
                .iter()
                .find(|tag| tag.name == "d")
                .map(|tag| tag.cardinality);
            assert_eq!(
                d_tag_cardinality,
                Some(TagCardinality::RequiredOne),
                "{}",
                contract.id
            );
        }
    }
}

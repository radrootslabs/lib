#![cfg(feature = "json")]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};
use radroots_event::{
    contract::{EventAuthoringPolicy, all_event_contracts},
    envelope::kind::KIND_CLASSIFIED_LISTING,
    food::availability::{
        FoodAvailabilityDetails, FoodAvailabilityDetailsParts, FoodAvailabilityStatus, FoodContent,
        FoodCurrency, FoodIdentifier, FoodPrice, FoodPublishedAt, FoodQuantity, FoodText, FoodUnit,
    },
    media::AuthoredImage,
    post::{
        AuthoredAsk, AuthoredPhotoUpdate, AuthoredPostImage, AuthoredUpdate, PostImageDimensions,
        comment::{AuthoredNip22Comment, Nip22EventRootReference},
        deletion::{
            AuthoredNip09DeletionRequest, Nip09DeletionAddressTarget, Nip09DeletionEventTarget,
        },
        reply::{AuthoredNip10Reply, Nip10ReplyReference},
    },
    profile::AuthoredProfile,
};
use radroots_event_codec::authoring::{
    AuthoredEventPlan, PlanWireV1, REGISTRY_V7_TYPED_AUTHORING_CONTRACT_IDS,
};
use serde::Deserialize;

const AUTHOR: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const OTHER_AUTHOR: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";
const CREATED_AT: u64 = 1_784_347_200;
const ROOT_EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TARGET_EVENT_ID: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
const RELAY: &str = "wss://relay.example.com";
const WORKSPACE_CORPUS_PATH: &str =
    "../../contracts/conformance/vectors/event/authored_operations.v1.json";

#[derive(Debug, Deserialize)]
struct Corpus {
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
struct Vector {
    input: Input,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
struct Input {
    contract_id: String,
    authoring: String,
}

#[derive(Debug, Deserialize)]
struct Expected {
    kind: u32,
    created_at: u64,
    pubkey: String,
    tags: Vec<Vec<String>>,
    content: String,
    event_id: String,
}

#[test]
fn registry_inventory_has_exactly_one_typed_plan_path_and_round_trips_every_plan() {
    let plans = typed_plans();
    let registry = all_event_contracts()
        .iter()
        .filter(|contract| contract.authoring_policy() == EventAuthoringPolicy::TypedOnly)
        .map(|contract| contract.id)
        .collect::<Vec<_>>();
    assert_eq!(registry, REGISTRY_V7_TYPED_AUTHORING_CONTRACT_IDS);
    assert_eq!(plans.len(), REGISTRY_V7_TYPED_AUTHORING_CONTRACT_IDS.len());
    assert_eq!(
        plans.keys().copied().collect::<BTreeSet<_>>(),
        REGISTRY_V7_TYPED_AUTHORING_CONTRACT_IDS
            .into_iter()
            .collect::<BTreeSet<_>>()
    );

    for (contract_id, plan) in &plans {
        assert_eq!(plan.body().contract().contract_id().as_str(), *contract_id);
        let json = PlanWireV1::from_plan(plan).to_json().expect("plan wire");
        assert_eq!(
            PlanWireV1::from_json(&json)
                .unwrap_or_else(|error| panic!("{contract_id} historical decode failed: {error}"))
                .into_plan(),
            *plan,
            "{contract_id}"
        );
    }
}

#[test]
fn every_typed_plan_is_byte_identical_to_the_frozen_authored_wire_corpus() {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_CORPUS_PATH);
    if !workspace_path.is_file() {
        return;
    }
    let corpus: Corpus = serde_json::from_str(
        &fs::read_to_string(&workspace_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", workspace_path.display())),
    )
    .expect("authored corpus");
    let expected = corpus
        .vectors
        .into_iter()
        .filter(|vector| vector.input.authoring == "typed")
        .map(|vector| (vector.input.contract_id, vector.expected))
        .collect::<BTreeMap<_, _>>();
    let plans = typed_plans();
    assert_eq!(expected.len(), plans.len());

    for (contract_id, plan) in plans {
        let expected = expected
            .get(contract_id)
            .unwrap_or_else(|| panic!("missing corpus vector for {contract_id}"));
        assert_eq!(plan.created_at(), expected.created_at, "{contract_id}");
        assert_eq!(plan.author().to_hex(), expected.pubkey, "{contract_id}");
        assert_eq!(plan.body().kind(), expected.kind, "{contract_id}");
        assert_eq!(plan.body().tags(), expected.tags, "{contract_id}");
        assert_eq!(plan.body().content(), expected.content, "{contract_id}");
        assert_eq!(
            plan.expected_event_id().to_hex(),
            expected.event_id,
            "{contract_id}"
        );
    }
}

fn typed_plans() -> BTreeMap<&'static str, AuthoredEventPlan> {
    let mut plans = BTreeMap::new();
    let profile = AuthoredProfile::new("Alice \"Sprout\"")
        .expect("profile")
        .with_display_name("Alice's Orchard")
        .with_about("Tree fruit\nDirect from the farm")
        .with_bot(false);
    plans.insert(
        "radroots.profile.metadata.v1",
        AuthoredEventPlan::from_profile(&profile, CREATED_AT, AUTHOR).expect("profile plan"),
    );

    let update = AuthoredUpdate::new("Farm update: \"ready\"\\\n🍓").expect("update");
    plans.insert(
        "radroots.social.update.v1",
        AuthoredEventPlan::from_update(&update, CREATED_AT, AUTHOR).expect("update plan"),
    );

    let image = authored_post_image();
    let photo = AuthoredPhotoUpdate::new(format!("Harvest {}", image.url()), vec![image.clone()])
        .expect("photo");
    plans.insert(
        "radroots.social.photo_update.v1",
        AuthoredEventPlan::from_photo_update(&photo, CREATED_AT, AUTHOR).expect("photo plan"),
    );

    let ask =
        AuthoredAsk::new(format!("Is this ready? {}", image.url()), vec![image]).expect("ask");
    plans.insert(
        "radroots.social.ask.v1",
        AuthoredEventPlan::from_ask(&ask, CREATED_AT, AUTHOR).expect("ask plan"),
    );

    let reply = AuthoredNip10Reply::direct(
        "Direct reply",
        Nip10ReplyReference::parse(ROOT_EVENT_ID, OTHER_AUTHOR, Some(RELAY)).expect("root"),
    )
    .expect("reply");
    plans.insert(
        "radroots.social.reply.v1",
        AuthoredEventPlan::from_nip10_reply(&reply, CREATED_AT, AUTHOR).expect("reply plan"),
    );

    let deletion = AuthoredNip09DeletionRequest::new(
        "superseded",
        vec![Nip09DeletionEventTarget::parse(TARGET_EVENT_ID, 1).expect("event target")],
        vec![
            Nip09DeletionAddressTarget::parse(format!("30402:{OTHER_AUTHOR}:carrots"))
                .expect("listing target"),
            Nip09DeletionAddressTarget::parse(format!("0:{OTHER_AUTHOR}:"))
                .expect("profile target"),
        ],
    )
    .expect("deletion");
    plans.insert(
        "radroots.social.deletion_request.v1",
        AuthoredEventPlan::from_nip09_deletion_request(&deletion, CREATED_AT, AUTHOR)
            .expect("deletion plan"),
    );

    let comment = AuthoredNip22Comment::top_level_event(
        "Are these carrots available Saturday?",
        Nip22EventRootReference::parse(
            ROOT_EVENT_ID,
            OTHER_AUTHOR,
            KIND_CLASSIFIED_LISTING,
            Some(RELAY),
        )
        .expect("comment root"),
    )
    .expect("comment");
    plans.insert(
        "radroots.social.comment.v1",
        AuthoredEventPlan::from_nip22_comment(&comment, CREATED_AT, AUTHOR).expect("comment plan"),
    );

    plans.insert(
        "radroots.food.availability.v1",
        AuthoredEventPlan::from_food_availability(&food_details(), CREATED_AT, AUTHOR)
            .expect("food plan"),
    );
    plans
}

fn authored_post_image() -> AuthoredPostImage {
    let bytes = b"strawberries";
    let hash = Sha256::digest(bytes);
    let media_type = MediaType::parse("image/webp").expect("media type");
    let descriptor = BlobDescriptor::new(
        BlobUrl::parse(&format!("https://media.example/{hash}.webp")).expect("blob URL"),
        hash,
        bytes.len() as u64,
        media_type.clone(),
        CREATED_AT,
    )
    .expect("descriptor")
    .approve_reference()
    .expect("approved reference")
    .verify_bytes(bytes, &media_type)
    .expect("verified bytes");
    AuthoredPostImage::new(
        AuthoredImage::try_from(descriptor).expect("authored image"),
        PostImageDimensions::new(1200, 900).expect("dimensions"),
        "Harvest",
    )
    .expect("post image")
}

fn food_details() -> FoodAvailabilityDetails {
    FoodAvailabilityDetails::new(FoodAvailabilityDetailsParts {
        content: FoodContent::new("Carrots available this week.").expect("content"),
        identifier: FoodIdentifier::parse("nantes-carrots").expect("identifier"),
        title: FoodText::new("Nantes Carrots").expect("title"),
        summary: FoodText::new("Fresh bunches").expect("summary"),
        published_at: FoodPublishedAt::new(1_784_347_100).expect("published_at"),
        location: FoodText::new("Central Saanich, BC").expect("location"),
        price: FoodPrice::new(
            "3",
            FoodCurrency::parse("CAD").expect("currency"),
            FoodUnit::Pound,
        )
        .expect("price"),
        quantity: Some(FoodQuantity::new("24", FoodUnit::Pound).expect("quantity")),
        status: FoodAvailabilityStatus::Active,
        images: Vec::new(),
    })
    .expect("food details")
}

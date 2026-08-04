mod support;

use std::{borrow::Cow, fs, path::Path};

use nostr::{
    Event, Keys, SecretKey, UnsignedEvent,
    secp256k1::rand::{CryptoRng, RngCore},
};
use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};
use radroots_event::{
    GenericEventDraft,
    envelope::kind::KIND_CLASSIFIED_LISTING,
    food::availability::{
        FoodAvailabilityDetails, FoodAvailabilityDetailsParts, FoodAvailabilityStatus, FoodContent,
        FoodCurrency, FoodIdentifier, FoodPrice, FoodPublishedAt, FoodQuantity, FoodText, FoodUnit,
    },
    id::{ClassifiedListingAddress, DTag, EventId, InventoryBinId, TradeId},
    media::AuthoredImage,
    post::{
        AuthoredAsk, AuthoredPhotoUpdate, AuthoredPostImage, AuthoredUpdate, PostImageDimensions,
        RADROOTS_POST_CONTENT_MAX_BYTES,
        comment::{AuthoredNip22Comment, Nip22EventRootReference},
        deletion::{
            AuthoredNip09DeletionRequest, Nip09DeletionAddressTarget, Nip09DeletionEventTarget,
        },
        reply::{AuthoredNip10Reply, Nip10ReplyReference},
    },
    profile::AuthoredProfile,
    trade::{
        FulfillmentProfileV1, RADROOTS_TRADE_PROPOSAL_CONTRACT_ID, RADROOTS_TRADE_SCHEMA_VERSION,
        TradeCancellationProfileV1, TradeCandidateLineV1, TradeCandidateTermsV1,
        TradeEconomicAdjustmentV1, TradeEconomicsProfileV1, TradeMutationBodyV1,
        TradeMutationEnvelopeV1,
    },
    wire::canonical_nip01_event_id_preimage,
};
use radroots_event_codec::encode::trade::trade_mutation_event_build;
use radroots_identity::PublicKey;
use radroots_nostr::{
    event::{
        GenericBuilder, Kind, Timestamp, build_ask, build_food_availability,
        build_nip09_deletion_request, build_nip10_reply, build_nip22_comment, build_photo_update,
        build_profile, build_update,
    },
    tag::Tag,
};
use serde::{Deserialize, Serialize};

use support::{
    APPROVED_FIXTURE_NAMESPACE, FIXTURE_ALICE_PUBLIC_KEY_HEX, FIXTURE_ALICE_SECRET_KEY_HEX,
    FIXTURE_BOB_PUBLIC_KEY_HEX, RELAY_PRIMARY_WSS,
};

const CREATED_AT: u64 = 1_784_347_200;
const ROOT_EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TARGET_EVENT_ID: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";
const PACKAGED_CORPUS: &str = include_str!("fixtures/authored_operations.v1.json");
const WORKSPACE_CORPUS_PATH: &str =
    "../../contracts/conformance/vectors/event/authored_operations.v1.json";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Corpus {
    suite: String,
    contract_version: String,
    vectors: Vec<WireVector>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct WireVector {
    id: String,
    kind: String,
    input: WireInput,
    expected: WireExpected,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct WireInput {
    contract_id: String,
    authoring: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct WireExpected {
    kind: u32,
    created_at: u64,
    pubkey: String,
    tags: Vec<Vec<String>>,
    content: String,
    preimage: String,
    event_id: String,
    signature: String,
    raw_json: String,
}

#[test]
fn checked_in_authored_wire_corpus_is_exact_and_executable() {
    let corpus_json = conformance_vectors();
    let expected: Corpus = serde_json::from_str(&corpus_json).expect("authored corpus must parse");
    let actual = Corpus {
        suite: "authored_operations_wire".to_owned(),
        contract_version: "1.0.0".to_owned(),
        vectors: authored_wire_vectors(),
    };

    if expected.vectors.is_empty() {
        panic!(
            "authored corpus requires generated vectors:\n{}",
            serde_json::to_string_pretty(&actual).expect("serialize generated corpus")
        );
    }
    assert_eq!(actual, expected);
    assert_eq!(actual.vectors.len(), 10);
    assert_eq!(APPROVED_FIXTURE_NAMESPACE, "radroots-approved-fixture-v1");
    assert_eq!(
        actual
            .vectors
            .iter()
            .filter(|vector| vector.input.authoring == "typed")
            .count(),
        8
    );
    assert!(actual.vectors.iter().all(|vector| {
        !vector
            .expected
            .raw_json
            .contains(FIXTURE_ALICE_SECRET_KEY_HEX)
            && !vector
                .expected
                .preimage
                .contains(FIXTURE_ALICE_SECRET_KEY_HEX)
    }));
}

#[test]
fn authored_post_content_boundary_is_exact_without_bloating_the_corpus() {
    let exact = "x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES);
    assert!(AuthoredUpdate::new(exact).is_ok());
    assert!(AuthoredUpdate::new("x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES + 1)).is_err());
}

fn authored_wire_vectors() -> Vec<WireVector> {
    let keys = fixture_keys();
    let created_at = Timestamp::from_secs(CREATED_AT);
    let mut vectors = Vec::with_capacity(10);

    let profile = AuthoredProfile::new("Alice \"Sprout\"")
        .expect("profile name")
        .with_display_name("Alice's Orchard")
        .with_about("Tree fruit\nDirect from the farm")
        .with_bot(false);
    vectors.push(vector(
        "typed_profile_optional_escaping_001",
        "radroots.profile.metadata.v1",
        "typed",
        deterministic_event(
            build_profile(&profile)
                .expect("profile builder")
                .custom_created_at(created_at)
                .sign_with_keys(&keys)
                .expect("profile event"),
            &keys,
        ),
    ));

    let update = AuthoredUpdate::new("Farm update: \"ready\"\\\n🍓").expect("authored update");
    vectors.push(vector(
        "typed_update_escaping_002",
        "radroots.social.update.v1",
        "typed",
        deterministic_event(
            build_update(&update)
                .expect("update builder")
                .custom_created_at(created_at)
                .sign_with_keys(&keys)
                .expect("update event"),
            &keys,
        ),
    ));

    let image = authored_post_image();
    let photo = AuthoredPhotoUpdate::new(format!("Harvest {}", image.url()), vec![image.clone()])
        .expect("photo update");
    vectors.push(vector(
        "typed_photo_update_imeta_003",
        "radroots.social.photo_update.v1",
        "typed",
        deterministic_event(
            build_photo_update(&photo)
                .expect("photo builder")
                .custom_created_at(created_at)
                .sign_with_keys(&keys)
                .expect("photo event"),
            &keys,
        ),
    ));

    let ask =
        AuthoredAsk::new(format!("Is this ready? {}", image.url()), vec![image]).expect("ask");
    vectors.push(vector(
        "typed_ask_marker_order_004",
        "radroots.social.ask.v1",
        "typed",
        deterministic_event(
            build_ask(&ask)
                .expect("ask builder")
                .custom_created_at(created_at)
                .sign_with_keys(&keys)
                .expect("ask event"),
            &keys,
        ),
    ));

    let reply = AuthoredNip10Reply::direct(
        "Direct reply",
        Nip10ReplyReference::parse(
            ROOT_EVENT_ID,
            FIXTURE_BOB_PUBLIC_KEY_HEX,
            Some(RELAY_PRIMARY_WSS),
        )
        .expect("reply root"),
    )
    .expect("reply");
    vectors.push(vector(
        "typed_nip10_direct_reply_005",
        "radroots.social.reply.v1",
        "typed",
        deterministic_event(
            build_nip10_reply(&reply)
                .expect("reply builder")
                .custom_created_at(created_at)
                .sign_with_keys(&keys)
                .expect("reply event"),
            &keys,
        ),
    ));

    let deletion = deletion_request();
    vectors.push(vector(
        "typed_nip09_sorted_targets_006",
        "radroots.social.deletion_request.v1",
        "typed",
        deterministic_event(
            build_nip09_deletion_request(&deletion)
                .expect("deletion builder")
                .custom_created_at(created_at)
                .sign_with_keys(&keys)
                .expect("deletion event"),
            &keys,
        ),
    ));

    let comment = AuthoredNip22Comment::top_level_event(
        "Are these carrots available Saturday?",
        Nip22EventRootReference::parse(
            ROOT_EVENT_ID,
            FIXTURE_BOB_PUBLIC_KEY_HEX,
            KIND_CLASSIFIED_LISTING,
            Some(RELAY_PRIMARY_WSS),
        )
        .expect("comment root"),
    )
    .expect("comment");
    vectors.push(vector(
        "typed_nip22_top_level_event_007",
        "radroots.social.comment.v1",
        "typed",
        deterministic_event(
            build_nip22_comment(&comment)
                .expect("comment builder")
                .custom_created_at(created_at)
                .sign_with_keys(&keys)
                .expect("comment event"),
            &keys,
        ),
    ));

    vectors.push(vector(
        "typed_food_availability_optional_quantity_008",
        "radroots.food.availability.v1",
        "typed",
        deterministic_event(
            build_food_availability(&food_details(), created_at)
                .expect("food builder")
                .sign_with_keys(&keys)
                .expect("food event"),
            &keys,
        ),
    ));

    vectors.push(generic_vector(
        "generic_operational_listing_009",
        "radroots.operational_listing.published.v1",
        CREATED_AT,
        30_402,
        operational_listing_tags(),
        "# Nantes Carrots\n\nFresh bunches harvested in Saanich".to_owned(),
        &keys,
    ));

    let trade = trade_mutation_event_build(trade_proposal()).expect("trade wire parts");
    vectors.push(generic_vector(
        "generic_trade_proposal_010",
        RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
        CREATED_AT,
        trade.kind,
        trade.tags,
        trade.content,
        &keys,
    ));

    vectors
}

fn generic_vector(
    id: &str,
    contract_id: &str,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    keys: &Keys,
) -> WireVector {
    let draft = GenericEventDraft::new(
        contract_id,
        kind,
        created_at,
        tags,
        content,
        FIXTURE_ALICE_PUBLIC_KEY_HEX,
    )
    .expect("validated generic draft");
    draft
        .validate_for_authoring()
        .expect("authorable generic draft");
    let nostr_tags = draft
        .tags_as_vec()
        .into_iter()
        .map(Tag::parse)
        .collect::<Result<Vec<_>, _>>()
        .expect("Nostr tags");
    let event = GenericBuilder::new(
        Kind::Custom(u16::try_from(kind).expect("Nostr kind")),
        draft.content(),
    )
    .tags(nostr_tags)
    .custom_created_at(Timestamp::from_secs(created_at))
    .sign_with_keys(keys)
    .expect("generic event");
    assert_eq!(event.id.to_hex(), draft.expected_event_id().to_hex());
    vector(id, contract_id, "generic", deterministic_event(event, keys))
}

fn deterministic_event(event: Event, keys: &Keys) -> Event {
    let unsigned = UnsignedEvent {
        id: Some(event.id),
        pubkey: event.pubkey,
        created_at: event.created_at,
        kind: event.kind,
        tags: event.tags,
        content: event.content,
    };
    unsigned
        .sign_with_ctx(nostr::SECP256K1, &mut CorpusAuxRng, keys)
        .expect("deterministically signed corpus event")
}

fn vector(id: &str, contract_id: &str, authoring: &str, event: Event) -> WireVector {
    event.verify().expect("valid corpus event");
    let tags = event
        .tags
        .iter()
        .map(|tag| tag.as_slice().to_vec())
        .collect::<Vec<_>>();
    let preimage = canonical_nip01_event_id_preimage(
        &event.pubkey.to_hex(),
        event.created_at.as_secs(),
        u32::from(event.kind.as_u16()),
        &tags,
        &event.content,
    )
    .expect("canonical preimage");
    WireVector {
        id: id.to_owned(),
        kind: "authored_operations.wire".to_owned(),
        input: WireInput {
            contract_id: contract_id.to_owned(),
            authoring: authoring.to_owned(),
        },
        expected: WireExpected {
            kind: u32::from(event.kind.as_u16()),
            created_at: event.created_at.as_secs(),
            pubkey: event.pubkey.to_hex(),
            tags,
            content: event.content.clone(),
            preimage,
            event_id: event.id.to_hex(),
            signature: event.sig.to_string(),
            raw_json: serde_json::to_string(&event).expect("canonical event JSON"),
        },
    }
}

struct CorpusAuxRng;

impl RngCore for CorpusAuxRng {
    fn next_u32(&mut self) -> u32 {
        0x7261_6472
    }

    fn next_u64(&mut self) -> u64 {
        0x7261_6472_6f6f_7473
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        for (index, byte) in destination.iter_mut().enumerate() {
            *byte = b"radroots-authored-wire-corpus"[index % 29];
        }
    }

    fn try_fill_bytes(
        &mut self,
        destination: &mut [u8],
    ) -> Result<(), nostr::secp256k1::rand::Error> {
        self.fill_bytes(destination);
        Ok(())
    }
}

impl CryptoRng for CorpusAuxRng {}

fn fixture_keys() -> Keys {
    Keys::new(SecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("fixture secret key"))
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

fn deletion_request() -> AuthoredNip09DeletionRequest {
    AuthoredNip09DeletionRequest::new(
        "superseded",
        vec![Nip09DeletionEventTarget::parse(TARGET_EVENT_ID, 1).expect("event target")],
        vec![
            Nip09DeletionAddressTarget::parse(format!(
                "30402:{FIXTURE_BOB_PUBLIC_KEY_HEX}:carrots"
            ))
            .expect("listing target"),
            Nip09DeletionAddressTarget::parse(format!("0:{FIXTURE_BOB_PUBLIC_KEY_HEX}:"))
                .expect("profile target"),
        ],
    )
    .expect("deletion request")
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

fn operational_listing_tags() -> Vec<Vec<String>> {
    [
        &["d", "AAAAAAAAAAAAAAAAAAAAAg"][..],
        &["p", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
        &["a", "30340:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:AAAAAAAAAAAAAAAAAAAAAA"],
        &["key", "carrot-nantes"],
        &["title", "Nantes Carrots"],
        &["category", "produce"],
        &["summary", "Fresh bunches harvested in Saanich"],
        &["published_at", "1700000000"],
        &["radroots:primary_bin", "bunch"],
        &["radroots:bin", "bunch", "1", "each"],
        &["radroots:price", "bunch", "4", "CAD", "1", "each"],
        &["price", "4", "CAD"],
        &["inventory", "24"],
        &["status", "active"],
        &["delivery", "pickup"],
        &["location", "Saanich Peninsula", "Victoria", "BC", "CA"],
        &["g", "c28hr"],
    ]
    .into_iter()
    .map(|tag| tag.iter().map(|value| (*value).to_owned()).collect())
    .collect()
}

fn trade_proposal() -> TradeMutationEnvelopeV1 {
    TradeMutationEnvelopeV1 {
        mutation_id: None,
        contract_id: RADROOTS_TRADE_PROPOSAL_CONTRACT_ID.to_owned(),
        schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
        trade_id: TradeId::parse("11111111111111111111111111111111").expect("trade id"),
        root_mutation_id: None,
        buyer_pubkey: PublicKey::from_hex(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("buyer key"),
        seller_pubkey: PublicKey::from_hex(FIXTURE_BOB_PUBLIC_KEY_HEX).expect("seller key"),
        farm_id: DTag::parse("farm-1").expect("farm id"),
        parent_mutation_ids: Vec::new(),
        author_pubkey: PublicKey::from_hex(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("author key"),
        counterparty_pubkey: PublicKey::from_hex(FIXTURE_BOB_PUBLIC_KEY_HEX)
            .expect("counterparty key"),
        authored_at_unix_s: CREATED_AT,
        body: TradeMutationBodyV1::Proposal {
            candidate: TradeCandidateTermsV1 {
                candidate_id: None,
                schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
                base_candidate_id: None,
                supersession_intent: None,
                buyer_pubkey: PublicKey::from_hex(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("buyer key"),
                seller_pubkey: PublicKey::from_hex(FIXTURE_BOB_PUBLIC_KEY_HEX).expect("seller key"),
                farm_id: DTag::parse("farm-1").expect("farm id"),
                lines: vec![TradeCandidateLineV1 {
                    line_id: DTag::parse("line-1").expect("line id"),
                    listing_addr: ClassifiedListingAddress::parse(format!(
                        "30402:{FIXTURE_BOB_PUBLIC_KEY_HEX}:listing-1"
                    ))
                    .expect("listing address"),
                    listing_event_id: EventId::parse("c".repeat(64)).expect("listing event id"),
                    listing_snapshot_sha256: "d".repeat(64),
                    product_id: "carrots".to_owned(),
                    option_id: None,
                    bin_id: InventoryBinId::parse("bin-1").expect("bin id"),
                    quantity_mantissa: "2".to_owned(),
                    quantity_scale: 0,
                    unit_code: "count".to_owned(),
                    unit_profile: "mvp-count".to_owned(),
                    unit_price_mantissa: "500".to_owned(),
                    currency_code: "USD".to_owned(),
                    line_subtotal_mantissa: "1000".to_owned(),
                    replaces_line_id: None,
                }],
                line_tombstones: Vec::new(),
                economics: TradeEconomicsProfileV1 {
                    profile_id: "mvp-fixed".to_owned(),
                    currency_code: "USD".to_owned(),
                    currency_exponent: 2,
                    rounding_profile: "half-even".to_owned(),
                    subtotal_mantissa: "1000".to_owned(),
                    discount_total_mantissa: "0".to_owned(),
                    adjustment_total_mantissa: "0".to_owned(),
                    total_mantissa: "1000".to_owned(),
                    adjustments: Vec::<TradeEconomicAdjustmentV1>::new(),
                },
                fulfillment: FulfillmentProfileV1 {
                    profile_id: "market-pickup".to_owned(),
                    method: "pickup".to_owned(),
                    starts_at_unix_s: 1_800_000_000,
                    ends_at_unix_s: 1_800_003_600,
                    timezone: "America/New_York".to_owned(),
                    utc_offset_seconds: -18_000,
                    fold: 0,
                    location_class: "farmstand".to_owned(),
                    requires_private_terms: false,
                },
                cancellation: TradeCancellationProfileV1 {
                    profile_id: "buyer-pre-agreement".to_owned(),
                    buyer_pre_agreement: true,
                    post_agreement_cutoff_unix_s: None,
                },
                private_terms: None,
                proposal_expires_at_unix_s: 1_799_999_000,
            },
        },
    }
}

fn conformance_vectors() -> Cow<'static, str> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    if !manifest_dir.join(WORKSPACE_CONTRACT_MARKER_PATH).is_file() {
        return Cow::Borrowed(PACKAGED_CORPUS);
    }
    let workspace_path = manifest_dir.join(WORKSPACE_CORPUS_PATH);
    let workspace = fs::read_to_string(&workspace_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", workspace_path.display()));
    assert_eq!(workspace, PACKAGED_CORPUS, "packaged authored corpus drift");
    Cow::Owned(workspace)
}

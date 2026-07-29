#![forbid(unsafe_code)]

use radroots_blossom::Sha256;
use radroots_event::food_availability::RadrootsFoodIdentifier;
use radroots_event_store::{
    RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1,
    RADROOTS_FOOD_AVAILABILITY_PROJECTION_VERSION_V1,
    RadrootsAddressableTransitionEventReferenceV1, RadrootsAddressableTransitionScopeV1,
    RadrootsEventIngest, RadrootsEventStore, RadrootsEventStoreSourceGeneration,
    RadrootsFoodAvailabilitySearchQueryV1, RadrootsFoodAvailabilityStatusFilterV1,
    RadrootsNip09SuppressionEvidenceV1, RadrootsRawHeadDecision,
    RadrootsStoreProducedCanonicalEventV1, RadrootsStoredFoodAvailabilityV1,
    RadrootsStoredRawEvent,
};
use radroots_identity::PublicKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256 as Sha256Hasher};

const RESULT_VECTOR_EXECUTOR_ID: &str =
    "radroots_event_store.food_availability_projection_v1.result_vector_executor.v1";
const SOURCE_GENERATION_ACTIVE_SENTINEL: &str = "active";
const RESULT_VECTOR_BYTES: &[u8] = include_bytes!(
    "../../../contracts/conformance/vectors/event_store/food_availability_projection.v1.json"
);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FoodAvailabilityProjectionVector {
    schema_version: u32,
    contract_id: String,
    feed_version: u32,
    projection_version: u32,
    scope_kinds: Vec<u32>,
    cases: Vec<ProjectionCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectionCase {
    id: String,
    events: Vec<ObservedEvent>,
    expected: ExpectedCase,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservedEvent {
    role: ProjectionInputRole,
    observed_at_ms: i64,
    expected_ingest: ExpectedIngest,
    event: SignedEvent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ProjectionInputRole {
    ScopedFood,
    ScopedNonFood,
    UnrelatedAddressable,
    Causal,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedIngest {
    admission_status: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    admission_code: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    contract_id: RequiredNullable<String>,
    event_class: String,
    valid_stream_eligible: bool,
    raw_head_decision: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SignedEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedCase {
    coordinate: ExpectedCoordinate,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    projection: RequiredNullable<ExpectedProjection>,
    searches: Vec<ExpectedSearch>,
    transition_page: ExpectedTransitionPage,
    event_visibility: Vec<ExpectedVisibility>,
    historical_visibility_witnesses: Vec<ExpectedHistoricalVisibilityWitness>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedCoordinate {
    kind: u32,
    pubkey: String,
    d_tag: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedProjection {
    event_id: String,
    content: String,
    title: String,
    summary: String,
    published_at: u64,
    location: String,
    price_amount: String,
    price_currency: String,
    price_unit: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    quantity_amount: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    quantity_unit: RequiredNullable<String>,
    status: String,
    diagnostics: Vec<String>,
    images: Vec<ExpectedImage>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedImage {
    #[serde(deserialize_with = "deserialize_required_nullable")]
    url: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    width: RequiredNullable<u32>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    height: RequiredNullable<u32>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    blossom_sha256: RequiredNullable<String>,
    diagnostics: Vec<String>,
    qualifies: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedSearch {
    query: String,
    event_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedTransitionPage {
    source_high_water: i64,
    has_more: bool,
    next_cursor: ExpectedTransitionCursor,
    transitions: Vec<ExpectedTransition>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedTransitionCursor {
    source_generation: String,
    feed_version: u32,
    scope_fingerprint: String,
    last_transition_seq: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedTransition {
    transition_seq: i64,
    source_generation: String,
    origin: String,
    coordinate: ExpectedCoordinate,
    raw_head: ExpectedEventReference,
    raw_head_created_at: u64,
    admission_status: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    admission_code: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    contract_id: RequiredNullable<String>,
    visibility: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    suppression: RequiredNullable<ExpectedSuppressionEvidence>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    cause_event: RequiredNullable<ExpectedTransitionCause>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    canonical_visible_event: RequiredNullable<ExpectedCanonicalVisibleEvent>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    retracted_event: RequiredNullable<ExpectedEventReference>,
    raw_head_decision: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedEventReference {
    event_id: String,
    event_seq: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedSuppressionEvidence {
    outcome: String,
    reason: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    event_reference_request_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    address_reference_request_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    address_reference_cutoff: RequiredNullable<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedTransitionCause {
    event: ExpectedEventReference,
    pubkey: String,
    created_at: u64,
    kind: u32,
    admission_status: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    admission_code: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    contract_id: RequiredNullable<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedCanonicalVisibleEvent {
    event: ExpectedEventReference,
    raw_json_sha256: String,
    admission_status: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    contract_id: RequiredNullable<String>,
    event_class: String,
    valid_stream_eligible: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedVisibility {
    event: ExpectedEventReference,
    source_generation: String,
    admission_status: String,
    decision: String,
    is_raw_head: bool,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    raw_head_event_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    suppression: RequiredNullable<ExpectedSuppressionEvidence>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedHistoricalVisibilityWitness {
    transition_seq: i64,
    event_id: String,
    final_decision: String,
}

#[derive(Debug)]
struct RequiredNullable<T>(Option<T>);

fn deserialize_required_nullable<'de, D, T>(
    deserializer: D,
) -> Result<RequiredNullable<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(RequiredNullable)
}

#[tokio::test]
async fn food_availability_projection_v1_result_vector() {
    assert_eq!(
        RESULT_VECTOR_EXECUTOR_ID,
        "radroots_event_store.food_availability_projection_v1.result_vector_executor.v1"
    );
    assert_eq!(sha256_hex(RESULT_VECTOR_BYTES).len(), 64);

    let vector: FoodAvailabilityProjectionVector =
        serde_json::from_slice(RESULT_VECTOR_BYTES).expect("strict FoodAvailability vector");
    assert_eq!(vector.schema_version, 1);
    assert_eq!(
        vector.contract_id,
        "radroots_event_store.food_availability_projection_v1"
    );
    assert_eq!(
        vector.feed_version,
        RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1
    );
    assert_eq!(
        vector.projection_version,
        RADROOTS_FOOD_AVAILABILITY_PROJECTION_VERSION_V1
    );
    assert_eq!(vector.scope_kinds, [30_402]);
    assert!(!vector.cases.is_empty());

    for case in vector.cases {
        execute_case(case).await;
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256Hasher::digest(bytes))
}

fn raw_head_decision_code(decision: &RadrootsRawHeadDecision) -> &'static str {
    match decision {
        RadrootsRawHeadDecision::Applied => "applied",
        RadrootsRawHeadDecision::NotHeadSelected
        | RadrootsRawHeadDecision::NotPersisted
        | RadrootsRawHeadDecision::SkippedDuplicate => "not_head_selected",
        RadrootsRawHeadDecision::SkippedOlder => "skipped_older",
        RadrootsRawHeadDecision::SkippedSameTimestampHigherEventId => {
            "skipped_same_timestamp_higher_event_id"
        }
        RadrootsRawHeadDecision::MalformedCoordinate => "malformed_coordinate",
    }
}

async fn execute_case(case: ProjectionCase) {
    let store = RadrootsEventStore::open_memory()
        .await
        .unwrap_or_else(|error| panic!("{}: open event store: {error}", case.id));

    for (index, observed) in case.events.iter().enumerate() {
        let (expected_kind, expected_event_class) = match observed.role {
            ProjectionInputRole::ScopedFood | ProjectionInputRole::ScopedNonFood => {
                (30_402, "addressable")
            }
            ProjectionInputRole::UnrelatedAddressable => (30_340, "addressable"),
            ProjectionInputRole::Causal => (5, "regular"),
        };
        assert_eq!(
            observed.event.kind, expected_kind,
            "{}: input role",
            case.id
        );
        assert_eq!(
            observed.expected_ingest.event_class, expected_event_class,
            "{}: input role event class",
            case.id
        );
        let raw_json = serde_json::to_string(&observed.event)
            .unwrap_or_else(|error| panic!("{}: serialize signed event: {error}", case.id));
        let ingest = RadrootsEventIngest::from_raw_json(raw_json, observed.observed_at_ms)
            .unwrap_or_else(|error| panic!("{}: verify signed event: {error}", case.id));
        let receipt = store
            .ingest_event(ingest)
            .await
            .unwrap_or_else(|error| panic!("{}: ingest signed event: {error}", case.id));
        assert_eq!(receipt.event_id, observed.event.id, "{}", case.id);
        let expected_sequence = i64::try_from(index + 1).expect("fixture sequence fits i64");
        assert!(
            receipt.persistence.is_inserted(),
            "{}: input persistence",
            case.id
        );
        assert_eq!(
            receipt.persistence.sequence(),
            Some(expected_sequence),
            "{}: input sequence",
            case.id
        );
        assert_eq!(
            receipt.admission_status.as_str(),
            observed.expected_ingest.admission_status,
            "{}: input admission",
            case.id
        );
        assert_eq!(
            receipt.admission_code.as_deref(),
            observed.expected_ingest.admission_code.0.as_deref(),
            "{}: input admission code",
            case.id
        );
        assert_eq!(
            receipt.contract_id.as_deref(),
            observed.expected_ingest.contract_id.0.as_deref(),
            "{}: input contract",
            case.id
        );
        assert_eq!(
            receipt.valid_stream_eligible, observed.expected_ingest.valid_stream_eligible,
            "{}: input stream eligibility",
            case.id
        );
        assert_eq!(
            raw_head_decision_code(&receipt.raw_head_decision),
            observed.expected_ingest.raw_head_decision,
            "{}: input raw-head decision",
            case.id
        );
    }
    let active_generation = store
        .source_generation()
        .await
        .unwrap_or_else(|error| panic!("{}: active source generation: {error}", case.id));

    assert_eq!(case.expected.coordinate.kind, 30_402, "{}", case.id);
    let public_key = PublicKey::from_hex(&case.expected.coordinate.pubkey)
        .unwrap_or_else(|error| panic!("{}: expected public key: {error}", case.id));
    let identifier = RadrootsFoodIdentifier::parse(&case.expected.coordinate.d_tag)
        .unwrap_or_else(|error| panic!("{}: expected identifier: {error}", case.id));
    let projection = store
        .food_availability_v1(&public_key, &identifier)
        .await
        .unwrap_or_else(|error| panic!("{}: load projection: {error}", case.id));

    match (&case.expected.projection.0, projection.as_ref()) {
        (Some(expected), Some(actual)) => assert_projection(&case.id, expected, actual),
        (None, None) => {}
        (expected, actual) => panic!(
            "{}: projection presence mismatch: expected={}, actual={}",
            case.id,
            expected.is_some(),
            actual.is_some()
        ),
    }

    let recent = store
        .recent_food_availability_v1(RadrootsFoodAvailabilityStatusFilterV1::Any, 16)
        .await
        .unwrap_or_else(|error| panic!("{}: recent projection query: {error}", case.id));
    assert_eq!(
        recent.len(),
        usize::from(case.expected.projection.0.is_some()),
        "{}: recent projection count",
        case.id
    );

    for expected in &case.expected.searches {
        let query = RadrootsFoodAvailabilitySearchQueryV1::parse(&expected.query)
            .unwrap_or_else(|error| panic!("{}: parse search query: {error}", case.id));
        let actual = store
            .search_food_availability_v1(&query, RadrootsFoodAvailabilityStatusFilterV1::Any, 16)
            .await
            .unwrap_or_else(|error| panic!("{}: search projection: {error}", case.id));
        let event_ids = actual
            .iter()
            .map(|projection| projection.event_id().as_str())
            .collect::<Vec<_>>();
        assert_eq!(event_ids, expected.event_ids, "{}: search", case.id);
    }

    let scope = RadrootsAddressableTransitionScopeV1::food_availability();
    assert_eq!(scope.kinds(), [30_402]);
    let page = store
        .addressable_transition_page_v1(&scope, None, 64)
        .await
        .unwrap_or_else(|error| panic!("{}: load transition feed: {error}", case.id));
    let expected_page = &case.expected.transition_page;
    assert_eq!(
        page.source_high_water(),
        expected_page.source_high_water,
        "{}",
        case.id
    );
    assert_eq!(page.has_more(), expected_page.has_more, "{}", case.id);
    assert_active_generation(
        &case.id,
        "cursor source generation",
        &expected_page.next_cursor.source_generation,
        page.next_cursor().source_generation(),
        active_generation,
    );
    assert_eq!(
        page.next_cursor().feed_version(),
        expected_page.next_cursor.feed_version,
        "{}: cursor feed version",
        case.id
    );
    assert_eq!(
        page.next_cursor().scope_fingerprint().to_hex(),
        expected_page.next_cursor.scope_fingerprint,
        "{}: cursor scope fingerprint",
        case.id
    );
    assert_eq!(
        page.next_cursor().last_transition_seq(),
        expected_page.next_cursor.last_transition_seq,
        "{}: cursor transition sequence",
        case.id
    );
    assert_eq!(
        page.transitions().len(),
        expected_page.transitions.len(),
        "{}: transition count",
        case.id
    );
    for (actual, expected) in page.transitions().iter().zip(&expected_page.transitions) {
        assert_eq!(
            actual.transition_seq(),
            expected.transition_seq,
            "{}",
            case.id
        );
        assert_active_generation(
            &case.id,
            "transition source generation",
            &expected.source_generation,
            actual.source_generation(),
            active_generation,
        );
        assert_eq!(actual.origin().as_str(), expected.origin, "{}", case.id);
        assert_eq!(
            actual.coordinate().kind(),
            expected.coordinate.kind,
            "{}",
            case.id
        );
        assert_eq!(
            actual.coordinate().pubkey().to_hex(),
            expected.coordinate.pubkey,
            "{}",
            case.id
        );
        assert_eq!(
            actual.coordinate().d_tag(),
            expected.coordinate.d_tag,
            "{}",
            case.id
        );
        assert_event_reference(&case.id, "raw head", &expected.raw_head, actual.raw_head());
        assert_eq!(
            actual.raw_head_created_at(),
            expected.raw_head_created_at,
            "{}: raw-head timestamp",
            case.id
        );
        assert_eq!(
            actual.admission_status().as_str(),
            expected.admission_status,
            "{}: admission status",
            case.id
        );
        assert_eq!(
            actual.admission_code(),
            expected.admission_code.0.as_deref(),
            "{}: admission code",
            case.id
        );
        assert_eq!(
            actual.contract_id(),
            expected.contract_id.0.as_deref(),
            "{}: contract id",
            case.id
        );
        assert_eq!(
            actual.visibility().as_str(),
            expected.visibility,
            "{}: transition visibility",
            case.id
        );
        assert_optional_suppression(
            &case.id,
            expected.suppression.0.as_ref(),
            actual.suppression(),
        );
        match (expected.cause_event.0.as_ref(), actual.cause_event()) {
            (Some(expected), Some(actual)) => {
                assert_event_reference(&case.id, "cause", &expected.event, actual.event());
                assert_eq!(actual.pubkey().to_hex(), expected.pubkey, "{}", case.id);
                assert_eq!(actual.created_at(), expected.created_at, "{}", case.id);
                assert_eq!(actual.kind(), expected.kind, "{}", case.id);
                assert_eq!(
                    actual.admission_status().as_str(),
                    expected.admission_status,
                    "{}",
                    case.id
                );
                assert_eq!(
                    actual.admission_code(),
                    expected.admission_code.0.as_deref(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    actual.contract_id(),
                    expected.contract_id.0.as_deref(),
                    "{}",
                    case.id
                );
            }
            (None, None) => {}
            (expected, actual) => panic!(
                "{}: cause presence mismatch: expected={}, actual={}",
                case.id,
                expected.is_some(),
                actual.is_some()
            ),
        }
        match (
            expected.canonical_visible_event.0.as_ref(),
            actual.visible_event(),
        ) {
            (Some(expected), Some(actual)) => {
                assert_canonical_visible_event(&case, expected, actual)
            }
            (None, None) => {}
            (expected, actual) => panic!(
                "{}: canonical visible-event presence mismatch: expected={}, actual={}",
                case.id,
                expected.is_some(),
                actual.is_some()
            ),
        }
        match (
            expected.retracted_event.0.as_ref(),
            actual.retracted_event(),
        ) {
            (Some(expected), Some(actual)) => {
                assert_event_reference(&case.id, "retracted event", expected, actual)
            }
            (None, None) => {}
            (expected, actual) => panic!(
                "{}: retracted-event presence mismatch: expected={}, actual={}",
                case.id,
                expected.is_some(),
                actual.is_some()
            ),
        }
        assert_eq!(
            actual.raw_head_decision().as_str(),
            expected.raw_head_decision,
            "{}: raw-head decision",
            case.id
        );
    }
    assert_eq!(
        page.next_cursor().last_transition_seq(),
        page.source_high_water(),
        "{}: cursor reaches captured high-water",
        case.id
    );

    for expected in &case.expected.event_visibility {
        let actual = store
            .current_event_visibility_v1(&expected.event.event_id)
            .await
            .unwrap_or_else(|error| panic!("{}: current visibility: {error}", case.id))
            .unwrap_or_else(|| panic!("{}: expected stored event visibility", case.id));
        assert_active_generation(
            &case.id,
            "current visibility source generation",
            &expected.source_generation,
            actual.source_generation(),
            active_generation,
        );
        assert_stored_event_matches_input(&case, &expected.event, actual.event());
        assert_eq!(
            actual.admission_status().as_str(),
            expected.admission_status,
            "{}: current admission status",
            case.id
        );
        assert_eq!(
            actual.decision().as_str(),
            expected.decision,
            "{}: current decision",
            case.id
        );
        assert_eq!(actual.is_raw_head(), expected.is_raw_head, "{}", case.id);
        assert_eq!(
            actual.raw_head_event_id().map(|event_id| event_id.as_str()),
            expected.raw_head_event_id.0.as_deref(),
            "{}: current raw head",
            case.id
        );
        assert_optional_suppression(
            &case.id,
            expected.suppression.0.as_ref(),
            actual.suppression(),
        );
    }

    for witness in &case.expected.historical_visibility_witnesses {
        let historical = page
            .transitions()
            .iter()
            .find(|transition| transition.transition_seq() == witness.transition_seq)
            .unwrap_or_else(|| panic!("{}: historical transition is absent", case.id));
        let historical_payload = historical
            .visible_event()
            .unwrap_or_else(|| panic!("{}: historical visible payload is absent", case.id));
        assert_eq!(
            historical_payload.event_id().as_str(),
            witness.event_id,
            "{}: historical visible payload identity",
            case.id
        );
        let expected_final = case
            .expected
            .event_visibility
            .iter()
            .find(|visibility| visibility.event.event_id == witness.event_id)
            .unwrap_or_else(|| panic!("{}: final visibility witness is absent", case.id));
        assert_eq!(
            expected_final.decision, witness.final_decision,
            "{}: historical/final witness contract",
            case.id
        );
        assert_ne!(
            witness.final_decision, "visible",
            "{}: historical payload must diverge from final visibility",
            case.id
        );
        let final_visibility = store
            .current_event_visibility_v1(&witness.event_id)
            .await
            .unwrap_or_else(|error| panic!("{}: final historical visibility: {error}", case.id))
            .unwrap_or_else(|| panic!("{}: final historical event is absent", case.id));
        assert_eq!(
            final_visibility.decision().as_str(),
            witness.final_decision,
            "{}: transition-time payload coexists with final visibility",
            case.id
        );
    }

    store
        .audit_food_availability_projection_v1()
        .await
        .unwrap_or_else(|error| panic!("{}: exhaustive projection audit: {error}", case.id));
}

fn assert_active_generation(
    case_id: &str,
    label: &str,
    expected: &str,
    actual: RadrootsEventStoreSourceGeneration,
    active: RadrootsEventStoreSourceGeneration,
) {
    assert_eq!(
        expected, SOURCE_GENERATION_ACTIVE_SENTINEL,
        "{case_id}: {label}"
    );
    assert_eq!(actual, active, "{case_id}: {label}");
}

fn assert_event_reference(
    case_id: &str,
    label: &str,
    expected: &ExpectedEventReference,
    actual: &RadrootsAddressableTransitionEventReferenceV1,
) {
    assert_eq!(
        actual.event_id().as_str(),
        expected.event_id,
        "{case_id}: {label}"
    );
    assert_eq!(actual.event_seq(), expected.event_seq, "{case_id}: {label}");
}

fn assert_optional_suppression(
    case_id: &str,
    expected: Option<&ExpectedSuppressionEvidence>,
    actual: Option<&RadrootsNip09SuppressionEvidenceV1>,
) {
    match (expected, actual) {
        (Some(expected), Some(actual)) => {
            assert_eq!(actual.outcome().code(), expected.outcome, "{case_id}");
            assert_eq!(actual.reason().code(), expected.reason, "{case_id}");
            assert_eq!(
                actual
                    .event_reference_request_id()
                    .map(|event_id| event_id.as_str()),
                expected.event_reference_request_id.0.as_deref(),
                "{case_id}"
            );
            assert_eq!(
                actual
                    .address_reference_request_id()
                    .map(|event_id| event_id.as_str()),
                expected.address_reference_request_id.0.as_deref(),
                "{case_id}"
            );
            assert_eq!(
                actual.address_reference_cutoff(),
                expected.address_reference_cutoff.0,
                "{case_id}"
            );
        }
        (None, None) => {}
        (expected, actual) => panic!(
            "{case_id}: suppression presence mismatch: expected={}, actual={}",
            expected.is_some(),
            actual.is_some()
        ),
    }
}

fn assert_canonical_visible_event(
    case: &ProjectionCase,
    expected: &ExpectedCanonicalVisibleEvent,
    actual: &RadrootsStoreProducedCanonicalEventV1,
) {
    let observed = case
        .events
        .iter()
        .find(|observed| observed.event.id == expected.event.event_id)
        .unwrap_or_else(|| panic!("{}: expected event is absent from input", case.id));
    assert_eq!(actual.event_id().as_str(), observed.event.id, "{}", case.id);
    assert_eq!(
        actual.pubkey().to_hex(),
        observed.event.pubkey,
        "{}",
        case.id
    );
    assert_eq!(
        actual.created_at(),
        observed.event.created_at,
        "{}",
        case.id
    );
    assert_eq!(actual.kind(), observed.event.kind, "{}", case.id);
    let decoded: SignedEvent = serde_json::from_str(actual.raw_json())
        .unwrap_or_else(|error| panic!("{}: decode canonical raw JSON: {error}", case.id));
    assert_eq!(
        decoded, observed.event,
        "{}: canonical signed payload",
        case.id
    );
    assert_eq!(
        sha256_hex(actual.raw_json().as_bytes()),
        expected.raw_json_sha256,
        "{}: canonical raw JSON digest",
        case.id
    );
    assert_eq!(
        observed.expected_ingest.admission_status, expected.admission_status,
        "{}",
        case.id
    );
    assert_eq!(
        observed.expected_ingest.contract_id.0.as_deref(),
        expected.contract_id.0.as_deref(),
        "{}",
        case.id
    );
    assert_eq!(
        observed.expected_ingest.event_class, expected.event_class,
        "{}",
        case.id
    );
    assert_eq!(
        observed.expected_ingest.valid_stream_eligible, expected.valid_stream_eligible,
        "{}",
        case.id
    );
}

fn assert_stored_event_matches_input(
    case: &ProjectionCase,
    expected: &ExpectedEventReference,
    actual: &RadrootsStoredRawEvent,
) {
    let observed = case
        .events
        .iter()
        .find(|observed| observed.event.id == expected.event_id)
        .unwrap_or_else(|| panic!("{}: expected event is absent from input", case.id));
    let raw_json = serde_json::to_string(&observed.event)
        .unwrap_or_else(|error| panic!("{}: serialize expected event: {error}", case.id));
    let tags_json = serde_json::to_string(&observed.event.tags)
        .unwrap_or_else(|error| panic!("{}: serialize expected tags: {error}", case.id));
    assert_eq!(actual.seq, expected.event_seq, "{}", case.id);
    assert_eq!(actual.event_id, observed.event.id, "{}", case.id);
    assert_eq!(actual.pubkey, observed.event.pubkey, "{}", case.id);
    assert_eq!(actual.created_at, observed.event.created_at, "{}", case.id);
    assert_eq!(actual.kind, observed.event.kind, "{}", case.id);
    assert_eq!(actual.tags_json, tags_json, "{}", case.id);
    assert_eq!(actual.content, observed.event.content, "{}", case.id);
    assert_eq!(actual.sig, observed.event.sig, "{}", case.id);
    assert_eq!(actual.raw_json, raw_json, "{}", case.id);
    assert_eq!(
        actual.admission_status.as_str(),
        observed.expected_ingest.admission_status,
        "{}",
        case.id
    );
    assert_eq!(
        actual.contract_id.as_deref(),
        observed.expected_ingest.contract_id.0.as_deref(),
        "{}",
        case.id
    );
    assert_eq!(
        actual.event_class.as_str(),
        observed.expected_ingest.event_class,
        "{}",
        case.id
    );
    assert_eq!(
        actual.valid_stream_eligible, observed.expected_ingest.valid_stream_eligible,
        "{}",
        case.id
    );
    assert_eq!(
        actual.inserted_at_ms, observed.observed_at_ms,
        "{}",
        case.id
    );
    assert_eq!(actual.updated_at_ms, observed.observed_at_ms, "{}", case.id);
}

fn assert_projection(
    case_id: &str,
    expected: &ExpectedProjection,
    actual: &RadrootsStoredFoodAvailabilityV1,
) {
    assert_eq!(actual.event_id().as_str(), expected.event_id, "{case_id}");
    assert_eq!(actual.content().as_str(), expected.content, "{case_id}");
    assert_eq!(actual.title().as_str(), expected.title, "{case_id}");
    assert_eq!(actual.summary().as_str(), expected.summary, "{case_id}");
    assert_eq!(
        actual.published_at().as_u64(),
        expected.published_at,
        "{case_id}"
    );
    assert_eq!(actual.location().as_str(), expected.location, "{case_id}");
    assert_eq!(actual.price().amount(), expected.price_amount, "{case_id}");
    assert_eq!(
        actual.price().currency().as_str(),
        expected.price_currency,
        "{case_id}"
    );
    assert_eq!(
        actual.price().unit().as_str(),
        expected.price_unit,
        "{case_id}"
    );
    assert_eq!(
        actual.quantity().map(|quantity| quantity.amount()),
        expected.quantity_amount.0.as_deref(),
        "{case_id}"
    );
    assert_eq!(
        actual.quantity().map(|quantity| quantity.unit().as_str()),
        expected.quantity_unit.0.as_deref(),
        "{case_id}"
    );
    assert_eq!(actual.status().as_str(), expected.status, "{case_id}");
    assert_eq!(
        actual
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
        expected.diagnostics,
        "{case_id}: projection diagnostics"
    );
    assert_eq!(actual.images().len(), expected.images.len(), "{case_id}");

    for (actual, expected) in actual.images().iter().zip(&expected.images) {
        assert_eq!(actual.url(), expected.url.0.as_deref(), "{case_id}");
        assert_eq!(
            actual.dimensions().map(|dimensions| dimensions.width()),
            expected.width.0,
            "{case_id}"
        );
        assert_eq!(
            actual.dimensions().map(|dimensions| dimensions.height()),
            expected.height.0,
            "{case_id}"
        );
        let expected_blossom_sha256 = expected.blossom_sha256.0.as_deref().map(|value| {
            Sha256::from_hex(value)
                .unwrap_or_else(|error| panic!("{case_id}: expected Blossom digest: {error}"))
        });
        assert_eq!(
            actual.blossom_sha256(),
            expected_blossom_sha256,
            "{case_id}"
        );
        assert_eq!(
            actual
                .diagnostics()
                .iter()
                .map(|diagnostic| diagnostic.code())
                .collect::<Vec<_>>(),
            expected.diagnostics,
            "{case_id}: image diagnostics"
        );
        assert_eq!(actual.qualifies(), expected.qualifies, "{case_id}");
    }
}

#[test]
fn food_availability_projection_v1_vector_requires_complete_nullable_fields() {
    let vector: serde_json::Value =
        serde_json::from_slice(RESULT_VECTOR_BYTES).expect("result-vector JSON");
    let mut missing = vector.clone();
    missing["cases"][0]["expected"]["projection"]
        .as_object_mut()
        .expect("expected projection")
        .remove("quantity_amount");
    assert!(
        serde_json::from_value::<FoodAvailabilityProjectionVector>(missing).is_err(),
        "nullable result fields must still be present"
    );
}

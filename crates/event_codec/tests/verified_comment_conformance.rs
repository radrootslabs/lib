#![cfg(feature = "json")]

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use radroots_event::{
    envelope::EventEnvelope,
    envelope::EventEnvelopeLimits,
    envelope::EventEnvelopeParts,
    post::comment::{
        AuthoredNip22Comment, Nip22AddressRootReference, Nip22CommentError,
        Nip22CommentParentReference, Nip22EventRootReference,
    },
};
use radroots_event_codec::{
    comment::{
        admission::verify_and_admit_nip22_comment_event,
        authored::authored_nip22_comment_to_wire_parts,
        inbound::{
            RadrootsInboundNip22CommentPosition, RadrootsInboundNip22CommentProjection,
            RadrootsInboundNip22CommentRoot, RadrootsInboundNip22Participant,
            project_verified_nip22_comment_event,
        },
    },
    verification::verify_nip01_event,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const PACKAGED_VECTORS: &str = include_str!("fixtures/comment_verified_profile.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/comment/verified_profile.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";

#[derive(Debug, Deserialize)]
struct Suite {
    suite: String,
    contract_version: String,
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
struct Vector {
    id: String,
    kind: String,
    input: Value,
    expected: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

#[test]
fn comment_operation_vectors_execute_the_declared_public_functions() {
    let vectors = conformance_vectors();
    assert!(
        !vectors.contains("mutation")
            && !vectors.contains("\"base\"")
            && !vectors.contains("secret_key"),
        "frozen Comment corpus must not contain runtime generation descriptors"
    );
    let suite: Suite = serde_json::from_str(&vectors).expect("Comment vectors must parse");
    assert_eq!(suite.suite, "nip22_comment_profile");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_vector_inventory(&suite.vectors);

    for vector in &suite.vectors {
        execute(vector);
    }
}

fn conformance_vectors() -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_VECTOR_PATH);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                PACKAGED_VECTORS,
                "packaged Comment vectors must match {}",
                workspace_path.display()
            );
            Cow::Owned(canonical)
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && !Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(WORKSPACE_CONTRACT_MARKER_PATH)
                    .is_file() =>
        {
            Cow::Borrowed(PACKAGED_VECTORS)
        }
        Err(error) => panic!("failed to read {}: {error}", workspace_path.display()),
    }
}

fn assert_vector_inventory(vectors: &[Vector]) {
    let mut ids = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut authored_errors = BTreeSet::new();
    let mut projection_errors = BTreeSet::new();
    let mut diagnostic_codes = BTreeSet::new();
    for vector in vectors {
        assert!(
            ids.insert(vector.id.as_str(), vector.kind.as_str())
                .is_none(),
            "duplicate Comment vector id {}",
            vector.id
        );
        *kinds.entry(vector.kind.as_str()).or_insert(0usize) += 1;
        if vector.kind == "social.comment.build_authored_draft.invalid" {
            authored_errors.insert(expected_str(vector, "error"));
        }
        if vector.kind == "social.comment.project_verified_event.invalid" {
            projection_errors.insert(expected_str(vector, "error"));
        }
        if vector.kind == "social.comment.project_verified_event.valid" {
            for diagnostic in vector.expected["diagnostics"]
                .as_array()
                .unwrap_or_else(|| panic!("{} expected.diagnostics must be an array", vector.id))
            {
                diagnostic_codes.insert(
                    diagnostic["code"].as_str().unwrap_or_else(|| {
                        panic!("{} diagnostic.code must be a string", vector.id)
                    }),
                );
            }
        }

        if vector.kind.contains("project_verified_event")
            || vector.kind.contains("verify_and_admit_event")
        {
            let input = vector
                .input
                .as_object()
                .unwrap_or_else(|| panic!("{} input must be an object", vector.id));
            assert_eq!(
                input.keys().map(String::as_str).collect::<Vec<_>>(),
                ["event_json"],
                "{} must contain only fixed event_json",
                vector.id
            );
        } else {
            let input = vector
                .input
                .as_object()
                .unwrap_or_else(|| panic!("{} input must be an object", vector.id));
            assert_eq!(
                input.keys().map(String::as_str).collect::<Vec<_>>(),
                ["content", "position", "root"],
                "{} authored input fields drifted",
                vector.id
            );
        }
    }
    assert_eq!(vectors.len(), 114);
    assert_eq!(
        kinds,
        BTreeMap::from([
            ("social.comment.build_authored_draft.invalid", 17),
            ("social.comment.build_authored_draft.valid", 14),
            ("social.comment.project_verified_event.invalid", 45),
            ("social.comment.project_verified_event.valid", 30),
            ("social.comment.verify_and_admit_event.invalid", 4),
            ("social.comment.verify_and_admit_event.valid", 4),
        ])
    );
    assert_eq!(
        authored_errors,
        BTreeSet::from([
            "comment_content_missing",
            "comment_content_too_large",
            "comment_event_wire_too_large",
            "comment_parent_author_invalid",
            "comment_parent_event_id_invalid",
            "comment_parent_reference_mismatch",
            "comment_relay_invalid",
            "comment_revision_event_id_invalid",
            "comment_root_author_invalid",
            "comment_root_coordinate_invalid",
            "comment_root_event_id_invalid",
            "comment_root_kind_unsupported",
            "comment_tag_element_too_large",
        ])
    );
    assert_eq!(
        projection_errors,
        BTreeSet::from([
            "comment_content_missing",
            "comment_content_too_large",
            "comment_event_wire_too_large",
            "comment_parent_author_ambiguous",
            "comment_parent_author_mismatch",
            "comment_parent_author_missing",
            "comment_parent_cardinality",
            "comment_parent_coordinate_invalid",
            "comment_parent_event_id_invalid",
            "comment_parent_form_unsupported",
            "comment_parent_kind_cardinality",
            "comment_parent_kind_invalid",
            "comment_parent_reference_mismatch",
            "comment_parent_reference_shape",
            "comment_revision_event_id_invalid",
            "comment_revision_missing",
            "comment_revision_shape",
            "comment_root_author_cardinality",
            "comment_root_author_invalid",
            "comment_root_author_mismatch",
            "comment_root_cardinality",
            "comment_root_coordinate_invalid",
            "comment_root_event_id_invalid",
            "comment_root_form_unsupported",
            "comment_root_kind_cardinality",
            "comment_root_kind_mismatch",
            "comment_root_kind_unsupported",
            "comment_root_reference_shape",
            "comment_tag_bytes_exceeded",
            "comment_tag_count_exceeded",
            "comment_tag_element_count_exceeded",
            "comment_tag_element_too_large",
            "unsupported_kind",
        ])
    );
    assert_eq!(
        diagnostic_codes,
        BTreeSet::from([
            "comment_parent_author_duplicate_ignored",
            "comment_parent_author_hint_ignored",
            "comment_parent_author_invalid_ignored",
            "comment_parent_author_relay_ignored",
            "comment_parent_author_shape_ignored",
            "comment_parent_relay_ignored",
            "comment_revision_relay_ignored",
            "comment_root_author_hint_ignored",
            "comment_root_author_relay_ignored",
            "comment_root_relay_ignored",
        ])
    );
}

fn execute(vector: &Vector) {
    match vector.kind.as_str() {
        "social.comment.build_authored_draft.valid" => {
            let comment = authored_comment(vector)
                .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
            let first = authored_nip22_comment_to_wire_parts(&comment);
            let second = authored_nip22_comment_to_wire_parts(&comment);
            assert_eq!(first, second, "{} repeat encoding drifted", vector.id);
            assert_eq!(
                serde_json::to_value(first).expect("authored Comment result must serialize"),
                vector.expected,
                "{}",
                vector.id
            );
        }
        "social.comment.build_authored_draft.invalid" => {
            let error = authored_comment(vector).expect_err("invalid authored Comment must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        "social.comment.project_verified_event.valid" => {
            let verified = verify_nip01_event(fixture_envelope(vector))
                .unwrap_or_else(|error| panic!("{} verification failed: {error}", vector.id));
            let projection = project_verified_nip22_comment_event(&verified)
                .unwrap_or_else(|error| panic!("{} projection failed: {error}", vector.id));
            assert_eq!(
                projection_snapshot(&projection),
                vector.expected,
                "{}",
                vector.id
            );
        }
        "social.comment.project_verified_event.invalid" => {
            let verified = verify_nip01_event(fixture_envelope(vector))
                .unwrap_or_else(|error| panic!("{} verification failed: {error}", vector.id));
            let error = project_verified_nip22_comment_event(&verified)
                .expect_err("invalid verified Comment projection must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        "social.comment.verify_and_admit_event.valid" => {
            let envelope = fixture_envelope(vector);
            let expected_event = envelope.clone();
            let admitted = verify_and_admit_nip22_comment_event(envelope)
                .unwrap_or_else(|error| panic!("{} admission failed: {error}", vector.id));
            assert_eq!(admitted.event(), &expected_event, "{}", vector.id);
            assert_eq!(admitted.contract().id, "radroots.social.comment.v1");
            assert_eq!(
                projection_snapshot(admitted.projection()),
                vector.expected,
                "{}",
                vector.id
            );
            let (verified, projection) = admitted.into_parts();
            assert_eq!(verified.event(), &expected_event, "{}", vector.id);
            assert_eq!(projection.contract_id(), "radroots.social.comment.v1");
        }
        "social.comment.verify_and_admit_event.invalid" => {
            let error = verify_and_admit_nip22_comment_event(fixture_envelope(vector))
                .expect_err("invalid signed Comment must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        other => panic!("{} has unknown vector kind {other}", vector.id),
    }
}

fn authored_comment(vector: &Vector) -> Result<AuthoredNip22Comment, Nip22CommentError> {
    let content = input_str(vector, "content");
    let root = input_object(vector, "root");
    let position = input_object(vector, "position");
    let position_type = object_str(position, "type", &vector.id);

    match object_str(root, "type", &vector.id) {
        "event" => {
            let root_reference = Nip22EventRootReference::parse(
                object_str(root, "event_id", &vector.id),
                object_str(root, "author", &vector.id),
                object_u32(root, "kind", &vector.id),
                object_optional_str(root, "relay", &vector.id),
            )?;
            match position_type {
                "top_event" => AuthoredNip22Comment::top_level_event(content, root_reference),
                "nested" => AuthoredNip22Comment::nested(
                    content,
                    root_reference,
                    nested_parent(position, &vector.id)?,
                ),
                other => panic!("{} event root has incompatible position {other}", vector.id),
            }
        }
        "address" => {
            let root_reference = Nip22AddressRootReference::parse(
                object_str(root, "coordinate", &vector.id),
                object_optional_str(root, "relay", &vector.id),
            )?;
            assert_eq!(
                root_reference.author().to_hex(),
                object_str(root, "author", &vector.id),
                "{} declared address author drifted",
                vector.id
            );
            assert_eq!(
                root_reference.kind().as_u32(),
                object_u32(root, "kind", &vector.id),
                "{} declared address kind drifted",
                vector.id
            );
            match position_type {
                "top_address" => AuthoredNip22Comment::parse_top_level_address(
                    content,
                    root_reference,
                    object_str(position, "current_revision", &vector.id),
                ),
                "nested" => AuthoredNip22Comment::nested(
                    content,
                    root_reference,
                    nested_parent(position, &vector.id)?,
                ),
                other => panic!(
                    "{} address root has incompatible position {other}",
                    vector.id
                ),
            }
        }
        other => panic!("{} has unknown root type {other}", vector.id),
    }
}

fn nested_parent(
    position: &serde_json::Map<String, Value>,
    vector_id: &str,
) -> Result<Nip22CommentParentReference, Nip22CommentError> {
    let parent = position["parent"]
        .as_object()
        .unwrap_or_else(|| panic!("{vector_id} position.parent must be an object"));
    Nip22CommentParentReference::parse(
        object_str(parent, "event_id", vector_id),
        object_str(parent, "author", vector_id),
        object_optional_str(parent, "relay", vector_id),
    )
}

fn fixture_envelope(vector: &Vector) -> EventEnvelope {
    let event_json = input_str(vector, "event_json");
    let raw: RawEvent = serde_json::from_str(event_json)
        .unwrap_or_else(|error| panic!("{} event_json failed to parse: {error}", vector.id));
    assert_eq!(
        serde_json::to_string(&raw).expect("raw event must serialize"),
        event_json,
        "{} event_json must be compact and canonical",
        vector.id
    );

    let mut limits = EventEnvelopeLimits::default();
    limits.max_content_bytes = limits.max_content_bytes.max(raw.content.len());
    limits.max_tag_count = limits.max_tag_count.max(raw.tags.len());
    limits.max_total_tag_elements = limits
        .max_total_tag_elements
        .max(raw.tags.iter().map(Vec::len).sum());
    limits.max_tag_element_bytes = limits.max_tag_element_bytes.max(
        raw.tags
            .iter()
            .flat_map(|tag| tag.iter())
            .map(String::len)
            .max()
            .unwrap_or_default(),
    );
    limits.max_total_tag_bytes = limits.max_total_tag_bytes.max(
        raw.tags
            .iter()
            .flat_map(|tag| tag.iter())
            .map(String::len)
            .sum(),
    );
    EventEnvelope::new_with_limits(
        EventEnvelopeParts {
            id: raw.id,
            author: raw.pubkey,
            created_at: raw.created_at,
            kind: raw.kind,
            tags: raw.tags,
            content: raw.content,
            sig: raw.sig,
        },
        limits,
    )
    .unwrap_or_else(|error| panic!("{} envelope failed: {error}", vector.id))
}

fn projection_snapshot(projection: &RadrootsInboundNip22CommentProjection) -> Value {
    let root = match projection.root() {
        RadrootsInboundNip22CommentRoot::Event(root) => json!({
            "type": "event",
            "tag_index": root.tag_index(),
            "event_id": root.event_id().to_hex(),
            "kind_tag_index": root.kind_tag_index(),
            "kind_raw_tag": root.kind_raw_tag(),
            "kind": root.kind().as_u32(),
            "relay": root.relay().map(|relay| relay.as_str()),
            "author_hint": root.author_hint().map(|author| author.to_hex()),
            "author": participant_snapshot(root.author()),
            "raw_tag": root.raw_tag(),
        }),
        RadrootsInboundNip22CommentRoot::Address(root) => json!({
            "type": "address",
            "tag_index": root.tag_index(),
            "coordinate": root.coordinate().as_str(),
            "kind_tag_index": root.kind_tag_index(),
            "kind_raw_tag": root.kind_raw_tag(),
            "kind": root.kind().as_u32(),
            "relay": root.relay().map(|relay| relay.as_str()),
            "author": participant_snapshot(root.author()),
            "raw_tag": root.raw_tag(),
        }),
    };
    let position = match projection.position() {
        RadrootsInboundNip22CommentPosition::TopLevelEvent { reference } => json!({
            "type": "top_event",
            "reference": {
                "tag_index": reference.tag_index(),
                "event_id": reference.event_id().to_hex(),
                "kind_tag_index": reference.kind_tag_index(),
                "kind_raw_tag": reference.kind_raw_tag(),
                "kind": reference.kind(),
                "relay": reference.relay().map(|relay| relay.as_str()),
                "author_hint": reference.author_hint().map(|author| author.to_hex()),
                "author": participant_snapshot(reference.author()),
                "raw_tag": reference.raw_tag(),
            },
        }),
        RadrootsInboundNip22CommentPosition::TopLevelAddress {
            reference,
            current_revision,
        } => json!({
            "type": "top_address",
            "reference": {
                "tag_index": reference.tag_index(),
                "coordinate": reference.coordinate().as_str(),
                "kind_tag_index": reference.kind_tag_index(),
                "kind_raw_tag": reference.kind_raw_tag(),
                "kind": reference.kind(),
                "relay": reference.relay().map(|relay| relay.as_str()),
                "author": participant_snapshot(reference.author()),
                "raw_tag": reference.raw_tag(),
            },
            "current_revision": {
                "tag_index": current_revision.tag_index(),
                "event_id": current_revision.event_id().to_hex(),
                "relay": current_revision.relay().map(|relay| relay.as_str()),
                "raw_tag": current_revision.raw_tag(),
            },
        }),
        RadrootsInboundNip22CommentPosition::Nested { parent } => json!({
            "type": "nested",
            "parent": {
                "tag_index": parent.tag_index(),
                "event_id": parent.event_id().to_hex(),
                "kind_tag_index": parent.kind_tag_index(),
                "kind_raw_tag": parent.kind_raw_tag(),
                "kind": parent.kind(),
                "relay": parent.relay().map(|relay| relay.as_str()),
                "author_hint": parent.author_hint().map(|author| author.to_hex()),
                "author": participant_snapshot(parent.author()),
                "raw_tag": parent.raw_tag(),
            },
        }),
    };
    json!({
        "contract_id": projection.contract_id(),
        "direct": projection.is_direct(),
        "root": root,
        "position": position,
        "mentions": projection
            .mentions()
            .iter()
            .map(participant_snapshot)
            .collect::<Vec<_>>(),
        "diagnostics": projection
            .diagnostics()
            .iter()
            .map(|diagnostic| json!({
                "code": diagnostic.code(),
                "tag_index": diagnostic.tag_index(),
                "raw_tag": diagnostic.raw_tag(),
            }))
            .collect::<Vec<_>>(),
        "raw_tags": projection.raw_tags(),
    })
}

fn participant_snapshot(participant: &RadrootsInboundNip22Participant) -> Value {
    json!({
        "tag_index": participant.tag_index(),
        "pubkey": participant.pubkey().to_hex(),
        "relay": participant.relay().map(|relay| relay.as_str()),
        "raw_tag": participant.raw_tag(),
    })
}

fn input_object<'a>(vector: &'a Vector, field: &str) -> &'a serde_json::Map<String, Value> {
    vector.input[field]
        .as_object()
        .unwrap_or_else(|| panic!("{} input.{field} must be an object", vector.id))
}

fn input_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.input[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} input.{field} must be a string", vector.id))
}

fn object_str<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
    vector_id: &str,
) -> &'a str {
    object[field]
        .as_str()
        .unwrap_or_else(|| panic!("{vector_id} {field} must be a string"))
}

fn object_optional_str<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
    vector_id: &str,
) -> Option<&'a str> {
    match object.get(field) {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_str()
                .unwrap_or_else(|| panic!("{vector_id} {field} must be a string or null")),
        ),
    }
}

fn object_u32(object: &serde_json::Map<String, Value>, field: &str, vector_id: &str) -> u32 {
    u32::try_from(
        object[field]
            .as_u64()
            .unwrap_or_else(|| panic!("{vector_id} {field} must be an integer")),
    )
    .unwrap_or_else(|_| panic!("{vector_id} {field} must fit u32"))
}

fn expected_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.expected[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} expected.{field} must be a string", vector.id))
}

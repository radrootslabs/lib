#![cfg(all(feature = "serde_json", feature = "nostr"))]

use std::{borrow::Cow, collections::BTreeMap, fs, path::Path};

use radroots_blossom::{
    RadrootsBlossomBlobDescriptor, RadrootsBlossomBlobUrl, RadrootsBlossomMediaType,
    RadrootsBlossomSha256,
};
use radroots_event::{
    RadrootsAuthoredImage, RadrootsEventEnvelope, RadrootsNip01EventWire,
    contract::identify_event_contract,
    post::{
        RadrootsAuthoredAsk, RadrootsAuthoredPhotoUpdate, RadrootsAuthoredPostError,
        RadrootsAuthoredPostImage, RadrootsAuthoredUpdate, RadrootsPostImageDimensions,
    },
    reply::{RadrootsAuthoredNip10Reply, RadrootsNip10ReplyError, RadrootsNip10ReplyReference},
    wire::RadrootsNip01EventWireParts,
};
use radroots_event_codec::post::{
    admission::{RadrootsPostAdmissionOutcome, verify_and_admit_post_event},
    authored::{
        authored_ask_to_wire_parts, authored_photo_update_to_wire_parts,
        authored_update_to_wire_parts,
    },
    inbound::{RadrootsInboundPostProjection, RadrootsPostClassification, RadrootsPostDiagnostic},
};
use radroots_event_codec::reply::{
    admission::verify_and_admit_nip10_reply_event,
    authored::authored_nip10_reply_to_wire_parts,
    inbound::{
        RadrootsInboundNip10EventReference, RadrootsInboundNip10Participant,
        RadrootsInboundNip10ReplyProjection, RadrootsNip10ReplyDiagnostic, RadrootsNip10ReplyStyle,
        project_verified_nip10_reply_event,
    },
};
use radroots_event_codec::{
    post::inbound::project_verified_post_event, verification::verify_nip01_event,
};
use serde::Deserialize;
use serde_json::{Value, json};

const PACKAGED_VECTORS: &str = include_str!("fixtures/post_verified_profiles.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/post/verified_profiles.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";
const EXPECTED_VECTOR_CASES: [(&str, &str); 64] = [
    (
        "admit_signed_duplicate_normalized_ask_marker",
        "social.post.verify_and_admit_event.invalid",
    ),
    (
        "admit_signed_empty_e_thread_candidate",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_empty_e_value_thread_candidate",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_invalid_signature",
        "social.post.verify_and_admit_event.invalid",
    ),
    (
        "admit_signed_kind_20_is_not_photo_update",
        "social.post.verify_and_admit_event.invalid",
    ),
    (
        "admit_signed_nip10_invalid_signature",
        "social.reply.verify_and_admit_event.invalid",
    ),
    (
        "admit_signed_nip10_marked_direct",
        "social.reply.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_nip10_positional_direct",
        "social.reply.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_normalized_ask_precedes_malformed_media",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_structural_photo",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_thread_candidate_precedes_ask_and_media",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_update",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "authored_ask_blank",
        "social.ask.build_authored_draft.invalid",
    ),
    ("authored_ask_wire", "social.ask.build_authored_draft.valid"),
    (
        "authored_nip10_ambiguous_parent",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_canonical_ipv6_relay",
        "social.reply.build_authored_draft.valid",
    ),
    (
        "authored_nip10_direct_wire",
        "social.reply.build_authored_draft.valid",
    ),
    (
        "authored_nip10_invalid_relay_bad_percent_host",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_empty_port",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_ipv4_overflow",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_ipvfuture",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_percent_host",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_port_overflow",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_zero_port",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_event_id",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_nested_wire",
        "social.reply.build_authored_draft.valid",
    ),
    (
        "authored_photo_update_mime_underscore",
        "social.photo_update.build_authored_draft.invalid",
    ),
    (
        "authored_photo_update_wire",
        "social.photo_update.build_authored_draft.valid",
    ),
    (
        "authored_update_blank",
        "social.update.build_authored_draft.invalid",
    ),
    (
        "authored_update_wire",
        "social.update.build_authored_draft.valid",
    ),
    (
        "project_signed_duplicate_normalized_ask_marker",
        "social.post.project_verified_event.invalid",
    ),
    (
        "project_signed_duplicate_singleton_is_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_empty_inbound_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_kind_20_is_not_photo_update",
        "social.post.project_verified_event.invalid",
    ),
    (
        "project_signed_malformed_ask_marker_is_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_malformed_imeta_is_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_mixed_imeta_is_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_ambiguous_same_reference",
        "social.reply.project_verified_event.invalid",
    ),
    (
        "project_signed_nip10_author_hint_mismatch",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_blank_content",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_invalid_event_id",
        "social.reply.project_verified_event.invalid",
    ),
    (
        "project_signed_nip10_invalid_relay",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_canonical_relay_authorities",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_malformed_relay_authorities",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_invalid_author_hint_tolerated",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_invalid_participant_tolerated",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_lone_reply_marker",
        "social.reply.project_verified_event.invalid",
    ),
    (
        "project_signed_nip10_marked_direct",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_marked_with_citation",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_marked_with_malformed_citation",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_marked_nested_reordered",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_missing_author",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_positional_direct",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_positional_direct_with_author_hint",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_positional_many",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_positional_many_with_author_hints",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_positional_malformed_middle_citation",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_precedes_ask_and_media",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_unknown_marker",
        "social.reply.project_verified_event.invalid",
    ),
    (
        "project_signed_normalized_ask_precedes_malformed_media",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_photo_preserves_fallbacks_and_unknown_fields",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_structural_photo",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_thread_candidate_precedes_ask_and_media",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_update",
        "social.post.project_verified_event.valid",
    ),
];

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

#[test]
fn post_operation_vectors_execute_the_declared_public_functions() {
    let vectors = conformance_vectors();
    let suite: Suite = serde_json::from_str(&vectors).expect("verified post vectors must parse");
    assert_eq!(suite.suite, "post_profiles");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_eq!(vector_cases(&suite.vectors), expected_vector_cases());

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
                "packaged verified post vectors must match {}",
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

fn execute(vector: &Vector) {
    match vector.kind.as_str() {
        "social.update.build_authored_draft.valid" => {
            let update = RadrootsAuthoredUpdate::new(input_str(vector, "content"))
                .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
            let first = authored_update_to_wire_parts(&update);
            let second = authored_update_to_wire_parts(&update);
            assert_eq!(first, second, "{} repeat encoding drifted", vector.id);
            assert_eq!(wire_parts_value(&first), vector.expected, "{}", vector.id);
        }
        "social.update.build_authored_draft.invalid" => {
            let error = RadrootsAuthoredUpdate::new(input_str(vector, "content"))
                .expect_err("invalid authored Update must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        "social.photo_update.build_authored_draft.valid" => {
            let images = authored_images(vector)
                .unwrap_or_else(|error| panic!("{} image construction failed: {error}", vector.id));
            let photo = RadrootsAuthoredPhotoUpdate::new(input_str(vector, "content"), images)
                .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
            let first = authored_photo_update_to_wire_parts(&photo);
            let second = authored_photo_update_to_wire_parts(&photo);
            assert_eq!(first, second, "{} repeat encoding drifted", vector.id);
            assert_eq!(wire_parts_value(&first), vector.expected, "{}", vector.id);
        }
        "social.photo_update.build_authored_draft.invalid" => {
            let error = authored_images(vector).expect_err("invalid authored image must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        "social.ask.build_authored_draft.valid" => {
            let images = authored_images(vector)
                .unwrap_or_else(|error| panic!("{} image construction failed: {error}", vector.id));
            let ask = RadrootsAuthoredAsk::new(input_str(vector, "content"), images)
                .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
            let first = authored_ask_to_wire_parts(&ask);
            let second = authored_ask_to_wire_parts(&ask);
            assert_eq!(first, second, "{} repeat encoding drifted", vector.id);
            assert_eq!(wire_parts_value(&first), vector.expected, "{}", vector.id);
        }
        "social.ask.build_authored_draft.invalid" => {
            let error = RadrootsAuthoredAsk::new(input_str(vector, "content"), Vec::new())
                .expect_err("invalid authored Ask must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        "social.reply.build_authored_draft.valid" => {
            let reply = authored_reply(vector)
                .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
            let first = authored_nip10_reply_to_wire_parts(&reply);
            let second = authored_nip10_reply_to_wire_parts(&reply);
            assert_eq!(first, second, "{} repeat encoding drifted", vector.id);
            assert_eq!(wire_parts_value(&first), vector.expected, "{}", vector.id);
        }
        "social.reply.build_authored_draft.invalid" => {
            let error = authored_reply(vector).expect_err("invalid authored Reply must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        "social.reply.project_verified_event.valid" => {
            let envelope = canonical_envelope(input_str(vector, "event_json"));
            let verified = verify_nip01_event(envelope)
                .unwrap_or_else(|error| panic!("{} verification failed: {error}", vector.id));
            let projection = project_verified_nip10_reply_event(&verified)
                .unwrap_or_else(|error| panic!("{} projection failed: {error}", vector.id));
            assert_eq!(
                reply_projection_value(&projection),
                vector.expected,
                "{}",
                vector.id
            );
        }
        "social.reply.project_verified_event.invalid" => {
            let envelope = canonical_envelope(input_str(vector, "event_json"));
            let verified = verify_nip01_event(envelope)
                .unwrap_or_else(|error| panic!("{} verification failed: {error}", vector.id));
            let error = project_verified_nip10_reply_event(&verified)
                .expect_err("invalid verified Reply projection must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        "social.reply.verify_and_admit_event.valid" => {
            let envelope = canonical_envelope(input_str(vector, "event_json"));
            let expected_envelope = envelope.clone();
            let admitted = verify_and_admit_nip10_reply_event(envelope)
                .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
            assert_eq!(admitted.event(), &expected_envelope, "{}", vector.id);
            let actual = json!({
                "contract_id": admitted.contract().id,
                "style": reply_style_label(admitted.projection().style()),
                "direct": admitted.projection().is_direct(),
            });
            let (verified, projection) = admitted.into_parts();
            assert_eq!(verified.event(), &expected_envelope, "{}", vector.id);
            assert_eq!(projection.contract_id(), "radroots.social.reply.v1");
            assert_eq!(actual, vector.expected, "{}", vector.id);
        }
        "social.reply.verify_and_admit_event.invalid" => {
            let envelope = canonical_envelope(input_str(vector, "event_json"));
            let error = verify_and_admit_nip10_reply_event(envelope)
                .expect_err("invalid signed Reply vector must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        "social.post.project_verified_event.valid" => {
            let envelope = canonical_envelope(input_str(vector, "event_json"));
            let verified = verify_nip01_event(envelope)
                .unwrap_or_else(|error| panic!("{} verification failed: {error}", vector.id));
            let projection = project_verified_post_event(&verified)
                .unwrap_or_else(|error| panic!("{} projection failed: {error}", vector.id));
            assert_eq!(
                projection_value(&projection),
                vector.expected,
                "{}",
                vector.id
            );
        }
        "social.post.project_verified_event.invalid" => {
            let envelope = canonical_envelope(input_str(vector, "event_json"));
            let verified = verify_nip01_event(envelope)
                .unwrap_or_else(|error| panic!("{} verification failed: {error}", vector.id));
            let error = project_verified_post_event(&verified)
                .expect_err("invalid verified projection must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        "social.post.verify_and_admit_event.valid" => {
            let envelope = canonical_envelope(input_str(vector, "event_json"));
            let expected_envelope = envelope.clone();
            let generic = identify_event_contract(
                envelope.kind_u32(),
                &envelope.tags_as_vec(),
                envelope.content(),
            )
            .expect("unsigned post identification remains available");
            assert_eq!(generic.id, "radroots.social.post.v1", "{}", vector.id);

            let outcome = verify_and_admit_post_event(envelope)
                .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
            let actual = match outcome {
                RadrootsPostAdmissionOutcome::Root(admitted) => {
                    assert_eq!(admitted.event(), &expected_envelope, "{}", vector.id);
                    let actual = json!({
                        "outcome": "root",
                        "contract_id": admitted.contract().id,
                        "projection": projection_value(admitted.projection()),
                    });
                    let (verified, projection) = admitted.into_parts();
                    assert_eq!(verified.event(), &expected_envelope, "{}", vector.id);
                    assert!(projection.classification().is_root_card(), "{}", vector.id);
                    actual
                }
                RadrootsPostAdmissionOutcome::ThreadExcluded(candidate) => {
                    assert_eq!(candidate.event(), &expected_envelope, "{}", vector.id);
                    let actual = json!({
                        "outcome": "thread_excluded",
                        "projection": projection_value(candidate.projection()),
                    });
                    let (verified, projection) = candidate.into_parts();
                    assert_eq!(verified.event(), &expected_envelope, "{}", vector.id);
                    assert_eq!(
                        projection.classification(),
                        RadrootsPostClassification::ThreadExcluded,
                        "{}",
                        vector.id
                    );
                    actual
                }
                _ => panic!("{} returned an unsupported admission outcome", vector.id),
            };
            assert_eq!(actual, vector.expected, "{}", vector.id);
        }
        "social.post.verify_and_admit_event.invalid" => {
            let envelope = canonical_envelope(input_str(vector, "event_json"));
            let error = verify_and_admit_post_event(envelope)
                .expect_err("invalid signed post vector must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
        }
        kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
    }
}

fn projection_value(projection: &RadrootsInboundPostProjection) -> Value {
    json!({
        "classification": classification_label(projection.classification()),
        "contract_id": projection.classification().contract_id(),
        "ask_marker": projection.ask_marker(),
        "diagnostics": diagnostic_codes(projection.diagnostics()),
        "imeta": projection.imeta().iter().map(|media| json!({
            "raw_fields": media.raw_fields(),
            "url": media.url(),
            "sha256": media.sha256(),
            "media_type": media.media_type(),
            "dimensions": media.dimensions().map(|dimensions| json!({
                "width": dimensions.width(),
                "height": dimensions.height(),
            })),
            "size": media.size(),
            "alt": media.alt(),
            "fallbacks": media.fallbacks(),
            "unknown_fields": media.unknown_fields(),
            "diagnostics": diagnostic_codes(media.diagnostics()),
            "qualifies_photo": media.qualifies_photo(),
        })).collect::<Vec<_>>(),
    })
}

fn reply_projection_value(projection: &RadrootsInboundNip10ReplyProjection) -> Value {
    json!({
        "style": reply_style_label(projection.style()),
        "contract_id": projection.contract_id(),
        "direct": projection.is_direct(),
        "root": reply_event_reference_value(projection.root()),
        "parent": projection.reply_reference().map(reply_event_reference_value),
        "citations": projection
            .citations()
            .iter()
            .map(reply_event_reference_value)
            .collect::<Vec<_>>(),
        "participants": projection
            .participants()
            .iter()
            .map(reply_participant_value)
            .collect::<Vec<_>>(),
        "diagnostics": projection
            .diagnostics()
            .iter()
            .map(reply_diagnostic_value)
            .collect::<Vec<_>>(),
    })
}

fn reply_event_reference_value(reference: &RadrootsInboundNip10EventReference) -> Value {
    json!({
        "tag_index": reference.tag_index(),
        "raw_tag": reference.raw_tag(),
        "event_id": reference.event_id().as_str(),
        "relay": reference.relay().map(|relay| relay.as_str()),
        "author_hint": reference.author_hint().map(|author| author.as_str()),
    })
}

fn reply_participant_value(participant: &RadrootsInboundNip10Participant) -> Value {
    json!({
        "tag_index": participant.tag_index(),
        "raw_tag": participant.raw_tag(),
        "pubkey": participant.pubkey().as_str(),
        "relay": participant.relay().map(|relay| relay.as_str()),
    })
}

fn reply_diagnostic_value(diagnostic: &RadrootsNip10ReplyDiagnostic) -> Value {
    json!({
        "code": diagnostic.code(),
        "tag_index": diagnostic.tag_index(),
        "raw_tag": diagnostic.raw_tag(),
    })
}

fn wire_parts_value(parts: &RadrootsNip01EventWireParts) -> Value {
    json!({
        "kind": parts.kind,
        "content": parts.content,
        "tags": parts.tags,
    })
}

fn authored_reply(vector: &Vector) -> Result<RadrootsAuthoredNip10Reply, RadrootsNip10ReplyError> {
    let root = authored_reply_reference(vector, &vector.input["root"])?;
    match vector.input.get("parent") {
        None | Some(Value::Null) => {
            RadrootsAuthoredNip10Reply::direct(input_str(vector, "content"), root)
        }
        Some(parent) => RadrootsAuthoredNip10Reply::nested(
            input_str(vector, "content"),
            root,
            authored_reply_reference(vector, parent)?,
        ),
    }
}

fn authored_reply_reference(
    vector: &Vector,
    input: &Value,
) -> Result<RadrootsNip10ReplyReference, RadrootsNip10ReplyError> {
    let relay = match input.get("relay") {
        None | Some(Value::Null) => None,
        Some(relay) => Some(
            relay
                .as_str()
                .unwrap_or_else(|| panic!("{} Reply relay must be a string or null", vector.id)),
        ),
    };
    RadrootsNip10ReplyReference::parse(
        value_str(vector, input, "event_id"),
        value_str(vector, input, "author"),
        relay,
    )
}

fn authored_images(
    vector: &Vector,
) -> Result<Vec<RadrootsAuthoredPostImage>, RadrootsAuthoredPostError> {
    vector
        .input
        .get("images")
        .and_then(Value::as_array)
        .map_or_else(
            || Ok(Vec::new()),
            |images| {
                images
                    .iter()
                    .map(|image| authored_image(vector, image))
                    .collect()
            },
        )
}

fn authored_image(
    vector: &Vector,
    input: &Value,
) -> Result<RadrootsAuthoredPostImage, RadrootsAuthoredPostError> {
    let bytes = value_str(vector, input, "bytes_utf8").as_bytes();
    let media_type = RadrootsBlossomMediaType::parse(value_str(vector, input, "media_type"))
        .unwrap_or_else(|error| panic!("{} media type setup failed: {error}", vector.id));
    let hash = RadrootsBlossomSha256::digest(bytes);
    let descriptor = RadrootsBlossomBlobDescriptor::new(
        RadrootsBlossomBlobUrl::parse(value_str(vector, input, "url"))
            .unwrap_or_else(|error| panic!("{} URL setup failed: {error}", vector.id)),
        hash,
        u64::try_from(bytes.len()).expect("authored image byte length must fit u64"),
        media_type.clone(),
        value_u64(vector, input, "uploaded"),
    )
    .unwrap_or_else(|error| panic!("{} descriptor setup failed: {error}", vector.id))
    .approve_reference()
    .unwrap_or_else(|error| panic!("{} URL approval failed: {error}", vector.id))
    .verify_bytes(bytes, &media_type)
    .unwrap_or_else(|error| panic!("{} byte verification failed: {error}", vector.id));
    let image = RadrootsAuthoredImage::try_from(descriptor)
        .unwrap_or_else(|error| panic!("{} image typestate setup failed: {error}", vector.id));
    let dimensions = RadrootsPostImageDimensions::new(
        u32::try_from(value_u64(vector, input, "width"))
            .unwrap_or_else(|_| panic!("{} image.width must fit u32", vector.id)),
        u32::try_from(value_u64(vector, input, "height"))
            .unwrap_or_else(|_| panic!("{} image.height must fit u32", vector.id)),
    )?;
    let mut image =
        RadrootsAuthoredPostImage::new(image, dimensions, value_str(vector, input, "alt"))?;
    if let Some(fallbacks) = input.get("fallbacks").and_then(Value::as_array) {
        for fallback in fallbacks {
            image = image.try_with_fallback(
                RadrootsBlossomBlobUrl::parse(
                    fallback
                        .as_str()
                        .unwrap_or_else(|| panic!("{} image fallback must be a string", vector.id)),
                )
                .unwrap_or_else(|error| panic!("{} fallback setup failed: {error}", vector.id))
                .approve()
                .unwrap_or_else(|error| panic!("{} fallback approval failed: {error}", vector.id)),
            )?;
        }
    }
    Ok(image)
}

fn classification_label(classification: RadrootsPostClassification) -> &'static str {
    match classification {
        RadrootsPostClassification::ThreadExcluded => "thread_excluded",
        RadrootsPostClassification::Update => "update",
        RadrootsPostClassification::PhotoUpdate => "photo_update",
        RadrootsPostClassification::Ask => "ask",
        _ => "future",
    }
}

fn reply_style_label(style: RadrootsNip10ReplyStyle) -> &'static str {
    match style {
        RadrootsNip10ReplyStyle::Marked => "marked",
        RadrootsNip10ReplyStyle::LegacyPositional => "legacy_positional",
        _ => "future",
    }
}

fn diagnostic_codes(diagnostics: &[RadrootsPostDiagnostic]) -> Vec<&'static str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code())
        .collect()
}

fn canonical_envelope(raw_json: &str) -> RadrootsEventEnvelope {
    RadrootsNip01EventWire::parse_json(raw_json)
        .expect("canonical raw event")
        .into_envelope()
        .expect("event envelope")
}

fn input_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.input[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} input.{field} must be a string", vector.id))
}

fn value_str<'a>(vector: &Vector, value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} image.{field} must be a string", vector.id))
}

fn value_u64(vector: &Vector, value: &Value, field: &str) -> u64 {
    value[field]
        .as_u64()
        .unwrap_or_else(|| panic!("{} image.{field} must be a u64", vector.id))
}

fn expected_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.expected[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} expected.{field} must be a string", vector.id))
}

fn vector_cases(vectors: &[Vector]) -> BTreeMap<&str, &str> {
    let mut cases = BTreeMap::new();
    for vector in vectors {
        assert!(
            cases
                .insert(vector.id.as_str(), vector.kind.as_str())
                .is_none(),
            "duplicate post vector id {}",
            vector.id
        );
    }
    cases
}

fn expected_vector_cases() -> BTreeMap<&'static str, &'static str> {
    EXPECTED_VECTOR_CASES.into_iter().collect()
}

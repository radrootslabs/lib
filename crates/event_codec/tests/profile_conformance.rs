#![cfg(feature = "json")]

use std::{borrow::Cow, fs, path::Path};

use radroots_blossom::{BlobDescriptor, ByteVerifiedDescriptor};
use radroots_event::{
    media::{AuthoredImage, AuthoredImageError},
    profile::{
        AuthoredProfile, AuthoredProfileError, Nip05Identifier,
        RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES,
    },
};
use radroots_event_codec::decode::profile::{
    RadrootsProfileMetadataParseError, parse_inbound_profile_metadata,
};
use radroots_event_codec::encode::profile::{
    RadrootsAuthoredProfileEncodeError, authored_profile_to_wire_parts,
};
use serde::Deserialize;
use serde_json::{Map, Value};

const PACKAGED_VECTORS: &str = include_str!("fixtures/profile_metadata.v1.json");
const WORKSPACE_VECTOR_PATH: &str = "../../contracts/conformance/vectors/profile/metadata.v1.json";
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredProfileInput {
    name: String,
    display_name: Option<String>,
    about: Option<String>,
    picture: Option<AuthoredMediaInput>,
    banner: Option<AuthoredMediaInput>,
    nip05: Option<String>,
    bot: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredMediaInput {
    bytes_utf8: String,
    descriptor: BlobDescriptor,
}

#[test]
fn checked_in_profile_vectors_execute_against_strict_and_tolerant_public_apis() {
    let vectors = conformance_vectors();
    let suite: Suite = serde_json::from_str(&vectors).expect("Profile vectors must parse");
    assert_eq!(suite.suite, "profile_metadata");
    assert_eq!(suite.contract_version, "1.0.0");
    assert!(!suite.vectors.is_empty());

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
                "packaged Profile vectors must match {}",
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
        "profile.nip05.parse.valid" => nip05_valid(vector),
        "profile.nip05.parse.invalid" => nip05_invalid(vector),
        "profile.authored.new.invalid" => authored_name_invalid(vector),
        "profile.image.from_verified_descriptor.invalid" => authored_image_invalid(vector),
        "profile.build_authored_draft.valid" => authored_valid(vector),
        "profile.build_authored_draft.limit.valid" => authored_limit_valid(vector),
        "profile.build_authored_draft.invalid" => authored_invalid(vector),
        "profile.parse_inbound_metadata.valid" => inbound_valid(vector),
        "profile.parse_inbound_metadata.limit.valid" => inbound_limit_valid(vector),
        "profile.parse_inbound_metadata.invalid" => inbound_invalid(vector),
        kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
    }
}

fn nip05_valid(vector: &Vector) {
    let identifier = Nip05Identifier::parse(input_str(vector, "identifier"))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        identifier.as_str(),
        expected_str(vector, "identifier"),
        "{}",
        vector.id
    );
    assert_eq!(
        identifier.local_part(),
        expected_str(vector, "local_part"),
        "{}",
        vector.id
    );
    assert_eq!(
        identifier.domain(),
        expected_str(vector, "domain"),
        "{}",
        vector.id
    );
    assert_eq!(
        expected_str(vector, "identity_verification"),
        "not_performed"
    );
}

fn nip05_invalid(vector: &Vector) {
    let error = Nip05Identifier::parse(input_str(vector, "identifier"))
        .expect_err("invalid NIP-05 vector must fail");
    assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
}

fn authored_valid(vector: &Vector) {
    let profile = authored_profile(&vector.input["profile"], &vector.id);
    let wire = authored_profile_to_wire_parts(&profile)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    let expected = &vector.expected["wire_parts"];
    assert_eq!(
        u64::from(wire.kind),
        expected["kind"].as_u64().unwrap(),
        "{}",
        vector.id
    );
    assert_eq!(
        wire.content,
        expected["content"].as_str().unwrap(),
        "{}",
        vector.id
    );
    assert_eq!(
        serde_json::to_value(wire.tags).unwrap(),
        expected["tags"],
        "{}",
        vector.id
    );
    assert_eq!(
        expected_str(vector, "upload_completion"),
        "not_attested_by_codec"
    );
}

fn authored_name_invalid(vector: &Vector) {
    let error = AuthoredProfile::new(input_str(vector, "name"))
        .expect_err("invalid authored Profile name must fail");
    assert_eq!(error, AuthoredProfileError::InvalidName);
    assert_eq!(error.code(), expected_str(vector, "error"));
}

fn authored_limit_valid(vector: &Vector) {
    let name_bytes = vector.input["generated_name_bytes"]
        .as_u64()
        .expect("generated_name_bytes") as usize;
    let profile = AuthoredProfile::new("a".repeat(name_bytes)).unwrap();
    let wire = authored_profile_to_wire_parts(&profile)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        u64::from(wire.kind),
        vector.expected["kind"].as_u64().unwrap()
    );
    assert_eq!(wire.content.len(), expected_usize(vector, "content_bytes"));
    assert_eq!(
        wire.content.len(),
        RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES
    );
    assert_eq!(
        serde_json::to_value(wire.tags).unwrap(),
        vector.expected["tags"]
    );
    assert_eq!(
        expected_str(vector, "upload_completion"),
        "not_attested_by_codec"
    );
}

fn authored_image_invalid(vector: &Vector) {
    let input: AuthoredMediaInput = serde_json::from_value(vector.input["media"].clone())
        .unwrap_or_else(|error| panic!("{} media fixture failed: {error}", vector.id));
    let descriptor = verified_descriptor(&input, &vector.id);
    let error = AuthoredImage::try_from(descriptor).expect_err("non-image Profile media must fail");
    assert_eq!(error, AuthoredImageError::MediaTypeNotImage);
    assert_eq!(error.code(), expected_str(vector, "error"));
}

fn authored_invalid(vector: &Vector) {
    let name_bytes = vector.input["generated_name_bytes"]
        .as_u64()
        .expect("generated_name_bytes") as usize;
    let profile = AuthoredProfile::new("a".repeat(name_bytes)).unwrap();
    let error = authored_profile_to_wire_parts(&profile)
        .expect_err("oversized authored Profile metadata must fail");
    assert_eq!(error.code(), expected_str(vector, "error"));
    assert!(!error.to_string().is_empty());
    match error {
        RadrootsAuthoredProfileEncodeError::ContentTooLarge { max, actual } => {
            assert_eq!(max, expected_usize(vector, "max"));
            assert_eq!(actual, expected_usize(vector, "actual"));
            assert_eq!(max, RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES);
        }
        _ => panic!("{} returned an unexpected authored error", vector.id),
    }
}

fn authored_profile(input: &Value, vector_id: &str) -> AuthoredProfile {
    let input: AuthoredProfileInput = serde_json::from_value(input.clone())
        .unwrap_or_else(|error| panic!("{vector_id} authored input failed: {error}"));
    let mut profile = AuthoredProfile::new(input.name)
        .unwrap_or_else(|error| panic!("{vector_id} name failed: {error}"));
    if let Some(value) = input.display_name {
        profile = profile.with_display_name(value);
    }
    if let Some(value) = input.about {
        profile = profile.with_about(value);
    }
    if let Some(value) = input.picture {
        profile = profile.with_picture(authored_image(&value, vector_id));
    }
    if let Some(value) = input.banner {
        profile = profile.with_banner(authored_image(&value, vector_id));
    }
    if let Some(value) = input.nip05 {
        profile = profile.with_nip05(
            Nip05Identifier::parse(&value)
                .unwrap_or_else(|error| panic!("{vector_id} NIP-05 failed: {error}")),
        );
    }
    if let Some(value) = input.bot {
        profile = profile.with_bot(value);
    }
    profile
}

fn authored_image(input: &AuthoredMediaInput, vector_id: &str) -> AuthoredImage {
    AuthoredImage::try_from(verified_descriptor(input, vector_id))
        .unwrap_or_else(|error| panic!("{vector_id} Profile image failed: {error}"))
}

fn verified_descriptor(input: &AuthoredMediaInput, vector_id: &str) -> ByteVerifiedDescriptor {
    let media_type = input.descriptor.media_type().clone();
    input
        .descriptor
        .clone()
        .approve_reference()
        .unwrap_or_else(|error| panic!("{vector_id} descriptor approval failed: {error}"))
        .verify_bytes(input.bytes_utf8.as_bytes(), &media_type)
        .unwrap_or_else(|error| panic!("{vector_id} byte verification failed: {error}"))
}

fn inbound_valid(vector: &Vector) {
    let content = inbound_content(vector);
    let metadata = parse_inbound_profile_metadata(&content)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(metadata.raw_content(), content.as_ref(), "{}", vector.id);

    let raw: Value = serde_json::from_str(&content).expect("valid vector JSON");
    assert_eq!(
        serde_json::to_value(metadata.raw_fields()).unwrap(),
        raw,
        "{}",
        vector.id
    );
    assert_eq!(
        serde_json::to_value(metadata.residual_fields()).unwrap(),
        vector.expected["residual_fields"],
        "{}",
        vector.id
    );
    assert_eq!(
        projected_metadata(&metadata),
        vector.expected["projected"],
        "{}",
        vector.id
    );
    assert_eq!(
        metadata.nip05_identity_verification().code(),
        expected_str(vector, "identity_verification")
    );

    if let Some(media) = vector.expected.get("media_verification") {
        if media.get("picture").is_some() {
            assert_eq!(
                metadata.picture().expect("picture").to_string(),
                metadata.picture().unwrap().as_str()
            );
            assert_eq!(media["picture"], "unverified");
        }
        if media.get("banner").is_some() {
            assert_eq!(
                metadata.banner().expect("banner").to_string(),
                metadata.banner().unwrap().as_str()
            );
            assert_eq!(media["banner"], "unverified");
        }
    }
}

fn inbound_limit_valid(vector: &Vector) {
    let content = inbound_content(vector);
    let metadata = parse_inbound_profile_metadata(&content)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(content.len(), expected_usize(vector, "content_bytes"));
    assert_eq!(content.len(), RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES);
    assert_eq!(metadata.raw_content(), content.as_ref());
    assert_eq!(metadata.name().map(str::len), Some(content.len() - 11));
}

fn projected_metadata(
    metadata: &radroots_event_codec::decode::profile::RadrootsInboundProfileMetadata,
) -> Value {
    let mut projected = Map::new();
    insert_string(&mut projected, "name", metadata.name());
    insert_string(&mut projected, "display_name", metadata.display_name());
    insert_string(&mut projected, "about", metadata.about());
    insert_string(
        &mut projected,
        "picture",
        metadata.picture().map(|value| value.as_str()),
    );
    insert_string(
        &mut projected,
        "banner",
        metadata.banner().map(|value| value.as_str()),
    );
    insert_string(
        &mut projected,
        "nip05",
        metadata.nip05().map(|value| value.as_str()),
    );
    if let Some(value) = metadata.bot() {
        projected.insert("bot".to_string(), Value::Bool(value));
    }
    Value::Object(projected)
}

fn insert_string(projected: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        projected.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn inbound_invalid(vector: &Vector) {
    let content = inbound_content(vector);
    let error = parse_inbound_profile_metadata(&content)
        .expect_err("invalid inbound Profile vector must fail");
    assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
    assert!(!error.to_string().is_empty());
    match error {
        RadrootsProfileMetadataParseError::ContentTooLarge { max, actual } => {
            assert_eq!(max, expected_usize(vector, "max"));
            assert_eq!(actual, expected_usize(vector, "actual"));
        }
        RadrootsProfileMetadataParseError::DuplicateField(field) => {
            assert_eq!(field, expected_str(vector, "field"), "{}", vector.id);
        }
        RadrootsProfileMetadataParseError::InvalidJson
        | RadrootsProfileMetadataParseError::RootNotObject => {}
        _ => panic!("{} returned an unexpected inbound error", vector.id),
    }
}

fn inbound_content<'a>(vector: &'a Vector) -> Cow<'a, str> {
    if let Some(content) = vector.input.get("content").and_then(Value::as_str) {
        return Cow::Borrowed(content);
    }
    let bytes = vector.input["generated_content_bytes"]
        .as_u64()
        .expect("input.generated_content_bytes") as usize;
    assert!(bytes >= 11);
    let value = "a".repeat(bytes - 11);
    let content = format!(r#"{{"name":"{value}"}}"#);
    assert_eq!(content.len(), bytes);
    Cow::Owned(content)
}

fn input_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.input[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} input.{field} must be a string", vector.id))
}

fn expected_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector.expected[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} expected.{field} must be a string", vector.id))
}

fn expected_usize(vector: &Vector, field: &str) -> usize {
    vector.expected[field]
        .as_u64()
        .unwrap_or_else(|| panic!("{} expected.{field} must be an integer", vector.id)) as usize
}

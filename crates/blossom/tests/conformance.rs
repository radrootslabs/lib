#![cfg(feature = "serde")]

use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256, hash::HashPath};
use serde::Deserialize;
use serde_json::Value;
use std::{borrow::Cow, collections::BTreeSet, fs, path::Path};

const PACKAGED_VECTORS: &str = include_str!("fixtures/hash_path_and_descriptor.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/blossom/hash_path_and_descriptor.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";
const SUPPORTED_VECTOR_KINDS: [&str; 17] = [
    "blossom.sha256.digest",
    "blossom.sha256.parse.valid",
    "blossom.sha256.parse.invalid",
    "blossom.hash_path.parse.valid",
    "blossom.hash_path.parse.invalid",
    "blossom.blob_url.parse.valid",
    "blossom.blob_url.parse.invalid",
    "blossom.reference_policy.valid",
    "blossom.reference_policy.invalid",
    "blossom.media_type.parse.valid",
    "blossom.media_type.parse.invalid",
    "blossom.descriptor.parse.valid",
    "blossom.descriptor.parse.invalid",
    "blossom.descriptor.approve_reference.valid",
    "blossom.descriptor.approve_reference.invalid",
    "blossom.descriptor.verify_bytes.valid",
    "blossom.descriptor.verify_bytes.invalid",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite {
    suite: String,
    contract_version: String,
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    id: String,
    kind: String,
    input: Value,
    expected: Value,
}

#[test]
fn checked_in_vectors_execute_against_public_api() {
    let vectors = conformance_vectors();
    let suite: Suite = serde_json::from_str(&vectors).expect("Blossom vectors must parse");
    assert_eq!(suite.suite, "blossom_hash_path_and_descriptor");
    assert_eq!(suite.contract_version, "1.0.0");
    assert!(!suite.vectors.is_empty());
    assert_vector_inventory(&suite.vectors);

    for vector in &suite.vectors {
        execute(vector);
    }
}

fn assert_vector_inventory(vectors: &[Vector]) {
    let mut ids = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    for vector in vectors {
        assert!(!vector.id.trim().is_empty(), "vector id must be nonblank");
        assert!(
            ids.insert(vector.id.as_str()),
            "duplicate vector id {}",
            vector.id
        );
        assert!(
            vector.input.is_object(),
            "{} input must be an object",
            vector.id
        );
        assert!(
            vector.expected.is_object(),
            "{} expected must be an object",
            vector.id
        );
        kinds.insert(vector.kind.as_str());
    }
    assert_eq!(kinds, BTreeSet::from(SUPPORTED_VECTOR_KINDS));
}

fn conformance_vectors() -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_VECTOR_PATH);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                PACKAGED_VECTORS,
                "packaged Blossom vectors must match {}",
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
        "blossom.sha256.digest" => sha256_digest(vector),
        "blossom.sha256.parse.valid" => sha256_parse_valid(vector),
        "blossom.sha256.parse.invalid" => sha256_parse_invalid(vector),
        "blossom.hash_path.parse.valid" => hash_path_parse_valid(vector),
        "blossom.hash_path.parse.invalid" => hash_path_parse_invalid(vector),
        "blossom.blob_url.parse.valid" => blob_url_parse_valid(vector),
        "blossom.blob_url.parse.invalid" => blob_url_parse_invalid(vector),
        "blossom.reference_policy.valid" => reference_policy_valid(vector),
        "blossom.reference_policy.invalid" => reference_policy_invalid(vector),
        "blossom.media_type.parse.valid" => media_type_parse_valid(vector),
        "blossom.media_type.parse.invalid" => media_type_parse_invalid(vector),
        "blossom.descriptor.parse.valid" => descriptor_parse_valid(vector),
        "blossom.descriptor.parse.invalid" => descriptor_parse_invalid(vector),
        "blossom.descriptor.approve_reference.valid" => descriptor_approval_valid(vector),
        "blossom.descriptor.approve_reference.invalid" => descriptor_approval_invalid(vector),
        "blossom.descriptor.verify_bytes.valid" => descriptor_verify_valid(vector),
        "blossom.descriptor.verify_bytes.invalid" => descriptor_verify_invalid(vector),
        kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
    }
}

fn sha256_digest(vector: &Vector) {
    let bytes = input_bytes(vector);
    assert_eq!(
        Sha256::digest(&bytes).to_string(),
        expected_str(vector, "sha256"),
        "{}",
        vector.id
    );
}

fn sha256_parse_valid(vector: &Vector) {
    let parsed = Sha256::from_hex(input_str(vector, "sha256"))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        parsed.to_string(),
        expected_str(vector, "sha256"),
        "{}",
        vector.id
    );
}

fn sha256_parse_invalid(vector: &Vector) {
    let error = Sha256::from_hex(input_str(vector, "sha256"))
        .expect_err("invalid SHA-256 vector must fail");
    assert_error(vector, error.code());
}

fn hash_path_parse_valid(vector: &Vector) {
    let parsed = HashPath::parse(input_str(vector, "path"))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        parsed.hash().to_string(),
        expected_str(vector, "sha256"),
        "{}",
        vector.id
    );
    assert_eq!(
        parsed.extension().map(|extension| extension.as_str()),
        optional_expected_str(vector, "extension"),
        "{}",
        vector.id
    );
}

fn hash_path_parse_invalid(vector: &Vector) {
    let error =
        HashPath::parse(input_str(vector, "path")).expect_err("invalid hash-path vector must fail");
    assert_error(vector, error.code());
}

fn blob_url_parse_valid(vector: &Vector) {
    let parsed = parse_blob_url(vector);
    assert_eq!(
        parsed.scheme(),
        expected_str(vector, "scheme"),
        "{}",
        vector.id
    );
    assert_eq!(parsed.host(), expected_str(vector, "host"), "{}", vector.id);
    assert_eq!(
        parsed.port().map(u64::from),
        vector.expected.get("port").and_then(Value::as_u64),
        "{}",
        vector.id
    );
    assert_eq!(
        parsed.hash_path().hash().to_string(),
        expected_str(vector, "sha256"),
        "{}",
        vector.id
    );
    assert_eq!(
        parsed
            .hash_path()
            .extension()
            .map(|extension| extension.as_str()),
        optional_expected_str(vector, "extension"),
        "{}",
        vector.id
    );
}

fn blob_url_parse_invalid(vector: &Vector) {
    let error =
        BlobUrl::parse(input_str(vector, "url")).expect_err("invalid blob-URL vector must fail");
    assert_error(vector, error.code());
}

fn reference_policy_valid(vector: &Vector) {
    let parsed = parse_blob_url(vector);
    let transport = if parsed.is_https() {
        "https"
    } else if parsed.is_loopback_http() {
        "loopback_http"
    } else {
        panic!("{} is not an approved reference", vector.id);
    };
    let approved = parsed
        .approve()
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(vector.expected["approved"], true, "{}", vector.id);
    assert_eq!(
        transport,
        expected_str(vector, "transport"),
        "{}",
        vector.id
    );
    assert!(!approved.as_str().is_empty());
}

fn reference_policy_invalid(vector: &Vector) {
    let error = parse_blob_url(vector)
        .approve()
        .expect_err("insecure reference vector must fail");
    assert_error(vector, error.code());
}

fn media_type_parse_valid(vector: &Vector) {
    let parsed = MediaType::parse(input_str(vector, "media_type"))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        parsed.as_str(),
        expected_str(vector, "media_type"),
        "{}",
        vector.id
    );
}

fn media_type_parse_invalid(vector: &Vector) {
    let error = MediaType::parse(input_str(vector, "media_type"))
        .expect_err("invalid media-type vector must fail");
    assert_error(vector, error.code());
}

fn descriptor_parse_valid(vector: &Vector) {
    let descriptor = parse_descriptor(vector);
    let actual = serde_json::to_value(descriptor).expect("descriptor must serialize");
    assert_eq!(actual, vector.expected["descriptor"], "{}", vector.id);
}

fn descriptor_parse_invalid(vector: &Vector) {
    let input = &vector.input["descriptor"];
    serde_json::from_value::<BlobDescriptor>(input.clone())
        .expect_err("invalid descriptor vector must fail");
    let code = classify_descriptor_input(input);
    assert_error(vector, code);
    if expected_error(vector) == "missing_descriptor_field" {
        let field = expected_str(vector, "field");
        assert!(input.get(field).is_none(), "{} retains {field}", vector.id);
    }
}

fn descriptor_approval_valid(vector: &Vector) {
    let approved = parse_descriptor(vector)
        .approve_reference()
        .unwrap_or_else(|error| panic!("{} approval failed: {error}", vector.id));
    assert_eq!(
        approved.url().as_str(),
        expected_str(vector, "approved_url"),
        "{}",
        vector.id
    );
}

fn descriptor_approval_invalid(vector: &Vector) {
    let error = parse_descriptor(vector)
        .approve_reference()
        .expect_err("invalid descriptor approval vector must fail");
    assert_error(vector, error.code());
}

fn descriptor_verify_valid(vector: &Vector) {
    let bytes = input_bytes(vector);
    let media_type = approved_media_type(vector);
    let verified = parse_descriptor(vector)
        .approve_reference()
        .unwrap_or_else(|error| panic!("{} approval failed: {error}", vector.id))
        .verify_bytes(&bytes, &media_type)
        .unwrap_or_else(|error| panic!("{} verification failed: {error}", vector.id));
    let expected = &vector.expected["verified"];
    assert_eq!(
        verified.sha256().to_string(),
        expected["sha256"].as_str().expect("expected sha256"),
        "{}",
        vector.id
    );
    assert_eq!(
        verified.size(),
        expected["size"].as_u64().expect("expected size"),
        "{}",
        vector.id
    );
    assert_eq!(
        verified.media_type().as_str(),
        expected["media_type"]
            .as_str()
            .expect("expected media_type"),
        "{}",
        vector.id
    );
}

fn descriptor_verify_invalid(vector: &Vector) {
    let bytes = input_bytes(vector);
    let media_type = approved_media_type(vector);
    let error = parse_descriptor(vector)
        .approve_reference()
        .unwrap_or_else(|error| panic!("{} approval failed: {error}", vector.id))
        .verify_bytes(&bytes, &media_type)
        .expect_err("invalid descriptor verification vector must fail");
    assert_error(vector, error.code());
}

fn parse_blob_url(vector: &Vector) -> BlobUrl {
    BlobUrl::parse(input_str(vector, "url"))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id))
}

fn parse_descriptor(vector: &Vector) -> BlobDescriptor {
    serde_json::from_value(vector.input["descriptor"].clone())
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id))
}

fn approved_media_type(vector: &Vector) -> MediaType {
    MediaType::parse(input_str(vector, "approved_media_type"))
        .unwrap_or_else(|error| panic!("{} media type failed: {error}", vector.id))
}

fn input_bytes(vector: &Vector) -> Vec<u8> {
    hex::decode(input_str(vector, "bytes_hex"))
        .unwrap_or_else(|error| panic!("{} bytes_hex failed: {error}", vector.id))
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

fn optional_expected_str<'a>(vector: &'a Vector, field: &str) -> Option<&'a str> {
    vector.expected.get(field).and_then(Value::as_str)
}

fn expected_error(vector: &Vector) -> &str {
    expected_str(vector, "error")
}

fn assert_error(vector: &Vector, actual: &str) {
    assert_eq!(actual, expected_error(vector), "{}", vector.id);
}

fn classify_descriptor_input(input: &Value) -> &'static str {
    for field in ["url", "sha256", "size", "type", "uploaded"] {
        if input.get(field).is_none() {
            return "missing_descriptor_field";
        }
    }

    let url = match BlobUrl::parse(
        input["url"]
            .as_str()
            .expect("descriptor url must be a string"),
    ) {
        Ok(url) => url,
        Err(error) => return error.code(),
    };
    let sha256 = match Sha256::from_hex(
        input["sha256"]
            .as_str()
            .expect("descriptor sha256 must be a string"),
    ) {
        Ok(sha256) => sha256,
        Err(error) => return error.code(),
    };
    let Some(size) = input["size"].as_u64() else {
        return "invalid_descriptor_size";
    };
    let media_type = match MediaType::parse(
        input["type"]
            .as_str()
            .expect("descriptor type must be a string"),
    ) {
        Ok(media_type) => media_type,
        Err(error) => return error.code(),
    };
    let Some(uploaded) = input["uploaded"].as_u64() else {
        return "invalid_descriptor_uploaded";
    };

    BlobDescriptor::new(url, sha256, size, media_type, uploaded)
        .expect_err("invalid descriptor vector must fail structured construction")
        .code()
}

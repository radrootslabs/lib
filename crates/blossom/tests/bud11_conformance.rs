use radroots_blossom::{
    Sha256,
    authorization::{
        AuthoredUploadClaim, AuthorizationAction, AuthorizationClaim, AuthorizationContent,
        AuthorizationTarget, AuthorizationValidation, RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND,
        ServerDomain, ServerScopeRequirement,
    },
};
use serde::Deserialize;
use serde_json::Value;
use std::{borrow::Cow, fs, path::Path};

const PACKAGED_VECTORS: &str = include_str!("fixtures/bud11_claims.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/blossom/bud11_claims.v1.json";
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

#[test]
fn checked_in_bud11_vectors_execute_against_public_api() {
    let vectors = conformance_vectors();
    let suite: Suite = serde_json::from_str(&vectors).expect("BUD-11 vectors must parse");
    assert_eq!(suite.suite, "blossom_bud11_claims");
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
                "packaged BUD-11 vectors must match {}",
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
        "blossom.bud11.action.parse.valid" => action_parse_valid(vector),
        "blossom.bud11.action.parse.invalid" => action_parse_invalid(vector),
        "blossom.bud11.server_domain.parse.valid" => server_domain_parse_valid(vector),
        "blossom.bud11.server_domain.parse.invalid" => server_domain_parse_invalid(vector),
        "blossom.bud11.content.parse.valid" => content_parse_valid(vector),
        "blossom.bud11.content.parse.invalid" => content_parse_invalid(vector),
        "blossom.bud11.claim.parse.valid" => claim_parse_valid(vector),
        "blossom.bud11.claim.parse.invalid" => claim_parse_invalid(vector),
        "blossom.bud11.validation.new.valid" => validation_new_valid(vector),
        "blossom.bud11.validation.new.invalid" => validation_new_invalid(vector),
        "blossom.bud11.claim.validate.valid" => claim_validate_valid(vector),
        "blossom.bud11.claim.validate.invalid" => claim_validate_invalid(vector),
        "blossom.bud11.authored_upload.valid" => authored_upload_valid(vector),
        "blossom.bud11.authored_upload.invalid" => authored_upload_invalid(vector),
        kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
    }
}

fn action_parse_valid(vector: &Vector) {
    let action = AuthorizationAction::parse(input_str(vector, "action"))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        action.as_str(),
        expected_str(vector, "action"),
        "{}",
        vector.id
    );
    assert_eq!(
        action.to_string(),
        expected_str(vector, "action"),
        "{}",
        vector.id
    );
}

fn action_parse_invalid(vector: &Vector) {
    let error = AuthorizationAction::parse(input_str(vector, "action"))
        .expect_err("invalid action vector must fail");
    assert_error(vector, error.code());
}

fn server_domain_parse_valid(vector: &Vector) {
    let domain = ServerDomain::parse(input_str(vector, "domain"))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        domain.as_str(),
        expected_str(vector, "domain"),
        "{}",
        vector.id
    );
    assert_eq!(
        domain.to_string(),
        expected_str(vector, "domain"),
        "{}",
        vector.id
    );
}

fn server_domain_parse_invalid(vector: &Vector) {
    let error = ServerDomain::parse(input_str(vector, "domain"))
        .expect_err("invalid server-domain vector must fail");
    assert_error(vector, error.code());
}

fn content_parse_valid(vector: &Vector) {
    let content = AuthorizationContent::parse(input_str(vector, "content"))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        content.as_str(),
        expected_str(vector, "content"),
        "{}",
        vector.id
    );
    assert_eq!(
        content.to_string(),
        expected_str(vector, "content"),
        "{}",
        vector.id
    );
}

fn content_parse_invalid(vector: &Vector) {
    let error = AuthorizationContent::parse(input_str(vector, "content"))
        .expect_err("invalid content vector must fail");
    assert_error(vector, error.code());
}

fn claim_parse_valid(vector: &Vector) {
    let claim = parse_claim(&vector.input, &vector.id);
    assert_claim(vector, &claim);
}

fn claim_parse_invalid(vector: &Vector) {
    let error = try_parse_claim(&vector.input).expect_err("invalid claim vector must fail");
    assert_error(vector, error.code());
}

fn validation_new_valid(vector: &Vector) {
    let validation =
        build_validation(vector).unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    assert_eq!(
        validation.max_created_age_seconds(),
        Some(expected_u64(vector, "max_created_age")),
        "{}",
        vector.id
    );
}

fn validation_new_invalid(vector: &Vector) {
    let error = build_validation(vector).expect_err("invalid validation-policy vector must fail");
    assert_error(vector, error.code());
}

fn claim_validate_valid(vector: &Vector) {
    let claim = parse_claim(&vector.input["claim"], &vector.id);
    let validation = build_validation(vector)
        .unwrap_or_else(|error| panic!("{} policy failed: {error}", vector.id));
    let validated = claim
        .validate(&validation)
        .unwrap_or_else(|error| panic!("{} validation failed: {error}", vector.id));
    assert_eq!(
        validated.action().as_str(),
        expected_str(vector, "action"),
        "{}",
        vector.id
    );
}

fn claim_validate_invalid(vector: &Vector) {
    let claim = parse_claim(&vector.input["claim"], &vector.id);
    let validation = build_validation(vector)
        .unwrap_or_else(|error| panic!("{} policy failed: {error}", vector.id));
    let error = claim
        .validate(&validation)
        .expect_err("invalid claim-validation vector must fail");
    assert_error(vector, error.code());
}

fn authored_upload_valid(vector: &Vector) {
    let authored =
        authored_upload(vector).unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
    let wire = authored.wire_parts();
    assert_eq!(
        wire.kind(),
        RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND,
        "{}",
        vector.id
    );
    assert_eq!(
        wire.kind() as u64,
        expected_u64(vector, "kind"),
        "{}",
        vector.id
    );
    assert_eq!(
        wire.content(),
        expected_str(vector, "content"),
        "{}",
        vector.id
    );
    assert_eq!(
        wire.created_at(),
        expected_u64(vector, "created_at"),
        "{}",
        vector.id
    );
    assert_eq!(
        authored.expiration(),
        expected_u64(vector, "expiration"),
        "{}",
        vector.id
    );
    assert_eq!(
        serde_json::to_value(wire.tags()).expect("wire tags must serialize"),
        vector.expected["tags"],
        "{}",
        vector.id
    );
}

fn authored_upload_invalid(vector: &Vector) {
    let error = authored_upload(vector).expect_err("invalid authored-upload vector must fail");
    assert_error(vector, error.code());
}

fn authored_upload(vector: &Vector) -> Result<AuthoredUploadClaim, radroots_blossom::Error> {
    let content = AuthorizationContent::parse(input_str(vector, "content"))?;
    let server = ServerDomain::parse(input_str(vector, "server_domain"))?;
    let hash = Sha256::from_hex(input_str(vector, "hash"))?;
    AuthoredUploadClaim::new(
        content,
        server,
        hash,
        input_u64(vector, "created_at"),
        input_u64(vector, "lifetime"),
    )
}

fn try_parse_claim(input: &Value) -> Result<AuthorizationClaim, radroots_blossom::Error> {
    let tags: Vec<Vec<String>> =
        serde_json::from_value(input["tags"].clone()).expect("claim tags must be string arrays");
    AuthorizationClaim::parse(
        value_str(input, "content"),
        value_u64(input, "created_at"),
        &tags,
    )
}

fn parse_claim(input: &Value, id: &str) -> AuthorizationClaim {
    try_parse_claim(input).unwrap_or_else(|error| panic!("{id} claim parse failed: {error}"))
}

fn assert_claim(vector: &Vector, claim: &AuthorizationClaim) {
    assert_eq!(
        claim.content().as_str(),
        expected_str(vector, "content"),
        "{}",
        vector.id
    );
    assert_eq!(
        claim.created_at(),
        expected_u64(vector, "created_at"),
        "{}",
        vector.id
    );
    assert_eq!(
        claim.action().as_str(),
        expected_str(vector, "action"),
        "{}",
        vector.id
    );
    assert_eq!(
        claim.expiration(),
        expected_u64(vector, "expiration"),
        "{}",
        vector.id
    );
    let servers: Vec<String> = claim
        .server_domains()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        servers,
        expected_strings(vector, "servers"),
        "{}",
        vector.id
    );
    let hashes: Vec<String> = claim.hashes().iter().map(ToString::to_string).collect();
    assert_eq!(hashes, expected_strings(vector, "hashes"), "{}", vector.id);
}

fn build_validation(vector: &Vector) -> Result<AuthorizationValidation, radroots_blossom::Error> {
    let target = parse_target(&vector.input["target"]);
    let server = ServerDomain::parse(input_str(vector, "server_domain"))?;
    let server_scope = match input_str(vector, "server_scope") {
        "optional_any_match" => ServerScopeRequirement::OptionalAnyMatch,
        "required_any_match" => ServerScopeRequirement::RequiredAnyMatch,
        value => panic!("{} has unsupported server scope {value}", vector.id),
    };
    AuthorizationValidation::new(
        target,
        server,
        server_scope,
        input_u64(vector, "now"),
        input_u64(vector, "max_created_age"),
    )
}

fn parse_target(input: &Value) -> AuthorizationTarget {
    let hash = || Sha256::from_hex(value_str(input, "hash")).expect("target hash must be valid");
    match value_str(input, "type") {
        "get_blob" => AuthorizationTarget::GetBlob(hash()),
        "upload" => AuthorizationTarget::Upload(hash()),
        "list" => AuthorizationTarget::List,
        "delete_blob" => AuthorizationTarget::DeleteBlob(hash()),
        "mirror" => AuthorizationTarget::Mirror(hash()),
        "media" => AuthorizationTarget::Media(hash()),
        value => panic!("unsupported BUD-11 target {value}"),
    }
}

fn input_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    value_str(&vector.input, field)
}

fn input_u64(vector: &Vector, field: &str) -> u64 {
    value_u64(&vector.input, field)
}

fn expected_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    value_str(&vector.expected, field)
}

fn expected_u64(vector: &Vector, field: &str) -> u64 {
    value_u64(&vector.expected, field)
}

fn expected_strings(vector: &Vector, field: &str) -> Vec<String> {
    serde_json::from_value(vector.expected[field].clone())
        .unwrap_or_else(|error| panic!("{} expected {field} is invalid: {error}", vector.id))
}

fn value_str<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} must be a string"))
}

fn value_u64(value: &Value, field: &str) -> u64 {
    value[field]
        .as_u64()
        .unwrap_or_else(|| panic!("{field} must be an unsigned integer"))
}

fn assert_error(vector: &Vector, actual: &str) {
    assert_eq!(actual, expected_str(vector, "error"), "{}", vector.id);
}

#![cfg(feature = "blossom")]

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use radroots_blossom::{
    Sha256,
    authorization::{
        AuthoredUploadClaim, AuthorizationClaim, AuthorizationContent, AuthorizationTarget,
        AuthorizationValidation, RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS,
        RADROOTS_BLOSSOM_AUTH_MAX_HORIZON_SECONDS, RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND,
        ServerDomain, ServerScopeRequirement,
    },
};
use radroots_nostr::blossom::{
    RadrootsNostrBlossomError, radroots_nostr_decode_verify_blossom_authorization_header,
    radroots_nostr_encode_blossom_authorization_header, radroots_nostr_sign_blossom_authorization,
};
use radroots_nostr::types::{
    RadrootsNostrEvent, RadrootsNostrKeys, RadrootsNostrKind, RadrootsNostrTag,
    RadrootsNostrTimestamp,
};
use serde::Deserialize;
use serde_json::Value;
use std::{borrow::Cow, fs, path::Path};

const PACKAGED_VECTORS: &str = include_str!("fixtures/bud11_nostr_adapter.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/blossom/bud11_nostr_adapter.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";

const SECRET_KEY: &str = "0000000000000000000000000000000000000000000000000000000000000001";
const HASH: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
const SERVER: &str = "cdn.example.com";
const CONTENT: &str = "Upload Victoria farm photo";
const CREATED_AT: u64 = 1_700_000_000;
const LIFETIME: u64 = RADROOTS_BLOSSOM_AUTH_MAX_HORIZON_SECONDS;

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
fn checked_in_blossom_bud11_nostr_vectors_execute_against_public_api() {
    let vectors = conformance_vectors();
    let suite: Suite = serde_json::from_str(&vectors).expect("BUD-11 Nostr vectors must parse");
    assert_eq!(suite.suite, "blossom_bud11_nostr_adapter");
    assert_eq!(suite.contract_version, "1.0.0");
    assert!(!suite.vectors.is_empty());

    for vector in &suite.vectors {
        execute_vector(vector);
    }
}

fn conformance_vectors() -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_VECTOR_PATH);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                PACKAGED_VECTORS,
                "packaged BUD-11 Nostr vectors must match {}",
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

fn execute_vector(vector: &Vector) {
    assert_embedded_event_matches_header(vector);
    let validation = vector_validation(vector);
    match vector.kind.as_str() {
        "blossom.bud11.nostr.decode_verify.valid" => {
            let verified = radroots_nostr_decode_verify_blossom_authorization_header(
                input_str(vector, "header"),
                &validation,
            )
            .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
            assert_eq!(
                verified.event_id().to_string(),
                expected_str(vector, "event_id"),
                "{}",
                vector.id
            );
            assert_eq!(
                verified.author().to_string(),
                expected_str(vector, "pubkey"),
                "{}",
                vector.id
            );
            assert_eq!(
                verified.claim().action().as_str(),
                expected_str(vector, "action"),
                "{}",
                vector.id
            );
            assert_eq!(
                verified.claim().content().as_str(),
                expected_str(vector, "content"),
                "{}",
                vector.id
            );
            assert_eq!(
                verified.claim().created_at(),
                expected_u64(vector, "created_at"),
                "{}",
                vector.id
            );
            assert_eq!(
                verified.claim().expiration(),
                expected_u64(vector, "expiration"),
                "{}",
                vector.id
            );
            assert_eq!(
                verified
                    .claim()
                    .hashes()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                expected_strings(vector, "hashes"),
                "{}",
                vector.id
            );
            assert_eq!(
                verified
                    .claim()
                    .server_domains()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                expected_strings(vector, "servers"),
                "{}",
                vector.id
            );
        }
        "blossom.bud11.nostr.decode_verify.invalid" => {
            let error = radroots_nostr_decode_verify_blossom_authorization_header(
                input_str(vector, "header"),
                &validation,
            )
            .expect_err("invalid BUD-11 Nostr vector must fail");
            assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
            if let Some(actual_kind) = vector.expected.get("actual_kind") {
                assert_eq!(
                    error,
                    RadrootsNostrBlossomError::InvalidEventKind {
                        actual: actual_kind.as_u64().expect("actual_kind must be u64"),
                    },
                    "{}",
                    vector.id
                );
            }
        }
        kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
    }
}

fn assert_embedded_event_matches_header(vector: &Vector) {
    let Some(event_json) = vector.input.get("event_json").and_then(Value::as_str) else {
        return;
    };
    let payload = header_payload(input_str(vector, "header"));
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .unwrap_or_else(|error| panic!("{} header failed Base64url decode: {error}", vector.id));
    assert_eq!(decoded, event_json.as_bytes(), "{}", vector.id);
}

fn header_payload(header: &str) -> &str {
    let first_space = header
        .as_bytes()
        .iter()
        .position(|byte| *byte == b' ')
        .expect("authorization header requires SP");
    assert!(
        header.as_bytes()[..first_space].eq_ignore_ascii_case(b"Nostr"),
        "authorization header requires Nostr scheme"
    );
    let payload_start = header.as_bytes()[first_space..]
        .iter()
        .position(|byte| *byte != b' ')
        .map_or(header.len(), |offset| first_space + offset);
    &header[payload_start..]
}

fn vector_validation(vector: &Vector) -> AuthorizationValidation {
    let validation = vector.input.get("validation").expect("validation input");
    let target = validation.get("target").expect("validation target");
    let target_type = target
        .get("type")
        .and_then(Value::as_str)
        .expect("validation target type");
    let target_hash = || {
        Sha256::from_hex(
            target
                .get("hash")
                .and_then(Value::as_str)
                .expect("validation target hash"),
        )
        .expect("valid validation target hash")
    };
    let target = match target_type {
        "get_blob" => AuthorizationTarget::GetBlob(target_hash()),
        "upload" => AuthorizationTarget::Upload(target_hash()),
        "list" => AuthorizationTarget::List,
        "delete_blob" => AuthorizationTarget::DeleteBlob(target_hash()),
        "mirror" => AuthorizationTarget::Mirror(target_hash()),
        "media" => AuthorizationTarget::Media(target_hash()),
        other => panic!("{} has unsupported target {other}", vector.id),
    };
    let server = ServerDomain::parse(
        validation
            .get("server_domain")
            .and_then(Value::as_str)
            .expect("validation server_domain"),
    )
    .expect("valid validation server_domain");
    let server_scope = match validation
        .get("server_scope")
        .and_then(Value::as_str)
        .expect("validation server_scope")
    {
        "optional_any_match" => ServerScopeRequirement::OptionalAnyMatch,
        "required_any_match" => ServerScopeRequirement::RequiredAnyMatch,
        other => panic!("{} has unsupported server scope {other}", vector.id),
    };
    AuthorizationValidation::new(
        target,
        server,
        server_scope,
        validation
            .get("now")
            .and_then(Value::as_u64)
            .expect("validation now"),
        validation
            .get("max_created_age")
            .and_then(Value::as_u64)
            .expect("validation max_created_age"),
    )
    .expect("valid vector validation")
}

fn input_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector
        .input
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{} missing string input {field}", vector.id))
}

fn expected_str<'a>(vector: &'a Vector, field: &str) -> &'a str {
    vector
        .expected
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{} missing string expectation {field}", vector.id))
}

fn expected_u64(vector: &Vector, field: &str) -> u64 {
    vector
        .expected
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("{} missing u64 expectation {field}", vector.id))
}

fn expected_strings(vector: &Vector, field: &str) -> Vec<String> {
    vector
        .expected
        .get(field)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{} missing array expectation {field}", vector.id))
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("{} has non-string {field} value", vector.id))
                .to_owned()
        })
        .collect()
}

fn keys() -> RadrootsNostrKeys {
    RadrootsNostrKeys::parse(SECRET_KEY).expect("fixed test secret key")
}

fn hash() -> Sha256 {
    Sha256::from_hex(HASH).expect("fixed test hash")
}

fn server() -> ServerDomain {
    ServerDomain::parse(SERVER).expect("fixed test server")
}

fn authored_claim() -> AuthoredUploadClaim {
    AuthoredUploadClaim::new(
        AuthorizationContent::parse(CONTENT).expect("fixed test content"),
        server(),
        hash(),
        CREATED_AT,
        LIFETIME,
    )
    .expect("fixed authored claim")
}

fn validation() -> AuthorizationValidation {
    AuthorizationValidation::new(
        AuthorizationTarget::Upload(hash()),
        server(),
        ServerScopeRequirement::RequiredAnyMatch,
        CREATED_AT + 1,
        RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS,
    )
    .expect("fixed validation policy")
}

fn raw_header(event: &RadrootsNostrEvent) -> String {
    let json = serde_json::to_vec(event).expect("test event JSON");
    raw_json_header(&String::from_utf8(json).expect("test event JSON is UTF-8"))
}

fn raw_json_header(json: &str) -> String {
    format!("Nostr {}", URL_SAFE_NO_PAD.encode(json))
}

fn event_from_header(header: &str) -> RadrootsNostrEvent {
    let json = URL_SAFE_NO_PAD
        .decode(header_payload(header))
        .expect("test authorization payload");
    serde_json::from_slice(&json).expect("test authorization event")
}

fn sign_raw(kind: u16, content: &str, tags: Vec<RadrootsNostrTag>) -> RadrootsNostrEvent {
    nostr::EventBuilder::new(RadrootsNostrKind::Custom(kind), content)
        .tags(tags)
        .custom_created_at(RadrootsNostrTimestamp::from_secs(CREATED_AT))
        .sign_with_keys(&keys())
        .expect("raw test event signs")
}

#[test]
fn blossom_authored_claim_signs_and_roundtrips_without_signature_assumptions() {
    let claim = authored_claim();
    let signed = radroots_nostr_sign_blossom_authorization(&keys(), &claim)
        .expect("sign authored authorization");
    let wire = claim.wire_parts();

    let header = radroots_nostr_encode_blossom_authorization_header(&signed);
    let event = event_from_header(header.as_str());

    assert_eq!(event.kind.as_u16(), wire.kind());
    assert_eq!(event.created_at.as_secs(), wire.created_at());
    assert_eq!(event.content, wire.content());
    assert_eq!(
        event
            .tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect::<Vec<_>>(),
        wire.tags()
    );
    assert!(event.verify_id());
    assert!(event.verify_signature());
    assert_eq!(signed.event_id(), event.id);
    assert_eq!(signed.author(), event.pubkey);
    assert_eq!(signed.created_at(), event.created_at);

    assert!(header.as_str().starts_with("Nostr "));
    assert!(!header.as_str().contains('='));
    assert_eq!(header.as_ref(), header.as_str());
    assert!(!format!("{header:?}").contains(header.as_str()));
    assert!(!format!("{signed:?}").contains(&event.sig.to_string()));

    let verified =
        radroots_nostr_decode_verify_blossom_authorization_header(header.as_str(), &validation())
            .expect("verify header");
    assert_eq!(verified.event_id(), event.id);
    assert_eq!(verified.author(), keys().public_key());
    assert_eq!(verified.created_at(), event.created_at);
    assert_eq!(verified.claim().content().to_string(), CONTENT);
    assert_eq!(verified.claim().created_at(), CREATED_AT);
    assert_eq!(verified.claim().action().to_string(), "upload");
    assert_eq!(verified.claim().expiration(), CREATED_AT + LIFETIME);
    assert_eq!(verified.claim().server_domains(), &[server()]);
    assert_eq!(verified.claim().hashes(), &[hash()]);
    assert!(!format!("{verified:?}").contains(&event.sig.to_string()));
}

#[test]
fn blossom_header_rejects_noncanonical_and_malformed_encodings() {
    let signed = radroots_nostr_sign_blossom_authorization(&keys(), &authored_claim())
        .expect("sign authored authorization");
    let valid = radroots_nostr_encode_blossom_authorization_header(&signed).into_string();
    let policy = validation();

    for accepted in [
        valid.replacen("Nostr ", "nostr ", 1),
        valid.replacen("Nostr ", "NOSTR   ", 1),
    ] {
        radroots_nostr_decode_verify_blossom_authorization_header(&accepted, &policy)
            .expect("auth-scheme is case-insensitive and accepts 1*SP");
    }

    let cases = [
        (
            format!(" {valid}"),
            RadrootsNostrBlossomError::InvalidHeaderWhitespace,
        ),
        (
            "Nostr e 30".to_owned(),
            RadrootsNostrBlossomError::InvalidHeaderWhitespace,
        ),
        (
            "Nostr".to_owned(),
            RadrootsNostrBlossomError::InvalidHeaderScheme,
        ),
        (
            "Nostr ".to_owned(),
            RadrootsNostrBlossomError::EmptyHeaderPayload,
        ),
        (
            format!("{valid}="),
            RadrootsNostrBlossomError::HeaderPaddingForbidden,
        ),
        (
            "Nostr +".to_owned(),
            RadrootsNostrBlossomError::InvalidHeaderBase64,
        ),
        (
            "Nostr Zh".to_owned(),
            RadrootsNostrBlossomError::NonCanonicalHeaderBase64,
        ),
        (
            "Nostr _w".to_owned(),
            RadrootsNostrBlossomError::InvalidHeaderUtf8,
        ),
        (
            "Nostr e30".to_owned(),
            RadrootsNostrBlossomError::InvalidEventJson,
        ),
    ];

    for (header, expected) in cases {
        assert_eq!(
            radroots_nostr_decode_verify_blossom_authorization_header(&header, &policy),
            Err(expected),
            "{header}"
        );
    }
}

#[test]
fn blossom_header_rejects_kind_narrowing_and_json_shape_laundering() {
    let signed = radroots_nostr_sign_blossom_authorization(&keys(), &authored_claim())
        .expect("sign authored authorization");
    let encoded = radroots_nostr_encode_blossom_authorization_header(&signed);
    let event_json =
        serde_json::to_string(&event_from_header(encoded.as_str())).expect("event JSON");
    let policy = validation();

    let cases = [
        (
            event_json.replacen("\"kind\":24242", "\"kind\":89778", 1),
            RadrootsNostrBlossomError::InvalidEventKind { actual: 89_778 },
        ),
        (
            format!(
                "{},\"unknown\":true}}",
                event_json.strip_suffix('}').unwrap()
            ),
            RadrootsNostrBlossomError::InvalidEventJson,
        ),
        (
            event_json.replacen("\"content\":", "\"unknown\":", 1),
            RadrootsNostrBlossomError::InvalidEventJson,
        ),
        (
            event_json.replacen("\"kind\":24242", "\"kind\":24242,\"kind\":24242", 1),
            RadrootsNostrBlossomError::InvalidEventJson,
        ),
        (
            event_json.replacen("\"kind\":24242", "\"kind\":\"24242\"", 1),
            RadrootsNostrBlossomError::InvalidEventJson,
        ),
        ("[]".to_owned(), RadrootsNostrBlossomError::InvalidEventJson),
        ("{".to_owned(), RadrootsNostrBlossomError::InvalidEventJson),
    ];

    for (json, expected) in cases {
        assert_eq!(
            radroots_nostr_decode_verify_blossom_authorization_header(
                &raw_json_header(&json),
                &policy,
            ),
            Err(expected),
            "{json}"
        );
    }
}

#[test]
fn blossom_header_authenticates_before_claim_parsing() {
    let policy = validation();

    let wrong_kind = sign_raw(1, "", Vec::new());
    assert_eq!(
        radroots_nostr_decode_verify_blossom_authorization_header(
            &raw_header(&wrong_kind),
            &policy
        ),
        Err(RadrootsNostrBlossomError::InvalidEventKind { actual: 1 })
    );

    let malformed_claim = sign_raw(RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND, "", Vec::new());
    let mut bad_id = malformed_claim.clone();
    bad_id.content.push('x');
    assert_eq!(
        radroots_nostr_decode_verify_blossom_authorization_header(&raw_header(&bad_id), &policy),
        Err(RadrootsNostrBlossomError::InvalidEventId)
    );

    let mut bad_signature = malformed_claim.clone();
    bad_signature.sig = sign_raw(
        RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND,
        "different",
        Vec::new(),
    )
    .sig;
    assert!(bad_signature.verify_id());
    assert!(!bad_signature.verify_signature());
    assert_eq!(
        radroots_nostr_decode_verify_blossom_authorization_header(
            &raw_header(&bad_signature),
            &policy
        ),
        Err(RadrootsNostrBlossomError::InvalidEventSignature)
    );

    let raw_tags: Vec<Vec<String>> = Vec::new();
    let pure_error = AuthorizationClaim::parse("", CREATED_AT, &raw_tags)
        .expect_err("empty content is not a claim");
    let adapter_error = radroots_nostr_decode_verify_blossom_authorization_header(
        &raw_header(&malformed_claim),
        &policy,
    )
    .expect_err("authenticated malformed claim must fail");
    assert_eq!(adapter_error.code(), pure_error.code());
    assert_eq!(adapter_error.blossom_claim_error(), Some(&pure_error));
    assert_eq!(adapter_error.to_string(), pure_error.to_string());

    let signed = radroots_nostr_sign_blossom_authorization(&keys(), &authored_claim())
        .expect("sign authored authorization");
    let media_policy = AuthorizationValidation::new(
        AuthorizationTarget::Media(hash()),
        server(),
        ServerScopeRequirement::RequiredAnyMatch,
        CREATED_AT + 1,
        RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS,
    )
    .expect("media validation policy");
    let validation_error = radroots_nostr_decode_verify_blossom_authorization_header(
        radroots_nostr_encode_blossom_authorization_header(&signed).as_str(),
        &media_policy,
    )
    .expect_err("upload claim must not authorize media endpoint");
    assert_eq!(validation_error.code(), "authorization_action_mismatch");
}

#[test]
fn blossom_adapter_error_codes_are_stable_and_distinct() {
    let errors = [
        RadrootsNostrBlossomError::InvalidHeaderWhitespace,
        RadrootsNostrBlossomError::InvalidHeaderScheme,
        RadrootsNostrBlossomError::EmptyHeaderPayload,
        RadrootsNostrBlossomError::HeaderPaddingForbidden,
        RadrootsNostrBlossomError::InvalidHeaderBase64,
        RadrootsNostrBlossomError::NonCanonicalHeaderBase64,
        RadrootsNostrBlossomError::InvalidHeaderUtf8,
        RadrootsNostrBlossomError::InvalidEventJson,
        RadrootsNostrBlossomError::InvalidEventKind { actual: 1 },
        RadrootsNostrBlossomError::InvalidEventId,
        RadrootsNostrBlossomError::InvalidEventSignature,
        RadrootsNostrBlossomError::EventSigning,
    ];
    let mut codes = errors
        .iter()
        .map(RadrootsNostrBlossomError::code)
        .collect::<Vec<_>>();
    let before = codes.len();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), before);
    assert!(errors.iter().all(|error| !error.to_string().is_empty()));
    assert_eq!(errors[0].blossom_claim_error(), None);
}

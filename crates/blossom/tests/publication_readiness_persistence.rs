#![cfg(feature = "serde")]

use radroots_blossom::{
    PublicationReadinessEvidence, RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CANONICAL_VECTOR: &[u8] = include_bytes!(
    "../../../contracts/conformance/vectors/blossom/publication_readiness_persistence.v1.json"
);
const PACKAGED_VECTOR: &[u8] = include_bytes!("fixtures/publication_readiness_persistence.v1.json");
const CANONICAL_EVIDENCE: &str = concat!(
    "{\"schema_version\":1,\"policy_version\":1,",
    "\"url\":\"https://cdn.example/",
    "4490130851783ff662845f5e72f1948618cc87f951f00f6c2ffb3dc01f3f40fd.png\",",
    "\"sha256\":\"4490130851783ff662845f5e72f1948618cc87f951f00f6c2ffb3dc01f3f40fd\",",
    "\"size\":70,\"media_type\":\"image/png\",\"raster_format\":\"png\",",
    "\"dimensions\":{\"width\":1,\"height\":1},",
    "\"bud02_status\":201,\"bud01_head_status\":200,\"bud01_get_status\":200,",
    "\"uploaded\":1800000001,",
    "\"evidence_digest\":\"637ac3e9ffbb00fbacb60dcf98b466c949f99fd9f608e163127bea970b91670c\"}"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorSuite {
    suite: String,
    contract_version: String,
    vectors: Vec<VectorCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorCase {
    id: String,
    kind: String,
    input: VectorInput,
    expected: VectorExpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorInput {
    mutation: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorExpected {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    policy_version: Option<u16>,
    #[serde(default)]
    schema_version: Option<u32>,
}

#[derive(Serialize)]
struct EvidenceDimensionsWire {
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct EvidenceWire<'a> {
    schema_version: u32,
    policy_version: u16,
    url: &'a str,
    sha256: &'a str,
    size: u64,
    media_type: &'a str,
    raster_format: &'a str,
    dimensions: EvidenceDimensionsWire,
    bud02_status: u16,
    bud01_head_status: u16,
    bud01_get_status: u16,
    uploaded: u64,
    evidence_digest: String,
}

#[test]
fn publication_readiness_persistence_vector_executes_every_case() {
    assert_eq!(CANONICAL_VECTOR, PACKAGED_VECTOR, "packaged vector drift");
    let suite: VectorSuite = serde_json::from_slice(PACKAGED_VECTOR).expect("persistence vector");
    assert_eq!(suite.suite, "blossom_publication_readiness_persistence");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_eq!(suite.vectors.len(), 31);

    for vector in suite.vectors {
        let bytes = mutated_evidence(&vector.input.mutation);
        match vector.kind.as_str() {
            "blossom.publication_readiness_evidence.from_canonical_json.valid" => {
                let evidence = PublicationReadinessEvidence::from_canonical_json(&bytes)
                    .unwrap_or_else(|error| panic!("{}: {error}", vector.id));
                assert_eq!(
                    Some(evidence.schema_version()),
                    vector.expected.schema_version
                );
                assert_eq!(
                    Some(evidence.policy_version()),
                    vector.expected.policy_version
                );
                assert_eq!(evidence.to_canonical_json().unwrap(), bytes);
                assert!(vector.expected.error.is_none(), "{}", vector.id);
            }
            "blossom.publication_readiness_evidence.to_canonical_json.valid" => {
                let evidence = PublicationReadinessEvidence::from_canonical_json(&bytes)
                    .unwrap_or_else(|error| panic!("{}: {error}", vector.id));
                assert_eq!(evidence.to_canonical_json().unwrap(), bytes);
                assert_eq!(
                    Some(evidence.schema_version()),
                    vector.expected.schema_version
                );
                assert_eq!(
                    Some(evidence.policy_version()),
                    vector.expected.policy_version
                );
                assert!(vector.expected.error.is_none(), "{}", vector.id);
            }
            "blossom.publication_readiness_evidence.from_canonical_json.invalid" => {
                let error = PublicationReadinessEvidence::from_canonical_json(&bytes).unwrap_err();
                assert_eq!(
                    Some(error.code()),
                    vector.expected.error.as_deref(),
                    "{}",
                    vector.id
                );
                assert!(vector.expected.schema_version.is_none(), "{}", vector.id);
                assert!(vector.expected.policy_version.is_none(), "{}", vector.id);
            }
            kind => panic!("{} has unsupported vector kind {kind}", vector.id),
        }
    }
}

fn mutated_evidence(mutation: &str) -> Vec<u8> {
    let canonical = CANONICAL_EVIDENCE.as_bytes();
    match mutation {
        "none" => canonical.to_vec(),
        "leading_whitespace" => [b" ".as_slice(), canonical].concat(),
        "noncanonical_escaping" => replace_once(
            canonical,
            b"https://cdn.example/",
            br"https:\/\/cdn.example/",
        ),
        "reordered_fields" => replace_once(
            canonical,
            b"{\"schema_version\":1,\"policy_version\":1",
            b"{\"policy_version\":1,\"schema_version\":1",
        ),
        "unknown_field" => append_field(canonical, b",\"unknown\":true"),
        "bud11_authorization" => append_field(canonical, b",\"authorization\":\"Nostr token\""),
        "missing_uploaded" => replace_once(canonical, b",\"uploaded\":1800000001", b""),
        "duplicate_schema_version" => replace_once(canonical, b"{", b"{\"schema_version\":1,"),
        "size_wrong_type" => replace_once(canonical, b"\"size\":70", b"\"size\":\"70\""),
        "schema_version" => {
            replace_once(canonical, b"\"schema_version\":1", b"\"schema_version\":2")
        }
        "policy_version" => {
            replace_once(canonical, b"\"policy_version\":1", b"\"policy_version\":2")
        }
        "url_invalid" => replace_once(canonical, b"https://cdn.example/", b"http://cdn.example/"),
        "url_exact_max" => evidence_for_url(&url_with_length(
            RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES,
        )),
        "url_over_max" => evidence_for_url(&url_with_length(
            RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES + 1,
        )),
        "url_hash_mismatch" => replace_once(
            canonical,
            b"https://cdn.example/449013",
            b"https://cdn.example/049013",
        ),
        "sha256_invalid" => replace_once(canonical, b"\"sha256\":\"449013", b"\"sha256\":\"Z49013"),
        "size_zero" => replace_once(canonical, b"\"size\":70", b"\"size\":0"),
        "size_over_max" => replace_once(canonical, b"\"size\":70", b"\"size\":10485761"),
        "mime_unsupported" => replace_once(
            canonical,
            b"\"media_type\":\"image/png\"",
            b"\"media_type\":\"image/gif\"",
        ),
        "format_mismatch" => replace_once(
            canonical,
            b"\"raster_format\":\"png\"",
            b"\"raster_format\":\"jpeg\"",
        ),
        "width_zero" => replace_once(canonical, b"\"width\":1", b"\"width\":0"),
        "height_over_max" => replace_once(canonical, b"\"height\":1", b"\"height\":16385"),
        "pixels_over_max" => replace_once(
            canonical,
            b"\"width\":1,\"height\":1",
            b"\"width\":5000,\"height\":5000",
        ),
        "bud02_status" => replace_once(canonical, b"\"bud02_status\":201", b"\"bud02_status\":202"),
        "bud01_head_status" => replace_once(
            canonical,
            b"\"bud01_head_status\":200",
            b"\"bud01_head_status\":204",
        ),
        "bud01_get_status" => replace_once(
            canonical,
            b"\"bud01_get_status\":200",
            b"\"bud01_get_status\":206",
        ),
        "uploaded" => replace_once(
            canonical,
            b"\"uploaded\":1800000001",
            b"\"uploaded\":1800000002",
        ),
        "digest_invalid" => replace_once(
            canonical,
            b"\"evidence_digest\":\"637ac3",
            b"\"evidence_digest\":\"Z37ac3",
        ),
        "digest_mismatch" => replace_once(
            canonical,
            b"\"evidence_digest\":\"637ac3",
            b"\"evidence_digest\":\"037ac3",
        ),
        "oversized_input" => {
            vec![b' '; RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES + 1]
        }
        other => panic!("unsupported persistence mutation {other}"),
    }
}

fn url_with_length(length: usize) -> String {
    const HASH: &str = "4490130851783ff662845f5e72f1948618cc87f951f00f6c2ffb3dc01f3f40fd";
    let prefix = format!("https://cdn.example/{HASH}.");
    assert!(length >= prefix.len());
    let url = format!("{prefix}{}", "p".repeat(length - prefix.len()));
    assert_eq!(url.len(), length);
    url
}

fn evidence_for_url(url: &str) -> Vec<u8> {
    const HASH: &str = "4490130851783ff662845f5e72f1948618cc87f951f00f6c2ffb3dc01f3f40fd";
    let evidence = EvidenceWire {
        schema_version: 1,
        policy_version: 1,
        url,
        sha256: HASH,
        size: 70,
        media_type: "image/png",
        raster_format: "png",
        dimensions: EvidenceDimensionsWire {
            width: 1,
            height: 1,
        },
        bud02_status: 201,
        bud01_head_status: 200,
        bud01_get_status: 200,
        uploaded: 1_800_000_001,
        evidence_digest: readiness_evidence_digest(url, HASH),
    };
    serde_json::to_vec(&evidence).unwrap()
}

fn readiness_evidence_digest(url: &str, sha256: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"radroots.blossom.publication-readiness-evidence.v1\0");
    hasher.update(1_u16.to_be_bytes());
    update_length_prefixed(&mut hasher, url.as_bytes());
    hasher.update(hex::decode(sha256).unwrap());
    hasher.update(70_u64.to_be_bytes());
    update_length_prefixed(&mut hasher, b"image/png");
    hasher.update([2]);
    hasher.update(1_u32.to_be_bytes());
    hasher.update(1_u32.to_be_bytes());
    hasher.update(201_u16.to_be_bytes());
    hasher.update(200_u16.to_be_bytes());
    hasher.update(200_u16.to_be_bytes());
    hasher.update(1_800_000_001_u64.to_be_bytes());
    hex::encode(hasher.finalize())
}

fn update_length_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn append_field(canonical: &[u8], field: &[u8]) -> Vec<u8> {
    let mut output = canonical[..canonical.len() - 1].to_vec();
    output.extend_from_slice(field);
    output.push(b'}');
    output
}

fn replace_once(input: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let index = input
        .windows(from.len())
        .position(|candidate| candidate == from)
        .unwrap_or_else(|| panic!("missing mutation source {}", String::from_utf8_lossy(from)));
    let mut output = Vec::with_capacity(input.len() - from.len() + to.len());
    output.extend_from_slice(&input[..index]);
    output.extend_from_slice(to);
    output.extend_from_slice(&input[index + from.len()..]);
    output
}

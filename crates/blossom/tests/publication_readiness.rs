use radroots_blossom::{
    RadrootsBlossomApprovedBlobUrl, RadrootsBlossomAuthoredRasterDimensions,
    RadrootsBlossomBlobDescriptor, RadrootsBlossomBlobUrl, RadrootsBlossomBud01GetObservation,
    RadrootsBlossomBud01HeadObservation, RadrootsBlossomBud02UploadObservation,
    RadrootsBlossomError, RadrootsBlossomMediaType, RadrootsBlossomRasterDecodeObservation,
    RadrootsBlossomRasterDimensions, RadrootsBlossomRasterFormat, RadrootsBlossomSha256,
    verify_publication_readiness,
};
use serde::Deserialize;
use serde_json::Value;

const PACKAGED_VECTORS: &[u8] = include_bytes!("fixtures/publication_readiness.v1.json");

#[derive(Deserialize)]
struct VectorFile {
    vectors: Vec<Vector>,
}

#[derive(Deserialize)]
struct Vector {
    id: String,
    kind: String,
    input: Value,
    expected: Value,
}

#[test]
fn publication_readiness_vectors_execute_against_public_api() {
    let canonical = canonical_vectors();
    assert_eq!(canonical, PACKAGED_VECTORS, "packaged vector mirror drift");
    let vector_file: VectorFile = serde_json::from_slice(PACKAGED_VECTORS).unwrap();
    assert_eq!(vector_file.vectors.len(), 33);
    for vector in &vector_file.vectors {
        match vector.kind.as_str() {
            "blossom.verify_publication_readiness.valid" => execute_valid(vector),
            "blossom.verify_publication_readiness.invalid" => execute_invalid(vector),
            kind => panic!("{} has unsupported kind {kind}", vector.id),
        }
    }
}

fn canonical_vectors() -> &'static [u8] {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/conformance/vectors/blossom/publication_readiness.v1.json"
    );
    std::fs::read(path)
        .unwrap_or_else(|error| panic!("read canonical publication-readiness vectors: {error}"))
        .leak()
}

fn execute_valid(vector: &Vector) {
    let mutation = input_str(vector, "mutation");
    let result = run_mutation(vector, mutation).unwrap_or_else(|error| {
        panic!(
            "{} unexpectedly failed: {} ({})",
            vector.id,
            error,
            error.code()
        )
    });
    assert_eq!(
        result.url().as_str(),
        expected_str(vector, "url"),
        "{}",
        vector.id
    );
    assert_eq!(
        result.sha256().to_string(),
        expected_str(vector, "sha256"),
        "{}",
        vector.id
    );
    assert_eq!(result.size(), expected_u64(vector, "size"), "{}", vector.id);
    assert_eq!(
        result.media_type().as_str(),
        expected_str(vector, "media_type"),
        "{}",
        vector.id
    );
    assert_eq!(
        result.raster_format().as_str(),
        expected_str(vector, "format"),
        "{}",
        vector.id
    );
    assert_eq!(
        u64::from(result.dimensions().width()),
        expected_u64(vector, "width"),
        "{}",
        vector.id
    );
    assert_eq!(
        u64::from(result.dimensions().height()),
        expected_u64(vector, "height"),
        "{}",
        vector.id
    );
    assert_eq!(
        u64::from(result.bud02_status().as_u16()),
        expected_u64(vector, "upload_status"),
        "{}",
        vector.id
    );
    if let Some(expected_digest) = vector
        .expected
        .get("evidence_digest")
        .and_then(Value::as_str)
    {
        assert_eq!(
            result.evidence_digest().to_string(),
            expected_digest,
            "{}",
            vector.id
        );
    }
}

fn execute_invalid(vector: &Vector) {
    let mutation = input_str(vector, "mutation");
    let error =
        run_mutation(vector, mutation).expect_err("invalid publication-readiness vector must fail");
    assert_eq!(error.code(), expected_str(vector, "error"), "{}", vector.id);
}

fn run_mutation(
    vector: &Vector,
    mutation: &str,
) -> Result<radroots_blossom::RadrootsBlossomPublicationReadinessEvidence, RadrootsBlossomError> {
    let canonical = hex::decode(input_str(vector, "bytes_hex")).unwrap();
    let mut sealed_bytes = canonical.clone();
    let mut exact_authored_bytes = canonical.clone();
    let mut retrieved_bytes = canonical.clone();
    let mut media_type = "image/png";
    let mut upload_status = 201;
    let mut head_status = 200;
    let mut get_status = 200;
    let mut upload_origin = "https://cdn.example";
    let mut head_origin = "https://cdn.example";
    let mut get_origin = "https://cdn.example";
    let mut upload_hash_bytes = canonical.clone();
    let mut upload_size_delta = 0_i64;
    let mut upload_media_type = "image/png";
    let mut head_size_delta = 0_i64;
    let mut head_media_type = "image/png";
    let mut get_declared_size_delta = 0_i64;
    let mut decode_format = RadrootsBlossomRasterFormat::Png;
    let mut decode_hash_bytes = canonical.clone();
    let mut decode_size_delta = 0_i64;
    let mut frame_count = 1;
    let mut decoded_width = 1;
    let mut decoded_height = 1;
    let mut authored_dimensions = Some((1, 1));

    match mutation {
        "none" => {}
        "upload_status_200" => {
            upload_status = 200;
            authored_dimensions = None;
        }
        "upload_status_202" => upload_status = 202,
        "head_status_204" => head_status = 204,
        "get_status_206" => get_status = 206,
        "get_size_over_max" => get_declared_size_delta = 10_485_760,
        "get_body_missing" => retrieved_bytes.clear(),
        "get_body_short" => {
            retrieved_bytes.pop();
        }
        "get_body_trailing" => retrieved_bytes.push(0),
        "authored_bytes_short" => {
            exact_authored_bytes.pop();
        }
        "authored_bytes_wrong_hash" => exact_authored_bytes[69] ^= 1,
        "upload_url_mismatch" => upload_origin = "https://other.example",
        "upload_hash_mismatch" => upload_hash_bytes[69] ^= 1,
        "upload_size_mismatch" => upload_size_delta = 1,
        "upload_mime_mismatch" => upload_media_type = "image/jpeg",
        "head_url_mismatch" => head_origin = "https://other.example",
        "head_size_mismatch" => head_size_delta = 1,
        "head_mime_mismatch" => head_media_type = "image/jpeg",
        "get_url_mismatch" => get_origin = "https://other.example",
        "get_declared_size_mismatch" => {
            retrieved_bytes.pop();
            get_declared_size_delta = -1;
        }
        "get_bytes_wrong_hash" => retrieved_bytes[69] ^= 1,
        "unsupported_mime" => {
            media_type = "image/gif";
            upload_media_type = "image/gif";
            head_media_type = "image/gif";
        }
        "malformed_container" => {
            sealed_bytes[0] = 0;
            exact_authored_bytes = sealed_bytes.clone();
            retrieved_bytes = sealed_bytes.clone();
            upload_hash_bytes = sealed_bytes.clone();
            decode_hash_bytes = sealed_bytes.clone();
        }
        "animated_png" => {
            let iend = sealed_bytes.len() - 12;
            sealed_bytes.splice(
                iend..iend,
                [
                    0, 0, 0, 8, b'a', b'c', b'T', b'L', 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0,
                ],
            );
            exact_authored_bytes = sealed_bytes.clone();
            retrieved_bytes = sealed_bytes.clone();
            upload_hash_bytes = sealed_bytes.clone();
            decode_hash_bytes = sealed_bytes.clone();
        }
        "decode_format_mismatch" => decode_format = RadrootsBlossomRasterFormat::Jpeg,
        "decode_length_mismatch" => decode_size_delta = 1,
        "decode_hash_mismatch" => decode_hash_bytes[69] ^= 1,
        "decode_container_dimension_mismatch" => {
            decoded_width = 2;
            authored_dimensions = None;
        }
        "authored_dimension_mismatch" => authored_dimensions = Some((2, 1)),
        "decode_zero_frames" => frame_count = 0,
        "decode_zero_width" => decoded_width = 0,
        "decode_dimension_over_max" => decoded_width = 16_385,
        "decode_pixel_limit" => {
            decoded_width = 5_000;
            decoded_height = 5_000;
        }
        other => panic!("{} has unknown mutation {other}", vector.id),
    }

    if mutation == "get_size_over_max" {
        let url = approved_url(get_origin, &sealed_bytes);
        return RadrootsBlossomBud01GetObservation::from_complete_body(
            get_status,
            url,
            adjusted_size(sealed_bytes.len(), get_declared_size_delta),
            &retrieved_bytes,
        )
        .and_then(|_| unreachable_result());
    }

    let expected_media_type = RadrootsBlossomMediaType::parse(media_type).unwrap();
    let authored_descriptor = descriptor(
        "https://cdn.example",
        &sealed_bytes,
        sealed_bytes.len() as u64,
        media_type,
    )
    .approve_reference()?
    .verify_bytes(&sealed_bytes, &expected_media_type)?;

    let upload = RadrootsBlossomBud02UploadObservation::new(
        upload_status,
        descriptor(
            upload_origin,
            &upload_hash_bytes,
            adjusted_size(sealed_bytes.len(), upload_size_delta),
            upload_media_type,
        ),
    )?;
    let head = RadrootsBlossomBud01HeadObservation::new(
        head_status,
        approved_url(head_origin, &sealed_bytes),
        adjusted_size(sealed_bytes.len(), head_size_delta),
        RadrootsBlossomMediaType::parse(head_media_type).unwrap(),
    )?;
    let get = RadrootsBlossomBud01GetObservation::from_complete_body(
        get_status,
        approved_url(get_origin, &sealed_bytes),
        adjusted_size(sealed_bytes.len(), get_declared_size_delta),
        &retrieved_bytes,
    )?;
    let decode = RadrootsBlossomRasterDecodeObservation::new(
        decode_format,
        RadrootsBlossomSha256::digest(&decode_hash_bytes),
        adjusted_size(sealed_bytes.len(), decode_size_delta),
        frame_count,
        decoded_width,
        decoded_height,
    )?;
    let authored_dimensions = match authored_dimensions {
        Some((width, height)) => RadrootsBlossomAuthoredRasterDimensions::Exact(
            RadrootsBlossomRasterDimensions::new(width, height)?,
        ),
        None => RadrootsBlossomAuthoredRasterDimensions::Unspecified,
    };
    verify_publication_readiness(
        &authored_descriptor,
        &exact_authored_bytes,
        authored_dimensions,
        &upload,
        &head,
        &get,
        &decode,
    )
}

fn unreachable_result()
-> Result<radroots_blossom::RadrootsBlossomPublicationReadinessEvidence, RadrootsBlossomError> {
    unreachable!("oversized GET construction must fail")
}

fn descriptor(
    origin: &str,
    hash_bytes: &[u8],
    size: u64,
    media_type: &str,
) -> RadrootsBlossomBlobDescriptor {
    let hash = RadrootsBlossomSha256::digest(hash_bytes);
    RadrootsBlossomBlobDescriptor::new(
        RadrootsBlossomBlobUrl::parse(&format!("{origin}/{hash}.png")).unwrap(),
        hash,
        size,
        RadrootsBlossomMediaType::parse(media_type).unwrap(),
        1_800_000_000,
    )
    .unwrap()
}

fn approved_url(origin: &str, hash_bytes: &[u8]) -> RadrootsBlossomApprovedBlobUrl {
    let hash = RadrootsBlossomSha256::digest(hash_bytes);
    RadrootsBlossomBlobUrl::parse(&format!("{origin}/{hash}.png"))
        .unwrap()
        .approve()
        .unwrap()
}

fn adjusted_size(length: usize, delta: i64) -> u64 {
    u64::try_from(i64::try_from(length).unwrap() + delta).unwrap()
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

fn expected_u64(vector: &Vector, field: &str) -> u64 {
    vector.expected[field]
        .as_u64()
        .unwrap_or_else(|| panic!("{} expected.{field} must be an unsigned integer", vector.id))
}

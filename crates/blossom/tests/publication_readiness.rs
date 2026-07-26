#![cfg(feature = "raster-decode")]

use image::{ExtendedColorType, ImageEncoder, codecs::webp::WebPEncoder};
use radroots_blossom::{
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_SCHEMA_VERSION,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION,
    RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES, RadrootsBlossomApprovedBlobUrl,
    RadrootsBlossomAuthoredRasterDimensions, RadrootsBlossomBlobDescriptor, RadrootsBlossomBlobUrl,
    RadrootsBlossomBud01GetObservation, RadrootsBlossomBud01HeadObservation,
    RadrootsBlossomBud02UploadObservation, RadrootsBlossomError, RadrootsBlossomMediaType,
    RadrootsBlossomPublicationReadinessEvidence, RadrootsBlossomRasterDimensions,
    RadrootsBlossomRasterFormat, RadrootsBlossomSha256, verify_publication_readiness,
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
    assert_eq!(vector_file.vectors.len(), 37);
    for vector in &vector_file.vectors {
        match vector.kind.as_str() {
            "blossom.verify_publication_readiness.valid" => execute_valid(vector),
            "blossom.verify_publication_readiness.invalid" => execute_invalid(vector),
            kind => panic!("{} has unsupported kind {kind}", vector.id),
        }
    }
}

#[test]
fn publication_readiness_accepts_public_jpeg_and_still_webp() {
    for (bytes, media_type, extension, format) in [
        (
            encoded_jpeg(),
            "image/jpeg",
            "jpg",
            RadrootsBlossomRasterFormat::Jpeg,
        ),
        (
            encoded_still_webp(),
            "image/webp",
            "webp",
            RadrootsBlossomRasterFormat::StillWebP,
        ),
    ] {
        let bytes = bytes.as_slice();
        let evidence = verify_public_raster(
            bytes,
            media_type,
            extension,
            RadrootsBlossomAuthoredRasterDimensions::Exact(
                RadrootsBlossomRasterDimensions::new(1, 1).unwrap(),
            ),
        )
        .unwrap();

        assert_eq!(evidence.raster_format(), format);
        assert_eq!(evidence.raster_format().to_string(), format.as_str());
        assert_eq!(evidence.dimensions().pixels(), 1);
        assert_eq!(evidence.uploaded(), 1_800_000_001);
        assert_eq!(
            evidence.evidence_digest().as_sha256().to_string(),
            evidence.evidence_digest().to_string()
        );
    }
}

#[test]
fn publication_readiness_evidence_round_trips_only_through_strict_canonical_json() {
    let bytes = canonical_png();
    let evidence = verify_public_raster(
        &bytes,
        "image/png",
        "png",
        RadrootsBlossomAuthoredRasterDimensions::Exact(
            RadrootsBlossomRasterDimensions::new(1, 1).unwrap(),
        ),
    )
    .unwrap();
    let canonical = evidence.to_canonical_json().unwrap();
    assert!(canonical.len() <= RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES);

    let reloaded = RadrootsBlossomPublicationReadinessEvidence::from_canonical_json(&canonical)
        .expect("canonical evidence must reload");
    assert_eq!(reloaded, evidence);
    assert_eq!(
        reloaded.schema_version(),
        RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_SCHEMA_VERSION
    );
    assert_eq!(
        reloaded.policy_version(),
        RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION
    );
    assert_eq!(reloaded.to_canonical_json().unwrap(), canonical);

    let pretty = serde_json::to_vec_pretty(
        &serde_json::from_slice::<Value>(&canonical).expect("canonical evidence JSON"),
    )
    .unwrap();
    assert_eq!(
        RadrootsBlossomPublicationReadinessEvidence::from_canonical_json(&pretty)
            .unwrap_err()
            .code(),
        "publication_readiness_evidence_json_non_canonical"
    );

    let mut unknown = canonical.clone();
    unknown.pop();
    unknown.extend_from_slice(br#","authorization":"Nostr token"}"#);
    assert_eq!(
        RadrootsBlossomPublicationReadinessEvidence::from_canonical_json(&unknown)
            .unwrap_err()
            .code(),
        "publication_readiness_evidence_invalid_json"
    );

    let oversized = vec![b' '; RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES + 1];
    assert_eq!(
        RadrootsBlossomPublicationReadinessEvidence::from_canonical_json(&oversized)
            .unwrap_err()
            .code(),
        "publication_readiness_evidence_too_large"
    );
}

#[test]
fn publication_readiness_evidence_revalidates_every_persisted_fact() {
    let evidence = verify_public_raster(
        &canonical_png(),
        "image/png",
        "png",
        RadrootsBlossomAuthoredRasterDimensions::Unspecified,
    )
    .unwrap();
    let canonical = evidence.to_canonical_json().unwrap();

    let cases = [
        (
            "schema_version",
            serde_json::json!(2),
            "publication_readiness_evidence_schema_version_unsupported",
        ),
        (
            "policy_version",
            serde_json::json!(2),
            "publication_readiness_evidence_policy_version_unsupported",
        ),
        (
            "size",
            serde_json::json!(0),
            "publication_readiness_evidence_field_invalid",
        ),
        (
            "media_type",
            serde_json::json!("image/jpeg"),
            "publication_readiness_evidence_field_invalid",
        ),
        (
            "raster_format",
            serde_json::json!("jpeg"),
            "publication_readiness_evidence_field_invalid",
        ),
        (
            "bud02_status",
            serde_json::json!(202),
            "publication_readiness_evidence_field_invalid",
        ),
        (
            "bud01_head_status",
            serde_json::json!(204),
            "publication_readiness_evidence_field_invalid",
        ),
        (
            "bud01_get_status",
            serde_json::json!(206),
            "publication_readiness_evidence_field_invalid",
        ),
        (
            "uploaded",
            serde_json::json!(1_800_000_002_u64),
            "publication_readiness_evidence_digest_mismatch",
        ),
        (
            "evidence_digest",
            serde_json::json!("00".repeat(32)),
            "publication_readiness_evidence_digest_mismatch",
        ),
    ];
    for (field, value, expected) in cases {
        let mut wire: Value = serde_json::from_slice(&canonical).unwrap();
        wire[field] = value;
        let mutated = serde_json::to_vec(&wire).unwrap();
        assert_eq!(
            RadrootsBlossomPublicationReadinessEvidence::from_canonical_json(&mutated)
                .unwrap_err()
                .code(),
            expected,
            "{field}"
        );
    }

    let mut dimensions: Value = serde_json::from_slice(&canonical).unwrap();
    dimensions["dimensions"]["width"] = serde_json::json!(0);
    assert_eq!(
        RadrootsBlossomPublicationReadinessEvidence::from_canonical_json(
            &serde_json::to_vec(&dimensions).unwrap(),
        )
        .unwrap_err()
        .code(),
        "publication_readiness_evidence_field_invalid"
    );
}

#[test]
fn publication_readiness_enforces_nonempty_bytes_and_bounded_urls() {
    let empty_hash = RadrootsBlossomSha256::digest(&[]);
    let empty_url = format!("https://cdn.example/{empty_hash}.png");
    let empty_media_type = RadrootsBlossomMediaType::parse("image/png").unwrap();
    let empty_descriptor = RadrootsBlossomBlobDescriptor::new(
        RadrootsBlossomBlobUrl::parse(&empty_url).unwrap(),
        empty_hash,
        0,
        empty_media_type.clone(),
        1_800_000_000,
    )
    .unwrap()
    .approve_reference()
    .unwrap()
    .verify_bytes(&[], &empty_media_type)
    .unwrap();
    let empty_upload = RadrootsBlossomBud02UploadObservation::new(
        201,
        RadrootsBlossomBlobDescriptor::new(
            RadrootsBlossomBlobUrl::parse(&empty_url).unwrap(),
            empty_hash,
            0,
            empty_media_type.clone(),
            1_800_000_001,
        )
        .unwrap(),
    )
    .unwrap();
    let empty_approved_url = RadrootsBlossomBlobUrl::parse(&empty_url)
        .unwrap()
        .approve()
        .unwrap();
    let empty_head = RadrootsBlossomBud01HeadObservation::new(
        200,
        empty_approved_url.clone(),
        0,
        empty_media_type,
    )
    .unwrap();
    let nonempty_get =
        RadrootsBlossomBud01GetObservation::from_complete_body(200, empty_approved_url, 1, &[0])
            .unwrap();
    assert_eq!(
        verify_publication_readiness(
            &empty_descriptor,
            &[],
            RadrootsBlossomAuthoredRasterDimensions::Unspecified,
            &empty_upload,
            &empty_head,
            &nonempty_get,
        )
        .unwrap_err()
        .code(),
        "publication_raster_empty"
    );

    let bytes = canonical_png();
    let hash = RadrootsBlossomSha256::digest(&bytes);
    let prefix = format!("https://cdn.example/{hash}.");
    let exact_extension =
        "p".repeat(RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES - prefix.len());
    let evidence = verify_public_raster(
        &bytes,
        "image/png",
        &exact_extension,
        RadrootsBlossomAuthoredRasterDimensions::Unspecified,
    )
    .unwrap();
    assert_eq!(
        evidence.url().as_str().len(),
        RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES
    );

    let over_extension = format!("{exact_extension}p");
    assert_eq!(
        verify_public_raster(
            &bytes,
            "image/png",
            &over_extension,
            RadrootsBlossomAuthoredRasterDimensions::Unspecified,
        )
        .unwrap_err()
        .code(),
        "publication_readiness_url_too_large"
    );
}

#[test]
fn publication_readiness_rejects_forbidden_and_corrupt_jpeg_and_animated_rasters() {
    let jpeg = encoded_jpeg();
    let scan = jpeg
        .windows(2)
        .position(|window| window == b"\xff\xda")
        .unwrap();
    let segment_length = usize::from(u16::from_be_bytes([jpeg[scan + 2], jpeg[scan + 3]]));
    let entropy_start = scan + 2 + segment_length;
    let eoi = jpeg.len() - 2;
    assert!(entropy_start < eoi);
    let entropy_length = eoi - entropy_start;
    for keep in [0, 1, entropy_length / 2, entropy_length - 1] {
        let mut truncated = jpeg[..entropy_start + keep].to_vec();
        truncated.extend_from_slice(b"\xff\xd9");
        assert_eq!(
            verify_public_raster(
                &truncated,
                "image/jpeg",
                "jpg",
                RadrootsBlossomAuthoredRasterDimensions::Unspecified,
            )
            .unwrap_err()
            .code(),
            "publication_raster_decode_failed"
        );
    }

    let mut malformed_dqt = jpeg.clone();
    let sof = malformed_dqt
        .windows(2)
        .position(|window| window == b"\xff\xc0")
        .unwrap();
    malformed_dqt.drain(sof - 3..sof);
    assert_eq!(
        verify_public_raster(
            &malformed_dqt,
            "image/jpeg",
            "jpg",
            RadrootsBlossomAuthoredRasterDimensions::Unspecified,
        )
        .unwrap_err()
        .code(),
        "invalid_publication_raster"
    );

    let mut progressive = jpeg;
    progressive[sof + 1] = 0xc2;
    assert_eq!(
        verify_public_raster(
            &progressive,
            "image/jpeg",
            "jpg",
            RadrootsBlossomAuthoredRasterDimensions::Unspecified,
        )
        .unwrap_err()
        .code(),
        "publication_jpeg_process_forbidden"
    );

    assert_eq!(
        verify_public_raster(
            &encoded_animated_png(),
            "image/png",
            "png",
            RadrootsBlossomAuthoredRasterDimensions::Unspecified,
        )
        .unwrap_err()
        .code(),
        "publication_raster_animation_forbidden"
    );

    assert_eq!(
        verify_public_raster(
            &encoded_animated_webp(),
            "image/webp",
            "webp",
            RadrootsBlossomAuthoredRasterDimensions::Unspecified,
        )
        .unwrap_err()
        .code(),
        "publication_raster_animation_forbidden"
    );
}

fn encoded_jpeg() -> Vec<u8> {
    hex::decode(
        "ffd8ffe000104a46494600010100000100010000ffdb0043000302020302020303030304030304050805050404050a070706080c0a0c0c0b0a0b0b0d0e12100d0e110e0b0b1016101113141515150c0f171816141812141514ffdb00430103040405040509050509140d0b0d1414141414141414141414141414141414141414141414141414141414141414141414141414141414141414141414141414ffc00011080001000103012200021101031101ffc4001f0000010501010101010100000000000000000102030405060708090a0bffc400b5100002010303020403050504040000017d01020300041105122131410613516107227114328191a1082342b1c11552d1f02433627282090a161718191a25262728292a3435363738393a434445464748494a535455565758595a636465666768696a737475767778797a838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae1e2e3e4e5e6e7e8e9eaf1f2f3f4f5f6f7f8f9faffc4001f0100030101010101010101010000000000000102030405060708090a0bffc400b51100020102040403040705040400010277000102031104052131061241510761711322328108144291a1b1c109233352f0156272d10a162434e125f11718191a262728292a35363738393a434445464748494a535455565758595a636465666768696a737475767778797a82838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae2e3e4e5e6e7e8e9eaf2f3f4f5f6f7f8f9faffda000c03010002110311003f00f9ca8a28afc3cfe6f3ffd9",
    )
    .unwrap()
}

fn canonical_png() -> Vec<u8> {
    hex::decode(
        "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c6360f8cff0000003e201e03810ac1e0000000049454e44ae426082",
    )
    .unwrap()
}

fn encoded_still_webp() -> Vec<u8> {
    let mut bytes = Vec::new();
    WebPEncoder::new_lossless(&mut bytes)
        .write_image(&[0, 128, 0, 255], 1, 1, ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

fn encoded_animated_png() -> Vec<u8> {
    let canonical = hex::decode(
        "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c6360f8cff0000003e201e03810ac1e0000000049454e44ae426082",
    )
    .unwrap();
    let mut output = b"\x89PNG\r\n\x1a\n".to_vec();
    output.extend_from_slice(&png_chunk(*b"IHDR", &canonical[16..29]));
    output.extend_from_slice(&png_chunk(*b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0]));
    output.extend_from_slice(&png_chunk(*b"fcTL", &apng_frame_control(0)));
    output.extend_from_slice(&png_chunk(*b"IDAT", &canonical[41..54]));
    output.extend_from_slice(&png_chunk(*b"fcTL", &apng_frame_control(1)));

    let mut frame_data = 2_u32.to_be_bytes().to_vec();
    frame_data.extend_from_slice(&canonical[41..54]);
    output.extend_from_slice(&png_chunk(*b"fdAT", &frame_data));
    output.extend_from_slice(&png_chunk(*b"IEND", &[]));
    output
}

fn apng_frame_control(sequence: u32) -> [u8; 26] {
    let mut control = [0_u8; 26];
    control[..4].copy_from_slice(&sequence.to_be_bytes());
    control[4..8].copy_from_slice(&1_u32.to_be_bytes());
    control[8..12].copy_from_slice(&1_u32.to_be_bytes());
    control[20..22].copy_from_slice(&1_u16.to_be_bytes());
    control[22..24].copy_from_slice(&10_u16.to_be_bytes());
    control
}

fn encoded_animated_webp() -> Vec<u8> {
    let still = encoded_still_webp();
    let mut output = b"RIFF\0\0\0\0WEBP".to_vec();
    let mut extended_header = [0_u8; 10];
    extended_header[0] = 0x02;
    push_webp_chunk(&mut output, *b"VP8X", &extended_header);
    push_webp_chunk(&mut output, *b"ANIM", &[0; 6]);

    let mut frame = [0_u8; 16].to_vec();
    frame[12] = 1;
    frame.extend_from_slice(&still[12..]);
    push_webp_chunk(&mut output, *b"ANMF", &frame);
    let riff_size = (output.len() as u32) - 8;
    output[4..8].copy_from_slice(&riff_size.to_le_bytes());
    output
}

fn push_webp_chunk(output: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    output.extend_from_slice(&kind);
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    output.extend_from_slice(data);
    if data.len() & 1 == 1 {
        output.push(0);
    }
}

fn verify_public_raster(
    bytes: &[u8],
    media_type: &str,
    extension: &str,
    authored_dimensions: RadrootsBlossomAuthoredRasterDimensions,
) -> Result<radroots_blossom::RadrootsBlossomPublicationReadinessEvidence, RadrootsBlossomError> {
    let hash = RadrootsBlossomSha256::digest(bytes);
    let url = format!("https://cdn.example/{hash}.{extension}");
    let media_type = RadrootsBlossomMediaType::parse(media_type).unwrap();
    let authored_descriptor = RadrootsBlossomBlobDescriptor::new(
        RadrootsBlossomBlobUrl::parse(&url).unwrap(),
        hash,
        bytes.len() as u64,
        media_type.clone(),
        1_800_000_000,
    )
    .unwrap()
    .approve_reference()
    .unwrap()
    .verify_bytes(bytes, &media_type)
    .unwrap();
    let upload = RadrootsBlossomBud02UploadObservation::new(
        201,
        RadrootsBlossomBlobDescriptor::new(
            RadrootsBlossomBlobUrl::parse(&url).unwrap(),
            hash,
            bytes.len() as u64,
            media_type.clone(),
            1_800_000_001,
        )
        .unwrap(),
    )
    .unwrap();
    let approved_url = RadrootsBlossomBlobUrl::parse(&url)
        .unwrap()
        .approve()
        .unwrap();
    let head = RadrootsBlossomBud01HeadObservation::new(
        200,
        approved_url.clone(),
        bytes.len() as u64,
        media_type,
    )
    .unwrap();
    let get = RadrootsBlossomBud01GetObservation::from_complete_body(
        200,
        approved_url,
        bytes.len() as u64,
        bytes,
    )
    .unwrap();
    verify_publication_readiness(
        &authored_descriptor,
        bytes,
        authored_dimensions,
        &upload,
        &head,
        &get,
    )
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
        }
        "animated_png" => {
            sealed_bytes = encoded_animated_png();
            exact_authored_bytes = sealed_bytes.clone();
            retrieved_bytes = sealed_bytes.clone();
            upload_hash_bytes = sealed_bytes.clone();
        }
        "declared_mime_jpeg" => {
            media_type = "image/jpeg";
            upload_media_type = "image/jpeg";
            head_media_type = "image/jpeg";
        }
        "corrupt_png_crc" => {
            sealed_bytes[57] ^= 1;
            exact_authored_bytes = sealed_bytes.clone();
            retrieved_bytes = sealed_bytes.clone();
            upload_hash_bytes = sealed_bytes.clone();
        }
        "corrupt_png_deflate" => {
            sealed_bytes[41] = 0;
            let crc = png_crc(*b"IDAT", &sealed_bytes[41..54]);
            sealed_bytes[54..58].copy_from_slice(&crc.to_be_bytes());
            exact_authored_bytes = sealed_bytes.clone();
            retrieved_bytes = sealed_bytes.clone();
            upload_hash_bytes = sealed_bytes.clone();
        }
        "invalid_png_color_type" => {
            sealed_bytes[25] = 1;
            let crc = png_crc(*b"IHDR", &sealed_bytes[16..29]);
            sealed_bytes[29..33].copy_from_slice(&crc.to_be_bytes());
            exact_authored_bytes = sealed_bytes.clone();
            retrieved_bytes = sealed_bytes.clone();
            upload_hash_bytes = sealed_bytes.clone();
        }
        "authored_dimension_mismatch" => authored_dimensions = Some((2, 1)),
        "animated_webp" => {
            sealed_bytes = encoded_animated_webp();
            exact_authored_bytes = sealed_bytes.clone();
            retrieved_bytes = sealed_bytes.clone();
            upload_hash_bytes = sealed_bytes.clone();
            media_type = "image/webp";
            upload_media_type = "image/webp";
            head_media_type = "image/webp";
            authored_dimensions = None;
        }
        "zero_width" => {
            sealed_bytes[16..20].copy_from_slice(&0_u32.to_be_bytes());
            exact_authored_bytes = sealed_bytes.clone();
            retrieved_bytes = sealed_bytes.clone();
            upload_hash_bytes = sealed_bytes.clone();
        }
        "dimension_over_max" => {
            sealed_bytes[16..20].copy_from_slice(&16_385_u32.to_be_bytes());
            exact_authored_bytes = sealed_bytes.clone();
            retrieved_bytes = sealed_bytes.clone();
            upload_hash_bytes = sealed_bytes.clone();
        }
        "pixel_limit" => {
            sealed_bytes[16..20].copy_from_slice(&5_000_u32.to_be_bytes());
            sealed_bytes[20..24].copy_from_slice(&5_000_u32.to_be_bytes());
            exact_authored_bytes = sealed_bytes.clone();
            retrieved_bytes = sealed_bytes.clone();
            upload_hash_bytes = sealed_bytes.clone();
        }
        "progressive_jpeg" => {
            let mut jpeg = encoded_jpeg();
            let sof = jpeg
                .windows(2)
                .position(|window| window == b"\xff\xc0")
                .unwrap();
            jpeg[sof + 1] = 0xc2;
            replace_raster_bytes(
                &mut sealed_bytes,
                &mut exact_authored_bytes,
                &mut retrieved_bytes,
                &mut upload_hash_bytes,
                jpeg,
            );
            media_type = "image/jpeg";
            upload_media_type = "image/jpeg";
            head_media_type = "image/jpeg";
            authored_dimensions = None;
        }
        "jpeg_entropy_stripped" | "jpeg_entropy_partial" => {
            let jpeg = encoded_jpeg();
            let scan = jpeg
                .windows(2)
                .position(|window| window == b"\xff\xda")
                .unwrap();
            let segment_length = usize::from(u16::from_be_bytes([jpeg[scan + 2], jpeg[scan + 3]]));
            let entropy_start = scan + 2 + segment_length;
            let entropy_length = jpeg.len() - 2 - entropy_start;
            let keep = if mutation == "jpeg_entropy_stripped" {
                0
            } else {
                entropy_length / 2
            };
            let mut truncated = jpeg[..entropy_start + keep].to_vec();
            truncated.extend_from_slice(b"\xff\xd9");
            replace_raster_bytes(
                &mut sealed_bytes,
                &mut exact_authored_bytes,
                &mut retrieved_bytes,
                &mut upload_hash_bytes,
                truncated,
            );
            media_type = "image/jpeg";
            upload_media_type = "image/jpeg";
            head_media_type = "image/jpeg";
            authored_dimensions = None;
        }
        "malformed_jpeg_dqt" => {
            let mut jpeg = encoded_jpeg();
            let sof = jpeg
                .windows(2)
                .position(|window| window == b"\xff\xc0")
                .unwrap();
            jpeg.drain(sof - 3..sof);
            replace_raster_bytes(
                &mut sealed_bytes,
                &mut exact_authored_bytes,
                &mut retrieved_bytes,
                &mut upload_hash_bytes,
                jpeg,
            );
            media_type = "image/jpeg";
            upload_media_type = "image/jpeg";
            head_media_type = "image/jpeg";
            authored_dimensions = None;
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
    )
}

fn replace_raster_bytes(
    sealed_bytes: &mut Vec<u8>,
    exact_authored_bytes: &mut Vec<u8>,
    retrieved_bytes: &mut Vec<u8>,
    upload_hash_bytes: &mut Vec<u8>,
    replacement: Vec<u8>,
) {
    *sealed_bytes = replacement;
    exact_authored_bytes.clone_from(sealed_bytes);
    retrieved_bytes.clone_from(sealed_bytes);
    upload_hash_bytes.clone_from(sealed_bytes);
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

fn png_chunk(kind: [u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::new();
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(&kind);
    chunk.extend_from_slice(data);
    chunk.extend_from_slice(&png_crc(kind, data).to_be_bytes());
    chunk
}

fn png_crc(kind: [u8; 4], data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in kind.iter().chain(data) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
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

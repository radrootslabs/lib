#![cfg(feature = "raster-decode")]

use radroots_blossom::{
    RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES,
    RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES, RadrootsBlossomAuthoredRasterDimensions,
    RadrootsBlossomBlobDescriptor, RadrootsBlossomBlobUrl, RadrootsBlossomBud01GetObservation,
    RadrootsBlossomBud01HeadObservation, RadrootsBlossomBud02UploadObservation,
    RadrootsBlossomError, RadrootsBlossomMediaType, RadrootsBlossomSha256,
    verify_publication_readiness,
};
use serde::Deserialize;
use std::{env, fs, io::Write, path::PathBuf, process::Command};
use tempfile::Builder;

const FIXTURE: &str = include_str!("fixtures/raster_decoder_security.v1.json");
const RESOURCE_CASE_ENV: &str = "RADROOTS_DECODER_RESOURCE_CASE";
const RESOURCE_FIXTURE_ROOT_ENV: &str = "RADROOTS_DECODER_RESOURCE_FIXTURE_ROOT";
const RESOURCE_AXIS_CASE_ENV: &str = "RADROOTS_DECODER_RESOURCE_AXIS_CASE";
const RESOURCE_WIDTH: u32 = 5_000;
const RESOURCE_HEIGHT: u32 = 4_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResourceProbeCase {
    JpegGrayscale,
    JpegRgb,
    JpegCmyk,
    JpegSof1,
    PngRgb,
    PngPalette,
    PngRgba,
    PngAdam7,
    WebpVp8Rgb,
    WebpVp8Alpha,
    WebpVp8lRgb,
    WebpVp8lAlpha,
}

impl ResourceProbeCase {
    const ALL: [Self; 12] = [
        Self::JpegGrayscale,
        Self::JpegRgb,
        Self::JpegCmyk,
        Self::JpegSof1,
        Self::PngRgb,
        Self::PngPalette,
        Self::PngRgba,
        Self::PngAdam7,
        Self::WebpVp8Rgb,
        Self::WebpVp8Alpha,
        Self::WebpVp8lRgb,
        Self::WebpVp8lAlpha,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::JpegGrayscale => "jpeg_grayscale",
            Self::JpegRgb => "jpeg_rgb",
            Self::JpegCmyk => "jpeg_cmyk",
            Self::JpegSof1 => "jpeg_sof1",
            Self::PngRgb => "png_rgb",
            Self::PngPalette => "png_palette",
            Self::PngRgba => "png_rgba",
            Self::PngAdam7 => "png_adam7",
            Self::WebpVp8Rgb => "webp_vp8_rgb",
            Self::WebpVp8Alpha => "webp_vp8_alpha",
            Self::WebpVp8lRgb => "webp_vp8l_rgb",
            Self::WebpVp8lAlpha => "webp_vp8l_alpha",
        }
    }

    const fn fixture_name(self) -> &'static str {
        match self {
            Self::JpegGrayscale => "jpeg_grayscale.jpg",
            Self::JpegRgb => "jpeg_rgb.jpg",
            Self::JpegCmyk => "jpeg_cmyk.jpg",
            Self::JpegSof1 => "jpeg_sof1.jpg",
            Self::PngRgb => "png_rgb.png",
            Self::PngPalette => "png_palette.png",
            Self::PngRgba => "png_rgba.png",
            Self::PngAdam7 => "png_adam7.png",
            Self::WebpVp8Rgb => "webp_vp8_rgb.webp",
            Self::WebpVp8Alpha => "webp_vp8_alpha.webp",
            Self::WebpVp8lRgb => "webp_vp8l_rgb.webp",
            Self::WebpVp8lAlpha => "webp_vp8l_alpha.webp",
        }
    }

    const fn format(self) -> &'static str {
        match self {
            Self::JpegGrayscale | Self::JpegRgb | Self::JpegCmyk | Self::JpegSof1 => "jpeg",
            Self::PngRgb | Self::PngPalette | Self::PngRgba | Self::PngAdam7 => "png",
            Self::WebpVp8Rgb | Self::WebpVp8Alpha | Self::WebpVp8lRgb | Self::WebpVp8lAlpha => {
                "webp"
            }
        }
    }

    const fn logical_decoded_bytes(self) -> u64 {
        match self {
            Self::JpegGrayscale
            | Self::JpegRgb
            | Self::JpegCmyk
            | Self::JpegSof1
            | Self::PngRgb
            | Self::PngPalette => 60_000_000,
            Self::PngRgba
            | Self::PngAdam7
            | Self::WebpVp8Rgb
            | Self::WebpVp8Alpha
            | Self::WebpVp8lRgb
            | Self::WebpVp8lAlpha => 80_000_000,
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|case| case.id() == id)
    }

    fn validate_process(self, bytes: &[u8]) {
        match self {
            Self::JpegGrayscale => assert_eq!(jpeg_process(bytes), (0xc0, 1)),
            Self::JpegRgb => assert_eq!(jpeg_process(bytes), (0xc0, 3)),
            Self::JpegCmyk => assert_eq!(jpeg_process(bytes), (0xc0, 4)),
            Self::JpegSof1 => assert_eq!(jpeg_process(bytes), (0xc1, 3)),
            Self::PngRgb => assert_eq!(png_process(bytes), (2, 0)),
            Self::PngPalette => assert_eq!(png_process(bytes), (3, 0)),
            Self::PngRgba => assert_eq!(png_process(bytes), (6, 0)),
            Self::PngAdam7 => assert_eq!(png_process(bytes), (6, 1)),
            Self::WebpVp8Rgb => assert_eq!(webp_process(bytes), (*b"VP8 ", false)),
            Self::WebpVp8Alpha => assert_eq!(webp_process(bytes), (*b"VP8 ", true)),
            Self::WebpVp8lRgb => assert_eq!(webp_process(bytes), (*b"VP8L", false)),
            Self::WebpVp8lAlpha => assert_eq!(webp_process(bytes), (*b"VP8L", true)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AxisProbeCase {
    Width,
    Height,
}

impl AxisProbeCase {
    fn from_id(id: &str) -> Option<Self> {
        match id {
            "width_16384" => Some(Self::Width),
            "height_16384" => Some(Self::Height),
            _ => None,
        }
    }

    const fn fixture_name(self) -> &'static str {
        match self {
            Self::Width => "axis_width_16384.png",
            Self::Height => "axis_height_16384.png",
        }
    }

    const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Width => (16_384, 1),
            Self::Height => (1, 16_384),
        }
    }
}

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
    input: VectorInput,
    expected: VectorExpected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorInput {
    format: String,
    bytes_hex: String,
    mutation: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorExpected {
    accepted: bool,
    width: Option<u32>,
    height: Option<u32>,
    error: Option<String>,
}

fn suite() -> Suite {
    serde_json::from_str(FIXTURE).expect("decoder-security fixture must parse")
}

fn media(format: &str) -> (&'static str, &'static str) {
    match format {
        "jpeg" => ("image/jpeg", "jpg"),
        "png" => ("image/png", "png"),
        "webp" => ("image/webp", "webp"),
        other => panic!("unsupported fixture format {other}"),
    }
}

fn png_chunk(kind: [u8; 4], data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len() + 12);
    output.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
    output.extend_from_slice(&kind);
    output.extend_from_slice(data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    output.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    output
}

fn verify(bytes: &[u8], format: &str) -> Result<(u32, u32), RadrootsBlossomError> {
    let (media_type, extension) = media(format);
    let hash = RadrootsBlossomSha256::digest(bytes);
    let url = format!("https://cdn.example/{hash}.{extension}");
    let media_type = RadrootsBlossomMediaType::parse(media_type).unwrap();
    let descriptor = RadrootsBlossomBlobDescriptor::new(
        RadrootsBlossomBlobUrl::parse(&url).unwrap(),
        hash,
        bytes.len() as u64,
        media_type.clone(),
        1_800_000_000,
    )?;
    let authored = descriptor
        .clone()
        .approve_reference()?
        .verify_bytes(bytes, &media_type)?;
    let upload = RadrootsBlossomBud02UploadObservation::new(201, descriptor)?;
    let approved_url = RadrootsBlossomBlobUrl::parse(&url)?.approve()?;
    let head = RadrootsBlossomBud01HeadObservation::new(
        200,
        approved_url.clone(),
        bytes.len() as u64,
        media_type,
    )?;
    let get = RadrootsBlossomBud01GetObservation::from_complete_body(
        200,
        approved_url,
        bytes.len() as u64,
        bytes,
    )?;
    let evidence = verify_publication_readiness(
        &authored,
        bytes,
        RadrootsBlossomAuthoredRasterDimensions::Unspecified,
        &upload,
        &head,
        &get,
    )?;
    Ok((
        evidence.dimensions().width(),
        evidence.dimensions().height(),
    ))
}

#[test]
fn decoder_regression_corpus_executes_every_case() {
    let suite = suite();
    assert_eq!(suite.suite, "blossom_raster_decoder_security");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_eq!(suite.vectors.len(), 30);
    for vector in suite.vectors {
        assert!(!vector.input.mutation.is_empty());
        let bytes = hex::decode(&vector.input.bytes_hex).unwrap();
        let result = verify(&bytes, &vector.input.format);
        if vector.expected.accepted {
            let (width, height) = result.unwrap_or_else(|error| {
                panic!("{} unexpectedly failed with {}", vector.id, error.code())
            });
            assert_eq!(
                vector.kind,
                "blossom.verify_publication_readiness.decoder_security.accepted"
            );
            assert_eq!(Some(width), vector.expected.width, "{} width", vector.id);
            assert_eq!(Some(height), vector.expected.height, "{} height", vector.id);
            assert!(vector.expected.error.is_none(), "{} error", vector.id);
        } else {
            let error = result.expect_err(&format!("{} unexpectedly passed", vector.id));
            assert_eq!(
                vector.kind,
                "blossom.verify_publication_readiness.decoder_security.rejected"
            );
            assert_eq!(
                Some(error.code()),
                vector.expected.error.as_deref(),
                "{} error",
                vector.id
            );
            assert!(vector.expected.width.is_none(), "{} width", vector.id);
            assert!(vector.expected.height.is_none(), "{} height", vector.id);
        }
    }
}

#[test]
#[ignore = "requires the Nix-pinned independent ImageMagick decoder"]
fn decoder_differential_matches_independent_backend() {
    let executable = env::var("RADROOTS_INDEPENDENT_RASTER_DECODER")
        .expect("RADROOTS_INDEPENDENT_RASTER_DECODER must name the pinned magick executable");
    for vector in suite().vectors {
        if !vector.expected.accepted {
            continue;
        }
        let bytes = hex::decode(&vector.input.bytes_hex).unwrap();
        let (width, height) = verify(&bytes, &vector.input.format).unwrap();
        let (_, extension) = media(&vector.input.format);
        let mut file = Builder::new()
            .suffix(&format!(".{extension}"))
            .tempfile()
            .unwrap();
        file.write_all(&bytes).unwrap();
        file.flush().unwrap();

        let identify = Command::new(&executable)
            .args([
                "identify",
                "-format",
                "%m %w %h %n\\n",
                file.path().to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(identify.status.success(), "{} identify failed", vector.id);
        let output = String::from_utf8(identify.stdout).unwrap();
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 1, "{} frame count", vector.id);
        let fields = lines[0].split_ascii_whitespace().collect::<Vec<_>>();
        assert_eq!(fields.len(), 4, "{} identify fields", vector.id);
        assert_eq!(fields[0].to_ascii_lowercase(), vector.input.format);
        assert_eq!(fields[1].parse::<u32>().unwrap(), width);
        assert_eq!(fields[2].parse::<u32>().unwrap(), height);
        assert_eq!(fields[3], "1");

        let decoded = Command::new(&executable)
            .args([
                file.path().to_str().unwrap(),
                "-alpha",
                "on",
                "-depth",
                "8",
                "rgba:-",
            ])
            .output()
            .unwrap();
        assert!(decoded.status.success(), "{} decode failed", vector.id);
        assert_eq!(
            decoded.stdout.len(),
            usize::try_from(u64::from(width) * u64::from(height) * 4).unwrap(),
            "{} decoded byte count",
            vector.id
        );
    }
}

#[test]
#[ignore = "executed in isolation by the governed peak-RSS lane"]
fn maximum_resource_probe() {
    let case_id = env::var(RESOURCE_CASE_ENV).expect("resource case must be selected");
    let case = ResourceProbeCase::from_id(&case_id).expect("resource case must be governed");
    let bytes = resource_fixture(case.fixture_name());
    assert_eq!(
        RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES,
        80_000_000
    );
    assert!(!bytes.is_empty());
    assert!(bytes.len() as u64 <= RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES);
    assert!(case.logical_decoded_bytes() <= RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES);
    case.validate_process(&bytes);
    assert_eq!(
        verify(&bytes, case.format()).unwrap(),
        (RESOURCE_WIDTH, RESOURCE_HEIGHT)
    );
}

#[test]
#[ignore = "executed with prepared fixtures by the governed axis-boundary lane"]
fn axis_resource_probe() {
    let case_id = env::var(RESOURCE_AXIS_CASE_ENV).expect("axis case must be selected");
    let case = AxisProbeCase::from_id(&case_id).expect("axis case must be governed");
    let bytes = resource_fixture(case.fixture_name());
    assert!(!bytes.is_empty());
    assert!(bytes.len() as u64 <= RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES);
    assert_eq!(png_process(&bytes), (2, 0));
    assert_eq!(verify(&bytes, "png").unwrap(), case.dimensions());
}

#[test]
fn resource_probe_inventory_is_closed() {
    assert_eq!(
        ResourceProbeCase::ALL.map(ResourceProbeCase::id),
        [
            "jpeg_grayscale",
            "jpeg_rgb",
            "jpeg_cmyk",
            "jpeg_sof1",
            "png_rgb",
            "png_palette",
            "png_rgba",
            "png_adam7",
            "webp_vp8_rgb",
            "webp_vp8_alpha",
            "webp_vp8l_rgb",
            "webp_vp8l_alpha",
        ]
    );
    for case in ResourceProbeCase::ALL {
        assert_eq!(ResourceProbeCase::from_id(case.id()), Some(case));
        assert!(case.logical_decoded_bytes() > 0);
        assert!(
            case.logical_decoded_bytes() <= RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES
        );
    }
    assert!(ResourceProbeCase::from_id("png_gray").is_none());
    assert!(AxisProbeCase::from_id("width_16384").is_some());
    assert!(AxisProbeCase::from_id("height_16384").is_some());
    assert!(AxisProbeCase::from_id("width_16385").is_none());
}

#[test]
fn encoded_byte_boundary_executes_the_public_operation() {
    let base = suite()
        .vectors
        .into_iter()
        .find(|vector| vector.id == "png_rgb_8bit")
        .expect("PNG RGB vector must exist");
    assert_eq!(base.input.mutation, "none");
    let exact = padded_png(
        &hex::decode(base.input.bytes_hex).unwrap(),
        RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES as usize,
    );
    assert_eq!(
        exact.len() as u64,
        RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES
    );
    assert_eq!(
        verify(&exact, "png").unwrap(),
        (base.expected.width.unwrap(), base.expected.height.unwrap())
    );

    let mut one_over = exact;
    one_over.push(0);
    assert_eq!(
        verify(&one_over, "png").unwrap_err().code(),
        "publication_raster_byte_limit_exceeded"
    );
}

fn resource_fixture_root() -> PathBuf {
    env::var_os(RESOURCE_FIXTURE_ROOT_ENV)
        .map(PathBuf::from)
        .expect("resource fixture root must be supplied")
}

fn resource_fixture(name: &str) -> Vec<u8> {
    fs::read(resource_fixture_root().join(name))
        .expect("prepared resource fixture must be readable")
}

fn jpeg_process(bytes: &[u8]) -> (u8, u8) {
    let frames = bytes
        .windows(10)
        .filter_map(|window| {
            (window[0] == 0xff && is_jpeg_start_of_frame(window[1]))
                .then_some((window[1], window[4], window[9]))
        })
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 1);
    let (process, precision, components) = frames[0];
    assert_eq!(precision, 8);
    (process, components)
}

fn is_jpeg_start_of_frame(marker: u8) -> bool {
    matches!(
        marker,
        0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf
    )
}

fn png_process(bytes: &[u8]) -> (u8, u8) {
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"));
    assert_eq!(bytes[24], 8);
    (bytes[25], bytes[28])
}

fn webp_process(bytes: &[u8]) -> ([u8; 4], bool) {
    assert!(bytes.len() >= 20);
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WEBP");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8,
        bytes.len()
    );

    let mut position = 12_usize;
    let mut primary = None;
    let mut vp8x_alpha = false;
    let mut alpha_chunk = false;
    let mut vp8l_alpha = false;
    while position < bytes.len() {
        let kind: [u8; 4] = bytes[position..position + 4].try_into().unwrap();
        let length =
            u32::from_le_bytes(bytes[position + 4..position + 8].try_into().unwrap()) as usize;
        let data_start = position + 8;
        let data_end = data_start + length;
        let data = &bytes[data_start..data_end];
        position = data_end + (length & 1);
        match &kind {
            b"VP8X" => vp8x_alpha = data[0] & 0x10 != 0,
            b"ALPH" => alpha_chunk = true,
            b"VP8 " => assert!(primary.replace(kind).is_none()),
            b"VP8L" => {
                assert!(primary.replace(kind).is_none());
                let bits = u32::from_le_bytes(data[1..5].try_into().unwrap());
                vp8l_alpha = bits & (1 << 28) != 0;
            }
            _ => {}
        }
    }
    assert_eq!(position, bytes.len());
    let primary = primary.expect("WebP primary chunk must exist");
    let alpha = if primary == *b"VP8L" {
        vp8l_alpha
    } else {
        assert_eq!(vp8x_alpha, alpha_chunk);
        vp8x_alpha
    };
    (primary, alpha)
}

fn padded_png(base: &[u8], target_length: usize) -> Vec<u8> {
    assert!(base.len() >= 12);
    assert!(base.ends_with(&png_chunk(*b"IEND", &[])));
    let padding_length = target_length
        .checked_sub(base.len() + 12)
        .expect("target length must leave room for an ancillary chunk");
    let iend_start = base.len() - 12;
    let mut output = Vec::with_capacity(target_length);
    output.extend_from_slice(&base[..iend_start]);
    output.extend_from_slice(&png_chunk(*b"raDr", &vec![0; padding_length]));
    output.extend_from_slice(&base[iend_start..]);
    assert_eq!(output.len(), target_length);
    output
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

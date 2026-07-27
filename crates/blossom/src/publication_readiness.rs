#[cfg(feature = "serde")]
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
#[cfg(feature = "raster-decode")]
use image::{ImageDecoder, Limits, codecs::png::PngDecoder};
#[cfg(feature = "raster-decode")]
use libwebp::{WebPDecodeRGBAInto, WebPGetInfo};
#[cfg(any(feature = "raster-decode", feature = "serde"))]
use sha2::{Digest, Sha256};
#[cfg(feature = "raster-decode")]
use std::io::Cursor;
#[cfg(feature = "raster-decode")]
use zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};
#[cfg(feature = "raster-decode")]
use zune_jpeg::JpegDecoder as StrictJpegDecoder;

#[cfg(feature = "raster-decode")]
mod sequential_jpeg;

#[cfg(feature = "serde")]
use crate::RadrootsBlossomBlobUrl;
#[cfg(feature = "raster-decode")]
use crate::RadrootsBlossomByteVerifiedDescriptor;
use crate::{
    RadrootsBlossomApprovedBlobUrl, RadrootsBlossomBlobDescriptor, RadrootsBlossomError,
    RadrootsBlossomMediaType, RadrootsBlossomSha256,
};

const _: () = assert!(usize::BITS <= u64::BITS);

pub const RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION: u16 = 1;
pub const RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_SCHEMA_VERSION: u32 = 1;
pub const RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES: usize = 8 * 1024;
pub const RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES: usize = 4 * 1024;
pub const RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES: u64 = 10_485_760;
pub const RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES: u64 = 80_000_000;
pub const RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION: u32 = 16_384;
pub const RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS: u64 = 20_000_000;

#[cfg(any(feature = "raster-decode", test))]
const PUBLICATION_RASTER_MAX_CONTAINER_RECORDS: usize = 65_536;

#[cfg(any(feature = "raster-decode", feature = "serde"))]
const READINESS_EVIDENCE_DIGEST_DOMAIN: &[u8] =
    b"radroots.blossom.publication-readiness-evidence.v1\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RadrootsBlossomBud02UploadStatus {
    Ok,
    Created,
}

impl RadrootsBlossomBud02UploadStatus {
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::Created => 201,
        }
    }

    fn parse(status: u16) -> Result<Self, RadrootsBlossomError> {
        match status {
            200 => Ok(Self::Ok),
            201 => Ok(Self::Created),
            actual => Err(RadrootsBlossomError::InvalidBud02UploadStatus { actual }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RadrootsBlossomRasterFormat {
    Jpeg,
    Png,
    StillWebP,
}

impl RadrootsBlossomRasterFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::StillWebP => "still_webp",
        }
    }

    pub fn from_media_type(
        media_type: &RadrootsBlossomMediaType,
    ) -> Result<Self, RadrootsBlossomError> {
        match media_type.as_str() {
            "image/jpeg" => Ok(Self::Jpeg),
            "image/png" => Ok(Self::Png),
            "image/webp" => Ok(Self::StillWebP),
            _ => Err(RadrootsBlossomError::UnsupportedPublicationRasterMediaType),
        }
    }

    #[cfg(any(feature = "raster-decode", feature = "serde"))]
    const fn digest_code(self) -> u8 {
        match self {
            Self::Jpeg => 1,
            Self::Png => 2,
            Self::StillWebP => 3,
        }
    }
}

impl fmt::Display for RadrootsBlossomRasterFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RadrootsBlossomRasterDimensions {
    width: u32,
    height: u32,
}

impl RadrootsBlossomRasterDimensions {
    pub fn new(width: u32, height: u32) -> Result<Self, RadrootsBlossomError> {
        if width == 0
            || height == 0
            || width > RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION
            || height > RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION
        {
            return Err(
                RadrootsBlossomError::PublicationRasterDimensionsOutOfRange { width, height },
            );
        }
        let pixels = u64::from(width) * u64::from(height);
        if pixels > RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS {
            return Err(RadrootsBlossomError::PublicationRasterPixelLimitExceeded { pixels });
        }
        Ok(Self { width, height })
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }

    pub const fn pixels(self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RadrootsBlossomAuthoredRasterDimensions {
    Unspecified,
    Exact(RadrootsBlossomRasterDimensions),
}

impl RadrootsBlossomAuthoredRasterDimensions {
    #[cfg(feature = "raster-decode")]
    const fn exact(self) -> Option<RadrootsBlossomRasterDimensions> {
        match self {
            Self::Unspecified => None,
            Self::Exact(dimensions) => Some(dimensions),
        }
    }
}

/// A successful BUD-02 response descriptor observed by a transport adapter.
///
/// Construction accepts only status 200 or 201 and applies the public URL
/// approval policy. It does not represent BUD-11 authorization or entitlement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsBlossomBud02UploadObservation {
    status: RadrootsBlossomBud02UploadStatus,
    descriptor: crate::RadrootsBlossomApprovedDescriptor,
}

impl RadrootsBlossomBud02UploadObservation {
    pub fn new(
        status: u16,
        descriptor: RadrootsBlossomBlobDescriptor,
    ) -> Result<Self, RadrootsBlossomError> {
        Ok(Self {
            status: RadrootsBlossomBud02UploadStatus::parse(status)?,
            descriptor: descriptor.approve_reference()?,
        })
    }

    pub const fn status(&self) -> RadrootsBlossomBud02UploadStatus {
        self.status
    }

    pub fn descriptor(&self) -> &crate::RadrootsBlossomApprovedDescriptor {
        &self.descriptor
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsBlossomBud01HeadObservation {
    url: RadrootsBlossomApprovedBlobUrl,
    content_length: u64,
    media_type: RadrootsBlossomMediaType,
}

impl RadrootsBlossomBud01HeadObservation {
    pub fn new(
        status: u16,
        url: RadrootsBlossomApprovedBlobUrl,
        content_length: u64,
        media_type: RadrootsBlossomMediaType,
    ) -> Result<Self, RadrootsBlossomError> {
        if status != 200 {
            return Err(RadrootsBlossomError::InvalidBud01HeadStatus { actual: status });
        }
        Ok(Self {
            url,
            content_length,
            media_type,
        })
    }

    pub fn url(&self) -> &RadrootsBlossomApprovedBlobUrl {
        &self.url
    }

    pub const fn content_length(&self) -> u64 {
        self.content_length
    }

    pub fn media_type(&self) -> &RadrootsBlossomMediaType {
        &self.media_type
    }
}

pub struct RadrootsBlossomBud01GetCollector {
    url: RadrootsBlossomApprovedBlobUrl,
    declared_size: u64,
    bytes: Vec<u8>,
}

impl RadrootsBlossomBud01GetCollector {
    pub fn new(
        status: u16,
        url: RadrootsBlossomApprovedBlobUrl,
        declared_size: u64,
    ) -> Result<Self, RadrootsBlossomError> {
        if status != 200 {
            return Err(RadrootsBlossomError::InvalidBud01GetStatus { actual: status });
        }
        if declared_size > RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES {
            return Err(RadrootsBlossomError::PublicationRasterByteLimitExceeded {
                declared: declared_size,
                maximum: RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES,
            });
        }
        let capacity = usize::try_from(declared_size)
            .map_err(|_| RadrootsBlossomError::PublicationGetBodyAllocationFailed)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| RadrootsBlossomError::PublicationGetBodyAllocationFailed)?;
        Ok(Self {
            url,
            declared_size,
            bytes,
        })
    }

    pub fn push_chunk(&mut self, chunk: &[u8]) -> Result<(), RadrootsBlossomError> {
        let actual = self
            .bytes
            .len()
            .checked_add(chunk.len())
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(RadrootsBlossomError::PublicationGetBodyLengthOverflow)?;
        if actual > self.declared_size {
            return Err(RadrootsBlossomError::PublicationGetBodyTrailing {
                declared: self.declared_size,
                actual,
            });
        }
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }

    pub fn finish(self) -> Result<RadrootsBlossomBud01GetObservation, RadrootsBlossomError> {
        let actual = self.bytes.len() as u64;
        if actual == 0 {
            return Err(RadrootsBlossomError::PublicationGetBodyMissing);
        }
        if actual < self.declared_size {
            return Err(RadrootsBlossomError::PublicationGetBodyShort {
                declared: self.declared_size,
                actual,
            });
        }
        Ok(RadrootsBlossomBud01GetObservation {
            url: self.url,
            declared_size: self.declared_size,
            bytes: self.bytes,
        })
    }
}

pub struct RadrootsBlossomBud01GetObservation {
    url: RadrootsBlossomApprovedBlobUrl,
    declared_size: u64,
    bytes: Vec<u8>,
}

impl RadrootsBlossomBud01GetObservation {
    pub fn from_complete_body(
        status: u16,
        url: RadrootsBlossomApprovedBlobUrl,
        declared_size: u64,
        bytes: &[u8],
    ) -> Result<Self, RadrootsBlossomError> {
        let mut collector = RadrootsBlossomBud01GetCollector::new(status, url, declared_size)?;
        collector.push_chunk(bytes)?;
        collector.finish()
    }

    pub fn url(&self) -> &RadrootsBlossomApprovedBlobUrl {
        &self.url
    }

    pub const fn declared_size(&self) -> u64 {
        self.declared_size
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for RadrootsBlossomBud01GetObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RadrootsBlossomBud01GetObservation")
            .field("url", &self.url)
            .field("declared_size", &self.declared_size)
            .field("body_length", &self.bytes.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RadrootsBlossomPublicationReadinessEvidenceDigest(RadrootsBlossomSha256);

impl RadrootsBlossomPublicationReadinessEvidenceDigest {
    pub const fn as_sha256(self) -> RadrootsBlossomSha256 {
        self.0
    }
}

impl fmt::Display for RadrootsBlossomPublicationReadinessEvidenceDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsBlossomPublicationReadinessEvidence {
    url: RadrootsBlossomApprovedBlobUrl,
    sha256: RadrootsBlossomSha256,
    size: u64,
    media_type: RadrootsBlossomMediaType,
    raster_format: RadrootsBlossomRasterFormat,
    dimensions: RadrootsBlossomRasterDimensions,
    bud02_status: RadrootsBlossomBud02UploadStatus,
    uploaded: u64,
    evidence_digest: RadrootsBlossomPublicationReadinessEvidenceDigest,
}

impl RadrootsBlossomPublicationReadinessEvidence {
    pub const fn schema_version(&self) -> u32 {
        RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_SCHEMA_VERSION
    }

    pub const fn policy_version(&self) -> u16 {
        RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION
    }

    pub fn url(&self) -> &RadrootsBlossomApprovedBlobUrl {
        &self.url
    }

    pub const fn sha256(&self) -> RadrootsBlossomSha256 {
        self.sha256
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub fn media_type(&self) -> &RadrootsBlossomMediaType {
        &self.media_type
    }

    pub const fn raster_format(&self) -> RadrootsBlossomRasterFormat {
        self.raster_format
    }

    pub const fn dimensions(&self) -> RadrootsBlossomRasterDimensions {
        self.dimensions
    }

    pub const fn bud02_status(&self) -> RadrootsBlossomBud02UploadStatus {
        self.bud02_status
    }

    pub const fn uploaded(&self) -> u64 {
        self.uploaded
    }

    pub const fn evidence_digest(&self) -> RadrootsBlossomPublicationReadinessEvidenceDigest {
        self.evidence_digest
    }

    #[cfg(feature = "serde")]
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, RadrootsBlossomError> {
        serialize_readiness_evidence(self)
    }

    #[cfg(feature = "serde")]
    pub fn from_canonical_json(bytes: &[u8]) -> Result<Self, RadrootsBlossomError> {
        if bytes.len() > RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES {
            return Err(RadrootsBlossomError::PublicationReadinessEvidenceTooLarge {
                max: RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES,
                actual: bytes.len(),
            });
        }
        let wire: PublicationReadinessEvidenceWire = serde_json::from_slice(bytes)
            .map_err(|_| RadrootsBlossomError::PublicationReadinessEvidenceInvalidJson)?;
        let evidence = readiness_evidence_from_wire(wire)?;
        if serialize_readiness_evidence(&evidence)? != bytes {
            return Err(RadrootsBlossomError::PublicationReadinessEvidenceNonCanonicalJson);
        }
        Ok(evidence)
    }
}

#[cfg(feature = "raster-decode")]
pub fn verify_publication_readiness(
    authored_descriptor: &RadrootsBlossomByteVerifiedDescriptor,
    exact_authored_bytes: &[u8],
    authored_dimensions: RadrootsBlossomAuthoredRasterDimensions,
    upload: &RadrootsBlossomBud02UploadObservation,
    head: &RadrootsBlossomBud01HeadObservation,
    get: &RadrootsBlossomBud01GetObservation,
) -> Result<RadrootsBlossomPublicationReadinessEvidence, RadrootsBlossomError> {
    let expected_url = authored_descriptor.url();
    let expected_hash = authored_descriptor.sha256();
    let expected_size = authored_descriptor.size();
    let expected_media_type = authored_descriptor.media_type();

    if expected_url.as_str().len() > RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES {
        return Err(RadrootsBlossomError::PublicationReadinessUrlTooLarge {
            max: RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES,
            actual: expected_url.as_str().len(),
        });
    }
    if expected_size == 0 {
        return Err(RadrootsBlossomError::PublicationRasterEmpty);
    }
    if expected_size > RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES {
        return Err(RadrootsBlossomError::PublicationRasterByteLimitExceeded {
            declared: expected_size,
            maximum: RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES,
        });
    }
    let authored_size = exact_authored_bytes.len() as u64;
    if authored_size != expected_size {
        return Err(RadrootsBlossomError::PublicationAuthoredBytesSizeMismatch {
            expected: expected_size,
            actual: authored_size,
        });
    }
    if RadrootsBlossomSha256::digest(exact_authored_bytes) != expected_hash {
        return Err(RadrootsBlossomError::PublicationAuthoredBytesHashMismatch);
    }

    let upload_descriptor = upload.descriptor().descriptor();
    if upload_descriptor.sha256() != expected_hash {
        return Err(RadrootsBlossomError::PublicationUploadHashMismatch);
    }
    if upload.descriptor().url() != expected_url {
        return Err(RadrootsBlossomError::PublicationUploadUrlMismatch);
    }
    if upload_descriptor.size() != expected_size {
        return Err(RadrootsBlossomError::PublicationUploadSizeMismatch {
            expected: expected_size,
            actual: upload_descriptor.size(),
        });
    }
    if upload_descriptor.media_type() != expected_media_type {
        return Err(RadrootsBlossomError::PublicationUploadMediaTypeMismatch);
    }

    if head.url() != expected_url {
        return Err(RadrootsBlossomError::PublicationHeadUrlMismatch);
    }
    if head.content_length() != expected_size {
        return Err(RadrootsBlossomError::PublicationHeadSizeMismatch {
            expected: expected_size,
            actual: head.content_length(),
        });
    }
    if head.media_type() != expected_media_type {
        return Err(RadrootsBlossomError::PublicationHeadMediaTypeMismatch);
    }

    if get.url() != expected_url {
        return Err(RadrootsBlossomError::PublicationGetUrlMismatch);
    }
    if get.declared_size() != expected_size {
        return Err(RadrootsBlossomError::PublicationGetDeclaredSizeMismatch {
            expected: expected_size,
            actual: get.declared_size(),
        });
    }
    validate_retrieved_body(
        expected_hash,
        exact_authored_bytes,
        get.bytes(),
        RadrootsBlossomSha256::digest(get.bytes()),
    )?;

    let expected_format = RadrootsBlossomRasterFormat::from_media_type(expected_media_type)?;
    let decoded_dimensions = decode_raster(get.bytes(), expected_format)?;
    if authored_dimensions
        .exact()
        .is_some_and(|dimensions| dimensions != decoded_dimensions)
    {
        return Err(RadrootsBlossomError::PublicationAuthoredRasterDimensionMismatch);
    }

    let evidence_digest = evidence_digest_from_facts(&ReadinessEvidenceDigestFacts {
        url: expected_url,
        sha256: expected_hash,
        size: expected_size,
        media_type: expected_media_type,
        format: expected_format,
        dimensions: decoded_dimensions,
        bud02_status: upload.status(),
        uploaded: upload_descriptor.uploaded(),
    });
    Ok(RadrootsBlossomPublicationReadinessEvidence {
        url: expected_url.clone(),
        sha256: expected_hash,
        size: expected_size,
        media_type: expected_media_type.clone(),
        raster_format: expected_format,
        dimensions: decoded_dimensions,
        bud02_status: upload.status(),
        uploaded: upload_descriptor.uploaded(),
        evidence_digest,
    })
}

#[cfg(feature = "raster-decode")]
fn decode_raster(
    bytes: &[u8],
    format: RadrootsBlossomRasterFormat,
) -> Result<RadrootsBlossomRasterDimensions, RadrootsBlossomError> {
    match format {
        RadrootsBlossomRasterFormat::Jpeg => {
            let container = inspect_jpeg_container(bytes)?;
            decode_complete_jpeg(bytes, container)
        }
        RadrootsBlossomRasterFormat::Png => {
            let container = inspect_png_container(bytes)?;
            let decoder = PngDecoder::with_limits(Cursor::new(bytes), raster_decode_limits())
                .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
            let decoder_animated = decoder
                .is_apng()
                .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
            reject_animation(container.animated, decoder_animated)?;
            decode_complete_raster(decoder, container.dimensions)
        }
        RadrootsBlossomRasterFormat::StillWebP => {
            let container = inspect_webp_container(bytes)?;
            reject_animation(container.animated, false)?;
            decode_complete_webp(bytes, container.dimensions)
        }
    }
}

#[cfg(feature = "raster-decode")]
fn decode_complete_webp(
    bytes: &[u8],
    container_dimensions: RadrootsBlossomRasterDimensions,
) -> Result<RadrootsBlossomRasterDimensions, RadrootsBlossomError> {
    let (width, height) =
        WebPGetInfo(bytes).map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    let dimensions = RadrootsBlossomRasterDimensions::new(width, height)?;
    require_matching_dimensions(dimensions, container_dimensions)?;

    let decoded_length = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4));
    let decoded_bytes = bounded_decoded_byte_length(decoded_length)?;
    let stride = width
        .checked_mul(4)
        .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    let mut decoded = allocate_decoded_buffer(decoded_bytes)?;
    WebPDecodeRGBAInto(bytes, &mut decoded, stride)
        .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    Ok(dimensions)
}

#[cfg(feature = "raster-decode")]
fn decode_complete_jpeg(
    bytes: &[u8],
    container: JpegContainerInspection,
) -> Result<RadrootsBlossomRasterDimensions, RadrootsBlossomError> {
    sequential_jpeg::validate(bytes, container)?;
    let mut decoder =
        StrictJpegDecoder::new_with_options(ZCursor::new(bytes), strict_jpeg_decoder_options());
    decoder
        .decode_headers()
        .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    let dimensions = strict_jpeg_dimensions(decoder.dimensions())?;
    require_matching_dimensions(dimensions, container.dimensions)?;
    let decoded_bytes = bounded_jpeg_output_buffer_size(decoder.output_buffer_size())?;
    let mut decoded = allocate_decoded_buffer(decoded_bytes)?;
    decoder
        .decode_into(&mut decoded)
        .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    Ok(dimensions)
}

#[cfg(feature = "raster-decode")]
fn strict_jpeg_decoder_options() -> DecoderOptions {
    DecoderOptions::default()
        .set_strict_mode(true)
        .set_use_unsafe(false)
        .set_max_width(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION as usize)
        .set_max_height(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION as usize)
        .jpeg_set_out_colorspace(ColorSpace::RGB)
}

#[cfg(feature = "raster-decode")]
fn strict_jpeg_dimensions(
    dimensions: Option<(usize, usize)>,
) -> Result<RadrootsBlossomRasterDimensions, RadrootsBlossomError> {
    let (width, height) = dimensions.ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    let width =
        u32::try_from(width).map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    let height =
        u32::try_from(height).map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    RadrootsBlossomRasterDimensions::new(width, height)
}

#[cfg(feature = "raster-decode")]
fn decode_complete_raster<D: ImageDecoder>(
    mut decoder: D,
    container_dimensions: RadrootsBlossomRasterDimensions,
) -> Result<RadrootsBlossomRasterDimensions, RadrootsBlossomError> {
    let (width, height) = decoder.dimensions();
    let dimensions = RadrootsBlossomRasterDimensions::new(width, height)?;
    require_matching_dimensions(dimensions, container_dimensions)?;

    decoder
        .set_limits(raster_decode_limits())
        .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    let decoded_bytes = bounded_decoded_byte_length(Some(decoder.total_bytes()))?;
    let mut decoded = allocate_decoded_buffer(decoded_bytes)?;
    decoder
        .read_image(&mut decoded)
        .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    Ok(dimensions)
}

#[cfg(feature = "raster-decode")]
fn require_matching_dimensions(
    decoded: RadrootsBlossomRasterDimensions,
    container: RadrootsBlossomRasterDimensions,
) -> Result<(), RadrootsBlossomError> {
    if decoded != container {
        return Err(RadrootsBlossomError::PublicationRasterContainerDimensionMismatch);
    }
    Ok(())
}

#[cfg(feature = "raster-decode")]
fn bounded_jpeg_output_buffer_size(
    decoded_bytes: Option<usize>,
) -> Result<u64, RadrootsBlossomError> {
    bounded_decoded_byte_length(decoded_bytes.map(|decoded_bytes| decoded_bytes as u64))
}

#[cfg(feature = "raster-decode")]
fn bounded_decoded_byte_length(decoded_bytes: Option<u64>) -> Result<u64, RadrootsBlossomError> {
    let decoded_bytes = decoded_bytes.ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    if decoded_bytes > RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES {
        return Err(
            RadrootsBlossomError::PublicationRasterDecodedByteLimitExceeded {
                decoded: decoded_bytes,
                maximum: RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES,
            },
        );
    }
    Ok(decoded_bytes)
}

#[cfg(feature = "raster-decode")]
fn reject_animation(
    container_animated: bool,
    decoder_animated: bool,
) -> Result<(), RadrootsBlossomError> {
    if container_animated || decoder_animated {
        return Err(RadrootsBlossomError::PublicationRasterAnimationForbidden);
    }
    Ok(())
}

#[cfg(feature = "raster-decode")]
fn allocate_decoded_buffer(decoded_bytes: u64) -> Result<Vec<u8>, RadrootsBlossomError> {
    #[cfg(target_pointer_width = "64")]
    let decoded_length = decoded_bytes as usize;
    #[cfg(not(target_pointer_width = "64"))]
    let decoded_length = usize::try_from(decoded_bytes)
        .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeAllocationFailed)?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(decoded_length)
        .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeAllocationFailed)?;
    decoded.resize(decoded_length, 0);
    Ok(decoded)
}

#[cfg(feature = "raster-decode")]
fn raster_decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION);
    limits.max_image_height = Some(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION);
    limits.max_alloc = Some(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES);
    limits
}

#[cfg(feature = "raster-decode")]
fn validate_retrieved_body(
    expected_hash: RadrootsBlossomSha256,
    exact_authored_bytes: &[u8],
    retrieved_bytes: &[u8],
    retrieved_hash: RadrootsBlossomSha256,
) -> Result<(), RadrootsBlossomError> {
    if retrieved_hash != expected_hash {
        return Err(RadrootsBlossomError::PublicationRetrievedBytesHashMismatch);
    }
    if retrieved_bytes != exact_authored_bytes {
        return Err(RadrootsBlossomError::PublicationRetrievedBytesMismatch);
    }
    Ok(())
}

#[cfg(any(feature = "raster-decode", feature = "serde"))]
struct ReadinessEvidenceDigestFacts<'a> {
    url: &'a RadrootsBlossomApprovedBlobUrl,
    sha256: RadrootsBlossomSha256,
    size: u64,
    media_type: &'a RadrootsBlossomMediaType,
    format: RadrootsBlossomRasterFormat,
    dimensions: RadrootsBlossomRasterDimensions,
    bud02_status: RadrootsBlossomBud02UploadStatus,
    uploaded: u64,
}

#[cfg(any(feature = "raster-decode", feature = "serde"))]
fn evidence_digest_from_facts(
    facts: &ReadinessEvidenceDigestFacts<'_>,
) -> RadrootsBlossomPublicationReadinessEvidenceDigest {
    let mut hasher = Sha256::new();
    hasher.update(READINESS_EVIDENCE_DIGEST_DOMAIN);
    hasher.update(RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION.to_be_bytes());
    update_length_prefixed(&mut hasher, facts.url.as_str().as_bytes());
    hasher.update(facts.sha256.as_bytes());
    hasher.update(facts.size.to_be_bytes());
    update_length_prefixed(&mut hasher, facts.media_type.as_str().as_bytes());
    hasher.update([facts.format.digest_code()]);
    hasher.update(facts.dimensions.width().to_be_bytes());
    hasher.update(facts.dimensions.height().to_be_bytes());
    hasher.update(facts.bud02_status.as_u16().to_be_bytes());
    hasher.update(200_u16.to_be_bytes());
    hasher.update(200_u16.to_be_bytes());
    hasher.update(facts.uploaded.to_be_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest);
    RadrootsBlossomPublicationReadinessEvidenceDigest(RadrootsBlossomSha256::from_bytes(bytes))
}

#[cfg(any(feature = "raster-decode", feature = "serde"))]
fn update_length_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationReadinessDimensionsWire {
    width: u32,
    height: u32,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationReadinessEvidenceWire {
    schema_version: u32,
    policy_version: u16,
    url: String,
    sha256: String,
    size: u64,
    media_type: String,
    raster_format: String,
    dimensions: PublicationReadinessDimensionsWire,
    bud02_status: u16,
    bud01_head_status: u16,
    bud01_get_status: u16,
    uploaded: u64,
    evidence_digest: String,
}

#[cfg(feature = "serde")]
fn serialize_readiness_evidence(
    evidence: &RadrootsBlossomPublicationReadinessEvidence,
) -> Result<Vec<u8>, RadrootsBlossomError> {
    let wire = PublicationReadinessEvidenceWire {
        schema_version: RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_SCHEMA_VERSION,
        policy_version: RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION,
        url: evidence.url.as_str().to_string(),
        sha256: evidence.sha256.to_string(),
        size: evidence.size,
        media_type: evidence.media_type.as_str().to_string(),
        raster_format: evidence.raster_format.as_str().to_string(),
        dimensions: PublicationReadinessDimensionsWire {
            width: evidence.dimensions.width(),
            height: evidence.dimensions.height(),
        },
        bud02_status: evidence.bud02_status.as_u16(),
        bud01_head_status: 200,
        bud01_get_status: 200,
        uploaded: evidence.uploaded,
        evidence_digest: evidence.evidence_digest.to_string(),
    };
    let bytes = serde_json::to_vec(&wire)
        .map_err(|_| RadrootsBlossomError::PublicationReadinessEvidenceSerialization)?;
    if bytes.len() > RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES {
        return Err(RadrootsBlossomError::PublicationReadinessEvidenceTooLarge {
            max: RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES,
            actual: bytes.len(),
        });
    }
    Ok(bytes)
}

#[cfg(feature = "serde")]
fn readiness_evidence_from_wire(
    wire: PublicationReadinessEvidenceWire,
) -> Result<RadrootsBlossomPublicationReadinessEvidence, RadrootsBlossomError> {
    if wire.schema_version != RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_SCHEMA_VERSION {
        return Err(
            RadrootsBlossomError::PublicationReadinessEvidenceUnsupportedSchemaVersion {
                expected: RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_SCHEMA_VERSION,
                actual: wire.schema_version,
            },
        );
    }
    if wire.policy_version != RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION {
        return Err(
            RadrootsBlossomError::PublicationReadinessEvidenceUnsupportedPolicyVersion {
                expected: RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION,
                actual: wire.policy_version,
            },
        );
    }
    if wire.url.len() > RADROOTS_BLOSSOM_PUBLICATION_READINESS_URL_MAX_BYTES {
        return Err(
            RadrootsBlossomError::PublicationReadinessEvidenceInvalidField { field: "url" },
        );
    }
    if wire.size == 0 || wire.size > RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES {
        return Err(
            RadrootsBlossomError::PublicationReadinessEvidenceInvalidField { field: "size" },
        );
    }
    let sha256 = RadrootsBlossomSha256::from_hex(&wire.sha256).map_err(|_| {
        RadrootsBlossomError::PublicationReadinessEvidenceInvalidField { field: "sha256" }
    })?;
    let media_type = RadrootsBlossomMediaType::parse(&wire.media_type).map_err(|_| {
        RadrootsBlossomError::PublicationReadinessEvidenceInvalidField {
            field: "media_type",
        }
    })?;
    if media_type.as_str() != wire.media_type {
        return Err(
            RadrootsBlossomError::PublicationReadinessEvidenceInvalidField {
                field: "media_type",
            },
        );
    }
    let raster_format = match wire.raster_format.as_str() {
        "jpeg" => RadrootsBlossomRasterFormat::Jpeg,
        "png" => RadrootsBlossomRasterFormat::Png,
        "still_webp" => RadrootsBlossomRasterFormat::StillWebP,
        _ => {
            return Err(
                RadrootsBlossomError::PublicationReadinessEvidenceInvalidField {
                    field: "raster_format",
                },
            );
        }
    };
    if RadrootsBlossomRasterFormat::from_media_type(&media_type).map_err(|_| {
        RadrootsBlossomError::PublicationReadinessEvidenceInvalidField {
            field: "media_type",
        }
    })? != raster_format
    {
        return Err(
            RadrootsBlossomError::PublicationReadinessEvidenceInvalidField {
                field: "raster_format",
            },
        );
    }
    let dimensions =
        RadrootsBlossomRasterDimensions::new(wire.dimensions.width, wire.dimensions.height)
            .map_err(
                |_| RadrootsBlossomError::PublicationReadinessEvidenceInvalidField {
                    field: "dimensions",
                },
            )?;
    let bud02_status =
        RadrootsBlossomBud02UploadStatus::parse(wire.bud02_status).map_err(|_| {
            RadrootsBlossomError::PublicationReadinessEvidenceInvalidField {
                field: "bud02_status",
            }
        })?;
    if wire.bud01_head_status != 200 {
        return Err(
            RadrootsBlossomError::PublicationReadinessEvidenceInvalidField {
                field: "bud01_head_status",
            },
        );
    }
    if wire.bud01_get_status != 200 {
        return Err(
            RadrootsBlossomError::PublicationReadinessEvidenceInvalidField {
                field: "bud01_get_status",
            },
        );
    }
    let blob_url = RadrootsBlossomBlobUrl::parse(&wire.url).map_err(|_| {
        RadrootsBlossomError::PublicationReadinessEvidenceInvalidField { field: "url" }
    })?;
    if blob_url.as_str() != wire.url {
        return Err(
            RadrootsBlossomError::PublicationReadinessEvidenceInvalidField { field: "url" },
        );
    }
    let approved = RadrootsBlossomBlobDescriptor::new(
        blob_url,
        sha256,
        wire.size,
        media_type.clone(),
        wire.uploaded,
    )
    .and_then(RadrootsBlossomBlobDescriptor::approve_reference)
    .map_err(|_| RadrootsBlossomError::PublicationReadinessEvidenceInvalidField { field: "url" })?;
    let evidence_digest = RadrootsBlossomPublicationReadinessEvidenceDigest(
        RadrootsBlossomSha256::from_hex(&wire.evidence_digest).map_err(|_| {
            RadrootsBlossomError::PublicationReadinessEvidenceInvalidField {
                field: "evidence_digest",
            }
        })?,
    );
    let expected_digest = evidence_digest_from_facts(&ReadinessEvidenceDigestFacts {
        url: approved.url(),
        sha256,
        size: wire.size,
        media_type: &media_type,
        format: raster_format,
        dimensions,
        bud02_status,
        uploaded: wire.uploaded,
    });
    if evidence_digest != expected_digest {
        return Err(RadrootsBlossomError::PublicationReadinessEvidenceDigestMismatch);
    }
    Ok(RadrootsBlossomPublicationReadinessEvidence {
        url: approved.url().clone(),
        sha256,
        size: wire.size,
        media_type,
        raster_format,
        dimensions,
        bud02_status,
        uploaded: wire.uploaded,
        evidence_digest,
    })
}

#[cfg(any(feature = "raster-decode", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RasterContainerInspection {
    dimensions: RadrootsBlossomRasterDimensions,
    animated: bool,
}

#[cfg(any(feature = "raster-decode", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct JpegContainerInspection {
    dimensions: RadrootsBlossomRasterDimensions,
    components: u8,
}

#[cfg(any(feature = "raster-decode", test))]
fn invalid_raster<T>() -> Result<T, RadrootsBlossomError> {
    Err(RadrootsBlossomError::InvalidPublicationRaster)
}

#[cfg(any(feature = "raster-decode", test))]
fn inspect_png_container(bytes: &[u8]) -> Result<RasterContainerInspection, RadrootsBlossomError> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if !bytes.starts_with(SIGNATURE) {
        return invalid_raster();
    }
    let mut position = SIGNATURE.len();
    let mut dimensions = None;
    let mut has_image_data = false;
    let mut image_data_ended = false;
    let mut color_type = None;
    let mut has_palette = false;
    let mut animated = false;
    let mut records = 0_usize;
    while position < bytes.len() {
        records = records
            .checked_add(1)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        if records > PUBLICATION_RASTER_MAX_CONTAINER_RECORDS {
            return invalid_raster();
        }
        let header_end = position
            .checked_add(8)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let header = bytes
            .get(position..header_end)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let length = u32::from_be_bytes(
            header[..4]
                .try_into()
                .map_err(|_| RadrootsBlossomError::InvalidPublicationRaster)?,
        ) as usize;
        let kind: [u8; 4] = header[4..]
            .try_into()
            .map_err(|_| RadrootsBlossomError::InvalidPublicationRaster)?;
        let data_start = header_end;
        let data_end = data_start
            .checked_add(length)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let chunk_end = data_end
            .checked_add(4)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let data = bytes
            .get(data_start..data_end)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        if chunk_end > bytes.len() {
            return invalid_raster();
        }
        position = chunk_end;

        match &kind {
            b"IHDR" if dimensions.is_none() && data.len() == 13 && data_start == 16 => {
                let width = u32::from_be_bytes(
                    data[..4]
                        .try_into()
                        .map_err(|_| RadrootsBlossomError::InvalidPublicationRaster)?,
                );
                let height = u32::from_be_bytes(
                    data[4..8]
                        .try_into()
                        .map_err(|_| RadrootsBlossomError::InvalidPublicationRaster)?,
                );
                let bit_depth = data[8];
                let parsed_color_type = data[9];
                if bit_depth != 8 || data[10] != 0 || data[11] != 0 || data[12] > 1 {
                    return Err(RadrootsBlossomError::PublicationRasterProcessForbidden);
                }
                if !matches!(parsed_color_type, 0 | 2 | 3 | 4 | 6) {
                    return Err(RadrootsBlossomError::PublicationRasterDecodeFailed);
                }
                dimensions = Some(RadrootsBlossomRasterDimensions::new(width, height)?);
                color_type = Some(parsed_color_type);
            }
            b"IHDR" => return invalid_raster(),
            b"PLTE" if dimensions.is_some() && !has_palette && !has_image_data => {
                if data.is_empty() || data.len() % 3 != 0 || data.len() > 768 {
                    return invalid_raster();
                }
                has_palette = true;
            }
            b"PLTE" => return invalid_raster(),
            b"IDAT"
                if dimensions.is_some()
                    && !image_data_ended
                    && (color_type != Some(3) || has_palette) =>
            {
                has_image_data = true;
            }
            b"IDAT" => return invalid_raster(),
            b"acTL" | b"fcTL" | b"fdAT" => animated = true,
            b"IEND" if data.is_empty() && has_image_data && position == bytes.len() => {
                return Ok(RasterContainerInspection {
                    dimensions: dimensions.ok_or(RadrootsBlossomError::InvalidPublicationRaster)?,
                    animated,
                });
            }
            b"IEND" => return invalid_raster(),
            _ if dimensions.is_none() => return invalid_raster(),
            _ if kind[0] & 0x20 == 0 => return invalid_raster(),
            _ => image_data_ended |= has_image_data,
        }
    }
    invalid_raster()
}

#[cfg(any(feature = "raster-decode", test))]
fn inspect_webp_container(bytes: &[u8]) -> Result<RasterContainerInspection, RadrootsBlossomError> {
    if bytes.len() < 20 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return invalid_raster();
    }
    let riff_size = u32::from_le_bytes(
        bytes[4..8]
            .try_into()
            .map_err(|_| RadrootsBlossomError::InvalidPublicationRaster)?,
    ) as usize;
    if riff_size.checked_add(8) != Some(bytes.len()) {
        return invalid_raster();
    }

    let mut position = 12_usize;
    let mut dimensions = None;
    let mut primary_chunks = 0_u8;
    let mut animated = false;
    let mut extended = false;
    let mut records = 0_usize;
    while position < bytes.len() {
        records = records
            .checked_add(1)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        if records > PUBLICATION_RASTER_MAX_CONTAINER_RECORDS {
            return invalid_raster();
        }
        let header_end = position
            .checked_add(8)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let header = bytes
            .get(position..header_end)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let kind: [u8; 4] = header[..4]
            .try_into()
            .map_err(|_| RadrootsBlossomError::InvalidPublicationRaster)?;
        let length = u32::from_le_bytes(
            header[4..]
                .try_into()
                .map_err(|_| RadrootsBlossomError::InvalidPublicationRaster)?,
        ) as usize;
        let data_end = header_end
            .checked_add(length)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let padded_end = data_end
            .checked_add(length & 1)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let data = bytes
            .get(header_end..data_end)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        if padded_end > bytes.len() {
            return invalid_raster();
        }
        position = padded_end;

        match &kind {
            b"ANIM" | b"ANMF" => animated = true,
            b"VP8X" if data.len() == 10 && !extended && primary_chunks == 0 => {
                if data[0] & 0b1100_0001 != 0 || data[1..4] != [0, 0, 0] {
                    return Err(RadrootsBlossomError::PublicationRasterProcessForbidden);
                }
                extended = true;
                animated |= data[0] & 0b0000_0010 != 0;
                let width = 1 + read_u24_le(&data[4..7]);
                let height = 1 + read_u24_le(&data[7..10]);
                dimensions = Some(RadrootsBlossomRasterDimensions::new(width, height)?);
            }
            b"VP8X" => return invalid_raster(),
            b"VP8L" => {
                primary_chunks = primary_chunks.saturating_add(1);
                let header = data
                    .get(..5)
                    .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
                if header[0] != 0x2f {
                    return invalid_raster();
                }
                let bits = u32::from_le_bytes(
                    header[1..5]
                        .try_into()
                        .map_err(|_| RadrootsBlossomError::InvalidPublicationRaster)?,
                );
                let parsed = RadrootsBlossomRasterDimensions::new(
                    (bits & 0x3fff) + 1,
                    ((bits >> 14) & 0x3fff) + 1,
                )?;
                if dimensions.is_some_and(|value| value != parsed) {
                    return Err(RadrootsBlossomError::PublicationRasterContainerDimensionMismatch);
                }
                dimensions = Some(parsed);
            }
            b"VP8 " => {
                primary_chunks = primary_chunks.saturating_add(1);
                let header = data
                    .get(..10)
                    .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
                if &header[3..6] != b"\x9d\x01\x2a" {
                    return invalid_raster();
                }
                let width = u16::from_le_bytes([header[6], header[7]]) & 0x3fff;
                let height = u16::from_le_bytes([header[8], header[9]]) & 0x3fff;
                let parsed =
                    RadrootsBlossomRasterDimensions::new(u32::from(width), u32::from(height))?;
                if dimensions.is_some_and(|value| value != parsed) {
                    return Err(RadrootsBlossomError::PublicationRasterContainerDimensionMismatch);
                }
                dimensions = Some(parsed);
            }
            _ => {}
        }
    }
    if primary_chunks == 0 {
        if !animated {
            return invalid_raster();
        }
    } else if primary_chunks != 1 {
        return invalid_raster();
    }
    Ok(RasterContainerInspection {
        dimensions: dimensions.ok_or(RadrootsBlossomError::InvalidPublicationRaster)?,
        animated,
    })
}

#[cfg(any(feature = "raster-decode", test))]
fn read_u24_le(bytes: &[u8]) -> u32 {
    u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16)
}

#[cfg(any(feature = "raster-decode", test))]
fn inspect_jpeg_container(bytes: &[u8]) -> Result<JpegContainerInspection, RadrootsBlossomError> {
    if bytes.len() < 4 || !bytes.starts_with(b"\xff\xd8") {
        return invalid_raster();
    }
    let mut position = 2_usize;
    let mut dimensions = None;
    let mut components = None;
    let mut records = 0_usize;
    loop {
        records = records
            .checked_add(1)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        if records > PUBLICATION_RASTER_MAX_CONTAINER_RECORDS {
            return invalid_raster();
        }
        if bytes.get(position) != Some(&0xff) {
            return invalid_raster();
        }
        while bytes.get(position) == Some(&0xff) {
            position = position
                .checked_add(1)
                .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        }
        let marker = *bytes
            .get(position)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        position = position
            .checked_add(1)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        match marker {
            0xd9 if position == bytes.len() => {
                return Ok(JpegContainerInspection {
                    dimensions: dimensions.ok_or(RadrootsBlossomError::InvalidPublicationRaster)?,
                    components: components.ok_or(RadrootsBlossomError::InvalidPublicationRaster)?,
                });
            }
            0xd9 | 0x00 | 0xd8 | 0xd0..=0xd7 => return invalid_raster(),
            0x01 => continue,
            _ => {}
        }

        let length_end = position
            .checked_add(2)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let length_bytes = bytes
            .get(position..length_end)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let length = usize::from(u16::from_be_bytes(
            length_bytes
                .try_into()
                .map_err(|_| RadrootsBlossomError::InvalidPublicationRaster)?,
        ));
        if length < 2 {
            return invalid_raster();
        }
        let data_start = length_end;
        let data_end = position
            .checked_add(length)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        let data = bytes
            .get(data_start..data_end)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        position = data_end;

        if is_jpeg_start_of_frame(marker) {
            if dimensions.is_some() || data.len() < 6 {
                return invalid_raster();
            }
            if !matches!(marker, 0xc0 | 0xc1) || data[0] != 8 {
                return Err(RadrootsBlossomError::PublicationJpegProcessForbidden);
            }
            let component_count = data[5];
            if !matches!(component_count, 1 | 3 | 4)
                || data.len() != 6 + 3 * usize::from(component_count)
            {
                return invalid_raster();
            }
            let height = u32::from(u16::from_be_bytes([data[1], data[2]]));
            let width = u32::from(u16::from_be_bytes([data[3], data[4]]));
            dimensions = Some(RadrootsBlossomRasterDimensions::new(width, height)?);
            components = Some(component_count);
        }

        if marker == 0xda {
            position = jpeg_scan_end(bytes, position)?;
        }
    }
}

#[cfg(any(feature = "raster-decode", test))]
fn is_jpeg_start_of_frame(marker: u8) -> bool {
    matches!(
        marker,
        0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf
    )
}

#[cfg(any(feature = "raster-decode", test))]
fn jpeg_scan_end(bytes: &[u8], mut position: usize) -> Result<usize, RadrootsBlossomError> {
    while position < bytes.len() {
        if bytes[position] != 0xff {
            position = position
                .checked_add(1)
                .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
            continue;
        }
        let marker_start = position;
        while bytes.get(position) == Some(&0xff) {
            position = position
                .checked_add(1)
                .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        }
        let marker = *bytes
            .get(position)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        match marker {
            0x00 | 0xd0..=0xd7 => {
                position = position
                    .checked_add(1)
                    .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
            }
            _ => return Ok(marker_start),
        }
    }
    invalid_raster()
}

#[cfg(test)]
fn validate_png_container(
    bytes: &[u8],
) -> Result<RadrootsBlossomRasterDimensions, RadrootsBlossomError> {
    static_container_dimensions(inspect_png_container(bytes)?)
}

#[cfg(test)]
fn validate_webp_container(
    bytes: &[u8],
) -> Result<Option<RadrootsBlossomRasterDimensions>, RadrootsBlossomError> {
    static_container_dimensions(inspect_webp_container(bytes)?).map(Some)
}

#[cfg(test)]
fn validate_jpeg_container(
    bytes: &[u8],
) -> Result<RadrootsBlossomRasterDimensions, RadrootsBlossomError> {
    Ok(inspect_jpeg_container(bytes)?.dimensions)
}

#[cfg(test)]
fn static_container_dimensions(
    inspection: RasterContainerInspection,
) -> Result<RadrootsBlossomRasterDimensions, RadrootsBlossomError> {
    if inspection.animated {
        return Err(RadrootsBlossomError::PublicationRasterAnimationForbidden);
    }
    Ok(inspection.dimensions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RadrootsBlossomByteVerifiedDescriptor;
    #[cfg(feature = "raster-decode")]
    use alloc::boxed::Box;
    use alloc::{format, string::ToString};
    #[cfg(feature = "raster-decode")]
    use image::{ColorType, ImageError, ImageResult};

    const PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60,
        0xf8, 0xcf, 0xf0, 0x00, 0x00, 0x03, 0xe2, 0x01, 0xe0, 0x38, 0x10, 0xac, 0x1e, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    const JPEG: &[u8] = &[
        0xff, 0xd8, 0xff, 0xc0, 0x00, 0x0b, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00,
        0xff, 0xda, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3f, 0x00, 0x00, 0xff, 0xd9,
    ];

    const STILL_WEBP: &[u8] = &[
        b'R', b'I', b'F', b'F', 18, 0, 0, 0, b'W', b'E', b'B', b'P', b'V', b'P', b'8', b'L', 5, 0,
        0, 0, 0x2f, 0, 0, 0, 0, 0,
    ];

    #[cfg(feature = "raster-decode")]
    fn sequential_jpeg() -> Vec<u8> {
        hex::decode(
            "ffd8ffe000104a46494600010100000100010000ffdb0043000302020302020303030304030304050805050404050a070706080c0a0c0c0b0a0b0b0d0e12100d0e110e0b0b1016101113141515150c0f171816141812141514ffdb00430103040405040509050509140d0b0d1414141414141414141414141414141414141414141414141414141414141414141414141414141414141414141414141414ffc00011080001000103012200021101031101ffc4001f0000010501010101010100000000000000000102030405060708090a0bffc400b5100002010303020403050504040000017d01020300041105122131410613516107227114328191a1082342b1c11552d1f02433627282090a161718191a25262728292a3435363738393a434445464748494a535455565758595a636465666768696a737475767778797a838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae1e2e3e4e5e6e7e8e9eaf1f2f3f4f5f6f7f8f9faffc4001f0100030101010101010101010000000000000102030405060708090a0bffc400b51100020102040403040705040400010277000102031104052131061241510761711322328108144291a1b1c109233352f0156272d10a162434e125f11718191a262728292a35363738393a434445464748494a535455565758595a636465666768696a737475767778797a82838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae2e3e4e5e6e7e8e9eaf2f3f4f5f6f7f8f9faffda000c03010002110311003f00f9ca8a28afc3cfe6f3ffd9",
        )
        .unwrap()
    }

    #[cfg(feature = "raster-decode")]
    fn malformed_dqt_jpeg() -> Vec<u8> {
        let mut jpeg = sequential_jpeg();
        let sof = jpeg
            .windows(2)
            .position(|window| window == b"\xff\xc0")
            .unwrap();
        jpeg.drain(sof - 3..sof);
        jpeg
    }

    fn png_with_chunks(chunks: &[([u8; 4], &[u8])]) -> Vec<u8> {
        let mut output = b"\x89PNG\r\n\x1a\n".to_vec();
        for (kind, data) in chunks {
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(kind);
            output.extend_from_slice(data);
            output.extend_from_slice(&[0; 4]);
        }
        output
    }

    fn png_with_record_count(record_count: usize) -> Vec<u8> {
        assert!(record_count >= 3);
        let ihdr = &PNG[16..29];
        let idat = &PNG[41..54];
        let mut output = b"\x89PNG\r\n\x1a\n".to_vec();
        for (kind, data) in core::iter::once((*b"IHDR", ihdr))
            .chain(core::iter::repeat_n((*b"tEXt", &[][..]), record_count - 3))
            .chain([(*b"IDAT", idat), (*b"IEND", &[][..])])
        {
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(&kind);
            output.extend_from_slice(data);
            output.extend_from_slice(&[0; 4]);
        }
        output
    }

    fn webp_with_chunks(chunks: &[([u8; 4], &[u8])]) -> Vec<u8> {
        let mut output = b"RIFF\0\0\0\0WEBP".to_vec();
        for (kind, data) in chunks {
            output.extend_from_slice(kind);
            output.extend_from_slice(&(data.len() as u32).to_le_bytes());
            output.extend_from_slice(data);
            if data.len() & 1 == 1 {
                output.push(0);
            }
        }
        let riff_size = (output.len() as u32) - 8;
        output[4..8].copy_from_slice(&riff_size.to_le_bytes());
        output
    }

    fn webp_with_record_count(record_count: usize) -> Vec<u8> {
        assert!(record_count >= 1);
        let mut output = b"RIFF\0\0\0\0WEBP".to_vec();
        for _ in 1..record_count {
            output.extend_from_slice(b"JUNK\0\0\0\0");
        }
        output.extend_from_slice(b"VP8L\x05\0\0\0\x2f\0\0\0\0\0");
        let riff_size = (output.len() as u32) - 8;
        output[4..8].copy_from_slice(&riff_size.to_le_bytes());
        output
    }

    fn jpeg_with_app_record_count(record_count: usize) -> Vec<u8> {
        let mut output = b"\xff\xd8".to_vec();
        for _ in 0..record_count {
            output.extend_from_slice(b"\xff\xe0\0\x02");
        }
        output.extend_from_slice(b"\xff\xd9");
        output
    }

    fn descriptor(bytes: &[u8], media_type: &str, origin: &str) -> RadrootsBlossomBlobDescriptor {
        let hash = RadrootsBlossomSha256::digest(bytes);
        RadrootsBlossomBlobDescriptor::new(
            crate::RadrootsBlossomBlobUrl::parse(&format!("{origin}/{hash}.png")).unwrap(),
            hash,
            bytes.len() as u64,
            RadrootsBlossomMediaType::parse(media_type).unwrap(),
            1_800_000_000,
        )
        .unwrap()
    }

    fn verified(bytes: &[u8]) -> RadrootsBlossomByteVerifiedDescriptor {
        let media_type = RadrootsBlossomMediaType::parse("image/png").unwrap();
        descriptor(bytes, "image/png", "https://cdn.example")
            .approve_reference()
            .unwrap()
            .verify_bytes(bytes, &media_type)
            .unwrap()
    }

    fn observations(
        bytes: &[u8],
    ) -> (
        RadrootsBlossomBud02UploadObservation,
        RadrootsBlossomBud01HeadObservation,
        RadrootsBlossomBud01GetObservation,
    ) {
        let expected = verified(bytes);
        let upload = RadrootsBlossomBud02UploadObservation::new(
            201,
            descriptor(bytes, "image/png", "https://cdn.example"),
        )
        .unwrap();
        let url = expected.url().clone();
        let media_type = RadrootsBlossomMediaType::parse("image/png").unwrap();
        let head = RadrootsBlossomBud01HeadObservation::new(
            200,
            url.clone(),
            bytes.len() as u64,
            media_type,
        )
        .unwrap();
        let get = RadrootsBlossomBud01GetObservation::from_complete_body(
            200,
            url,
            bytes.len() as u64,
            bytes,
        )
        .unwrap();
        (upload, head, get)
    }

    #[cfg(feature = "raster-decode")]
    #[test]
    fn publication_readiness_accepts_exact_complete_observations() {
        let expected = verified(PNG);
        let (upload, head, get) = observations(PNG);
        let evidence = verify_publication_readiness(
            &expected,
            PNG,
            RadrootsBlossomAuthoredRasterDimensions::Exact(
                RadrootsBlossomRasterDimensions::new(1, 1).unwrap(),
            ),
            &upload,
            &head,
            &get,
        )
        .unwrap();
        assert_eq!(
            evidence.url().as_str(),
            "https://cdn.example/4490130851783ff662845f5e72f1948618cc87f951f00f6c2ffb3dc01f3f40fd.png"
        );
        assert_eq!(evidence.sha256(), RadrootsBlossomSha256::digest(PNG));
        assert_eq!(evidence.size(), PNG.len() as u64);
        assert_eq!(evidence.media_type().as_str(), "image/png");
        assert_eq!(evidence.raster_format(), RadrootsBlossomRasterFormat::Png);
        assert_eq!(evidence.dimensions().pixels(), 1);
        assert_eq!(evidence.bud02_status().as_u16(), 201);
        assert_eq!(evidence.uploaded(), 1_800_000_000);
        assert_eq!(
            evidence.evidence_digest().to_string(),
            "44e63303e594ea42d863be995b23ac4297ed77e4378d0707c94f28e77164bd3b"
        );
        assert_eq!(
            evidence.evidence_digest().as_sha256().to_string(),
            evidence.evidence_digest().to_string()
        );
    }

    #[cfg(all(feature = "raster-decode", feature = "serde"))]
    #[test]
    fn readiness_evidence_serializer_rejects_oversized_internal_state() {
        let expected = verified(PNG);
        let (upload, head, get) = observations(PNG);
        let mut evidence = verify_publication_readiness(
            &expected,
            PNG,
            RadrootsBlossomAuthoredRasterDimensions::Unspecified,
            &upload,
            &head,
            &get,
        )
        .unwrap();
        let oversized_url = format!(
            "https://cdn.example/{}.{}",
            evidence.sha256(),
            "p".repeat(RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES),
        );
        evidence.url = RadrootsBlossomBlobUrl::parse(&oversized_url)
            .unwrap()
            .approve()
            .unwrap();

        match evidence.to_canonical_json().unwrap_err() {
            RadrootsBlossomError::PublicationReadinessEvidenceTooLarge { max, actual } => {
                assert_eq!(
                    max,
                    RADROOTS_BLOSSOM_PUBLICATION_READINESS_EVIDENCE_MAX_BYTES
                );
                assert!(actual > max);
            }
            error => panic!("unexpected evidence serialization error: {error}"),
        }
    }

    #[cfg(all(feature = "raster-decode", feature = "serde"))]
    #[test]
    fn readiness_evidence_wire_rejects_invalid_and_noncanonical_fields() {
        fn canonical_wire() -> PublicationReadinessEvidenceWire {
            let expected = verified(PNG);
            let (upload, head, get) = observations(PNG);
            let evidence = verify_publication_readiness(
                &expected,
                PNG,
                RadrootsBlossomAuthoredRasterDimensions::Unspecified,
                &upload,
                &head,
                &get,
            )
            .unwrap();
            serde_json::from_slice(&evidence.to_canonical_json().unwrap()).unwrap()
        }

        for media_type in ["not a media type", "image/PNG", "image/gif"] {
            let mut wire = canonical_wire();
            wire.media_type = media_type.to_string();
            assert_eq!(
                readiness_evidence_from_wire(wire).unwrap_err().code(),
                "publication_readiness_evidence_field_invalid",
                "{media_type}",
            );
        }

        for raster_format in ["still_webp", "unknown"] {
            let mut wire = canonical_wire();
            wire.raster_format = raster_format.to_string();
            assert_eq!(
                readiness_evidence_from_wire(wire).unwrap_err().code(),
                "publication_readiness_evidence_field_invalid",
                "{raster_format}",
            );
        }

        let canonical_url = canonical_wire().url;
        for url in [
            "not a URL".to_string(),
            canonical_url.replace("cdn.example", "CDN.example"),
        ] {
            let mut wire = canonical_wire();
            wire.url = url.clone();
            assert_eq!(
                readiness_evidence_from_wire(wire).unwrap_err().code(),
                "publication_readiness_evidence_field_invalid",
                "{url}",
            );
        }
    }

    #[test]
    fn bounded_get_collector_rejects_status_bounds_and_body_shape() {
        let url = verified(PNG).url().clone();
        assert_eq!(
            RadrootsBlossomBud01GetCollector::new(206, url.clone(), 1)
                .err()
                .unwrap()
                .code(),
            "invalid_bud01_get_status"
        );
        assert_eq!(
            RadrootsBlossomBud01GetCollector::new(
                200,
                url.clone(),
                RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES + 1,
            )
            .err()
            .unwrap()
            .code(),
            "publication_raster_byte_limit_exceeded"
        );
        assert_eq!(
            RadrootsBlossomBud01GetCollector::new(200, url.clone(), 1)
                .unwrap()
                .finish()
                .err()
                .unwrap()
                .code(),
            "publication_get_body_missing"
        );
        let mut short = RadrootsBlossomBud01GetCollector::new(200, url.clone(), 2).unwrap();
        short.push_chunk(b"a").unwrap();
        assert_eq!(
            short.finish().err().unwrap().code(),
            "publication_get_body_short"
        );
        let mut trailing = RadrootsBlossomBud01GetCollector::new(200, url, 1).unwrap();
        assert_eq!(
            trailing.push_chunk(b"ab").unwrap_err().code(),
            "publication_get_body_trailing"
        );
    }

    #[test]
    fn observation_constructors_reject_invalid_status_and_dimensions() {
        assert_eq!(
            RadrootsBlossomBud02UploadObservation::new(
                204,
                descriptor(PNG, "image/png", "https://cdn.example")
            )
            .unwrap_err()
            .code(),
            "invalid_bud02_upload_status"
        );
        let url = verified(PNG).url().clone();
        assert_eq!(
            RadrootsBlossomBud01HeadObservation::new(
                204,
                url,
                PNG.len() as u64,
                RadrootsBlossomMediaType::parse("image/png").unwrap(),
            )
            .unwrap_err()
            .code(),
            "invalid_bud01_head_status"
        );
        for (width, height, code) in [
            (0, 1, "publication_raster_dimensions_out_of_range"),
            (5_000, 5_000, "publication_raster_pixel_limit_exceeded"),
        ] {
            assert_eq!(
                RadrootsBlossomRasterDimensions::new(width, height)
                    .unwrap_err()
                    .code(),
                code
            );
        }
        assert_eq!(
            RadrootsBlossomRasterFormat::StillWebP.to_string(),
            "still_webp"
        );
        assert_eq!(
            RadrootsBlossomRasterFormat::from_media_type(
                &RadrootsBlossomMediaType::parse("image/png;charset=utf-8").unwrap(),
            )
            .unwrap_err()
            .code(),
            "unsupported_publication_raster_media_type"
        );
    }

    #[test]
    fn raster_dimensions_reject_each_axis_boundary() {
        assert_eq!(
            RadrootsBlossomRasterDimensions::new(
                RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION,
                1,
            )
            .unwrap()
            .pixels(),
            u64::from(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION)
        );
        assert_eq!(
            RadrootsBlossomRasterDimensions::new(
                1,
                RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION,
            )
            .unwrap()
            .pixels(),
            u64::from(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION)
        );
        for (width, height) in [
            (0, 1),
            (1, 0),
            (RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION + 1, 1),
            (1, RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION + 1),
        ] {
            assert_eq!(
                RadrootsBlossomRasterDimensions::new(width, height)
                    .unwrap_err()
                    .code(),
                "publication_raster_dimensions_out_of_range"
            );
        }
        assert_eq!(
            RadrootsBlossomRasterDimensions::new(5_000, 4_000)
                .unwrap()
                .pixels(),
            RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS
        );
        assert_eq!(
            RadrootsBlossomRasterDimensions::new(5_001, 4_000)
                .unwrap_err()
                .code(),
            "publication_raster_pixel_limit_exceeded"
        );
    }

    #[cfg(feature = "raster-decode")]
    #[test]
    fn readiness_rejects_oversized_authored_bytes_and_digest_collisions() {
        let oversized = alloc::vec![
            0_u8;
            RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES as usize + 1
        ];
        let oversized_descriptor = verified(&oversized);
        let (upload, head, get) = observations(PNG);
        assert_eq!(
            verify_publication_readiness(
                &oversized_descriptor,
                &oversized,
                RadrootsBlossomAuthoredRasterDimensions::Unspecified,
                &upload,
                &head,
                &get,
            )
            .unwrap_err()
            .code(),
            "publication_raster_byte_limit_exceeded"
        );

        let expected_hash = RadrootsBlossomSha256::digest(PNG);
        let mut different_bytes = PNG.to_vec();
        different_bytes[0] ^= 1;
        assert_eq!(
            validate_retrieved_body(expected_hash, PNG, &different_bytes, expected_hash)
                .unwrap_err()
                .code(),
            "publication_retrieved_bytes_mismatch"
        );
        assert_eq!(
            validate_retrieved_body(
                expected_hash,
                PNG,
                PNG,
                RadrootsBlossomSha256::digest(b"different"),
            )
            .unwrap_err()
            .code(),
            "publication_retrieved_bytes_hash_mismatch"
        );
        validate_retrieved_body(expected_hash, PNG, PNG, expected_hash).unwrap();
    }

    #[test]
    fn closed_container_validation_accepts_each_format() {
        assert_eq!(
            validate_png_container(PNG).unwrap(),
            RadrootsBlossomRasterDimensions::new(1, 1).unwrap()
        );
        assert_eq!(
            validate_jpeg_container(JPEG).unwrap(),
            RadrootsBlossomRasterDimensions::new(1, 1).unwrap()
        );
        assert_eq!(
            validate_webp_container(STILL_WEBP).unwrap(),
            Some(RadrootsBlossomRasterDimensions::new(1, 1).unwrap())
        );
    }

    #[test]
    fn closed_container_validation_rejects_animation_trailing_and_malformed_bytes() {
        let mut apng = PNG.to_vec();
        let iend = apng.len() - 12;
        apng.splice(
            iend..iend,
            [
                0, 0, 0, 8, b'a', b'c', b'T', b'L', 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        );
        assert_eq!(
            validate_png_container(&apng).unwrap_err().code(),
            "publication_raster_animation_forbidden"
        );
        assert_eq!(
            validate_png_container(b"not png").unwrap_err().code(),
            "invalid_publication_raster"
        );
        assert_eq!(
            validate_webp_container(b"not webp").unwrap_err().code(),
            "invalid_publication_raster"
        );
        assert_eq!(
            validate_jpeg_container(b"not jpeg").unwrap_err().code(),
            "invalid_publication_raster"
        );

        let mut animated_webp = [
            b'R', b'I', b'F', b'F', 22, 0, 0, 0, b'W', b'E', b'B', b'P', b'V', b'P', b'8', b'X',
            10, 0, 0, 0, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(
            validate_webp_container(&animated_webp).unwrap_err().code(),
            "publication_raster_animation_forbidden"
        );
        animated_webp[4] = 21;
        assert_eq!(
            validate_webp_container(&animated_webp).unwrap_err().code(),
            "invalid_publication_raster"
        );

        let mut trailing_jpeg = JPEG.to_vec();
        trailing_jpeg.push(0);
        assert_eq!(
            validate_jpeg_container(&trailing_jpeg).unwrap_err().code(),
            "invalid_publication_raster"
        );

        let mut duplicate_sof_jpeg = JPEG.to_vec();
        duplicate_sof_jpeg.splice(15..15, JPEG[2..15].iter().copied());
        assert_eq!(
            validate_jpeg_container(&duplicate_sof_jpeg)
                .unwrap_err()
                .code(),
            "invalid_publication_raster"
        );
    }

    #[test]
    fn png_container_rejects_each_chunk_order_and_termination_failure() {
        let ihdr = &PNG[16..29];
        let idat = &PNG[41..54];

        let mut short_header = PNG[..8].to_vec();
        short_header.push(0);
        for malformed in [PNG[..8].to_vec(), short_header, PNG[..30].to_vec()] {
            assert_eq!(
                validate_png_container(&malformed).unwrap_err().code(),
                "invalid_publication_raster"
            );
        }

        let short_ihdr = png_with_chunks(&[(*b"IHDR", &ihdr[..12])]);
        let duplicate_ihdr = png_with_chunks(&[(*b"IHDR", ihdr), (*b"IHDR", ihdr)]);
        let idat_before_ihdr = png_with_chunks(&[(*b"IDAT", idat)]);
        let animation_before_ihdr = png_with_chunks(&[(*b"acTL", &[0; 8]), (*b"IHDR", ihdr)]);
        for malformed in [
            short_ihdr,
            duplicate_ihdr,
            idat_before_ihdr,
            animation_before_ihdr,
        ] {
            assert_eq!(
                validate_png_container(&malformed).unwrap_err().code(),
                "invalid_publication_raster"
            );
        }

        let with_ancillary = png_with_chunks(&[
            (*b"IHDR", ihdr),
            (*b"tEXt", b"key\0value"),
            (*b"IDAT", idat),
            (*b"IEND", &[]),
        ]);
        assert_eq!(
            validate_png_container(&with_ancillary).unwrap(),
            RadrootsBlossomRasterDimensions::new(1, 1).unwrap()
        );

        let nonempty_iend =
            png_with_chunks(&[(*b"IHDR", ihdr), (*b"IDAT", idat), (*b"IEND", &[0])]);
        let no_image_data = png_with_chunks(&[(*b"IHDR", ihdr), (*b"IEND", &[])]);
        let mut trailing = png_with_chunks(&[(*b"IHDR", ihdr), (*b"IDAT", idat), (*b"IEND", &[])]);
        trailing.push(0);
        let missing_iend = png_with_chunks(&[(*b"IHDR", ihdr), (*b"IDAT", idat)]);
        for malformed in [nonempty_iend, no_image_data, trailing, missing_iend] {
            assert_eq!(
                validate_png_container(&malformed).unwrap_err().code(),
                "invalid_publication_raster"
            );
        }
    }

    #[test]
    fn png_container_enforces_eight_bit_process_palette_and_record_limits() {
        for index in [24, 26, 27, 28] {
            let mut forbidden = PNG.to_vec();
            forbidden[index] = match index {
                24 => 16,
                28 => 2,
                _ => 1,
            };
            assert_eq!(
                validate_png_container(&forbidden).unwrap_err().code(),
                "publication_raster_process_forbidden"
            );
        }

        let mut malformed_color = PNG.to_vec();
        malformed_color[25] = 1;
        assert_eq!(
            validate_png_container(&malformed_color).unwrap_err().code(),
            "publication_raster_decode_failed"
        );

        let mut indexed_ihdr = PNG[16..29].to_vec();
        indexed_ihdr[9] = 3;
        let missing_palette = png_with_chunks(&[
            (*b"IHDR", &indexed_ihdr),
            (*b"IDAT", &PNG[41..54]),
            (*b"IEND", &[]),
        ]);
        assert_eq!(
            validate_png_container(&missing_palette).unwrap_err().code(),
            "invalid_publication_raster"
        );
        let indexed = png_with_chunks(&[
            (*b"IHDR", &indexed_ihdr),
            (*b"PLTE", &[0, 0, 0]),
            (*b"IDAT", &PNG[41..54]),
            (*b"IEND", &[]),
        ]);
        assert_eq!(
            validate_png_container(&indexed).unwrap(),
            RadrootsBlossomRasterDimensions::new(1, 1).unwrap()
        );

        assert!(
            validate_png_container(&png_with_record_count(
                PUBLICATION_RASTER_MAX_CONTAINER_RECORDS
            ))
            .is_ok()
        );
        assert_eq!(
            validate_png_container(&png_with_record_count(
                PUBLICATION_RASTER_MAX_CONTAINER_RECORDS + 1
            ))
            .unwrap_err()
            .code(),
            "invalid_publication_raster"
        );
    }

    #[test]
    fn png_container_rejects_every_palette_and_unknown_chunk_boundary() {
        let ihdr = &PNG[16..29];
        let idat = &PNG[41..54];
        let oversized_palette = [0_u8; 771];

        for palette in [&[][..], &[0, 0][..], &oversized_palette[..]] {
            let malformed = png_with_chunks(&[
                (*b"IHDR", ihdr),
                (*b"PLTE", palette),
                (*b"IDAT", idat),
                (*b"IEND", &[]),
            ]);
            assert_eq!(
                validate_png_container(&malformed).unwrap_err().code(),
                "invalid_publication_raster"
            );
        }

        for malformed in [
            png_with_chunks(&[(*b"PLTE", &[0, 0, 0]), (*b"IHDR", ihdr)]),
            png_with_chunks(&[
                (*b"IHDR", ihdr),
                (*b"PLTE", &[0, 0, 0]),
                (*b"PLTE", &[0, 0, 0]),
            ]),
            png_with_chunks(&[(*b"IHDR", ihdr), (*b"IDAT", idat), (*b"PLTE", &[0, 0, 0])]),
            png_with_chunks(&[(*b"tEXt", &[]), (*b"IHDR", ihdr)]),
            png_with_chunks(&[(*b"IHDR", ihdr), (*b"ABCD", &[])]),
            png_with_chunks(&[
                (*b"IHDR", ihdr),
                (*b"IDAT", idat),
                (*b"tEXt", &[]),
                (*b"IDAT", idat),
            ]),
        ] {
            assert_eq!(
                validate_png_container(&malformed).unwrap_err().code(),
                "invalid_publication_raster"
            );
        }
    }

    #[test]
    fn webp_container_covers_extended_lossless_and_lossy_boundaries() {
        let vp8x_1x1 = [0_u8; 10];
        let mut vp8x_2x1 = vp8x_1x1;
        vp8x_2x1[4] = 1;
        let mut vp8x_animated = vp8x_1x1;
        vp8x_animated[0] = 0x02;
        let mut vp8x_reserved = vp8x_1x1;
        vp8x_reserved[0] = 0x01;
        let vp8l_1x1 = [0x2f, 0, 0, 0, 0];
        let vp8_1x1 = [0, 0, 0, 0x9d, 0x01, 0x2a, 1, 0, 1, 0];

        let mut bad_riff = STILL_WEBP.to_vec();
        bad_riff[0] = b'X';
        let mut bad_webp = STILL_WEBP.to_vec();
        bad_webp[8] = b'X';
        for malformed in [bad_riff, bad_webp] {
            assert_eq!(
                validate_webp_container(&malformed).unwrap_err().code(),
                "invalid_publication_raster"
            );
        }

        let mut missing_padding = webp_with_chunks(&[(*b"VP8L", &vp8l_1x1)]);
        missing_padding.pop();
        let riff_size = (missing_padding.len() as u32) - 8;
        missing_padding[4..8].copy_from_slice(&riff_size.to_le_bytes());
        assert_eq!(
            validate_webp_container(&missing_padding)
                .unwrap_err()
                .code(),
            "invalid_publication_raster"
        );

        for animated_kind in [*b"ANIM", *b"ANMF"] {
            assert_eq!(
                validate_webp_container(&webp_with_chunks(&[
                    (*b"VP8X", &vp8x_animated),
                    (animated_kind, &[]),
                ]))
                .unwrap_err()
                .code(),
                "publication_raster_animation_forbidden"
            );
        }

        let extended_lossless = webp_with_chunks(&[(*b"VP8X", &vp8x_1x1), (*b"VP8L", &vp8l_1x1)]);
        assert_eq!(
            validate_webp_container(&extended_lossless).unwrap(),
            Some(RadrootsBlossomRasterDimensions::new(1, 1).unwrap())
        );
        assert_eq!(
            validate_webp_container(&webp_with_chunks(&[(*b"VP8X", &[0; 9])]))
                .unwrap_err()
                .code(),
            "invalid_publication_raster"
        );
        assert_eq!(
            validate_webp_container(&webp_with_chunks(&[(*b"VP8X", &vp8x_reserved)]))
                .unwrap_err()
                .code(),
            "publication_raster_process_forbidden"
        );
        assert_eq!(
            validate_webp_container(&webp_with_chunks(&[
                (*b"VP8X", &vp8x_1x1),
                (*b"VP8X", &vp8x_1x1),
            ]))
            .unwrap_err()
            .code(),
            "invalid_publication_raster"
        );
        assert_eq!(
            validate_webp_container(&webp_with_chunks(&[(*b"VP8L", &[0; 5])]))
                .unwrap_err()
                .code(),
            "invalid_publication_raster"
        );
        assert_eq!(
            validate_webp_container(&webp_with_chunks(&[
                (*b"VP8X", &vp8x_2x1),
                (*b"VP8L", &vp8l_1x1),
            ]))
            .unwrap_err()
            .code(),
            "publication_raster_container_dimension_mismatch"
        );

        assert_eq!(
            validate_webp_container(&webp_with_chunks(&[(*b"VP8 ", &vp8_1x1)])).unwrap(),
            Some(RadrootsBlossomRasterDimensions::new(1, 1).unwrap())
        );
        let mut bad_vp8_signature = vp8_1x1;
        bad_vp8_signature[3] = 0;
        assert_eq!(
            validate_webp_container(&webp_with_chunks(&[(*b"VP8 ", &bad_vp8_signature)]))
                .unwrap_err()
                .code(),
            "invalid_publication_raster"
        );
        assert_eq!(
            validate_webp_container(&webp_with_chunks(&[
                (*b"VP8X", &vp8x_2x1),
                (*b"VP8 ", &vp8_1x1),
            ]))
            .unwrap_err()
            .code(),
            "publication_raster_container_dimension_mismatch"
        );

        let with_unknown = webp_with_chunks(&[(*b"JUNK", &[0; 2]), (*b"VP8L", &vp8l_1x1)]);
        assert_eq!(
            validate_webp_container(&with_unknown).unwrap(),
            Some(RadrootsBlossomRasterDimensions::new(1, 1).unwrap())
        );
        for malformed in [
            webp_with_chunks(&[(*b"JUNK", &[0; 8])]),
            webp_with_chunks(&[(*b"VP8L", &vp8l_1x1), (*b"VP8L", &vp8l_1x1)]),
            webp_with_chunks(&[(*b"VP8L", &[0x2f; 4])]),
            webp_with_chunks(&[(*b"VP8 ", &[0; 9])]),
        ] {
            assert_eq!(
                validate_webp_container(&malformed).unwrap_err().code(),
                "invalid_publication_raster"
            );
        }

        let large_bits = 0x3fff_u32 | (0x3fff_u32 << 14);
        let mut large_vp8l = [0_u8; 5];
        large_vp8l[0] = 0x2f;
        large_vp8l[1..].copy_from_slice(&large_bits.to_le_bytes());
        assert_eq!(
            validate_webp_container(&webp_with_chunks(&[(*b"VP8L", &large_vp8l)]))
                .unwrap_err()
                .code(),
            "publication_raster_pixel_limit_exceeded"
        );

        let mut vp8x_reserved_bytes = vp8x_1x1;
        vp8x_reserved_bytes[1] = 1;
        for malformed in [
            webp_with_chunks(&[(*b"VP8L", &vp8l_1x1), (*b"VP8X", &vp8x_1x1)]),
            webp_with_chunks(&[(*b"VP8X", &vp8x_reserved_bytes)]),
        ] {
            assert!(validate_webp_container(&malformed).is_err());
        }
    }

    #[test]
    fn container_record_limits_reject_one_over_for_webp_and_jpeg() {
        assert!(
            validate_webp_container(&webp_with_record_count(
                PUBLICATION_RASTER_MAX_CONTAINER_RECORDS
            ))
            .is_ok()
        );
        assert_eq!(
            validate_webp_container(&webp_with_record_count(
                PUBLICATION_RASTER_MAX_CONTAINER_RECORDS + 1
            ))
            .unwrap_err()
            .code(),
            "invalid_publication_raster"
        );

        let one_over = jpeg_with_app_record_count(PUBLICATION_RASTER_MAX_CONTAINER_RECORDS);
        assert_eq!(
            validate_jpeg_container(&one_over).unwrap_err().code(),
            "invalid_publication_raster"
        );
        #[cfg(feature = "raster-decode")]
        {
            assert_eq!(
                sequential_jpeg::validate(
                    &one_over,
                    JpegContainerInspection {
                        dimensions: RadrootsBlossomRasterDimensions::new(1, 1).unwrap(),
                        components: 1,
                    },
                )
                .unwrap_err()
                .code(),
                "publication_raster_decode_failed"
            );
        }
    }

    #[test]
    fn jpeg_container_covers_marker_segment_and_scan_boundaries() {
        let tiny = [0xff, 0xd8];
        let bad_marker = [0xff, 0xd8, 0x00, 0x00];
        let short_segment_length = [0xff, 0xd8, 0xff, 0xe0, 0x00, 0x01];
        let short_sof = [
            0xff, 0xd8, 0xff, 0xc0, 0x00, 0x07, 0x08, 0x00, 0x01, 0x00, 0x01,
        ];
        let invalid_component_count = [
            0xff, 0xd8, 0xff, 0xc0, 0x00, 0x0e, 0x08, 0x00, 0x01, 0x00, 0x01, 0x02, 0x01, 0x11,
            0x00, 0x02, 0x11, 0x00,
        ];
        let mismatched_component_length = [
            0xff, 0xd8, 0xff, 0xc0, 0x00, 0x0e, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11,
            0x00, 0x02, 0x11, 0x00,
        ];
        for malformed in [
            tiny.as_slice(),
            bad_marker.as_slice(),
            short_segment_length.as_slice(),
            short_sof.as_slice(),
            invalid_component_count.as_slice(),
            mismatched_component_length.as_slice(),
            &JPEG[..JPEG.len() - 2],
            &[0xff, 0xd8, 0xff],
            &[0xff, 0xd8, 0xff, 0xe0, 0x00],
            &[0xff, 0xd8, 0xff, 0xe0, 0x00, 0x05, 0x00],
            &[0xff, 0xd8, 0xff, 0xd9],
        ] {
            assert_eq!(
                validate_jpeg_container(malformed).unwrap_err().code(),
                "invalid_publication_raster"
            );
        }

        let temporal_marker = [&JPEG[..2], &[0xff, 0x01], &JPEG[2..]].concat();
        assert_eq!(
            validate_jpeg_container(&temporal_marker).unwrap(),
            RadrootsBlossomRasterDimensions::new(1, 1).unwrap()
        );

        let mut stuffed_and_restart = JPEG[..JPEG.len() - 2].to_vec();
        stuffed_and_restart.extend_from_slice(&[0xff, 0x00, 0xff, 0xd0, 0xff, 0xd9]);
        assert_eq!(
            validate_jpeg_container(&stuffed_and_restart).unwrap(),
            RadrootsBlossomRasterDimensions::new(1, 1).unwrap()
        );
    }

    #[test]
    fn get_debug_redacts_complete_body() {
        let get = observations(PNG).2;
        let debug = format!("{get:?}");
        assert!(debug.contains("body_length"));
        assert!(!debug.contains("89504e47"));
        assert_eq!(get.bytes(), PNG);
    }

    #[cfg(feature = "raster-decode")]
    struct FakeDecoder {
        dimensions: (u32, u32),
        total_bytes: u64,
        fail_limits: bool,
        fail_read: bool,
    }

    #[cfg(feature = "raster-decode")]
    impl ImageDecoder for FakeDecoder {
        fn dimensions(&self) -> (u32, u32) {
            self.dimensions
        }

        fn color_type(&self) -> ColorType {
            ColorType::Rgba8
        }

        fn total_bytes(&self) -> u64 {
            self.total_bytes
        }

        fn read_image(self, _buffer: &mut [u8]) -> ImageResult<()> {
            if self.fail_read {
                return Err(ImageError::IoError(std::io::Error::other(
                    "synthetic decode failure",
                )));
            }
            Ok(())
        }

        fn read_image_boxed(self: Box<Self>, buffer: &mut [u8]) -> ImageResult<()> {
            (*self).read_image(buffer)
        }

        fn set_limits(&mut self, _limits: Limits) -> ImageResult<()> {
            if self.fail_limits {
                return Err(ImageError::IoError(std::io::Error::other(
                    "synthetic limit failure",
                )));
            }
            Ok(())
        }
    }

    #[cfg(feature = "raster-decode")]
    #[test]
    fn decoder_authority_rejects_animation_resource_and_agreement_failures() {
        assert_eq!(
            RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES,
            80_000_000
        );
        reject_animation(false, false).unwrap();
        for (container_animated, decoder_animated) in [(true, false), (false, true), (true, true)] {
            assert_eq!(
                reject_animation(container_animated, decoder_animated)
                    .unwrap_err()
                    .code(),
                "publication_raster_animation_forbidden"
            );
        }

        let dimensions = RadrootsBlossomRasterDimensions::new(1, 1).unwrap();
        let decoder = |dimensions, total_bytes, fail_limits, fail_read| FakeDecoder {
            dimensions,
            total_bytes,
            fail_limits,
            fail_read,
        };
        assert_eq!(
            decode_complete_raster(decoder((2, 1), 0, false, false), dimensions)
                .unwrap_err()
                .code(),
            "publication_raster_container_dimension_mismatch"
        );
        assert_eq!(
            decode_complete_raster(decoder((1, 1), 0, false, false), dimensions).unwrap(),
            dimensions
        );
        assert_eq!(
            decode_complete_raster(
                decoder(
                    (1, 1),
                    RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES + 1,
                    false,
                    false,
                ),
                dimensions,
            )
            .unwrap_err()
            .code(),
            "publication_raster_decoded_byte_limit_exceeded"
        );
        assert_eq!(
            decode_complete_raster(decoder((1, 1), 0, true, false), dimensions)
                .unwrap_err()
                .code(),
            "publication_raster_decode_failed"
        );
        assert_eq!(
            decode_complete_raster(decoder((1, 1), 0, false, true), dimensions)
                .unwrap_err()
                .code(),
            "publication_raster_decode_failed"
        );
        assert_eq!(
            allocate_decoded_buffer(u64::MAX).unwrap_err().code(),
            "publication_raster_decode_allocation_failed"
        );
        assert_eq!(
            bounded_decoded_byte_length(None).unwrap_err().code(),
            "publication_raster_decode_failed"
        );
        assert_eq!(
            bounded_decoded_byte_length(Some(
                RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES + 1,
            ))
            .unwrap_err()
            .code(),
            "publication_raster_decoded_byte_limit_exceeded"
        );
        assert_eq!(
            bounded_decoded_byte_length(Some(
                RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES,
            ))
            .unwrap(),
            RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES
        );
        assert_eq!(
            bounded_jpeg_output_buffer_size(None).unwrap_err().code(),
            "publication_raster_decode_failed"
        );
        assert_eq!(bounded_jpeg_output_buffer_size(Some(0)).unwrap(), 0);

        let direct_decoder = decoder((1, 1), 0, false, false);
        assert_eq!(direct_decoder.color_type(), ColorType::Rgba8);
        Box::new(decoder((1, 1), 0, false, false))
            .read_image_boxed(&mut [])
            .unwrap();

        let jpeg = sequential_jpeg();
        let container = inspect_jpeg_container(&jpeg).unwrap();
        assert_eq!(container.dimensions, dimensions);
        assert_eq!(container.components, 3);
        decode_complete_jpeg(&jpeg, container).unwrap();

        let mut extended_sequential = jpeg.clone();
        let sof = extended_sequential
            .windows(2)
            .position(|window| window == b"\xff\xc0")
            .unwrap();
        extended_sequential[sof + 1] = 0xc1;
        let extended_container = inspect_jpeg_container(&extended_sequential).unwrap();
        decode_complete_jpeg(&extended_sequential, extended_container).unwrap();

        let mut progressive = jpeg.clone();
        progressive[sof + 1] = 0xc2;
        assert_eq!(
            inspect_jpeg_container(&progressive).unwrap_err().code(),
            "publication_jpeg_process_forbidden"
        );
        let mut twelve_bit = jpeg.clone();
        twelve_bit[sof + 4] = 12;
        assert_eq!(
            inspect_jpeg_container(&twelve_bit).unwrap_err().code(),
            "publication_jpeg_process_forbidden"
        );
        assert_eq!(
            inspect_jpeg_container(&malformed_dqt_jpeg())
                .unwrap_err()
                .code(),
            "invalid_publication_raster"
        );

        let scan = jpeg
            .windows(2)
            .position(|window| window == b"\xff\xda")
            .unwrap();
        let scan_length = usize::from(u16::from_be_bytes([jpeg[scan + 2], jpeg[scan + 3]]));
        let entropy_start = scan + 2 + scan_length;
        let entropy_end = jpeg.len() - 2;
        let entropy_length = entropy_end - entropy_start;
        for keep in [0, 1, entropy_length / 2, entropy_length - 1] {
            let mut truncated = jpeg[..entropy_start + keep].to_vec();
            truncated.extend_from_slice(b"\xff\xd9");
            let truncated_container = inspect_jpeg_container(&truncated).unwrap();
            assert_eq!(
                decode_complete_jpeg(&truncated, truncated_container)
                    .unwrap_err()
                    .code(),
                "publication_raster_decode_failed"
            );
        }

        let mismatched_container = JpegContainerInspection {
            dimensions: RadrootsBlossomRasterDimensions::new(2, 1).unwrap(),
            ..container
        };
        assert_eq!(
            decode_complete_jpeg(&jpeg, mismatched_container)
                .unwrap_err()
                .code(),
            "publication_raster_container_dimension_mismatch"
        );
        assert_eq!(
            decode_complete_jpeg(b"not jpeg", container)
                .unwrap_err()
                .code(),
            "publication_raster_decode_failed"
        );
        assert_eq!(
            decode_complete_jpeg(
                &jpeg,
                JpegContainerInspection {
                    components: 2,
                    ..container
                },
            )
            .unwrap_err()
            .code(),
            "publication_raster_container_dimension_mismatch"
        );

        let jpeg_options = strict_jpeg_decoder_options();
        assert!(jpeg_options.strict_mode());
        assert!(!jpeg_options.use_unsafe());
        assert_eq!(
            jpeg_options.max_width(),
            RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION as usize
        );
        assert_eq!(
            jpeg_options.max_height(),
            RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION as usize
        );
        assert_eq!(jpeg_options.jpeg_get_out_colorspace(), ColorSpace::RGB);
        assert_eq!(strict_jpeg_dimensions(Some((1, 1))).unwrap(), dimensions);
        assert_eq!(
            strict_jpeg_dimensions(None).unwrap_err().code(),
            "publication_raster_decode_failed"
        );

        let limits = raster_decode_limits();
        assert_eq!(
            limits.max_image_width,
            Some(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION)
        );
        assert_eq!(
            limits.max_image_height,
            Some(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION)
        );
        assert_eq!(
            limits.max_alloc,
            Some(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DECODED_BYTES)
        );
    }
}

use alloc::vec::Vec;
use core::fmt;
use sha2::{Digest, Sha256};

use crate::{
    RadrootsBlossomApprovedBlobUrl, RadrootsBlossomBlobDescriptor,
    RadrootsBlossomByteVerifiedDescriptor, RadrootsBlossomError, RadrootsBlossomMediaType,
    RadrootsBlossomSha256,
};

const _: () = assert!(usize::BITS <= u64::BITS);

pub const RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION: u16 = 1;
pub const RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES: u64 = 10_485_760;
pub const RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_DIMENSION: u32 = 16_384;
pub const RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_PIXELS: u64 = 20_000_000;

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
    const fn exact(self) -> Option<RadrootsBlossomRasterDimensions> {
        match self {
            Self::Unspecified => None,
            Self::Exact(dimensions) => Some(dimensions),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsBlossomRasterDecodeObservation {
    format: RadrootsBlossomRasterFormat,
    complete_bytes_sha256: RadrootsBlossomSha256,
    complete_byte_length: u64,
    dimensions: RadrootsBlossomRasterDimensions,
}

impl RadrootsBlossomRasterDecodeObservation {
    pub fn new(
        format: RadrootsBlossomRasterFormat,
        complete_bytes_sha256: RadrootsBlossomSha256,
        complete_byte_length: u64,
        frame_count: u32,
        width: u32,
        height: u32,
    ) -> Result<Self, RadrootsBlossomError> {
        if frame_count != 1 {
            return Err(RadrootsBlossomError::PublicationRasterFrameCountMismatch {
                actual: frame_count,
            });
        }
        Ok(Self {
            format,
            complete_bytes_sha256,
            complete_byte_length,
            dimensions: RadrootsBlossomRasterDimensions::new(width, height)?,
        })
    }

    pub const fn format(&self) -> RadrootsBlossomRasterFormat {
        self.format
    }

    pub const fn complete_bytes_sha256(&self) -> RadrootsBlossomSha256 {
        self.complete_bytes_sha256
    }

    pub const fn complete_byte_length(&self) -> u64 {
        self.complete_byte_length
    }

    pub const fn dimensions(&self) -> RadrootsBlossomRasterDimensions {
        self.dimensions
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
}

pub fn verify_publication_readiness(
    authored_descriptor: &RadrootsBlossomByteVerifiedDescriptor,
    exact_authored_bytes: &[u8],
    authored_dimensions: RadrootsBlossomAuthoredRasterDimensions,
    upload: &RadrootsBlossomBud02UploadObservation,
    head: &RadrootsBlossomBud01HeadObservation,
    get: &RadrootsBlossomBud01GetObservation,
    decode: &RadrootsBlossomRasterDecodeObservation,
) -> Result<RadrootsBlossomPublicationReadinessEvidence, RadrootsBlossomError> {
    let expected_url = authored_descriptor.url();
    let expected_hash = authored_descriptor.sha256();
    let expected_size = authored_descriptor.size();
    let expected_media_type = authored_descriptor.media_type();

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
    let retrieved_hash = RadrootsBlossomSha256::digest(get.bytes());
    if retrieved_hash != expected_hash {
        return Err(RadrootsBlossomError::PublicationRetrievedBytesHashMismatch);
    }
    if get.bytes() != exact_authored_bytes {
        return Err(RadrootsBlossomError::PublicationRetrievedBytesMismatch);
    }

    let expected_format = RadrootsBlossomRasterFormat::from_media_type(expected_media_type)?;
    let container_dimensions = validate_raster_container(get.bytes(), expected_format)?;
    if decode.format() != expected_format {
        return Err(RadrootsBlossomError::PublicationRasterDecodeFormatMismatch);
    }
    if decode.complete_byte_length() != expected_size {
        return Err(
            RadrootsBlossomError::PublicationRasterDecodeLengthMismatch {
                expected: expected_size,
                actual: decode.complete_byte_length(),
            },
        );
    }
    if decode.complete_bytes_sha256() != expected_hash {
        return Err(RadrootsBlossomError::PublicationRasterDecodeHashMismatch);
    }
    if container_dimensions.is_some_and(|dimensions| dimensions != decode.dimensions()) {
        return Err(RadrootsBlossomError::PublicationRasterContainerDimensionMismatch);
    }
    if authored_dimensions
        .exact()
        .is_some_and(|dimensions| dimensions != decode.dimensions())
    {
        return Err(RadrootsBlossomError::PublicationAuthoredRasterDimensionMismatch);
    }

    let evidence_digest = evidence_digest(
        authored_descriptor,
        expected_format,
        decode.dimensions(),
        upload,
    );
    Ok(RadrootsBlossomPublicationReadinessEvidence {
        url: expected_url.clone(),
        sha256: expected_hash,
        size: expected_size,
        media_type: expected_media_type.clone(),
        raster_format: expected_format,
        dimensions: decode.dimensions(),
        bud02_status: upload.status(),
        uploaded: upload_descriptor.uploaded(),
        evidence_digest,
    })
}

fn evidence_digest(
    descriptor: &RadrootsBlossomByteVerifiedDescriptor,
    format: RadrootsBlossomRasterFormat,
    dimensions: RadrootsBlossomRasterDimensions,
    upload: &RadrootsBlossomBud02UploadObservation,
) -> RadrootsBlossomPublicationReadinessEvidenceDigest {
    let mut hasher = Sha256::new();
    hasher.update(READINESS_EVIDENCE_DIGEST_DOMAIN);
    hasher.update(RADROOTS_BLOSSOM_PUBLICATION_READINESS_POLICY_VERSION.to_be_bytes());
    update_length_prefixed(&mut hasher, descriptor.url().as_str().as_bytes());
    hasher.update(descriptor.sha256().as_bytes());
    hasher.update(descriptor.size().to_be_bytes());
    update_length_prefixed(&mut hasher, descriptor.media_type().as_str().as_bytes());
    hasher.update([format.digest_code()]);
    hasher.update(dimensions.width().to_be_bytes());
    hasher.update(dimensions.height().to_be_bytes());
    hasher.update(upload.status().as_u16().to_be_bytes());
    hasher.update(200_u16.to_be_bytes());
    hasher.update(200_u16.to_be_bytes());
    hasher.update(upload.descriptor().descriptor().uploaded().to_be_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest);
    RadrootsBlossomPublicationReadinessEvidenceDigest(RadrootsBlossomSha256::from_bytes(bytes))
}

fn update_length_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn validate_raster_container(
    bytes: &[u8],
    format: RadrootsBlossomRasterFormat,
) -> Result<Option<RadrootsBlossomRasterDimensions>, RadrootsBlossomError> {
    match format {
        RadrootsBlossomRasterFormat::Jpeg => validate_jpeg_container(bytes).map(Some),
        RadrootsBlossomRasterFormat::Png => validate_png_container(bytes).map(Some),
        RadrootsBlossomRasterFormat::StillWebP => validate_webp_container(bytes),
    }
}

fn invalid_raster<T>() -> Result<T, RadrootsBlossomError> {
    Err(RadrootsBlossomError::InvalidPublicationRaster)
}

fn validate_png_container(
    bytes: &[u8],
) -> Result<RadrootsBlossomRasterDimensions, RadrootsBlossomError> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if !bytes.starts_with(SIGNATURE) {
        return invalid_raster();
    }
    let mut position = SIGNATURE.len();
    let mut dimensions = None;
    let mut has_image_data = false;
    while position < bytes.len() {
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
                dimensions = Some(RadrootsBlossomRasterDimensions::new(width, height)?);
            }
            b"IHDR" => return invalid_raster(),
            b"IDAT" if dimensions.is_some() => has_image_data = true,
            b"acTL" | b"fcTL" | b"fdAT" => {
                return Err(RadrootsBlossomError::PublicationRasterFrameCountMismatch {
                    actual: 2,
                });
            }
            b"IEND" if data.is_empty() && has_image_data && position == bytes.len() => {
                return dimensions.ok_or(RadrootsBlossomError::InvalidPublicationRaster);
            }
            b"IEND" => return invalid_raster(),
            _ if dimensions.is_none() => return invalid_raster(),
            _ => {}
        }
    }
    invalid_raster()
}

fn validate_webp_container(
    bytes: &[u8],
) -> Result<Option<RadrootsBlossomRasterDimensions>, RadrootsBlossomError> {
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
    while position < bytes.len() {
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
            b"ANIM" | b"ANMF" => {
                return Err(RadrootsBlossomError::PublicationRasterFrameCountMismatch {
                    actual: 2,
                });
            }
            b"VP8X" if data.len() == 10 => {
                if data[0] & 0b0000_0010 != 0 {
                    return Err(RadrootsBlossomError::PublicationRasterFrameCountMismatch {
                        actual: 2,
                    });
                }
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
    if position != bytes.len() || primary_chunks != 1 {
        return invalid_raster();
    }
    Ok(dimensions)
}

fn read_u24_le(bytes: &[u8]) -> u32 {
    u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16)
}

fn validate_jpeg_container(
    bytes: &[u8],
) -> Result<RadrootsBlossomRasterDimensions, RadrootsBlossomError> {
    if bytes.len() < 4 || !bytes.starts_with(b"\xff\xd8") {
        return invalid_raster();
    }
    let mut position = 2_usize;
    let mut dimensions = None;
    loop {
        let marker_start = position;
        if bytes.get(position) != Some(&0xff) {
            return invalid_raster();
        }
        while bytes.get(position) == Some(&0xff) {
            position += 1;
        }
        let marker = *bytes
            .get(position)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        position += 1;
        match marker {
            0xd9 if position == bytes.len() => {
                return dimensions.ok_or(RadrootsBlossomError::InvalidPublicationRaster);
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
            let height = u32::from(u16::from_be_bytes([data[1], data[2]]));
            let width = u32::from(u16::from_be_bytes([data[3], data[4]]));
            dimensions = Some(RadrootsBlossomRasterDimensions::new(width, height)?);
        }

        if marker == 0xda {
            position = jpeg_scan_end(bytes, position)?;
            if position <= marker_start {
                return invalid_raster();
            }
        }
    }
}

fn is_jpeg_start_of_frame(marker: u8) -> bool {
    matches!(
        marker,
        0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf
    )
}

fn jpeg_scan_end(bytes: &[u8], mut position: usize) -> Result<usize, RadrootsBlossomError> {
    while position < bytes.len() {
        if bytes[position] != 0xff {
            position += 1;
            continue;
        }
        let marker_start = position;
        while bytes.get(position) == Some(&0xff) {
            position += 1;
        }
        let marker = *bytes
            .get(position)
            .ok_or(RadrootsBlossomError::InvalidPublicationRaster)?;
        match marker {
            0x00 | 0xd0..=0xd7 => position += 1,
            _ => return Ok(marker_start),
        }
    }
    invalid_raster()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, string::ToString};

    const PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60,
        0xf8, 0xcf, 0xf0, 0x00, 0x00, 0x04, 0x01, 0x01, 0x00, 0x18, 0xdd, 0x8d, 0xb1, 0x00, 0x00,
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
        RadrootsBlossomRasterDecodeObservation,
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
        let decode = RadrootsBlossomRasterDecodeObservation::new(
            RadrootsBlossomRasterFormat::Png,
            RadrootsBlossomSha256::digest(bytes),
            bytes.len() as u64,
            1,
            1,
            1,
        )
        .unwrap();
        (upload, head, get, decode)
    }

    #[test]
    fn publication_readiness_accepts_exact_complete_observations() {
        let expected = verified(PNG);
        let (upload, head, get, decode) = observations(PNG);
        let evidence = verify_publication_readiness(
            &expected,
            PNG,
            RadrootsBlossomAuthoredRasterDimensions::Exact(
                RadrootsBlossomRasterDimensions::new(1, 1).unwrap(),
            ),
            &upload,
            &head,
            &get,
            &decode,
        )
        .unwrap();
        assert_eq!(
            evidence.url().as_str(),
            "https://cdn.example/0d1c097e006a87476e84014ba5842f04c725ed2fc5a081743ab2b5bf13a538b9.png"
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
            "c52edeba688fa36c7963a478a35ff78504d7dd79a637c67f93d5acb635110660"
        );
        assert_eq!(
            evidence.evidence_digest().as_sha256().to_string(),
            evidence.evidence_digest().to_string()
        );
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
    fn observation_constructors_reject_invalid_status_frames_and_dimensions() {
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
        for (frames, width, height, code) in [
            (2, 1, 1, "publication_raster_frame_count_mismatch"),
            (1, 0, 1, "publication_raster_dimensions_out_of_range"),
            (1, 5_000, 5_000, "publication_raster_pixel_limit_exceeded"),
        ] {
            assert_eq!(
                RadrootsBlossomRasterDecodeObservation::new(
                    RadrootsBlossomRasterFormat::Png,
                    RadrootsBlossomSha256::digest(PNG),
                    PNG.len() as u64,
                    frames,
                    width,
                    height,
                )
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
            "publication_raster_frame_count_mismatch"
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
            "publication_raster_frame_count_mismatch"
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
    fn get_debug_redacts_complete_body() {
        let get = observations(PNG).2;
        let debug = format!("{get:?}");
        assert!(debug.contains("body_length"));
        assert!(!debug.contains("89504e47"));
        assert_eq!(get.bytes(), PNG);
    }
}

#[cfg(feature = "serde")]
use alloc::string::String;
use alloc::{collections::BTreeMap, vec::Vec};
use core::{fmt, str::FromStr};
use mediatype::{MediaTypeBuf, ReadParams};

use crate::{
    RadrootsBlossomApprovedBlobUrl, RadrootsBlossomBlobUrl, RadrootsBlossomError,
    RadrootsBlossomSha256,
};

const _: () = assert!(usize::BITS <= u64::BITS);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RadrootsBlossomMediaType(MediaTypeBuf);

impl RadrootsBlossomMediaType {
    pub fn parse(value: &str) -> Result<Self, RadrootsBlossomError> {
        let parsed =
            MediaTypeBuf::from_str(value).map_err(|_| RadrootsBlossomError::InvalidMediaType)?;
        if parsed.as_str() != value || parsed.ty().as_str() == "*" || parsed.subty().as_str() == "*"
        {
            return Err(RadrootsBlossomError::InvalidMediaType);
        }
        let canonical = parsed.canonicalize();
        let mut unique_params = BTreeMap::new();
        for (name, value) in canonical.params() {
            if unique_params.insert(name, value).is_some() {
                return Err(RadrootsBlossomError::InvalidMediaType);
            }
        }
        let ordered_params = unique_params.into_iter().collect::<Vec<_>>();
        Ok(Self(MediaTypeBuf::from_parts(
            canonical.ty(),
            canonical.subty(),
            canonical.suffix(),
            &ordered_params,
        )))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for RadrootsBlossomMediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RadrootsBlossomMediaType {
    type Err = RadrootsBlossomError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for RadrootsBlossomMediaType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsBlossomMediaType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsBlossomBlobDescriptor {
    url: RadrootsBlossomBlobUrl,
    sha256: RadrootsBlossomSha256,
    size: u64,
    media_type: RadrootsBlossomMediaType,
    uploaded: u64,
}

impl RadrootsBlossomBlobDescriptor {
    pub fn new(
        url: RadrootsBlossomBlobUrl,
        sha256: RadrootsBlossomSha256,
        size: u64,
        media_type: RadrootsBlossomMediaType,
        uploaded: u64,
    ) -> Result<Self, RadrootsBlossomError> {
        if url.hash_path().extension().is_none() {
            return Err(RadrootsBlossomError::DescriptorExtensionRequired);
        }
        if url.hash_path().hash() != sha256 {
            return Err(RadrootsBlossomError::DescriptorHashMismatch);
        }
        Ok(Self {
            url,
            sha256,
            size,
            media_type,
            uploaded,
        })
    }

    pub fn url(&self) -> &RadrootsBlossomBlobUrl {
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

    pub const fn uploaded(&self) -> u64 {
        self.uploaded
    }

    pub fn approve_reference(
        self,
    ) -> Result<RadrootsBlossomApprovedDescriptor, RadrootsBlossomError> {
        let approved_url = self.url.clone().approve()?;
        Ok(RadrootsBlossomApprovedDescriptor {
            descriptor: self,
            approved_url,
        })
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize)]
struct DescriptorRef<'a> {
    url: &'a RadrootsBlossomBlobUrl,
    sha256: RadrootsBlossomSha256,
    size: u64,
    #[serde(rename = "type")]
    media_type: &'a RadrootsBlossomMediaType,
    uploaded: u64,
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct DescriptorWire {
    url: RadrootsBlossomBlobUrl,
    sha256: RadrootsBlossomSha256,
    size: u64,
    #[serde(rename = "type")]
    media_type: RadrootsBlossomMediaType,
    uploaded: u64,
}

#[cfg(feature = "serde")]
impl serde::Serialize for RadrootsBlossomBlobDescriptor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        DescriptorRef {
            url: &self.url,
            sha256: self.sha256,
            size: self.size,
            media_type: &self.media_type,
            uploaded: self.uploaded,
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsBlossomBlobDescriptor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = DescriptorWire::deserialize(deserializer)?;
        Self::new(
            wire.url,
            wire.sha256,
            wire.size,
            wire.media_type,
            wire.uploaded,
        )
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsBlossomApprovedDescriptor {
    descriptor: RadrootsBlossomBlobDescriptor,
    approved_url: RadrootsBlossomApprovedBlobUrl,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsBlossomByteCommitment {
    sha256: RadrootsBlossomSha256,
    size: u64,
    media_type: RadrootsBlossomMediaType,
}

impl RadrootsBlossomByteCommitment {
    pub fn from_bytes(bytes: &[u8], media_type: RadrootsBlossomMediaType) -> Self {
        Self {
            sha256: RadrootsBlossomSha256::digest(bytes),
            size: bytes.len() as u64,
            media_type,
        }
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
}

impl RadrootsBlossomApprovedDescriptor {
    pub fn descriptor(&self) -> &RadrootsBlossomBlobDescriptor {
        &self.descriptor
    }

    pub fn url(&self) -> &RadrootsBlossomApprovedBlobUrl {
        &self.approved_url
    }

    pub fn into_descriptor(self) -> RadrootsBlossomBlobDescriptor {
        self.descriptor
    }

    pub fn verify_bytes(
        self,
        bytes: &[u8],
        approved_media_type: &RadrootsBlossomMediaType,
    ) -> Result<RadrootsBlossomByteVerifiedDescriptor, RadrootsBlossomError> {
        let commitment =
            RadrootsBlossomByteCommitment::from_bytes(bytes, approved_media_type.clone());
        self.verify_commitment(&commitment)
    }

    pub fn verify_commitment(
        self,
        commitment: &RadrootsBlossomByteCommitment,
    ) -> Result<RadrootsBlossomByteVerifiedDescriptor, RadrootsBlossomError> {
        if self.descriptor.size != commitment.size {
            return Err(RadrootsBlossomError::BlobSizeMismatch {
                expected: self.descriptor.size,
                actual: commitment.size,
            });
        }
        if self.descriptor.media_type != commitment.media_type {
            return Err(RadrootsBlossomError::BlobMediaTypeMismatch);
        }
        if self.descriptor.sha256 != commitment.sha256 {
            return Err(RadrootsBlossomError::BlobHashMismatch);
        }
        Ok(RadrootsBlossomByteVerifiedDescriptor(self))
    }
}

/// An approved descriptor whose hash, size, and media type match supplied bytes.
///
/// This state does not attest that a network upload occurred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsBlossomByteVerifiedDescriptor(RadrootsBlossomApprovedDescriptor);

impl RadrootsBlossomByteVerifiedDescriptor {
    pub fn descriptor(&self) -> &RadrootsBlossomBlobDescriptor {
        self.0.descriptor()
    }

    pub fn url(&self) -> &RadrootsBlossomApprovedBlobUrl {
        self.0.url()
    }

    pub const fn sha256(&self) -> RadrootsBlossomSha256 {
        self.0.descriptor.sha256
    }

    pub const fn size(&self) -> u64 {
        self.0.descriptor.size
    }

    pub fn media_type(&self) -> &RadrootsBlossomMediaType {
        &self.0.descriptor.media_type
    }

    pub fn into_descriptor(self) -> RadrootsBlossomBlobDescriptor {
        self.0.into_descriptor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, string::ToString};

    const HELLO_HASH: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn descriptor(origin: &str, bytes: &[u8], media_type: &str) -> RadrootsBlossomBlobDescriptor {
        let hash = RadrootsBlossomSha256::digest(bytes);
        RadrootsBlossomBlobDescriptor::new(
            RadrootsBlossomBlobUrl::parse(&format!("{origin}/{hash}.txt")).unwrap(),
            hash,
            u64::try_from(bytes.len()).unwrap(),
            RadrootsBlossomMediaType::parse(media_type).unwrap(),
            1_725_105_921,
        )
        .unwrap()
    }

    #[test]
    fn media_type_canonicalizes_parameters_and_compares_case_insensitively() {
        let lower =
            RadrootsBlossomMediaType::parse("image/svg+xml; profile=web; charset=UTF-8").unwrap();
        let upper = RadrootsBlossomMediaType::from_str("IMAGE/SVG+XML; CHARSET=UTF-8; PROFILE=web")
            .unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.as_str(), "image/svg+xml; charset=UTF-8; profile=web");
        assert_eq!(upper.as_str(), lower.as_str());
        assert_eq!(lower.to_string(), lower.as_str());
        let json = serde_json::to_string(&lower).unwrap();
        assert_eq!(
            serde_json::from_str::<RadrootsBlossomMediaType>(&json).unwrap(),
            lower
        );
    }

    #[test]
    fn media_type_rejects_invalid_values_and_types() {
        for value in [
            "",
            "image",
            "image/",
            "*/*",
            "image/*",
            "image/png; profile=a; PROFILE=b",
            " image/png",
            "image/png ",
            "image/png\n",
        ] {
            assert_eq!(
                RadrootsBlossomMediaType::parse(value),
                Err(RadrootsBlossomError::InvalidMediaType)
            );
        }
        assert!(serde_json::from_str::<RadrootsBlossomMediaType>("42").is_err());
    }

    #[test]
    fn descriptor_requires_extension_and_matching_url_hash() {
        let hash = RadrootsBlossomSha256::from_hex(HELLO_HASH).unwrap();
        let media_type = RadrootsBlossomMediaType::parse("text/plain").unwrap();
        let no_extension =
            RadrootsBlossomBlobUrl::parse(&format!("https://cdn.example.com/{HELLO_HASH}"))
                .unwrap();
        assert_eq!(
            RadrootsBlossomBlobDescriptor::new(no_extension, hash, 5, media_type.clone(), 1),
            Err(RadrootsBlossomError::DescriptorExtensionRequired)
        );
        let wrong_hash = RadrootsBlossomSha256::digest(b"wrong");
        let url =
            RadrootsBlossomBlobUrl::parse(&format!("https://cdn.example.com/{HELLO_HASH}.txt"))
                .unwrap();
        assert_eq!(
            RadrootsBlossomBlobDescriptor::new(url, wrong_hash, 5, media_type, 1),
            Err(RadrootsBlossomError::DescriptorHashMismatch)
        );
    }

    #[test]
    fn descriptor_serde_roundtrip_tolerates_extension_fields() {
        let raw = format!(
            r#"{{"url":"https://cdn.example.com/{HELLO_HASH}.txt","sha256":"{HELLO_HASH}","size":5,"type":"text/plain","uploaded":1725105921,"magnet":"magnet:?xt=urn:test"}}"#
        );
        let parsed: RadrootsBlossomBlobDescriptor = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.url().hash_path().hash().to_string(), HELLO_HASH);
        assert_eq!(parsed.sha256().to_string(), HELLO_HASH);
        assert_eq!(parsed.size(), 5);
        assert_eq!(parsed.media_type().as_str(), "text/plain");
        assert_eq!(parsed.uploaded(), 1_725_105_921);
        let encoded = serde_json::to_value(&parsed).unwrap();
        assert_eq!(encoded["type"], "text/plain");
        assert!(encoded.get("magnet").is_none());
    }

    #[test]
    fn descriptor_deserialize_revalidates_invariants() {
        let wrong_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let raw = format!(
            r#"{{"url":"https://cdn.example.com/{HELLO_HASH}.txt","sha256":"{wrong_hash}","size":5,"type":"text/plain","uploaded":1}}"#
        );
        assert!(serde_json::from_str::<RadrootsBlossomBlobDescriptor>(&raw).is_err());
    }

    #[test]
    fn byte_verified_descriptor_requires_approved_url_size_type_and_hash() {
        let media_type = RadrootsBlossomMediaType::parse("text/plain").unwrap();
        let commitment = RadrootsBlossomByteCommitment::from_bytes(b"hello", media_type.clone());
        assert_eq!(commitment.sha256().to_string(), HELLO_HASH);
        assert_eq!(commitment.size(), 5);
        assert_eq!(commitment.media_type(), &media_type);
        let approved = descriptor("https://cdn.example.com", b"hello", "text/plain")
            .approve_reference()
            .unwrap();
        assert_eq!(
            approved.url().as_str(),
            approved.descriptor().url().as_str()
        );
        let verified = approved.clone().verify_commitment(&commitment).unwrap();
        assert_eq!(verified.url().as_str(), approved.url().as_str());
        assert_eq!(verified.descriptor(), approved.descriptor());
        assert_eq!(verified.sha256().to_string(), HELLO_HASH);
        assert_eq!(verified.size(), 5);
        assert_eq!(verified.media_type(), &media_type);
        assert_eq!(verified.clone().into_descriptor(), *approved.descriptor());
        assert_eq!(approved.clone().into_descriptor(), *approved.descriptor());

        assert_eq!(
            descriptor("https://cdn.example.com", b"hello", "text/plain")
                .approve_reference()
                .unwrap()
                .verify_bytes(b"hell", &media_type),
            Err(RadrootsBlossomError::BlobSizeMismatch {
                expected: 5,
                actual: 4,
            })
        );
        let image_type = RadrootsBlossomMediaType::parse("image/png").unwrap();
        assert_eq!(
            descriptor("https://cdn.example.com", b"hello", "text/plain")
                .approve_reference()
                .unwrap()
                .verify_bytes(b"hello", &image_type),
            Err(RadrootsBlossomError::BlobMediaTypeMismatch)
        );
        let hash_mismatch = RadrootsBlossomBlobDescriptor::new(
            RadrootsBlossomBlobUrl::parse(&format!(
                "https://cdn.example.com/{}.txt",
                RadrootsBlossomSha256::digest(b"world")
            ))
            .unwrap(),
            RadrootsBlossomSha256::digest(b"world"),
            5,
            media_type.clone(),
            1,
        )
        .unwrap()
        .approve_reference()
        .unwrap();
        assert_eq!(
            hash_mismatch.verify_bytes(b"hello", &media_type),
            Err(RadrootsBlossomError::BlobHashMismatch)
        );
    }

    #[test]
    fn empty_blob_can_be_verified_with_explicit_default_media_type() {
        let media_type = RadrootsBlossomMediaType::parse("application/octet-stream").unwrap();
        let verified = descriptor("http://localhost:3000", b"", "application/octet-stream")
            .approve_reference()
            .unwrap()
            .verify_bytes(b"", &media_type)
            .unwrap();
        assert_eq!(verified.size(), 0);
    }

    #[test]
    fn public_http_descriptor_cannot_advance_to_approved() {
        assert_eq!(
            descriptor("http://cdn.example.com", b"hello", "text/plain").approve_reference(),
            Err(RadrootsBlossomError::InsecureBlobUrl)
        );
    }
}

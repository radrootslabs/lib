use crate::{
    Error, MediaType, Sha256,
    url::{ApprovedBlobUrl, BlobUrl},
};

const _: () = assert!(usize::BITS <= u64::BITS);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobDescriptor {
    url: BlobUrl,
    sha256: Sha256,
    size: u64,
    media_type: MediaType,
    uploaded: u64,
}

impl BlobDescriptor {
    pub fn new(
        url: BlobUrl,
        sha256: Sha256,
        size: u64,
        media_type: MediaType,
        uploaded: u64,
    ) -> Result<Self, Error> {
        if url.hash_path().extension().is_none() {
            return Err(Error::DescriptorExtensionRequired);
        }
        if url.hash_path().hash() != sha256 {
            return Err(Error::DescriptorHashMismatch);
        }
        Ok(Self {
            url,
            sha256,
            size,
            media_type,
            uploaded,
        })
    }

    pub fn url(&self) -> &BlobUrl {
        &self.url
    }

    pub const fn sha256(&self) -> Sha256 {
        self.sha256
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    pub const fn uploaded(&self) -> u64 {
        self.uploaded
    }

    pub fn approve_reference(self) -> Result<ApprovedDescriptor, Error> {
        let approved_url = self.url.clone().approve()?;
        Ok(ApprovedDescriptor {
            descriptor: self,
            approved_url,
        })
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize)]
struct DescriptorRef<'a> {
    url: &'a BlobUrl,
    sha256: Sha256,
    size: u64,
    #[serde(rename = "type")]
    media_type: &'a MediaType,
    uploaded: u64,
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct DescriptorWire {
    url: BlobUrl,
    sha256: Sha256,
    size: u64,
    #[serde(rename = "type")]
    media_type: MediaType,
    uploaded: u64,
}

#[cfg(feature = "serde")]
impl serde::Serialize for BlobDescriptor {
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
impl<'de> serde::Deserialize<'de> for BlobDescriptor {
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
pub struct ApprovedDescriptor {
    descriptor: BlobDescriptor,
    approved_url: ApprovedBlobUrl,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ByteCommitment {
    sha256: Sha256,
    size: u64,
    media_type: MediaType,
}

impl ByteCommitment {
    pub fn from_bytes(bytes: &[u8], media_type: MediaType) -> Self {
        Self {
            sha256: Sha256::digest(bytes),
            size: bytes.len() as u64,
            media_type,
        }
    }

    pub const fn sha256(&self) -> Sha256 {
        self.sha256
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }
}

impl ApprovedDescriptor {
    pub fn descriptor(&self) -> &BlobDescriptor {
        &self.descriptor
    }

    pub fn url(&self) -> &ApprovedBlobUrl {
        &self.approved_url
    }

    pub fn into_descriptor(self) -> BlobDescriptor {
        self.descriptor
    }

    pub fn verify_bytes(
        self,
        bytes: &[u8],
        approved_media_type: &MediaType,
    ) -> Result<ByteVerifiedDescriptor, Error> {
        let commitment = ByteCommitment::from_bytes(bytes, approved_media_type.clone());
        self.verify_commitment(&commitment)
    }

    pub fn verify_commitment(
        self,
        commitment: &ByteCommitment,
    ) -> Result<ByteVerifiedDescriptor, Error> {
        if self.descriptor.size != commitment.size {
            return Err(Error::BlobSizeMismatch {
                expected: self.descriptor.size,
                actual: commitment.size,
            });
        }
        if self.descriptor.media_type != commitment.media_type {
            return Err(Error::BlobMediaTypeMismatch);
        }
        if self.descriptor.sha256 != commitment.sha256 {
            return Err(Error::BlobHashMismatch);
        }
        Ok(ByteVerifiedDescriptor(self))
    }
}

/// An approved descriptor whose hash, size, and media type match supplied bytes.
///
/// This state does not attest that a network upload occurred.
/// Its private representation prevents callers from forging the typestate:
///
/// ```compile_fail
/// use radroots_blossom::{ByteVerifiedDescriptor, descriptor::ApprovedDescriptor};
///
/// fn forge(approved: ApprovedDescriptor) -> ByteVerifiedDescriptor {
///     ByteVerifiedDescriptor(approved)
/// }
/// ```
///
/// Construction is only available through descriptor verification:
///
/// ```compile_fail
/// use radroots_blossom::{ByteVerifiedDescriptor, descriptor::ApprovedDescriptor};
///
/// fn bypass_verification(approved: ApprovedDescriptor) -> ByteVerifiedDescriptor {
///     ByteVerifiedDescriptor::new(approved)
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ByteVerifiedDescriptor(ApprovedDescriptor);

impl ByteVerifiedDescriptor {
    pub fn descriptor(&self) -> &BlobDescriptor {
        self.0.descriptor()
    }

    pub fn url(&self) -> &ApprovedBlobUrl {
        self.0.url()
    }

    pub const fn sha256(&self) -> Sha256 {
        self.0.descriptor.sha256
    }

    pub const fn size(&self) -> u64 {
        self.0.descriptor.size
    }

    pub fn media_type(&self) -> &MediaType {
        &self.0.descriptor.media_type
    }

    pub fn into_descriptor(self) -> BlobDescriptor {
        self.0.into_descriptor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, string::ToString};
    use core::str::FromStr;

    const HELLO_HASH: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn descriptor(origin: &str, bytes: &[u8], media_type: &str) -> BlobDescriptor {
        let hash = Sha256::digest(bytes);
        BlobDescriptor::new(
            BlobUrl::parse(&format!("{origin}/{hash}.txt")).unwrap(),
            hash,
            u64::try_from(bytes.len()).unwrap(),
            MediaType::parse(media_type).unwrap(),
            1_725_105_921,
        )
        .unwrap()
    }

    #[test]
    fn media_type_canonicalizes_parameters_and_compares_case_insensitively() {
        let lower = MediaType::parse("image/svg+xml; profile=web; charset=UTF-8").unwrap();
        let upper = MediaType::from_str("IMAGE/SVG+XML; CHARSET=UTF-8; PROFILE=web").unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.as_str(), "image/svg+xml; charset=UTF-8; profile=web");
        assert_eq!(upper.as_str(), lower.as_str());
        assert_eq!(lower.to_string(), lower.as_str());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn media_type_serde_round_trips_and_revalidates() {
        let media_type = MediaType::parse("image/svg+xml; charset=UTF-8").unwrap();
        let json = serde_json::to_string(&media_type).unwrap();
        assert_eq!(
            serde_json::from_str::<MediaType>(&json).unwrap(),
            media_type
        );
        assert!(serde_json::from_str::<MediaType>("42").is_err());
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
            assert_eq!(MediaType::parse(value), Err(Error::InvalidMediaType));
        }
    }

    #[test]
    fn descriptor_requires_extension_and_matching_url_hash() {
        let hash = Sha256::from_hex(HELLO_HASH).unwrap();
        let media_type = MediaType::parse("text/plain").unwrap();
        let no_extension =
            BlobUrl::parse(&format!("https://cdn.example.com/{HELLO_HASH}")).unwrap();
        assert_eq!(
            BlobDescriptor::new(no_extension, hash, 5, media_type.clone(), 1),
            Err(Error::DescriptorExtensionRequired)
        );
        let wrong_hash = Sha256::digest(b"wrong");
        let url = BlobUrl::parse(&format!("https://cdn.example.com/{HELLO_HASH}.txt")).unwrap();
        assert_eq!(
            BlobDescriptor::new(url, wrong_hash, 5, media_type, 1),
            Err(Error::DescriptorHashMismatch)
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn descriptor_serde_roundtrip_tolerates_extension_fields() {
        let raw = format!(
            r#"{{"url":"https://cdn.example.com/{HELLO_HASH}.txt","sha256":"{HELLO_HASH}","size":5,"type":"text/plain","uploaded":1725105921,"magnet":"magnet:?xt=urn:test"}}"#
        );
        let parsed: BlobDescriptor = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.url().hash_path().hash().to_string(), HELLO_HASH);
        assert_eq!(parsed.sha256().to_string(), HELLO_HASH);
        assert_eq!(parsed.size(), 5);
        assert_eq!(parsed.media_type().as_str(), "text/plain");
        assert_eq!(parsed.uploaded(), 1_725_105_921);
        let encoded = serde_json::to_value(&parsed).unwrap();
        assert_eq!(encoded["type"], "text/plain");
        assert!(encoded.get("magnet").is_none());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn descriptor_deserialize_revalidates_invariants() {
        let wrong_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let raw = format!(
            r#"{{"url":"https://cdn.example.com/{HELLO_HASH}.txt","sha256":"{wrong_hash}","size":5,"type":"text/plain","uploaded":1}}"#
        );
        assert!(serde_json::from_str::<BlobDescriptor>(&raw).is_err());
    }

    #[test]
    fn byte_verified_descriptor_requires_approved_url_size_type_and_hash() {
        let media_type = MediaType::parse("text/plain").unwrap();
        let commitment = ByteCommitment::from_bytes(b"hello", media_type.clone());
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
            Err(Error::BlobSizeMismatch {
                expected: 5,
                actual: 4,
            })
        );
        let image_type = MediaType::parse("image/png").unwrap();
        assert_eq!(
            descriptor("https://cdn.example.com", b"hello", "text/plain")
                .approve_reference()
                .unwrap()
                .verify_bytes(b"hello", &image_type),
            Err(Error::BlobMediaTypeMismatch)
        );
        let hash_mismatch = BlobDescriptor::new(
            BlobUrl::parse(&format!(
                "https://cdn.example.com/{}.txt",
                Sha256::digest(b"world")
            ))
            .unwrap(),
            Sha256::digest(b"world"),
            5,
            media_type.clone(),
            1,
        )
        .unwrap()
        .approve_reference()
        .unwrap();
        assert_eq!(
            hash_mismatch.verify_bytes(b"hello", &media_type),
            Err(Error::BlobHashMismatch)
        );
    }

    #[test]
    fn empty_blob_can_be_verified_with_explicit_default_media_type() {
        let media_type = MediaType::parse("application/octet-stream").unwrap();
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
            Err(Error::InsecureBlobUrl)
        );
    }
}

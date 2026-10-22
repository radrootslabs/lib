use super::{ListingError, text};
use alloc::string::String;
use core::fmt;

/// Bounded public object-reference claims, without network or decoder authority.
/// The digest is a declared SHA-256 value, not evidence of obtained file bytes.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Media {
    url: String,
    #[cfg_attr(feature = "serde", serde(serialize_with = "serialize_digest"))]
    sha256: [u8; 32],
    mime: String,
    alt: String,
}

impl Media {
    /// Bounds all claims before copying. URL/MIME claims are nonempty and kept
    /// exactly; transport approval and actual content validation belong to use.
    pub fn new(url: &str, sha256: [u8; 32], mime: &str, alt: &str) -> Result<Self, ListingError> {
        text(url, 2048, false)?;
        text(mime, 80, false)?;
        text(alt, 240, true)?;
        Ok(Self {
            url: url.into(),
            sha256,
            mime: mime.into(),
            alt: alt.into(),
        })
    }

    /// Admits exactly64 lowercase hexadecimal digits before validated capture.
    pub fn from_hex_digest(
        url: &str,
        sha256: &str,
        mime: &str,
        alt: &str,
    ) -> Result<Self, ListingError> {
        if sha256.len() != 64 {
            return Err(ListingError::InvalidMediaDigest);
        }
        let digit = |byte| match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err(ListingError::InvalidMediaDigest),
        };
        let mut digest = [0; 32];
        let (pairs, _) = sha256.as_bytes().as_chunks::<2>();
        for (out, &[a, b]) in digest.iter_mut().zip(pairs) {
            *out = digit(a)? * 16 + digit(b)?;
        }
        Self::new(url, digest, mime, alt)
    }
    /// Borrows the original claim; this grants no endpoint or fetch trust.
    pub fn url(&self) -> &str {
        &self.url
    }
    /// Borrows the exact declared digest; file verification is separate.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// Borrows the declared MIME claim; this is not a supported-codec promise.
    pub fn mime(&self) -> &str {
        &self.mime
    }
    /// Borrows the exact alternative text without rendering it.
    pub fn alt(&self) -> &str {
        &self.alt
    }
}
impl fmt::Debug for Media {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Media(<redacted>)")
    }
}
#[cfg(feature = "serde")]
fn serialize_digest<S: serde::Serializer>(
    value: &[u8; 32],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    struct Digest<'a>(&'a [u8; 32]);
    impl fmt::Display for Digest<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for byte in self.0 {
                write!(f, "{byte:02x}")?;
            }
            Ok(())
        }
    }
    serializer.collect_str(&Digest(value))
}

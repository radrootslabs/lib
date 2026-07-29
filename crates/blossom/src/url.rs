//! Structural Blossom blob URLs and the Radroots-approved reference state.
//!
//! [`BlobUrl`] preserves received HTTP or HTTPS references after strict
//! structural parsing. [`BlobUrl::approve`] produces [`ApprovedBlobUrl`] only
//! for HTTPS and loopback HTTP references. Approval permits a caller to consider
//! the reference for transport; it does not perform a request or establish host
//! reputation, byte integrity, authenticity, or application media safety.

#[cfg(feature = "serde")]
use alloc::string::String;
use alloc::string::ToString;
use core::{fmt, str::FromStr};
use unicode_general_category::{GeneralCategory, get_general_category};
use url_nostd::{Host, Url};

use crate::{error::Error, hash::HashPath};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BlobUrl {
    url: Url,
    hash_path: HashPath,
}

impl BlobUrl {
    pub fn parse(value: &str) -> Result<Self, Error> {
        if !value.contains("://") || !raw_url_text_is_valid(value) {
            return Err(Error::InvalidBlobUrl);
        }
        let url = Url::parse(value).map_err(|_| Error::InvalidBlobUrl)?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(Error::UnsupportedBlobUrlScheme);
        }
        if raw_authority(value).contains('@') {
            return Err(Error::BlobUrlCredentialsForbidden);
        }
        if url.query().is_some() {
            return Err(Error::BlobUrlQueryForbidden);
        }
        if url.fragment().is_some() {
            return Err(Error::BlobUrlFragmentForbidden);
        }
        if value.contains('\\')
            || value.contains('%')
            || value.contains("/./")
            || value.contains("/../")
        {
            return Err(Error::InvalidBlobUrl);
        }
        validate_authority(value, &url)?;
        let hash_path = HashPath::parse(url.path())?;
        Ok(Self { url, hash_path })
    }

    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    pub fn scheme(&self) -> &str {
        self.url.scheme()
    }

    pub fn host(&self) -> &str {
        self.url
            .host_str()
            .expect("validated HTTP and HTTPS URLs always have a host")
    }

    pub fn port(&self) -> Option<u16> {
        self.url.port()
    }

    pub fn hash_path(&self) -> &HashPath {
        &self.hash_path
    }

    pub fn is_https(&self) -> bool {
        self.url.scheme() == "https"
    }

    pub fn is_loopback_http(&self) -> bool {
        self.url.scheme() == "http" && self.url.host().is_some_and(host_is_loopback)
    }

    pub fn approve(self) -> Result<ApprovedBlobUrl, Error> {
        if !self.is_https() && !self.is_loopback_http() {
            return Err(Error::InsecureBlobUrl);
        }
        Ok(ApprovedBlobUrl(self))
    }
}

impl fmt::Display for BlobUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BlobUrl {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for BlobUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BlobUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ApprovedBlobUrl(BlobUrl);

impl ApprovedBlobUrl {
    pub fn as_blob_url(&self) -> &BlobUrl {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_blob_url(self) -> BlobUrl {
        self.0
    }
}

impl fmt::Display for ApprovedBlobUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

fn host_is_loopback(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => {
            domain == "localhost"
                || domain.strip_suffix(".localhost").is_some_and(|prefix| {
                    !prefix.is_empty() && prefix.split('.').all(|label| !label.is_empty())
                })
        }
        Host::Ipv4(address) => address.octets()[0] == 127,
        Host::Ipv6(address) => address.segments() == [0, 0, 0, 0, 0, 0, 0, 1],
    }
}

fn validate_authority(value: &str, url: &Url) -> Result<(), Error> {
    let raw_host = raw_authority_host(value);
    if let Some(port) = raw_authority_port(value) {
        match port.parse::<u16>() {
            Ok(1..) => {}
            Ok(0) | Err(_) => return Err(Error::InvalidBlobUrl),
        }
    }
    match url.host() {
        Some(Host::Domain(_)) if !raw_dns_host_is_valid(raw_host) => {
            return Err(Error::InvalidBlobUrl);
        }
        Some(Host::Ipv4(address)) if raw_host != address.to_string() => {
            return Err(Error::InvalidBlobUrl);
        }
        Some(_) => {}
        None => return Err(Error::InvalidBlobUrl),
    }
    Ok(())
}

fn raw_url_text_is_valid(value: &str) -> bool {
    !value.chars().any(|character| {
        character.is_whitespace()
            || matches!(
                get_general_category(character),
                GeneralCategory::Control | GeneralCategory::Format
            )
    })
}

fn raw_dns_host_is_valid(host: &str) -> bool {
    !host.is_empty()
        && host.is_ascii()
        && host.len() <= 253
        && host.split('.').all(raw_dns_label_is_valid)
}

fn raw_dns_label_is_valid(label: &str) -> bool {
    let bytes = label.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 63
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
}

fn raw_authority(value: &str) -> &str {
    let (_, remainder) = value
        .split_once("://")
        .expect("blob URL parser requires an explicit scheme delimiter");
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    &remainder[..authority_end]
}

fn raw_authority_host(value: &str) -> &str {
    let host_and_port = raw_authority(value);
    if let Some(bracketed) = host_and_port.strip_prefix('[') {
        return bracketed
            .split_once(']')
            .expect("validated bracketed URL host must close")
            .0;
    }
    host_and_port
        .rsplit_once(':')
        .map_or(host_and_port, |(host, _)| host)
}

fn raw_authority_port(value: &str) -> Option<&str> {
    let host_and_port = raw_authority(value);
    if let Some(bracketed) = host_and_port.strip_prefix('[') {
        let (_, suffix) = bracketed
            .split_once(']')
            .expect("validated bracketed URL host must close");
        return suffix.strip_prefix(':');
    }
    host_and_port.rsplit_once(':').map(|(_, port)| port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{
        format,
        string::{String, ToString},
    };

    const HASH: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn url(origin: &str) -> String {
        format!("{origin}/{HASH}.txt")
    }

    #[test]
    fn https_reference_is_structural_and_approved() {
        let parsed = BlobUrl::parse(&url("https://cdn.example.com")).unwrap();
        assert!(parsed.is_https());
        assert!(!parsed.is_loopback_http());
        assert_eq!(parsed.hash_path().hash().to_string(), HASH);
        assert_eq!(parsed.hash_path().extension().unwrap().as_str(), "txt");
        assert_eq!(parsed.scheme(), "https");
        assert_eq!(parsed.host(), "cdn.example.com");
        assert_eq!(parsed.port(), None);
        assert_eq!(parsed.clone().approve().unwrap().as_str(), parsed.as_str());
        assert_eq!(parsed.to_string(), url("https://cdn.example.com"));
    }

    #[test]
    fn loopback_http_reference_set_is_approved() {
        for origin in [
            "http://localhost:3000",
            "http://media.localhost",
            "http://127.0.0.1",
            "http://127.255.10.9:8080",
            "http://[::1]:3000",
            "http://[0:0:0:0:0:0:0:1]",
        ] {
            let parsed = BlobUrl::parse(&url(origin)).unwrap();
            assert!(parsed.is_loopback_http(), "{origin}");
            let canonical = parsed.as_str().to_string();
            let approved = parsed.approve().unwrap();
            assert_eq!(approved.as_blob_url().as_str(), canonical);
            assert_eq!(approved.clone().into_blob_url().as_str(), canonical);
            assert_eq!(approved.to_string(), canonical);
        }
    }

    #[test]
    fn public_and_private_http_are_structural_but_not_approved() {
        for origin in [
            "http://cdn.example.com",
            "http://localhost.example.com",
            "http://192.168.1.2",
            "http://10.0.0.2",
            "http://[::]",
        ] {
            let parsed = BlobUrl::parse(&url(origin)).unwrap();
            assert!(!parsed.is_https());
            assert!(!parsed.is_loopback_http());
            assert_eq!(parsed.approve(), Err(Error::InsecureBlobUrl), "{origin}");
        }
    }

    #[test]
    fn blob_url_rejects_scheme_host_credentials_query_and_fragment() {
        let cases = [
            (
                url("ftp://cdn.example.com"),
                Error::UnsupportedBlobUrlScheme,
            ),
            (
                format!("https://user@cdn.example.com/{HASH}.txt"),
                Error::BlobUrlCredentialsForbidden,
            ),
            (
                format!("https://:password@cdn.example.com/{HASH}.txt"),
                Error::BlobUrlCredentialsForbidden,
            ),
            (
                format!("https://@cdn.example.com/{HASH}.txt"),
                Error::BlobUrlCredentialsForbidden,
            ),
            (
                format!("https://:@cdn.example.com/{HASH}.txt"),
                Error::BlobUrlCredentialsForbidden,
            ),
            (
                format!("https://cdn.example.com/{HASH}.txt?a=1"),
                Error::BlobUrlQueryForbidden,
            ),
            (
                format!("https://cdn.example.com/{HASH}.txt#x"),
                Error::BlobUrlFragmentForbidden,
            ),
        ];
        for (value, expected) in cases {
            assert_eq!(BlobUrl::parse(&value), Err(expected), "{value}");
        }
    }

    #[test]
    fn blob_url_rejects_non_root_encoded_and_traversal_paths() {
        for value in [
            format!("https://cdn.example.com/x/{HASH}.txt"),
            format!("https://cdn.example.com/{HASH}.txt/x"),
            format!("https://cdn.example.com/{HASH}%2etxt"),
            format!("https://cdn.example.com/x/./../{HASH}.txt"),
            format!("https://cdn.example.com/{HASH}/../{HASH}.txt"),
            format!("https://cdn.example.com/{HASH}\\x"),
        ] {
            assert!(BlobUrl::parse(&value).is_err(), "{value}");
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn blob_url_serde_revalidates_structure() {
        let parsed = BlobUrl::parse(&url("https://cdn.example.com")).unwrap();
        let json = serde_json::to_string(&parsed).unwrap();
        assert_eq!(serde_json::from_str::<BlobUrl>(&json).unwrap(), parsed);
        assert!(serde_json::from_str::<BlobUrl>("false").is_err());
        assert!(
            serde_json::from_str::<BlobUrl>("\"https://cdn.example.com/not-a-hash.png\"").is_err()
        );
    }

    #[test]
    fn malformed_url_is_rejected() {
        assert_eq!(BlobUrl::from_str("not a url"), Err(Error::InvalidBlobUrl));
        assert!(BlobUrl::parse(&format!("https:///{HASH}.txt")).is_err());
        assert!(BlobUrl::parse(&url("https://cdn.example.com:0")).is_err());
        assert!(BlobUrl::parse(&url("https://cdn.example.com:00")).is_err());
        assert!(BlobUrl::parse(&url("https://cdn.example.com:")).is_err());
        for value in [
            format!(" https://cdn.example.com/{HASH}.txt"),
            format!("https://cdn.example.com/{HASH}.txt\n"),
            format!("https://cdn.example.com/{HASH}.txt\t"),
            format!("https://cdn.example.com/{HASH}.txt\u{7f}"),
            format!("https://media\u{200b}.example/{HASH}.txt"),
            format!("https://media\u{2060}.example/{HASH}.txt"),
        ] {
            assert_eq!(
                BlobUrl::parse(&value),
                Err(Error::InvalidBlobUrl),
                "{value:?}"
            );
        }
        for origin in [
            "http://0177.0.0.1",
            "http://0x7f.0.0.1",
            "http://127.1",
            "https://2130706433",
        ] {
            assert_eq!(
                BlobUrl::parse(&url(origin)),
                Err(Error::InvalidBlobUrl),
                "{origin}"
            );
        }
    }

    #[test]
    fn raw_dns_authority_is_validated_from_preserved_input() {
        for origin in [
            "https://média.example",
            "https://foo_bar.example",
            "https://-foo.example",
            "https://foo-.example",
            "https://foo..example",
            "https://.example",
            "https://example.",
        ] {
            assert_eq!(
                BlobUrl::parse(&url(origin)),
                Err(Error::InvalidBlobUrl),
                "{origin}"
            );
        }

        let label_too_long = "a".repeat(64);
        assert_eq!(
            BlobUrl::parse(&url(&format!("https://{label_too_long}.example"))),
            Err(Error::InvalidBlobUrl)
        );
        let host_too_long = format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(62)
        );
        assert!(host_too_long.len() > 253);
        assert_eq!(
            BlobUrl::parse(&url(&format!("https://{host_too_long}"))),
            Err(Error::InvalidBlobUrl)
        );

        let maximum_host = format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(61)
        );
        assert_eq!(maximum_host.len(), 253);
        assert!(BlobUrl::parse(&url(&format!("https://{maximum_host}"))).is_ok());
        assert!(BlobUrl::parse(&url("https://xn--mdia-9oa.example")).is_ok());
    }

    #[test]
    fn authority_helpers_reject_absent_and_malformed_loopback_hosts() {
        assert!(!host_is_loopback(Host::Domain(".localhost")));
        assert!(!host_is_loopback(Host::Domain("media..localhost")));

        let hostless = Url::parse("file:///blob").unwrap();
        assert_eq!(
            validate_authority("file:///blob", &hostless),
            Err(Error::InvalidBlobUrl)
        );
    }
}

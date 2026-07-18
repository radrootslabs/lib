#[cfg(feature = "serde")]
use alloc::string::String;
use alloc::string::ToString;
use core::{fmt, str::FromStr};
use url_nostd::{Host, Url};

use crate::{RadrootsBlossomError, RadrootsBlossomHashPath};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RadrootsBlossomBlobUrl {
    url: Url,
    hash_path: RadrootsBlossomHashPath,
}

impl RadrootsBlossomBlobUrl {
    pub fn parse(value: &str) -> Result<Self, RadrootsBlossomError> {
        if !value.contains("://")
            || value
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(RadrootsBlossomError::InvalidBlobUrl);
        }
        let url = Url::parse(value).map_err(|_| RadrootsBlossomError::InvalidBlobUrl)?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(RadrootsBlossomError::UnsupportedBlobUrlScheme);
        }
        if raw_authority(value).contains('@') {
            return Err(RadrootsBlossomError::BlobUrlCredentialsForbidden);
        }
        if url.query().is_some() {
            return Err(RadrootsBlossomError::BlobUrlQueryForbidden);
        }
        if url.fragment().is_some() {
            return Err(RadrootsBlossomError::BlobUrlFragmentForbidden);
        }
        if value.contains('\\')
            || value.contains('%')
            || value.contains("/./")
            || value.contains("/../")
        {
            return Err(RadrootsBlossomError::InvalidBlobUrl);
        }
        validate_authority(value, &url)?;
        let hash_path = RadrootsBlossomHashPath::parse(url.path())?;
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

    pub fn hash_path(&self) -> &RadrootsBlossomHashPath {
        &self.hash_path
    }

    pub fn is_https(&self) -> bool {
        self.url.scheme() == "https"
    }

    pub fn is_loopback_http(&self) -> bool {
        self.url.scheme() == "http" && self.url.host().is_some_and(host_is_loopback)
    }

    pub fn approve(self) -> Result<RadrootsBlossomApprovedBlobUrl, RadrootsBlossomError> {
        if !self.is_https() && !self.is_loopback_http() {
            return Err(RadrootsBlossomError::InsecureBlobUrl);
        }
        Ok(RadrootsBlossomApprovedBlobUrl(self))
    }
}

impl fmt::Display for RadrootsBlossomBlobUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RadrootsBlossomBlobUrl {
    type Err = RadrootsBlossomError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for RadrootsBlossomBlobUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsBlossomBlobUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RadrootsBlossomApprovedBlobUrl(RadrootsBlossomBlobUrl);

impl RadrootsBlossomApprovedBlobUrl {
    pub fn as_blob_url(&self) -> &RadrootsBlossomBlobUrl {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_blob_url(self) -> RadrootsBlossomBlobUrl {
        self.0
    }
}

impl fmt::Display for RadrootsBlossomApprovedBlobUrl {
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

fn validate_authority(value: &str, url: &Url) -> Result<(), RadrootsBlossomError> {
    let raw_host = raw_authority_host(value);
    if let Some(port) = raw_authority_port(value) {
        match port.parse::<u16>() {
            Ok(1..) => {}
            Ok(0) | Err(_) => return Err(RadrootsBlossomError::InvalidBlobUrl),
        }
    }
    if let Some(Host::Ipv4(address)) = url.host()
        && raw_host != address.to_string()
    {
        return Err(RadrootsBlossomError::InvalidBlobUrl);
    }
    Ok(())
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
    use alloc::{format, string::ToString};

    const HASH: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn url(origin: &str) -> String {
        format!("{origin}/{HASH}.txt")
    }

    #[test]
    fn https_reference_is_structural_and_approved() {
        let parsed = RadrootsBlossomBlobUrl::parse(&url("https://cdn.example.com")).unwrap();
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
            let parsed = RadrootsBlossomBlobUrl::parse(&url(origin)).unwrap();
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
            "http://.localhost",
            "http://a..localhost",
        ] {
            let parsed = RadrootsBlossomBlobUrl::parse(&url(origin)).unwrap();
            assert!(!parsed.is_https());
            assert!(!parsed.is_loopback_http());
            assert_eq!(
                parsed.approve(),
                Err(RadrootsBlossomError::InsecureBlobUrl),
                "{origin}"
            );
        }
    }

    #[test]
    fn blob_url_rejects_scheme_host_credentials_query_and_fragment() {
        let cases = [
            (
                url("ftp://cdn.example.com"),
                RadrootsBlossomError::UnsupportedBlobUrlScheme,
            ),
            (
                format!("https://user@cdn.example.com/{HASH}.txt"),
                RadrootsBlossomError::BlobUrlCredentialsForbidden,
            ),
            (
                format!("https://:password@cdn.example.com/{HASH}.txt"),
                RadrootsBlossomError::BlobUrlCredentialsForbidden,
            ),
            (
                format!("https://@cdn.example.com/{HASH}.txt"),
                RadrootsBlossomError::BlobUrlCredentialsForbidden,
            ),
            (
                format!("https://:@cdn.example.com/{HASH}.txt"),
                RadrootsBlossomError::BlobUrlCredentialsForbidden,
            ),
            (
                format!("https://cdn.example.com/{HASH}.txt?a=1"),
                RadrootsBlossomError::BlobUrlQueryForbidden,
            ),
            (
                format!("https://cdn.example.com/{HASH}.txt#x"),
                RadrootsBlossomError::BlobUrlFragmentForbidden,
            ),
        ];
        for (value, expected) in cases {
            assert_eq!(
                RadrootsBlossomBlobUrl::parse(&value),
                Err(expected),
                "{value}"
            );
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
            assert!(RadrootsBlossomBlobUrl::parse(&value).is_err(), "{value}");
        }
    }

    #[test]
    fn blob_url_serde_revalidates_structure() {
        let parsed = RadrootsBlossomBlobUrl::parse(&url("https://cdn.example.com")).unwrap();
        let json = serde_json::to_string(&parsed).unwrap();
        assert_eq!(
            serde_json::from_str::<RadrootsBlossomBlobUrl>(&json).unwrap(),
            parsed
        );
        assert!(serde_json::from_str::<RadrootsBlossomBlobUrl>("false").is_err());
        assert!(
            serde_json::from_str::<RadrootsBlossomBlobUrl>(
                "\"https://cdn.example.com/not-a-hash.png\""
            )
            .is_err()
        );
    }

    #[test]
    fn malformed_url_is_rejected() {
        assert_eq!(
            RadrootsBlossomBlobUrl::from_str("not a url"),
            Err(RadrootsBlossomError::InvalidBlobUrl)
        );
        assert!(RadrootsBlossomBlobUrl::parse(&format!("https:///{HASH}.txt")).is_err());
        assert!(RadrootsBlossomBlobUrl::parse(&url("https://cdn.example.com:0")).is_err());
        assert!(RadrootsBlossomBlobUrl::parse(&url("https://cdn.example.com:00")).is_err());
        assert!(RadrootsBlossomBlobUrl::parse(&url("https://cdn.example.com:")).is_err());
        for value in [
            format!(" https://cdn.example.com/{HASH}.txt"),
            format!("https://cdn.example.com/{HASH}.txt\n"),
            format!("https://cdn.example.com/{HASH}.txt\t"),
            format!("https://cdn.example.com/{HASH}.txt\u{7f}"),
        ] {
            assert_eq!(
                RadrootsBlossomBlobUrl::parse(&value),
                Err(RadrootsBlossomError::InvalidBlobUrl),
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
                RadrootsBlossomBlobUrl::parse(&url(origin)),
                Err(RadrootsBlossomError::InvalidBlobUrl),
                "{origin}"
            );
        }
    }
}

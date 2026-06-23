use crate::codec::{decode_connection_link, encode_connection_link};
use crate::error::{RadrootsSimplexAgentProtoError, RadrootsSimplexAgentUnsupportedLinkKind};
use crate::model::RadrootsSimplexAgentConnectionLink;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use base64::Engine as _;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use core::fmt;
use core::str::FromStr;

pub const RADROOTS_SIMPLEX_AGENT_SHORT_LINK_ID_LENGTH: usize = 24;
pub const RADROOTS_SIMPLEX_AGENT_SHORT_LINK_KEY_LENGTH: usize = 32;
pub const RADROOTS_SIMPLEX_AGENT_SHORT_LINK_SERVER_KEY_HASH_LENGTH: usize = 32;
const RADROOTS_SIMPLEX_AGENT_SHORT_INVITATION_FIXED_DATA_TAG: &[u8] = b"RRSIF1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsSimplexAgentShortLinkScheme {
    Simplex,
    Https,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentShortInvitationLink {
    pub scheme: RadrootsSimplexAgentShortLinkScheme,
    pub hosts: Vec<String>,
    pub port: Option<u16>,
    pub server_key_hash: Option<Vec<u8>>,
    pub link_id: Vec<u8>,
    pub link_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentShortInvitationFixedData {
    pub root_public_signature_key: Vec<u8>,
    pub invitation: RadrootsSimplexAgentConnectionLink,
}

impl RadrootsSimplexAgentShortInvitationLink {
    pub fn render(&self) -> Result<String, RadrootsSimplexAgentProtoError> {
        validate_field_length(
            "link_id",
            &self.link_id,
            RADROOTS_SIMPLEX_AGENT_SHORT_LINK_ID_LENGTH,
        )?;
        validate_field_length(
            "link_key",
            &self.link_key,
            RADROOTS_SIMPLEX_AGENT_SHORT_LINK_KEY_LENGTH,
        )?;
        let link_id = URL_SAFE_NO_PAD.encode(&self.link_id);
        let link_key = URL_SAFE_NO_PAD.encode(&self.link_key);
        let mut output = match self.scheme {
            RadrootsSimplexAgentShortLinkScheme::Simplex => {
                format!("simplex:/i#{link_id}/{link_key}")
            }
            RadrootsSimplexAgentShortLinkScheme::Https => {
                let host =
                    self.hosts
                        .first()
                        .ok_or(RadrootsSimplexAgentProtoError::InvalidLink(
                            "https short invitation link requires a primary host".to_string(),
                        ))?;
                validate_host(host)?;
                format!("https://{host}/i#{link_id}/{link_key}")
            }
        };

        let mut query = Vec::<String>::new();
        let query_hosts = match self.scheme {
            RadrootsSimplexAgentShortLinkScheme::Simplex => self.hosts.as_slice(),
            RadrootsSimplexAgentShortLinkScheme::Https => self.hosts.get(1..).unwrap_or(&[]),
        };
        if !query_hosts.is_empty() {
            for host in query_hosts {
                validate_host(host)?;
            }
            query.push(format!("h={}", query_hosts.join(",")));
        }
        if let Some(port) = self.port {
            query.push(format!("p={port}"));
        }
        if let Some(server_key_hash) = self.server_key_hash.as_ref() {
            validate_field_length(
                "server_key_hash",
                server_key_hash,
                RADROOTS_SIMPLEX_AGENT_SHORT_LINK_SERVER_KEY_HASH_LENGTH,
            )?;
            query.push(format!("c={}", URL_SAFE_NO_PAD.encode(server_key_hash)));
        }
        if !query.is_empty() {
            output.push('?');
            output.push_str(&query.join("&"));
        }
        Ok(output)
    }
}

pub fn encode_short_invitation_fixed_data(
    root_public_signature_key: &[u8],
    invitation: &RadrootsSimplexAgentConnectionLink,
) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    let encoded_invitation = encode_connection_link(invitation)?;
    let mut buffer = Vec::new();
    buffer.extend_from_slice(RADROOTS_SIMPLEX_AGENT_SHORT_INVITATION_FIXED_DATA_TAG);
    push_short_bytes(&mut buffer, root_public_signature_key)?;
    push_large_bytes(&mut buffer, &encoded_invitation)?;
    Ok(buffer)
}

pub fn decode_short_invitation_fixed_data(
    bytes: &[u8],
) -> Result<RadrootsSimplexAgentShortInvitationFixedData, RadrootsSimplexAgentProtoError> {
    let mut cursor = ShortLinkDataCursor::new(bytes);
    cursor.expect_tag(RADROOTS_SIMPLEX_AGENT_SHORT_INVITATION_FIXED_DATA_TAG)?;
    let root_public_signature_key = cursor.read_short_bytes()?;
    let invitation = decode_connection_link(&cursor.read_large_bytes()?)?;
    cursor.finish()?;
    Ok(RadrootsSimplexAgentShortInvitationFixedData {
        root_public_signature_key,
        invitation,
    })
}

pub fn encode_short_invitation_user_data(
    invitation: &RadrootsSimplexAgentConnectionLink,
) -> Vec<u8> {
    invitation.connection_id.clone()
}

impl fmt::Display for RadrootsSimplexAgentShortInvitationLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.render().map_err(|_| fmt::Error)?.fmt(f)
    }
}

impl FromStr for RadrootsSimplexAgentShortInvitationLink {
    type Err = RadrootsSimplexAgentProtoError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_short_invitation_link(value)
    }
}

pub fn parse_short_invitation_link(
    value: &str,
) -> Result<RadrootsSimplexAgentShortInvitationLink, RadrootsSimplexAgentProtoError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(RadrootsSimplexAgentProtoError::InvalidLink(
            "empty short invitation link".to_string(),
        ));
    }

    if let Some(rest) = value.strip_prefix("simplex:/") {
        return parse_scheme_link(
            RadrootsSimplexAgentShortLinkScheme::Simplex,
            None,
            rest,
            value,
        );
    }
    if let Some(rest) = value.strip_prefix("https://") {
        let (authority, path) = rest
            .split_once('/')
            .ok_or_else(|| RadrootsSimplexAgentProtoError::InvalidLink(value.to_string()))?;
        if authority.is_empty() || authority.contains('@') {
            return Err(RadrootsSimplexAgentProtoError::InvalidLink(
                value.to_string(),
            ));
        }
        validate_host(authority)?;
        return parse_scheme_link(
            RadrootsSimplexAgentShortLinkScheme::Https,
            Some(authority),
            path,
            value,
        );
    }

    Err(RadrootsSimplexAgentProtoError::InvalidLink(
        value.to_string(),
    ))
}

fn parse_scheme_link(
    scheme: RadrootsSimplexAgentShortLinkScheme,
    primary_host: Option<&str>,
    rest: &str,
    original: &str,
) -> Result<RadrootsSimplexAgentShortInvitationLink, RadrootsSimplexAgentProtoError> {
    let (raw_path, fragment_and_query) = rest
        .split_once('#')
        .ok_or_else(|| RadrootsSimplexAgentProtoError::InvalidLink(original.to_string()))?;
    let path = raw_path.strip_suffix('/').unwrap_or(raw_path);
    if path != "i" {
        return Err(RadrootsSimplexAgentProtoError::UnsupportedLink(
            unsupported_path_kind(path),
        ));
    }

    let (fragment, query) = fragment_and_query
        .split_once('?')
        .map_or((fragment_and_query, None), |(fragment, query)| {
            (fragment, Some(query))
        });
    let (link_id_raw, link_key_raw) = fragment
        .split_once('/')
        .ok_or_else(|| RadrootsSimplexAgentProtoError::InvalidLink(original.to_string()))?;
    if link_id_raw.is_empty() || link_key_raw.is_empty() || link_key_raw.contains('/') {
        return Err(RadrootsSimplexAgentProtoError::InvalidLink(
            original.to_string(),
        ));
    }

    let mut hosts = primary_host
        .map(|host| alloc::vec![host.to_string()])
        .unwrap_or_default();
    let mut port = None;
    let mut server_key_hash = None;

    if let Some(query) = query {
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (key, raw_value) = pair.split_once('=').ok_or_else(|| {
                RadrootsSimplexAgentProtoError::InvalidLinkParameter {
                    key: pair.to_string(),
                    reason: "parameter must use key=value form".to_string(),
                }
            })?;
            match key {
                "h" => {
                    if hosts.len() > primary_host.iter().count() {
                        return Err(duplicate_param("h"));
                    }
                    let parsed_hosts = parse_hosts(raw_value)?;
                    hosts.extend(parsed_hosts);
                }
                "p" => {
                    if port.replace(parse_port(raw_value)?).is_some() {
                        return Err(duplicate_param("p"));
                    }
                }
                "c" => {
                    if server_key_hash
                        .replace(decode_sized_base64url(
                            "server_key_hash",
                            raw_value,
                            RADROOTS_SIMPLEX_AGENT_SHORT_LINK_SERVER_KEY_HASH_LENGTH,
                        )?)
                        .is_some()
                    {
                        return Err(duplicate_param("c"));
                    }
                }
                _ => {
                    return Err(RadrootsSimplexAgentProtoError::InvalidLinkParameter {
                        key: key.to_string(),
                        reason: "unsupported short-link parameter".to_string(),
                    });
                }
            }
        }
    }

    Ok(RadrootsSimplexAgentShortInvitationLink {
        scheme,
        hosts,
        port,
        server_key_hash,
        link_id: decode_sized_base64url(
            "link_id",
            link_id_raw,
            RADROOTS_SIMPLEX_AGENT_SHORT_LINK_ID_LENGTH,
        )?,
        link_key: decode_sized_base64url(
            "link_key",
            link_key_raw,
            RADROOTS_SIMPLEX_AGENT_SHORT_LINK_KEY_LENGTH,
        )?,
    })
}

fn unsupported_path_kind(path: &str) -> RadrootsSimplexAgentUnsupportedLinkKind {
    match path {
        "contact" => RadrootsSimplexAgentUnsupportedLinkKind::FullContactLink,
        "a" | "address" => RadrootsSimplexAgentUnsupportedLinkKind::ContactAddress,
        "g" | "group" => RadrootsSimplexAgentUnsupportedLinkKind::Group,
        "c" | "channel" => RadrootsSimplexAgentUnsupportedLinkKind::Channel,
        "r" | "relay" => RadrootsSimplexAgentUnsupportedLinkKind::Relay,
        "f" | "file" => RadrootsSimplexAgentUnsupportedLinkKind::File,
        "x" | "xrcp" => RadrootsSimplexAgentUnsupportedLinkKind::Xrcp,
        "b" | "bot" => RadrootsSimplexAgentUnsupportedLinkKind::Bot,
        _ => RadrootsSimplexAgentUnsupportedLinkKind::Unknown(path.to_string()),
    }
}

fn decode_base64url(
    field: &'static str,
    value: &str,
) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    URL_SAFE_NO_PAD
        .decode(value.as_bytes())
        .or_else(|_| URL_SAFE.decode(value.as_bytes()))
        .map_err(|_| RadrootsSimplexAgentProtoError::InvalidBase64Url {
            field,
            value: value.to_string(),
        })
}

fn decode_sized_base64url(
    field: &'static str,
    value: &str,
    expected: usize,
) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    let bytes = decode_base64url(field, value)?;
    validate_field_length(field, &bytes, expected)?;
    Ok(bytes)
}

fn validate_field_length(
    field: &'static str,
    bytes: &[u8],
    expected: usize,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    if bytes.len() != expected {
        return Err(RadrootsSimplexAgentProtoError::InvalidLinkFieldLength {
            field,
            expected,
            actual: bytes.len(),
        });
    }
    Ok(())
}

fn parse_hosts(value: &str) -> Result<Vec<String>, RadrootsSimplexAgentProtoError> {
    if value.is_empty() {
        return Err(RadrootsSimplexAgentProtoError::InvalidLinkParameter {
            key: "h".to_string(),
            reason: "host list cannot be empty".to_string(),
        });
    }
    let hosts = value
        .split(',')
        .map(|host| host.trim().to_string())
        .collect::<Vec<_>>();
    for host in &hosts {
        validate_host(host)?;
    }
    Ok(hosts)
}

fn validate_host(host: &str) -> Result<(), RadrootsSimplexAgentProtoError> {
    if host.is_empty()
        || host
            .chars()
            .any(|ch| ch.is_ascii_whitespace() || matches!(ch, '/' | '?' | '#' | '&' | '=' | ','))
    {
        return Err(RadrootsSimplexAgentProtoError::InvalidLinkParameter {
            key: "h".to_string(),
            reason: "host contains an invalid short-link character".to_string(),
        });
    }
    Ok(())
}

fn parse_port(value: &str) -> Result<u16, RadrootsSimplexAgentProtoError> {
    value
        .parse::<u16>()
        .map_err(|_| RadrootsSimplexAgentProtoError::InvalidPort(value.to_string()))
}

fn duplicate_param(key: &str) -> RadrootsSimplexAgentProtoError {
    RadrootsSimplexAgentProtoError::InvalidLinkParameter {
        key: key.to_string(),
        reason: "duplicate short-link parameter".to_string(),
    }
}

fn push_short_bytes(
    buffer: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RadrootsSimplexAgentProtoError> {
    if value.len() > u8::MAX as usize {
        return Err(RadrootsSimplexAgentProtoError::InvalidShortFieldLength(
            value.len(),
        ));
    }
    buffer.push(value.len() as u8);
    buffer.extend_from_slice(value);
    Ok(())
}

fn push_large_bytes(
    buffer: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RadrootsSimplexAgentProtoError> {
    if value.len() > u16::MAX as usize {
        return Err(RadrootsSimplexAgentProtoError::InvalidLargeFieldLength(
            value.len(),
        ));
    }
    buffer.extend_from_slice(&(value.len() as u16).to_be_bytes());
    buffer.extend_from_slice(value);
    Ok(())
}

struct ShortLinkDataCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ShortLinkDataCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn expect_tag(&mut self, tag: &[u8]) -> Result<(), RadrootsSimplexAgentProtoError> {
        if self.remaining().len() < tag.len() {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        }
        let next = &self.remaining()[..tag.len()];
        if next != tag {
            return Err(RadrootsSimplexAgentProtoError::InvalidTag(
                String::from_utf8_lossy(next).into_owned(),
            ));
        }
        self.offset += tag.len();
        Ok(())
    }

    fn read_short_bytes(&mut self) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
        let len = self.read_byte()? as usize;
        self.read_exact(len)
    }

    fn read_large_bytes(&mut self) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
        if self.remaining().len() < 2 {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        }
        let len =
            u16::from_be_bytes([self.bytes[self.offset], self.bytes[self.offset + 1]]) as usize;
        self.offset += 2;
        self.read_exact(len)
    }

    fn read_byte(&mut self) -> Result<u8, RadrootsSimplexAgentProtoError> {
        if self.offset >= self.bytes.len() {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        }
        let value = self.bytes[self.offset];
        self.offset += 1;
        Ok(value)
    }

    fn read_exact(&mut self, len: usize) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
        if self.remaining().len() < len {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        }
        let value = self.remaining()[..len].to_vec();
        self.offset += len;
        Ok(value)
    }

    fn remaining(&self) -> &'a [u8] {
        &self.bytes[self.offset..]
    }

    fn finish(&self) -> Result<(), RadrootsSimplexAgentProtoError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(RadrootsSimplexAgentProtoError::TrailingBytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_link() -> RadrootsSimplexAgentShortInvitationLink {
        RadrootsSimplexAgentShortInvitationLink {
            scheme: RadrootsSimplexAgentShortLinkScheme::Simplex,
            hosts: alloc::vec!["relay-a.example".to_string(), "relay-b.example".to_string()],
            port: Some(5223),
            server_key_hash: Some((0_u8..32).collect()),
            link_id: (32_u8..56).collect(),
            link_key: (64_u8..96).collect(),
        }
    }

    fn sample_connection_link() -> RadrootsSimplexAgentConnectionLink {
        let key_1 = radroots_simplex_smp_crypto::prelude::official_x448_keypair_from_seed(
            b"rr-synth-short-link-x3dh-1",
        );
        let key_2 = radroots_simplex_smp_crypto::prelude::official_x448_keypair_from_seed(
            b"rr-synth-short-link-x3dh-2",
        );
        RadrootsSimplexAgentConnectionLink {
            invitation_queue:
                radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpQueueUri::parse(
                    "smp://c2VydmVyLWlk@relay.example/c2VuZGVy#/?v=4&dh=cmVjZWl2ZXI&q=m",
                )
                .expect("queue"),
            connection_id: b"conn-synth-short-link".to_vec(),
            e2e_ratchet_params:
                radroots_simplex_smp_crypto::prelude::RadrootsSimplexOfficialX3dhParams {
                    version_range:
                        radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpVersionRange::new(
                            1, 2,
                        )
                        .expect("version range"),
                    key_1: key_1.public_key,
                    key_2: key_2.public_key,
                    pq_public_key: None,
                    pq_ciphertext: None,
                },
            contact_address: false,
        }
    }

    #[test]
    fn renders_and_parses_simplex_invitation_short_link() {
        let link = sample_link();
        let rendered = link.render().expect("rendered link");

        assert!(rendered.starts_with("simplex:/i#"));
        assert!(rendered.contains("?h=relay-a.example,relay-b.example&p=5223&c="));
        let fragment = rendered
            .split_once('#')
            .expect("fragment")
            .1
            .split_once('?')
            .expect("query")
            .0;
        assert!(!fragment.contains('='));
        assert_eq!(
            parse_short_invitation_link(&rendered).expect("parsed"),
            link
        );
    }

    #[test]
    fn renders_and_parses_https_invitation_short_link() {
        let mut link = sample_link();
        link.scheme = RadrootsSimplexAgentShortLinkScheme::Https;
        link.hosts = alloc::vec!["relay-a.example".to_string(), "relay-b.example".to_string()];

        let rendered = link.render().expect("rendered link");

        assert!(rendered.starts_with("https://relay-a.example/i#"));
        assert!(rendered.contains("?h=relay-b.example&p=5223&c="));
        assert_eq!(
            parse_short_invitation_link(&rendered).expect("parsed"),
            link
        );
    }

    #[test]
    fn rejects_full_contact_links() {
        let error = parse_short_invitation_link("simplex:/contact#/?v=1&smp=ignored&e2e=ignored")
            .expect_err("full links fail");

        assert!(matches!(
            error,
            RadrootsSimplexAgentProtoError::UnsupportedLink(
                RadrootsSimplexAgentUnsupportedLinkKind::FullContactLink
            )
        ));
    }

    #[test]
    fn rejects_unsupported_short_link_kinds() {
        let link = sample_link().render().expect("rendered link");
        let (_, fragment) = link.split_once('#').expect("fragment");
        let contact = format!("simplex:/a#{fragment}");
        let group = format!("simplex:/g#{fragment}");
        let channel = format!("simplex:/c#{fragment}");

        assert!(matches!(
            parse_short_invitation_link(&contact),
            Err(RadrootsSimplexAgentProtoError::UnsupportedLink(
                RadrootsSimplexAgentUnsupportedLinkKind::ContactAddress
            ))
        ));
        assert!(matches!(
            parse_short_invitation_link(&group),
            Err(RadrootsSimplexAgentProtoError::UnsupportedLink(
                RadrootsSimplexAgentUnsupportedLinkKind::Group
            ))
        ));
        assert!(matches!(
            parse_short_invitation_link(&channel),
            Err(RadrootsSimplexAgentProtoError::UnsupportedLink(
                RadrootsSimplexAgentUnsupportedLinkKind::Channel
            ))
        ));
    }

    #[test]
    fn rejects_invalid_base64url_parts() {
        let error =
            parse_short_invitation_link("simplex:/i#***/AAAA").expect_err("invalid link id fails");

        assert!(matches!(
            error,
            RadrootsSimplexAgentProtoError::InvalidBase64Url {
                field: "link_id",
                ..
            }
        ));
    }

    #[test]
    fn rejects_wrong_sized_decodable_parts() {
        let link_id = URL_SAFE_NO_PAD.encode([1_u8; RADROOTS_SIMPLEX_AGENT_SHORT_LINK_ID_LENGTH]);
        let link_key = URL_SAFE_NO_PAD.encode([2_u8; 4]);
        let error = parse_short_invitation_link(&format!("simplex:/i#{link_id}/{link_key}"))
            .expect_err("short link key fails");

        assert!(matches!(
            error,
            RadrootsSimplexAgentProtoError::InvalidLinkFieldLength {
                field: "link_key",
                expected: RADROOTS_SIMPLEX_AGENT_SHORT_LINK_KEY_LENGTH,
                actual: 4,
            }
        ));
    }

    #[test]
    fn rejects_unknown_query_parameters() {
        let link = sample_link().render().expect("rendered link");
        let error = parse_short_invitation_link(&format!("{link}&z=1"))
            .expect_err("unknown parameter fails");

        assert!(matches!(
            error,
            RadrootsSimplexAgentProtoError::InvalidLinkParameter { key, .. } if key == "z"
        ));
    }

    #[test]
    fn encodes_and_decodes_short_invitation_fixed_data() {
        let invitation = sample_connection_link();
        let root_public_key = vec![42_u8; 32];
        let encoded =
            encode_short_invitation_fixed_data(&root_public_key, &invitation).expect("encoded");
        let decoded = decode_short_invitation_fixed_data(&encoded).expect("decoded");

        assert_eq!(decoded.root_public_signature_key, root_public_key);
        assert_eq!(decoded.invitation, invitation);
        assert_eq!(
            encode_short_invitation_user_data(&decoded.invitation),
            b"conn-synth-short-link".to_vec()
        );
    }

    #[test]
    fn rejects_short_invitation_fixed_data_with_trailing_bytes() {
        let mut encoded =
            encode_short_invitation_fixed_data(&[42_u8; 32], &sample_connection_link())
                .expect("encoded");
        encoded.push(0);

        assert!(matches!(
            decode_short_invitation_fixed_data(&encoded),
            Err(RadrootsSimplexAgentProtoError::TrailingBytes)
        ));
    }
}

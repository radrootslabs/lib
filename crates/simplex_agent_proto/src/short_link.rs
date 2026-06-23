use crate::error::{RadrootsSimplexAgentProtoError, RadrootsSimplexAgentUnsupportedLinkKind};
use crate::model::RadrootsSimplexAgentConnectionLink;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use base64::Engine as _;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use core::fmt;
use core::str::FromStr;
use radroots_simplex_smp_crypto::prelude::{
    RadrootsSimplexOfficialX3dhParams, decode_ed25519_public_key_x509,
    decode_official_x448_public_key_der, decode_x25519_public_key_x509,
    encode_ed25519_public_key_x509, encode_official_x448_public_key_der,
    encode_x25519_public_key_x509,
};
use radroots_simplex_smp_proto::prelude::{
    RadrootsSimplexSmpQueueMode, RadrootsSimplexSmpQueueUri, RadrootsSimplexSmpServerAddress,
    RadrootsSimplexSmpVersionRange,
};

pub const RADROOTS_SIMPLEX_AGENT_SHORT_LINK_ID_LENGTH: usize = 24;
pub const RADROOTS_SIMPLEX_AGENT_SHORT_LINK_KEY_LENGTH: usize = 32;
pub const RADROOTS_SIMPLEX_AGENT_SHORT_LINK_SERVER_KEY_HASH_LENGTH: usize = 32;
const SIMPLEX_AGENT_SHORT_LINK_MIN_VERSION: u16 = 2;
const SIMPLEX_AGENT_SHORT_LINK_CURRENT_VERSION: u16 = 7;
const SIMPLEX_CONNECTION_MODE_INVITATION: u8 = b'I';
const SIMPLEX_QUEUE_MODE_MESSAGING: u8 = b'M';
const SIMPLEX_QUEUE_MODE_CONTACT: u8 = b'C';
const SIMPLEX_MAYBE_NOTHING: u8 = b'0';
const SIMPLEX_MAYBE_JUST: u8 = b'1';
const SIMPLEX_RATCHET_KEM_PROPOSED: u8 = b'P';
const SIMPLEX_RATCHET_KEM_ACCEPTED: u8 = b'A';
const SIMPLEX_USER_LINK_DATA_LARGE_TAG: u8 = u8::MAX;

type ShortLinkResult<T> = Result<T, RadrootsSimplexAgentProtoError>;

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
    pub agent_version_range: RadrootsSimplexSmpVersionRange,
    pub root_public_signature_key: Vec<u8>,
    pub invitation: RadrootsSimplexAgentConnectionLink,
    pub link_entity_id: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexAgentShortInvitationUserData {
    pub agent_version_range: RadrootsSimplexSmpVersionRange,
    pub user_data: Vec<u8>,
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
    let agent_version_range = official_agent_version_range()?;
    let encoded_root_public_key = encode_ed25519_public_key_x509(root_public_signature_key)
        .map_err(|error| RadrootsSimplexAgentProtoError::InvalidLink(error.to_string()))?;
    let mut buffer = Vec::new();
    push_version_range(&mut buffer, agent_version_range);
    push_short_bytes(&mut buffer, &encoded_root_public_key)?;
    encode_official_invitation_connection_request(&mut buffer, agent_version_range, invitation)?;
    Ok(buffer)
}

pub fn decode_short_invitation_fixed_data(
    bytes: &[u8],
) -> Result<RadrootsSimplexAgentShortInvitationFixedData, RadrootsSimplexAgentProtoError> {
    let mut cursor = ShortLinkDataCursor::new(bytes);
    let agent_version_range = cursor.read_version_range()?;
    let root_public_signature_key = decode_ed25519_public_key_x509(&cursor.read_short_bytes()?)
        .map_err(|error| RadrootsSimplexAgentProtoError::InvalidLink(error.to_string()))?;
    let mut invitation = decode_official_invitation_connection_request(&mut cursor)?;
    let link_entity_id = if cursor.remaining().is_empty() {
        None
    } else {
        Some(cursor.read_short_bytes()?)
    };
    if let Some(link_entity_id) = link_entity_id.as_ref() {
        invitation.connection_id = link_entity_id.clone();
    }
    Ok(RadrootsSimplexAgentShortInvitationFixedData {
        agent_version_range,
        root_public_signature_key,
        invitation,
        link_entity_id,
    })
}

pub fn encode_short_invitation_user_data(
    invitation: &RadrootsSimplexAgentConnectionLink,
) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
    let agent_version_range = official_agent_version_range()?;
    let mut buffer = Vec::new();
    buffer.push(SIMPLEX_CONNECTION_MODE_INVITATION);
    push_version_range(&mut buffer, agent_version_range);
    push_user_link_data(&mut buffer, &invitation.connection_id)?;
    Ok(buffer)
}

pub fn decode_short_invitation_user_data(
    bytes: &[u8],
) -> Result<RadrootsSimplexAgentShortInvitationUserData, RadrootsSimplexAgentProtoError> {
    let mut cursor = ShortLinkDataCursor::new(bytes);
    cursor.expect_byte(SIMPLEX_CONNECTION_MODE_INVITATION)?;
    let agent_version_range = cursor.read_version_range()?;
    let user_data = cursor.read_user_link_data()?;
    Ok(RadrootsSimplexAgentShortInvitationUserData {
        agent_version_range,
        user_data,
    })
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

fn official_agent_version_range() -> ShortLinkResult<RadrootsSimplexSmpVersionRange> {
    Ok(RadrootsSimplexSmpVersionRange::new(
        SIMPLEX_AGENT_SHORT_LINK_MIN_VERSION,
        SIMPLEX_AGENT_SHORT_LINK_CURRENT_VERSION,
    )?)
}

fn encode_official_invitation_connection_request(
    buffer: &mut Vec<u8>,
    agent_version_range: RadrootsSimplexSmpVersionRange,
    invitation: &RadrootsSimplexAgentConnectionLink,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    buffer.push(SIMPLEX_CONNECTION_MODE_INVITATION);
    push_version_range(buffer, agent_version_range);
    push_queue_list(buffer, core::slice::from_ref(&invitation.invitation_queue))?;
    push_maybe_large_bytes(buffer, None)?;
    encode_official_x3dh_params(buffer, &invitation.e2e_ratchet_params)
}

fn decode_official_invitation_connection_request(
    cursor: &mut ShortLinkDataCursor<'_>,
) -> Result<RadrootsSimplexAgentConnectionLink, RadrootsSimplexAgentProtoError> {
    cursor.expect_byte(SIMPLEX_CONNECTION_MODE_INVITATION)?;
    let _agent_version_range = cursor.read_version_range()?;
    let invitation_queues = cursor.read_queue_list()?;
    let _client_data = cursor.read_maybe_large_bytes()?;
    let e2e_ratchet_params = cursor.read_x3dh_params()?;
    let invitation_queue = invitation_queues.into_iter().next().ok_or_else(|| {
        RadrootsSimplexAgentProtoError::InvalidLink(
            "short invitation connection request has no SMP queues".to_string(),
        )
    })?;
    Ok(RadrootsSimplexAgentConnectionLink {
        invitation_queue,
        connection_id: Vec::new(),
        e2e_ratchet_params,
        contact_address: false,
    })
}

fn push_version_range(buffer: &mut Vec<u8>, version_range: RadrootsSimplexSmpVersionRange) {
    buffer.extend_from_slice(&version_range.min.to_be_bytes());
    buffer.extend_from_slice(&version_range.max.to_be_bytes());
}

fn push_queue_list(
    buffer: &mut Vec<u8>,
    queues: &[RadrootsSimplexSmpQueueUri],
) -> Result<(), RadrootsSimplexAgentProtoError> {
    if queues.is_empty() || queues.len() > u8::MAX as usize {
        return Err(RadrootsSimplexAgentProtoError::InvalidShortFieldLength(
            queues.len(),
        ));
    }
    buffer.push(queues.len() as u8);
    for queue in queues {
        encode_official_queue_uri(buffer, queue)?;
    }
    Ok(())
}

fn encode_official_queue_uri(
    buffer: &mut Vec<u8>,
    queue: &RadrootsSimplexSmpQueueUri,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    push_version_range(buffer, queue.version_range);
    encode_official_server_address(buffer, &queue.server)?;
    push_short_bytes(buffer, &decode_base64url("sender_id", &queue.sender_id)?)?;
    let queue_public_key =
        decode_base64url("recipient_dh_public_key", &queue.recipient_dh_public_key)?;
    let queue_public_key = encode_x25519_public_key_x509(
        &decode_x25519_public_key_x509(&queue_public_key)
            .map_err(|error| RadrootsSimplexAgentProtoError::InvalidLink(error.to_string()))?,
    )
    .map_err(|error| RadrootsSimplexAgentProtoError::InvalidLink(error.to_string()))?;
    push_short_bytes(buffer, &queue_public_key)?;
    if queue.version_range.min >= 4 {
        if let Some(queue_mode) = queue.queue_mode {
            buffer.push(match queue_mode {
                RadrootsSimplexSmpQueueMode::Messaging => SIMPLEX_QUEUE_MODE_MESSAGING,
                RadrootsSimplexSmpQueueMode::Contact => SIMPLEX_QUEUE_MODE_CONTACT,
            });
        }
    } else if queue.sender_can_secure() {
        buffer.push(b'T');
    }
    Ok(())
}

fn encode_official_server_address(
    buffer: &mut Vec<u8>,
    server: &RadrootsSimplexSmpServerAddress,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    push_string_list(buffer, &server.hosts)?;
    let port = server
        .port
        .map_or_else(String::new, |port| port.to_string());
    push_string(buffer, &port)?;
    push_short_bytes(
        buffer,
        &decode_base64url("server_identity", &server.server_identity)?,
    )
}

fn encode_official_x3dh_params(
    buffer: &mut Vec<u8>,
    params: &RadrootsSimplexOfficialX3dhParams,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    push_version_range(buffer, params.version_range);
    push_short_bytes(
        buffer,
        &encode_official_x448_public_key_der(&params.key_1).map_err(|error| {
            RadrootsSimplexAgentProtoError::InvalidE2eParameters(error.to_string())
        })?,
    )?;
    push_short_bytes(
        buffer,
        &encode_official_x448_public_key_der(&params.key_2).map_err(|error| {
            RadrootsSimplexAgentProtoError::InvalidE2eParameters(error.to_string())
        })?,
    )?;
    buffer.push(SIMPLEX_MAYBE_NOTHING);
    Ok(())
}

fn push_string_list(
    buffer: &mut Vec<u8>,
    values: &[String],
) -> Result<(), RadrootsSimplexAgentProtoError> {
    if values.is_empty() || values.len() > u8::MAX as usize {
        return Err(RadrootsSimplexAgentProtoError::InvalidShortFieldLength(
            values.len(),
        ));
    }
    buffer.push(values.len() as u8);
    for value in values {
        push_string(buffer, value)?;
    }
    Ok(())
}

fn push_string(buffer: &mut Vec<u8>, value: &str) -> Result<(), RadrootsSimplexAgentProtoError> {
    push_short_bytes(buffer, value.as_bytes())
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

fn push_user_link_data(
    buffer: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), RadrootsSimplexAgentProtoError> {
    if value.len() < SIMPLEX_USER_LINK_DATA_LARGE_TAG as usize {
        push_short_bytes(buffer, value)
    } else {
        buffer.push(SIMPLEX_USER_LINK_DATA_LARGE_TAG);
        push_large_bytes(buffer, value)
    }
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

fn push_maybe_large_bytes(
    buffer: &mut Vec<u8>,
    value: Option<&[u8]>,
) -> Result<(), RadrootsSimplexAgentProtoError> {
    match value {
        Some(value) => {
            buffer.push(SIMPLEX_MAYBE_JUST);
            push_large_bytes(buffer, value)
        }
        None => {
            buffer.push(SIMPLEX_MAYBE_NOTHING);
            Ok(())
        }
    }
}

struct ShortLinkDataCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ShortLinkDataCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), RadrootsSimplexAgentProtoError> {
        let actual = self.read_byte()?;
        if actual != expected {
            return Err(RadrootsSimplexAgentProtoError::InvalidTag(
                String::from_utf8_lossy(&[actual]).into_owned(),
            ));
        }
        Ok(())
    }

    fn read_version_range(
        &mut self,
    ) -> Result<RadrootsSimplexSmpVersionRange, RadrootsSimplexAgentProtoError> {
        if self.remaining().len() < 4 {
            return Err(RadrootsSimplexAgentProtoError::UnexpectedEof);
        }
        let min = u16::from_be_bytes([self.bytes[self.offset], self.bytes[self.offset + 1]]);
        let max = u16::from_be_bytes([self.bytes[self.offset + 2], self.bytes[self.offset + 3]]);
        self.offset += 4;
        Ok(RadrootsSimplexSmpVersionRange::new(min, max)?)
    }

    fn read_queue_list(
        &mut self,
    ) -> Result<Vec<RadrootsSimplexSmpQueueUri>, RadrootsSimplexAgentProtoError> {
        let len = self.read_byte()? as usize;
        if len == 0 {
            return Err(RadrootsSimplexAgentProtoError::InvalidShortFieldLength(0));
        }
        let mut queues = Vec::with_capacity(len);
        for _ in 0..len {
            queues.push(self.read_queue_uri()?);
        }
        Ok(queues)
    }

    fn read_queue_uri(
        &mut self,
    ) -> Result<RadrootsSimplexSmpQueueUri, RadrootsSimplexAgentProtoError> {
        let version_range = self.read_version_range()?;
        let server = self.read_server_address()?;
        let sender_id = URL_SAFE.encode(self.read_short_bytes()?);
        let recipient_dh_public_key = self.read_short_bytes()?;
        let recipient_dh_public_key = encode_x25519_public_key_x509(
            &decode_x25519_public_key_x509(&recipient_dh_public_key)
                .map_err(|error| RadrootsSimplexAgentProtoError::InvalidLink(error.to_string()))?,
        )
        .map_err(|error| RadrootsSimplexAgentProtoError::InvalidLink(error.to_string()))?;
        let recipient_dh_public_key = URL_SAFE.encode(recipient_dh_public_key);
        let queue_mode = match self.peek_byte() {
            Some(SIMPLEX_QUEUE_MODE_MESSAGING) => {
                self.read_byte()?;
                Some(RadrootsSimplexSmpQueueMode::Messaging)
            }
            Some(SIMPLEX_QUEUE_MODE_CONTACT) => {
                self.read_byte()?;
                Some(RadrootsSimplexSmpQueueMode::Contact)
            }
            Some(b'T') if version_range.min < 4 => {
                self.read_byte()?;
                Some(RadrootsSimplexSmpQueueMode::Messaging)
            }
            Some(b'F') if version_range.min < 4 => {
                self.read_byte()?;
                Some(RadrootsSimplexSmpQueueMode::Contact)
            }
            _ => None,
        };
        Ok(RadrootsSimplexSmpQueueUri {
            server,
            sender_id,
            version_range,
            recipient_dh_public_key,
            queue_mode,
        })
    }

    fn read_server_address(
        &mut self,
    ) -> Result<RadrootsSimplexSmpServerAddress, RadrootsSimplexAgentProtoError> {
        let hosts = self.read_string_list()?;
        let port = match self.read_string()?.as_str() {
            "" => None,
            value => Some(
                value
                    .parse::<u16>()
                    .map_err(|_| RadrootsSimplexAgentProtoError::InvalidPort(value.to_string()))?,
            ),
        };
        let server_identity = URL_SAFE.encode(self.read_short_bytes()?);
        Ok(RadrootsSimplexSmpServerAddress {
            server_identity,
            hosts,
            port,
        })
    }

    fn read_string_list(&mut self) -> Result<Vec<String>, RadrootsSimplexAgentProtoError> {
        let len = self.read_byte()? as usize;
        if len == 0 {
            return Err(RadrootsSimplexAgentProtoError::InvalidShortFieldLength(0));
        }
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            values.push(self.read_string()?);
        }
        Ok(values)
    }

    fn read_string(&mut self) -> Result<String, RadrootsSimplexAgentProtoError> {
        String::from_utf8(self.read_short_bytes()?)
            .map_err(|error| RadrootsSimplexAgentProtoError::InvalidUtf8(error.to_string()))
    }

    fn read_x3dh_params(
        &mut self,
    ) -> Result<RadrootsSimplexOfficialX3dhParams, RadrootsSimplexAgentProtoError> {
        let version_range = self.read_version_range()?;
        let key_1 =
            decode_official_x448_public_key_der(&self.read_short_bytes()?).map_err(|error| {
                RadrootsSimplexAgentProtoError::InvalidE2eParameters(error.to_string())
            })?;
        let key_2 =
            decode_official_x448_public_key_der(&self.read_short_bytes()?).map_err(|error| {
                RadrootsSimplexAgentProtoError::InvalidE2eParameters(error.to_string())
            })?;
        let (pq_public_key, pq_ciphertext) = self.read_optional_kem_params()?;
        Ok(RadrootsSimplexOfficialX3dhParams {
            version_range,
            key_1,
            key_2,
            pq_public_key,
            pq_ciphertext,
        })
    }

    fn read_optional_kem_params(
        &mut self,
    ) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>), RadrootsSimplexAgentProtoError> {
        match self.read_byte()? {
            SIMPLEX_MAYBE_NOTHING => Ok((None, None)),
            SIMPLEX_MAYBE_JUST => match self.read_byte()? {
                SIMPLEX_RATCHET_KEM_PROPOSED => Ok((Some(self.read_large_bytes()?), None)),
                SIMPLEX_RATCHET_KEM_ACCEPTED => {
                    let ciphertext = self.read_large_bytes()?;
                    let public_key = self.read_large_bytes()?;
                    Ok((Some(public_key), Some(ciphertext)))
                }
                tag => Err(RadrootsSimplexAgentProtoError::InvalidTag(
                    String::from_utf8_lossy(&[tag]).into_owned(),
                )),
            },
            tag => Err(RadrootsSimplexAgentProtoError::InvalidTag(
                String::from_utf8_lossy(&[tag]).into_owned(),
            )),
        }
    }

    fn read_maybe_large_bytes(
        &mut self,
    ) -> Result<Option<Vec<u8>>, RadrootsSimplexAgentProtoError> {
        match self.read_byte()? {
            SIMPLEX_MAYBE_NOTHING => Ok(None),
            SIMPLEX_MAYBE_JUST => Ok(Some(self.read_large_bytes()?)),
            tag => Err(RadrootsSimplexAgentProtoError::InvalidTag(
                String::from_utf8_lossy(&[tag]).into_owned(),
            )),
        }
    }

    fn read_user_link_data(&mut self) -> Result<Vec<u8>, RadrootsSimplexAgentProtoError> {
        let len = self.read_byte()?;
        if len == SIMPLEX_USER_LINK_DATA_LARGE_TAG {
            self.read_large_bytes()
        } else {
            self.read_exact(len as usize)
        }
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

    fn peek_byte(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
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
        let queue_key =
            radroots_simplex_smp_crypto::prelude::RadrootsSimplexSmpX25519Keypair::from_seed(
                b"rr-synth-short-link-queue-dh",
            );
        let server_id = URL_SAFE.encode([7_u8; 32]);
        let sender_id = URL_SAFE.encode([9_u8; RADROOTS_SIMPLEX_AGENT_SHORT_LINK_ID_LENGTH]);
        let queue_dh = URL_SAFE.encode(
            radroots_simplex_smp_crypto::prelude::encode_x25519_public_key_x509(
                &queue_key.public_key,
            )
            .expect("queue key"),
        );
        let key_1 = radroots_simplex_smp_crypto::prelude::official_x448_keypair_from_seed(
            b"rr-synth-short-link-x3dh-1",
        );
        let key_2 = radroots_simplex_smp_crypto::prelude::official_x448_keypair_from_seed(
            b"rr-synth-short-link-x3dh-2",
        );
        RadrootsSimplexAgentConnectionLink {
            invitation_queue:
                radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpQueueUri::parse(&format!(
                    "smp://{server_id}@relay.example/{sender_id}#/?v=4&dh={queue_dh}&q=m"
                ))
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
        let encoded_user_data = encode_short_invitation_user_data(&invitation).expect("user data");
        let decoded_user_data =
            decode_short_invitation_user_data(&encoded_user_data).expect("decoded user data");

        assert_ne!(&encoded[..6], b"RRSIF1");
        assert_eq!(decoded.agent_version_range.min, 2);
        assert_eq!(decoded.agent_version_range.max, 7);
        assert_eq!(decoded.root_public_signature_key, root_public_key);
        assert_eq!(decoded.link_entity_id, None);
        assert!(decoded.invitation.connection_id.is_empty());
        assert_eq!(
            decoded.invitation.invitation_queue,
            invitation.invitation_queue
        );
        assert_eq!(
            decoded.invitation.e2e_ratchet_params,
            invitation.e2e_ratchet_params
        );
        assert_eq!(decoded_user_data.agent_version_range.min, 2);
        assert_eq!(decoded_user_data.agent_version_range.max, 7);
        assert_eq!(
            decoded_user_data.user_data,
            b"conn-synth-short-link".to_vec()
        );
    }

    #[test]
    fn rejects_legacy_radroots_short_invitation_fixed_data() {
        let mut legacy = b"RRSIF1".to_vec();
        legacy.push(32);
        legacy.extend_from_slice(&[42_u8; 32]);
        legacy.extend_from_slice(&0_u16.to_be_bytes());

        assert!(decode_short_invitation_fixed_data(&legacy).is_err());
    }
}

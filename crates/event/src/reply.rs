#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
use core::{borrow::Borrow, fmt, net::Ipv6Addr, ops::Deref, str::FromStr};

use crate::{
    ids::{RadrootsEventId, RadrootsIdParseError, RadrootsPublicKey},
    post::{
        RADROOTS_POST_CONTENT_MAX_BYTES, RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
        RADROOTS_POST_TAG_ELEMENT_MAX_BYTES, RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
    },
};

const RADROOTS_NIP10_REPLY_SIGNED_EVENT_FIXED_MAX_BYTES: usize = "{\"id\":\"".len()
    + 64
    + "\",\"pubkey\":\"".len()
    + 64
    + "\",\"created_at\":".len()
    + 20
    + ",\"kind\":1,\"tags\":".len()
    + ",\"content\":".len()
    + ",\"sig\":\"".len()
    + 128
    + "\"}".len();

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip10ReplyError {
    ContentMissing,
    ContentTooLarge { max: usize, actual: usize },
    EventIdInvalid(RadrootsIdParseError),
    AuthorInvalid(RadrootsIdParseError),
    RelayInvalid(RadrootsIdParseError),
    NestedParentMatchesRoot,
    TagElementTooLarge { max: usize, actual: usize },
    TagBytesExceeded { max: usize, actual: usize },
    EventWireTooLarge { max: usize, actual: usize },
}

impl RadrootsNip10ReplyError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContentMissing => "reply_content_missing",
            Self::ContentTooLarge { .. } => "reply_content_too_large",
            Self::EventIdInvalid(_) => "reply_event_id_invalid",
            Self::AuthorInvalid(_) => "reply_author_invalid",
            Self::RelayInvalid(_) => "reply_relay_invalid",
            Self::NestedParentMatchesRoot => "reply_reference_ambiguous",
            Self::TagElementTooLarge { .. } => "reply_tag_element_too_large",
            Self::TagBytesExceeded { .. } => "reply_tag_bytes_exceeded",
            Self::EventWireTooLarge { .. } => "reply_event_wire_too_large",
        }
    }
}

impl fmt::Display for RadrootsNip10ReplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentMissing => {
                formatter.write_str("authored NIP-10 reply content must be non-whitespace")
            }
            Self::ContentTooLarge { max, actual } => {
                write!(
                    formatter,
                    "authored NIP-10 reply content is {actual} bytes; max is {max}"
                )
            }
            Self::EventIdInvalid(error) => {
                write!(formatter, "NIP-10 reply event id is invalid: {error}")
            }
            Self::AuthorInvalid(error) => {
                write!(formatter, "NIP-10 reply author is invalid: {error}")
            }
            Self::RelayInvalid(error) => {
                write!(formatter, "NIP-10 reply relay hint is invalid: {error}")
            }
            Self::NestedParentMatchesRoot => {
                formatter.write_str("nested NIP-10 reply parent must differ from the thread root")
            }
            Self::TagElementTooLarge { max, actual } => {
                write!(
                    formatter,
                    "authored NIP-10 reply tag element is {actual} bytes; max is {max}"
                )
            }
            Self::TagBytesExceeded { max, actual } => {
                write!(
                    formatter,
                    "authored NIP-10 reply tag bytes are {actual}; max is {max}"
                )
            }
            Self::EventWireTooLarge { max, actual } => write!(
                formatter,
                "authored NIP-10 reply canonical signed event is at most {actual} bytes; max is {max}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip10ReplyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EventIdInvalid(error)
            | Self::AuthorInvalid(error)
            | Self::RelayInvalid(error) => Some(error),
            _ => None,
        }
    }
}

/// One canonical relay hint for a NIP-10 event or participant reference.
///
/// This profile intentionally accepts a conservative, byte-stable subset of
/// WebSocket URLs. It does not inherit browser URL normalization, legacy IPv4
/// syntax, Unicode host processing, user information, or fragments.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsNip10RelayHint(String);

impl RadrootsNip10RelayHint {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsIdParseError> {
        validate_nip10_relay_hint(value.as_ref()).map(Self)
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    #[inline]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for RadrootsNip10RelayHint {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for RadrootsNip10RelayHint {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for RadrootsNip10RelayHint {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<RadrootsNip10RelayHint> for String {
    #[inline]
    fn from(value: RadrootsNip10RelayHint) -> Self {
        value.into_string()
    }
}

impl fmt::Display for RadrootsNip10RelayHint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RadrootsNip10RelayHint {
    type Err = RadrootsIdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for RadrootsNip10RelayHint {
    type Error = RadrootsIdParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for RadrootsNip10RelayHint {
    type Error = RadrootsIdParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

/// One syntactically validated reference used by strict NIP-10 authoring.
///
/// The caller asserts that the target is a kind-1 event. This value does not
/// retrieve the target or prove its existence, kind, signature, or author.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNip10ReplyReference {
    event_id: RadrootsEventId,
    author: RadrootsPublicKey,
    relay: Option<RadrootsNip10RelayHint>,
}

impl RadrootsNip10ReplyReference {
    pub fn new(
        event_id: RadrootsEventId,
        author: RadrootsPublicKey,
        relay: Option<RadrootsNip10RelayHint>,
    ) -> Result<Self, RadrootsNip10ReplyError> {
        if let Some(relay) = &relay {
            validate_tag_element(relay.as_str())?;
        }
        Ok(Self {
            event_id,
            author,
            relay,
        })
    }

    pub fn parse(
        event_id: impl AsRef<str>,
        author: impl AsRef<str>,
        relay: Option<&str>,
    ) -> Result<Self, RadrootsNip10ReplyError> {
        let event_id =
            RadrootsEventId::parse(event_id).map_err(RadrootsNip10ReplyError::EventIdInvalid)?;
        let author =
            RadrootsPublicKey::parse(author).map_err(RadrootsNip10ReplyError::AuthorInvalid)?;
        let relay = match relay {
            None | Some("") => None,
            Some(relay) => Some(
                RadrootsNip10RelayHint::parse(relay)
                    .map_err(RadrootsNip10ReplyError::RelayInvalid)?,
            ),
        };
        Self::new(event_id, author, relay)
    }

    pub const fn event_id(&self) -> &RadrootsEventId {
        &self.event_id
    }

    pub const fn author(&self) -> &RadrootsPublicKey {
        &self.author
    }

    pub const fn relay(&self) -> Option<&RadrootsNip10RelayHint> {
        self.relay.as_ref()
    }

    pub fn relay_or_empty(&self) -> &str {
        self.relay
            .as_ref()
            .map_or("", RadrootsNip10RelayHint::as_str)
    }
}

fn validate_nip10_relay_hint(value: &str) -> Result<String, RadrootsIdParseError> {
    if value.is_empty() {
        return Err(RadrootsIdParseError::Empty);
    }
    if !value.bytes().all(|byte| matches!(byte, 0x21..=0x7e)) {
        return Err(RadrootsIdParseError::InvalidCharacter);
    }
    if value.bytes().any(|byte| matches!(byte, b'#' | b'\\')) {
        return Err(RadrootsIdParseError::InvalidFormat);
    }

    let remainder = value
        .strip_prefix("wss://")
        .or_else(|| value.strip_prefix("ws://"))
        .ok_or(RadrootsIdParseError::InvalidFormat)?;
    let authority_end = remainder
        .bytes()
        .position(|byte| matches!(byte, b'/' | b'?'))
        .unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty()
        || authority.bytes().any(|byte| byte == b'@')
        || matches!(remainder.as_bytes().first(), Some(b'/' | b'?'))
    {
        return Err(RadrootsIdParseError::InvalidFormat);
    }
    if !nip10_relay_authority_is_valid(authority)
        || !nip10_relay_path_and_query_are_valid(&remainder[authority_end..])
    {
        return Err(RadrootsIdParseError::InvalidFormat);
    }
    Ok(value.to_string())
}

fn nip10_relay_authority_is_valid(authority: &str) -> bool {
    if let Some(ipv6) = authority.strip_prefix('[') {
        let Some(closing_index) = ipv6.find(']') else {
            return false;
        };
        let address = &ipv6[..closing_index];
        let suffix = &ipv6[closing_index + 1..];
        return canonical_nip10_ipv6(address)
            && (suffix.is_empty() || suffix.strip_prefix(':').is_some_and(canonical_nip10_port));
    }

    let mut parts = authority.split(':');
    let host = parts.next().unwrap_or_default();
    let port = parts.next();
    if parts.next().is_some()
        || port.is_some_and(|port| !canonical_nip10_port(port))
        || host.is_empty()
    {
        return false;
    }
    canonical_nip10_ipv4(host) || canonical_nip10_dns_host(host)
}

fn canonical_nip10_port(value: &str) -> bool {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || value.len() > 5
        || value.len() > 1 && value.starts_with('0')
    {
        return false;
    }
    value
        .parse::<u32>()
        .is_ok_and(|port| (1..=u16::MAX.into()).contains(&port))
}

fn canonical_nip10_ipv4(value: &str) -> bool {
    let mut count = 0usize;
    for part in value.split('.') {
        if part.is_empty()
            || !part.bytes().all(|byte| byte.is_ascii_digit())
            || part.len() > 1 && part.starts_with('0')
            || part.parse::<u8>().is_err()
        {
            return false;
        }
        count += 1;
    }
    count == 4
}

fn canonical_nip10_dns_host(value: &str) -> bool {
    if value.len() > 253 {
        return false;
    }
    let mut final_label = "";
    for label in value.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with("xn--")
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || !label
                .as_bytes()
                .first()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            || !label
                .as_bytes()
                .last()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            return false;
        }
        final_label = label;
    }
    !nip10_dns_label_is_whatwg_number(final_label)
}

fn nip10_dns_label_is_whatwg_number(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_digit())
        || value
            .strip_prefix("0x")
            .is_some_and(|digits| digits.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn canonical_nip10_ipv6(value: &str) -> bool {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f' | b':'))
    {
        return false;
    }
    let Ok(address) = value.parse::<Ipv6Addr>() else {
        return false;
    };
    canonical_nip10_ipv6_string(address) == value
}

fn canonical_nip10_ipv6_string(address: Ipv6Addr) -> String {
    let segments = address.segments();
    let mut best_start = 0usize;
    let mut best_len = 0usize;
    let mut index = 0usize;
    while index < segments.len() {
        if segments[index] != 0 {
            index += 1;
            continue;
        }
        let start = index;
        while index < segments.len() && segments[index] == 0 {
            index += 1;
        }
        let len = index - start;
        if len >= 2 && len > best_len {
            best_start = start;
            best_len = len;
        }
    }

    let mut canonical = String::new();
    if best_len == 0 {
        push_nip10_ipv6_segments(&mut canonical, &segments);
        return canonical;
    }
    push_nip10_ipv6_segments(&mut canonical, &segments[..best_start]);
    canonical.push_str("::");
    push_nip10_ipv6_segments(&mut canonical, &segments[best_start + best_len..]);
    canonical
}

fn push_nip10_ipv6_segments(output: &mut String, segments: &[u16]) {
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            output.push(':');
        }
        push_nip10_ipv6_segment(output, *segment);
    }
}

fn push_nip10_ipv6_segment(output: &mut String, segment: u16) {
    let mut shift = 12u32;
    while shift > 0 && segment >> shift == 0 {
        shift -= 4;
    }
    loop {
        let nibble = ((segment >> shift) & 0x0f) as u8;
        output.push(match nibble {
            0..=9 => (b'0' + nibble) as char,
            _ => (b'a' + nibble - 10) as char,
        });
        if shift == 0 {
            break;
        }
        shift -= 4;
    }
}

fn nip10_relay_path_and_query_are_valid(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if let Some(query) = value.strip_prefix('?') {
        return nip10_relay_component_is_valid(query, true);
    }
    let Some(path) = value.strip_prefix('/') else {
        return false;
    };
    let (path, query) = path
        .split_once('?')
        .map_or((path, None), |(path, query)| (path, Some(query)));
    nip10_relay_component_is_valid(path, false)
        && query.is_none_or(|query| nip10_relay_component_is_valid(query, true))
}

fn nip10_relay_component_is_valid(value: &str, query: bool) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !nip10_upper_hex_digit(bytes[index + 1])
                || !nip10_upper_hex_digit(bytes[index + 2])
            {
                return false;
            }
            index += 3;
            continue;
        }
        if !nip10_relay_pchar(bytes[index]) && !(query && bytes[index] == b'?') {
            return false;
        }
        index += 1;
    }
    true
}

fn nip10_relay_pchar(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
                | b'/'
        )
}

fn nip10_upper_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'A'..=b'F')
}

/// Strict authored marked NIP-10 reply.
///
/// Direct replies contain one `root` reference. Nested replies contain one
/// `root` and one distinct `reply` reference. The type is intentionally opaque
/// and has no Serde construction path.
///
/// ```compile_fail
/// let _: radroots_event::reply::RadrootsAuthoredNip10Reply =
///     serde_json::from_str(r#"{"content":"reply"}"#).unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAuthoredNip10Reply {
    content: String,
    root: RadrootsNip10ReplyReference,
    parent: Option<RadrootsNip10ReplyReference>,
}

impl RadrootsAuthoredNip10Reply {
    pub fn direct(
        content: impl Into<String>,
        root: RadrootsNip10ReplyReference,
    ) -> Result<Self, RadrootsNip10ReplyError> {
        Self::new(content.into(), root, None)
    }

    pub fn nested(
        content: impl Into<String>,
        root: RadrootsNip10ReplyReference,
        parent: RadrootsNip10ReplyReference,
    ) -> Result<Self, RadrootsNip10ReplyError> {
        if root.event_id == parent.event_id {
            return Err(RadrootsNip10ReplyError::NestedParentMatchesRoot);
        }
        Self::new(content.into(), root, Some(parent))
    }

    fn new(
        content: String,
        root: RadrootsNip10ReplyReference,
        parent: Option<RadrootsNip10ReplyReference>,
    ) -> Result<Self, RadrootsNip10ReplyError> {
        validate_content(&content)?;
        validate_authored_reply_wire_size(&content, &root, parent.as_ref())?;
        Ok(Self {
            content,
            root,
            parent,
        })
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub const fn root(&self) -> &RadrootsNip10ReplyReference {
        &self.root
    }

    pub const fn parent(&self) -> Option<&RadrootsNip10ReplyReference> {
        self.parent.as_ref()
    }

    pub const fn is_direct(&self) -> bool {
        self.parent.is_none()
    }
}

fn validate_content(content: &str) -> Result<(), RadrootsNip10ReplyError> {
    if content.trim().is_empty() {
        return Err(RadrootsNip10ReplyError::ContentMissing);
    }
    if content.len() > RADROOTS_POST_CONTENT_MAX_BYTES {
        return Err(RadrootsNip10ReplyError::ContentTooLarge {
            max: RADROOTS_POST_CONTENT_MAX_BYTES,
            actual: content.len(),
        });
    }
    Ok(())
}

fn validate_tag_element(element: &str) -> Result<(), RadrootsNip10ReplyError> {
    if element.len() > RADROOTS_POST_TAG_ELEMENT_MAX_BYTES {
        return Err(RadrootsNip10ReplyError::TagElementTooLarge {
            max: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
            actual: element.len(),
        });
    }
    Ok(())
}

fn validate_authored_reply_wire_size(
    content: &str,
    root: &RadrootsNip10ReplyReference,
    parent: Option<&RadrootsNip10ReplyReference>,
) -> Result<(), RadrootsNip10ReplyError> {
    let mut tag_bytes = 0usize;
    let mut tags_json_bytes = 2usize;
    let mut tag_count = 0usize;

    add_tag(
        &mut tag_bytes,
        &mut tags_json_bytes,
        &mut tag_count,
        &["e", root.event_id.as_str(), root.relay_or_empty(), "root"],
    );
    if let Some(parent) = parent {
        add_tag(
            &mut tag_bytes,
            &mut tags_json_bytes,
            &mut tag_count,
            &[
                "e",
                parent.event_id.as_str(),
                parent.relay_or_empty(),
                "reply",
            ],
        );
    }
    add_tag(
        &mut tag_bytes,
        &mut tags_json_bytes,
        &mut tag_count,
        &["p", root.author.as_str()],
    );
    if let Some(parent) = parent.filter(|parent| parent.author != root.author) {
        add_tag(
            &mut tag_bytes,
            &mut tags_json_bytes,
            &mut tag_count,
            &["p", parent.author.as_str()],
        );
    }

    if tag_bytes > RADROOTS_POST_TAG_TOTAL_MAX_BYTES {
        return Err(RadrootsNip10ReplyError::TagBytesExceeded {
            max: RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
            actual: tag_bytes,
        });
    }
    let actual = RADROOTS_NIP10_REPLY_SIGNED_EVENT_FIXED_MAX_BYTES
        .saturating_add(tags_json_bytes)
        .saturating_add(canonical_json_string_bytes(content));
    if actual > RADROOTS_POST_EVENT_WIRE_MAX_BYTES {
        return Err(RadrootsNip10ReplyError::EventWireTooLarge {
            max: RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
            actual,
        });
    }
    Ok(())
}

fn add_tag(
    tag_bytes: &mut usize,
    tags_json_bytes: &mut usize,
    tag_count: &mut usize,
    elements: &[&str],
) {
    if *tag_count > 0 {
        *tags_json_bytes = tags_json_bytes.saturating_add(1);
    }
    *tags_json_bytes = tags_json_bytes.saturating_add(2);
    for (index, element) in elements.iter().enumerate() {
        if index > 0 {
            *tags_json_bytes = tags_json_bytes.saturating_add(1);
        }
        *tags_json_bytes = tags_json_bytes.saturating_add(canonical_json_string_bytes(element));
        *tag_bytes = tag_bytes.saturating_add(element.len());
    }
    *tag_count = tag_count.saturating_add(1);
}

fn canonical_json_string_bytes(value: &str) -> usize {
    value.chars().fold(2usize, |total, character| {
        total.saturating_add(match character {
            '"' | '\\' | '\u{0008}' | '\t' | '\n' | '\u{000c}' | '\r' => 2,
            '\u{0000}'..='\u{001f}' => 6,
            _ => character.len_utf8(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(event: char, author: char) -> RadrootsNip10ReplyReference {
        RadrootsNip10ReplyReference::parse(
            event.to_string().repeat(64),
            author.to_string().repeat(64),
            Some("wss://relay.example"),
        )
        .expect("reference")
    }

    #[test]
    fn builds_direct_and_nested_replies_with_distinct_coordinates() {
        let direct =
            RadrootsAuthoredNip10Reply::direct("Direct", reference('a', 'b')).expect("direct");
        assert!(direct.is_direct());
        assert!(direct.parent().is_none());

        let nested =
            RadrootsAuthoredNip10Reply::nested("Nested", reference('a', 'b'), reference('c', 'd'))
                .expect("nested");
        assert!(!nested.is_direct());
        assert_eq!(
            nested.parent().expect("parent").event_id().as_str(),
            "c".repeat(64)
        );
    }

    #[test]
    fn rejects_blank_content_and_ambiguous_nested_parent() {
        assert_eq!(
            RadrootsAuthoredNip10Reply::direct("\t", reference('a', 'b')).unwrap_err(),
            RadrootsNip10ReplyError::ContentMissing
        );

        let root = reference('a', 'b');
        let parent = reference('a', 'c');
        assert_eq!(
            RadrootsAuthoredNip10Reply::nested("Nested", root, parent).unwrap_err(),
            RadrootsNip10ReplyError::NestedParentMatchesRoot
        );
    }

    #[test]
    fn parses_and_canonicalizes_reference_identifiers() {
        let reference = RadrootsNip10ReplyReference::parse(
            "A".repeat(64),
            "B".repeat(64),
            Some("wss://relay.example"),
        )
        .expect("reference");
        assert_eq!(reference.event_id().as_str(), "a".repeat(64));
        assert_eq!(reference.author().as_str(), "b".repeat(64));
        assert_eq!(
            reference.relay().expect("relay").as_str(),
            "wss://relay.example"
        );

        let error =
            RadrootsNip10ReplyReference::parse("not-an-id", "b".repeat(64), None).unwrap_err();
        assert_eq!(error.code(), "reply_event_id_invalid");
    }

    #[test]
    fn canonical_relay_hints_accept_portable_hosts_paths_and_queries() {
        for value in [
            "wss://relay.example",
            "ws://127.0.0.1:21003",
            "wss://localhost",
            "wss://[::1]",
            "wss://[2001:db8::1]:65535/nostr/v1?region=ca-bc&next=%2Ffeed",
            "wss://[::ffff:c000:201]",
            "wss://relay.example:443?",
            "wss://relay.example/a/b:@!$&'()*+,;=~_-?next=/feed??page=1",
        ] {
            let relay = RadrootsNip10RelayHint::parse(value)
                .unwrap_or_else(|error| panic!("{value} must be canonical: {error}"));
            assert_eq!(relay.as_str(), value);
            assert_eq!(relay.to_string(), value);
            assert_eq!(
                value.parse::<RadrootsNip10RelayHint>().expect("FromStr"),
                relay
            );
        }

        let maximum_host = format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(61)
        );
        assert_eq!(maximum_host.len(), 253);
        RadrootsNip10RelayHint::parse(format!("wss://{maximum_host}")).expect("253-byte DNS host");
    }

    #[test]
    fn canonical_relay_hints_reject_normalizing_or_ambiguous_forms() {
        assert_eq!(
            RadrootsNip10RelayHint::parse("").unwrap_err(),
            RadrootsIdParseError::Empty
        );
        for value in [
            "WSS://relay.example",
            "https://relay.example",
            "wss://",
            "wss:///relay.example",
            "wss:////relay.example",
            "wss://?region=ca-bc",
            "wss://user@relay.example",
            "wss://@relay.example",
            "wss://relay.example#read",
            "wss://relay.example\\path",
            "wss://Relay.example",
            "wss://relay_example",
            "wss://-relay.example",
            "wss://relay-.example",
            "wss://relay..example",
            "wss://relay.example.",
            "wss://xn--fa-hia.example",
            "wss://example.999",
            "wss://example.0x",
            "wss://example.0x1",
            "wss://%65xample.com",
            "wss://127.1",
            "wss://2130706433",
            "wss://0x7f.1",
            "wss://01.2.3.4",
            "wss://256.1.1.1",
            "wss://[2001:DB8::1]",
            "wss://[2001:0db8::1]",
            "wss://[2001:db8:0:0:0:0:0:1]",
            "wss://[2001:0:0:1::1:1]",
            "wss://[::ffff:192.0.2.1]",
            "wss://[fe80::1%25en0]",
            "wss://[v1.foo]",
            "wss://2001:db8::1",
            "wss://relay.example:",
            "wss://relay.example:0",
            "wss://relay.example:01",
            "wss://relay.example:+443",
            "wss://relay.example:65536",
            "wss://relay.example/[raw]",
            "wss://relay.example/%",
            "wss://relay.example/%2",
            "wss://relay.example/%2f",
            "wss://relay.example/%GG",
        ] {
            assert_eq!(
                RadrootsNip10RelayHint::parse(value).unwrap_err(),
                RadrootsIdParseError::InvalidFormat,
                "{value}"
            );
        }

        for value in [
            "wss://relay.example path",
            "wss://relay.example/\u{007f}",
            "wss://relay.example/é",
        ] {
            assert_eq!(
                RadrootsNip10RelayHint::parse(value).unwrap_err(),
                RadrootsIdParseError::InvalidCharacter,
                "{value}"
            );
        }

        let label_too_long = "a".repeat(64);
        assert_eq!(
            RadrootsNip10RelayHint::parse(format!("wss://{label_too_long}.example")).unwrap_err(),
            RadrootsIdParseError::InvalidFormat
        );
        let host_too_long = format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(62)
        );
        assert_eq!(host_too_long.len(), 254);
        assert_eq!(
            RadrootsNip10RelayHint::parse(format!("wss://{host_too_long}")).unwrap_err(),
            RadrootsIdParseError::InvalidFormat
        );
    }

    #[test]
    fn enforces_content_and_relay_element_boundaries() {
        let exact_content = "x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES);
        RadrootsAuthoredNip10Reply::direct(exact_content, reference('a', 'b'))
            .expect("exact decoded content limit");
        assert!(matches!(
            RadrootsAuthoredNip10Reply::direct(
                "x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES + 1),
                reference('a', 'b'),
            ),
            Err(RadrootsNip10ReplyError::ContentTooLarge {
                max: RADROOTS_POST_CONTENT_MAX_BYTES,
                actual,
            }) if actual == RADROOTS_POST_CONTENT_MAX_BYTES + 1
        ));

        let prefix = "wss://relay.example/";
        let exact_relay = format!(
            "{prefix}{}",
            "a".repeat(RADROOTS_POST_TAG_ELEMENT_MAX_BYTES - prefix.len())
        );
        RadrootsNip10ReplyReference::parse("a".repeat(64), "b".repeat(64), Some(&exact_relay))
            .expect("exact tag-element limit");
        let overflow_relay = format!("{exact_relay}a");
        assert!(matches!(
            RadrootsNip10ReplyReference::parse(
                "a".repeat(64),
                "b".repeat(64),
                Some(&overflow_relay),
            ),
            Err(RadrootsNip10ReplyError::TagElementTooLarge {
                max: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
                actual,
            }) if actual == RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1
        ));
    }

    #[test]
    fn escaped_content_cannot_cross_compact_signed_wire_limit() {
        let mut lower = 1usize;
        let mut upper = RADROOTS_POST_CONTENT_MAX_BYTES;
        while lower < upper {
            let candidate = lower + (upper - lower).div_ceil(2);
            if RadrootsAuthoredNip10Reply::direct("\u{0001}".repeat(candidate), reference('a', 'b'))
                .is_ok()
            {
                lower = candidate;
            } else {
                upper = candidate - 1;
            }
        }

        RadrootsAuthoredNip10Reply::direct("\u{0001}".repeat(lower), reference('a', 'b'))
            .expect("largest escaped content fitting the wire budget");
        assert!(matches!(
            RadrootsAuthoredNip10Reply::direct("\u{0001}".repeat(lower + 1), reference('a', 'b'),),
            Err(RadrootsNip10ReplyError::EventWireTooLarge {
                max: RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
                ..
            })
        ));
    }
}

#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::fmt;

use crate::{
    ids::{RadrootsEventId, RadrootsIdParseError, RadrootsPublicKey},
    post::{
        RADROOTS_POST_CONTENT_MAX_BYTES, RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
        RADROOTS_POST_TAG_ELEMENT_MAX_BYTES, RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
    },
    relay_hint::RadrootsNostrRelayHint,
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

/// One syntactically validated reference used by strict NIP-10 authoring.
///
/// The caller asserts that the target is a kind-1 event. This value does not
/// retrieve the target or prove its existence, kind, signature, or author.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNip10ReplyReference {
    event_id: RadrootsEventId,
    author: RadrootsPublicKey,
    relay: Option<RadrootsNostrRelayHint>,
}

impl RadrootsNip10ReplyReference {
    pub fn new(
        event_id: RadrootsEventId,
        author: RadrootsPublicKey,
        relay: Option<RadrootsNostrRelayHint>,
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
                RadrootsNostrRelayHint::parse(relay)
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

    pub const fn relay(&self) -> Option<&RadrootsNostrRelayHint> {
        self.relay.as_ref()
    }

    pub fn relay_or_empty(&self) -> &str {
        self.relay
            .as_ref()
            .map_or("", RadrootsNostrRelayHint::as_str)
    }
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

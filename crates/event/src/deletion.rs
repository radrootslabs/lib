#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use core::fmt;

use crate::{
    ids::{
        RadrootsEventId, RadrootsIdParseError, RadrootsNip01Coordinate,
        RadrootsNip01CoordinateParseError,
    },
    wire::{
        DEFAULT_CONTENT_MAX_BYTES, DEFAULT_RAW_JSON_MAX_BYTES, DEFAULT_TAG_ELEMENT_MAX_BYTES,
        DEFAULT_TAG_MAX_COUNT, DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT, DEFAULT_TAG_TOTAL_MAX_BYTES,
    },
};

pub const RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES: usize = DEFAULT_CONTENT_MAX_BYTES;
pub const RADROOTS_NIP09_DELETION_TAG_MAX_COUNT: usize = DEFAULT_TAG_MAX_COUNT;
pub const RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT: usize =
    DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT;
pub const RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES: usize = DEFAULT_TAG_ELEMENT_MAX_BYTES;
pub const RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES: usize = DEFAULT_TAG_TOTAL_MAX_BYTES;
pub const RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES: usize = DEFAULT_RAW_JSON_MAX_BYTES;
pub const RADROOTS_NIP09_DELETION_TARGET_KIND_MAX: u32 = u16::MAX as u32;

const RADROOTS_NIP09_DELETION_SIGNED_EVENT_FIXED_MAX_BYTES: usize = "{\"id\":\"".len()
    + 64
    + "\",\"pubkey\":\"".len()
    + 64
    + "\",\"created_at\":".len()
    + 20
    + ",\"kind\":5,\"tags\":".len()
    + ",\"content\":".len()
    + ",\"sig\":\"".len()
    + 128
    + "\"}".len();

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip09DeletionError {
    ContentTooLarge { max: usize, actual: usize },
    EventIdInvalid(RadrootsIdParseError),
    CoordinateInvalid(RadrootsNip01CoordinateParseError),
    TargetKindOutOfRange { max: u32, actual: u32 },
    DuplicateEventTarget { event_id: String },
    DuplicateAddressTarget { coordinate: String },
    TargetMissing,
    TagCountExceeded { max: usize, actual: usize },
    TagElementTooLarge { max: usize, actual: usize },
    TagBytesExceeded { max: usize, actual: usize },
    EventWireTooLarge { max: usize, actual: usize },
}

impl RadrootsNip09DeletionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContentTooLarge { .. } => "deletion_content_too_large",
            Self::EventIdInvalid(_) | Self::TargetKindOutOfRange { .. } => {
                "deletion_event_target_invalid"
            }
            Self::CoordinateInvalid(_) => "deletion_address_target_invalid",
            Self::DuplicateEventTarget { .. } => "deletion_event_target_duplicate",
            Self::DuplicateAddressTarget { .. } => "deletion_address_target_duplicate",
            Self::TargetMissing => "deletion_target_missing",
            Self::TagCountExceeded { .. } => "deletion_tag_count_exceeded",
            Self::TagElementTooLarge { .. } => "deletion_tag_element_too_large",
            Self::TagBytesExceeded { .. } => "deletion_tag_bytes_exceeded",
            Self::EventWireTooLarge { .. } => "deletion_event_wire_too_large",
        }
    }
}

impl fmt::Display for RadrootsNip09DeletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentTooLarge { max, actual } => write!(
                formatter,
                "authored NIP-09 deletion content is {actual} bytes; max is {max}"
            ),
            Self::EventIdInvalid(error) => {
                write!(
                    formatter,
                    "NIP-09 deletion event target is invalid: {error}"
                )
            }
            Self::CoordinateInvalid(error) => {
                write!(
                    formatter,
                    "NIP-09 deletion address target is invalid: {error}"
                )
            }
            Self::TargetKindOutOfRange { max, actual } => write!(
                formatter,
                "NIP-09 deletion event target kind {actual} exceeds {max}"
            ),
            Self::DuplicateEventTarget { event_id } => write!(
                formatter,
                "NIP-09 deletion event target `{event_id}` is duplicated"
            ),
            Self::DuplicateAddressTarget { coordinate } => write!(
                formatter,
                "NIP-09 deletion address target {coordinate:?} is duplicated"
            ),
            Self::TargetMissing => {
                formatter.write_str("authored NIP-09 deletion requires an event or address target")
            }
            Self::TagCountExceeded { max, actual } => write!(
                formatter,
                "authored NIP-09 deletion has {actual} tags; max is {max}"
            ),
            Self::TagElementTooLarge { max, actual } => write!(
                formatter,
                "authored NIP-09 deletion tag element is {actual} bytes; max is {max}"
            ),
            Self::TagBytesExceeded { max, actual } => write!(
                formatter,
                "authored NIP-09 deletion tag bytes are {actual}; max is {max}"
            ),
            Self::EventWireTooLarge { max, actual } => write!(
                formatter,
                "authored NIP-09 deletion maximum canonical signed event size is {actual} bytes; max is {max}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip09DeletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EventIdInvalid(error) => Some(error),
            Self::CoordinateInvalid(error) => Some(error),
            _ => None,
        }
    }
}

/// One event-id target with caller-asserted target-kind metadata.
///
/// The kind hint is required for canonical authored `k` tags. This type does
/// not prove that the target exists or actually has the asserted kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNip09DeletionEventTarget {
    event_id: RadrootsEventId,
    kind_hint: u32,
}

impl RadrootsNip09DeletionEventTarget {
    pub fn new(
        event_id: RadrootsEventId,
        kind_hint: u32,
    ) -> Result<Self, RadrootsNip09DeletionError> {
        if kind_hint > RADROOTS_NIP09_DELETION_TARGET_KIND_MAX {
            return Err(RadrootsNip09DeletionError::TargetKindOutOfRange {
                max: RADROOTS_NIP09_DELETION_TARGET_KIND_MAX,
                actual: kind_hint,
            });
        }
        Ok(Self {
            event_id,
            kind_hint,
        })
    }

    pub fn parse(
        event_id: impl AsRef<str>,
        kind_hint: u32,
    ) -> Result<Self, RadrootsNip09DeletionError> {
        Self::new(
            RadrootsEventId::parse(event_id).map_err(RadrootsNip09DeletionError::EventIdInvalid)?,
            kind_hint,
        )
    }

    #[inline]
    pub const fn event_id(&self) -> &RadrootsEventId {
        &self.event_id
    }

    #[inline]
    pub const fn kind_hint(&self) -> u32 {
        self.kind_hint
    }
}

/// One NIP-01 replaceable or addressable coordinate target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNip09DeletionAddressTarget {
    coordinate: RadrootsNip01Coordinate,
}

impl RadrootsNip09DeletionAddressTarget {
    pub const fn new(coordinate: RadrootsNip01Coordinate) -> Self {
        Self { coordinate }
    }

    pub fn parse(coordinate: impl AsRef<str>) -> Result<Self, RadrootsNip09DeletionError> {
        let coordinate = coordinate.as_ref();
        if coordinate.len() > RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES {
            return Err(RadrootsNip09DeletionError::TagElementTooLarge {
                max: RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES,
                actual: coordinate.len(),
            });
        }
        RadrootsNip01Coordinate::parse(coordinate)
            .map(Self::new)
            .map_err(RadrootsNip09DeletionError::CoordinateInvalid)
    }

    #[inline]
    pub const fn coordinate(&self) -> &RadrootsNip01Coordinate {
        &self.coordinate
    }

    #[inline]
    pub const fn kind_hint(&self) -> u32 {
        self.coordinate.kind()
    }
}

/// Strict authored kind-5 NIP-09 deletion request.
///
/// Targets are canonicalized at construction. Event targets sort by event ID,
/// address targets sort by coordinate, and derived kind hints are unique and
/// ascending. The request remains an immutable protocol statement; it performs
/// no target-author authorization or deletion effect.
///
/// This type is opaque and has no Serde construction path.
///
/// ```compile_fail
/// let _: radroots_event::deletion::RadrootsAuthoredNip09DeletionRequest =
///     serde_json::from_str(
///         r#"{"content":"","event_targets":[],"address_targets":[]}"#
///     ).unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAuthoredNip09DeletionRequest {
    content: String,
    event_targets: Vec<RadrootsNip09DeletionEventTarget>,
    address_targets: Vec<RadrootsNip09DeletionAddressTarget>,
    kind_hints: Vec<u32>,
    maximum_signed_event_wire_bytes: usize,
}

impl RadrootsAuthoredNip09DeletionRequest {
    pub fn new(
        content: impl Into<String>,
        mut event_targets: Vec<RadrootsNip09DeletionEventTarget>,
        mut address_targets: Vec<RadrootsNip09DeletionAddressTarget>,
    ) -> Result<Self, RadrootsNip09DeletionError> {
        let content = content.into();
        if content.len() > RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES {
            return Err(RadrootsNip09DeletionError::ContentTooLarge {
                max: RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES,
                actual: content.len(),
            });
        }
        if event_targets.is_empty() && address_targets.is_empty() {
            return Err(RadrootsNip09DeletionError::TargetMissing);
        }

        let kind_hints = collect_authored_deletion_kind_hints(&event_targets, &address_targets)?;

        let maximum_signed_event_wire_bytes = validate_authored_deletion_wire_size(
            &content,
            &event_targets,
            &address_targets,
            &kind_hints,
        )?;

        event_targets.sort_by(|left, right| left.event_id.cmp(&right.event_id));
        if let Some(duplicates) = event_targets
            .windows(2)
            .find(|targets| targets[0].event_id == targets[1].event_id)
        {
            return Err(RadrootsNip09DeletionError::DuplicateEventTarget {
                event_id: duplicates[0].event_id.to_string(),
            });
        }

        address_targets.sort_by(|left, right| left.coordinate.cmp(&right.coordinate));
        if let Some(duplicates) = address_targets
            .windows(2)
            .find(|targets| targets[0].coordinate == targets[1].coordinate)
        {
            return Err(RadrootsNip09DeletionError::DuplicateAddressTarget {
                coordinate: duplicates[0].coordinate.to_string(),
            });
        }

        Ok(Self {
            content,
            event_targets,
            address_targets,
            kind_hints,
            maximum_signed_event_wire_bytes,
        })
    }

    #[inline]
    pub fn content(&self) -> &str {
        self.content.as_str()
    }

    #[inline]
    pub fn event_targets(&self) -> &[RadrootsNip09DeletionEventTarget] {
        self.event_targets.as_slice()
    }

    #[inline]
    pub fn address_targets(&self) -> &[RadrootsNip09DeletionAddressTarget] {
        self.address_targets.as_slice()
    }

    #[inline]
    pub fn kind_hints(&self) -> &[u32] {
        self.kind_hints.as_slice()
    }

    #[inline]
    pub fn target_count(&self) -> usize {
        self.event_targets
            .len()
            .saturating_add(self.address_targets.len())
    }

    /// Compact canonical signed-event size using `u64::MAX` for `created_at`.
    #[inline]
    pub const fn maximum_signed_event_wire_bytes(&self) -> usize {
        self.maximum_signed_event_wire_bytes
    }
}

fn collect_authored_deletion_kind_hints(
    event_targets: &[RadrootsNip09DeletionEventTarget],
    address_targets: &[RadrootsNip09DeletionAddressTarget],
) -> Result<Vec<u32>, RadrootsNip09DeletionError> {
    const WORD_BITS: usize = u64::BITS as usize;
    const WORD_COUNT: usize = (RADROOTS_NIP09_DELETION_TARGET_KIND_MAX as usize + 1) / WORD_BITS;

    let mut seen = [0_u64; WORD_COUNT];
    let mut unique_kind_count = 0usize;
    for kind in event_targets
        .iter()
        .map(|target| target.kind_hint())
        .chain(address_targets.iter().map(|target| target.kind_hint()))
    {
        let kind = kind as usize;
        let word = kind / WORD_BITS;
        let mask = 1_u64 << (kind % WORD_BITS);
        if seen[word] & mask == 0 {
            seen[word] |= mask;
            unique_kind_count = unique_kind_count.saturating_add(1);
        }
    }

    let target_count = event_targets.len().saturating_add(address_targets.len());
    let tag_count = target_count.saturating_add(unique_kind_count);
    if tag_count > RADROOTS_NIP09_DELETION_TAG_MAX_COUNT {
        return Err(RadrootsNip09DeletionError::TagCountExceeded {
            max: RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
            actual: tag_count,
        });
    }

    let mut kind_hints = Vec::with_capacity(unique_kind_count);
    for (word_index, word) in seen.into_iter().enumerate() {
        let mut remaining = word;
        while remaining != 0 {
            let bit = remaining.trailing_zeros() as usize;
            kind_hints.push((word_index * WORD_BITS + bit) as u32);
            remaining &= remaining - 1;
        }
    }
    Ok(kind_hints)
}

fn validate_authored_deletion_wire_size(
    content: &str,
    event_targets: &[RadrootsNip09DeletionEventTarget],
    address_targets: &[RadrootsNip09DeletionAddressTarget],
    kind_hints: &[u32],
) -> Result<usize, RadrootsNip09DeletionError> {
    let tag_count = event_targets
        .len()
        .saturating_add(address_targets.len())
        .saturating_add(kind_hints.len());
    if tag_count > RADROOTS_NIP09_DELETION_TAG_MAX_COUNT {
        return Err(RadrootsNip09DeletionError::TagCountExceeded {
            max: RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
            actual: tag_count,
        });
    }

    let mut tag_bytes = 0usize;
    let mut tags_json_bytes = 2usize;
    let mut visited_tags = 0usize;
    for target in event_targets {
        add_tag_size(
            &mut tag_bytes,
            &mut tags_json_bytes,
            &mut visited_tags,
            "e",
            target.event_id().as_str(),
        )?;
    }
    for target in address_targets {
        add_tag_size(
            &mut tag_bytes,
            &mut tags_json_bytes,
            &mut visited_tags,
            "a",
            target.coordinate().as_str(),
        )?;
    }
    for kind in kind_hints {
        let kind = kind.to_string();
        add_tag_size(
            &mut tag_bytes,
            &mut tags_json_bytes,
            &mut visited_tags,
            "k",
            kind.as_str(),
        )?;
    }

    if tag_bytes > RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES {
        return Err(RadrootsNip09DeletionError::TagBytesExceeded {
            max: RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES,
            actual: tag_bytes,
        });
    }

    let actual = RADROOTS_NIP09_DELETION_SIGNED_EVENT_FIXED_MAX_BYTES
        .saturating_add(tags_json_bytes)
        .saturating_add(canonical_json_string_bytes(content));
    if actual > RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES {
        return Err(RadrootsNip09DeletionError::EventWireTooLarge {
            max: RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES,
            actual,
        });
    }
    Ok(actual)
}

fn add_tag_size(
    tag_bytes: &mut usize,
    tags_json_bytes: &mut usize,
    tag_count: &mut usize,
    name: &str,
    value: &str,
) -> Result<(), RadrootsNip09DeletionError> {
    for element in [name, value] {
        if element.len() > RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES {
            return Err(RadrootsNip09DeletionError::TagElementTooLarge {
                max: RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES,
                actual: element.len(),
            });
        }
        *tag_bytes = tag_bytes.saturating_add(element.len());
    }
    if *tag_count > 0 {
        *tags_json_bytes = tags_json_bytes.saturating_add(1);
    }
    *tags_json_bytes = tags_json_bytes
        .saturating_add(2)
        .saturating_add(canonical_json_string_bytes(name))
        .saturating_add(1)
        .saturating_add(canonical_json_string_bytes(value));
    *tag_count = tag_count.saturating_add(1);
    Ok(())
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
    use crate::ids::RADROOTS_NIP01_COORDINATE_MAX_BYTES;

    fn event_target(character: char, kind: u32) -> RadrootsNip09DeletionEventTarget {
        RadrootsNip09DeletionEventTarget::parse(character.to_string().repeat(64), kind)
            .expect("event target")
    }

    fn numeric_event_target(index: usize, kind: u32) -> RadrootsNip09DeletionEventTarget {
        RadrootsNip09DeletionEventTarget::parse(format!("{index:064x}"), kind)
            .expect("numeric event target")
    }

    fn address_target(
        kind: u32,
        character: char,
        identifier: &str,
    ) -> RadrootsNip09DeletionAddressTarget {
        RadrootsNip09DeletionAddressTarget::parse(format!(
            "{kind}:{}:{identifier}",
            crate::test_valid_hex_64(character)
        ))
        .expect("address target")
    }

    fn address_target_with_total_bytes(
        total_bytes: usize,
        index: usize,
    ) -> RadrootsNip09DeletionAddressTarget {
        let prefix = format!("30000:{}:", crate::test_valid_hex_64('a'));
        let suffix = format!("{index:04x}");
        assert!(prefix.len() + suffix.len() <= total_bytes);
        RadrootsNip09DeletionAddressTarget::parse(format!(
            "{prefix}{}{suffix}",
            "x".repeat(total_bytes - prefix.len() - suffix.len())
        ))
        .expect("fixed-size address target")
    }

    #[test]
    fn target_constructors_validate_identity_and_kind_hint() {
        let target = RadrootsNip09DeletionEventTarget::parse("A".repeat(64), u16::MAX as u32)
            .expect("event target");
        assert_eq!(target.event_id().as_str(), "a".repeat(64));
        assert_eq!(target.kind_hint(), u16::MAX as u32);
        assert_eq!(
            RadrootsNip09DeletionEventTarget::parse("5".repeat(64), 5)
                .expect("kind-5 target")
                .kind_hint(),
            5
        );
        assert!(matches!(
            RadrootsNip09DeletionEventTarget::parse("not-an-id", 1),
            Err(RadrootsNip09DeletionError::EventIdInvalid(
                RadrootsIdParseError::InvalidLength {
                    expected: 64,
                    actual: 9
                }
            ))
        ));
        assert_eq!(
            RadrootsNip09DeletionEventTarget::parse("a".repeat(64), u16::MAX as u32 + 1)
                .unwrap_err(),
            RadrootsNip09DeletionError::TargetKindOutOfRange {
                max: u16::MAX as u32,
                actual: u16::MAX as u32 + 1,
            }
        );

        let address = RadrootsNip09DeletionAddressTarget::parse(format!(
            "30000:{}:",
            crate::test_valid_hex_64('B')
        ))
        .expect("address target");
        assert_eq!(
            address.coordinate().as_str(),
            format!("30000:{}:", crate::test_valid_hex_64('b'))
        );
        assert_eq!(address.kind_hint(), 30_000);

        let coordinate_prefix = format!("30000:{}:", "a".repeat(64));
        let oversized_coordinate = format!(
            "{coordinate_prefix}{}",
            "x".repeat(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES + 1 - coordinate_prefix.len())
        );
        assert_eq!(
            RadrootsNip09DeletionAddressTarget::parse(oversized_coordinate),
            Err(RadrootsNip09DeletionError::TagElementTooLarge {
                max: RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES,
                actual: RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES + 1,
            })
        );
    }

    #[test]
    fn authored_request_allows_event_address_and_mixed_batches() {
        let event_only =
            RadrootsAuthoredNip09DeletionRequest::new("", vec![event_target('a', 1)], Vec::new())
                .expect("event-only request");
        assert_eq!(event_only.target_count(), 1);
        assert_eq!(event_only.kind_hints(), &[1]);

        let address_only = RadrootsAuthoredNip09DeletionRequest::new(
            "withdrawn",
            Vec::new(),
            vec![address_target(30_402, 'b', "victoria-kale")],
        )
        .expect("address-only request");
        assert_eq!(address_only.target_count(), 1);
        assert_eq!(address_only.kind_hints(), &[30_402]);

        let mixed = RadrootsAuthoredNip09DeletionRequest::new(
            "\t撤回 🌱\n",
            vec![event_target('c', 31_922)],
            vec![address_target(30_402, 'd', "victoria-carrots")],
        )
        .expect("mixed request");
        assert_eq!(mixed.content(), "\t撤回 🌱\n");
        assert_eq!(mixed.target_count(), 2);
        assert_eq!(mixed.kind_hints(), &[30_402, 31_922]);
    }

    #[test]
    fn authored_request_sorts_targets_and_deduplicates_kind_hints() {
        let request = RadrootsAuthoredNip09DeletionRequest::new(
            "duplicate crop listing",
            vec![event_target('f', 30_402), event_target('a', 1)],
            vec![
                address_target(31_923, 'e', "harvest"),
                address_target(30_402, 'b', "produce"),
            ],
        )
        .expect("canonical request");

        assert_eq!(
            request.event_targets()[0].event_id().as_str(),
            "a".repeat(64)
        );
        assert_eq!(
            request.event_targets()[1].event_id().as_str(),
            "f".repeat(64)
        );
        assert_eq!(
            request.address_targets()[0].coordinate().as_str(),
            format!("30402:{}:produce", crate::test_valid_hex_64('b'))
        );
        assert_eq!(
            request.address_targets()[1].coordinate().as_str(),
            format!("31923:{}:harvest", crate::test_valid_hex_64('e'))
        );
        assert_eq!(request.kind_hints(), &[1, 30_402, 31_923]);
    }

    #[test]
    fn authored_request_rejects_canonical_duplicate_targets() {
        let uppercase =
            RadrootsNip09DeletionEventTarget::parse("A".repeat(64), 1).expect("uppercase event");
        let lowercase = RadrootsNip09DeletionEventTarget::parse("a".repeat(64), 31_922)
            .expect("lowercase event");
        assert_eq!(
            RadrootsAuthoredNip09DeletionRequest::new("", vec![uppercase, lowercase], Vec::new(),)
                .unwrap_err(),
            RadrootsNip09DeletionError::DuplicateEventTarget {
                event_id: "a".repeat(64)
            }
        );

        let uppercase_address = RadrootsNip09DeletionAddressTarget::parse(format!(
            "030402:{}:produce",
            crate::test_valid_hex_64('A')
        ))
        .expect("uppercase address");
        assert_eq!(
            RadrootsAuthoredNip09DeletionRequest::new(
                "",
                Vec::new(),
                vec![uppercase_address, address_target(30_402, 'a', "produce"),],
            )
            .unwrap_err(),
            RadrootsNip09DeletionError::DuplicateAddressTarget {
                coordinate: format!("30402:{}:produce", crate::test_valid_hex_64('a'))
            }
        );
    }

    #[test]
    fn authored_request_requires_target_union_and_caps_content() {
        assert_eq!(
            RadrootsAuthoredNip09DeletionRequest::new("", Vec::new(), Vec::new()).unwrap_err(),
            RadrootsNip09DeletionError::TargetMissing
        );
        RadrootsAuthoredNip09DeletionRequest::new(
            "x".repeat(RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES),
            vec![event_target('a', 1)],
            Vec::new(),
        )
        .expect("exact content byte limit");
        assert_eq!(
            RadrootsAuthoredNip09DeletionRequest::new(
                "x".repeat(RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES + 1),
                vec![event_target('a', 1)],
                Vec::new(),
            )
            .unwrap_err(),
            RadrootsNip09DeletionError::ContentTooLarge {
                max: RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES,
                actual: RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES + 1,
            }
        );
    }

    #[test]
    fn authored_request_enforces_tag_count_budget() {
        let exact_targets = (0..RADROOTS_NIP09_DELETION_TAG_MAX_COUNT - 1)
            .map(|index| numeric_event_target(index, 1))
            .collect();
        RadrootsAuthoredNip09DeletionRequest::new("", exact_targets, Vec::new())
            .expect("1023 targets plus one kind tag");

        let overflow_targets = (0..RADROOTS_NIP09_DELETION_TAG_MAX_COUNT)
            .map(|index| numeric_event_target(index, 1))
            .collect();
        assert_eq!(
            RadrootsAuthoredNip09DeletionRequest::new("", overflow_targets, Vec::new())
                .unwrap_err(),
            RadrootsNip09DeletionError::TagCountExceeded {
                max: RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
                actual: RADROOTS_NIP09_DELETION_TAG_MAX_COUNT + 1,
            }
        );

        let duplicate_overflow = vec![event_target('a', 1); RADROOTS_NIP09_DELETION_TAG_MAX_COUNT];
        assert_eq!(
            RadrootsAuthoredNip09DeletionRequest::new("", duplicate_overflow, Vec::new())
                .unwrap_err(),
            RadrootsNip09DeletionError::TagCountExceeded {
                max: RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
                actual: RADROOTS_NIP09_DELETION_TAG_MAX_COUNT + 1,
            }
        );
    }

    #[test]
    fn duplicate_address_error_escapes_opaque_identifier_controls() {
        let target = address_target(30_000, 'b', "line\nbreak");
        let error =
            RadrootsAuthoredNip09DeletionRequest::new("", Vec::new(), vec![target.clone(), target])
                .unwrap_err();
        let rendered = error.to_string();
        assert!(rendered.contains("\\n"));
        assert!(!rendered.contains('\n'));
    }

    #[test]
    fn authored_request_enforces_exact_aggregate_tag_byte_budget() {
        let mut exact_targets = (0..31)
            .map(|index| {
                address_target_with_total_bytes(RADROOTS_NIP01_COORDINATE_MAX_BYTES, index)
            })
            .collect::<Vec<_>>();
        exact_targets.push(address_target_with_total_bytes(4_058, 31));
        let exact_tag_bytes = exact_targets
            .iter()
            .map(|target| 1 + target.coordinate().as_str().len())
            .sum::<usize>()
            + "k".len()
            + "30000".len();
        assert_eq!(exact_tag_bytes, RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES);
        RadrootsAuthoredNip09DeletionRequest::new("", Vec::new(), exact_targets)
            .expect("exact aggregate tag byte limit");

        let mut overflow_targets = (0..31)
            .map(|index| {
                address_target_with_total_bytes(RADROOTS_NIP01_COORDINATE_MAX_BYTES, index)
            })
            .collect::<Vec<_>>();
        overflow_targets.push(address_target_with_total_bytes(4_059, 31));
        assert_eq!(
            RadrootsAuthoredNip09DeletionRequest::new("", Vec::new(), overflow_targets)
                .unwrap_err(),
            RadrootsNip09DeletionError::TagBytesExceeded {
                max: RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES,
                actual: RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES + 1,
            }
        );
    }

    #[test]
    fn authored_request_counts_json_escaping_in_wire_budget() {
        let mut lower = 0usize;
        let mut upper = RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES;
        while lower < upper {
            let candidate = lower + (upper - lower).div_ceil(2);
            if RadrootsAuthoredNip09DeletionRequest::new(
                "\u{0001}".repeat(candidate),
                vec![event_target('a', 1)],
                Vec::new(),
            )
            .is_ok()
            {
                lower = candidate;
            } else {
                upper = candidate - 1;
            }
        }
        RadrootsAuthoredNip09DeletionRequest::new(
            "\u{0001}".repeat(lower),
            vec![event_target('a', 1)],
            Vec::new(),
        )
        .expect("largest escaped content fitting the wire budget");
        assert!(matches!(
            RadrootsAuthoredNip09DeletionRequest::new(
                "\u{0001}".repeat(lower + 1),
                vec![event_target('a', 1)],
                Vec::new(),
            ),
            Err(RadrootsNip09DeletionError::EventWireTooLarge { max, .. })
                if max == RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES
        ));
    }

    #[test]
    fn deletion_errors_expose_stable_codes_and_messages() {
        let errors = [
            RadrootsNip09DeletionError::ContentTooLarge { max: 1, actual: 2 },
            RadrootsNip09DeletionError::EventIdInvalid(RadrootsIdParseError::InvalidFormat),
            RadrootsNip09DeletionError::TargetKindOutOfRange { max: 1, actual: 2 },
            RadrootsNip09DeletionError::CoordinateInvalid(
                RadrootsNip01CoordinateParseError::InvalidFormat,
            ),
            RadrootsNip09DeletionError::DuplicateEventTarget {
                event_id: "a".repeat(64),
            },
            RadrootsNip09DeletionError::DuplicateAddressTarget {
                coordinate: format!("30000:{}:", "a".repeat(64)),
            },
            RadrootsNip09DeletionError::TargetMissing,
            RadrootsNip09DeletionError::TagCountExceeded { max: 1, actual: 2 },
            RadrootsNip09DeletionError::TagElementTooLarge { max: 1, actual: 2 },
            RadrootsNip09DeletionError::TagBytesExceeded { max: 1, actual: 2 },
            RadrootsNip09DeletionError::EventWireTooLarge { max: 1, actual: 2 },
        ];
        let expected = [
            "deletion_content_too_large",
            "deletion_event_target_invalid",
            "deletion_event_target_invalid",
            "deletion_address_target_invalid",
            "deletion_event_target_duplicate",
            "deletion_address_target_duplicate",
            "deletion_target_missing",
            "deletion_tag_count_exceeded",
            "deletion_tag_element_too_large",
            "deletion_tag_bytes_exceeded",
            "deletion_event_wire_too_large",
        ];
        for (error, expected) in errors.into_iter().zip(expected) {
            assert_eq!(error.code(), expected);
            assert!(!error.to_string().is_empty());
        }
    }
}

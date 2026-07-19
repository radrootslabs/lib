#[cfg(not(feature = "std"))]
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;
#[cfg(feature = "std")]
use std::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};

use radroots_event::{
    deletion::{
        RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES, RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES,
        RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES, RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
        RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT,
        RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES, RADROOTS_NIP09_DELETION_TARGET_KIND_MAX,
    },
    ids::{
        RadrootsEventId, RadrootsIdParseError, RadrootsNip01Coordinate,
        RadrootsNip01CoordinateParseError,
    },
    kinds::KIND_DELETION_REQUEST,
};

use crate::verification::RadrootsSignatureVerifiedEvent;

const RADROOTS_NIP09_DELETION_SIGNED_EVENT_FIXED_BYTES: usize = "{\"id\":\"".len()
    + 64
    + "\",\"pubkey\":\"".len()
    + 64
    + "\",\"created_at\":".len()
    + ",\"kind\":5,\"tags\":".len()
    + ",\"content\":".len()
    + ",\"sig\":\"".len()
    + 128
    + "\"}".len();

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip09DeletionDiagnostic {
    KindAdvisoryShapeIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    KindAdvisoryInvalidIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    KindAdvisoryDuplicateIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    KindAdvisoryConflictIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
}

impl RadrootsNip09DeletionDiagnostic {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::KindAdvisoryShapeIgnored { .. } => "deletion_kind_advisory_shape_ignored",
            Self::KindAdvisoryInvalidIgnored { .. } => "deletion_kind_advisory_invalid_ignored",
            Self::KindAdvisoryDuplicateIgnored { .. } => "deletion_kind_advisory_duplicate_ignored",
            Self::KindAdvisoryConflictIgnored { .. } => "deletion_kind_advisory_conflict_ignored",
        }
    }

    pub const fn tag_index(&self) -> usize {
        match self {
            Self::KindAdvisoryShapeIgnored { tag_index, .. }
            | Self::KindAdvisoryInvalidIgnored { tag_index, .. }
            | Self::KindAdvisoryDuplicateIgnored { tag_index, .. }
            | Self::KindAdvisoryConflictIgnored { tag_index, .. } => *tag_index,
        }
    }

    pub fn raw_tag(&self) -> &[String] {
        match self {
            Self::KindAdvisoryShapeIgnored { raw_tag, .. }
            | Self::KindAdvisoryInvalidIgnored { raw_tag, .. }
            | Self::KindAdvisoryDuplicateIgnored { raw_tag, .. }
            | Self::KindAdvisoryConflictIgnored { raw_tag, .. } => raw_tag,
        }
    }
}

impl fmt::Display for RadrootsNip09DeletionDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip09DeletionEventTarget {
    tag_index: usize,
    event_id: RadrootsEventId,
    raw_tag: Vec<String>,
}

impl RadrootsInboundNip09DeletionEventTarget {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub const fn event_id(&self) -> &RadrootsEventId {
        &self.event_id
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip09DeletionAddressTarget {
    tag_index: usize,
    coordinate: RadrootsNip01Coordinate,
    raw_tag: Vec<String>,
}

impl RadrootsInboundNip09DeletionAddressTarget {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub const fn coordinate(&self) -> &RadrootsNip01Coordinate {
        &self.coordinate
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip09DeletionKindAdvisory {
    tag_index: usize,
    kind: u32,
    raw_tag: Vec<String>,
}

impl RadrootsInboundNip09DeletionKindAdvisory {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub const fn kind(&self) -> u32 {
        self.kind
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }
}

/// Tolerant effect-free projection of one verified kind-5 request.
///
/// Raw tags preserve exact source order, duplicates, trailing elements, and
/// unknown tags. Canonical target and advisory views are unique and sorted,
/// retaining first-seen source provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip09DeletionProjection {
    event_targets: Vec<RadrootsInboundNip09DeletionEventTarget>,
    address_targets: Vec<RadrootsInboundNip09DeletionAddressTarget>,
    kind_advisories: Vec<RadrootsInboundNip09DeletionKindAdvisory>,
    diagnostics: Vec<RadrootsNip09DeletionDiagnostic>,
    raw_tags: Vec<Vec<String>>,
}

impl RadrootsInboundNip09DeletionProjection {
    pub fn event_targets(&self) -> &[RadrootsInboundNip09DeletionEventTarget] {
        &self.event_targets
    }

    pub fn address_targets(&self) -> &[RadrootsInboundNip09DeletionAddressTarget] {
        &self.address_targets
    }

    pub fn kind_advisories(&self) -> &[RadrootsInboundNip09DeletionKindAdvisory] {
        &self.kind_advisories
    }

    pub fn diagnostics(&self) -> &[RadrootsNip09DeletionDiagnostic] {
        &self.diagnostics
    }

    pub fn raw_tags(&self) -> &[Vec<String>] {
        &self.raw_tags
    }

    pub const fn contract_id(&self) -> &'static str {
        "radroots.social.deletion_request.v1"
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip09DeletionProjectionError {
    UnsupportedKind {
        actual: u32,
    },
    ContentTooLarge {
        max: usize,
        actual: usize,
    },
    TagCountExceeded {
        max: usize,
        actual: usize,
    },
    TagElementCountExceeded {
        max: usize,
        actual: usize,
    },
    TagElementTooLarge {
        max: usize,
        actual: usize,
        tag_index: usize,
        element_index: usize,
    },
    TagBytesExceeded {
        max: usize,
        actual: usize,
    },
    EventWireTooLarge {
        max: usize,
        actual: usize,
    },
    EventTargetShape {
        tag_index: usize,
    },
    EventTargetInvalid {
        tag_index: usize,
        error: RadrootsIdParseError,
    },
    AddressTargetShape {
        tag_index: usize,
    },
    AddressTargetInvalid {
        tag_index: usize,
        error: RadrootsNip01CoordinateParseError,
    },
    TargetMissing,
}

impl RadrootsNip09DeletionProjectionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedKind { .. } => "unsupported_kind",
            Self::ContentTooLarge { .. } => "deletion_content_too_large",
            Self::TagCountExceeded { .. } => "deletion_tag_count_exceeded",
            Self::TagElementCountExceeded { .. } => "deletion_tag_element_count_exceeded",
            Self::TagElementTooLarge { .. } => "deletion_tag_element_too_large",
            Self::TagBytesExceeded { .. } => "deletion_tag_bytes_exceeded",
            Self::EventWireTooLarge { .. } => "deletion_event_wire_too_large",
            Self::EventTargetShape { .. } => "deletion_event_target_shape",
            Self::EventTargetInvalid { .. } => "deletion_event_target_invalid",
            Self::AddressTargetShape { .. } => "deletion_address_target_shape",
            Self::AddressTargetInvalid { .. } => "deletion_address_target_invalid",
            Self::TargetMissing => "deletion_target_missing",
        }
    }
}

impl fmt::Display for RadrootsNip09DeletionProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedKind { actual } => {
                write!(formatter, "NIP-09 deletion kind must be 5, got {actual}")
            }
            Self::ContentTooLarge { max, actual } => write!(
                formatter,
                "NIP-09 deletion content is {actual} bytes; max is {max}"
            ),
            Self::TagCountExceeded { max, actual } => {
                write!(formatter, "NIP-09 deletion has {actual} tags; max is {max}")
            }
            Self::TagElementCountExceeded { max, actual } => write!(
                formatter,
                "NIP-09 deletion has {actual} total tag elements; max is {max}"
            ),
            Self::TagElementTooLarge {
                max,
                actual,
                tag_index,
                element_index,
            } => write!(
                formatter,
                "NIP-09 deletion tag {tag_index} element {element_index} is {actual} bytes; max is {max}"
            ),
            Self::TagBytesExceeded { max, actual } => write!(
                formatter,
                "NIP-09 deletion tag bytes are {actual}; max is {max}"
            ),
            Self::EventWireTooLarge { max, actual } => write!(
                formatter,
                "NIP-09 deletion compact signed event is {actual} bytes; max is {max}"
            ),
            Self::EventTargetShape { tag_index } => write!(
                formatter,
                "NIP-09 deletion event target tag {tag_index} has an invalid shape"
            ),
            Self::EventTargetInvalid { tag_index, error } => write!(
                formatter,
                "NIP-09 deletion event target tag {tag_index} is invalid: {error}"
            ),
            Self::AddressTargetShape { tag_index } => write!(
                formatter,
                "NIP-09 deletion address target tag {tag_index} has an invalid shape"
            ),
            Self::AddressTargetInvalid { tag_index, error } => write!(
                formatter,
                "NIP-09 deletion address target tag {tag_index} is invalid: {error}"
            ),
            Self::TargetMissing => {
                formatter.write_str("NIP-09 deletion requires a valid event or address target")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip09DeletionProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EventTargetInvalid { error, .. } => Some(error),
            Self::AddressTargetInvalid { error, .. } => Some(error),
            _ => None,
        }
    }
}

/// Projects a signature-and-id verified kind-5 NIP-09 deletion request.
///
/// This boundary validates and canonicalizes request metadata only. It performs
/// no target lookup, same-author authorization, suppression, store mutation,
/// address cutoff, replacement, or deletion-request immunity evaluation.
pub fn project_verified_nip09_deletion_request_event(
    verified_event: &RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsInboundNip09DeletionProjection, RadrootsNip09DeletionProjectionError> {
    let event = verified_event.event();
    project_nip09_deletion_request_parts(
        event.kind_u32(),
        &event.tags_as_vec(),
        event.content(),
        event.created_at_u64(),
    )
}

pub(crate) fn project_nip09_deletion_request_parts(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    created_at: u64,
) -> Result<RadrootsInboundNip09DeletionProjection, RadrootsNip09DeletionProjectionError> {
    if kind != KIND_DELETION_REQUEST {
        return Err(RadrootsNip09DeletionProjectionError::UnsupportedKind { actual: kind });
    }
    if content.len() > RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES {
        return Err(RadrootsNip09DeletionProjectionError::ContentTooLarge {
            max: RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES,
            actual: content.len(),
        });
    }
    validate_tag_and_wire_budgets(tags, content, decimal_digits(created_at))?;

    let mut event_targets = BTreeMap::new();
    let mut address_targets = BTreeMap::new();
    for (tag_index, tag) in tags.iter().enumerate() {
        match tag.first().map(String::as_str) {
            Some("e") => {
                let Some(value) = tag.get(1) else {
                    return Err(RadrootsNip09DeletionProjectionError::EventTargetShape {
                        tag_index,
                    });
                };
                let event_id = RadrootsEventId::parse(value).map_err(|error| {
                    RadrootsNip09DeletionProjectionError::EventTargetInvalid { tag_index, error }
                })?;
                if !event_targets.contains_key(&event_id) {
                    event_targets.insert(
                        event_id.clone(),
                        RadrootsInboundNip09DeletionEventTarget {
                            tag_index,
                            event_id,
                            raw_tag: tag.clone(),
                        },
                    );
                }
            }
            Some("a") => {
                let Some(value) = tag.get(1) else {
                    return Err(RadrootsNip09DeletionProjectionError::AddressTargetShape {
                        tag_index,
                    });
                };
                let coordinate = RadrootsNip01Coordinate::parse(value).map_err(|error| {
                    RadrootsNip09DeletionProjectionError::AddressTargetInvalid { tag_index, error }
                })?;
                if !address_targets.contains_key(&coordinate) {
                    address_targets.insert(
                        coordinate.clone(),
                        RadrootsInboundNip09DeletionAddressTarget {
                            tag_index,
                            coordinate,
                            raw_tag: tag.clone(),
                        },
                    );
                }
            }
            _ => {}
        }
    }
    if event_targets.is_empty() && address_targets.is_empty() {
        return Err(RadrootsNip09DeletionProjectionError::TargetMissing);
    }

    let has_event_targets = !event_targets.is_empty();
    let address_kinds = address_targets
        .keys()
        .map(RadrootsNip01Coordinate::kind)
        .collect::<BTreeSet<_>>();
    let mut kind_advisories = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for (tag_index, tag) in tags.iter().enumerate() {
        if !tag.first().is_some_and(|name| name == "k") {
            continue;
        }
        let Some(value) = tag.get(1) else {
            diagnostics.push(RadrootsNip09DeletionDiagnostic::KindAdvisoryShapeIgnored {
                tag_index,
                raw_tag: tag.clone(),
            });
            continue;
        };
        let Ok(kind) = value.parse::<u32>() else {
            diagnostics.push(
                RadrootsNip09DeletionDiagnostic::KindAdvisoryInvalidIgnored {
                    tag_index,
                    raw_tag: tag.clone(),
                },
            );
            continue;
        };
        if kind > RADROOTS_NIP09_DELETION_TARGET_KIND_MAX || kind.to_string() != *value {
            diagnostics.push(
                RadrootsNip09DeletionDiagnostic::KindAdvisoryInvalidIgnored {
                    tag_index,
                    raw_tag: tag.clone(),
                },
            );
            continue;
        }
        if kind_advisories.contains_key(&kind) {
            diagnostics.push(
                RadrootsNip09DeletionDiagnostic::KindAdvisoryDuplicateIgnored {
                    tag_index,
                    raw_tag: tag.clone(),
                },
            );
            continue;
        }
        kind_advisories.insert(
            kind,
            RadrootsInboundNip09DeletionKindAdvisory {
                tag_index,
                kind,
                raw_tag: tag.clone(),
            },
        );
    }

    if !has_event_targets {
        for (kind, advisory) in &kind_advisories {
            if !address_kinds.contains(kind) {
                diagnostics.push(
                    RadrootsNip09DeletionDiagnostic::KindAdvisoryConflictIgnored {
                        tag_index: advisory.tag_index,
                        raw_tag: advisory.raw_tag.clone(),
                    },
                );
            }
        }
    }
    diagnostics.sort_by_key(RadrootsNip09DeletionDiagnostic::tag_index);

    Ok(RadrootsInboundNip09DeletionProjection {
        event_targets: event_targets.into_values().collect(),
        address_targets: address_targets.into_values().collect(),
        kind_advisories: kind_advisories.into_values().collect(),
        diagnostics,
        raw_tags: tags.to_vec(),
    })
}

fn validate_tag_and_wire_budgets(
    tags: &[Vec<String>],
    content: &str,
    created_at_digits: usize,
) -> Result<(), RadrootsNip09DeletionProjectionError> {
    if tags.len() > RADROOTS_NIP09_DELETION_TAG_MAX_COUNT {
        return Err(RadrootsNip09DeletionProjectionError::TagCountExceeded {
            max: RADROOTS_NIP09_DELETION_TAG_MAX_COUNT,
            actual: tags.len(),
        });
    }
    let tag_element_count = tags
        .iter()
        .fold(0usize, |total, tag| total.saturating_add(tag.len()));
    if tag_element_count > RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT {
        return Err(
            RadrootsNip09DeletionProjectionError::TagElementCountExceeded {
                max: RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT,
                actual: tag_element_count,
            },
        );
    }

    let mut tag_bytes = 0usize;
    let mut tags_json_bytes = 2usize;
    for (tag_index, tag) in tags.iter().enumerate() {
        if tag_index > 0 {
            tags_json_bytes = tags_json_bytes.saturating_add(1);
        }
        tags_json_bytes = tags_json_bytes.saturating_add(2);
        for (element_index, element) in tag.iter().enumerate() {
            if element.len() > RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES {
                return Err(RadrootsNip09DeletionProjectionError::TagElementTooLarge {
                    max: RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES,
                    actual: element.len(),
                    tag_index,
                    element_index,
                });
            }
            if element_index > 0 {
                tags_json_bytes = tags_json_bytes.saturating_add(1);
            }
            tags_json_bytes = tags_json_bytes.saturating_add(canonical_json_string_bytes(element));
            tag_bytes = tag_bytes.saturating_add(element.len());
        }
    }
    if tag_bytes > RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES {
        return Err(RadrootsNip09DeletionProjectionError::TagBytesExceeded {
            max: RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES,
            actual: tag_bytes,
        });
    }

    let actual = RADROOTS_NIP09_DELETION_SIGNED_EVENT_FIXED_BYTES
        .saturating_add(created_at_digits)
        .saturating_add(tags_json_bytes)
        .saturating_add(canonical_json_string_bytes(content));
    if actual > RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES {
        return Err(RadrootsNip09DeletionProjectionError::EventWireTooLarge {
            max: RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES,
            actual,
        });
    }
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

const fn decimal_digits(mut value: u64) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(character: char) -> String {
        character.to_string().repeat(64)
    }

    fn valid_event_tags() -> Vec<Vec<String>> {
        vec![
            vec!["e".to_string(), h('a')],
            vec!["k".to_string(), "1".to_string()],
        ]
    }

    #[test]
    fn projects_raw_mixed_duplicates_trailing_and_unknown_tags() {
        let coordinate_b = format!("30402:{}:produce", h('b'));
        let coordinate_c = format!("31923:{}:market", h('c'));
        let tags = vec![
            vec!["x".to_string(), "unknown".to_string()],
            vec!["e".to_string(), h('f'), "relay".to_string()],
            vec![
                "a".to_string(),
                coordinate_c.clone(),
                "trailing".to_string(),
            ],
            vec!["e".to_string(), h('a')],
            vec!["e".to_string(), h('F'), "duplicate".to_string()],
            vec!["a".to_string(), coordinate_b.clone()],
            vec![
                "a".to_string(),
                format!("030402:{}:produce", h('B')),
                "duplicate".to_string(),
            ],
            vec!["k".to_string(), "31923".to_string(), "trailing".to_string()],
            vec!["k".to_string(), "1".to_string()],
        ];
        let projection = project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1)
            .expect("projection");

        assert_eq!(projection.raw_tags(), tags);
        assert_eq!(
            projection
                .event_targets()
                .iter()
                .map(|target| (target.event_id().as_str(), target.tag_index()))
                .collect::<Vec<_>>(),
            vec![(h('a').as_str(), 3), (h('f').as_str(), 1)]
        );
        assert_eq!(
            projection
                .address_targets()
                .iter()
                .map(|target| (target.coordinate().as_str(), target.tag_index()))
                .collect::<Vec<_>>(),
            vec![(coordinate_b.as_str(), 5), (coordinate_c.as_str(), 2)]
        );
        assert_eq!(
            projection
                .kind_advisories()
                .iter()
                .map(|advisory| (advisory.kind(), advisory.tag_index()))
                .collect::<Vec<_>>(),
            vec![(1, 8), (31_923, 7)]
        );
        assert_eq!(projection.event_targets()[1].raw_tag(), tags[1]);
        assert_eq!(projection.address_targets()[1].raw_tag(), tags[2]);
        assert!(projection.diagnostics().is_empty());
    }

    #[test]
    fn emits_advisory_diagnostics_in_source_order() {
        let coordinate = format!("30402:{}:produce", h('b'));
        let tags = vec![
            vec!["k".to_string()],
            vec!["a".to_string(), coordinate],
            vec!["k".to_string(), "+30402".to_string()],
            vec!["k".to_string(), "30402".to_string()],
            vec![
                "k".to_string(),
                "30402".to_string(),
                "duplicate".to_string(),
            ],
            vec!["k".to_string(), "31923".to_string()],
            vec!["k".to_string(), "65536".to_string()],
        ];
        let projection = project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1)
            .expect("projection");

        assert_eq!(
            projection
                .diagnostics()
                .iter()
                .map(|diagnostic| (diagnostic.code(), diagnostic.tag_index()))
                .collect::<Vec<_>>(),
            vec![
                ("deletion_kind_advisory_shape_ignored", 0),
                ("deletion_kind_advisory_invalid_ignored", 2),
                ("deletion_kind_advisory_duplicate_ignored", 4),
                ("deletion_kind_advisory_conflict_ignored", 5),
                ("deletion_kind_advisory_invalid_ignored", 6),
            ]
        );
        assert_eq!(
            projection
                .kind_advisories()
                .iter()
                .map(RadrootsInboundNip09DeletionKindAdvisory::kind)
                .collect::<Vec<_>>(),
            vec![30_402, 31_923]
        );
        for diagnostic in projection.diagnostics() {
            assert_eq!(diagnostic.raw_tag(), tags[diagnostic.tag_index()]);
        }
    }

    #[test]
    fn event_target_prevents_unprovable_kind_conflict() {
        let tags = vec![
            vec!["a".to_string(), format!("30402:{}:produce", h('b'))],
            vec!["e".to_string(), h('a')],
            vec!["k".to_string(), "31923".to_string()],
        ];
        let projection = project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1)
            .expect("mixed projection");
        assert_eq!(projection.kind_advisories()[0].kind(), 31_923);
        assert!(projection.diagnostics().is_empty());
    }

    #[test]
    fn accepts_empty_content_and_trailing_target_fields() {
        let tags = vec![
            vec!["e".to_string(), h('a'), String::new(), "extra".to_string()],
            vec![
                "a".to_string(),
                format!("30000:{}:", h('b')),
                "extra".to_string(),
            ],
        ];
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1)
            .expect("trailing fields");
    }

    #[test]
    fn first_malformed_target_in_source_order_is_a_hard_error() {
        let tags = vec![
            vec!["a".to_string(), format!("30000:{}:", h('b'))],
            vec!["e".to_string()],
            vec!["a".to_string(), "bad".to_string()],
        ];
        assert_eq!(
            project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1).unwrap_err(),
            RadrootsNip09DeletionProjectionError::EventTargetShape { tag_index: 1 }
        );

        let tags = vec![
            vec!["e".to_string(), h('a')],
            vec!["a".to_string(), "bad".to_string()],
            vec!["e".to_string(), "bad".to_string()],
        ];
        assert!(matches!(
            project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, "", 1),
            Err(RadrootsNip09DeletionProjectionError::AddressTargetInvalid { tag_index: 1, .. })
        ));
    }

    #[test]
    fn missing_target_union_follows_target_shape_and_validity() {
        assert_eq!(
            project_nip09_deletion_request_parts(
                KIND_DELETION_REQUEST,
                &[vec!["x".to_string()], vec!["k".to_string()]],
                "",
                1,
            )
            .unwrap_err(),
            RadrootsNip09DeletionProjectionError::TargetMissing
        );
        assert_eq!(
            project_nip09_deletion_request_parts(
                KIND_DELETION_REQUEST,
                &[vec!["e".to_string()]],
                "",
                1,
            )
            .unwrap_err(),
            RadrootsNip09DeletionProjectionError::EventTargetShape { tag_index: 0 }
        );
    }

    #[test]
    fn enforces_exact_error_precedence_before_target_semantics() {
        let oversized_content = "x".repeat(RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES + 1);
        let oversized_tags = vec![vec!["e".to_string()]; RADROOTS_NIP09_DELETION_TAG_MAX_COUNT + 1];
        assert!(matches!(
            project_nip09_deletion_request_parts(1, &oversized_tags, &oversized_content, 1),
            Err(RadrootsNip09DeletionProjectionError::UnsupportedKind { actual: 1 })
        ));
        assert!(matches!(
            project_nip09_deletion_request_parts(
                KIND_DELETION_REQUEST,
                &oversized_tags,
                &oversized_content,
                1
            ),
            Err(RadrootsNip09DeletionProjectionError::ContentTooLarge { .. })
        ));
        assert!(matches!(
            project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &oversized_tags, "", 1),
            Err(RadrootsNip09DeletionProjectionError::TagCountExceeded { .. })
        ));

        let too_many_elements = vec![
            vec!["x".to_string(); RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT],
            vec!["e".to_string()],
        ];
        assert_eq!(
            project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &too_many_elements, "", 1)
                .unwrap_err(),
            RadrootsNip09DeletionProjectionError::TagElementCountExceeded {
                max: RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT,
                actual: RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT + 1,
            }
        );

        let oversized_element = vec![
            vec![
                "x".to_string(),
                "x".repeat(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES + 1),
            ],
            vec!["e".to_string()],
        ];
        assert_eq!(
            project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &oversized_element, "", 1)
                .unwrap_err(),
            RadrootsNip09DeletionProjectionError::TagElementTooLarge {
                max: RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES,
                actual: RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES + 1,
                tag_index: 0,
                element_index: 1,
            }
        );
    }

    #[test]
    fn accepts_exact_shared_resource_boundaries() {
        let mut tag_count = valid_event_tags();
        tag_count.extend(
            (tag_count.len()..RADROOTS_NIP09_DELETION_TAG_MAX_COUNT).map(|_| vec!["x".to_string()]),
        );
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tag_count, "", 1)
            .expect("exact tag-count boundary");

        let mut element_count = valid_event_tags();
        element_count.push(vec!["x".to_string(); 8]);
        element_count.extend(
            (element_count.len()..RADROOTS_NIP09_DELETION_TAG_MAX_COUNT)
                .map(|_| vec!["x".to_string(); 4]),
        );
        assert_eq!(
            element_count.iter().map(Vec::len).sum::<usize>(),
            RADROOTS_NIP09_DELETION_TAG_TOTAL_ELEMENT_MAX_COUNT
        );
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &element_count, "", 1)
            .expect("exact tag-element boundary");

        let exact_element = vec![
            vec!["e".to_string(), h('a')],
            vec![
                "x".to_string(),
                "x".repeat(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES),
            ],
        ];
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &exact_element, "", 1)
            .expect("exact individual tag-element boundary");

        let mut tag_bytes = valid_event_tags();
        tag_bytes.extend((0..31).map(|_| {
            vec![
                String::new(),
                "x".repeat(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES),
            ]
        }));
        tag_bytes.push(vec![String::new(), "x".repeat(4_029)]);
        assert_eq!(
            tag_bytes
                .iter()
                .flat_map(|tag| tag.iter())
                .map(String::len)
                .sum::<usize>(),
            RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES
        );
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tag_bytes, "", 1)
            .expect("exact aggregate tag-byte boundary");

        project_nip09_deletion_request_parts(
            KIND_DELETION_REQUEST,
            &valid_event_tags(),
            &"x".repeat(RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES),
            1,
        )
        .expect("exact content boundary");
    }

    #[test]
    fn enforces_aggregate_tag_and_compact_wire_budgets_before_targets() {
        let mut tag_bytes = vec![
            vec![
                String::new(),
                "x".repeat(RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES),
            ];
            RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES
                / RADROOTS_NIP09_DELETION_TAG_ELEMENT_MAX_BYTES
        ];
        tag_bytes.push(vec!["e".to_string()]);
        assert_eq!(
            project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tag_bytes, "", 1)
                .unwrap_err(),
            RadrootsNip09DeletionProjectionError::TagBytesExceeded {
                max: RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES,
                actual: RADROOTS_NIP09_DELETION_TAG_TOTAL_MAX_BYTES + 1,
            }
        );

        let escaped_content = "\u{0001}".repeat(50_000);
        assert!(matches!(
            project_nip09_deletion_request_parts(
                KIND_DELETION_REQUEST,
                &[vec!["e".to_string()]],
                &escaped_content,
                1
            ),
            Err(RadrootsNip09DeletionProjectionError::EventWireTooLarge { .. })
        ));
    }

    #[test]
    fn error_codes_and_messages_are_stable() {
        let errors = [
            RadrootsNip09DeletionProjectionError::UnsupportedKind { actual: 1 },
            RadrootsNip09DeletionProjectionError::ContentTooLarge { max: 1, actual: 2 },
            RadrootsNip09DeletionProjectionError::TagCountExceeded { max: 1, actual: 2 },
            RadrootsNip09DeletionProjectionError::TagElementCountExceeded { max: 1, actual: 2 },
            RadrootsNip09DeletionProjectionError::TagElementTooLarge {
                max: 1,
                actual: 2,
                tag_index: 3,
                element_index: 4,
            },
            RadrootsNip09DeletionProjectionError::TagBytesExceeded { max: 1, actual: 2 },
            RadrootsNip09DeletionProjectionError::EventWireTooLarge { max: 1, actual: 2 },
            RadrootsNip09DeletionProjectionError::EventTargetShape { tag_index: 0 },
            RadrootsNip09DeletionProjectionError::EventTargetInvalid {
                tag_index: 0,
                error: RadrootsIdParseError::InvalidFormat,
            },
            RadrootsNip09DeletionProjectionError::AddressTargetShape { tag_index: 0 },
            RadrootsNip09DeletionProjectionError::AddressTargetInvalid {
                tag_index: 0,
                error: RadrootsNip01CoordinateParseError::InvalidFormat,
            },
            RadrootsNip09DeletionProjectionError::TargetMissing,
        ];
        let expected = [
            "unsupported_kind",
            "deletion_content_too_large",
            "deletion_tag_count_exceeded",
            "deletion_tag_element_count_exceeded",
            "deletion_tag_element_too_large",
            "deletion_tag_bytes_exceeded",
            "deletion_event_wire_too_large",
            "deletion_event_target_shape",
            "deletion_event_target_invalid",
            "deletion_address_target_shape",
            "deletion_address_target_invalid",
            "deletion_target_missing",
        ];
        for (error, expected) in errors.into_iter().zip(expected) {
            assert_eq!(error.code(), expected);
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn verified_projection_api_cannot_accept_an_unverified_envelope() {
        fn project(
            event: &RadrootsSignatureVerifiedEvent,
        ) -> Result<RadrootsInboundNip09DeletionProjection, RadrootsNip09DeletionProjectionError>
        {
            project_verified_nip09_deletion_request_event(event)
        }
        let _ = project;
    }

    #[test]
    fn wire_estimator_uses_actual_created_at_width() {
        let tags = valid_event_tags();
        let mut content = "\u{0001}".repeat(43_600);
        while project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, &content, u64::MAX)
            .is_ok()
        {
            content.push('\u{0001}');
        }
        assert!(matches!(
            project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, &content, u64::MAX),
            Err(RadrootsNip09DeletionProjectionError::EventWireTooLarge { .. })
        ));
        project_nip09_deletion_request_parts(KIND_DELETION_REQUEST, &tags, &content, 1)
            .expect("short created_at width remains within the wire budget");
    }
}

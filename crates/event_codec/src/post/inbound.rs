#[cfg(not(feature = "std"))]
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::fmt;
#[cfg(feature = "std")]
use std::collections::{BTreeMap, BTreeSet};

use radroots_event::{
    kinds::KIND_POST,
    post::{
        RADROOTS_ASK_MARKER_TAG_VALUE, RADROOTS_POST_ALT_MAX_BYTES,
        RADROOTS_POST_CONTENT_MAX_BYTES, RADROOTS_POST_IMETA_MAX_COUNT,
        RadrootsPostImageDimensions, post_image_media_type_is_valid, post_media_http_url_is_valid,
    },
};

use crate::verification::RadrootsSignatureVerifiedEvent;

const REQUIRED_IMETA_FIELDS: [&str; 6] = ["url", "x", "m", "dim", "size", "alt"];

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsPostDiagnostic {
    AskMarkerShape,
    ImetaCountExceeded,
    ImetaFieldInvalid,
    ImetaUrlMissing,
    ImetaMetadataMissing,
    ImetaSingletonDuplicate,
    ImetaUrlMissingFromContent,
    DuplicateImetaUrl,
    ImetaUrlInvalid,
    ImetaHashInvalid,
    ImetaMimeInvalid,
    ImetaDimensionsInvalid,
    ImetaSizeInvalid,
    ImetaAltInvalid,
    ImetaAltTooLarge,
    ImetaFallbackUrlInvalid,
}

impl RadrootsPostDiagnostic {
    pub const fn code(self) -> &'static str {
        match self {
            Self::AskMarkerShape => "ask_marker_shape",
            Self::ImetaCountExceeded => "imeta_count_exceeded",
            Self::ImetaFieldInvalid => "imeta_field_invalid",
            Self::ImetaUrlMissing => "imeta_url_missing",
            Self::ImetaMetadataMissing => "imeta_metadata_missing",
            Self::ImetaSingletonDuplicate => "imeta_singleton_duplicate",
            Self::ImetaUrlMissingFromContent => "imeta_url_missing_from_content",
            Self::DuplicateImetaUrl => "duplicate_imeta_url",
            Self::ImetaUrlInvalid => "imeta_url_invalid",
            Self::ImetaHashInvalid => "imeta_hash_invalid",
            Self::ImetaMimeInvalid => "imeta_mime_invalid",
            Self::ImetaDimensionsInvalid => "imeta_dimensions_invalid",
            Self::ImetaSizeInvalid => "imeta_size_invalid",
            Self::ImetaAltInvalid => "imeta_alt_invalid",
            Self::ImetaAltTooLarge => "imeta_alt_too_large",
            Self::ImetaFallbackUrlInvalid => "imeta_fallback_url_invalid",
        }
    }
}

impl fmt::Display for RadrootsPostDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// Product projection for a verified kind-1 event.
///
/// ThreadExcluded is an exclusion classification only; strict NIP-10 parsing
/// remains a separate contract. Update, PhotoUpdate, and Ask are root-card
/// profiles.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsPostClassification {
    ThreadExcluded,
    Update,
    PhotoUpdate,
    Ask,
}

impl RadrootsPostClassification {
    pub const fn contract_id(self) -> &'static str {
        match self {
            Self::ThreadExcluded => "radroots.social.post.v1",
            Self::Update => "radroots.social.update.v1",
            Self::PhotoUpdate => "radroots.social.photo_update.v1",
            Self::Ask => "radroots.social.ask.v1",
        }
    }

    pub const fn is_root_card(self) -> bool {
        !matches!(self, Self::ThreadExcluded)
    }
}

/// One raw inbound NIP-92 `imeta` projection.
///
/// URLs and metadata remain unverified even when the entry qualifies for
/// PhotoUpdate classification. Classification is structural and performs no
/// network request or blob verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundPostImeta {
    raw_fields: Vec<String>,
    url: Option<String>,
    sha256: Option<String>,
    media_type: Option<String>,
    dimensions: Option<RadrootsPostImageDimensions>,
    size: Option<u64>,
    alt: Option<String>,
    fallbacks: Vec<String>,
    unknown_fields: Vec<String>,
    diagnostics: Vec<RadrootsPostDiagnostic>,
}

impl RadrootsInboundPostImeta {
    pub fn raw_fields(&self) -> &[String] {
        &self.raw_fields
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn sha256(&self) -> Option<&str> {
        self.sha256.as_deref()
    }

    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    pub const fn dimensions(&self) -> Option<RadrootsPostImageDimensions> {
        self.dimensions
    }

    pub const fn size(&self) -> Option<u64> {
        self.size
    }

    pub fn alt(&self) -> Option<&str> {
        self.alt.as_deref()
    }

    pub fn fallbacks(&self) -> &[String] {
        &self.fallbacks
    }

    pub fn unknown_fields(&self) -> &[String] {
        &self.unknown_fields
    }

    pub fn diagnostics(&self) -> &[RadrootsPostDiagnostic] {
        &self.diagnostics
    }

    pub fn qualifies_photo(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Tolerant, ordered product projection of one verified kind-1 event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundPostProjection {
    classification: RadrootsPostClassification,
    ask_marker: Option<Vec<String>>,
    imeta: Vec<RadrootsInboundPostImeta>,
    diagnostics: Vec<RadrootsPostDiagnostic>,
}

impl RadrootsInboundPostProjection {
    pub const fn classification(&self) -> RadrootsPostClassification {
        self.classification
    }

    pub fn ask_marker(&self) -> Option<&[String]> {
        self.ask_marker.as_deref()
    }

    pub fn imeta(&self) -> &[RadrootsInboundPostImeta] {
        &self.imeta
    }

    pub fn diagnostics(&self) -> &[RadrootsPostDiagnostic] {
        &self.diagnostics
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsPostProjectionError {
    InvalidKind { expected: u32, actual: u32 },
    ContentTooLarge { max: usize, actual: usize },
    AskMarkerCount,
}

impl RadrootsPostProjectionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidKind { .. } => "invalid_kind",
            Self::ContentTooLarge { .. } => "post_content_too_large",
            Self::AskMarkerCount => "ask_marker_count",
        }
    }
}

impl fmt::Display for RadrootsPostProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKind { expected, actual } => {
                write!(
                    formatter,
                    "post event kind must be {expected}, got {actual}"
                )
            }
            Self::ContentTooLarge { max, actual } => {
                write!(formatter, "post content is {actual} bytes; max is {max}")
            }
            Self::AskMarkerCount => {
                formatter.write_str("post event must not contain multiple normalized Ask markers")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsPostProjectionError {}

/// Projects a signature-and-id verified kind-1 event without admitting it to a
/// relay or claiming media verification.
///
/// Any `e` tag excludes the event before Ask or media inspection. This function
/// does not claim that the event is a valid NIP-10 reply; that belongs to the
/// dedicated reply contract.
pub fn project_verified_post_event(
    verified_event: &RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsInboundPostProjection, RadrootsPostProjectionError> {
    let event = verified_event.event();
    project_inbound_post_parts(event.kind_u32(), &event.tags_as_vec(), event.content())
}

pub(crate) fn project_inbound_post_parts(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsInboundPostProjection, RadrootsPostProjectionError> {
    if kind != KIND_POST {
        return Err(RadrootsPostProjectionError::InvalidKind {
            expected: KIND_POST,
            actual: kind,
        });
    }
    if content.len() > RADROOTS_POST_CONTENT_MAX_BYTES {
        return Err(RadrootsPostProjectionError::ContentTooLarge {
            max: RADROOTS_POST_CONTENT_MAX_BYTES,
            actual: content.len(),
        });
    }
    if tags
        .iter()
        .any(|tag| tag.first().is_some_and(|key| key == "e"))
    {
        return Ok(RadrootsInboundPostProjection {
            classification: RadrootsPostClassification::ThreadExcluded,
            ask_marker: None,
            imeta: Vec::new(),
            diagnostics: Vec::new(),
        });
    }

    let (ask_marker, marker_diagnostics) = project_ask_marker(tags)?;
    let imeta_tags = tags
        .iter()
        .filter(|tag| tag.first().is_some_and(|key| key == "imeta"))
        .collect::<Vec<_>>();
    let mut diagnostics = marker_diagnostics;
    if imeta_tags.len() > RADROOTS_POST_IMETA_MAX_COUNT {
        diagnostics.push(RadrootsPostDiagnostic::ImetaCountExceeded);
    }
    let imeta = project_imeta(imeta_tags, content);
    diagnostics.extend(
        imeta
            .iter()
            .flat_map(|item| item.diagnostics.iter().copied()),
    );
    let classification = if ask_marker.is_some() {
        RadrootsPostClassification::Ask
    } else if !imeta.is_empty() && diagnostics.is_empty() {
        RadrootsPostClassification::PhotoUpdate
    } else {
        RadrootsPostClassification::Update
    };
    Ok(RadrootsInboundPostProjection {
        classification,
        ask_marker,
        imeta,
        diagnostics,
    })
}

fn project_ask_marker(
    tags: &[Vec<String>],
) -> Result<(Option<Vec<String>>, Vec<RadrootsPostDiagnostic>), RadrootsPostProjectionError> {
    let candidates = tags
        .iter()
        .filter(|tag| {
            tag.first().is_some_and(|key| key == "t")
                && tag.get(1).is_some_and(|value| normalized_ask_marker(value))
        })
        .collect::<Vec<_>>();
    if candidates.len() > 1 {
        return Err(RadrootsPostProjectionError::AskMarkerCount);
    }
    let Some(candidate) = candidates.first() else {
        return Ok((None, Vec::new()));
    };
    if candidate.len() != 2 {
        return Ok((None, vec![RadrootsPostDiagnostic::AskMarkerShape]));
    }
    Ok((Some((*candidate).clone()), Vec::new()))
}

fn normalized_ask_marker(value: &str) -> bool {
    value
        .trim_matches(|character| {
            matches!(
                character,
                ' ' | '\t' | '\n' | '\r' | '\u{000b}' | '\u{000c}'
            )
        })
        .eq_ignore_ascii_case(RADROOTS_ASK_MARKER_TAG_VALUE)
}

fn project_imeta(tags: Vec<&Vec<String>>, content: &str) -> Vec<RadrootsInboundPostImeta> {
    let mut projections = Vec::with_capacity(tags.len());
    let mut seen_urls = BTreeSet::new();
    for tag in tags {
        let raw_fields = tag[1..].to_vec();
        let mut fields = BTreeMap::new();
        let mut fallbacks = Vec::new();
        let mut unknown_fields = Vec::new();
        let mut diagnostics = Vec::new();

        for raw_field in &raw_fields {
            let Some((key, value)) = raw_field.split_once(' ') else {
                diagnostics.push(RadrootsPostDiagnostic::ImetaFieldInvalid);
                continue;
            };
            if key.is_empty() || value.is_empty() {
                diagnostics.push(RadrootsPostDiagnostic::ImetaFieldInvalid);
            } else if key == "fallback" {
                fallbacks.push(value.to_string());
            } else if imeta_singleton_field(key) {
                if fields.contains_key(key) {
                    diagnostics.push(RadrootsPostDiagnostic::ImetaSingletonDuplicate);
                } else {
                    fields.insert(key.to_string(), value.to_string());
                }
            } else {
                unknown_fields.push(raw_field.clone());
            }
        }

        let url = fields.get("url").cloned();
        if url.is_none() {
            diagnostics.push(RadrootsPostDiagnostic::ImetaUrlMissing);
        } else if REQUIRED_IMETA_FIELDS
            .iter()
            .any(|required| !fields.contains_key(*required))
        {
            diagnostics.push(RadrootsPostDiagnostic::ImetaMetadataMissing);
        }
        if let Some(url) = &url {
            if !content.contains(url) {
                diagnostics.push(RadrootsPostDiagnostic::ImetaUrlMissingFromContent);
            }
            if !seen_urls.insert(url.clone()) {
                diagnostics.push(RadrootsPostDiagnostic::DuplicateImetaUrl);
            }
            if !post_media_http_url_is_valid(url) {
                diagnostics.push(RadrootsPostDiagnostic::ImetaUrlInvalid);
            }
        }

        let sha256 = fields.get("x").cloned();
        if sha256.as_deref().is_some_and(|value| !lower_hex_64(value)) {
            diagnostics.push(RadrootsPostDiagnostic::ImetaHashInvalid);
        }
        let media_type = fields.get("m").cloned();
        if media_type
            .as_deref()
            .is_some_and(|value| !post_image_media_type_is_valid(value))
        {
            diagnostics.push(RadrootsPostDiagnostic::ImetaMimeInvalid);
        }
        let dimensions = fields.get("dim").and_then(|value| {
            parse_dimensions(value).or_else(|| {
                diagnostics.push(RadrootsPostDiagnostic::ImetaDimensionsInvalid);
                None
            })
        });
        let size = fields.get("size").and_then(|value| {
            parse_nonzero_u64(value).or_else(|| {
                diagnostics.push(RadrootsPostDiagnostic::ImetaSizeInvalid);
                None
            })
        });
        let alt = fields.get("alt").cloned();
        if let Some(alt) = &alt {
            if alt.trim().is_empty() {
                diagnostics.push(RadrootsPostDiagnostic::ImetaAltInvalid);
            } else if alt.len() > RADROOTS_POST_ALT_MAX_BYTES {
                diagnostics.push(RadrootsPostDiagnostic::ImetaAltTooLarge);
            }
        }
        for fallback in &fallbacks {
            if !post_media_http_url_is_valid(fallback) {
                diagnostics.push(RadrootsPostDiagnostic::ImetaFallbackUrlInvalid);
            }
        }

        projections.push(RadrootsInboundPostImeta {
            raw_fields,
            url,
            sha256,
            media_type,
            dimensions,
            size,
            alt,
            fallbacks,
            unknown_fields,
            diagnostics,
        });
    }
    projections
}

fn imeta_singleton_field(value: &str) -> bool {
    matches!(
        value,
        "url"
            | "m"
            | "x"
            | "ox"
            | "size"
            | "dim"
            | "magnet"
            | "i"
            | "blurhash"
            | "thumb"
            | "image"
            | "summary"
            | "alt"
            | "service"
    )
}

fn lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn parse_dimensions(value: &str) -> Option<RadrootsPostImageDimensions> {
    let (width, height) = value.split_once('x')?;
    if !canonical_nonzero_decimal(width) || !canonical_nonzero_decimal(height) {
        return None;
    }
    RadrootsPostImageDimensions::new(width.parse().ok()?, height.parse().ok()?).ok()
}

fn parse_nonzero_u64(value: &str) -> Option<u64> {
    canonical_nonzero_decimal(value)
        .then(|| value.parse().ok())
        .flatten()
}

fn canonical_nonzero_decimal(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(|byte| matches!(byte, b'1'..=b'9'))
        && value.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::post::RadrootsAuthoredUpdate;

    #[test]
    fn thread_exclusion_precedes_ask_and_media_projection() {
        let projection = project_inbound_post_parts(
            KIND_POST,
            &[
                vec!["e".to_string(), "parent".to_string()],
                vec!["t".to_string(), "radroots-ask".to_string()],
                vec!["imeta".to_string(), "x malformed".to_string()],
            ],
            "reply",
        )
        .unwrap();

        assert_eq!(
            projection.classification(),
            RadrootsPostClassification::ThreadExcluded
        );
        assert!(projection.imeta().is_empty());
        assert!(projection.diagnostics().is_empty());
    }

    #[test]
    fn every_empty_or_malformed_event_reference_is_thread_excluded() {
        for event_reference in [
            vec!["e".to_string()],
            vec!["e".to_string(), String::new()],
            vec!["e".to_string(), "not-an-event-id".to_string()],
        ] {
            let projection =
                project_inbound_post_parts(KIND_POST, &[event_reference], "candidate").unwrap();

            assert_eq!(
                projection.classification(),
                RadrootsPostClassification::ThreadExcluded
            );
            assert!(!projection.classification().is_root_card());
        }
    }

    #[test]
    fn normalized_ask_precedes_malformed_media_and_retains_diagnostics() {
        let projection = project_inbound_post_parts(
            KIND_POST,
            &[
                vec!["t".to_string(), " RADROOTS-ASK ".to_string()],
                vec![
                    "imeta".to_string(),
                    "url https://cdn.example/leaf.webp".to_string(),
                    "x malformed".to_string(),
                ],
            ],
            "Question https://cdn.example/leaf.webp",
        )
        .unwrap();

        assert_eq!(projection.classification(), RadrootsPostClassification::Ask);
        assert_eq!(
            diagnostic_codes(projection.diagnostics()),
            ["imeta_metadata_missing", "imeta_hash_invalid"]
        );
        assert_eq!(
            projection.ask_marker().unwrap(),
            ["t".to_string(), " RADROOTS-ASK ".to_string()]
        );
    }

    #[test]
    fn photo_preserves_repeatable_fallbacks_and_ordered_unknown_fields() {
        let projection = project_inbound_post_parts(
            KIND_POST,
            &[qualifying_imeta(vec![
                "fallback https://cache-one.example/harvest.webp",
                "x-farm cultivar-strawberry",
                "fallback https://cache-two.example/harvest.webp",
                "future-field retained value",
            ])],
            "Harvest https://cdn.example/harvest.webp",
        )
        .unwrap();
        let media = &projection.imeta()[0];

        assert_eq!(
            projection.classification(),
            RadrootsPostClassification::PhotoUpdate
        );
        assert_eq!(
            media.fallbacks(),
            [
                "https://cache-one.example/harvest.webp".to_string(),
                "https://cache-two.example/harvest.webp".to_string(),
            ]
        );
        assert_eq!(
            media.unknown_fields(),
            [
                "x-farm cultivar-strawberry".to_string(),
                "future-field retained value".to_string(),
            ]
        );
        assert!(media.qualifies_photo());
    }

    #[test]
    fn duplicate_singletons_and_mixed_imeta_downgrade_to_update() {
        let mut duplicate = qualifying_imeta(Vec::new());
        duplicate.insert(
            3,
            "x bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        );
        let malformed = vec![
            "imeta".to_string(),
            "url https://cdn.example/leaf.webp".to_string(),
            "x malformed".to_string(),
        ];
        let projection = project_inbound_post_parts(
            KIND_POST,
            &[duplicate, malformed],
            "Harvest https://cdn.example/harvest.webp and https://cdn.example/leaf.webp",
        )
        .unwrap();

        assert_eq!(
            projection.classification(),
            RadrootsPostClassification::Update
        );
        assert_eq!(
            diagnostic_codes(projection.diagnostics()),
            [
                "imeta_singleton_duplicate",
                "imeta_metadata_missing",
                "imeta_hash_invalid",
            ]
        );
    }

    #[test]
    fn duplicate_urls_and_excess_imeta_downgrade_to_update() {
        let duplicate = qualifying_imeta(Vec::new());
        let projection = project_inbound_post_parts(
            KIND_POST,
            &[duplicate.clone(), duplicate],
            "Harvest https://cdn.example/harvest.webp",
        )
        .unwrap();
        assert_eq!(
            projection.classification(),
            RadrootsPostClassification::Update
        );
        assert_eq!(
            diagnostic_codes(projection.diagnostics()),
            ["duplicate_imeta_url"]
        );

        let imeta = qualifying_imeta(Vec::new());
        let tags = vec![imeta; RADROOTS_POST_IMETA_MAX_COUNT + 1];
        let projection = project_inbound_post_parts(
            KIND_POST,
            &tags,
            "Harvest https://cdn.example/harvest.webp",
        )
        .unwrap();
        assert_eq!(
            projection.classification(),
            RadrootsPostClassification::Update
        );
        assert_eq!(
            projection.diagnostics().first(),
            Some(&RadrootsPostDiagnostic::ImetaCountExceeded)
        );
    }

    #[test]
    fn invalid_imeta_fields_report_stable_ordered_diagnostics() {
        let projection = project_inbound_post_parts(
            KIND_POST,
            &[vec![
                "imeta".to_string(),
                "url ftp://cdn.example/harvest.webp".to_string(),
                "x AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
                "m image/webp;quality=90".to_string(),
                "dim 01x0".to_string(),
                "size 0".to_string(),
                "alt \t".to_string(),
                "fallback file:///harvest.webp".to_string(),
            ]],
            "Harvest ftp://cdn.example/harvest.webp",
        )
        .unwrap();

        assert_eq!(
            projection.classification(),
            RadrootsPostClassification::Update
        );
        assert_eq!(
            diagnostic_codes(projection.diagnostics()),
            [
                "imeta_url_invalid",
                "imeta_hash_invalid",
                "imeta_mime_invalid",
                "imeta_dimensions_invalid",
                "imeta_size_invalid",
                "imeta_alt_invalid",
                "imeta_fallback_url_invalid",
            ]
        );
    }

    #[test]
    fn malformed_fields_and_oversized_alt_never_qualify_photo() {
        let oversized_alt = "a".repeat(RADROOTS_POST_ALT_MAX_BYTES + 1);
        let mut tag = qualifying_imeta(Vec::new());
        tag.push("malformed".to_string());
        tag[6] = format!("alt {oversized_alt}");
        let projection = project_inbound_post_parts(
            KIND_POST,
            &[tag],
            "Harvest https://cdn.example/harvest.webp",
        )
        .unwrap();

        assert_eq!(
            projection.classification(),
            RadrootsPostClassification::Update
        );
        assert_eq!(
            diagnostic_codes(projection.diagnostics()),
            ["imeta_field_invalid", "imeta_alt_too_large"]
        );
    }

    #[test]
    fn malformed_and_duplicate_normalized_ask_markers_are_distinct() {
        let malformed = project_inbound_post_parts(
            KIND_POST,
            &[vec![
                "t".to_string(),
                "RADROOTS-ASK".to_string(),
                "extra".to_string(),
            ]],
            "Question",
        )
        .unwrap();
        assert_eq!(
            malformed.classification(),
            RadrootsPostClassification::Update
        );
        assert_eq!(
            diagnostic_codes(malformed.diagnostics()),
            ["ask_marker_shape"]
        );

        let error = project_inbound_post_parts(
            KIND_POST,
            &[
                vec!["t".to_string(), "radroots-ask".to_string()],
                vec!["t".to_string(), " RADROOTS-ASK ".to_string()],
            ],
            "Question",
        )
        .unwrap_err();
        assert_eq!(error.code(), "ask_marker_count");
    }

    #[test]
    fn empty_inbound_root_is_update_without_becoming_valid_authored_content() {
        let projection = project_inbound_post_parts(KIND_POST, &[], "\t").unwrap();
        assert_eq!(
            projection.classification(),
            RadrootsPostClassification::Update
        );
        assert!(RadrootsAuthoredUpdate::new("\t").is_err());
    }

    #[test]
    fn projection_rejects_wrong_kind_and_oversized_content() {
        assert_eq!(
            project_inbound_post_parts(20, &[], "photo")
                .unwrap_err()
                .code(),
            "invalid_kind"
        );
        let oversized = "x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES + 1);
        assert_eq!(
            project_inbound_post_parts(KIND_POST, &[], &oversized)
                .unwrap_err()
                .code(),
            "post_content_too_large"
        );
    }

    fn qualifying_imeta(extra: Vec<&str>) -> Vec<String> {
        let mut tag = vec![
            "imeta".to_string(),
            "url https://cdn.example/harvest.webp".to_string(),
            "x aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            "m image/webp".to_string(),
            "dim 1200x900".to_string(),
            "size 12345".to_string(),
            "alt Harvest".to_string(),
        ];
        tag.extend(extra.into_iter().map(str::to_string));
        tag
    }

    fn diagnostic_codes(diagnostics: &[RadrootsPostDiagnostic]) -> Vec<&'static str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect()
    }
}

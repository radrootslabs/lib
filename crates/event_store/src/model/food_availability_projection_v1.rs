use super::{
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1, RadrootsEventStoreSourceGeneration,
};
use crate::RadrootsEventStoreError;
use crate::error::require_invariant;
use radroots_blossom::Sha256;
use radroots_event::{
    food::availability::{
        FoodAvailabilityStatus, FoodContent, FoodIdentifier, FoodImageDimensions, FoodPrice,
        FoodPublishedAt, FoodQuantity, FoodText, RADROOTS_FOOD_IMAGE_MAX_COUNT,
        food_media_blossom_digest,
    },
    id::EventId,
};
use radroots_event_codec::decode::food_availability::{
    RadrootsFoodAvailabilityImageDiagnostic, RadrootsInboundFoodAvailabilityImage,
    RadrootsInboundFoodAvailabilityProjection,
};
use radroots_identity::PublicKey;

pub const RADROOTS_FOOD_AVAILABILITY_PROJECTION_VERSION_V1: u32 = 1;
pub const RADROOTS_FOOD_AVAILABILITY_PROJECTION_APPLY_PAGE_LIMIT_V1: u32 =
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1;
pub const RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_BYTES_V1: usize = 256;
pub const RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_TERMS_V1: usize = 16;

/// A bounded literal-term query for the FoodAvailability FTS5 projection.
///
/// Terms are combined with `AND`. Every term is emitted as an escaped FTS5
/// string literal, so caller text cannot introduce columns, operators, prefix
/// matching, or grouping into the generated expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsFoodAvailabilitySearchQueryV1 {
    canonical_query: String,
    terms: Vec<String>,
    fts5_match_expression: String,
}

impl RadrootsFoodAvailabilitySearchQueryV1 {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsEventStoreError> {
        let value = value.as_ref();
        if value.len() > RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_BYTES_V1 {
            return Err(RadrootsEventStoreError::FoodAvailabilitySearchTooLarge {
                max: RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_BYTES_V1,
                actual: value.len(),
            });
        }

        let terms = value
            .split(|character: char| character.is_whitespace() || character.is_control())
            .filter(|term| !term.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if terms.is_empty() {
            return Err(RadrootsEventStoreError::FoodAvailabilitySearchEmpty);
        }
        if terms.len() > RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_TERMS_V1 {
            return Err(
                RadrootsEventStoreError::FoodAvailabilitySearchTooManyTerms {
                    max: RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_TERMS_V1,
                    actual: terms.len(),
                },
            );
        }

        let canonical_query = terms.join(" ");
        let mut fts5_match_expression = String::with_capacity(
            canonical_query
                .len()
                .saturating_add(terms.len().saturating_mul(8)),
        );
        for (index, term) in terms.iter().enumerate() {
            if index > 0 {
                fts5_match_expression.push_str(" AND ");
            }
            push_fts5_string_literal(&mut fts5_match_expression, term);
        }

        Ok(Self {
            canonical_query,
            terms,
            fts5_match_expression,
        })
    }

    pub fn as_str(&self) -> &str {
        self.canonical_query.as_str()
    }

    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    pub(crate) fn fts5_match_expression(&self) -> &str {
        self.fts5_match_expression.as_str()
    }
}

impl core::fmt::Display for RadrootsFoodAvailabilitySearchQueryV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<&str> for RadrootsFoodAvailabilitySearchQueryV1 {
    type Error = RadrootsEventStoreError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

fn push_fts5_string_literal(output: &mut String, term: &str) {
    output.push('"');
    for character in term.chars() {
        if character == '"' {
            output.push('"');
        }
        output.push(character);
    }
    output.push('"');
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RadrootsFoodAvailabilityStatusFilterV1 {
    #[default]
    Any,
    Active,
    Sold,
}

impl RadrootsFoodAvailabilityStatusFilterV1 {
    pub const fn status(self) -> Option<FoodAvailabilityStatus> {
        match self {
            Self::Any => None,
            Self::Active => Some(FoodAvailabilityStatus::Active),
            Self::Sold => Some(FoodAvailabilityStatus::Sold),
        }
    }

    pub(crate) const fn storage_value(self) -> Option<&'static str> {
        match self.status() {
            None => None,
            Some(status) => Some(status.as_str()),
        }
    }
}

impl From<FoodAvailabilityStatus> for RadrootsFoodAvailabilityStatusFilterV1 {
    fn from(value: FoodAvailabilityStatus) -> Self {
        match value {
            FoodAvailabilityStatus::Active => Self::Active,
            FoodAvailabilityStatus::Sold => Self::Sold,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredFoodAvailabilityImageV1 {
    image_index: u32,
    raw_tag: Vec<String>,
    url: Option<String>,
    dimensions: Option<FoodImageDimensions>,
    blossom_sha256: Option<Sha256>,
    diagnostics: Vec<RadrootsFoodAvailabilityImageDiagnostic>,
}

impl RadrootsStoredFoodAvailabilityImageV1 {
    fn from_projection(
        image_index: usize,
        image: &RadrootsInboundFoodAvailabilityImage,
    ) -> Result<Self, RadrootsEventStoreError> {
        let image_index = u32::try_from(image_index).map_err(|_| {
            food_projection_drift("projected image index exceeds the u32 storage range")
        })?;
        let url = image.url().map(str::to_owned);
        let blossom_sha256 = url.as_deref().and_then(food_media_blossom_digest);
        Ok(Self {
            image_index,
            raw_tag: image.raw_tag().to_vec(),
            url,
            dimensions: image.dimensions(),
            blossom_sha256,
            diagnostics: image.diagnostics().to_vec(),
        })
    }

    pub const fn image_index(&self) -> u32 {
        self.image_index
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub const fn dimensions(&self) -> Option<FoodImageDimensions> {
        self.dimensions
    }

    pub const fn blossom_sha256(&self) -> Option<Sha256> {
        self.blossom_sha256
    }

    pub fn diagnostics(&self) -> &[RadrootsFoodAvailabilityImageDiagnostic] {
        &self.diagnostics
    }

    /// Reports whether tolerant inbound projection produced no diagnostics.
    ///
    /// This does not establish Blossom hosting or byte verification. Callers
    /// must require `blossom_sha256()` plus runtime upload/retrieval evidence
    /// for Blossom-specific product behavior.
    pub fn qualifies(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredFoodAvailabilityV1 {
    source_generation: RadrootsEventStoreSourceGeneration,
    pubkey: PublicKey,
    identifier: FoodIdentifier,
    event_id: EventId,
    event_seq: i64,
    created_at: u64,
    content: FoodContent,
    title: FoodText,
    summary: FoodText,
    published_at: FoodPublishedAt,
    location: FoodText,
    price: FoodPrice,
    quantity: Option<FoodQuantity>,
    status: FoodAvailabilityStatus,
    source_transition_seq: i64,
    diagnostics: Vec<RadrootsFoodAvailabilityImageDiagnostic>,
    images: Vec<RadrootsStoredFoodAvailabilityImageV1>,
}

impl RadrootsStoredFoodAvailabilityV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_projection(
        source_generation: RadrootsEventStoreSourceGeneration,
        pubkey: PublicKey,
        event_id: EventId,
        event_seq: i64,
        created_at: u64,
        source_transition_seq: i64,
        projection: &RadrootsInboundFoodAvailabilityProjection,
    ) -> Result<Self, RadrootsEventStoreError> {
        require_invariant(event_seq > 0, || {
            food_projection_drift(format!(
                "event sequence must be positive, found {event_seq}"
            ))
        })?;
        require_invariant(source_transition_seq > 0, || {
            food_projection_drift(format!(
                "source transition sequence must be positive, found {source_transition_seq}"
            ))
        })?;
        projection
            .published_at()
            .validate_created_at(created_at)
            .map_err(|error| food_projection_drift(error.to_string()))?;
        if projection
            .quantity()
            .is_some_and(|quantity| quantity.unit() != projection.price().unit())
        {
            return Err(food_projection_drift(
                "quantity unit does not match the price unit",
            ));
        }
        require_invariant(
            projection.images().len() <= RADROOTS_FOOD_IMAGE_MAX_COUNT,
            || {
                food_projection_drift(format!(
                    "bounded projection has {} images; maximum is {RADROOTS_FOOD_IMAGE_MAX_COUNT}",
                    projection.images().len()
                ))
            },
        )?;

        let images = projection
            .images()
            .iter()
            .enumerate()
            .map(|(index, image)| {
                RadrootsStoredFoodAvailabilityImageV1::from_projection(index, image)
            })
            .collect::<Result<Vec<_>, _>>()?;
        validate_projection_diagnostics(projection.diagnostics(), &images)?;

        Ok(Self {
            source_generation,
            pubkey,
            identifier: projection.identifier().clone(),
            event_id,
            event_seq,
            created_at,
            content: projection.content().clone(),
            title: projection.title().clone(),
            summary: projection.summary().clone(),
            published_at: projection.published_at(),
            location: projection.location().clone(),
            price: projection.price().clone(),
            quantity: projection.quantity().cloned(),
            status: projection.status(),
            source_transition_seq,
            diagnostics: projection.diagnostics().to_vec(),
            images,
        })
    }

    pub const fn source_generation(&self) -> RadrootsEventStoreSourceGeneration {
        self.source_generation
    }

    pub const fn pubkey(&self) -> &PublicKey {
        &self.pubkey
    }

    pub const fn identifier(&self) -> &FoodIdentifier {
        &self.identifier
    }

    pub const fn d_tag(&self) -> &FoodIdentifier {
        self.identifier()
    }

    pub const fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub const fn event_seq(&self) -> i64 {
        self.event_seq
    }

    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    pub const fn content(&self) -> &FoodContent {
        &self.content
    }

    pub const fn title(&self) -> &FoodText {
        &self.title
    }

    pub const fn summary(&self) -> &FoodText {
        &self.summary
    }

    pub const fn published_at(&self) -> FoodPublishedAt {
        self.published_at
    }

    pub const fn location(&self) -> &FoodText {
        &self.location
    }

    pub const fn price(&self) -> &FoodPrice {
        &self.price
    }

    pub const fn quantity(&self) -> Option<&FoodQuantity> {
        self.quantity.as_ref()
    }

    pub const fn status(&self) -> FoodAvailabilityStatus {
        self.status
    }

    pub const fn source_transition_seq(&self) -> i64 {
        self.source_transition_seq
    }

    pub fn diagnostics(&self) -> &[RadrootsFoodAvailabilityImageDiagnostic] {
        &self.diagnostics
    }

    pub fn images(&self) -> &[RadrootsStoredFoodAvailabilityImageV1] {
        &self.images
    }
}

fn validate_projection_diagnostics(
    diagnostics: &[RadrootsFoodAvailabilityImageDiagnostic],
    images: &[RadrootsStoredFoodAvailabilityImageV1],
) -> Result<(), RadrootsEventStoreError> {
    if images.iter().any(|image| {
        image
            .diagnostics()
            .contains(&RadrootsFoodAvailabilityImageDiagnostic::CountExceeded)
    }) {
        return Err(food_projection_drift(
            "image-level diagnostics contain the projection-wide count diagnostic",
        ));
    }

    let count_exceeded =
        diagnostics.first() == Some(&RadrootsFoodAvailabilityImageDiagnostic::CountExceeded);
    if count_exceeded && images.len() != RADROOTS_FOOD_IMAGE_MAX_COUNT {
        return Err(food_projection_drift(format!(
            "count-exceeded projection retained {} images instead of {RADROOTS_FOOD_IMAGE_MAX_COUNT}",
            images.len()
        )));
    }
    let mut expected = Vec::new();
    if count_exceeded {
        expected.push(RadrootsFoodAvailabilityImageDiagnostic::CountExceeded);
    }
    for image in images {
        expected.extend_from_slice(image.diagnostics());
    }
    if diagnostics != expected {
        return Err(food_projection_drift(
            "projection diagnostics do not match the ordered bounded image diagnostics",
        ));
    }
    Ok(())
}

fn food_projection_drift(reason: impl Into<String>) -> RadrootsEventStoreError {
    RadrootsEventStoreError::FoodAvailabilityProjectionDrift {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_is_bounded_canonical_and_literal() {
        let query =
            RadrootsFoodAvailabilitySearchQueryV1::parse("  fresh\ncarrots OR title:beets* a\"b  ")
                .expect("bounded query");

        assert_eq!(query.as_str(), "fresh carrots OR title:beets* a\"b");
        assert_eq!(
            query.terms(),
            &["fresh", "carrots", "OR", "title:beets*", "a\"b"]
        );
        assert_eq!(
            query.fts5_match_expression(),
            "\"fresh\" AND \"carrots\" AND \"OR\" AND \"title:beets*\" AND \"a\"\"b\""
        );
    }

    #[test]
    fn search_query_rejects_empty_oversized_and_excessive_terms() {
        assert!(matches!(
            RadrootsFoodAvailabilitySearchQueryV1::parse(" \n\t\0 "),
            Err(RadrootsEventStoreError::FoodAvailabilitySearchEmpty)
        ));

        let oversized = "a".repeat(RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_BYTES_V1 + 1);
        assert!(matches!(
            RadrootsFoodAvailabilitySearchQueryV1::parse(&oversized),
            Err(RadrootsEventStoreError::FoodAvailabilitySearchTooLarge {
                max: RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_BYTES_V1,
                actual,
            }) if actual == oversized.len()
        ));

        let excessive = (0..=RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_TERMS_V1)
            .map(|index| format!("term{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(matches!(
            RadrootsFoodAvailabilitySearchQueryV1::parse(&excessive),
            Err(RadrootsEventStoreError::FoodAvailabilitySearchTooManyTerms {
                max: RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_TERMS_V1,
                actual,
            }) if actual == RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_TERMS_V1 + 1
        ));
    }

    #[test]
    fn status_filter_exposes_only_governed_storage_values() {
        assert_eq!(
            RadrootsFoodAvailabilityStatusFilterV1::default().storage_value(),
            None
        );
        assert_eq!(
            RadrootsFoodAvailabilityStatusFilterV1::Active.storage_value(),
            Some("active")
        );
        assert_eq!(
            RadrootsFoodAvailabilityStatusFilterV1::from(FoodAvailabilityStatus::Sold)
                .storage_value(),
            Some("sold")
        );
    }

    #[test]
    fn stored_image_derives_qualification_from_typed_diagnostics() {
        let qualified = RadrootsStoredFoodAvailabilityImageV1 {
            image_index: 0,
            raw_tag: vec![
                "image".to_owned(),
                "https://example.test/image.webp".to_owned(),
            ],
            url: Some("https://example.test/image.webp".to_owned()),
            dimensions: None,
            blossom_sha256: None,
            diagnostics: Vec::new(),
        };
        let diagnosed = RadrootsStoredFoodAvailabilityImageV1 {
            diagnostics: vec![RadrootsFoodAvailabilityImageDiagnostic::DimensionsMissing],
            ..qualified.clone()
        };

        assert!(qualified.qualifies());
        assert!(!diagnosed.qualifies());
        assert_eq!(
            diagnosed.diagnostics(),
            &[RadrootsFoodAvailabilityImageDiagnostic::DimensionsMissing]
        );
        assert_eq!(qualified.image_index(), 0);
        assert_eq!(qualified.raw_tag()[0], "image");
        assert_eq!(qualified.url(), Some("https://example.test/image.webp"));
        assert_eq!(qualified.dimensions(), None);
        assert_eq!(qualified.blossom_sha256(), None);
    }

    #[test]
    fn projection_wide_count_diagnostic_requires_the_bounded_image_count() {
        assert!(matches!(
            validate_projection_diagnostics(
                &[RadrootsFoodAvailabilityImageDiagnostic::CountExceeded],
                &[]
            ),
            Err(RadrootsEventStoreError::FoodAvailabilityProjectionDrift { .. })
        ));
    }

    #[test]
    fn projection_diagnostics_reject_drift_and_preserve_order() {
        let image = |index, diagnostics| RadrootsStoredFoodAvailabilityImageV1 {
            image_index: index,
            raw_tag: vec!["image".to_owned()],
            url: None,
            dimensions: None,
            blossom_sha256: None,
            diagnostics,
        };

        let image_level_count = image(
            0,
            vec![RadrootsFoodAvailabilityImageDiagnostic::CountExceeded],
        );
        assert!(validate_projection_diagnostics(&[], &[image_level_count]).is_err());

        let diagnosed = image(
            0,
            vec![RadrootsFoodAvailabilityImageDiagnostic::DimensionsMissing],
        );
        assert!(validate_projection_diagnostics(&[], std::slice::from_ref(&diagnosed)).is_err());
        assert!(
            validate_projection_diagnostics(
                &[RadrootsFoodAvailabilityImageDiagnostic::DimensionsMissing],
                &[diagnosed],
            )
            .is_ok()
        );

        let retained = (0..RADROOTS_FOOD_IMAGE_MAX_COUNT)
            .map(|index| image(index as u32, Vec::new()))
            .collect::<Vec<_>>();
        assert!(
            validate_projection_diagnostics(
                &[RadrootsFoodAvailabilityImageDiagnostic::CountExceeded],
                &retained,
            )
            .is_ok()
        );
    }
}

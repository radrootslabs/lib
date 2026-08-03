//! Validated forward and reverse locality queries.

use crate::model::Country;
use crate::{Candidate, Error, Point};

const DEFAULT_LIMIT: usize = 10;
const DEFAULT_COUNTRY_LIMIT: usize = 300;
const DEFAULT_REVERSE_RADIUS_DEGREES: f64 = 0.5;
const MAX_LIMIT: usize = 1_000;
const MAX_REVERSE_RADIUS_DEGREES: f64 = 10.0;

/// A validated GeoNames lookup request.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub(crate) kind: QueryKind,
    limit: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum QueryKind {
    Locality {
        locality: String,
        region: Option<String>,
        country: Option<String>,
    },
    Freeform(String),
    FeatureId(i64),
    Reverse {
        point: Point,
        radius_degrees: f64,
    },
    Countries,
}

/// Results from one [`Query`], with provider-owned storage kept private.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryResult {
    kind: QueryResultKind,
}

#[derive(Clone, Debug, PartialEq)]
enum QueryResultKind {
    Candidates(Vec<Candidate>),
    Countries(Vec<Country>),
}

impl QueryResult {
    pub(crate) fn candidates(candidates: Vec<Candidate>) -> Self {
        Self {
            kind: QueryResultKind::Candidates(candidates),
        }
    }

    pub(crate) fn countries(countries: Vec<Country>) -> Self {
        Self {
            kind: QueryResultKind::Countries(countries),
        }
    }

    /// Returns locality candidates, or `None` for a country-list result.
    #[must_use]
    pub fn as_candidates(&self) -> Option<&[Candidate]> {
        match &self.kind {
            QueryResultKind::Candidates(candidates) => Some(candidates),
            QueryResultKind::Countries(_) => None,
        }
    }

    /// Returns countries, or `None` for a locality result.
    #[must_use]
    pub fn as_countries(&self) -> Option<&[Country]> {
        match &self.kind {
            QueryResultKind::Candidates(_) => None,
            QueryResultKind::Countries(countries) => Some(countries),
        }
    }
}

impl Query {
    /// Creates a structured locality query.
    pub fn locality(locality: impl Into<String>) -> Result<Self, Error> {
        Ok(Self {
            kind: QueryKind::Locality {
                locality: normalized_query_text(locality)?,
                region: None,
                country: None,
            },
            limit: DEFAULT_LIMIT,
        })
    }

    /// Creates a free-form locality query.
    pub fn freeform(query: impl Into<String>) -> Result<Self, Error> {
        Ok(Self {
            kind: QueryKind::Freeform(normalized_query_text(query)?),
            limit: DEFAULT_LIMIT,
        })
    }

    /// Creates an exact GeoNames feature query.
    pub fn feature_id(feature_id: u64) -> Result<Self, Error> {
        Ok(Self {
            kind: QueryKind::FeatureId(
                i64::try_from(feature_id).map_err(|_| Error::InvalidFeatureId)?,
            ),
            limit: 1,
        })
    }

    /// Creates a nearest-locality query around an explicit point.
    #[must_use]
    pub const fn reverse(point: Point) -> Self {
        Self {
            kind: QueryKind::Reverse {
                point,
                radius_degrees: DEFAULT_REVERSE_RADIUS_DEGREES,
            },
            limit: 1,
        }
    }

    /// Creates a deterministic country-list query.
    #[must_use]
    pub const fn countries() -> Self {
        Self {
            kind: QueryKind::Countries,
            limit: DEFAULT_COUNTRY_LIMIT,
        }
    }

    /// Narrows a structured locality query by administrative region.
    pub fn with_region(mut self, region: impl Into<String>) -> Result<Self, Error> {
        let QueryKind::Locality {
            region: current, ..
        } = &mut self.kind
        else {
            return Err(Error::QueryOptionNotApplicable);
        };
        *current = Some(normalized_query_text(region)?);
        Ok(self)
    }

    /// Narrows a structured locality query by country identifier or name.
    pub fn with_country(mut self, country: impl Into<String>) -> Result<Self, Error> {
        let QueryKind::Locality {
            country: current, ..
        } = &mut self.kind
        else {
            return Err(Error::QueryOptionNotApplicable);
        };
        *current = Some(normalized_query_text(country)?);
        Ok(self)
    }

    /// Sets the maximum result count.
    pub fn with_limit(mut self, limit: usize) -> Result<Self, Error> {
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(Error::InvalidQueryLimit);
        }
        self.limit = limit;
        Ok(self)
    }

    /// Sets the square prefilter radius for a reverse query.
    pub fn with_radius_degrees(mut self, radius_degrees: f64) -> Result<Self, Error> {
        if !radius_degrees.is_finite()
            || !(0.0..=MAX_REVERSE_RADIUS_DEGREES).contains(&radius_degrees)
            || radius_degrees == 0.0
        {
            return Err(Error::InvalidQueryRadius);
        }
        let QueryKind::Reverse {
            radius_degrees: current,
            ..
        } = &mut self.kind
        else {
            return Err(Error::QueryOptionNotApplicable);
        };
        *current = radius_degrees;
        Ok(self)
    }

    /// Returns the maximum result count.
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Returns structured locality fields when this is a locality query.
    #[must_use]
    pub fn locality_fields(&self) -> Option<(&str, Option<&str>, Option<&str>)> {
        match &self.kind {
            QueryKind::Locality {
                locality,
                region,
                country,
            } => Some((locality, region.as_deref(), country.as_deref())),
            _ => None,
        }
    }

    /// Returns free-form text when this is a free-form query.
    #[must_use]
    pub fn freeform_text(&self) -> Option<&str> {
        match &self.kind {
            QueryKind::Freeform(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the feature identifier when this is an exact feature query.
    #[must_use]
    pub fn exact_feature_id(&self) -> Option<u64> {
        match &self.kind {
            QueryKind::FeatureId(value) => u64::try_from(*value).ok(),
            _ => None,
        }
    }

    /// Returns the point when this is a reverse query.
    #[must_use]
    pub fn reverse_point(&self) -> Option<Point> {
        match &self.kind {
            QueryKind::Reverse { point, .. } => Some(*point),
            _ => None,
        }
    }

    /// Returns whether this query requests the country list.
    #[must_use]
    pub fn is_country_list(&self) -> bool {
        matches!(&self.kind, QueryKind::Countries)
    }
}

fn normalized_query_text(value: impl Into<String>) -> Result<String, Error> {
    let value = value.into();
    if value.is_empty() || value.trim() != value {
        return Err(Error::InvalidQueryText);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{Query, QueryKind};
    use crate::{Error, Point};

    #[test]
    fn structured_queries_keep_normalized_filters_private() {
        let query = Query::locality("Victoria")
            .expect("locality")
            .with_region("British Columbia")
            .expect("region")
            .with_country("CA")
            .expect("country")
            .with_limit(7)
            .expect("limit");
        assert_eq!(query.limit(), 7);
        assert_eq!(
            query.kind,
            QueryKind::Locality {
                locality: "Victoria".to_owned(),
                region: Some("British Columbia".to_owned()),
                country: Some("CA".to_owned()),
            }
        );
    }

    #[test]
    fn query_constructors_reject_ambiguous_or_unbounded_input() {
        assert_eq!(Query::locality(""), Err(Error::InvalidQueryText));
        assert_eq!(Query::freeform(" Victoria "), Err(Error::InvalidQueryText));
        assert_eq!(
            Query::feature_id(1).and_then(|query| query.with_country("CA")),
            Err(Error::QueryOptionNotApplicable)
        );
        assert_eq!(
            Query::countries().with_limit(0),
            Err(Error::InvalidQueryLimit)
        );
        assert_eq!(
            Query::countries().with_limit(1_001),
            Err(Error::InvalidQueryLimit)
        );
        assert_eq!(Query::feature_id(u64::MAX), Err(Error::InvalidFeatureId));
    }

    #[test]
    fn exact_reverse_and_country_queries_have_bounded_defaults() {
        let point = Point::new(48.4284, -123.3656).expect("point");
        assert_eq!(Query::feature_id(6_174_041).expect("feature").limit(), 1);
        assert_eq!(Query::reverse(point).limit(), 1);
        assert_eq!(Query::countries().limit(), 300);
        assert_eq!(
            Query::reverse(point).with_radius_degrees(0.0),
            Err(Error::InvalidQueryRadius)
        );
        assert_eq!(
            Query::locality("Victoria")
                .expect("locality")
                .with_radius_degrees(1.0),
            Err(Error::QueryOptionNotApplicable)
        );
    }
}

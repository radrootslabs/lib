//! Validated forward and reverse locality queries.

use crate::{Error, Point};

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 100;

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
    FeatureId(u64),
    Reverse(Point),
    Countries,
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
    #[must_use]
    pub const fn feature_id(feature_id: u64) -> Self {
        Self {
            kind: QueryKind::FeatureId(feature_id),
            limit: 1,
        }
    }

    /// Creates a nearest-locality query around an explicit point.
    #[must_use]
    pub const fn reverse(point: Point) -> Self {
        Self {
            kind: QueryKind::Reverse(point),
            limit: 1,
        }
    }

    /// Creates a deterministic country-list query.
    #[must_use]
    pub const fn countries() -> Self {
        Self {
            kind: QueryKind::Countries,
            limit: DEFAULT_LIMIT,
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
            QueryKind::FeatureId(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the point when this is a reverse query.
    #[must_use]
    pub fn reverse_point(&self) -> Option<Point> {
        match &self.kind {
            QueryKind::Reverse(value) => Some(*value),
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
            Query::feature_id(1).with_country("CA"),
            Err(Error::QueryOptionNotApplicable)
        );
        assert_eq!(
            Query::countries().with_limit(0),
            Err(Error::InvalidQueryLimit)
        );
        assert_eq!(
            Query::countries().with_limit(101),
            Err(Error::InvalidQueryLimit)
        );
    }

    #[test]
    fn exact_reverse_and_country_queries_have_bounded_defaults() {
        let point = Point::new(48.4284, -123.3656).expect("point");
        assert_eq!(Query::feature_id(617_4041).limit(), 1);
        assert_eq!(Query::reverse(point).limit(), 1);
        assert_eq!(Query::countries().limit(), 10);
    }
}

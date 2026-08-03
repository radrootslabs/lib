//! Provider-owned locality result models.

use crate::Error;

/// A geographic point in decimal degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    latitude: f64,
    longitude: f64,
}

impl Point {
    /// Creates a finite point within the WGS84 latitude/longitude bounds.
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, Error> {
        if !latitude.is_finite()
            || !longitude.is_finite()
            || !(-90.0..=90.0).contains(&latitude)
            || !(-180.0..=180.0).contains(&longitude)
        {
            return Err(Error::InvalidPoint);
        }
        Ok(Self {
            latitude,
            longitude,
        })
    }

    /// Returns the latitude in decimal degrees.
    #[must_use]
    pub const fn latitude(self) -> f64 {
        self.latitude
    }

    /// Returns the longitude in decimal degrees.
    #[must_use]
    pub const fn longitude(self) -> f64 {
        self.longitude
    }
}

/// One deterministic locality candidate returned by GeoNames.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    feature_id: u64,
    name: String,
    admin1_id: Option<String>,
    admin1_name: Option<String>,
    country_id: String,
    country_name: Option<String>,
    point: Point,
    display_name: String,
}

impl Candidate {
    /// Returns the stable GeoNames feature identifier.
    #[must_use]
    pub const fn feature_id(&self) -> u64 {
        self.feature_id
    }

    /// Returns the canonical locality name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the opaque first-level administrative identifier.
    #[must_use]
    pub fn admin1_id(&self) -> Option<&str> {
        self.admin1_id.as_deref()
    }

    /// Returns the first-level administrative name when present.
    #[must_use]
    pub fn admin1_name(&self) -> Option<&str> {
        self.admin1_name.as_deref()
    }

    /// Returns the ISO-like country identifier stored by the asset.
    #[must_use]
    pub fn country_id(&self) -> &str {
        &self.country_id
    }

    /// Returns the country name when present.
    #[must_use]
    pub fn country_name(&self) -> Option<&str> {
        self.country_name.as_deref()
    }

    /// Returns the candidate coordinate.
    #[must_use]
    pub const fn point(&self) -> Point {
        self.point
    }

    /// Returns the deterministic human-readable label.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

#[cfg(test)]
mod tests {
    use super::{Candidate, Point};
    use crate::Error;

    #[test]
    fn points_enforce_finite_geographic_bounds() {
        assert_eq!(
            Point::new(48.4284, -123.3656),
            Ok(Point {
                latitude: 48.4284,
                longitude: -123.3656,
            })
        );
        for (latitude, longitude) in [
            (f64::NAN, 0.0),
            (0.0, f64::INFINITY),
            (-90.1, 0.0),
            (90.1, 0.0),
            (0.0, -180.1),
            (0.0, 180.1),
        ] {
            assert_eq!(Point::new(latitude, longitude), Err(Error::InvalidPoint));
        }
    }

    #[test]
    fn candidates_expose_provider_values_without_public_fields() {
        let point = Point::new(48.4284, -123.3656).expect("valid point");
        let candidate = Candidate {
            feature_id: 617_4041,
            name: "Victoria".to_owned(),
            admin1_id: Some("BC".to_owned()),
            admin1_name: Some("British Columbia".to_owned()),
            country_id: "CA".to_owned(),
            country_name: Some("Canada".to_owned()),
            point,
            display_name: "Victoria, British Columbia, Canada".to_owned(),
        };
        assert_eq!(candidate.feature_id(), 617_4041);
        assert_eq!(candidate.name(), "Victoria");
        assert_eq!(candidate.admin1_id(), Some("BC"));
        assert_eq!(candidate.admin1_name(), Some("British Columbia"));
        assert_eq!(candidate.country_id(), "CA");
        assert_eq!(candidate.country_name(), Some("Canada"));
        assert_eq!(candidate.point(), point);
        assert_eq!(
            candidate.display_name(),
            "Victoria, British Columbia, Canada"
        );
    }
}

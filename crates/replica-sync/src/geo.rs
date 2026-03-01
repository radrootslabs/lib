#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::farm::{RadrootsGeoJsonPoint, RadrootsGeoJsonPolygon};

const EARTH_RADIUS_M: f64 = 6_378_137.0;

pub fn geojson_point_from_lat_lng(lat: f64, lng: f64) -> RadrootsGeoJsonPoint {
    RadrootsGeoJsonPoint {
        r#type: String::from("Point"),
        coordinates: [lng, lat],
    }
}

pub fn geojson_polygon_circle_wgs84(
    lat: f64,
    lng: f64,
    radius_m: f64,
    steps: usize,
) -> RadrootsGeoJsonPolygon {
    let steps = if steps < 3 { 3 } else { steps };
    let lat1 = lat.to_radians();
    let lng1 = lng.to_radians();
    let angular = radius_m / EARTH_RADIUS_M;
    let sin_lat1 = lat1.sin();
    let cos_lat1 = lat1.cos();
    let sin_ang = angular.sin();
    let cos_ang = angular.cos();

    let mut ring = Vec::with_capacity(steps + 1);
    for idx in 0..=steps {
        let bearing = (idx as f64) * core::f64::consts::TAU / (steps as f64);
        let sin_bearing = bearing.sin();
        let cos_bearing = bearing.cos();

        let sin_lat2 = sin_lat1 * cos_ang + cos_lat1 * sin_ang * cos_bearing;
        let lat2 = sin_lat2.asin();
        let y = sin_bearing * sin_ang * cos_lat1;
        let x = cos_ang - sin_lat1 * sin_lat2;
        let lng2 = lng1 + y.atan2(x);

        let lat_deg = round_coord(lat2.to_degrees());
        let lng_deg = round_coord(normalize_lng(lng2.to_degrees()));
        ring.push([lng_deg, lat_deg]);
    }

    RadrootsGeoJsonPolygon {
        r#type: String::from("Polygon"),
        coordinates: vec![ring],
    }
}

fn round_coord(value: f64) -> f64 {
    let scale = 1_000_000.0;
    (value * scale).round() / scale
}

fn normalize_lng(value: f64) -> f64 {
    let mut lng = value;
    while lng > 180.0 {
        lng -= 360.0;
    }
    while lng < -180.0 {
        lng += 360.0;
    }
    lng
}

#[cfg(test)]
mod tests {
    use super::{geojson_point_from_lat_lng, geojson_polygon_circle_wgs84};

    #[test]
    fn point_uses_lng_lat_coordinate_order() {
        let point = geojson_point_from_lat_lng(37.7, -122.4);
        assert_eq!(point.r#type, "Point");
        assert_eq!(point.coordinates, [-122.4, 37.7]);
    }

    #[test]
    fn polygon_enforces_minimum_steps_and_closed_ring() {
        let polygon = geojson_polygon_circle_wgs84(37.7, -122.4, 100.0, 1);
        assert_eq!(polygon.r#type, "Polygon");
        assert_eq!(polygon.coordinates.len(), 1);
        let ring = &polygon.coordinates[0];
        assert_eq!(ring.len(), 4);
        assert_eq!(ring.first(), ring.last());
    }

    #[test]
    fn polygon_normalizes_longitudes_into_wgs84_range() {
        let positive = geojson_polygon_circle_wgs84(0.0, 540.0, 10.0, 8);
        for point in &positive.coordinates[0] {
            assert!(point[0] <= 180.0);
            assert!(point[0] >= -180.0);
        }

        let negative = geojson_polygon_circle_wgs84(0.0, -540.0, 10.0, 8);
        for point in &negative.coordinates[0] {
            assert!(point[0] <= 180.0);
            assert!(point[0] >= -180.0);
        }
    }
}

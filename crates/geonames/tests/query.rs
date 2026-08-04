use radroots_geonames::{Point, Query};

#[test]
fn packaged_query_construction_remains_validated_and_inert() {
    let locality = Query::locality("Victoria")
        .expect("locality")
        .with_region("BC")
        .expect("region")
        .with_country("CA")
        .expect("country")
        .with_limit(5)
        .expect("limit");
    assert_eq!(locality.limit(), 5);

    let reverse = Query::reverse(Point::new(48.4284, -123.3656).expect("point"))
        .with_radius_degrees(0.25)
        .expect("radius");
    assert_eq!(reverse.limit(), 1);
}

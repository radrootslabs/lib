use radroots_geonames::asset::official_asset_spec;
use radroots_geonames::{Point, Query};

fn main() -> Result<(), radroots_geonames::Error> {
    let spec = official_asset_spec();
    let locality = Query::locality("Victoria")?
        .with_region("BC")?
        .with_country("CA")?
        .with_limit(5)?;
    let reverse = Query::reverse(Point::new(48.4284, -123.3656)?).with_radius_degrees(0.25)?;

    println!(
        "prepared asset {} and query limits {}/{} without I/O",
        spec.version(),
        locality.limit(),
        reverse.limit()
    );
    Ok(())
}

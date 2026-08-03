//! Explicit GeoNames database lifecycle.

/// An opened, verified GeoNames database.
///
/// Construction is introduced with the explicit database lifecycle checkpoint;
/// this type performs no work merely by being linked or imported.
#[derive(Debug)]
pub struct Geocoder {
    _private: (),
}

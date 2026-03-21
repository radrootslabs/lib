use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeocoderError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("country center not found for {country_id}")]
    CountryCenterNotFound { country_id: String },
}

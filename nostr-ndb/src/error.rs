use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsNostrNdbError {
    #[error("database path must be utf-8")]
    NonUtf8Path,

    #[error("nostrdb error: {0}")]
    Ndb(String),
}

#[cfg(feature = "ndb")]
impl From<nostrdb::Error> for RadrootsNostrNdbError {
    fn from(value: nostrdb::Error) -> Self {
        Self::Ndb(value.to_string())
    }
}
